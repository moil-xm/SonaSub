# Validation

This document records the checks that were actually executed for the native
subtitle pipeline review, the result of each check and the gaps that remain.
The commands are listed with their real exit status. A check that the
environment could not complete is named as such; it is never replaced by a
simulated result.

## Environment

| Item      | Value                                                      |
| --------- | ---------------------------------------------------------- |
| Platform  | macOS 27.2, arm64                                          |
| Hardware  | Apple M2 Max                                               |
| Rust      | 1.97.0, project `rust-version = 1.95`                      |
| Node.js   | 24.22.1                                                    |
| pnpm      | 12.15.0 (`packageManager` in `package.json`)               |
| App build | `pnpm --filter @sonasub/desktop tauri:build --bundles app` |
| Models    | `~/Library/Application Support/dev.sonasub.desktop/models` |
| Audio     | 16 kHz mono test clips in `/tmp/sonasub-research/audio`    |

The official `sherpa-onnx` crate downloads and links its version-matched
static runtime during compilation; no separate native preparation step exists.

## Automated Checks

| Command                                                                                     | Result                                        |
| ------------------------------------------------------------------------------------------- | --------------------------------------------- |
| `pnpm fmt`                                                                                  | pass — 175 files formatted                    |
| `pnpm check` (Vite+ format, lint, type check)                                               | pass — 175 formatted, 138 lint/type clean     |
| `pnpm typecheck`                                                                            | pass — 138 files, no type errors              |
| `pnpm test`                                                                                 | pass — 9 files, 40 tests                      |
| `pnpm build` (desktop + extension)                                                          | pass — existing >500 kB chunk warning remains |
| `pnpm check:rust` (`cargo fmt --check`, `clippy -D warnings`)                               | pass                                          |
| `cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml -- --test-threads=1` | pass — 113 passed, 14 ignored                 |
| `node --check scripts/prepare-models.mjs`                                                   | pass                                          |
| `pnpm install --frozen-lockfile`                                                            | pass                                          |
| `git diff --check`                                                                          | pass                                          |

The 14 ignored Rust tests include the real-model tests and the real-device
CPAL capture probe. They are **not** part of the automatic suite and are
reported separately below.

## OpenAI-Compatible Translation Change

The optional remote translation engine was checked with the following commands
from the repository root. `CARGO_NET_OFFLINE=true` was used after the new
crates were fetched because the registry download stalled in this environment.

| Command                                                                                               | Result                                                |
| ----------------------------------------------------------------------------------------------------- | ----------------------------------------------------- |
| `pnpm check`                                                                                          | pass — 173 formatted, 136 lint/type clean             |
| `pnpm test`                                                                                           | pass — 7 files, 38 tests                              |
| `pnpm build`                                                                                          | pass — existing >500 kB desktop chunk warning remains |
| `CARGO_NET_OFFLINE=true pnpm check:rust`                                                              | pass — `cargo fmt --check` and `clippy -D warnings`   |
| `cargo test --locked --offline --manifest-path apps/desktop/src-tauri/Cargo.toml -- --test-threads=1` | pass — 109 passed, 14 ignored                         |
| `git diff --check`                                                                                    | pass                                                  |
| `CARGO_NET_OFFLINE=true pnpm --filter @sonasub/desktop tauri:build --no-sign`                         | pass — unsigned release `.app`, 47.40 MiB             |

The Rust suite covers old config/record compatibility, default and remote
configuration persistence, empty API keys, secret-redacted `Debug`, 30
translation targets, Bergamot's two targets, `zh`/`yue` handling, engine-aware
routing, same-language skipping, patch engine/model retention, and loopback
HTTP stub cases for custom paths, request shape, Bearer present/absent, text
extraction, empty output, HTTP errors, truncation, safety stops and timeout.
The frontend suite covers engine-aware readiness and Bergamot target limits.

