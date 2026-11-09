import { useEffect, useState } from "react";
import { api, type AuditEntry } from "../api";
import { bytes, clock } from "../format";
import { useT, type T } from "../i18n";

function describe(e: AuditEntry, t: T): { title: string; detail: string; lamp: string } {
  const d = e.data as Record<string, any>;
  const titles = (d.summary as { title: string }[] | undefined)?.map((s) => s.title).join(", ");
  switch (e.kind) {
    case "plan_executed":
      return {
        title: d.dry_run ? t("history.dryRun", { size: bytes(d.freed_bytes) }) : t("history.freed", { size: bytes(d.freed_bytes) }),
        detail: [
          titles,
          d.trashed ? t("history.inTrash", { n: d.trashed }) : "",
          d.deleted ? t("history.deletedN", { n: d.deleted }) : "",
          d.failed ? t("history.errors", { n: d.failed }) : "",
        ]
          .filter(Boolean)
          .join(" · "),
        lamp: d.failed ? "warning" : "ok",
      };
    case "plan_approved":
      return { title: t("history.approved"), detail: t("history.approvedDetail", { n: d.items, size: bytes(d.total_bytes) }), lamp: "info" };
    case "plan_proposed":
      return { title: t("history.proposed"), detail: `${bytes(d.total_bytes)}${titles ? ` · ${titles}` : ""}`, lamp: "info" };
    case "plan_created":
      return { title: t("history.created"), detail: `${bytes(d.total_bytes)}${titles ? ` · ${titles}` : ""}`, lamp: "info" };
    case "plan_cancelled":
      return { title: t("history.cancelled"), detail: "", lamp: "info" };
    default:
      return { title: e.kind, detail: "", lamp: "info" };
  }
}

export function History({ refreshKey }: { refreshKey: number }) {
  const { t } = useT();
  const [entries, setEntries] = useState<AuditEntry[] | null>(null);
  useEffect(() => {
    api.history().then((r) => setEntries(r.entries)).catch(() => setEntries([]));
  }, [refreshKey]);

  return (
    <div className="view">
      <div className="view-head">
        <h2>{t("history.title")}</h2>
        <p>{t("history.intro")}</p>
      </div>
      <div className="rows timeline">
        {entries === null ? (
          <div className="empty">{t("history.loading")}</div>
        ) : entries.length === 0 ? (
          <div className="empty">{t("history.empty")}</div>
        ) : (
          entries.map((e, i) => {
            const x = describe(e, t);
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
