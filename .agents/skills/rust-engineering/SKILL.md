---
name: rust-engineering
description: Use when implementing, refactoring, debugging or reviewing Rust in SonaSub, including ownership, API design, Result errors, worker synchronization, unsafe or FFI, Cargo configuration and Rust validation.
---

# Rust engineering

## Read first

- [AGENTS.md](../../../AGENTS.md) for project constraints and current scope.
- [Cargo.toml](../../../apps/desktop/src-tauri/Cargo.toml), the touched modules and their tests. Respect the configured edition and `rust-version`; do not assume a newer API is available.
- [project-boundaries](../project-boundaries/SKILL.md) for ownership and IPC; [project-verification](../project-verification/SKILL.md) for checks.
- For capture and timing also use [native-audio-pipeline](../native-audio-pipeline/SKILL.md); for session workers and shutdown use [native-session-lifecycle](../native-session-lifecycle/SKILL.md).

## Workflow

1. Identify the owning module, existing call sites, data ownership and error boundary before changing the API. Reuse the project's existing types and error conventions; introduce a typed error when callers need to distinguish cases, not simply to wrap every `String`.
2. Implement the smallest change that expresses real states and invariants. Prefer enums for mutually exclusive states and the narrowest useful visibility. Add traits only for an actual shared behavior or replacement boundary; avoid speculative managers and generic frameworks.
3. Borrow where natural; transfer ownership when it makes lifetime or thread ownership clearer. Avoid cloning solely to appease the borrow checker, but do not introduce elaborate lifetimes to avoid an inexpensive necessary clone. Do not reach for `Arc<Mutex<_>>` without a real shared-state requirement.
4. Propagate recoverable errors with context and retain the cause where the existing interface permits. Do not hide failures with default values or silently discard important results. Production `unwrap`/`panic` must not handle expected failures; `expect` is appropriate only for a documented invariant. Tests may use either for clarity.
5. Keep locks scoped around state access; do not hold them while waiting on threads, blocking I/O or inference unless the synchronization contract requires it. If an async runtime is introduced, avoid blocking it and holding a lock across `.await` without a justified design.
6. Use safe Rust where practical. Keep `unsafe` and FFI boundaries small; for every unsafe block explain `// SAFETY:` and, for public unsafe functions, document `# Safety`. Check pointer validity, lifetime, ownership, destruction, callbacks, thread guarantees and panic/unwind rules against the native API. Never assert `Send`/`Sync` without establishing the native contract.
7. Add tests for changed behavior and reproducible bugs, focusing on ownership, failure and concurrency where relevant. Do not add tests that merely restate implementation details.

## Project-specific native constraints

- The desktop uses in-process native workers; do not introduce a separate engine process, network service or Node runtime without an explicit architecture change.
- Preserve the pinned sherpa-onnx runtime and ABI/patch relationship described in [architecture](../../../docs/architecture.md) and [models](../../../docs/models.md). Check those sources before changing bindings, native dependencies or build scripts.
- Tauri commands and serialized events are public frontend contracts: update their consumers and tests when changing their shape, and update capabilities when a new permission is needed.

## Verification and delivery

For Rust changes, follow [project-verification](../project-verification/SKILL.md): prepare the pinned native runtime once per checkout, run `pnpm check:rust` and the documented serial Rust tests. Model and real-audio changes need the additional native checks described there. State the commands actually run and any unavailable platform, model, permission or real-device checks; never treat mock results as native verification.
