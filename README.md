# SonaSub

**English** | [简体中文](README.zh-CN.md)

Open-source real-time subtitling project. The repository is currently at the engineering-skeleton stage: a desktop app shell, a Chrome extension popup and a local engine entry point, together with shared tooling configuration and collaboration conventions. Audio capture, speech recognition and translation are not implemented yet.

## Tech Stack

- Tooling: pnpm workspace + Vite+ (`vp` drives formatting, linting, type checking and builds)
- Desktop: Tauri 2 + Rust, frontend in React + TypeScript
- Styling and components: Tailwind CSS 4 + CSS Variables, shadcn/ui, static Iconify icons
- Routing and state: TanStack Router / Query / Form, Zustand, Day.js
- Local engine: Node.js 24 LTS running TypeScript directly
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
pnpm start:engine       # Run the engine entry point and exit cleanly

pnpm check              # Formatting + linting + type checking
pnpm typecheck          # Type checking only
pnpm lint               # Linting only
pnpm fmt                # Formatting only
pnpm check:rust         # cargo fmt --check + clippy
pnpm build              # Build desktop / engine / extension
pnpm build:extension:zip # Package the extension zip
```

The desktop native backend can also be built on its own: `pnpm --filter @sonasub/desktop exec tauri build --debug --no-bundle`. `bundle.active` is currently `false`, so no installer is produced.

Extension development output lives in `apps/extension/.output/chrome-mv3-dev` and production output in `apps/extension/.output/chrome-mv3`; load either from the Chrome extensions page via "Load unpacked".

## Structure

```
apps/desktop      Desktop frontend and the src-tauri native backend
apps/engine       Local processing engine entry point
apps/extension    Chrome extension popup
packages/ui       Shared components, theme, icons and styles
packages/protocol Home of cross-module communication contracts (currently empty)
.agents/skills    Project-specific Skills
docs/architecture.md  Module responsibilities and dependency boundaries
AGENTS.md         Repository-wide development conventions
```

Development conventions are in [AGENTS.md](AGENTS.md) and module boundaries in [docs/architecture.md](docs/architecture.md); both are written in Chinese.
