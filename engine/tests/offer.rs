//! The local offer: minimal, with redundant audio, inactive video on port 9,
//! and nothing secret in the redacted output.

use headsetbridge_engine::offer::{generate_offer, redact_sdp};

#[tokio::test]
async fn minimal_offer_with_red_inactive_video_and_port_nine() {
    let offer = generate_offer().await.unwrap();
    let channels: Vec<(&str, &str, bool)> = offer.channels.iter()
        .map(|c| (c.name.as_str(), c.protocol.as_str(), c.ordered)).collect();
    assert_eq!(channels, vec![
        ("input", "1.0", true), ("chat", "chatV1", true),
        ("control", "controlV1", true), ("message", "messageV1", true),
    ]);

    let sections: Vec<&str> = offer.sdp.split("\r\nm=").collect();
    assert_eq!(sections.len(), 4, "audio, video and application, in this order");
    let (audio, video, data) = (sections[1], sections[2], sections[3]);
    assert!(audio.starts_with("audio 9 UDP/TLS/RTP/SAVPF 63 111"), "{audio}");
    assert!(audio.contains("a=rtpmap:63 red/48000/2"));
    assert!(audio.contains("a=fmtp:63 111/111"));
    assert!(audio.contains("a=rtpmap:111 opus/48000/2"));
    assert!(audio.contains("a=fmtp:111 minptime=10;useinbandfec=1;stereo=1"));
    assert!(audio.contains("a=recvonly"));
    assert!(video.starts_with("video 9 UDP/TLS/RTP/SAVPF 115"), "{video}");
    assert!(video.contains("a=fmtp:115 level-asymmetry-allowed=1;packetization-mode=0;profile-level-id=42e01f"));
    assert!(video.contains("a=inactive"));
    assert!(data.starts_with("application 9 UDP/DTLS/SCTP webrtc-datachannel"), "{data}");
    assert!(data.contains("a=sctp-port:5000"));
    assert!(!offer.sdp.contains("m=video 0"));
}

#[tokio::test]
async fn redacted_offer_hides_credentials() {
    let offer = generate_offer().await.unwrap();
    let visible = redact_sdp(&offer.sdp);
    for forbidden in ["a=ice-ufrag:", "a=ice-pwd:", "a=fingerprint:", "a=candidate:", "o=", "c="] {
        assert!(!visible.lines().any(|line| line.starts_with(forbidden)), "leaked {forbidden}");
    }
    assert!(visible.contains("m=video 9"));
    assert!(visible.contains("a=inactive"));
}
