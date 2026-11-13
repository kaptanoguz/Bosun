//! Temizlik: taramanın bulduğu alanlar, güvenlik etiketleri ve seçim. Hiçbir şey burada
//! silinmez; "Planı incele" yalnızca bir plan hazırlatır ve onay penceresini açar.

use crate::client;
use crate::dialogs;
use crate::i18n::{self, t, tv};
use crate::shell::shell;
use crate::ui;
use adw::prelude::*;
use bosun_core::categories::CategoryId;
use bosun_core::scan::{CategoryScan, Inspection};
use bosun_core::{Method, Safety};
use gtk::glib;
use serde_json::json;
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

/// Bir kategorinin açılınca gösterdiği öğe sayısı.
const ITEM_LIMIT: usize = 12;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    All,
    Safe,
    Caution,
    Pick,
    Admin,
}

const FILTERS: [(Filter, &str); 5] = [
    (Filter::All, "clean.f.all"),
    (Filter::Safe, "clean.f.safe"),
    (Filter::Caution, "clean.f.caution"),
    (Filter::Pick, "clean.f.pick"),
    (Filter::Admin, "clean.f.admin"),
];

pub struct Cleanup {
    root: gtk::Stack,
    filters: gtk::Box,
    narrow: Cell<bool>,
    last_scan: gtk::Label,
    rescan: gtk::Button,
    progress: gtk::ProgressBar,
    progress_text: gtk::Label,
    search: gtk::SearchEntry,
    list: gtk::Box,
    total: gtk::Label,
    count: gtk::Label,
    review: gtk::Button,
    filter: Cell<Filter>,
    /// `None`: henüz dokunulmadı, varsayılan seçim geçerli.
    selected: RefCell<Option<HashSet<CategoryId>>>,
    picked: RefCell<HashSet<String>>,
}

fn is_default(c: &CategoryScan) -> bool {
    c.bulk_selectable && c.safety == Safety::Safe && c.item_count > 0
}

fn matches(c: &CategoryScan, f: Filter, q: &str) -> bool {
    let privileged = c.method == Method::Privileged;
    let kind_ok = match f {
        Filter::All => true,
        Filter::Admin => privileged,
        Filter::Pick => c.safety == Safety::ReviewOnly,
        Filter::Safe => c.safety == Safety::Safe && !privileged,
        Filter::Caution => c.safety == Safety::Caution && !privileged,
    };
    kind_ok
        && (q.is_empty()
            || c.title.to_lowercase().contains(q)
            || c.description.to_lowercase().contains(q)
            || c.items
                .iter()
                .any(|i| i.path.to_string_lossy().to_lowercase().contains(q)))
}

fn safety_tag(c: &CategoryScan) -> gtk::Label {
    match (c.method, c.safety) {
        (Method::Permanent, _) => ui::tag(&t("clean.permanent"), "error"),
        (Method::Unlink, _) => ui::tag(&t("clean.linkOnly"), "success"),
        (_, Safety::Safe) => ui::tag(&t("clean.safe"), "success"),
        (_, Safety::Caution) => ui::tag(&t("clean.caution"), "warning"),
        (_, Safety::ReviewOnly) => ui::tag(&t("clean.review_only"), "accent"),
    }
}

fn size_text(c: &CategoryScan) -> String {
    // Kırık bağlantılar gibi yer kaplamayan öğelerde boyut yerine adet.
    if c.bytes == 0 && c.item_count > 0 {
        tv("clean.count", &[("n", c.item_count.into())])
    } else {
        i18n::bytes(c.bytes)
    }
}

fn path_text(p: &std::path::Path) -> String {
    i18n::short_path(&p.to_string_lossy())
}

