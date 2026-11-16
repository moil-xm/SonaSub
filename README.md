# SonaSub

**English** | [简体中文](README.zh-CN.md)

Open-source real-time subtitling project. The desktop frontend now communicates with a real local Node.js engine through Tauri Rust, with start, status, stop, restart, failure reporting and exit cleanup. The Chrome extension still contains only a popup. Audio capture, speech recognition, translation and extension capture are not implemented yet.

## Tech Stack

- Tooling: pnpm workspace + Vite+ (`vp` drives formatting, linting, type checking and builds)
- Desktop: Tauri 2 + Rust, frontend in React + TypeScript
- Styling and components: Tailwind CSS 4 + CSS Variables, shadcn/ui, static Iconify icons
- Interface languages: i18next + react-i18next, shared `en-US` / `zh-CN` resources and typed selectors
- Routing and state: TanStack Router / Query / Form, Zustand, Day.js
- Local engine: Node.js 24 LTS; TypeScript in development, a bundled runtime and self-contained build in installers
- Extension: WXT + React + TypeScript (Manifest V3)

## Requirements

- Node.js 24 LTS (`^24.11.0`)
- pnpm 12 (`packageManager` is pinned to `pnpm@12.15.0`)
- Rust stable ≥ 1.90 (with `rustfmt` and `clippy`), required only for the desktop native backend
- macOS needs Xcode Command Line Tools; Linux needs the Tauri 2 system dependencies such as WebKitGTK

## Setup

```bash
pnpm install
```

## Commands

```bash
pnpm dev:desktop        # Desktop frontend dev server at http://localhost:5173
pnpm dev:desktop:app    # Tauri desktop app (compiles Rust and opens a window)
pnpm dev:extension      # WXT extension development
pnpm start:engine       # Refuses standalone startup without Rust-owned authentication (expected failure)

pnpm test               # Protocol, real Node.js engine, desktop UI and i18n tests
pnpm check              # Formatting + linting + type checking
pnpm typecheck          # Type checking only
pnpm lint               # Linting only
pnpm fmt                # Formatting only
pnpm check:rust         # cargo fmt --check + clippy
pnpm build              # Build desktop / engine / extension
pnpm --filter @sonasub/desktop prepare:engine # Prepare the distributed Node.js runtime and engine
pnpm --filter @sonasub/desktop tauri:build    # Build the current platform installer
pnpm --filter @sonasub/desktop exec tauri build --debug --bundles app --no-sign # Local unsigned macOS verification build
pnpm build:extension:zip # Package the extension zip
```

The overview page exposes engine state, start/stop/restart actions, real versions and startup time. Development diagnostics show the PID and loopback port; authentication credentials never reach the frontend.

The desktop header and extension popup expose the UI language setting. Preferences persist across restarts and follow the system language by default.

The local macOS verification bundle is `apps/desktop/src-tauri/target/debug/bundle/macos/SonaSub.app`. Building downloads and checksum-verifies Node.js 24.22.1; the runtime and engine live under `Contents/Resources/`. An unsigned local build is not a signed/notarized release.

Extension development output lives in `apps/extension/.output/chrome-mv3-dev` and production output in `apps/extension/.output/chrome-mv3`; load either from the Chrome extensions page via "Load unpacked".

## Structure

```
apps/desktop      Desktop frontend and the src-tauri native backend
apps/engine       Loopback WebSocket engine and real process information
apps/extension    Chrome extension popup
packages/ui       Shared components, theme, icons and styles
packages/i18n     Shared interface translations and typed i18next setup
packages/protocol Desktop-backend/engine communication contract
.agents/skills    Project-specific Skills
docs/architecture.md  Module responsibilities and dependency boundaries
AGENTS.md         Repository-wide development conventions
```

Development conventions are in [AGENTS.md](AGENTS.md) and module boundaries in [docs/architecture.md](docs/architecture.md); both are written in English.

## Current Verification

- `pnpm install --frozen-lockfile`, `pnpm check`, `pnpm check:rust`, `pnpm test` (22 tests), `pnpm build`.
- `cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml -- --test-threads=1` (14 tests): real-process concurrency, stop/restart, unexpected exit, control-connection loss, authentication isolation and exit cleanup.
- The same 11 real-process Rust integration tests pass with external network access denied and loopback allowed.
- A macOS arm64 `.app` is built and launched with its bundled Node.js and engine artifact; start/stop/restart, failure recovery, unexpected-exit reporting, oversized-peer isolation and managed-child exit cleanup are checked in the real app.
- The CI workflow defines frozen installs, checks, TypeScript tests, builds, prepared engine resources and serial Rust tests; standalone engine startup is checked as an expected credential failure. These changes have not been pushed, so a new remote CI run is not yet verified.
- Windows, Linux and macOS x64 installer execution remain unverified.

## Star History

<a href="https://www.star-history.com/?repos=moil-xm%2FSonaSub&type=date&legend=top-left">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/chart?repos=moil-xm/SonaSub&type=date&theme=dark&legend=top-left" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/chart?repos=moil-xm/SonaSub&type=date&legend=top-left" />
   <img alt="Star History Chart" src="https://api.star-history.com/chart?repos=moil-xm/SonaSub&type=date&legend=top-left" />
 </picture>
</a>
