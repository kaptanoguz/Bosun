//! Oturum açılışında başlayan uygulamalar (XDG autostart).
//!
//! Bosun bu girdileri yalnızca listeler ve kullanıcı istediğinde **açar/kapatır**: hiçbir dosya
//! silinmez. Kapatmak `~/.config/autostart/<ad>.desktop` içine `Hidden=true` ve
//! `X-GNOME-Autostart-enabled=false` yazar (sistem girdisi için kullanıcı kopyası oluşturur);
//! açmak bunları geri alır. Masaüstünün çalışması için gereken girdiler kapatılamaz.

use crate::leftovers::{desktop_program, DesktopTarget, ProgramIndex};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StartupApp {
    /// Dosya adı (`nm-applet.desktop`); açma/kapama bununla yapılır.
    pub file: String,
    pub name: String,
    pub comment: String,
    pub exec: String,
    pub enabled: bool,
    /// Sistem (/etc/xdg/autostart) girdisi mi, yoksa kullanıcının kendi girdisi mi.
    pub system: bool,
    /// Masaüstünün parçası (anahtar zinciri, polkit ajanı, ayar servisi…): kapatılamaz.
    pub essential: bool,
    /// Çalıştırdığı program artık yok.
    pub missing: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum AutostartError {
    #[error("unknown startup entry: {0}")]
    Unknown(String),
    #[error("{0} is part of the desktop and cannot be turned off")]
    Essential(String),
    #[error("could not write {path}: {err}")]
    Write { path: PathBuf, err: std::io::Error },
}

/// Kapatılması masaüstünü bozabilecek girdiler (dosya adında geçen parçalar).
const ESSENTIAL: &[&str] = &[
    "polkit",
    "keyring",
    "at-spi",
    "xdg-user-dirs",
    "pulseaudio",
    "pipewire",
    "settings-daemon",
    "org.gnome.settingsdaemon",
    "gsd-",
    "xfsettingsd",
    "xfce4-notifyd",
    "notification-daemon",
    "cinnamon-",
    "mate-",
    "spice-vdagent",
    "snap-userd",
    "ibus",
    "fcitx",
    "geoclue",
    "user-dirs",
    "xembedsniproxy",
    "plasma",
];

fn is_essential(file: &str) -> bool {
    let f = file.to_ascii_lowercase();
    ESSENTIAL.iter().any(|e| f.contains(e))
}

pub fn user_dir(home: &Path) -> PathBuf {
    home.join(".config/autostart")
}

pub const SYSTEM_DIR: &str = "/etc/xdg/autostart";

/// `XDG_CURRENT_DESKTOP` ("X-Cinnamon", "ubuntu:GNOME" …), küçük harfle.
pub fn current_desktops() -> Vec<String> {
    std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase())
        .collect()
}

/// `[Desktop Entry]` bölümündeki anahtarlar (yerelleştirilmiş anahtarlar hariç).
fn entry_keys(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut in_entry = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            out.entry(k.trim().to_string())
                .or_insert_with(|| v.trim().to_string());
        }
    }
    out
}

fn shown_in(keys: &BTreeMap<String, String>, desktops: &[String]) -> bool {
    if desktops.is_empty() {
        return true;
    }
    let list = |k: &str| -> Vec<String> {
        keys.get(k)
            .map(|v| {
                v.split(';')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_ascii_lowercase())
                    .collect()
            })
            .unwrap_or_default()
    };
    let only = list("OnlyShowIn");
    if !only.is_empty() && !only.iter().any(|d| desktops.contains(d)) {
        return false;
    }
    !list("NotShowIn").iter().any(|d| desktops.contains(d))
}

fn enabled(keys: &BTreeMap<String, String>) -> bool {
    let is_true = |k: &str| keys.get(k).is_some_and(|v| v.eq_ignore_ascii_case("true"));
    let is_false = |k: &str| keys.get(k).is_some_and(|v| v.eq_ignore_ascii_case("false"));
    !is_true("Hidden") && !is_false("X-GNOME-Autostart-enabled")
}

