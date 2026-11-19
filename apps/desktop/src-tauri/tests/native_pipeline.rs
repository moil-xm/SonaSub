use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use crossbeam_channel::{bounded, unbounded, Receiver};
use sherpa_onnx::{
    FastClusteringConfig, OfflineSpeakerDiarization, OfflineSpeakerDiarizationConfig,
};
use sonasub_desktop_lib::{
    asr::{AsrControl, AsrEngine, AsrEvent, AsrInput, Recognition},
    config::{AsrMode, RecognitionConfig},
    models::{
        translation_files, ModelPaths, ASR_MODELS, ASR_QWEN3_06B_ID, ASR_QWEN3_17B_ID,
        ASR_SEGMENTED_ID, ASR_STREAMING_ID, BERGAMOT_ENGINE_ID, DIARIZATION_FILES, SPEAKER_FILES,
        TRANSLATION_MODELS, VAD_FILES,
    },
    speaker::{DiarizationEngine, SpeakerRegistry},
    translation::{BergamotEngine, TranslationEngine},
};

fn models() -> ModelPaths {
    if let Ok(root) = std::env::var("SONASUB_MODELS_DIR") {
        return ModelPaths::new(PathBuf::from(root));
    }
    let home = std::env::var("HOME").expect("HOME must be set");
    ModelPaths::new(
        PathBuf::from(home).join("Library/Application Support/dev.sonasub.desktop/models"),
    )
}

fn audio_path(name: &str) -> PathBuf {
    let root = std::env::var("SONASUB_TEST_AUDIO_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp/sonasub-research/audio"));
    root.join(name)
}

fn read_16k_mono(path: &Path) -> Vec<f32> {
    let mut reader = hound::WavReader::open(path)
        .unwrap_or_else(|error| panic!("failed to open {}: {error}", path.display()));
    let spec = reader.spec();
    assert_eq!(spec.sample_rate, 16_000, "test clips must be 16 kHz");
    assert_eq!(spec.channels, 1, "test clips must be mono");
    match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .expect("valid float WAV"),
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|sample| sample.map(|value| value as f32 * scale))
                .collect::<Result<Vec<_>, _>>()
                .expect("valid integer WAV")
        }
    }
}

fn run_asr(model_id: &str, mode: AsrMode, samples: &[f32]) -> (Vec<String>, Vec<String>) {
    run_asr_with_language(model_id, mode, "auto", samples)
}

fn run_asr_with_language(
    model_id: &str,
    mode: AsrMode,
    source_language: &str,
    samples: &[f32],
) -> (Vec<String>, Vec<String>) {
    let run = run_asr_measured(model_id, mode, source_language, samples);
    (run.finals, run.partials)
}

struct AsrRun {
    finals: Vec<String>,
    partials: Vec<String>,
    languages: Vec<Option<String>>,
    initialization_secs: f64,
    processing_secs: f64,
}

