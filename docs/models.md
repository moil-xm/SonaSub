# Local Models

SonaSub keeps model weights outside the application bundle. The desktop app
installs them into the platform application-data directory, and only starts a
network download when the user chooses one from the Models page.

## Default Location

macOS:

```text
~/Library/Application Support/dev.sonasub.desktop/models/
```

Set `SONASUB_MODELS_DIR` to use another root for tests or local experiments:

```bash
SONASUB_MODELS_DIR=/path/to/models pnpm prepare:models
SONASUB_MODELS_DIR=/path/to/models apps/desktop/src-tauri/target/debug/sonasub-desktop
```

The desktop app and the ignored native tests read the same variable. The Models
page always shows the active install root, so a test run cannot be mistaken for
the normal application-data directory.

## Preparation

```bash
pnpm prepare:models
```

The preparation script:

- downloads through `/usr/bin/curl` with retries, resumable partial files and
  exact archive-size checks;
- keeps successful downloads in `models/.cache/` so a later failure does not
  discard earlier progress;
- extracts archives into a temporary directory and atomically installs files;
- verifies every installed file against the pinned SHA-256 value.

Re-run the same command after a network failure. Cached archives are not used
by the application at runtime and may be deleted safely.

Node.js is used only by this development script. The packaged desktop app
downloads archives itself in Rust and has no Node.js runtime.

## Pinned Model Set

| Purpose                  | Model                           | Version/source                                                | Runtime files                                                                                    |
| ------------------------ | ------------------------------- | ------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| Streaming ASR            | Zipformer bilingual zh-en, int8 | `sherpa-onnx-streaming-zipformer-bilingual-zh-en-2023-02-20`  | `sherpa/streaming-zipformer-bilingual-zh-en/{encoder,decoder,joiner}.onnx`, `tokens.txt`         |
| Segmented ASR            | SenseVoice multilingual, int8   | `sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17`     | `sherpa/sense-voice-int8/{model.onnx,tokens.txt}`                                                |
| Segmented ASR            | Qwen3-ASR 0.6B, int8            | Official `sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25`         | `sherpa/qwen3-asr-0.6b-int8/{conv_frontend.onnx,encoder.int8.onnx,decoder.int8.onnx,tokenizer/}` |
| Segmented ASR            | Qwen3-ASR 1.7B, int8            | ModelScope `zengshuishui/Qwen3-ASR-onnx` at pinned revision   | `sherpa/qwen3-asr-1.7b-int8/{conv_frontend.onnx,encoder.int8.onnx,decoder.int8.onnx,tokenizer/}` |
| VAD                      | Silero VAD                      | `silero_vad.onnx` from sherpa-onnx                            | `sherpa/silero_vad.onnx`                                                                         |
| Speaker embedding        | 3D-Speaker CAM++ zh-en, 16 kHz  | `3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx` | `sherpa/speaker/campplus-zh-en.onnx`                                                             |
| Diarization              | Pyannote segmentation 3.0, int8 | `sherpa-onnx-pyannote-segmentation-3-0`                       | `sherpa/diarization/pyannote-segmentation-3.0.onnx`                                              |
| Translation en → zh-Hans | Bergamot v3.0                   | Firefox Translations model attachments                        | `bergamot/en-zh-Hans/{model.bin,src.spm,trg.spm,lex.bin}`                                        |
| Translation zh-Hans → en | Bergamot v3.1                   | Firefox Translations model attachments                        | `bergamot/zh-Hans-en/{model.bin,vocab.spm,lex.bin}`                                              |

The sherpa-onnx runtime is pinned through the official `sherpa-onnx` Rust crate
v2.0.1. The native Bergamot adapter is `fxtranslate 0.5.0`. The optional
OpenAI-compatible adapter uses `adk-model 2.3.0` and adds no model weights to
the installer; its model name is resolved by the remote endpoint.

## Native Runtime

The `sherpa-onnx` crate downloads and statically links a version-matched
native archive during compilation. No custom source build, patch or legacy
runtime path override is required.

The regression test forces the crash condition with a real two-speaker clip and
one cluster:

