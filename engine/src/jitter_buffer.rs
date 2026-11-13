//! The receive policy of the Node prototype: a jitter buffer for 20 ms Opus
//! packets that decides when to play, conceal (PLC), stretch or trim.
//! The decoder and the sound card are injected: this module never opens WASAPI.

use std::collections::{HashMap, VecDeque};

const MAX_DISTANCE: u16 = 50;
const MAX_CONCEALED_IN_A_ROW: u8 = 5;
const REPEAT_WINDOW_MS: f64 = 10_000.0;
const SEGMENT_MS: f64 = 5_000.0;
const NETWORK_WINDOW_MS: f64 = 60_000.0;
const BLOCK_SAMPLES: usize = 480 * 2; // 10 ms, stereo at 48 kHz.

pub trait Decoder {
    /// `None` requests 20 ms of PLC; the output is 960 samples per channel.
    fn decode(&mut self, payload: Option<&[u8]>) -> Vec<i16>;
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Counters {
    pub played: u64,
    pub concealed: u64,
    pub stretched: u64,
    pub trimmed: u64,
    pub late_arrivals: u64,
    pub too_late: u64,
    pub duplicates: u64,
    pub underruns: u64,
    /// Packets decoded on arrival (network thread) instead of in the sound card callback.
    pub decoded_early: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmissionKind {
    Real(u16),
    PlcLoss(u16),
    PlcStretch(u16),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Emission {
    pub kind: EmissionKind,
    pub first_sample: i16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Resolved { Real, Concealed }

#[derive(Clone, Copy, Debug)]
struct Segment {
    start: f64,
    t0: f64,
    seq0: u16,
    min: f64,
    max: f64,
}

pub struct JitterBuffer<D: Decoder> {
    decoder: D,
    /// The PCM of packet `next`, decoded on arrival.
    predecoded: Option<(u16, Vec<i16>)>,
    packets: HashMap<u16, Vec<u8>>,
    resolved: HashMap<u16, Resolved>,
    resolved_order: VecDeque<u16>,
    segments: VecDeque<Segment>,
    blocks: VecDeque<Vec<i16>>,
    pub next: Option<u16>,
    pub playing: bool,
    pub counters: Counters,
    in_burst: bool,
    buffer_ms: u32,
    min_ms: u32,
    max_ms: u32,
    calm_ms: f64,
    concealed_in_a_row: u8,
    last_late_ms: f64,
    last_underrun_ms: f64,
}

impl<D: Decoder> JitterBuffer<D> {
    pub fn new(decoder: D, buffer_ms: u32, min_ms: u32, max_ms: u32, calm_ms: f64, now_ms: f64) -> Self {
        Self {
            decoder, predecoded: None, packets: HashMap::new(), resolved: HashMap::new(),
            resolved_order: VecDeque::new(), segments: VecDeque::new(),
            blocks: VecDeque::new(), next: None, playing: false,
            counters: Counters::default(), buffer_ms, min_ms, max_ms, calm_ms,
            concealed_in_a_row: 0, last_late_ms: f64::NEG_INFINITY,
            last_underrun_ms: now_ms, in_burst: false,
        }
    }

    /// How far ahead of `next` a packet may arrive without counting as a sequence
    /// jump: 50 packets plus those that fit in the target.
    fn max_distance(&self) -> u16 { MAX_DISTANCE.saturating_add((self.target_ms() / 20).min(1000) as u16) }

    /// What is buffered: decoded and still to decode (ms).
    pub fn level_ms(&self) -> usize { self.queue_ms() }

    pub fn target_ms(&self) -> u32 {
        self.buffer_ms.clamp(self.min_ms, self.max_ms)
    }

    pub fn pending_blocks(&self) -> usize { self.blocks.len() }

    /// The spread of network delay over the last minute (ms).
    pub fn network_jitter_ms(&self) -> u32 {
        self.segments.iter().map(|s| s.max - s.min).fold(0.0, f64::max).round() as u32
    }

    fn remember(&mut self, seq: u16, kind: Resolved) {
        if !self.resolved.contains_key(&seq) { self.resolved_order.push_back(seq); }
        self.resolved.insert(seq, kind);
        if self.resolved.len() > 512 {
            if let Some(oldest) = self.resolved_order.pop_front() { self.resolved.remove(&oldest); }
        }
    }

    fn measure_arrival(&mut self, seq: u16, now_ms: f64) {
        if self.segments.back().is_none_or(|s| now_ms - s.start > SEGMENT_MS) {
            self.segments.push_back(Segment { start: now_ms, t0: now_ms, seq0: seq, min: 0.0, max: 0.0 });
            while self.segments.len() > 1 && now_ms - self.segments.front().unwrap().start > NETWORK_WINDOW_MS {
                self.segments.pop_front();
            }
            return;
        }
        let segment = self.segments.back_mut().unwrap();
        let distance = seq.wrapping_sub(segment.seq0) as i32;
        let distance = if distance > 32768 { distance - 65536 } else { distance };
        let delay = now_ms - (segment.t0 + f64::from(distance) * 20.0);
        segment.min = segment.min.min(delay);
        segment.max = segment.max.max(delay);
    }

    /// A packet arrived. `now_ms` is injected so the same 10 s window as Node can be verified.
    pub fn receive(&mut self, seq: u16, payload: Vec<u8>, now_ms: f64) -> Vec<Emission> {
        self.measure_arrival(seq, now_ms);
        if self.next.is_none() {
            self.next = Some(seq);
            self.last_underrun_ms = now_ms;
        }
        let distance = seq.wrapping_sub(self.next.unwrap());
        if distance > 32768 {
            match self.resolved.get(&seq) {
                Some(Resolved::Real) => { self.counters.duplicates += 1; return vec![]; }
                Some(Resolved::Concealed) => { self.counters.too_late += 1; return vec![]; }
                None if (0u16).wrapping_sub(distance) <= self.max_distance() => {
                    self.counters.too_late += 1; return vec![];
                }
                None => {}
            }
        }
        if self.packets.contains_key(&seq) { self.counters.duplicates += 1; return vec![]; }
        if distance > self.max_distance() {
            // A big jump, also backwards, restarts the sequence without a run of PLC.
            self.predecoded = None;
            self.packets.clear();
            self.resolved.clear();
            self.resolved_order.clear();
            self.segments.clear();
            self.next = Some(seq);
        }
        self.packets.insert(seq, payload);
        let emitted = self.refill(now_ms);
        self.predecode();
        emitted
    }

    /// libopus outside the sound card callback: if packet `next` is already
    /// here, decode it on arrival (network thread) and let `refill` use it as is.
    /// Same packet, same order for libopus, no PLC in between.
    fn predecode(&mut self) {
        if self.predecoded.is_some() { return }
        let Some(seq) = self.next else { return };
        let Some(payload) = self.packets.get(&seq) else { return };
        let pcm = self.decoder.decode(Some(payload));
        self.predecoded = Some((seq, pcm));
        self.counters.decoded_early += 1;
    }

    /// The PCM of `seq`: decoded on arrival, or now.
    fn decode(&mut self, seq: u16, payload: &[u8]) -> Vec<i16> {
        match self.predecoded.take() {
            Some((s, pcm)) if s == seq => pcm,
            _ => self.decoder.decode(Some(payload)),
        }
    }

    /// The sound card consumed a 10 ms block.
    pub fn consume_block(&mut self, now_ms: f64) -> (Option<Vec<i16>>, Vec<Emission>) {
        self.consume_block_in_callback(now_ms, false)
    }

    /// Like `consume_block`, for a sound card that asks for several blocks in one
    /// callback (WASAPI through cpal sometimes asks for 20-22 ms at once when
    /// Windows is late). `burst` = not the first block of the callback: missing
    /// audio there is the sound card's fault, not the network's, so it is
    /// stretched but does not count as late (the buffer does not grow).
    pub fn consume_block_in_callback(&mut self, now_ms: f64, burst: bool) -> (Option<Vec<i16>>, Vec<Emission>) {
        self.in_burst = burst;
        let block = self.blocks.pop_front();
        let emitted = self.refill(now_ms);
        self.in_burst = false;
        (block, emitted)
    }

    fn grow(&mut self) { self.buffer_ms = (self.buffer_ms + 10).min(self.max_ms); }

    fn late(&mut self, now_ms: f64) {
        if self.in_burst { return; }
        self.counters.late_arrivals += 1;
        if now_ms - self.last_late_ms < REPEAT_WINDOW_MS { self.grow(); }
        self.last_late_ms = now_ms;
        self.last_underrun_ms = now_ms;
    }

    fn underrun(&mut self, now_ms: f64) {
        if !self.playing { return; }
        self.playing = false;
        self.concealed_in_a_row = 0;
        self.next = None;
        self.counters.underruns += 1;
        self.last_underrun_ms = now_ms;
        self.grow();
    }

    fn calm_down(&mut self, now_ms: f64) {
        if now_ms - self.last_underrun_ms > self.calm_ms && self.buffer_ms > self.min_ms {
            self.buffer_ms = self.buffer_ms.saturating_sub(10).max(self.min_ms);
            self.last_underrun_ms = now_ms;
        }
    }

    fn queue_ms(&self) -> usize { self.blocks.len() * 10 + self.packets.len() * 20 }

    fn refill(&mut self, now_ms: f64) -> Vec<Emission> {
        let mut emitted = vec![];
        if self.next.is_none() { return emitted; }
        if !self.playing {
            if self.packets.len() * 20 < self.target_ms() as usize { return emitted; }
            self.playing = true;
        }
        let target_blocks = (self.target_ms() as usize).div_ceil(10);
        while self.blocks.len() < target_blocks {
            let seq = self.next.unwrap();
            let (pcm, kind, advance) = if let Some(payload) = self.packets.remove(&seq) {
                let pcm = self.decode(seq, &payload);
                self.remember(seq, Resolved::Real);
                self.concealed_in_a_row = 0;
                // A silent packet is dropped only when the queue exceeds target + 20 ms.
                if self.queue_ms() > self.target_ms() as usize + 20 && is_silence(&pcm) {
                    self.counters.trimmed += 1;
                    self.next = Some(seq.wrapping_add(1));
                    continue;
                }
                self.counters.played += 1;
                (pcm, EmissionKind::Real(seq), true)
            } else if self.blocks.is_empty() && (!self.packets.is_empty() || self.concealed_in_a_row < MAX_CONCEALED_IN_A_ROW) {
                let pcm = self.decoder.decode(None);
                self.counters.concealed += 1;
                if !self.packets.is_empty() {
                    self.remember(seq, Resolved::Concealed);
                    self.concealed_in_a_row += 1;
                    (pcm, EmissionKind::PlcLoss(seq), true)
                } else {
                    if self.concealed_in_a_row == 0 { self.late(now_ms); }
                    self.counters.stretched += 1;
                    self.concealed_in_a_row += 1;
                    (pcm, EmissionKind::PlcStretch(seq), false)
                }
            } else {
                if self.blocks.is_empty() { self.underrun(now_ms); }
                return emitted;
            };
            if advance { self.next = Some(seq.wrapping_add(1)); }
            emitted.push(Emission { kind, first_sample: *pcm.first().unwrap_or(&0) });
            for block in pcm.chunks_exact(BLOCK_SAMPLES) {
                self.blocks.push_back(block.to_vec());
            }
        }
        self.calm_down(now_ms);
        emitted
    }
}

/// The same int16 threshold as the prototype's `esSilencio`: all samples in [-32, 32].
pub fn is_silence(pcm: &[i16]) -> bool { pcm.iter().all(|sample| sample.abs_diff(0) <= 32) }

#[cfg(test)]
mod tests {
    use super::*;

    /// With the buffer full, libopus runs when the packet arrives (network
    /// thread) and not in the sound card callback, in the same order.
    #[test]
    fn libopus_on_arrival_and_in_order() {
        use std::sync::{Arc, Mutex};
        #[derive(Clone, Default)]
        struct Recorded(Arc<Mutex<Vec<u8>>>);
        impl Decoder for Recorded {
            fn decode(&mut self, payload: Option<&[u8]>) -> Vec<i16> {
                let b = payload.and_then(|p| p.first()).copied().unwrap_or(255);
                self.0.lock().unwrap().push(b);
                vec![i16::from(b); 960 * 2]
            }
        }
        let record = Recorded::default();
        let mut j = JitterBuffer::new(record.clone(), 40, 40, 200, 60_000.0, 0.0);
        // One packet more than the buffer: each arrives with it full, which is
        // when it used to be decoded in the sound card callback.
        for i in 0..3u16 { j.receive(i, vec![i as u8], 0.0); }
        let mut in_callbacks = 0;
        let mut played = Vec::new();
        for i in 3..50u16 {
            for k in 0..2 {
                let before = record.0.lock().unwrap().len();
                if let Some(b) = j.consume_block(f64::from(i) * 20.0 + f64::from(k) * 10.0).0 { played.push(b[0]) }
                in_callbacks += record.0.lock().unwrap().len() - before;
            }
            j.receive(i, vec![i as u8], f64::from(i) * 20.0 + 15.0);
        }
        let order = record.0.lock().unwrap().clone();
        assert_eq!(order, (0..order.len() as u8).collect::<Vec<_>>(), "libopus in order and without PLC");
        assert_eq!(in_callbacks, 0, "no decoding in the sound card callback");
        assert!(j.counters.decoded_early > 40, "{:?}", j.counters);
        // What plays is unchanged: each packet, two blocks, in order.
        assert!(played.windows(2).all(|w| w[1] == w[0] || w[1] == w[0] + 1), "{played:?}");
    }
}
