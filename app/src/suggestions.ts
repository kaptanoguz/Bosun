// Sohbetin açılış önerileri: o an gerçekten görülen bulgulardan üretilir,
// yetmezse genel sorularla tamamlanır.
import type { Finding } from "./api";
import type { T } from "./i18n";

const MAX = 4;

export function starters(findings: Finding[], t: T): string[] {
  const out: string[] = [];
  for (const f of findings) {
    if (f.muted) continue;
    if (f.id.startsWith("disk:")) {
      const mount = f.id.slice(5);
      out.push(t("chat.askDisk", { mount: mount === "/" ? t("chat.systemDisk") : mount }));
    }
    else if (f.id.startsWith("leak:")) out.push(t("chat.askLeak", { app: f.id.slice(5) }));
    else if (f.id === "cpu") out.push(t("chat.askCpu"));
    else if (f.id === "mem" || f.id === "swap") out.push(t("chat.askMemory"));
    else if (f.id === "temp") out.push(t("chat.askHeat"));
    else if (f.id.startsWith("units:")) out.push(t("chat.askUnits", { units: f.detail }));
    else if (f.id === "reclaimable") out.push(t("chat.askReclaim"));
  }
  for (const k of ["chat.s1", "chat.s3", "chat.s4", "chat.s2"] as const) out.push(t(k));
  return [...new Set(out)].slice(0, MAX);
}
