pub mod asr;
pub mod audio;
pub mod config;
pub mod models;
pub mod records;
pub mod session;
pub mod speaker;
pub mod subtitles;
pub mod translation;

use records::RecordManager;
use session::SessionManager;
use tauri::{Manager, RunEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let manager = SessionManager::new(app.handle().clone())?;
            app.manage(manager);
            let records = RecordManager::new(app.handle().clone())?;
            app.manage(records);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            session::get_capabilities,
            models::list_model_installs,
            models::get_model_download_settings,
            models::set_model_download_directory,
            models::download_model,
            models::import_model_files,
            session::get_config,
            session::save_config,
            session::session_snapshot,
            session::start_session,
            session::stop_session,
            session::restart_session,
            records::list_records,
            records::list_record_devices,
            records::get_record,
            records::create_record,
            records::update_record,
            records::start_record,
            records::pause_record,
            records::resume_record,
            records::stop_record,
            records::add_record_device,
            records::remove_record_device,
            records::list_record_subtitles,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build SonaSub desktop application");
    app.run(|handle, event| {
        if matches!(event, RunEvent::ExitRequested { .. } | RunEvent::Exit) {
            if let Some(manager) = handle.try_state::<SessionManager>() {
                manager.shutdown();
            }
            if let Some(manager) = handle.try_state::<RecordManager>() {
                manager.shutdown();
            }
        }
    });
}
