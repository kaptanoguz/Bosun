// Biçimlendirme yardımcıları. Yerel ayar, seçili dile göre `setLocale` ile değişir.

const UNITS = ["B", "KB", "MB", "GB", "TB"];
let locale = "en-US";
let lang: "en" | "tr" = "en";
let rtf = new Intl.RelativeTimeFormat("en", { numeric: "auto" });

export function setLocale(l: "en" | "tr") {
  lang = l;
  locale = l === "tr" ? "tr-TR" : "en-US";
  rtf = new Intl.RelativeTimeFormat(l, { numeric: "auto" });
}

export function bytes(n: number): string {
  if (!Number.isFinite(n) || n <= 0) return "0 B";
  let v = n;
  let u = 0;
  while (v >= 1024 && u < UNITS.length - 1) {
    v /= 1024;
    u++;
  }
  const digits = u === 0 || v >= 100 ? 0 : 1;
  return `${v.toLocaleString(locale, { maximumFractionDigits: digits, minimumFractionDigits: digits })} ${UNITS[u]}`;
}

export function num(n: number): string {
  return n.toLocaleString(locale);
}

export function pct(used: number, total: number): number {
  return total > 0 ? (used / total) * 100 : 0;
}

/** Türkçede yüzde işareti başa gelir (%42), İngilizcede sona (42%). */
export function percent(n: number): string {
  const r = Math.round(n);
  return lang === "tr" ? `%${r}` : `${r}%`;
}

export function uptime(secs: number): string {
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const [dl, hl, ml] = lang === "tr" ? ["gün", "sa", "dk"] : ["d", "h", "min"];
  if (d > 0) return `${d} ${dl} ${h} ${hl}`;
  if (h > 0) return `${h} ${hl} ${m} ${ml}`;
  return `${m} ${ml}`;
}

export function ago(iso: string | null | undefined): string {
  if (!iso) return "—";
  const diff = (new Date(iso).getTime() - Date.now()) / 1000;
  const abs = Math.abs(diff);
  if (abs < 45) return lang === "tr" ? "az önce" : "just now";
  if (abs < 3600) return rtf.format(Math.round(diff / 60), "minute");
  if (abs < 86400) return rtf.format(Math.round(diff / 3600), "hour");
  return rtf.format(Math.round(diff / 86400), "day");
}

export function clock(iso: string): string {
  return new Date(iso).toLocaleString(locale, { dateStyle: "medium", timeStyle: "short" });
}

export function timeOfDay(iso: string): string {
  return new Date(iso).toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit" });
}

/** Ev dizinini ~ ile kısaltır. */
export function shortPath(p: string): string {
  return p.replace(/^\/home\/[^/]+/, "~");
}

/** Eşiklere göre seviye: iyi, dikkat, kritik. */
export function level(v: number, warn: number, crit: number): "ok" | "warning" | "critical" {
  return v >= crit ? "critical" : v >= warn ? "warning" : "ok";
}

/** Ev klasöründen bu klasöre kadar olan adımlar. */
export function crumbs(path: string, home: string): { name: string; path: string }[] {
  const out = [{ name: "", path: home }];
  if (path === home || !path.startsWith(`${home}/`)) return out;
  let cur = home;
  for (const part of path.slice(home.length + 1).split("/")) {
    cur = `${cur}/${part}`;
    out.push({ name: part, path: cur });
  }
  return out;
}
