use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{command, State};

use crate::{
    audio::SystemOutput,
    config::{AsrMode, ConfigStore, TranslationMode},
};

const SHERPA_RELEASE: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download";
const FIREFOX_ATTACHMENTS: &str =
    "https://firefox-settings-attachments.cdn.mozilla.net/main-workspace/translations-models-v2";

static DOWNLOAD_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

pub const ASR_STREAMING_ID: &str = "zipformer-bilingual-streaming";
pub const ASR_SEGMENTED_ID: &str = "sense-voice-int8";
pub const BERGAMOT_ENGINE_ID: &str = "bergamot";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AsrModelKind {
    Streaming,
    Segmented,
}

impl AsrModelKind {
    pub fn mode(self) -> AsrMode {
        match self {
            Self::Streaming => AsrMode::Streaming,
            Self::Segmented => AsrMode::Segmented,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModelFile {
    pub relative: &'static str,
    pub min_bytes: u64,
    pub sha256: Option<&'static str>,
}

#[derive(Debug, Clone)]
pub struct AsrModelSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: AsrModelKind,
    pub source_languages: &'static [&'static str],
    /// Whether a forced source language changes the recognition result.
    pub supports_source_language: bool,
    /// Why a forced source language is not available for this model.
    pub source_language_note: Option<&'static str>,
    pub upstream: &'static str,
    pub version: &'static str,
    pub files: &'static [ModelFile],
}

#[derive(Debug, Clone)]
pub struct TranslationModelSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub source_language: &'static str,
    pub target_language: &'static str,
    pub upstream: &'static str,
    pub version: &'static str,
    pub model: ModelFile,
    pub source_vocab: ModelFile,
    pub target_vocab: Option<ModelFile>,
    pub shortlist: Option<ModelFile>,
}

pub const ASR_MODELS: &[AsrModelSpec] = &[
    AsrModelSpec {
        id: ASR_STREAMING_ID,
        label: "Zipformer bilingual zh-en (streaming)",
        kind: AsrModelKind::Streaming,
        source_languages: &["zh", "en"],
        supports_source_language: false,
        source_language_note: Some(
            "The bilingual streaming model detects the language of every utterance. A forced source language would not change the result.",
        ),
        upstream: "https://github.com/k2-fsa/sherpa-onnx/releases/tag/asr-models",
        version: "sherpa-onnx-streaming-zipformer-bilingual-zh-en-2023-02-20",
        files: &[
            ModelFile {
                relative: "sherpa/streaming-zipformer-bilingual-zh-en/encoder.onnx",
                min_bytes: 1_000_000,
                sha256: Some("8fa764187a261844f859d7143ebaa563af5d10adfece4c18a8f414c88cba2a9b"),
            },
            ModelFile {
                relative: "sherpa/streaming-zipformer-bilingual-zh-en/decoder.onnx",
                min_bytes: 100_000,
                sha256: Some("1a70c593d71e53f023f5f55b0b4cfff5055abb786ee3992e5f63dc2e273cc4fa"),
            },
            ModelFile {
                relative: "sherpa/streaming-zipformer-bilingual-zh-en/joiner.onnx",
                min_bytes: 100_000,
                sha256: Some("1ed689c5ed19dbaa725d9d191bb4822b5f4855a39e1ffd28cbc1f340d25b2ee0"),
            },
            ModelFile {
                relative: "sherpa/streaming-zipformer-bilingual-zh-en/tokens.txt",
                min_bytes: 10_000,
                sha256: Some("a8e0e4ec53810e433789b54a5c0134a7eaa2ffca595a6334d54c00da858841d3"),
            },
        ],
    },
    AsrModelSpec {
        id: ASR_SEGMENTED_ID,
        label: "SenseVoice multilingual (segmented)",
        kind: AsrModelKind::Segmented,
        source_languages: &["zh", "en", "ja", "ko", "yue"],
        supports_source_language: true,
        source_language_note: None,
        upstream: "https://github.com/k2-fsa/sherpa-onnx/releases/tag/asr-models",
        version: "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17",
        files: &[
            ModelFile {
                relative: "sherpa/sense-voice-int8/model.onnx",
                min_bytes: 100_000_000,
                sha256: Some("c71f0ce00bec95b07744e116345e33d8cbbe08cef896382cf907bf4b51a2cd51"),
            },
            ModelFile {
                relative: "sherpa/sense-voice-int8/tokens.txt",
                min_bytes: 100_000,
                sha256: Some("f449eb28dc567533d7fa59be34e2abca8784f771850c78a47fb731a31429a1dc"),
            },
        ],
    },
];

pub const TRANSLATION_MODELS: &[TranslationModelSpec] = &[
    TranslationModelSpec {
        id: "bergamot-en-zh-Hans",
        label: "Bergamot English to Simplified Chinese",
        source_language: "en",
        target_language: "zh-Hans",
        upstream: "https://firefox.settings.services.mozilla.com/v1/buckets/main/collections/translations-models-v2/records",
        version: "Bergamot v3.0",
        model: ModelFile {
            relative: "bergamot/en-zh-Hans/model.bin",
            min_bytes: 10_000_000,
            sha256: Some("4e5accc141373565ddc8fa1565bceaa8d0c3482a82cab8131c719ebcc6c2157c"),
        },
        source_vocab: ModelFile {
            relative: "bergamot/en-zh-Hans/src.spm",
            min_bytes: 100_000,
            sha256: Some("bd9b65504acc6d9726dd281f7defc2adb7c2c22d0688fe2f84697de25197c8c5"),
        },
        target_vocab: Some(ModelFile {
            relative: "bergamot/en-zh-Hans/trg.spm",
            min_bytes: 100_000,
            sha256: Some("aded6993c36e440284d11cec3f6b8aef9c0e43188a772d80be342a713adf223d"),
        }),
        shortlist: Some(ModelFile {
            relative: "bergamot/en-zh-Hans/lex.bin",
            min_bytes: 100_000,
            sha256: Some("4a5e5827788060f1d718a8132b69440929387514a045796e9b77f935db68c055"),
        }),
    },
    TranslationModelSpec {
        id: "bergamot-zh-Hans-en",
        label: "Bergamot Simplified Chinese to English",
        source_language: "zh-Hans",
        target_language: "en",
        upstream: "https://firefox.settings.services.mozilla.com/v1/buckets/main/collections/translations-models-v2/records",
        version: "Bergamot v3.1",
        model: ModelFile {
            relative: "bergamot/zh-Hans-en/model.bin",
            min_bytes: 10_000_000,
            sha256: Some("5cd149601802fc8a18124a1c1306144dbbedc058630c4ddb2d53aa76fa9c7c06"),
        },
        source_vocab: ModelFile {
            relative: "bergamot/zh-Hans-en/vocab.spm",
            min_bytes: 100_000,
            sha256: Some("dff594318ab7d8b7b60b844ab98ebe6b932ae8045fab15235404c787715965b3"),
        },
        target_vocab: None,
        shortlist: Some(ModelFile {
            relative: "bergamot/zh-Hans-en/lex.bin",
            min_bytes: 100_000,
            sha256: Some("8524dd4c93ffd7f0ae7be32f77a7c14fea7cdbbebe34ea6ad0ec368d5ddd8b13"),
        }),
    },
];