/// Bir kategorinin öğelerini, notlarını ve (varsa) terminal komutunu genişleyen satıra ekler.
fn add_items(e: &adw::ExpanderRow, c: &CategoryScan) {
    for i in c.items.iter().take(ITEM_LIMIT) {
        let r = ui::row(&path_text(&i.path), "");
        r.set_tooltip_text(Some(&i.path.to_string_lossy()));
        r.add_css_class("item");
        if i.bytes > 0 {
            r.add_suffix(&ui::value(&i18n::bytes(i.bytes)));
        }
        e.add_row(&r);
    }
    if c.item_count > ITEM_LIMIT {
        let r = ui::row(
            &tv("clean.more", &[("n", (c.item_count - ITEM_LIMIT).into())]),
            "",
        );
        r.add_css_class("dim-label");
        e.add_row(&r);
    }
    let notes = gtk::Box::new(gtk::Orientation::Vertical, 6);
    if c.truncated {
        notes.append(&ui::dim_label(&tv(
            "clean.truncated",
            &[("n", c.item_count.into())],
        )));
    }
    if let Some(l) = &c.locked {
        let dirs = l
            .dirs
            .iter()
            .map(|d| path_text(d))
            .collect::<Vec<_>>()
            .join(", ");
        notes.append(&ui::dim_label(&tv(
            "clean.locked",
            &[
                ("n", l.items.into()),
                ("size", i18n::bytes(l.bytes).into()),
                ("dirs", dirs.into()),
            ],
        )));
        notes.append(&ui::command(&l.command));
    }
    if c.method == Method::Privileged {
        if let Some(cmd) = &c.manual_command {
            notes.append(&ui::dim_label(&t("clean.orTerminal")));
            notes.append(&ui::command(cmd));
        }
    }
    if notes.first_child().is_some() {
        notes.set_margin_top(10);
        notes.set_margin_bottom(10);
        notes.set_margin_start(12);
        notes.set_margin_end(12);
        e.add_row(&notes);
    }
}

impl Cleanup {
    pub fn new() -> Rc<Cleanup> {
        let (scroll, content) = ui::page(900);

        let head = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let last_scan = ui::dim_label("");
        last_scan.set_hexpand(true);
        let rescan = ui::button(&t("clean.rescan"));
        head.append(&last_scan);
        head.append(&rescan);
        content.append(&head);

        let progress = gtk::ProgressBar::new();
        let progress_text = ui::dim_label("");
        progress_text.add_css_class("caption");
        let pbox = gtk::Box::new(gtk::Orientation::Vertical, 4);
        pbox.append(&progress);
        pbox.append(&progress_text);
        content.append(&pbox);

        let filters = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let search = gtk::SearchEntry::builder()
            .placeholder_text(t("clean.search"))
            .hexpand(true)
            .build();
        filters.append(&search);
        let seg = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        seg.add_css_class("linked");
        seg.set_tooltip_text(Some(&t("clean.filter")));
        // Filtre düğmeleri sığmazsa yana kaydırılır; pencereyi genişletmez.
        let seg_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_width(true)
            .child(&seg)
            .build();
        filters.append(&seg_scroll);
        content.append(&filters);

        let list = gtk::Box::new(gtk::Orientation::Vertical, 24);
        content.append(&list);

        let total = gtk::Label::builder().xalign(0.0).build();
        total.add_css_class("title-3");
        total.add_css_class("numeric");
        let count = ui::dim_label("");
        count.add_css_class("caption");
        let sums = gtk::Box::new(gtk::Orientation::Vertical, 0);
        sums.append(&total);
        sums.append(&count);
        let review = ui::pill(&t("clean.review"), "suggested-action");
        let bar = gtk::ActionBar::new();
        bar.pack_start(&sums);
        bar.pack_end(&review);

        let tv_ = adw::ToolbarView::new();
        tv_.set_content(Some(&scroll));
        tv_.add_bottom_bar(&bar);

        let root = gtk::Stack::new();
        let waiting = ui::status_page(
            "edit-clear-all-symbolic",
            &t("clean.scanning"),
            &t("clean.scanningLong"),
        );
        root.add_named(&waiting, Some("waiting"));
        root.add_named(&tv_, Some("main"));

        let page = Rc::new(Cleanup {
            root,
            filters,
            narrow: Cell::new(false),
            last_scan,
            rescan,
            progress,
            progress_text,
            search,
            list,
            total,
            count,
            review,
            filter: Cell::new(Filter::All),
            selected: RefCell::new(None),
            picked: RefCell::default(),
        });

        let mut first: Option<gtk::ToggleButton> = None;
        for (f, key) in FILTERS {
            let b = gtk::ToggleButton::builder()
                .label(t(key))
                .active(f == Filter::All)
                .build();
            if let Some(g) = &first {
                b.set_group(Some(g));
            } else {
                first = Some(b.clone());
            }
            let p = Rc::downgrade(&page);
            b.connect_toggled(move |b| {
                if let (true, Some(p)) = (b.is_active(), p.upgrade()) {
                    p.filter.set(f);
                    p.rebuild_list();
                }
            });
            seg.append(&b);
        }
        {
            let p = Rc::downgrade(&page);
            page.search.connect_search_changed(move |_| {
                if let Some(p) = p.upgrade() {
                    p.rebuild_list();
                }
            });
        }
        page.rescan.connect_clicked(|_| rescan_or_cancel());
        {
            let p = Rc::downgrade(&page);
            page.review.connect_clicked(move |_| {
                if let Some(p) = p.upgrade() {
                    p.create_plan();
                }
            });
        }
        page.update_progress();
        page
    }

