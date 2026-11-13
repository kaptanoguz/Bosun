//! Ayarlar: dil, bildirimler, sohbet sağlayıcısı, Bosun'un yaptığı değişiklikler ve tanılama.
//! Ayarları servis kaydeder; API anahtarı yalnızca buradan servise gider, geri gelmez.

use crate::client;
use crate::dialogs;
use crate::i18n::{t, tv};
use crate::shell::shell;
use crate::ui;
use adw::prelude::*;
use bosun_core::admin::{Applied, HelperStatus};
use bosun_core::Lang;
use gtk::glib;
use serde::Deserialize;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Debug, Clone, Deserialize)]
struct ProviderInfo {
    id: String,
    label: String,
    default_model: String,
    has_key: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct Settings_ {
    language: Lang,
    #[serde(default)]
    theme: String,
    provider: String,
    model: String,
    #[serde(default)]
    model_override: String,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    providers: Vec<ProviderInfo>,
    chat_enabled: bool,
    share_file_paths: bool,
    notifications: bool,
    #[serde(default = "yes")]
    weekly_summary: bool,
    #[serde(default)]
    persist_chat: bool,
    config_path: String,
}

fn yes() -> bool {
    true
}

const LANGS: [(Lang, &str); 2] = [(Lang::En, "English"), (Lang::Tr, "Türkçe")];

/// Tema seçenekleri: kayıtlı değer ile görünen anahtar.
const THEMES: [(&str, &str); 3] = [
    ("system", "settings.themeSystem"),
    ("light", "settings.themeLight"),
    ("dark", "settings.themeDark"),
];

pub struct Settings {
    root: gtk::Stack,
    page: RefCell<adw::PreferencesPage>,
    settings: RefCell<Option<Settings_>>,
    dirty: Cell<bool>,
    /// Satırlar kurulurken tetiklenen bildirimleri yok saymak için.
    building: Cell<bool>,
}

impl Settings {
    pub fn new() -> Rc<Settings> {
        let page = adw::PreferencesPage::new();
        let root = gtk::Stack::new();
        let offline = ui::status_page(
            "network-offline-symbolic",
            &t("settings.title"),
            &t("settings.offline"),
        );
        root.add_named(&offline, Some("offline"));
        root.add_named(&page, Some("main"));
        root.set_visible_child_name("main");
        Rc::new(Settings {
            root,
            page: RefCell::new(page),
            settings: RefCell::new(None),
            dirty: Cell::new(true),
            building: Cell::new(false),
        })
    }

    pub fn widget(&self) -> gtk::Widget {
        self.root.clone().upcast()
    }

    pub fn on_show(self: &Rc<Self>) {
        if self.dirty.replace(false) {
            self.load();
        }
    }

    pub fn mark_dirty(self: &Rc<Self>) {
        self.dirty.set(true);
        if self.root.is_mapped() {
            self.on_show();
        }
    }

    fn load(self: &Rc<Self>) {
        let p = self.clone();
        glib::spawn_future_local(async move {
            let s = client::get::<Settings_>("settings.get", Value::Null).await;
            let applied = client::call("admin.applied", Value::Null).await.ok();
            match s {
                Ok(s) => {
                    *p.settings.borrow_mut() = Some(s);
                    p.root.set_visible_child_name("main");
                    p.render(applied);
                }
                Err(_) => {
                    p.dirty.set(true);
                    p.root.set_visible_child_name("offline");
                    p.render_offline();
                }
            }
        });
    }

    /// Sayfayı boş bir yenisiyle değiştirir.
    fn fresh_page(&self) -> adw::PreferencesPage {
        let old = self.page.borrow().clone();
        self.root.remove(&old);
        let page = adw::PreferencesPage::new();
        self.root.add_named(&page, Some("main"));
        self.root.set_visible_child_name("main");
        *self.page.borrow_mut() = page.clone();
        page
    }

    fn render_offline(self: &Rc<Self>) {
        // Servis kapalıyken de tanılama kopyalanabilsin.
        let page = adw::PreferencesPage::new();
        page.add(&self.diagnostics_group(None));
        self.root
            .remove(&self.root.child_by_name("offline").expect("offline page"));
        let status = ui::status_page(
            "network-offline-symbolic",
            &t("settings.title"),
            &t("settings.offline"),
        );
        let b = gtk::Box::new(gtk::Orientation::Vertical, 0);
        b.append(&status);
        b.append(&page);
        self.root.add_named(&b, Some("offline"));
        self.root.set_visible_child_name("offline");
    }

