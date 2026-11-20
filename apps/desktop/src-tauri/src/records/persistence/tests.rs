use std::time::Instant;

use rusqlite::params;

use crate::{
    audio::{AudioDeviceRef, AudioSourceKind},
    config::AppConfig,
    subtitles::{SubtitleSegment, TranslationState, TranslationStatus},
};

use super::super::types::{RecordSource, RecordState, SourceRunState, StoredRecord};
use super::*;

fn config() -> AppConfig {
    let mut config = AppConfig::default();
    config.audio.record = vec![device("device-a")];
    config
}

fn device(id: &str) -> AudioDeviceRef {
    AudioDeviceRef::new(AudioSourceKind::SystemOutput, id)
}

fn segment_for(
    source_run_id: &str,
    device_id: &str,
    local_id: u64,
    occurrence_ms: u64,
    text: &str,
) -> SubtitleSegment {
    SubtitleSegment {
        id: format!("{source_run_id}:{local_id}"),
        session_id: source_run_id.to_owned(),
        record_id: "record".to_owned(),
        source_run_id: source_run_id.to_owned(),
        source_kind: AudioSourceKind::SystemOutput,
        device_id: device_id.to_owned(),
        device_name: device_id.to_owned(),
        local_id,
        start_secs: occurrence_ms as f64 / 1_000.0,
        end_secs: Some(occurrence_ms as f64 / 1_000.0 + 1.0),
        device_time_secs: occurrence_ms as f64 / 1_000.0,
        occurrence_ms,
        text: text.to_owned(),
        revision: 1,
        is_final: true,
        speaker: None,
        speaker_revision: 0,
        translation: Some(TranslationState {
            target_language: "zh-Hans".to_owned(),
            text: Some("你好".to_owned()),
            status: TranslationStatus::Ready,
            revision: 1,
            engine: "bergamot".to_owned(),
            mode: "text".to_owned(),
            model_id: "bergamot-en-zh-Hans".to_owned(),
            error: None,
            updated_at_ms: 10,
        }),
        source_language: Some("en".to_owned()),
        created_at_ms: occurrence_ms,
        updated_at_ms: occurrence_ms,
        last_change: Instant::now(),
    }
}

fn segment() -> SubtitleSegment {
    segment_for("run-a", "device-a", 1, 10_000, "hello")
}

