const UNITS = ["B", "KB", "MB", "GB", "TB"];

export function bytes(n: number): string {
  if (!Number.isFinite(n) || n <= 0) return "0 B";
  let v = n;
  let u = 0;
  while (v >= 1024 && u < UNITS.length - 1) {
    v /= 1024;
    u++;
  }
  const digits = u === 0 || v >= 100 ? 0 : 1;
  return `${v.toLocaleString("tr-TR", { maximumFractionDigits: digits, minimumFractionDigits: digits })} ${UNITS[u]}`;
}

export function pct(used: number, total: number): number {
  return total > 0 ? (used / total) * 100 : 0;
}

export function percent(n: number): string {
  return `%${Math.round(n)}`;
}

export function uptime(secs: number): string {
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (d > 0) return `${d} gün ${h} sa`;
  if (h > 0) return `${h} sa ${m} dk`;
  return `${m} dk`;
}

const rtf = new Intl.RelativeTimeFormat("tr", { numeric: "auto" });

export function ago(iso: string | null | undefined): string {
  if (!iso) return "—";
  const diff = (new Date(iso).getTime() - Date.now()) / 1000;
  const abs = Math.abs(diff);
  if (abs < 45) return "az önce";
  if (abs < 3600) return rtf.format(Math.round(diff / 60), "minute");
  if (abs < 86400) return rtf.format(Math.round(diff / 3600), "hour");
  return rtf.format(Math.round(diff / 86400), "day");
}

export function clock(iso: string): string {
  return new Date(iso).toLocaleString("tr-TR", { dateStyle: "medium", timeStyle: "short" });
}

/** Ev dizinini ~ ile kısaltır. */
export function shortPath(p: string): string {
  return p.replace(/^\/home\/[^/]+/, "~");
}
