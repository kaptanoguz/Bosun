//! Yönetici yetkisi gerektiren eylemler. Bunları `bosun-helper` (root, pkexec ile) yapar;
//! burada yalnızca kimlikleri, açıklamaları ve Bosun'un uyguladığı ayarların tespiti vardır.

use crate::i18n::Lang;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const HELPER_PATH: &str = "/usr/lib/bosun/bosun-helper";
pub const JOURNALD_DROPIN: &str = "/etc/systemd/journald.conf.d/60-bosun.conf";
pub const SYSCTL_DROPIN: &str = "/etc/sysctl.d/60-bosun-swappiness.conf";

/// Yardımcının bildiği tüm eylemler (testte yardımcıyla karşılaştırılır).
pub const ALL_IDS: &[&str] = &[
    "journal-vacuum",
    "apt-clean",
    "journald-limit",
    "journald-limit-undo",
    "snap-retain",
    "snap-retain-undo",
    "swappiness",
    "swappiness-undo",
    "flatpak-unused",
    "apt-autoremove",
    "purge-residual",
];

pub fn is_known(id: &str) -> bool {
    ALL_IDS.contains(&id)
}

/// Kullanıcıya onay penceresinde gösterilecek: ne yapılacak, tam olarak hangi komutla.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionInfo {
    pub id: String,
    pub title: String,
    pub effect: String,
    /// Yardımcının root olarak çalıştıracağı komutların okunur hali.
    pub commands: Vec<String>,
    pub undo: Option<String>,
}

pub fn info(id: &str, l: Lang) -> Option<ActionInfo> {
    let (title, effect, commands, undo): (String, String, Vec<&str>, Option<&str>) = match id {
        "journal-vacuum" => (
            l.s("Shrink system logs", "Sistem günlüklerini küçült"),
            l.s("Old journal entries are deleted until the logs fit in 200 MB.", "Günlükler 200 MB'a sığana kadar eski kayıtlar silinir."),
            vec!["journalctl --vacuum-size=200M"],
            None,
        ),
        "apt-clean" => (
            l.s("Clear the APT package cache", "APT paket önbelleğini temizle"),
            l.s("Downloaded .deb files are removed. Installed software is not affected.", "İndirilmiş .deb dosyaları silinir. Kurulu programlar etkilenmez."),
            vec!["apt-get clean"],
            None,
        ),
        "journald-limit" => (
            l.s("Cap system logs at 500 MB", "Sistem günlüklerini 500 MB ile sınırla"),
            l.s("Adds a small journald setting file and restarts journald. You can undo it from Settings.", "Küçük bir journald ayar dosyası ekler ve journald'ı yeniden başlatır. Ayarlar'dan geri alabilirsiniz."),
            vec!["write /etc/systemd/journald.conf.d/60-bosun.conf (SystemMaxUse=500M)", "systemctl restart systemd-journald"],
            Some("journald-limit-undo"),
        ),
        "journald-limit-undo" => (
            l.s("Remove the system log cap", "Sistem günlüğü sınırını kaldır"),
            l.s("Deletes Bosun's journald setting file and restarts journald.", "Bosun'un journald ayar dosyasını siler ve journald'ı yeniden başlatır."),
            vec!["rm /etc/systemd/journald.conf.d/60-bosun.conf", "systemctl restart systemd-journald"],
            None,
        ),
        "snap-retain" => (
            l.s("Keep 2 Snap revisions", "Snap'te 2 sürüm tut"),
            l.s("Snap keeps one fewer old revision of each package. Extra revisions go away on the next refresh.", "Snap her paketin bir eski sürümünü daha az tutar. Fazla sürümler bir sonraki güncellemede silinir."),
            vec!["snap set system refresh.retain=2"],
            Some("snap-retain-undo"),
        ),
        "snap-retain-undo" => (
            l.s("Restore Snap's default (3 revisions)", "Snap varsayılanına dön (3 sürüm)"),
            l.s("Snap goes back to keeping 3 revisions.", "Snap yeniden 3 sürüm tutar."),
            vec!["snap set system refresh.retain=3"],
            None,
        ),
        "swappiness" => (
            l.s("Set swappiness to 20", "swappiness değerini 20 yap"),
            l.s("The kernel uses swap less eagerly. Takes effect immediately and survives reboots.", "Çekirdek swap'ı daha az kullanır. Hemen etkili olur, yeniden başlatmada da korunur."),
            vec!["write /etc/sysctl.d/60-bosun-swappiness.conf (vm.swappiness=20)", "sysctl -w vm.swappiness=20"],
            Some("swappiness-undo"),
        ),
        "swappiness-undo" => (
            l.s("Restore default swappiness (60)", "Varsayılan swappiness'a dön (60)"),
            l.s("Deletes Bosun's sysctl file and sets swappiness back to 60.", "Bosun'un sysctl dosyasını siler ve swappiness'ı 60'a döndürür."),
            vec!["rm /etc/sysctl.d/60-bosun-swappiness.conf", "sysctl -w vm.swappiness=60"],
            None,
        ),
        "flatpak-unused" => (
            l.s("Remove unused Flatpak runtimes", "Kullanılmayan Flatpak çalışma ortamlarını kaldır"),
            l.s(
                "Runtimes that no installed Flatpak app needs any more are uninstalled. Your apps keep working.",
                "Kurulu hiçbir Flatpak uygulamasının artık ihtiyaç duymadığı çalışma ortamları kaldırılır. Uygulamalarınız çalışmaya devam eder.",
            ),
            vec!["flatpak uninstall --system --unused --assumeyes"],
            None,
        ),
        "apt-autoremove" => (
            l.s("Remove packages nothing needs", "Gereksiz paketleri kaldır"),
            l.s(
                "Removes dependencies that were installed automatically for programs you've since uninstalled, along with their settings.",
                "Sonradan kaldırdığınız programlar için otomatik kurulmuş bağımlılıkları ayarlarıyla birlikte kaldırır.",
            ),
            vec!["apt-get autoremove --purge -y"],
            None,
        ),
        "purge-residual" => (
            l.s("Delete settings of removed packages", "Kaldırılmış paketlerin ayarlarını sil"),
            l.s(
                "Deletes the system-wide configuration files that removed packages left behind. Nothing that is installed is touched.",
                "Kaldırılmış paketlerin geride bıraktığı sistem ayar dosyalarını siler. Kurulu hiçbir şeye dokunulmaz.",
            ),
            vec!["dpkg --purge <packages in 'rc' state>"],
            None,
        ),
        _ => return None,
    };
    Some(ActionInfo {
        id: id.into(),
        title,
        effect,
        commands: commands.into_iter().map(str::to_string).collect(),
        undo: undo.map(str::to_string),
    })
}

