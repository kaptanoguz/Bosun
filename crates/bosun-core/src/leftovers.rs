//! Eski programlardan kalanlar: kırık sembolik bağlantılar, programı artık olmayan
//! kısayollar, kaldırılmış paketlerin artıkları ve sahipsiz görünen ayar klasörleri.
//! Burada yalnızca **bulunur**; silme her zaman plan + onay üzerinden olur.

use crate::scan::ScanItem;
use crate::util::path_size;
use chrono::{DateTime, Utc};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use walkdir::WalkDir;

/// Kırık bağlantı taramasında içine girilmeyen klasörler (büyük, gizli ya da başkasına ait).
const LINK_SKIP: &[&str] = &[
    ".cache",
    ".git",
    "node_modules",
    ".cargo",
    ".rustup",
    ".npm",
    ".nvm",
    ".var",
    "snap",
    ".mozilla",
    ".librewolf",
    ".steam",
    ".wine",
    ".ssh",
    ".gnupg",
    ".password-store",
    ".pki",
    "Trash",
    ".venvs",
    ".local/share/Trash",
    "target",
    ".gradle",
    ".m2",
    ".android",
    ".java",
];

/// Kırık bağlantı taramasında tutulan en fazla öğe.
pub const BROKEN_LINK_LIMIT: usize = 2000;

fn modified(p: &Path) -> Option<DateTime<Utc>> {
    std::fs::symlink_metadata(p)
        .ok()?
        .modified()
        .ok()
        .map(DateTime::<Utc>::from)
}

/// Hedefi olmayan sembolik bağlantılar. Bağlantının kendisi silinir, hedef zaten yoktur.
pub fn broken_links(home: &Path) -> Vec<ScanItem> {
    let mut out = Vec::new();
    let walker = WalkDir::new(home)
        .follow_links(false)
        .max_depth(5)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 || !e.file_type().is_dir() {
                return true;
            }
            let rel = e.path().strip_prefix(home).unwrap_or(e.path());
            let name = e.file_name().to_string_lossy();
            !LINK_SKIP.iter().any(|s| *s == name || Path::new(s) == rel)
        });
    for e in walker.filter_map(Result::ok) {
        if !e.path_is_symlink() {
            continue;
        }
        if matches!(std::fs::metadata(e.path()), Err(err) if err.kind() == std::io::ErrorKind::NotFound)
        {
            out.push(ScanItem {
                path: e.path().to_path_buf(),
                bytes: 0,
                modified: modified(e.path()),
            });
            if out.len() >= BROKEN_LINK_LIMIT {
                break;
            }
        }
    }
    out
}

/// Bir programın var olup olmadığını PATH ve bilinen konumlarda arar.
pub struct ProgramIndex {
    dirs: Vec<PathBuf>,
    flatpak_roots: Vec<PathBuf>,
}

impl ProgramIndex {
    pub fn new(home: &Path) -> Self {
        let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).collect())
            .unwrap_or_default();
        for d in [
            "/usr/local/bin",
            "/usr/bin",
            "/bin",
            "/usr/sbin",
            "/sbin",
            "/usr/games",
            "/snap/bin",
            "/var/lib/flatpak/exports/bin",
        ] {
            dirs.push(d.into());
        }
        dirs.push(home.join(".local/bin"));
        dirs.push(home.join(".local/share/flatpak/exports/bin"));
        dirs.push(home.join("bin"));
        dirs.dedup();
        Self {
            dirs,
            flatpak_roots: vec![
                PathBuf::from("/var/lib/flatpak/app"),
                home.join(".local/share/flatpak/app"),
            ],
        }
    }

    #[cfg(test)]
    pub fn with_dirs(dirs: Vec<PathBuf>) -> Self {
        Self {
            dirs,
            flatpak_roots: vec![],
        }
    }

    pub fn exists(&self, program: &str) -> bool {
        if program.contains('/') {
            return Path::new(program).exists();
        }
        self.dirs.iter().any(|d| d.join(program).exists())
    }

    pub(crate) fn flatpak_installed(&self, app_id: &str) -> bool {
        self.flatpak_roots.iter().any(|r| r.join(app_id).exists())
    }
}

