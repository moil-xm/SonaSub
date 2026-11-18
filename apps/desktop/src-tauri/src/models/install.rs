use std::{fs, path::Path};

use serde::Serialize;

use super::paths::ModelPaths;
use super::registry::{
    translation_files, ModelFile, ASR_MODELS, ASR_SEGMENTED_ID, ASR_STREAMING_ID,
    DIARIZATION_FILES, SPEAKER_FILES, TRANSLATION_MODELS, VAD_FILES,
};

const SHERPA_RELEASE: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download";

const FIREFOX_ATTACHMENTS: &str =
    "https://firefox-settings-attachments.cdn.mozilla.net/main-workspace/translations-models-v2";

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

pub(super) fn group_expected_sources(group: &ModelInstallGroup) -> &[(&str, &str)] {
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

impl ModelPaths {
    pub fn install_groups(&self) -> Vec<ModelInstallGroup> {
        super::install::model_install_groups(&self.root)
    }
}

#[cfg(test)]
mod tests {
    use crate::models::ModelPaths;

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
}
