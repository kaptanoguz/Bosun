//! Tarama ve temizlik planları. Silme yalnızca onaylanmış planla `approve` içinden yapılır.

use super::param_str;
use crate::state::Ctx;
use anyhow::{bail, Context, Result};
use bosun_core::execute::{execute, SystemRemover};
use bosun_core::plan::{ApproveError, CleanupPlan, Selection};
use chrono::Utc;
use serde_json::{json, Value};
use uuid::Uuid;

pub async fn handle(ctx: &Ctx, method: &str, p: &Value) -> Result<Value> {
    match method {
        "scan" => {
            let scan = if p["refresh"].as_bool().unwrap_or(false) {
                ctx.scan_now().await
            } else {
                ctx.ensure_scan().await
            };
            Ok(serde_json::to_value(
                scan.trimmed(p["limit"].as_u64().unwrap_or(25) as usize),
            )?)
        }
        "scan.cancel" => {
            let running = ctx.scanning.load(std::sync::atomic::Ordering::SeqCst);
            ctx.scan_cancel
                .store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(json!({ "cancelled": running }))
        }
        "scan.inspect" => {
            // Yalnızca "tek tek seçin" kategorilerinde taramanın bulduğu klasörlere bakılabilir.
            let path = std::path::PathBuf::from(param_str(p, "path")?);
            let scan = ctx.ensure_scan().await;
            let allowed = scan
                .categories
                .iter()
                .filter(|c| c.safety == bosun_core::Safety::ReviewOnly)
                .any(|c| c.items.iter().any(|i| i.path == path));
            if !allowed {
                bail!("path is not part of the current scan");
            }
            let insp =
                tokio::task::spawn_blocking(move || bosun_core::scan::inspect(&path, 12)).await?;
            Ok(serde_json::to_value(insp)?)
        }
        "plan.create" => {
            let mut sel: Selection =
                serde_json::from_value(p.clone()).context("invalid selection")?;
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
        "disk.plan" => {
            // Disk ekranında seçilen tek bir dosya: yalnızca ev klasöründe, korumasız ve
            // dosya olmalı. Silme yine onay penceresinden geçer.
            let wanted = std::path::PathBuf::from(param_str(p, "path")?);
            let (home, policy) = {
                let st = ctx.state.read().await;
                (st.policy.home().to_path_buf(), st.policy.clone())
            };
            let path = policy
                .validate(&wanted, &home)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let meta = std::fs::metadata(&path)?;
            anyhow::ensure!(meta.is_file(), "only single files can be picked here");
            let lang = ctx.state.read().await.cfg.language;
            let plan = CleanupPlan::for_file(path, meta.len(), "user", lang);
            ctx.audit.append("plan_created", json!({"plan_id": plan.id, "origin": "user", "total_bytes": plan.total_bytes, "summary": plan.summary}));
            let preview = plan.preview(30);
            ctx.state.write().await.plans.insert(plan.id, plan);
            Ok(serde_json::to_value(preview)?)
        }
        "disk.clean" => {
            // Disk ekranında bir klasörün altında taramanın bulduğu temizlenebilir öğeler.
            // Yalnızca taramadaki yollar, yalnızca çöpe taşıma; silme yine onaydan geçer.
            let wanted = std::path::PathBuf::from(param_str(p, "path")?);
            let home = ctx.state.read().await.policy.home().to_path_buf();
            let path = bosun_core::usage::within_home(&wanted, &home)
                .context("only folders inside your home folder can be cleaned here")?;
            let scan = ctx.ensure_scan().await;
            let lang = ctx.state.read().await.cfg.language;
            let plan = CleanupPlan::for_folder(&scan, &path, "user", lang);
            anyhow::ensure!(!plan.items.is_empty(), "nothing cleanable was found here");
            ctx.audit.append("plan_created", json!({"plan_id": plan.id, "origin": "user", "total_bytes": plan.total_bytes, "summary": plan.summary}));
            let preview = plan.preview(30);
            ctx.state.write().await.plans.insert(plan.id, plan);
            Ok(serde_json::to_value(preview)?)
        }
        "plan.approve" => approve(ctx, p).await,
        _ => unreachable!("cleanup does not handle {method}"),
    }
}

async fn approve(ctx: &Ctx, p: &Value) -> Result<Value> {
    let id: Uuid = param_str(p, "plan_id")?
        .parse()
        .context("invalid plan id")?;
    let digest = param_str(p, "digest")?;
    let dry_run = p["dry_run"].as_bool().unwrap_or(false);

    // Planı depodan çıkar: aynı plan iki kez uygulanamaz.
    let (plan, policy) = {
        let mut st = ctx.state.write().await;
        let lang = st.cfg.language;
        let plan = st
            .plans
            .get(&id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!(ApproveError::NotFound.message(lang)))?;
        plan.check_approval(digest, Utc::now())
            .map_err(|e| anyhow::anyhow!(e.message(lang)))?;
        if !dry_run {
            st.plans.remove(&id);
        }
        (plan, st.policy.clone())
    };

    ctx.audit.append("plan_approved", json!({"plan_id": id, "dry_run": dry_run, "items": plan.items.len(), "total_bytes": plan.total_bytes}));
    let plan2 = plan.clone();
    let report =
        tokio::task::spawn_blocking(move || execute(&plan2, &policy, &SystemRemover, dry_run))
            .await?;
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
        // Silinenler disk kullanımı ekranında eski boyutlarıyla görünmesin.
        if let Ok(mut u) = ctx.usage.lock() {
            u.1.clear();
        }
        ctx.rescan.notify_one();
    }
    Ok(serde_json::to_value(report)?)
}
