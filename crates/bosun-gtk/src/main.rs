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

/// Masaüstünün GTK tema adı; ilk uygulamada okunur ve "system" seçilince geri yüklenir.
static DESKTOP_GTK_THEME: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// "system" takip eder; "light" ve "dark" uygulamayı zorlar.
pub fn apply_theme(theme: &str) {
    let scheme = match theme {
        "light" => adw::ColorScheme::ForceLight,
        "dark" => adw::ColorScheme::ForceDark,
        _ => adw::ColorScheme::Default,
    };
    adw::StyleManager::default().set_color_scheme(scheme);
    let Some(settings) = gtk::Settings::default() else {
        return;
    };
    // Masaüstü GTK'nın koyu tercihini açmış olabilir; libadwaita bunu desteklemiyor ve açık
    // seçimini ezebiliyor. Tercih, uygulamanın kendi seçimiyle aynı olsun.
    settings.set_gtk_application_prefer_dark_theme(theme == "dark");
    // Masaüstü GTK teması (ör. Mint-Y-Dark) kendi GTK 4 CSS'ini libadwaita'nın üstüne yükler
    // ve pencereyi siyaha boyar. Zorlanan seçimde libadwaita'nın kendi boş teması kullanılır;
    // "Adwaita" adı GTK'nın eski yerleşik temasını yükleyip libadwaita ile karıştırır.
    let desktop = DESKTOP_GTK_THEME.get_or_init(|| {
        settings
            .gtk_theme_name()
            .map(|n| n.to_string())
            .unwrap_or_default()
    });
    let name = if theme == "system" {
        desktop.as_str()
    } else {
        "Adwaita-empty"
    };
    if !name.is_empty() {
        settings.set_gtk_theme_name(Some(name));
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
                // Kullanıcının ~/.config/gtk-4.0/gtk.css dosyası (masaüstü temaları buraya
                // kopyalanabiliyor) Bosun'un kendi sınıflarını ezmesin.
                gtk::STYLE_PROVIDER_PRIORITY_USER + 1,
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