#[test]
fn an_old_record_config_without_openai_compatible_still_loads() {
    let directory = std::env::temp_dir().join(format!("sonasub-records-{}", uuid::Uuid::new_v4()));
    let store = RecordStore::open(directory.join("records.sqlite")).unwrap();
    let config_json = r#"{
        "audio": {
            "live": {"kind": "systemOutput", "deviceId": null},
            "record": [{"kind": "systemOutput", "id": "device-a"}]
        },
        "translation": {
            "engine": "bergamot",
            "modelId": "bergamot-en-zh-Hans",
            "mode": "auto",
            "trigger": "finalOnly",
            "targetLanguage": "zh-Hans"
        }
    }"#;
    store
        .connection()
        .execute(
            "INSERT INTO records (
                id, name, config_json, state, created_at_ms, updated_at_ms,
                started_at_ms, ended_at_ms, last_error
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                "record",
                "Record",
                config_json,
                RecordState::Paused.as_str(),
                1_i64,
                2_i64,
                Option::<i64>::None,
                Option::<i64>::None,
                Option::<String>::None,
            ],
        )
        .unwrap();

    let record = store.load_record("record").unwrap().unwrap();

    assert_eq!(record.config.translation.engine, "bergamot");
    assert_eq!(record.config.translation.model_id, "bergamot-en-zh-Hans");
    assert_eq!(
        record.config.translation.openai_compatible.base_url,
        "https://api.openai.com/v1"
    );
    assert!(record.config.translation.openai_compatible.model.is_empty());
    assert!(record
        .config
        .translation
        .openai_compatible
        .api_key
        .is_empty());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn a_record_config_keeps_openai_compatible_settings() {
    let directory = std::env::temp_dir().join(format!("sonasub-records-{}", uuid::Uuid::new_v4()));
    let store = RecordStore::open(directory.join("records.sqlite")).unwrap();
    let mut config = config();
    config.translation.engine = "openai-compatible".to_owned();
    config.translation.target_language = "ja".to_owned();
    config.translation.openai_compatible.base_url = "https://example.test/v1".to_owned();
    config.translation.openai_compatible.model = "remote-model".to_owned();
    let api_key = ["record", "value"].concat();
    config.translation.openai_compatible.api_key = api_key.clone();
    store
        .save_record(&StoredRecord {
            summary: super::super::types::RecordSummary {
                id: "record".to_owned(),
                name: "Record".to_owned(),
                state: RecordState::Paused,
                devices: vec![device("device-a")],
                active_devices: Vec::new(),
                has_active_source: false,
                last_error: None,
                created_at_ms: 1,
                updated_at_ms: 2,
                revision: 0,
            },
            config,
            started_at_ms: None,
            ended_at_ms: None,
        })
        .unwrap();

    let loaded = store.load_record("record").unwrap().unwrap();

    assert_eq!(loaded.config.translation.engine, "openai-compatible");
    assert_eq!(
        loaded.config.translation.openai_compatible.model,
        "remote-model"
    );
    assert_eq!(loaded.config.translation.openai_compatible.api_key, api_key);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn a_record_config_is_normalized_when_saved_and_loaded() {
    let directory = std::env::temp_dir().join(format!("sonasub-records-{}", uuid::Uuid::new_v4()));
    let store = RecordStore::open(directory.join("records.sqlite")).unwrap();
    let mut config = config();
    config.translation.engine = "openai-compatible".to_owned();
    config.translation.target_language = " zh ".to_owned();
    config.translation.openai_compatible.base_url =
        " https://example.test/v1/chat/completions/ ".to_owned();
    config.translation.openai_compatible.model = " remote-model ".to_owned();
    store
        .save_record(&StoredRecord {
            summary: super::super::types::RecordSummary {
                id: "record".to_owned(),
                name: "Record".to_owned(),
                state: RecordState::Paused,
                devices: vec![device("device-a")],
                active_devices: Vec::new(),
                has_active_source: false,
                last_error: None,
                created_at_ms: 1,
                updated_at_ms: 2,
                revision: 0,
            },
            config,
            started_at_ms: None,
            ended_at_ms: None,
        })
        .unwrap();

    let loaded = store.load_record("record").unwrap().unwrap();

    assert_eq!(loaded.config.translation.target_language, "zh-Hans");
    assert_eq!(
        loaded.config.translation.openai_compatible.base_url,
        "https://example.test/v1"
    );
    assert_eq!(
        loaded.config.translation.openai_compatible.model,
        "remote-model"
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn records_sources_and_subtitles_survive_a_reopen() {
    let directory = std::env::temp_dir().join(format!("sonasub-records-{}", uuid::Uuid::new_v4()));
    let path = directory.join("records.sqlite");
    let store = RecordStore::open(path.clone()).unwrap();
    let record = StoredRecord {
        summary: super::super::types::RecordSummary {
            id: "record".to_owned(),
            name: "Record".to_owned(),
            state: RecordState::Paused,
            devices: vec![device("device-a")],
            active_devices: Vec::new(),
            has_active_source: false,
            last_error: None,
            created_at_ms: 1,
            updated_at_ms: 2,
            revision: 0,
        },
        config: config(),
        started_at_ms: Some(1),
        ended_at_ms: None,
    };
    store.save_record(&record).unwrap();
    store
        .insert_source_run(&RecordSource {
            source_run_id: "run-a".to_owned(),
            record_id: "record".to_owned(),
            source_kind: AudioSourceKind::SystemOutput,
            device_id: "device-a".to_owned(),
            device_name: "Device A".to_owned(),
            state: SourceRunState::Running,
            elapsed_base_ms: 0,
            elapsed_ms: 2_000,
            started_at_ms: 2,
            ended_at_ms: None,
            error: None,
        })
        .unwrap();
    store.upsert_subtitle(&segment()).unwrap();
    drop(store);

    let store = RecordStore::open(path).unwrap();
    assert_eq!(store.list_records().unwrap().len(), 1);
    assert_eq!(
        store
            .device_elapsed_ms("record", &device("device-a"))
            .unwrap(),
        2_000
    );
    let page = store.list_subtitles("record", None, 50).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(
        page.items[0].translation.as_ref().unwrap().text.as_deref(),
        Some("你好")
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn identical_local_ids_from_different_source_runs_coexist() {
    let directory = std::env::temp_dir().join(format!("sonasub-records-{}", uuid::Uuid::new_v4()));
    let store = RecordStore::open(directory.join("records.sqlite")).unwrap();
    store
        .save_record(&StoredRecord {
            summary: super::super::types::RecordSummary {
                id: "record".to_owned(),
                name: "Record".to_owned(),
                state: RecordState::Paused,
                devices: vec![device("device-a"), device("device-b")],
                active_devices: Vec::new(),
                has_active_source: false,
                last_error: None,
                created_at_ms: 1,
                updated_at_ms: 2,
                revision: 0,
            },
            config: config(),
            started_at_ms: Some(1),
            ended_at_ms: None,
        })
        .unwrap();
    store
        .upsert_subtitle(&segment_for("run-a", "device-a", 1, 1_000, "from A"))
        .unwrap();
    store
        .upsert_subtitle(&segment_for("run-b", "device-b", 1, 2_000, "from B"))
        .unwrap();

    let page = store.list_subtitles("record", None, 50).unwrap();
    let texts = page
        .items
        .iter()
        .map(|segment| segment.text.as_str())
        .collect::<Vec<_>>();
    assert_eq!(texts, vec!["from A", "from B"]);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn complete_history_can_be_paged_after_the_live_window_is_trimmed() {
    let directory = std::env::temp_dir().join(format!("sonasub-records-{}", uuid::Uuid::new_v4()));
    let path = directory.join("records.sqlite");
    let store = RecordStore::open(path.clone()).unwrap();
    store
        .save_record(&StoredRecord {
            summary: super::super::types::RecordSummary {
                id: "record".to_owned(),
                name: "Record".to_owned(),
                state: RecordState::Ended,
                devices: vec![device("device-a")],
                active_devices: Vec::new(),
                has_active_source: false,
                last_error: None,
                created_at_ms: 1,
                updated_at_ms: 2,
                revision: 0,
            },
            config: config(),
            started_at_ms: Some(1),
            ended_at_ms: Some(2),
        })
        .unwrap();
    for local_id in 0..7 {
        store
            .upsert_subtitle(&segment_for(
                "run-a",
                "device-a",
                local_id,
                local_id * 1_000,
                &format!("caption {local_id}"),
            ))
            .unwrap();
    }
    assert_eq!(store.list_recent_segments("record", 3).unwrap().len(), 3);
    drop(store);

    let store = RecordStore::open(path).unwrap();
    let mut before = None;
    let mut pages = Vec::new();
    loop {
        let page = store.list_subtitles("record", before.as_ref(), 2).unwrap();
        pages.push(
            page.items
                .iter()
                .map(|segment| segment.local_id)
                .collect::<Vec<_>>(),
        );
        if !page.has_more {
            break;
        }
        before = page.next_before;
    }
    pages.reverse();
    let ids = pages.into_iter().flatten().collect::<Vec<_>>();
    assert_eq!(ids, (0..7).collect::<Vec<_>>());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn interrupted_records_and_sources_recover_as_paused_and_disconnected() {
    let directory = std::env::temp_dir().join(format!("sonasub-records-{}", uuid::Uuid::new_v4()));
    let store = RecordStore::open(directory.join("records.sqlite")).unwrap();
    store
        .save_record(&StoredRecord {
            summary: super::super::types::RecordSummary {
                id: "record".to_owned(),
                name: "Record".to_owned(),
                state: RecordState::Running,
                devices: vec![device("device-a")],
                active_devices: vec![device("device-a")],
                has_active_source: true,
                last_error: None,
                created_at_ms: 1,
                updated_at_ms: 2,
                revision: 0,
            },
            config: config(),
            started_at_ms: Some(1),
            ended_at_ms: None,
        })
        .unwrap();
    store
        .insert_source_run(&RecordSource {
            source_run_id: "run-a".to_owned(),
            record_id: "record".to_owned(),
            source_kind: AudioSourceKind::SystemOutput,
            device_id: "device-a".to_owned(),
            device_name: "Device A".to_owned(),
            state: SourceRunState::Running,
            elapsed_base_ms: 0,
            elapsed_ms: 2_000,
            started_at_ms: 2,
            ended_at_ms: None,
            error: None,
        })
        .unwrap();

    store.recover_active_records(123).unwrap();

    let record = store.load_record("record").unwrap().unwrap();
    assert_eq!(record.summary.state, RecordState::Paused);
    assert_eq!(record.summary.updated_at_ms, 123);
    let sources = store.list_source_runs("record").unwrap();
    assert_eq!(sources[0].state, SourceRunState::Disconnected);
    assert_eq!(sources[0].ended_at_ms, Some(123));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn subtitle_batch_rolls_back_when_one_segment_cannot_be_saved() {
    let directory = std::env::temp_dir().join(format!("sonasub-records-{}", uuid::Uuid::new_v4()));
    let store = RecordStore::open(directory.join("records.sqlite")).unwrap();
    store
        .save_record(&StoredRecord {
            summary: super::super::types::RecordSummary {
                id: "record".to_owned(),
                name: "Record".to_owned(),
                state: RecordState::Paused,
                devices: vec![device("device-a")],
                active_devices: Vec::new(),
                has_active_source: false,
                last_error: None,
                created_at_ms: 1,
                updated_at_ms: 2,
                revision: 0,
            },
            config: config(),
            started_at_ms: Some(1),
            ended_at_ms: None,
        })
        .unwrap();
    store
        .insert_source_run(&RecordSource {
            source_run_id: "run-a".to_owned(),
            record_id: "record".to_owned(),
            source_kind: AudioSourceKind::SystemOutput,
            device_id: "device-a".to_owned(),
            device_name: "Device A".to_owned(),
            state: SourceRunState::Running,
            elapsed_base_ms: 0,
            elapsed_ms: 0,
            started_at_ms: 2,
            ended_at_ms: None,
            error: None,
        })
        .unwrap();

    let valid = segment_for("run-a", "device-a", 1, 1_000, "valid");
    let mut invalid = segment_for("run-a", "device-a", 2, 2_000, "invalid");
    invalid.record_id = "missing-record".to_owned();
    let error = store
        .upsert_subtitles(&[valid, invalid], Some("run-a"), 0)
        .unwrap_err();
    assert!(error.contains("Failed to save subtitle"));
    assert!(store
        .list_subtitles("record", None, 50)
        .unwrap()
        .items
        .is_empty());
    assert_eq!(
        store
            .device_elapsed_ms("record", &device("device-a"))
            .unwrap(),
        0
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn subtitle_batch_updates_elapsed_in_the_same_transaction() {
    let directory = std::env::temp_dir().join(format!("sonasub-records-{}", uuid::Uuid::new_v4()));
    let path = directory.join("records.sqlite");
    let store = RecordStore::open(path.clone()).unwrap();
    store
        .save_record(&StoredRecord {
            summary: super::super::types::RecordSummary {
                id: "record".to_owned(),
                name: "Record".to_owned(),
                state: RecordState::Paused,
                devices: vec![device("device-a")],
                active_devices: Vec::new(),
                has_active_source: false,
                last_error: None,
                created_at_ms: 1,
                updated_at_ms: 2,
                revision: 0,
            },
            config: config(),
            started_at_ms: Some(1),
            ended_at_ms: None,
        })
        .unwrap();
    store
        .insert_source_run(&RecordSource {
            source_run_id: "run-a".to_owned(),
            record_id: "record".to_owned(),
            source_kind: AudioSourceKind::SystemOutput,
            device_id: "device-a".to_owned(),
            device_name: "Device A".to_owned(),
            state: SourceRunState::Running,
            elapsed_base_ms: 1_000,
            elapsed_ms: 0,
            started_at_ms: 2,
            ended_at_ms: None,
            error: None,
        })
        .unwrap();

    store
        .upsert_subtitles(
            &[
                segment_for("run-a", "device-a", 1, 1_000, "first"),
                segment_for("run-a", "device-a", 2, 2_000, "second"),
            ],
            Some("run-a"),
            4_000,
        )
        .unwrap();
    drop(store);

    let store = RecordStore::open(path).unwrap();
    let page = store.list_subtitles("record", None, 50).unwrap();
    assert_eq!(page.items.len(), 2);
    assert_eq!(
        store
            .device_elapsed_ms("record", &device("device-a"))
            .unwrap(),
        5_000
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn deleting_a_record_removes_its_captions_and_device_history() {
    let directory = std::env::temp_dir().join(format!("sonasub-records-{}", uuid::Uuid::new_v4()));
    let path = directory.join("records.sqlite");
    let store = RecordStore::open(path.clone()).unwrap();
    for (id, name) in [("record", "Record"), ("other", "Other")] {
        store
            .save_record(&StoredRecord {
                summary: super::super::types::RecordSummary {
                    id: id.to_owned(),
                    name: name.to_owned(),
                    state: RecordState::Ended,
                    devices: vec![device("device-a")],
                    active_devices: Vec::new(),
                    has_active_source: false,
                    last_error: None,
                    created_at_ms: 1,
                    updated_at_ms: 2,
                    revision: 0,
                },
                config: config(),
                started_at_ms: Some(1),
                ended_at_ms: Some(3),
            })
            .unwrap();
    }
    store
        .insert_source_run(&RecordSource {
            source_run_id: "run-a".to_owned(),
            record_id: "record".to_owned(),
            source_kind: AudioSourceKind::SystemOutput,
            device_id: "device-a".to_owned(),
            device_name: "Device A".to_owned(),
            state: SourceRunState::Ended,
            elapsed_base_ms: 0,
            elapsed_ms: 2_000,
            started_at_ms: 2,
            ended_at_ms: Some(3),
            error: None,
        })
        .unwrap();
    store.upsert_subtitle(&segment()).unwrap();

    assert!(store.delete_record("record").unwrap());

    assert!(store.load_record("record").unwrap().is_none());
    assert_eq!(store.subtitle_count("record").unwrap(), 0);
    assert!(store.list_source_runs("record").unwrap().is_empty());
    assert!(store.load_record("other").unwrap().is_some());
    assert!(!store.delete_record("record").unwrap());

    drop(store);
    let store = RecordStore::open(path).unwrap();
    assert_eq!(store.list_records().unwrap().len(), 1);
    std::fs::remove_dir_all(directory).unwrap();
}
