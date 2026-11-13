//! One audio-only session with the console over WebRTC: connect, play the
//! audio track into the jitter buffer, keep the session alive and close it.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::data_channel_state::RTCDataChannelState;
use webrtc::data_channel::RTCDataChannel;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

use crate::audio::SharedBuffer;
use crate::channels::{authorization_request, configuration, gamepad_added, gamepad_packet, handshake, metadata_packet, send_binary, transaction_complete, GamepadFrame};
use crate::log::log;
use crate::offer::{create_connection, create_offer};
use crate::stats::{End, Stats};
use crate::xhome::XHome;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// The console sends 50 packets/s even during silence, so 3 s without any is a drop.
const AUDIO_TIMEOUT: Duration = Duration::from_secs(3);

/// `tokio::time::interval` without bursts: missed ticks (the single thread was
/// late) do not all fire at once; it continues from now.
pub fn interval(period: Duration) -> tokio::time::Interval {
    let mut i = tokio::time::interval(period);
    i.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    i
}

fn uuid() -> String { uuid::Uuid::new_v4().to_string() }

fn ms(t0: Instant) -> f64 { t0.elapsed().as_secs_f64() * 1000.0 }

/// Connects and keeps the session until `stop` is notified or the connection
/// drops (`End::Dropped`); always closes it on the console with DELETE.
pub async fn run(mut xhome: XHome, console: &str, t0: Instant, stats: Arc<Stats>, buffer: Option<SharedBuffer>, stop: Arc<tokio::sync::Notify>) -> Result<End> {
    let (pc, channels) = create_connection().await?;
    let channel = |name: &str| channels.iter().find(|c| c.label() == name).cloned().expect("the four channels exist");
    let (input, control, message) = (channel("input"), channel("control"), channel("message"));

    // Audio: count packets and the mode the encoder chose (TOC byte, RFC 6716 §3.1).
    let s = stats.clone();
    // Only the FIRST audio source (SSRC) goes into the buffer. With music the
    // console sometimes opens a second one; mixing two sequence numberings in one
    // buffer caused jumps, concealment and trimming.
    let chosen = Arc::new(AtomicU32::new(0));
    pc.on_track(Box::new(move |track, _, _| {
        let (s, buffer, chosen) = (s.clone(), buffer.clone(), chosen.clone());
        Box::pin(async move {
            if track.kind() != webrtc::rtp_transceiver::rtp_codec::RTPCodecType::Audio { return }
            log(t0, &format!("audio track: ssrc {:08x}, {} ({})", track.ssrc(), track.codec().capability.mime_type, track.id()));
            let mut seen: Vec<u32> = Vec::new();
            while let Ok((rtp, _)) = track.read_rtp().await {
                let ssrc = rtp.header.ssrc;
                let _ = chosen.compare_exchange(0, ssrc, Ordering::Relaxed, Ordering::Relaxed);
                if ssrc != chosen.load(Ordering::Relaxed) {
                    if !seen.contains(&ssrc) {
                        seen.push(ssrc);
                        log(t0, &format!("another audio source: ssrc {ssrc:08x}, PT {}, {} B; not played", rtp.header.payload_type, rtp.payload.len()));
                    }
                    s.other_sources.fetch_add(1, Ordering::Relaxed);
                    continue;
                }
                if let Some(b) = &buffer {
                    let now = ms(t0);
                    if let Ok(mut b) = b.lock() {
                        let seq = rtp.header.sequence_number;
                        // RED (PT 63): first the copies of earlier packets (if that
                        // one was lost and has not played yet, they fill it in), then the primary.
                        match (rtp.header.payload_type == 63).then(|| unpack_red(&rtp.payload)).flatten() {
                            Some(blocks) => {
                                for (back, opus) in blocks.iter().filter(|(k, o)| *k > 0 && !o.is_empty()) {
                                    s.redundant.fetch_add(1, Ordering::Relaxed);
                                    b.receive(seq.wrapping_sub(*back), opus.to_vec(), now);
                                }
                                if let Some((_, primary)) = blocks.last() { b.receive(seq, primary.to_vec(), now); }
                            }
                            None => { b.receive(seq, rtp.payload.to_vec(), now); }
                        }
                    }
                }
                s.audio(t0);
                s.packets.fetch_add(1, Ordering::Relaxed);
                s.bytes.fetch_add(rtp.payload.len() as u64, Ordering::Relaxed);
                // CELT FB = config 28-31 (toc >> 3); stereo = bit 2.
                if let Some(&toc) = rtp.payload.first() {
                    if toc >> 3 >= 28 && toc & 4 != 0 { s.celt_fullband_stereo.fetch_add(1, Ordering::Relaxed); }
                }
            }
        })
    }));
    pc.on_peer_connection_state_change(Box::new(move |state| {
        log(t0, &format!("connection: {state}"));
        Box::pin(async {})
    }));

    // Protocol: Handshake when `message` opens; after HandshakeAck, authorization,
    // gamepad announcement, metadata, configuration and one idle frame per second
    // (without the announcement AND the frames: KickForNoClientConnection after 58 s).
    let sequence = Arc::new(Mutex::new(0u32));
    let end = Arc::new(tokio::sync::Notify::new());
    let drop_reason: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    {
        let m = message.clone();
        message.on_open(Box::new(move || {
            let m = m.clone();
            Box::pin(async move { let _ = send_binary(&m, &handshake(&uuid())).await; })
        }));
    }
    {
        let (input, control, message2, sequence, end, drop_reason) = (input.clone(), control.clone(), message.clone(), sequence.clone(), end.clone(), drop_reason.clone());
        message.on_message(Box::new(move |msg: DataChannelMessage| {
            let (input, control, message2, sequence, end, drop_reason) = (input.clone(), control.clone(), message2.clone(), sequence.clone(), end.clone(), drop_reason.clone());
            Box::pin(async move {
                let text = String::from_utf8_lossy(&msg.data).to_string();
                let json: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
                // What the console sends and when (type and target; the content only when short).
                let target = json["target"].as_str().unwrap_or("");
                let content = json["content"].as_str().unwrap_or("");
                log(t0, &format!("console → {} {target} {}", json["type"].as_str().unwrap_or("?"), if content.len() <= 160 { content } else { "(long)" }));
                // Like the reference clients: every transaction is completed
                // (dialogs with Result 0 = accept).
                if json["type"] == "TransactionStart" {
                    if let Some(id) = json["id"].as_str() {
                        let reply = if target.contains("ShowMessageDialog") { "{\"Result\":0}" } else { "" };
                        let _ = send_binary(&message2, &transaction_complete(id, reply)).await;
                    }
                }
                if json["type"] == "HandshakeAck" {
                    start_after_ack(&input, &control, &message2, sequence.clone(), t0).await;
                }
                if text.contains("WarningForBeingIdle") {
                    // A stick nudge below the dead zone.
                    let mut s = sequence.lock().await;
                    *s += 1;
                    let _ = send_binary(&input, &gamepad_packet(*s, ms(t0), &[GamepadFrame { lx: 0.1, ly: 0.0 }, GamepadFrame::default()])).await;
                    log(t0, "idle warning → nudge");
                }
                if text.contains("sessionLifetimeManagement") {
                    let reason = serde_json::from_str::<serde_json::Value>(json["content"].as_str().unwrap_or("{}")).unwrap_or_default()["reason"].as_str().unwrap_or("").to_owned();
                    log(t0, &format!("console: {reason}"));
                    // KickForStopCommand answers our DELETE; any other Kick* is an eviction.
                    if reason.starts_with("Kick") && reason != "KickForStopCommand" {
                        *drop_reason.lock().await = Some(reason);
                        end.notify_one();
                    }
                }
            })
        }));
    }

    // Everything local BEFORE /play: offer, local description and ICE candidates
    // are ready; after Provisioned only the exchange and the connection remain.
    let offer = create_offer(&pc).await?;
    pc.set_local_description(RTCSessionDescription::offer(offer.clone())?).await?;
    let mut gathered = pc.gathering_complete_promise().await;
    let _ = gathered.recv().await;
    let local: Vec<String> = pc.local_description().await.map(|d| d.sdp).unwrap_or_default()
        .lines().filter_map(|l| l.trim_end().strip_prefix("a=").filter(|c| c.starts_with("candidate:")).map(str::to_owned)).collect();

    stats.mark_play(t0);
    let play = Instant::now();
    let since = move || play.elapsed().as_millis();
    xhome.start(console).await?;
    log(t0, "session created");
    xhome.wait_for_console(|state| log(t0, &format!("console state: {state} (+{} ms after /play)", since()))).await?;

    let answer = xhome.exchange_sdp(&offer).await?;
    // Which audio codecs the console chose (only rtpmap, fmtp and rtcp-fb of m=audio).
    {
        let mut in_audio = false;
        let mut chosen = Vec::new();
        for l in answer.lines() {
            if l.starts_with("m=") { in_audio = l.starts_with("m=audio"); if in_audio { chosen.push(l.trim().to_owned()) } }
            else if in_audio && (l.starts_with("a=rtpmap") || l.starts_with("a=fmtp") || l.starts_with("a=rtcp-fb")) { chosen.push(l.trim().to_owned()) }
        }
        log(t0, &format!("audio answer: {}", chosen.join(" | ")));
    }
    pc.set_remote_description(RTCSessionDescription::answer(answer)?).await?;
    log(t0, &format!("offer accepted (+{} ms after /play)", since()));

    let remote = xhome.exchange_ice(&local).await?;
    log(t0, &format!("candidates: {} local, {} from the console (+{} ms after /play)", local.len(), remote.len(), since()));
    for c in remote {
        if let Err(e) = pc.add_ice_candidate(RTCIceCandidateInit { candidate: c, sdp_mid: Some("0".into()), sdp_mline_index: Some(0), username_fragment: None }).await {
            log(t0, &format!("candidate rejected: {e}"));
        }
    }

    // Keepalive every 30 s, gamepad frames at 1 Hz and a check every second.
    let mut keepalive = interval(Duration::from_secs(30));
    let mut tick = interval(Duration::from_secs(1));
    let (mut last_packets, mut last_progress) = (stats.packets.load(Ordering::Relaxed), Instant::now());
    let mut pair_logged = false;
    let result = loop {
        tokio::select! {
            _ = end.notified() => break End::Dropped(drop_reason.lock().await.clone().unwrap_or_default()),
            _ = stop.notified() => break End::Stopped,
            // In its own task: a slow reply from the service (15 s timeout) must not stall the session loop.
            _ = keepalive.tick() => {
                let x = xhome.clone();
                tokio::spawn(async move {
                    if let Err(e) = x.keepalive().await { log(t0, &format!("keepalive: {e}")) }
                });
            }
            _ = tick.tick() => {
                if input.ready_state() == RTCDataChannelState::Open {
                    let mut s = sequence.lock().await;
                    *s += 1;
                    let _ = send_binary(&input, &gamepad_packet(*s, ms(t0), &[GamepadFrame::default()])).await;
                }
                // The chosen path (at home, host ↔ host over the LAN).
                if !pair_logged && pc.connection_state() == RTCPeerConnectionState::Connected {
                    if let Some(pair) = pc.sctp().transport().ice_transport().get_selected_candidate_pair().await {
                        log(t0, &format!("ICE pair: {pair}"));
                        pair_logged = true;
                    }
                }
                let now = stats.packets.load(Ordering::Relaxed);
                if now != last_packets { last_packets = now; last_progress = Instant::now(); }
                if last_progress.elapsed() > AUDIO_TIMEOUT { break End::Dropped("audio stopped".into()) }
                if matches!(pc.connection_state(), RTCPeerConnectionState::Failed | RTCPeerConnectionState::Closed) {
                    break End::Dropped("connection lost".into())
                }
            }
        }
    };
    if let Err(e) = xhome.stop().await { log(t0, &format!("could not close the session on the console: {e}")) }
    pc.close().await?;
    Ok(result)
}

