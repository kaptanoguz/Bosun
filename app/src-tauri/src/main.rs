// Sürüm derlemesinde ek konsol penceresi açılmasın.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // AppImage'da ayrı bir servis ikilisi yoktur: aynı dosya `--daemon` ile arka plan servisi olur.
    if std::env::args().any(|a| a == "--daemon") {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio çalışma ortamı kurulamadı");
        if let Err(e) = rt.block_on(bosund::run()) {
            eprintln!("bosund: {e:#}");
            std::process::exit(1);
        }
        return;
    }
    bosun_app_lib::run()
}
