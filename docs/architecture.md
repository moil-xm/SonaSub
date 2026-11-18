# Native Architecture

SonaSub is a native macOS desktop application. Tauri and Rust own system-audio
capture, inference workers, subtitle scheduling and process cleanup. React is
a control and presentation layer that reaches the backend only through Tauri
IPC and events. There is no Node.js runtime, local WebSocket service,
authentication handshake or browser-side inference.

## Module Responsibilities

| Path                     | Responsibility                                                                                                                       | Technology                                                       |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------- |
| `apps/desktop`           | React control UI; Tauri commands, events and native desktop shell                                                                    | React + TypeScript, TanStack Router/Query/Form, Zustand, Tauri 2 |
| `apps/desktop/src-tauri` | Audio capture, DSP, sherpa-onnx workers, speaker analysis, Bergamot worker, subtitle scheduling, configuration and session lifecycle | Rust, flexaudio, sherpa-onnx, fxtranslate                        |
| `apps/extension`         | Chrome extension (Manifest V3), popup only; no capture implementation yet                                                            | WXT + React + TypeScript                                         |
| `packages/ui`            | Shared shadcn/ui components, theme variables, styles and hooks                                                                       | Tailwind CSS 4, CSS Variables, Radix primitives                  |
| `packages/i18n`          | Shared `en-US` / `zh-CN` resources, i18next setup and typed selectors                                                                | i18next + react-i18next                                          |

The desktop's Rust modules are deliberately separated:

- `audio/`: macOS system-output capture and 48 kHz stereo to 16 kHz mono DSP.
- `asr/`: a common input/event adapter with streaming Zipformer and segmented
  Silero VAD + SenseVoice implementations.
- `speaker/`: CAM++ embedding registry and windowed Pyannote diarization.
- `translation/`: the `TranslationEngine` boundary, Bergamot adapter and bounded
  translation queue.
- `subtitles/`: stable segment IDs, revisions and independent text, speaker and
  translation patches.
- `session/`: serialized lifecycle, event emission, metrics and cleanup.
  `session/workers.rs` owns the recognition, speaker, diarization and
  translation worker threads, their bounded queues and the shutdown
  bookkeeping; `session/mod.rs` owns the session thread, the subtitle
  scheduling and the published state.

## Native Runtime

The desktop app uses the official `sherpa-onnx` Rust API pinned to v2.0.1. The
crate downloads a version-matched native archive and statically links it into
the desktop binary; no source patch, custom CMake build or legacy runtime path
override is involved. Inference runs inside the desktop process.

The diarization top-k regression test in
`apps/desktop/src-tauri/tests/native_pipeline.rs` still exercises the case
where active speakers exceed the requested cluster count.

- `config.rs` / `models.rs`: validated persisted configuration, the pinned model
  registry and capability resolution.

## Dependency Direction

```mermaid
graph TD
  desktop[apps/desktop React] --> ui[packages/ui]
  desktop --> i18n[packages/i18n]
  extension[apps/extension] --> ui
  extension --> i18n
  desktop -. Tauri IPC + events .-> rust[src-tauri Rust]
  rust --> audio[Core Audio + DSP]
  rust --> asr[sherpa-onnx workers]
  rust --> speaker[speaker + diarization workers]
  rust --> translation[Bergamot worker]
  rust --> models[local model directory]
```

- Apps never import each other's internal source; cross-app reuse goes through
  `packages/*`.
- Packages never depend on an app, Tauri, Chrome or Node-only APIs.
- React never loads a native model or receives a continuous audio stream.
- Native dependencies belong to `apps/desktop/src-tauri`; browser behavior
  belongs to the extension.

## Capture And Timeline

macOS output is captured through a Core Audio process tap
(`flexaudio`/`flexaudio-os-macos`) at 48 kHz stereo in 20 ms chunks. The capture
thread only polls native chunks, copies them into a bounded channel and records
capture warnings. It performs no inference, disk access or network request.

