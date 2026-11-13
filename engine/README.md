# headsetbridge-engine

The audio engine of HeadsetBridge: an audio-only Xbox Remote Play client.
It signs in with the Xbox account, opens a Remote Play session over WebRTC that
asks the console for audio only, and plays the Opus stream on the Windows
default output device. The desktop app (`src/HeadsetBridge.App`) starts it,
reads its log and stops it.

Derived from [HeadsetBridge for Xbox](https://github.com/MGPersonalProfile/headsetbridge-for-xbox)
(commit `d1df5bcca184810e61c366b40ac7d59a148513fc`). Compared to that project,
only the WebRTC route used by the desktop app is kept; the native Nano route,
music/relay mode, mouse and keyboard input, microphone, debug server and WAV
dump were removed, and all code, files and log lines were translated to English.

## Interface with the desktop app

| What | Value |
|---|---|
| Executable | `headsetbridge-engine.exe [--lang=de\|en] [--console=<id>]` |
| Stop | write `stop` to `\\.\pipe\headsetbridge-engine` (or start a second instance) |
| Log | `%LOCALAPPDATA%\HeadsetBridge\engine\engine.log` |
| Sign-in | `%LOCALAPPDATA%\HeadsetBridge\engine\account.json` (secret, rotates) |

The app derives its status from specific log lines (`startup:`, `sign in at`,
`authenticated;`, `session created`, `console state:`, `connection:`,
`first audio`, `reconnecting`, `waiting for the console`, `stop requested`,
`stopped`, `audio output: … (shared mode)` and the 10 s `audio: … pkt/s` summary).
When changing any of them, update `src/HeadsetBridge.Core/LogParser.cs` and its tests.

On first start the engine copies the sign-in of version 1.x
(`%LOCALAPPDATA%\XboxSoloAudio\cuenta.json`) so users stay signed in.

## Modules

| File | Purpose |
|---|---|
| `main.rs` | options, single instance, retry loop, 10 s statistics |
| `auth.rs` | Microsoft / Xbox Live sign-in (device code), console lookup and wake-up |
| `xhome.rs` | xHome signalling: `/play`, state, SDP/ICE exchange, keepalive, DELETE |
| `offer.rs` | WebRTC peer connection and SDP offer (RED + Opus, inactive video) |
| `channels.rs` | data-channel messages (handshake, gamepad, configuration) |
| `session.rs` | one session: audio track → jitter buffer, protocol, drop detection |
| `jitter_buffer.rs` | receive policy: buffering, PLC, stretching, trimming |
| `audio.rs` | Opus decoder and WASAPI shared-mode output following the Windows default |
| `log.rs`, `paths.rs`, `lang.rs`, `retry.rs`, `stats.rs`, `http.rs` | helpers |

## Build and test

Requires the Rust GNU toolchain, CMake, Ninja and MinGW-w64; `Build.ps1` in the
repository root builds libopus and the engine. For tests on their own, with
libopus already built:

```powershell
$env:RUSTUP_TOOLCHAIN = 'stable-x86_64-pc-windows-gnu'
$env:LIBOPUS_LIB_DIR = '<opus-install>'; $env:LIBOPUS_STATIC = '1'; $env:LIBOPUS_NO_PKG = '1'
cargo test
```

`tests/vectors.rs` checks the data-channel messages byte for byte against
captures from the Node prototype (`testdata/vectors`).

Not affiliated with, endorsed or sponsored by Microsoft. Xbox is a trademark of
the Microsoft group of companies.
