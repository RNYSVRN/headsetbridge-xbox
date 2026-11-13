# Changelog

## 2.0.0

- New native Windows 11 app (WinUI 3, Windows App SDK 1.8) replacing the WebView2/React interface; WebView2 is no longer required.
- English and German, switchable in the settings (default: Windows display language); the sign-in dialog of the engine follows the app language.
- Preview mode `--demo` with a simulated connection; start options `--lang=en|de` and `--theme=light|dark`.
- Audio engine slimmed down to the WebRTC route used by the app and translated to English (code, files, log lines). Data moved to `%LOCALAPPDATA%\HeadsetBridge\engine`; the 1.x sign-in is taken over automatically. The sign-in code is no longer written to the log.
- Links in the app (sound settings, colors, project pages) open through the Windows shell.
- New build script (libopus, engine, tests, license notices, self-contained app, NSIS setup, portable ZIP); license notices generated from the components actually shipped.
- Setup in English and German, per-user, without the WebView2 bootstrapper; removes leftovers of 1.x.

## 1.2.0

- Last version with the WebView2/React interface (German only), based on HeadsetBridge for Xbox commit `d1df5bcca184810e61c366b40ac7d59a148513fc`.
