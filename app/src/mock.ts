// Tarayıcıda (Tauri olmadan) arayüz geliştirmek için sahte daemon.
// Gerçek uygulamada hiç kullanılmaz.

import type { CategoryScan, DaemonEvent, Plan, Snapshot } from "./api";

const GB = 1024 ** 3;
const MB = 1024 ** 2;
const now = () => new Date().toISOString();

function snap(i: number): Snapshot {
  const t = Date.now() - (240 - i) * 15000;
  return {
    ts: new Date(t).toISOString(),
    cpu_percent: 12 + 10 * Math.sin(i / 9) + (i % 17 === 0 ? 35 : 0) + Math.random() * 6,
    cpu_count: 8,
    mem_total: 16 * GB,
    mem_used: (9.2 + Math.sin(i / 30) * 0.8) * GB,
    swap_total: 4 * GB,
    swap_used: 0.6 * GB,
    load_avg: [1.24, 1.1, 0.98],
    uptime_secs: 3 * 86400 + 5 * 3600,
    disks: [
      { mount: "/", device: "/dev/nvme0n1p2", fs: "ext4", total: 476 * GB, available: 58 * GB },
      { mount: "/home/user/Arşiv", device: "/dev/sda1", fs: "ext4", total: 931 * GB, available: 402 * GB },
    ],
    temps: [{ label: "Package id 0", celsius: 54 }],
    net_rx_bps: Math.round(240000 + 180000 * Math.sin(i / 5)),
    net_tx_bps: Math.round(30000 + 12000 * Math.cos(i / 7)),
    battery: { percent: 78, status: "Discharging", power_w: 9.4 },
    gpu: { name: "AMD Radeon Graphics", util_percent: 12 + (i % 9), mem_used: 1.1 * GB, mem_total: 2 * GB, temp_c: 49 },
    disk_read_bps: Math.round(Math.max(0, 900000 + 2400000 * Math.sin(i / 4)) * (i % 7 === 0 ? 6 : 1)),
    disk_write_bps: Math.round(600000 + 500000 * Math.cos(i / 6) + (i % 11 === 0 ? 3000000 : 0)),
    top_apps: [
      { name: "firefox", cpu_percent: 9.4 + (i % 5), mem_bytes: 3.1 * GB, processes: 14 },
      { name: "code", cpu_percent: 4.2 + (i % 3), mem_bytes: 1.4 * GB, processes: 11 },
      { name: "cargo", cpu_percent: 3.8, mem_bytes: 610 * MB, processes: 3 },
      { name: "cinnamon", cpu_percent: 2.1, mem_bytes: 410 * MB, processes: 1 },
      { name: "Xorg", cpu_percent: 1.6, mem_bytes: 180 * MB, processes: 1 },
      { name: "dockerd", cpu_percent: 1.1, mem_bytes: 140 * MB, processes: 1 },
      { name: "telegram-desktop", cpu_percent: 0.6, mem_bytes: 520 * MB, processes: 1 },
      { name: "pipewire", cpu_percent: 0.4, mem_bytes: 24 * MB, processes: 3 },
    ],
  };
}

const containers = {
  engine: "docker",
  running: 3,
  total: 7,
  containers: [
    { name: "shop-api-1", image: "shop/api:dev", cpu_percent: 12.4, mem_bytes: 164 * MB },
    { name: "shop-postgres-1", image: "postgres:17", cpu_percent: 2.1, mem_bytes: 38 * MB },
    { name: "shop-redis-1", image: "redis:7", cpu_percent: 0.3, mem_bytes: 9 * MB },
  ],
};

const startup = [
  { file: "telegram.desktop", name: "Telegram", comment: "Messaging app", exec: "telegram-desktop -startintray", enabled: true, system: false, essential: false, missing: false },
  { file: "discord-stable.desktop", name: "Discord", comment: "", exec: "/opt/discord/Discord", enabled: true, system: false, essential: false, missing: true },
  { file: "nextcloud.desktop", name: "Nextcloud", comment: "Sync files with your Nextcloud", exec: "nextcloud --background", enabled: false, system: false, essential: false, missing: false },
  { file: "blueman.desktop", name: "Blueman Applet", comment: "Bluetooth manager", exec: "blueman-applet", enabled: true, system: true, essential: false, missing: false },
  { file: "mintupdate.desktop", name: "Update Manager", comment: "Show available updates", exec: "mintupdate-launcher", enabled: true, system: true, essential: false, missing: false },
  { file: "nm-applet.desktop", name: "Network", comment: "Manage your network connections", exec: "nm-applet", enabled: true, system: true, essential: false, missing: false },
  { file: "polkit-gnome-authentication-agent-1.desktop", name: "PolicyKit Authentication Agent", comment: "", exec: "/usr/lib/policykit-1-gnome/polkit-gnome-authentication-agent-1", enabled: true, system: true, essential: true, missing: false },
  { file: "gnome-keyring-secrets.desktop", name: "Secret Storage Service", comment: "GNOME Keyring: Secret Service", exec: "/usr/bin/gnome-keyring-daemon --start --components=secrets", enabled: true, system: true, essential: true, missing: false },
];

