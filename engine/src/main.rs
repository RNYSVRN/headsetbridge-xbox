//! `headsetbridge-engine.exe`: connects to the console (the first one that is on,
//! or `--console=<id>`) and plays its audio until asked to stop.
//!
//! Options:
//!   --lang=de|en     language of the sign-in dialog (default English)
//!   --console=<id>   use this console instead of the first one that is on
//!   --offer          print the redacted local SDP offer and exit (no account needed)
//!   --version        print the version and exit
//!
//! Only one instance runs at a time (named pipe `\\.\pipe\headsetbridge-engine`).
//! Starting a second instance, or writing `stop` to the pipe, stops the running
//! one; it closes the console session before exiting. The exe has no console
//! window: everything goes to the log (see log.rs and paths.rs).

#![cfg_attr(windows, windows_subsystem = "windows")]

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use headsetbridge_engine::audio::{watch_output, Opus, SharedBuffer, CALLBACK_NS, OPUS_NS, OUTPUT_LATEST_US, OUTPUT_MAX_US};
use headsetbridge_engine::jitter_buffer::JitterBuffer;
use headsetbridge_engine::log::log;
use headsetbridge_engine::retry::{wait, Wait, SLOW_INTERVAL};
use headsetbridge_engine::session::interval;
use headsetbridge_engine::stats::{End, Stats};
use headsetbridge_engine::xhome::{Credentials, XHome};
use headsetbridge_engine::{auth, offer, session};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

const PIPE: &str = r"\\.\pipe\headsetbridge-engine";
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Jitter buffer: 20 ms (one packet) target, growing up to 200 ms on late
/// packets and shrinking back after a calm minute.
const BUFFER_MS: u32 = 20;
const BUFFER_MAX_MS: u32 = 200;
const BUFFER_CALM_MS: f64 = 60_000.0;

fn option(name: &str) -> Option<String> {
    std::env::args().find_map(|a| a.strip_prefix(&format!("--{name}=")).map(str::to_owned))
}

fn flag(name: &str) -> bool { std::env::args().any(|a| a == format!("--{name}")) }

/// A single tokio thread is enough for one session (one per core doubled the network CPU).
fn main() -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    let r = runtime.block_on(run());
    // The log is written by its own thread: do not lose the last lines.
    headsetbridge_engine::log::flush();
    r
}

async fn run() -> Result<()> {
    if flag("version") {
        println!("headsetbridge-engine {VERSION}");
        return Ok(());
    }
    if flag("offer") {
        let o = offer::generate_offer().await?;
        println!("{}", offer::redact_sdp(&o.sdp));
        return Ok(());
    }
    if let Some(code) = option("lang") { headsetbridge_engine::lang::set(&code) }
    let requested_console = option("console");
    let t0 = Instant::now();
    // The first line of every start: the desktop app reads the log from here.
    log(t0, &format!("startup: headsetbridge-engine {VERSION}"));
    log(t0, "route: WebRTC");
    headsetbridge_engine::paths::migrate_legacy();

    // Single instance through a named pipe: if one is already running, ask it to
    // stop (it closes its session) and exit.
    let stop = Arc::new(tokio::sync::Notify::new());
    match ServerOptions::new().first_pipe_instance(true).create(PIPE) {
        Err(_) => {
            if let Ok(mut other) = ClientOptions::new().open(PIPE) { let _ = other.write_all(b"stop\n").await; }
            return Ok(());
        }
        Ok(mut server) => {
            let stop = stop.clone();
            tokio::spawn(async move {
                loop {
                    if server.connect().await.is_ok() {
                        let mut read = [0u8; 16];
                        let n = server.read(&mut read).await.unwrap_or(0);
                        if read[..n].starts_with(b"stop") {
                            log(t0, "stop requested");
                            stop.notify_one();
                        }
                    }
                    server = match ServerOptions::new().create(PIPE) { Ok(s) => s, Err(_) => break };
                }
            });
        }
    }

    // Audio: jitter buffer and shared-mode output, kept alive until the end.
    let now = t0.elapsed().as_secs_f64() * 1000.0;
    let buffer: SharedBuffer = Arc::new(std::sync::Mutex::new(JitterBuffer::new(Opus::new()?, BUFFER_MS, BUFFER_MS, BUFFER_MAX_MS, BUFFER_CALM_MS, now)));
    watch_output(buffer.clone(), t0);

    let stats = Arc::new(Stats::default());
    log_stats_every_10_s(stats.clone(), buffer.clone(), t0);

    // Retries (retry.rs): 1, 2, 4, 8 and 16 s, then every 30 s while waiting for
    // the console; an attempt that delivered audio resets the count. Every
    // attempt gets fresh credentials (the /connect token is short-lived).
    let mut failures = 0u32;
    // The xccs authorization and console of the last attempt, to check whether
    // the console is on while waiting (without new credentials every time).
    let mut to_check: Option<(String, Option<String>)> = None;
    loop {
        let packets_before = stats.packets.load(Ordering::Relaxed);
        let result: Result<End> = async {
            let credentials = credentials_on_own_thread(requested_console.is_none()).await?;
            let console = requested_console.clone().or_else(|| credentials.console.clone()).ok_or("no console found on this account")?;
            if let Some(x) = &credentials.xccs { to_check = Some((x.clone(), Some(console.clone()))) }
            log(t0, &format!("authenticated; region {}", credentials.region));
            if credentials.woken_up { log(t0, "the console was in standby: woken up remotely") }
            session::run(XHome::new(credentials)?, &console, t0, stats.clone(), Some(buffer.clone()), stop.clone()).await
        }.await;
        let reason = match result {
            Ok(End::Stopped) => break,
            Ok(End::Dropped(reason)) => reason,
            Err(e) => e.to_string(),
        };
        if stats.packets.load(Ordering::Relaxed) > packets_before { failures = 0 }
        failures += 1;
        if !wait_before_retry(failures, &reason, &stop, &to_check, t0).await { break }
    }
    log(t0, "stopped");
    Ok(())
}