    /// Dar pencerede arama kutusu ile filtre düğmeleri alt alta durur.
    /// Satırlardaki sabit genişlikler de küçülür.
    pub fn set_narrow(self: &Rc<Self>, on: bool) {
        self.filters.set_orientation(if on {
            gtk::Orientation::Vertical
        } else {
            gtk::Orientation::Horizontal
        });
        if self.narrow.replace(on) != on {
            self.rebuild();
        }
    }

    pub fn widget(&self) -> gtk::Widget {
        self.root.clone().upcast()
    }

    /// Yeni tarama sonucu geldi: seçimler korunur, yalnızca artık olmayan öğeler düşer.
    pub fn rebuild(self: &Rc<Self>) {
        let s = shell();
        let d = s.data.borrow();
        let Some(scan) = &d.scan else {
            self.root.set_visible_child_name("waiting");
            return;
        };
        self.root.set_visible_child_name("main");
        let present: HashSet<String> = scan
            .categories
            .iter()
            .flat_map(|c| {
                c.items
                    .iter()
                    .map(|i| i.path.to_string_lossy().into_owned())
            })
            .collect();
        self.picked.borrow_mut().retain(|p| present.contains(p));
        drop(d);
        self.update_last_scan();
        self.rebuild_list();
        self.update_progress();
    }

    fn selection(&self, cats: &[CategoryScan]) -> HashSet<CategoryId> {
        self.selected.borrow().clone().unwrap_or_else(|| {
            cats.iter()
                .filter(|c| is_default(c))
                .map(|c| c.id)
                .collect()
        })
    }

    fn toggle_category(&self, id: CategoryId, on: bool) {
        let cats = shell()
            .data
            .borrow()
            .scan
            .as_ref()
            .map(|s| s.categories.clone())
            .unwrap_or_default();
        let mut sel = self.selection(&cats);
        if on {
            sel.insert(id);
        } else {
            sel.remove(&id);
        }
        *self.selected.borrow_mut() = Some(sel);
        self.update_total();
    }

    fn toggle_pick(&self, path: String, on: bool) {
        let mut p = self.picked.borrow_mut();
        if on {
            p.insert(path);
        } else {
            p.remove(&path);
        }
        drop(p);
        self.update_total();
    }

