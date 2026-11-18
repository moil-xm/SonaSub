use std::{fs, path::PathBuf};

use sha2::{Digest, Sha256};

use super::{
    registry::{AsrModelSpec, ModelFile, TranslationModelSpec, ASR_MODELS, TRANSLATION_MODELS},
    status::{ModelFileStatus, ModelStatus},
};

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
}