fn run_asr_measured(
    model_id: &str,
    mode: AsrMode,
    source_language: &str,
    samples: &[f32],
) -> AsrRun {
    let paths = models();
    let config = RecognitionConfig {
        mode,
        model_id: model_id.to_owned(),
        source_language: source_language.to_owned(),
        max_segment_secs: 12.0,
        ..RecognitionConfig::default()
    };
    let initialization_started = Instant::now();
    let mut engine = AsrEngine::load(mode, &paths, &config, model_id).expect("load ASR model");
    let initialization_secs = initialization_started.elapsed().as_secs_f64();
    let processing_started = Instant::now();
    let mut finals = Vec::new();
    let mut partials = Vec::new();
    let mut languages = Vec::new();
    let chunk = 3_200usize;
    for (index, samples) in samples.chunks(chunk).enumerate() {
        let start_frame = (index * chunk * 3) as u64;
        let mut emit = |event| match event {
            AsrEvent::Final(final_event) => {
                finals.push(final_event.text);
                languages.push(final_event.language);
            }
            AsrEvent::Partial(partial) => partials.push(partial.text),
            AsrEvent::Warning(message) => eprintln!("ASR warning: {message}"),
            AsrEvent::Fatal(message) => panic!("ASR failed: {message}"),
        };
        engine
            .push(samples, start_frame, &mut emit)
            .expect("push audio");
    }
    let mut emit = |event| match event {
        AsrEvent::Final(final_event) => {
            finals.push(final_event.text);
            languages.push(final_event.language);
        }
        AsrEvent::Partial(partial) => partials.push(partial.text),
        AsrEvent::Warning(message) => eprintln!("ASR warning: {message}"),
        AsrEvent::Fatal(message) => panic!("ASR failed: {message}"),
    };
    engine.flush(&mut emit).expect("flush ASR");
    AsrRun {
        finals,
        partials,
        languages,
        initialization_secs,
        processing_secs: processing_started.elapsed().as_secs_f64(),
    }
}

fn has_repeated_word_ngram(text: &str, size: usize) -> bool {
    let words = text
        .split_whitespace()
        .map(|word| word.to_lowercase())
        .collect::<Vec<_>>();
    if words.len() < size * 2 {
        return false;
    }
    let windows = words.windows(size).collect::<Vec<_>>();
    windows.iter().enumerate().any(|(index, candidate)| {
        windows
            .iter()
            .enumerate()
            .any(|(other_index, other)| other_index != index && other == candidate)
    })
}

enum RealtimeEvent {
    Final {
        end_secs: f64,
        text: String,
        language: Option<String>,
    },
    Partial,
    Warning(String),
    Fatal(String),
    PushTiming {
        elapsed: Duration,
        finals: usize,
    },
    Done,
}

#[derive(Default)]
struct RealtimeMetrics {
    finals: Vec<String>,
    languages: Vec<Option<String>>,
    final_latencies_ms: Vec<u64>,
    max_queue_depth: usize,
    dropped_audio_chunks: usize,
    busy_duration: Duration,
    max_push_duration: Duration,
    max_finals_per_push: usize,
    done: bool,
    fatal: Option<String>,
}

impl RealtimeMetrics {
    fn record(&mut self, event: RealtimeEvent, started: Instant) {
        match event {
            RealtimeEvent::Final {
                end_secs,
                text,
                language,
            } => {
                let audio_elapsed = started.elapsed().as_secs_f64();
                self.final_latencies_ms
                    .push(((audio_elapsed - end_secs).max(0.0) * 1_000.0).round() as u64);
                self.finals.push(text);
                self.languages.push(language);
            }
            RealtimeEvent::Partial => {}
            RealtimeEvent::Warning(message) => eprintln!("ASR warning: {message}"),
            RealtimeEvent::Fatal(message) => self.fatal = Some(message),
            RealtimeEvent::PushTiming { elapsed, finals } => {
                self.busy_duration += elapsed;
                self.max_push_duration = self.max_push_duration.max(elapsed);
                self.max_finals_per_push = self.max_finals_per_push.max(finals);
            }
            RealtimeEvent::Done => self.done = true,
        }
    }

    fn drain_available(&mut self, events: &Receiver<RealtimeEvent>, started: Instant) {
        while let Ok(event) = events.try_recv() {
            self.record(event, started);
        }
    }
}

struct RealtimeRun {
    audio_secs: f64,
    wall_secs: f64,
    metrics: RealtimeMetrics,
}

