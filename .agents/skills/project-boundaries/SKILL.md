---
name: project-boundaries
description: Use when deciding where SonaSub code belongs or how modules connect - desktop frontend calling Tauri commands, src-tauri Rust vs engine ownership, Chrome extension permissions or messaging, shared packages, packages/protocol contract types, or any new dependency between apps and packages.
---

# project-boundaries

## Read First

- [docs/architecture.md](../../../docs/architecture.md): responsibility table and allowed dependency direction.
- [AGENTS.md](../../../AGENTS.md): boundaries and current stage.
- Entries on both sides: `apps/desktop/src-tauri/src/{lib,engine,protocol,resources}.rs`, `apps/desktop/src-tauri/capabilities/default.json`, `apps/engine/src/main.ts`, `apps/extension/wxt.config.ts`, `packages/protocol/README.md`.

## Steps

1. Draw the data flow: who produces the data, who consumes it, and which runtime boundary it crosses (Tauri IPC / extension messaging / engine process). Decide ownership before writing any API.
2. Place by ownership: native capabilities → `src-tauri` (new commands update `capabilities` permissions, kept minimal); local processing → `apps/engine`; browser/extension capabilities → the owning app; cross-app types or contracts → `packages/protocol`; shared UI → `packages/ui`.
3. Check the dependency direction: apps never import each other's internal source; packages never depend on apps; `packages/ui` never contains `@tauri-apps/*`, `chrome.*` or `node:*`. Cross-app reuse becomes an explicit `exports` entry under `packages/*`.
4. The engine protocol covers only authentication, runtime information, control responses and shutdown notification; Rust owns the process/connection/credentials and monitors both the child and the established control connection. React goes through Tauri IPC only. Update TS, Rust and tests together when the contract changes, and do not predefine audio/subtitle/translation fields.
5. Add platform-specific dependencies only to the owning app; confirm a dependency added to a shared package holds for every consumer.
6. Use `rg` to check for out-of-bounds imports (for example `apps/extension` referencing `apps/desktop/src`, or `packages/ui` referencing `node:`).

## Constraints

- Follow the minimal API rule in AGENTS.md: satisfy the real call site first, never reserve interfaces for "might be needed later".
- Do not add undecided audio/recognition/translation/multi-tab flows.

## Verification

- `pnpm check` + `pnpm build`; Rust changes add `pnpm check:rust` and `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -- --test-threads=1`; engine changes add `pnpm test`.
- `pnpm start:engine` must refuse to start without Rust-owned credentials and is no longer a success criterion. Lifecycle changes run `pnpm dev:desktop:app` or the real `.app` to verify start, stop/restart, unexpected exit, control-connection loss, recovery and exit cleanup.
- Distribution-resource changes run `pnpm --filter @sonasub/desktop prepare:engine` plus a real installer build/launch; state clearly which platforms could not be verified.

## Delivery

Ownership and dependency-direction changes, new contracts or permissions with their rationale, boundary-check results, commands run with results, and unverified items.
