use std::{fs, path::Path};

use serde::Serialize;

use super::paths::ModelPaths;
use super::registry::{
    translation_files, AsrModelSpec, ModelFile, ASR_MODELS, ASR_QWEN3_06B_ID, ASR_QWEN3_17B_ID,
    ASR_SEGMENTED_ID, ASR_STREAMING_ID, DIARIZATION_FILES, SPEAKER_FILES, TRANSLATION_MODELS,
    VAD_FILES,
};

const SHERPA_RELEASE: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download";
pub(super) const MODELSCOPE_API: &str =
    "https://modelscope.cn/api/v1/models/zengshuishui/Qwen3-ASR-onnx/repo";
pub(super) const QWEN3_17B_REVISION: &str = "9c182309f7bb075f241424441add9e16c5086dfb";

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
    pub archive_sha256: Option<String>,
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
        ASR_STREAMING_ID | ASR_SEGMENTED_ID | ASR_QWEN3_06B_ID | ASR_QWEN3_17B_ID => "recognition",
        _ => "supporting",
    }
}

fn sherpa_group(
    root: &Path,
    spec: &AsrModelSpec,
    url: String,
    archive_sha256: Option<&'static str>,
) -> ModelInstallGroup {
    ModelInstallGroup {
        id: spec.id.to_owned(),
        label: spec.label.to_owned(),
        version: spec.version.to_owned(),
        category: sherpa_category(spec.id).to_owned(),
        upstream: "https://github.com/k2-fsa/sherpa-onnx/releases".to_owned(),
        download_kind: if url.ends_with(".tar.bz2") {
            "tarBz2".to_owned()
        } else {
            "file".to_owned()
        },
        download_url: url,
        download_bytes: spec.download_bytes,
        download_urls: Vec::new(),
        download_bytes_each: Vec::new(),
        archive_sha256: archive_sha256.map(ToOwned::to_owned),
        files: spec
            .files
            .iter()
            .map(|file| {
                install_file_from_spec(
                    root,
                    file,
                    spec.source_name.unwrap_or_else(|| sherpa_source_name(file)),
                )
            })
            .collect(),
    }
}

fn qwen3_group(root: &Path, spec: &'static AsrModelSpec) -> ModelInstallGroup {
    let sources: Vec<(&str, &str)> = if spec.id == ASR_QWEN3_06B_ID {
        qwen3_06b_runtime_files().to_vec()
    } else {
        qwen3_17b_runtime_files()
            .iter()
            .map(|(_, name, _, _)| (*name, *name))
            .collect()
    };
    let download_kind = if spec.id == ASR_QWEN3_06B_ID {
        "tarBz2"
    } else {
        "files"
    };
    let download_url = if spec.id == ASR_QWEN3_06B_ID {
        format!(
            "{SHERPA_RELEASE}/asr-models/{}",
            spec.source_name.unwrap_or_default()
        )
    } else {
        format!("{MODELSCOPE_API}?Revision={QWEN3_17B_REVISION}")
    };
    ModelInstallGroup {
        id: spec.id.to_owned(),
        label: spec.label.to_owned(),
        version: spec.version.to_owned(),
        category: sherpa_category(spec.id).to_owned(),
        upstream: spec.upstream.to_owned(),
        download_kind: download_kind.to_owned(),
        download_url,
        download_bytes: spec.download_bytes,
        download_urls: Vec::new(),
        download_bytes_each: Vec::new(),
        archive_sha256: qwen3_archive_spec(spec.id).map(|(_, sha256)| sha256.to_owned()),
        files: spec
            .files
            .iter()
            .map(|file| {
                let source_name = sources
                    .iter()
                    .find(|(relative, _)| *relative == file.relative)
                    .map(|(_, source)| *source)
                    .unwrap_or(file.relative);
                install_file_from_spec(root, file, source_name)
            })
            .collect(),
    }
}

