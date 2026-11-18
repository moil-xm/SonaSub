fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "get_capabilities",
            "list_model_installs",
            "get_model_download_settings",
            "set_model_download_directory",
            "download_model",
            "import_model_files",
            "get_config",
            "save_config",
            "session_snapshot",
            "start_session",
            "stop_session",
            "restart_session",
        ]),
    ))
    .expect("build Tauri application permissions");
}
