use std::path::Path;

use tauri::State;

use crate::audio::AudioDeviceRef;

use super::{
    manager::RecordManager,
    types::{RecordDevice, RecordExportSummary, RecordInput, RecordListItem, RecordSnapshot},
};

#[tauri::command]
pub fn list_records(state: State<'_, RecordManager>) -> Vec<RecordListItem> {
    state.list_records()
}

#[tauri::command]
pub fn list_record_devices(state: State<'_, RecordManager>) -> Vec<RecordDevice> {
    state.list_devices()
}

#[tauri::command]
pub fn get_record(
    state: State<'_, RecordManager>,
    record_id: String,
) -> Result<RecordSnapshot, String> {
    state.get_record(&record_id)
}

#[tauri::command]
pub fn create_record(
    state: State<'_, RecordManager>,
    input: RecordInput,
) -> Result<RecordSnapshot, String> {
    state.create_record(input)
}

#[tauri::command]
pub fn update_record(
    state: State<'_, RecordManager>,
    record_id: String,
    input: RecordInput,
) -> Result<RecordSnapshot, String> {
    state.update_record(&record_id, input)
}

#[tauri::command]
pub async fn start_record(
    state: State<'_, RecordManager>,
    record_id: String,
) -> Result<RecordSnapshot, String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.start_record(&record_id))
        .await
        .map_err(|error| format!("Record start failed: {error}"))?
}

#[tauri::command]
pub async fn pause_record(
    state: State<'_, RecordManager>,
    record_id: String,
) -> Result<RecordSnapshot, String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.pause_record(&record_id))
        .await
        .map_err(|error| format!("Record pause failed: {error}"))?
}

#[tauri::command]
pub async fn resume_record(
    state: State<'_, RecordManager>,
    record_id: String,
) -> Result<RecordSnapshot, String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.resume_record(&record_id))
        .await
        .map_err(|error| format!("Record resume failed: {error}"))?
}

#[tauri::command]
pub async fn stop_record(
    state: State<'_, RecordManager>,
    record_id: String,
) -> Result<RecordSnapshot, String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.stop_record(&record_id))
        .await
        .map_err(|error| format!("Record stop failed: {error}"))?
}

#[tauri::command]
pub fn delete_record(state: State<'_, RecordManager>, record_id: String) -> Result<(), String> {
    state.delete_record(&record_id)
}

#[tauri::command]
pub async fn export_record(
    state: State<'_, RecordManager>,
    record_id: String,
    destination: String,
) -> Result<RecordExportSummary, String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        manager.export_record(&record_id, Path::new(&destination))
    })
    .await
    .map_err(|error| format!("Record export failed: {error}"))?
}

#[tauri::command]
pub async fn add_record_device(
    state: State<'_, RecordManager>,
    record_id: String,
    device: AudioDeviceRef,
) -> Result<RecordSnapshot, String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.add_device(&record_id, device))
        .await
        .map_err(|error| format!("Record device add failed: {error}"))?
}

#[tauri::command]
pub async fn remove_record_device(
    state: State<'_, RecordManager>,
    record_id: String,
    device: AudioDeviceRef,
) -> Result<RecordSnapshot, String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.remove_device(&record_id, &device))
        .await
        .map_err(|error| format!("Record device removal failed: {error}"))?
}