fn supplementary_spec(
    id: &'static str,
    label: &'static str,
    version: &'static str,
    files: &'static [ModelFile],
    download_bytes: u64,
) -> AsrModelSpec {
    AsrModelSpec {
        id,
        label,
        kind: super::registry::AsrModelKind::Segmented,
        source_languages: &[],
        supports_source_language: false,
        source_language_note: None,
        upstream: "",
        version,
        files,
        download_bytes,
        source_name: None,
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
        archive_sha256: None,
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

fn registered_asr(id: &str) -> &'static AsrModelSpec {
    ASR_MODELS
        .iter()
        .find(|model| model.id == id)
        .unwrap_or_else(|| panic!("ASR model `{id}` is not registered"))
}

fn model_install_groups(root: &Path) -> Vec<ModelInstallGroup> {
    let streaming = registered_asr(ASR_STREAMING_ID);
    let sense_voice = registered_asr(ASR_SEGMENTED_ID);
    vec![
        sherpa_group(
            root,
            streaming,
            format!("{SHERPA_RELEASE}/asr-models/{}.tar.bz2", streaming.version),
            None,
        ),
        sherpa_group(
            root,
            sense_voice,
            format!(
                "{SHERPA_RELEASE}/asr-models/{}.tar.bz2",
                sense_voice.version
            ),
            None,
        ),
        qwen3_group(root, registered_asr(ASR_QWEN3_06B_ID)),
        qwen3_group(root, registered_asr(ASR_QWEN3_17B_ID)),
        bergamot_group(root, 0),
        bergamot_group(root, 1),
        sherpa_group(
            root,
            &supplementary_spec("silero-vad", "Silero VAD", "silero_vad.onnx", VAD_FILES, 643_854),
            format!("{SHERPA_RELEASE}/asr-models/silero_vad.onnx"),
            None,
        ),
        sherpa_group(
            root,
            &supplementary_spec("campplus-zh-en", "3D-Speaker CAM++ zh-en", "3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx", SPEAKER_FILES, 28_281_164),
            format!("{SHERPA_RELEASE}/speaker-recongition-models/3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx"),
            None,
        ),
        sherpa_group(
            root,
            &supplementary_spec("pyannote-segmentation-3.0", "Pyannote segmentation 3.0", "sherpa-onnx-pyannote-segmentation-3-0", DIARIZATION_FILES, 6_958_444),
            format!("{SHERPA_RELEASE}/speaker-segmentation-models/sherpa-onnx-pyannote-segmentation-3-0.tar.bz2"),
            None,
        ),
    ]
}

pub(super) fn qwen3_archive_spec(id: &str) -> Option<(&'static str, &'static str)> {
    let spec = ASR_MODELS.iter().find(|spec| spec.id == id)?;
    let source = spec.source_name?;
    let sha256 = match spec.id {
        ASR_QWEN3_06B_ID => "393f8a14e2f5fb96746aaab342997a40641001fbd5bf9592a080a8329178ee96",
        _ => return None,
    };
    Some((source, sha256))
}

pub(super) fn qwen3_17b_runtime_files() -> &'static [(&'static str, &'static str, u64, &'static str)]
{
    &[
        (
            "model_1.7B/conv_frontend.onnx",
            "conv_frontend.onnx",
            48_080_441,
            "fa894a4ba53da6a4238f2a6ca0b09362e505d39cecbd646051b033e2e8d7e2fb",
        ),
        (
            "model_1.7B/encoder.int8.onnx",
            "encoder.int8.onnx",
            314_222_162,
            "436fbd910a0c8914851e5ac1354e807be9f283d08a5da728adaa609731c41469",
        ),
        (
            "model_1.7B/decoder.int8.onnx",
            "decoder.int8.onnx",
            2_037_458_645,
            "c43c853fa6e97d08365cb8a5502b360b595cd43c00dc60e4d8ca7cc18cad460b",
        ),
        (
            "tokenizer/merges.txt",
            "merges.txt",
            1_671_853,
            "8831e4f1a044471340f7c0a83d7bd71306a5b867e95fd870f74d0c5308a904d5",
        ),
        (
            "tokenizer/tokenizer_config.json",
            "tokenizer_config.json",
            12_487,
            "4942d005604266809309cabc9f4e9cb89ce855d59b14681fdc0e1cc62ea26c4c",
        ),
        (
            "tokenizer/vocab.json",
            "vocab.json",
            2_776_833,
            "ca10d7e9fb3ed18575dd1e277a2579c16d108e32f27439684afa0e10b1440910",
        ),
    ]
}

pub(super) fn qwen3_06b_runtime_files() -> &'static [(&'static str, &'static str)] {
    &[
        (
            "sherpa/qwen3-asr-0.6b-int8/conv_frontend.onnx",
            "conv_frontend.onnx",
        ),
        (
            "sherpa/qwen3-asr-0.6b-int8/encoder.int8.onnx",
            "encoder.int8.onnx",
        ),
        (
            "sherpa/qwen3-asr-0.6b-int8/decoder.int8.onnx",
            "decoder.int8.onnx",
        ),
        (
            "sherpa/qwen3-asr-0.6b-int8/tokenizer/merges.txt",
            "merges.txt",
        ),
        (
            "sherpa/qwen3-asr-0.6b-int8/tokenizer/tokenizer_config.json",
            "tokenizer_config.json",
        ),
        (
            "sherpa/qwen3-asr-0.6b-int8/tokenizer/vocab.json",
            "vocab.json",
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
    use crate::models::{ModelPaths, ASR_QWEN3_06B_ID, ASR_QWEN3_17B_ID};

    #[test]
    fn install_groups_are_partitioned_by_purpose() {
        let groups = ModelPaths::new(std::env::temp_dir()).install_groups();
        assert_eq!(groups.len(), 9);
        assert_eq!(
            groups
                .iter()
                .filter(|group| group.category == "recognition")
                .count(),
            4
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

    #[test]
    fn qwen3_groups_include_all_runtime_files_and_pinned_checksums() {
        let groups = ModelPaths::new(std::env::temp_dir()).install_groups();
        for id in [ASR_QWEN3_06B_ID, ASR_QWEN3_17B_ID] {
            let group = groups
                .iter()
                .find(|group| group.id == id)
                .expect("Qwen3 install group exists");
            assert_eq!(group.category, "recognition");
            assert_eq!(group.files.len(), 6);
            assert!(group.files.iter().all(|file| file
                .expected_sha256
                .as_deref()
                .is_some_and(|hash| hash.len() == 64)));
            assert!(group
                .files
                .iter()
                .any(|file| file.relative.ends_with("tokenizer/vocab.json")));
        }

        let official = groups
            .iter()
            .find(|group| group.id == ASR_QWEN3_06B_ID)
            .unwrap();
        assert_eq!(
            official.archive_sha256.as_deref(),
            Some("393f8a14e2f5fb96746aaab342997a40641001fbd5bf9592a080a8329178ee96")
        );
        let community = groups
            .iter()
            .find(|group| group.id == ASR_QWEN3_17B_ID)
            .unwrap();
        assert!(community.archive_sha256.is_none());
        assert_eq!(community.download_kind, "files");
    }
}
