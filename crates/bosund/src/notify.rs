//! Masaüstü bildirimleri (`notify-send`), aynı bulgu için tekrar koruması ile.

use bosun_core::rules::{Finding, Severity};
use std::collections::HashMap;
use std::time::{Duration, Instant};

const REPEAT_AFTER: Duration = Duration::from_secs(6 * 3600);

#[derive(Default)]
pub struct Notifier {
    last_sent: HashMap<String, Instant>,
}

impl Notifier {
    /// Uyarı ve kritik bulgular için, son 6 saatte gönderilmemişse bildirim gönderir.
    pub fn consider(&mut self, findings: &[Finding], enabled: bool) {
        let now = Instant::now();
        for f in findings
            .iter()
            .filter(|f| f.severity >= Severity::Warning && !f.muted)
        {
            // Kimliği yüzdelik değerden bağımsız tut ("disk:/" gibi).
            let fresh = self
                .last_sent
                .get(&f.id)
                .is_none_or(|t| now.duration_since(*t) > REPEAT_AFTER);
            if !fresh {
                continue;
            }
            self.last_sent.insert(f.id.clone(), now);
            if enabled {
                send(f);
            }
        }
        // Artık geçerli olmayan bulguları unut ki tekrar ortaya çıkarlarsa yeniden bildirilsin.
        self.last_sent
            .retain(|id, _| findings.iter().any(|f| &f.id == id));
    }
}

fn send(f: &Finding) {
    let urgency = if f.severity == Severity::Critical {
        "critical"
    } else {
        "normal"
    };
    let icon = if f.severity == Severity::Critical {
        "dialog-error"
    } else {
        "dialog-warning"
    };
    let res = std::process::Command::new("notify-send")
        .args([
            "-a",
            "Bosun",
            "-u",
            urgency,
            "-i",
            icon,
            &format!("Bosun · {}", f.title),
            &f.detail,
        ])
        .spawn();
    if let Err(e) = res {
        tracing::debug!("could not run notify-send: {e}");
    }
}

/// Haftalık özet: temizlenebilir alan bu kadarı geçince haftada en fazla bir kez hatırlatılır.
pub const SUMMARY_MIN_BYTES: u64 = 1024 * 1024 * 1024;
const SUMMARY_EVERY: chrono::Duration = chrono::Duration::days(7);

fn summary_path() -> std::path::PathBuf {
    crate::audit::dirs_state().join("bosun/weekly-summary")
}

/// Son özetin zamanı (servis yeniden başlasa da korunur).
fn last_summary() -> Option<chrono::DateTime<chrono::Utc>> {
    std::fs::read_to_string(summary_path())
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

/// Özet şimdi gönderilmeli mi? İlk seferde hemen değil: kurulumdan bir hafta sonra.
pub fn summary_due(
    last: Option<chrono::DateTime<chrono::Utc>>,
    now: chrono::DateTime<chrono::Utc>,
    reclaimable: u64,
) -> bool {
    reclaimable >= SUMMARY_MIN_BYTES && last.is_some_and(|t| now - t >= SUMMARY_EVERY)
}

/// Tarama sonrası çağrılır. Bildirim yalnızca bilgi verir; temizlik yine onay ister.
pub fn weekly_summary(reclaimable: u64, enabled: bool, lang: bosun_core::Lang) {
    let now = chrono::Utc::now();
    let last = last_summary();
    if last.is_none() {
        // Saymaya bugünden başla.
        let _ = std::fs::write(summary_path(), now.to_rfc3339());
        return;
    }
    if !enabled || !summary_due(last, now, reclaimable) {
        return;
    }
    let _ = std::fs::write(summary_path(), now.to_rfc3339());
    let size = bosun_core::util::human_bytes(reclaimable);
    let title = lang.s(
        format!("Bosun · {size} can be freed"),
        format!("Bosun · {size} yer açılabilir"),
    );
    let body = lang.s(
        "Caches and temporary files. Open Bosun to review them; nothing is removed until you approve.",
        "Önbellekler ve geçici dosyalar. Gözden geçirmek için Bosun'u açın; siz onaylamadan hiçbir şey silinmez.",
    );
    let res = std::process::Command::new("notify-send")
        .args(["-a", "Bosun", "-i", "edit-clear", &title, &body])
        .spawn();
    if let Err(e) = res {
        tracing::debug!("could not run notify-send: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    #[test]
    fn summary_waits_a_week_and_needs_enough_space() {
        let now = Utc::now();
        let gb = SUMMARY_MIN_BYTES;
        assert!(
            !summary_due(None, now, 10 * gb),
            "first run only starts the clock"
        );
        assert!(!summary_due(Some(now - Duration::days(6)), now, 10 * gb));
        assert!(summary_due(Some(now - Duration::days(7)), now, gb));
        assert!(!summary_due(Some(now - Duration::days(30)), now, gb - 1));
    }
}
