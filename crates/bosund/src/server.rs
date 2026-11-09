//! Unix socket sunucusu: NDJSON istek/yanıt ve olay akışı.

use crate::agent;
use crate::state::{findings_payload, Ctx};
use anyhow::{bail, Context, Result};
use bosun_core::execute::{execute, SystemRemover};
use bosun_core::ipc::{Request, Response, METHODS, PROTOCOL_VERSION};
use bosun_core::config::Config;
use bosun_core::plan::{ApproveError, CleanupPlan, Selection};
use bosun_core::Lang;
use chrono::Utc;
use serde_json::{json, Value};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;
use uuid::Uuid;

pub async fn serve(ctx: Ctx, path: &Path) -> Result<()> {
    if path.exists() {
        if UnixStream::connect(path).await.is_ok() {
            bail!("another bosund is already running ({})", path.display());
        }
        std::fs::remove_file(path).ok();
    }
    let listener = UnixListener::bind(path).with_context(|| format!("cannot listen on {}", path.display()))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    let my_uid = std::fs::metadata(path)?.uid();
    tracing::info!("listening on {}", path.display());

    loop {
        let (stream, _) = listener.accept().await?;
        // Yalnızca aynı kullanıcının süreçleri bağlanabilir.
        match stream.peer_cred() {
            Ok(c) if c.uid() == my_uid => {}
            Ok(c) => {
                tracing::warn!("rejected connection from foreign UID {}", c.uid());
                continue;
            }
            Err(e) => {
                tracing::warn!("could not read peer credentials: {e}");
                continue;
            }
        }
        let ctx = ctx.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_conn(ctx, stream).await {
                tracing::debug!("connection closed: {e}");
            }
        });
    }
}

async fn handle_conn(ctx: Ctx, stream: UnixStream) -> Result<()> {
    let (read, mut write) = stream.into_split();
    let (tx, mut rx) = mpsc::channel::<String>(256);

    let writer = tokio::spawn(async move {
        while let Some(line) = rx.recv().await {
            if write.write_all(line.as_bytes()).await.is_err() || write.write_all(b"\n").await.is_err() {
                break;
            }
        }
    });

    let mut lines = BufReader::new(read).lines();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let req: Request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                tx.send(serde_json::to_string(&Response::err(0, format!("invalid request: {e}")))?).await?;
                continue;
            }
        };
        if req.method == "subscribe" {
            let mut events = ctx.events.subscribe();
            let tx2 = tx.clone();
            tokio::spawn(async move {
                loop {
                    match events.recv().await {
                        Ok(ev) => {
                            let Ok(s) = serde_json::to_string(&ev) else { continue };
                            if tx2.send(s).await.is_err() {
                                break;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(_) => break,
                    }
                }
            });
            tx.send(serde_json::to_string(&Response::ok(req.id, json!({"subscribed": true})))?).await?;
            continue;
        }
        // Uzun süren istekler (sohbet, tarama) diğerlerini bekletmesin.
        let ctx = ctx.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            let resp = match dispatch(&ctx, &req).await {
                Ok(v) => Response::ok(req.id, v),
                Err(e) => Response::err(req.id, format!("{e:#}")),
            };
            if let Ok(s) = serde_json::to_string(&resp) {
                let _ = tx.send(s).await;
            }
        });
    }
    drop(tx);
    writer.abort();
    Ok(())
}

fn param_str<'a>(p: &'a Value, key: &str) -> Result<&'a str> {
    p[key].as_str().with_context(|| format!("missing parameter '{key}'"))
}

