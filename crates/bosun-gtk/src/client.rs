//! Servisle konuşma: Unix socket üzerinden NDJSON istekleri ve olay akışı.
//!
//! Ağ işleri arka plandaki bir tokio çalışma ortamında yapılır; GTK ana döngüsü yalnızca
//! sonuçları bekler (`glib::spawn_future_local` içinden `await`).

use anyhow::{anyhow, bail, Context, Result};
use bosun_core::ipc::{socket_path, Request, Response, METHODS};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Uygulama boyunca tek bir arka plan çalışma ortamı.
pub fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("could not start the background runtime")
    })
}

/// Servis kapalıyken dönen hata; arayüz bunu "servis çalışmıyor" olarak gösterir.
pub const OFFLINE: &str = "DAEMON_OFFLINE";

fn timeout_for(method: &str) -> Duration {
    match method {
        "chat" | "scan" | "usage" => Duration::from_secs(120),
        _ => Duration::from_secs(30),
    }
}

async fn call_raw(method: String, params: Value) -> Result<Value> {
    let mut stream = UnixStream::connect(socket_path())
        .await
        .map_err(|_| anyhow!(OFFLINE))?;
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let req = Request { id, method, params };
    let mut line = serde_json::to_string(&req)?;
    line.push('\n');
    stream.write_all(line.as_bytes()).await?;
    let mut lines = BufReader::new(stream).lines();
    while let Some(l) = lines.next_line().await? {
        let resp: Response = serde_json::from_str(&l).context("invalid response")?;
        if resp.id != id {
            continue;
        }
        return if resp.ok {
            Ok(resp.result.unwrap_or(Value::Null))
        } else {
            Err(anyhow!(resp.error.unwrap_or_else(|| "error".into())))
        };
    }
    bail!("the service closed the connection")
}

/// Servise bir istek gönderir. Yalnızca izin listesindeki yöntemler (abonelik hariç).
pub async fn call(method: &str, params: Value) -> Result<Value> {
    if method == "subscribe" || !METHODS.contains(&method) {
        bail!("method not allowed: {method}");
    }
    let (m, wait) = (method.to_string(), timeout_for(method));
    runtime()
        .spawn(async move {
            tokio::time::timeout(wait, call_raw(m, params))
                .await
                .map_err(|_| anyhow!("the service did not answer in time"))?
        })
        .await?
}

/// İsteği gönderir ve yanıtı verilen türe çevirir.
pub async fn get<T: DeserializeOwned>(method: &str, params: Value) -> Result<T> {
    Ok(serde_json::from_value(call(method, params).await?)?)
}

pub fn is_offline(e: &anyhow::Error) -> bool {
    e.to_string() == OFFLINE
}

/// Servisten gelenler: bağlantı durumu ya da bir olay.
#[derive(Debug, Clone)]
pub enum Incoming {
    Connected(bool),
    Event(String, Value),
}