The release bundle contains only `Info.plist`, the icon and the executable.
It contains no Node.js runtime or model weights, and `otool -L` lists only
system frameworks and libraries. A binary string scan found configuration
field names but no concrete API key or credential value.

Real Bergamot model translation was previously validated with the pinned
weights, but the ignored real-model suite was not rerun after this change.
Real OpenAI-compatible translation is **unverified**: no user-provided remote
endpoint and API key were available. The loopback stub verifies the wire
format and failure handling, not a production provider.

## Real-Model Tests

Run explicitly with the locally prepared models:

```bash
SONASUB_MODELS_DIR=/tmp/sonasub-all-models-qwen pnpm prepare:models

cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml \
  --test native_pipeline -- --ignored --test-threads=1
```

`pnpm prepare:models` completed with `All pinned models are ready in
/tmp/sonasub-all-models-qwen`. It verified every pinned archive/runtime
checksum without leaving weights in the repository.

Result: 13 passed in 275.46 s. The 30-minute case was run separately with
`SONASUB_QWEN_LONG_TEST=1` and passed in 1802.84 s; the ignored suite invocation
above reports the opt-in message and returns from that test immediately unless
the variable is set.

The full run uses a temporary model root that links the existing Zipformer,
SenseVoice, Bergamot and speaker assets to the two Qwen3 model directories, so
one run verifies all pinned files and engines. The Qwen multilingual tests run
in `auto` mode because the pinned sherpa-onnx Qwen stream exposes no language
override. The 1.7B decoder-scaffold test also asserts that valid `language=en`
metadata is preserved when the runtime emits it.

| Test                                                          | Covers                                                       |
| ------------------------------------------------------------- | ------------------------------------------------------------ |
| `model_files_match_the_recorded_checksums`                    | all pinned model and Qwen3 runtime file integrity            |
| `streaming_asr_transcribes_a_real_clip`                       | streaming Zipformer on a real clip                           |
| `segmented_asr_transcribes_a_real_clip`                       | VAD + SenseVoice on a real clip                              |
| `bergamot_translates_both_configured_directions`              | `en → zh-Hans` and `zh-Hans → en`                            |
| `windowed_diarization_finds_multiple_speakers`                | windowed diarization on a real clip                          |
| `diarization_handles_more_active_speakers_than_clusters`      | the pinned top-k clamp regression case                       |
| `qwen3_06b_transcribes_a_real_clip_in_auto_language_mode`     | Qwen3 0.6B automatic language detection and VAD-final output |
| `qwen3_06b_transcribes_chinese_japanese_and_mixed_clips`      | Qwen3 0.6B Chinese, Japanese and Chinese-English code switch |
| `qwen3_06b_handles_a_long_clip_and_records_runtime_metrics`   | Qwen3 0.6B across multiple VAD segments                      |
| `qwen3_06b_sustains_thirty_minutes_of_realtime_audio`         | paced 30-minute input, queue, latency and RSS run            |
| `qwen3_17b_transcribes_a_real_clip`                           | Qwen3 1.7B and decoder-scaffold cleanup                      |
| `qwen3_17b_transcribes_chinese_japanese_and_mixed_clips`      | Qwen3 1.7B Chinese, Japanese and Chinese-English code switch |
| `qwen3_17b_handles_a_long_clip_without_degenerate_repetition` | 1.7B long output, repeated n-gram and duplicate-final checks |

These tests exercise the real engines with the real weights. They do not
exercise Core Audio capture, the session lifecycle or the frontend.

## Qwen3 ASR Validation

The official 0.6B archive matched its release SHA-256. The archive contained
`conv_frontend.onnx`, `encoder.int8.onnx`, `decoder.int8.onnx` and three
tokenizer files, and every extracted file matched the pinned runtime SHA-256.
The 1.7B ModelScope export was downloaded from revision
`9c182309f7bb075f241424441add9e16c5086dfb`; all six files matched their exact
byte sizes and pinned SHA-256 values.

