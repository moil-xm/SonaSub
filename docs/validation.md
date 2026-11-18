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
| Rust      | stable toolchain, `rust-version = 1.90`                    |
| pnpm      | 12.15.0 (`packageManager` in `package.json`)               |
| App build | `pnpm --filter @sonasub/desktop tauri build --debug`       |
| Models    | `~/Library/Application Support/dev.sonasub.desktop/models` |
| Audio     | 16 kHz mono test clips in `/tmp/sonasub-research/audio`    |

The official `sherpa-onnx` crate downloads and links its version-matched
static runtime during compilation; no separate native preparation step exists.

## Automated Checks

| Command                                                       | Result                                         |
| ------------------------------------------------------------- | ---------------------------------------------- |
| `pnpm check` (Vite+ format, lint, type check)                 | pass — 137 files formatted, no lint/type error |
| `pnpm test`                                                   | pass — 5 files, 24 tests                       |
| `pnpm build` (desktop + extension)                            | pass                                           |
| `pnpm check:rust` (`cargo fmt --check`, `clippy -D warnings`) | pass                                           |
| `cargo test --locked -- --test-threads=1`                     | pass — 37 passed, 6 ignored                    |
| `pnpm install --frozen-lockfile`                              | pass                                           |
| `git diff --check`                                            | pass                                           |

The six ignored Rust tests are the real-model tests. They are **not** part
of the automatic suite and are reported separately below.

## Real-Model Tests

Run explicitly with the locally prepared models:

```bash
cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml \
  --test native_pipeline -- --ignored --test-threads=1
```

Result: 6 passed in 45.17 s.

| Test                                                     | Covers                                 |
| -------------------------------------------------------- | -------------------------------------- |
| `model_files_match_the_recorded_checksums`               | pinned model integrity                 |
| `streaming_asr_transcribes_a_real_clip`                  | streaming Zipformer on a real clip     |
| `segmented_asr_transcribes_a_real_clip`                  | VAD + SenseVoice on a real clip        |
| `bergamot_translates_both_configured_directions`         | `en → zh-Hans` and `zh-Hans → en`      |
| `windowed_diarization_finds_multiple_speakers`           | windowed diarization on a real clip    |
| `diarization_handles_more_active_speakers_than_clusters` | the pinned top-k clamp regression case |

These tests exercise the real engines with the real weights. They do not
exercise Core Audio capture, the session lifecycle or the frontend.

## Regression Coverage

The automatic suite covers the scheduling and state logic that does not need
the acoustic models.

| Requested regression                          | Test                                                                                                                                                                                                      |
| --------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Stop with a full queue and a slow worker      | `session::workers::tests::the_stop_signal_reaches_a_worker_behind_a_full_audio_queue`                                                                                                                     |
| Cancel or fail during initialization          | `session::workers::tests::a_failing_initialization_releases_a_worker_that_is_blocked_on_the_event_queue`, `a_disabled_worker_counts_as_finished`, plus the `stage()` failure path in `Runner::initialize` |
| Worker exited before the last ASR event       | `session::workers::tests::the_translation_queue_waits_for_the_last_recognition_event`                                                                                                                     |
| Timeline after an audio gap                   | `asr::segmented::tests::a_gap_resets_the_detector_and_keeps_absolute_timestamps`, `a_gap_flushes_the_speech_that_precedes_it`                                                                             |
| Final text without a partial                  | `asr::streaming::tests::a_final_result_without_a_partial_still_becomes_a_subtitle`, `an_empty_final_result_keeps_the_partial_text`, `an_endpoint_finalizes_the_pending_partial`                           |
| 1000 subtitles with `historyLimit = 400`      | `session-cache.test.ts` → "keeps the subtitle list within the configured history limit"                                                                                                                   |
| Old-session subtitle while starting           | `session-cache.test.ts` → "ignores a subtitle of the previous session while a new one starts"                                                                                                             |
| Old-version patch after a newer subtitle      | `session-cache.test.ts` → "rejects an older text revision after a newer subtitle was applied"                                                                                                             |
| Late old translation and late old snapshot    | `session-cache.test.ts` → "rejects a translation that belongs to an older text revision", "keeps the newer channels when a late snapshot arrives"                                                         |
| Embedding and diarization in different orders | `subtitles::tests::a_diarization_label_replaces_an_earlier_embedding_label`, `a_late_embedding_does_not_erase_a_diarization_label`                                                                        |

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

`pnpm --filter @sonasub/desktop tauri build --debug` produced
`apps/desktop/src-tauri/target/debug/bundle/macos/SonaSub.app` (69 MB).

- The bundle contains only the executable, the icon and `Info.plist`.
- It contains no Node.js runtime, no engine or protocol artifact and no
  bundled model weights.
- `otool -L` lists only system frameworks and system libraries; the pinned
  sherpa-onnx runtime is linked statically.

## Native System-Audio Verification

The packaged debug app was driven through the real macOS process tap. The
audio source was the system output device: a 16 kHz mono test clip was
played through `afplay` and captured by the app.

macOS 14.4 or later requires the `kTCCServiceSystemAudioRecording` privacy
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

## Game 4 Caption Baseline (2026-11-18)

This run used the records workspace from `d73c236` with the streaming model
installer changes, on an Apple M2 Max running macOS 27.2 arm64. A temporary
release build used the local model weights and a fresh SQLite database per
run. Native workers, subtitle batching, persistence and React rendering ran
through the desktop application's records pipeline.