const history = Array.from({ length: 240 }, (_, i) => snap(i));

const scan = {
  ts: now(),
  duration_ms: 812,
  categories: ([
    { id: "browser_cache", title: "Tarayıcı önbelleği", description: "Web sayfalarının geçici kopyaları. Oturumlar ve şifreler etkilenmez; tarayıcı açıksa kapatmak daha iyi olur.", safety: "safe", method: "trash", bulk_selectable: true, manual_command: null, bytes: 2.4 * GB, item_count: 3, items: [{ path: "/home/user/.cache/mozilla", bytes: 1.6 * GB, modified: now() }, { path: "/home/user/.cache/google-chrome", bytes: 0.8 * GB, modified: now() }] },
    { id: "cargo_cache", title: "Cargo indirme önbelleği", description: "Rust crate arşivleri (.crate). Bir sonraki derlemede yeniden indirilir.", safety: "safe", method: "trash", bulk_selectable: true, manual_command: null, bytes: 1.1 * GB, item_count: 1, items: [{ path: "/home/user/.cargo/registry/cache/index.crates.io-1949cf8c6b5b557f", bytes: 1.1 * GB, modified: now() }] },
    { id: "npm_cache", title: "npm önbelleği", description: "npm'in indirdiği paket arşivleri. Bir sonraki kurulumda yeniden indirilir.", safety: "safe", method: "trash", bulk_selectable: true, manual_command: null, bytes: 860 * MB, item_count: 2, items: [{ path: "/home/user/.npm/_cacache/content-v2", bytes: 840 * MB, modified: now() }] },
    { id: "thumbnails", title: "Küçük resim önbelleği", description: "Dosya yöneticisinin ürettiği önizlemeler. Gerektiğinde yeniden oluşturulur.", safety: "safe", method: "trash", bulk_selectable: true, manual_command: null, bytes: 412 * MB, item_count: 3, items: [{ path: "/home/user/.cache/thumbnails/large", bytes: 300 * MB, modified: now() }] },
    { id: "user_cache", title: "Eski uygulama önbellekleri", description: "~/.cache içinde 30 günden uzun süredir değişmemiş dosyalar. Uygulamalar gerekirse yeniden indirir.", safety: "caution", method: "trash", bulk_selectable: true, manual_command: null, bytes: 1.3 * GB, item_count: 4210, locked: { items: 2, bytes: 196 * MB, dirs: ["/home/user/.cache/waydroid-script"], command: `sudo chown -R "$USER:" '/home/user/.cache/waydroid-script'` }, items: [{ path: "/home/user/.cache/ms-playwright/chromium-1140/chrome-linux/chrome", bytes: 380 * MB, modified: now() }] },
    { id: "old_logs", title: "Eski kullanıcı günlükleri", description: "~/.local/state altında 30 günden eski .log dosyaları.", safety: "caution", method: "trash", bulk_selectable: true, manual_command: null, bytes: 38 * MB, item_count: 12, items: [] },
    { id: "trash", title: "Çöp kutusu", description: "Çöp kutusundaki dosyalar. Bu işlem KALICIDIR, geri alınamaz.", safety: "caution", method: "permanent", bulk_selectable: true, manual_command: null, bytes: 3.7 * GB, item_count: 46, items: [] },
    { id: "journal", title: "Sistem günlükleri (journald)", description: "systemd günlükleri. 500 MB'a indirilmesi önerilir; yönetici yetkisi gerekir.", safety: "safe", method: "privileged", bulk_selectable: false, manual_command: "sudo journalctl --vacuum-size=500M", bytes: 2.9 * GB, item_count: 1, items: [] },
    { id: "apt_cache", title: "APT paket önbelleği", description: "Kurulmuş paketlerin indirilmiş .deb dosyaları. `sudo apt-get clean` ile temizlenir.", safety: "safe", method: "privileged", bulk_selectable: false, manual_command: "sudo apt-get clean", bytes: 640 * MB, item_count: 88, items: [] },
    { id: "large_files", title: "İndirilenlerdeki büyük dosyalar", description: "500 MB'tan büyük indirilmiş dosyalar. Otomatik silinmez; tek tek seçmelisiniz.", safety: "review_only", method: "trash", bulk_selectable: false, manual_command: null, bytes: 7.9 * GB, item_count: 3, items: [
      { path: "/home/user/Downloads/linuxmint-22.2-cinnamon-64bit.iso", bytes: 2.9 * GB, modified: "2026-09-02T10:00:00Z" },
      { path: "/home/user/Downloads/ubuntu-24.04.3-desktop-amd64.iso", bytes: 3.1 * GB, modified: "2026-10-11T10:00:00Z" },
      { path: "/home/user/Downloads/android-studio-2026.1.tar.gz", bytes: 1.9 * GB, modified: "2026-07-20T10:00:00Z" },
    ] },
    { id: "flatpak_cache", title: "Flatpak uygulama önbellekleri", description: "Flatpak uygulamalarının önbellek klasörleri (~/.var/app/*/cache). Uygulamalar gerektiğinde yeniden oluşturur.", safety: "safe", method: "trash", bulk_selectable: true, manual_command: null, bytes: 530 * MB, item_count: 4, items: [{ path: "/home/user/.var/app/org.gimp.GIMP/cache/gegl-0.4", bytes: 410 * MB, modified: now() }] },
    { id: "broken_links", title: "Kırık bağlantılar", description: "Ev klasörünüzde hedefi artık olmayan sembolik bağlantılar. Yalnızca bağlantının kendisi silinir.", safety: "safe", method: "unlink", bulk_selectable: true, manual_command: null, bytes: 0, item_count: 5, items: [
      { path: "/home/user/.local/bin/node18", bytes: 0, modified: now() },
      { path: "/home/user/Desktop/Eski Proje", bytes: 0, modified: now() },
      { path: "/home/user/.local/share/applications/defaults.list", bytes: 0, modified: now() },
    ] },
    { id: "orphan_launchers", title: "Kırık kısayollar", description: "Çalıştırdığı program artık kurulu olmayan menü ve otomatik başlatma kısayolları.", safety: "caution", method: "trash", bulk_selectable: true, manual_command: null, bytes: 1800, item_count: 3, items: [
      { path: "/home/user/.local/share/applications/skypeforlinux.desktop", bytes: 600, modified: now() },
      { path: "/home/user/.config/autostart/dropbox.desktop", bytes: 400, modified: now() },
    ] },
    { id: "leftover_configs", title: "Eski programlardan kalmış olabilecek klasörler", description: "~/.config ve ~/.local/share içinde kurulu hiçbir programla eşleşmeyen ve 6 aydır değişmemiş klasörler. Bu bir tahmindir: işaretlemeden önce her birine bakın.", safety: "review_only", method: "trash", bulk_selectable: false, manual_command: null, bytes: 1.2 * GB, item_count: 3, items: [
      { path: "/home/user/.config/skypeforlinux", bytes: 690 * MB, modified: "2025-11-02T10:00:00Z" },
      { path: "/home/user/.local/share/Steam-old", bytes: 480 * MB, modified: "2025-06-02T10:00:00Z" },
      { path: "/home/user/.config/teamviewer", bytes: 42 * MB, modified: "2026-01-14T10:00:00Z" },
    ] },
    { id: "orphan_packages", title: "Artık hiçbir şeyin ihtiyaç duymadığı paketler", description: "Sonradan kaldırdığınız programlar için kurulmuş bağımlılıklar. Yönetici yetkisi gerekir.", safety: "caution", method: "privileged", bulk_selectable: false, manual_command: "sudo apt autoremove --purge", admin_action: "apt-autoremove", bytes: 812 * MB, item_count: 7, items: [
      { path: "linux-headers-6.8.0-40", bytes: 92 * MB, modified: null }, { path: "linux-image-6.8.0-40-generic", bytes: 640 * MB, modified: null }, { path: "libwxgtk3.0-gtk3-0v5", bytes: 40 * MB, modified: null },
    ] },
    { id: "residual_packages", title: "Kaldırılmış paketlerin ayar dosyaları", description: "Kaldırılmış paketlerden geriye kalan sistem ayar dosyaları. Yönetici yetkisi gerekir.", safety: "safe", method: "privileged", bulk_selectable: false, manual_command: "sudo apt purge '~c'", admin_action: "purge-residual", bytes: 0, item_count: 4, items: [
      { path: "skypeforlinux", bytes: 0, modified: null }, { path: "teamviewer", bytes: 0, modified: null }, { path: "libreoffice-core", bytes: 0, modified: null }, { path: "zoom", bytes: 0, modified: null },
    ] },
  ] as unknown as CategoryScan[]).map((c) => ({
    ...c,
    admin_action: c.admin_action ?? (c.id === "journal" ? "journal-vacuum" : c.id === "apt_cache" ? "apt-clean" : null),
  })),
};

