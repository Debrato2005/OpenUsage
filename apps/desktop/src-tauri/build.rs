fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "bootstrap",
            "setup_workspace",
            "unlock_workspace",
            "set_theme",
            "create_backup",
        ]),
    ))
    .expect("Failed to build the OpenUsage application manifest");
}
