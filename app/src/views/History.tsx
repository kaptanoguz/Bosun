import { useEffect, useState } from "react";
import { api, type AuditEntry } from "../api";
import { clock } from "../format";
import { useT } from "../i18n";
import { toRows } from "./historyRows";

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