The created Qwen offline stream reports no `language` option under
sherpa-onnx 2.0.1. The registry therefore marks both Qwen sizes as
auto-language only, backend and record validation reject a persisted forced
language, and the frontend disables the control. Setting the option would not
affect inference.

Real inference in the supported `auto` mode produced these examples:

- Qwen3 0.6B English: `Pencil with black lead writes best.`,
  `The lamp shone with a steady green flame.`,
  `Baths and lodging are free to new men.`, and
  `The glow deepened in the eyes of the sweet girl.`
- Qwen3 0.6B Chinese: `今天天气很好，我们一起去公园散步。`
- Qwen3 0.6B Japanese:
  `今日はとても良い天気です。公園へ散歩に行きましょう。`
- Qwen3 0.6B Chinese-English mixed input:
  `今天的会议在下午三点开始。Please bring the latest project report.`
- Qwen3 1.7B English produced the same four final texts with no native
  partials. One segment carried `language=en` metadata.
- Qwen3 1.7B Chinese, Japanese and Chinese-English mixed input produced the
  same three texts shown above.

The first uncorrected 1.7B run returned
`language English<asr_text>Clothes and lodging are free to new men.` in one
segment. The Qwen adapter now removes the leading Qwen decoder scaffold before
the subtitle event is emitted. The regression test rejects any `<asr_text>`
marker, and the rerun returned
`Clothes and lodging are free to new men.`

### Runtime measurements

The measured long clip was generated locally with macOS `say`, converted to
16 kHz mono PCM WAV, and lasted 35.269 seconds. It crossed three VAD segments.
`processing` includes the shared VAD, decoding, timestamp and final-event path,
so RTF is `processing / audio duration`.

| Model | Initialization | Processing | RTF   | Peak process RSS |
| ----- | -------------- | ---------- | ----- | ---------------- |
| 0.6B  | 1.507 s        | 10.959 s   | 0.311 | 1.55 GB          |
| 1.7B  | 2.319 s        | 25.843 s   | 0.733 | 4.17 GB          |

Peak RSS was measured with `/usr/bin/time -l` on the corresponding real-model
test process. It includes the test harness and model runtime. The 0.6B and
1.7B long clips each produced three finals. The 1.7B outputs contained no
repeated eight-word sequence and no duplicate consecutive final.

These are single-machine observations, not release guarantees. The following
were not measured by the native test harness:

- End-to-end subtitle delivery latency after the ASR final event, including
  speaker, translation and frontend patches.
- Isolated per-segment ASR inference time. The end-to-end push duration is
  recorded, but it also includes VAD and event scheduling.
- A 30-minute sustained run with Qwen3 1.7B.
- A 30-minute Qwen3 system-output session in the packaged macOS app.

### 30-minute real-time queue run

`qwen3_06b_sustains_thirty_minutes_of_realtime_audio` fed a 35.269-second
16 kHz mono clip 51 times through bounded recognition input from a native test
harness. Input arrival was paced to real time, but this was not Core Audio
capture and did not run the React frontend.

| Metric                             | Value                           |
| ---------------------------------- | ------------------------------- |
| Audio                              | 1798.700 s (29 min 58.7 s)      |
| Wall time                          | 1801.311 s                      |
| Final subtitles                    | 128                             |
| Maximum input queue                | 31 chunks                       |
| Dropped audio chunks               | 0                               |
| Recognition-busy time              | 580.277 s                       |
| Recognition RTF                    | 0.323                           |
| Average final-arrival delay        | 6218 ms                         |
| Maximum final-arrival delay        | 8618 ms                         |
| Maximum single push                | 6.373 s                         |
| Maximum finals emitted from a push | 1                               |
| Peak process RSS                   | 2,261,499,904 bytes (2.106 GiB) |

The queue stayed bounded and caught up after inference bursts. The test measured
delay from a final segment's audio end to the consumer receiving its `AsrFinal`.
It does not include speaker analysis, Bergamot, subtitle IPC or React rendering.

