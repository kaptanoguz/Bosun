import { useEffect, useState } from "react";
import { api, type AuditEntry } from "../api";
import { bytes, clock } from "../format";

function describe(e: AuditEntry): { title: string; detail: string; lamp: string } {
  const d = e.data as Record<string, any>;
  const titles = (d.summary as { title: string }[] | undefined)?.map((s) => s.title).join(", ");
  switch (e.kind) {
    case "plan_executed":
      return {
        title: d.dry_run ? `Deneme: ${bytes(d.freed_bytes)} boşaltılabilirdi` : `${bytes(d.freed_bytes)} boşaltıldı`,
        detail: [titles, d.trashed ? `${d.trashed} öğe çöp kutusunda` : "", d.deleted ? `${d.deleted} öğe kalıcı silindi` : "", d.failed ? `${d.failed} hata` : ""].filter(Boolean).join(" · "),
        lamp: d.failed ? "warning" : "ok",
      };
    case "plan_approved":
      return { title: "Plan onaylandı", detail: `${d.items} öğe, ${bytes(d.total_bytes)}`, lamp: "info" };
    case "plan_proposed":
      return { title: "Bosun bir plan önerdi", detail: `${bytes(d.total_bytes)}${titles ? ` · ${titles}` : ""}`, lamp: "info" };
    case "plan_created":
      return { title: "Plan hazırlandı", detail: `${bytes(d.total_bytes)}${titles ? ` · ${titles}` : ""}`, lamp: "info" };
    case "plan_cancelled":
      return { title: "Plan iptal edildi", detail: "", lamp: "info" };
    default:
      return { title: e.kind, detail: "", lamp: "info" };
  }
}

export function History({ refreshKey }: { refreshKey: number }) {
  const [entries, setEntries] = useState<AuditEntry[] | null>(null);
  useEffect(() => {
    api.history().then((r) => setEntries(r.entries)).catch(() => setEntries([]));
  }, [refreshKey]);

  return (
    <div className="view">
      <div className="view-head">
        <h2>Geçmiş</h2>
        <p>Bosun'un yaptığı her işlem burada kayıtlıdır.</p>
      </div>
      <div className="rows timeline">
        {entries === null ? (
          <div className="empty">Yükleniyor…</div>
        ) : entries.length === 0 ? (
          <div className="empty">Henüz bir temizlik yapılmadı.</div>
        ) : (
          entries.map((e, i) => {
            const x = describe(e);
            return (
              <div className="row" key={i}>
                <span className={`lamp ${x.lamp}`} />
                <div style={{ flex: 1 }}>
                  <b>{x.title}</b>
                  {x.detail && <p>{x.detail}</p>}
                </div>
                <time dateTime={e.ts}>{clock(e.ts)}</time>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}
