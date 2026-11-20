use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::{self, File},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use crate::{
    audio::AudioSourceKind,
    subtitles::{SubtitleSegment, TranslationStatus},
};

use super::{
    manager::RecordManager,
    types::{RecordExportSummary, RecordSource},
};

const MAX_FILE_STEM_CHARS: usize = 80;

#[derive(Debug)]
struct SourceExport {
    source_run_id: String,
    source_kind: AudioSourceKind,
    device_id: String,
    device_name: String,
    started_at_ms: u64,
    segments: Vec<SubtitleSegment>,
}

impl RecordManager {
    pub fn export_record(
        &self,
        record_id: &str,
        destination: &Path,
    ) -> Result<RecordExportSummary, String> {
        if !destination.is_absolute() {
            return Err("The export destination must be an absolute directory.".to_owned());
        }
        let destination = destination
            .canonicalize()
            .map_err(|error| format!("The export destination is invalid: {error}"))?;
        if !destination.is_dir() {
            return Err("The export destination is not a directory.".to_owned());
        }

        let (record_name, sources, save_audio) = {
            let runtime = self.runtime();
            let record = runtime
                .records
                .get(record_id)
                .ok_or_else(|| format!("Unknown record `{record_id}`."))?;
            if record.summary.state.is_active() {
                return Err("Pause or stop the record before exporting.".to_owned());
            }
            (
                record.summary.name.clone(),
                record.sources.clone(),
                record.config.audio.save_audio,
            )
        };

        let segments = self.inner.store.list_export_segments(record_id)?;
        let mut by_source: BTreeMap<String, Vec<SubtitleSegment>> = BTreeMap::new();
        for segment in segments {
            by_source
                .entry(segment.source_run_id.clone())
                .or_default()
                .push(segment);
        }

        let mut source_exports = Vec::new();
        let mut known_sources = HashSet::new();
        for source in &sources {
            let run_segments = by_source.remove(&source.source_run_id).unwrap_or_default();
            let has_audio =
                audio_path(&self.inner.audio_dir, record_id, &source.source_run_id).is_file();
            if run_segments.is_empty() && !has_audio {
                continue;
            }
            known_sources.insert(source.source_run_id.clone());
            source_exports.push(source_export_from_source(source, run_segments));
        }
        for (source_run_id, run_segments) in by_source {
            if known_sources.contains(&source_run_id) {
                continue;
            }
            let Some(first) = run_segments.first() else {
                continue;
            };
            source_exports.push(SourceExport {
                source_run_id,
                source_kind: first.source_kind,
                device_id: first.device_id.clone(),
                device_name: first.device_name.clone(),
                started_at_ms: first.occurrence_ms,
                segments: run_segments,
            });
        }
        source_exports.sort_by(|left, right| {
            left.started_at_ms
                .cmp(&right.started_at_ms)
                .then_with(|| left.source_run_id.cmp(&right.source_run_id))
        });

        if source_exports.is_empty() {
            return Err("There are no finalized captions or audio files to export.".to_owned());
        }

        let export_dir = unique_export_dir(&destination, &record_name, record_id);
        fs::create_dir_all(&export_dir).map_err(|error| {
            format!(
                "Failed to create the export directory {}: {error}",
                export_dir.display()
            )
        })?;

        let mut audio_files = 0usize;
        let mut original_lrc_files = 0usize;
        let mut translation_lrc_files = 0usize;
        let mut missing_audio_runs = 0usize;
        let mut used_stems = HashSet::new();
        let mut device_runs: HashMap<String, usize> = HashMap::new();

        for source in &source_exports {
            let device_key = format!("{}:{}", source.source_kind.as_str(), source.device_id);
            let run_index = device_runs.entry(device_key).or_insert(0);
            *run_index += 1;
            let base_stem = format!("{}-{}", safe_file_stem(&source.device_name), *run_index);
            let stem = unique_stem(base_stem, &source.source_run_id, &mut used_stems);

            let stored_audio = audio_path(&self.inner.audio_dir, record_id, &source.source_run_id);
            if stored_audio.is_file() {
                let exported_audio = export_dir.join(format!("{stem}.wav"));
                fs::copy(&stored_audio, &exported_audio).map_err(|error| {
                    format!(
                        "Failed to copy audio {} to {}: {error}",
                        stored_audio.display(),
                        exported_audio.display()
                    )
                })?;
                audio_files += 1;
            } else if save_audio {
                missing_audio_runs += 1;
            }

            let original_path = export_dir.join(format!("{stem}.original.lrc"));
            if write_original_lrc(&original_path, &source.segments)? {
                original_lrc_files += 1;
            }
            let translation_path = export_dir.join(format!("{stem}.translation.lrc"));
            if write_translation_lrc(&translation_path, &source.segments)? {
                translation_lrc_files += 1;
            }
        }

        if audio_files == 0 && original_lrc_files == 0 && translation_lrc_files == 0 {
            let _ = fs::remove_dir_all(&export_dir);
            return Err("There are no finalized captions or audio files to export.".to_owned());
        }

        Ok(RecordExportSummary {
            directory: export_dir.display().to_string(),
            audio_files,
            original_lrc_files,
            translation_lrc_files,
            audio_disabled: !save_audio,
            missing_audio_runs,
        })
    }
}