    fn render(self: &Rc<Self>, applied: Option<Value>) {
        let page = self.fresh_page();
        self.building.set(true);
        let s = self.settings.borrow().clone().expect("loaded");

        // Genel
        let general = ui::group("", "");
        let lang = adw::ComboRow::builder()
            .title(t("settings.language"))
            .subtitle(t("settings.languageHint"))
            .model(&gtk::StringList::new(&LANGS.map(|(_, l)| l)))
            .selected(
                LANGS
                    .iter()
                    .position(|(l, _)| *l == s.language)
                    .unwrap_or(0) as u32,
            )
            .build();
        {
            let p = Rc::downgrade(self);
            lang.connect_selected_notify(move |r| {
                if let Some(p) = p.upgrade() {
                    if !p.building.get() {
                        p.set_language(LANGS[r.selected() as usize].0);
                    }
                }
            });
        }
        general.add(&lang);
        let theme = adw::ComboRow::builder()
            .title(t("settings.theme"))
            .subtitle(t("settings.themeHint"))
            .model(&gtk::StringList::new(
                &THEMES
                    .map(|(_, key)| t(key))
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
            ))
            .selected(THEMES.iter().position(|(v, _)| *v == s.theme).unwrap_or(0) as u32)
            .build();
        {
            let p = Rc::downgrade(self);
            theme.connect_selected_notify(move |r| {
                if let Some(p) = p.upgrade() {
                    if !p.building.get() {
                        p.set_theme(THEMES[r.selected() as usize].0);
                    }
                }
            });
        }
        general.add(&theme);
        let notif = self.switch(
            "notifications",
            "settings.notifications",
            "settings.notificationsHint",
            s.notifications,
        );
        general.add(&notif);
        let weekly = self.switch(
            "weekly_summary",
            "settings.weeklySummary",
            "settings.weeklySummaryHint",
            s.weekly_summary && s.notifications,
        );
        weekly.set_sensitive(s.notifications);
        general.add(&weekly);
        general.add(&self.switch(
            "persist_chat",
            "settings.persistChat",
            "settings.persistChatHint",
            s.persist_chat,
        ));
        let clear = ui::row(&t("settings.clearChat"), &t("settings.clearChatHint"));
        let clear_btn = ui::button(&t("chat.clearConfirm"));
        clear_btn.add_css_class("destructive-action");
        clear_btn.set_valign(gtk::Align::Center);
        clear_btn.connect_clicked(|_| dialogs::clear_chat());
        clear.add_suffix(&clear_btn);
        general.add(&clear);
        page.add(&general);

        page.add(&self.ai_group(&s));

        // Bosun'un yaptığı değişiklikler
        if let Some(a) = applied {
            let g = ui::group(&t("settings.applied"), &t("settings.appliedHint"));
            let list: Vec<Applied> =
                serde_json::from_value(a["applied"].clone()).unwrap_or_default();
            if list.is_empty() {
                g.add(&ui::row(&t("settings.appliedNone"), ""));
            }
            for x in list {
                let r = ui::row(&x.title, "");
                let img = gtk::Image::from_icon_name("emblem-ok-symbolic");
                img.add_css_class("success");
                r.add_prefix(&img);
                let undo = ui::button(&t("settings.undo"));
                let action = x.undo_action.clone();
                undo.connect_clicked(move |_| dialogs::admin(&action, vec![]));
                r.add_suffix(&undo);
                g.add(&r);
            }
            let helper = bosun_core::admin::helper_status();
            let ok = helper == HelperStatus::Ok;
            let e = ui::expander(
                &t("settings.helper"),
                &t(if ok {
                    "settings.helperOk"
                } else {
                    "settings.helperMissing"
                }),
            );
            e.set_subtitle_lines(0);
            let img = gtk::Image::from_icon_name(if ok {
                "emblem-ok-symbolic"
            } else {
                "dialog-warning-symbolic"
            });
            img.add_css_class(if ok { "success" } else { "warning" });
            e.add_prefix(&img);
            e.set_enable_expansion(!ok);
            if !ok {
                let n = dialogs::helper_notice(helper);
                n.set_margin_top(10);
                n.set_margin_bottom(10);
                n.set_margin_start(12);
                n.set_margin_end(12);
                e.add_row(&n);
            }
            g.add(&e);
            page.add(&g);
        }

        page.add(&self.diagnostics_group(Some(&s.config_path)));
        self.building.set(false);
    }

