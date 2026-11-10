//! Pencere, kenar çubuğu ve servisten gelen durum.
//!
//! Tüm durum tek bir yerde (`Data`) tutulur; servisten gelen her olay önce onu günceller,
//! sonra ilgili ekranı yeniler. Dil değişince arayüz baştan kurulur.

use crate::client::{self, Incoming};
use crate::i18n::{self, t, tv};
use crate::pages::{chat, cleanup, disk, history, overview, settings, startup};
use crate::ui;
use adw::prelude::*;
use bosun_core::containers::Summary;
use bosun_core::metrics::Snapshot;
use bosun_core::rules::{Finding, Severity};
use bosun_core::scan::ScanResult;
use bosun_core::{advice::Advice, Lang};
use gtk::glib;
use serde::Deserialize;
use serde_json::{json, Value};
use std::cell::{Cell, OnceCell, RefCell};
use std::rc::Rc;

/// Son bir saat: 15 saniyede bir ölçümle 240 nokta.
const HISTORY_POINTS: usize = 240;
/// Önerilen temizlik bu boyutu geçince kenar çubuğunda gösterilir.
const NAV_RECLAIM_MIN: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize)]
pub struct Hello {
    pub version: String,
    pub host: bosun_core::metrics::HostInfo,
    #[serde(default)]
    pub chat_enabled: bool,
    #[serde(default)]
    pub language: Option<Lang>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FindingsPayload {
    pub findings: Vec<Finding>,
    pub advice: Vec<Advice>,
    pub overall: Option<Severity>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    pub title: String,
}

/// Sohbetteki bir mesaj.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: Role,
    pub text: String,
    pub plans: Vec<bosun_core::CleanupPlan>,
    pub tools: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    Error,
}

#[derive(Default)]
pub struct Data {
    pub connected: Option<bool>,
    pub hello: Option<Hello>,
    pub latest: Option<Snapshot>,
    pub history: Vec<Snapshot>,
    pub scan: Option<ScanResult>,
    pub scanning: bool,
    pub progress: Option<Progress>,
    pub findings: FindingsPayload,
    pub containers: Option<Summary>,
    pub messages: Vec<ChatMessage>,
}

impl Data {
    /// Güvenli ve toplu seçilebilir kategorilerde bulunan toplam.
    pub fn reclaimable(&self) -> u64 {
        self.scan
            .as_ref()
            .map(|s| {
                s.categories
                    .iter()
                    .filter(|c| c.bulk_selectable && c.safety == bosun_core::Safety::Safe)
                    .map(|c| c.bytes)
                    .sum()
            })
            .unwrap_or(0)
    }

    pub fn active_warnings(&self) -> usize {
        self.findings
            .findings
            .iter()
            .filter(|f| f.severity != Severity::Info && !f.muted)
            .count()
    }

    pub fn chat_enabled(&self) -> bool {
        self.hello.as_ref().is_some_and(|h| h.chat_enabled)
    }
}

pub const VIEWS: [&str; 7] = [
    "overview", "cleanup", "disk", "startup", "chat", "history", "settings",
];

fn view_icon(id: &str) -> &'static str {
    match id {
        "overview" => "computer-symbolic",
        "cleanup" => "edit-clear-all-symbolic",
        "disk" => "drive-harddisk-symbolic",
        "startup" => "system-run-symbolic",
        "chat" => "chat-message-new-symbolic",
        "history" => "document-open-recent-symbolic",
        _ => "emblem-system-symbolic",
    }
}

