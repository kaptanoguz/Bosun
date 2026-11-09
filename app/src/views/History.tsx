import { useEffect, useState } from "react";
import { api, type AuditEntry } from "../api";
import { bytes, clock, shortPath } from "../format";
import { useT, type T } from "../i18n";

type Data = Record<string, any>;

interface Row {
  ts: string;
  title: string;
  detail: string;
  lamp: "ok" | "warning" | "info";
  dim?: boolean;
}

const PLAN_KINDS = new Set(["plan_created", "plan_proposed", "plan_approved", "plan_executed", "plan_cancelled"]);

const summaryTitles = (d: Data | undefined) => (d?.summary as { title: string }[] | undefined)?.map((s) => s.title).join(", ") ?? "";

/** Bir planın kayıtlarını (hazırlandı, onaylandı, uygulandı…) tek satıra indirir. */
function planRow(events: AuditEntry[], t: T): Row {
  const by = (k: string) => events.find((e) => e.kind === k)?.data as Data | undefined;
  const start = by("plan_created") ?? by("plan_proposed");
  const done = by("plan_executed");
  const fromChat = !!by("plan_proposed");
  const titles = summaryTitles(done ?? start);
  const ts = events[0].ts;

  if (done) {
    return {
      ts,
      title: done.dry_run ? t("history.dryRun", { size: bytes(done.freed_bytes) }) : t("history.freed", { size: bytes(done.freed_bytes) }),
      detail: [
        titles,
        done.trashed ? t("history.inTrash", { n: done.trashed }) : "",
        done.deleted ? t("history.deletedN", { n: done.deleted }) : "",
        done.failed ? t("history.errors", { n: done.failed }) : "",
        fromChat ? t("history.fromChat") : "",
      ]
        .filter(Boolean)
        .join(" · "),
      lamp: done.failed ? "warning" : "ok",
    };
  }
  const approved = by("plan_approved");
  if (approved) {
    return { ts, title: t("history.approved"), detail: t("history.approvedDetail", { n: approved.items, size: bytes(approved.total_bytes) }), lamp: "info" };
  }
  return {
    ts,
    title: fromChat ? t("history.proposed") : t("history.notApplied"),
    detail: [start ? bytes(start.total_bytes) : "", titles].filter(Boolean).join(" · "),
    lamp: "info",
    dim: true,
  };
}

function otherRow(e: AuditEntry, t: T): Row {
  const d = e.data as Data;
  switch (e.kind) {
    case "admin_action": {
      const name = String(d.title ?? d.action);
      return d.ok
        ? { ts: e.ts, title: name, detail: d.freed_bytes ? t("history.freed", { size: bytes(d.freed_bytes) }) : t("history.adminOk"), lamp: "ok" }
        : { ts: e.ts, title: t("history.adminFail", { action: name }), detail: String(d.error ?? ""), lamp: "warning" };
    }
    case "startup_toggled":
      return { ts: e.ts, title: t(d.enabled ? "history.startupOn" : "history.startupOff", { name: d.name }), detail: d.file ? shortPath(String(d.file)) : "", lamp: "info" };
    default:
      return { ts: e.ts, title: e.kind, detail: "", lamp: "info" };
  }
}

/** Kayıtlar en yenisi başta gelir; aynı plana ait olanlar ilk göründükleri yerde birleşir. */
export function toRows(entries: AuditEntry[], t: T): Row[] {
  const plans = new Map<string, AuditEntry[]>();
  for (const e of entries) {
    const id = (e.data as Data).plan_id;
    if (PLAN_KINDS.has(e.kind) && typeof id === "string") {
      const list = plans.get(id);
      if (list) list.push(e);
      else plans.set(id, [e]);
    }
  }
  const rows: Row[] = [];
  const seen = new Set<string>();
  for (const e of entries) {
    const id = (e.data as Data).plan_id;
    if (PLAN_KINDS.has(e.kind)) {
      if (typeof id !== "string") rows.push(planRow([e], t));
      else if (!seen.has(id)) {
        seen.add(id);
        rows.push(planRow(plans.get(id)!, t));
      }
    } else {
      rows.push(otherRow(e, t));
    }
  }
  return rows;
}

export function History({ refreshKey }: { refreshKey: number }) {
  const { t } = useT();
  const [entries, setEntries] = useState<AuditEntry[] | null>(null);
  useEffect(() => {
    api.history().then((r) => setEntries(r.entries)).catch(() => setEntries([]));
  }, [refreshKey]);

  const rows = entries ? toRows(entries, t) : null;

  return (
    <div className="view narrow">
      <div className="view-head">
        <h2>{t("history.title")}</h2>
        <p>{t("history.intro")}</p>
      </div>
      <div className="rows timeline">
        {rows === null ? (
          <div className="empty">{t("history.loading")}</div>
        ) : rows.length === 0 ? (
          <div className="empty">{t("history.empty")}</div>
        ) : (
          rows.map((r, i) => (
            <div className={`row${r.dim ? " muted" : ""}`} key={i}>
              <span className={`lamp ${r.lamp}`} />
              <div className="grow">
                <b>{r.title}</b>
                {r.detail && <p>{r.detail}</p>}
              </div>
              <time dateTime={r.ts}>{clock(r.ts)}</time>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
