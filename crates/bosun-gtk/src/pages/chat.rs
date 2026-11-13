//! Sohbet: Bosun canlı ölçümlerle yanıt verir. Temizlik önerirse bir onay kartı çıkar;
//! sohbetin kendisi hiçbir şeyi uygulayamaz.

use crate::client;
use crate::dialogs;
use crate::i18n::{self, t, tv};
use crate::shell::{shell, ChatMessage, Role};
use crate::ui;
use adw::prelude::*;
use bosun_core::rules::Finding;
use bosun_core::CleanupPlan;
use gtk::glib;
use serde_json::json;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

const KNOWN_TOOLS: [&str; 6] = [
    "get_system_snapshot",
    "get_cleanup_candidates",
    "get_findings",
    "get_large_files",
    "get_startup_apps",
    "propose_cleanup",
];
const MAX_STARTERS: usize = 4;

fn tool_name(id: &str) -> String {
    if KNOWN_TOOLS.contains(&id) {
        t(&format!("chat.tool.{id}"))
    } else {
        id.to_string()
    }
}

/// Açılış önerileri: o an görülen bulgulardan, yetmezse genel sorularla.
pub fn starters(findings: &[Finding]) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for f in findings.iter().filter(|f| !f.muted) {
        let q = if let Some(mount) = f.id.strip_prefix("disk:") {
            let m = if mount == "/" {
                t("chat.systemDisk")
            } else {
                mount.to_string()
            };
            tv("chat.askDisk", &[("mount", m.into())])
        } else if let Some(app) = f.id.strip_prefix("leak:") {
            tv("chat.askLeak", &[("app", app.into())])
        } else if f.id.starts_with("units:") {
            tv("chat.askUnits", &[("units", f.detail.clone().into())])
        } else {
            match f.id.as_str() {
                "cpu" => t("chat.askCpu"),
                "mem" | "swap" => t("chat.askMemory"),
                "temp" => t("chat.askHeat"),
                "reclaimable" => t("chat.askReclaim"),
                _ => continue,
            }
        };
        out.push(q);
    }
    for k in ["chat.s1", "chat.s3", "chat.s4", "chat.s2"] {
        out.push(t(k));
    }
    let mut seen = std::collections::HashSet::new();
    out.retain(|q| seen.insert(q.clone()));
    out.truncate(MAX_STARTERS);
    out
}

/// `kod` → eş aralıklı, **kalın** → kalın. Geri kalan her şey kaçışlanır.
pub fn markup(line: &str) -> String {
    let mut out = String::new();
    for (i, part) in line.split('`').enumerate() {
        if i % 2 == 1 {
            out += &format!("<tt>{}</tt>", glib::markup_escape_text(part));
            continue;
        }
        for (j, p) in part.split("**").enumerate() {
            let e = glib::markup_escape_text(p);
            if j % 2 == 1 {
                out += &format!("<b>{e}</b>");
            } else {
                out += &e;
            }
        }
    }
    out
}

/// Bir yanıtı satırlarına ayırır; tek başına duran `komut` satırları kopyalanabilir olur.
fn rich_text(text: &str) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let mut para: Vec<String> = vec![];
    let flush = |para: &mut Vec<String>, b: &gtk::Box| {
        if para.is_empty() {
            return;
        }
        let l = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .selectable(true)
            .use_markup(true)
            .build();
        l.set_markup(
            &para
                .iter()
                .map(|x| markup(x))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        b.append(&l);
        para.clear();
    };
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.len() > 2
            && trimmed.starts_with('`')
            && trimmed.ends_with('`')
            && trimmed.matches('`').count() == 2
        {
            flush(&mut para, &b);
            b.append(&ui::command(&trimmed[1..trimmed.len() - 1]));
        } else if trimmed.starts_with("```") {
            continue; // kod bloğu çitleri
        } else {
            para.push(line.to_string());
        }
    }
    flush(&mut para, &b);
    b
}

fn plan_card(p: &CleanupPlan) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
    card.add_css_class("card");
    card.add_css_class("plan-card");
    let h = ui::label(&tv(
        "chat.proposal",
        &[("size", i18n::bytes(p.total_bytes).into())],
    ));
    h.add_css_class("heading");
    card.append(&h);
    if let Some(r) = &p.reason {
        card.append(&ui::label(r));
    }
    let sum = p
        .summary
        .iter()
        .map(|s| format!("{} ({})", s.title, i18n::bytes(s.bytes)))
        .collect::<Vec<_>>()
        .join(", ");
    card.append(&ui::dim_label(&sum));
    let b = ui::button(&t("chat.reviewApprove"));
    b.add_css_class("suggested-action");
    b.set_halign(gtk::Align::Start);
    b.set_sensitive(!p.items.is_empty());
    let plan = p.clone();
    b.connect_clicked(move |_| dialogs::plan(plan.clone()));
    card.append(&b);
    card
}

