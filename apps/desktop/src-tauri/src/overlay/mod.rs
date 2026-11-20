use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder};

use crate::records::RecordManager;
use crate::session::SessionManager;

pub const CAPTION_OVERLAY_LABEL: &str = "caption-overlay";
pub const CAPTION_OVERLAY_URL: &str = "overlay.html";
pub const MAIN_WINDOW_LABEL: &str = "main";
pub const LOCK_EVENT: &str = "overlay://lock";
pub const BOUND_RECORD_EVENT: &str = "overlay://bound-record";
pub const OPEN_EVENT: &str = "overlay://open";
pub const CLOSED_EVENT: &str = "overlay://closed";

const INITIAL_WIDTH: f64 = 480.0;
const INITIAL_HEIGHT: f64 = 220.0;
const MIN_WIDTH: f64 = 320.0;
const MIN_HEIGHT: f64 = 120.0;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptionOverlayLockState {
    pub locked: bool,
    pub is_open: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptionOverlayBinding {
    pub record_id: String,
}

#[derive(Debug, Default)]
struct OverlayRuntime {
    bound_record_id: Option<String>,
    is_open: bool,
    is_locked: bool,
}

#[derive(Debug, Default)]
pub struct OverlayState {
    runtime: Arc<Mutex<OverlayRuntime>>,
    exit_cleanup: Arc<ExitCleanupGuard>,
}

#[derive(Debug, Default)]
struct ExitCleanupGuard {
    was_requested: AtomicBool,
}

impl ExitCleanupGuard {
    fn run_once<F: FnOnce()>(&self, cleanup: F) {
        if !self.was_requested.swap(true, Ordering::AcqRel) {
            cleanup();
        }
    }
}

#[derive(Debug, Clone)]
struct OverlayRuntimeSnapshot {
    bound_record_id: Option<String>,
    is_open: bool,
    is_locked: bool,
}

impl OverlayRuntimeSnapshot {
    fn lock_state(&self) -> CaptionOverlayLockState {
        CaptionOverlayLockState {
            locked: self.is_locked,
            is_open: self.is_open,
        }
    }
}

fn read_runtime(runtime: &Arc<Mutex<OverlayRuntime>>) -> Result<OverlayRuntimeSnapshot, String> {
    let state = runtime.lock().map_err(|error| error.to_string())?;
    Ok(OverlayRuntimeSnapshot {
        bound_record_id: state.bound_record_id.clone(),
        is_open: state.is_open,
        is_locked: state.is_locked,
    })
}

fn update_runtime<F>(runtime: &Arc<Mutex<OverlayRuntime>>, update: F) -> Result<(), String>
where
    F: FnOnce(&mut OverlayRuntime),
{
    let mut state = runtime.lock().map_err(|error| error.to_string())?;
    update(&mut state);
    Ok(())
}

impl OverlayState {
    pub fn run_event_loop(app: &AppHandle, event: RunEvent) {
        match event {
            RunEvent::ExitRequested { .. } => {
                if let Some(state) = app.try_state::<OverlayState>() {
                    state.shutdown_managers_once(app);
                }
            }
            RunEvent::WindowEvent {
                label,
                event: window_event,
                ..
            } if label == MAIN_WINDOW_LABEL || label == CAPTION_OVERLAY_LABEL => match window_event
            {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    if label == CAPTION_OVERLAY_LABEL {
                        let _ = close_caption_overlay(app.clone());
                    } else {
                        if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
                            let _ = window.hide();
                        }
                    }
                }
                tauri::WindowEvent::Destroyed if label == CAPTION_OVERLAY_LABEL => {
                    update_runtime_on_destroyed(app);
                }
                _ => {}
            },
            #[cfg(target_os = "macos")]
            RunEvent::Reopen { .. } => show_main_window(app),
            _ => {}
        }
    }
}

fn show_main_window(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    let _ = app.show();
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::ExitCleanupGuard;

    #[test]
    fn exit_cleanup_guard_runs_once() {
        let guard = ExitCleanupGuard::default();
        let count = AtomicUsize::new(0);
        let call = || {
            count.fetch_add(1, Ordering::SeqCst);
        };

        guard.run_once(call);
        guard.run_once(call);

        assert_eq!(count.load(Ordering::SeqCst), 1);
    }
}

impl OverlayState {
    fn shutdown_managers_once(&self, app: &AppHandle) {
        self.exit_cleanup.run_once(|| {
            if let Some(manager) = app.try_state::<SessionManager>() {
                manager.shutdown();
            }
            if let Some(manager) = app.try_state::<RecordManager>() {
                manager.shutdown();
            }
        });
    }
}

