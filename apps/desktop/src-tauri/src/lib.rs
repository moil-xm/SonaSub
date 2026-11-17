pub mod asr;
pub mod audio;
pub mod config;
pub mod models;
pub mod session;
pub mod speaker;
pub mod subtitles;
pub mod translation;

use session::SessionManager;
use tauri::{Manager, RunEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let manager = SessionManager::new(app.handle().clone())?;
            app.manage(manager);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            session::get_capabilities,
            session::get_config,
            session::save_config,
            session::session_snapshot,
            session::start_session,
            session::stop_session,
            session::restart_session,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build SonaSub desktop application");
    app.run(|handle, event| {
        if matches!(event, RunEvent::ExitRequested { .. } | RunEvent::Exit) {
            if let Some(manager) = handle.try_state::<SessionManager>() {
                manager.shutdown();
            }
        }
    });
}
