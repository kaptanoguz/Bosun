//! Genel bakış: canlı ölçümler, bulgular, en yoğun uygulamalar, kapsayıcılar ve öneriler.

use crate::dialogs;
use crate::i18n::{self, t, tv};
use crate::shell::{shell, Data};
use crate::spark::Spark;
use crate::ui;
use adw::prelude::*;
use bosun_core::metrics::Snapshot;
use bosun_core::rules::Severity;
use gtk::glib;
use serde_json::json;
use std::cell::Cell;
use std::rc::Rc;

/// Bir ölçüm kartı: başlık, büyük değer, ayrıntılar ve eğri ya da çubuk.
struct Card {
    root: gtk::Box,
    value: gtk::Label,
    lines: [gtk::Label; 2],
    spark: Option<Spark>,
    bar: Option<gtk::LevelBar>,
}

impl Card {
    fn new(title: &str, fmt: Option<fn(f64) -> String>) -> Card {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 2);
        root.add_css_class("card");
        root.add_css_class("metric");
        let head = ui::dim_label(title);
        head.add_css_class("caption-heading");
        let value = gtk::Label::builder().xalign(0.0).build();
        value.add_css_class("title-1");
        value.add_css_class("numeric");
        let lines = [ui::dim_label(""), ui::dim_label("")];
        root.append(&head);
        root.append(&value);
        for l in &lines {
            l.add_css_class("caption");
            root.append(l);
        }
        let spacer = gtk::Box::builder().vexpand(true).build();
        root.append(&spacer);
        let (spark, bar) = match fmt {
            Some(f) => {
                let s = Spark::new(f);
                s.area.set_margin_top(8);
                root.append(&s.area);
                (Some(s), None)
            }
            None => {
                let b = ui::bar(0.0, false);
                b.set_margin_top(20);
                b.set_margin_bottom(14);
                root.append(&b);
                (None, Some(b))
            }
        };
        Card {
            root,
            value,
            lines,
            spark,
            bar,
        }
    }

    fn set(&self, value: &str, level: Level, a: &str, b: &str) {
        self.value.set_label(value);
        for c in ["warning", "error"] {
            self.value.remove_css_class(c);
        }
        match level {
            Level::Warn => self.value.add_css_class("warning"),
            Level::Bad => self.value.add_css_class("error"),
            Level::Ok => {}
        }
        self.lines[0].set_label(a);
        self.lines[1].set_label(b);
        self.lines[1].set_visible(!b.is_empty());
    }
}

#[derive(Clone, Copy)]
enum Level {
    Ok,
    Warn,
    Bad,
}

fn level(v: f64, warn: f64, bad: f64) -> Level {
    if v >= bad {
        Level::Bad
    } else if v >= warn {
        Level::Warn
    } else {
        Level::Ok
    }
}

fn pct_fmt(v: f64) -> String {
    i18n::percent(v)
}

fn rate_fmt(v: f64) -> String {
    format!("{}/s", i18n::bytes(v as u64))
}

pub struct Overview {
    root: gtk::Stack,
    subtitle: gtk::Label,
    cpu: Card,
    mem: Card,
    disk: Card,
    io: Card,
    net: Card,
    gpu: Card,
    facts: gtk::Label,
    findings: gtk::ListBox,
    apps_group: adw::PreferencesGroup,
    apps: gtk::ListBox,
    by_memory: Cell<bool>,
    containers: adw::PreferencesGroup,
    containers_list: gtk::ListBox,
    disks: adw::PreferencesGroup,
    disks_list: gtk::ListBox,
    advice: gtk::ListBox,
}

