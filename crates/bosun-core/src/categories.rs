//! Temizlik kategorileri. Model ya da arayüz yalnızca bu kimliklerden seçebilir;
//! hiçbir yerden serbest bir yol kabul edilmez.

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
    LargeFiles,
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
    /// Yönetici yetkisi gerekir; Faz 1'de yalnızca öneri.
    Privileged,
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
    CategoryId::LargeFiles,
];

/// `~/.cache` altında tarayıcılara ait alt klasörler (BrowserCache kategorisine aittir).
pub const BROWSER_CACHE_DIRS: &[&str] =
    &["mozilla", "librewolf", "google-chrome", "chromium", "BraveSoftware", "microsoft-edge", "vivaldi", "opera"];

/// `~/.cache` altında UserCache taramasından hariç tutulanlar.
pub const USER_CACHE_EXCLUDE: &[&str] = &["thumbnails", "bosun", "fontconfig", "pip", "uv"];

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
            Self::LargeFiles => "large_files",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Thumbnails => "Küçük resim önbelleği",
            Self::BrowserCache => "Tarayıcı önbelleği",
            Self::UserCache => "Eski uygulama önbellekleri",
            Self::NpmCache => "npm önbelleği",
            Self::CargoCache => "Cargo indirme önbelleği",
            Self::OldLogs => "Eski kullanıcı günlükleri",
            Self::Trash => "Çöp kutusu",
            Self::Journal => "Sistem günlükleri (journald)",
            Self::AptCache => "APT paket önbelleği",
            Self::LargeFiles => "İndirilenlerdeki büyük dosyalar",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Thumbnails => "Dosya yöneticisinin ürettiği önizlemeler. Gerektiğinde yeniden oluşturulur.",
            Self::BrowserCache => "Web sayfalarının geçici kopyaları. Oturumlar ve şifreler etkilenmez; tarayıcı açıksa kapatmak daha iyi olur.",
            Self::UserCache => "~/.cache içinde 30 günden uzun süredir değişmemiş dosyalar. Uygulamalar gerekirse yeniden indirir.",
            Self::NpmCache => "npm'in indirdiği paket arşivleri. Bir sonraki kurulumda yeniden indirilir.",
            Self::CargoCache => "Rust crate arşivleri (.crate). Bir sonraki derlemede yeniden indirilir.",
            Self::OldLogs => "~/.local/state altında 30 günden eski .log dosyaları.",
            Self::Trash => "Çöp kutusundaki dosyalar. Bu işlem KALICIDIR, geri alınamaz.",
            Self::Journal => "systemd günlükleri. 200 MB'a indirilmesi önerilir; yönetici yetkisi gerekir.",
            Self::AptCache => "Kurulmuş paketlerin indirilmiş .deb dosyaları. `sudo apt-get clean` ile temizlenir.",
            Self::LargeFiles => "500 MB'tan büyük indirilmiş dosyalar. Otomatik silinmez; tek tek seçmelisiniz.",
        }
    }

    pub fn safety(self) -> Safety {
        match self {
            Self::Thumbnails | Self::BrowserCache | Self::NpmCache | Self::CargoCache | Self::Journal | Self::AptCache => {
                Safety::Safe
            }
            Self::UserCache | Self::OldLogs | Self::Trash => Safety::Caution,
            Self::LargeFiles => Safety::ReviewOnly,
        }
    }

    pub fn method(self) -> Method {
        match self {
            Self::Trash => Method::Permanent,
            Self::Journal | Self::AptCache => Method::Privileged,
            _ => Method::Trash,
        }
    }

    /// Bu kategorinin öğelerinin bulunabileceği kökler. Doğrulama bu köklere göre yapılır.
    pub fn roots(self, home: &Path) -> Vec<PathBuf> {
        let cache = home.join(".cache");
        match self {
            Self::Thumbnails => vec![cache.join("thumbnails")],
            Self::BrowserCache => {
                let mut v: Vec<PathBuf> = BROWSER_CACHE_DIRS.iter().map(|d| cache.join(d)).collect();
                // Flatpak tarayıcıları
                for app in ["org.mozilla.firefox", "com.google.Chrome", "org.chromium.Chromium", "io.gitlab.librewolf-community", "com.brave.Browser"] {
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
            Self::LargeFiles => ["Downloads", "İndirilenler", "Download"].iter().map(|d| home.join(d)).collect(),
        }
    }

    /// Asistanın ya da arayüzün tüm kategoriyi tek seferde plana eklemesine izin var mı?
    pub fn bulk_selectable(self) -> bool {
        self.safety() != Safety::ReviewOnly && self.method() != Method::Privileged
    }

    /// Yönetici yetkisi gerektiren kategoriler için kullanıcıya önerilecek komut.
    pub fn manual_command(self) -> Option<&'static str> {
        match self {
            Self::Journal => Some("sudo journalctl --vacuum-size=200M"),
            Self::AptCache => Some("sudo apt-get clean"),
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
    }
}
