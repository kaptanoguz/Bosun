//! Docker ve Podman: yalnızca **sarkan (dangling) imajlar** ve **derleme önbelleği**.
//! Etiketli imajlara, kapsayıcılara ve birimlere (volume) asla dokunulmaz.
//! Komutlar sabittir ve kullanıcının kendi yetkisiyle çalışır (root gerekmez; Docker için
//! kullanıcının `docker` grubunda olması gerekir).

use crate::scan::ScanItem;
use std::path::PathBuf;
use std::process::Command;

/// Öğe kimliği → çalıştırılacak sabit komut. Plan öğelerinin "yolu" bu kimliklerden biridir.
pub const ACTIONS: &[(&str, &[&str])] = &[
    (
        "docker:dangling-images",
        &["docker", "image", "prune", "--force"],
    ),
    (
        "docker:build-cache",
        &["docker", "builder", "prune", "--force"],
    ),
    (
        "podman:dangling-images",
        &["podman", "image", "prune", "--force"],
    ),
];

pub fn command_for(id: &str) -> Option<&'static [&'static str]> {
    ACTIONS.iter().find(|(k, _)| *k == id).map(|(_, c)| *c)
}

/// "1.234GB", "512.3 MB", "0B", "12kB" → bayt (Docker/Podman ondalık birim kullanır).
pub fn parse_size(s: &str) -> Option<u64> {
    let s = s.trim();
    let split = s.find(|c: char| c.is_ascii_alphabetic())?;
    let (num, unit) = s.split_at(split);
    let v: f64 = num.trim().parse().ok()?;
    let mult = match unit.trim().to_ascii_uppercase().as_str() {
        "B" => 1.0,
        "KB" => 1e3,
        "MB" => 1e6,
        "GB" => 1e9,
        "TB" => 1e12,
        _ => return None,
    };
    Some((v * mult) as u64)
}

fn output(cmd: &str, args: &[&str]) -> Option<String> {
    let o = Command::new(cmd)
        .args(args)
        .env("LANG", "C")
        .output()
        .ok()?;
    o.status
        .success()
        .then(|| String::from_utf8_lossy(&o.stdout).into_owned())
}

fn sum_sizes(text: &str) -> (usize, u64) {
    let sizes: Vec<u64> = text.lines().filter_map(parse_size).collect();
    (sizes.len(), sizes.iter().sum())
}

/// `docker system df` satırlarından "Build Cache" geri kazanılabilir boyutu: "Build Cache\t1.2GB (100%)".
pub fn parse_build_cache(df: &str) -> u64 {
    df.lines()
        .find_map(|l| l.strip_prefix("Build Cache\t"))
        .and_then(|r| r.split_whitespace().next())
        .and_then(parse_size)
        .unwrap_or(0)
}

pub fn scan() -> Vec<ScanItem> {
    let mut out = Vec::new();
    let item = |id: &str, bytes: u64| ScanItem {
        path: PathBuf::from(id),
        bytes,
        modified: None,
    };
    if let Some(t) = output(
        "docker",
        &[
            "image",
            "ls",
            "--filter",
            "dangling=true",
            "--format",
            "{{.Size}}",
        ],
    ) {
        let (n, bytes) = sum_sizes(&t);
        if n > 0 {
            out.push(item("docker:dangling-images", bytes));
        }
    }
    if let Some(t) = output(
        "docker",
        &["system", "df", "--format", "{{.Type}}\t{{.Reclaimable}}"],
    ) {
        let bytes = parse_build_cache(&t);
        if bytes > 0 {
            out.push(item("docker:build-cache", bytes));
        }
    }
    if let Some(t) = output(
        "podman",
        &[
            "images",
            "--filter",
            "dangling=true",
            "--format",
            "{{.Size}}",
        ],
    ) {
        let (n, bytes) = sum_sizes(&t);
        if n > 0 {
            out.push(item("podman:dangling-images", bytes));
        }
    }
    out
}

/// Bir kapsayıcı öğesini temizler (yalnızca `ACTIONS` içindeki kimlikler).
pub fn run(id: &str) -> Result<(), String> {
    let cmd = command_for(id).ok_or_else(|| format!("unknown container action: {id}"))?;
    let o = Command::new(cmd[0])
        .args(&cmd[1..])
        .output()
        .map_err(|e| format!("{}: {e}", cmd[0]))?;
    if o.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&o.stderr);
        if err.contains("permission denied") {
            Err(format!(
                "{}: permission denied (add your user to the docker group)",
                cmd[0]
            ))
        } else {
            Err(format!("{}: {}", cmd.join(" "), err.trim()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sizes() {
        assert_eq!(parse_size("1.5GB"), Some(1_500_000_000));
        assert_eq!(parse_size("512 MB"), Some(512_000_000));
        assert_eq!(parse_size("0B"), Some(0));
        assert_eq!(parse_size("12kB"), Some(12_000));
        assert_eq!(parse_size("lots"), None);
    }

    #[test]
    fn parses_build_cache() {
        let df = "Images\t1.2GB (40%)\nContainers\t0B (0%)\nBuild Cache\t3.4GB\n";
        assert_eq!(parse_build_cache(df), 3_400_000_000);
    }

    #[test]
    fn only_known_actions() {
        assert!(command_for("docker:dangling-images").is_some());
        assert!(command_for("docker system prune -a").is_none());
        for (_, c) in ACTIONS {
            assert!(
                !c.contains(&"-a") && !c.contains(&"--all") && !c.contains(&"--volumes"),
                "{c:?}"
            );
        }
        assert!(run("rm -rf /").is_err());
    }
}