## Regression Coverage

The automatic suite covers the scheduling and state logic that does not need
the acoustic models.

| Requested regression                          | Test                                                                                                                                                                                                       |
| --------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Stop with a full queue and a slow worker      | `pipeline::workers::tests::the_stop_signal_reaches_a_worker_behind_a_full_audio_queue`                                                                                                                     |
| Resampler tail delivered on stop              | `pipeline::workers::tests::stop_delivers_the_resampler_tail_before_the_engine_flush`                                                                                                                       |
| Cancel or fail during initialization          | `pipeline::workers::tests::a_failing_initialization_releases_a_worker_that_is_blocked_on_the_event_queue`, `a_disabled_worker_counts_as_finished`, plus the `stage()` failure path in `Runner::initialize` |
| Worker exited before the last ASR event       | `pipeline::workers::tests::the_translation_queue_waits_for_the_last_recognition_event`                                                                                                                     |
| Timeline after an audio gap                   | `asr::segmented::tests::a_gap_resets_the_detector_and_keeps_absolute_timestamps`, `a_gap_flushes_the_speech_that_precedes_it`                                                                              |
| Final text without a partial                  | `asr::streaming::tests::a_final_result_without_a_partial_still_becomes_a_subtitle`, `an_empty_final_result_keeps_the_partial_text`, `an_endpoint_finalizes_the_pending_partial`                            |
| 1000 subtitles with `historyLimit = 400`      | `session-cache.test.ts` → "keeps the subtitle list within the configured history limit"                                                                                                                    |
| Old-session subtitle while starting           | `session-cache.test.ts` → "ignores a subtitle of the previous session while a new one starts"                                                                                                              |
| Old-version patch after a newer subtitle      | `session-cache.test.ts` → "rejects an older text revision after a newer subtitle was applied"                                                                                                              |
| Late old translation and late old snapshot    | `session-cache.test.ts` → "rejects a translation that belongs to an older text revision", "keeps the newer channels when a late snapshot arrives"                                                          |
| Embedding and diarization in different orders | `subtitles::tests::a_diarization_label_replaces_an_earlier_embedding_label`, `a_late_embedding_does_not_erase_a_diarization_label`                                                                         |

## Dependency Policy

CI failed with `ERR_PNPM_MINIMUM_RELEASE_AGE_VIOLATION` for `i18next@27.1.3`
and `react-i18next@18.0.2`. pnpm 12 defaults `minimumReleaseAge` to 1440
minutes; the developer machine overrode it to `0` in a global pnpm config, so
the repository and CI disagreed.

`pnpm-workspace.yaml` now declares `minimumReleaseAge: 0` for every
environment, and both CI jobs pin `pnpm/action-setup` to `12.15.0`, the same
version as `packageManager`. The lockfile stays frozen:

```bash
pnpm install --frozen-lockfile
```

This was verified by temporarily setting the global pnpm configuration to
`minimumReleaseAge: 1440`: the effective value stayed `0` and the frozen
install completed. The global configuration was restored afterwards.

## Packaging

`pnpm --filter @sonasub/desktop tauri build --debug --bundles app --no-sign`
produced
`apps/desktop/src-tauri/target/debug/bundle/macos/SonaSub.app` (87.42 MiB).

- The bundle contains only the executable, the icon and `Info.plist`.
- It contains no Node.js runtime, no engine or protocol artifact and no
  bundled model weights.
- `otool -L` lists only system frameworks and system libraries; the pinned
  sherpa-onnx runtime is linked statically.

After both Qwen model directories were installed under the real application
model root, the debug app reported **9 installed bundles out of 9**. Both
Qwen3 cards exposed their source revision, segmented mode, 30-language list,
download size, local usage, download action, and file/folder import actions.
Session settings listed both Qwen3 models and displayed the Silero VAD
final-only explanation.

