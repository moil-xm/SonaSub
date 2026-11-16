fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "start_command",
            "stop_command",
            "restart_command",
            "status_command",
        ]),
    ))
    .expect("build Tauri application permissions");
}
