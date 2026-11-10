//! Ayarlar: dil, yapay zekâ sağlayıcısı ve anahtarlar.

use crate::state::Ctx;
use anyhow::{bail, Context, Result};
use bosun_core::config::{Config, Provider};
use bosun_core::Lang;
use serde_json::{json, Value};

pub async fn handle(ctx: &Ctx, method: &str, p: &Value) -> Result<Value> {
    match method {
        "settings.get" => {
            let st = ctx.state.read().await;
            Ok(settings_payload(&st.cfg))
        }
        "settings.set" => {
            if let Some(code) = p["language"].as_str() {
                let lang =
                    Lang::parse(code).with_context(|| format!("unsupported language: {code}"))?;
                Config::save_language(lang).context("could not write the config file")?;
                ctx.state.write().await.cfg.language = lang;
                // Kategori adları, bulgular ve öneriler yeni dilde yeniden üretilsin.
                ctx.rescan.notify_one();
            }
            if let Some(theme) = p["theme"].as_str() {
                anyhow::ensure!(
                    matches!(theme, "system" | "light" | "dark"),
                    "unsupported theme: {theme}"
                );
                Config::save_theme(theme).context("could not write the config file")?;
                ctx.state.write().await.cfg.theme = theme.to_string();
            }
            if let Some(id) = p["provider"].as_str() {
                let provider =
                    Provider::parse(id).with_context(|| format!("unsupported provider: {id}"))?;
                let model = p["model"].as_str().unwrap_or("").trim().to_string();
                let base_url = p["base_url"]
                    .as_str()
                    .map(str::trim)
                    .filter(|u| !u.is_empty())
                    .map(str::to_string);
                if let Some(u) = &base_url {
                    if !(u.starts_with("https://")
                        || u.starts_with("http://localhost")
                        || u.starts_with("http://127.0.0.1"))
                    {
                        bail!("the server address must use https:// (plain http is only allowed for localhost)");
                    }
                }
                // Anahtar yalnızca gönderildiyse değişir; "" anahtarı siler.
                let api_key = p["api_key"].as_str();
                Config::save_ai(provider, &model, base_url.as_deref(), api_key)
                    .context("could not write the config file")?;
                let mut st = ctx.state.write().await;
                let changed_provider = st.cfg.provider != provider;
                st.cfg.provider = provider;
                st.cfg.model = model;
                st.cfg.base_url = base_url;
                if let Some(k) = api_key {
                    if k.trim().is_empty() {
                        st.cfg.api_keys.remove(provider.id());
                        if provider == Provider::Anthropic {
                            st.cfg.api_key = None;
                        }
                    } else {
                        st.cfg
                            .api_keys
                            .insert(provider.id().into(), k.trim().into());
                    }
                }
                if changed_provider {
                    st.chats.clear();
                }
                ctx.audit.append(
                    "settings_ai",
                    json!({"provider": provider, "model": st.cfg.effective_model()}),
                );
            }
            for key in [
                "share_file_paths",
                "notifications",
                "weekly_summary",
                "persist_chat",
            ] {
                if let Some(v) = p[key].as_bool() {
                    Config::save_bool(key, v).context("could not write the config file")?;
                    let mut st = ctx.state.write().await;
                    match key {
                        "share_file_paths" => st.cfg.share_file_paths = v,
                        "notifications" => st.cfg.notifications = v,
                        "weekly_summary" => st.cfg.weekly_summary = v,
                        _ => {
                            st.cfg.persist_chat = v;
                            if v {
                                crate::state::save_chats(&st.chats);
                            } else {
                                let _ = std::fs::remove_file(crate::state::chats_path());
                            }
                        }
                    }
                }
            }
            let payload = settings_payload(&ctx.state.read().await.cfg);
            ctx.emit("settings", payload.clone());
            Ok(payload)
        }
        _ => unreachable!("settings does not handle {method}"),
    }
}

fn settings_payload(cfg: &Config) -> Value {
    // Anahtarların kendisi asla gönderilmez; yalnızca kayıtlı olup olmadıkları.
    let providers: Vec<Value> = Provider::ALL
        .iter()
        .map(|p| {
            json!({
                "id": p,
                "label": p.label(),
                "default_model": p.default_model(),
                "default_base_url": p.default_base_url(),
                "has_key": cfg.key_for(*p).is_some(),
                "needs_key": *p != Provider::Custom,
            })
        })
        .collect();
    json!({
        "language": cfg.language,
        "theme": cfg.theme,
        "provider": cfg.provider,
        "model": cfg.effective_model(),
        "model_override": cfg.model,
        "base_url": cfg.base_url,
        "providers": providers,
        "chat_enabled": cfg.has_api_key(),
        "share_file_paths": cfg.share_file_paths,
        "notifications": cfg.notifications,
        "weekly_summary": cfg.weekly_summary,
        "persist_chat": cfg.persist_chat,
        "config_path": Config::path(),
    })
}
