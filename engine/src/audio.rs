//! Playback: Opus (libopus via the `opus` crate) → `JitterBuffer` → the Windows
//! default output with cpal (WASAPI shared mode, so other apps keep playing on
//! the same headphones).
//!
//! The sound card asks for audio whenever it wants; for every 10 ms it consumes,
//! one block is taken from the jitter buffer, which decides whether to stretch,
//! trim or stay silent. Without a block, that slot plays silence.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::jitter_buffer::{Decoder, JitterBuffer};
use crate::log::log;

const BLOCK_SAMPLES: usize = 480 * 2; // 10 ms stereo at 48 kHz

/// How long until what is written to the sound card is heard (cpal's
/// `playback - callback`: audio already queued in WASAPI), in µs: the latest
/// value and the maximum since the last read. Does not include the latency of
/// the device itself (USB, DAC).
pub static OUTPUT_LATEST_US: AtomicU64 = AtomicU64::new(0);
pub static OUTPUT_MAX_US: AtomicU64 = AtomicU64::new(0);

/// CPU time of the audio path, in accumulated ns: the whole sound card callback
/// (buffer + decoding + copy) and, within it, libopus alone.
pub static CALLBACK_NS: AtomicU64 = AtomicU64::new(0);
pub static OPUS_NS: AtomicU64 = AtomicU64::new(0);

/// libopus. `None` = 20 ms of PLC (packet lost or not here in time).
pub struct Opus(opus::Decoder);

impl Opus {
    pub fn new() -> Result<Self, opus::Error> { Ok(Self(opus::Decoder::new(48_000, opus::Channels::Stereo)?)) }
}

impl Decoder for Opus {
    fn decode(&mut self, payload: Option<&[u8]>) -> Vec<i16> {
        let start = Instant::now();
        let mut pcm = vec![0i16; 960 * 2];
        // With an empty packet libopus conceals as much as asked: 960 samples.
        let samples = self.0.decode(payload.unwrap_or(&[]), &mut pcm, false).unwrap_or(0);
        pcm.truncate(samples * 2);
        pcm.resize(960 * 2, 0);
        OPUS_NS.fetch_add(start.elapsed().as_nanos() as u64, Ordering::Relaxed);
        pcm
    }
}

pub type SharedBuffer = Arc<Mutex<JitterBuffer<Opus>>>;

/// Fills what the sound card asks for (interleaved stereo f32) with 10 ms blocks
/// from the jitter buffer. When the buffer runs dry it does not cut hard: it
/// repeats the last block while fading out over 10 ms, and fades back in over
/// another 10 ms when audio returns (no clicks).
#[derive(Default)]
pub struct Filler { current: Vec<i16>, position: usize, current_is_audio: bool, gain: f32, target: f32 }

impl Filler {
    pub fn fill<D: Decoder>(&mut self, data: &mut [f32], buffer: &Mutex<JitterBuffer<D>>, t0: Instant) {
        const STEP: f32 = 1.0 / BLOCK_SAMPLES as f32;
        let mut first = true;
        for sample in data.iter_mut() {
            if self.position >= self.current.len() {
                // Time for another 10 ms block. Further blocks in the same
                // callback are a burst of the sound card, not of the network.
                let now = t0.elapsed().as_secs_f64() * 1000.0;
                let block = buffer.lock().map(|mut b| b.consume_block_in_callback(now, !first).0).unwrap_or(None);
                first = false;
                match block {
                    Some(b) => { self.target = 1.0; self.current = b; self.current_is_audio = true }
                    None => {
                        self.target = 0.0;
                        // Keep repeating the last audio block while it fades out; then zeros.
                        if !(self.gain > 0.0 && self.current_is_audio) {
                            self.current.clear();
                            self.current.resize(BLOCK_SAMPLES, 0);
                            self.current_is_audio = false;
                        }
                    }
                }
                self.position = 0;
            }
            if self.gain < self.target { self.gain = (self.gain + STEP).min(self.target) }
            else if self.gain > self.target { self.gain = (self.gain - STEP).max(self.target) }
            *sample = self.current[self.position] as f32 / 32768.0 * self.gain;
            self.position += 1;
        }
    }
}

/// Should the output be reopened? `current`: the name of the open device (None:
/// none open). Reopen when it failed or when the Windows default device is now
/// another one (e.g. a USB headset was plugged back in).
pub fn should_reopen(failed: bool, current: Option<&str>, default: Option<&str>) -> bool {
    let Some(current) = current else { return true };
    failed || default.is_some_and(|d| d != current)
}

