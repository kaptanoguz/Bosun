//! Daemon'un paylaşılan durumu ve arka plan döngüleri.

use crate::audit::AuditLog;
use crate::notify::Notifier;
use bosun_core::advice::{self, Advice};
use bosun_core::config::Config;
use bosun_core::ipc::Event;
use bosun_core::metrics::{Collector, HostInfo, Snapshot};
use bosun_core::plan::CleanupPlan;
use bosun_core::rules::{self, Finding, RuleInput};
use bosun_core::scan::{self, ScanResult};
use bosun_core::Policy;
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, Notify, RwLock};
use uuid::Uuid;

/// Grafikler için tutulan geçmiş süresi.
const HISTORY_SECS: u64 = 3600;

pub struct State {
    pub cfg: Config,
    pub policy: Policy,
    pub host: HostInfo,
    pub history: VecDeque<Snapshot>,
    pub scan: Option<Arc<ScanResult>>,
    pub findings: Vec<Finding>,
    pub advice: Vec<Advice>,
    pub failed_units: Vec<String>,
    pub mem_high_since: Option<Instant>,
    pub plans: HashMap<Uuid, CleanupPlan>,
    /// Sohbet geçmişi, hangi sağlayıcının biçiminde tutulduğuyla birlikte.
    pub chats: HashMap<String, (bosun_core::config::Provider, Vec<Value>)>,
}

#[derive(Clone)]
pub struct Ctx {
    pub state: Arc<RwLock<State>>,
    pub events: broadcast::Sender<Event>,
    pub audit: AuditLog,
    pub rescan: Arc<Notify>,
    pub scan_done: Arc<Notify>,
    /// Süren taramayı iptal etmek için.
    pub scan_cancel: Arc<AtomicBool>,
    pub scanning: Arc<AtomicBool>,
    pub http: reqwest::Client,
}

/// Sohbet geçmişinin diskteki yeri (yalnızca kullanıcı okuyabilir).
pub fn chats_path() -> std::path::PathBuf {
    crate::audit::dirs_state().join("bosun/chats.json")
}

pub fn load_chats() -> HashMap<String, (bosun_core::config::Provider, Vec<Value>)> {
    std::fs::read_to_string(chats_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_chats(chats: &HashMap<String, (bosun_core::config::Provider, Vec<Value>)>) {
    use std::os::unix::fs::PermissionsExt;
    let path = chats_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension("json.tmp");
    let ok = serde_json::to_vec(chats)
        .ok()
        .and_then(|data| std::fs::write(&tmp, data).ok())
        .and_then(|_| std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600)).ok())
        .and_then(|_| std::fs::rename(&tmp, &path).ok());
    if ok.is_none() {
        tracing::warn!("could not save chat history");
    }
}

impl Ctx {
    pub fn new(cfg: Config, host: HostInfo) -> Self {
        let (events, _) = broadcast::channel(256);
        let cfg_persist = cfg.persist_chat;
        let state = State {
            cfg,
            policy: Policy::for_current_user(),
            host,
            history: VecDeque::new(),
            scan: None,
            findings: vec![],
            advice: vec![],
            failed_units: vec![],
            mem_high_since: None,
            plans: HashMap::new(),
            chats: if cfg_persist {
                load_chats()
            } else {
                HashMap::new()
            },
        };
        Self {
            state: Arc::new(RwLock::new(state)),
            events,
            audit: AuditLog::new(),
            rescan: Arc::new(Notify::new()),
            scan_done: Arc::new(Notify::new()),
            scan_cancel: Arc::new(AtomicBool::new(false)),
            scanning: Arc::new(AtomicBool::new(false)),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(90))
                .build()
                .expect("failed to build HTTP client"),
        }
    }

    pub fn emit(&self, event: &str, data: Value) {
        let _ = self.events.send(Event {
            event: event.into(),
            data,
        });
    }

    /// Bir tarama başlatır ve bitmesini bekler.
    pub async fn scan_now(&self) -> Arc<ScanResult> {
        let done = self.scan_done.notified();
        self.rescan.notify_one();
        done.await;
        self.state
            .read()
            .await
            .scan
            .clone()
            .expect("scan result missing")
    }

    /// Tarama sonucu yoksa önce tarar.
    pub async fn ensure_scan(&self) -> Arc<ScanResult> {
        if let Some(s) = self.state.read().await.scan.clone() {
            return s;
        }
        self.scan_now().await
    }

    /// Kural motorunu güncel verilerle yeniden çalıştırır.
    pub async fn reevaluate(&self, notifier: &mut Notifier) {
        let mut st = self.state.write().await;
        let Some(snap) = st.history.back().cloned() else {
            return;
        };
        let mem_high_secs = st
            .mem_high_since
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0);
        let scan = st.scan.clone();
        let findings = rules::evaluate(
            &RuleInput {
                snapshot: &snap,
                mem_high_secs,
                scan: scan.as_deref(),
                failed_units: &st.failed_units,
                lang: st.cfg.language,
            },
            &st.cfg.thresholds,
        );
        notifier.consider(&findings, st.cfg.notifications);
        let changed =
            serde_json::to_value(&findings).ok() != serde_json::to_value(&st.findings).ok();
        st.findings = findings;
        if changed {
            let data = findings_payload(&st);
            drop(st);
            self.emit("findings", data);
        }
    }
}