fn run_realtime_qwen(samples: &[f32], cycles: usize) -> RealtimeRun {
    let paths = models();
    let config = RecognitionConfig {
        mode: AsrMode::Segmented,
        model_id: ASR_QWEN3_06B_ID.to_owned(),
        source_language: "auto".to_owned(),
        max_segment_secs: 12.0,
        ..RecognitionConfig::default()
    };
    let mut engine = AsrEngine::load(AsrMode::Segmented, &paths, &config, ASR_QWEN3_06B_ID)
        .expect("load Qwen3 0.6B");

    let (input_tx, input_rx) = bounded(128);
    let (control_tx, control_rx) = bounded(1);
    let (events_tx, events_rx) = unbounded();
    let worker = thread::spawn(move || {
        loop {
            let mut finals = 0usize;
            let mut emit = |event| {
                let event = match event {
                    AsrEvent::Final(final_event) => {
                        finals += 1;
                        RealtimeEvent::Final {
                            end_secs: final_event.end_secs,
                            text: final_event.text,
                            language: final_event.language,
                        }
                    }
                    AsrEvent::Partial(_) => RealtimeEvent::Partial,
                    AsrEvent::Warning(message) => RealtimeEvent::Warning(message),
                    AsrEvent::Fatal(message) => RealtimeEvent::Fatal(message),
                };
                let _ = events_tx.send(event);
            };
            let message = crossbeam_channel::select_biased! {
                recv(control_rx) -> message => {
                    match message {
                        Ok(AsrControl::Stop) => {
                            if let Err(error) = engine.flush(&mut emit) {
                                let _ = events_tx.send(RealtimeEvent::Fatal(error));
                            }
                            break;
                        }
                        Err(_) => break,
                    }
                }
                recv(input_rx) -> message => match message {
                    Ok(message) => message,
                    Err(_) => break,
                },
            };
            let AsrInput::Audio {
                samples,
                start_frame_48k,
            } = message
            else {
                continue;
            };
            let push_started = Instant::now();
            if let Err(error) = engine.push(&samples, start_frame_48k, &mut emit) {
                let _ = events_tx.send(RealtimeEvent::Fatal(error));
                break;
            }
            let _ = events_tx.send(RealtimeEvent::PushTiming {
                elapsed: push_started.elapsed(),
                finals,
            });
        }
        let _ = events_tx.send(RealtimeEvent::Done);
    });

    const CHUNK_SAMPLES: usize = 3_200;
    let total_samples = samples.len() * cycles;
    let total_chunks = total_samples.div_ceil(CHUNK_SAMPLES);
    let started = Instant::now();
    let mut metrics = RealtimeMetrics::default();
    for chunk_index in 0..total_chunks {
        let absolute_sample = chunk_index * CHUNK_SAMPLES;
        let chunk_len = (total_samples - absolute_sample).min(CHUNK_SAMPLES);
        let chunk_end_secs = (absolute_sample + chunk_len) as f64 / 16_000.0;
        let deadline = started + Duration::from_secs_f64(chunk_end_secs);
        if let Some(wait) = deadline.checked_duration_since(Instant::now()) {
            thread::sleep(wait);
        }
        let chunk = (0..chunk_len)
            .map(|offset| samples[(absolute_sample + offset) % samples.len()])
            .collect::<Vec<_>>();
        match input_tx.try_send(AsrInput::Audio {
            samples: chunk,
            start_frame_48k: (absolute_sample * 3) as u64,
        }) {
            Ok(()) => {}
            Err(crossbeam_channel::TrySendError::Full(_)) => {
                metrics.dropped_audio_chunks += 1;
            }
            Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                panic!("ASR worker disconnected during realtime input");
            }
        }
        metrics.max_queue_depth = metrics.max_queue_depth.max(input_tx.len());
        metrics.drain_available(&events_rx, started);
        if metrics.fatal.is_some() || metrics.done {
            break;
        }
    }
    while !input_tx.is_empty() && metrics.fatal.is_none() {
        thread::sleep(Duration::from_millis(10));
        metrics.drain_available(&events_rx, started);
    }
    control_tx
        .send(AsrControl::Stop)
        .expect("send recognition stop");
    while !metrics.done {
        let event = events_rx
            .recv_timeout(Duration::from_secs(30))
            .expect("wait for the recognition worker");
        metrics.record(event, started);
    }
    worker.join().expect("join recognition worker");

    RealtimeRun {
        audio_secs: total_samples as f64 / 16_000.0,
        wall_secs: started.elapsed().as_secs_f64(),
        metrics,
    }
}

