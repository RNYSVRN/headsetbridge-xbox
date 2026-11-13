//! The jitter buffer against the behaviour of the Node prototype.

use headsetbridge_engine::jitter_buffer::{is_silence, Decoder, Emission, EmissionKind, JitterBuffer};

/// Decodes every packet into samples whose low byte is the packet's first byte; PLC → 255.
struct FakeDecoder;
impl Decoder for FakeDecoder {
    fn decode(&mut self, payload: Option<&[u8]>) -> Vec<i16> {
        let byte = payload.and_then(|p| p.first()).copied().unwrap_or(255);
        vec![i16::from_le_bytes([byte, byte]); 960 * 2]
    }
}

struct Bench {
    buffer: JitterBuffer<FakeDecoder>,
    now_ms: f64,
    pcm: Vec<u8>,
    kinds: Vec<EmissionKind>,
}

impl Bench {
    fn new(buffer_ms: u32) -> Self {
        Self { buffer: JitterBuffer::new(FakeDecoder, buffer_ms, 20, 80, 60_000.0, 0.0),
               now_ms: 0.0, pcm: vec![], kinds: vec![] }
    }
    fn collect(&mut self, emissions: Vec<Emission>) {
        for emission in emissions {
            self.pcm.push(emission.first_sample as u8);
            self.kinds.push(emission.kind);
        }
    }
    fn receive(&mut self, seq: u16, delay_ms: f64) {
        self.now_ms = f64::from(seq) * 20.0 + delay_ms;
        let emissions = self.buffer.receive(seq, vec![seq as u8], self.now_ms);
        self.collect(emissions);
    }
    fn play(&mut self) {
        assert!(self.buffer.pending_blocks() > 0, "there was PCM pending for the sound card");
        let (block, emissions) = self.buffer.consume_block(self.now_ms);
        assert_eq!(block.unwrap().len(), 480 * 2);
        self.collect(emissions);
    }
}

#[test]
fn normal_order_duplicate_and_waiting_for_the_buffer() {
    let mut b = Bench::new(40);
    b.receive(1, 0.0);
    assert!(b.pcm.is_empty());
    b.receive(2, 0.0);
    assert_eq!(b.pcm, [1, 2]);
    b.receive(2, 0.0);
    assert_eq!(b.buffer.counters.played, 2);
    assert_eq!(b.buffer.counters.duplicates, 1);
    assert_eq!(b.buffer.pending_blocks(), 4);
}

#[test]
fn reordered_packet_arrives_before_the_sound_card_deadline() {
    let mut b = Bench::new(40);
    b.receive(10, 0.0); b.receive(11, 0.0); b.receive(13, 0.0);
    assert_eq!(b.buffer.counters.concealed, 0);
    b.play();
    b.receive(12, 0.0);
    b.play(); b.play();
    assert_eq!(b.pcm, [10, 11, 12, 13]);
    assert_eq!(b.buffer.counters.concealed, 0);
}

#[test]
fn a_loss_is_concealed_once_and_the_late_packet_is_dropped() {
    let mut b = Bench::new(40);
    b.receive(20, 0.0); b.receive(21, 0.0); b.receive(23, 0.0);
    b.play(); b.play(); b.play();
    assert_eq!(b.pcm, [20, 21]);
    b.play();
    assert_eq!(b.pcm, [20, 21, 255, 23]);
    b.receive(22, 0.0);
    assert_eq!(b.buffer.counters.concealed, 1);
    assert_eq!(b.buffer.counters.too_late, 1);
    assert_eq!(b.buffer.counters.played, 3);
}

#[test]
fn big_jumps_and_backward_restarts_resync_without_plc() {
    let mut b = Bench::new(40);
    b.receive(30, 0.0); b.receive(31, 0.0);
    b.receive(100, 0.0); b.receive(101, 0.0); b.play();
    assert!(b.pcm.contains(&100));
    assert_eq!(b.buffer.counters.concealed, 0);

    let mut b = Bench::new(40);
    b.receive(10000, 0.0); b.receive(10001, 0.0);
    b.receive(1, 0.0); b.receive(2, 0.0);
    b.play(); b.play(); b.play();
    assert_eq!(b.buffer.next, Some(3));
    assert_eq!(b.buffer.counters.concealed, 0);
}

