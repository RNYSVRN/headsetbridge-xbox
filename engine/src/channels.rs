//! The data-channel messages, byte for byte as verified against the Node
//! prototype (see `tests/vectors.rs`). JSON travels as UTF-8 bytes over SCTP,
//! never as `send_text`.

use bytes::Bytes;
use webrtc::data_channel::RTCDataChannel;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Channel {
    pub name: &'static str,
    pub protocol: &'static str,
}

/// The four channels, in the order of xbox-xcloud-player.
pub const CHANNELS: [Channel; 4] = [
    Channel { name: "input", protocol: "1.0" },
    Channel { name: "chat", protocol: "chatV1" },
    Channel { name: "control", protocol: "controlV1" },
    Channel { name: "message", protocol: "messageV1" },
];

const ACCESS_KEY: &str = "4BDB3609-C1F1-4195-9B37-FEFF45DA8B8E";

/// Index of the virtual controller announced on the control channel.
const GAMEPAD_INDEX: u8 = 0;

/// One gamepad frame; only the left stick is ever moved (the idle nudge).
#[derive(Clone, Copy, Debug, Default)]
pub struct GamepadFrame {
    pub lx: f64,
    pub ly: f64,
}

fn header(kind: u16, sequence: u32, time_ms: f64) -> [u8; 14] {
    let mut bytes = [0; 14];
    bytes[0..2].copy_from_slice(&kind.to_le_bytes());
    bytes[2..6].copy_from_slice(&sequence.to_le_bytes());
    bytes[6..14].copy_from_slice(&time_ms.to_le_bytes());
    bytes
}

/// Type 8 is ClientMetadata (xcloud-rs `packets/input.rs`, bit 0x08).
pub fn metadata_packet(sequence: u32, time_ms: f64, max_touchpoints: u8) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(15);
    bytes.extend_from_slice(&header(8, sequence, time_ms));
    bytes.push(max_touchpoints);
    bytes
}

fn axis(value: f64) -> i16 {
    // JS truncates towards zero after clamping to ±32767; `round` would change the frames.
    (value * 32767.0).trunc().clamp(-32767.0, 32767.0) as i16
}

/// Type 2: gamepad frames (index, buttons, sticks, triggers, physicality).
pub fn gamepad_packet(sequence: u32, time_ms: f64, frames: &[GamepadFrame]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(14 + 1 + 23 * frames.len());
    bytes.extend_from_slice(&header(2, sequence, time_ms));
    bytes.push(frames.len() as u8);
    for frame in frames {
        bytes.push(GAMEPAD_INDEX);
        bytes.extend_from_slice(&0u16.to_le_bytes()); // buttons
        bytes.extend_from_slice(&axis(frame.lx).to_le_bytes());
        bytes.extend_from_slice(&axis(-frame.ly).to_le_bytes());
        for _ in 0..2 { bytes.extend_from_slice(&0i16.to_le_bytes()); } // right stick
        for _ in 0..2 { bytes.extend_from_slice(&0u16.to_le_bytes()); } // triggers
        // Physicality: physical (LE) and virtual (BE), 0 like Greenlight.
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_be_bytes());
    }
    bytes
}

fn quote(text: &str) -> String {
    serde_json::to_string(text).expect("serializing a &str cannot fail")
}

pub fn handshake(id: &str) -> Vec<u8> {
    format!("{{\"type\":\"Handshake\",\"version\":\"messageV1\",\"id\":{},\"cv\":\"\"}}", quote(id)).into_bytes()
}

pub fn authorization_request() -> Vec<u8> {
    format!("{{\"message\":\"authorizationRequest\",\"accessKey\":\"{ACCESS_KEY}\"}}").into_bytes()
}

pub fn gamepad_added() -> Vec<u8> {
    format!(r#"{{"message":"gamepadChanged","gamepadIndex":{GAMEPAD_INDEX},"wasAdded":true}}"#).into_bytes()
}

/// Reply to a `TransactionStart` from the console (stratix `MessageChannel.swift`).
pub fn transaction_complete(id: &str, content: &str) -> Vec<u8> {
    format!("{{\"type\":\"TransactionComplete\",\"content\":{},\"id\":{},\"cv\":\"\"}}", quote(content), quote(id)).into_bytes()
}

fn message(target: &str, content: &str, id: &str) -> Vec<u8> {
    format!(
        "{{\"type\":\"Message\",\"content\":{},\"id\":{},\"target\":{},\"cv\":\"\"}}",
        quote(content), quote(id), quote(target)
    ).into_bytes()
}

/// The six configuration messages sent after HandshakeAck, by `index`.
pub fn configuration(index: usize, id: &str, client_app_install_id: &str) -> Option<Vec<u8>> {
    let (target, content) = match index {
        0 => ("/streaming/systemUi/configuration", "{\"version\":[0,1,0],\"systemUis\":[]}".to_owned()),
        1 => ("/streaming/properties/clientappinstallidchanged", format!("{{\"clientAppInstallId\":{}}}", quote(client_app_install_id))),
        2 => ("/streaming/characteristics/orientationchanged", "{\"orientation\":0}".to_owned()),
        3 => ("/streaming/characteristics/touchinputenabledchanged", "{\"touchInputEnabled\":false}".to_owned()),
        4 => ("/streaming/characteristics/clientdevicecapabilities", "{}".to_owned()),
        5 => ("/streaming/characteristics/dimensionschanged", "{\"horizontal\":1920,\"vertical\":1080,\"preferredWidth\":1920,\"preferredHeight\":1080,\"safeAreaLeft\":0,\"safeAreaTop\":0,\"safeAreaRight\":1920,\"safeAreaBottom\":1080,\"supportsCustomResolution\":true}".to_owned()),
        _ => return None,
    };
    Some(message(target, &content, id))
}

/// Sends `data` as `Bytes`, which makes `RTCDataChannel::send` use the binary PPID.
pub async fn send_binary(channel: &RTCDataChannel, data: &[u8]) -> webrtc::error::Result<usize> {
    channel.send(&Bytes::copy_from_slice(data)).await
}