const findingsEn = {
  overall: "warning",
  failed_units: [],
  findings: [
    { id: "disk:/", severity: "warning", title: "/ is 88% full", detail: "58 GB free of 476 GB.", category: null },
    { id: "leak:telegram-desktop", severity: "info", title: "telegram-desktop keeps using more memory", detail: "It grew by about 180 MB per hour over 95 minutes and now uses 520 MB. Restarting it gives the memory back.", category: null },
    { id: "reclaimable", severity: "info", title: "5.3 GB can be safely cleaned", detail: "Caches and temporary files. Review them on the Cleanup tab.", category: null },
    { id: "big:journal", severity: "info", title: "System logs (journald): 2.9 GB", detail: "Bosun can clean this on the Cleanup tab after asking for your password.", category: "journal" },
  ],
  advice: [
    { id: "journald_max_use", title: "Cap the size of system logs", detail: "By default journald can grow to 10% of the disk. A 500 MB cap is enough for most desktops.", current: "No limit (2.9 GB in use)", suggested: "SystemMaxUse=500M", command: "sudo mkdir -p /etc/systemd/journald.conf.d && echo -e '[Journal]\\nSystemMaxUse=500M' | sudo tee /etc/systemd/journald.conf.d/bosun.conf && sudo systemctl restart systemd-journald", admin_action: "journald-limit" },
    { id: "snap_retain", title: "Keep fewer old Snap revisions", detail: "These snaps keep 3 or more revisions on disk: core22, gnome-42-2204.", current: "refresh.retain = 3 (default)", suggested: "refresh.retain = 2", command: "sudo snap set system refresh.retain=2", admin_action: "snap-retain" },
  ],
};

