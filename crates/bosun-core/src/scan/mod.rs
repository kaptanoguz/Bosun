//! Temizlik adaylarının taranması. Tarama yalnızca okur; hiçbir şeyi değiştirmez.

use crate::categories::{self, CategoryId, Method, Safety, BROWSER_CACHE_DIRS, USER_CACHE_EXCLUDE};
use crate::i18n::Lang;
use crate::leftovers;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

mod inspect;
mod system;
mod walk;

pub use inspect::{inspect, InspectEntry, Inspection};
pub use system::{estimate_unused_runtimes, parse_journal_usage, parse_pins, pin_matches};
use system::{flatpak_unused, journal_bytes};
use walk::{children, is_stale, walk_files};

pub const STALE_DAYS: u64 = 30;
pub const LARGE_FILE_BYTES: u64 = 500 * 1024 * 1024;
/// Bir kategoride tutulan en fazla öğe; aşılırsa kategori "kırpıldı" olarak işaretlenir.
pub const MAX_ITEMS: usize = 20_000;

/// Taramanın ayarlanabilir parametreleri (yapılandırmadan gelir).
#[derive(Debug, Clone, Copy)]
pub struct ScanOptions {
    pub lang: Lang,
    pub stale_days: u64,
    pub large_file_bytes: u64,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            lang: Lang::En,
            stale_days: STALE_DAYS,
            large_file_bytes: LARGE_FILE_BYTES,
        }
    }
}

/// İptal bayrağı olmadan çağrılan yerler için.
static NEVER: AtomicBool = AtomicBool::new(false);

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
    /// Yetkili kategoriler için `bosun-helper` eylemi (arayüz şifreyle uygulatır).
    pub admin_action: Option<String>,
    pub bytes: u64,
    pub item_count: usize,
    /// Öğe sınırına ulaşıldı; gerçek sayı ve boyut daha büyük olabilir.
    #[serde(default)]
    pub truncated: bool,
    /// Bulunan ama Bosun'un kendi yetkisiyle kaldıramayacağı öğeler (klasör başka bir
    /// kullanıcıya, çoğunlukla `sudo` ile çalışmış bir programa ait). Listeye ve toplama
    /// girmezler; arayüz sahipliği düzelten komutu gösterir.
    #[serde(default)]
    pub locked: Option<Locked>,
    /// Tüm öğeler (daemon içinde tutulur; IPC üzerinden kırpılarak gönderilir).
    pub items: Vec<ScanItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Locked {
    pub items: usize,
    pub bytes: u64,
    /// Yazılamayan en üst klasörler (kategori kökünün altında), en fazla birkaç tane.
    pub dirs: Vec<PathBuf>,
    pub command: String,
}

/// Bir yol çöpe taşınabilir ya da silinebilir mi? Üst klasöre yazma ve geçme hakkı gerekir;
/// yapışkan (sticky) klasörde ayrıca öğe bizim olmalı.
pub fn removable(path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    let Some(parent) = path.parent() else {
        return false;
    };
    if !writable(parent) {
        return false;
    }
    let Ok(pm) = std::fs::metadata(parent) else {
        return false;
    };
    // Linux'ta bir klasörü başka bir yere taşımak (çöpe atmak) ".." girdisini günceller;
    // bunun için klasörün kendisine de yazabilmek gerekir.
    let is_dir = std::fs::symlink_metadata(path).is_ok_and(|m| m.is_dir());
    if is_dir && !writable(path) {
        return false;
    }
    if pm.mode() & 0o1000 != 0 {
        // SAFETY: geteuid her zaman başarılıdır.
        let me = unsafe { libc::geteuid() };
        return std::fs::symlink_metadata(path).is_ok_and(|m| m.uid() == me);
    }
    true
}

fn writable(dir: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    std::ffi::CString::new(dir.as_os_str().as_bytes())
        // SAFETY: geçerli, NUL ile biten bir C dizgesi; access yalnızca okur.
        .is_ok_and(|c| unsafe { libc::access(c.as_ptr(), libc::W_OK | libc::X_OK) } == 0)
}