impl Overview {
    pub fn new() -> Rc<Overview> {
        let (scroll, content) = ui::page(1000);
        let subtitle = ui::dim_label("");
        content.append(&subtitle);

        let cards = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .homogeneous(true)
            .min_children_per_line(1)
            .max_children_per_line(3)
            .column_spacing(12)
            .row_spacing(12)
            .build();
        let cpu = Card::new(&t("overview.cpu"), Some(pct_fmt));
        let mem = Card::new(&t("overview.memory"), Some(pct_fmt));
        let disk = Card::new(&tv("overview.disk", &[("mount", "/".into())]), None);
        let io = Card::new(&t("overview.diskIo"), Some(rate_fmt));
        let net = Card::new(&t("overview.network"), Some(rate_fmt));
        let gpu = Card::new(&t("overview.gpu"), Some(pct_fmt));
        for c in [&cpu, &mem, &disk, &io, &net, &gpu] {
            cards.append(&c.root);
        }
        content.append(&cards);
        let facts = ui::dim_label("");
        content.append(&facts);

        let findings = ui::boxed_list();
        content.append(&findings);

        let apps_group = ui::group(&t("overview.busiest"), "");
        let sort = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        sort.add_css_class("linked");
        sort.set_valign(gtk::Align::Center);
        let by_cpu = gtk::ToggleButton::builder()
            .label(t("overview.cpu"))
            .active(true)
            .build();
        let by_mem = gtk::ToggleButton::builder()
            .label(t("overview.memory"))
            .group(&by_cpu)
            .build();
        sort.append(&by_cpu);
        sort.append(&by_mem);
        sort.set_tooltip_text(Some(&t("overview.sortBy")));
        apps_group.set_header_suffix(Some(&sort));
        let apps = ui::boxed_list();
        apps_group.add(&apps);
        content.append(&apps_group);

        let containers = ui::group("", "");
        let containers_list = ui::boxed_list();
        containers.add(&containers_list);
        content.append(&containers);

        let disks = ui::group(&t("overview.otherDisks"), "");
        let disks_list = ui::boxed_list();
        disks.add(&disks_list);
        content.append(&disks);

        let advice_group = ui::group(&t("overview.advice"), &t("overview.adviceHint"));
        let advice = ui::boxed_list();
        advice_group.add(&advice);
        content.append(&advice_group);

        let root = gtk::Stack::new();
        let waiting = ui::status_page("computer-symbolic", &t("overview.waiting"), "");
        root.add_named(&waiting, Some("waiting"));
        root.add_named(&scroll, Some("main"));

        let page = Rc::new(Overview {
            root,
            subtitle,
            cpu,
            mem,
            disk,
            io,
            net,
            gpu,
            facts,
            findings,
            apps_group,
            apps,
            by_memory: Cell::new(false),
            containers,
            containers_list,
            disks,
            disks_list,
            advice,
        });
        {
            let p = Rc::downgrade(&page);
            by_mem.connect_toggled(move |b| {
                if let Some(p) = p.upgrade() {
                    p.by_memory.set(b.is_active());
                    p.update_apps(&shell().data.borrow());
                }
            });
        }
        page
    }

    pub fn widget(&self) -> gtk::Widget {
        self.root.clone().upcast()
    }

    pub fn update_all(&self) {
        self.update_metrics();
        self.update_findings();
        self.update_containers();
    }

    pub fn update_subtitle(&self) {
        let s = shell();
        let d = s.data.borrow();
        let mut text = d
            .latest
            .as_ref()
            .map(|l| tv("status.live", &[("ago", i18n::ago(l.ts).into())]))
            .unwrap_or_default();
        if let Some(scan) = &d.scan {
            text += " · ";
            text += &tv("status.lastScan", &[("ago", i18n::ago(scan.ts).into())]).to_lowercase();
        }
        self.subtitle.set_label(&text);
    }

    pub fn update_metrics(&self) {
        let s = shell();
        let d = s.data.borrow();
        if d.latest.is_none() {
            self.root.set_visible_child_name("waiting");
            return;
        }
        self.root.set_visible_child_name("main");
        drop(d);
        self.update_subtitle();
        let d = s.data.borrow();
        let l = d.latest.as_ref().expect("checked above");
        self.update_cards(l, &d.history);
        self.update_facts(l);
        self.update_apps(&d);
        self.update_disks(l);
    }

