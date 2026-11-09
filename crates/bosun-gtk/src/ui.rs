//! Küçük widget yardımcıları. Satır başlıkları düz metindir (markup kapalı), çünkü dosya
//! yolları ve uygulama adları `&` ya da `<` içerebilir.

use adw::prelude::*;
use gtk::glib;

pub fn label(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .build()
}

pub fn dim_label(text: &str) -> gtk::Label {
    let l = label(text);
    l.add_css_class("dim-label");
    l
}

pub fn heading(text: &str) -> gtk::Label {
    let l = label(text);
    l.add_css_class("title-2");
    l
}

pub fn row(title: &str, subtitle: &str) -> adw::ActionRow {
    let r = adw::ActionRow::builder()
        .use_markup(false)
        .title(title)
        .build();
    if !subtitle.is_empty() {
        r.set_subtitle(subtitle);
    }
    r
}

pub fn expander(title: &str, subtitle: &str) -> adw::ExpanderRow {
    let r = adw::ExpanderRow::builder()
        .use_markup(false)
        .title(title)
        .build();
    if !subtitle.is_empty() {
        r.set_subtitle(subtitle);
    }
    r
}

pub fn group(title: &str, description: &str) -> adw::PreferencesGroup {
    let g = adw::PreferencesGroup::new();
    if !title.is_empty() {
        g.set_title(&glib::markup_escape_text(title));
    }
    if !description.is_empty() {
        g.set_description(Some(&glib::markup_escape_text(description)));
    }
    g
}

pub fn button(text: &str) -> gtk::Button {
    gtk::Button::builder()
        .label(text)
        .valign(gtk::Align::Center)
        .build()
}

pub fn pill(text: &str, class: &str) -> gtk::Button {
    let b = button(text);
    b.add_css_class("pill");
    if !class.is_empty() {
        b.add_css_class(class);
    }
    b
}

pub fn icon_button(icon: &str, tooltip: &str) -> gtk::Button {
    let b = gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(tooltip)
        .valign(gtk::Align::Center)
        .build();
    b.add_css_class("flat");
    b
}

/// Sağda duran sayı ya da boyut.
pub fn value(text: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.add_css_class("numeric");
    l.set_valign(gtk::Align::Center);
    l
}

/// Küçük renkli etiket (Güvenli, Dikkat, Kalıcı…).
pub fn tag(text: &str, class: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.add_css_class("tag");
    if !class.is_empty() {
        l.add_css_class(class);
    }
    l.set_valign(gtk::Align::Center);
    l
}

pub fn bar(fraction: f64, warn: bool) -> gtk::LevelBar {
    let b = gtk::LevelBar::builder()
        .min_value(0.0)
        .max_value(1.0)
        .value(fraction.clamp(0.0, 1.0))
        .build();
    // Varsayılan eşikleri kaldır; renk yalnızca bizim "warn" sınıfımızdan gelsin.
    b.remove_offset_value(Some(gtk::LEVEL_BAR_OFFSET_LOW));
    b.remove_offset_value(Some(gtk::LEVEL_BAR_OFFSET_HIGH));
    b.remove_offset_value(Some(gtk::LEVEL_BAR_OFFSET_FULL));
    b.add_css_class("thin");
    if warn {
        b.add_css_class("warn");
    }
    b
}

/// Kopyalanabilir bir komut satırı.
pub fn command(cmd: &str) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    b.add_css_class("command");
    let l = gtk::Label::builder()
        .label(cmd)
        .xalign(0.0)
        .hexpand(true)
        .selectable(true)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::Char)
        .build();
    l.add_css_class("monospace");
    let copy = icon_button("edit-copy-symbolic", &crate::i18n::t("copy"));
    let text = cmd.to_string();
    copy.connect_clicked(move |btn| {
        btn.clipboard().set_text(&text);
        btn.set_icon_name("object-select-symbolic");
        let btn = btn.clone();
        glib::timeout_add_seconds_local_once(2, move || btn.set_icon_name("edit-copy-symbolic"));
    });
    b.append(&l);
    b.append(&copy);
    b
}

pub fn clear_box(b: &gtk::Box) {
    while let Some(c) = b.first_child() {
        b.remove(&c);
    }
}

pub fn clear_list(l: &gtk::ListBox) {
    l.remove_all();
}

pub fn boxed_list() -> gtk::ListBox {
    let l = gtk::ListBox::new();
    l.set_selection_mode(gtk::SelectionMode::None);
    l.add_css_class("boxed-list");
    l
}

/// Kaydırılabilir, ortalanmış bir sayfa gövdesi.
pub fn page(max_width: i32) -> (gtk::ScrolledWindow, gtk::Box) {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content.set_margin_start(12);
    content.set_margin_end(12);
    let clamp = adw::Clamp::builder()
        .maximum_size(max_width)
        .tightening_threshold(max_width - 100)
        .child(&content)
        .build();
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&clamp)
        .build();
    (scroll, content)
}

pub fn spinner() -> gtk::Spinner {
    let s = gtk::Spinner::new();
    s.start();
    s
}

/// Uzun bir hatayı kullanıcıya göstermeden önce kısaltır.
pub fn error_text(e: &anyhow::Error) -> String {
    let s = e.to_string();
    if crate::client::is_offline(e) {
        crate::i18n::t("status.offline")
    } else if s.chars().count() > 300 {
        s.chars().take(300).collect::<String>() + "…"
    } else {
        s
    }
}

pub fn status_page(icon: &str, title: &str, description: &str) -> adw::StatusPage {
    adw::StatusPage::builder()
        .icon_name(icon)
        .title(glib::markup_escape_text(title).as_str())
        .description(glib::markup_escape_text(description).as_str())
        .vexpand(true)
        .build()
}
