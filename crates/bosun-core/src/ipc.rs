//! Daemon ile arayüz arasındaki protokol: satır başına bir JSON (NDJSON) üzerinden
//! istek/yanıt ve sunucudan itilen olaylar.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

pub const PROTOCOL_VERSION: u32 = 1;

/// Arayüzün çağırmasına izin verilen yöntemler. Listede olmayan her şey reddedilir.
pub const METHODS: &[&str] = &[
    "hello",
    "snapshot",
    "scan",
    "findings",
    "plan.create",
    "plan.approve",
    "plan.cancel",
    "history",
    "chat",
    "chat.history",
    "chat.reset",
    "settings.get",
    "settings.set",
    "admin.info",
    "admin.applied",
    "admin.report",
    "scan.cancel",
    "findings.mute",
    "scan.inspect",
    "subscribe",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub id: u64,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Response {
    pub fn ok(id: u64, result: Value) -> Self {
        Self {
            id,
            ok: true,
            result: Some(result),
            error: None,
        }
    }
    pub fn err(id: u64, e: impl ToString) -> Self {
        Self {
            id,
            ok: false,
            result: None,
            error: Some(e.to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub event: String,
    pub data: Value,
}

/// `$XDG_RUNTIME_DIR/bosun.sock` (yoksa `/tmp/bosun-<uid>.sock`).
pub fn socket_path() -> PathBuf {
    if let Ok(p) = std::env::var("BOSUN_SOCKET") {
        return PathBuf::from(p);
    }
    match std::env::var("XDG_RUNTIME_DIR") {
        Ok(dir) if !dir.is_empty() => PathBuf::from(dir).join("bosun.sock"),
        _ => {
            let uid = std::fs::metadata(dirs::home_dir().unwrap_or_default())
                .map(|m| std::os::unix::fs::MetadataExt::uid(&m))
                .unwrap_or(0);
            PathBuf::from(format!("/tmp/bosun-{uid}.sock"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_params_default_to_null() {
        let r: Request = serde_json::from_str(r#"{"id":1,"method":"snapshot"}"#).unwrap();
        assert!(r.params.is_null());
    }

    #[test]
    fn response_omits_empty_fields() {
        let s = serde_json::to_string(&Response::ok(2, serde_json::json!(1))).unwrap();
        assert!(!s.contains("error"));
    }
}