The final source tree was rebuilt as the same 87.42 MiB debug bundle after the
automated checks. The Computer Use tool became unavailable during application
update, so the final rebuild was not re-driven through the UI or used for a
Qwen system-output session. The 9/9 observation belongs to the immediately
preceding debug build using the same installed model root and model UI.

## Historical Native System-Audio Verification (pre-CPAL)

Before the CPAL migration, the packaged debug app was driven through the then
current macOS system-output backend. The audio source was the system output
device: a 16 kHz mono test clip was played through `afplay` and captured by the
app.

macOS 14.6 or later requires the `kTCCServiceSystemAudioRecording` privacy
grant before a process tap can be created. Rebuilding the bundle changes the
ad-hoc code signature, so the grant has to be given again after every
rebuild. The grant was given for the build under test.

### Segmented mode, English to Simplified Chinese

SenseVoice multilingual (segmented) with VAD, windowed diarization and the
Bergamot `en → zh-Hans` model.

- A pass over the English clips produced 24 to 28 subtitles.
- The absolute timestamps ran from `00:01.4` to `02:45.4` and followed the
  audio, not the arrival time of the recognition result.
- Speaker labels were assigned (`说话人 1/2/3/4/6/7/8`), and the uncertain
  (`不确定`) and overlap (`重叠`) markers were rendered.
- Every finalized subtitle carried a Simplified Chinese translation.

### Streaming mode

Zipformer bilingual zh-en (streaming).

- The `语音语言` control was disabled and explained that the model detects the
  language per utterance, so a forced source language has no effect.
- Live subtitles with timestamps, speaker labels and translations were
  produced.
- The English output of this model is less accurate than the SenseVoice
  output: it is upper-cased and drops or garbles words. This is a property of
  the pinned Zipformer checkpoint, not of the session lifecycle.

### Reverse translation, Chinese to English

The same Chinese clip with the translation target set to English and the
`zh-Hans → en` Bergamot model.

- 7 subtitles were produced, each with a Chinese source text, an English
  translation and a speaker label.
- Absolute timestamps ran from `00:00.1` to `00:39.8`.

### Stop, restart and shutdown

- Stop returned the session to `空闲` in 1.5 s after the reverse translation
  test and in 3.46 s after the 32-minute run. The last subtitle was flushed
  _with_ its translation, which proves that the translation queue is only
  closed after the last recognition event was consumed.
- Restart cleared the caption list and reset the timeline.
- A second session started with an empty subtitle list, so the history of the
  previous session did not leak into the new one.
