---
name: project-verification
description: Use when finishing a SonaSub change and deciding which checks to run (pnpm check, test, build, check:rust, native model tests, packaged .app and real system-audio verification), whether docs-only or Skills-only edits need compiling, reporting results and gaps, or locating the owning workspace after a failure.
---

# project-verification

## Read First

- [AGENTS.md](../../../AGENTS.md): commands and delivery requirements.
- [docs/models.md](../../../docs/models.md): model preparation and ignored native tests.
- [package.json](../../../package.json) and the `package.json` scripts of the affected workspace.
- `git status --porcelain=v1` and `git diff --stat` to confirm change scope and preserve existing staged/unstaged/untracked content.

## Choose By Scope

| Change scope                 | Commands                                                                                                                                    |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| Any TS/TSX/config            | `pnpm check`                                                                                                                                |
| Desktop frontend             | `pnpm test` + `pnpm --filter @sonasub/desktop build`; for UI changes also open `pnpm dev:desktop` or the real `.app`                        |
| Tauri/Rust                   | `pnpm check:rust` + `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -- --test-threads=1`                                      |
| Native models/pipeline       | `pnpm prepare:models`; ignored tests with `--ignored`; then real `.app` verification with system-output playback                            |
| Session lifecycle            | Real `.app`: start, concurrent start, stop, restart, failure, config lock, app-exit cleanup and stale-event isolation                       |
| Extension                    | `pnpm --filter @sonasub/extension build` (output `.output/chrome-mv3`, `manifest_version` 3); interaction changes also `pnpm dev:extension` |
| Workspace/dependencies/locks | `pnpm install` then `pnpm build`                                                                                                            |
| All apps                     | `pnpm build`                                                                                                                                |
| Docs/Skills only             | frontmatter and reference-path checks, `git diff --check`; no compilation required                                                          |

## Steps

1. List changed files and map them to the table; run only the checks that cover the change, expanding to `pnpm build` when necessary.
2. Run each command and read its full output and exit code; on failure find the owning workspace and root cause, fix it, then rerun the same command. Never swallow errors.
3. Skill changes: each `SKILL.md` `name` matches its directory, `description` is non-empty, referenced relative paths exist and commands exist in `package.json`.
4. Record what the environment cannot complete (GUI, permission, missing model, other platform) with the reason, never "should work".
5. Native pipeline verification must be real:
   - start a session and play audio through the macOS system output (not microphone input);
   - verify both streaming and segmented ASR;
   - verify source text arrives before translation and speaker updates;
   - verify both configured Bergamot directions;
   - check multi-speaker attribution, silence, short utterances and overlap uncertainty;
   - test config persistence, model/mode switching, stop, restart and app-exit cleanup;
   - run with models present and network denied to confirm offline operation;
   - record subtitle/translation/speaker latency, CPU and RSS.
6. Packaging: after changing native dependencies or resources, rebuild the macOS `.app` and inspect the bundle for Node.js, engine/protocol artifacts, non-system dynamic libraries and bundled model weights.
7. Native runtime: the official `sherpa-onnx` crate downloads its version-matched static runtime during compilation. Keep the crate version pinned and avoid overriding the native library directory with a stale local build.
8. Model checks: `pnpm prepare:models` verifies every pinned SHA-256. Normal CI must not download large models; the model tests stay `#[ignore]` and are run explicitly.
9. CI gates include frozen installs, `pnpm check`, `pnpm test`, builds, `pnpm check:rust` and serial Rust tests. The Rust job runs on macOS because system capture is macOS-only.
10. Rerun the checks covering the final change before finishing so results match current files.

## Constraints

- Never add filler tests for logic that does not exist; never fix unrelated failures for a local change, only report them.
- Do not re-run checks that already passed unless files changed again or an open risk remains.
- Never present an interface-only or mock run as real capture/inference verification.

## Delivery

Change scope, each command with its result (passed/failed/not run and why), real runtime observations, and remaining risks or unverified platforms.
