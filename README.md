# SonaSub

**English** | [简体中文](README.zh-CN.md)

Open-source real-time subtitling for system audio. SonaSub captures the macOS
output device, runs streaming or segmented speech recognition with
sherpa-onnx, associates speakers, translates locally with Bergamot or through
an optional OpenAI-compatible endpoint, and renders live captions. The complete
runtime is native Rust and the packaged desktop app contains no Node.js.

Source text appears as soon as recognition produces it; speaker labels and
translation are independent asynchronous updates on the same subtitle segment.
There are no mock results, fallback engines, fabricated streaming translations
or microphone fallbacks.

## Features

- macOS system-output capture through Core Audio process taps (macOS 14.4+ arm64).
- Streaming Zipformer zh-en recognition with partial and final text.
- Segmented Silero VAD + SenseVoice recognition for zh, en, ja, ko and yue.
- Segmented Silero VAD + Qwen3-ASR 0.6B/1.7B INT8 recognition for 30 languages
  and 22 Chinese dialects, with final captions only.
- CAM++ speaker embeddings and bounded-window Pyannote diarization.
- Local Bergamot translation for en -> zh-Hans and zh-Hans -> en.
- Optional OpenAI-Compatible translation with a custom Base URL, model and API
  key for 30 target languages. Caption text leaves the Mac only when this
  engine is selected.
- Rust subtitle scheduling with stable IDs, revisions and independent text,
  speaker and translation patches.
- React live-caption workbench with model/mode configuration, model readiness,
  session controls, notices and telemetry.
- Offline recognition and Bergamot translation once all required models are
  present; the OpenAI-compatible engine needs access to its endpoint.

The Chrome extension remains a separate Manifest V3 project with only a popup.
Tab capture is future work and is not used as a hidden audio fallback.

## Technology

- Tooling: pnpm workspace + Vite+ (`vp` for formatting, linting, type checking,
  tests and builds)
- Desktop: Tauri 2 + Rust; React + TypeScript
- UI: Tailwind CSS 4 + CSS Variables, shadcn/ui, static Iconify icons
- Routing/state: TanStack Router / Query / Form, Zustand, Day.js
- Native speech: official `sherpa-onnx` Rust API v2.0.1
- Native system audio: `flexaudio 0.8.1` / `flexaudio-os-macos 0.8.1`
- Local translation: `fxtranslate 0.5.0` with pinned Bergamot models
- Remote translation: `adk-model 2.3.0` OpenAI-compatible client
- Extension: WXT + React + TypeScript (Manifest V3)

## Requirements

- Node.js 24 LTS (`^24.11.0`) and pnpm 12 for development and model preparation.
  Node.js is not used by the packaged desktop app.
- Rust stable >= 1.95 with `rustfmt` and `clippy`.
- macOS 14.4 or later on Apple Silicon (arm64); Xcode Command Line Tools.
- `curl`, `tar` and `zstd` for model preparation (`brew install zstd` if needed).

System capture is implemented and verified on macOS arm64. Windows, Linux and
macOS x64 are not verified; starting a session there returns an explicit
unsupported-platform error.

## Setup

```bash
pnpm install
pnpm prepare:models
pnpm dev:desktop:app
```

The first capture asks for macOS System Audio Recording permission. SonaSub
does not request microphone input as a substitute.

The official `sherpa-onnx` crate downloads and statically links a runtime that
matches v2.0.1 during the Rust build. It is not a separate service or bundled
Node.js process.

Model files live outside the installer:

```text
~/Library/Application Support/dev.sonasub.desktop/models/
```

Sources, versions, checksums and the resume/cache workflow are documented in
[docs/models.md](docs/models.md).

In the packaged app, open **Models** to choose which models to install. Each
bundle card shows approximate download size, disk and compute requirements,
and a short quality summary. Use **Download and install** to fetch the pinned
official archive or files with one click, or import files you already
downloaded or the extracted model folder. You can also choose a separate
download directory. Nothing downloads unless you start it from the model page.

### Qwen3 ASR model sources

`qwen3-asr-0.6b-int8` is the official sherpa-onnx INT8 release from
2026-03-25. `qwen3-asr-1.7b-int8` uses the ModelScope ONNX export by
`zengshuishui`, pinned to revision
`9c182309f7bb075f241424441add9e16c5086dfb`. The ModelScope card declares
Apache-2.0 and links the sherpa-onnx compatibility path, while explicitly
describing the export as a reference implementation without a production
readiness guarantee.

Both variants use the same Rust `Qwen3Backend`. SonaSub feeds Silero VAD
segments into the offline recognizer and emits final captions only. The
selected sherpa-onnx 2.0.1 Qwen stream has no language override, so Qwen models
use automatic language detection and the session language control is disabled.
There are no native Qwen partial captions.

## Commands