fn source_export_from_source(
    source: &RecordSource,
    segments: Vec<SubtitleSegment>,
) -> SourceExport {
    SourceExport {
        source_run_id: source.source_run_id.clone(),
        source_kind: source.source_kind,
        device_id: source.device_id.clone(),
        device_name: source.device_name.clone(),
        started_at_ms: source.started_at_ms,
        segments,
    }
}

fn audio_path(audio_dir: &Path, record_id: &str, source_run_id: &str) -> PathBuf {
    audio_dir
        .join(record_id)
        .join(format!("{source_run_id}.wav"))
}

fn write_original_lrc(path: &Path, segments: &[SubtitleSegment]) -> Result<bool, String> {
    let lines = segments
        .iter()
        .filter_map(|segment| {
            let text = sanitize_lrc_text(&segment.text);
            if text.is_empty() {
                return None;
            }
            Some((segment.start_secs, segment.id.clone(), text))
        })
        .collect();
    write_lrc(path, lines)
}

fn write_translation_lrc(path: &Path, segments: &[SubtitleSegment]) -> Result<bool, String> {
    let lines = segments
        .iter()
        .filter_map(|segment| {
            let translation = segment.translation.as_ref()?;
            if translation.status != TranslationStatus::Ready {
                return None;
            }
            let text = translation.text.as_deref()?;
            let text = sanitize_lrc_text(text);
            if text.is_empty() {
                return None;
            }
            Some((segment.start_secs, segment.id.clone(), text))
        })
        .collect();
    write_lrc(path, lines)
}

fn write_lrc(path: &Path, mut lines: Vec<(f64, String, String)>) -> Result<bool, String> {
    if lines.is_empty() {
        return Ok(false);
    }
    lines.sort_by(|left, right| {
        left.0
            .total_cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
    });
    let file = File::create(path)
        .map_err(|error| format!("Failed to create the LRC file {}: {error}", path.display()))?;
    let mut writer = BufWriter::new(file);
    for (start_secs, _, text) in lines {
        writeln!(writer, "[{}]{}", lrc_timestamp(start_secs), text)
            .map_err(|error| format!("Failed to write the LRC file {}: {error}", path.display()))?;
    }
    writer
        .flush()
        .map_err(|error| format!("Failed to flush the LRC file {}: {error}", path.display()))?;
    Ok(true)
}

fn lrc_timestamp(seconds: f64) -> String {
    let total_centis = if seconds.is_finite() {
        (seconds.max(0.0) * 100.0).round() as u64
    } else {
        0
    };
    let minutes = total_centis / 6_000;
    let seconds = (total_centis / 100) % 60;
    let centis = total_centis % 100;
    format!("{minutes:02}:{seconds:02}.{centis:02}")
}

