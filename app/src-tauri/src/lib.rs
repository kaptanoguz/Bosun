//! Bosun arayüzünün Rust tarafı. Arayüz, daemon ile yalnızca buradaki iki komut üzerinden
//! konuşur; izin verilen yöntemler beyaz listeyle sınırlıdır.

use bosun_core::ipc::{socket_path, Request, Response, METHODS};
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// `subscribe` olay akışı için ayrılmıştır; arayüz doğrudan çağıramaz.
fn allowed(method: &str) -> bool {
    method != "subscribe" && METHODS.contains(&method)
}

async fn call(method: &str, params: Value, timeout: Duration) -> Result<Value, String> {
    let fut = async {
        let mut stream = UnixStream::connect(socket_path())
            .await
            .map_err(|_| "DAEMON_OFFLINE".to_string())?;
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let req = Request { id, method: method.into(), params };
        let mut line = serde_json::to_string(&req).map_err(|e| e.to_string())?;
        line.push('\n');
        stream.write_all(line.as_bytes()).await.map_err(|e| e.to_string())?;
        let mut lines = BufReader::new(stream).lines();
        while let Some(l) = lines.next_line().await.map_err(|e| e.to_string())? {
            let Ok(resp) = serde_json::from_str::<Response>(&l) else { continue };
            if resp.id != id {
                continue;
            }
            return if resp.ok {
                Ok(resp.result.unwrap_or(Value::Null))
            } else {
                Err(resp.error.unwrap_or_else(|| "bilinmeyen hata".into()))
            };
        }
        Err("daemon bağlantıyı kapattı".into())
    };
    tokio::time::timeout(timeout, fut).await.map_err(|_| "zaman aşımı".to_string())?
}

#[tauri::command]
async fn daemon_call(method: String, params: Option<Value>) -> Result<Value, String> {
    if !allowed(&method) {
        return Err(format!("izin verilmeyen yöntem: {method}"));
    }
    let timeout = if method == "chat" || method == "scan" { 120 } else { 30 };
    call(&method, params.unwrap_or(Value::Null), Duration::from_secs(timeout)).await
}

/// `systemctl --user start bosund` ile servisi başlatmayı dener.
#[tauri::command]
async fn start_daemon() -> Result<(), String> {
    let status = tokio::process::Command::new("systemctl")
        .args(["--user", "start", "bosund.service"])
        .status()
        .await
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("Servis başlatılamadı. Kurulum adımlarını README'de bulabilirsiniz.".into())
    }
}

/// Daemon'un olay akışını dinler ve arayüze iletir; bağlantı koparsa yeniden dener.
async fn event_pump(app: AppHandle) {
    loop {
        if let Ok(mut stream) = UnixStream::connect(socket_path()).await {
            let sub = r#"{"id":0,"method":"subscribe","params":{}}"#.to_string() + "\n";
            if stream.write_all(sub.as_bytes()).await.is_ok() {
                let _ = app.emit("bosun-connection", true);
                let mut lines = BufReader::new(stream).lines();
                while let Ok(Some(l)) = lines.next_line().await {
                    if let Ok(v) = serde_json::from_str::<Value>(&l) {
                        if v.get("event").is_some() {
                            let _ = app.emit("bosun-event", v);
                        }
                    }
                }
            }
        }
        let _ = app.emit("bosun-connection", false);
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![daemon_call, start_daemon])
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(event_pump(handle));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Bosun başlatılamadı");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitelist() {
        assert!(allowed("snapshot"));
        assert!(allowed("plan.approve"));
        assert!(!allowed("subscribe"));
        assert!(!allowed("exec"));
    }
}