#[test]
#[ignore = "requires locally downloaded sherpa-onnx and Bergamot models"]
fn model_files_match_the_recorded_checksums() {
    let paths = models();
    let mut files = Vec::new();
    for model in ASR_MODELS {
        files.extend(model.files.iter().cloned());
    }
    for model in TRANSLATION_MODELS {
        files.extend(translation_files(model));
    }
    files.extend(VAD_FILES.iter().cloned());
    files.extend(SPEAKER_FILES.iter().cloned());
    files.extend(DIARIZATION_FILES.iter().cloned());
    let failed = files
        .iter()
        .filter_map(|file| {
            let actual = match paths.verify_file(file) {
                Ok(actual) => actual,
                Err(error) => return Some(format!("{}: {error}", file.relative)),
            };
            let Some(expected) = file.sha256 else {
                return Some(format!("{} has no recorded checksum", file.relative));
            };
            if expected.eq_ignore_ascii_case(&actual) {
                None
            } else {
                Some(format!(
                    "{} checksum mismatch: expected {expected}, got {actual}",
                    file.relative
                ))
            }
        })
        .collect::<Vec<_>>();
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

#[test]
#[ignore = "requires locally downloaded sherpa-onnx models and test WAVs"]
fn streaming_asr_transcribes_a_real_clip() {
    let samples = read_16k_mono(&audio_path("1-two-speakers-en.wav"));
    let (finals, partials) = run_asr(ASR_STREAMING_ID, AsrMode::Streaming, &samples);
    eprintln!("streaming finals: {finals:?}");
    assert!(!partials.is_empty(), "streaming ASR must emit partial text");
    assert!(!finals.is_empty(), "streaming ASR must emit final text");
    assert!(finals.iter().any(|text| !text.trim().is_empty()));
}

#[test]
#[ignore = "requires locally downloaded sherpa-onnx models and test WAVs"]
fn segmented_asr_transcribes_a_real_clip() {
    let samples = read_16k_mono(&audio_path("3-two-speakers-en.wav"));
    let (finals, _) = run_asr(ASR_SEGMENTED_ID, AsrMode::Segmented, &samples);
    eprintln!("segmented finals: {finals:?}");
    assert!(!finals.is_empty(), "segmented ASR must emit final text");
    assert!(finals.iter().any(|text| !text.trim().is_empty()));
}

#[test]
#[ignore = "requires the Qwen3 ASR 0.6B model, Silero VAD and a test WAV"]
fn qwen3_06b_transcribes_a_real_clip_in_auto_language_mode() {
    let paths = models();
    let model = ASR_MODELS
        .iter()
        .find(|model| model.id == ASR_QWEN3_06B_ID)
        .unwrap();
    let missing = paths.missing(model.files);
    assert!(
        missing.is_empty(),
        "prepare the Qwen3 0.6B model first: {}",
        missing.join(", ")
    );
    let samples = read_16k_mono(&audio_path("1-two-speakers-en.wav"));
    let (finals, partials) =
        run_asr_with_language(ASR_QWEN3_06B_ID, AsrMode::Segmented, "auto", &samples);
    eprintln!("Qwen3 0.6B finals: {finals:?}");
    assert!(partials.is_empty(), "Qwen3 is segmented, not streaming");
    assert!(!finals.is_empty(), "Qwen3 must emit final text");
    assert!(finals.iter().any(|text| !text.trim().is_empty()));
}

#[test]
#[ignore = "requires the Qwen3 ASR 0.6B model, Silero VAD and generated multilingual WAVs"]
fn qwen3_06b_transcribes_chinese_japanese_and_mixed_clips() {
    for (language, file) in [
        ("auto", "qwen-zh.wav"),
        ("auto", "qwen-ja.wav"),
        ("auto", "qwen-mixed.wav"),
    ] {
        let samples = read_16k_mono(&audio_path(file));
        let (finals, partials) =
            run_asr_with_language(ASR_QWEN3_06B_ID, AsrMode::Segmented, language, &samples);
        eprintln!("Qwen3 0.6B {language} finals: {finals:?}");
        assert!(partials.is_empty(), "Qwen3 is segmented, not streaming");
        assert!(!finals.is_empty(), "Qwen3 must emit final text for {file}");
        assert!(
            finals.iter().all(|text| !text.contains("<asr_text>")),
            "Qwen3 decoder scaffolding must not reach subtitle text"
        );
    }
}

#[test]
#[ignore = "requires the Qwen3 ASR 0.6B model, Silero VAD and a generated long WAV"]
fn qwen3_06b_handles_a_long_clip_and_records_runtime_metrics() {
    let samples = read_16k_mono(&audio_path("qwen-long.wav"));
    let run = run_asr_measured(ASR_QWEN3_06B_ID, AsrMode::Segmented, "auto", &samples);
    let audio_secs = samples.len() as f64 / 16_000.0;
    eprintln!(
        "Qwen3 0.6B long clip: audio={audio_secs:.3}s init={:.3}s processing={:.3}s rtf={:.3} finals={:?}",
        run.initialization_secs,
        run.processing_secs,
        run.processing_secs / audio_secs,
        run.finals
    );
    assert!(run.partials.is_empty(), "Qwen3 is segmented, not streaming");
    assert!(
        run.finals.len() >= 2,
        "the long clip should cross multiple VAD segments"
    );
    assert!(run.finals.iter().all(|text| !text.contains("<asr_text>")));
}

#[test]
#[ignore = "requires Qwen3 0.6B and SONASUB_QWEN_LONG_TEST=1 for a 30-minute realtime run"]
fn qwen3_06b_sustains_thirty_minutes_of_realtime_audio() {
    if std::env::var("SONASUB_QWEN_LONG_TEST").as_deref() != Ok("1") {
        eprintln!("Set SONASUB_QWEN_LONG_TEST=1 to run the 30-minute realtime test.");
        return;
    }
    let samples = read_16k_mono(&audio_path("qwen-long.wav"));
    let run = run_realtime_qwen(&samples, 51);
    let average_latency_ms = run.metrics.final_latencies_ms.iter().copied().sum::<u64>()
        / run.metrics.final_latencies_ms.len().max(1) as u64;
    let max_latency_ms = run
        .metrics
        .final_latencies_ms
        .iter()
        .copied()
        .max()
        .unwrap_or(0);
    eprintln!(
        "Qwen3 0.6B realtime: audio={:.3}s wall={:.3}s finals={} max_queue={} dropped={} busy={:.3}s rtf={:.3} avg_final_latency={}ms max_final_latency={}ms max_push={:.3}s max_finals_per_push={}",
        run.audio_secs,
        run.wall_secs,
        run.metrics.finals.len(),
        run.metrics.max_queue_depth,
        run.metrics.dropped_audio_chunks,
        run.metrics.busy_duration.as_secs_f64(),
        run.metrics.busy_duration.as_secs_f64() / run.audio_secs,
        average_latency_ms,
        max_latency_ms,
        run.metrics.max_push_duration.as_secs_f64(),
        run.metrics.max_finals_per_push,
    );
    assert!(run.metrics.fatal.is_none(), "{:?}", run.metrics.fatal);
    assert!(run.audio_secs >= 1_798.0);
    assert!(run.metrics.dropped_audio_chunks == 0);
    assert!(!run.metrics.finals.is_empty());
}

#[test]
#[ignore = "requires the 2.4 GB Qwen3 ASR 1.7B model, Silero VAD and a test WAV"]
fn qwen3_17b_transcribes_a_real_clip() {
    let paths = models();
    let model = ASR_MODELS
        .iter()
        .find(|model| model.id == ASR_QWEN3_17B_ID)
        .unwrap();
    let missing = paths.missing(model.files);
    assert!(
        missing.is_empty(),
        "prepare the Qwen3 1.7B model first: {}",
        missing.join(", ")
    );
    let samples = read_16k_mono(&audio_path("1-two-speakers-en.wav"));
    let run = run_asr_measured(ASR_QWEN3_17B_ID, AsrMode::Segmented, "auto", &samples);
    eprintln!("Qwen3 1.7B finals: {:?}", run.finals);
    eprintln!("Qwen3 1.7B languages: {:?}", run.languages);
    assert!(run.partials.is_empty(), "Qwen3 is segmented, not streaming");
    assert!(!run.finals.is_empty(), "Qwen3 must emit final text");
    assert!(run.finals.iter().any(|text| !text.trim().is_empty()));
    assert!(
        run.finals.iter().all(|text| !text.contains("<asr_text>")),
        "Qwen3 decoder scaffolding must not reach subtitle text"
    );
    assert!(
        run.languages
            .iter()
            .flatten()
            .any(|language| language == "en"),
        "Qwen3 language metadata must survive decoder-scaffold cleanup"
    );
}

#[test]
#[ignore = "requires the 2.4 GB Qwen3 ASR 1.7B model, Silero VAD and generated multilingual WAVs"]
fn qwen3_17b_transcribes_chinese_japanese_and_mixed_clips() {
    for (language, file) in [
        ("auto", "qwen-zh.wav"),
        ("auto", "qwen-ja.wav"),
        ("auto", "qwen-mixed.wav"),
    ] {
        let samples = read_16k_mono(&audio_path(file));
        let run = run_asr_measured(ASR_QWEN3_17B_ID, AsrMode::Segmented, language, &samples);
        eprintln!(
            "Qwen3 1.7B {language} finals: {:?}, languages: {:?}",
            run.finals, run.languages
        );
        assert!(run.partials.is_empty(), "Qwen3 is segmented, not streaming");
        assert!(
            !run.finals.is_empty(),
            "Qwen3 must emit final text for {file}"
        );
        assert!(
            run.finals.iter().all(|text| !text.contains("<asr_text>")),
            "Qwen3 decoder scaffolding must not reach subtitle text"
        );
        assert!(
            run.finals
                .iter()
                .all(|text| !has_repeated_word_ngram(text, 8)),
            "Qwen3 must not emit an eight-word decoding loop"
        );
    }
}

#[test]
#[ignore = "requires the 2.4 GB Qwen3 ASR 1.7B model, Silero VAD and a generated long WAV"]
fn qwen3_17b_handles_a_long_clip_without_degenerate_repetition() {
    let samples = read_16k_mono(&audio_path("qwen-long.wav"));
    let run = run_asr_measured(ASR_QWEN3_17B_ID, AsrMode::Segmented, "auto", &samples);
    let audio_secs = samples.len() as f64 / 16_000.0;
    eprintln!(
        "Qwen3 1.7B long clip: audio={audio_secs:.3}s init={:.3}s processing={:.3}s rtf={:.3} finals={:?}",
        run.initialization_secs,
        run.processing_secs,
        run.processing_secs / audio_secs,
        run.finals
    );
    assert!(run.partials.is_empty(), "Qwen3 is segmented, not streaming");
    assert!(
        run.finals.len() >= 2,
        "the long clip should cross multiple VAD segments"
    );
    assert!(run.finals.iter().all(|text| !text.contains("<asr_text>")));
    assert!(
        run.finals
            .iter()
            .all(|text| !has_repeated_word_ngram(text, 8)),
        "Qwen3 must not emit an eight-word decoding loop"
    );
    assert!(
        run.finals.windows(2).all(|pair| pair[0] != pair[1]),
        "Qwen3 must not repeat the same consecutive final"
    );
}

#[test]
#[ignore = "requires locally downloaded Bergamot models"]
fn bergamot_translates_both_configured_directions() {
    let paths = models();
    let en_zh = TRANSLATION_MODELS
        .iter()
        .find(|model| model.source_language == "en" && model.target_language == "zh-Hans")
        .unwrap();
    let zh_en = TRANSLATION_MODELS
        .iter()
        .find(|model| model.source_language == "zh-Hans" && model.target_language == "en")
        .unwrap();
    let mut engine = BergamotEngine::load(&paths, en_zh).unwrap();
    assert_eq!(engine.capabilities().engine, BERGAMOT_ENGINE_ID);
    assert!(!engine.capabilities().streaming);
    let translated = engine
        .translate("The weather is nice today.")
        .expect("English to Chinese");
    eprintln!("en -> zh-Hans: {translated}");
    assert!(translated.chars().any(is_han));
    drop(engine);
    let mut engine = BergamotEngine::load(&paths, zh_en).unwrap();
    let translated = engine
        .translate("今天天气很好。")
        .expect("Chinese to English");
    eprintln!("zh-Hans -> en: {translated}");
    assert!(translated
        .chars()
        .any(|character| character.is_ascii_alphabetic()));
}

#[test]
#[ignore = "requires locally downloaded sherpa-onnx models and a multi-speaker WAV"]
fn windowed_diarization_finds_multiple_speakers() {
    let paths = models();
    let samples = read_16k_mono(&audio_path("0-four-speakers-zh.wav"));
    let segmentation = paths.path("sherpa/diarization/pyannote-segmentation-3.0.onnx");
    let embedding = paths.path("sherpa/speaker/campplus-zh-en.onnx");
    let mut engine = DiarizationEngine::new(&segmentation, &embedding).unwrap();
    let registry = Arc::new(Mutex::new(SpeakerRegistry::new(8, 0.55)));
    let window = engine
        .analyze(0.0, Arc::new(samples), &registry)
        .expect("diarize window");
    eprintln!(
        "diarization: {} intervals, {} overlaps, {} ms",
        window.intervals.len(),
        window.overlap_regions.len(),
        window.latency_ms
    );
    assert!(!window.intervals.is_empty());
    let ids = window
        .intervals
        .iter()
        .filter_map(|interval| interval.speaker_id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(ids.len() >= 2, "expected at least two speaker clusters");
}

#[test]
#[ignore = "requires locally downloaded sherpa-onnx models and a two-speaker WAV"]
fn diarization_handles_more_active_speakers_than_clusters() {
    let paths = models();
    let samples = read_16k_mono(&audio_path("2-two-speakers-en.wav"));
    let segmentation = paths.path("sherpa/diarization/pyannote-segmentation-3.0.onnx");
    let embedding = paths.path("sherpa/speaker/campplus-zh-en.onnx");
    let mut config = OfflineSpeakerDiarizationConfig {
        clustering: FastClusteringConfig {
            num_clusters: 1,
            threshold: 0.5,
            compute_confidence: false,
        },
        min_duration_on: 0.25,
        min_duration_off: 0.25,
        ..OfflineSpeakerDiarizationConfig::default()
    };
    config.segmentation.pyannote.model = Some(segmentation.display().to_string());
    config.embedding.model = Some(embedding.display().to_string());
    let engine = OfflineSpeakerDiarization::create(&config).expect("load diarization models");
    let result = engine
        .process(&samples)
        .expect("diarization must handle more active speakers than clusters");
    assert!(!result.sort_by_start_time().is_empty());
}

fn is_han(character: char) -> bool {
    ('\u{4e00}'..='\u{9fff}').contains(&character)
}