```bash
cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml \
  --test native_pipeline diarization_handles_more_active_speakers_than_clusters \
  -- --ignored --nocapture
```

The official v2.0.1 runtime must return speaker segments for this input.

## Capabilities

- The streaming Zipformer accepts 16 kHz mono audio and produces partial and
  final text continuously. It does not wait for a VAD endpoint.
- The streaming Zipformer detects the language per utterance and cannot honour
  a forced source language. The capability reports that, the configuration
  control is locked on automatic, and the session reports the language that is
  actually in effect. The segmented SenseVoice accepts a forced language, and
  a configuration that forces an unsupported language is rejected before the
  session starts.
- SenseVoice uses Silero VAD and a maximum segment length to produce complete
  segment text. It is segmented recognition, not native streaming.
- Qwen3-ASR 0.6B and 1.7B reuse Silero VAD, timeline calculation, discontinuity
  handling, subtitle finalization, flush and shutdown. Both report only the
  segmented capability and never claim a native partial stream.
- Both Qwen models declare the 30 languages supported by the Qwen3-ASR family.
  sherpa-onnx v2.0.1 exposes neither a model-config language field nor a
  `language` option on the Qwen offline stream. SonaSub therefore keeps Qwen
  on automatic language detection, rejects a persisted forced language, and
  disables the language control in the frontend.
- SenseVoice recognizes Chinese, English, Japanese, Korean and Cantonese.
  Bergamot translation is limited to the two pinned directions (`en` and
  `zh-Hans` targets). Other targets remain source-only with an explicit
  `unavailable` translation state when Bergamot is selected. Conservative
  script routing handles Chinese, English, Japanese kana and Korean Hangul when
  Qwen has no language tag; other scripts are marked `unknown` instead of being
  routed incorrectly through Bergamot.
- OpenAI-Compatible selection skips local translation-model checks. It exposes
  all 30 registered target languages, accepts a custom Base URL and model, and
  allows an empty API key for unauthenticated local endpoints. It does not
  discover models, test the connection at startup or maintain conversation
  history.
- Bergamot returns one complete translated string for a submitted source
  string. The adapter declares `text` only; `streaming` is not supported.
- The translation adapter separates the native runtime (`fxtranslate`) from
  the model family (`bergamot-firefox`). The runtime executes the pinned
  weights; the model family names their Firefox Translations format.
- The remote adapter reports `adk-model` as its runtime and
  `openai-compatible` as its model family. It sends one Chat Completions
  request per caption with a 20-second timeout and retries disabled, so a slow
  or failing endpoint cannot create hidden duplicate requests.
- CAM++ labels finalized subtitle segments. Pyannote analysis runs over
  bounded audio windows and associates local clusters with session-wide
  speakers through embeddings; it is windowed quasi-real-time diarization,
  not whole-recording analysis.
- Pyannote overlap regions mark uncertain multi-speaker attribution. No
  source-separation model is included, so overlapping speech is not
  transcribed into separate speaker tracks.

## Integrity And Startup

When Bergamot is selected, the app checks that every required local translation
file exists and reaches the model's minimum size before starting a session. It
does not hash hundreds of megabytes on the UI thread at startup. Full SHA-256
verification is performed by `pnpm prepare:models` and by the ignored native
test:

```bash
cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml \
  --test native_pipeline model_files_match_the_recorded_checksums -- --ignored --nocapture
```

If files are missing, the readiness panel lists the affected model and the
session start fails with the absolute expected path instead of selecting a
different engine. OpenAI-Compatible instead validates that the Base URL is an
absolute `http://` or `https://` address and that a model name is present. An
empty API key is valid. Startup constructs the client without making a network
request.

Rust compilation fails if the official crate cannot obtain or link its
version-matched native runtime.

### Remote translation and local credentials

OpenAI-Compatible sends the finalized caption text to the configured Base URL.
The Base URL, remote model and API key are saved as plain text in the
application `config.json` and in each record's `records.sqlite` configuration
snapshot. The key is redacted from `Debug` output, logs and surfaced errors.
There is no environment-variable fallback.