const findings = {
  overall: "warning",
  failed_units: [],
  findings: [
    { id: "disk:/", severity: "warning", title: "/ %88 dolu", detail: "58 GB boş alan kaldı (476 GB toplam).", category: null },
    { id: "leak:telegram-desktop", severity: "info", title: "telegram-desktop giderek daha çok bellek kullanıyor", detail: "95 dakikadır saatte yaklaşık 180 MB büyüdü, şu an 520 MB kullanıyor. Yeniden başlatmak belleği geri verir.", category: null },
    { id: "reclaimable", severity: "info", title: "5,3 GB güvenle temizlenebilir", detail: "Önbellekler ve geçici dosyalar. Temizlik sekmesinden gözden geçirebilirsiniz.", category: null },
    { id: "big:journal", severity: "info", title: "Sistem günlükleri (journald): 2,9 GB", detail: "Bosun bunu Temizlik sekmesinden, şifrenizi sorduktan sonra temizleyebilir.", category: "journal" },
  ],
  advice: [
    { id: "journald_max_use", title: "Sistem günlüklerine boyut sınırı koyun", detail: "journald varsayılan olarak diskin %10'una kadar büyüyebilir. 500 MB sınırı çoğu masaüstü için yeterlidir.", current: "Sınır yok (2,9 GB kullanımda)", suggested: "SystemMaxUse=500M", command: "sudo mkdir -p /etc/systemd/journald.conf.d && echo -e '[Journal]\\nSystemMaxUse=500M' | sudo tee /etc/systemd/journald.conf.d/bosun.conf && sudo systemctl restart systemd-journald" },
    { id: "snap_retain", title: "Snap'in eski sürüm sayısını azaltın", detail: "Şu paketlerin 3 veya daha fazla sürümü diskte duruyor: core22, gnome-42-2204.", current: "refresh.retain = 3 (varsayılan)", suggested: "refresh.retain = 2", command: "sudo snap set system refresh.retain=2" },
  ],
};

function makePlan(categories: string[], large: string[], origin: "user" | "assistant" = "user", reason: string | null = null): Plan {
  const items: Plan["items"] = [];
  const blocked: Plan["blocked"] = [];
  for (const id of categories) {
    const c = scan.categories.find((c) => c.id === id)!;
    if (!c.bulk_selectable) {
      blocked.push({ category: id, reason: "Yönetici yetkisi gerekir; Bosun bunu şimdilik kendisi yapmaz.", manual_command: c.manual_command });
      continue;
    }
    if (c.items.length === 0) items.push({ category: id, path: `/home/user/.local/share/${id}`, bytes: c.bytes, method: c.method as Plan["items"][0]["method"] });
    for (const it of c.items) items.push({ category: id, path: it.path, bytes: it.bytes, method: c.method as Plan["items"][0]["method"] });
  }
  for (const p of large) {
    const it = scan.categories.find((c) => c.id === "large_files")!.items.find((i) => i.path === p)!;
    items.push({ category: "large_files", path: p, bytes: it.bytes, method: "trash" });
  }
  const summary: Plan["summary"] = [];
  for (const it of items) {
    const s = summary.find((s) => s.category === it.category);
    if (s) {
      s.items++;
      s.bytes += it.bytes;
    } else {
      const c = scan.categories.find((c) => c.id === it.category)!;
      summary.push({ category: it.category, title: c.title, method: it.method, items: 1, bytes: it.bytes });
    }
  }
  return {
    id: crypto.randomUUID(),
    created_at: now(),
    expires_at: new Date(Date.now() + 600000).toISOString(),
    origin,
    reason,
    items,
    summary,
    blocked,
    total_bytes: items.reduce((a, b) => a + b.bytes, 0),
    has_permanent: items.some((i) => i.method === "permanent"),
    digest: "mock",
  };
}

const ago = (ms: number) => new Date(Date.now() - ms).toISOString();
const HOUR = 3600000;
const audit = [
  { ts: ago(3 * HOUR), kind: "startup_toggled", data: { name: "Nextcloud", enabled: false, file: "/home/user/.config/autostart/com.nextcloud.desktopclient.nextcloud.desktop" } },
  { ts: ago(26 * HOUR), kind: "admin_action", data: { action: "journal-vacuum", title: "Sistem günlüklerini küçült", ok: true, freed_bytes: 2.4 * GB } },
  { ts: ago(48 * HOUR), kind: "plan_executed", data: { plan_id: "p2", freed_bytes: 3.2 * GB, trashed: 14, deleted: 0, failed: 0, skipped: 0, summary: [{ title: "Tarayıcı önbelleği" }, { title: "Küçük resim önbelleği" }] } },
  { ts: ago(48 * HOUR + 30000), kind: "plan_approved", data: { plan_id: "p2", items: 14, total_bytes: 3.2 * GB } },
  { ts: ago(48 * HOUR + 60000), kind: "plan_proposed", data: { plan_id: "p2", origin: "assistant", total_bytes: 3.2 * GB, summary: [{ title: "Tarayıcı önbelleği" }, { title: "Küçük resim önbelleği" }] } },
  { ts: ago(50 * HOUR), kind: "plan_cancelled", data: { plan_id: "p1" } },
  { ts: ago(50 * HOUR + 40000), kind: "plan_created", data: { plan_id: "p1", origin: "user", total_bytes: 1.1 * GB, summary: [{ title: "Cargo indirme önbelleği" }] } },
];