/// Keeps the output open on its own thread (a cpal stream cannot move between
/// threads) and follows the Windows default device. Checks a flag every 2 s.
pub fn watch_output(buffer: SharedBuffer, t0: Instant) {
    std::thread::spawn(move || {
        let failed = Arc::new(AtomicBool::new(false));
        let mut current: Option<(cpal::Stream, String)> = None;
        loop {
            let default = if current.is_some() { cpal::default_host().default_output_device().and_then(|d| d.name().ok()) } else { None };
            if should_reopen(failed.load(Ordering::Relaxed), current.as_ref().map(|(_, n)| n.as_str()), default.as_deref()) {
                if let (Some((_, before)), Some(now)) = (&current, &default) {
                    log(t0, &format!("audio output: the Windows default changed from \"{before}\" to \"{now}\"; reopening"));
                }
                current = None;
                failed.store(false, Ordering::Relaxed);
                match open_output(buffer.clone(), t0, failed.clone()) {
                    Ok((stream, name)) => {
                        log(t0, &format!("audio output: {name} (shared mode)"));
                        current = Some((stream, name));
                    }
                    Err(e) => log(t0, &format!("audio output: not available yet: {e}")),
                }
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    });
}

/// Opens the Windows default output and returns the stream, which must be kept
/// alive while playing, and the device name.
pub fn open_output(buffer: SharedBuffer, t0: Instant, failed: Arc<AtomicBool>) -> Result<(cpal::Stream, String), Box<dyn std::error::Error + Send + Sync>> {
    let device = cpal::default_host().default_output_device().ok_or("no audio output device")?;
    let name = device.name().unwrap_or_default();
    // Opus decodes at 48 kHz; Windows shared mode resamples if the device runs at another rate.
    let config = cpal::StreamConfig { channels: 2, sample_rate: cpal::SampleRate(48_000), buffer_size: cpal::BufferSize::Default };

    let mut filler = Filler::default();
    // Diagnostics: how much the sound card asks for per callback (the WASAPI period).
    let mut seen_sizes: Vec<usize> = Vec::new();
    let stream = device.build_output_stream(
        &config,
        move |data: &mut [f32], info: &cpal::OutputCallbackInfo| {
            let start = Instant::now();
            let stamp = info.timestamp();
            if let Some(wait) = stamp.playback.duration_since(&stamp.callback) {
                let us = wait.as_micros() as u64;
                OUTPUT_LATEST_US.store(us, Ordering::Relaxed);
                OUTPUT_MAX_US.fetch_max(us, Ordering::Relaxed);
            }
            let size = data.len() / 2;
            if seen_sizes.len() < 8 && !seen_sizes.contains(&size) {
                seen_sizes.push(size);
                log(t0, &format!("audio output: the device asks for {size} samples ({:.1} ms) per callback", size as f64 / 48.0));
            }
            filler.fill(data, &buffer, t0);
            CALLBACK_NS.fetch_add(start.elapsed().as_nanos() as u64, Ordering::Relaxed);
        },
        move |error| {
            log(t0, &format!("audio output: error: {error}"));
            failed.store(true, Ordering::Relaxed);
        },
        None,
    )?;
    stream.play()?;
    Ok((stream, name))
}

#[cfg(test)]
mod tests {
    #[test]
    fn reopen_follows_the_windows_default() {
        use super::should_reopen as r;
        assert!(r(false, None, None), "nothing open yet");
        assert!(r(true, Some("Speakers"), Some("Speakers")), "the device failed");
        assert!(r(false, Some("Speakers"), Some("Headset (USB)")), "the default changed");
        assert!(!r(false, Some("Speakers"), Some("Speakers")));
        assert!(!r(false, Some("Speakers"), None));
    }

    /// When the buffer runs dry the output fades out instead of cutting hard.
    #[test]
    fn fades_out_when_the_buffer_runs_dry() {
        use std::sync::{Arc, Mutex};
        use crate::jitter_buffer::{Decoder, JitterBuffer};
        struct Loud;
        impl Decoder for Loud { fn decode(&mut self, _: Option<&[u8]>) -> Vec<i16> { vec![16384; 960 * 2] } }
        let buffer = Arc::new(Mutex::new(JitterBuffer::new(Loud, 20, 20, 20, 60_000.0, 0.0)));
        buffer.lock().unwrap().receive(0, vec![1], 0.0);
        let mut filler = super::Filler::default();
        let t0 = std::time::Instant::now();
        // 2 real blocks + 5 concealed packets (10 blocks), then the buffer is dry.
        let mut data = vec![0f32; super::BLOCK_SAMPLES * 16];
        filler.fill(&mut data, &buffer, t0);
        let last = *data.last().unwrap();
        assert!(last.abs() < 0.01, "silent at the end, got {last}");
        assert!(data.iter().any(|s| *s > 0.2), "audio at the start");
    }
}
