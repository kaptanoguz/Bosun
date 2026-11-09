//! Bosun arayüzünün Rust tarafı. Arayüz, daemon ile yalnızca buradaki iki komut üzerinden
//! konuşur; izin verilen yöntemler beyaz listeyle sınırlıdır.

use bosun_core::ipc::{socket_path, Request, Response, METHODS};
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, WindowEvent};
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

/// `systemctl --user enable --now bosund` ile servisi başlatır ve oturum açılışına ekler.
#[tauri::command]
async fn start_daemon() -> Result<(), String> {
    let status = tokio::process::Command::new("systemctl")
        .args(["--user", "enable", "--now", "bosund.service"])
        .status()
        .await
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("Servis başlatılamadı. Kurulum adımlarını README'de bulabilirsiniz.".into())
    }
}

/// Yardımcının durumu, pkexec'i çalıştıracak olan bu süreçten bakılarak.
#[tauri::command]
fn helper_status() -> bosun_core::admin::HelperStatus {
    bosun_core::admin::helper_status()
}

/// Yönetici yetkisi gerektiren bir eylemi `pkexec` ile yardımcıya yaptırır.
///
/// Şifre penceresini polkit (masaüstünün kendi doğrulama ajanı) gösterir; bu yüzden
/// çağrı, grafik oturumda çalışan arayüzden yapılır, arka plan servisinden değil.
/// Yalnızca bilinen eylem kimlikleri kabul edilir ve yardımcı root'a ait değilse çalıştırılmaz.
#[tauri::command]
async fn run_admin(action: String) -> Result<Value, String> {
    use bosun_core::admin::{helper_installed, is_known, HELPER_PATH};
    if !is_known(&action) {
        return Err(format!("unknown action: {action}"));
    }
    if !helper_installed() {
        return Err(format!("HELPER_MISSING:{:?}", bosun_core::admin::helper_status()));
    }
    let out = tokio::process::Command::new("/usr/bin/pkexec")
        .args([HELPER_PATH, action.as_str()])
        .output()
        .await
        .map_err(|e| format!("pkexec: {e}"))?;
    // 126: kullanıcı pencereyi kapattı / yetki verilmedi, 127: doğrulama başarısız.
    if matches!(out.status.code(), Some(126) | Some(127)) && out.stdout.is_empty() {
        return Err("CANCELLED".into());
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let result: Value = stdout
        .lines()
        .rev()
        .find_map(|l| serde_json::from_str(l).ok())
        .ok_or_else(|| format!("unexpected helper output: {}", String::from_utf8_lossy(&out.stderr).trim()))?;
    let ok = result["ok"].as_bool().unwrap_or(false);
    let report = serde_json::json!({
        "action": action,
        "ok": ok,
        "freed_bytes": result["freed_bytes"].as_u64().unwrap_or(0),
        "error": result["error"],
    });
    // Kayıt ve yeniden tarama için servise bildir (servis kapalıysa sessizce geç).
    let _ = call("admin.report", report, Duration::from_secs(10)).await;
    if ok {
        Ok(result)
    } else {
        Err(result["error"].as_str().unwrap_or("helper failed").to_string())
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

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let tr = bosun_core::config::Config::load().language == bosun_core::Lang::Tr;
    let show = MenuItem::with_id(app, "show", if tr { "Bosun'u aç" } else { "Open Bosun" }, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", if tr { "Çık" } else { "Quit" }, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;
    let mut tray = TrayIconBuilder::with_id("bosun")
        .tooltip("Bosun")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, e| match e.id.as_ref() {
            "show" => show_main(app),
            "quit" => app.exit(0),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![daemon_call, start_daemon, run_admin, helper_status])
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(event_pump(handle));
            setup_tray(app)?;
            Ok(())
        })
        // Pencere kapatılınca uygulama tepside çalışmaya devam eder; çıkış tepsi menüsünden.
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
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
