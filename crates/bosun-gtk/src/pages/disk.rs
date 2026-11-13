//! Disk kullanımı: ev klasöründe yer kaplayanlar, büyükten küçüğe. Ölçer; dosya silmek
//! yalnızca onaylanmış bir planla, çöp kutusuna taşıyarak yapılır.

use crate::client;
use crate::i18n::{self, t, tv};
use crate::shell::shell;
use crate::ui;
use adw::prelude::*;
use bosun_core::usage::Listing;
use gtk::glib;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// Bu kadar satırdan sonrası tek satırda özetlenir.
const MAX_ROWS: usize = 40;

/// Ev klasöründen açık klasöre kadar olan yol parçaları.
pub fn crumbs(path: &Path, home: &Path) -> Vec<(String, PathBuf)> {
    let mut out = vec![(String::new(), home.to_path_buf())];
    if let Ok(rest) = path.strip_prefix(home) {
        let mut cur = home.to_path_buf();
        for part in rest.components() {
            cur.push(part);
            out.push((part.as_os_str().to_string_lossy().into_owned(), cur.clone()));
        }
    }
    out
}

pub struct Disk {
    root: gtk::Box,
    crumbs: gtk::Box,
    total: gtk::Label,
    refresh: gtk::Button,
    spinner: gtk::Spinner,
    list: gtk::ListBox,
    note: gtk::Label,
    error: gtk::Label,
    listing: RefCell<Option<(Listing, PathBuf)>>,
    loaded: Cell<bool>,
    /// Dar pencerede çubuk ve yüzde gizlenir; ad ve boyut kalır.
    narrow: Cell<bool>,
}