fn sanitize_lrc_text(text: &str) -> String {
    text.replace(['\r', '\n'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn unique_export_dir(destination: &Path, record_name: &str, record_id: &str) -> PathBuf {
    let base = format!("{}-{}", safe_file_stem(record_name), short_id(record_id));
    let mut candidate = destination.join(&base);
    let mut index = 2;
    while candidate.exists() {
        candidate = destination.join(format!("{base}-{index}"));
        index += 1;
    }
    candidate
}

fn unique_stem(base: String, source_run_id: &str, used: &mut HashSet<String>) -> String {
    if used.insert(base.clone()) {
        return base;
    }
    let with_id = format!("{base}-{}", short_id(source_run_id));
    if used.insert(with_id.clone()) {
        return with_id;
    }
    let mut index = 2;
    loop {
        let candidate = format!("{base}-{index}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
        index += 1;
    }
}

fn safe_file_stem(value: &str) -> String {
    let mut stem = String::new();
    for character in value.chars() {
        if character.is_alphanumeric() || matches!(character, ' ' | '-' | '_' | '.') {
            stem.push(character);
        } else {
            stem.push('-');
        }
    }
    let stem = stem
        .trim_matches(|character: char| character == ' ' || character == '-' || character == '.')
        .chars()
        .take(MAX_FILE_STEM_CHARS)
        .collect::<String>();
    if stem.is_empty() {
        "audio".to_owned()
    } else {
        stem
    }
}

fn short_id(value: &str) -> String {
    value.chars().take(8).collect()
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use crate::subtitles::TranslationState;

    fn segment(
        id: &str,
        start_secs: f64,
        text: &str,
        translation: Option<&str>,
    ) -> SubtitleSegment {
        SubtitleSegment {
            id: id.to_owned(),
            session_id: "run".to_owned(),
            record_id: "record".to_owned(),
            source_run_id: "run".to_owned(),
            source_kind: AudioSourceKind::SystemOutput,
            device_id: "device".to_owned(),
            device_name: "Device".to_owned(),
            local_id: 1,
            start_secs,
            end_secs: Some(start_secs + 1.0),
            device_time_secs: start_secs,
            occurrence_ms: (start_secs * 1_000.0) as u64,
            text: text.to_owned(),
            revision: 1,
            is_final: true,
            speaker: None,
            speaker_revision: 0,
            translation: translation.map(|text| TranslationState {
                target_language: "zh-Hans".to_owned(),
                text: Some(text.to_owned()),
                status: TranslationStatus::Ready,
                revision: 1,
                engine: "bergamot".to_owned(),
                mode: "text".to_owned(),
                model_id: "bergamot-en-zh-Hans".to_owned(),
                error: None,
                updated_at_ms: 1,
            }),
            source_language: Some("en".to_owned()),
            created_at_ms: 1,
            updated_at_ms: 1,
            last_change: Instant::now(),
        }
    }

    #[test]
    fn formats_lrc_timestamps() {
        assert_eq!(lrc_timestamp(0.0), "00:00.00");
        assert_eq!(lrc_timestamp(61.239), "01:01.24");
        assert_eq!(lrc_timestamp(-2.0), "00:00.00");
        assert_eq!(lrc_timestamp(f64::NAN), "00:00.00");
    }

    #[test]
    fn writes_separate_original_and_translation_lrc_files() {
        let directory =
            std::env::temp_dir().join(format!("sonasub-export-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&directory).unwrap();
        let original_path = directory.join("run.original.lrc");
        let translation_path = directory.join("run.translation.lrc");
        let segments = vec![
            segment("run:1", 1.0, "hello\nworld", Some("你好")),
            segment("run:2", 2.5, "second", None),
        ];

        assert!(write_original_lrc(&original_path, &segments).unwrap());
        assert!(write_translation_lrc(&translation_path, &segments).unwrap());

        assert_eq!(
            fs::read_to_string(&original_path).unwrap(),
            "[00:01.00]hello world\n[00:02.50]second\n"
        );
        assert_eq!(
            fs::read_to_string(&translation_path).unwrap(),
            "[00:01.00]你好\n"
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn skips_translation_file_when_no_ready_translation_exists() {
        let directory =
            std::env::temp_dir().join(format!("sonasub-export-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("run.translation.lrc");
        let segments = vec![segment("run:1", 1.0, "hello", None)];

        assert!(!write_translation_lrc(&path, &segments).unwrap());
        assert!(!path.exists());
        fs::remove_dir_all(directory).unwrap();
    }
}
