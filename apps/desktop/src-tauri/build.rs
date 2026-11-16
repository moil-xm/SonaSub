fn main() {
    println!("cargo:rerun-if-env-changed=SHERPA_LIB_PATH");

    let sherpa_lib_path = std::env::var_os("SHERPA_LIB_PATH").unwrap_or_else(|| {
        panic!("SHERPA_LIB_PATH is not set. Run `pnpm prepare:native` from the repository root.")
    });
    let sherpa_lib_dir = std::path::PathBuf::from(sherpa_lib_path).join("lib");
    for library in ["libsherpa-onnx-c-api.a", "libonnxruntime.a"] {
        let path = sherpa_lib_dir.join(library);
        assert!(
            path.is_file(),
            "Missing native speech runtime library {}. Run `pnpm prepare:native` from the repository root.",
            path.display()
        );
    }

    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "get_capabilities",
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
