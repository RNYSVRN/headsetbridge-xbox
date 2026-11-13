//! HeadsetBridge engine: an audio-only Xbox Remote Play client.
//!
//! The desktop app starts `headsetbridge-engine.exe`, watches its log file and
//! stops it through a named pipe. The engine signs in with the Xbox account,
//! opens a Remote Play session over WebRTC that asks the console for audio only,
//! and plays the Opus stream on the Windows default output device.

pub mod audio;
pub mod auth;
pub mod channels;
pub mod http;
pub mod jitter_buffer;
pub mod lang;
pub mod log;
pub mod offer;
pub mod paths;
pub mod retry;
pub mod session;
pub mod stats;
pub mod xhome;
