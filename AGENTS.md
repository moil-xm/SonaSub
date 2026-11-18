# SonaSub

Open-source real-time subtitling project. The desktop app captures macOS system audio and runs sherpa-onnx recognition, speaker analysis and Bergamot translation locally in Rust; React controls the session through Tauri IPC and events. pnpm workspace + Vite+ manage the JS/TS toolchain, while Tauri and WXT keep their own build entries.

## Language

- Primary descriptions are English: documentation, code comments, log and error messages, and UI copy.
- Chinese appears only in localized files such as `README.zh-CN.md`, and in explicit language labels or annotations.

## Tech Stack and Module Responsibilities

| Module           | Responsibility                                                                                  | Technology                                                                               |
| ---------------- | ----------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| `apps/desktop`   | React control UI; Rust system-audio capture, inference workers, subtitles and session lifecycle | React + TypeScript, TanStack Router/Query/Form, Zustand, Tauri 2 + Rust                  |
| `apps/extension` | Chrome extension (Manifest V3), popup only for now                                              | WXT + React + TypeScript                                                                 |
| `packages/ui`    | Shared components, theme variables, icons, styles, hooks                                        | shadcn/ui, Tailwind CSS 4 + CSS Variables, static Iconify icons, Day.js provided by apps |
| `packages/i18n`  | Shared English and Simplified Chinese resources and typed i18next setup                         | i18next + react-i18next                                                                  |

Dependency direction and import conventions are in [docs/architecture.md](docs/architecture.md).

## Boundaries

- Apps never import each other's internal source; packages never depend on apps.
- `packages/ui` must not call Tauri, Chrome extension or Node-only APIs.
- Native capabilities and local processing live in `apps/desktop/src-tauri`; platform-specific code stays in the owning app.
- Cross-app reuse goes through `packages/*` only, and new exports are added to that package's `exports`.
- React controls the session only through Tauri IPC and events; continuous audio, model handles, worker threads and lifecycle stay in Rust.
- The packaged desktop app must not bundle, download or launch Node.js. The extension is isolated from desktop Tauri IPC and has no capture implementation yet.
- Model weights live in the platform application-data directory, never in the installer; see [docs/models.md](docs/models.md).

## File Organization and Naming

- Directories and non-component modules use kebab-case; React components and types use PascalCase, functions and variables use camelCase.
- Every React component lives in its own directory named after it in kebab-case, following the Ant Design `components/<name>/` layout. `apps/desktop/src/components/live-subtitles/` holds `LiveSubtitles.tsx`, its colocated `LiveSubtitles.test.tsx` and `LiveSubtitles.type.ts`, the named sub-components (`SubtitleRow.tsx`) and an `index.ts` public entry that only re-exports.
- The same layout applies to `.ts` modules: `apps/desktop/src/lib/session/` holds `session.ts`, its colocated `session.type.ts` and an `index.ts` public entry. PascalCase stays reserved for React components and types; a module keeps its kebab-case file name.
- Consumers import the component directory (`@/components/live-subtitles`), never the implementation file inside it.
- A directory that groups sibling files of one kind is a collection, not a module, and keeps its own `index.ts` (`packages/i18n/src/resources/`); app entry files (`main.tsx`, `routeTree.ts`) stay at the workspace root.
- Desktop pages live in `apps/desktop/src/routes/<route>/` and are registered in `src/routeTree.ts`; layout components live in `src/components`.
- Shared shadcn/ui primitives stay flat in `packages/ui/src/components/<name>.tsx` and `packages/ui/src/lib/utils.ts` because the shadcn CLI writes flat files; package-internal imports use `#lib/*`, desktop-internal imports use `@/*`.
- Declare dependencies in the workspace that uses them; dev tools go to `devDependencies`; versions are pinned exactly (`.npmrc`).

## TypeScript / React / Rust

- TypeScript runs in strict mode (`tsconfig.base.json`: `strict`, `noUncheckedIndexedAccess`, `verbatimModuleSyntax`); type imports use `import type`.
- React uses function components and hooks, never class components; compose component props with `React.ComponentProps<...>`.
- Rust follows `cargo fmt` and `cargo clippy -D warnings`; new Tauri commands update `capabilities` in the same change.
- Follow KISS, DRY and YAGNI: trust internal types and contracts, validate only at external inputs and system boundaries; no duplicate checks, pointless try-catch, error-swallowing fallbacks or abstractions without a reuse need. Details are in [.cursor/rules/minimal-api-surface.mdc](.cursor/rules/minimal-api-surface.mdc).
- Never fake subtitles, simulated recognition results or empty implementations to make a feature look finished.