/// Unpacks RED (RFC 2198): returns (distance back in 20 ms packets, Opus payload)
/// for every block, the primary last with distance 0. `None` if malformed.
pub fn unpack_red(payload: &[u8]) -> Option<Vec<(u16, &[u8])>> {
    let mut headers = Vec::new();
    let mut i = 0;
    loop {
        let b = *payload.get(i)?;
        if b & 0x80 == 0 { i += 1; break }
        let h = payload.get(i..i + 4)?;
        let offset_ts = (u16::from(h[1]) << 6) | (u16::from(h[2]) >> 2);
        let length = (usize::from(h[2] & 0x03) << 8) | usize::from(h[3]);
        headers.push((offset_ts, length));
        i += 4;
    }
    let mut blocks = Vec::new();
    for (offset_ts, length) in headers {
        blocks.push((offset_ts / 960, payload.get(i..i + length)?));
        i += length;
    }
    blocks.push((0, payload.get(i..)?));
    Some(blocks)
}

async fn start_after_ack(input: &Arc<RTCDataChannel>, control: &Arc<RTCDataChannel>, message: &Arc<RTCDataChannel>, sequence: Arc<Mutex<u32>>, t0: Instant) {
    let _ = send_binary(control, &authorization_request()).await;
    let _ = send_binary(control, &gamepad_added()).await;
    let s = *sequence.lock().await;
    let _ = send_binary(input, &metadata_packet(s, ms(t0), 2)).await;
    let install_id = uuid();
    for i in 0.. {
        let Some(m) = configuration(i, &uuid(), &install_id) else { break };
        let _ = send_binary(message, &m).await;
    }
    log(t0, "HandshakeAck → control, input and configuration sent");
}
