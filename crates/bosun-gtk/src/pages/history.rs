//! Geçmiş: temizlikler, yönetici eylemleri ve başlangıç değişiklikleri, en yenisi başta.
//! Aynı plana ait kayıtlar (hazırlandı, onaylandı, uygulandı…) tek satıra iner.

use crate::client;
use crate::i18n::{self, t, tv};
use crate::ui;
use adw::prelude::*;
use chrono::{DateTime, Utc};
use gtk::glib;
use serde::Deserialize;
use serde_json::{json, Value};
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

#[derive(Debug, Clone, Deserialize)]
pub struct AuditEntry {
    pub ts: DateTime<Utc>,
    pub kind: String,
    #[serde(default)]
    pub data: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lamp {
    Ok,
    Warning,
    Info,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub ts: DateTime<Utc>,
    pub title: String,
    pub detail: String,
    pub lamp: Lamp,
    pub dim: bool,
}

const PLAN_KINDS: [&str; 5] = [
    "plan_created",
    "plan_proposed",
    "plan_approved",
    "plan_executed",
    "plan_cancelled",
];

fn bytes_of(v: &Value) -> String {
    i18n::bytes(v.as_u64().unwrap_or(0))
}

fn summary_titles(d: Option<&Value>) -> String {
    d.and_then(|d| d["summary"].as_array())
        .map(|a| {
            a.iter()
                .filter_map(|s| s["title"].as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}

fn count(v: &Value) -> usize {
    v.as_u64()
        .map(|n| n as usize)
        .or_else(|| v.as_array().map(Vec::len))
        .unwrap_or(0)
}

fn plan_row(events: &[&AuditEntry]) -> Row {
    let by = |k: &str| events.iter().find(|e| e.kind == k).map(|e| &e.data);
    let start = by("plan_created").or_else(|| by("plan_proposed"));
    let done = by("plan_executed");
    let from_chat = by("plan_proposed").is_some();
    let titles = summary_titles(done.or(start));
    let ts = events[0].ts;

    if let Some(d) = done {
        let failed = count(&d["failed"]);
        let mut detail = vec![titles];
        let (trashed, deleted) = (count(&d["trashed"]), count(&d["deleted"]));
        if trashed > 0 {
            detail.push(tv("history.inTrash", &[("n", trashed.into())]));
        }
        if deleted > 0 {
            detail.push(tv("history.deletedN", &[("n", deleted.into())]));
        }
        if failed > 0 {
            detail.push(tv("history.errors", &[("n", failed.into())]));
        }
        if from_chat {
            detail.push(t("history.fromChat"));
        }
        let key = if d["dry_run"].as_bool().unwrap_or(false) {
            "history.dryRun"
        } else {
            "history.freed"
        };
        return Row {
            ts,
            title: tv(key, &[("size", bytes_of(&d["freed_bytes"]).into())]),
            detail: detail
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" · "),
            lamp: if failed > 0 { Lamp::Warning } else { Lamp::Ok },
            dim: false,
        };
    }
    if let Some(a) = by("plan_approved") {
        return Row {
            ts,
            title: t("history.approved"),
            detail: tv(
                "history.approvedDetail",
                &[
                    ("n", count(&a["items"]).into()),
                    ("size", bytes_of(&a["total_bytes"]).into()),
                ],
            ),
            lamp: Lamp::Info,
            dim: false,
        };
    }
    let size = start
        .map(|s| bytes_of(&s["total_bytes"]))
        .unwrap_or_default();
    Row {
        ts,
        title: t(if from_chat {
            "history.proposed"
        } else {
            "history.notApplied"
        }),
        detail: [size, titles]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" · "),
        lamp: Lamp::Info,
        dim: true,
    }
}

fn other_row(e: &AuditEntry) -> Row {
    let d = &e.data;
    let str_of = |v: &Value| {
        v.as_str().map(str::to_string).unwrap_or_else(|| {
            if v.is_null() {
                String::new()
            } else {
                v.to_string()
            }
        })
    };
    match e.kind.as_str() {
        "admin_action" => {
            let name = d["title"]
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| str_of(&d["action"]));
            if d["ok"].as_bool().unwrap_or(false) {
                let detail = if d["freed_bytes"].as_u64().unwrap_or(0) > 0 {
                    tv(
                        "history.freed",
                        &[("size", bytes_of(&d["freed_bytes"]).into())],
                    )
                } else {
                    t("history.adminOk")
                };
                Row {
                    ts: e.ts,
                    title: name,
                    detail,
                    lamp: Lamp::Ok,
                    dim: false,
                }
            } else {
                Row {
                    ts: e.ts,
                    title: tv("history.adminFail", &[("action", name.into())]),
                    detail: str_of(&d["error"]),
                    lamp: Lamp::Warning,
                    dim: false,
                }
            }
        }
        "startup_toggled" => {
            let key = if d["enabled"].as_bool().unwrap_or(false) {
                "history.startupOn"
            } else {
                "history.startupOff"
            };
            Row {
                ts: e.ts,
                title: tv(key, &[("name", str_of(&d["name"]).into())]),
                detail: d["file"].as_str().map(i18n::short_path).unwrap_or_default(),
                lamp: Lamp::Info,
                dim: false,
            }
        }
        _ => Row {
            ts: e.ts,
            title: e.kind.clone(),
            detail: String::new(),
            lamp: Lamp::Info,
            dim: false,
        },
    }
}

/// Kayıtlar en yenisi başta gelir; aynı plana ait olanlar ilk göründükleri yerde birleşir.
pub fn to_rows(entries: &[AuditEntry]) -> Vec<Row> {
    let plan_id = |e: &AuditEntry| e.data["plan_id"].as_str().map(str::to_string);
    let mut plans: HashMap<String, Vec<&AuditEntry>> = HashMap::new();
    for e in entries {
        if let (true, Some(id)) = (PLAN_KINDS.contains(&e.kind.as_str()), plan_id(e)) {
            plans.entry(id).or_default().push(e);
        }
    }
    let mut seen = HashSet::new();
    let mut rows = vec![];
    for e in entries {
        if !PLAN_KINDS.contains(&e.kind.as_str()) {
            rows.push(other_row(e));
            continue;
        }
        match plan_id(e) {
            None => rows.push(plan_row(&[e])),
            Some(id) => {
                if seen.insert(id.clone()) {
                    rows.push(plan_row(&plans[&id]));
                }
            }
        }
    }
    rows
}

pub struct History {
    root: gtk::Box,
    list: gtk::Box,
    dirty: Cell<bool>,
    visible: Cell<bool>,
}

impl History {
    pub fn new() -> Rc<History> {
        let (scroll, content) = ui::page(760);
        content.append(&ui::dim_label(&t("history.intro")));
        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.append(&list);
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&scroll);
        let page = Rc::new(History {
            root,
            list,
            dirty: Cell::new(true),
            visible: Cell::new(false),
        });
        {
            let p = Rc::downgrade(&page);
            page.root.connect_unmap(move |_| {
                if let Some(p) = p.upgrade() {
                    p.visible.set(false);
                }
            });
        }
        page
    }

