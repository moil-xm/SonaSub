---
name: project-boundaries
description: Use when deciding where SonaSub code belongs or how modules connect - React calling Tauri commands/events, src-tauri Rust ownership, audio/ASR/speaker/translation/subtitle modules, Chrome extension permissions or messaging, shared packages, or any new dependency between apps and packages.
---

# project-boundaries

## Read First

- [docs/architecture.md](../../../docs/architecture.md): native pipeline, dependency direction, lifecycle and event contract.
- [docs/models.md](../../../docs/models.md): model registry, preparation and capability limits.
- [AGENTS.md](../../../AGENTS.md): boundaries and current scope.
- Desktop entries: `apps/desktop/src-tauri/src/lib.rs`, `session/mod.rs`, `pipeline/mod.rs`, `audio/mod.rs`, `asr/mod.rs`, `speaker/mod.rs`, `translation/mod.rs`, `subtitles/mod.rs`, `models/mod.rs`, `config.rs`.
- Frontend entries: `apps/desktop/src/lib/session.ts`, `session.type.ts`, `session-cache.ts`, `apps/desktop/src-tauri/capabilities/default.json`, `apps/extension/wxt.config.ts`.

## Steps

1. Draw the data flow: who produces the data, who consumes it, and which boundary it crosses (Tauri IPC/event, Rust worker channel, browser extension messaging). Decide ownership before writing an API.
2. Place by ownership: system capture, DSP, model loading, inference workers, subtitle scheduling and translation → `apps/desktop/src-tauri`; React control/presentation → `apps/desktop/src`; browser-specific behavior → `apps/extension`; cross-app UI/i18n → `packages/*`.
3. Keep the process boundary narrow. React receives typed snapshots and subtitle patches; it never receives continuous audio, model handles or credentials. New commands/events update the Rust serializer, `session.type.ts`, the Query cache reducer and tests together.
4. New Tauri commands update `capabilities/default.json` only when a permission is actually needed; prefer the existing minimal command set. Do not reintroduce a local server, WebSocket, HTTP or Node process.
5. Keep inference behind the existing boundaries: `AsrEngine`, `TranslationEngine`, speaker registry/diarization, and the subtitle store. Add a compile-time adapter rather than scattering engine-name checks. Do not create one engine per model or a dynamic plugin loader.
6. Model capabilities come from `models/` (model + adapter + interface), not from UI flags. A user preference may request a mode; the backend resolves it and rejects unsupported explicit modes.
7. The extension stays isolated: it must not call desktop Tauri IPC, import apps/desktop source, or claim capture functionality that does not exist. Future tab capture is a new source adapter, not a hidden fallback.
8. Use `rg` to check out-of-bounds imports (for example extension importing desktop source, `packages/ui` importing `@tauri-apps/*`, or desktop bundling `node:*` at runtime).

## Constraints

- Follow the minimal API rule in AGENTS.md: satisfy the real call site first, never reserve interfaces for "might be needed later".
- Do not add microphone capture as a fallback for system audio, or any simulated/mock inference path.
- Model weights stay outside the application bundle; preparation is developer tooling.
- Keep macOS-only behavior explicit. Unsupported platforms must report the capability as unavailable.

## Verification

- `pnpm check` + `pnpm build` for TypeScript/contract changes.
- Rust changes: `pnpm check:rust` and serial `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -- --test-threads=1`.
- Model/pipeline changes: run the ignored native tests with prepared models and verify the real `.app` with system-output playback. State clearly which platforms were not verified.
- Packaging/resource changes: rebuild the macOS `.app` and inspect that no Node.js, engine, protocol or bundled model files remain.

## Delivery

Ownership and dependency-direction changes, contract/permission rationale, boundary-check results, commands run with results, and unverified items.
