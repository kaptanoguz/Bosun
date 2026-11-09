//! Temizlik adaylarının taranması. Tarama yalnızca okur; hiçbir şeyi değiştirmez.

use crate::categories::{self, CategoryId, Method, Safety, BROWSER_CACHE_DIRS, USER_CACHE_EXCLUDE};
use crate::i18n::Lang;
use crate::util::path_size;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use walkdir::WalkDir;

pub const STALE_DAYS: u64 = 30;
pub const LARGE_FILE_BYTES: u64 = 500 * 1024 * 1024;
const MAX_ITEMS: usize = 20_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanItem {
    pub path: PathBuf,
    pub bytes: u64,
    pub modified: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryScan {
    pub id: CategoryId,
    pub title: String,
    pub description: String,
    pub safety: Safety,
    pub method: Method,
    pub bulk_selectable: bool,
    pub manual_command: Option<String>,
    pub bytes: u64,
    pub item_count: usize,
    /// Tüm öğeler (daemon içinde tutulur; IPC üzerinden kırpılarak gönderilir).
    pub items: Vec<ScanItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub ts: DateTime<Utc>,
    pub duration_ms: u64,
    pub categories: Vec<CategoryScan>,
}

impl ScanResult {
    pub fn get(&self, id: CategoryId) -> Option<&CategoryScan> {
        self.categories.iter().find(|c| c.id == id)
    }

    /// Kullanıcı onayıyla tek seferde temizlenebilecek toplam alan.
    pub fn reclaimable_bytes(&self) -> u64 {
        self.categories
            .iter()
            .filter(|c| c.bulk_selectable && c.safety == Safety::Safe)
            .map(|c| c.bytes)
            .sum()
    }

    /// Kopya: her kategoride en büyük `n` öğe (IPC ve LLM için).
    pub fn trimmed(&self, n: usize) -> ScanResult {
        let mut out = self.clone();
        for c in &mut out.categories {
            c.items.sort_by_key(|i| std::cmp::Reverse(i.bytes));
            c.items.truncate(n);
        }
        out
    }
}

fn modified(meta: &std::fs::Metadata) -> Option<DateTime<Utc>> {
    meta.modified().ok().map(DateTime::<Utc>::from)
}

fn is_stale(meta: &std::fs::Metadata, days: u64) -> bool {
    meta.modified()
        .ok()
        .and_then(|m| SystemTime::now().duration_since(m).ok())
        .is_some_and(|age| age > Duration::from_secs(days * 86_400))
}

/// Bir kökün doğrudan çocuklarını (dosya ya da dizin) öğe olarak listeler.
fn children(root: &Path) -> Vec<ScanItem> {
    let Ok(rd) = std::fs::read_dir(root) else { return vec![] };
    rd.filter_map(Result::ok)
        .filter_map(|e| {
            let meta = e.path().symlink_metadata().ok()?;
            if meta.file_type().is_symlink() {
                return None;
            }
            let path = e.path();
            let bytes = path_size(&path);
            Some(ScanItem { path, bytes, modified: modified(&meta) })
        })
        .filter(|i| i.bytes > 0)
        .collect()
}

/// Bir kök altındaki dosyaları filtreyle gezer.
fn walk_files(
    root: &Path,
    max_depth: usize,
    skip_dir: impl Fn(&Path) -> bool,
    keep: impl Fn(&Path, &std::fs::Metadata) -> bool,
) -> Vec<ScanItem> {
    let mut out = Vec::new();
    let walker = WalkDir::new(root)
        .follow_links(false)
        .max_depth(max_depth)
        .into_iter()
        .filter_entry(|e| !(e.file_type().is_dir() && e.depth() > 0 && skip_dir(e.path())));
    for entry in walker.filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if keep(entry.path(), &meta) {
            out.push(ScanItem { path: entry.path().to_path_buf(), bytes: meta.len(), modified: modified(&meta) });
            if out.len() >= MAX_ITEMS {
                break;
            }
        }
    }
    out
}

/// `journalctl --disk-usage` çıktısını ayrıştırır: "... take up 1.2G in the file system."
pub fn parse_journal_usage(text: &str) -> Option<u64> {
    let idx = text.find("take up ")? + "take up ".len();
    let token = text[idx..].split_whitespace().next()?;
    let (num, unit) = token.split_at(token.find(|c: char| c.is_ascii_alphabetic()).unwrap_or(token.len()));
    let value: f64 = num.parse().ok()?;
    let mult = match unit.chars().next().map(|c| c.to_ascii_uppercase()) {
        None | Some('B') => 1.0,
        Some('K') => 1024.0,
        Some('M') => 1024.0 * 1024.0,
        Some('G') => 1024.0 * 1024.0 * 1024.0,
        Some('T') => 1024.0f64.powi(4),
        _ => return None,
    };
    Some((value * mult) as u64)
}

