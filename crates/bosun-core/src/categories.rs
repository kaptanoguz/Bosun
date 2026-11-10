//! Temizlik kategorileri. Model ya da arayüz yalnızca bu kimliklerden seçebilir;
//! hiçbir yerden serbest bir yol kabul edilmez.

use crate::i18n::Lang;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum CategoryId {
    Thumbnails,
    BrowserCache,
    UserCache,
    NpmCache,
    CargoCache,
    OldLogs,
    Trash,
    Journal,
    AptCache,
    FlatpakCache,
    DevCaches,
    SteamCache,
    ContainerCache,
    FlatpakUnused,
    BrokenLinks,
    OrphanLaunchers,
    LeftoverConfigs,
    OrphanPackages,
    ResidualPackages,
    LargeFiles,
    /// Disk kullanımı ekranından tek tek seçilen dosya.
    ChosenFile,
}

/// Kullanıcıya gösterilen güvenlik seviyesi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Safety {
    /// Uygulamalar gerekirse yeniden oluşturur.
    Safe,
    /// Genelde güvenli ama kullanıcı bir göz atmalı.
    Caution,
    /// Otomatik önerilmez; kullanıcı dosyaları tek tek seçer.
    ReviewOnly,
}

/// Silme yöntemi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    /// XDG çöp kutusuna taşı (geri alınabilir).
    Trash,
    /// Kalıcı sil (yalnızca çöp kutusunun kendisi için).
    Permanent,
    /// Yönetici yetkisi gerekir; `bosun-helper` ile şifre sorularak yapılır.
    Privileged,
    /// Yalnızca kırık sembolik bağlantının kendisini sil (hedef zaten yok).
    Unlink,
    /// Sabit, önceden tanımlı bir komut (ör. `docker image prune`); öğe kimliği komutu seçer.
    Command,
}

pub const ALL: &[CategoryId] = &[
    CategoryId::Thumbnails,
    CategoryId::BrowserCache,
    CategoryId::UserCache,
    CategoryId::NpmCache,
    CategoryId::CargoCache,
    CategoryId::OldLogs,
    CategoryId::Trash,
    CategoryId::Journal,
    CategoryId::AptCache,
    CategoryId::FlatpakCache,
    CategoryId::DevCaches,
    CategoryId::SteamCache,
    CategoryId::ContainerCache,
    CategoryId::FlatpakUnused,
    CategoryId::BrokenLinks,
    CategoryId::OrphanLaunchers,
    CategoryId::LeftoverConfigs,
    CategoryId::OrphanPackages,
    CategoryId::ResidualPackages,
    CategoryId::LargeFiles,
];

/// `~/.cache` altında tarayıcılara ait alt klasörler (BrowserCache kategorisine aittir).
pub const BROWSER_CACHE_DIRS: &[&str] = &[
    "mozilla",
    "librewolf",
    "google-chrome",
    "chromium",
    "BraveSoftware",
    "microsoft-edge",
    "vivaldi",
    "opera",
];

/// BrowserCache'e dahil edilen Flatpak tarayıcıları.
pub const FLATPAK_BROWSERS: &[&str] = &[
    "org.mozilla.firefox",
    "com.google.Chrome",
    "org.chromium.Chromium",
    "io.gitlab.librewolf-community",
    "com.brave.Browser",
];

/// `~/.cache` altında UserCache taramasından hariç tutulanlar.
pub const USER_CACHE_EXCLUDE: &[&str] = &[
    "thumbnails",
    "bosun",
    "fontconfig",
    "pip",
    "uv",
    "pypoetry",
    "go-build",
    "yarn",
    "huggingface",
];

/// Geliştirici araçlarının indirme önbellekleri (ev dizinine göre).
pub const DEV_CACHES: &[&str] = &[
    ".cache/pip",
    ".cache/uv",
    ".cache/pypoetry",
    ".cache/go-build",
    ".cache/yarn",
    ".conda/pkgs",
    ".gradle/caches",
];