/// Bosun'un daha önce uyguladığı ve geri alınabilecek bir ayar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Applied {
    pub id: String,
    pub title: String,
    pub undo_action: String,
}

/// `snap get system refresh.retain` (ayarlanmamışsa None).
pub fn snap_retain() -> Option<u32> {
    let out = std::process::Command::new("snap").args(["get", "system", "refresh.retain"]).output().ok()?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

pub fn applied(l: Lang) -> Vec<Applied> {
    applied_at(Path::new("/"), snap_retain(), l)
}

pub fn applied_at(root: &Path, snap_retain: Option<u32>, l: Lang) -> Vec<Applied> {
    let mut out = Vec::new();
    let under = |p: &str| root.join(p.trim_start_matches('/'));
    if under(JOURNALD_DROPIN).exists() {
        out.push(Applied {
            id: "journald-limit".into(),
            title: l.s("System logs capped at 500 MB", "Sistem günlükleri 500 MB ile sınırlı"),
            undo_action: "journald-limit-undo".into(),
        });
    }
    if under(SYSCTL_DROPIN).exists() {
        out.push(Applied {
            id: "swappiness".into(),
            title: l.s("Swappiness set to 20", "swappiness 20 olarak ayarlı"),
            undo_action: "swappiness-undo".into(),
        });
    }
    if snap_retain == Some(2) {
        out.push(Applied {
            id: "snap-retain".into(),
            title: l.s("Snap keeps 2 revisions", "Snap 2 sürüm tutuyor"),
            undo_action: "snap-retain-undo".into(),
        });
    }
    out
}

/// Yardımcı kurulu ve root'a ait mi? (Başkasının yazabildiği bir yardımcıyı asla çalıştırmayız.)
pub fn helper_installed() -> bool {
    helper_ok(Path::new(HELPER_PATH))
}

pub fn helper_ok(path: &Path) -> bool {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let Ok(m) = std::fs::symlink_metadata(path) else { return false };
    m.is_file() && m.uid() == 0 && m.permissions().mode() & 0o022 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_has_info_and_undo_is_known() {
        for id in ALL_IDS {
            let i = info(id, Lang::En).unwrap();
            assert!(!i.commands.is_empty());
            if let Some(u) = i.undo {
                assert!(is_known(&u));
            }
            assert!(info(id, Lang::Tr).is_some());
        }
        assert!(info("rm -rf /", Lang::En).is_none());
    }

    #[test]
    fn detects_applied_settings() {
        let d = tempfile::tempdir().unwrap();
        assert!(applied_at(d.path(), None, Lang::En).is_empty());
        std::fs::create_dir_all(d.path().join("etc/sysctl.d")).unwrap();
        std::fs::write(d.path().join("etc/sysctl.d/60-bosun-swappiness.conf"), "x").unwrap();
        let a = applied_at(d.path(), Some(2), Lang::En);
        assert_eq!(a.len(), 2);
        assert_eq!(a[0].undo_action, "swappiness-undo");
    }

    #[test]
    fn helper_must_not_be_group_or_world_writable() {
        use std::os::unix::fs::PermissionsExt;
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("h");
        std::fs::write(&p, "x").unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o757)).unwrap();
        assert!(!helper_ok(&p));
    }
}
