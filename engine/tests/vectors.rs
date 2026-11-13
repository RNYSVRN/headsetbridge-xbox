//! The data-channel messages must be the exact bytes the Node prototype produced.

use headsetbridge_engine::channels::{
    authorization_request, configuration, gamepad_added, gamepad_packet, handshake, metadata_packet, GamepadFrame, CHANNELS,
};

const MANIFEST: &str = include_str!("../testdata/vectors/manifest.json");

#[test]
fn channel_order_and_protocols_match_node() {
    let manifest: serde_json::Value = serde_json::from_str(MANIFEST).unwrap();
    let actual: Vec<Vec<&str>> = CHANNELS.iter().map(|c| vec![c.name, c.protocol]).collect();
    assert_eq!(serde_json::to_value(actual).unwrap(), manifest["channelOrder"]);
}

#[test]
fn json_messages_are_the_same_utf8_bytes_as_node() {
    let manifest: serde_json::Value = serde_json::from_str(MANIFEST).unwrap();
    let ids: Vec<&str> = manifest["ids"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    assert_eq!(handshake(ids[0]), include_bytes!("../testdata/vectors/handshake.bin"));
    assert_eq!(authorization_request(), include_bytes!("../testdata/vectors/authorization.bin"));
    assert_eq!(gamepad_added(), include_bytes!("../testdata/vectors/gamepad-added.bin"));
    let expected: [&[u8]; 6] = [
        include_bytes!("../testdata/vectors/configuration0.bin"),
        include_bytes!("../testdata/vectors/configuration1.bin"),
        include_bytes!("../testdata/vectors/configuration2.bin"),
        include_bytes!("../testdata/vectors/configuration3.bin"),
        include_bytes!("../testdata/vectors/configuration4.bin"),
        include_bytes!("../testdata/vectors/configuration5.bin"),
    ];
    for (index, expected) in expected.into_iter().enumerate() {
        let id = ids[if index == 0 { 1 } else { index + 2 }];
        assert_eq!(configuration(index, id, ids[2]).unwrap(), expected, "configuration {index}");
    }
    assert!(configuration(6, ids[0], ids[2]).is_none());
}

#[test]
fn input_packets_have_a_fourteen_byte_header() {
    let manifest: serde_json::Value = serde_json::from_str(MANIFEST).unwrap();
    let time = manifest["timeMs"].as_f64().unwrap();
    let metadata = metadata_packet(0, time, 2);
    assert_eq!(metadata.len(), 15);
    assert_eq!(metadata, include_bytes!("../testdata/vectors/metadata.bin"));

    let idle = gamepad_packet(42, time, &[GamepadFrame::default()]);
    assert_eq!(idle.len(), 14 + 1 + 23);
    assert_eq!(idle, include_bytes!("../testdata/vectors/gamepad-idle.bin"));

    let nudge = gamepad_packet(1, time, &[GamepadFrame { lx: 0.1, ..Default::default() }, GamepadFrame::default()]);
    assert_eq!(nudge.len(), 14 + 1 + 23 * 2);
    assert_eq!(nudge, include_bytes!("../testdata/vectors/idle-nudge.bin"));

    let two = gamepad_packet(65535, time, &[GamepadFrame { lx: 0.1, ly: -0.5 }, GamepadFrame::default()]);
    assert_eq!(two, include_bytes!("../testdata/vectors/gamepad-two-frames.bin"));
    assert_eq!(&two[0..2], &[2, 0]); // xcloud-rs leaves out that second byte.
    assert_eq!(&two[2..6], &65535u32.to_le_bytes());
    assert_eq!(&two[6..14], &time.to_le_bytes());
}

#[test]
fn red_rfc2198_one_redundant_block() {
    // Redundant block PT 111, offset 960 (one packet), 3 bytes; primary PT 111, 2 bytes.
    let payload = [0x80 | 111, 0x0F, 0x00, 0x03, 111, 0xAA, 0xBB, 0xCC, 0x11, 0x22];
    let blocks = headsetbridge_engine::session::unpack_red(&payload).unwrap();
    assert_eq!(blocks, vec![(1, &[0xAA, 0xBB, 0xCC][..]), (0, &[0x11, 0x22][..])]);
}

#[test]
fn red_without_redundancy_and_malformed() {
    assert_eq!(headsetbridge_engine::session::unpack_red(&[111, 0x01]).unwrap(), vec![(0, &[0x01][..])]);
    assert!(headsetbridge_engine::session::unpack_red(&[0x80 | 111, 0x0F, 0x00, 0x09, 111, 0xAA]).is_none());
}
