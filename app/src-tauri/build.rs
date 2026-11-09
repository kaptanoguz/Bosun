fn main() {
    // Uygulama komutlarını açıkça bildir: capability dosyasında izin verilmeyen
    // hiçbir komut arayüzden çağrılamaz.
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&["daemon_call", "start_daemon", "run_admin", "helper_status"])),
    )
    .expect("tauri-build başarısız");
}