    fn rebuild_list(self: &Rc<Self>) {
        ui::clear_box(&self.list);
        let s = shell();
        let d = s.data.borrow();
        let Some(scan) = &d.scan else { return };
        let q = self.search.text().trim().to_lowercase();
        let shown: Vec<&CategoryScan> = scan
            .categories
            .iter()
            .filter(|c| matches(c, self.filter.get(), &q))
            .collect();
        let sel = self.selection(&scan.categories);
        let largest = scan
            .categories
            .iter()
            .map(|c| c.bytes)
            .max()
            .unwrap_or(1)
            .max(1);

        // Bosun'un kendi temizleyebildikleri
        let user: Vec<_> = shown
            .iter()
            .filter(|c| c.bulk_selectable && (c.item_count > 0 || c.locked.is_some()))
            .collect();
        let clean = shown
            .iter()
            .filter(|c| c.bulk_selectable && c.item_count == 0 && c.locked.is_none())
            .count();
        if !user.is_empty() || clean > 0 {
            let g = ui::group(&t("clean.groupUser"), "");
            let l = ui::boxed_list();
            for c in user {
                let e = ui::expander(&c.title, &c.description);
                e.set_subtitle_lines(2);
                let check = gtk::CheckButton::builder()
                    .active(sel.contains(&c.id))
                    .valign(gtk::Align::Center)
                    .margin_start(4)
                    .margin_end(12)
                    .build();
                check.set_sensitive(c.item_count > 0);
                check.set_tooltip_text(Some(&c.title));
                let (p, id) = (Rc::downgrade(self), c.id);
                check.connect_toggled(move |b| {
                    if let Some(p) = p.upgrade() {
                        p.toggle_category(id, b.is_active());
                    }
                });
                e.add_prefix(&check);
                // Boyut, çubuk ve etiket sabit genişlikte: satırlar alt alta hizalı kalsın.
                let side = gtk::Box::new(gtk::Orientation::Vertical, 4);
                side.set_valign(gtk::Align::Center);
                let narrow = self.narrow.get();
                side.set_width_request(if narrow { 64 } else { 100 });
                let size = ui::value(&size_text(c));
                size.set_xalign(1.0);
                side.append(&size);
                side.append(&ui::bar(c.bytes as f64 / largest as f64, false));
                e.add_suffix(&side);
                let tag = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                if !narrow {
                    tag.set_width_request(84);
                }
                tag.set_halign(gtk::Align::End);
                let label = safety_tag(c);
                label.set_hexpand(true);
                label.set_halign(gtk::Align::End);
                tag.append(&label);
                e.add_suffix(&tag);
                add_items(&e, c);
                l.append(&e);
            }
            if clean > 0 {
                let r = ui::row(&tv("clean.alreadyClean", &[("n", clean.into())]), "");
                r.add_css_class("dim-label");
                l.append(&r);
            }
            g.add(&l);
            self.list.append(&g);
        }

        // Tek tek seçilecekler (büyük dosyalar, eski program klasörleri)
        for c in shown
            .iter()
            .filter(|c| c.safety == Safety::ReviewOnly && !c.items.is_empty())
        {
            let g = ui::group(
                &format!("{} · {}", c.title, t("clean.pickHint")),
                &c.description,
            );
            let all = c
                .items
                .iter()
                .all(|i| self.picked.borrow().contains(&*i.path.to_string_lossy()));
            let toggle = ui::button(&if all {
                t("clean.selectNone")
            } else {
                tv("clean.selectAll", &[("n", c.items.len().into())])
            });
            toggle.add_css_class("flat");
            let paths: Vec<String> = c
                .items
                .iter()
                .map(|i| i.path.to_string_lossy().into_owned())
                .collect();
            let p = Rc::downgrade(self);
            toggle.connect_clicked(move |_| {
                if let Some(p) = p.upgrade() {
                    {
                        let mut set = p.picked.borrow_mut();
                        for x in &paths {
                            if all {
                                set.remove(x);
                            } else {
                                set.insert(x.clone());
                            }
                        }
                    }
                    p.rebuild_list();
                }
            });
            g.set_header_suffix(Some(&toggle));
            let l = ui::boxed_list();
            for i in &c.items {
                let path = i.path.to_string_lossy().into_owned();
                let when = i
                    .modified
                    .map(|m| format!(" · {}", i18n::ago(m)))
                    .unwrap_or_default();
                let r = ui::row(
                    &path_text(&i.path),
                    &format!("{}{when}", i18n::bytes(i.bytes)),
                );
                r.set_tooltip_text(Some(&path));
                let check = gtk::CheckButton::builder()
                    .active(self.picked.borrow().contains(&path))
                    .valign(gtk::Align::Center)
                    .build();
                let (p, x) = (Rc::downgrade(self), path.clone());
                check.connect_toggled(move |b| {
                    if let Some(p) = p.upgrade() {
                        p.toggle_pick(x.clone(), b.is_active());
                    }
                });
                r.add_prefix(&check);
                r.set_activatable_widget(Some(&check));
                if c.id == CategoryId::LeftoverConfigs {
                    r.add_suffix(&look_inside(&path));
                }
                l.append(&r);
            }
            g.add(&l);
            self.list.append(&g);
        }

        // Yönetici yetkisi gerekenler
        let admin: Vec<_> = shown
            .iter()
            .filter(|c| c.method == Method::Privileged && c.item_count > 0)
            .collect();
        if !admin.is_empty() {
            let g = ui::group(&t("clean.groupAdmin"), "");
            let l = ui::boxed_list();
            for c in admin {
                let e = ui::expander(&c.title, &c.description);
                e.set_subtitle_lines(2);
                e.add_prefix(&gtk::Image::from_icon_name("security-high-symbolic"));
                let size = if c.bytes > 0 {
                    i18n::bytes(c.bytes)
                } else {
                    tv("clean.packages", &[("n", c.item_count.into())])
                };
                e.add_suffix(&ui::value(&size));
                if let Some(action) = &c.admin_action {
                    let b = ui::button(&t("clean.adminRun"));
                    let action = action.clone();
                    let items: Vec<String> = c
                        .items
                        .iter()
                        .map(|i| i.path.to_string_lossy().into_owned())
                        .collect();
                    b.connect_clicked(move |_| dialogs::admin(&action, items.clone()));
                    e.add_suffix(&b);
                }
                add_items(&e, c);
                l.append(&e);
            }
            g.add(&l);
            self.list.append(&g);
        }
        drop(d);
        if self.list.first_child().is_none() {
            self.list.append(&ui::status_page(
                "edit-find-symbolic",
                &t("clean.noMatch"),
                "",
            ));
        }
        self.update_total();
    }