fn overlay_runtime(app: &AppHandle) -> Result<Arc<Mutex<OverlayRuntime>>, String> {
    app.try_state::<OverlayState>()
        .map(|state| state.inner().runtime.clone())
        .ok_or_else(|| "Caption overlay runtime is not initialized.".to_owned())
}

pub fn init(app: &AppHandle) -> tauri::Result<()> {
    app.manage(OverlayState::default());
    Ok(())
}

#[tauri::command]
pub fn open_caption_overlay(app: AppHandle, record_id: String) -> Result<(), String> {
    let runtime = overlay_runtime(&app)?;
    update_runtime(&runtime, |state| {
        state.bound_record_id = Some(record_id.clone());
        state.is_open = true;
        state.is_locked = false;
    })?;

    if let Some(window) = app.get_webview_window(CAPTION_OVERLAY_LABEL) {
        apply_lock_state(&app, false)?;
        window.show().map_err(|error| error.to_string())?;
        window.set_focus().map_err(|error| error.to_string())?;
    } else {
        WebviewWindowBuilder::new(
            &app,
            CAPTION_OVERLAY_LABEL,
            WebviewUrl::App(CAPTION_OVERLAY_URL.into()),
        )
        .title("SonaSub captions")
        .inner_size(INITIAL_WIDTH, INITIAL_HEIGHT)
        .min_inner_size(MIN_WIDTH, MIN_HEIGHT)
        .resizable(true)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .skip_taskbar(true)
        .shadow(true)
        .build()
        .map_err(|error| error.to_string())?;
        apply_lock_state(&app, false)?;
    }

    app.emit_to(
        CAPTION_OVERLAY_LABEL,
        BOUND_RECORD_EVENT,
        CaptionOverlayBinding { record_id },
    )
    .map_err(|error| error.to_string())?;
    emit_lock_state(&app)?;
    app.emit(OPEN_EVENT, ()).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn close_caption_overlay(app: AppHandle) -> Result<(), String> {
    let runtime = overlay_runtime(&app)?;
    update_runtime(&runtime, |state| {
        state.is_open = false;
        state.is_locked = false;
        state.bound_record_id = None;
    })?;
    if let Some(window) = app.get_webview_window(CAPTION_OVERLAY_LABEL) {
        window
            .set_ignore_cursor_events(false)
            .map_err(|error| error.to_string())?;
        window.destroy().map_err(|error| error.to_string())?;
    }
    emit_lock_state(&app)?;
    app.emit(CLOSED_EVENT, ())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_caption_overlay_locked(app: AppHandle, locked: bool) -> Result<(), String> {
    let runtime = overlay_runtime(&app)?;
    if locked && !read_runtime(&runtime)?.is_open {
        return Err("The caption overlay is not open.".to_owned());
    }
    update_runtime(&runtime, |state| {
        state.is_locked = locked;
    })?;
    apply_lock_state(&app, locked)
}

#[tauri::command]
pub fn caption_overlay_lock_state(app: AppHandle) -> Result<CaptionOverlayLockState, String> {
    let runtime = overlay_runtime(&app)?;
    Ok(read_runtime(&runtime)?.lock_state())
}

#[tauri::command]
pub fn caption_overlay_binding(app: AppHandle) -> Result<Option<String>, String> {
    let runtime = overlay_runtime(&app)?;
    Ok(read_runtime(&runtime)?.bound_record_id)
}

#[tauri::command]
pub fn request_app_exit(app: AppHandle) {
    if let Some(state) = app.try_state::<OverlayState>() {
        state.shutdown_managers_once(&app);
    }
    let _ = close_caption_overlay(app.clone());
    app.exit(0);
}

fn apply_lock_state(app: &AppHandle, locked: bool) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(CAPTION_OVERLAY_LABEL) {
        window
            .set_ignore_cursor_events(locked)
            .map_err(|error| error.to_string())?;
        window
            .set_resizable(!locked)
            .map_err(|error| error.to_string())?;
    }
    emit_lock_state(app)
}

fn update_runtime_on_destroyed(app: &AppHandle) {
    let Ok(runtime) = overlay_runtime(app) else {
        return;
    };
    let was_open = read_runtime(&runtime).is_ok_and(|state| state.is_open);
    if was_open && app.get_webview_window(CAPTION_OVERLAY_LABEL).is_some() {
        return;
    }
    let _ = update_runtime(&runtime, |state| {
        state.is_open = false;
        state.is_locked = false;
        state.bound_record_id = None;
    });
    let _ = emit_lock_state(app);
    let _ = app.emit(CLOSED_EVENT, ());
}

fn emit_lock_state(app: &AppHandle) -> Result<(), String> {
    let runtime = overlay_runtime(app)?;
    let lock_state = read_runtime(&runtime)?.lock_state();
    app.emit(LOCK_EVENT, lock_state)
        .map_err(|error| error.to_string())
}
