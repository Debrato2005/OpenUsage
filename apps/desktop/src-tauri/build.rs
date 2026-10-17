fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&["bootstrap"])),
    )
    .expect("Failed to build the OpenUsage application manifest");
}
