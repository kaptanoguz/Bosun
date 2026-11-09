//! bosund — Bosun arka plan servisi.
//!
//! Sistemi izler, kuralları çalıştırır, bildirim gönderir ve arayüze Unix socket
//! üzerinden hizmet verir. Root olarak çalıştırılması reddedilir.

mod agent;
mod audit;
mod notify;
mod server;
mod state;

use anyhow::{bail, Result};
use bosun_core::config::Config;
use bosun_core::metrics::Collector;
use std::os::unix::fs::MetadataExt;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("BOSUN_LOG")
                .unwrap_or_else(|_| "info".into()),
        )
        .with_target(false)
        .init();

    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!("bosund {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let home = dirs_home()?;
    if std::fs::metadata(&home).map(|m| m.uid()).unwrap_or(0) == 0 && home.as_os_str() == "/root" {
        bail!("bosund must not run as root; start it as your own user with `systemctl --user`");
    }

    let cfg = Config::load();
    if !cfg.has_api_key() {
        tracing::warn!("no API key; chat is disabled, monitoring and cleanup keep working");
    }
    let collector = Collector::try_new()
        .map_err(|e| anyhow::anyhow!("could not read system information: {e:?}"))?;
    let ctx = state::Ctx::new(cfg, collector.host_info());

    tokio::spawn(state::sampler(ctx.clone(), collector));
    let c = ctx.clone();
    tokio::spawn(async move {
        // İlk ölçüm ayar önerileri için gerekli; taramayı biraz geciktir.
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        state::scanner(c).await
    });

    let sock = bosun_core::ipc::socket_path();
    tokio::select! {
        r = server::serve(ctx, &sock) => r?,
        _ = tokio::signal::ctrl_c() => tracing::info!("shutting down"),
    }
    let _ = std::fs::remove_file(&sock);
    Ok(())
}

fn dirs_home() -> Result<std::path::PathBuf> {
    match std::env::var_os("HOME") {
        Some(h) if !h.is_empty() => Ok(h.into()),
        _ => bail!("HOME is not set"),
    }
}