    fn update_cards(&self, l: &Snapshot, history: &[Snapshot]) {
        let times: Vec<_> = history.iter().map(|s| s.ts).collect();
        let series = |f: &dyn Fn(&Snapshot) -> f64| history.iter().map(f).collect::<Vec<f64>>();
        let b = |v: u64| i18n::bytes(v);

        let cpu = l.cpu_percent as f64;
        self.cpu.set(
            &i18n::percent(cpu),
            level(cpu, 85.0, 95.0),
            &tv("overview.cores", &[("n", l.cpu_count.into())]),
            "",
        );
        if let Some(sp) = &self.cpu.spark {
            sp.set(
                series(&|s| s.cpu_percent as f64),
                None,
                Some(100.0),
                times.clone(),
            );
        }

        let mem = i18n::pct(l.mem_used, l.mem_total);
        let swap = if l.swap_total > 0 {
            tv(
                "overview.swapUse",
                &[
                    ("used", b(l.swap_used).into()),
                    ("total", b(l.swap_total).into()),
                ],
            )
        } else {
            t("overview.noSwap")
        };
        self.mem.set(
            &i18n::percent(mem),
            level(mem, 85.0, 92.0),
            &tv(
                "overview.of",
                &[
                    ("used", b(l.mem_used).into()),
                    ("total", b(l.mem_total).into()),
                ],
            ),
            &swap,
        );
        if let Some(sp) = &self.mem.spark {
            sp.set(
                series(&|s| i18n::pct(s.mem_used, s.mem_total)),
                None,
                Some(100.0),
                times.clone(),
            );
        }

        if let Some(root) = l.root_disk() {
            let p = i18n::pct(root.total - root.available, root.total);
            self.disk.set(
                &i18n::percent(p),
                level(p, 85.0, 95.0),
                &tv(
                    "overview.freeOf",
                    &[
                        ("free", b(root.available).into()),
                        ("total", b(root.total).into()),
                    ],
                ),
                "",
            );
            if let Some(bar) = &self.disk.bar {
                bar.set_value(p / 100.0);
                if p >= 85.0 {
                    bar.add_css_class("warn");
                } else {
                    bar.remove_css_class("warn");
                }
            }
        }

        self.io.set(
            &rate_fmt((l.disk_read_bps + l.disk_write_bps) as f64),
            Level::Ok,
            &tv("overview.read", &[("v", b(l.disk_read_bps).into())]),
            &tv("overview.write", &[("v", b(l.disk_write_bps).into())]),
        );
        if let Some(sp) = &self.io.spark {
            sp.set(
                series(&|s| s.disk_read_bps as f64),
                Some(series(&|s| s.disk_write_bps as f64)),
                None,
                times.clone(),
            );
        }

        self.net.set(
            &rate_fmt((l.net_rx_bps + l.net_tx_bps) as f64),
            Level::Ok,
            &tv("overview.down", &[("v", b(l.net_rx_bps).into())]),
            &tv("overview.up", &[("v", b(l.net_tx_bps).into())]),
        );
        if let Some(sp) = &self.net.spark {
            sp.set(
                series(&|s| s.net_rx_bps as f64),
                Some(series(&|s| s.net_tx_bps as f64)),
                None,
                times.clone(),
            );
        }

        match &l.gpu {
            Some(g) => {
                self.gpu.root.set_visible(true);
                let mem = if g.mem_total > 0 {
                    format!("{} / {}", b(g.mem_used), b(g.mem_total))
                } else {
                    g.name.clone()
                };
                let temp = g
                    .temp_c
                    .map(|c| format!("{} °C", c.round()))
                    .unwrap_or_default();
                self.gpu.set(
                    &i18n::percent(g.util_percent as f64),
                    Level::Ok,
                    &mem,
                    &temp,
                );
                if let Some(sp) = &self.gpu.spark {
                    sp.set(
                        series(&|s| s.gpu.as_ref().map_or(0.0, |g| g.util_percent as f64)),
                        None,
                        Some(100.0),
                        times,
                    );
                }
            }
            None => self.gpu.root.set_visible(false),
        }
        // FlowBox kendi çocuğunu sarar; gizli kartın kutusu da gizlenmeli.
        if let Some(parent) = self.gpu.root.parent() {
            parent.set_visible(l.gpu.is_some());
        }
    }