/// Dil değişince baştan kurulan her şey.
pub struct Ui {
    split: adw::NavigationSplitView,
    content_page: adw::NavigationPage,
    stack: gtk::Stack,
    main: gtk::Stack,
    nav: gtk::ListBox,
    counts: Vec<(&'static str, gtk::Label)>,
    status_icon: gtk::Image,
    status_text: gtk::Label,
    status_live: gtk::Label,
    host: gtk::Label,
    offline_banner: adw::Banner,
    version_banner: adw::Banner,
    toasts: adw::ToastOverlay,
    pub overview: Rc<overview::Overview>,
    pub cleanup: Rc<cleanup::Cleanup>,
    pub disk: Rc<disk::Disk>,
    pub startup: Rc<startup::Startup>,
    pub chat: Rc<chat::Chat>,
    pub history: Rc<history::History>,
    pub settings: Rc<settings::Settings>,
}

pub struct Shell {
    pub window: adw::ApplicationWindow,
    split: adw::NavigationSplitView,
    pub data: RefCell<Data>,
    ui: RefCell<Option<Rc<Ui>>>,
    view: RefCell<&'static str>,
    lang: Cell<Lang>,
}

thread_local! {
    static SHELL: OnceCell<Rc<Shell>> = const { OnceCell::new() };
}

/// Uygulamanın tek penceresi.
pub fn shell() -> Rc<Shell> {
    SHELL.with(|s| s.get().expect("shell is created at startup").clone())
}

pub fn ui() -> Rc<Ui> {
    shell().ui.borrow().clone().expect("ui is built at startup")
}

pub fn build(app: &adw::Application) {
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Bosun")
        .icon_name("bosun")
        .default_width(1120)
        .default_height(780)
        .width_request(360)
        .height_request(400)
        .build();
    let split = adw::NavigationSplitView::builder()
        .min_sidebar_width(220.0)
        .max_sidebar_width(260.0)
        .build();
    // Dar pencerede kenar çubuğu ayrı bir sayfaya katlanır.
    let bp = adw::Breakpoint::new(
        adw::BreakpointCondition::parse("max-width: 680sp").expect("valid breakpoint"),
    );
    bp.add_setter(&split, "collapsed", Some(&true.to_value()));
    window.add_breakpoint(bp);
    window.set_content(Some(&split));
    // Açık ve koyu görünümün tonları style.css'te bu sınıflara bağlı.
    let style = adw::StyleManager::default();
    let tone = {
        let window = window.clone();
        move |sm: &adw::StyleManager| {
            let (on, off) = if sm.is_dark() {
                ("bosun-dark", "bosun-light")
            } else {
                ("bosun-light", "bosun-dark")
            };
            window.remove_css_class(off);
            window.add_css_class(on);
        }
    };
    tone(&style);
    style.connect_dark_notify(tone);
    let sh = Rc::new(Shell {
        window,
        split,
        data: RefCell::new(Data::default()),
        ui: RefCell::new(None),
        view: RefCell::new("overview"),
        lang: Cell::new(i18n::lang()),
    });
    SHELL.with(|s| {
        let _ = s.set(sh.clone());
    });
    // Geliştirme ve ekran görüntüleri için: BOSUN_VIEW=cleanup ile o ekranda açılır.
    if let Some(v) = std::env::var("BOSUN_VIEW")
        .ok()
        .and_then(|v| VIEWS.iter().copied().find(|x| *x == v))
    {
        *sh.view.borrow_mut() = v;
    }
    sh.rebuild();
    sh.window.present();

    // Olay akışı: bağlantı durumu ve servis olayları.
    let rx = client::subscribe();
    glib::spawn_future_local(async move {
        while let Ok(m) = rx.recv().await {
            shell().on_incoming(m);
        }
    });
    glib::spawn_future_local(async { shell().reload().await });
    // "Son ölçüm 1 dk önce" gibi göreli zamanları tazele.
    glib::timeout_add_seconds_local(30, || {
        let s = shell();
        s.update_status();
        let v = ui();
        v.overview.update_subtitle();
        v.cleanup.update_last_scan();
        glib::ControlFlow::Continue
    });
}

impl Shell {
    /// Arayüzü (geçerli dilde) baştan kurar; açık ekran korunur.
    pub fn rebuild(&self) {
        self.lang.set(i18n::lang());
        let ui = Rc::new(build_ui(&self.split));
        *self.ui.borrow_mut() = Some(ui.clone());
        let view = *self.view.borrow();
        self.select(view);
        ui.overview.update_all();
        ui.cleanup.rebuild();
        self.update_status();
    }