```bash
pnpm dev:desktop        # Desktop frontend dev server at http://localhost:5173
pnpm dev:desktop:app    # Tauri desktop app (compiles Rust)
pnpm dev:extension      # WXT extension development

pnpm prepare:models     # Download and checksum every pinned model
pnpm check              # Formatting + linting + type checking
pnpm check:fix          # Auto-fix formatting and fixable lint issues
pnpm typecheck          # Type checking only
pnpm lint               # Linting only
pnpm fmt                # Formatting only
pnpm check:rust         # cargo fmt --check + clippy -D warnings
pnpm test               # Desktop UI/state and shared i18n tests
pnpm build              # Build desktop frontend and extension

pnpm --filter @sonasub/desktop tauri:build
pnpm --filter @sonasub/desktop exec tauri build --debug --bundles app --no-sign
pnpm build:extension:zip
```

Rust tests:

```bash
cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml -- --test-threads=1
```

The large-model tests are ignored by default:

```bash
cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml \
  --test native_pipeline -- --ignored --test-threads=1 --nocapture
```

## Configuration

The preferences panel controls:

- system output device;
- ASR mode (`auto` / `streaming` / `segmented`), model and source language;
- maximum segment length, VAD threshold, silence/speech minimums and the
  stable-segment wait;
- translation engine (`bergamot` / `openai-compatible`), mode
  (`auto` / `streaming` / `text`), trigger
  (`finalOnly` / `stableSegment`) and target language;
- OpenAI-Compatible Base URL, remote model and API key;
- speaker embedding/diarization, window length, speaker capacity and matching
  threshold;
- subtitle history limit.

`auto` resolves from the selected model and compiled adapter. Explicit
unsupported modes are rejected and disabled in the UI. Bergamot resolves to
complete-text translation only; `streaming` is not available. Configuration is
saved atomically, locked while a session is active and applied by the next
start or restart.

Bergamot exposes only its two pinned directions (`en` and `zh-Hans` targets);
the other target languages are shown as disabled and require
OpenAI-Compatible. The remote adapter accepts all 30 registered target
languages, but the remote model ultimately decides which ones it supports.
Its Base URL must be an absolute `http://` or `https://` URL. A trailing `/`
and a mistaken `/chat/completions` suffix are removed before the request; an
empty API key is allowed for unauthenticated local endpoints.

The OpenAI-compatible Base URL, model and API key are stored as plain text in
the local `config.json` and in each record's `records.sqlite` configuration
snapshot. Debug output, logs and surfaced errors redact the key. There is no
environment-variable fallback.

## Architecture

```
apps/desktop                 React workbench + src-tauri Rust application
  src/                       Tauri IPC/events, live captions and configuration
  src-tauri/src/audio/       Core Audio system capture and 48k -> 16k DSP
  src-tauri/src/asr/         Streaming Zipformer, segmented SenseVoice and Qwen3
  src-tauri/src/speaker/     CAM++ registry and windowed diarization
  src-tauri/src/translation/ TranslationEngine boundary, Bergamot and remote adapters
  src-tauri/src/subtitles/   Stable IDs, revisions and patch scheduling
  src-tauri/src/session/     Live caption lifecycle, state and events
  src-tauri/src/records/     Saved recordings, device runs and SQLite storage
  src-tauri/src/pipeline/    Shared inference workers, queues and metrics
  src-tauri/src/models/      Model registry, capabilities and installation
apps/extension               Chrome extension popup (no capture yet)
packages/ui                  Shared components, theme, icons and styles
packages/i18n                English and Simplified Chinese UI resources
docs/architecture.md         Runtime boundaries and pipeline details
docs/models.md               Model sources, versions and checksums
docs/validation.md           Executed checks, native results and open gaps
```

React controls Rust only through Tauri commands and events. Audio samples,
model handles and inference workers never cross into JavaScript. See
[docs/architecture.md](docs/architecture.md) for the full pipeline and
[AGENTS.md](AGENTS.md) for development conventions.

## Packaging

The macOS bundle is:

```text
apps/desktop/src-tauri/target/debug/bundle/macos/SonaSub.app
```

The bundle contains the executable, icon and plist only. It has no Node.js
runtime, engine artifact, WebSocket protocol or model weights. A local
unsigned build is not a signed/notarized release.

## Verification

The final local verification is recorded in the delivery report for this
change. It covers TypeScript checks and tests, Rust format/clippy/tests, the
ignored native model tests, a macOS `.app` bundle inspection with real system
output playback, both ASR paths, both Bergamot directions, speaker/overlap
behavior, configuration persistence, stop/restart and exit cleanup.

Windows, Linux and macOS x64 installers remain unverified.

## Star History

<a href="https://www.star-history.com/?repos=moil-xm%2FSonaSub&type=date&legend=top-left">
 <picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/chart?repos=moil-xm/SonaSub&type=date&theme=dark&legend=top-left" />
  <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/chart?repos=moil-xm/SonaSub&type=date&legend=top-left" />
  <img alt="Star History Chart" src="https://api.star-history.com/chart?repos=moil-xm/SonaSub&type=date&legend=top-left" />
 </picture>
</a>
