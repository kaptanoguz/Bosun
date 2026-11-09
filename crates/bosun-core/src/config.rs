//! Yapılandırma: `~/.config/bosun/config.toml`. Ortam değişkenleri dosyayı geçersiz kılar.

use crate::i18n::Lang;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Sohbet için kullanılan yapay zekâ sağlayıcısı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    #[default]
    Anthropic,
    /// xAI Grok (OpenAI uyumlu API)
    Xai,
    /// Google Gemini (OpenAI uyumlu uç nokta)
    Google,
    Openai,
    /// OpenAI uyumlu herhangi bir sunucu: Ollama, LM Studio, OpenRouter…
    Custom,
}

impl Provider {
    pub const ALL: [Provider; 5] = [
        Provider::Anthropic,
        Provider::Xai,
        Provider::Google,
        Provider::Openai,
        Provider::Custom,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::Xai => "xai",
            Self::Google => "google",
            Self::Openai => "openai",
            Self::Custom => "custom",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.id() == s)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Anthropic => "Anthropic Claude",
            Self::Xai => "xAI Grok",
            Self::Google => "Google Gemini",
            Self::Openai => "OpenAI",
            Self::Custom => "Custom (OpenAI-compatible)",
        }
    }

    /// Model alanı boş bırakılırsa kullanılan model.
    pub fn default_model(self) -> &'static str {
        match self {
            Self::Anthropic => "claude-sonnet-5-5",
            Self::Xai => "grok-4.5",
            Self::Google => "gemini-3.8-flash",
            Self::Openai => "gpt-5",
            Self::Custom => "llama3.1",
        }
    }

    /// OpenAI uyumlu sağlayıcıların temel adresi (Anthropic ve Custom için None).
    pub fn default_base_url(self) -> Option<&'static str> {
        match self {
            Self::Xai => Some("https://api.x.ai/v1"),
            Self::Google => Some("https://generativelanguage.googleapis.com/v1beta/openai"),
            Self::Openai => Some("https://api.openai.com/v1"),
            Self::Anthropic | Self::Custom => None,
        }
    }

    /// Anahtar için okunan ortam değişkenleri.
    pub fn env_vars(self) -> &'static [&'static str] {
        match self {
            Self::Anthropic => &["ANTHROPIC_API_KEY"],
            Self::Xai => &["XAI_API_KEY"],
            Self::Google => &["GEMINI_API_KEY", "GOOGLE_API_KEY"],
            Self::Openai => &["OPENAI_API_KEY"],
            Self::Custom => &["BOSUN_CUSTOM_API_KEY"],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Thresholds {
    pub disk_warn: f32,
    pub disk_crit: f32,
    pub mem_warn: f32,
    pub mem_sustain_secs: u64,
    pub swap_warn: f32,
    /// Toplam CPU bu yüzdenin üzerinde `cpu_sustain_secs` kalırsa "CPU meşgul" bulgusu.
    pub cpu_busy: f32,
    pub cpu_sustain_secs: u64,
    /// Belleği en az `leak_minutes` boyunca saatte bu kadar MB büyüyen uygulama bildirilir.
    pub leak_mb_per_hour: u64,
    pub leak_minutes: u64,
    pub temp_crit: f32,
    pub reclaimable_info_gb: f32,
    /// Bu kadar gündür değişmeyen önbellek ve günlük dosyaları "eski" sayılır.
    pub stale_days: u64,
    /// İndirilenlerde bu boyutun (MB) üzerindeki dosyalar listelenir.
    pub large_file_mb: u64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            disk_warn: 85.0,
            disk_crit: 95.0,
            mem_warn: 90.0,
            mem_sustain_secs: 300,
            swap_warn: 50.0,
            cpu_busy: 90.0,
            cpu_sustain_secs: 60,
            leak_mb_per_hour: 100,
            leak_minutes: 45,
            temp_crit: 85.0,
            reclaimable_info_gb: 2.0,
            stale_days: 30,
            large_file_mb: 500,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Arayüz, bildirimler ve asistan yanıtlarının dili ("en" ya da "tr").
    pub language: Lang,
    /// Sohbet sağlayıcısı (varsayılan Anthropic).
    pub provider: Provider,
    /// Model adı; boşsa sağlayıcının varsayılanı kullanılır.
    pub model: String,
    /// Yalnızca "custom" için (ör. Ollama: http://localhost:11434/v1). Diğerlerinde isteğe bağlı geçersiz kılma.
    pub base_url: Option<String>,
    /// Sağlayıcı başına API anahtarları: `[api_keys] xai = "…"`. Arayüz bunları asla geri okumaz.
    pub api_keys: BTreeMap<String, String>,
    /// Eski sürümlerle uyumluluk: tek Anthropic anahtarı.
    pub api_key: Option<String>,
    /// Büyük dosya yollarının API'ye gönderilmesine izin ver.
    pub share_file_paths: bool,
    pub sample_interval_secs: u64,
    pub scan_interval_secs: u64,
    pub notifications: bool,
    /// Sessize alınan bulgu kimlikleri (ör. "disk:/media/yedek").
    pub muted_findings: Vec<String>,
    /// Sohbet geçmişini ~/.local/state/bosun/chats.json dosyasında sakla (servis yeniden başlasa da sürer).
    pub persist_chat: bool,
    pub thresholds: Thresholds,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            language: Lang::En,
            provider: Provider::Anthropic,
            model: String::new(),
            base_url: None,
            api_keys: BTreeMap::new(),
            api_key: None,
            share_file_paths: false,
            sample_interval_secs: 5,
            scan_interval_secs: 600,
            notifications: true,
            persist_chat: true,
            muted_findings: Vec::new(),
            thresholds: Thresholds::default(),
        }
    }
}

