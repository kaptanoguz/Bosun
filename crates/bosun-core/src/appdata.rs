//! `~/.config` altındaki uygulamaların yeniden oluşturulabilen verileri.
//!
//! Chromium tabanlı tarayıcılar (Brave, Brave Nightly, Chrome, Edge, Vivaldi…) ve Electron
//! uygulamaları (Claude, VS Code, Slack, Discord, Antigravity…) önbelleklerinin bir kısmını
//! `~/.cache` yerine ayar klasörlerinde tutar. Burada yalnızca adı bilinen ve uygulamanın
//! kendisinin yeniden oluşturduğu klasörler aranır; profiller, yer imleri, çerezler, şifreler,
//! eklentiler ve yerel depolama asla listelenmez.

use std::path::{Path, PathBuf};

/// Bir Chromium/Electron kullanıcı veri klasöründe ya da profilinde, içeriği güvenle
/// silinebilen alt klasörler. Uygulama bir sonraki açılışta bunları yeniden oluşturur.
pub const CHROMIUM_CACHE_DIRS: &[&str] = &[
    "Cache",
    "Code Cache",
    "GPUCache",
    "DawnCache",
    "DawnGraphiteCache",
    "DawnWebGPUCache",
    "GrShaderCache",
    "GraphiteDawnCache",
    "ShaderCache",
    "Service Worker/CacheStorage",
    "Service Worker/ScriptCache",
    "component_crx_cache",
    "extensions_crx_cache",
    "Crash Reports",
    // İndirilen yapay zekâ modelleri: Chrome'un cihaz üstü Gemini Nano modeli ve
    // öneri modelleri. Tarayıcı gerekirse yeniden indirir.
    "OptGuideOnDeviceModel",
    "optimization_guide_model_store",
    // VS Code ve türevleri (Cursor, Antigravity…)
    "CachedData",
    "CachedExtensionVSIXs",
    "CachedProfilesData",
];

/// Ev klasörüne göre, adıyla bilinen ve içeriği yeniden indirilen klasörler.
pub const KNOWN_DIRS: &[&str] = &[
    // Claude masaüstü uygulamasının sanal makine paketleri; gerektiğinde yeniden indirilir.
    ".config/Claude/vm_bundles",
];

/// Klasör bir Chromium/Electron kullanıcı veri klasörü ya da profili mi?
fn is_chromium_dir(dir: &Path) -> bool {
    dir.join("Local State").is_file() || dir.join("Preferences").is_file()
}

fn real_dir(p: &Path) -> bool {
    p.symlink_metadata().is_ok_and(|m| m.is_dir())
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| real_dir(p))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// Silinebilir verinin bulunduğu klasörler (içerikleri temizlenir, klasörün kendisi kalır).
pub fn roots(home: &Path) -> Vec<PathBuf> {
    let config = home.join(".config");
    let mut out: Vec<PathBuf> = KNOWN_DIRS
        .iter()
        .map(|d| home.join(d))
        .filter(|p| real_dir(p))
        .collect();

    // Kullanıcı veri klasörleri: ~/.config/<uygulama> ya da ~/.config/<üretici>/<uygulama>
    // (ör. BraveSoftware/Brave-Browser-Nightly).
    let mut data_dirs = Vec::new();
    for a in subdirs(&config) {
        if is_chromium_dir(&a) {
            data_dirs.push(a);
        } else {
            data_dirs.extend(subdirs(&a).into_iter().filter(|b| is_chromium_dir(b)));
        }
    }
    for d in data_dirs {
        // Kullanıcı veri klasörünün kendisi ve içindeki profiller (Default, Profile 1…).
        let mut places = vec![d.clone()];
        places.extend(
            subdirs(&d)
                .into_iter()
                .filter(|p| p.join("Preferences").is_file()),
        );
        for place in places {
            for name in CHROMIUM_CACHE_DIRS {
                let p = place.join(name);
                if real_dir(&p) && !out.contains(&p) {
                    out.push(p);
                }
            }
        }
    }
    out
}

/// Bir yolun hangi uygulamaya ait olduğu, kullanıcıya göstermek için
/// (ör. "BraveSoftware/Brave-Browser-Nightly").
pub fn app_name(path: &Path, home: &Path) -> Option<String> {
    let rest = path.strip_prefix(home.join(".config")).ok()?;
    let mut parts = rest.components().map(|c| c.as_os_str().to_string_lossy());
    let first = parts.next()?.into_owned();
    let second = parts.next().map(|s| s.into_owned());
    Some(match second {
        Some(s) if !is_chromium_dir(&home.join(".config").join(&first)) => {
            format!("{first}/{s}")
        }
        _ => first,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn finds_browser_and_electron_caches_but_not_profiles() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let cfg = home.join(".config");
        // Brave Nightly: üretici klasörü altında kullanıcı veri klasörü ve profil.
        let brave = cfg.join("BraveSoftware/Brave-Browser-Nightly");
        fs::create_dir_all(brave.join("Default/Code Cache/js")).unwrap();
        fs::create_dir_all(brave.join("Default/Service Worker/CacheStorage")).unwrap();
        fs::create_dir_all(brave.join("Default/Local Storage")).unwrap();
        fs::create_dir_all(brave.join("GrShaderCache")).unwrap();
        fs::write(brave.join("Local State"), "{}").unwrap();
        fs::write(brave.join("Default/Preferences"), "{}").unwrap();
        // Claude (Electron) ve sanal makine paketleri
        let claude = cfg.join("Claude");
        fs::create_dir_all(claude.join("vm_bundles")).unwrap();
        fs::create_dir_all(claude.join("Cache")).unwrap();
        fs::write(claude.join("Local State"), "{}").unwrap();
        // Chromium olmayan bir uygulamanın "Cache" klasörüne dokunulmaz.
        fs::create_dir_all(cfg.join("someapp/Cache")).unwrap();

        let r = roots(home);
        assert!(r.contains(&claude.join("vm_bundles")));
        assert!(r.contains(&claude.join("Cache")));
        assert!(r.contains(&brave.join("Default/Code Cache")));
        assert!(r.contains(&brave.join("Default/Service Worker/CacheStorage")));
        assert!(r.contains(&brave.join("GrShaderCache")));
        assert!(!r.iter().any(|p| p.ends_with("Local Storage")));
        assert!(!r.contains(&cfg.join("someapp/Cache")));
        assert_eq!(
            r.iter()
                .filter(|p| **p == claude.join("vm_bundles"))
                .count(),
            1
        );

        assert_eq!(
            app_name(&brave.join("Default/Code Cache"), home).as_deref(),
            Some("BraveSoftware/Brave-Browser-Nightly")
        );
        assert_eq!(
            app_name(&claude.join("vm_bundles"), home).as_deref(),
            Some("Claude")
        );
    }
}