    fn switch(
        self: &Rc<Self>,
        key: &'static str,
        title: &str,
        hint: &str,
        active: bool,
    ) -> adw::SwitchRow {
        let r = adw::SwitchRow::builder()
            .title(t(title))
            .subtitle(t(hint))
            .active(active)
            .build();
        let p = Rc::downgrade(self);
        r.connect_active_notify(move |r| {
            let Some(p) = p.upgrade() else { return };
            if p.building.get() {
                return;
            }
            let (value, row) = (r.is_active(), r.clone());
            let p = p.clone();
            glib::spawn_future_local(async move {
                match client::get::<Settings_>("settings.set", json!({ key: value })).await {
                    Ok(s) => {
                        let notif = s.notifications;
                        *p.settings.borrow_mut() = Some(s);
                        // Haftalık hatırlatma bildirimlere bağlı.
                        if key == "notifications" {
                            p.dirty.set(true);
                            p.on_show();
                        }
                        let _ = notif;
                    }
                    Err(e) => {
                        shell().toast(&ui::error_text(&e));
                        p.building.set(true);
                        row.set_active(!value);
                        p.building.set(false);
                    }
                }
            });
        });
        r
    }

    fn set_theme(self: &Rc<Self>, theme: &'static str) {
        glib::spawn_future_local(async move {
            match client::get::<Settings_>("settings.set", json!({ "theme": theme })).await {
                Ok(_) => {
                    crate::apply_theme(theme);
                    shell().toast(&t("settings.saved"));
                }
                Err(e) => shell().toast(&ui::error_text(&e)),
            }
        });
    }

    fn set_language(self: &Rc<Self>, l: Lang) {
        glib::spawn_future_local(async move {
            match client::get::<Settings_>("settings.set", json!({ "language": l })).await {
                Ok(s) => {
                    let s_ = shell();
                    s_.set_language(s.language);
                    s_.toast(&t("settings.saved"));
                }
                Err(e) => shell().toast(&ui::error_text(&e)),
            }
        });
    }