fn journal_bytes() -> u64 {
    std::process::Command::new("journalctl")
        .arg("--disk-usage")
        .output()
        .ok()
        .and_then(|o| parse_journal_usage(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_else(|| path_size(Path::new("/var/log/journal")))
}

pub fn scan_category(id: CategoryId, home: &Path, lang: Lang) -> CategoryScan {
    let roots = id.roots(home);
    let mut items: Vec<ScanItem> = match id {
        CategoryId::Thumbnails | CategoryId::BrowserCache | CategoryId::NpmCache | CategoryId::CargoCache => {
            roots.iter().flat_map(|r| children(r)).collect()
        }
        CategoryId::Trash => {
            let r = &roots[0];
            let mut v = children(&r.join("files"));
            v.extend(children(&r.join("info")));
            v
        }
        CategoryId::UserCache => {
            let cache = &roots[0];
            let skip: Vec<PathBuf> = BROWSER_CACHE_DIRS
                .iter()
                .chain(USER_CACHE_EXCLUDE.iter())
                .map(|d| cache.join(d))
                .collect();
            walk_files(cache, 8, |p| skip.iter().any(|s| s == p), |_, m| is_stale(m, STALE_DAYS))
        }
        CategoryId::OldLogs => walk_files(
            &roots[0],
            6,
            |_| false,
            |p, m| {
                let name = p.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
                (name.ends_with(".log") || name.contains(".log.")) && is_stale(m, STALE_DAYS)
            },
        ),
        CategoryId::LargeFiles => {
            let mut v = Vec::new();
            for r in &roots {
                // Aynı klasöre iki ad (Downloads ve İndirilenler bağlantısı) varsa tekrar sayma.
                if r.symlink_metadata().map(|m| m.file_type().is_symlink()).unwrap_or(true) {
                    continue;
                }
                v.extend(walk_files(r, 3, |_| false, |_, m| m.len() >= LARGE_FILE_BYTES));
            }
            v
        }
        CategoryId::AptCache => walk_files(&roots[0], 1, |_| false, |p, _| p.extension().is_some_and(|e| e == "deb")),
        CategoryId::Journal => {
            let bytes = journal_bytes();
            if bytes > 0 {
                vec![ScanItem { path: roots[0].clone(), bytes, modified: None }]
            } else {
                vec![]
            }
        }
    };
    items.sort_by_key(|i| std::cmp::Reverse(i.bytes));
    let bytes = items.iter().map(|i| i.bytes).sum();
    CategoryScan {
        id,
        title: id.title(lang).into(),
        description: id.description(lang).into(),
        safety: id.safety(),
        method: id.method(),
        bulk_selectable: id.bulk_selectable(),
        manual_command: id.manual_command().map(str::to_string),
        bytes,
        item_count: items.len(),
        items,
    }
}

pub fn scan_all(home: &Path, lang: Lang) -> ScanResult {
    let start = std::time::Instant::now();
    let categories = categories::ALL.iter().map(|id| scan_category(*id, home, lang)).collect();
    ScanResult { ts: Utc::now(), duration_ms: start.elapsed().as_millis() as u64, categories }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parses_journal_usage() {
        let s = "Archived and active journals take up 1.5G in the file system.";
        assert_eq!(parse_journal_usage(s), Some((1.5 * 1024.0 * 1024.0 * 1024.0) as u64));
        assert_eq!(parse_journal_usage("take up 800.0M in"), Some(800 * 1024 * 1024));
        assert_eq!(parse_journal_usage("nonsense"), None);
    }

    #[test]
    fn user_cache_skips_browser_and_fresh_files() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        fs::create_dir_all(home.join(".cache/app")).unwrap();
        fs::create_dir_all(home.join(".cache/mozilla")).unwrap();
        fs::write(home.join(".cache/app/fresh"), vec![0u8; 10]).unwrap();
        fs::write(home.join(".cache/mozilla/old"), vec![0u8; 10]).unwrap();
        // Taze dosyalar ve tarayıcı klasörü UserCache'e girmez.
        let scan = scan_category(CategoryId::UserCache, home, Lang::En);
        assert_eq!(scan.item_count, 0);
        // Ama BrowserCache'e girer.
        let b = scan_category(CategoryId::BrowserCache, home, Lang::En);
        assert_eq!(b.item_count, 1);
        assert_eq!(b.bytes, 10);
    }

    #[test]
    fn thumbnails_children_are_items() {
        let dir = tempfile::tempdir().unwrap();
        let t = dir.path().join(".cache/thumbnails/normal");
        fs::create_dir_all(&t).unwrap();
        fs::write(t.join("a.png"), vec![0u8; 42]).unwrap();
        let s = scan_category(CategoryId::Thumbnails, dir.path(), Lang::En);
        assert_eq!(s.item_count, 1);
        assert_eq!(s.bytes, 42);
        assert!(s.items[0].path.ends_with("thumbnails/normal"));
    }
}
