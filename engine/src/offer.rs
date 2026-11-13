//! The WebRTC peer connection and the SDP offer the console accepts.

use std::sync::Arc;

use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::data_channel::data_channel_init::RTCDataChannelInit;
use webrtc::data_channel::RTCDataChannel;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_codec::{RTCRtpCodecCapability, RTCRtpCodecParameters, RTPCodecType};
use webrtc::rtp_transceiver::rtp_transceiver_direction::RTCRtpTransceiverDirection;
use webrtc::rtp_transceiver::RTCRtpTransceiverInit;

use crate::channels::CHANNELS;

#[derive(Debug, Eq, PartialEq)]
pub struct CreatedChannel {
    pub name: String,
    pub protocol: String,
    pub ordered: bool,
}

pub struct LocalOffer {
    pub sdp: String,
    pub channels: Vec<CreatedChannel>,
}

/// A local offer, to check it without a console (`--offer`, tests).
pub async fn generate_offer() -> webrtc::error::Result<LocalOffer> {
    let (pc, created) = create_connection().await?;
    let channels = created.iter().map(|c| CreatedChannel {
        name: c.label().into(),
        protocol: c.protocol().into(),
        ordered: c.ordered(),
    }).collect();
    let sdp = create_offer(&pc).await?;
    pc.close().await?;
    Ok(LocalOffer { sdp, channels })
}

/// The offer as it is sent to the console: port 9 on the inactive video section.
pub async fn create_offer(pc: &RTCPeerConnection) -> webrtc::error::Result<String> {
    // An inactive section with port 0 counts as "rejected" and the console does
    // not answer; werift does the same and the Node prototype fixes it the same way.
    Ok(pc.create_offer(None).await?.sdp.replacen("\r\nm=video 0 ", "\r\nm=video 9 ", 1))
}

/// The connection with the minimal offer the console accepts and the four data
/// channels, in the order of xbox-xcloud-player: channels, audio, inactive video.
pub async fn create_connection() -> webrtc::error::Result<(Arc<RTCPeerConnection>, Vec<Arc<RTCDataChannel>>)> {
    let mut media = MediaEngine::default();
    // Redundant audio (RFC 2198, `red/48000/2` PT 63 with `111/111`, like
    // Chromium), offered before Opus. The console accepts it and sends a copy of
    // the previous packet in every packet: it recovers about 2/3 of the Wi-Fi
    // losses. Asking for 2 copies does not help: the console sends one.
    media.register_codec(RTCRtpCodecParameters {
        capability: RTCRtpCodecCapability {
            mime_type: "audio/red".into(),
            clock_rate: 48_000,
            channels: 2,
            sdp_fmtp_line: "111/111".into(),
            ..Default::default()
        },
        payload_type: 63,
        ..Default::default()
    }, RTPCodecType::Audio)?;
    // The defaults add many codecs; the console accepted only these payload types.
    media.register_codec(RTCRtpCodecParameters {
        capability: RTCRtpCodecCapability {
            mime_type: "audio/opus".into(),
            clock_rate: 48_000,
            channels: 2,
            sdp_fmtp_line: "minptime=10;useinbandfec=1;stereo=1".into(),
            ..Default::default()
        },
        payload_type: 111,
        ..Default::default()
    }, RTPCodecType::Audio)?;
    media.register_codec(RTCRtpCodecParameters {
        capability: RTCRtpCodecCapability {
            mime_type: "video/H264".into(),
            clock_rate: 90_000,
            sdp_fmtp_line: "level-asymmetry-allowed=1;packetization-mode=0;profile-level-id=42e01f".into(),
            ..Default::default()
        },
        payload_type: 115,
        ..Default::default()
    }, RTPCodecType::Video)?;

    // Receiver reports, NACK and TWCC like a browser.
    let registry = register_default_interceptors(Registry::new(), &mut media)?;
    let api = APIBuilder::new().with_media_engine(media).with_interceptor_registry(registry).build();
    let pc = Arc::new(api.new_peer_connection(RTCConfiguration::default()).await?);
    let mut channels = Vec::with_capacity(CHANNELS.len());
    for spec in CHANNELS {
        channels.push(pc.create_data_channel(spec.name, Some(RTCDataChannelInit {
            ordered: Some(true),
            protocol: Some(spec.protocol.into()),
            ..Default::default()
        })).await?);
    }

    // Receive only: webrtc-rs has no RED packetizer and fails while preparing to
    // send if the transceiver sends ("codec does not have a payloader").
    pc.add_transceiver_from_kind(RTPCodecType::Audio, Some(RTCRtpTransceiverInit {
        direction: RTCRtpTransceiverDirection::Recvonly,
        send_encodings: vec![],
    })).await?;
    // webrtc-rs only creates sendrecv or recvonly transceivers: create it
    // receiving and switch it to inactive before the offer. Without video the
    // console does not encode any.
    let video = pc.add_transceiver_from_kind(RTPCodecType::Video, Some(RTCRtpTransceiverInit {
        direction: RTCRtpTransceiverDirection::Recvonly,
        send_encodings: vec![],
    })).await?;
    video.set_direction(RTCRtpTransceiverDirection::Inactive).await;
    Ok((pc, channels))
}

/// The SDP without credentials and addresses, safe to print or log.
pub fn redact_sdp(sdp: &str) -> String {
    sdp.lines()
        .map(str::trim_end)
        .filter(|line| ![
            "o=", "c=", "a=ice-ufrag:", "a=ice-pwd:", "a=fingerprint:",
            "a=candidate:", "a=end-of-candidates", "a=ssrc:", "a=msid:",
        ].iter().any(|prefix| line.starts_with(prefix)))
        .collect::<Vec<_>>()
        .join("\n")
}
