# SonaSub

Open-source real-time subtitling project. The desktop app and the local engine already communicate for real, including the full process lifecycle; pnpm workspace + Vite+ manage the JS/TS toolchain, while Tauri and WXT keep their own build entries.

## Language

- Primary descriptions are English: documentation, code comments, log and error messages, and UI copy.
- Chinese appears only in localized files such as `README.zh-CN.md`, and in explicit language labels or annotations.

## Tech Stack and Module Responsibilities

| Module              | Responsibility                                                     | Technology                                                                               |
| ------------------- | ------------------------------------------------------------------ | ---------------------------------------------------------------------------------------- |
| `apps/desktop`      | React state and control UI; Rust process, connection and lifecycle | React + TypeScript, TanStack Router/Query/Form, Zustand, Tauri 2 + Rust                  |
| `apps/engine`       | Loopback WebSocket service and real runtime information            | Node.js 24 LTS; TypeScript in development, self-contained build in distribution          |
| `apps/extension`    | Chrome extension (Manifest V3)                                     | WXT + React + TypeScript                                                                 |
| `packages/ui`       | Shared components, theme variables, icons, styles                  | shadcn/ui, Tailwind CSS 4 + CSS Variables, static Iconify icons, Day.js provided by apps |
| `packages/protocol` | Contracts between the desktop backend and the engine               | TypeScript types and boundary parsing                                                    |

Dependency direction and import conventions are in [docs/architecture.md](docs/architecture.md).

## Boundaries

- Apps never import each other's internal source; packages never depend on apps.
- `packages/ui` must not call Tauri, Chrome extension or Node-only APIs.
- Native capabilities live in `src-tauri`, local processing in `apps/engine`, platform code in the owning app.
- Cross-app reuse goes through `packages/*` only, and new exports are added to that package's `exports`.
- React controls the engine only through Tauri IPC; the process handle, port, credentials and lifecycle stay in Rust. The engine listens on `127.0.0.1` only and accepts control requests after authentication.

## File Organization and Naming

- Files and directories use kebab-case; React components and types use PascalCase, functions and variables use camelCase.
- Desktop pages live in `apps/desktop/src/routes` and are registered in `src/routeTree.ts`; layout components live in `src/components`.
- Shared components live in `packages/ui/src/components/<name>.tsx`; package-internal imports use `#lib/*`, desktop-internal imports use `@/*`.
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

- TanStack Query: engine state and action results; local IPC queries and actions use `networkMode: 'always'`, and Rust state events update or invalidate the cache.
- TanStack Form: form fields and validation state (dependency pinned, no forms yet).
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
pnpm build                   # vp run -r build: desktop (Vite+), engine (vp pack), extension (WXT)
pnpm test                    # Protocol and real Node.js engine tests
pnpm dev:desktop             # Desktop frontend dev server (http://localhost:5173)
pnpm dev:desktop:app         # Tauri desktop app (compiles Rust)
pnpm dev:extension           # WXT extension development (Chrome)
pnpm start:engine            # Refuses standalone startup without Rust-owned credentials (expected failure)
pnpm --filter @sonasub/desktop prepare:engine # Prepare the distributed Node.js runtime and engine artifact
pnpm --filter @sonasub/desktop tauri:build    # Build the installer for the current platform
```

## Skills

Project Skills live in [.agents/skills](.agents/skills); pick by task, reading all of them is not required:

| Task                                                     | Skill                                                                |
| -------------------------------------------------------- | -------------------------------------------------------------------- |
| Workspace, dependencies, toolchain, exports, scripts     | [project-workspace](.agents/skills/project-workspace/SKILL.md)       |
| Pages, shared components, theme, forms, frontend state   | [project-frontend](.agents/skills/project-frontend/SKILL.md)         |
| Naming, types and state habits for handwritten React/TSX | [react-conventions](.agents/skills/react-conventions/SKILL.md)       |
| Desktop/Rust/engine/extension/shared package ownership   | [project-boundaries](.agents/skills/project-boundaries/SKILL.md)     |
| Choosing checks for the change and preparing delivery    | [project-verification](.agents/skills/project-verification/SKILL.md) |

Add a Skill only when a stable, repeated, project-specific judgment rule exists; never create one Skill per third-party library.

## Delivery Requirements

- State the change scope and owning module; list the verification commands actually run with their results, plus anything unverified in this environment.
- Check `git status` before and after changes, preserve existing staged, unstaged and untracked work, and never restore, delete or reformat unrelated files.
- Docs-only or Skills-only changes: check frontmatter, reference paths and `git diff --check`; no compilation required.

## Current Stage (temporary, not permanent rules)

- This stage implements engine start, connection, status query, stop, restart, failure reporting and exit cleanup.
- Later tasks will decide: audio capture, speech recognition, translation, model downloads, multi-tab handling and extension capture.
- `packages/protocol` contains only the current engine protocol; the desktop has no TanStack Form form yet; the extension has only a popup.
- These limits are lifted as tasks progress and are not long-term constraints.
