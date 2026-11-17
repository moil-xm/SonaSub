---
name: native-session-lifecycle
description: Use when changing or reviewing SonaSub native session start, stop, restart, app exit, worker threads, model initialization, cancellation, status events, stale results or resource cleanup.
---

# Native session lifecycle

## Read first

- [architecture](../../../docs/architecture.md), especially Session Lifecycle, Subtitle Scheduling and Frontend Contract.
- `apps/desktop/src-tauri/src/session/`, `src/lib.rs` and the affected worker modules; inspect related frontend session types, event listeners and tests when contracts change.
- Use [rust-engineering](../rust-engineering/SKILL.md) for Rust implementation and [project-verification](../project-verification/SKILL.md) for checks. Use [native-audio-pipeline](../native-audio-pipeline/SKILL.md) when capture, queues or timing are affected.

## Workflow

1. Trace the existing `SessionManager` operation lock and status transitions for start, stop, restart and app exit. Establish which thread owns each handle, channel, cancellation signal and model before changing the lifecycle.
2. Cover both successful initialization and every changed failure path: validation, missing model, permission, capture, worker creation, startup timeout and cancellation. A ready/running status must follow actual readiness; release partially initialized resources.
3. Preserve serialized lifecycle operations and the behavior of repeated starts and starts while stopping. Examine races among startup, stop, worker failure and app exit; do not publish a stale worker's status or events into a newer session.
4. Keep shutdown ordered: stop capture, flush ASR as appropriate, allow native inference to reach a safe boundary, close worker queues and join owned workers. Inspect the actual code when changing this sequence. Native inference is not assumed preemptively cancellable; do not promise immediate shutdown or abandon live workers to simulate it.
5. Preserve session/subtitle identity, text and configuration revisions when processing delayed results. Old or invalidated translation results must not overwrite current source text; translation failure must not remove the source caption. Keep status snapshots, events and frontend cache handling consistent.
6. When adding a worker or changing a queue, define ownership, failure reporting, disconnect, full-queue and cancellation semantics. Do not introduce a process monitor or WebSocket control connection: workers run inside the desktop process and a native crash is not isolated from the UI.
7. Add focused tests for affected transitions, failure cleanup, repeated/concurrent operations and stale results where practical. Do not invent mocks that claim real native inference has been verified.

## Verification and delivery

Use [project-verification](../project-verification/SKILL.md) for `pnpm check:rust`, serial Rust tests and the applicable real `.app` lifecycle checks. Report tests actually run and remaining gaps, particularly missing models, permissions, real system audio or macOS hardware.