fn desktop_files(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return vec![];
    };
    rd.filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "desktop") && p.is_file())
        .filter_map(|p| Some((p.file_name()?.to_string_lossy().into_owned(), p)))
        .collect()
}

/// Bu masaüstünde oturum açılışında başlayan (ya da kapatılmış) girdiler: önce kullanıcınınkiler.
pub fn list(home: &Path, system_dir: &Path, desktops: &[String]) -> Vec<StartupApp> {
    let idx = ProgramIndex::new(home);
    let mut by_file: BTreeMap<String, (PathBuf, bool)> = BTreeMap::new();
    for (f, p) in desktop_files(system_dir) {
        by_file.insert(f, (p, true));
    }
    for (f, p) in desktop_files(&user_dir(home)) {
        // Kullanıcı kopyası sistem girdisini geçersiz kılar; ama girdi yine de sistemindir.
        let system = by_file.contains_key(&f);
        by_file.insert(f, (p, system));
    }
    let mut out: Vec<StartupApp> = by_file
        .into_iter()
        .filter_map(|(file, (path, system))| {
            let text = std::fs::read_to_string(&path).ok()?;
            let keys = entry_keys(&text);
            if !shown_in(&keys, desktops) || keys.get("Type").is_some_and(|t| t != "Application") {
                return None;
            }
            let missing = match desktop_program(&text) {
                Some(DesktopTarget::Program(p)) => !idx.exists(&p),
                Some(DesktopTarget::Flatpak(id)) => !idx.flatpak_installed(&id),
                None => false,
            };
            Some(StartupApp {
                name: keys
                    .get("Name")
                    .cloned()
                    .unwrap_or_else(|| file.trim_end_matches(".desktop").to_string()),
                comment: keys.get("Comment").cloned().unwrap_or_default(),
                exec: keys.get("Exec").cloned().unwrap_or_default(),
                enabled: enabled(&keys),
                essential: is_essential(&file),
                system,
                missing,
                file,
            })
        })
        .collect();
    out.sort_by(|a, b| {
        (a.system, a.essential, a.name.to_lowercase()).cmp(&(
            b.system,
            b.essential,
            b.name.to_lowercase(),
        ))
    });
    out
}

/// `[Desktop Entry]` bölümünde açma/kapama anahtarlarını yeniden yazar; diğer her şey aynı kalır.
pub fn with_enabled(text: &str, on: bool) -> String {
    let mut out = Vec::new();
    let mut in_entry = false;
    let mut written = false;
    let keys = [
        format!("Hidden={}", !on),
        format!("X-GNOME-Autostart-enabled={on}"),
    ];
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            if in_entry && !written {
                // Bölüm bitti: anahtarları boş satırlardan önce ekle.
                while out.last().is_some_and(|l: &String| l.trim().is_empty()) {
                    out.pop();
                }
                out.extend(keys.iter().cloned());
                out.push(String::new());
                written = true;
            }
            in_entry = t == "[Desktop Entry]";
            out.push(line.to_string());
            continue;
        }
        if in_entry && (t.starts_with("Hidden=") || t.starts_with("X-GNOME-Autostart-enabled=")) {
            continue;
        }
        out.push(line.to_string());
    }
    if !written {
        if !in_entry {
            out.push("[Desktop Entry]".into());
        }
        out.extend(keys.iter().cloned());
    }
    let mut s = out.join("\n");
    s.push('\n');
    s
}