The engine can use any OpenAI-compatible Chat Completions endpoint, but the
remote model decides which of the 30 target languages it actually supports.
SonaSub does not bundle remote model weights, discover models or verify the
endpoint during startup.

## Installation In The App

Open **Models** in the desktop app. Models are grouped as complete bundles:
speech recognition (streaming ASR and segmented ASR), translation (each
Bergamot direction), and supporting models (VAD, speaker embedding and
diarization). Every card shows the approximate download size, installed
footprint, compute needs, quality notes, and how many required files are
already present. The page header shows both the fixed install root and the
selected download directory.

Each bundle offers two methods:

- **Download and install**: downloads the pinned official archive or file(s)
  into the selected download directory, verifies each pinned SHA-256 and
  installs the runtime files into the application-data directory.
- **Import selected files**: selects the official archive, the downloaded
  model attachments, or all extracted runtime files from disk. The same
  verification and atomic install path is used.
- **Import model folder**: recursively collects a selected extracted model or
  tokenizer directory, matches the pinned file names and requires the complete
  Qwen runtime set before installing it.

Use **Choose download directory** to keep archives elsewhere; SonaSub saves
that preference in the application config directory. **Use default directory**
stores archives under `models/downloads`.

Downloads and imports run on Rust worker tasks. A file is written to a
temporary path, fully hashed, and renamed into place only when its checksum
matches. A failure leaves the previous installed file untouched. Downloads use
connect and read timeouts, and discard an incomplete temporary file so the next
attempt starts cleanly.

The official Qwen3-ASR 0.6B archive is checked against its archive SHA-256
before extraction and every extracted runtime file is checked again. The 1.7B
community export is downloaded as six files from a fixed ModelScope revision;
each file has an exact byte size and SHA-256. The two models have separate
installation roots and can coexist with SenseVoice, Zipformer and each other.

### Qwen3-ASR 1.7B source and license

The sherpa-onnx models release documents the 0.6B checkpoint directly. The
1.7B path uses `zengshuishui/Qwen3-ASR-onnx`, pinned to revision
`9c182309f7bb075f241424441add9e16c5086dfb`. Its ModelScope card declares
Apache-2.0 and links the sherpa-onnx compatibility implementation. The same
card explicitly calls the export a reference demo and gives no completeness or
production-readiness guarantee.

This repository pins the export because it is the source linked by the official
sherpa-onnx documentation, but its runtime behavior and resource use remain a
validation item. The app must not relabel it as an official Qwen release or an
officially production-supported sherpa-onnx asset.

`pnpm prepare:models` remains a developer convenience for obtaining the full
pinned set from a checkout. It is not required by the packaged application.

The flow uses these Tauri commands and permissions:

- `list_model_installs` / `allow-list-model-installs` lists bundles and pinned
  source information.
- `get_model_download_settings` and `set_model_download_directory` manage the
  optional download directory.
- `download_model` / `allow-download-model` downloads and installs one bundle.
- `import_model_files` / `allow-import-model-files` imports one bundle.
- `dialog:default` lets the user choose files or a directory.

## Pinned SHA-256

These values are duplicated in
`apps/desktop/src-tauri/src/models/registry.rs` so the preparation script and
the runtime model registry cannot drift silently.

