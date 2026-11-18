use std::{fs, path::PathBuf};

use tauri::{command, State};

use crate::config::ConfigStore;

use super::{
    install::ModelInstallGroup,
    status::ModelDownloadSettings,
    transfer::{download_dir, download_group, import_group, DOWNLOAD_DIR},
};

#[command]
pub fn list_model_installs(
    state: State<'_, crate::session::SessionManager>,
) -> Vec<ModelInstallGroup> {
    state.model_paths().install_groups()
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