#[test]
fn a_real_drop_after_100_ms_of_plc_refills_the_buffer() {
    let mut b = Bench::new(20);
    b.receive(1, 0.0);
    for _ in 0..40 {
        if b.buffer.counters.underruns != 0 { break; }
        b.play();
    }
    assert_eq!(b.buffer.counters.concealed, 5);
    assert_eq!(b.buffer.counters.late_arrivals, 1);
    assert_eq!(b.buffer.counters.underruns, 1);
    assert!(!b.buffer.playing);
    b.receive(7, 0.0);
    assert_eq!(b.pcm.len(), 6);
    b.receive(8, 0.0);
    assert_eq!(b.pcm, [1, 255, 255, 255, 255, 255, 7, 8]);
}

#[test]
fn plc_stretches_and_two_late_arrivals_in_ten_seconds_add_ten_ms() {
    let mut b = Bench::new(20);
    b.receive(1, 0.0); b.receive(2, 0.0);
    while b.buffer.counters.concealed == 0 { b.play(); }
    assert_eq!(b.buffer.counters.stretched, 1);
    assert_eq!(b.buffer.counters.late_arrivals, 1);
    assert_eq!(b.buffer.target_ms(), 20);
    b.receive(3, 30.0);
    assert_eq!(b.buffer.counters.too_late, 0);
    while !b.pcm.contains(&3) { b.play(); }
    assert_eq!(b.pcm, [1, 2, 255, 3]);
    while b.buffer.counters.concealed < 2 { b.play(); }
    assert_eq!(b.buffer.counters.late_arrivals, 2);
    assert_eq!(b.buffer.counters.underruns, 0);
    assert_eq!(b.buffer.target_ms(), 30);
}

#[test]
fn surplus_queue_trims_silence_but_keeps_sound() {
    let mut b = Bench::new(20);
    for seq in 252..=258 { b.receive(seq, f64::from(258 - seq) * -20.0); }
    for _ in 0..20 {
        if b.buffer.pending_blocks() > 0 { b.play(); }
    }
    assert!(b.buffer.counters.trimmed >= 1);
    assert!(!b.pcm.contains(&0));
    for mark in [252, 253, 254, 1, 2] { assert!(b.pcm.contains(&mark), "missing {mark}"); }
}

#[test]
fn arrival_jitter_is_measured_without_inflating_the_target() {
    let mut b = Bench::new(20);
    for seq in 1..=5 { b.receive(seq, 0.0); }
    b.receive(6, 35.0);
    assert_eq!(b.buffer.network_jitter_ms(), 35);
    assert_eq!(b.buffer.target_ms(), 20);
}

#[test]
fn silence_uses_the_int16_threshold_of_node() {
    assert!(is_silence(&[-32, 0, 32]));
    assert!(!is_silence(&[-33, 0]));
    assert!(!is_silence(&[33]));
}

#[test]
fn a_sound_card_burst_stretches_without_counting_as_network_delay() {
    // The sound card asks for 20-22 ms at once when Windows is late (measured with
    // cpal): emptying the buffer like that is not the network's fault and must
    // not raise the target.
    let mut b = Bench::new(20);
    b.receive(1, 0.0);
    b.receive(2, 0.0);
    for _ in 0..6 {
        let (_, emissions) = b.buffer.consume_block_in_callback(b.now_ms, true);
        b.collect(emissions);
    }
    assert!(b.buffer.counters.stretched >= 1, "stretched: no gap of silence");
    assert_eq!(b.buffer.counters.late_arrivals, 0);
    assert_eq!(b.buffer.target_ms(), 20);
}