fn bubble(m: &ChatMessage) -> gtk::Widget {
    if m.role == Role::User {
        let l = ui::label(&m.text);
        l.set_selectable(true);
        l.add_css_class("bubble");
        l.set_halign(gtk::Align::End);
        l.set_margin_start(80);
        return l.upcast();
    }
    let b = gtk::Box::new(gtk::Orientation::Vertical, 8);
    b.set_margin_end(40);
    let body = rich_text(&m.text);
    if m.role == Role::Error {
        body.add_css_class("error");
    }
    b.append(&body);
    for p in &m.plans {
        b.append(&plan_card(p));
    }
    if !m.tools.is_empty() {
        let mut names: Vec<String> = m.tools.iter().map(|x| tool_name(x)).collect();
        names.dedup();
        let l = ui::dim_label(&tv("chat.looked", &[("list", names.join(", ").into())]));
        l.add_css_class("caption");
        b.append(&l);
    }
    b.upcast()
}

pub struct Chat {
    root: gtk::Stack,
    log: gtk::Box,
    scroll: gtk::ScrolledWindow,
    entry: gtk::Entry,
    send: gtk::Button,
    reset: gtk::Button,
    welcome: gtk::Box,
    chips: gtk::Box,
    live: gtk::Box,
    live_text: RefCell<(u64, String)>,
    live_label: gtk::Label,
    live_tool: gtk::Label,
    busy: Cell<bool>,
}

impl Chat {
    pub fn new() -> Rc<Chat> {
        let root = gtk::Stack::new();

        let need = ui::status_page(
            "chat-message-new-symbolic",
            &t("chat.needKey"),
            &t("chat.needKeyBody"),
        );
        let open = ui::pill(&t("chat.openSettings"), "suggested-action");
        open.set_halign(gtk::Align::Center);
        open.connect_clicked(|_| shell().select("settings"));
        need.set_child(Some(&open));
        root.add_named(&need, Some("off"));

        let log = gtk::Box::new(gtk::Orientation::Vertical, 18);
        log.set_margin_top(24);
        log.set_margin_bottom(24);
        log.set_margin_start(12);
        log.set_margin_end(12);

        let welcome = gtk::Box::new(gtk::Orientation::Vertical, 8);
        welcome.set_margin_top(24);
        welcome.append(&ui::heading(&t("chat.welcome")));
        welcome.append(&ui::dim_label(&t("chat.welcomeBody")));
        let chips = gtk::Box::new(gtk::Orientation::Vertical, 8);
        chips.set_margin_top(12);
        welcome.append(&chips);

        let live = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let live_label = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .selectable(true)
            .use_markup(true)
            .build();
        let live_tool = ui::dim_label("");
        live_tool.add_css_class("caption");
        let typing = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        typing.append(&ui::spinner());
        typing.append(&live_tool);
        live.append(&live_label);
        live.append(&typing);
        live.set_visible(false);

        let outer = gtk::Box::new(gtk::Orientation::Vertical, 18);
        outer.append(&log);
        let clamp = adw::Clamp::builder()
            .maximum_size(780)
            .child(&outer)
            .build();
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&clamp)
            .build();

        let entry = gtk::Entry::builder()
            .placeholder_text(t("chat.placeholder"))
            .hexpand(true)
            .build();
        entry.set_tooltip_text(Some(&t("chat.message")));
        entry.set_max_length(4000);
        let send = gtk::Button::builder()
            .icon_name("go-up-symbolic")
            .tooltip_text(t("chat.send"))
            .build();
        send.add_css_class("suggested-action");
        send.add_css_class("circular");
        let reset = ui::icon_button("user-trash-symbolic", &t("chat.clear"));
        let composer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        composer.set_margin_top(10);
        composer.set_margin_bottom(10);
        composer.set_margin_start(12);
        composer.set_margin_end(12);
        composer.append(&entry);
        composer.append(&send);
        composer.append(&reset);
        let composer_clamp = adw::Clamp::builder()
            .maximum_size(780)
            .child(&composer)
            .build();

        let tv_ = adw::ToolbarView::new();
        tv_.set_content(Some(&scroll));
        tv_.add_bottom_bar(&composer_clamp);
        root.add_named(&tv_, Some("on"));