/// `.desktop` dosyasının çalıştırdığı programı bulur. Kısayol değilse (`Type=Link`) None.
pub fn desktop_program(text: &str) -> Option<DesktopTarget> {
    let mut in_entry = false;
    let mut exec = None;
    let mut try_exec = None;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        if let Some(v) = line.strip_prefix("TryExec=") {
            try_exec = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("Exec=") {
            exec.get_or_insert_with(|| v.trim().to_string());
        }
    }
    if let Some(t) = try_exec.filter(|t| !t.is_empty()) {
        return Some(DesktopTarget::Program(unquote(&t)));
    }
    let exec = exec?;
    let mut tokens = exec.split_whitespace().map(unquote).peekable();
    let mut first = tokens.next()?;
    // "env A=B prog …" biçimini atla.
    if first == "env" || first.ends_with("/env") {
        first = tokens.find(|t| !t.contains('=') && !t.starts_with('-'))?;
    }
    if first == "flatpak" || first.ends_with("/flatpak") {
        let rest: Vec<String> = tokens.collect();
        if rest.first().map(String::as_str) == Some("run") {
            if let Some(id) = rest.iter().skip(1).find(|t| !t.starts_with('-')) {
                return Some(DesktopTarget::Flatpak(id.clone()));
            }
        }
    }
    Some(DesktopTarget::Program(first))
}

#[derive(Debug, PartialEq, Eq)]
pub enum DesktopTarget {
    Program(String),
    Flatpak(String),
}

fn unquote(s: &str) -> String {
    s.trim_matches(|c| c == '"' || c == '\'').to_string()
}

/// Çalıştırdığı program artık olmayan menü ve otomatik başlatma kısayolları.
pub fn orphan_launchers(home: &Path, idx: &ProgramIndex) -> Vec<ScanItem> {
    let mut out = Vec::new();
    for dir in [
        home.join(".local/share/applications"),
        home.join(".config/autostart"),
    ] {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.filter_map(Result::ok) {
            let p = e.path();
            if p.extension().is_none_or(|x| x != "desktop") || p.is_symlink() {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&p) else {
                continue;
            };
            let orphan = match desktop_program(&text) {
                Some(DesktopTarget::Program(prog)) => !idx.exists(&prog),
                Some(DesktopTarget::Flatpak(id)) => !idx.flatpak_installed(&id),
                None => false,
            };
            if orphan {
                let bytes = e.metadata().map(|m| m.len()).unwrap_or(0);
                out.push(ScanItem {
                    path: p.clone(),
                    bytes,
                    modified: modified(&p),
                });
            }
        }
    }
    out
}