/// Bir girdiyi açar ya da kapatır. Yalnızca `list` içinde görünen dosya adları kabul edilir.
pub fn set_enabled(
    home: &Path,
    system_dir: &Path,
    desktops: &[String],
    file: &str,
    on: bool,
) -> Result<StartupApp, AutostartError> {
    let entry = list(home, system_dir, desktops)
        .into_iter()
        .find(|a| a.file == file)
        .ok_or_else(|| AutostartError::Unknown(file.into()))?;
    if !on && entry.essential {
        return Err(AutostartError::Essential(entry.name));
    }
    let dir = user_dir(home);
    let target = dir.join(file);
    let source = if target.is_file() {
        target.clone()
    } else {
        system_dir.join(file)
    };
    let werr = |err| AutostartError::Write {
        path: target.clone(),
        err,
    };
    if target.is_symlink() {
        return Err(werr(std::io::Error::other("is a symbolic link")));
    }
    let text = std::fs::read_to_string(&source).map_err(werr)?;
    std::fs::create_dir_all(&dir).map_err(werr)?;
    let tmp = dir.join(format!(".{file}.bosun-tmp"));
    std::fs::write(&tmp, with_enabled(&text, on)).map_err(werr)?;
    std::fs::rename(&tmp, &target).map_err(werr)?;
    list(home, system_dir, desktops)
        .into_iter()
        .find(|a| a.file == file)
        .ok_or_else(|| AutostartError::Unknown(file.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let d = tempfile::tempdir().unwrap();
        let home = d.path().join("home");
        let sys = d.path().join("xdg");
        std::fs::create_dir_all(user_dir(&home)).unwrap();
        std::fs::create_dir_all(&sys).unwrap();
        std::fs::write(
            sys.join("nm-applet.desktop"),
            "[Desktop Entry]\nType=Application\nName=Network\nExec=sh\n",
        )
        .unwrap();
        std::fs::write(
            sys.join("polkit-agent.desktop"),
            "[Desktop Entry]\nType=Application\nName=Polkit\nExec=sh\n",
        )
        .unwrap();
        std::fs::write(
            sys.join("kde-only.desktop"),
            "[Desktop Entry]\nType=Application\nName=KDE thing\nExec=sh\nOnlyShowIn=KDE;\n",
        )
        .unwrap();
        std::fs::write(
            user_dir(&home).join("discord.desktop"),
            "[Desktop Entry]\nType=Application\nName=Discord\nExec=/nonexistent/discord\n\n[Desktop Action x]\nName=X\n",
        )
        .unwrap();
        (d, home, sys)
    }

    #[test]
    fn lists_entries_for_this_desktop() {
        let (_d, home, sys) = setup();
        let apps = list(&home, &sys, &["x-cinnamon".into()]);
        let names: Vec<_> = apps.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["Discord", "Network", "Polkit"]);
        assert!(apps[0].missing && !apps[0].system);
        assert!(apps[2].essential);
        assert!(apps.iter().all(|a| a.enabled));
    }

    #[test]
    fn toggles_with_a_user_copy() {
        let (_d, home, sys) = setup();
        let ds = vec!["x-cinnamon".to_string()];
        let a = set_enabled(&home, &sys, &ds, "nm-applet.desktop", false).unwrap();
        assert!(!a.enabled && a.system);
        // Sistem dosyasına dokunulmadı.
        let orig = std::fs::read_to_string(sys.join("nm-applet.desktop")).unwrap();
        assert!(!orig.contains("Hidden"));
        let a = set_enabled(&home, &sys, &ds, "nm-applet.desktop", true).unwrap();
        assert!(a.enabled);
        let user = std::fs::read_to_string(user_dir(&home).join("nm-applet.desktop")).unwrap();
        assert_eq!(user.matches("Hidden=").count(), 1);
    }

    #[test]
    fn keeps_other_sections() {
        let text = "[Desktop Entry]\nName=Discord\nHidden=false\n\n[Desktop Action x]\nName=X\n";
        let off = with_enabled(text, false);
        assert_eq!(
            off,
            "[Desktop Entry]\nName=Discord\nHidden=true\nX-GNOME-Autostart-enabled=false\n\n[Desktop Action x]\nName=X\n"
        );
        assert!(enabled(&entry_keys(&with_enabled(&off, true))));
    }

    #[test]
    fn refuses_essential_and_unknown() {
        let (_d, home, sys) = setup();
        assert!(matches!(
            set_enabled(&home, &sys, &[], "polkit-agent.desktop", false),
            Err(AutostartError::Essential(_))
        ));
        assert!(set_enabled(&home, &sys, &[], "polkit-agent.desktop", true).is_ok());
        for bad in ["../x.desktop", "nope.desktop", "kde-only.desktop"] {
            assert!(set_enabled(&home, &sys, &["x-cinnamon".into()], bad, false).is_err());
        }
    }
}
