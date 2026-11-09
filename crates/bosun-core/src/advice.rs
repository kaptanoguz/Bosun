//! Ayar önerileri. Yalnızca okur; uygulamak her zaman kullanıcının elindedir.

use crate::categories::CategoryId;
use crate::i18n::Lang;
use crate::metrics::Snapshot;
use crate::scan::ScanResult;
use crate::util::bytes_in;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Advice {
    pub id: String,
    pub title: String,
    pub detail: String,
    pub current: String,
    pub suggested: String,
    /// Kullanıcının kendi terminalinde çalıştırabileceği komut.
    pub command: Option<String>,
    /// Bosun'un şifre sorarak uygulayabileceği yardımcı eylemi.
    pub admin_action: Option<String>,
}

fn read_trim(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
}

/// journald.conf ve drop-in dosyalarında `SystemMaxUse=` tanımlı mı?
fn journald_max_use(root: &Path) -> Option<String> {
    let mut files = vec![root.join("etc/systemd/journald.conf")];
    if let Ok(rd) = std::fs::read_dir(root.join("etc/systemd/journald.conf.d")) {
        let mut extra: Vec<_> = rd.filter_map(Result::ok).map(|e| e.path()).collect();
        extra.sort();
        files.extend(extra);
    }
    let mut value = None;
    for f in files {
        let Ok(text) = std::fs::read_to_string(&f) else {
            continue;
        };
        for line in text.lines() {
            let line = line.trim();
            if let Some(v) = line.strip_prefix("SystemMaxUse=") {
                value = Some(v.trim().to_string());
            }
        }
    }
    value.filter(|v| !v.is_empty())
}

/// /var/lib/snapd/snaps içinde her snap'in kaç revizyonu tutuluyor?
fn snap_revisions(dir: &Path) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return map;
    };
    for e in rd.filter_map(Result::ok) {
        let name = e.file_name().to_string_lossy().to_string();
        if let Some(stem) = name.strip_suffix(".snap") {
            if let Some((base, _rev)) = stem.rsplit_once('_') {
                *map.entry(base.to_string()).or_default() += 1;
            }
        }
    }
    map
}

pub fn collect(snapshot: &Snapshot, scan: Option<&ScanResult>, lang: Lang) -> Vec<Advice> {
    collect_at(
        Path::new("/"),
        snapshot,
        scan,
        crate::admin::snap_retain(),
        lang,
    )
}