/// The summary line every 10 s; the desktop app reads packets/s and the buffer from it.
fn log_stats_every_10_s(stats: Arc<Stats>, buffer: SharedBuffer, t0: Instant) {
    tokio::spawn(async move {
        let mut every = interval(Duration::from_secs(10));
        every.tick().await;
        let (mut packets, mut bytes) = (0, 0);
        loop {
            every.tick().await;
            let (p, b) = (stats.packets.load(Ordering::Relaxed), stats.bytes.load(Ordering::Relaxed));
            let state = buffer.lock().ok().map(|j| format!(", buffer {} ms (target {}), {:?}", j.pending_blocks() * 10, j.target_ms(), j.counters)).unwrap_or_default();
            let output = format!(", output {:.1} ms (max {:.1})", OUTPUT_LATEST_US.load(Ordering::Relaxed) as f64 / 1000.0, OUTPUT_MAX_US.swap(0, Ordering::Relaxed) as f64 / 1000.0);
            // % of one core over 10 s: ns / 1e10 * 100.
            let cpu = format!(", audio CPU {:.2} % (opus {:.2} %)", CALLBACK_NS.swap(0, Ordering::Relaxed) as f64 / 1e8, OPUS_NS.swap(0, Ordering::Relaxed) as f64 / 1e8);
            log(t0, &format!("audio: {} pkt/s, {:.1} kbps, CELT FB stereo {} of {}, other sources {}, redundant {}{state}{output}{cpu}",
                (p - packets) as f64 / 10.0, (b - bytes) as f64 * 8.0 / 10_000.0,
                stats.celt_fullband_stereo.load(Ordering::Relaxed), p, stats.other_sources.load(Ordering::Relaxed), stats.redundant.load(Ordering::Relaxed)));
            (packets, bytes) = (p, b);
        }
    });
}

/// The wait before attempt number `failures` (1 for the first after a failure):
/// 1-2-4-8-16 s, then — never giving up — check every 30 s whether the console
/// is on. `false`: a stop was requested.
async fn wait_before_retry(failures: u32, reason: &str, stop: &tokio::sync::Notify, to_check: &Option<(String, Option<String>)>, t0: Instant) -> bool {
    match wait(failures) {
        Wait::Quick(d) => {
            log(t0, &format!("reconnecting in {} s ({reason})", d.as_secs()));
            tokio::time::timeout(d, stop.notified()).await.is_err()
        }
        Wait::Slow => {
            log(t0, &format!("waiting for the console ({reason}): checking every {} s whether it is on", SLOW_INTERVAL.as_secs()));
            loop {
                if tokio::time::timeout(SLOW_INTERVAL, stop.notified()).await.is_ok() { return false }
                let Some((authorization, console)) = to_check else { return true };
                match auth::console_is_on(authorization, console.as_deref()).await {
                    Ok(true) => { log(t0, "waiting for the console: it is on; reconnecting"); return true }
                    Ok(false) => {}
                    // Network down or token expired: try the whole thing (fresh credentials).
                    Err(e) => { log(t0, &format!("waiting for the console: could not check ({e}); trying anyway")); return true }
                }
            }
        }
    }
}

/// `auth::authenticate` on its own thread with its own runtime. Sign-in does
/// blocking work (lock file waits, files); on the single tokio thread it would
/// stall the live session.
async fn credentials_on_own_thread(pick_console: bool) -> Result<Credentials> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let r = tokio::runtime::Builder::new_current_thread().enable_all().build()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            .and_then(|rt| rt.block_on(auth::authenticate(pick_console)));
        let _ = tx.send(r);
    });
    rx.await.map_err(|_| "the sign-in thread ended without an answer")?
}
