//! Unix socket sunucusu: NDJSON istek/yanıt ve olay akışı.

use crate::agent;
use crate::state::{findings_payload, Ctx};
use anyhow::{bail, Context, Result};
use bosun_core::execute::{execute, SystemRemover};
use bosun_core::ipc::{Request, Response, METHODS, PROTOCOL_VERSION};
use bosun_core::plan::{ApproveError, CleanupPlan, Selection};
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
            bail!("başka bir bosund zaten çalışıyor ({})", path.display());
        }
        std::fs::remove_file(path).ok();
    }
    let listener = UnixListener::bind(path).with_context(|| format!("{} dinlenemiyor", path.display()))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    let my_uid = std::fs::metadata(path)?.uid();
    tracing::info!("dinleniyor: {}", path.display());

    loop {
        let (stream, _) = listener.accept().await?;
        // Yalnızca aynı kullanıcının süreçleri bağlanabilir.
        match stream.peer_cred() {
            Ok(c) if c.uid() == my_uid => {}
            Ok(c) => {
                tracing::warn!("yabancı UID {} reddedildi", c.uid());
                continue;
            }
            Err(e) => {
                tracing::warn!("eş kimliği okunamadı: {e}");
                continue;
            }
        }
        let ctx = ctx.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_conn(ctx, stream).await {
                tracing::debug!("bağlantı kapandı: {e}");
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
                tx.send(serde_json::to_string(&Response::err(0, format!("geçersiz istek: {e}")))?).await?;
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
    p[key].as_str().with_context(|| format!("'{key}' parametresi eksik"))
}

pub async fn dispatch(ctx: &Ctx, req: &Request) -> Result<Value> {
    if !METHODS.contains(&req.method.as_str()) {
        bail!("bilinmeyen yöntem: {}", req.method);
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
            let mut sel: Selection = serde_json::from_value(p.clone()).context("geçersiz seçim")?;
            sel.origin = "user".into();
            let scan = ctx.ensure_scan().await;
            let plan = CleanupPlan::create(&scan, &sel);
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
                bail!("mesaj çok uzun");
            }
            let r = agent::chat(ctx, conv, msg).await?;
            Ok(json!({ "reply": r.reply, "plans": r.plans, "tools_used": r.tools_used }))
        }
        "chat.reset" => {
            let conv = p["conversation_id"].as_str().unwrap_or("default");
            ctx.state.write().await.chats.remove(conv);
            Ok(json!({"reset": true}))
        }
        _ => unreachable!(),
    }
}

async fn approve(ctx: &Ctx, p: &Value) -> Result<Value> {
    let id: Uuid = param_str(p, "plan_id")?.parse().context("geçersiz plan kimliği")?;
    let digest = param_str(p, "digest")?;
    let dry_run = p["dry_run"].as_bool().unwrap_or(false);

    // Planı depodan çıkar: aynı plan iki kez uygulanamaz.
    let (plan, policy) = {
        let mut st = ctx.state.write().await;
        let plan = st.plans.get(&id).cloned().ok_or(ApproveError::NotFound)?;
        plan.check_approval(digest, Utc::now())?;
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