    pub fn select(&self, id: &str) {
        let Some(id) = VIEWS.iter().copied().find(|v| *v == id) else {
            return;
        };
        *self.view.borrow_mut() = id;
        let ui = ui();
        ui.stack.set_visible_child_name(id);
        ui.content_page.set_title(&t(&format!("nav.{id}")));
        if let Some(row) = ui
            .nav
            .row_at_index(VIEWS.iter().position(|v| *v == id).unwrap_or(0) as i32)
        {
            if ui.nav.selected_row().as_ref() != Some(&row) {
                ui.nav.select_row(Some(&row));
            }
        }
        ui.split.set_show_content(true);
        match id {
            "disk" => ui.disk.on_show(),
            "startup" => ui.startup.on_show(),
            "history" => ui.history.on_show(),
            "settings" => ui.settings.on_show(),
            "chat" => ui.chat.on_show(),
            _ => {}
        }
    }

    pub fn toast(&self, text: &str) {
        let toast = adw::Toast::new(text);
        toast.set_timeout(4);
        ui().toasts.add_toast(toast);
    }

    /// Bağlantı kurulunca (ya da yeniden kurulunca) her şeyi yeniden yükler.
    pub async fn reload(&self) {
        let res = (
            client::get::<Hello>("hello", Value::Null).await,
            client::call("snapshot", Value::Null).await,
            client::get::<FindingsPayload>("findings", Value::Null).await,
        );
        let (hello, snap, findings) = match res {
            (Ok(h), Ok(s), Ok(f)) => (h, s, f),
            _ => {
                self.data.borrow_mut().connected = Some(false);
                self.update_status();
                return;
            }
        };
        if let Some(l) = hello.language {
            self.set_language(l);
        }
        {
            let mut d = self.data.borrow_mut();
            d.hello = Some(hello);
            d.latest = serde_json::from_value(snap["latest"].clone()).ok();
            d.history = serde_json::from_value(snap["history"].clone()).unwrap_or_default();
            d.findings = findings;
            d.connected = Some(true);
        }
        {
            let v = ui();
            v.overview.update_all();
            v.chat.refresh();
        }
        self.update_status();

        if let Ok(scan) =
            client::get::<ScanResult>("scan", json!({"refresh": false, "limit": 40})).await
        {
            self.data.borrow_mut().scan = Some(scan);
            ui().cleanup.rebuild();
            self.update_status();
        }
        if let Ok(c) = client::call("containers", Value::Null).await {
            self.data.borrow_mut().containers =
                serde_json::from_value(c["summary"].clone()).ok().flatten();
            ui().overview.update_containers();
        }
        // Servis yeniden başladıysa kayıtlı sohbeti geri getir (ekranda sohbet yoksa).
        if let Ok(h) = client::call("chat.history", json!({"conversation_id": "main"})).await {
            let restored: Vec<ChatMessage> = h["messages"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|m| {
                            let role = match m["role"].as_str()? {
                                "user" => Role::User,
                                "assistant" => Role::Assistant,
                                _ => return None,
                            };
                            Some(ChatMessage {
                                role,
                                text: m["text"].as_str()?.to_string(),
                                plans: vec![],
                                tools: vec![],
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            let mut d = self.data.borrow_mut();
            if d.messages.is_empty() && !restored.is_empty() {
                d.messages = restored;
                drop(d);
                ui().chat.refresh();
            }
        }
    }

    pub fn set_language(&self, l: Lang) {
        if l != self.lang.get() {
            i18n::set_lang(l);
            self.rebuild();
        }
    }

    fn on_incoming(&self, m: Incoming) {
        match m {
            Incoming::Connected(up) => {
                self.data.borrow_mut().connected = Some(up);
                self.update_status();
                if up {
                    glib::spawn_future_local(async { shell().reload().await });
                }
            }
            Incoming::Event(name, data) => self.on_event(&name, data),
        }
    }

    fn on_event(&self, name: &str, data: Value) {
        let v = ui();
        match name {
            "metrics" => {
                let Ok(s) = serde_json::from_value::<Snapshot>(data) else {
                    return;
                };
                {
                    let mut d = self.data.borrow_mut();
                    if d.history.len() >= HISTORY_POINTS {
                        let n = d.history.len() + 1 - HISTORY_POINTS;
                        d.history.drain(..n);
                    }
                    // Geçmiş yalnızca eğri için; süreç listesi en son ölçümde tutulur.
                    let mut h = s.clone();
                    h.top_apps.clear();
                    d.history.push(h);
                    d.latest = Some(s);
                }
                v.overview.update_metrics();
                self.update_status();
            }
            "findings" => {
                if let Ok(f) = serde_json::from_value(data) {
                    self.data.borrow_mut().findings = f;
                    v.overview.update_findings();
                    v.chat.refresh_starters();
                    self.update_status();
                }
            }
            "containers" => {
                self.data.borrow_mut().containers = serde_json::from_value(data["summary"].clone())
                    .ok()
                    .flatten();
                v.overview.update_containers();
            }
            "scan_started" | "scan_progress" => {
                {
                    let mut d = self.data.borrow_mut();
                    d.scanning = true;
                    d.progress = serde_json::from_value(data).ok();
                }
                v.cleanup.update_progress();
            }
            "scan_cancelled" => {
                {
                    let mut d = self.data.borrow_mut();
                    d.scanning = false;
                    d.progress = None;
                }
                v.cleanup.update_progress();
            }
            "scan" => {
                if let Ok(s) = serde_json::from_value(data) {
                    let mut d = self.data.borrow_mut();
                    d.scan = Some(s);
                    d.scanning = false;
                    d.progress = None;
                }
                v.cleanup.rebuild();
                v.overview.update_subtitle();
                v.disk.mark_cleanable();
                self.update_status();
            }
            "plan_executed" => v.history.mark_dirty(),
            "chat_delta" => {
                if data["conversation_id"] == "main" {
                    v.chat.on_delta(
                        data["turn"].as_u64().unwrap_or(0),
                        data["text"].as_str().unwrap_or(""),
                    );
                }
            }
            "chat_tool" => {
                if data["conversation_id"] == "main" {
                    v.chat.on_tool(data["name"].as_str().unwrap_or(""));
                }
            }
            "settings" => {
                if let Some(l) = data["language"].as_str().and_then(Lang::parse) {
                    self.set_language(l);
                }
            }
            _ => {}
        }
    }

    /// Kenar çubuğundaki durum, sayaçlar ve şeritler.
    pub fn update_status(&self) {
        let ui = ui();
        let d = self.data.borrow();
        let offline = d.connected == Some(false);
        let (icon, key, class) = if offline {
            ("network-offline-symbolic", "status.offline", "dim-label")
        } else {
            match d.findings.overall {
                Some(Severity::Critical) => ("dialog-error-symbolic", "status.critical", "error"),
                Some(Severity::Warning) => ("dialog-warning-symbolic", "status.warning", "warning"),
                _ => ("emblem-ok-symbolic", "status.ok", "success"),
            }
        };
        ui.status_icon.set_icon_name(Some(icon));
        for c in ["dim-label", "error", "warning", "success"] {
            ui.status_icon.remove_css_class(c);
        }
        ui.status_icon.add_css_class(class);
        ui.status_text.set_label(&t(key));
        match (&d.latest, offline) {
            (Some(s), false) => {
                ui.status_live
                    .set_label(&tv("status.live", &[("ago", i18n::ago(s.ts).into())]));
                ui.status_live.set_visible(true);
            }
            _ => ui.status_live.set_visible(false),
        }
        if let Some(h) = &d.hello {
            ui.host.set_label(&format!(
                "{}\n{}\nBosun {}",
                h.host.hostname, h.host.os, h.version
            ));
        }

        let warn = d.active_warnings();
        let reclaim = d.reclaimable();
        for (id, l) in &ui.counts {
            let text = match *id {
                "overview" if warn > 0 => warn.to_string(),
                "cleanup" if reclaim > NAV_RECLAIM_MIN => i18n::bytes(reclaim),
                _ => String::new(),
            };
            l.set_visible(!text.is_empty());
            l.set_label(&text);
        }

        ui.offline_banner
            .set_revealed(offline && d.latest.is_some());
        let mismatch = !offline
            && d.hello
                .as_ref()
                .is_some_and(|h| h.version != env!("CARGO_PKG_VERSION"));
        if let (true, Some(h)) = (mismatch, &d.hello) {
            ui.version_banner.set_title(&tv(
                "version.mismatch",
                &[
                    ("service", h.version.clone().into()),
                    ("app", env!("CARGO_PKG_VERSION").into()),
                ],
            ));
        }
        ui.version_banner.set_revealed(mismatch);
        // Hiç veri yokken servis kapalıysa, ekranların yerine "servisi başlat" sayfası.
        let blank = offline && d.latest.is_none() && *self.view.borrow() != "settings";
        ui.main
            .set_visible_child_name(if blank { "offline" } else { "views" });
    }
}

fn start_service(error: gtk::Label) {
    glib::spawn_future_local(async move {
        error.set_visible(false);
        match client::start_daemon().await {
            Ok(()) => shell().toast(&t("offline.starting")),
            Err(e) => {
                error.set_label(&e.to_string());
                error.set_visible(true);
            }
        }
    });
}

fn offline_page() -> gtk::Widget {
    let page = ui::status_page(
        "network-offline-symbolic",
        &t("offline.title"),
        &t("offline.body"),
    );
    page.set_icon_name(Some("bosun"));
    let b = gtk::Box::new(gtk::Orientation::Vertical, 12);
    b.set_halign(gtk::Align::Center);
    let start = ui::pill(&t("offline.start"), "suggested-action");
    start.set_halign(gtk::Align::Center);
    let error = ui::label("");
    error.add_css_class("error");
    error.set_visible(false);
    {
        let error = error.clone();
        start.connect_clicked(move |_| start_service(error.clone()));
    }
    b.append(&start);
    b.append(&error);
    let or = ui::dim_label(&t("offline.orTerminal"));
    or.set_margin_top(12);
    b.append(&or);
    b.append(&ui::command("systemctl --user enable --now bosund"));
    page.set_child(Some(&b));
    page.upcast()
}

fn nav_row(id: &'static str) -> (gtk::ListBoxRow, gtk::Label) {
    let b = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    b.append(&gtk::Image::from_icon_name(view_icon(id)));
    let l = gtk::Label::builder()
        .label(t(&format!("nav.{id}")))
        .xalign(0.0)
        .hexpand(true)
        .build();
    b.append(&l);
    let count = gtk::Label::new(None);
    count.add_css_class("nav-count");
    count.add_css_class("numeric");
    count.set_visible(false);
    b.append(&count);
    let row = gtk::ListBoxRow::builder().child(&b).build();
    row.set_widget_name(id);
    (row, count)
}

fn build_ui(split: &adw::NavigationSplitView) -> Ui {
    // Kenar çubuğu
    let nav = gtk::ListBox::new();
    nav.add_css_class("navigation-sidebar");
    let mut counts = vec![];
    for id in VIEWS {
        let (row, count) = nav_row(id);
        nav.append(&row);
        counts.push((id, count));
    }
    nav.connect_row_activated(|_, row| {
        let name = row.widget_name();
        shell().select(name.as_str());
    });

    let status_icon = gtk::Image::from_icon_name("emblem-ok-symbolic");
    let status_text = gtk::Label::builder().xalign(0.0).build();
    status_text.add_css_class("heading");
    let status_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    status_row.append(&status_icon);
    status_row.append(&status_text);
    let status_live = ui::dim_label("");
    status_live.add_css_class("caption");
    let host = ui::dim_label("");
    host.add_css_class("caption");
    host.set_margin_top(6);
    let foot = gtk::Box::new(gtk::Orientation::Vertical, 4);
    foot.add_css_class("side-foot");
    foot.append(&status_row);
    foot.append(&status_live);
    foot.append(&host);

    let side_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let nav_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&nav)
        .build();
    side_box.append(&nav_scroll);
    side_box.append(&foot);
    let side_tv = adw::ToolbarView::new();
    let side_header = adw::HeaderBar::new();
    side_header.set_title_widget(Some(&adw::WindowTitle::new("Bosun", "")));
    side_tv.add_top_bar(&side_header);
    side_tv.set_content(Some(&side_box));
    let sidebar = adw::NavigationPage::builder()
        .title("Bosun")
        .tag("sidebar")
        .child(&side_tv)
        .build();

    // Ekranlar
    let overview = overview::Overview::new();
    let cleanup = cleanup::Cleanup::new();
    let disk = disk::Disk::new();
    let startup = startup::Startup::new();
    let chat = chat::Chat::new();
    let history = history::History::new();
    let settings = settings::Settings::new();
    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .transition_duration(120)
        .build();
    stack.add_named(&overview.widget(), Some("overview"));
    stack.add_named(&cleanup.widget(), Some("cleanup"));
    stack.add_named(&disk.widget(), Some("disk"));
    stack.add_named(&startup.widget(), Some("startup"));
    stack.add_named(&chat.widget(), Some("chat"));
    stack.add_named(&history.widget(), Some("history"));
    stack.add_named(&settings.widget(), Some("settings"));

    let main = gtk::Stack::new();
    main.add_named(&stack, Some("views"));
    main.add_named(&offline_page(), Some("offline"));

    let offline_banner = adw::Banner::builder()
        .title(t("offline.banner"))
        .button_label(t("offline.start"))
        .build();
    offline_banner.connect_button_clicked(|_| {
        glib::spawn_future_local(async {
            if let Err(e) = client::start_daemon().await {
                shell().toast(&e.to_string());
            }
        });
    });
    let version_banner = adw::Banner::builder()
        .button_label(t("version.restart"))
        .build();
    version_banner.connect_button_clicked(|_| {
        glib::spawn_future_local(async {
            match client::restart_daemon().await {
                Ok(()) => {
                    glib::timeout_future_seconds(2).await;
                    shell().reload().await;
                }
                Err(e) => shell().toast(&e.to_string()),
            }
        });
    });

    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&main));
    let content_tv = adw::ToolbarView::new();
    content_tv.add_top_bar(&adw::HeaderBar::new());
    content_tv.add_top_bar(&offline_banner);
    content_tv.add_top_bar(&version_banner);
    content_tv.set_content(Some(&toasts));
    let content_page = adw::NavigationPage::builder()
        .title(t("nav.overview"))
        .tag("content")
        .child(&content_tv)
        .build();

    split.set_sidebar(Some(&sidebar));
    split.set_content(Some(&content_page));
    let split = split.clone();

    Ui {
        split,
        content_page,
        stack,
        main,
        nav,
        counts,
        status_icon,
        status_text,
        status_live,
        host,
        offline_banner,
        version_banner,
        toasts,
        overview,
        cleanup,
        disk,
        startup,
        chat,
        history,
        settings,
    }
}

impl Ui {
    pub fn add_toast(&self, toast: adw::Toast) {
        self.toasts.add_toast(toast);
    }
}