// bosun-core/src/admin.rs'den üretildi (yalnızca tarayıcı önizlemesi için).
const ADMIN_INFO: Record<string, { title: string[]; effect: string[]; commands: string[]; undo: string | null }> = {
  "journal-vacuum": {
    "title": [
      "Shrink system logs",
      "Sistem günlüklerini küçült"
    ],
    "effect": [
      "Old journal entries are deleted until the logs fit in 500 MB.",
      "Günlükler 500 MB'a sığana kadar eski kayıtlar silinir."
    ],
    "commands": [
      "journalctl --vacuum-size=500M"
    ],
    "undo": null
  },
  "apt-clean": {
    "title": [
      "Clear the APT package cache",
      "APT paket önbelleğini temizle"
    ],
    "effect": [
      "Downloaded .deb files are removed. Installed software is not affected.",
      "İndirilmiş .deb dosyaları silinir. Kurulu programlar etkilenmez."
    ],
    "commands": [
      "apt-get clean"
    ],
    "undo": null
  },
  "journald-limit": {
    "title": [
      "Cap system logs at 500 MB",
      "Sistem günlüklerini 500 MB ile sınırla"
    ],
    "effect": [
      "Adds a small journald setting file and restarts journald. You can undo it from Settings.",
      "Küçük bir journald ayar dosyası ekler ve journald'ı yeniden başlatır. Ayarlar'dan geri alabilirsiniz."
    ],
    "commands": [
      "write /etc/systemd/journald.conf.d/60-bosun.conf (SystemMaxUse=500M)",
      "systemctl restart systemd-journald"
    ],
    "undo": "journald-limit-undo"
  },
  "journald-limit-undo": {
    "title": [
      "Remove the system log cap",
      "Sistem günlüğü sınırını kaldır"
    ],
    "effect": [
      "Deletes Bosun's journald setting file and restarts journald.",
      "Bosun'un journald ayar dosyasını siler ve journald'ı yeniden başlatır."
    ],
    "commands": [
      "rm /etc/systemd/journald.conf.d/60-bosun.conf",
      "systemctl restart systemd-journald"
    ],
    "undo": null
  },
  "snap-retain": {
    "title": [
      "Keep 2 Snap revisions",
      "Snap'te 2 sürüm tut"
    ],
    "effect": [
      "Snap keeps one fewer old revision of each package. Extra revisions go away on the next refresh.",
      "Snap her paketin bir eski sürümünü daha az tutar. Fazla sürümler bir sonraki güncellemede silinir."
    ],
    "commands": [
      "snap set system refresh.retain=2"
    ],
    "undo": "snap-retain-undo"
  },
  "snap-retain-undo": {
    "title": [
      "Restore Snap's default (3 revisions)",
      "Snap varsayılanına dön (3 sürüm)"
    ],
    "effect": [
      "Snap goes back to keeping 3 revisions.",
      "Snap yeniden 3 sürüm tutar."
    ],
    "commands": [
      "snap set system refresh.retain=3"
    ],
    "undo": null
  },
  "swappiness": {
    "title": [
      "Set swappiness to 20",
      "swappiness değerini 20 yap"
    ],
    "effect": [
      "The kernel uses swap less eagerly. Takes effect immediately and survives reboots.",
      "Çekirdek swap'ı daha az kullanır. Hemen etkili olur, yeniden başlatmada da korunur."
    ],
    "commands": [
      "write /etc/sysctl.d/60-bosun-swappiness.conf (vm.swappiness=20)",
      "sysctl -w vm.swappiness=20"
    ],
    "undo": "swappiness-undo"
  },
  "swappiness-undo": {
    "title": [
      "Restore default swappiness (60)",
      "Varsayılan swappiness'a dön (60)"
    ],
    "effect": [
      "Deletes Bosun's sysctl file and sets swappiness back to 60.",
      "Bosun'un sysctl dosyasını siler ve swappiness'ı 60'a döndürür."
    ],
    "commands": [
      "rm /etc/sysctl.d/60-bosun-swappiness.conf",
      "sysctl -w vm.swappiness=60"
    ],
    "undo": null
  },
  "flatpak-unused": {
    "title": [
      "Remove unused Flatpak runtimes",
      "Kullanılmayan Flatpak çalışma ortamlarını kaldır"
    ],
    "effect": [
      "Runtimes that no installed Flatpak app needs any more are uninstalled. Your apps keep working.",
      "Kurulu hiçbir Flatpak uygulamasının artık ihtiyaç duymadığı çalışma ortamları kaldırılır. Uygulamalarınız çalışmaya devam eder."
    ],
    "commands": [
      "flatpak uninstall --system --unused --assumeyes"
    ],
    "undo": null
  },
  "apt-autoremove": {
    "title": [
      "Remove packages nothing needs",
      "Gereksiz paketleri kaldır"
    ],
    "effect": [
      "Removes dependencies that were installed automatically for programs you've since uninstalled, along with their settings.",
      "Sonradan kaldırdığınız programlar için otomatik kurulmuş bağımlılıkları ayarlarıyla birlikte kaldırır."
    ],
    "commands": [
      "apt-get autoremove --purge -y"
    ],
    "undo": null
  },
  "purge-residual": {
    "title": [
      "Delete settings of removed packages",
      "Kaldırılmış paketlerin ayarlarını sil"
    ],
    "effect": [
      "Deletes the system-wide configuration files that removed packages left behind. Nothing that is installed is touched.",
      "Kaldırılmış paketlerin geride bıraktığı sistem ayar dosyalarını siler. Kurulu hiçbir şeye dokunulmaz."
    ],
    "commands": [
      "dpkg --purge <packages in 'rc' state>"
    ],
    "undo": null
  }
};

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

