use tauri::State;

use crate::config::AppConfig;

use super::{types::SessionSnapshot, ConfigView, SessionManager};

#[tauri::command]
pub async fn get_capabilities(
    state: State<'_, SessionManager>,
) -> Result<crate::models::Capabilities, String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.capabilities())
        .await
        .map_err(|error| format!("Capability query failed: {error}"))
}

#[tauri::command]
pub fn get_config(state: State<'_, SessionManager>) -> ConfigView {
    state.inner().config()
}

#[tauri::command]
pub async fn save_config(
    state: State<'_, SessionManager>,
    config: AppConfig,
) -> Result<ConfigView, String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.save_config(config))
        .await
        .map_err(|error| format!("Configuration save failed: {error}"))?
}

#[tauri::command]
pub fn session_snapshot(state: State<'_, SessionManager>) -> SessionSnapshot {
    state.inner().snapshot()
}

#[tauri::command]
pub async fn start_session(state: State<'_, SessionManager>) -> Result<SessionSnapshot, String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.start())
        .await
        .map_err(|error| format!("Session start failed: {error}"))?
}

#[tauri::command]
pub async fn stop_session(state: State<'_, SessionManager>) -> Result<SessionSnapshot, String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.stop())
        .await
        .map_err(|error| format!("Session stop failed: {error}"))?
}

#[tauri::command]
pub async fn restart_session(state: State<'_, SessionManager>) -> Result<SessionSnapshot, String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.restart())
        .await
        .map_err(|error| format!("Session restart failed: {error}"))?
}