    /// "Son tarama 3 dk önce" metnini tazeler.
    pub fn update_last_scan(&self) {
        if let Some(scan) = &shell().data.borrow().scan {
            self.last_scan
                .set_label(&tv("clean.lastScan", &[("ago", i18n::ago(scan.ts).into())]));
        }
    }

    fn update_total(&self) {
        let s = shell();
        let d = s.data.borrow();
        let Some(scan) = &d.scan else { return };
        let sel = self.selection(&scan.categories);
        let picked = self.picked.borrow();
        let picked_bytes: u64 = scan
            .categories
            .iter()
            .filter(|c| c.safety == Safety::ReviewOnly)
            .flat_map(|c| &c.items)
            .filter(|i| picked.contains(&*i.path.to_string_lossy()))
            .map(|i| i.bytes)
            .sum();
        let total: u64 = scan
            .categories
            .iter()
            .filter(|c| sel.contains(&c.id))
            .map(|c| c.bytes)
            .sum::<u64>()
            + picked_bytes;
        let n = sel.len() + picked.len();
        self.total.set_label(&i18n::bytes(total));
        self.count.set_label(&if n == 0 {
            t("clean.nothing")
        } else {
            tv("clean.selected", &[("n", n.into())])
        });
        self.review.set_sensitive(n > 0);
    }

    pub fn update_progress(&self) {
        let s = shell();
        let d = s.data.borrow();
        let scanning = d.scanning;
        self.progress.set_visible(scanning);
        self.progress_text.set_visible(scanning);
        self.rescan.set_label(&t(if scanning {
            "clean.cancelScan"
        } else {
            "clean.rescan"
        }));
        match &d.progress {
            Some(p) if p.total > 0 => {
                self.progress.set_fraction(p.done as f64 / p.total as f64);
                self.progress_text.set_label(&tv(
                    "clean.progress",
                    &[
                        ("title", p.title.clone().into()),
                        ("done", (p.done + 1).into()),
                        ("total", p.total.into()),
                    ],
                ));
            }
            _ => {
                self.progress.pulse();
                self.progress_text.set_label(&t("clean.scanning"));
            }
        }
    }