| File                                                      | SHA-256                                                            |
| --------------------------------------------------------- | ------------------------------------------------------------------ |
| `sherpa/streaming-zipformer-bilingual-zh-en/encoder.onnx` | `8fa764187a261844f859d7143ebaa563af5d10adfece4c18a8f414c88cba2a9b` |
| `sherpa/streaming-zipformer-bilingual-zh-en/decoder.onnx` | `1a70c593d71e53f023f5f55b0b4cfff5055abb786ee3992e5f63dc2e273cc4fa` |
| `sherpa/streaming-zipformer-bilingual-zh-en/joiner.onnx`  | `1ed689c5ed19dbaa725d9d191bb4822b5f4855a39e1ffd28cbc1f340d25b2ee0` |
| `sherpa/streaming-zipformer-bilingual-zh-en/tokens.txt`   | `a8e0e4ec53810e433789b54a5c0134a7eaa2ffca595a6334d54c00da858841d3` |
| `sherpa/sense-voice-int8/model.onnx`                      | `c71f0ce00bec95b07744e116345e33d8cbbe08cef896382cf907bf4b51a2cd51` |
| `sherpa/sense-voice-int8/tokens.txt`                      | `f449eb28dc567533d7fa59be34e2abca8784f771850c78a47fb731a31429a1dc` |
| Qwen3 0.6B archive                                        | `393f8a14e2f5fb96746aaab342997a40641001fbd5bf9592a080a8329178ee96` |
| `sherpa/qwen3-asr-0.6b-int8/conv_frontend.onnx`           | `d22dc4423e0940e49884e903d2ea2f7e5567c14fc1aed97e4e26d6b8f208ef9e` |
| `sherpa/qwen3-asr-0.6b-int8/encoder.int8.onnx`            | `60748d3e6744a57c9c91e1b17424a6c2990567e8adceb0783940c03ed98fa9d9` |
| `sherpa/qwen3-asr-0.6b-int8/decoder.int8.onnx`            | `4f6885be5959ae26af3089d38ee7972c5fafbeeb1cf8d5e76eab6d8b61ca5771` |
| `sherpa/qwen3-asr-1.7b-int8/conv_frontend.onnx`           | `fa894a4ba53da6a4238f2a6ca0b09362e505d39cecbd646051b033e2e8d7e2fb` |
| `sherpa/qwen3-asr-1.7b-int8/encoder.int8.onnx`            | `436fbd910a0c8914851e5ac1354e807be9f283d08a5da728adaa609731c41469` |
| `sherpa/qwen3-asr-1.7b-int8/decoder.int8.onnx`            | `c43c853fa6e97d08365cb8a5502b360b595cd43c00dc60e4d8ca7cc18cad460b` |
| `sherpa/silero_vad.onnx`                                  | `9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6` |
| `sherpa/speaker/campplus-zh-en.onnx`                      | `aa3cfc16963a10586a9393f5035d6d6b57e98d358b347f80c2a30bf4f00ceba2` |
| `sherpa/diarization/pyannote-segmentation-3.0.onnx`       | `d582f4b4c6b48205de7e0643c57df0df5615a3c176189be3fc461e9d18827b5d` |
| `bergamot/en-zh-Hans/model.bin`                           | `4e5accc141373565ddc8fa1565bceaa8d0c3482a82cab8131c719ebcc6c2157c` |
| `bergamot/en-zh-Hans/src.spm`                             | `bd9b65504acc6d9726dd281f7defc2adb7c2c22d0688fe2f84697de25197c8c5` |
| `bergamot/en-zh-Hans/trg.spm`                             | `aded6993c36e440284d11cec3f6b8aef9c0e43188a772d80be342a713adf223d` |
| `bergamot/en-zh-Hans/lex.bin`                             | `4a5e5827788060f1d718a8132b69440929387514a045796e9b77f935db68c055` |
| `bergamot/zh-Hans-en/model.bin`                           | `5cd149601802fc8a18124a1c1306144dbbedc058630c4ddb2d53aa76fa9c7c06` |
| `bergamot/zh-Hans-en/vocab.spm`                           | `dff594318ab7d8b7b60b844ab98ebe6b932ae8045fab15235404c787715965b3` |
| `bergamot/zh-Hans-en/lex.bin`                             | `8524dd4c93ffd7f0ae7be32f77a7c14fea7cdbbebe34ea6ad0ec368d5ddd8b13` |

Both Qwen tokenizer directories share these pinned files:

| File component          | SHA-256                                                            |
| ----------------------- | ------------------------------------------------------------------ |
| `merges.txt`            | `8831e4f1a044471340f7c0a83d7bd71306a5b867e95fd870f74d0c5308a904d5` |
| `tokenizer_config.json` | `4942d005604266809309cabc9f4e9cb89ce855d59b14681fdc0e1cc62ea26c4c` |
| `vocab.json`            | `ca10d7e9fb3ed18575dd1e277a2579c16d108e32f27439684afa0e10b1440910` |
