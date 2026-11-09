//! Denetim günlüğü: `~/.local/state/bosun/audit.jsonl` (yalnızca ekleme).

use chrono::Utc;
use serde_json::{json, Value};
use std::io::Write;
use std::path::PathBuf;

#[derive(Clone)]
pub struct AuditLog {
    path: PathBuf,
}

impl AuditLog {
    pub fn new() -> Self {
        let dir = dirs_state().join("bosun");
        let _ = std::fs::create_dir_all(&dir);
        Self {
            path: dir.join("audit.jsonl"),
        }
    }

    pub fn append(&self, kind: &str, data: Value) {
        self.rotate_if_needed();
        let line = json!({ "ts": Utc::now(), "kind": kind, "data": data });
        let res = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .and_then(|mut f| writeln!(f, "{line}"));
        if let Err(e) = res {
            tracing::error!("could not write audit log: {e}");
        }
    }

    /// Günlük 2 MB'ı aşınca bir önceki kopyanın (`audit.jsonl.1`) yerine taşınır.
    /// En fazla ~4 MB yer kaplar; eski kayıtlar böylece kendiliğinden düşer.
    fn rotate_if_needed(&self) {
        const MAX: u64 = 2 * 1024 * 1024;
        if std::fs::metadata(&self.path)
            .map(|m| m.len() > MAX)
            .unwrap_or(false)
        {
            let _ = std::fs::rename(&self.path, self.path.with_extension("jsonl.1"));
        }
    }

    /// Son `n` kayıt, en yenisi başta (gerekirse döndürülmüş dosyadan da okur).
    pub fn tail(&self, n: usize) -> Vec<Value> {
        let mut out: Vec<Value> = Vec::new();
        for p in [self.path.clone(), self.path.with_extension("jsonl.1")] {
            if out.len() >= n {
                break;
            }
            let Ok(text) = std::fs::read_to_string(&p) else {
                continue;
            };
            out.extend(
                text.lines()
                    .rev()
                    .filter_map(|l| serde_json::from_str(l).ok())
                    .take(n - out.len()),
            );
        }
        out
    }
}

pub fn dirs_state() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}
