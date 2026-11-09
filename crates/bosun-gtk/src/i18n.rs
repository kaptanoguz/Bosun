//! Arayüz metinleri (`locales/*.json`) ve sayı/zaman biçimleri.
//!
//! Metin şablonları `{ad}` yer tutucuları ve tekil/çoğul için `{n|item|items}` kullanır.

use bosun_core::Lang;
use chrono::{DateTime, Local, Utc};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

type Dict = HashMap<String, String>;

fn dicts() -> &'static (Dict, Dict) {
    static D: OnceLock<(Dict, Dict)> = OnceLock::new();
    D.get_or_init(|| {
        let parse =
            |s: &str| -> Dict { serde_json::from_str(s).expect("locale file is valid JSON") };
        (
            parse(include_str!("../locales/en.json")),
            parse(include_str!("../locales/tr.json")),
        )
    })
}

static TURKISH: AtomicBool = AtomicBool::new(false);

pub fn set_lang(l: Lang) {
    TURKISH.store(l == Lang::Tr, Ordering::Relaxed);
}

pub fn lang() -> Lang {
    if TURKISH.load(Ordering::Relaxed) {
        Lang::Tr
    } else {
        Lang::En
    }
}

/// Anahtarın metni (Türkçede eksikse İngilizcesi, o da yoksa anahtarın kendisi).
pub fn t(key: &str) -> String {
    let (en, tr) = dicts();
    let d = if lang() == Lang::Tr { tr } else { en };
    d.get(key)
        .or_else(|| en.get(key))
        .cloned()
        .unwrap_or_else(|| key.to_string())
}

/// Yer tutucularla: `tv("plan.items", &[("n", 3.into())])`.
pub fn tv(key: &str, vars: &[(&str, Value)]) -> String {
    fill(&t(key), vars)
}

fn as_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

pub fn fill(template: &str, vars: &[(&str, Value)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('}') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let inner = &rest[start + 1..start + end];
        let mut parts = inner.split('|');
        let name = parts.next().unwrap_or("");
        let value = vars.iter().find(|(k, _)| *k == name).map(|(_, v)| v);
        match (value, parts.next(), parts.next()) {
            (Some(v), Some(one), Some(many)) => {
                out.push_str(if v.as_f64() == Some(1.0) { one } else { many })
            }
            (Some(v), None, _) => out.push_str(&as_text(v)),
            _ => out.push_str(&rest[start..start + end + 1]),
        }
        rest = &rest[start + end + 1..];
    }
    out.push_str(rest);
    out
}

fn decimal(s: String) -> String {
    if lang() == Lang::Tr {
        s.replace('.', ",")
    } else {
        s
    }
}

/// 1536 → "1.5 KB" (Türkçede "1,5 KB").
pub fn bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    if n == 0 {
        return "0 B".into();
    }
    let mut v = n as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    let s = if u == 0 || v >= 100.0 {
        format!("{v:.0}")
    } else {
        format!("{v:.1}")
    };
    format!("{} {}", decimal(s), UNITS[u])
}

/// Türkçede yüzde işareti başa gelir.
pub fn percent(v: f64) -> String {
    let r = v.round() as i64;
    if lang() == Lang::Tr {
        format!("%{r}")
    } else {
        format!("{r}%")
    }
}

pub fn pct(used: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        used as f64 / total as f64 * 100.0
    }
}

pub fn ago(ts: DateTime<Utc>) -> String {
    let secs = (Utc::now() - ts).num_seconds().max(0);
    let tr = lang() == Lang::Tr;
    match secs {
        0..45 => (if tr { "az önce" } else { "just now" }).into(),
        45..3600 => {
            let m = (secs + 30) / 60;
            if tr {
                format!("{m} dk önce")
            } else {
                format!("{m} min ago")
            }
        }
        3600..86400 => {
            let h = (secs + 1800) / 3600;
            if tr {
                format!("{h} sa önce")
            } else {
                format!("{h} h ago")
            }
        }
        _ => {
            let d = (secs + 43200) / 86400;
            if tr {
                format!("{d} gün önce")
            } else {
                format!("{d} d ago")
            }
        }
    }
}

pub fn clock(ts: DateTime<Utc>) -> String {
    let l = ts.with_timezone(&Local);
    if lang() == Lang::Tr {
        l.format("%d.%m.%Y %H:%M").to_string()
    } else {
        l.format("%b %-d, %Y, %H:%M").to_string()
    }
}

pub fn time_of_day(ts: DateTime<Utc>) -> String {
    ts.with_timezone(&Local).format("%H:%M").to_string()
}

pub fn uptime(secs: u64) -> String {
    let (d, h, m) = (secs / 86400, secs % 86400 / 3600, secs % 3600 / 60);
    let (dl, hl, ml) = if lang() == Lang::Tr {
        ("gün", "sa", "dk")
    } else {
        ("d", "h", "min")
    };
    if d > 0 {
        format!("{d} {dl} {h} {hl}")
    } else if h > 0 {
        format!("{h} {hl} {m} {ml}")
    } else {
        format!("{m} {ml}")
    }
}

/// Ev klasörünü ~ ile kısaltır.
pub fn short_path(p: &str) -> String {
    match std::env::var("HOME") {
        Ok(h) if !h.is_empty() && p.starts_with(&h) => format!("~{}", &p[h.len()..]),
        _ => p.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn fills_placeholders_and_plurals() {
        assert_eq!(fill("{n} {n|item|items}", &[("n", json!(1))]), "1 item");
        assert_eq!(fill("{n} {n|item|items}", &[("n", json!(4))]), "4 items");
        assert_eq!(fill("{size} free", &[("size", json!("2 GB"))]), "2 GB free");
        assert_eq!(fill("{missing} stays", &[]), "{missing} stays");
    }

    #[test]
    fn both_languages_have_the_same_keys_and_placeholders() {
        let (en, tr) = dicts();
        let vars = |s: &str| {
            let mut v: Vec<String> = s
                .split('{')
                .skip(1)
                .filter_map(|p| p.split(['}', '|']).next().map(str::to_string))
                .collect();
            v.sort();
            v.dedup();
            v
        };
        for (k, v) in en {
            let other = tr
                .get(k)
                .unwrap_or_else(|| panic!("missing Turkish text for {k}"));
            assert_eq!(vars(v), vars(other), "{k}");
        }
        assert_eq!(en.len(), tr.len());
    }

    #[test]
    fn sizes() {
        set_lang(Lang::En);
        assert_eq!(bytes(1536), "1.5 KB");
        assert_eq!(bytes(476 * 1024 * 1024 * 1024), "476 GB");
        assert_eq!(percent(42.4), "42%");
    }
}
