//! Onay pencereleri: temizlik planı ve yönetici eylemleri. Onay olmadan hiçbir şey çalışmaz.

use crate::client::{self, AdminOutcome};
use crate::i18n::{self, t, tv};
use crate::shell::{shell, ui as app_ui};
use crate::ui;
use adw::prelude::*;
use bosun_core::admin::{ActionInfo, HelperStatus, HELPER_PATH};
use bosun_core::execute::ExecutionReport;
use bosun_core::{CleanupPlan, Method};
use chrono::Utc;
use gtk::glib;
use serde_json::{json, Value};
use std::cell::Cell;
use std::rc::Rc;

fn body_box() -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Vertical, 12);
    b.set_margin_top(12);
    b.set_margin_bottom(24);
    b.set_margin_start(24);
    b.set_margin_end(24);
    b
}

/// Plan onay penceresi. Kapatılırsa plan servis tarafında iptal edilir.
pub fn plan(plan: CleanupPlan) {
    let dialog = adw::Dialog::builder()
        .title(t("plan.title"))
        .content_width(560)
        .build();
    let done = Rc::new(Cell::new(false));
    let plan = Rc::new(plan);

    let tv_ = adw::ToolbarView::new();
    tv_.add_top_bar(&adw::HeaderBar::new());
    let body = body_box();

    let intro = ui::dim_label(plan.reason.as_deref().unwrap_or(&t("plan.intro")));
    body.append(&intro);
    let big = ui::label(&i18n::bytes(plan.total_bytes));
    big.add_css_class("title-1");
    big.add_css_class("numeric");
    body.append(&big);
    body.append(&ui::dim_label(&format!(
        "{} · {}",
        tv("plan.items", &[("n", plan.items.len().into())]),
        t(if plan.has_permanent {
            "plan.partlyPermanent"
        } else {
            "plan.undoable"
        })
    )));
    if plan.has_permanent {
        let warn = ui::label(&t("plan.permanentWarning"));
        warn.add_css_class("callout");
        warn.add_css_class("error");
        body.append(&warn);
    }

    let list = ui::boxed_list();
    for s in &plan.summary {
        let count = tv("plan.items", &[("n", s.items.into())]);
        let sub = if s.bytes > 0 {
            format!("{} · {count}", i18n::bytes(s.bytes))
        } else {
            count
        };
        let e = ui::expander(&s.title, &sub);
        match s.method {
            Method::Permanent => e.add_suffix(&ui::tag(&t("plan.permanentTag"), "error")),
            Method::Unlink => e.add_suffix(&ui::tag(&t("clean.linkOnly"), "success")),
            _ => {}
        }
        let items: Vec<_> = plan
            .items
            .iter()
            .filter(|i| i.category == s.category)
            .collect();
        for i in items.iter().take(20) {
            let r = ui::row(&i18n::short_path(&i.path.to_string_lossy()), "");
            r.set_tooltip_text(Some(&i.path.to_string_lossy()));
            if i.bytes > 0 {
                r.add_suffix(&ui::value(&i18n::bytes(i.bytes)));
            }
            e.add_row(&r);
        }
        if items.len() > 20 {
            e.add_row(&ui::row(
                &tv("plan.more", &[("n", (items.len() - 20).into())]),
                "",
            ));
        }
        list.append(&e);
    }
    body.append(&list);

    if !plan.blocked.is_empty() {
        let h = ui::label(&t("plan.notIncluded"));
        h.add_css_class("heading");
        h.set_margin_top(8);
        body.append(&h);
        for b in &plan.blocked {
            body.append(&ui::dim_label(&b.reason));
            if let Some(cmd) = &b.manual_command {
                body.append(&ui::command(cmd));
            }
        }
    }
    let error = ui::label("");
    error.add_css_class("error");
    error.set_visible(false);
    body.append(&error);

    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(560)
        .child(&body)
        .build();
    tv_.set_content(Some(&scroll));

    let countdown = ui::dim_label("");
    countdown.add_css_class("caption");
    countdown.set_hexpand(true);
    let cancel = ui::button(&t("plan.cancel"));
    let label = if plan.has_permanent {
        tv(
            "plan.deleteNow",
            &[("size", i18n::bytes(plan.total_bytes).into())],
        )
    } else {
        tv(
            "plan.moveToTrash",
            &[("size", i18n::bytes(plan.total_bytes).into())],
        )
    };
    let approve = ui::button(&label);
    approve.add_css_class(if plan.has_permanent {
        "destructive-action"
    } else {
        "suggested-action"
    });
    approve.set_sensitive(!plan.items.is_empty());
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_margin_top(12);
    actions.set_margin_bottom(12);
    actions.set_margin_start(24);
    actions.set_margin_end(24);
    actions.append(&countdown);
    actions.append(&cancel);
    actions.append(&approve);
    tv_.add_bottom_bar(&actions);
    dialog.set_child(Some(&tv_));
    dialog.set_default_widget(Some(&approve));
    approve.grab_focus();

    // Geri sayım: plan 10 dakika geçerlidir.
    let tick = {
        let (plan, countdown, approve) = (plan.clone(), countdown.clone(), approve.clone());
        move || {
            let left = (plan.expires_at - Utc::now()).num_seconds().max(0);
            if left == 0 {
                countdown.set_label(&t("plan.expired"));
                approve.set_sensitive(false);
            } else {
                let time = format!("{}:{:02}", left / 60, left % 60);
                countdown.set_label(&tv("plan.validFor", &[("time", time.into())]));
            }
        }
    };
    tick();
    let timer = glib::timeout_add_seconds_local(1, {
        let tick = tick.clone();
        move || {
            tick();
            glib::ControlFlow::Continue
        }
    });
    let timer = Rc::new(Cell::new(Some(timer)));

    {
        let d = dialog.clone();
        cancel.connect_clicked(move |_| {
            d.close();
        });
    }
    {
        let (plan, done, timer) = (plan.clone(), done.clone(), timer.clone());
        dialog.connect_closed(move |_| {
            if let Some(id) = timer.take() {
                id.remove();
            }
            if !done.get() {
                let id = plan.id;
                glib::spawn_future_local(async move {
                    let _ = client::call("plan.cancel", json!({ "plan_id": id })).await;
                });
            }
        });
    }
    {
        let (dialog_, plan, done) = (dialog.clone(), plan.clone(), done.clone());
        approve.connect_clicked(move |btn| {
            // İşlem sürerken pencere kapatılamasın.
            btn.set_sensitive(false);
            cancel.set_sensitive(false);
            dialog_.set_can_close(false);
            error.set_visible(false);
            let (dialog, plan, done, error, btn, cancel) = (
                dialog_.clone(),
                plan.clone(),
                done.clone(),
                error.clone(),
                btn.clone(),
                cancel.clone(),
            );
            glib::spawn_future_local(async move {
                let r = client::get::<ExecutionReport>(
                    "plan.approve",
                    json!({ "plan_id": plan.id, "digest": plan.digest, "dry_run": false }),
                )
                .await;
                dialog.set_can_close(true);
                match r {
                    Ok(report) => {
                        done.set(true);
                        show_report(&dialog, &report);
                        shell().toast(&tv(
                            "toast.freed",
                            &[("size", i18n::bytes(report.freed_bytes).into())],
                        ));
                        app_ui().history.mark_dirty();
                    }
                    Err(e) => {
                        error.set_label(&ui::error_text(&e));
                        error.set_visible(true);
                        btn.set_sensitive(true);
                        cancel.set_sensitive(true);
                    }
                }
            });
        });
    }
    dialog.present(Some(&shell().window));
}

