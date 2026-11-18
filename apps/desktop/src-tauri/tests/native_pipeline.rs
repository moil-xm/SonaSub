use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use sherpa_onnx::{
    FastClusteringConfig, OfflineSpeakerDiarization, OfflineSpeakerDiarizationConfig,
};
use sonasub_desktop_lib::{
    asr::{AsrEngine, AsrEvent, Recognition},
    config::{AsrMode, RecognitionConfig},
    models::{
        translation_files, ModelPaths, ASR_MODELS, ASR_SEGMENTED_ID, ASR_STREAMING_ID,
        BERGAMOT_ENGINE_ID, DIARIZATION_FILES, SPEAKER_FILES, TRANSLATION_MODELS, VAD_FILES,
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
    let paths = models();
    let config = RecognitionConfig {
        mode,
        model_id: model_id.to_owned(),
        source_language: "auto".to_owned(),
        max_segment_secs: 12.0,
        ..RecognitionConfig::default()
    };
    let mut engine = AsrEngine::load(mode, &paths, &config, model_id).expect("load ASR model");
    let mut finals = Vec::new();
    let mut partials = Vec::new();
    let chunk = 3_200usize;
    for (index, samples) in samples.chunks(chunk).enumerate() {
        let start_frame = (index * chunk * 3) as u64;
        let mut emit = |event| match event {
            AsrEvent::Final(final_event) => finals.push(final_event.text),
            AsrEvent::Partial(partial) => partials.push(partial.text),
            AsrEvent::Warning(message) => eprintln!("ASR warning: {message}"),
            AsrEvent::Fatal(message) => panic!("ASR failed: {message}"),
        };
        engine
            .push(samples, start_frame, &mut emit)
            .expect("push audio");
    }
    let mut emit = |event| match event {
        AsrEvent::Final(final_event) => finals.push(final_event.text),
        AsrEvent::Partial(partial) => partials.push(partial.text),
        AsrEvent::Warning(message) => eprintln!("ASR warning: {message}"),
        AsrEvent::Fatal(message) => panic!("ASR failed: {message}"),
    };
    engine.flush(&mut emit).expect("flush ASR");
    (finals, partials)
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