/// `path`'in `root` altında kalan, yazılamayan en üst atası; yoksa üst klasörü.
fn top_locked_dir(path: &Path, root: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or(path);
    let mut top = parent.to_path_buf();
    for a in parent.ancestors() {
        if a == root || !a.starts_with(root) {
            break;
        }
        if !writable(a) {
            top = a.to_path_buf();
        }
    }
    top
}

fn shell_quote(p: &Path) -> String {
    format!("'{}'", p.to_string_lossy().replace('\'', "'\\''"))
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

    /// `under` altında kalan ve Disk ekranından tek tıkla çöpe taşınabilecek öğeler:
    /// yalnızca toplu seçilebilen ve çöpe taşınan kategorilerden.
    pub fn trashable_under<'a>(
        &'a self,
        under: &'a Path,
    ) -> impl Iterator<Item = (CategoryId, &'a ScanItem)> + 'a {
        self.categories
            .iter()
            .filter(|c| c.bulk_selectable && c.method == Method::Trash)
            .flat_map(move |c| {
                c.items
                    .iter()
                    .filter(move |i| i.path.starts_with(under))
                    .map(move |i| (c.id, i))
            })
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

pub fn scan_category(id: CategoryId, home: &Path, lang: Lang) -> CategoryScan {
    scan_category_with(
        id,
        home,
        &ScanOptions {
            lang,
            ..Default::default()
        },
        &NEVER,
    )
}

pub fn scan_category_with(
    id: CategoryId,
    home: &Path,
    opts: &ScanOptions,
    cancel: &AtomicBool,
) -> CategoryScan {
    let lang = opts.lang;
    let stale = opts.stale_days;
    let roots = id.roots(home);
    let mut items: Vec<ScanItem> = match id {
        CategoryId::Thumbnails
        | CategoryId::BrowserCache
        | CategoryId::AppCaches
        | CategoryId::NpmCache
        | CategoryId::CargoCache
        | CategoryId::FlatpakCache
        | CategoryId::DevCaches
        | CategoryId::SteamCache => roots.iter().flat_map(|r| children(r)).collect(),
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
            walk_files(
                cache,
                8,
                cancel,
                |p| skip.iter().any(|s| s == p),
                |_, m| is_stale(m, stale),
            )
        }
        CategoryId::OldLogs => walk_files(
            &roots[0],
            6,
            cancel,
            |_| false,
            |p, m| {
                let name = p
                    .file_name()
                    .map(|n| n.to_string_lossy())
                    .unwrap_or_default();
                (name.ends_with(".log") || name.contains(".log.")) && is_stale(m, stale)
            },
        ),
        // Disk ekranından seçilen dosyalar taramada aranmaz.
        CategoryId::ChosenFile => Vec::new(),
        CategoryId::LargeFiles => {
            let mut v = Vec::new();
            for r in &roots {
                // Aynı klasöre iki ad (Downloads ve İndirilenler bağlantısı) varsa tekrar sayma.
                if r.symlink_metadata()
                    .map(|m| m.file_type().is_symlink())
                    .unwrap_or(true)
                {
                    continue;
                }
                v.extend(walk_files(
                    r,
                    3,
                    cancel,
                    |_| false,
                    |_, m| m.len() >= opts.large_file_bytes,
                ));
            }
            v
        }
        CategoryId::AptCache => walk_files(
            &roots[0],
            1,
            cancel,
            |_| false,
            |p, _| p.extension().is_some_and(|e| e == "deb"),
        ),
        CategoryId::FlatpakUnused => flatpak_unused(&roots[0]),
        CategoryId::ContainerCache => crate::containers::scan(),
        CategoryId::BrokenLinks => leftovers::broken_links(home),
        CategoryId::OrphanLaunchers => {
            leftovers::orphan_launchers(home, &leftovers::ProgramIndex::new(home))
        }
        CategoryId::LeftoverConfigs => {
            let idx = leftovers::ProgramIndex::new(home);
            let installed = leftovers::installed_names(home, &idx);
            leftovers::leftover_dirs(home, &installed)
        }
        CategoryId::OrphanPackages => leftovers::orphan_packages(),
        CategoryId::ResidualPackages => leftovers::residual_packages(),
        CategoryId::Journal => {
            let bytes = journal_bytes();
            if bytes > 0 {
                vec![ScanItem {
                    path: roots[0].clone(),
                    bytes,
                    modified: None,
                }]
            } else {
                vec![]
            }
        }
    };
    // Kendi yetkimizle kaldıramayacağımız öğeleri ayır: aksi halde her temizlikte
    // "başarısız" olur ve sonraki taramada yine listelenirler.
    let mut locked = None;
    if matches!(
        id.method(),
        Method::Trash | Method::Permanent | Method::Unlink
    ) {
        let (ok, bad): (Vec<_>, Vec<_>) = items.into_iter().partition(|i| removable(&i.path));
        items = ok;
        if !bad.is_empty() {
            let mut dirs: Vec<PathBuf> = Vec::new();
            for i in &bad {
                let root = roots.iter().find(|r| i.path.starts_with(r));
                let d = root.map_or_else(
                    || i.path.parent().unwrap_or(&i.path).to_path_buf(),
                    |r| top_locked_dir(&i.path, r),
                );
                if !dirs.contains(&d) && dirs.len() < 5 {
                    dirs.push(d);
                }
            }
            let command = format!(
                "sudo chown -R \"$USER:\" {}",
                dirs.iter()
                    .map(|d| shell_quote(d))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            locked = Some(Locked {
                items: bad.len(),
                bytes: bad.iter().map(|i| i.bytes).sum(),
                dirs,
                command,
            });
        }
    }
    items.sort_by_key(|i| std::cmp::Reverse(i.bytes));
    let bytes = items.iter().map(|i| i.bytes).sum();
    let truncated = items.len() >= MAX_ITEMS
        || (id == CategoryId::BrokenLinks && items.len() >= leftovers::BROKEN_LINK_LIMIT);
    CategoryScan {
        id,
        title: id.title(lang).into(),
        description: id.description(lang).into(),
        safety: id.safety(),
        method: id.method(),
        bulk_selectable: id.bulk_selectable(),
        manual_command: id.manual_command().map(str::to_string),
        admin_action: id.admin_action().map(str::to_string),
        bytes,
        item_count: items.len(),
        truncated,
        locked,
        items,
    }
}

pub fn scan_all(home: &Path, lang: Lang) -> ScanResult {
    scan_all_with(
        home,
        &ScanOptions {
            lang,
            ..Default::default()
        },
        &NEVER,
        |_, _, _| {},
    )
    .expect("iptal edilmedi")
}

/// Tüm kategorileri tarar. Her kategoriden önce `progress(tamamlanan, toplam, kategori)` çağrılır.
/// `cancel` ayarlanırsa tarama yarıda bırakılır ve None döner.
pub fn scan_all_with(
    home: &Path,
    opts: &ScanOptions,
    cancel: &AtomicBool,
    progress: impl Fn(usize, usize, CategoryId),
) -> Option<ScanResult> {
    let start = std::time::Instant::now();
    let total = categories::ALL.len();
    let mut categories = Vec::with_capacity(total);
    for (i, id) in categories::ALL.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        progress(i, total, *id);
        categories.push(scan_category_with(*id, home, opts, cancel));
    }
    if cancel.load(Ordering::Relaxed) {
        return None;
    }
    progress(total, total, categories::ALL[total - 1]);
    Some(ScanResult {
        ts: Utc::now(),
        duration_ms: start.elapsed().as_millis() as u64,
        categories,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn estimates_unused_flatpak_runtimes() {
        let apps = "org.gnome.Platform/x86_64/46\norg.freedesktop.Platform/x86_64/23.08\n";
        let runtimes = "org.gnome.Platform\tx86_64\t46\n\
org.gnome.Platform\tx86_64\t44\n\
org.gnome.Platform.Locale\tx86_64\t46\n\
org.freedesktop.Platform\tx86_64\t23.08\n\
org.freedesktop.Platform.GL.default\tx86_64\t23.08\n\
org.freedesktop.Platform.openh264\tx86_64\t2.2.0\n\
org.kde.Platform\tx86_64\t5.15-23.08\n";
        let unused: Vec<String> = estimate_unused_runtimes(apps, runtimes)
            .into_iter()
            .map(|(id, _, b)| format!("{id}/{b}"))
            .collect();
        assert_eq!(
            unused,
            vec!["org.gnome.Platform/44", "org.kde.Platform/5.15-23.08"]
        );
        // Senin sistemindeki durum: uygulama eklentisi ve GTK teması "kullanılmıyor" sayılmamalı.
        let apps = "com.github.rafostar.Clapper\torg.gnome.Platform/x86_64/47\n";
        let runtimes = "com.github.rafostar.Clapper.Enhancers\tx86_64\tstable\n\
org.gtk.Gtk3theme.Mint-Y-Dark-Aqua\tx86_64\t3.22\n\
org.gnome.Platform\tx86_64\t47\n";
        assert!(estimate_unused_runtimes(apps, runtimes).is_empty());
        // Ekran görüntüsündeki durum: GPU ve codec eklentileri listelenmemeli.
        let runtimes = "org.freedesktop.Platform.GL.default\tx86_64\t26.08-extra\n\
org.freedesktop.Platform.VAAPI.Intel\tx86_64\t25.08\n\
org.freedesktop.Platform.codecs-extra\tx86_64\t26.08-extra\n";
        assert!(estimate_unused_runtimes(apps, runtimes).is_empty());
        assert!(estimate_unused_runtimes("", "").is_empty());
    }

    #[test]
    fn pinned_runtimes_are_recognised() {
        let pins = parse_pins("runtime/org.gnome.Platform/x86_64/44\nruntime/org.kde.*\n");
        assert_eq!(pins.len(), 2);
        assert!(pin_matches(&pins[0], "org.gnome.Platform", "x86_64", "44"));
        assert!(!pin_matches(&pins[0], "org.gnome.Platform", "x86_64", "46"));
        assert!(pin_matches(
            "runtime/org.gnome.Platform//44",
            "org.gnome.Platform",
            "aarch64",
            "44"
        ));
        assert!(pin_matches(&pins[1], "org.kde.Platform", "x86_64", "6.7"));
    }

    #[test]
    fn flatpak_app_caches_are_found_but_not_browsers() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join(".var/app/org.gimp.GIMP/cache/gegl");
        let ff = dir.path().join(".var/app/org.mozilla.firefox/cache/x");
        fs::create_dir_all(&app).unwrap();
        fs::create_dir_all(&ff).unwrap();
        fs::write(app.join("t"), vec![0u8; 20]).unwrap();
        fs::write(ff.join("t"), vec![0u8; 30]).unwrap();
        let f = scan_category(CategoryId::FlatpakCache, dir.path(), Lang::En);
        assert_eq!(f.bytes, 20);
        let b = scan_category(CategoryId::BrowserCache, dir.path(), Lang::En);
        assert_eq!(b.bytes, 30);
    }

    #[test]
    fn cancel_stops_the_scan() {
        let dir = tempfile::tempdir().unwrap();
        let flag = AtomicBool::new(true);
        assert!(scan_all_with(dir.path(), &ScanOptions::default(), &flag, |_, _, _| {}).is_none());
        let flag = AtomicBool::new(false);
        let seen = std::cell::Cell::new(0);
        let r = scan_all_with(dir.path(), &ScanOptions::default(), &flag, |_, _, _| {
            seen.set(seen.get() + 1)
        })
        .unwrap();
        assert_eq!(r.categories.len(), categories::ALL.len());
        assert_eq!(seen.get(), categories::ALL.len() + 1);
    }

    #[test]
    fn configurable_stale_days_and_large_files() {
        let dir = tempfile::tempdir().unwrap();
        let dl = dir.path().join("Downloads");
        fs::create_dir_all(&dl).unwrap();
        fs::write(dl.join("mid.bin"), vec![0u8; 2048]).unwrap();
        let opts = ScanOptions {
            large_file_bytes: 1024,
            ..Default::default()
        };
        let c = scan_category_with(CategoryId::LargeFiles, dir.path(), &opts, &NEVER);
        assert_eq!(c.item_count, 1);
        assert!(!c.truncated);
    }

    #[test]
    fn dev_caches_are_found() {
        let dir = tempfile::tempdir().unwrap();
        let pip = dir.path().join(".cache/pip/http-v2");
        fs::create_dir_all(&pip).unwrap();
        fs::write(pip.join("x"), vec![0u8; 77]).unwrap();
        let c = scan_category(CategoryId::DevCaches, dir.path(), Lang::En);
        assert_eq!(c.bytes, 77);
        // Aynı dosya "eski uygulama önbellekleri"nde ikinci kez sayılmaz.
        assert!(USER_CACHE_EXCLUDE.contains(&"pip"));
    }

    #[test]
    fn inspect_lists_biggest_children() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("a")).unwrap();
        fs::write(dir.path().join("a/big"), vec![0u8; 100]).unwrap();
        fs::write(dir.path().join("small"), vec![0u8; 5]).unwrap();
        let i = inspect(dir.path(), 10);
        assert_eq!(i.files, 2);
        assert_eq!(i.bytes, 105);
        assert_eq!(i.entries[0].name, "a");
        assert!(i.entries[0].is_dir);
    }

    #[test]
    fn parses_journal_usage() {
        let s = "Archived and active journals take up 1.5G in the file system.";
        assert_eq!(
            parse_journal_usage(s),
            Some((1.5 * 1024.0 * 1024.0 * 1024.0) as u64)
        );
        assert_eq!(
            parse_journal_usage("take up 800.0M in"),
            Some(800 * 1024 * 1024)
        );
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

    /// Root her yere yazabildiği için izin testleri ancak normal kullanıcıyla anlamlıdır.
    fn running_as_root() -> bool {
        // SAFETY: geteuid her zaman başarılıdır.
        unsafe { libc::geteuid() == 0 }
    }

    fn chmod(p: &Path, mode: u32) {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(p, fs::Permissions::from_mode(mode)).unwrap();
    }

    #[test]
    fn unwritable_items_are_counted_as_locked() {
        if running_as_root() {
            eprintln!("skipped: running as root");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let ws = home.join(".cache/waydroid-script");
        let sub = ws.join("downloads");
        fs::create_dir_all(&sub).unwrap();
        fs::create_dir_all(home.join(".cache/app")).unwrap();
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(90 * 86_400);
        for p in [sub.join("img.zip"), home.join(".cache/app/blob")] {
            fs::write(&p, vec![0u8; 100]).unwrap();
            fs::File::options()
                .write(true)
                .open(&p)
                .unwrap()
                .set_modified(old)
                .unwrap();
        }
        chmod(&sub, 0o555);
        chmod(&ws, 0o555);
        let s = scan_category(CategoryId::UserCache, home, Lang::En);
        chmod(&ws, 0o755);
        chmod(&sub, 0o755);

        assert_eq!(s.item_count, 1);
        assert_eq!(s.bytes, 100);
        assert!(s.items[0].path.ends_with("app/blob"));
        let l = s.locked.expect("locked summary");
        assert_eq!((l.items, l.bytes), (1, 100));
        assert_eq!(l.dirs, vec![ws.clone()]);
        assert!(l.command.starts_with("sudo chown -R \"$USER:\" '"));
        assert!(l.command.ends_with("/.cache/waydroid-script'"));
    }

    #[test]
    fn unwritable_directory_item_is_locked() {
        if running_as_root() {
            eprintln!("skipped: running as root");
            return;
        }
        // Klasör öğesinin üstü yazılabilir olsa da klasörün kendisi yazılamıyorsa çöpe
        // taşınamaz (Linux ".." girdisini güncellemek ister).
        let dir = tempfile::tempdir().unwrap();
        let t = dir.path().join(".cache/thumbnails");
        fs::create_dir_all(t.join("normal")).unwrap();
        fs::create_dir_all(t.join("large")).unwrap();
        fs::write(t.join("normal/a.png"), vec![0u8; 10]).unwrap();
        fs::write(t.join("large/b.png"), vec![0u8; 20]).unwrap();
        chmod(&t.join("large"), 0o555);
        let s = scan_category(CategoryId::Thumbnails, dir.path(), Lang::En);
        chmod(&t.join("large"), 0o755);

        assert_eq!(s.item_count, 1);
        assert!(s.items[0].path.ends_with("thumbnails/normal"));
        let l = s.locked.expect("locked summary");
        assert_eq!((l.items, l.bytes), (1, 20));
    }

    #[test]
    fn shell_quote_escapes_single_quotes() {
        assert_eq!(shell_quote(Path::new("/a/it's")), r"'/a/it'\''s'");
    }
}
