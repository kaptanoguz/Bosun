//! Sistem metrikleri: CPU, RAM, swap, diskler, sıcaklık, yük ortalaması.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Instant;
use sysinfo::{Components, Disks, Networks, System};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    pub mount: String,
    pub device: String,
    pub fs: String,
    pub total: u64,
    pub available: u64,
}

impl DiskInfo {
    pub fn used_percent(&self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        (self.total - self.available) as f32 / self.total as f32 * 100.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Temperature {
    pub label: String,
    pub celsius: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub ts: DateTime<Utc>,
    pub cpu_percent: f32,
    pub cpu_count: usize,
    pub mem_total: u64,
    pub mem_used: u64,
    pub swap_total: u64,
    pub swap_used: u64,
    pub load_avg: [f64; 3],
    pub uptime_secs: u64,
    pub disks: Vec<DiskInfo>,
    pub temps: Vec<Temperature>,
    /// Tüm fiziksel arayüzlerde saniyedeki bayt (loopback ve sanal köprüler hariç).
    #[serde(default)]
    pub net_rx_bps: u64,
    #[serde(default)]
    pub net_tx_bps: u64,
    #[serde(default)]
    pub battery: Option<Battery>,
    #[serde(default)]
    pub gpu: Option<Gpu>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Battery {
    pub percent: f32,
    /// "Charging", "Discharging", "Full", "Not charging"…
    pub status: String,
    pub power_w: Option<f32>,
}

impl Battery {
    pub fn discharging(&self) -> bool {
        self.status.eq_ignore_ascii_case("discharging")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Gpu {
    pub name: String,
    pub util_percent: f32,
    pub mem_used: u64,
    pub mem_total: u64,
    pub temp_c: Option<f32>,
}

impl Snapshot {
    pub fn mem_percent(&self) -> f32 {
        pct(self.mem_used, self.mem_total)
    }
    pub fn swap_percent(&self) -> f32 {
        pct(self.swap_used, self.swap_total)
    }
    pub fn max_temp(&self) -> Option<f32> {
        self.temps
            .iter()
            .map(|t| t.celsius)
            .fold(None, |a, t| Some(a.map_or(t, |a: f32| a.max(t))))
    }
    /// Kök dosya sistemi (yoksa en büyük disk).
    pub fn root_disk(&self) -> Option<&DiskInfo> {
        self.disks
            .iter()
            .find(|d| d.mount == "/")
            .or_else(|| self.disks.iter().max_by_key(|d| d.total))
    }
}

fn pct(used: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        used as f32 / total as f32 * 100.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostInfo {
    pub hostname: String,
    pub os: String,
    pub kernel: String,
    pub cpu_brand: String,
}

/// Uzun ömürlü toplayıcı. CPU kullanımı iki ölçüm arasındaki farktan hesaplandığı için
/// aynı nesne tekrar tekrar kullanılmalıdır.
pub struct Collector {
    sys: System,
    disks: Option<Disks>,
    components: Option<Components>,
    networks: Option<Networks>,
    last_net: Instant,
    ticks: u64,
    last_gpu: Option<Gpu>,
}

/// Sayılmayan ağ arayüzleri (yerel ve sanal).
fn virtual_iface(name: &str) -> bool {
    name == "lo"
        || [
            "docker", "br-", "veth", "virbr", "vnet", "tun", "tap", "podman", "cni", "flannel",
        ]
        .iter()
        .any(|p| name.starts_with(p))
}

/// /sys/class/power_supply altındaki ilk pil.
pub fn read_battery(root: &Path) -> Option<Battery> {
    let rd = std::fs::read_dir(root).ok()?;
    for e in rd.filter_map(Result::ok) {
        let p = e.path();
        let read = |f: &str| {
            std::fs::read_to_string(p.join(f))
                .ok()
                .map(|s| s.trim().to_string())
        };
        if read("type").as_deref() != Some("Battery") || read("present").as_deref() == Some("0") {
            continue;
        }
        let percent: f32 = read("capacity")?.parse().ok()?;
        let status = read("status").unwrap_or_else(|| "Unknown".into());
        let power_w = read("power_now")
            .and_then(|v| v.parse::<f64>().ok())
            .map(|uw| (uw / 1e6) as f32)
            .filter(|w| *w > 0.0);
        return Some(Battery {
            percent,
            status,
            power_w,
        });
    }
    None
}

/// NVIDIA (nvidia-smi) ya da AMD (sysfs) GPU kullanımı. Intel için güvenilir bir kaynak yok.
fn read_gpu() -> Option<Gpu> {
    if Path::new("/usr/bin/nvidia-smi").exists() {
        let o = std::process::Command::new("/usr/bin/nvidia-smi")
            .args([
                "--query-gpu=name,utilization.gpu,memory.used,memory.total,temperature.gpu",
                "--format=csv,noheader,nounits",
            ])
            .output()
            .ok()?;
        return parse_nvidia(&String::from_utf8_lossy(&o.stdout));
    }
    let rd = std::fs::read_dir("/sys/class/drm").ok()?;
    for e in rd.filter_map(Result::ok) {
        let dev = e.path().join("device");
        let busy = std::fs::read_to_string(dev.join("gpu_busy_percent")).ok();
        if let Some(b) = busy.and_then(|b| b.trim().parse::<f32>().ok()) {
            let num = |f: &str| {
                std::fs::read_to_string(dev.join(f))
                    .ok()
                    .and_then(|v| v.trim().parse::<u64>().ok())
                    .unwrap_or(0)
            };
            let temp_c = std::fs::read_dir(dev.join("hwmon"))
                .ok()
                .and_then(|mut rd| rd.next())
                .and_then(|h| h.ok())
                .and_then(|h| std::fs::read_to_string(h.path().join("temp1_input")).ok())
                .and_then(|v| v.trim().parse::<f32>().ok())
                .map(|m| m / 1000.0);
            return Some(Gpu {
                name: "AMD GPU".into(),
                util_percent: b,
                mem_used: num("mem_info_vram_used"),
                mem_total: num("mem_info_vram_total"),
                temp_c,
            });
        }
    }
    None
}

/// "NVIDIA GeForce RTX 3060, 34, 2048, 12288, 61" (MiB) → Gpu
pub fn parse_nvidia(text: &str) -> Option<Gpu> {
    let line = text.lines().next()?;
    let c: Vec<&str> = line.split(',').map(str::trim).collect();
    if c.len() < 5 {
        return None;
    }
    Some(Gpu {
        name: c[0].to_string(),
        util_percent: c[1].parse().ok()?,
        mem_used: c[2].parse::<u64>().ok()? * 1024 * 1024,
        mem_total: c[3].parse::<u64>().ok()? * 1024 * 1024,
        temp_c: c[4].parse().ok(),
    })
}

/// Kullanıcıya anlamlı olmayan sanal/geçici dosya sistemleri.
const IGNORED_FS: &[&str] = &[
    "tmpfs",
    "devtmpfs",
    "squashfs",
    "overlay",
    "efivarfs",
    "ramfs",
    "fuse.portal",
];

impl Default for Collector {
    fn default() -> Self {
        Self::new()
    }
}

impl Collector {
    /// sysinfo bu platformda çalışmıyorsa hata döner.
    pub fn try_new() -> Result<Self, sysinfo::Error> {
        let mut sys = System::new()?;
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        Ok(Self {
            sys,
            // Disk/sıcaklık okunamazsa (konteyner vb.) boş liste ile devam et.
            disks: Disks::new_with_refreshed_list().ok(),
            components: Components::new_with_refreshed_list().ok(),
            networks: Networks::new_with_refreshed_list().ok(),
            last_net: Instant::now(),
            ticks: 0,
            last_gpu: None,
        })
    }

    pub fn new() -> Self {
        Self::try_new().expect("sistem bilgisi okunamadı")
    }

    pub fn host_info(&self) -> HostInfo {
        HostInfo {
            hostname: System::host_name().unwrap_or_default(),
            os: System::long_os_version().unwrap_or_default(),
            kernel: System::kernel_version().unwrap_or_default(),
            cpu_brand: self
                .sys
                .cpus()
                .first()
                .map(|c| c.brand().trim().to_string())
                .unwrap_or_default(),
        }
    }

    pub fn collect(&mut self) -> Snapshot {
        self.ticks += 1;
        let (mut rx, mut tx) = (0u64, 0u64);
        let elapsed = self.last_net.elapsed().as_secs_f64().max(0.001);
        if let Some(n) = self.networks.as_mut() {
            n.refresh(true);
            for (name, data) in n.iter() {
                if !virtual_iface(&name.to_string_lossy()) {
                    rx += data.received();
                    tx += data.transmitted();
                }
            }
        }
        self.last_net = Instant::now();
        // nvidia-smi her çağrıda ~50 ms sürer; üç ölçümde bir yeter.
        if self.ticks % 3 == 1 {
            self.last_gpu = read_gpu();
        }
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        if let Some(d) = self.disks.as_mut() {
            d.refresh(true);
        }
        if let Some(c) = self.components.as_mut() {
            c.refresh(true);
        }

        let mut seen = std::collections::HashSet::new();
        let disks = self
            .disks
            .iter()
            .flat_map(|d| d.iter())
            .filter(|d| {
                let fs = d.file_system().to_string_lossy();
                !IGNORED_FS.contains(&fs.as_ref()) && d.total_space() > 0
            })
            // Aynı aygıt birden çok noktaya bağlıysa (btrfs alt birimleri vb.) bir kez say.
            .filter(|d| seen.insert(d.name().to_string_lossy().to_string()))
            .map(|d| DiskInfo {
                mount: d.mount_point().to_string_lossy().into_owned(),
                device: d.name().to_string_lossy().into_owned(),
                fs: d.file_system().to_string_lossy().into_owned(),
                total: d.total_space(),
                available: d.available_space(),
            })
            .collect();

        let temps = self
            .components
            .iter()
            .flat_map(|c| c.iter())
            .filter_map(|c| {
                let t = c.temperature()?;
                (t.is_finite() && t > 0.0).then(|| Temperature {
                    label: c.label().to_string(),
                    celsius: t,
                })
            })
            .collect();

        let load = System::load_average()
            .map(|l| [l.one, l.five, l.fifteen])
            .unwrap_or_default();
        Snapshot {
            ts: Utc::now(),
            cpu_percent: self.sys.global_cpu_usage(),
            cpu_count: self.sys.cpus().len(),
            mem_total: self.sys.total_memory(),
            mem_used: self.sys.used_memory(),
            swap_total: self.sys.total_swap(),
            swap_used: self.sys.used_swap(),
            load_avg: load,
            uptime_secs: System::uptime().unwrap_or_default(),
            disks,
            temps,
            net_rx_bps: (rx as f64 / elapsed) as u64,
            net_tx_bps: (tx as f64 / elapsed) as u64,
            battery: read_battery(Path::new("/sys/class/power_supply")),
            gpu: self.last_gpu.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_battery_from_sysfs() {
        let d = tempfile::tempdir().unwrap();
        let ac = d.path().join("AC");
        let bat = d.path().join("BAT0");
        std::fs::create_dir_all(&ac).unwrap();
        std::fs::create_dir_all(&bat).unwrap();
        std::fs::write(ac.join("type"), "Mains\n").unwrap();
        std::fs::write(bat.join("type"), "Battery\n").unwrap();
        std::fs::write(bat.join("capacity"), "78\n").unwrap();
        std::fs::write(bat.join("status"), "Discharging\n").unwrap();
        std::fs::write(bat.join("power_now"), "9500000\n").unwrap();
        let b = read_battery(d.path()).unwrap();
        assert_eq!(b.percent, 78.0);
        assert!(b.discharging());
        assert_eq!(b.power_w, Some(9.5));
        assert!(read_battery(&ac).is_none());
    }

    #[test]
    fn parses_nvidia_smi() {
        let g = parse_nvidia("NVIDIA GeForce RTX 3060, 34, 2048, 12288, 61\n").unwrap();
        assert_eq!(g.util_percent, 34.0);
        assert_eq!(g.mem_total, 12288 * 1024 * 1024);
        assert_eq!(g.temp_c, Some(61.0));
        assert!(parse_nvidia("").is_none());
    }

    #[test]
    fn ignores_virtual_interfaces() {
        assert!(virtual_iface("lo") && virtual_iface("docker0") && virtual_iface("veth12ab"));
        assert!(!virtual_iface("wlp2s0") && !virtual_iface("enp3s0"));
    }

    #[test]
    fn collects_plausible_values() {
        let mut c = Collector::new();
        let s = c.collect();
        assert!(s.mem_total > 0);
        assert!(s.cpu_count > 0);
        assert!(s.mem_percent() <= 100.0);
    }
}
