# AGENTS.md

## Repository Shape
- Rust workspace members are only `ale-core`, `ale-cli`, and `ale-gui`; make source changes there, not in generated package/release snapshots.
- `ale-core/src/lib.rs` exposes `AleEngine`; default features enable `cloud` only, while `local-inference` pulls in Whisper/ORT/local TTS-heavy deps.
- `ale-gui` is both the desktop binary (`src/main.rs`) and mobile library entry (`src/android.rs`, plus iOS-gated modules); Slint UI source is in `ale-gui/ui/`.
- Android Java/resources under `ale-gui/android/` are real source inputs for mobile automation/services; `ale-gui/build.rs` only watches them for Android builds.
- The root `Cargo.toml` patches `i-slint-backend-winit` to `vendor/i-slint-backend-winit-1.16.1`; do not remove or bypass that patch casually.
- `ale-server` is not in this workspace. Treat `docs/API.md` and server references in generated/readme content as historical unless the task is explicitly about those docs.
- Ignore build/runtime artifacts: `target/`, `dist/`, `release/`, `Ale, My Eyes!.app/`, `*.dmg`, `*.apk`, `ale-my-eyes-*` package dirs/archives, model files, `ale-gui/android/build/`, and `config/config.json`.

## Verification Commands
- No `.github/workflows/` are present in this checkout; verify locally instead of citing CI.
- Format check: `cargo fmt --all -- --check`.
- Whole workspace check: `cargo check --workspace`. If native GUI/audio/capture deps are missing, focus with `cargo check -p ale-core`, `cargo check -p ale-cli`, or `cargo check -p ale-gui`.
- Tests are plain Cargo tests: `cargo test`, `cargo test -p ale-core`, or `cargo test -p ale-core <test_name>` for a focused test.
- Optional lint: `cargo clippy --workspace --all-targets`.
- Run apps with `cargo run -p ale-gui` or `cargo run -p ale-cli -- <transcribe|synthesize|describe|test-connection|status>`; CLI flags include `transcribe --audio input.wav`, `synthesize --text ... --output out.wav`, and `describe --image image.png`.

## Build Notes
- `ale-gui` uses Slint `1.16`; `ale-gui/build.rs` compiles `ale-gui/ui/app.slint`, which imports the other `.slint` files.
- Linux `cargo check --workspace` needs native GUI/audio/capture deps installed: `libspeechd-dev libasound2-dev libfontconfig-dev libpipewire-0.3-dev libwayland-dev libxrandr-dev libdbus-1-dev libegl-dev libgbm-dev libxcb-shape0-dev libxcb-xfixes0-dev`.
- Desktop GUI uses `cpal`/`rodio` audio, `xcap` screen capture, and `enigo` automation; Android uses `oboe`, JNI Camera2, AccessibilityService Java, a foreground service, and Slint's Android backend.
- Android packaging is `./scripts/package-android.sh`; it requires SDK + `keytool`, resolves NDK from `ANDROID_NDK_ROOT`/`ANDROID_NDK_VERSION` or SDK `ndk/27.3.13750724` with newest fallback, installs `cargo-apk` if missing, runs `scripts/build-android-java.sh`, then builds arm64 and armv7 APKs by default.
- `scripts/build-android-java.sh` needs `ANDROID_HOME` or `ANDROID_SDK_ROOT` and prefers SDK platform `android-34`; it writes classes/dex under `ale-gui/android/build/`.
- macOS packaging is `./scripts/package-macos.sh`; it builds the full workspace, creates `Ale, My Eyes!.app`, ad-hoc signs it, then prompts interactively before creating a DMG.

## Architecture Notes
- Long-lived app config is created under the user config directory at `ale-my-eyes/config.json`; `AleEngine` stores `memory.json` next to that config, and tests use `/tmp/ale-my-eyes-test/config.json` or per-test `/tmp` paths.
- `ale-core/src/cloud.rs` is OpenAI-compatible by default (`gpt-4o`, `whisper-1`, `tts-1`) and also has Anthropic/Google/Azure/custom provider mapping; never commit real API keys.
- `ale-gui/src/lib.rs` owns the main app wiring: engine setup, continuous listening, VAD timer, screen/camera capture, settings save, and TTS playback.
- Automation actions are defined in `ale-core/src/actions.rs`; desktop execution is in `ale-gui/src/automation.rs`, Android execution is in `ale-gui/src/android_automation.rs` plus Java bridge/service code.

## Packaging
- `./scripts/package-linux.sh`, `./scripts/package-windows.sh`, and `./scripts/package-android.sh` delete/recreate repo-root package dirs and archives; run them only when packaging is the task.
- `./scripts/create-release.sh` deletes and recreates `release/` with generated source/docs/quickstart tarballs; use it only when intentionally regenerating release snapshots/docs.
- `./scripts/build-release.sh` writes `dist/` and tolerates a failed release build with `|| echo`; do not treat it as a verification substitute for `cargo check`/`cargo test`.