        let page = Rc::new(Chat {
            root,
            log,
            scroll,
            entry,
            send,
            reset,
            welcome,
            chips,
            live,
            live_text: RefCell::new((0, String::new())),
            live_label,
            live_tool,
            busy: Cell::new(false),
        });
        {
            let p = Rc::downgrade(&page);
            page.entry.connect_activate(move |e| {
                if let Some(p) = p.upgrade() {
                    p.submit(e.text().to_string());
                }
            });
        }
        {
            let p = Rc::downgrade(&page);
            page.send.connect_clicked(move |_| {
                if let Some(p) = p.upgrade() {
                    p.submit(p.entry.text().to_string());
                }
            });
        }
        {
            let p = Rc::downgrade(&page);
            page.entry.connect_changed(move |e| {
                if let Some(p) = p.upgrade() {
                    p.send
                        .set_sensitive(!p.busy.get() && !e.text().trim().is_empty());
                }
            });
        }
        page.send.set_sensitive(false);
        page.reset.connect_clicked(|_| crate::dialogs::clear_chat());
        page
    }

    pub fn widget(&self) -> gtk::Widget {
        self.root.clone().upcast()
    }

    pub fn on_show(self: &Rc<Self>) {
        self.refresh();
        if self.root.visible_child_name().as_deref() == Some("on") {
            self.entry.grab_focus();
        }
    }

    pub fn refresh_starters(self: &Rc<Self>) {
        ui::clear_box(&self.chips);
        let findings = shell().data.borrow().findings.findings.clone();
        for q in starters(&findings) {
            // Uzun öneriler dar pencerede alt satıra geçsin.
            let l = ui::label(&q);
            let b = gtk::Button::builder().child(&l).build();
            b.add_css_class("pill");
            b.set_halign(gtk::Align::Start);
            let p = Rc::downgrade(self);
            b.connect_clicked(move |_| {
                if let Some(p) = p.upgrade() {
                    p.submit(q.clone());
                }
            });
            self.chips.append(&b);
        }
    }

    /// Mesaj listesini baştan çizer.
    pub fn refresh(self: &Rc<Self>) {
        let s = shell();
        let d = s.data.borrow();
        self.root
            .set_visible_child_name(if d.chat_enabled() { "on" } else { "off" });
        ui::clear_box(&self.log);
        let empty = d.messages.is_empty();
        self.reset.set_visible(!empty);
        if empty {
            self.log.append(&self.welcome);
        }
        for m in &d.messages {
            self.log.append(&bubble(m));
        }
        self.log.append(&self.live);
        drop(d);
        if empty {
            self.refresh_starters();
        }
        self.scroll_to_end();
    }

    fn scroll_to_end(&self) {
        let adj = self.scroll.vadjustment();
        glib::idle_add_local_once(move || adj.set_value(adj.upper()));
    }

    fn submit(self: &Rc<Self>, text: String) {
        let msg = text.trim().to_string();
        if msg.is_empty() || self.busy.get() {
            return;
        }
        self.entry.set_text("");
        self.busy.set(true);
        self.send.set_sensitive(false);
        shell().data.borrow_mut().messages.push(ChatMessage {
            role: Role::User,
            text: msg.clone(),
            plans: vec![],
            tools: vec![],
        });
        *self.live_text.borrow_mut() = (u64::MAX, String::new());
        self.live_label.set_visible(false);
        self.live_tool.set_label(&t("chat.typing"));
        self.refresh();
        self.live.set_visible(true);
        let p = self.clone();
        glib::spawn_future_local(async move {
            let r =
                client::call("chat", json!({ "message": msg, "conversation_id": "main" })).await;
            let reply = match r {
                Ok(v) => ChatMessage {
                    role: Role::Assistant,
                    text: v["reply"].as_str().unwrap_or("").to_string(),
                    plans: serde_json::from_value(v["plans"].clone()).unwrap_or_default(),
                    tools: serde_json::from_value(v["tools_used"].clone()).unwrap_or_default(),
                },
                Err(e) => ChatMessage {
                    role: Role::Error,
                    text: ui::error_text(&e),
                    plans: vec![],
                    tools: vec![],
                },
            };
            shell().data.borrow_mut().messages.push(reply);
            p.busy.set(false);
            p.live.set_visible(false);
            p.send.set_sensitive(!p.entry.text().trim().is_empty());
            p.refresh();
        });
    }

    /// Yanıt yazılırken gelen parçalar.
    pub fn on_delta(&self, turn: u64, text: &str) {
        if !self.busy.get() {
            return;
        }
        let mut l = self.live_text.borrow_mut();
        if l.0 != turn {
            *l = (turn, String::new());
        }
        l.1.push_str(text);
        self.live_label
            .set_markup(&l.1.lines().map(markup).collect::<Vec<_>>().join("\n"));
        self.live_label.set_visible(true);
        self.live_tool.set_label("");
        self.scroll_to_end();
    }

    pub fn on_tool(&self, name: &str) {
        if self.busy.get() {
            self.live_tool
                .set_label(&tv("chat.looking", &[("what", tool_name(name).into())]));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markup_escapes_and_formats() {
        assert_eq!(
            markup("run `rm -rf <x>` & **now**"),
            "run <tt>rm -rf &lt;x&gt;</tt> &amp; <b>now</b>"
        );
        assert_eq!(markup("a ` b"), "a <tt> b</tt>");
    }

    #[test]
    fn starters_come_from_findings_first() {
        crate::i18n::set_lang(bosun_core::Lang::En);
        let f = |id: &str| Finding {
            id: id.into(),
            severity: bosun_core::rules::Severity::Warning,
            title: String::new(),
            detail: String::new(),
            category: None,
            muted: false,
        };
        let s = starters(&[f("disk:/"), f("cpu"), f("unknown")]);
        assert_eq!(s.len(), MAX_STARTERS);
        assert_eq!(
            s[0],
            "The system disk is almost full. What can I safely remove?"
        );
        assert_eq!(s[1], "What is keeping the CPU busy?");
    }
}
