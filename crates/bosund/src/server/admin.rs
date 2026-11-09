//! Yönetici eylemleri: bilgi, uygulanmış ayarlar ve yardımcının sonucunu kaydetme.

use super::param_str;
use crate::state::Ctx;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

pub async fn handle(ctx: &Ctx, method: &str, p: &Value) -> Result<Value> {
    match method {
        "admin.info" => {
            let id = param_str(p, "action")?;
            let lang = ctx.state.read().await.cfg.language;
            let info = bosun_core::admin::info(id, lang)
                .with_context(|| format!("unknown action: {id}"))?;
            Ok(json!({
                "info": info,
                "helper_installed": bosun_core::admin::helper_installed(),
                "helper_status": bosun_core::admin::helper_status(),
            }))
        }
        "admin.applied" => {
            let lang = ctx.state.read().await.cfg.language;
            let applied =
                tokio::task::spawn_blocking(move || bosun_core::admin::applied(lang)).await?;
            Ok(json!({
                "applied": applied,
                "helper_installed": bosun_core::admin::helper_installed(),
                "helper_status": bosun_core::admin::helper_status(),
            }))
        }
        "admin.report" => {
            // Arayüz, pkexec ile çalıştırdığı yardımcının sonucunu bildirir: kaydet ve yeniden tara.
            let action = param_str(p, "action")?;
            if !bosun_core::admin::is_known(action) {
                bail!("unknown action: {action}");
            }
            let ok = p["ok"].as_bool().unwrap_or(false);
            ctx.audit.append(
                "admin_action",
                json!({
                    "action": action,
                    "ok": ok,
                    "freed_bytes": p["freed_bytes"].as_u64().unwrap_or(0),
                    "error": p["error"].as_str(),
                }),
            );
            if ok {
                ctx.rescan.notify_one();
            }
            Ok(json!({ "recorded": true }))
        }
        _ => unreachable!("admin does not handle {method}"),
    }
}
