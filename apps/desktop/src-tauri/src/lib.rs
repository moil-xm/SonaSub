pub mod engine;
pub mod protocol;
pub mod resources;

use engine::{EngineManager, EngineState};
use resources::EngineResources;
use tauri::{Emitter, Manager, RunEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .setup(|app| {
            let resources = EngineResources::resolve(app.handle())?;
            let manager = EngineManager::new(resources);
            let handle = app.handle().clone();
            manager.set_listener(move |event| {
                let state = match event {
                    engine::EngineEvent::Stopped => EngineState::Stopped,
                    engine::EngineEvent::Starting { pid } => EngineState::Starting { pid },
                    engine::EngineEvent::Running { info, port } => {
                        EngineState::Running { info, port }
                    }
                    engine::EngineEvent::Stopping { info, port } => {
                        EngineState::Stopping { info, port }
                    }
                    engine::EngineEvent::Failed { error } => EngineState::Failed { error },
                };
                let _ = handle.emit("engine://state", state);
            });
            app.manage(manager);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            engine::start_command,
            engine::stop_command,
            engine::restart_command,
            engine::status_command
        ])
        .build(tauri::generate_context!())
        .expect("failed to build SonaSub desktop application");
    app.run(|handle, event| {
        if matches!(event, RunEvent::ExitRequested { .. } | RunEvent::Exit) {
            handle.state::<EngineManager>().shutdown();
        }
    });
}