    fn update_facts(&self, l: &Snapshot) {
        let mut parts = vec![
            tv(
                "overview.uptime",
                &[("uptime", i18n::uptime(l.uptime_secs).into())],
            ),
            tv(
                "overview.loadLine",
                &[
                    ("a", format!("{:.2}", l.load_avg[0]).into()),
                    ("b", format!("{:.2}", l.load_avg[1]).into()),
                    ("c", format!("{:.2}", l.load_avg[2]).into()),
                ],
            ),
        ];
        if let Some(temp) = l.max_temp() {
            parts.push(tv(
                "overview.hottest",
                &[("t", (temp.round() as i64).into())],
            ));
        }
        if let Some(bat) = &l.battery {
            let status = t(&format!(
                "battery.{}",
                bat.status.to_lowercase().replace(' ', "_")
            ));
            let power = bat
                .power_w
                .map(|w| format!(", {w:.1} W"))
                .unwrap_or_default();
            parts.push(format!(
                "{} {} ({status}{power})",
                t("overview.battery"),
                i18n::percent(bat.percent as f64)
            ));
        }
        self.facts.set_label(&parts.join(" · "));
    }

    fn update_apps(&self, d: &Data) {
        ui::clear_list(&self.apps);
        let Some(l) = &d.latest else { return };
        let mut apps = l.top_apps.clone();
        if self.by_memory.get() {
            apps.sort_by_key(|a| std::cmp::Reverse(a.mem_bytes));
        } else {
            apps.sort_by(|a, b| b.cpu_percent.total_cmp(&a.cpu_percent));
        }
        if apps.is_empty() {
            self.apps.append(&ui::row(&t("overview.noApps"), ""));
        }
        for a in apps.iter().take(8) {
            let r = ui::row(
                &a.name,
                &tv("overview.processCount", &[("n", a.processes.into())]),
            );
            let cpu = ui::value(&format!("{:.1}%", a.cpu_percent));
            cpu.set_width_chars(7);
            cpu.set_xalign(1.0);
            if a.cpu_percent >= 20.0 {
                cpu.add_css_class("warning");
            }
            let mem = ui::value(&i18n::bytes(a.mem_bytes));
            mem.set_width_chars(9);
            mem.set_xalign(1.0);
            mem.add_css_class("dim-label");
            r.add_suffix(&cpu);
            r.add_suffix(&mem);
            self.apps.append(&r);
        }
        self.apps_group.set_visible(true);
    }

    fn update_disks(&self, l: &Snapshot) {
        ui::clear_list(&self.disks_list);
        let root = l.root_disk().map(|d| d.mount.clone());
        let others: Vec<_> = l
            .disks
            .iter()
            .filter(|d| Some(&d.mount) != root.as_ref())
            .collect();
        self.disks.set_visible(!others.is_empty());
        for d in others {
            let p = i18n::pct(d.total - d.available, d.total);
            let r = ui::row(
                &d.mount,
                &tv(
                    "overview.free",
                    &[("size", i18n::bytes(d.available).into())],
                ),
            );
            let bar = ui::bar(p / 100.0, p >= 85.0);
            bar.set_width_request(120);
            bar.set_valign(gtk::Align::Center);
            r.add_suffix(&bar);
            let v = ui::value(&i18n::percent(p));
            v.set_width_chars(5);
            r.add_suffix(&v);
            self.disks_list.append(&r);
        }
    }

    pub fn update_containers(&self) {
        let s = shell();
        let d = s.data.borrow();
        ui::clear_list(&self.containers_list);
        let Some(c) = &d.containers else {
            self.containers.set_visible(false);
            return;
        };
        self.containers.set_visible(true);
        self.containers.set_title(if c.engine == "podman" {
            "Podman"
        } else {
            "Docker"
        });
        self.containers
            .set_description(Some(&glib::markup_escape_text(&tv(
                "overview.containersCount",
                &[("running", c.running.into()), ("total", c.total.into())],
            ))));
        if c.containers.is_empty() {
            self.containers_list
                .append(&ui::row(&t("overview.noContainers"), ""));
        }
        for x in c.containers.iter().take(6) {
            let r = ui::row(&x.name, &x.image);
            r.add_suffix(&ui::value(&format!("{:.1}%", x.cpu_percent)));
            let mem = ui::value(&i18n::bytes(x.mem_bytes));
            mem.add_css_class("dim-label");
            r.add_suffix(&mem);
            self.containers_list.append(&r);
        }
    }

