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
