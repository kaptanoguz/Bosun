// Yönetici yetkisi gerektiren bir eylem için onay penceresi. Ne yapılacağını ve tam olarak
// hangi komutların root olarak çalışacağını gösterir; şifreyi sistemin kendi penceresi sorar.
import { useEffect, useState } from "react";
import { api, runAdmin, type AdminInfo, type AdminResult } from "../api";
import { bytes, shortPath } from "../format";
import { useT } from "../i18n";
import { Command, Icon } from "./Icon";

interface Props {
  action: string;
  /** İsteğe bağlı: etkilenecek öğeler (ör. kaldırılacak paketler). */
  items?: string[];
  onClose: (done: boolean) => void;
}

type State = "confirm" | "running" | "done" | "error" | "cancelled" | "missing";

export function AdminSheet({ action, items, onClose }: Props) {
  const { t } = useT();
  const [info, setInfo] = useState<AdminInfo | null>(null);
  const [state, setState] = useState<State>("confirm");
  const [result, setResult] = useState<AdminResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api
      .adminInfo(action)
      .then((r) => {
        setInfo(r.info);
        if (!r.helper_installed) setState("missing");
      })
      .catch((e) => {
        setError(String(e));
        setState("error");
      });
  }, [action]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && state !== "running" && onClose(state === "done");
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  async function go() {
    setState("running");
    setError(null);
    try {
      setResult(await runAdmin(action));
      setState("done");
    } catch (e) {
      const msg = String(e);
      if (msg.includes("CANCELLED")) setState("cancelled");
      else if (msg.includes("HELPER_MISSING")) setState("missing");
      else {
        setError(msg);
        setState("error");
      }
    }
  }

  const busy = state === "running";
  return (
    <div className="scrim" onMouseDown={(e) => e.target === e.currentTarget && !busy && onClose(state === "done")}>
      <section className="sheet" role="dialog" aria-modal="true" aria-labelledby="admin-title">
        {state === "done" ? (
          <>
            <div className="body result">
              <Icon name="check" className="check" />
              <h2 id="admin-title" style={{ font: "600 24px var(--font-cond)", margin: "10px 0 6px" }}>{info?.title}</h2>
              <p style={{ color: "var(--ink-2)" }}>
                {result?.freed_bytes ? t("admin.freed", { size: bytes(result.freed_bytes) }) : t("admin.done")}
              </p>
              {info?.undo && <p style={{ color: "var(--ink-3)", fontSize: 14 }}>{t("admin.undoHint")}</p>}
            </div>
            <footer>
              <button className="btn primary" onClick={() => onClose(true)} autoFocus>{t("plan.done")}</button>
            </footer>
          </>
        ) : (
          <>
            <header>
              <h2 id="admin-title">{info?.title ?? "…"}</h2>
              <p>{info?.effect}</p>
            </header>
            <div className="body">
              {items && items.length > 0 && (
                <>
                  <h3 className="sub-h">{t("admin.affects", { n: items.length })}</h3>
                  <ul className="paths" style={{ marginBottom: 16 }}>
                    {items.slice(0, 12).map((i) => (
                      <li key={i} title={i}>{shortPath(i)}</li>
                    ))}
                    {items.length > 12 && <li>{t("plan.more", { n: items.length - 12 })}</li>}
                  </ul>
                </>
              )}
              {info && (
                <>
                  <h3 className="sub-h">{t("admin.runsAsRoot")}</h3>
                  <ul className="paths cmds">
                    {info.commands.map((c) => (
                      <li key={c}><code>{c}</code></li>
                    ))}
                  </ul>
                </>
              )}
              {state === "missing" ? (
                <div className="callout note">
                  <p style={{ margin: "0 0 6px" }}>{t("admin.missing")}</p>
                  <Command text="./install.sh --helper" />
                </div>
              ) : state === "cancelled" ? (
                <div className="callout note">{t("admin.cancelled")}</div>
              ) : state === "error" ? (
                <div className="callout">{error}</div>
              ) : (
                <div className="callout note">{t("admin.passwordNote")}</div>
              )}
            </div>
            <footer>
              <button className="btn ghost" onClick={() => onClose(false)} disabled={busy}>{t("plan.cancel")}</button>
              <button className="btn primary" onClick={go} disabled={busy || !info || state === "missing"} autoFocus>
                {busy ? <Icon name="refresh" className="icon spin" /> : <Icon name="shield" />}
                {state === "cancelled" || state === "error" ? t("admin.retry") : t("admin.continue")}
              </button>
            </footer>
          </>
        )}
      </section>
    </div>
  );
}