    pub fn update_findings(&self) {
        let s = shell();
        let d = s.data.borrow();
        ui::clear_list(&self.findings);
        let active: Vec<_> = d.findings.findings.iter().filter(|f| !f.muted).collect();
        let muted: Vec<_> = d.findings.findings.iter().filter(|f| f.muted).collect();
        for f in &active {
            let r = ui::row(&f.title, &f.detail);
            r.set_subtitle_lines(0);
            let (icon, class) = match f.severity {
                Severity::Critical => ("dialog-error-symbolic", "error"),
                Severity::Warning => ("dialog-warning-symbolic", "warning"),
                Severity::Info => ("dialog-information-symbolic", "accent"),
            };
            let img = gtk::Image::from_icon_name(icon);
            img.add_css_class(class);
            r.add_prefix(&img);
            if f.id == "reclaimable" || f.category.is_some() {
                let b = ui::button(&t("overview.reviewCleanup"));
                b.connect_clicked(|_| shell().select("cleanup"));
                r.add_suffix(&b);
            }
            if f.severity != Severity::Info || f.id.starts_with("leak:") {
                let b = ui::button(&t("overview.mute"));
                b.add_css_class("flat");
                let id = f.id.clone();
                b.connect_clicked(move |_| mute(id.clone(), true));
                r.add_suffix(&b);
            }
            self.findings.append(&r);
        }
        if active.is_empty() {
            let r = ui::row(&t("overview.allGood"), &t("overview.allGoodBody"));
            let img = gtk::Image::from_icon_name("emblem-ok-symbolic");
            img.add_css_class("success");
            r.add_prefix(&img);
            self.findings.append(&r);
        }
        if !muted.is_empty() {
            let e = ui::expander(&tv("overview.mutedCount", &[("n", muted.len().into())]), "");
            for f in muted {
                let r = ui::row(&f.title, &f.detail);
                let b = ui::button(&t("overview.unmute"));
                b.add_css_class("flat");
                let id = f.id.clone();
                b.connect_clicked(move |_| mute(id.clone(), false));
                r.add_suffix(&b);
                e.add_row(&r);
            }
            self.findings.append(&e);
        }

        ui::clear_list(&self.advice);
        if d.findings.advice.is_empty() {
            self.advice.append(&ui::row(&t("overview.noAdvice"), ""));
        }
        for a in &d.findings.advice {
            let e = ui::expander(&a.title, &a.detail);
            e.set_subtitle_lines(0);
            if let Some(action) = &a.admin_action {
                let b = ui::button(&t("overview.apply"));
                let action = action.clone();
                b.connect_clicked(move |_| dialogs::admin(&action, vec![]));
                e.add_suffix(&b);
            }
            let change = gtk::Box::new(gtk::Orientation::Vertical, 6);
            change.set_margin_top(10);
            change.set_margin_bottom(10);
            change.set_margin_start(12);
            change.set_margin_end(12);
            let diff = gtk::Label::builder()
                .xalign(0.0)
                .wrap(true)
                .use_markup(true)
                .build();
            diff.set_markup(&format!(
                "<s>{}</s>  →  <b>{}</b>",
                glib::markup_escape_text(&a.current),
                glib::markup_escape_text(&a.suggested)
            ));
            change.append(&diff);
            if let Some(cmd) = &a.command {
                let or = ui::dim_label(&t("clean.orTerminal"));
                or.add_css_class("caption");
                change.append(&or);
                change.append(&ui::command(cmd));
            }
            e.add_row(&change);
            self.advice.append(&e);
        }
    }
}

fn mute(id: String, muted: bool) {
    glib::spawn_future_local(async move {
        match crate::client::get("findings.mute", json!({"id": id, "muted": muted})).await {
            Ok(f) => {
                let s = shell();
                s.data.borrow_mut().findings = f;
                crate::shell::ui().overview.update_findings();
                s.update_status();
            }
            Err(e) => shell().toast(&ui::error_text(&e)),
        }
    });
}
