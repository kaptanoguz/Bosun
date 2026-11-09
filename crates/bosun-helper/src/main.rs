//! bosun-helper — Bosun'un yönetici yetkisi gerektiren işleri yapan yardımcısı.
//!
//! `pkexec /usr/lib/bosun/bosun-helper <eylem>` ile çalıştırılır. Güvenlik ilkeleri:
//! - Tek bir argüman kabul eder ve bu argüman sabit bir eylem listesinden olmalıdır.
//!   Yol, sayı ya da komut gibi serbest girdi **hiçbir şekilde** alınmaz.
//! - Çalıştırdığı her program mutlak yolla çağrılır; ortam değişkenleri temizlenir.
//! - Yazdığı dosyalar sabit adlıdır ve yalnızca `/etc/systemd/journald.conf.d` ile
//!   `/etc/sysctl.d` altındadır; geri alma eylemleri yalnızca bu dosyaları siler.
//! - Sonucu stdout'a tek satır JSON olarak yazar.

use std::path::Path;
use std::process::{Command, ExitCode};

const JOURNALD_DROPIN: &str = "/etc/systemd/journald.conf.d/60-bosun.conf";
const SYSCTL_DROPIN: &str = "/etc/sysctl.d/60-bosun-swappiness.conf";
const APT_ARCHIVES: &str = "/var/cache/apt/archives";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    JournalVacuum,
    AptClean,
    JournaldLimit,
    JournaldLimitUndo,
    SnapRetain,
    SnapRetainUndo,
    Swappiness,
    SwappinessUndo,
    FlatpakUnused,
    AptAutoremove,
    PurgeResidual,
}

impl Action {
    pub const ALL: [Action; 11] = [
        Action::JournalVacuum,
        Action::AptClean,
        Action::JournaldLimit,
        Action::JournaldLimitUndo,
        Action::SnapRetain,
        Action::SnapRetainUndo,
        Action::Swappiness,
        Action::SwappinessUndo,
        Action::FlatpakUnused,
        Action::AptAutoremove,
        Action::PurgeResidual,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Action::JournalVacuum => "journal-vacuum",
            Action::AptClean => "apt-clean",
            Action::JournaldLimit => "journald-limit",
            Action::JournaldLimitUndo => "journald-limit-undo",
            Action::SnapRetain => "snap-retain",
            Action::SnapRetainUndo => "snap-retain-undo",
            Action::Swappiness => "swappiness",
            Action::SwappinessUndo => "swappiness-undo",
            Action::FlatpakUnused => "flatpak-unused",
            Action::AptAutoremove => "apt-autoremove",
            Action::PurgeResidual => "purge-residual",
        }
    }

    pub fn parse(s: &str) -> Option<Action> {
        Action::ALL.into_iter().find(|a| a.id() == s)
    }
}

struct Outcome {
    message: String,
    freed_bytes: u64,
}

fn run(program: &str, args: &[&str]) -> Result<String, String> {
    run_env(program, args, &[])
}

fn run_env(program: &str, args: &[&str], extra_env: &[(&str, &str)]) -> Result<String, String> {
    let out = Command::new(program)
        .args(args)
        .env_clear()
        .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
        .env("LANG", "C.UTF-8")
        .envs(extra_env.iter().copied())
        .output()
        .map_err(|e| format!("{program}: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    if out.status.success() {
        Ok(text)
    } else {
        Err(format!("{program} {}: {}", args.join(" "), text.trim()))
    }
}

/// `journalctl --disk-usage` çıktısından bayt: "... take up 1.2G in the file system."
pub fn parse_journal_usage(text: &str) -> Option<u64> {
    let idx = text.find("take up ")? + "take up ".len();
    let token = text[idx..].split_whitespace().next()?;
    let split = token.find(|c: char| c.is_ascii_alphabetic()).unwrap_or(token.len());
    let (num, unit) = token.split_at(split);
    let value: f64 = num.parse().ok()?;
    let mult = match unit.chars().next().map(|c| c.to_ascii_uppercase()) {
        None | Some('B') => 1.0,
        Some('K') => 1024.0,
        Some('M') => 1024.0 * 1024.0,
        Some('G') => 1024.0 * 1024.0 * 1024.0,
        Some('T') => 1024f64.powi(4),
        _ => return None,
    };
    Some((value * mult) as u64)
}

fn journal_usage() -> u64 {
    run("/usr/bin/journalctl", &["--disk-usage"]).ok().and_then(|t| parse_journal_usage(&t)).unwrap_or(0)
}

fn deb_bytes(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .filter(|e| e.path().extension().is_some_and(|x| x == "deb"))
                .filter_map(|e| e.metadata().ok())
                .map(|m| m.len())
                .sum()
        })
        .unwrap_or(0)
}