/// Masaüstü ortamlarına ve sisteme ait, hiçbir zaman "kalıntı" sayılmayacak klasörler.
const CONFIG_KEEP: &[&str] = &[
    "autostart",
    "gtk-2.0",
    "gtk-3.0",
    "gtk-4.0",
    "dconf",
    "pulse",
    "systemd",
    "menus",
    "fontconfig",
    "ibus",
    "nemo",
    "caja",
    "xfce4",
    "mate",
    "user-dirs",
    "environment.d",
    "bosun",
    "trash",
    "applications",
    "flatpak",
    "keyrings",
    "icons",
    "themes",
    "fonts",
    "recently-used",
    "gvfs-metadata",
    "tracker",
    "tracker3",
    "sounds",
    "mime",
    "desktop-directories",
    "xorg",
    "session",
    "pipewire",
    "wireplumber",
    "containers",
    "nautilus",
    "evolution",
    "goa-1.0",
    "gnome-session",
    "cinnamon",
    "cinnamon-session",
    "linuxmint",
    "mintupdate",
    "xapp",
    "xreader",
    "xviewer",
    "xed",
    "pix",
    "hypnotix",
    "warpinator",
    "timeshift",
    "baobab",
    "vlc",
    "libreoffice",
    "enchant",
    "speech-dispatcher",
    "configstore",
    "kdeconnect",
    "Trolltech.conf",
    "qt5ct",
    "qt6ct",
    "kdedefaults",
    "plasma-workspace",
    "kwalletd",
    "zeitgeist",
    "webkitgtk",
    "gstreamer-1.0",
    "babl-2.0",
    "gegl-0.4",
    "glib-2.0",
    "gnome-shell",
    "gnome-settings-daemon",
    "ubuntu-advantage",
    "update-notifier",
    "nvidia",
    "Mousepad",
    "orage",
    "Thunar",
    "xfce4-session",
    "lxsession",
    "openbox",
    "Code",
    "vscode",
    "JetBrains",
    "Google",
    "Microsoft",
    "Electron",
    "ms-playwright",
    "nvim",
    "vim",
    "git",
    "gh",
    "npm",
    "pnpm",
    "yarn",
    "pip",
    "uv",
    "rclone",
    "htop",
    "btop",
    "neofetch",
    "fastfetch",
];

const STALE_LEFTOVER_DAYS: u64 = 180;

fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
}

/// Kurulu yazılımların adları: paketler, PATH'teki programlar, .desktop kimlikleri, Flatpak ve Snap.
pub fn installed_names(home: &Path, idx: &ProgramIndex) -> HashSet<String> {
    let mut names = HashSet::new();
    if let Ok(o) = std::process::Command::new("dpkg-query")
        .args(["-W", "-f=${Package}\n"])
        .output()
    {
        names.extend(String::from_utf8_lossy(&o.stdout).lines().map(normalize));
    }
    for d in &idx.dirs {
        if let Ok(rd) = std::fs::read_dir(d) {
            names.extend(
                rd.filter_map(Result::ok)
                    .map(|e| normalize(&e.file_name().to_string_lossy())),
            );
        }
    }
    let app_dirs = [
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
        PathBuf::from("/var/lib/snapd/desktop/applications"),
        home.join(".local/share/applications"),
        home.join(".local/share/flatpak/exports/share/applications"),
    ];
    for d in app_dirs.iter().chain(idx.flatpak_roots.iter()) {
        if let Ok(rd) = std::fs::read_dir(d) {
            for e in rd.filter_map(Result::ok) {
                let n = e
                    .file_name()
                    .to_string_lossy()
                    .trim_end_matches(".desktop")
                    .to_string();
                names.insert(normalize(&n));
                // "org.gimp.GIMP" → "gimp" gibi parçalar da eşleşsin.
                names.extend(
                    n.split(['.', '_', '-'])
                        .filter(|p| p.len() >= 4)
                        .map(normalize),
                );
            }
        }
    }
    if let Ok(rd) = std::fs::read_dir("/snap") {
        names.extend(
            rd.filter_map(Result::ok)
                .map(|e| normalize(&e.file_name().to_string_lossy())),
        );
    }
    names.retain(|n| n.len() >= 3);
    names
}

/// Bir klasör adı kurulu bir yazılımla eşleşiyor mu? Şüphede "eşleşiyor" der (silmeye değil, korumaya yatkın).
pub fn matches_installed(dir_name: &str, installed: &HashSet<String>) -> bool {
    let n = normalize(dir_name);
    if n.len() < 3 {
        return true;
    }
    installed.iter().any(|i| {
        i == &n || (n.len() >= 4 && i.contains(&n)) || (i.len() >= 4 && n.contains(i.as_str()))
    })
}

fn newest_mtime(dir: &Path) -> Option<SystemTime> {
    WalkDir::new(dir)
        .follow_links(false)
        .max_depth(3)
        .into_iter()
        .filter_map(Result::ok)
        .filter_map(|e| e.metadata().ok()?.modified().ok())
        .max()
}

