use tauri::State;

use super::{
    fetch_models, list_providers, save_provider, test_provider as test_provider_remote,
    ProviderConfig,
};
use crate::session::SessionManager;

#[tauri::command]
pub fn list_providers_command(
    state: State<'_, SessionManager>,
) -> Result<super::ProviderView, String> {
    Ok(list_providers(
        &state.inner().config_dir().join("providers.json"),
    ))
}

#[tauri::command]
pub fn save_provider_command(
    state: State<'_, SessionManager>,
    provider: ProviderConfig,
) -> Result<super::ProviderView, String> {
    save_provider(&state.inner().config_dir().join("providers.json"), provider)
}

#[tauri::command]
pub async fn fetch_provider_models(
    base_url: String,
    api_key: String,
) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || fetch_models(&base_url, &api_key))
        .await
        .map_err(|error| format!("Model fetch failed: {error}"))?
}

#[tauri::command]
pub async fn test_provider_command(
    base_url: String,
    api_key: String,
    model: String,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || test_provider_remote(&base_url, &api_key, &model))
        .await
        .map_err(|error| format!("Provider test failed: {error}"))?
}
