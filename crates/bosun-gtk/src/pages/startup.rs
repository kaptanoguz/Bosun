//! Oturum açılışında başlayan uygulamalar. Kapatmak hiçbir şeyi silmez; geri açılabilir.

use crate::client;
use crate::i18n::{t, tv};
use crate::shell::shell;
use crate::ui;
use adw::prelude::*;
use bosun_core::autostart::StartupApp;
use gtk::glib;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub struct Startup {
    root: gtk::Box,
    count: gtk::Label,
    list: gtk::Box,
    apps: RefCell<Vec<StartupApp>>,
    loaded: Cell<bool>,
}

impl Startup {
    pub fn new() -> Rc<Startup> {
        let (scroll, content) = ui::page(900);
        content.append(&ui::dim_label(&t("startup.lede")));
        let count = ui::label("");
        count.add_css_class("heading");
        content.append(&count);
        let list = gtk::Box::new(gtk::Orientation::Vertical, 24);
        list.append(&ui::dim_label(&t("startup.loading")));
        content.append(&list);
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&scroll);
        Rc::new(Startup {
            root,
            count,
            list,
            apps: RefCell::default(),
            loaded: Cell::new(false),
        })
    }

    pub fn widget(&self) -> gtk::Widget {
        self.root.clone().upcast()
    }

    pub fn on_show(self: &Rc<Self>) {
        if self.loaded.replace(true) {
            return;
        }
        let p = self.clone();
        glib::spawn_future_local(async move {
            match client::call("startup.list", Value::Null).await {
                Ok(v) => {
                    *p.apps.borrow_mut() =
                        serde_json::from_value(v["apps"].clone()).unwrap_or_default();
                    p.render();
                }
                Err(e) => {
                    p.loaded.set(false);
                    ui::clear_box(&p.list);
                    let l = ui::label(&ui::error_text(&e));
                    l.add_css_class("error");
                    p.list.append(&l);
                }
            }
        });
    }

    fn render(self: &Rc<Self>) {
        ui::clear_box(&self.list);
        let apps = self.apps.borrow();
        let on = apps.iter().filter(|a| a.enabled).count();
        self.count.set_label(&tv(
            "startup.count",
            &[("on", on.into()), ("total", apps.len().into())],
        ));
        if apps.is_empty() {
            self.list.append(&ui::dim_label(&t("startup.none")));
            return;
        }
        let mine = ui::boxed_list();
        let desktop = ui::expander(
            &tv(
                "startup.showDesktop",
                &[("n", apps.iter().filter(|a| a.essential).count().into())],
            ),
            "",
        );
        for a in apps.iter() {
            let source = if a.essential {
                t("startup.desktop")
            } else if a.system {
                t("startup.system")
            } else {
                t("startup.yours")
            };
            let detail = if a.comment.is_empty() {
                &a.exec
            } else {
                &a.comment
            };
            let r = adw::SwitchRow::builder()
                .use_markup(false)
                .title(&a.name)
                .subtitle(format!("{source} · {detail}"))
                .active(a.enabled)
                .build();
            r.set_tooltip_text(Some(&a.exec));
            if a.missing {
                r.add_suffix(&ui::tag(&t("startup.missing"), "warning"));
            }
            if a.essential && a.enabled {
                r.set_sensitive(false);
                r.set_tooltip_text(Some(&t("startup.essentialHint")));
            }
            let (p, file) = (Rc::downgrade(self), a.file.clone());
            r.connect_active_notify(move |r| {
                if let Some(p) = p.upgrade() {
                    p.toggle(r.clone(), file.clone());
                }
            });
            if a.essential {
                desktop.add_row(&r);
            } else {
                mine.append(&r);
            }
        }
        self.list.append(&mine);
        if apps.iter().any(|a| a.essential) {
            let l = ui::boxed_list();
            l.append(&desktop);
            self.list.append(&l);
        }
    }

    fn toggle(self: &Rc<Self>, row: adw::SwitchRow, file: String) {
        let want = row.is_active();
        let current = self
            .apps
            .borrow()
            .iter()
            .find(|a| a.file == file)
            .map(|a| a.enabled);
        if current == Some(want) {
            return; // geri alma sırasında tetiklenen bildirim
        }
        row.set_sensitive(false);
        let p = self.clone();
        glib::spawn_future_local(async move {
            let r =
                client::get::<StartupApp>("startup.set", json!({ "file": file, "enabled": want }))
                    .await;
            row.set_sensitive(true);
            match r {
                Ok(updated) => {
                    let key = if updated.enabled {
                        "startup.turnedOn"
                    } else {
                        "startup.turnedOff"
                    };
                    shell().toast(&tv(key, &[("name", updated.name.clone().into())]));
                    if let Some(a) = p
                        .apps
                        .borrow_mut()
                        .iter_mut()
                        .find(|a| a.file == updated.file)
                    {
                        *a = updated;
                    }
                    let apps = p.apps.borrow();
                    let on = apps.iter().filter(|a| a.enabled).count();
                    p.count.set_label(&tv(
                        "startup.count",
                        &[("on", on.into()), ("total", apps.len().into())],
                    ));
                    crate::shell::ui().history.mark_dirty();
                }
                Err(e) => {
                    shell().toast(&ui::error_text(&e));
                    row.set_active(!want);
                }
            }
        });
    }
}