fn show_report(dialog: &adw::Dialog, r: &ExecutionReport) {
    let mut lines = vec![t("plan.freed")];
    if r.trashed > 0 {
        lines.push(tv("plan.trashed", &[("n", r.trashed.into())]));
    }
    if r.deleted > 0 {
        lines.push(tv("plan.deleted", &[("n", r.deleted.into())]));
    }
    if !r.skipped.is_empty() {
        lines.push(tv("plan.skipped", &[("n", r.skipped.len().into())]));
    }
    if let Some(f) = r.failed.first() {
        lines.push(tv(
            "plan.failed",
            &[
                ("n", r.failed.len().into()),
                ("error", f.error.clone().into()),
            ],
        ));
    }
    let page = ui::status_page(
        "emblem-ok-symbolic",
        &i18n::bytes(r.freed_bytes),
        &lines.join("\n"),
    );
    page.add_css_class("compact");
    let ok = ui::pill(&t("plan.done"), "suggested-action");
    ok.set_halign(gtk::Align::Center);
    let d = dialog.clone();
    ok.connect_clicked(move |_| {
        d.close();
    });
    page.set_child(Some(&ok));
    // Alt çubuktaki düğmeler artık gerekmiyor.
    let fresh = adw::ToolbarView::new();
    fresh.add_top_bar(&adw::HeaderBar::new());
    fresh.set_content(Some(&page));
    dialog.set_child(Some(&fresh));
    ok.grab_focus();
}

pub fn helper_key(s: HelperStatus) -> String {
    serde_json::to_value(s)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| "missing".into())
}

/// Yardımcı neden kullanılamıyor ve nasıl düzeltilir.
pub fn helper_notice(status: HelperStatus) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let key = helper_key(status);
    b.append(&ui::label(&tv(
        &format!("helper.{key}"),
        &[("path", HELPER_PATH.into())],
    )));
    match status {
        HelperStatus::Missing | HelperStatus::NotAFile => {
            b.append(&ui::command(&format!(
                "sudo apt install --reinstall ./bosun_{}_amd64.deb",
                env!("CARGO_PKG_VERSION")
            )));
            let l = ui::dim_label(&t("helper.fromSource"));
            l.add_css_class("caption");
            b.append(&l);
            b.append(&ui::command("./install.sh --helper"));
        }
        _ => b.append(&ui::command(&format!(
            "sudo chown root:root {HELPER_PATH} && sudo chmod 755 {HELPER_PATH}"
        ))),
    }
    b
}