/// `~/.config` ve `~/.local/share` içinde kurulu hiçbir yazılımla eşleşmeyen ve uzun süredir
/// dokunulmamış klasörler. Bu bir tahmindir; arayüz bunları asla otomatik seçmez.
pub fn leftover_dirs(home: &Path, installed: &HashSet<String>) -> Vec<ScanItem> {
    let keep: HashSet<String> = CONFIG_KEEP.iter().map(|k| normalize(k)).collect();
    let mut out = Vec::new();
    for base in [home.join(".config"), home.join(".local/share")] {
        let Ok(rd) = std::fs::read_dir(&base) else {
            continue;
        };
        for e in rd.filter_map(Result::ok) {
            let p = e.path();
            let Ok(m) = p.symlink_metadata() else {
                continue;
            };
            if !m.is_dir() {
                continue;
            }
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.')
                || keep.contains(&normalize(&name))
                || matches_installed(&name, installed)
            {
                continue;
            }
            let stale = newest_mtime(&p)
                .and_then(|t| SystemTime::now().duration_since(t).ok())
                .is_some_and(|age| age > Duration::from_secs(STALE_LEFTOVER_DAYS * 86_400));
            if stale {
                let bytes = path_size(&p);
                out.push(ScanItem {
                    path: p.clone(),
                    bytes,
                    modified: modified(&p),
                });
            }
        }
    }
    out
}