    pub fn widget(&self) -> gtk::Widget {
        self.root.clone().upcast()
    }

    pub fn on_show(self: &Rc<Self>) {
        self.visible.set(true);
        if self.dirty.replace(false) {
            self.load();
        }
    }

    /// Yeni bir kayıt oluştu: ekran açıksa hemen, değilse bir dahaki açılışta yükle.
    pub fn mark_dirty(self: &Rc<Self>) {
        self.dirty.set(true);
        if self.visible.get() {
            self.on_show();
        }
    }

    fn load(self: &Rc<Self>) {
        if self.list.first_child().is_none() {
            self.list.append(&ui::dim_label(&t("history.loading")));
        }
        let p = self.clone();
        glib::spawn_future_local(async move {
            let entries: Vec<AuditEntry> =
                match client::call("history", json!({ "limit": 100 })).await {
                    Ok(v) => serde_json::from_value(v["entries"].clone()).unwrap_or_default(),
                    Err(_) => {
                        p.dirty.set(true);
                        vec![]
                    }
                };
            ui::clear_box(&p.list);
            let rows = to_rows(&entries);
            if rows.is_empty() {
                p.list.append(&ui::status_page(
                    "document-open-recent-symbolic",
                    &t("history.empty"),
                    "",
                ));
                return;
            }
            let list = ui::boxed_list();
            for r in rows {
                let row = ui::row(&r.title, &r.detail);
                row.set_subtitle_lines(2);
                let (icon, class) = match r.lamp {
                    Lamp::Ok => ("emblem-ok-symbolic", "success"),
                    Lamp::Warning => ("dialog-warning-symbolic", "warning"),
                    Lamp::Info => ("dialog-information-symbolic", "dim-label"),
                };
                let img = gtk::Image::from_icon_name(icon);
                img.add_css_class(class);
                row.add_prefix(&img);
                let time = ui::value(&i18n::clock(r.ts));
                time.add_css_class("dim-label");
                time.add_css_class("caption");
                row.add_suffix(&time);
                if r.dim {
                    row.add_css_class("dim-label");
                }
                list.append(&row);
            }
            p.list.append(&list);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(kind: &str, data: Value) -> AuditEntry {
        AuditEntry {
            ts: Utc::now(),
            kind: kind.into(),
            data,
        }
    }

    #[test]
    fn plan_events_collapse_into_one_row() {
        crate::i18n::set_lang(bosun_core::Lang::En);
        let entries = vec![
            e(
                "plan_executed",
                json!({"plan_id": "a", "freed_bytes": 2048, "trashed": 3, "deleted": 0, "failed": [], "dry_run": false, "summary": [{"title": "Thumbnails"}]}),
            ),
            e(
                "plan_approved",
                json!({"plan_id": "a", "items": 3, "total_bytes": 2048}),
            ),
            e(
                "plan_created",
                json!({"plan_id": "a", "total_bytes": 2048, "summary": [{"title": "Thumbnails"}]}),
            ),
            e(
                "startup_toggled",
                json!({"name": "Dropbox", "enabled": false, "file": "/x/dropbox.desktop"}),
            ),
            e("plan_created", json!({"plan_id": "b", "total_bytes": 10})),
        ];
        let rows = to_rows(&entries);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].title, "2.0 KB freed");
        assert_eq!(rows[0].detail, "Thumbnails · 3 items in the trash");
        assert_eq!(rows[0].lamp, Lamp::Ok);
        assert_eq!(rows[1].title, "Startup app turned off: Dropbox");
        assert!(rows[2].dim);
    }

    #[test]
    fn failed_admin_action_is_a_warning() {
        crate::i18n::set_lang(bosun_core::Lang::En);
        let rows = to_rows(&[e(
            "admin_action",
            json!({"action": "apt-clean", "ok": false, "error": "denied"}),
        )]);
        assert_eq!(rows[0].lamp, Lamp::Warning);
        assert_eq!(rows[0].detail, "denied");
    }
}