The session thread performs channel downmix and a windowed-sinc FIR decimation
to 16 kHz mono. Every chunk carries an absolute 48 kHz frame index. The ASR
input also carries that frame index, so a completed inference is assigned the
audio's original time, not the time at which the worker finished.

- A gap, dropped chunk, device recovery or discontinuity resets the resampler
  and audio history, emits an ASR discontinuity, and starts the next subtitle
  run after the real gap. Missing audio is never presented as continuous.
- The segmented path recreates the Silero VAD after a gap. Clearing the
  detector only drops the finished segments while its internal sample counter
  keeps running, which would shift every following timestamp; a real reset
  restarts that counter and the segment epoch, so the absolute time of a
  recovered run stays correct.
- A bounded ASR queue drops whole intervals when inference cannot keep up and
  records the dropped frames; the excluded interval is not bridged.
- Device loss, permission denial, terminal errors and stalled capture are
  surfaced as warnings or session failures.
- Subtitle history and the diarization audio ring are bounded.

System capture is implemented and verified on macOS 14.4 or later on arm64.
Windows, Linux and macOS x64 report the capability as unavailable and refuse to
start; they do not silently switch to microphone input.

## Recognition Paths

Both paths feed the same subtitle scheduler and use 16 kHz mono audio.

**Streaming:** the Zipformer bilingual zh-en int8 model runs continuous online
recognition with endpointing. Partial text is emitted as it changes; an endpoint
produces final text. It does not wait for a VAD utterance boundary. Final
segments keep their audio for speaker embedding.

**Segmented:** Silero VAD closes bounded utterances or cuts at the configured
maximum length, then SenseVoice int8 transcribes each segment. This is real
segmented recognition, not native streaming. It still produces results during
the session; it does not wait for the recording to end.

Model format and decoding differences stay inside `asr/`. `auto` resolves to the
selected model's actual capability. An explicit incompatible mode is rejected
with an error and is disabled in the UI; `auto` is never a fallback after an
inference failure.

A final result never depends on an earlier partial. If the recognizer only
produces text while flushing, that text still receives its own subtitle
identity, and an empty final result falls back to the pending partial instead
of dropping the segment.

## Speaker Attribution

Every finalized segment can be queued for CAM++ speaker embedding. The registry
maintains session-wide profiles with cosine matching, bounded capacity and
confidence/uncertainty fields. A label may appear after the source text.

Diarization runs over bounded rolling windows (20 seconds by default, configured
from 5 to 60 seconds), scheduled at roughly half-window intervals. Local
clusters are converted to embeddings and matched to the same session-wide
registry, so a local cluster number is not exposed as a global speaker ID.
Overlap regions mark attribution as uncertain. This is windowed quasi-real-time
analysis, not whole-recording diarization and not overlapping-speech
separation. No separate speaker tracks are produced.

The speaker queue and diarization queue are bounded; dropped work produces
notices or unknown/uncertain labels rather than blocking recognition.

The two attribution sources are merged by authority, not by arrival order. A
windowed diarization result knows which voices overlapped and covers the whole
window, so it replaces a preliminary embedding label including its overlap
marker. A late embedding result can therefore never erase a diarization label.
Each assignment increments the segment's speaker revision, and the frontend
rejects a speaker patch that is older than the revision it already applied.

## Translation

`TranslationEngine` is a small blocking interface. The only compiled adapter is
Bergamot through `fxtranslate`, using Firefox Translations-format models.
Complete source text is submitted on a dedicated worker; the result returns as
an asynchronous patch for the same subtitle ID and text revision.

- The adapter reports the native runtime (`fxtranslate`) and the model family
  (`bergamot-firefox`) separately. The runtime executes the model; the model
  family describes the pinned weight format. They are not interchangeable.
- The translation worker receives its engine through the `TranslationEngine`
  interface and is created by one factory entry point. Only the existing
  adapter is implemented; there is no plugin registry or dynamic loading.
- Bergamot declares complete-text (`text`) only. `streaming` is rejected; an
  asynchronous callback is not described as incremental translation.
