//! LLM'siz kural motoru: metriklerden ve taramadan bulgular üretir.

use crate::categories::CategoryId;
use crate::config::Thresholds;
use crate::metrics::Snapshot;
use crate::scan::ScanResult;
use crate::i18n::Lang;
use crate::util::bytes_in;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// Kararlı kimlik (bildirim tekrarını önlemek için kullanılır).
    pub id: String,
    pub severity: Severity,
    pub title: String,
    pub detail: String,
    pub category: Option<CategoryId>,
}

pub struct RuleInput<'a> {
    pub snapshot: &'a Snapshot,
    /// RAM'in kaç saniyedir eşiğin üzerinde olduğu.
    pub mem_high_secs: u64,
    pub scan: Option<&'a ScanResult>,
    pub failed_units: &'a [String],
    pub lang: Lang,
}

pub fn evaluate(input: &RuleInput, t: &Thresholds) -> Vec<Finding> {
    let mut out = Vec::new();
    let s = input.snapshot;
    let l = input.lang;
    let b = |n: u64| bytes_in(n, l);

    for d in &s.disks {
        let pct = d.used_percent();
        let sev = if pct >= t.disk_crit {
            Severity::Critical
        } else if pct >= t.disk_warn {
            Severity::Warning
        } else {
            continue;
        };
        out.push(Finding {
            id: format!("disk:{}", d.mount),
            severity: sev,
            title: l.s(format!("{} is {:.0}% full", d.mount, pct), format!("{} %{:.0} dolu", d.mount, pct)),
            detail: l.s(
                format!("{} free of {}.", b(d.available), b(d.total)),
                format!("{} boş alan kaldı ({} toplam).", b(d.available), b(d.total)),
            ),
            category: None,
        });
    }

    if s.mem_percent() >= t.mem_warn && input.mem_high_secs >= t.mem_sustain_secs {
        let mins = input.mem_high_secs / 60;
        out.push(Finding {
            id: "mem".into(),
            severity: Severity::Warning,
            title: l.s(format!("Memory {:.0}% in use", s.mem_percent()), format!("Bellek %{:.0} dolu", s.mem_percent())),
            detail: l.s(format!("It has stayed high for {mins} minutes."), format!("{mins} dakikadır yüksek seyrediyor.")),
            category: None,
        });
    }

    if s.swap_total > 0 && s.swap_percent() >= t.swap_warn {
        out.push(Finding {
            id: "swap".into(),
            severity: Severity::Warning,
            title: l.s(format!("Swap {:.0}% in use", s.swap_percent()), format!("Swap %{:.0} kullanımda", s.swap_percent())),
            detail: l.s(
                "The system is writing memory to disk, which slows things down.",
                "Sistem belleğe sığmayan veriyi diske yazıyor; bu yavaşlamaya yol açar.",
            ),
            category: None,
        });
    }

    if let Some(temp) = s.max_temp() {
        if temp >= t.temp_crit {
            out.push(Finding {
                id: "temp".into(),
                severity: Severity::Critical,
                title: l.s(format!("Temperature {temp:.0} °C"), format!("Sıcaklık {temp:.0} °C")),
                detail: l.s("Check the fans and airflow.", "Fanları ve hava akışını kontrol edin."),
                category: None,
            });
        }
    }

    if !input.failed_units.is_empty() {
        let n = input.failed_units.len();
        out.push(Finding {
            id: format!("units:{}", input.failed_units.join(",")),
            severity: Severity::Warning,
            title: l.s(
                format!("{n} service{} failed", if n == 1 { "" } else { "s" }),
                format!("{n} servis başarısız"),
            ),
            detail: input.failed_units.join(", "),
            category: None,
        });
    }

    if let Some(scan) = input.scan {
        let reclaim = scan.reclaimable_bytes();
        if reclaim as f64 >= t.reclaimable_info_gb as f64 * 1024.0 * 1024.0 * 1024.0 {
            out.push(Finding {
                id: "reclaimable".into(),
                severity: Severity::Info,
                title: l.s(format!("{} can be safely cleaned", b(reclaim)), format!("{} güvenle temizlenebilir", b(reclaim))),
                detail: l.s(
                    "Caches and temporary files. Review them on the Cleanup tab.",
                    "Önbellekler ve geçici dosyalar. Temizlik sekmesinden gözden geçirebilirsiniz.",
                ),
                category: None,
            });
        }
        for id in [CategoryId::Journal, CategoryId::AptCache, CategoryId::FlatpakUnused] {
            if let Some(c) = scan.get(id) {
                if c.bytes >= 1024 * 1024 * 1024 {
                    let cmd = id.manual_command().unwrap_or("");
                    out.push(Finding {
                        id: format!("big:{}", id.as_str()),
                        severity: Severity::Info,
                        title: format!("{}: {}", id.title(l), b(c.bytes)),
                        detail: l.s(
                            format!("Clean it from the Cleanup tab (asks for your password) or run: {cmd}"),
                            format!("Temizlik sekmesinden temizleyin (şifrenizi sorar) ya da çalıştırın: {cmd}"),
                        ),
                        category: Some(id),
                    });
                }
            }
        }
    }

    out.sort_by_key(|f| std::cmp::Reverse(f.severity));
    out
}

/// Genel sağlık durumu: bulguların en ağırı.
pub fn overall(findings: &[Finding]) -> Severity {
    findings.iter().map(|f| f.severity).max().unwrap_or(Severity::Info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::DiskInfo;
    use chrono::Utc;

    fn snap(disk_avail: u64) -> Snapshot {
        Snapshot {
            ts: Utc::now(),
            cpu_percent: 5.0,
            cpu_count: 4,
            mem_total: 100,
            mem_used: 95,
            swap_total: 0,
            swap_used: 0,
            load_avg: [0.0; 3],
            uptime_secs: 1,
            disks: vec![DiskInfo { mount: "/".into(), device: "sda".into(), fs: "ext4".into(), total: 100, available: disk_avail }],
            temps: vec![],
        }
    }

    #[test]
    fn disk_thresholds() {
        let t = Thresholds::default();
        let s = snap(3);
        let f = evaluate(&RuleInput { snapshot: &s, mem_high_secs: 0, scan: None, failed_units: &[], lang: Lang::En }, &t);
        assert_eq!(f[0].severity, Severity::Critical);
        let s = snap(50);
        let f = evaluate(&RuleInput { snapshot: &s, mem_high_secs: 0, scan: None, failed_units: &[], lang: Lang::En }, &t);
        assert!(f.is_empty());
    }

    #[test]
    fn memory_needs_sustained_pressure() {
        let t = Thresholds::default();
        let s = snap(50);
        let short = evaluate(&RuleInput { snapshot: &s, mem_high_secs: 10, scan: None, failed_units: &[], lang: Lang::En }, &t);
        assert!(short.is_empty());
        let long = evaluate(&RuleInput { snapshot: &s, mem_high_secs: 600, scan: None, failed_units: &[], lang: Lang::En }, &t);
        assert_eq!(long.len(), 1);
    }
}
