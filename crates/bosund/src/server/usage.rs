//! Disk kullanımı: ev klasöründe yeri neyin kapladığı. Yalnızca okur.

use crate::state::Ctx;
use anyhow::{Context, Result};
use bosun_core::usage;
use serde_json::Value;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Bu süreden eski boyutlar yeniden hesaplanır.
const FRESH_FOR: Duration = Duration::from_secs(5 * 60);

pub async fn handle(ctx: &Ctx, p: &Value) -> Result<Value> {
    let home = ctx.state.read().await.policy.home().to_path_buf();
    let wanted = p["path"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or_else(|| home.clone());
    let path = usage::within_home(&wanted, &home)
        .context("only folders inside your home folder can be measured")?;
    let refresh = p["refresh"].as_bool().unwrap_or(false);
    let cache = ctx.usage.clone();
    let listing = tokio::task::spawn_blocking(move || {
        let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
        let (since, sizes) = &mut *guard;
        if refresh || since.elapsed() > FRESH_FOR {
            sizes.clear();
            *since = Instant::now();
        }
        usage::list(&path, sizes)
    })
    .await??;
    let mut listing = listing;
    if let Some(scan) = ctx.state.read().await.scan.clone() {
        for e in listing.entries.iter_mut().filter(|e| !e.other_fs) {
            e.cleanable = scan.trashable_under(&e.path).map(|(_, i)| i.bytes).sum();
        }
    }
    let mut v = serde_json::to_value(listing)?;
    v["home"] = serde_json::json!(home);
    Ok(v)
}