- `auto` resolves to `text`.
- Pending requests for an older revision of the same subtitle are replaced.
  The queue is bounded and evicts the oldest request with an explicit error
  rather than leaving it pending.
- Results carry session, subtitle, source revision and target language.
  Cancelled, stale or config-invalidated results cannot overwrite newer text.
- Translation failure never blocks or removes the source caption. A segment
  whose detected source already matches the target is marked not-needed.
- Language directions come from the pinned local model registry
  (`en → zh-Hans`, `zh-Hans → en`); no arbitrary language pair is inferred from
  Firefox's supported-language list.
- The translation queue closes only after the recognition worker stopped and
  every event it produced was consumed. Closing it earlier would drop the
  translation of the last subtitle; never closing it keeps the translation
  worker waiting forever. The decision lives in
  `ShutdownState::may_close_translation`, which reads the event queue itself
  instead of receiving a boolean, so the emptiness check cannot be inverted
  at the call site.

## Subtitle Scheduling

The scheduler keeps one store per session. A stable subtitle ID is derived from
the session ID and the ASR local segment ID. The store tracks:

- audio start/end in session seconds;
- source text and a monotonically increasing text revision;
- final/partial state;
- optional speaker attribution and uncertainty;
- translation status, target, engine, model and source revision.

Patches are typed `upsert`, `finalize`, `speaker` or `translation`. Source,
speaker and translation patches update the same segment independently. React
applies patches by ID instead of appending every inference result as a new
caption.

Status, subtitles and statistics are independent channels, and each channel
carries a monotonic revision. The snapshot reports the revision of the status,
the subtitle list and the statistics it holds, and every event repeats the
revision it produced. A channel therefore never rolls back: an event that
arrives after a newer one is dropped, and a snapshot that was taken before a
newer event keeps the newer value for that channel. Within the subtitle list
the text, the speaker assignment and the translation each carry their own
revision, so a late patch cannot overwrite a newer value.

Patches are batched at 80 ms; statistics are emitted at 750 ms. History is
bounded by configuration (400 segments by default, 50–2000 allowed). The
backend store and the frontend cache apply the same limit, so neither the
subtitle history nor the rendered list grows without bound. Notices are
capped at 100 and duplicate adjacent notices within two seconds are
suppressed.

## Session Lifecycle

```text
idle → starting → running → stopping → idle
          ↘ failed (validation, model load, permission, capture or worker error)
running → stopping/failed (stop, restart, app exit or pipeline failure)
```

- Start, stop, restart and shutdown are serialized behind one operation lock.
  Repeated start reuses an active session; a session that is still stopping
  rejects a new start.
- Start loads only the models required by the selected ASR mode and enabled
  speaker features, waits up to 180 seconds for initialization, and reports
  the real failure instead of showing a ready state.
- Every failure after the workers exist releases them again, including a
  failure of a later initialization stage and a cancelled start.
- The stop signal for the recognition worker travels on its own control
  channel, which is never blocked by a full audio backlog. A worker that is
  busy decoding still reaches its flush and reports the last subtitle.
- Stop stops capture, flushes ASR, lets native inference reach a safe boundary,
  closes the translation queue and joins workers. Native inference cannot be
  preemptively cancelled.
- The shutdown waits for the worker output to be consumed before it closes the
  translation queue and before it confirms the session as finished. A worker
  that blocks on a full output channel is drained on a separate thread while
  the join runs, so the cleanup cannot deadlock.
- A startup timeout keeps the worker registered and reports the pending
  cancellation instead of claiming that the cleanup already finished. While a
  native call cannot be interrupted, the session reports the waiting state
  every few seconds instead of dropping the thread handle and treating the
  cleanup as done.
- App exit runs the same cleanup path. Models are reused for the session, not
  reloaded per segment.
- Workers are threads inside the desktop process. A native crash is not
  isolated from the UI; this boundary is shown in the session information.

## Frontend Contract