pub fn findings_payload(st: &State) -> Value {
    json!({
        "findings": st.findings,
        "advice": st.advice,
        "overall": rules::overall(&st.findings),
        "failed_units": st.failed_units,
    })
}

/// Metrik örnekleme döngüsü.
pub async fn sampler(ctx: Ctx, mut collector: Collector) {
    let (interval, mem_warn) = {
        let st = ctx.state.read().await;
        (st.cfg.sample_interval_secs, st.cfg.thresholds.mem_warn)
    };
    let cap = (HISTORY_SECS / interval) as usize;
    let mut notifier = Notifier::default();
    let mut tick = tokio::time::interval(Duration::from_secs(interval));
    loop {
        tick.tick().await;
        let snap = collector.collect();
        {
            let mut st = ctx.state.write().await;
            if snap.mem_percent() >= mem_warn {
                st.mem_high_since.get_or_insert_with(Instant::now);
            } else {
                st.mem_high_since = None;
            }
            st.history.push_back(snap.clone());
            while st.history.len() > cap {
                st.history.pop_front();
            }
        }
        ctx.emit("metrics", serde_json::to_value(&snap).unwrap_or_default());
        ctx.reevaluate(&mut notifier).await;
    }
}

/// Periyodik (ve istek üzerine) tarama döngüsü.
pub async fn scanner(ctx: Ctx) {
    let interval = ctx.state.read().await.cfg.scan_interval_secs;
    loop {
        ctx.emit("scan_started", json!({}));
        let (home, opts) = {
            let st = ctx.state.read().await;
            (st.policy.home().to_path_buf(), st.cfg.scan_options())
        };
        let lang = opts.lang;
        ctx.scan_cancel.store(false, Ordering::SeqCst);
        ctx.scanning.store(true, Ordering::SeqCst);
        let (cancel, events) = (ctx.scan_cancel.clone(), ctx.events.clone());
        let result = tokio::task::spawn_blocking(move || {
            scan::scan_all_with(&home, &opts, &cancel, |done, total, id| {
                let _ = events.send(bosun_core::ipc::Event {
                    event: "scan_progress".into(),
                    data: json!({ "done": done, "total": total, "category": id, "title": id.title(lang) }),
                });
            })
        })
        .await;
        ctx.scanning.store(false, Ordering::SeqCst);
        let failed = failed_units().await;
        match result {
            Ok(None) => {
                tracing::info!("scan cancelled");
                ctx.emit("scan_cancelled", json!({}));
            }
            Ok(Some(scan)) => {
                let scan = Arc::new(scan);
                let mut st = ctx.state.write().await;
                let latest = st.history.back().cloned();
                st.advice = latest
                    .map(|s| advice::collect(&s, Some(&scan), lang))
                    .unwrap_or_default();
                st.failed_units = failed;
                st.scan = Some(scan.clone());
                drop(st);
                tracing::info!(
                    "scan finished in {} ms, {} reclaimable",
                    scan.duration_ms,
                    bosun_core::util::human_bytes(scan.reclaimable_bytes())
                );
                ctx.emit(
                    "scan",
                    serde_json::to_value(scan.trimmed(25)).unwrap_or_default(),
                );
            }
            Err(e) => tracing::error!("scan failed: {e}"),
        }
        ctx.scan_done.notify_waiters();
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(interval)) => {}
            _ = ctx.rescan.notified() => {}
        }
    }
}

/// Hem sistem hem kullanıcı seviyesindeki başarısız systemd birimleri.
pub async fn failed_units() -> Vec<String> {
    let mut out = Vec::new();
    for scope in [None, Some("--user")] {
        let mut cmd = tokio::process::Command::new("systemctl");
        if let Some(s) = scope {
            cmd.arg(s);
        }
        cmd.args([
            "list-units",
            "--failed",
            "--plain",
            "--no-legend",
            "--no-pager",
        ]);
        if let Ok(o) = cmd.output().await {
            for line in String::from_utf8_lossy(&o.stdout).lines() {
                if let Some(unit) = line.split_whitespace().next() {
                    out.push(unit.trim_start_matches('●').trim().to_string());
                }
            }
        }
    }
    out.retain(|u| !u.is_empty());
    out.sort();
    out.dedup();
    out
}
