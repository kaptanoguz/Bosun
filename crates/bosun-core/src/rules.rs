//! LLM'siz kural motoru: metriklerden ve taramadan bulgular üretir.

use crate::categories::CategoryId;
use crate::config::Thresholds;
use crate::growth::Growth;
use crate::i18n::Lang;
use crate::metrics::Snapshot;
use crate::scan::ScanResult;
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
    /// Kullanıcı bu bulguyu sessize aldı: bildirim gitmez, genel duruma etki etmez.
    #[serde(default)]
    pub muted: bool,
}

pub struct RuleInput<'a> {
    pub snapshot: &'a Snapshot,
    /// RAM'in kaç saniyedir eşiğin üzerinde olduğu.
    pub mem_high_secs: u64,
    /// Toplam CPU'nun kaç saniyedir eşiğin üzerinde olduğu.
    pub cpu_high_secs: u64,
    /// Belleği düzenli büyüyen uygulamalar.
    pub growing: &'a [Growth],
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
            title: l.s(
                format!("{} is {:.0}% full", d.mount, pct),
                format!("{} %{:.0} dolu", d.mount, pct),
            ),
            detail: l.s(
                format!("{} free of {}.", b(d.available), b(d.total)),
                format!("{} boş alan kaldı ({} toplam).", b(d.available), b(d.total)),
            ),
            category: None,
            muted: false,
        });
    }

    if s.cpu_percent >= t.cpu_busy && input.cpu_high_secs >= t.cpu_sustain_secs {
        let mut apps: Vec<_> = s.top_apps.iter().filter(|a| a.cpu_percent >= 1.0).collect();
        apps.sort_by(|a, b| b.cpu_percent.total_cmp(&a.cpu_percent));
        let list = apps
            .iter()
            .take(3)
            .map(|a| format!("{} {:.0}%", a.name, a.cpu_percent))
            .collect::<Vec<_>>()
            .join(", ");
        let secs = input.cpu_high_secs;
        let dur_en = if secs >= 120 {
            format!("{} minutes", secs / 60)
        } else {
            format!("{secs} seconds")
        };
        let dur_tr = if secs >= 120 {
            format!("{} dakikadır", secs / 60)
        } else {
            format!("{secs} saniyedir")
        };
        out.push(Finding {
            id: "cpu".into(),
            severity: Severity::Warning,
            title: match apps.first().filter(|a| a.cpu_percent >= 10.0) {
                Some(a) => l.s(
                    format!("CPU is busy: {} uses {:.0}% of it", a.name, a.cpu_percent),
                    format!(
                        "İşlemci meşgul: {} %{:.0} kullanıyor",
                        a.name, a.cpu_percent
                    ),
                ),
                None => l.s(
                    format!("CPU {:.0}% busy", s.cpu_percent),
                    format!("İşlemci %{:.0} meşgul", s.cpu_percent),
                ),
            },
            detail: l.s(
                format!(
                    "Above {:.0}% for {dur_en}.{}",
                    t.cpu_busy,
                    if list.is_empty() {
                        String::new()
                    } else {
                        format!(" Busiest: {list}.")
                    }
                ),
                format!(
                    "{dur_tr} %{:.0}'in üzerinde.{}",
                    t.cpu_busy,
                    if list.is_empty() {
                        String::new()
                    } else {
                        format!(" En meşgul: {list}.")
                    }
                ),
            ),
            category: None,
            muted: false,
        });
    }

    if s.mem_percent() >= t.mem_warn && input.mem_high_secs >= t.mem_sustain_secs {
        let mins = input.mem_high_secs / 60;
        let mut apps: Vec<_> = s.top_apps.iter().collect();
        apps.sort_by_key(|a| std::cmp::Reverse(a.mem_bytes));
        let list = apps
            .iter()
            .take(3)
            .map(|a| format!("{} {}", a.name, b(a.mem_bytes)))
            .collect::<Vec<_>>()
            .join(", ");
        let (lead_en, lead_tr) = if list.is_empty() {
            (String::new(), String::new())
        } else {
            (
                format!(" Largest: {list}."),
                format!(" En büyükler: {list}."),
            )
        };
        out.push(Finding {
            id: "mem".into(),
            severity: Severity::Warning,
            title: l.s(
                format!("Memory {:.0}% in use", s.mem_percent()),
                format!("Bellek %{:.0} dolu", s.mem_percent()),
            ),
            detail: l.s(
                format!("It has stayed high for {mins} minutes.{lead_en}"),
                format!("{mins} dakikadır yüksek seyrediyor.{lead_tr}"),
            ),
            category: None,
            muted: false,
        });
    }

    for g in input.growing {
        let mins = g.minutes;
        out.push(Finding {
            id: format!("leak:{}", g.name),
            severity: Severity::Info,
            title: l.s(
                format!("{} keeps using more memory", g.name),
                format!("{} giderek daha çok bellek kullanıyor", g.name),
            ),
            detail: l.s(
                format!(
                    "It grew by about {} per hour over {mins} minutes and now uses {}. Restarting it gives the memory back.",
                    b(g.bytes_per_hour),
                    b(g.now_bytes)
                ),
                format!(
                    "{mins} dakikadır saatte yaklaşık {} büyüdü, şu an {} kullanıyor. Yeniden başlatmak belleği geri verir.",
                    b(g.bytes_per_hour),
                    b(g.now_bytes)
                ),
            ),
            category: None,
            muted: false,
        });
    }

    if s.swap_total > 0 && s.swap_percent() >= t.swap_warn {
        out.push(Finding {
            id: "swap".into(),
            severity: Severity::Warning,
            title: l.s(
                format!("Swap {:.0}% in use", s.swap_percent()),
                format!("Swap %{:.0} kullanımda", s.swap_percent()),
            ),
            detail: l.s(
                "The system is writing memory to disk, which slows things down.",
                "Sistem belleğe sığmayan veriyi diske yazıyor; bu yavaşlamaya yol açar.",
            ),
            category: None,
            muted: false,
        });
    }

    if let Some(temp) = s.max_temp() {
        if temp >= t.temp_crit {
            out.push(Finding {
                id: "temp".into(),
                severity: Severity::Critical,
                title: l.s(
                    format!("Temperature {temp:.0} °C"),
                    format!("Sıcaklık {temp:.0} °C"),
                ),
                detail: l.s(
                    "Check the fans and airflow.",
                    "Fanları ve hava akışını kontrol edin.",
                ),
                category: None,
                muted: false,
            });
        }
    }

    if let Some(b) = s
        .battery
        .as_ref()
        .filter(|b| b.discharging() && b.percent <= 15.0)
    {
        out.push(Finding {
            id: "battery".into(),
            severity: if b.percent <= 7.0 {
                Severity::Critical
            } else {
                Severity::Warning
            },
            title: l.s(
                format!("Battery at {:.0}%", b.percent),
                format!("Pil %{:.0}", b.percent),
            ),
            detail: l.s("Plug in the charger soon.", "Şarj aletini yakında takın."),
            category: None,
            muted: false,
        });
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
            muted: false,
        });
    }

    if let Some(scan) = input.scan {
        let reclaim = scan.reclaimable_bytes();
        if reclaim as f64 >= t.reclaimable_info_gb as f64 * 1024.0 * 1024.0 * 1024.0 {
            out.push(Finding {
                id: "reclaimable".into(),
                severity: Severity::Info,
                title: l.s(
                    format!("{} can be safely cleaned", b(reclaim)),
                    format!("{} güvenle temizlenebilir", b(reclaim)),
                ),
                detail: l.s(
                    "Caches and temporary files. Review them on the Cleanup tab.",
                    "Önbellekler ve geçici dosyalar. Temizlik sekmesinden gözden geçirebilirsiniz.",
                ),
                category: None,
                muted: false,
            });
        }
        for id in [
            CategoryId::Journal,
            CategoryId::AptCache,
            CategoryId::FlatpakUnused,
        ] {
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
                        muted: false,
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
    findings
        .iter()
        .filter(|f| !f.muted)
        .map(|f| f.severity)
        .max()
        .unwrap_or(Severity::Info)
}

/// Sessize alınan bulguları işaretler.
pub fn apply_mutes(findings: &mut [Finding], muted: &[String]) {
    for f in findings {
        f.muted = muted.iter().any(|m| m == &f.id);
    }
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
            disks: vec![DiskInfo {
                mount: "/".into(),
                device: "sda".into(),
                fs: "ext4".into(),
                total: 100,
                available: disk_avail,
            }],
            temps: vec![],
            net_rx_bps: 0,
            net_tx_bps: 0,
            battery: None,
            gpu: None,
            disk_read_bps: 0,
            disk_write_bps: 0,
            top_apps: vec![],
        }
    }

    #[test]
    fn busy_cpu_names_the_busiest_app() {
        use crate::metrics::AppUsage;
        let t = Thresholds::default();
        let mut s = snap(50);
        s.mem_used = 10;
        s.cpu_percent = 97.0;
        s.top_apps = vec![
            AppUsage {
                name: "zen".into(),
                cpu_percent: 5.0,
                mem_bytes: 1,
                processes: 3,
            },
            AppUsage {
                name: "bun".into(),
                cpu_percent: 61.0,
                mem_bytes: 1,
                processes: 9,
            },
        ];
        let input = |secs| RuleInput {
            snapshot: &s,
            mem_high_secs: 0,
            cpu_high_secs: secs,
            growing: &[],
            scan: None,
            failed_units: &[],
            lang: Lang::En,
        };
        assert!(evaluate(&input(10), &t).is_empty());
        let f = evaluate(&input(90), &t);
        assert_eq!(f[0].id, "cpu");
        assert_eq!(f[0].title, "CPU is busy: bun uses 61% of it");
        assert!(f[0].detail.contains("bun 61%, zen 5%"), "{}", f[0].detail);
    }

    #[test]
    fn memory_growth_is_reported() {
        let t = Thresholds::default();
        let mut s = snap(50);
        s.mem_used = 10;
        let g = [Growth {
            name: "zen".into(),
            bytes_per_hour: 207 << 20,
            minutes: 70,
            now_bytes: 10 << 30,
        }];
        let f = evaluate(
            &RuleInput {
                snapshot: &s,
                mem_high_secs: 0,
                cpu_high_secs: 0,
                growing: &g,
                scan: None,
                failed_units: &[],
                lang: Lang::En,
            },
            &t,
        );
        assert_eq!(f[0].id, "leak:zen");
        assert!(f[0].detail.contains("70 minutes"));
    }

    #[test]
    fn disk_thresholds() {
        let t = Thresholds::default();
        let s = snap(3);
        let f = evaluate(
            &RuleInput {
                snapshot: &s,
                mem_high_secs: 0,
                cpu_high_secs: 0,
                growing: &[],
                scan: None,
                failed_units: &[],
                lang: Lang::En,
            },
            &t,
        );
        assert_eq!(f[0].severity, Severity::Critical);
        let s = snap(50);
        let f = evaluate(
            &RuleInput {
                snapshot: &s,
                mem_high_secs: 0,
                cpu_high_secs: 0,
                growing: &[],
                scan: None,
                failed_units: &[],
                lang: Lang::En,
            },
            &t,
        );
        assert!(f.is_empty());
    }

    #[test]
    fn muted_findings_do_not_count() {
        let t = Thresholds::default();
        let s = snap(3);
        let mut f = evaluate(
            &RuleInput {
                snapshot: &s,
                mem_high_secs: 0,
                cpu_high_secs: 0,
                growing: &[],
                scan: None,
                failed_units: &[],
                lang: Lang::En,
            },
            &t,
        );
        assert_eq!(overall(&f), Severity::Critical);
        apply_mutes(&mut f, &["disk:/".to_string()]);
        assert!(f[0].muted);
        assert_eq!(overall(&f), Severity::Info);
    }

    #[test]
    fn memory_needs_sustained_pressure() {
        let t = Thresholds::default();
        let s = snap(50);
        let short = evaluate(
            &RuleInput {
                snapshot: &s,
                mem_high_secs: 10,
                cpu_high_secs: 0,
                growing: &[],
                scan: None,
                failed_units: &[],
                lang: Lang::En,
            },
            &t,
        );
        assert!(short.is_empty());
        let long = evaluate(
            &RuleInput {
                snapshot: &s,
                mem_high_secs: 600,
                cpu_high_secs: 0,
                growing: &[],
                scan: None,
                failed_units: &[],
                lang: Lang::En,
            },
            &t,
        );
        assert_eq!(long.len(), 1);
    }
}