## shadcn / Tailwind / Theme

- Add components on demand with `pnpm dlx shadcn@latest add <component>` into `packages/ui` (`components.json` is configured in `packages/ui` and `apps/desktop`); prefer official components and composition over a parallel wrapper system.
- Style only with Tailwind utilities and the CSS variables in `globals.css`; new colors start as variables mapped in `@theme inline`, never hard-coded values.
- Theme switching uses the store from `@sonasub/ui/theme` and `<html class="dark">`.
- Icons use `@iconify/react` with statically imported `@iconify-icons/lucide/<name>`; never load them at runtime.

## State Responsibilities

- TanStack Query: session capabilities, configuration, snapshot and action results; local IPC queries and actions use `networkMode: 'always'`, and Rust events update the cache by subtitle ID.
- TanStack Form: configuration fields and validation state; the form saves before a session starts and is locked while a session is active.
- Zustand: local interaction state such as the theme; never async resource caches.

## Commands

Run from the repository root:

```bash
pnpm install                 # Install dependencies (runs wxt prepare)
pnpm check                   # Vite+ formatting + linting + type checking
pnpm check:fix               # Auto-fix formatting and fixable lint issues
pnpm typecheck               # Type checking only
pnpm lint                    # Linting only
pnpm fmt                     # Formatting only
pnpm check:rust              # cargo fmt --check + clippy -D warnings
pnpm build                   # vp run -r build: desktop (Vite+) and extension (WXT)
pnpm test                    # Desktop UI/state and shared i18n tests
pnpm dev:desktop             # Desktop frontend dev server (http://localhost:5173)
pnpm dev:desktop:app         # Tauri desktop app (compiles Rust)
pnpm dev:extension           # WXT extension development (Chrome)
pnpm prepare:models          # Download and checksum the pinned local models
pnpm --filter @sonasub/desktop tauri:build    # Build the installer for the current platform
```

## Skills

Project Skills live in [.agents/skills](.agents/skills); pick by task, reading all of them is not required:

| Task                                                     | Skill                                                                        |
| -------------------------------------------------------- | ---------------------------------------------------------------------------- |
| Workspace, dependencies, toolchain, exports, scripts     | [project-workspace](.agents/skills/project-workspace/SKILL.md)               |
| Pages, shared components, theme, forms, frontend state   | [project-frontend](.agents/skills/project-frontend/SKILL.md)                 |
| Naming, types and state habits for handwritten React/TSX | [react-conventions](.agents/skills/react-conventions/SKILL.md)               |
| Desktop/Rust/extension/shared package ownership          | [project-boundaries](.agents/skills/project-boundaries/SKILL.md)             |
| Rust implementation, ownership, errors, concurrency, FFI | [rust-engineering](.agents/skills/rust-engineering/SKILL.md)                 |
| System capture, realtime audio, timing and backpressure  | [native-audio-pipeline](.agents/skills/native-audio-pipeline/SKILL.md)       |
| Native session startup, shutdown, worker ownership       | [native-session-lifecycle](.agents/skills/native-session-lifecycle/SKILL.md) |
| Choosing checks for the change and preparing delivery    | [project-verification](.agents/skills/project-verification/SKILL.md)         |

Add a Skill only when a stable, repeated, project-specific judgment rule exists; never create one Skill per third-party library.

## Delivery Requirements

- State the change scope and owning module; list the verification commands actually run with their results, plus anything unverified in this environment.
- The official `sherpa-onnx` crate downloads and links its matching static runtime during compilation; no separate native preparation step is required.
- Check `git status` before and after changes, preserve existing staged, unstaged and untracked work, and never restore, delete or reformat unrelated files.
- Docs-only or Skills-only changes: check frontmatter, reference paths and `git diff --check`; no compilation required.

## Current Scope

- The native desktop pipeline is implemented: macOS system-output capture, streaming and segmented ASR, windowed speaker analysis, Bergamot translation, subtitle scheduling and the live caption UI.
- The Chrome extension still contains only a popup. Tab capture and extension-to-desktop communication are future work, not a hidden fallback path.
- System capture is implemented and verified for macOS 14.4+ arm64. Other platforms and architectures must be reported as unverified rather than silently using another audio source.
- Native workers run in the desktop process; a native crash is not isolated from the UI.
