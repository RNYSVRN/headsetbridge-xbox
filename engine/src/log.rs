//! The engine log: standard output (if any) and `%LOCALAPPDATA%\HeadsetBridge\engine\engine.log`,
//! because the shipped exe has no console window. The file is rotated to
//! `engine.log.1` above 2 MiB. Tokens and sign-in codes are never logged.
//!
//! The desktop app parses this log to show the connection state, so the
//! wording of the lines it looks for (see `HeadsetBridge.Core/LogParser.cs`)
//! is part of the interface between the two.
//!
//! A dedicated thread writes to disk: callers (the single tokio thread with the
//! live session, the audio thread) only queue the line, so a slow
//! `create`/`write` (antivirus, NTFS) never blocks them.

use std::io::Write;
use std::sync::mpsc::{channel, Sender};
use std::sync::OnceLock;
use std::time::Instant;

const MAX_SIZE: u64 = 2 * 1024 * 1024;

enum Command { Line(String), Flush(Sender<()>) }

static WRITER: OnceLock<Sender<Command>> = OnceLock::new();

fn writer() -> &'static Sender<Command> {
    WRITER.get_or_init(|| {
        let (tx, rx) = channel::<Command>();
        std::thread::Builder::new().name("log".into()).spawn(move || {
            let path = crate::paths::log_file();
            if let Some(dir) = path.parent() { let _ = std::fs::create_dir_all(dir); }
            let open = || std::fs::OpenOptions::new().create(true).append(true).open(&path).ok();
            let mut file = open();
            let mut size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            for command in rx {
                match command {
                    Command::Line(line) => {
                        if size > MAX_SIZE {
                            drop(file.take()); // Windows cannot rename an open file.
                            let _ = std::fs::rename(&path, path.with_extension("log.1"));
                            file = open();
                            size = 0;
                        }
                        if file.is_none() { file = open() }
                        if let Some(f) = file.as_mut() {
                            if writeln!(f, "{line}").is_ok() { size += line.len() as u64 + 1 } else { file = None }
                        }
                    }
                    Command::Flush(done) => { let _ = done.send(()); }
                }
            }
        }).expect("log thread");
        tx
    })
}

/// Logs `text` with the milliseconds since `t0`: `[  1234 ms] text`.
pub fn log(t0: Instant, text: &str) {
    let line = format!("[{:>6} ms] {text}", t0.elapsed().as_millis());
    // Without a console (Windows subsystem) standard output discards silently.
    println!("{line}");
    let _ = writer().send(Command::Line(line));
}

/// Waits (up to 2 s) until everything queued is on disk: call before exiting.
pub fn flush() {
    let (tx, rx) = channel();
    if writer().send(Command::Flush(tx)).is_ok() { let _ = rx.recv_timeout(std::time::Duration::from_secs(2)); }
}

#[cfg(test)]
mod tests {
    #[test]
    fn log_does_not_wait_for_disk_and_flush_writes_everything() {
        let dir = std::env::temp_dir().join(format!("headsetbridge-log-{}", std::process::id()));
        std::env::set_var("LOCALAPPDATA", &dir);
        let t0 = std::time::Instant::now();
        for i in 0..1000 { super::log(t0, &format!("line {i}")) }
        super::flush();
        let text = std::fs::read_to_string(crate::paths::log_file()).unwrap();
        assert_eq!(text.lines().count(), 1000);
        assert!(text.lines().last().unwrap().ends_with("line 999"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