pub const SUPPORTED_LANGUAGES: &[(&str, &str)] =
    &[("zh-Hans", "Simplified Chinese"), ("en", "English")];

pub const LANGUAGE_LABEL_ZH: &str = "简体中文";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    pub id: String,
    pub label: String,
    pub available: bool,
    pub missing_files: Vec<String>,
    pub upstream: String,
    pub version: String,
    pub files: Vec<ModelFileStatus>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelFileStatus {
    pub path: String,
    pub present: bool,
    pub bytes: u64,
    pub expected_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInstallFile {
    pub relative: String,
    pub expected_sha256: Option<String>,
    pub min_bytes: u64,
    pub source_name: String,
    pub present: bool,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInstallGroup {
    pub id: String,
    pub label: String,
    pub version: String,
    pub category: String,
    pub upstream: String,
    pub download_kind: String,
    pub download_url: String,
    pub download_bytes: u64,
    pub download_urls: Vec<String>,
    pub download_bytes_each: Vec<u64>,
    pub files: Vec<ModelInstallFile>,
}

fn sherpa_source_name(file: &ModelFile) -> &str {
    Path::new(file.relative)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(file.relative)
}

fn sherpa_category(id: &str) -> &'static str {
    match id {
        ASR_STREAMING_ID | ASR_SEGMENTED_ID => "recognition",
        _ => "supporting",
    }
}

fn sherpa_group(
    root: &Path,
    id: &'static str,
    label: &'static str,
    version: &'static str,
    url: String,
    download_bytes: u64,
    files: &'static [ModelFile],
) -> ModelInstallGroup {
    ModelInstallGroup {
        id: id.to_owned(),
        label: label.to_owned(),
        version: version.to_owned(),
        category: sherpa_category(id).to_owned(),
        upstream: "https://github.com/k2-fsa/sherpa-onnx/releases".to_owned(),
        download_kind: if url.ends_with(".tar.bz2") {
            "tarBz2".to_owned()
        } else {
            "file".to_owned()
        },
        download_url: url,
        download_bytes,
        download_urls: Vec::new(),
        download_bytes_each: Vec::new(),
        files: files
            .iter()
            .map(|file| install_file_from_spec(root, file, sherpa_source_name(file)))
            .collect(),
    }
}

fn bergamot_group(root: &Path, index: usize) -> ModelInstallGroup {
    let spec = &TRANSLATION_MODELS[index];
    let files = translation_files(spec);
    let sources: &[&str] = match index {
        0 => &[
            "model.enzh.intgemm.alphas.bin.zst",
            "srcvocab.enzh.spm.zst",
            "trgvocab.enzh.spm.zst",
            "lex.50.50.enzh.s2t.bin.zst",
        ],
        _ => &[
            "model.zhen.intgemm.alphas.bin.zst",
            "vocab.zhen.spm.zst",
            "lex.50.50.zhen.s2t.bin.zst",
        ],
    };
    let attachments: &[&str] = match index {
        0 => &[
            "321191b6-2c42-4bca-84ec-b7b074b67bd6.zst",
            "83278466-fec4-4944-a99d-c2372fa9de88.zst",
            "84236654-1b66-42fc-9ba2-74c37f27e5eb.zst",
            "900beaed-2dba-4a88-ad7c-852ec9e73f6a.zst",
        ],
        _ => &[
            "8bb74789-fad9-40e0-8f49-d3e017106138.zst",
            "369efa59-d98e-4aa9-8a91-363ba16c3ba7.zst",
            "a2261cbe-7fe7-498d-a128-c08d918b42fc.zst",
        ],
    };
    let attachment_bytes: &[u64] = match index {
        0 => &[32_923_790, 348_078, 354_994, 2_499_275],
        _ => &[32_186_434, 611_595, 3_427_789],
    };
    let install_files = files
        .iter()
        .zip(sources.iter().copied())
        .map(|(file, source_name)| install_file_from_spec(root, file, source_name))
        .collect();
    let download_urls = attachments
        .iter()
        .map(|attachment| format!("{FIREFOX_ATTACHMENTS}/{attachment}"))
        .collect::<Vec<_>>();
    ModelInstallGroup {
        id: spec.id.to_owned(),
        label: spec.label.to_owned(),
        version: spec.version.to_owned(),
        category: "translation".to_owned(),
        upstream: spec.upstream.to_owned(),
        download_kind: "bergamot".to_owned(),
        download_url: download_urls[0].clone(),
        download_bytes: attachment_bytes.iter().sum(),
        download_urls,
        download_bytes_each: attachment_bytes.to_vec(),
        files: install_files,
    }
}

fn group_expected_sources(group: &ModelInstallGroup) -> &[(&str, &str)] {
    match group.id.as_str() {
        "zipformer-bilingual-streaming" => &[
            (
                "sherpa/streaming-zipformer-bilingual-zh-en/encoder.onnx",
                "encoder-epoch-99-avg-1.int8.onnx",
            ),
            (
                "sherpa/streaming-zipformer-bilingual-zh-en/decoder.onnx",
                "decoder-epoch-99-avg-1.int8.onnx",
            ),
            (
                "sherpa/streaming-zipformer-bilingual-zh-en/joiner.onnx",
                "joiner-epoch-99-avg-1.int8.onnx",
            ),
            (
                "sherpa/streaming-zipformer-bilingual-zh-en/tokens.txt",
                "tokens.txt",
            ),
        ],
        "sense-voice-int8" => &[
            ("sherpa/sense-voice-int8/model.onnx", "model.int8.onnx"),
            ("sherpa/sense-voice-int8/tokens.txt", "tokens.txt"),
        ],
        "pyannote-segmentation-3.0" => &[(
            "sherpa/diarization/pyannote-segmentation-3.0.onnx",
            "model.int8.onnx",
        )],
        _ => &[],
    }
}

fn download_dir(state: &crate::session::SessionManager) -> Result<PathBuf, String> {
    let configured = DOWNLOAD_DIR
        .lock()
        .map_err(|error| format!("Failed to read the download directory: {error}"))?
        .clone();
    if let Some(directory) = configured {
        return Ok(directory);
    }
    let config_dir = state.config_dir();
    let store = ConfigStore::new(config_dir.join("model-download-dir.json"));
    let loaded = store
        .load_raw()
        .and_then(|text| serde_json::from_str::<PathBuf>(&text).ok())
        .filter(|path| path.is_absolute());
    Ok(loaded.unwrap_or_else(|| state.model_root().join("downloads")))
}

fn download_destination(
    state: &crate::session::SessionManager,
    group_id: &str,
) -> Result<PathBuf, String> {
    Ok(download_dir(state)?.join(format!("{group_id}.download")))
}

fn pinned(file: &ModelInstallFile) -> Result<(&str, u64), String> {
    let sha256 = file
        .expected_sha256
        .as_deref()
        .ok_or_else(|| format!("{} has no pinned checksum.", file.relative))?;
    Ok((sha256, file.min_bytes))
}

fn find_source(directory: &Path, source_name: &str) -> Result<PathBuf, String> {
    if let Some(found) = find_nested_file(directory, source_name, 3) {
        return Ok(found);
    }
    Err(format!(
        "{source_name} was not found in {}",
        directory.display()
    ))
}

fn find_nested_file(directory: &Path, name: &str, depth: usize) -> Option<PathBuf> {
    let direct = directory.join(name);
    if direct.is_file() {
        return Some(direct);
    }
    if depth == 0 {
        return None;
    }
    for entry in fs::read_dir(directory).ok()?.flatten() {
        if entry.file_type().is_ok_and(|file_type| file_type.is_dir()) {
            if let Some(found) = find_nested_file(&entry.path(), name, depth - 1) {
                return Some(found);
            }
        }
    }
    None
}

fn install_sources(
    state: &crate::session::SessionManager,
    group: &ModelInstallGroup,
    directory: &Path,
) -> Result<(), String> {
    let expected = group_expected_sources(group);
    for file in &group.files {
        let (sha256, min_bytes) = pinned(file)?;
        let source_name = expected
            .iter()
            .find(|(relative, _)| *relative == file.relative)
            .map(|(_, name)| *name)
            .unwrap_or(file.source_name.as_str());
        let source = find_source(directory, source_name)?;
        let target = state.model_root().join(&file.relative);
        copy_verified(&source, &target, sha256, min_bytes)?;
    }
    Ok(())
}

fn extract_tar_bz2(archive: &Path, target: &Path) -> Result<(), String> {
    fs::create_dir_all(target)
        .map_err(|error| format!("failed to create {}: {error}", target.display()))?;
    let file = File::open(archive)
        .map_err(|error| format!("failed to read {}: {error}", archive.display()))?;
    let decoder = bzip2::read::BzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    archive
        .unpack(target)
        .map_err(|error| format!("failed to extract {}: {error}", target.display()))?;
    Ok(())
}

fn install_archive(
    state: &crate::session::SessionManager,
    group: &ModelInstallGroup,
    archive: &Path,
) -> Result<(), String> {
    if group.download_kind == "file" {
        return install_direct_file(&state.model_root(), group, archive);
    }
    let temporary = archive.with_extension("extracted");
    let _ = fs::remove_dir_all(&temporary);
    let result = if group.download_kind == "tarBz2" {
        extract_tar_bz2(archive, &temporary)
            .and_then(|()| install_sources(state, group, &temporary))
    } else {
        Err(format!(
            "Unsupported archive type `{}`.",
            group.download_kind
        ))
    };
    let _ = fs::remove_dir_all(&temporary);
    result
}

fn install_direct_file(
    root: &Path,
    group: &ModelInstallGroup,
    source: &Path,
) -> Result<(), String> {
    let [file] = group.files.as_slice() else {
        return Err(format!(
            "The `{}` group must contain exactly one file for a direct download.",
            group.id
        ));
    };
    let (sha256, min_bytes) = pinned(file)?;
    copy_verified(source, &root.join(&file.relative), sha256, min_bytes)
}

fn download_file(url: &str, destination: &Path, expected_bytes: u64) -> Result<(), String> {
    if let Ok(metadata) = fs::metadata(destination) {
        if metadata.is_file() && metadata.len() == expected_bytes {
            return Ok(());
        }
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let temporary = destination.with_extension("part");
    let result: Result<(), String> = (|| {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(15))
            .timeout_read(Duration::from_secs(30))
            .timeout_write(Duration::from_secs(30))
            .build();
        let response = agent
            .get(url)
            .call()
            .map_err(|error| format!("failed to download {}: {error}", redact_url(url)))?;
        let mut reader = response.into_reader();
        let mut output = File::create(&temporary)
            .map_err(|error| format!("failed to create {}: {error}", temporary.display()))?;
        let mut buffer = [0u8; 128 * 1024];
        loop {
            let read = reader
                .read(&mut buffer)
                .map_err(|error| format!("failed to read {}: {error}", redact_url(url)))?;
            if read == 0 {
                break;
            }
            output
                .write_all(&buffer[..read])
                .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;
        }
        output
            .sync_all()
            .map_err(|error| format!("failed to flush {}: {error}", temporary.display()))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result?;
    let actual = fs::metadata(&temporary)
        .map_err(|error| format!("failed to inspect {}: {error}", temporary.display()))?
        .len();
    if actual != expected_bytes {
        let _ = fs::remove_file(&temporary);
        return Err(format!(
            "downloaded {} bytes from {}; expected {expected_bytes}",
            actual,
            redact_url(url)
        ));
    }
    fs::rename(&temporary, destination)
        .map_err(|error| format!("failed to store {}: {error}", destination.display()))?;
    Ok(())
}

fn redact_url(url: &str) -> &str {
    url.rsplit_once('/').map(|(_, name)| name).unwrap_or(url)
}

fn zstd_to_memory(path: &Path) -> Result<Vec<u8>, String> {
    let data =
        fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    zstd::decode_all(data.as_slice())
        .map_err(|error| format!("failed to decompress {}: {error}", path.display()))
}

fn download_group(
    state: &crate::session::SessionManager,
    group: &ModelInstallGroup,
) -> Result<(), String> {
    let destination = download_destination(state, &group.id)?;
    fs::create_dir_all(&destination)
        .map_err(|error| format!("failed to create {}: {error}", destination.display()))?;

    if group.download_kind == "bergamot" {
        for (index, file) in group.files.iter().enumerate() {
            let (sha256, min_bytes) = pinned(file)?;
            let installed = state.model_root().join(&file.relative);
            if let Ok(bytes) = fs::read(&installed) {
                let actual = format!("{:x}", Sha256::digest(&bytes));
                if (bytes.len() as u64) >= min_bytes && actual.eq_ignore_ascii_case(sha256) {
                    continue;
                }
            }
            let url = group
                .download_urls
                .get(index)
                .ok_or_else(|| format!("No download URL for {}.", file.relative))?;
            let expected_bytes = group
                .download_bytes_each
                .get(index)
                .copied()
                .ok_or_else(|| format!("No download size for {}.", file.relative))?;
            let archive = destination.join(&file.source_name);
            download_file(url, &archive, expected_bytes)?;
            let bytes = zstd_to_memory(&archive)?;
            if (bytes.len() as u64) < min_bytes {
                return Err(format!(
                    "{} is {} bytes; at least {min_bytes} bytes are required.",
                    file.relative,
                    bytes.len()
                ));
            }
            let actual = format!("{:x}", Sha256::digest(&bytes));
            if !actual.eq_ignore_ascii_case(sha256) {
                return Err(format!(
                    "checksum mismatch for {}: expected {sha256}, got {actual}.",
                    file.relative
                ));
            }
            let target = state.model_root().join(&file.relative);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
            }
            let temporary = target.with_extension(format!("download-{}-tmp", std::process::id()));
            fs::write(&temporary, &bytes)
                .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;
            fs::rename(&temporary, &target)
                .map_err(|error| format!("failed to install {}: {error}", target.display()))?;
        }
        return Ok(());
    }

    let archive = destination.join(format!("{}.{}", group.id, group.download_kind));
    download_file(&group.download_url, &archive, group.download_bytes)?;
    install_archive(state, group, &archive)
}

fn import_group(
    state: &crate::session::SessionManager,
    group: &ModelInstallGroup,
    sources: &[String],
) -> Result<(), String> {
    let mut archives = Vec::new();
    let mut files = Vec::new();
    let mut installed = 0usize;
    for source in sources {
        let path = PathBuf::from(source);
        if !path.is_file() {
            return Err(format!("{} is not a file.", path.display()));
        }
        if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension == "bz2")
        {
            archives.push(path);
        } else {
            files.push(path);
        }
    }
    if !archives.is_empty() {
        for archive in archives {
            install_archive(state, group, &archive)?;
            installed += 1;
        }
    }
    for source in files {
        let name = source
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("{} has no file name.", source.display()))?;
        let Some(file) = group
            .files
            .iter()
            .find(|file| is_import_name(group, file, name))
        else {
            continue;
        };
        let (sha256, min_bytes) = pinned(file)?;
        let bytes = if group.download_kind == "bergamot" {
            zstd_to_memory(&source)?
        } else {
            fs::read(&source)
                .map_err(|error| format!("failed to read {}: {error}", source.display()))?
        };
        if (bytes.len() as u64) < min_bytes {
            return Err(format!(
                "{} is {} bytes; at least {min_bytes} bytes are required.",
                file.relative,
                bytes.len()
            ));
        }
        let actual = format!("{:x}", Sha256::digest(&bytes));
        if !actual.eq_ignore_ascii_case(sha256) {
            return Err(format!(
                "checksum mismatch for {}: expected {sha256}, got {actual}.",
                file.relative
            ));
        }
        let target = state.model_root().join(&file.relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
        let temporary = target.with_extension(format!("import-{}-tmp", std::process::id()));
        fs::write(&temporary, &bytes)
            .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;
        fs::rename(&temporary, &target)
            .map_err(|error| format!("failed to install {}: {error}", target.display()))?;
        installed += 1;
    }
    if installed == 0 {
        return Err(
            "None of the selected files match this model. Select its official archive or the runtime files listed on its card."
                .to_owned(),
        );
    }
    Ok(())
}

fn is_import_name(group: &ModelInstallGroup, file: &ModelInstallFile, name: &str) -> bool {
    if group.download_kind == "bergamot" {
        return file.source_name == name
            || Path::new(&file.relative)
                .file_name()
                .and_then(|file_name| file_name.to_str())
                == Some(name);
    }
    group_expected_sources(group)
        .iter()
        .find(|(relative, _)| *relative == file.relative)
        .is_some_and(|(_, source_name)| *source_name == name)
        || file.source_name == name
}

fn copy_verified(
    source: &Path,
    target: &Path,
    expected_sha256: &str,
    min_bytes: u64,
) -> Result<(), String> {
    let mut input = File::open(source)
        .map_err(|error| format!("failed to read {}: {error}", source.display()))?;
    let metadata = input
        .metadata()
        .map_err(|error| format!("failed to inspect {}: {error}", source.display()))?;
    if metadata.len() < min_bytes {
        return Err(format!(
            "{} is {} bytes; at least {min_bytes} bytes are required.",
            source.display(),
            metadata.len()
        ));
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let temporary = target.with_extension(format!("install-{}-tmp", std::process::id()));
    let mut hasher = Sha256::new();
    {
        let mut output = File::create(&temporary)
            .map_err(|error| format!("failed to create {}: {error}", temporary.display()))?;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let read = input
                .read(&mut buffer)
                .map_err(|error| format!("failed to read {}: {error}", source.display()))?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            output
                .write_all(&buffer[..read])
                .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;
        }
        output
            .sync_all()
            .map_err(|error| format!("failed to flush {}: {error}", temporary.display()))?;
    }
    let actual = format!("{:x}", hasher.finalize());
    if !actual.eq_ignore_ascii_case(expected_sha256) {
        let _ = fs::remove_file(&temporary);
        return Err(format!(
            "checksum mismatch for {}: expected {expected_sha256}, got {actual}.",
            source.display()
        ));
    }
    fs::rename(&temporary, target)
        .map_err(|error| format!("failed to install {}: {error}", target.display()))?;
    Ok(())
}

fn install_file_from_spec(root: &Path, file: &ModelFile, source_name: &str) -> ModelInstallFile {
    let path = root.join(file.relative);
    let metadata = fs::metadata(&path).ok();
    let present = metadata
        .as_ref()
        .is_some_and(|metadata| metadata.is_file() && metadata.len() >= file.min_bytes);
    ModelInstallFile {
        relative: file.relative.to_owned(),
        expected_sha256: file.sha256.map(ToOwned::to_owned),
        min_bytes: file.min_bytes,
        source_name: source_name.to_owned(),
        present,
        bytes: metadata.map(|metadata| metadata.len()).unwrap_or(0),
    }
}

fn model_install_groups(root: &Path) -> Vec<ModelInstallGroup> {
    vec![
        sherpa_group(
            root,
            ASR_STREAMING_ID,
            "Zipformer bilingual zh-en (streaming)",
            "sherpa-onnx-streaming-zipformer-bilingual-zh-en-2023-02-20",
            format!("{SHERPA_RELEASE}/asr-models/sherpa-onnx-streaming-zipformer-bilingual-zh-en-2023-02-20.tar.bz2"),
            511_274_346,
            ASR_MODELS[0].files,
        ),
        sherpa_group(
            root,
            ASR_SEGMENTED_ID,
            "SenseVoice multilingual (segmented)",
            "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17",
            format!("{SHERPA_RELEASE}/asr-models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17.tar.bz2"),
            163_002_883,
            ASR_MODELS[1].files,
        ),
        bergamot_group(root, 0),
        bergamot_group(root, 1),
        sherpa_group(
            root,
            "silero-vad",
            "Silero VAD",
            "silero_vad.onnx",
            format!("{SHERPA_RELEASE}/asr-models/silero_vad.onnx"),
            643_854,
            VAD_FILES,
        ),
        sherpa_group(
            root,
            "campplus-zh-en",
            "3D-Speaker CAM++ zh-en",
            "3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx",
            format!("{SHERPA_RELEASE}/speaker-recongition-models/3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx"),
            28_281_164,
            SPEAKER_FILES,
        ),
        sherpa_group(
            root,
            "pyannote-segmentation-3.0",
            "Pyannote segmentation 3.0",
            "sherpa-onnx-pyannote-segmentation-3-0",
            format!("{SHERPA_RELEASE}/speaker-segmentation-models/sherpa-onnx-pyannote-segmentation-3-0.tar.bz2"),
            6_958_444,
            DIARIZATION_FILES,
        ),
    ]
}

#[command]
pub fn list_model_installs(
    state: State<'_, crate::session::SessionManager>,
) -> Vec<ModelInstallGroup> {
    state.model_paths().install_groups()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDownloadSettings {
    pub directory: String,
    pub default_directory: String,
    pub models_dir: String,
}

#[command]
pub fn get_model_download_settings(
    state: State<'_, crate::session::SessionManager>,
) -> Result<ModelDownloadSettings, String> {
    Ok(ModelDownloadSettings {
        directory: download_dir(&state)?.display().to_string(),
        default_directory: state.model_root().join("downloads").display().to_string(),
        models_dir: state.model_root().display().to_string(),
    })
}

#[command]
pub fn set_model_download_directory(
    state: State<'_, crate::session::SessionManager>,
    directory: String,
) -> Result<ModelDownloadSettings, String> {
    let path = PathBuf::from(directory);
    if !path.is_absolute() {
        return Err("The download directory must be an absolute path.".to_owned());
    }
    fs::create_dir_all(&path)
        .map_err(|error| format!("failed to create {}: {error}", path.display()))?;
    let store = ConfigStore::new(state.config_dir().join("model-download-dir.json"));
    let encoded = serde_json::to_string(&path)
        .map_err(|error| format!("failed to serialize the download directory: {error}"))?;
    store.save_raw(&encoded)?;
    if let Ok(mut current) = DOWNLOAD_DIR.lock() {
        *current = Some(path.clone());
    }
    Ok(ModelDownloadSettings {
        directory: path.display().to_string(),
        default_directory: state.model_root().join("downloads").display().to_string(),
        models_dir: state.model_root().display().to_string(),
    })
}

#[command]
pub async fn download_model(
    state: State<'_, crate::session::SessionManager>,
    group_id: String,
) -> Result<(), String> {
    let manager = state.inner().clone();
    let group = state
        .model_paths()
        .install_groups()
        .into_iter()
        .find(|group| group.id == group_id)
        .ok_or_else(|| format!("Unknown model group `{group_id}`."))?;
    tauri::async_runtime::spawn_blocking(move || download_group(&manager, &group))
        .await
        .map_err(|error| format!("Model download task failed: {error}"))?
}

#[command]
pub async fn import_model_files(
    state: State<'_, crate::session::SessionManager>,
    group_id: String,
    sources: Vec<String>,
) -> Result<(), String> {
    let manager = state.inner().clone();
    let group = state
        .model_paths()
        .install_groups()
        .into_iter()
        .find(|group| group.id == group_id)
        .ok_or_else(|| format!("Unknown model group `{group_id}`."))?;
    tauri::async_runtime::spawn_blocking(move || import_group(&manager, &group, &sources))
        .await
        .map_err(|error| format!("Model import task failed: {error}"))?
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrModelCapability {
    #[serde(flatten)]
    pub status: ModelStatus,
    pub kind: String,
    #[serde(rename = "supportedModes")]
    pub supported_modes: Vec<String>,
    pub source_languages: Vec<String>,
    /// Whether a forced source language changes the recognition result.
    pub supports_source_language: bool,
    /// Why a forced source language is not available for this model.
    pub source_language_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationModelCapability {
    #[serde(flatten)]
    pub status: ModelStatus,
    pub source_language: String,
    pub target_language: String,
    pub engine: String,
    pub supported_modes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageOption {
    pub code: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioCapabilities {
    pub backend: String,
    pub system_capture: bool,
    pub supported_platform: String,
    pub permission: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub default_device: Option<String>,
    pub devices: Vec<SystemOutput>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub models_dir: String,
    pub platform: String,
    pub audio: AudioCapabilities,
    pub asr_models: Vec<AsrModelCapability>,
    pub translation_models: Vec<TranslationModelCapability>,
    pub translation_targets: Vec<LanguageOption>,
    pub translation_engine: String,
    /// The native runtime that executes the translation models.
    pub translation_runtime: String,
    /// The model family the compiled adapter reads.
    pub translation_model_family: String,
    pub translation_modes: Vec<ModeCapability>,
    pub diarization: ModelStatus,
    pub speaker_embedding: ModelStatus,
    pub vad: ModelStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeCapability {
    pub mode: String,
    pub available: bool,
    pub reason: Option<String>,
    /// A stable identifier for the reason, so the interface can show a
    /// localized explanation instead of the raw backend text.
    pub reason_code: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ModelPaths {
    pub root: PathBuf,
}

impl ModelPaths {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    pub fn asr(&self, id: &str) -> Result<&'static AsrModelSpec, String> {
        ASR_MODELS
            .iter()
            .find(|model| model.id == id)
            .ok_or_else(|| format!("Unknown ASR model id `{id}`."))
    }

    pub fn translation(
        &self,
        source: &str,
        target: &str,
    ) -> Result<&'static TranslationModelSpec, String> {
        TRANSLATION_MODELS
            .iter()
            .find(|model| {
                model.source_language == source && model.target_language == target
            })
            .ok_or_else(|| {
                format!(
                    "No local Bergamot model for {source} to {target}. Available directions: en to zh-Hans, zh-Hans to en."
                )
            })
    }

    pub fn translation_for_target(
        &self,
        target: &str,
    ) -> Result<&'static TranslationModelSpec, String> {
        TRANSLATION_MODELS
            .iter()
            .find(|model| model.target_language == target)
            .ok_or_else(|| {
                format!(
                    "No local Bergamot model targets `{target}`. Available targets: zh-Hans, en."
                )
            })
    }

    pub fn missing(&self, files: &[ModelFile]) -> Vec<String> {
        files
            .iter()
            .filter(|file| {
                let path = self.path(file.relative);
                match fs::metadata(&path) {
                    Ok(metadata) => !metadata.is_file() || metadata.len() < file.min_bytes,
                    Err(_) => true,
                }
            })
            .map(|file| file.relative.to_owned())
            .collect()
    }

    pub fn verify_file(&self, file: &ModelFile) -> Result<String, String> {
        let path = self.path(file.relative);
        let bytes = fs::read(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        Ok(format!("{:x}", Sha256::digest(bytes)))
    }

    pub fn model_status(
        &self,
        id: &str,
        label: &str,
        upstream: &str,
        version: &str,
        files: &[ModelFile],
    ) -> ModelStatus {
        let mut statuses = Vec::new();
        let mut missing = Vec::new();
        for file in files {
            let path = self.path(file.relative);
            let metadata = fs::metadata(&path).ok();
            let present = metadata
                .as_ref()
                .map(|metadata| metadata.is_file() && metadata.len() >= file.min_bytes)
                .unwrap_or(false);
            if !present {
                missing.push(file.relative.to_owned());
            }
            statuses.push(ModelFileStatus {
                path: path.display().to_string(),
                present,
                bytes: metadata.map(|metadata| metadata.len()).unwrap_or(0),
                expected_sha256: file.sha256.map(ToOwned::to_owned),
            });
        }
        ModelStatus {
            id: id.to_owned(),
            label: label.to_owned(),
            available: missing.is_empty(),
            missing_files: missing,
            upstream: upstream.to_owned(),
            version: version.to_owned(),
            files: statuses,
        }
    }

    pub fn install_groups(&self) -> Vec<ModelInstallGroup> {
        model_install_groups(&self.root)
    }

    pub fn capabilities(&self) -> Capabilities {
        let asr_models = ASR_MODELS
            .iter()
            .map(|model| {
                let kind = match model.kind {
                    AsrModelKind::Streaming => "streaming",
                    AsrModelKind::Segmented => "segmented",
                };
                AsrModelCapability {
                    status: self.model_status(
                        model.id,
                        model.label,
                        model.upstream,
                        model.version,
                        model.files,
                    ),
                    kind: kind.to_owned(),
                    supported_modes: vec![kind.to_owned()],
                    source_languages: model
                        .source_languages
                        .iter()
                        .map(|language| (*language).to_owned())
                        .collect(),
                    supports_source_language: model.supports_source_language,
                    source_language_reason: model.source_language_note.map(ToOwned::to_owned),
                }
            })
            .collect();
        let translation_models = TRANSLATION_MODELS
            .iter()
            .map(|model| TranslationModelCapability {
                status: self.model_status(
                    model.id,
                    model.label,
                    model.upstream,
                    model.version,
                    &translation_files(model),
                ),
                source_language: model.source_language.to_owned(),
                target_language: model.target_language.to_owned(),
                engine: BERGAMOT_ENGINE_ID.to_owned(),
                supported_modes: vec!["text".to_owned()],
            })
            .collect();
        Capabilities {
            models_dir: self.root.display().to_string(),
            platform: platform_label(),
            audio: audio_capabilities(),
            asr_models,
            translation_models,
            translation_targets: SUPPORTED_LANGUAGES
                .iter()
                .map(|(code, label)| LanguageOption {
                    code: (*code).to_owned(),
                    label: (*label).to_owned(),
                })
                .collect(),
            translation_engine: BERGAMOT_ENGINE_ID.to_owned(),
            translation_runtime: crate::translation::TRANSLATION_RUNTIME.to_owned(),
            translation_model_family: crate::translation::TRANSLATION_MODEL_FAMILY.to_owned(),
            translation_modes: vec![
                ModeCapability {
                    mode: "auto".to_owned(),
                    available: true,
                    reason: None,
                    reason_code: None,
                },
                ModeCapability {
                    mode: "text".to_owned(),
                    available: true,
                    reason: None,
                    reason_code: None,
                },
                ModeCapability {
                    mode: "streaming".to_owned(),
                    available: false,
                    reason: Some(
                        "The fxtranslate runtime returns complete translated text for one submitted source string; it does not expose incremental output."
                            .to_owned(),
                    ),
                    reason_code: Some("complete-text-only".to_owned()),
                },
            ],
            diarization: self.model_status(
                "pyannote-segmentation-3.0",
                "Pyannote segmentation 3.0",
                "https://github.com/k2-fsa/sherpa-onnx/releases/tag/speaker-segmentation-models",
                "sherpa-onnx-pyannote-segmentation-3-0",
                DIARIZATION_FILES,
            ),
            speaker_embedding: self.model_status(
                "campplus-zh-en",
                "3D-Speaker CAM++ zh-en",
                "https://github.com/k2-fsa/sherpa-onnx/releases/tag/speaker-recongition-models",
                "3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx",
                SPEAKER_FILES,
            ),
            vad: self.model_status(
                "silero-vad",
                "Silero VAD",
                "https://github.com/k2-fsa/sherpa-onnx/releases/tag/asr-models",
                "silero_vad.onnx",
                VAD_FILES,
            ),
        }
    }
}

pub const VAD_FILES: &[ModelFile] = &[ModelFile {
    relative: "sherpa/silero_vad.onnx",
    min_bytes: 100_000,
    sha256: Some("9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6"),
}];

pub const SPEAKER_FILES: &[ModelFile] = &[ModelFile {
    relative: "sherpa/speaker/campplus-zh-en.onnx",
    min_bytes: 10_000_000,
    sha256: Some("aa3cfc16963a10586a9393f5035d6d6b57e98d358b347f80c2a30bf4f00ceba2"),
}];

pub const DIARIZATION_FILES: &[ModelFile] = &[ModelFile {
    relative: "sherpa/diarization/pyannote-segmentation-3.0.onnx",
    min_bytes: 1_000_000,
    sha256: Some("d582f4b4c6b48205de7e0643c57df0df5615a3c176189be3fc461e9d18827b5d"),
}];

pub fn translation_files(model: &TranslationModelSpec) -> Vec<ModelFile> {
    let mut files = vec![model.model.clone(), model.source_vocab.clone()];
    if let Some(target) = &model.target_vocab {
        files.push(target.clone());
    }
    if let Some(shortlist) = &model.shortlist {
        files.push(shortlist.clone());
    }
    files
}

pub fn resolve_asr_mode(spec: &AsrModelSpec, requested: AsrMode) -> Result<AsrMode, String> {
    let actual = match requested {
        AsrMode::Auto => spec.kind.mode(),
        mode => mode,
    };
    if actual != spec.kind.mode() {
        let requested_label = mode_label(actual);
        return Err(format!(
            "ASR model `{}` does not support {requested_label}. Its capability is {}.",
            spec.label,
            mode_label(spec.kind.mode())
        ));
    }
    Ok(actual)
}

pub fn resolve_translation_mode(requested: TranslationMode) -> Result<TranslationMode, String> {
    match requested {
        TranslationMode::Auto | TranslationMode::Text => Ok(TranslationMode::Text),
        TranslationMode::Streaming => Err(
            "The Bergamot adapter supports complete-text translation only; incremental translation is not available."
                .to_owned(),
        ),
    }
}

pub fn mode_label(mode: AsrMode) -> &'static str {
    match mode {
        AsrMode::Auto => "auto",
        AsrMode::Streaming => "streaming recognition",
        AsrMode::Segmented => "segmented recognition",
    }
}

pub fn language_label(code: &str) -> &str {
    SUPPORTED_LANGUAGES
        .iter()
        .find(|(candidate, _)| *candidate == code)
        .map(|(_, label)| *label)
        .unwrap_or(code)
}

pub fn detect_translation_source(text: &str) -> &'static str {
    let mut han = 0usize;
    let mut latin = 0usize;
    for character in text.chars() {
        if ('\u{4e00}'..='\u{9fff}').contains(&character) {
            han += 1;
        } else if character.is_ascii_alphabetic() {
            latin += 1;
        }
    }
    if han > 0 && han * 2 >= latin {
        "zh-Hans"
    } else {
        "en"
    }
}

pub fn verify_files(root: &Path, files: &[ModelFile]) -> Vec<(String, bool, Option<String>)> {
    files
        .iter()
        .map(|file| {
            let path = root.join(file.relative);
            let actual = match fs::read(&path) {
                Ok(bytes) => Some(format!("{:x}", Sha256::digest(&bytes))),
                Err(_) => None,
            };
            let matches = match (&file.sha256, &actual) {
                (Some(expected), Some(actual)) => expected.eq_ignore_ascii_case(actual),
                (None, Some(_)) => true,
                (_, None) => false,
            };
            (file.relative.to_owned(), matches, actual)
        })
        .collect()
}

fn platform_label() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

fn audio_capabilities() -> AudioCapabilities {
    let devices = crate::audio::system_outputs().unwrap_or_default();
    let supported = crate::audio::is_supported();
    AudioCapabilities {
        backend: if supported {
            "Core Audio process tap".to_owned()
        } else {
            "Unsupported on this platform".to_owned()
        },
        system_capture: supported,
        supported_platform: "macOS 14.4 or later (arm64)".to_owned(),
        permission: "macOS System Audio Recording".to_owned(),
        sample_rate: crate::audio::CAPTURE_RATE,
        channels: crate::audio::CAPTURE_CHANNELS,
        default_device: crate::audio::default_system_output(),
        devices,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_follows_the_model_capability() {
        let streaming = ASR_MODELS
            .iter()
            .find(|model| model.id == ASR_STREAMING_ID)
            .unwrap();
        assert_eq!(
            resolve_asr_mode(streaming, AsrMode::Auto).unwrap(),
            AsrMode::Streaming
        );
        assert!(resolve_asr_mode(streaming, AsrMode::Segmented).is_err());
    }

    #[test]
    fn bergamot_never_claims_streaming() {
        assert_eq!(
            resolve_translation_mode(TranslationMode::Auto).unwrap(),
            TranslationMode::Text
        );
        assert!(resolve_translation_mode(TranslationMode::Streaming).is_err());
    }

    #[test]
    fn a_missing_translation_route_carries_a_stable_reason_code() {
        let paths = ModelPaths::new(std::env::temp_dir());
        let capabilities = paths.capabilities();
        let streaming = capabilities
            .translation_modes
            .iter()
            .find(|mode| mode.mode == "streaming")
            .expect("the streaming route is reported");
        assert!(!streaming.available);
        assert_eq!(streaming.reason_code.as_deref(), Some("complete-text-only"));
        assert!(streaming.reason.is_some());
    }

    #[test]
    fn language_detection_handles_mixed_scripts() {
        assert_eq!(detect_translation_source("今天天气很好。"), "zh-Hans");
        assert_eq!(detect_translation_source("The weather is nice."), "en");
    }

    #[test]
    fn install_groups_are_partitioned_by_purpose() {
        let groups = ModelPaths::new(std::env::temp_dir()).install_groups();
        assert_eq!(groups.len(), 7);
        assert_eq!(
            groups
                .iter()
                .filter(|group| group.category == "recognition")
                .count(),
            2
        );
        assert_eq!(
            groups
                .iter()
                .filter(|group| group.category == "translation")
                .count(),
            2
        );
        assert_eq!(
            groups
                .iter()
                .filter(|group| group.category == "supporting")
                .count(),
            3
        );
    }

    #[test]
    fn download_file_streams_the_expected_payload() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let payload = b"sonasub-model-download".to_vec();
        let served = payload.clone();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 1024];
            let _ = stream.read(&mut request);
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                served.len()
            )
            .unwrap();
            stream.write_all(&served).unwrap();
        });
        let directory =
            std::env::temp_dir().join(format!("sonasub-download-test-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let destination = directory.join("payload.bin");
        download_file(
            &format!("http://{address}/payload.bin"),
            &destination,
            payload.len() as u64,
        )
        .unwrap();
        server.join().unwrap();
        assert_eq!(fs::read(&destination).unwrap(), payload);
        let _ = fs::remove_file(&destination);
        let _ = fs::remove_dir(&directory);
    }

    #[test]
    fn direct_download_installs_a_single_file() {
        let payload = b"sonasub-single-file-model".to_vec();
        let sha256 = format!("{:x}", Sha256::digest(&payload));
        let group = ModelInstallGroup {
            id: "direct-test".to_owned(),
            label: "Direct test".to_owned(),
            version: "test".to_owned(),
            category: "supporting".to_owned(),
            upstream: "https://example.invalid".to_owned(),
            download_kind: "file".to_owned(),
            download_url: "https://example.invalid/model.onnx".to_owned(),
            download_bytes: payload.len() as u64,
            download_urls: Vec::new(),
            download_bytes_each: Vec::new(),
            files: vec![ModelInstallFile {
                relative: "sherpa/direct-test.onnx".to_owned(),
                expected_sha256: Some(sha256),
                min_bytes: payload.len() as u64,
                source_name: "direct-test.onnx".to_owned(),
                present: false,
                bytes: 0,
            }],
        };
        let directory =
            std::env::temp_dir().join(format!("sonasub-install-test-{}", std::process::id()));
        let source = directory.join("direct-test.onnx");
        fs::create_dir_all(&directory).unwrap();
        fs::write(&source, &payload).unwrap();
        install_direct_file(&directory, &group, &source).unwrap();
        assert_eq!(
            fs::read(directory.join("sherpa/direct-test.onnx")).unwrap(),
            payload
        );
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn archive_sources_are_found_in_a_nested_model_directory() {
        let directory =
            std::env::temp_dir().join(format!("sonasub-archive-test-{}", std::process::id()));
        let nested = directory.join("sherpa-onnx-model-directory");
        fs::create_dir_all(&nested).unwrap();
        let expected = nested.join("model.int8.onnx");
        fs::write(&expected, b"model").unwrap();
        assert_eq!(
            find_source(&directory, "model.int8.onnx").unwrap(),
            expected
        );
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn bergamot_groups_keep_files_and_attachments_in_sync() {
        let groups = ModelPaths::new(std::env::temp_dir()).install_groups();
        for group in groups
            .iter()
            .filter(|group| group.category == "translation")
        {
            assert_eq!(group.files.len(), group.download_urls.len(), "{}", group.id);
            assert_eq!(
                group.files.len(),
                group.download_bytes_each.len(),
                "{}",
                group.id
            );
            assert_eq!(
                group.download_bytes,
                group.download_bytes_each.iter().sum::<u64>(),
                "{}",
                group.id
            );
        }
        let zh_en = groups
            .iter()
            .find(|group| group.id == "bergamot-zh-Hans-en")
            .unwrap();
        assert_eq!(
            zh_en
                .files
                .iter()
                .map(|file| file.source_name.as_str())
                .collect::<Vec<_>>(),
            vec![
                "model.zhen.intgemm.alphas.bin.zst",
                "vocab.zhen.spm.zst",
                "lex.50.50.zhen.s2t.bin.zst",
            ]
        );
    }

    #[test]
    fn bergamot_import_accepts_runtime_and_attachment_names() {
        let group = ModelInstallGroup {
            id: "bergamot-test".to_owned(),
            label: "Bergamot test".to_owned(),
            version: "test".to_owned(),
            category: "translation".to_owned(),
            upstream: "https://example.invalid".to_owned(),
            download_kind: "bergamot".to_owned(),
            download_url: "https://example.invalid/model.zst".to_owned(),
            download_bytes: 1,
            download_urls: Vec::new(),
            download_bytes_each: Vec::new(),
            files: Vec::new(),
        };
        let file = ModelInstallFile {
            relative: "bergamot/en-zh-Hans/model.bin".to_owned(),
            expected_sha256: None,
            min_bytes: 1,
            source_name: "model.enzh.intgemm.alphas.bin.zst".to_owned(),
            present: false,
            bytes: 0,
        };
        assert!(is_import_name(&group, &file, "model.bin"));
        assert!(is_import_name(
            &group,
            &file,
            "model.enzh.intgemm.alphas.bin.zst"
        ));
        assert!(!is_import_name(&group, &file, "other.bin"));
    }
}