let mockLang: "en" | "tr" = (() => {
  try {
    return localStorage.getItem("bosun.lang") === "tr" ? "tr" : "en";
  } catch {
    return "en";
  }
})();
const L = (en: string, tr: string) => (mockLang === "tr" ? tr : en);

// İngilizce karşılıklar (sahte veride metinler Türkçe yazılı).
const EN: Record<string, string> = {
  "Sistem günlüklerini küçült": "Shrink system logs",
  "Küçük resim önbelleği": "Thumbnail cache",
  "Tarayıcı önbelleği": "Browser cache",
  "Eski uygulama önbellekleri": "Stale app caches",
  "npm önbelleği": "npm cache",
  "Cargo indirme önbelleği": "Cargo download cache",
  "Eski kullanıcı günlükleri": "Old user logs",
  "Çöp kutusu": "Trash",
  "Sistem günlükleri (journald)": "System logs (journald)",
  "APT paket önbelleği": "APT package cache",
  "Flatpak uygulama önbellekleri": "Flatpak app caches",
  "Geliştirici önbellekleri": "Developer caches",
  "Steam gölgelendirici önbellekleri": "Steam shader caches",
  "Docker / Podman artıkları": "Docker / Podman leftovers",
  "Kullanılmayan Flatpak çalışma ortamları": "Unused Flatpak runtimes",
  "Kırık bağlantılar": "Broken links",
  "Kırık kısayollar": "Broken shortcuts",
  "Eski programlardan kalmış olabilecek klasörler": "Possible leftovers from old programs",
  "Artık hiçbir şeyin ihtiyaç duymadığı paketler": "Packages nothing needs any more",
  "Kaldırılmış paketlerin ayar dosyaları": "Settings of removed packages",
  "İndirilenlerdeki büyük dosyalar": "Large files in Downloads",
  "Dosya yöneticisinin ürettiği önizlemeler. Gerektiğinde yeniden oluşturulur.": "Previews generated by the file manager. Rebuilt when needed.",
  "Web sayfalarının geçici kopyaları. Oturumlar ve şifreler etkilenmez; tarayıcı açıksa kapatmak daha iyi olur.": "Temporary copies of web pages. Sessions and passwords are not affected; closing the browser first is best.",
  "~/.cache içinde 30 günden uzun süredir değişmemiş dosyalar. Uygulamalar gerekirse yeniden indirir.": "Files in ~/.cache untouched for more than 30 days. Apps download them again if needed.",
  "npm'in indirdiği paket arşivleri. Bir sonraki kurulumda yeniden indirilir.": "Package archives downloaded by npm. Fetched again on the next install.",
  "Rust crate arşivleri (.crate). Bir sonraki derlemede yeniden indirilir.": "Rust crate archives (.crate). Fetched again on the next build.",
  "~/.local/state altında 30 günden eski .log dosyaları.": ".log files older than 30 days under ~/.local/state.",
  "Çöp kutusundaki dosyalar. Bu işlem KALICIDIR, geri alınamaz.": "Files in the trash. Emptying it is PERMANENT and cannot be undone.",
  "systemd günlükleri. 500 MB'a indirilmesi önerilir; yönetici yetkisi gerekir.": "systemd logs. Shrinking to 500 MB is recommended; requires administrator rights.",
  "Kurulmuş paketlerin indirilmiş .deb dosyaları. `sudo apt-get clean` ile temizlenir.": "Downloaded .deb files of installed packages. Cleared with `sudo apt-get clean`.",
  "Flatpak uygulamalarının önbellek klasörleri (~/.var/app/*/cache). Uygulamalar gerektiğinde yeniden oluşturur.": "Cache folders of Flatpak apps (~/.var/app/*/cache). Apps rebuild them when needed.",
  "pip, uv, Poetry, conda, Go, Yarn ve Gradle indirme önbellekleri. Gerektiğinde yeniden indirilir.": "Download caches of pip, uv, Poetry, conda, Go, Yarn and Gradle. They are fetched again when needed.",
  "Oyun başına derlenmiş gölgelendiriciler. Steam yeniden oluşturur; oyunun ilk açılışı kısa süre takılabilir.": "Compiled shaders per game. Steam rebuilds them; the first launch of a game may stutter briefly.",
  "Etiketsiz (sarkan) imajlar ve derleme önbelleği. Etiketli imajlara, kapsayıcılara ve birimlere asla dokunulmaz.": "Untagged (dangling) images and the build cache. Tagged images, containers and volumes are never touched.",
  "Kaldırılmış Flatpak uygulamalarından kalan çalışma ortamları (Bosun'un tahmini; son kararı Flatpak verir ve sabitlenmiş ortamları asla silmez). Yönetici yetkisi gerekir.": "Runtimes left behind by removed Flatpak apps (Bosun's estimate; Flatpak makes the final call and never removes pinned runtimes). Requires administrator rights.",
  "Ev klasörünüzde hedefi artık olmayan sembolik bağlantılar. Yalnızca bağlantının kendisi silinir.": "Symbolic links in your home folder whose target no longer exists. Only the link is removed.",
  "Çalıştırdığı program artık kurulu olmayan menü ve otomatik başlatma kısayolları.": "Menu and autostart entries whose program is no longer installed.",
  "~/.config ve ~/.local/share içinde kurulu hiçbir programla eşleşmeyen ve 6 aydır değişmemiş klasörler. Bu bir tahmindir: işaretlemeden önce her birine bakın.": "Folders in ~/.config and ~/.local/share that match no installed program and haven't changed in 6 months. This is a guess: check each one before you tick it.",
  "Sonradan kaldırdığınız programlar için kurulmuş bağımlılıklar. Yönetici yetkisi gerekir.": "Dependencies that were installed for programs you've since removed. Requires administrator rights.",
  "Kaldırılmış paketlerden geriye kalan sistem ayar dosyaları. Yönetici yetkisi gerekir.": "System-wide configuration files left behind by removed packages. Requires administrator rights.",
  "500 MB'tan büyük indirilmiş dosyalar. Otomatik silinmez; tek tek seçmelisiniz.": "Downloaded files larger than 500 MB. Never removed automatically; pick them one by one."
};
function localize<T>(v: T): T {
  if (mockLang === "tr") return v;
  return JSON.parse(JSON.stringify(v), (_k, x) => (typeof x === "string" && EN[x] ? EN[x] : x));
}
const mockToggles = { share_file_paths: false, notifications: true, persist_chat: true };
let mockAi = { provider: "anthropic", model: "", base_url: null as string | null, keys: { anthropic: true } as Record<string, boolean> };
const PROVIDERS = [
  ["anthropic", "Anthropic Claude", "claude-sonnet-5-5", null],
  ["xai", "xAI Grok", "grok-4.5", "https://api.x.ai/v1"],
  ["google", "Google Gemini", "gemini-3.8-flash", "https://generativelanguage.googleapis.com/v1beta/openai"],
  ["openai", "OpenAI", "gpt-5", "https://api.openai.com/v1"],
  ["custom", "Custom (OpenAI-compatible)", "llama3.1", null],
] as const;
const settings = () => {
  const def = PROVIDERS.find((p) => p[0] === mockAi.provider)![2];
  return {
    language: mockLang,
    provider: mockAi.provider,
    model: mockAi.model || def,
    model_override: mockAi.model,
    base_url: mockAi.base_url,
    providers: PROVIDERS.map(([id, label, dm, url]) => ({ id, label, default_model: dm, default_base_url: url, has_key: !!mockAi.keys[id], needs_key: id !== "custom" })),
    chat_enabled: !!mockAi.keys[mockAi.provider] || (mockAi.provider === "custom" && !!mockAi.base_url),
    share_file_paths: mockToggles.share_file_paths,
    notifications: mockToggles.notifications,
    persist_chat: mockToggles.persist_chat,
    config_path: "~/.config/bosun/config.toml",
  };
};

