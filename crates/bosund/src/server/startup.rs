//! Oturum açılışında başlayan uygulamalar.

use super::param_str;
use crate::state::Ctx;
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::Path;

pub async fn handle(ctx: &Ctx, method: &str, p: &Value) -> Result<Value> {
    match method {
        "startup.list" => {
            let home = ctx.state.read().await.policy.home().to_path_buf();
            let apps = tokio::task::spawn_blocking(move || {
                bosun_core::autostart::list(
                    &home,
                    Path::new(bosun_core::autostart::SYSTEM_DIR),
                    &bosun_core::autostart::current_desktops(),
                )
            })
            .await?;
            Ok(json!({ "apps": apps }))
        }
        "startup.set" => {
            // Yalnızca arayüzden, kullanıcının düğmesiyle gelir; asistanın bu yetkisi yoktur.
            let file = param_str(p, "file")?.to_string();
            let enabled = p["enabled"]
                .as_bool()
                .context("missing parameter 'enabled'")?;
            let home = ctx.state.read().await.policy.home().to_path_buf();
            let f = file.clone();
            let app = tokio::task::spawn_blocking(move || {
                bosun_core::autostart::set_enabled(
                    &home,
                    Path::new(bosun_core::autostart::SYSTEM_DIR),
                    &bosun_core::autostart::current_desktops(),
                    &f,
                    enabled,
                )
            })
            .await??;
            ctx.audit.append(
                "startup_toggled",
                json!({ "file": file, "name": app.name, "enabled": enabled }),
            );
            Ok(serde_json::to_value(app)?)
        }
        _ => unreachable!("startup does not handle {method}"),
    }
}
