//! Bosun masaüstü uygulaması (GTK4 + libadwaita).
//!
//! Arayüz yalnızca servisle (bosund) Unix socket üzerinden konuşur. Yönetici eylemleri
//! `pkexec` ile yardımcıya yaptırılır; şifreyi masaüstünün kendi penceresi sorar.

mod client;
mod dialogs;
mod i18n;
mod pages;
mod shell;
mod spark;
mod ui;

use adw::prelude::*;

const APP_ID: &str = "io.github.kaptanoguz.Bosun";

const CSS: &str = include_str!("style.css");

/// "system" takip eder; "light" ve "dark" uygulamayı zorlar.
pub fn apply_theme(theme: &str) {
    let scheme = match theme {
        "light" => adw::ColorScheme::ForceLight,
        "dark" => adw::ColorScheme::ForceDark,
        _ => adw::ColorScheme::Default,
    };
    adw::StyleManager::default().set_color_scheme(scheme);
    // Masaüstü GTK'nın koyu tercihini açmış olabilir; libadwaita bunu desteklemiyor ve açık
    // seçimini ezebiliyor. Tercih, uygulamanın kendi seçimiyle aynı olsun.
    if let Some(settings) = gtk::Settings::default() {
        settings.set_gtk_application_prefer_dark_theme(theme == "dark");
    }
}

fn main() -> gtk::glib::ExitCode {
    // Servisten ilk yanıt gelene kadar dil, ayar dosyasından okunur.
    let config = bosun_core::config::Config::load();
    i18n::set_lang(config.language);
    let theme = config.theme;

    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_startup(move |_| {
        apply_theme(&theme);
        let provider = gtk::CssProvider::new();
        provider.load_from_string(CSS);
        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });
    app.connect_activate(|app| {
        // İkinci kez açılırsa var olan pencere öne gelir.
        if let Some(w) = app.active_window() {
            w.present();
            return;
        }
        shell::build(app);
    });
    // Komut satırı argümanı yok; GTK'nın kendi seçenekleri de devre dışı.
    app.run_with_args::<&str>(&[])
}
