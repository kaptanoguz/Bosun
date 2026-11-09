//! Sistem metrikleri: CPU, RAM, swap, diskler, sıcaklık, yük ortalaması.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sysinfo::{Components, Disks, System};

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
}

impl Snapshot {
    pub fn mem_percent(&self) -> f32 {
        pct(self.mem_used, self.mem_total)
    }
    pub fn swap_percent(&self) -> f32 {
        pct(self.swap_used, self.swap_total)
    }
    pub fn max_temp(&self) -> Option<f32> {
        self.temps.iter().map(|t| t.celsius).fold(None, |a, t| Some(a.map_or(t, |a: f32| a.max(t))))
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
    if total == 0 { 0.0 } else { used as f32 / total as f32 * 100.0 }
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
}

/// Kullanıcıya anlamlı olmayan sanal/geçici dosya sistemleri.
const IGNORED_FS: &[&str] = &["tmpfs", "devtmpfs", "squashfs", "overlay", "efivarfs", "ramfs", "fuse.portal"];

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
            cpu_brand: self.sys.cpus().first().map(|c| c.brand().trim().to_string()).unwrap_or_default(),
        }
    }

    pub fn collect(&mut self) -> Snapshot {
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
                (t.is_finite() && t > 0.0).then(|| Temperature { label: c.label().to_string(), celsius: t })
            })
            .collect();

        let load = System::load_average().map(|l| [l.one, l.five, l.fifteen]).unwrap_or_default();
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_plausible_values() {
        let mut c = Collector::new();
        let s = c.collect();
        assert!(s.mem_total > 0);
        assert!(s.cpu_count > 0);
        assert!(s.mem_percent() <= 100.0);
    }
}