pub async fn dispatch(ctx: &Ctx, req: &Request) -> Result<Value> {
    if !METHODS.contains(&req.method.as_str()) {
        bail!("unknown method: {}", req.method);
    }
    let p = &req.params;
    match req.method.as_str() {
        "hello" => {
            let st = ctx.state.read().await;
            Ok(json!({
                "protocol": PROTOCOL_VERSION,
                "version": env!("CARGO_PKG_VERSION"),
                "host": st.host,
                "chat_enabled": st.cfg.has_api_key(),
                "model": st.cfg.model,
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
        "scan" => {
            let scan = if p["refresh"].as_bool().unwrap_or(false) { ctx.scan_now().await } else { ctx.ensure_scan().await };
            Ok(serde_json::to_value(scan.trimmed(p["limit"].as_u64().unwrap_or(25) as usize))?)
        }
        "findings" => Ok(findings_payload(&*ctx.state.read().await)),
        "plan.create" => {
            let mut sel: Selection = serde_json::from_value(p.clone()).context("invalid selection")?;
            sel.origin = "user".into();
            let scan = ctx.ensure_scan().await;
            let lang = ctx.state.read().await.cfg.language;
            let plan = CleanupPlan::create(&scan, &sel, lang);
            ctx.audit.append("plan_created", json!({"plan_id": plan.id, "origin": "user", "total_bytes": plan.total_bytes, "summary": plan.summary}));
            let preview = plan.preview(30);
            ctx.state.write().await.plans.insert(plan.id, plan);
            Ok(serde_json::to_value(preview)?)
        }
        "plan.cancel" => {
            let id: Uuid = param_str(p, "plan_id")?.parse()?;
            if ctx.state.write().await.plans.remove(&id).is_some() {
                ctx.audit.append("plan_cancelled", json!({"plan_id": id}));
            }
            Ok(json!({"cancelled": true}))
        }
        "plan.approve" => approve(ctx, p).await,
        "history" => Ok(json!({ "entries": ctx.audit.tail(p["limit"].as_u64().unwrap_or(50) as usize) })),
        "chat" => {
            let conv = p["conversation_id"].as_str().unwrap_or("default");
            let msg = param_str(p, "message")?;
            if msg.chars().count() > 4000 {
                bail!("message is too long");
            }
            let r = agent::chat(ctx, conv, msg).await?;
            Ok(json!({ "reply": r.reply, "plans": r.plans, "tools_used": r.tools_used }))
        }
        "settings.get" => {
            let st = ctx.state.read().await;
            Ok(settings_payload(&st.cfg))
        }
        "settings.set" => {
            if let Some(code) = p["language"].as_str() {
                let lang = Lang::parse(code).with_context(|| format!("unsupported language: {code}"))?;
                Config::save_language(lang).context("could not write the config file")?;
                ctx.state.write().await.cfg.language = lang;
                // Kategori adları, bulgular ve öneriler yeni dilde yeniden üretilsin.
                ctx.rescan.notify_one();
            }
            let payload = settings_payload(&ctx.state.read().await.cfg);
            ctx.emit("settings", payload.clone());
            Ok(payload)
        }
        "admin.info" => {
            let id = param_str(p, "action")?;
            let lang = ctx.state.read().await.cfg.language;
            let info = bosun_core::admin::info(id, lang).with_context(|| format!("unknown action: {id}"))?;
            Ok(json!({ "info": info, "helper_installed": bosun_core::admin::helper_installed() }))
        }
        "admin.applied" => {
            let lang = ctx.state.read().await.cfg.language;
            let applied = tokio::task::spawn_blocking(move || bosun_core::admin::applied(lang)).await?;
            Ok(json!({ "applied": applied, "helper_installed": bosun_core::admin::helper_installed() }))
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
        "chat.reset" => {
            let conv = p["conversation_id"].as_str().unwrap_or("default");
            ctx.state.write().await.chats.remove(conv);
            Ok(json!({"reset": true}))
        }
        _ => unreachable!(),
    }
}

fn settings_payload(cfg: &Config) -> Value {
    json!({
        "language": cfg.language,
        "model": cfg.model,
        "chat_enabled": cfg.has_api_key(),
        "share_file_paths": cfg.share_file_paths,
        "notifications": cfg.notifications,
        "config_path": Config::path(),
    })
}

async fn approve(ctx: &Ctx, p: &Value) -> Result<Value> {
    let id: Uuid = param_str(p, "plan_id")?.parse().context("invalid plan id")?;
    let digest = param_str(p, "digest")?;
    let dry_run = p["dry_run"].as_bool().unwrap_or(false);

    // Planı depodan çıkar: aynı plan iki kez uygulanamaz.
    let (plan, policy) = {
        let mut st = ctx.state.write().await;
        let lang = st.cfg.language;
        let plan = st.plans.get(&id).cloned().ok_or_else(|| anyhow::anyhow!(ApproveError::NotFound.message(lang)))?;
        plan.check_approval(digest, Utc::now()).map_err(|e| anyhow::anyhow!(e.message(lang)))?;
        if !dry_run {
            st.plans.remove(&id);
        }
        (plan, st.policy.clone())
    };

    ctx.audit.append("plan_approved", json!({"plan_id": id, "dry_run": dry_run, "items": plan.items.len(), "total_bytes": plan.total_bytes}));
    let plan2 = plan.clone();
    let report = tokio::task::spawn_blocking(move || execute(&plan2, &policy, &SystemRemover, dry_run)).await?;
    ctx.audit.append(
        "plan_executed",
        json!({
            "plan_id": id, "dry_run": dry_run, "freed_bytes": report.freed_bytes,
            "trashed": report.trashed, "deleted": report.deleted,
            "failed": report.failed.len(), "skipped": report.skipped.len(),
            "summary": plan.summary,
        }),
    );
    if !dry_run {
        ctx.emit("plan_executed", serde_json::to_value(&report)?);
        ctx.rescan.notify_one();
    }
    Ok(serde_json::to_value(report)?)
}
