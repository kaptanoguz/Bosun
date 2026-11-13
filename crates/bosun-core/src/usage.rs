//! Disk kullanımı: bir klasörün içindekilerin ne kadar yer kapladığı.
//!
//! Yalnızca okur. Sembolik bağlantılar izlenmez ve başka bir dosya sistemine (ayrı disk,
//! ağ paylaşımı) inilmez; bunlar ayrı işaretlenir.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub bytes: u64,
    pub is_dir: bool,
    /// Ayrı bir dosya sistemi (içine inilmedi, boyutu sayılmadı).
    pub other_fs: bool,
    /// Son taramaya göre bu öğenin altında çöpe taşınabilecek veri (bayt).
    #[serde(default)]
    pub cleanable: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Listing {
    pub path: PathBuf,
    pub bytes: u64,
    /// Büyükten küçüğe.
    pub entries: Vec<Entry>,
    /// Okunamayan klasör sayısı (izin yok vb.).
    pub unreadable: u64,
}

/// Önceden hesaplanmış klasör boyutları; alt klasöre inildiğinde yeniden yürümemek için.
pub type SizeCache = HashMap<PathBuf, u64>;

/// Bir klasörün toplam boyutu (gerçekte diskte kapladığı blok sayısına göre).
/// `depth` kadar alt seviyedeki klasörlerin boyutları `cache`'e yazılır.
fn dir_size(dir: &Path, dev: u64, depth: u32, cache: &mut SizeCache, unreadable: &mut u64) -> u64 {
    let Ok(rd) = std::fs::read_dir(dir) else {
        *unreadable += 1;
        return 0;
    };
    let mut total = 0;
    for e in rd.flatten() {
        let Ok(meta) = e.path().symlink_metadata() else {
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            if meta.dev() != dev {
                continue;
            }
            total += dir_size(&e.path(), dev, depth.saturating_sub(1), cache, unreadable);
        } else {
            total += disk_bytes(&meta);
        }
    }
    if depth > 0 {
        cache.insert(dir.to_path_buf(), total);
    }
    total
}

/// Diskte gerçekten kaplanan alan; seyrek dosyalarda (sanal makine diskleri gibi)
/// görünen boyut yanıltıcıdır.
fn disk_bytes(meta: &std::fs::Metadata) -> u64 {
    meta.blocks() * 512
}

/// `dir`'in doğrudan içindekileri boyutlarıyla listeler.
pub fn list(dir: &Path, cache: &mut SizeCache) -> std::io::Result<Listing> {
    let root_meta = dir.symlink_metadata()?;
    if !root_meta.is_dir() {
        return Err(std::io::Error::other("not a folder"));
    }
    let dev = root_meta.dev();
    let mut entries = Vec::new();
    let mut unreadable = 0;
    for e in std::fs::read_dir(dir)?.flatten() {
        let path = e.path();
        let Ok(meta) = path.symlink_metadata() else {
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        if meta.is_dir() {
            let other_fs = meta.dev() != dev;
            let bytes = if other_fs {
                0
            } else if let Some(b) = cache.get(&path) {
                *b
            } else {
                dir_size(&path, dev, 2, cache, &mut unreadable)
            };
            entries.push(Entry {
                name,
                path,
                bytes,
                is_dir: true,
                other_fs,
                cleanable: 0,
            });
        } else {
            entries.push(Entry {
                name,
                bytes: disk_bytes(&meta),
                path,
                is_dir: false,
                other_fs: false,
                cleanable: 0,
            });
        }
    }
    entries.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.name.cmp(&b.name)));
    let bytes = entries.iter().map(|e| e.bytes).sum();
    cache.insert(dir.to_path_buf(), bytes);
    Ok(Listing {
        path: dir.to_path_buf(),
        bytes,
        entries,
        unreadable,
    })
}

/// İstenen yol ev klasörünün içinde mi? Sembolik bağlantılarla dışarı çıkılamaz.
pub fn within_home(path: &Path, home: &Path) -> Option<PathBuf> {
    let real = path.canonicalize().ok()?;
    let home = home.canonicalize().ok()?;
    real.starts_with(&home).then_some(real)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_summed_and_sorted() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("big/inner")).unwrap();
        std::fs::write(root.join("big/inner/a"), vec![1u8; 64 * 1024]).unwrap();
        std::fs::write(root.join("big/b"), vec![1u8; 32 * 1024]).unwrap();
        std::fs::write(root.join("small"), vec![1u8; 4 * 1024]).unwrap();
        std::os::unix::fs::symlink(root.join("big"), root.join("link")).unwrap();

        let mut cache = SizeCache::new();
        let l = list(root, &mut cache).unwrap();
        let names: Vec<_> = l.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["big", "small"], "links are skipped, biggest first");
        assert!(l.entries[0].bytes >= 96 * 1024);
        assert_eq!(l.bytes, l.entries.iter().map(|e| e.bytes).sum::<u64>());
        // Alt klasörün boyutu önbellekte: içine inince yeniden yürünmez.
        assert!(cache.contains_key(&root.join("big/inner")));
        let inner = list(&root.join("big"), &mut cache).unwrap();
        assert_eq!(inner.entries[0].name, "inner");
    }

    #[test]
    fn paths_outside_home_are_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir_all(home.join("docs")).unwrap();
        std::os::unix::fs::symlink("/etc", home.join("etc")).unwrap();
        assert!(within_home(&home.join("docs"), &home).is_some());
        assert!(within_home(&home.join("etc"), &home).is_none());
        assert!(within_home(&home.join("docs/../.."), &home).is_none());
    }
}
