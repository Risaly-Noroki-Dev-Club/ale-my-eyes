# AGENTS.md

## Repository Shape
- This is a Rust workspace with active crates `ale-core`, `ale-cli`, and `ale-gui` declared in the root `Cargo.toml`.
- `ale-server` has been extracted to a separate project: [ale-server](https://github.com/Risaly-Noroki-Dev-Club/ale-server)
- Treat root crate directories as source of truth. `release/ale-my-eyes-source/` is a generated source snapshot from `scripts/create-release.sh`, not the primary code to edit.
- `dist/`, `release/`, `target/`, packaged app directories, model files, and `config/config.json` are build/runtime artifacts; avoid editing them unless the task is explicitly about packaging output.

## Useful Commands
- Check only the library crate: `cargo check -p ale-core`.
- Check CLI/core together: `cargo check -p ale-core -p ale-cli`.
- Check GUI separately: `cargo check -p ale-gui`.
- Run tests when present: `cargo test` or focused package tests with `cargo test -p ale-core`.
- Format and lint are plain Cargo commands: `cargo fmt` and `cargo clippy --workspace --all-targets`.
- Run entrypoints with `cargo run -p ale-cli -- <subcommand>` and `cargo run -p ale-gui`.

## Current Build Notes
- `cargo check --workspace` is expected to pass as of this file's latest update.
- `ale-gui` uses Slint `1.16` for cross-platform UI (desktop + Android). The `.slint` UI files are in `ale-gui/ui/` and compiled by `slint-build` in `build.rs`.
- `ale-gui` records audio through `cpal` on desktop and `oboe` on Android; Linux checks/builds need `libasound2-dev` and `libfontconfig-dev` installed.
- `ale-gui` desktop also uses `xcap` (screen capture) and `enigo` (keyboard/mouse automation); Linux builds additionally need `libpipewire-0.3-dev`, `libwayland-dev`, `libxrandr-dev`, `libdbus-1-dev`, `libegl-dev`, `libgbm-dev`, `libxcb-shape0-dev`, and `libxcb-xfixes0-dev`.
- `ale-gui` is a `cdylib` for Android builds via `cargo-apk`, and a regular binary for desktop.

## Architecture Notes
- `ale-core/src/lib.rs` exposes `AleEngine` and gates local ASR/VLM/LLM/TTS modules behind features. Default features only enable `cloud`.
- `ale-core/src/vad.rs` provides voice activity detection (energy-based VAD with state machine).
- `ale-core/src/actions.rs` defines the action protocol for desktop automation (click, type, key, scroll, file ops).
- `ale-core/src/context.rs` manages conversation context, visual memory, and long-term memory with auto-compaction.
- Config defaults are created by `ConfigFactory::create_default()` under the user config directory `ale-my-eyes/config.json`; test config uses `/tmp/ale-my-eyes-test/config.json`.
- CLI subcommands exist in `ale-cli/src/main.rs`, but transcribe/synthesize/describe/status are still TODO stubs.
- Cloud integration in `ale-core/src/cloud.rs` is OpenAI-shaped by default (`gpt-4o`, `whisper-1`, `tts-1`); do not introduce real API keys into tracked files.

## Packaging
- GitHub Actions is the source of truth for release artifacts: tag `v*` or manual dispatch publishes exactly one Ubuntu `.deb`, one Windows `.exe`, and one Android `.apk` to the GitHub Release.
- Linux packaging: `./scripts/package-linux.sh` builds release binaries and writes `ale-my-eyes-linux/` plus an archive in the repo root.
- Windows packaging: `./scripts/package-windows.sh` adds the `x86_64-pc-windows-msvc` target and writes `ale-my-eyes-windows/` plus a zip when zip/7z exists.
- Android packaging: `./scripts/package-android.sh` requires `ANDROID_NDK_ROOT`, installs `cargo-apk` if missing, and builds APKs via `cargo apk build`.
- `./scripts/create-release.sh` deletes and recreates `release/`; use it only when intentionally regenerating release bundles.