/// Bir dizinin toplam boyutu (sembolik bağlar takip edilmez).
fn dir_bytes(dir: &Path) -> u64 {
    let Ok(rd) = std::fs::read_dir(dir) else { return 0 };
    rd.filter_map(Result::ok)
        .map(|e| match e.path().symlink_metadata() {
            Ok(m) if m.is_dir() => dir_bytes(&e.path()),
            Ok(m) if m.is_file() => m.len(),
            _ => 0,
        })
        .sum()
}

fn write_file(path: &str, content: &str) -> Result<(), String> {
    let p = Path::new(path);
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(p, content).map_err(|e| format!("{path}: {e}"))
}

fn remove_file(path: &str) -> Result<bool, String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(format!("{path}: {e}")),
    }
}

fn perform(action: Action) -> Result<Outcome, String> {
    match action {
        Action::JournalVacuum => {
            let before = journal_usage();
            run("/usr/bin/journalctl", &["--vacuum-size=200M"])?;
            let after = journal_usage();
            Ok(Outcome { message: "journal vacuumed to 200M".into(), freed_bytes: before.saturating_sub(after) })
        }
        Action::AptClean => {
            let before = deb_bytes(Path::new(APT_ARCHIVES));
            run("/usr/bin/apt-get", &["clean"])?;
            let after = deb_bytes(Path::new(APT_ARCHIVES));
            Ok(Outcome { message: "apt cache cleaned".into(), freed_bytes: before.saturating_sub(after) })
        }
        Action::JournaldLimit => {
            write_file(JOURNALD_DROPIN, "# Bosun tarafından yazıldı; Bosun > Ayarlar'dan geri alınabilir.\n[Journal]\nSystemMaxUse=500M\n")?;
            run("/usr/bin/systemctl", &["restart", "systemd-journald"])?;
            Ok(Outcome { message: "journald limited to 500M".into(), freed_bytes: 0 })
        }
        Action::JournaldLimitUndo => {
            remove_file(JOURNALD_DROPIN)?;
            run("/usr/bin/systemctl", &["restart", "systemd-journald"])?;
            Ok(Outcome { message: "journald limit removed".into(), freed_bytes: 0 })
        }
        Action::SnapRetain | Action::SnapRetainUndo => {
            if !Path::new("/usr/bin/snap").exists() {
                return Err("snap is not installed".into());
            }
            let value = if action == Action::SnapRetain { "refresh.retain=2" } else { "refresh.retain=3" };
            run("/usr/bin/snap", &["set", "system", value])?;
            Ok(Outcome { message: format!("snap {value}"), freed_bytes: 0 })
        }
        Action::Swappiness => {
            write_file(SYSCTL_DROPIN, "# Bosun tarafından yazıldı; Bosun > Ayarlar'dan geri alınabilir.\nvm.swappiness=20\n")?;
            run("/usr/sbin/sysctl", &["-q", "-w", "vm.swappiness=20"])?;
            Ok(Outcome { message: "vm.swappiness=20".into(), freed_bytes: 0 })
        }
        Action::FlatpakUnused => {
            if !Path::new("/usr/bin/flatpak").exists() {
                return Err("flatpak is not installed".into());
            }
            let before = dir_bytes(Path::new("/var/lib/flatpak/runtime"));
            run("/usr/bin/flatpak", &["uninstall", "--system", "--unused", "--assumeyes", "--noninteractive"])?;
            let after = dir_bytes(Path::new("/var/lib/flatpak/runtime"));
            Ok(Outcome { message: "unused flatpak runtimes removed".into(), freed_bytes: before.saturating_sub(after) })
        }
        Action::AptAutoremove => {
            let out = run_env("/usr/bin/apt-get", &["autoremove", "--purge", "-y"], &[("DEBIAN_FRONTEND", "noninteractive")])?;
            let removed = out.lines().filter(|l| l.starts_with("Removing ") || l.starts_with("Purging ")).count();
            Ok(Outcome { message: format!("{removed} package steps completed"), freed_bytes: 0 })
        }
        Action::PurgeResidual => {
            let status = run("/usr/bin/dpkg-query", &["-W", "-f=${db:Status-Abbrev} ${Package}\n"])?;
            // Paket adları dpkg'nin kendisinden gelir; yine de yalnızca geçerli karakterleri kabul et.
            let names: Vec<&str> = status
                .lines()
                .filter_map(|l| l.strip_prefix("rc "))
                .map(str::trim)
                .filter(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_alphanumeric() || "+-.:".contains(c)))
                .collect();
            if names.is_empty() {
                return Ok(Outcome { message: "nothing to purge".into(), freed_bytes: 0 });
            }
            let mut args = vec!["--purge"];
            args.extend(names.iter().copied());
            run("/usr/bin/dpkg", &args)?;
            Ok(Outcome { message: format!("{} packages purged", names.len()), freed_bytes: 0 })
        }
        Action::SwappinessUndo => {
            remove_file(SYSCTL_DROPIN)?;
            // Dağıtım varsayılanına dön.
            run("/usr/sbin/sysctl", &["-q", "-w", "vm.swappiness=60"])?;
            Ok(Outcome { message: "vm.swappiness=60".into(), freed_bytes: 0 })
        }
    }
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn is_root() -> bool {
    // Etkin UID'yi /proc'tan oku (libc bağımlılığı olmadan).
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("Uid:")).map(|l| l.split_whitespace().nth(2) == Some("0")))
        .unwrap_or(false)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() == 1 && args[0] == "--version" {
        println!("bosun-helper {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    let action = match args.as_slice() {
        [a] => Action::parse(a),
        _ => None,
    };
    let Some(action) = action else {
        let ids: Vec<&str> = Action::ALL.iter().map(|a| a.id()).collect();
        println!("{{\"ok\":false,\"error\":{}}}", json_str(&format!("usage: bosun-helper <{}>", ids.join("|"))));
        return ExitCode::from(2);
    };
    if !is_root() {
        println!("{{\"ok\":false,\"action\":{},\"error\":\"must run as root via pkexec\"}}", json_str(action.id()));
        return ExitCode::from(3);
    }
    match perform(action) {
        Ok(o) => {
            println!(
                "{{\"ok\":true,\"action\":{},\"message\":{},\"freed_bytes\":{}}}",
                json_str(action.id()),
                json_str(&o.message),
                o.freed_bytes
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            println!("{{\"ok\":false,\"action\":{},\"error\":{}}}", json_str(action.id()), json_str(&e));
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_known_actions_parse() {
        for a in Action::ALL {
            assert_eq!(Action::parse(a.id()), Some(a));
        }
        for bad in ["", "rm", "apt-clean; rm -rf /", "../journal-vacuum", "APT-CLEAN", "journal-vacuum "] {
            assert_eq!(Action::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn every_core_action_is_supported() {
        // Arayüzün/servisin bildiği her eylem yardımcıda da olmalı.
        for id in bosun_core::admin::ALL_IDS {
            assert!(Action::parse(id).is_some(), "yardımcı {id} eylemini bilmiyor");
        }
    }

    #[test]
    fn json_escaping() {
        assert_eq!(json_str("a\"b\\c\nd"), r#""a\"b\\c\nd""#);
    }

    #[test]
    fn journal_usage_parses() {
        assert_eq!(parse_journal_usage("Archived and active journals take up 2.0G in the file system."), Some(2 * 1024 * 1024 * 1024));
    }
}