/// Yönetici eylemi: ne yapılacağını ve root olarak çalışacak komutları gösterir, onaylanırsa
/// `pkexec` ile yardımcıyı çalıştırır. Şifreyi sistemin kendi penceresi sorar.
pub fn admin(action: &str, items: Vec<String>) {
    let action = action.to_string();
    glib::spawn_future_local(async move {
        let info = match client::call("admin.info", json!({ "action": action })).await {
            Ok(v) => v,
            Err(e) => return shell().toast(&ui::error_text(&e)),
        };
        let Ok(info) = serde_json::from_value::<ActionInfo>(info["info"].clone()) else {
            return shell().toast(&t("version.oldService"));
        };
        // Asıl karar pkexec'i çalıştıracak olan bu süreçteki denetimdir.
        let helper = bosun_core::admin::helper_status();
        confirm_admin(action, info, items, helper);
    });
}

fn confirm_admin(action: String, info: ActionInfo, items: Vec<String>, helper: HelperStatus) {
    let d = adw::AlertDialog::new(Some(&info.title), Some(&info.effect));
    let extra = gtk::Box::new(gtk::Orientation::Vertical, 8);
    if !items.is_empty() {
        let h = ui::label(&tv("admin.affects", &[("n", items.len().into())]));
        h.add_css_class("heading");
        extra.append(&h);
        let mut text: Vec<String> = items.iter().take(12).map(|i| i18n::short_path(i)).collect();
        if items.len() > 12 {
            text.push(tv("plan.more", &[("n", (items.len() - 12).into())]));
        }
        let l = ui::dim_label(&text.join("\n"));
        l.add_css_class("caption");
        extra.append(&l);
    }
    let h = ui::label(&t("admin.runsAsRoot"));
    h.add_css_class("heading");
    extra.append(&h);
    let cmds = ui::label(&info.commands.join("\n"));
    cmds.add_css_class("monospace");
    cmds.add_css_class("caption");
    cmds.set_selectable(true);
    extra.append(&cmds);
    let ok = helper == HelperStatus::Ok;
    if ok {
        extra.append(&ui::dim_label(&t("admin.passwordNote")));
    } else {
        extra.append(&helper_notice(helper));
    }
    d.set_extra_child(Some(&extra));
    d.add_responses(&[("cancel", &t("plan.cancel")), ("run", &t("admin.continue"))]);
    d.set_response_appearance("run", adw::ResponseAppearance::Suggested);
    d.set_response_enabled("run", ok);
    d.set_default_response(Some("run"));
    d.set_close_response("cancel");
    let info = Rc::new(info);
    d.connect_response(Some("run"), move |_, _| {
        run_admin(action.clone(), info.clone())
    });
    d.present(Some(&shell().window));
}

fn run_admin(action: String, info: Rc<ActionInfo>) {
    let waiting = adw::Toast::new(&t("admin.waiting"));
    waiting.set_timeout(0);
    app_ui_toast(&waiting);
    glib::spawn_future_local(async move {
        let r = client::run_admin(&action).await;
        waiting.dismiss();
        match r {
            Ok(AdminOutcome::Done {
                freed_bytes,
                message,
            }) => {
                let mut body = if freed_bytes > 0 {
                    tv("admin.freed", &[("size", i18n::bytes(freed_bytes).into())])
                } else {
                    t("admin.done")
                };
                if action == "flatpak-unused" && freed_bytes == 0 {
                    body = format!("{body}\n\n{}", t("admin.flatpakNothing"));
                }
                if info.undo.is_some() {
                    body = format!("{body}\n\n{}", t("admin.undoHint"));
                }
                let d = adw::AlertDialog::new(Some(&info.title), Some(&body));
                if let Some(m) = message {
                    let out = ui::label(&m);
                    out.add_css_class("monospace");
                    out.add_css_class("caption");
                    out.set_selectable(true);
                    let scroll = gtk::ScrolledWindow::builder()
                        .max_content_height(220)
                        .propagate_natural_height(true)
                        .child(&out)
                        .build();
                    d.set_extra_child(Some(&scroll));
                }
                d.add_response("ok", &t("plan.done"));
                d.present(Some(&shell().window));
                after_admin();
            }
            Ok(AdminOutcome::Cancelled) => shell().toast(&t("admin.cancelled")),
            Err(e) => {
                let d = adw::AlertDialog::new(
                    Some(&tv(
                        "history.adminFail",
                        &[("action", info.title.clone().into())],
                    )),
                    Some(&e.to_string()),
                );
                d.add_response("ok", &t("plan.done"));
                d.present(Some(&shell().window));
                after_admin();
            }
        }
    });
}

fn app_ui_toast(toast: &adw::Toast) {
    app_ui().add_toast(toast.clone());
}

/// Yönetici eyleminden sonra bulguları, ayarları ve geçmişi tazele.
fn after_admin() {
    glib::spawn_future_local(async {
        if let Ok(f) = client::get("findings", Value::Null).await {
            shell().data.borrow_mut().findings = f;
            app_ui().overview.update_findings();
            shell().update_status();
        }
        let ui = app_ui();
        ui.history.mark_dirty();
        ui.settings.mark_dirty();
    });
}
