---
name: native-audio-pipeline
description: Use when implementing, debugging or reviewing SonaSub system-audio capture, DSP, resampling, audio timestamps, realtime buffering, ASR input, backpressure, capture recovery or subtitle timing.
---

# Native audio pipeline

## Read first

- [architecture](../../../docs/architecture.md), especially Capture And Timeline and Recognition Paths.
- `apps/desktop/src-tauri/src/audio/`, `asr/`, `session/`, `subtitles/` and the tests relevant to the changed path. Read [project-verification](../project-verification/SKILL.md) for native validation.
- Combine with [rust-engineering](../rust-engineering/SKILL.md); use [native-session-lifecycle](../native-session-lifecycle/SKILL.md) when startup, shutdown or worker ownership changes.

## Workflow

1. Trace the changed data from the capture source to DSP, ASR, subtitle patches and consumers. Identify the native callback versus the capture polling thread; do not assume they have identical execution constraints.
2. Keep realtime-sensitive paths bounded and nonblocking. Do not perform inference, disk/network I/O, model initialization, blocking sends or expensive logging there. Avoid unnecessary allocation and copying; measure before adding complexity.
3. Preserve the current format and timeline contracts unless the task explicitly changes them: capture at the device's actual CPAL format, statefully convert to 16 kHz mono, carry native input frame indices into the session timeline, and carry corresponding 16 kHz frames into ASR. Account for resampler state, callback-size changes and the flush tail.
   On macOS, an output device that also exposes input channels must go through the private process tap in `audio/macos_tap.rs`; keep the tap and aggregate alive until stream teardown, and keep the private aggregate out of device enumeration.
4. Trace each bounded queue's producer, consumer, capacity, full/disconnected behavior and shutdown. Preserve explicit drop accounting, notices and gap handling; do not silently bridge missing audio or replace a bounded queue with an unbounded one. Never block capture while waiting for a slow worker.
5. On a dropped chunk, discontinuity, recovery or device change, verify the relevant resampler/history reset, ASR discontinuity and subtitle timeline. Surface permission, device, stall and fatal errors through the existing session mechanisms; do not substitute microphone input.
6. Preserve actual recognition semantics: streaming Zipformer can emit partials, while VAD + SenseVoice is segmented; windowed diarization is not overlapping-speech separation. Keep subtitle IDs and source revisions stable across independent source, speaker and translation patches.
7. Add focused tests for affected timing, format, gaps and overload behavior when feasible. For latency or throughput claims, use repeatable measurements and record workload and platform rather than asserting improvement without evidence.

## Verification and delivery

Use [project-verification](../project-verification/SKILL.md) to select Rust checks, model tests and real `.app` system-output playback. Note model/WAV and audio-permission prerequisites; ignored native tests are not run by ordinary `cargo test`. Report measured behavior and any unverified real device or platform. macOS loopback requires macOS 14.6+; Windows WASAPI compile checks do not count as real capture verification.
