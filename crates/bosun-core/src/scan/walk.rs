//! Dosya sistemi gezme yardımcıları: çocuklar, filtreli yürüme, eskime.

use super::{ScanItem, MAX_ITEMS};
use crate::util::path_size;
use chrono::{DateTime, Utc};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};
use walkdir::WalkDir;

pub(super) fn modified(meta: &std::fs::Metadata) -> Option<DateTime<Utc>> {
    meta.modified().ok().map(DateTime::<Utc>::from)
}

pub(super) fn is_stale(meta: &std::fs::Metadata, days: u64) -> bool {
    meta.modified()
        .ok()
        .and_then(|m| SystemTime::now().duration_since(m).ok())
        .is_some_and(|age| age > Duration::from_secs(days * 86_400))
}

/// Bir kökün doğrudan çocuklarını (dosya ya da dizin) öğe olarak listeler.
pub(super) fn children(root: &Path) -> Vec<ScanItem> {
    let Ok(rd) = std::fs::read_dir(root) else {
        return vec![];
    };
    rd.filter_map(Result::ok)
        .filter_map(|e| {
            let meta = e.path().symlink_metadata().ok()?;
            if meta.file_type().is_symlink() {
                return None;
            }
            let path = e.path();
            let bytes = path_size(&path);
            Some(ScanItem {
                path,
                bytes,
                modified: modified(&meta),
            })
        })
        .filter(|i| i.bytes > 0)
        .collect()
}

/// Bir kök altındaki dosyaları filtreyle gezer.
pub(super) fn walk_files(
    root: &Path,
    max_depth: usize,
    cancel: &AtomicBool,
    skip_dir: impl Fn(&Path) -> bool,
    keep: impl Fn(&Path, &std::fs::Metadata) -> bool,
) -> Vec<ScanItem> {
    let mut out = Vec::new();
    let walker = WalkDir::new(root)
        .follow_links(false)
        .max_depth(max_depth)
        .into_iter()
        .filter_entry(|e| !(e.file_type().is_dir() && e.depth() > 0 && skip_dir(e.path())));
    for (n, entry) in walker.filter_map(Result::ok).enumerate() {
        if n % 2048 == 0 && cancel.load(Ordering::Relaxed) {
            break;
        }
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if keep(entry.path(), &meta) {
            out.push(ScanItem {
                path: entry.path().to_path_buf(),
                bytes: meta.len(),
                modified: modified(&meta),
            });
            if out.len() >= MAX_ITEMS {
                break;
            }
        }
    }
    out
}