export async function mockCall(method: string, params: unknown): Promise<unknown> {
  const p = (params ?? {}) as Record<string, unknown>;
  await wait(120);
  switch (method) {
    case "hello":
      return { version: __APP_VERSION__, host: { hostname: "mint-workstation", os: "Linux Mint 22.2", kernel: "6.8.0-71-generic", cpu_brand: "AMD Ryzen 7 5800U" }, chat_enabled: true, model: "claude-sonnet-5-5", config_path: "~/.config/bosun/config.toml", language: mockLang };
    case "admin.info": {
      const a = ADMIN_INFO[String(p.action)];
      return {
        helper_installed: true,
        info: a
          ? { id: p.action, title: L(a.title[0], a.title[1]), effect: L(a.effect[0], a.effect[1]), commands: a.commands, undo: a.undo }
          : { id: p.action, title: String(p.action), effect: "", commands: [String(p.action)], undo: null },
      };
    }
    case "admin.applied":
      return { helper_installed: true, applied: [{ id: "journald-limit", title: L("System logs capped at 500 MB", "Sistem günlükleri 500 MB ile sınırlı"), undo_action: "journald-limit-undo" }] };
    case "__run_admin":
      await wait(900);
      return { ok: true, action: p.action, message: "ok", freed_bytes: 2.7 * GB };
    case "settings.get":
      return settings();
    case "settings.set":
      for (const k of ["share_file_paths", "notifications", "persist_chat"] as const) if (typeof p[k] === "boolean") mockToggles[k] = p[k] as boolean;
      if (p.language) mockLang = p.language === "tr" ? "tr" : "en";
      if (p.provider) {
        mockAi = { ...mockAi, provider: String(p.provider), model: String(p.model ?? ""), base_url: (p.base_url as string) || null };
        if (typeof p.api_key === "string") mockAi.keys[String(p.provider)] = p.api_key !== "";
      }
      return settings();
    case "snapshot":
      return { latest: history[history.length - 1], history, host: {} };
    case "scan":
      if (p.refresh) await wait(900);
      return localize(scan);
    case "findings":
      return mockLang === "tr" ? findings : findingsEn;
    case "plan.create":
      return localize(makePlan((p.categories as string[]) ?? [], (p.large_files as string[]) ?? []));
    case "plan.approve":
      await wait(700);
      return { plan_id: p.plan_id, dry_run: false, trashed: 6, deleted: 0, skipped: [], failed: [], freed_bytes: 4.7 * GB };
    case "plan.cancel":
      return { cancelled: true };
    case "history":
      return localize({ entries: audit });
    case "findings.mute": {
      const src = mockLang === "tr" ? findings : findingsEn;
      for (const f of src.findings as { id: string; muted?: boolean }[]) if (f.id === p.id) f.muted = !!p.muted;
      return src;
    }
    case "scan.cancel":
      return { cancelled: true };
    case "scan.inspect":
      await wait(300);
      return {
        path: p.path, files: 214, bytes: 690 * MB, newest: "2025-11-02T10:00:00Z",
        entries: [
          { name: "Cache", is_dir: true, bytes: 520 * MB }, { name: "Local Storage", is_dir: true, bytes: 120 * MB },
          { name: "GPUCache", is_dir: true, bytes: 48 * MB }, { name: "settings.json", is_dir: false, bytes: 2400 },
        ],
      };
    case "containers":
      return { summary: containers };
    case "startup.list":
      return { apps: startup };
    case "startup.set": {
      const a = startup.find((x) => x.file === p.file);
      if (!a) throw new Error("unknown startup entry");
      if (a.essential && !p.enabled) throw new Error(`${a.name} is part of the desktop and cannot be turned off`);
      a.enabled = !!p.enabled;
      return { ...a };
    }
    case "chat.history":
      return { messages: [] };
    case "chat.reset":
      return { reset: true };
    case "chat": {
      await wait(1100);
      const m = String(p.message).toLowerCase();
      if (m.includes("temiz") || m.includes("disk") || m.includes("yer")) {
        return localize({
          reply: mockLang === "en" ? "Your root disk is 88% full with 58 GB free. The quickest wins are caches: browser (2.4 GB), Cargo (1.1 GB), npm (860 MB) and thumbnails (412 MB). I've put an approval card up for those, about 4.7 GB in total.\n\nSystem logs also take 2.9 GB. Clean them from the Cleanup tab (it asks for your password), or run:\n`sudo journalctl --vacuum-size=500M`" : "Kök disk %88 dolu, 58 GB boş alan kaldı. En hızlı kazanç önbelleklerden: tarayıcı (2,4 GB), Cargo (1,1 GB), npm (860 MB) ve küçük resimler (412 MB). Bunlar için bir plan hazırladım, toplam yaklaşık 4,7 GB.\n\nAyrıca sistem günlükleri 2,9 GB yer kaplıyor. Bunu yönetici yetkisiyle sen çalıştırmalısın:\n`sudo journalctl --vacuum-size=500M`",
          plans: [makePlan(["browser_cache", "cargo_cache", "npm_cache", "thumbnails", "journal"], [], "assistant", L("Root disk is 88% full; safe caches free about 4.7 GB.", "Kök disk %88 dolu; güvenli önbelleklerle yaklaşık 4,7 GB açılır."))],
          tools_used: ["get_system_snapshot", "get_cleanup_candidates", "propose_cleanup"],
        });
      }
      return { reply: mockLang === "en" ? "Overall the system looks healthy. CPU averages 14%, memory is 58% used and the hottest part is 54 °C. The one thing to watch is the root disk at 88%." : "Sistem genel olarak iyi durumda. İşlemci ortalama %14, bellek %58 kullanımda, sıcaklık 54 °C. Tek dikkat çeken şey kök diskin %88 dolu olması.", plans: [], tools_used: ["get_system_snapshot"] };
    }
  }
  throw new Error(`mock: bilinmeyen yöntem ${method}`);
}

export function mockEvents(cb: (e: DaemonEvent) => void): () => void {
  let i = 240;
  const t = setInterval(() => cb({ event: "metrics", data: snap(i++) }), 5000);
  return () => clearInterval(t);
}
