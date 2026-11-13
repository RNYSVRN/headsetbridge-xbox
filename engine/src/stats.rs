//! Counters of the received audio and how a session ended.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use crate::log::log;

/// What is counted about the received audio; read every 10 s for the log.
#[derive(Default)]
pub struct Stats {
    pub packets: AtomicU64,
    pub bytes: AtomicU64,
    /// Packets that are Opus CELT full-band stereo (the expected mode).
    pub celt_fullband_stereo: AtomicU64,
    /// Packets from another audio source (another SSRC), not played.
    pub other_sources: AtomicU64,
    /// Redundant copies (RED) received.
    pub redundant: AtomicU64,
    /// Milliseconds (since t0) of the last /play; 0 = its first audio already arrived.
    pub play_ms: AtomicU64,
}

impl Stats {
    /// Called right before /play.
    pub fn mark_play(&self, t0: Instant) { self.play_ms.store(t0.elapsed().as_millis().max(1) as u64, Ordering::Relaxed) }

    /// Called for every audio packet: the first one after /play logs how long it took.
    pub fn audio(&self, t0: Instant) {
        let play = self.play_ms.swap(0, Ordering::Relaxed);
        if play > 0 { log(t0, &format!("first audio {} ms after /play", t0.elapsed().as_millis() as u64 - play)) }
    }
}

/// How a session ended: it was asked to stop, or it dropped and should be retried.
#[derive(Debug)]
pub enum End {
    Stopped,
    Dropped(String),
}