/// Kaldırılmış ama ayar dosyaları duran paketler (dpkg "rc" durumu). Yol alanı paket adını taşır.
pub fn parse_residual(dpkg_status: &str) -> Vec<String> {
    dpkg_status
        .lines()
        .filter_map(|l| l.strip_prefix("rc "))
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

/// `apt-get -s autoremove` çıktısındaki "Remv paket [sürüm]" satırları.
pub fn parse_autoremove(sim: &str) -> Vec<String> {
    sim.lines()
        .filter_map(|l| l.strip_prefix("Remv "))
        .filter_map(|l| l.split_whitespace().next())
        .map(str::to_string)
        .collect()
}

fn package_items(names: Vec<String>) -> Vec<ScanItem> {
    if names.is_empty() {
        return vec![];
    }
    // Kurulu boyutlar (KiB); "rc" paketlerinde yoktur.
    let sizes: std::collections::HashMap<String, u64> = std::process::Command::new("dpkg-query")
        .arg("-W")
        .arg("-f=${Package} ${Installed-Size}\n")
        .args(&names)
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter_map(|l| {
                    let mut it = l.split_whitespace();
                    Some((
                        it.next()?.to_string(),
                        it.next()?.parse::<u64>().ok()? * 1024,
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    names
        .into_iter()
        .map(|n| ScanItem {
            bytes: sizes.get(&n).copied().unwrap_or(0),
            path: PathBuf::from(n),
            modified: None,
        })
        .collect()
}

pub fn residual_packages() -> Vec<ScanItem> {
    let out = std::process::Command::new("dpkg-query")
        .args(["-W", "-f=${db:Status-Abbrev} ${Package}\n"])
        .output();
    let names = out
        .map(|o| parse_residual(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default();
    // "rc " öneki iki karakterlik durum + boşluk; dpkg-query bunu "rc " olarak yazar.
    package_items(names)
}

pub fn orphan_packages() -> Vec<ScanItem> {
    let out = std::process::Command::new("apt-get")
        .args(["-s", "autoremove"])
        .env("LANG", "C")
        .output();
    let names = out
        .map(|o| parse_autoremove(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default();
    package_items(names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn finds_only_broken_links() {
        let d = tempfile::tempdir().unwrap();
        let h = d.path();
        fs::create_dir_all(h.join(".local/bin")).unwrap();
        fs::write(h.join("real"), b"x").unwrap();
        std::os::unix::fs::symlink(h.join("real"), h.join(".local/bin/ok")).unwrap();
        std::os::unix::fs::symlink(h.join("gone"), h.join(".local/bin/broken")).unwrap();
        fs::create_dir_all(h.join(".cache")).unwrap();
        std::os::unix::fs::symlink(h.join("gone2"), h.join(".cache/skipped")).unwrap();
        let found = broken_links(h);
        assert_eq!(found.len(), 1);
        assert!(found[0].path.ends_with(".local/bin/broken"));
    }

    #[test]
    fn parses_desktop_exec() {
        let t = "[Desktop Entry]\nName=X\nExec=env FOO=1 /opt/x/x --flag %U\n[Desktop Action new]\nExec=other\n";
        assert_eq!(
            desktop_program(t),
            Some(DesktopTarget::Program("/opt/x/x".into()))
        );
        let t = "[Desktop Entry]\nExec=/usr/bin/flatpak run --branch=stable org.gimp.GIMP %U\n";
        assert_eq!(
            desktop_program(t),
            Some(DesktopTarget::Flatpak("org.gimp.GIMP".into()))
        );
        let t = "[Desktop Entry]\nTryExec=foo\nExec=bar\n";
        assert_eq!(
            desktop_program(t),
            Some(DesktopTarget::Program("foo".into()))
        );
        assert_eq!(desktop_program("[Desktop Entry]\nType=Link\nURL=x\n"), None);
    }

    #[test]
    fn finds_orphan_launchers() {
        let d = tempfile::tempdir().unwrap();
        let h = d.path();
        let bin = h.join("bin");
        fs::create_dir_all(&bin).unwrap();
        fs::write(bin.join("present"), b"").unwrap();
        let apps = h.join(".local/share/applications");
        fs::create_dir_all(&apps).unwrap();
        fs::write(apps.join("a.desktop"), "[Desktop Entry]\nExec=present\n").unwrap();
        fs::write(
            apps.join("b.desktop"),
            "[Desktop Entry]\nExec=removed-app %F\n",
        )
        .unwrap();
        let idx = ProgramIndex::with_dirs(vec![bin]);
        let o = orphan_launchers(h, &idx);
        assert_eq!(o.len(), 1);
        assert!(o[0].path.ends_with("b.desktop"));
    }

    #[test]
    fn matching_is_conservative() {
        let installed: HashSet<String> = ["gimp", "googlechrome", "code", "obsstudio"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert!(matches_installed("GIMP", &installed));
        assert!(matches_installed("google-chrome", &installed));
        assert!(matches_installed("obs-studio", &installed));
        assert!(
            matches_installed("ab", &installed),
            "kısa adlar her zaman korunur"
        );
        assert!(!matches_installed("skypeforlinux", &installed));
    }

    #[test]
    fn leftover_dirs_need_age_and_no_match() {
        let d = tempfile::tempdir().unwrap();
        let h = d.path();
        fs::create_dir_all(h.join(".config/skypeforlinux")).unwrap();
        fs::create_dir_all(h.join(".config/gimp")).unwrap();
        fs::create_dir_all(h.join(".config/gtk-3.0")).unwrap();
        let installed: HashSet<String> = ["gimp".to_string()].into();
        // Taze klasörler kalıntı sayılmaz.
        assert!(leftover_dirs(h, &installed).is_empty());
    }

    #[test]
    fn parses_package_lists() {
        assert_eq!(
            parse_residual("ii bash\nrc skypeforlinux\nrc libfoo1\n"),
            vec!["skypeforlinux", "libfoo1"]
        );
        let sim = "Reading package lists...\nRemv libfoo1 [1.0-1]\nRemv linux-headers-6.8.0-40 [6.8.0-40.40]\n";
        assert_eq!(
            parse_autoremove(sim),
            vec!["libfoo1", "linux-headers-6.8.0-40"]
        );
    }
}