/// `root` testlerde sahte bir dosya sistemi kökü vermek içindir.
pub fn collect_at(
    root: &Path,
    snapshot: &Snapshot,
    scan: Option<&ScanResult>,
    snap_retain: Option<u32>,
    l: Lang,
) -> Vec<Advice> {
    let mut out = Vec::new();

    // 1) journald boyut sınırı
    let journal_bytes = scan
        .and_then(|s| s.get(CategoryId::Journal))
        .map(|c| c.bytes)
        .unwrap_or(0);
    if journal_max_missing_and_large(root, journal_bytes) {
        out.push(Advice {
            id: "journald_max_use".into(),
            title: l.s("Cap the size of system logs", "Sistem günlüklerine boyut sınırı koyun"),
            detail: l.s(
                "By default journald can grow to 10% of the disk. A 500 MB cap is enough for most desktops.",
                "journald varsayılan olarak diskin %10'una kadar büyüyebilir. 500 MB sınırı çoğu masaüstü için yeterlidir.",
            ),
            current: l.s(
                format!("No limit ({} in use)", bytes_in(journal_bytes, l)),
                format!("Sınır yok ({} kullanımda)", bytes_in(journal_bytes, l)),
            ),
            suggested: "SystemMaxUse=500M".into(),
            command: Some(
                "sudo mkdir -p /etc/systemd/journald.conf.d && echo -e '[Journal]\\nSystemMaxUse=500M' | sudo tee /etc/systemd/journald.conf.d/60-bosun.conf && sudo systemctl restart systemd-journald".into(),
            ),
            admin_action: Some("journald-limit".into()),
        });
    }

    // 2) Snap eski revizyonları
    let revs = snap_revisions(&root.join("var/lib/snapd/snaps"));
    let heavy: Vec<_> = revs
        .iter()
        .filter(|(_, n)| **n >= 3)
        .map(|(k, _)| k.clone())
        .collect();
    if !heavy.is_empty() && snap_retain.is_none_or(|r| r > 2) {
        out.push(Advice {
            id: "snap_retain".into(),
            title: l.s(
                "Keep fewer old Snap revisions",
                "Snap'in eski sürüm sayısını azaltın",
            ),
            detail: l.s(
                format!(
                    "These snaps keep 3 or more revisions on disk: {}.",
                    heavy.join(", ")
                ),
                format!(
                    "Şu paketlerin 3 veya daha fazla sürümü diskte duruyor: {}.",
                    heavy.join(", ")
                ),
            ),
            current: l.s(
                "refresh.retain = 3 (default)",
                "refresh.retain = 3 (varsayılan)",
            ),
            suggested: "refresh.retain = 2".into(),
            command: Some("sudo snap set system refresh.retain=2".into()),
            admin_action: Some("snap-retain".into()),
        });
    }

    // 3) Swappiness: bol RAM'li sistemde swap kullanılıyorsa
    if let Some(sw) =
        read_trim(&root.join("proc/sys/vm/swappiness")).and_then(|s| s.parse::<u32>().ok())
    {
        let ram_gb = snapshot.mem_total as f64 / 1024f64.powi(3);
        if sw >= 60 && ram_gb >= 15.0 && snapshot.swap_used > 512 * 1024 * 1024 {
            out.push(Advice {
                id: "swappiness".into(),
                title: l.s("Use swap less eagerly", "Swap kullanımını azaltın"),
                detail: l.s(
                    format!(
                        "With {ram_gb:.0} GB of RAM the system is still using {} of swap. A lower swappiness can make the desktop feel smoother.",
                        bytes_in(snapshot.swap_used, l)
                    ),
                    format!(
                        "{ram_gb:.0} GB RAM'iniz varken sistem {} swap kullanıyor. Daha düşük bir swappiness değeri masaüstünü daha akıcı yapabilir.",
                        bytes_in(snapshot.swap_used, l)
                    ),
                ),
                current: format!("vm.swappiness = {sw}"),
                suggested: "vm.swappiness = 20".into(),
                command: Some(
                    "echo 'vm.swappiness=20' | sudo tee /etc/sysctl.d/60-bosun-swappiness.conf && sudo sysctl --system".into(),
                ),
                admin_action: Some("swappiness".into()),
            });
        }
    }

    out
}

fn journal_max_missing_and_large(root: &Path, journal_bytes: u64) -> bool {
    journal_bytes >= 1024 * 1024 * 1024 && journald_max_use(root).is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn counts_snap_revisions() {
        let d = tempfile::tempdir().unwrap();
        let snaps = d.path().join("var/lib/snapd/snaps");
        fs::create_dir_all(&snaps).unwrap();
        for f in [
            "core22_1.snap",
            "core22_2.snap",
            "core22_3.snap",
            "firefox_9.snap",
        ] {
            fs::write(snaps.join(f), b"").unwrap();
        }
        let r = snap_revisions(&snaps);
        assert_eq!(r["core22"], 3);
        assert_eq!(r["firefox"], 1);
    }

    #[test]
    fn detects_journald_limit_in_dropin() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("etc/systemd/journald.conf.d")).unwrap();
        fs::write(
            d.path().join("etc/systemd/journald.conf"),
            "[Journal]\n#SystemMaxUse=\n",
        )
        .unwrap();
        assert!(journald_max_use(d.path()).is_none());
        fs::write(
            d.path().join("etc/systemd/journald.conf.d/x.conf"),
            "[Journal]\nSystemMaxUse=300M\n",
        )
        .unwrap();
        assert_eq!(journald_max_use(d.path()).as_deref(), Some("300M"));
    }
}
