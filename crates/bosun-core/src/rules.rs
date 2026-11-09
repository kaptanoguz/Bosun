//! LLM'siz kural motoru: metriklerden ve taramadan bulgular üretir.

use crate::categories::CategoryId;
use crate::config::Thresholds;
use crate::metrics::Snapshot;
use crate::scan::ScanResult;
use crate::util::human_bytes;
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
}

pub fn evaluate(input: &RuleInput, t: &Thresholds) -> Vec<Finding> {
    let mut out = Vec::new();
    let s = input.snapshot;

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
            title: format!("{} %{:.0} dolu", d.mount, pct),
            detail: format!("{} boş alan kaldı ({} toplam).", human_bytes(d.available), human_bytes(d.total)),
            category: None,
        });
    }

    if s.mem_percent() >= t.mem_warn && input.mem_high_secs >= t.mem_sustain_secs {
        out.push(Finding {
            id: "mem".into(),
            severity: Severity::Warning,
            title: format!("Bellek %{:.0} dolu", s.mem_percent()),
            detail: format!("{} dakikadır yüksek seyrediyor.", input.mem_high_secs / 60),
            category: None,
        });
    }

    if s.swap_total > 0 && s.swap_percent() >= t.swap_warn {
        out.push(Finding {
            id: "swap".into(),
            severity: Severity::Warning,
            title: format!("Swap %{:.0} kullanımda", s.swap_percent()),
            detail: "Sistem belleğe sığmayan veriyi diske yazıyor; bu yavaşlamaya yol açar.".into(),
            category: None,
        });
    }

    if let Some(temp) = s.max_temp() {
        if temp >= t.temp_crit {
            out.push(Finding {
                id: "temp".into(),
                severity: Severity::Critical,
                title: format!("Sıcaklık {temp:.0} °C"),
                detail: "Fanları ve hava akışını kontrol edin.".into(),
                category: None,
            });
        }
    }

    if !input.failed_units.is_empty() {
        out.push(Finding {
            id: format!("units:{}", input.failed_units.join(",")),
            severity: Severity::Warning,
            title: format!("{} servis başarısız", input.failed_units.len()),
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
                title: format!("{} güvenle temizlenebilir", human_bytes(reclaim)),
                detail: "Önbellekler ve geçici dosyalar. Temizlik sekmesinden gözden geçirebilirsiniz.".into(),
                category: None,
            });
        }
        for id in [CategoryId::Journal, CategoryId::AptCache] {
            if let Some(c) = scan.get(id) {
                if c.bytes >= 1024 * 1024 * 1024 {
                    out.push(Finding {
                        id: format!("big:{}", id.as_str()),
                        severity: Severity::Info,
                        title: format!("{}: {}", c.title, human_bytes(c.bytes)),
                        detail: format!("Terminalde çalıştırın: {}", id.manual_command().unwrap_or("")),
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
        let f = evaluate(&RuleInput { snapshot: &s, mem_high_secs: 0, scan: None, failed_units: &[] }, &t);
        assert_eq!(f[0].severity, Severity::Critical);
        let s = snap(50);
        let f = evaluate(&RuleInput { snapshot: &s, mem_high_secs: 0, scan: None, failed_units: &[] }, &t);
        assert!(f.is_empty());
    }

    #[test]
    fn memory_needs_sustained_pressure() {
        let t = Thresholds::default();
        let s = snap(50);
        let short = evaluate(&RuleInput { snapshot: &s, mem_high_secs: 10, scan: None, failed_units: &[] }, &t);
        assert!(short.is_empty());
        let long = evaluate(&RuleInput { snapshot: &s, mem_high_secs: 600, scan: None, failed_units: &[] }, &t);
        assert_eq!(long.len(), 1);
    }
}