### Live system output

The player kept its existing output and volume. A 24-second pass through
the default device delivered 1,199 consecutive 20 ms chunks. All had RMS
above 0.001. The tap reported zero dropped chunks, missing frames or
discontinuities. A separate 40.06-second run on that output used segmented
SenseVoice with English input and Bergamot English to Simplified Chinese.
All seven finalized subtitles and their translations appeared in the
WebView and were present in SQLite after stopping. The source reported
1,922,880 frames and zero drops. These measurements cover the device mix.

A separate developer probe checked two actual process taps using the Chrome
audio helper and `afplay`. Over three seconds the 500 Hz and 1,100 Hz sources
delivered 149 and 147 chunks, with wanted-to-other tone ratios of 78.7 and
76.5 dB respectively. Every 20 ms chunk contained 320 mono samples at
16 kHz and consecutive frame indices advanced by 960. Neither tap reported
a gap or a dropped chunk. The desktop build selects output devices for
records.

### Repeatable single- and two-source workload

The input was the first 120 seconds of the supplied BLG–GEN Game 4
highlights. The MP4 is 733.256 s long and has one 44.1 kHz stereo AAC track.
FFmpeg 9.0.2 produced exactly 5,760,000 stereo frames with this command.

```bash
ffmpeg -nostdin -v error -i "$VIDEO" -map 0:a:0 \
  -af 'aresample=48000,atrim=end_sample=5760000' \
  -ac 2 -c:a pcm_s16le -f s16le game4-first120.s16
```

The PCM SHA-256 was
`b48474245383e0bccd11f919a932df91c3e6b8b8de777c36e117a6818307188c`.
One or two timed readers fed the same PCM into the production capture
queues, in 960-frame chunks from a common clock. Each source had its own
Zipformer recognizer and Bergamot English-to-Simplified-Chinese worker.
Translation ran on final text and speaker analysis was disabled. Each trial
started a fresh app process. Trials ran in the order single, dual, dual,
single.

The WebView observer sampled each caption row on the next animation frame
after a React DOM change. First-text latency starts at the segment onset
shown in the row. Final-text and translation latency start at the
recognizer's final segment end. All three use the audio replay clock.
SQLite batch time covers all subtitle upserts and the source elapsed-time
update. The second pair of trials also timed each subtitle upsert inside
`Connection::execute`, including its implicit SQLite commit. Peak RSS comes
from `/usr/bin/time -l` for the native desktop process and excludes the
separate WebKit processes.

The first pair had complete WebView observations. The table shows the
nearest-rank p95 across segments or batches in each trial. The SQL upsert
column comes from the reverse-order pair.

| Sources | Finalized and translated | Dropped frames per source | First text p95 | Final text p95 | Translation p95 | SQLite batch p95 | Subtitle upsert p95 | Peak native RSS |
| ------- | ------------------------ | ------------------------- | -------------- | -------------- | --------------- | ---------------- | ------------------- | --------------- |
| One     | 26                       | 0                         | 399 ms         | 615 ms         | 692 ms          | 0.624 ms         | 0.544 ms            | 608.3 MiB       |
| Two     | 26 per source            | 0 / 0                     | 900 ms         | 1,015 ms       | 1,093 ms        | 3.541 ms         | 0.533 ms            | 1,028.2 MiB     |

The dual-source first-text p95 was 920 ms on the first source and 887 ms
on the second. Translation p95 was 1,093 and 1,065 ms respectively. The
first pair wrote 325 and 662 subtitle batches. The reverse pair timed 372
single-source and 744 dual-source subtitle upserts. Every source in all
four trials delivered 6,000 chunks, and the persisted final texts and
translations matched across sources and runs.

The reverse pair had incomplete UI coverage. In the single-source run the
WebView was hidden at the final check and the observer saw 20 of 26
translated rows. In the dual-source run the caption list was absent at the
final check and the observer saw 36 of 52 translated rows. All of those
subtitles and translations were present in SQLite.

An additional dual-source trial recorded the page URL and visibility. The
record stayed open and the WebView became hidden 84.4 seconds into the run.
At stop, the observer had seen 34 of the 52 translated rows saved in SQLite.
Reopening the records in visible windows displayed all 26 translated rows
from the single-source run and all 52 from each dual-source run. The
on-screen percentiles use the first pair, which had complete frame observations.

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

- Windows and Linux: system capture is not implemented there and the app
  reports the capability as unavailable. Not tested.
- macOS x64: the process tap path is implemented but was not exercised on an
  Intel Mac. Not tested.
- macOS 14.4 to 26.x: the capture path was verified on macOS 27.2 only. Not
  tested.

## Deferred Work

### Official sherpa-onnx Rust API Migration

The native speech modules now use the official `sherpa-onnx` crate pinned to
v2.0.1. The following checks were run after the migration:

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
- The in-app model import, validation and preparation interface is planned
  in [models.md](models.md#planned-in-app-model-preparation) and was not
  implemented in this round.

## Known Limitations

- Native inference runs in the desktop process. A native crash is not isolated
  from the UI and cannot be preemptively cancelled.
- Diarization is bounded-window quasi-real-time analysis, not whole-recording
  diarization and not overlapping-speech separation. Speaker attribution can
  fragment and overlapping speech is not split into separate tracks.
- The translation adapter is complete-text only. `streaming` is rejected
  rather than emulated.
- The debug bundle is ad-hoc signed and not notarized.