    fn ai_group(self: &Rc<Self>, s: &Settings_) -> adw::PreferencesGroup {
        let desc = if s.chat_enabled {
            tv("settings.chatOn", &[("model", s.model.clone().into())])
        } else {
            t("settings.chatOff")
        };
        let g = ui::group(&t("settings.chat"), &desc);
        let names: Vec<String> = s
            .providers
            .iter()
            .map(|p| {
                let name = if p.id == "custom" {
                    t("ai.custom")
                } else {
                    p.label.clone()
                };
                if p.has_key {
                    format!("{name} ✓")
                } else {
                    name
                }
            })
            .collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let current = s
            .providers
            .iter()
            .position(|p| p.id == s.provider)
            .unwrap_or(0);
        let provider = adw::ComboRow::builder()
            .title(t("ai.provider"))
            .subtitle(t("ai.providerHint"))
            .model(&gtk::StringList::new(&names))
            .selected(current as u32)
            .build();
        g.add(&provider);
        let base_url = adw::EntryRow::builder()
            .title(t("ai.baseUrl"))
            .text(s.base_url.clone().unwrap_or_default())
            .build();
        base_url.set_tooltip_text(Some(&t("ai.baseUrlHint")));
        g.add(&base_url);
        let model = adw::EntryRow::builder().text(&s.model_override).build();
        g.add(&model);
        let key = adw::PasswordEntryRow::builder().title(t("ai.key")).build();
        g.add(&key);
        let status = ui::row("", "");
        status.set_subtitle_lines(0);
        let remove = ui::button(&t("ai.removeKey"));
        remove.add_css_class("flat");
        let save = ui::button(&t("ai.save"));
        save.add_css_class("suggested-action");
        status.add_suffix(&remove);
        status.add_suffix(&save);
        g.add(&status);
        g.add(&self.switch(
            "share_file_paths",
            "settings.privacy",
            "settings.privacyHint",
            s.share_file_paths,
        ));

        let providers = Rc::new(s.providers.clone());
        let active = (s.provider.clone(), s.model.clone());
        // Seçilen sağlayıcıya göre alanları ve açıklamaları güncelle.
        let sync = {
            let (providers, base_url, model, key, status, remove) = (
                providers.clone(),
                base_url.clone(),
                model.clone(),
                key.clone(),
                status.clone(),
                remove.clone(),
            );
            move |i: usize, reset: bool| {
                let Some(info) = providers.get(i) else { return };
                let custom = info.id == "custom";
                base_url.set_visible(custom);
                model.set_title(&format!(
                    "{} · {}",
                    t("ai.model"),
                    tv(
                        "ai.modelHint",
                        &[("model", info.default_model.clone().into())]
                    )
                ));
                if reset {
                    model.set_text("");
                    base_url.set_text("");
                }
                key.set_text("");
                remove.set_visible(info.has_key);
                let state = if info.has_key {
                    t("ai.keySaved")
                } else if custom {
                    t("ai.keyOptional")
                } else {
                    t("ai.keyMissing")
                };
                status.set_title(&state);
                let label = if custom {
                    t("ai.custom")
                } else {
                    info.label.clone()
                };
                status.set_subtitle(&if info.id == active.0 {
                    format!(
                        "{} {}",
                        t("ai.keyPrivacy"),
                        tv(
                            "ai.active",
                            &[("label", label.into()), ("model", active.1.clone().into())]
                        )
                    )
                } else {
                    t("ai.keyPrivacy")
                });
            }
        };
        sync(current, false);
        {
            let sync = sync.clone();
            provider.connect_selected_notify(move |r| sync(r.selected() as usize, true));
        }

        let save_with = {
            let (providers, provider, base_url, model, key) = (
                providers.clone(),
                provider.clone(),
                base_url.clone(),
                model.clone(),
                key.clone(),
            );
            let p = Rc::downgrade(self);
            move |remove_key: bool| {
                let Some(info) = providers.get(provider.selected() as usize) else {
                    return;
                };
                let mut params = json!({
                    "provider": info.id,
                    "model": model.text().trim(),
                    "base_url": base_url.text().trim(),
                });
                let k = key.text().trim().to_string();
                if remove_key {
                    params["api_key"] = json!("");
                } else if !k.is_empty() {
                    params["api_key"] = json!(k);
                }
                let Some(p) = p.upgrade() else { return };
                glib::spawn_future_local(async move {
                    match client::get::<Settings_>("settings.set", params).await {
                        Ok(s) => {
                            *p.settings.borrow_mut() = Some(s);
                            let sh = shell();
                            sh.toast(&t("settings.saved"));
                            // Sohbet ekranı sağlayıcının durumunu "hello" ile öğrenir.
                            if let Ok(h) = client::get("hello", Value::Null).await {
                                sh.data.borrow_mut().hello = Some(h);
                                crate::shell::ui().chat.refresh();
                            }
                            p.dirty.set(true);
                            p.on_show();
                        }
                        Err(e) => shell().toast(&ui::error_text(&e)),
                    }
                });
            }
        };
        {
            let f = save_with.clone();
            save.connect_clicked(move |_| f(false));
        }
        remove.connect_clicked(move |_| save_with(true));
        g
    }

    fn diagnostics_group(self: &Rc<Self>, config_path: Option<&str>) -> adw::PreferencesGroup {
        let g = ui::group("", "");
        if let Some(path) = config_path {
            let e = ui::expander(&t("settings.configFile"), &t("settings.configHint"));
            e.set_subtitle_lines(0);
            let b = gtk::Box::new(gtk::Orientation::Vertical, 6);
            b.set_margin_top(10);
            b.set_margin_bottom(10);
            b.set_margin_start(12);
            b.set_margin_end(12);
            b.append(&ui::command(path));
            b.append(&ui::command("systemctl --user restart bosund"));
            e.add_row(&b);
            g.add(&e);
        }
        let r = ui::row(&t("settings.diagnostics"), &t("settings.diagnosticsHint"));
        r.set_subtitle_lines(0);
        let copy = ui::button(&t("settings.copyDiagnostics"));
        copy.connect_clicked(|b| {
            let b = b.clone();
            b.set_sensitive(false);
            glib::spawn_future_local(async move {
                let text = client::diagnostics().await;
                b.clipboard().set_text(&text);
                b.set_sensitive(true);
                shell().toast(&t("settings.copiedDiagnostics"));
            });
        });
        r.add_suffix(&copy);
        g.add(&r);
        g
    }
}