/// Olay akışını dinler; bağlantı koparsa birkaç saniyede bir yeniden dener.
pub fn subscribe() -> async_channel::Receiver<Incoming> {
    let (tx, rx) = async_channel::unbounded();
    runtime().spawn(async move {
        let mut was_up: Option<bool> = None;
        loop {
            if let Ok(mut stream) = UnixStream::connect(socket_path()).await {
                let sub = json!({"id": 0, "method": "subscribe", "params": {}}).to_string() + "\n";
                if stream.write_all(sub.as_bytes()).await.is_ok() {
                    was_up = Some(true);
                    if tx.send(Incoming::Connected(true)).await.is_err() {
                        return;
                    }
                    let mut lines = BufReader::new(stream).lines();
                    while let Ok(Some(l)) = lines.next_line().await {
                        let Ok(v) = serde_json::from_str::<Value>(&l) else {
                            continue;
                        };
                        if let Some(ev) = v["event"].as_str() {
                            if tx
                                .send(Incoming::Event(ev.to_string(), v["data"].clone()))
                                .await
                                .is_err()
                            {
                                return;
                            }
                        }
                    }
                }
            }
            if was_up != Some(false) {
                was_up = Some(false);
                if tx.send(Incoming::Connected(false)).await.is_err() {
                    return;
                }
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    });
    rx
}

async fn run(cmd: &str, args: &[&str]) -> Result<std::process::Output> {
    let (cmd, args): (String, Vec<String>) =
        (cmd.into(), args.iter().map(|s| s.to_string()).collect());
    runtime()
        .spawn(async move {
            tokio::process::Command::new(&cmd)
                .args(&args)
                .output()
                .await
        })
        .await?
        .context("could not run a command")
}

/// Servis birimi kurulu mu? .deb paketi `/usr/lib/systemd/user/bosund.service` getirir,
/// kaynak koddan kurulumda `./install.sh` yazar.
async fn ensure_unit() -> Result<()> {
    let has_unit = run("systemctl", &["--user", "cat", "bosund.service"])
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !has_unit {
        bail!(
            "bosund.service is not installed. Install the Bosun .deb package or run ./install.sh"
        );
    }
    Ok(())
}

/// `systemctl --user enable --now bosund`: servisi başlatır ve oturum açılışına ekler.
pub async fn start_daemon() -> Result<()> {
    ensure_unit().await?;
    let out = run(
        "systemctl",
        &["--user", "enable", "--now", "bosund.service"],
    )
    .await?;
    if !out.status.success() {
        bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(())
}

/// Güncellemeden sonra yeni servis sürümüne geçmek için.
pub async fn restart_daemon() -> Result<()> {
    run("systemctl", &["--user", "daemon-reload"]).await?;
    let out = run("systemctl", &["--user", "restart", "bosund.service"]).await?;
    if !out.status.success() {
        bail!("systemctl --user restart bosund failed");
    }
    Ok(())
}

/// Yönetici eyleminin sonucu.
pub enum AdminOutcome {
    Done {
        freed_bytes: u64,
        message: Option<String>,
    },
    Cancelled,
}

/// Yönetici yetkisi gerektiren bir eylemi `pkexec` ile yardımcıya yaptırır.
///
/// Şifreyi masaüstünün kendi polkit ajanı sorar; bu yüzden çağrı arka plan servisinden
/// değil, grafik oturumdaki arayüzden yapılır. Yalnızca bilinen eylemler kabul edilir ve
/// yardımcı root'a ait değilse çalıştırılmaz. pkexec yalnızca sistem konumlarında aranır.
pub async fn run_admin(action: &str) -> Result<AdminOutcome> {
    use bosun_core::admin::{helper_installed, helper_status, is_known, HELPER_PATH};
    if !is_known(action) {
        bail!("unknown action: {action}");
    }
    if !helper_installed() {
        bail!("HELPER_MISSING:{:?}", helper_status());
    }
    let pkexec = ["/usr/bin/pkexec", "/bin/pkexec"]
        .into_iter()
        .find(|p| std::path::Path::new(p).exists())
        .context("pkexec is not installed (package: pkexec or policykit-1)")?;
    let out = run(pkexec, &[HELPER_PATH, action]).await?;
    // 126: pencere kapatıldı / yetki verilmedi, 127: doğrulama başarısız.
    if matches!(out.status.code(), Some(126) | Some(127)) && out.stdout.is_empty() {
        return Ok(AdminOutcome::Cancelled);
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let result: Value = stdout
        .lines()
        .rev()
        .find_map(|l| serde_json::from_str(l).ok())
        .with_context(|| {
            format!(
                "unexpected helper output: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )
        })?;
    let ok = result["ok"].as_bool().unwrap_or(false);
    let freed_bytes = result["freed_bytes"].as_u64().unwrap_or(0);
    // Kayıt ve yeniden tarama için servise bildir (kapalıysa sessizce geç).
    let _ = call(
        "admin.report",
        json!({ "action": action, "ok": ok, "freed_bytes": freed_bytes, "error": result["error"] }),
    )
    .await;
    if ok {
        let message = result["message"]
            .as_str()
            .map(str::trim)
            .filter(|m| !m.is_empty())
            .map(str::to_string);
        Ok(AdminOutcome::Done {
            freed_bytes,
            message,
        })
    } else {
        bail!("{}", result["error"].as_str().unwrap_or("helper failed"))
    }
}

/// Hata bildirimi için tanılama metni. API anahtarları asla eklenmez; günlükte anahtara
/// benzeyen her şey gizlenir.
pub async fn diagnostics() -> String {
    async fn sh(cmd: &str, args: &[&str]) -> String {
        match run(cmd, args).await {
            Ok(o) => {
                String::from_utf8_lossy(&o.stdout).into_owned()
                    + &String::from_utf8_lossy(&o.stderr)
            }
            Err(e) => format!("({cmd}: {e})\n"),
        }
    }
    let mut out = format!("Bosun app {} (GTK)\n", env!("CARGO_PKG_VERSION"));
    out += &format!("helper: {:?}\n", bosun_core::admin::helper_status());
    out += &format!("socket: {}\n", socket_path().display());
    out += &format!(
        "service: {}",
        sh("systemctl", &["--user", "is-active", "bosund.service"]).await
    );
    out += &format!(
        "os: {}",
        sh("sh", &["-c", ". /etc/os-release && echo \"$PRETTY_NAME\""]).await
    );
    match call("hello", Value::Null).await {
        Ok(h) => {
            out += &format!(
                "daemon {} · provider {} · model {} · chat {} · language {}\n",
                h["version"], h["provider"], h["model"], h["chat_enabled"], h["language"]
            )
        }
        Err(e) => out += &format!("daemon: {e}\n"),
    }
    out += "\n--- journalctl --user -u bosund (last 60) ---\n";
    out += &sh(
        "journalctl",
        &[
            "--user",
            "-u",
            "bosund.service",
            "-n",
            "60",
            "--no-pager",
            "-o",
            "short-iso",
        ],
    )
    .await;
    redact(&out)
}

/// Anahtara benzeyen parçaları gizler (sk-…, xai-…, AIza…).
pub fn redact(s: &str) -> String {
    s.split_inclusive(|c: char| c.is_whitespace() || c == '"' || c == '\'')
        .map(|w| {
            let t = w.trim_matches(|c: char| c.is_whitespace() || c == '"' || c == '\'');
            if t.len() > 12
                && (t.starts_with("sk-") || t.starts_with("xai-") || t.starts_with("AIza"))
            {
                w.replace(t, "[redacted]")
            } else {
                w.to_string()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscribe_and_unknown_methods_are_refused() {
        let r = runtime();
        assert!(r.block_on(call("subscribe", Value::Null)).is_err());
        assert!(r.block_on(call("rm_rf", Value::Null)).is_err());
    }

    #[test]
    fn redacts_keys() {
        let s = redact("key sk-ant-abcdefghijklmnop and \"xai-1234567890abcdef\" ok");
        assert!(!s.contains("sk-ant-abc") && !s.contains("xai-123"));
        assert!(s.contains("ok"));
    }
}