    fn create_plan(&self) {
        let cats = shell()
            .data
            .borrow()
            .scan
            .as_ref()
            .map(|s| s.categories.clone())
            .unwrap_or_default();
        let sel: Vec<CategoryId> = self.selection(&cats).into_iter().collect();
        let picked: Vec<String> = self.picked.borrow().iter().cloned().collect();
        let review = self.review.clone();
        review.set_sensitive(false);
        glib::spawn_future_local(async move {
            let r = client::get::<bosun_core::CleanupPlan>(
                "plan.create",
                json!({"categories": sel, "picked": picked}),
            )
            .await;
            review.set_sensitive(true);
            match r {
                Ok(plan) => dialogs::plan(plan),
                Err(e) => shell().toast(&ui::error_text(&e)),
            }
        });
    }
}

fn rescan_or_cancel() {
    let s = shell();
    let scanning = s.data.borrow().scanning;
    if scanning {
        glib::spawn_future_local(async {
            let _ = client::call("scan.cancel", serde_json::Value::Null).await;
        });
        return;
    }
    {
        let mut d = s.data.borrow_mut();
        d.scanning = true;
        d.progress = None;
    }
    crate::shell::ui().cleanup.update_progress();
    glib::spawn_future_local(async {
        let r = client::get("scan", json!({"refresh": true, "limit": 40})).await;
        let s = shell();
        {
            let mut d = s.data.borrow_mut();
            d.scanning = false;
            d.progress = None;
            if let Ok(scan) = r {
                d.scan = Some(scan);
            }
        }
        crate::shell::ui().cleanup.rebuild();
        s.update_status();
    });
}

/// "İçine bak": klasörün en büyük çocuklarını bir açılır pencerede gösterir.
fn look_inside(path: &str) -> gtk::MenuButton {
    let pop = gtk::Popover::new();
    let b = gtk::MenuButton::builder()
        .label(t("clean.lookInside"))
        .valign(gtk::Align::Center)
        .popover(&pop)
        .build();
    b.add_css_class("flat");
    let path = path.to_string();
    let loaded = Rc::new(Cell::new(false));
    pop.connect_show(move |pop| {
        if loaded.replace(true) {
            return;
        }
        let body = gtk::Box::new(gtk::Orientation::Vertical, 6);
        body.set_margin_top(6);
        body.set_margin_bottom(6);
        body.set_margin_start(6);
        body.set_margin_end(6);
        body.set_width_request(320);
        body.append(&ui::dim_label(&t("clean.peekLoading")));
        pop.set_child(Some(&body));
        let (body, path, loaded) = (body.clone(), path.clone(), loaded.clone());
        glib::spawn_future_local(async move {
            let r = client::get::<Inspection>("scan.inspect", json!({ "path": path })).await;
            ui::clear_box(&body);
            match r {
                Ok(v) => {
                    let newest = v.newest.map(i18n::ago).unwrap_or_else(|| "—".into());
                    let sum = ui::label(&tv(
                        "clean.peekSummary",
                        &[
                            ("files", v.files.into()),
                            ("size", i18n::bytes(v.bytes).into()),
                            ("ago", newest.into()),
                        ],
                    ));
                    sum.add_css_class("heading");
                    body.append(&sum);
                    for e in v.entries {
                        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
                        let name =
                            ui::label(&format!("{}{}", if e.is_dir { "▸ " } else { "" }, e.name));
                        name.set_hexpand(true);
                        row.append(&name);
                        let size = ui::value(&i18n::bytes(e.bytes));
                        size.add_css_class("dim-label");
                        row.append(&size);
                        body.append(&row);
                    }
                }
                Err(e) => {
                    let l = ui::label(&ui::error_text(&e));
                    l.add_css_class("error");
                    body.append(&l);
                    loaded.set(false);
                }
            }
        });
    });
    b
}