impl Disk {
    pub fn new() -> Rc<Disk> {
        let (scroll, content) = ui::page(900);
        let intro = ui::dim_label(&t("disk.intro"));
        content.append(&intro);

        let head = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let crumbs = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        crumbs.set_hexpand(true);
        crumbs.add_css_class("crumbs");
        let total = gtk::Label::new(None);
        total.add_css_class("dim-label");
        let spinner = gtk::Spinner::new();
        let refresh = ui::button(&t("disk.refresh"));
        head.append(&crumbs);
        head.append(&total);
        head.append(&spinner);
        head.append(&refresh);
        content.append(&head);

        let error = ui::label("");
        error.add_css_class("error");
        error.set_visible(false);
        content.append(&error);

        let list = ui::boxed_list();
        content.append(&list);
        let note = ui::dim_label("");
        note.add_css_class("caption");
        content.append(&note);

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&scroll);
        let page = Rc::new(Disk {
            root,
            crumbs,
            total,
            refresh,
            spinner,
            list,
            note,
            error,
            listing: RefCell::new(None),
            loaded: Cell::new(false),
            narrow: Cell::new(false),
        });
        {
            let p = Rc::downgrade(&page);
            page.refresh.connect_clicked(move |_| {
                if let Some(p) = p.upgrade() {
                    let path = p.listing.borrow().as_ref().map(|(l, _)| l.path.clone());
                    p.open(path, true);
                }
            });
        }
        page
    }

    pub fn set_narrow(self: &Rc<Self>, on: bool) {
        if self.narrow.replace(on) != on && self.listing.borrow().is_some() {
            self.render();
        }
    }

    pub fn widget(&self) -> gtk::Widget {
        self.root.clone().upcast()
    }

    pub fn on_show(self: &Rc<Self>) {
        if !self.loaded.replace(true) {
            self.open(None, false);
        }
    }

    /// Tarama bittiğinde "temizlenebilir" boyutlarını tazele (boyutlar önbellekte, hızlıdır).
    pub fn mark_cleanable(self: &Rc<Self>) {
        self.reload();
    }

    /// Bir temizlikten sonra açık klasörü yeniden ölç (sunucu boyut önbelleğini boşalttı).
    pub fn reload(self: &Rc<Self>) {
        let path = self.listing.borrow().as_ref().map(|(l, _)| l.path.clone());
        if let Some(path) = path {
            self.open(Some(path), false);
        }
    }

    fn open(self: &Rc<Self>, path: Option<PathBuf>, refresh: bool) {
        self.spinner.start();
        self.refresh.set_sensitive(false);
        self.error.set_visible(false);
        self.list.set_sensitive(false);
        let p = self.clone();
        glib::spawn_future_local(async move {
            let params = match &path {
                Some(path) => json!({ "path": path, "refresh": refresh }),
                None => json!({ "refresh": refresh }),
            };
            let r = client::call("usage", params).await;
            p.spinner.stop();
            p.refresh.set_sensitive(true);
            p.list.set_sensitive(true);
            match r.and_then(|v: Value| {
                let home = PathBuf::from(v["home"].as_str().unwrap_or("/"));
                Ok((serde_json::from_value::<Listing>(v)?, home))
            }) {
                Ok(l) => {
                    *p.listing.borrow_mut() = Some(l);
                    p.render();
                }
                Err(e) => {
                    p.error.set_label(&ui::error_text(&e));
                    p.error.set_visible(true);
                    p.loaded.set(false);
                }
            }
        });
    }

    /// Seçilen dosya için plan ister ve onay penceresini açar. Silme orada onaylanınca olur.
    fn plan_trash(self: &Rc<Self>, path: PathBuf) {
        glib::spawn_future_local(async move {
            let r =
                client::get::<bosun_core::CleanupPlan>("disk.plan", json!({ "path": path })).await;
            match r {
                Ok(plan) => crate::dialogs::plan(plan),
                Err(e) => shell().toast(&ui::error_text(&e)),
            }
        });
    }

    /// Bu klasörün altındaki temizlenebilir veriler için plan ister; onay penceresinde
    /// neyin çöpe taşınacağı tek tek görünür.
    fn plan_clean(self: &Rc<Self>, path: PathBuf) {
        glib::spawn_future_local(async move {
            let r =
                client::get::<bosun_core::CleanupPlan>("disk.clean", json!({ "path": path })).await;
            match r {
                Ok(plan) => crate::dialogs::plan(plan),
                Err(e) => shell().toast(&ui::error_text(&e)),
            }
        });
    }

    fn render(self: &Rc<Self>) {
        let guard = self.listing.borrow();
        let Some((l, home)) = guard.as_ref() else {
            return;
        };
        ui::clear_box(&self.crumbs);
        let parts = crumbs(&l.path, home);
        let last = parts.len() - 1;
        for (i, (name, path)) in parts.into_iter().enumerate() {
            if i > 0 {
                let sep = ui::dim_label("/");
                sep.set_margin_start(2);
                sep.set_margin_end(2);
                self.crumbs.append(&sep);
            }
            let name = if name.is_empty() {
                t("disk.home")
            } else {
                name
            };
            if i == last {
                let b = gtk::Label::new(Some(&name));
                b.add_css_class("heading");
                b.set_margin_start(6);
                self.crumbs.append(&b);
            } else {
                let b = ui::button(&name);
                b.add_css_class("flat");
                let p = Rc::downgrade(self);
                b.connect_clicked(move |_| {
                    if let Some(p) = p.upgrade() {
                        p.open(Some(path.clone()), false);
                    }
                });
                self.crumbs.append(&b);
            }
        }
        self.total.set_label(&tv(
            "disk.total",
            &[
                ("size", i18n::bytes(l.bytes).into()),
                ("n", l.entries.len().into()),
            ],
        ));

        ui::clear_list(&self.list);
        if l.entries.is_empty() {
            self.list.append(&ui::row(&t("disk.empty"), ""));
        }
        let largest = l.entries.first().map(|e| e.bytes).unwrap_or(0).max(1);
        for e in l.entries.iter().take(MAX_ROWS) {
            let share = if l.bytes > 0 {
                i18n::percent(e.bytes as f64 / l.bytes as f64 * 100.0)
            } else {
                "—".into()
            };
            let r = ui::row(&e.name, "");
            r.add_prefix(&gtk::Image::from_icon_name(if e.is_dir {
                "folder-symbolic"
            } else {
                "text-x-generic-symbolic"
            }));
            if e.other_fs {
                r.add_suffix(&ui::tag(&t("disk.otherFs"), "accent"));
            }
            let cleanable = e.cleanable;
            if cleanable > 0 {
                let tag = ui::button(&tv(
                    "disk.cleanable",
                    &[("size", i18n::bytes(cleanable).into())],
                ));
                tag.add_css_class("flat");
                tag.add_css_class("tag-button");
                tag.add_css_class("success");
                tag.set_tooltip_text(Some(&t("disk.cleanTip")));
                let (p, path) = (Rc::downgrade(self), e.path.clone());
                tag.connect_clicked(move |_| {
                    if let Some(p) = p.upgrade() {
                        p.plan_clean(path.clone());
                    }
                });
                r.add_suffix(&tag);
            }
            let narrow = self.narrow.get();
            if e.bytes > 0 && !narrow {
                let bar = ui::bar(e.bytes as f64 / largest as f64, false);
                bar.set_width_request(140);
                bar.set_valign(gtk::Align::Center);
                r.add_suffix(&bar);
            }
            let pct = ui::value(&share);
            pct.add_css_class("dim-label");
            pct.set_width_chars(5);
            pct.set_xalign(1.0);
            if !narrow {
                r.add_suffix(&pct);
            }
            let size = ui::value(&if e.other_fs {
                "—".into()
            } else {
                i18n::bytes(e.bytes)
            });
            size.set_width_chars(8);
            size.set_xalign(1.0);
            r.add_suffix(&size);
            if !e.is_dir && !e.other_fs {
                let trash = ui::icon_button("user-trash-symbolic", &t("disk.trashTip"));
                let (p, path) = (Rc::downgrade(self), e.path.clone());
                trash.connect_clicked(move |_| {
                    if let Some(p) = p.upgrade() {
                        p.plan_trash(path.clone());
                    }
                });
                r.add_suffix(&trash);
            }
            let go = gtk::Image::from_icon_name("go-next-symbolic");
            r.add_suffix(&go);
            if !e.is_dir || e.other_fs {
                go.set_opacity(0.0); // yer tutucu: boyutlar hizalı kalsın
            } else {
                r.set_activatable(true);
                let (p, path) = (Rc::downgrade(self), e.path.clone());
                r.connect_activated(move |_| {
                    if let Some(p) = p.upgrade() {
                        p.open(Some(path.clone()), false);
                    }
                });
            }
            self.list.append(&r);
        }
        let rest = &l.entries[l.entries.len().min(MAX_ROWS)..];
        if !rest.is_empty() {
            let size: u64 = rest.iter().map(|e| e.bytes).sum();
            let r = ui::row(
                &tv(
                    "disk.smaller",
                    &[("n", rest.len().into()), ("size", i18n::bytes(size).into())],
                ),
                "",
            );
            r.add_css_class("dim-label");
            self.list.append(&r);
        }
        self.note.set_visible(l.unreadable > 0);
        self.note
            .set_label(&tv("disk.unreadable", &[("n", l.unreadable.into())]));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crumbs_walk_down_from_home() {
        let c = crumbs(Path::new("/home/a/x/y"), Path::new("/home/a"));
        let names: Vec<_> = c.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["", "x", "y"]);
        assert_eq!(c[2].1, PathBuf::from("/home/a/x/y"));
        assert_eq!(crumbs(Path::new("/home/a"), Path::new("/home/a")).len(), 1);
    }
}