- Before the fix described in [Shutdown Deadlock](#shutdown-deadlock), the
  same stop never returned.

### Diagnostics

The diagnostics page reports the adapter, the runtime and the model family
separately: `bergamot` / `fxtranslate` / `bergamot-firefox`. The route table
shows `auto` and `text` as supported and localizes the streaming rejection as
"翻译运行时对每个提交的原文返回完整译文，不提供增量输出。". The speech
recognition table reports `zh, en, ja, ko, yue` for the segmented model and
explains the automatic-only source language of the streaming model.

### Long run

Two consecutive sessions of more than 30 minutes each were recorded with
SenseVoice (segmented), windowed diarization and Bergamot `en → zh-Hans`.
The audio source was the three English clips played in a loop through the
system output device.

First session, 32 min 31 s:

| Metric                      | Value                        |
| --------------------------- | ---------------------------- |
| Finalized subtitles         | 310                          |
| Average recognition latency | 823 ms                       |
| Average translation latency | 188 ms                       |
| Dropped audio frames        | 0                            |
| Resident memory             | ~1.7 GB peak, ~1.15 GB after |
| Stop duration               | 3.46 s                       |

Second session, 31 min 45 s, sampled every 10 s (186 samples):

| Metric                | Value                                              |
| --------------------- | -------------------------------------------------- |
| Finalized subtitles   | 304 at the 31:39 mark                              |
| CPU                   | average 42.9 %, median 9.4 %, p95 127 %, max 258 % |
| Resident memory       | median 1665 MB, max 1668.6 MB                      |
| Resident memory trend | first quarter 1660 MB, last quarter 1668 MB        |
| Idle baseline         | 868 MB before the session started                  |

This session was ended with a restart rather than a stop, so it measures the
steady state of a running session. The stop path was measured in the first
session and after the reverse translation test.

The resident memory is flat during the session: the last quarter of the run
differs from the first quarter by 8 MB over 23 minutes, and the samples stay
inside a 168 MB band for the whole run. The high value is the memory-mapped
model weights plus the inference scratch space, not a growing cache. The
first session showed the same pattern and dropped from ~1.73 GB to ~1.15 GB
after the run, which is macOS reclaiming the mapped pages.

The CPU profile is a baseline of ~8 % with periodic inference bursts of
100 % to 260 % while a segment is recognized and translated. The peak sample
is the model load at the start of the session.

No audio frame was dropped and the session produced no runtime notice
("本次会话没有采集、识别或翻译提示"), so the capture never reported a gap
or an overrun.

### App exit

The app was quit with `Cmd+Q` while a session was running with every model
loaded. The process exited within 4 s, no `sonasub-*` worker survived the
process, and no orphan capture or playback process was left behind. A
relaunch reported the idle state with an empty subtitle list.

## Shutdown Deadlock

The review found a shutdown deadlock that the previous code introduced.

**Trigger.** `ShutdownState::may_close_translation` took a boolean named
`events_pending` and closed the translation queue when it was `false`. The
call site passed `self.workers.events.is_empty()`, so the meaning was
inverted: the queue was closed while events were still pending and never
closed once they were drained. The translation worker then blocked forever
in `TranslationQueue::pop`, never sent `Done(Translation)`, and
`SessionManager::stop_locked` blocked forever in `handle.join()`. The user
interface stayed at "停止中".

**Evidence.** A thread dump of the stuck process showed
`sonasub-translation` blocked in `TranslationQueue::pop` →
`Condvar::wait`, `sonasub-session` parked in `crossbeam_channel::select!`,
the Tokio worker inside `SessionManager::stop_locked` → `handle.join()`, a
CPU usage of 0.2 %, and no remaining `sonasub-asr`, `sonasub-speaker` or
`sonasub-diarization` threads.

**Fix.** `may_close_translation` now receives the event receiver and computes
`asr_done && !translation_closed && events.is_empty()` itself, so the
emptiness check cannot be inverted at the call site. The regression tests
`the_translation_queue_waits_for_the_last_recognition_event` and
`the_translation_queue_is_closed_only_once` fail when the condition is
inverted again.

## Unverified Platforms

- Windows: CPAL/WASAPI system loopback and microphone capture are implemented
  and the Rust build is compiled in CI. Real WASAPI capture has not been
  exercised on Windows hardware.
- Linux: audio capture is not implemented and the capability is unavailable.
- macOS x64: the CPAL loopback path is implemented but was not exercised on an
  Intel Mac. Not tested.
- macOS 14.6 to 26.x: the CPAL capture path was verified on macOS 27.2 only.
  Not tested.

## Deferred Work

### Historical sherpa-onnx Rust API migration

The native speech modules use the official `sherpa-onnx` crate pinned to
v2.0.1. The following table is the original post-migration record and predates
the Qwen3 additions. The current counts are in the sections above.

| Command                                                                                                                                  | Result                                       |
| ---------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------- |
| `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --check`                                                                    | pass                                         |
| `cargo clippy --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets -- -D warnings`                                   | pass                                         |
| `cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml -- --test-threads=1`                                              | pass — 37 passed, 6 ignored                  |
| `cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --test native_pipeline -- --ignored --test-threads=1 --nocapture` | pass — 6 passed in 43.75 s                   |
| `pnpm check`                                                                                                                             | pass — 137 files formatted, 100 files linted |
| `pnpm test`                                                                                                                              | pass — 24 tests                              |
| `pnpm build`                                                                                                                             | pass                                         |
| `pnpm --filter @sonasub/desktop tauri build --debug --bundles app --no-sign`                                                             | pass — `SonaSub.app`, 73.23 MiB              |
| `otool -L` on the packaged executable                                                                                                    | only system frameworks and libraries         |
| `git diff --check`                                                                                                                       | pass                                         |

The ignored real-model tests covered the recorded checksums, streaming
Zipformer, VAD + SenseVoice, Bergamot in both directions, windowed Pyannote
diarization and the top-k regression with one requested cluster. No mock
recognition result was used.

The packaged `.app` was not launched with a live macOS system-output capture,
and the 30-minute performance run was not repeated after the migration. Those
two checks remain unverified for this change.

- The historical results above were recorded against the earlier native
  pipeline. The migration to the official sherpa-onnx v2.0.1 Rust API is a
  separate change; the subsection above records its automated and real-model
  verification.

## CPAL Capture Probe (2026-11-20)

The migration was exercised on macOS 27.2 arm64 with CPAL 0.18.2 and real
Core Audio devices. The ignored integration test
`tests/audio_capture.rs::captures_system_output_and_microphone_simultaneously`
opened both sources at the same time, played a 440 Hz WAV through the current
output, and stopped both streams before asserting frame counts and events.

| Source        | CPAL device                         | Callback format        | Captured frames | Observation                       |
| ------------- | ----------------------------------- | ---------------------- | --------------- | --------------------------------- |
| System output | `coreaudio:BuiltInSpeakerDevice`    | 44,100 Hz / 2 ch / f32 | 177,664         | Peak 0.2000 from the played tone  |
| Microphone    | `coreaudio:BuiltInMicrophoneDevice` | 44,100 Hz / 1 ch / f32 | 187,904         | Captured concurrently with output |

Both streams survived activation, delivered audio concurrently and joined
without a fatal or device-lost event. The capture info reported the real
44.1 kHz callback formats rather than a fixed 48 kHz contract. Arbitrary-block
resampling remains covered separately by the deterministic `StreamingResampler`
and `AudioFlow` tests.
`system_profiler SPAudioDataType` reported 44,100 Hz for the built-in output
and microphone both immediately before and after the probe, so this run did
not leave either device at a different system sample rate. A second stop/start
probe also passed with the same formats (180,224 system-output frames and
188,928 microphone frames); the exact frame count varies with callback timing.

The debug bundle was rebuilt after the migration and inspected directly:

| Check                                     | Result                                                                                               |
| ----------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| `Contents/Info.plist` via `PlistBuddy`    | `NSMicrophoneUsageDescription`, `NSAudioCaptureUsageDescription` and `LSMinimumSystemVersion = 14.6` |
| `strings` scan of the packaged executable | no `flexaudio` strings                                                                               |
| `otool -L`                                | no flexaudio or Node.js libraries                                                                    |
| `Contents/Resources`                      | icon only; no model weights                                                                          |

### Two-Minute Real Capture Run

A dedicated probe built against the workspace crate opened the default system
output for 120 seconds while tones played, then reported the native timeline
and process cost:

| Measurement       | Result                                                   |
| ----------------- | -------------------------------------------------------- |
| Device and format | `coreaudio:BuiltInSpeakerDevice`, 44,100 Hz / 2 ch / f32 |
| Wall time         | 120.09 s                                                 |
| Captured audio    | 5,292,032 frames over 10,336 chunks = 120.00 s           |
| Duration drift    | -0.09 s                                                  |
| Dropped frames    | 0                                                        |
| Discontinuities   | 0                                                        |
| Warnings / fatals | 0 / 0                                                    |
| Peak RSS          | 17.9 MB                                                  |

The frame count matches wall time within callback-boundary rounding, so the
captured timeline did not drift at 44.1 kHz and the stream did not force a
different device rate. The built-in output still reported 44,100 Hz in
`system_profiler` after the run.

### Input-Capable 48 kHz Output (2026-11-20)

The same probe was repeated after `小米Type-C耳机` became the default output.
That device exposes two output channels and two input channels at 48,000 Hz,
so it exercises the macOS path that CPAL's own input configuration cannot
capture correctly. SonaSub created a private process tap and aggregate device,
then opened the aggregate through CPAL while the microphone source continued
to use the built-in microphone.

| Source        | CPAL device                                | Callback format        | Captured frames | Observation                      |
| ------------- | ------------------------------------------ | ---------------------- | --------------- | -------------------------------- |
| System output | `coreaudio:AppleUSBAudioEngine:...小米...` | 48,000 Hz / 2 ch / f32 | 194,048         | Peak 0.5150 from the played tone |
| Microphone    | `coreaudio:BuiltInMicrophoneDevice`        | 44,100 Hz / 1 ch / f32 | 176,640         | Captured concurrently            |

Both streams joined without a fatal or device-lost event. The loopback used
the USB headset's output mix rather than its microphone, and the unbuffered
playback remained audible because the tap uses the `Unmuted` mute behavior.
The private aggregate device is filtered out of the user-selectable device
list.

The capture probe verifies capture and shutdown. The installed release GUI was
then exercised end to end with the same USB headset:

| Step                                                          | Observed                                                                                                                          |
| ------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| Open **New record**                                           | `小米Type-C耳机` was auto-selected as the system-output source                                                                    |
| Open **Add device**                                           | `小米Type-C耳机` appeared under **System output** as `可用 系统默认`; its microphone direction also appeared under **Microphone** |
| Start the record                                              | the record changed to `运行中` with `1 台设备运行中`; no format-change or device-open error appeared                              |
| Play 12 seconds of speech through the headset                 | 4 subtitles appeared with English recognition and Simplified Chinese translation                                                  |
| Pause the record                                              | active sources dropped to zero and the device was released                                                                        |
| Open **New record** while the paused record still selected it | the same headset was selected again without an occupancy error                                                                    |

`system_profiler SPAudioDataType` still reported `Current SampleRate: 48000`
for the headset after the GUI run. The private loopback aggregate did not
appear in the user-selectable device list. The test record remains paused as
`小米Type-C耳机`; no production record configuration was changed.

### Record Device Occupancy (2026-11-20)

Occupancy is derived from every record's live capture sources instead of a
separate claim map. A device is reported as in use only while a record holds a
source for it, so a stopped, failed, timed-out or rolled-back source releases
the device together with the state transition that removes the source.

Verified on the installed release `.app`:

| Step                                               | Observed                                                                                                    |
| -------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| Start a record with the `BlackHole 2ch` microphone | `BlackHole 2ch` is disabled and labelled `MacBook Pro扬声器 正在使用`                                       |
| Pause that record                                  | `BlackHole 2ch` becomes `可用` without restarting the app                                                   |
| Open the device list                               | the list refetches on open; the header refresh button re-runs `list_record_devices` and keeps the list open |

The Rust suite adds `a_live_source_owns_its_device`,
`removing_the_active_source_releases_the_device` and
`a_stored_selection_without_a_live_source_is_not_a_claim`.

## Known Limitations

- Native inference runs in the desktop process. A native crash is not isolated
  from the UI and cannot be preemptively cancelled.
- Diarization is bounded-window quasi-real-time analysis, not whole-recording
  diarization and not overlapping-speech separation. Speaker attribution can
  fragment and overlapping speech is not split into separate tracks.
- The translation adapter is complete-text only. `streaming` is rejected
  rather than emulated.
- Qwen3 recognition is VAD-segmented. It produces final captions only and
  never claims native streaming partials.
- Qwen3 1.7B uses the pinned community ModelScope ONNX export. Its model card
  does not promise production readiness, and its measured peak process RSS was
  4.17 GB on the test machine.
- The debug bundle is ad-hoc signed and not notarized.
