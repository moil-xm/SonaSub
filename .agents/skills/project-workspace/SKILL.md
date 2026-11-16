---
name: project-workspace
description: Use when adding or changing a pnpm workspace, installing or upgrading a dependency (wxt, vite-plus, tauri, react, etc.), updating pnpm-lock.yaml or Cargo.lock, Vite+ toolchain settings, package exports, import aliases, or root/package scripts in SonaSub. Not for page UI or business logic.
---

# project-workspace

## Read First

- [AGENTS.md](../../../AGENTS.md): tech stack, dependency ownership, commands, current stage.
- [package.json](../../../package.json), [pnpm-workspace.yaml](../../../pnpm-workspace.yaml), [.npmrc](../../../.npmrc), [vite.config.ts](../../../vite.config.ts), [tsconfig.base.json](../../../tsconfig.base.json).
- The target workspace's `package.json`, `tsconfig.json` and its build config (`vite.config.ts` / `wxt.config.ts` / `src-tauri/tauri.conf.json`).

## Steps

1. Identify which workspace owns the change; add dependencies only to the package that uses them, and put dev tools in `devDependencies`.
2. Pin exact versions (`.npmrc` already sets `save-exact`). Before upgrading, check with `pnpm view <pkg> engines peerDependencies --json`: Node must be `^24.11.0`; any workspace declaring `vite` must use the alias `npm:@voidzero-dev/vite-plus-core@<vite-plus version>` (`vp` validates that core version), and the `vite` peer of WXT and `@vitejs/plugin-react` is satisfied by it. When upgrading `vite-plus`, update the aliases in the root, desktop and extension together, plus `overrides['vite@*']` in `pnpm-workspace.yaml`.
3. Adding a workspace: create the directory under `apps/*` or `packages/*` with a `package.json` (`private: true`, `type: module`) and a `tsconfig.json` (`extends ../../tsconfig.base.json`). Packages never depend on apps.
4. Adding a shared export: update the package's `exports` at the same time (`packages/ui` internal imports use `#lib/*`) and confirm consumers can resolve it.
5. Configure the toolchain only in the root `vite.config.ts` `lint` / `fmt` blocks, with package-level differences as `overrides`; never add duplicate tools such as ESLint, Prettier or Biome. Rust checks use `cargo fmt` / `clippy`.
   Root `pnpm test` discovers test projects under `apps/*` and `packages/*`; workspace-specific test environments stay in that workspace's Vite config.
6. Scripts: each package keeps its own `dev` / `build`, and the root aggregates with `vp run -r <script>` or `pnpm --filter <name> <script>`; Tauri and WXT keep their own build entries.
7. Run `pnpm install` to update `pnpm-lock.yaml`; after a Rust dependency change run `cargo generate-lockfile` or any cargo command inside `apps/desktop/src-tauri` to update `Cargo.lock`. Lock files must be committed.

## Constraints

- Follow the KISS/DRY/YAGNI and boundary rules in AGENTS.md; do not pre-build workspaces, exports or scripts for capabilities that are not decided yet.
- Never introduce a second formatting/checking toolchain; do not change `packageManager` or the Node major version unless the task explicitly requires it.

## Verification

- Dependency/export/script changes: `pnpm install`, then `pnpm check`; run `pnpm build` for affected apps.
- Root script or config changes only: `pnpm check` plus the changed script itself (for example `pnpm start:engine`).
- Rust dependency changes: `pnpm check:rust`.

## Delivery

Changed workspaces and files, version-choice rationale (compatibility check results), whether lock files were updated, commands run with results, and unverified items.
