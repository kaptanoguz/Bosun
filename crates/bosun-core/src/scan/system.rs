//! Sistem tarafı ölçümler: journald boyutu ve kullanılmayan Flatpak çalışma ortamları.

use super::ScanItem;
use crate::util::path_size;
use std::path::Path;

/// `journalctl --disk-usage` çıktısını ayrıştırır: "... take up 1.2G in the file system."
pub fn parse_journal_usage(text: &str) -> Option<u64> {
    let idx = text.find("take up ")? + "take up ".len();
    let token = text[idx..].split_whitespace().next()?;
    let (num, unit) = token.split_at(
        token
            .find(|c: char| c.is_ascii_alphabetic())
            .unwrap_or(token.len()),
    );
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

pub(super) fn journal_bytes() -> u64 {
    std::process::Command::new("journalctl")
        .arg("--disk-usage")
        .output()
        .ok()
        .and_then(|o| parse_journal_usage(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_else(|| path_size(Path::new("/var/log/journal")))
}

/// `flatpak list --app --columns=application,runtime` satırları: "org.gimp.GIMP\torg.gnome.Platform/x86_64/46"
/// (yalnızca çalışma ortamı sütunu da kabul edilir).
/// `flatpak list --runtime --columns=application,arch,branch` satırları: "org.gnome.Platform\tx86_64\t46".
/// Kullanılmayan çalışma ortamlarını *tahmin eder*: hiçbir uygulamanın çalışma ortamı olmayan ve
/// kullanılan bir ortamın eklentisi (ör. `.GL.default`, `.Locale`) olmayanlar. Kesin karar,
/// kaldırma sırasında flatpak'in kendisine aittir.
/// `flatpak pin --system` çıktısındaki desenler (ör. "runtime/org.gnome.Platform/x86_64/44").
/// Sabitlenmiş ortamları `flatpak uninstall --unused` asla silmez; onları listelemeyiz.
pub fn parse_pins(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| l.starts_with("runtime/"))
        .map(str::to_string)
        .collect()
}

/// Flatpak'in desen eşleşmesi: parçalar '/' ile ayrılır, boş ya da '*' her şeye uyar.
pub fn pin_matches(pin: &str, id: &str, arch: &str, branch: &str) -> bool {
    let mut p = pin.trim_start_matches("runtime/").split('/');
    let ok = |pat: Option<&str>, v: &str| match pat {
        None | Some("") | Some("*") => true,
        Some(x) if x.ends_with('*') => v.starts_with(&x[..x.len() - 1]),
        Some(x) => x == v,
    };
    ok(p.next(), id) && ok(p.next(), arch) && ok(p.next(), branch)
}

/// Flatpak'in kendiliğinden kurup yönettiği eklenti türleri (kimlikte geçen parçalar).
const AUTO_MANAGED: &[&str] = &[
    ".GL.",
    ".GL32.",
    ".VAAPI.",
    ".codecs",
    "ffmpeg",
    "openh264",
    ".Locale",
    "Gtk3theme.",
    "KStyle.",
    ".Codecs",
];

pub fn estimate_unused_runtimes(apps: &str, runtimes: &str) -> Vec<(String, String, String)> {
    // Uygulama kimlikleri: "com.github.rafostar.Clapper.Enhancers" gibi uygulama eklentileri için.
    let app_ids: Vec<String> = apps
        .lines()
        .filter_map(|l| l.split('\t').next())
        .map(str::trim)
        .filter(|id| !id.is_empty() && !id.contains('/'))
        .map(str::to_string)
        .collect();
    let used: Vec<(String, String)> = apps
        .lines()
        .filter_map(|l| {
            let rt = l.split('\t').map(str::trim).find(|c| c.contains('/'))?;
            let mut p = rt.split('/');
            let id = p.next()?.trim();
            let _arch = p.next()?;
            let branch = p.next()?.trim();
            (!id.is_empty()).then(|| (id.to_string(), branch.to_string()))
        })
        .collect();
    runtimes
        .lines()
        .filter_map(|l| {
            let cols: Vec<&str> = l.split('\t').map(str::trim).collect();
            match cols.as_slice() {
                [id, arch, branch, ..] if !id.is_empty() => {
                    Some((id.to_string(), arch.to_string(), branch.to_string()))
                }
                _ => None,
            }
        })
        .filter(|(id, _, branch)| {
            let direct = used.iter().any(|(u, b)| u == id && b == branch);
            // Kullanılan bir ortamın eklentisi ya da yerelleştirmesi mi? (aynı kök kimliği paylaşır)
            let extension = used.iter().any(|(u, _)| id.starts_with(&format!("{u}.")));
            let shared_base = used.iter().any(|(u, _)| {
                let base = u.rsplit_once('.').map(|(b, _)| b).unwrap_or(u);
                id.starts_with(&format!("{base}."))
                    && (id.contains(".GL")
                        || id.ends_with(".Locale")
                        || id.contains(".Codecs")
                        || id.contains("openh264"))
            });
            let app_extension = app_ids.iter().any(|a| id.starts_with(&format!("{a}.")));
            // GTK temaları, GPU sürücüleri (GL, VAAPI) ve codec eklentileri Flatpak'in uygulamalar
            // için kendiliğinden yönettiği eklentilerdir; uygulama varken bunları asla "kullanılmıyor" sayma.
            let auto_managed = AUTO_MANAGED.iter().any(|m| id.contains(m)) && !used.is_empty();
            !direct && !extension && !shared_base && !app_extension && !auto_managed
        })
        .collect()
}

pub(super) fn flatpak_unused(root: &Path) -> Vec<ScanItem> {
    let run = |args: &[&str]| {
        std::process::Command::new("flatpak")
            .args(args)
            .env("LANG", "C.UTF-8")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
    };
    let (Some(apps), Some(runtimes)) = (
        run(&["list", "--system", "--app", "--columns=application,runtime"]),
        run(&[
            "list",
            "--system",
            "--runtime",
            "--columns=application,arch,branch",
        ]),
    ) else {
        return vec![];
    };
    let pins = run(&["pin", "--system"])
        .map(|t| parse_pins(&t))
        .unwrap_or_default();
    estimate_unused_runtimes(&apps, &runtimes)
        .into_iter()
        .filter(|(id, arch, branch)| !pins.iter().any(|p| pin_matches(p, id, arch, branch)))
        // Kaldırılmış ama klasörü henüz temizlenmemiş ortamların "active" bağlantısı yoktur.
        .filter(|(id, arch, branch)| {
            root.join(id)
                .join(arch)
                .join(branch)
                .join("active")
                .exists()
        })
        .map(|(id, arch, branch)| {
            let path = root.join(&id).join(&arch).join(&branch);
            ScanItem {
                bytes: path_size(&path),
                path,
                modified: None,
            }
        })
        .filter(|i| i.bytes > 0)
        .collect()
}
