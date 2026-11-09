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
      { mount: "/home/oguz/Arşiv", device: "/dev/sda1", fs: "ext4", total: 931 * GB, available: 402 * GB },
    ],
    temps: [{ label: "Package id 0", celsius: 54 }],
  };
}

const history = Array.from({ length: 240 }, (_, i) => snap(i));

const scan = {
  ts: now(),
  duration_ms: 812,
  categories: ([
    { id: "browser_cache", title: "Tarayıcı önbelleği", description: "Web sayfalarının geçici kopyaları. Oturumlar ve şifreler etkilenmez; tarayıcı açıksa kapatmak daha iyi olur.", safety: "safe", method: "trash", bulk_selectable: true, manual_command: null, bytes: 2.4 * GB, item_count: 3, items: [{ path: "/home/oguz/.cache/mozilla", bytes: 1.6 * GB, modified: now() }, { path: "/home/oguz/.cache/google-chrome", bytes: 0.8 * GB, modified: now() }] },
    { id: "cargo_cache", title: "Cargo indirme önbelleği", description: "Rust crate arşivleri (.crate). Bir sonraki derlemede yeniden indirilir.", safety: "safe", method: "trash", bulk_selectable: true, manual_command: null, bytes: 1.1 * GB, item_count: 1, items: [{ path: "/home/oguz/.cargo/registry/cache/index.crates.io-1949cf8c6b5b557f", bytes: 1.1 * GB, modified: now() }] },
    { id: "npm_cache", title: "npm önbelleği", description: "npm'in indirdiği paket arşivleri. Bir sonraki kurulumda yeniden indirilir.", safety: "safe", method: "trash", bulk_selectable: true, manual_command: null, bytes: 860 * MB, item_count: 2, items: [{ path: "/home/oguz/.npm/_cacache/content-v2", bytes: 840 * MB, modified: now() }] },
    { id: "thumbnails", title: "Küçük resim önbelleği", description: "Dosya yöneticisinin ürettiği önizlemeler. Gerektiğinde yeniden oluşturulur.", safety: "safe", method: "trash", bulk_selectable: true, manual_command: null, bytes: 412 * MB, item_count: 3, items: [{ path: "/home/oguz/.cache/thumbnails/large", bytes: 300 * MB, modified: now() }] },
    { id: "user_cache", title: "Eski uygulama önbellekleri", description: "~/.cache içinde 30 günden uzun süredir değişmemiş dosyalar. Uygulamalar gerekirse yeniden indirir.", safety: "caution", method: "trash", bulk_selectable: true, manual_command: null, bytes: 1.3 * GB, item_count: 4210, items: [{ path: "/home/oguz/.cache/ms-playwright/chromium-1140/chrome-linux/chrome", bytes: 380 * MB, modified: now() }] },
    { id: "old_logs", title: "Eski kullanıcı günlükleri", description: "~/.local/state altında 30 günden eski .log dosyaları.", safety: "caution", method: "trash", bulk_selectable: true, manual_command: null, bytes: 38 * MB, item_count: 12, items: [] },
    { id: "trash", title: "Çöp kutusu", description: "Çöp kutusundaki dosyalar. Bu işlem KALICIDIR, geri alınamaz.", safety: "caution", method: "permanent", bulk_selectable: true, manual_command: null, bytes: 3.7 * GB, item_count: 46, items: [] },
    { id: "journal", title: "Sistem günlükleri (journald)", description: "systemd günlükleri. 500 MB'a indirilmesi önerilir; yönetici yetkisi gerekir.", safety: "safe", method: "privileged", bulk_selectable: false, manual_command: "sudo journalctl --vacuum-size=500M", bytes: 2.9 * GB, item_count: 1, items: [] },
    { id: "apt_cache", title: "APT paket önbelleği", description: "Kurulmuş paketlerin indirilmiş .deb dosyaları. `sudo apt-get clean` ile temizlenir.", safety: "safe", method: "privileged", bulk_selectable: false, manual_command: "sudo apt-get clean", bytes: 640 * MB, item_count: 88, items: [] },
    { id: "large_files", title: "İndirilenlerdeki büyük dosyalar", description: "500 MB'tan büyük indirilmiş dosyalar. Otomatik silinmez; tek tek seçmelisiniz.", safety: "review_only", method: "trash", bulk_selectable: false, manual_command: null, bytes: 7.9 * GB, item_count: 3, items: [
      { path: "/home/oguz/İndirilenler/linuxmint-22.2-cinnamon-64bit.iso", bytes: 2.9 * GB, modified: "2026-09-02T10:00:00Z" },
      { path: "/home/oguz/İndirilenler/ADA_drydock_survey_2026.zip", bytes: 3.1 * GB, modified: "2026-10-11T10:00:00Z" },
      { path: "/home/oguz/İndirilenler/android-studio-2026.1.tar.gz", bytes: 1.9 * GB, modified: "2026-07-20T10:00:00Z" },
    ] },
    { id: "flatpak_cache", title: "Flatpak uygulama önbellekleri", description: "Flatpak uygulamalarının önbellek klasörleri (~/.var/app/*/cache). Uygulamalar gerektiğinde yeniden oluşturur.", safety: "safe", method: "trash", bulk_selectable: true, manual_command: null, bytes: 530 * MB, item_count: 4, items: [{ path: "/home/oguz/.var/app/org.gimp.GIMP/cache/gegl-0.4", bytes: 410 * MB, modified: now() }] },
    { id: "broken_links", title: "Kırık bağlantılar", description: "Ev klasörünüzde hedefi artık olmayan sembolik bağlantılar. Yalnızca bağlantının kendisi silinir.", safety: "safe", method: "unlink", bulk_selectable: true, manual_command: null, bytes: 0, item_count: 5, items: [
      { path: "/home/oguz/.local/bin/node18", bytes: 0, modified: now() },
      { path: "/home/oguz/Masaüstü/Eski Proje", bytes: 0, modified: now() },
      { path: "/home/oguz/.local/share/applications/defaults.list", bytes: 0, modified: now() },
    ] },
    { id: "orphan_launchers", title: "Kırık kısayollar", description: "Çalıştırdığı program artık kurulu olmayan menü ve otomatik başlatma kısayolları.", safety: "caution", method: "trash", bulk_selectable: true, manual_command: null, bytes: 1800, item_count: 3, items: [
      { path: "/home/oguz/.local/share/applications/skypeforlinux.desktop", bytes: 600, modified: now() },
      { path: "/home/oguz/.config/autostart/dropbox.desktop", bytes: 400, modified: now() },
    ] },
    { id: "leftover_configs", title: "Eski programlardan kalmış olabilecek klasörler", description: "~/.config ve ~/.local/share içinde kurulu hiçbir programla eşleşmeyen ve 6 aydır değişmemiş klasörler. Bu bir tahmindir: işaretlemeden önce her birine bakın.", safety: "review_only", method: "trash", bulk_selectable: false, manual_command: null, bytes: 1.2 * GB, item_count: 3, items: [
      { path: "/home/oguz/.config/skypeforlinux", bytes: 690 * MB, modified: "2025-11-02T10:00:00Z" },
      { path: "/home/oguz/.local/share/Steam-old", bytes: 480 * MB, modified: "2025-06-02T10:00:00Z" },
      { path: "/home/oguz/.config/teamviewer", bytes: 42 * MB, modified: "2026-01-14T10:00:00Z" },
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
    { id: "reclaimable", severity: "info", title: "4.8 GB can be safely cleaned", detail: "Caches and temporary files. Review them on the Cleanup tab.", category: null },
    { id: "big:journal", severity: "info", title: "System logs (journald): 2.9 GB", detail: "Run in a terminal: sudo journalctl --vacuum-size=500M", category: "journal" },
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
    { id: "reclaimable", severity: "info", title: "4,8 GB güvenle temizlenebilir", detail: "Önbellekler ve geçici dosyalar. Temizlik sekmesinden gözden geçirebilirsiniz.", category: null },
    { id: "big:journal", severity: "info", title: "Sistem günlükleri (journald): 2,9 GB", detail: "Terminalde çalıştırın: sudo journalctl --vacuum-size=500M", category: "journal" },
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
    if (c.items.length === 0) items.push({ category: id, path: `/home/oguz/.local/share/${id}`, bytes: c.bytes, method: c.method as Plan["items"][0]["method"] });
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

const audit = [
  { ts: new Date(Date.now() - 86400000 * 2).toISOString(), kind: "plan_executed", data: { freed_bytes: 3.2 * GB, trashed: 14, deleted: 0, failed: 0, skipped: 0, summary: [{ title: "Tarayıcı önbelleği" }, { title: "Küçük resim önbelleği" }] } },
  { ts: new Date(Date.now() - 86400000 * 2 - 30000).toISOString(), kind: "plan_approved", data: { items: 14, total_bytes: 3.2 * GB } },
  { ts: new Date(Date.now() - 86400000 * 2 - 60000).toISOString(), kind: "plan_proposed", data: { origin: "assistant", total_bytes: 3.2 * GB } },
];

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
  "Tarayıcı önbelleği": "Browser cache",
  "Cargo indirme önbelleği": "Cargo download cache",
  "npm önbelleği": "npm cache",
  "Küçük resim önbelleği": "Thumbnail cache",
  "Eski uygulama önbellekleri": "Stale app caches",
  "Eski kullanıcı günlükleri": "Old user logs",
  "Çöp kutusu": "Trash",
  "Sistem günlükleri (journald)": "System logs (journald)",
  "APT paket önbelleği": "APT package cache",
  "İndirilenlerdeki büyük dosyalar": "Large files in Downloads",
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
      return { version: "0.1.0", host: { hostname: "oguz-mint", os: "Linux Mint 22.2", kernel: "6.8.0-71-generic", cpu_brand: "AMD Ryzen 7 5800U" }, chat_enabled: true, model: "claude-sonnet-5-5", config_path: "~/.config/bosun/config.toml", language: mockLang };
    case "admin.info":
      return {
        helper_installed: true,
        info: {
          id: p.action,
          title: L("Run administrator action", "Yönetici işlemini çalıştır"),
          effect: L("Requires your password.", "Şifrenizi gerektirir."),
          commands: [String(p.action)],
          undo: String(p.action).endsWith("-undo") ? null : `${p.action}-undo`,
        },
      };
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
      return { entries: audit };
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
    case "chat.history":
      return { messages: [] };
    case "chat.reset":
      return { reset: true };
    case "chat": {
      await wait(1100);
      const m = String(p.message).toLowerCase();
      if (m.includes("temiz") || m.includes("disk") || m.includes("yer")) {
        return localize({
          reply: mockLang === "en" ? "Your root disk is 88% full with 58 GB free. The quickest wins are caches: browser (2.4 GB), Cargo (1.1 GB), npm (860 MB) and thumbnails (412 MB). I've put an approval card up for those, about 4.7 GB in total.\n\nSystem logs also take 2.9 GB. That one needs administrator rights, so run it yourself:\n`sudo journalctl --vacuum-size=500M`" : "Kök disk %88 dolu, 58 GB boş alan kaldı. En hızlı kazanç önbelleklerden: tarayıcı (2,4 GB), Cargo (1,1 GB), npm (860 MB) ve küçük resimler (412 MB). Bunlar için bir plan hazırladım, toplam yaklaşık 4,7 GB.\n\nAyrıca sistem günlükleri 2,9 GB yer kaplıyor. Bunu yönetici yetkisiyle sen çalıştırmalısın:\n`sudo journalctl --vacuum-size=500M`",
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