Tauri commands:

- `get_capabilities`
- `get_config`
- `save_config`
- `session_snapshot`
- `start_session`
- `stop_session`
- `restart_session`

Tauri events:

- `session://status`
- `session://subtitles`
- `session://stats`
- `session://notice`

React queries and mutations use `networkMode: 'always'` because this is local
IPC, not network I/O. Event listeners are removed on unmount. Zustand remains
limited to local interaction state such as the theme; session data is a
TanStack Query cache updated by Rust events. The snapshot query merges a
freshly read snapshot into the cache per channel, and the cache re-reads the
snapshot once the event listeners are attached, so events published between
the first read and the subscription are not lost.

`starting`, `running` and `stopping` all carry the session they belong to. The
cache tracks the active session ID and drops a subtitle batch, a status event
or a snapshot that belongs to an older session or that carries a revision
older than the one already applied. The text revision, the revision of the
translation that belongs to it and the speaker revision are validated
separately, so a stale translation or a stale speaker label cannot replace a
newer value.

## Configuration And Capabilities

Configuration is persisted as JSON with atomic replace; invalid values are
rejected before saving and corrupt files are reported instead of silently
resetting. A running session locks configuration changes. Restart applies the
new saved configuration.

Important fields:

- ASR mode (`auto` / `streaming` / `segmented`), model and source language;
- maximum segment length, VAD threshold, minimum speech/silence and stable wait;
- translation mode (`auto` / `streaming` / `text`), trigger
  (`finalOnly` / `stableSegment`) and target language;
- speaker embedding/diarization switches, window, maximum speakers and
  similarity threshold;
- audio output device and subtitle history limit.

Capabilities are computed from the model registry, the compiled adapters and
the current platform. A user preference cannot turn an unsupported mode into a
supported one. UI language, speech source language and translation target
language are separate settings.

The source language is only a real setting for a model that can honour it. The
streaming Zipformer detects the language per utterance and ignores a forced
source language, so the control is locked on automatic and explains why; the
session reports the language that is actually in effect. The segmented
SenseVoice accepts a forced language. A configuration that forces a language a
model does not support is rejected before the session starts.

## Imports And Packaging

- Shared UI: `@sonasub/ui/components/*`, `@sonasub/ui/hooks/*`,
  `@sonasub/ui/lib/*`, `@sonasub/ui/theme`, `@sonasub/ui/globals.css`.
- Interface language: `@sonasub/i18n` for locale resources, i18next setup and
  typed selectors.
- `packages/ui` internal imports use `#lib/*` and `#hooks/*`; desktop internal
  imports use `@/*`.
- Desktop components follow the Ant Design-style `components/<name>/` layout:
  every component owns a kebab-case directory that holds its PascalCase
  implementation file, its named sub-components and an `index.ts` public
  entry. Imports point at the directory (`@/components/live-subtitles`), never
  at a file inside it.
- `.ts` modules use the same layout: `apps/desktop/src/lib/session/` holds
  `session.ts`, its `session.type.ts` and an `index.ts` public entry, and
  imports point at the directory (`@/lib/session`). A directory that groups
  sibling files of one kind, such as `packages/i18n/src/resources/`, is a
  collection that keeps its own `index.ts`.
- New exports must be listed in the owning package's `exports`.

The macOS bundle contains only the application executable, icon and plist. It
does not contain a Node.js runtime, engine artifact or WebSocket protocol.
Model weights stay in the application-data directory and are never bundled or
downloaded by the app. See [models.md](models.md).

## Interface Languages And Theme

`packages/i18n` owns both bundled locales. The desktop stores its locale
preference in `localStorage`; system Chinese resolves to `zh-CN`, system
English to `en-US`, and other languages to `en-US`. The active locale is
reflected in `<html lang>`.

`packages/ui/src/styles/globals.css` is the single Tailwind 4 and theme
variable entry. Theme switching uses the Zustand store from `@sonasub/ui/theme`
and `<html class="dark">`.
