//! Dil desteği. Varsayılan İngilizce; kullanıcı ayarlardan Türkçe seçebilir.
//! Servisin ürettiği tüm kullanıcıya dönük metinler (kategori adları, bulgular,
//! öneriler, plan gerekçeleri, asistanın yanıt dili) bu seçime göre üretilir.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[default]
    En,
    Tr,
}

impl Lang {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "en" | "english" => Some(Self::En),
            "tr" | "turkish" | "türkçe" => Some(Self::Tr),
            _ => None,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Tr => "tr",
        }
    }

    /// İki dildeki karşılıklardan seçili olanı döndürür.
    pub fn pick<'a>(self, en: &'a str, tr: &'a str) -> &'a str {
        match self {
            Self::En => en,
            Self::Tr => tr,
        }
    }

    /// `pick` ile aynı, ama sahipli `String` döndürür (format! sonuçları için).
    pub fn s(self, en: impl Into<String>, tr: impl Into<String>) -> String {
        match self {
            Self::En => en.into(),
            Self::Tr => tr.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_english_and_parses() {
        assert_eq!(Lang::default(), Lang::En);
        assert_eq!(Lang::parse("TR"), Some(Lang::Tr));
        assert_eq!(Lang::parse("de"), None);
        assert_eq!(serde_json::to_string(&Lang::Tr).unwrap(), "\"tr\"");
    }
}
