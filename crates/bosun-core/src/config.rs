//! Yapılandırma: `~/.config/bosun/config.toml`. Ortam değişkenleri dosyayı geçersiz kılar.

use crate::i18n::Lang;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Thresholds {
    pub disk_warn: f32,
    pub disk_crit: f32,
    pub mem_warn: f32,
    pub mem_sustain_secs: u64,
    pub swap_warn: f32,
    pub temp_crit: f32,
    pub reclaimable_info_gb: f32,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            disk_warn: 85.0,
            disk_crit: 95.0,
            mem_warn: 90.0,
            mem_sustain_secs: 300,
            swap_warn: 50.0,
            temp_crit: 85.0,
            reclaimable_info_gb: 2.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Arayüz, bildirimler ve asistan yanıtlarının dili ("en" ya da "tr").
    pub language: Lang,
    /// Anthropic API anahtarı. Boşsa sohbet devre dışı kalır, diğer her şey çalışır.
    pub api_key: Option<String>,
    pub model: String,
    /// Büyük dosya yollarının API'ye gönderilmesine izin ver.
    pub share_file_paths: bool,
    pub sample_interval_secs: u64,
    pub scan_interval_secs: u64,
    pub notifications: bool,
    pub thresholds: Thresholds,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            language: Lang::En,
            api_key: None,
            model: "claude-sonnet-5-5".into(),
            share_file_paths: false,
            sample_interval_secs: 5,
            scan_interval_secs: 600,
            notifications: true,
            thresholds: Thresholds::default(),
        }
    }
}

impl Config {
    pub fn path() -> PathBuf {
        dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("bosun/config.toml")
    }

    pub fn load() -> Self {
        let mut cfg: Config = std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default();
        if let Ok(k) = std::env::var("ANTHROPIC_API_KEY") {
            if !k.trim().is_empty() {
                cfg.api_key = Some(k);
            }
        }
        if let Ok(m) = std::env::var("BOSUN_MODEL") {
            cfg.model = m;
        }
        cfg.sample_interval_secs = cfg.sample_interval_secs.max(1);
        cfg.scan_interval_secs = cfg.scan_interval_secs.max(60);
        cfg
    }

    /// Yalnızca dil ayarını dosyaya yazar; dosyadaki diğer her şey (API anahtarı dahil) korunur.
    pub fn save_language(lang: Lang) -> std::io::Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let path = Self::path();
        let mut table: toml::Table = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_default();
        table.insert("language".into(), toml::Value::String(lang.code().into()));
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = toml::to_string_pretty(&table).map_err(std::io::Error::other)?;
        std::fs::write(&path, text)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
    }

    pub fn has_api_key(&self) -> bool {
        self.api_key.as_deref().is_some_and(|k| !k.trim().is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_toml_uses_defaults() {
        let c: Config = toml::from_str("model = \"x\"\n[thresholds]\ndisk_warn = 70.0").unwrap();
        assert_eq!(c.model, "x");
        assert_eq!(c.thresholds.disk_warn, 70.0);
        assert_eq!(c.thresholds.disk_crit, 95.0);
        assert!(c.notifications);
        assert_eq!(c.language, Lang::En);
        let c: Config = toml::from_str("language = \"tr\"").unwrap();
        assert_eq!(c.language, Lang::Tr);
    }
}
