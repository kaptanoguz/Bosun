//! Unix socket sunucusu: NDJSON istek/yanıt ve olay akışı.

use crate::state::Ctx;
use anyhow::{bail, Context, Result};
use bosun_core::ipc::{Request, Response, METHODS};
use serde_json::{json, Value};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;

mod admin;
mod chat;
mod cleanup;
mod settings;
mod startup;
mod status;
mod usage;

pub async fn serve(ctx: Ctx, path: &Path) -> Result<()> {
    if path.exists() {
        if UnixStream::connect(path).await.is_ok() {
            bail!("another bosund is already running ({})", path.display());
        }
        std::fs::remove_file(path).ok();
    }
    let listener =
        UnixListener::bind(path).with_context(|| format!("cannot listen on {}", path.display()))?;
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
            if write.write_all(line.as_bytes()).await.is_err()
                || write.write_all(b"\n").await.is_err()
            {
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
                tx.send(serde_json::to_string(&Response::err(
                    0,
                    format!("invalid request: {e}"),
                ))?)
                .await?;
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
                            let Ok(s) = serde_json::to_string(&ev) else {
                                continue;
                            };
                            if tx2.send(s).await.is_err() {
                                break;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(_) => break,
                    }
                }
            });
            tx.send(serde_json::to_string(&Response::ok(
                req.id,
                json!({"subscribed": true}),
            ))?)
            .await?;
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

pub(super) fn param_str<'a>(p: &'a Value, key: &str) -> Result<&'a str> {
    p[key]
        .as_str()
        .with_context(|| format!("missing parameter '{key}'"))
}

pub async fn dispatch(ctx: &Ctx, req: &Request) -> Result<Value> {
    if !METHODS.contains(&req.method.as_str()) {
        bail!("unknown method: {}", req.method);
    }
    let (m, p) = (req.method.as_str(), &req.params);
    match m {
        "hello" | "snapshot" | "findings" | "findings.mute" | "containers" | "history" => {
            status::handle(ctx, m, p).await
        }
        "scan" | "scan.cancel" | "scan.inspect" | "plan.create" | "plan.cancel"
        | "plan.approve" | "disk.plan" => cleanup::handle(ctx, m, p).await,
        "chat" | "chat.history" | "chat.reset" => chat::handle(ctx, m, p).await,
        "settings.get" | "settings.set" => settings::handle(ctx, m, p).await,
        "admin.info" | "admin.applied" | "admin.report" => admin::handle(ctx, m, p).await,
        "startup.list" | "startup.set" => startup::handle(ctx, m, p).await,
        "usage" => usage::handle(ctx, p).await,
        _ => bail!("method {m} is not handled"),
    }
}