impl Config {
    pub fn path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("bosun/config.toml")
    }

    pub fn load() -> Self {
        let mut cfg: Config = std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default();
        // Eski "api_key" alanı Anthropic anahtarıdır.
        if let Some(k) = cfg.api_key.clone().filter(|k| !k.trim().is_empty()) {
            cfg.api_keys.entry("anthropic".into()).or_insert(k);
        }
        for p in Provider::ALL {
            for var in p.env_vars() {
                if let Ok(k) = std::env::var(var) {
                    if !k.trim().is_empty() {
                        cfg.api_keys.insert(p.id().into(), k);
                    }
                }
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
        Self::update_file(|t| {
            t.insert("language".into(), toml::Value::String(lang.code().into()));
        })
    }

    /// Sohbet ayarlarını yazar. `api_key`: Some("") anahtarı siler, None dokunmaz.
    pub fn save_ai(
        provider: Provider,
        model: &str,
        base_url: Option<&str>,
        api_key: Option<&str>,
    ) -> std::io::Result<()> {
        Self::update_file(|t| {
            t.insert("provider".into(), toml::Value::String(provider.id().into()));
            t.insert("model".into(), toml::Value::String(model.trim().into()));
            match base_url.map(str::trim).filter(|u| !u.is_empty()) {
                Some(u) => t.insert("base_url".into(), toml::Value::String(u.into())),
                None => t.remove("base_url"),
            };
            if let Some(k) = api_key {
                let keys = t
                    .entry("api_keys")
                    .or_insert_with(|| toml::Value::Table(Default::default()));
                if let Some(keys) = keys.as_table_mut() {
                    if k.trim().is_empty() {
                        keys.remove(provider.id());
                    } else {
                        keys.insert(provider.id().into(), toml::Value::String(k.trim().into()));
                    }
                }
                // Eski tek anahtar alanı Anthropic'e aittir; silinince o da silinsin.
                if provider == Provider::Anthropic && k.trim().is_empty() {
                    t.remove("api_key");
                }
            }
        })
    }

    /// Açık/kapalı bir ayarı yazar (yalnızca bilinen anahtarlar).
    pub fn save_bool(key: &str, value: bool) -> std::io::Result<()> {
        if !matches!(key, "share_file_paths" | "notifications" | "persist_chat") {
            return Err(std::io::Error::other(format!("unknown setting: {key}")));
        }
        Self::update_file(|t| {
            t.insert(key.into(), toml::Value::Boolean(value));
        })
    }

    pub fn save_muted(list: &[String]) -> std::io::Result<()> {
        Self::update_file(|t| {
            t.insert(
                "muted_findings".into(),
                toml::Value::Array(
                    list.iter()
                        .map(|s| toml::Value::String(s.clone()))
                        .collect(),
                ),
            );
        })
    }

    pub fn scan_options(&self) -> crate::scan::ScanOptions {
        crate::scan::ScanOptions {
            lang: self.language,
            stale_days: self.thresholds.stale_days.max(1),
            large_file_bytes: self.thresholds.large_file_mb.max(1) * 1024 * 1024,
        }
    }

    /// Dosyayı tablo olarak okuyup değiştirir ve 0600 izniyle geri yazar.
    fn update_file(f: impl FnOnce(&mut toml::Table)) -> std::io::Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let path = Self::path();
        let mut table: toml::Table = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_default();
        f(&mut table);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = toml::to_string_pretty(&table).map_err(std::io::Error::other)?;
        std::fs::write(&path, text)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
    }

    pub fn key_for(&self, p: Provider) -> Option<&str> {
        self.api_keys
            .get(p.id())
            .map(String::as_str)
            .filter(|k| !k.trim().is_empty())
    }

    /// Etkin model (boşsa sağlayıcının varsayılanı).
    pub fn effective_model(&self) -> String {
        let m = self.model.trim();
        if m.is_empty() {
            self.provider.default_model().into()
        } else {
            m.into()
        }
    }

    /// OpenAI uyumlu sağlayıcılar için temel adres.
    pub fn effective_base_url(&self) -> Option<String> {
        self.base_url
            .as_deref()
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .map(|u| u.trim_end_matches('/').to_string())
            .or_else(|| self.provider.default_base_url().map(str::to_string))
    }

    /// Sohbet kullanılabilir mi? Özel sunucularda (ör. yerel Ollama) anahtar gerekmez, adres yeter.
    pub fn has_api_key(&self) -> bool {
        match self.provider {
            Provider::Custom => self.effective_base_url().is_some(),
            p => self.key_for(p).is_some(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_toml_uses_defaults() {
        let c: Config = toml::from_str("model = \"x\"\n[thresholds]\ndisk_warn = 70.0").unwrap();
        assert_eq!(c.effective_model(), "x");
        assert_eq!(c.thresholds.disk_warn, 70.0);
        assert_eq!(c.thresholds.disk_crit, 95.0);
        assert!(c.notifications);
        assert_eq!(c.language, Lang::En);
        let c: Config = toml::from_str("language = \"tr\"").unwrap();
        assert_eq!(c.language, Lang::Tr);
    }

    #[test]
    fn providers_and_keys() {
        let c: Config = toml::from_str("provider = \"xai\"\n[api_keys]\nxai = \"k\"").unwrap();
        assert_eq!(c.provider, Provider::Xai);
        assert!(c.has_api_key());
        assert_eq!(c.effective_model(), "grok-4.5");
        assert_eq!(
            c.effective_base_url().as_deref(),
            Some("https://api.x.ai/v1")
        );
        let c: Config =
            toml::from_str("provider = \"custom\"\nbase_url = \"http://localhost:11434/v1/\"")
                .unwrap();
        assert!(c.has_api_key(), "yerel sunucuda anahtar gerekmez");
        assert_eq!(
            c.effective_base_url().as_deref(),
            Some("http://localhost:11434/v1")
        );
        let c: Config = toml::from_str("provider = \"google\"").unwrap();
        assert!(!c.has_api_key());
        for p in Provider::ALL {
            assert_eq!(Provider::parse(p.id()), Some(p));
        }
    }
}
