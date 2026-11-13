//! Where the engine keeps its files: `%LOCALAPPDATA%\HeadsetBridge\engine\`.
//!
//! - `account.json`: the Microsoft refresh token (rotates on every use; secret).
//! - `engine.log`: the log the desktop app reads (rotated to `engine.log.1` at 2 MiB).
//! - `open-session`: the id of a console session that has not been closed yet.
//!
//! Version 1.x stored the same data under `%LOCALAPPDATA%\XboxSoloAudio\`
//! (`cuenta.json`, `rust.log`, `sesion-abierta`). [`migrate_legacy`] copies the
//! sign-in and the open-session marker once, so users stay signed in.

use std::path::PathBuf;

/// `%LOCALAPPDATA%\HeadsetBridge\engine`.
pub fn data_dir() -> PathBuf {
    PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_default()).join("HeadsetBridge").join("engine")
}

pub fn account_file() -> PathBuf { data_dir().join("account.json") }
pub fn log_file() -> PathBuf { data_dir().join("engine.log") }
pub fn open_session_file() -> PathBuf { data_dir().join("open-session") }

/// The folder used by version 1.x of the engine.
fn legacy_dir() -> PathBuf {
    PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_default()).join("XboxSoloAudio")
}

/// Copies the 1.x sign-in and open-session marker into the new folder if the
/// new files do not exist yet. The old folder is left untouched.
pub fn migrate_legacy() {
    let _ = std::fs::create_dir_all(data_dir());
    for (old, new) in [("cuenta.json", account_file()), ("sesion-abierta", open_session_file())] {
        let old = legacy_dir().join(old);
        if !new.exists() && old.exists() {
            let _ = std::fs::copy(&old, &new);
        }
    }
}
