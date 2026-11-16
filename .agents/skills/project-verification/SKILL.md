---
name: project-verification
description: Use when finishing a SonaSub change and deciding which checks to run (pnpm check, test, build, check:rust, engine lifecycle and packaged-resource verification), whether docs-only or Skills-only edits need compiling, executing checks, reporting results and gaps, or locating the owning workspace after a check or build fails.
---

# project-verification

## Read First

- [AGENTS.md](../../../AGENTS.md): commands and delivery requirements.
- [package.json](../../../package.json) and the `package.json` scripts of the affected workspace.
- `git status --porcelain=v1` and `git diff --stat` to confirm the change scope and preserve existing staged/unstaged/untracked content.

## Choose by Scope

| Change scope                     | Commands                                                                                                                                                  |
| -------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Any TS/TSX/config                | `pnpm check`                                                                                                                                              |
| Desktop frontend                 | `pnpm --filter @sonasub/desktop build`; for UI changes also `pnpm dev:desktop`                                                                            |
| Tauri / Rust                     | `pnpm check:rust` + `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -- --test-threads=1`; run the real app for UI changes                   |
| Engine                           | `pnpm test` + `pnpm --filter @sonasub/engine build`; standalone `pnpm start:engine` must refuse to start without credentials                              |
| Engine lifecycle / distribution  | `pnpm --filter @sonasub/desktop prepare:engine`; build and run the macOS `.app` and check real information, restart, unexpected exit and app-exit cleanup |
| Extension                        | `pnpm --filter @sonasub/extension build` (output `.output/chrome-mv3`, `manifest_version` 3); for interaction changes also `pnpm dev:extension`           |
| Workspace / dependencies / locks | `pnpm install` then `pnpm build`                                                                                                                          |
| All apps                         | `pnpm build`                                                                                                                                              |
| Docs / Skills only               | frontmatter and reference-path checks, `git diff --check`; no compilation required                                                                        |

## Steps

1. List the changed files and map them to the table; run only the commands that cover the change, expanding to `pnpm build` when necessary.
2. Run each command and read its full output and exit code; on failure locate the owning workspace and root cause, fix it, then re-run the same command. Never swallow errors.
3. Skill changes: check that each `SKILL.md` `name` matches its directory, that `description` is non-empty, that referenced relative paths exist and that commands exist in `package.json`.
4. Record items the environment cannot complete (GUI required, missing Rust system dependencies) with the reason, never with "should work".
5. Verify the lifecycle with real processes: concurrent starts do not duplicate, wrong credentials cannot control the engine, failed starts keep engine diagnostics, stop/restart update PID/port consistently, unexpected exit surfaces as failure, and app exit cleans only the managed child process. Offline verification targets built artifacts only and needs no internet.
6. Re-run the checks covering the final change before finishing, so results match the current files.

## Constraints

- Never add filler tests for logic that does not exist; never fix unrelated failures for a local change, only report them.
- Do not re-run checks that already passed unless files changed again or an open risk remains.

## Delivery

Change scope, each command with its result (passed/failed/not run and why), and remaining risks.
