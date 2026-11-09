//! "İçine bak" önizlemesi: bir klasörün en büyük çocukları ve son değişikliği.

use crate::util::path_size;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use walkdir::WalkDir;

/// Bir klasörün içine bakış: en büyük çocuklar, toplam dosya sayısı ve son değişiklik.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inspection {
    pub path: PathBuf,
    pub files: usize,
    pub bytes: u64,
    pub newest: Option<DateTime<Utc>>,
    pub entries: Vec<InspectEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectEntry {
    pub name: String,
    pub is_dir: bool,
    pub bytes: u64,
}

pub fn inspect(path: &Path, limit: usize) -> Inspection {
    let mut files = 0usize;
    let mut bytes = 0u64;
    let mut newest: Option<SystemTime> = None;
    for e in WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .take(200_000)
    {
        if let Ok(m) = e.metadata() {
            if m.is_file() {
                files += 1;
                bytes += m.len();
                if let Ok(t) = m.modified() {
                    newest = Some(newest.map_or(t, |n| n.max(t)));
                }
            }
        }
    }
    let mut entries: Vec<InspectEntry> = std::fs::read_dir(path)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .map(|e| {
                    let p = e.path();
                    let is_dir = p.symlink_metadata().map(|m| m.is_dir()).unwrap_or(false);
                    InspectEntry {
                        name: e.file_name().to_string_lossy().into_owned(),
                        is_dir,
                        bytes: path_size(&p),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    entries.sort_by_key(|e| std::cmp::Reverse(e.bytes));
    entries.truncate(limit);
    Inspection {
        path: path.to_path_buf(),
        files,
        bytes,
        newest: newest.map(DateTime::<Utc>::from),
        entries,
    }
}