impl CategoryId {
    pub fn parse(s: &str) -> Option<Self> {
        serde_json::from_value(serde_json::Value::String(s.to_string())).ok()
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Thumbnails => "thumbnails",
            Self::BrowserCache => "browser_cache",
            Self::UserCache => "user_cache",
            Self::NpmCache => "npm_cache",
            Self::CargoCache => "cargo_cache",
            Self::OldLogs => "old_logs",
            Self::Trash => "trash",
            Self::Journal => "journal",
            Self::AptCache => "apt_cache",
            Self::FlatpakCache => "flatpak_cache",
            Self::DevCaches => "dev_caches",
            Self::SteamCache => "steam_cache",
            Self::ContainerCache => "container_cache",
            Self::FlatpakUnused => "flatpak_unused",
            Self::BrokenLinks => "broken_links",
            Self::OrphanLaunchers => "orphan_launchers",
            Self::LeftoverConfigs => "leftover_configs",
            Self::OrphanPackages => "orphan_packages",
            Self::ResidualPackages => "residual_packages",
            Self::LargeFiles => "large_files",
            Self::ChosenFile => "chosen_file",
        }
    }

    pub fn title(self, lang: Lang) -> &'static str {
        let (en, tr) = match self {
            Self::Thumbnails => ("Thumbnail cache", "Küçük resim önbelleği"),
            Self::BrowserCache => ("Browser cache", "Tarayıcı önbelleği"),
            Self::UserCache => ("Stale app caches", "Eski uygulama önbellekleri"),
            Self::NpmCache => ("npm cache", "npm önbelleği"),
            Self::CargoCache => ("Cargo download cache", "Cargo indirme önbelleği"),
            Self::OldLogs => ("Old user logs", "Eski kullanıcı günlükleri"),
            Self::Trash => ("Trash", "Çöp kutusu"),
            Self::Journal => ("System logs (journald)", "Sistem günlükleri (journald)"),
            Self::AptCache => ("APT package cache", "APT paket önbelleği"),
            Self::FlatpakCache => ("Flatpak app caches", "Flatpak uygulama önbellekleri"),
            Self::DevCaches => ("Developer caches", "Geliştirici önbellekleri"),
            Self::SteamCache => ("Steam shader caches", "Steam gölgelendirici önbellekleri"),
            Self::ContainerCache => ("Docker / Podman leftovers", "Docker / Podman artıkları"),
            Self::FlatpakUnused => (
                "Unused Flatpak runtimes",
                "Kullanılmayan Flatpak çalışma ortamları",
            ),
            Self::BrokenLinks => ("Broken links", "Kırık bağlantılar"),
            Self::OrphanLaunchers => ("Broken shortcuts", "Kırık kısayollar"),
            Self::LeftoverConfigs => (
                "Possible leftovers from old programs",
                "Eski programlardan kalmış olabilecek klasörler",
            ),
            Self::OrphanPackages => (
                "Packages nothing needs any more",
                "Artık hiçbir şeyin ihtiyaç duymadığı paketler",
            ),
            Self::ResidualPackages => (
                "Settings of removed packages",
                "Kaldırılmış paketlerin ayar dosyaları",
            ),
            Self::LargeFiles => (
                "Large files in Downloads",
                "İndirilenlerdeki büyük dosyalar",
            ),
            Self::ChosenFile => (
                "File chosen in Disk usage",
                "Disk kullanımından seçilen dosya",
            ),
        };
        lang.pick(en, tr)
    }

    pub fn description(self, lang: Lang) -> &'static str {
        let (en, tr) = match self {
            Self::Thumbnails => (
                "Previews generated by the file manager. Rebuilt when needed.",
                "Dosya yöneticisinin ürettiği önizlemeler. Gerektiğinde yeniden oluşturulur.",
            ),
            Self::BrowserCache => (
                "Temporary copies of web pages. Sessions and passwords are not affected; closing the browser first is best.",
                "Web sayfalarının geçici kopyaları. Oturumlar ve şifreler etkilenmez; tarayıcı açıksa kapatmak daha iyi olur.",
            ),
            Self::UserCache => (
                "Files in ~/.cache untouched for more than 30 days. Apps download them again if needed.",
                "~/.cache içinde 30 günden uzun süredir değişmemiş dosyalar. Uygulamalar gerekirse yeniden indirir.",
            ),
            Self::NpmCache => (
                "Package archives downloaded by npm. Fetched again on the next install.",
                "npm'in indirdiği paket arşivleri. Bir sonraki kurulumda yeniden indirilir.",
            ),
            Self::CargoCache => (
                "Rust crate archives (.crate). Fetched again on the next build.",
                "Rust crate arşivleri (.crate). Bir sonraki derlemede yeniden indirilir.",
            ),
            Self::OldLogs => (
                ".log files older than 30 days under ~/.local/state.",
                "~/.local/state altında 30 günden eski .log dosyaları.",
            ),
            Self::Trash => (
                "Files in the trash. Emptying it is PERMANENT and cannot be undone.",
                "Çöp kutusundaki dosyalar. Bu işlem KALICIDIR, geri alınamaz.",
            ),
            Self::Journal => (
                "systemd logs. Shrinking to 500 MB is recommended; requires administrator rights.",
                "systemd günlükleri. 500 MB'a indirilmesi önerilir; yönetici yetkisi gerekir.",
            ),
            Self::AptCache => (
                "Downloaded .deb files of installed packages. Cleared with `sudo apt-get clean`.",
                "Kurulmuş paketlerin indirilmiş .deb dosyaları. `sudo apt-get clean` ile temizlenir.",
            ),
            Self::FlatpakCache => (
                "Cache folders of Flatpak apps (~/.var/app/*/cache). Apps rebuild them when needed.",
                "Flatpak uygulamalarının önbellek klasörleri (~/.var/app/*/cache). Uygulamalar gerektiğinde yeniden oluşturur.",
            ),
            Self::DevCaches => (
                "Download caches of pip, uv, Poetry, conda, Go, Yarn and Gradle. They are fetched again when needed.",
                "pip, uv, Poetry, conda, Go, Yarn ve Gradle indirme önbellekleri. Gerektiğinde yeniden indirilir.",
            ),
            Self::SteamCache => (
                "Compiled shaders per game. Steam rebuilds them; the first launch of a game may stutter briefly.",
                "Oyun başına derlenmiş gölgelendiriciler. Steam yeniden oluşturur; oyunun ilk açılışı kısa süre takılabilir.",
            ),
            Self::ContainerCache => (
                "Untagged (dangling) images and the build cache. Tagged images, containers and volumes are never touched.",
                "Etiketsiz (sarkan) imajlar ve derleme önbelleği. Etiketli imajlara, kapsayıcılara ve birimlere asla dokunulmaz.",
            ),
            Self::FlatpakUnused => (
                "Runtimes left behind by removed Flatpak apps (Bosun's estimate; Flatpak makes the final call and never removes pinned runtimes). Requires administrator rights.",
                "Kaldırılmış Flatpak uygulamalarından kalan çalışma ortamları (Bosun'un tahmini; son kararı Flatpak verir ve sabitlenmiş ortamları asla silmez). Yönetici yetkisi gerekir.",
            ),
            Self::BrokenLinks => (
                "Symbolic links in your home folder whose target no longer exists. Only the link is removed.",
                "Ev klasörünüzde hedefi artık olmayan sembolik bağlantılar. Yalnızca bağlantının kendisi silinir.",
            ),
            Self::OrphanLaunchers => (
                "Menu and autostart entries whose program is no longer installed.",
                "Çalıştırdığı program artık kurulu olmayan menü ve otomatik başlatma kısayolları.",
            ),
            Self::LeftoverConfigs => (
                "Folders in ~/.config and ~/.local/share that match no installed program and haven't changed in 6 months. This is a guess: check each one before you tick it.",
                "~/.config ve ~/.local/share içinde kurulu hiçbir programla eşleşmeyen ve 6 aydır değişmemiş klasörler. Bu bir tahmindir: işaretlemeden önce her birine bakın.",
            ),
            Self::OrphanPackages => (
                "Dependencies that were installed for programs you've since removed. Requires administrator rights.",
                "Sonradan kaldırdığınız programlar için kurulmuş bağımlılıklar. Yönetici yetkisi gerekir.",
            ),
            Self::ResidualPackages => (
                "System-wide configuration files left behind by removed packages. Requires administrator rights.",
                "Kaldırılmış paketlerden geriye kalan sistem ayar dosyaları. Yönetici yetkisi gerekir.",
            ),
            Self::ChosenFile => (
                "A file you picked in Disk usage. It goes to the trash after you approve the plan.",
                "Disk kullanımında sizin seçtiğiniz dosya. Planı onayladıktan sonra çöp kutusuna taşınır.",
            ),
            Self::LargeFiles => (
                "Downloaded files larger than 500 MB. Never removed automatically; pick them one by one.",
                "500 MB'tan büyük indirilmiş dosyalar. Otomatik silinmez; tek tek seçmelisiniz.",
            ),
        };
        lang.pick(en, tr)
    }

    pub fn safety(self) -> Safety {
        match self {
            Self::Thumbnails
            | Self::BrowserCache
            | Self::NpmCache
            | Self::CargoCache
            | Self::Journal
            | Self::AptCache
            | Self::FlatpakCache
            | Self::DevCaches
            | Self::SteamCache
            | Self::FlatpakUnused
            | Self::BrokenLinks
            | Self::ResidualPackages => Safety::Safe,
            Self::UserCache
            | Self::OldLogs
            | Self::Trash
            | Self::OrphanLaunchers
            | Self::OrphanPackages
            | Self::ContainerCache => Safety::Caution,
            Self::LargeFiles | Self::LeftoverConfigs | Self::ChosenFile => Safety::ReviewOnly,
        }
    }

    pub fn method(self) -> Method {
        match self {
            Self::Trash => Method::Permanent,
            Self::Journal
            | Self::AptCache
            | Self::FlatpakUnused
            | Self::OrphanPackages
            | Self::ResidualPackages => Method::Privileged,
            Self::BrokenLinks => Method::Unlink,
            Self::ContainerCache => Method::Command,
            _ => Method::Trash,
        }
    }

    /// Bu kategorinin öğelerinin bulunabileceği kökler. Doğrulama bu köklere göre yapılır.
    pub fn roots(self, home: &Path) -> Vec<PathBuf> {
        let cache = home.join(".cache");
        match self {
            Self::Thumbnails => vec![cache.join("thumbnails")],
            Self::BrowserCache => {
                let mut v: Vec<PathBuf> =
                    BROWSER_CACHE_DIRS.iter().map(|d| cache.join(d)).collect();
                // Flatpak tarayıcıları
                for app in FLATPAK_BROWSERS {
                    v.push(home.join(".var/app").join(app).join("cache"));
                }
                v
            }
            Self::UserCache => vec![cache],
            Self::NpmCache => vec![home.join(".npm/_cacache")],
            Self::CargoCache => vec![home.join(".cargo/registry/cache")],
            Self::OldLogs => vec![home.join(".local/state")],
            Self::Trash => vec![home.join(".local/share/Trash")],
            Self::Journal => vec![PathBuf::from("/var/log/journal")],
            Self::AptCache => vec![PathBuf::from("/var/cache/apt/archives")],
            Self::FlatpakCache => {
                // Tarayıcıların Flatpak önbellekleri BrowserCache'e aittir.
                let mut v: Vec<PathBuf> = std::fs::read_dir(home.join(".var/app"))
                    .map(|rd| rd.filter_map(Result::ok).map(|e| e.path()).collect())
                    .unwrap_or_default();
                v.retain(|p| {
                    let name = p
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    !FLATPAK_BROWSERS.contains(&name.as_str())
                });
                v.sort();
                v.into_iter().map(|p| p.join("cache")).collect()
            }
            Self::DevCaches => DEV_CACHES.iter().map(|d| home.join(d)).collect(),
            Self::SteamCache => {
                // ~/.steam/steam çoğunlukla ~/.local/share/Steam'e bağlantıdır; aynı klasörü iki kez sayma.
                let mut seen = std::collections::HashSet::new();
                [
                    ".local/share/Steam",
                    ".steam/steam",
                    ".var/app/com.valvesoftware.Steam/.local/share/Steam",
                ]
                .iter()
                .map(|d| home.join(d).join("steamapps/shadercache"))
                .filter(|p| p.canonicalize().map(|c| seen.insert(c)).unwrap_or(false))
                .collect()
            }
            Self::ContainerCache => vec![],
            Self::FlatpakUnused => vec![PathBuf::from("/var/lib/flatpak/runtime")],
            Self::BrokenLinks => vec![home.to_path_buf()],
            Self::OrphanLaunchers => vec![
                home.join(".local/share/applications"),
                home.join(".config/autostart"),
            ],
            Self::LeftoverConfigs => vec![home.join(".config"), home.join(".local/share")],
            Self::OrphanPackages | Self::ResidualPackages => vec![],
            Self::LargeFiles => ["Downloads", "İndirilenler", "Download"]
                .iter()
                .map(|d| home.join(d))
                .collect(),
            // Bu kategorinin öğeleri yalnızca Disk ekranından gelir; yol, ev klasörüne göre denetlenir.
            Self::ChosenFile => vec![home.to_path_buf()],
        }
    }

    /// Asistanın ya da arayüzün tüm kategoriyi tek seferde plana eklemesine izin var mı?
    pub fn bulk_selectable(self) -> bool {
        self.safety() != Safety::ReviewOnly && self.method() != Method::Privileged
    }

    /// Bu kategoriyi temizleyen yardımcı eylemi (yalnızca yetkili kategoriler).
    pub fn admin_action(self) -> Option<&'static str> {
        match self {
            Self::Journal => Some("journal-vacuum"),
            Self::AptCache => Some("apt-clean"),
            Self::FlatpakUnused => Some("flatpak-unused"),
            Self::OrphanPackages => Some("apt-autoremove"),
            Self::ResidualPackages => Some("purge-residual"),
            _ => None,
        }
    }

    /// Yönetici yetkisi gerektiren kategoriler için kullanıcıya önerilecek komut.
    pub fn manual_command(self) -> Option<&'static str> {
        match self {
            Self::Journal => Some("sudo journalctl --vacuum-size=500M"),
            Self::AptCache => Some("sudo apt-get clean"),
            Self::FlatpakUnused => Some("flatpak uninstall --unused"),
            Self::OrphanPackages => Some("sudo apt autoremove --purge"),
            Self::ResidualPackages => Some("sudo apt purge '~c'"),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip() {
        for c in ALL {
            assert_eq!(CategoryId::parse(c.as_str()), Some(*c));
        }
        assert_eq!(CategoryId::parse("rm -rf /"), None);
    }

    #[test]
    fn privileged_and_review_only_are_not_bulk() {
        assert!(!CategoryId::Journal.bulk_selectable());
        assert!(!CategoryId::AptCache.bulk_selectable());
        assert!(!CategoryId::LargeFiles.bulk_selectable());
        assert!(CategoryId::Thumbnails.bulk_selectable());
        assert!(!CategoryId::FlatpakUnused.bulk_selectable());
        assert!(CategoryId::FlatpakCache.bulk_selectable());
        for c in ALL {
            if c.method() == Method::Privileged {
                assert!(
                    c.admin_action().is_some_and(crate::admin::is_known),
                    "{c:?}"
                );
            }
        }
    }
}
