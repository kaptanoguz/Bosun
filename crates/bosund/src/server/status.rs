//! Durum: sürüm, ölçümler, bulgular, konteynerler ve geçmiş.

use super::param_str;
use crate::state::{findings_payload, Ctx};
use anyhow::{Context, Result};
use bosun_core::config::Config;
use bosun_core::ipc::PROTOCOL_VERSION;
use serde_json::{json, Value};

pub async fn handle(ctx: &Ctx, method: &str, p: &Value) -> Result<Value> {
    match method {
        "hello" => {
            let st = ctx.state.read().await;
            Ok(json!({
                "protocol": PROTOCOL_VERSION,
                "version": env!("CARGO_PKG_VERSION"),
                "host": st.host,
                "chat_enabled": st.cfg.has_api_key(),
                "model": st.cfg.effective_model(),
                "provider": st.cfg.provider,
                "config_path": bosun_core::config::Config::path(),
                "language": st.cfg.language,
                "helper_installed": bosun_core::admin::helper_installed(),
            }))
        }
        "snapshot" => {
            let st = ctx.state.read().await;
            // Grafikler için en fazla ~240 nokta gönder.
            let step = (st.history.len() / 240).max(1);
            let history: Vec<_> = st.history.iter().step_by(step).collect();
            Ok(json!({ "latest": st.history.back(), "history": history, "host": st.host }))
        }
        "findings" => Ok(findings_payload(&*ctx.state.read().await)),
        "findings.mute" => {
            let id = param_str(p, "id")?.to_string();
            let mute = p["muted"].as_bool().unwrap_or(true);
            let payload = {
                let mut st = ctx.state.write().await;
                st.cfg.muted_findings.retain(|m| m != &id);
                if mute {
                    st.cfg.muted_findings.push(id);
                }
                Config::save_muted(&st.cfg.muted_findings)
                    .context("could not write the config file")?;
                let muted = st.cfg.muted_findings.clone();
                bosun_core::rules::apply_mutes(&mut st.findings, &muted);
                findings_payload(&st)
            };
            ctx.emit("findings", payload.clone());
            Ok(payload)
        }
        "containers" => Ok(json!({ "summary": ctx.state.read().await.containers })),
        "history" => {
            let lang = ctx.state.read().await.cfg.language;
            let mut entries = ctx.audit.tail(p["limit"].as_u64().unwrap_or(50) as usize);
            // Yönetici eylemleri günlükte kimlikleriyle durur; ekranda okunur adları görünsün.
            for e in &mut entries {
                if e["kind"] == "admin_action" {
                    let title = e["data"]["action"]
                        .as_str()
                        .and_then(|a| bosun_core::admin::info(a, lang))
                        .map(|i| i.title);
                    if let Some(t) = title {
                        e["data"]["title"] = json!(t);
                    }
                }
            }
            Ok(json!({ "entries": entries }))
        }
        _ => unreachable!("status does not handle {method}"),
    }
}
