// Onay ekranı: planın içeriği, toplam boyut, geri alınabilirlik. Onay olmadan hiçbir şey çalışmaz.
import { useEffect, useState } from "react";
import { api, type Plan, type Report } from "../api";
import { bytes, shortPath } from "../format";
import { Command, Icon } from "./Icon";

interface Props {
  plan: Plan;
  onClose: (report?: Report) => void;
}

function useCountdown(until: string) {
  const [left, setLeft] = useState(() => new Date(until).getTime() - Date.now());
  useEffect(() => {
    const t = setInterval(() => setLeft(new Date(until).getTime() - Date.now()), 1000);
    return () => clearInterval(t);
  }, [until]);
  return Math.max(0, left);
}

export function PlanSheet({ plan, onClose }: Props) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [report, setReport] = useState<Report | null>(null);
  const left = useCountdown(plan.expires_at);
  const expired = left === 0;

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && !busy && close();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  function close() {
    if (!report) void api.cancelPlan(plan).catch(() => {});
    onClose(report ?? undefined);
  }

  async function approve() {
    setBusy(true);
    setError(null);
    try {
      setReport(await api.approvePlan(plan));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  const empty = plan.items.length === 0;
  const trashOnly = !plan.has_permanent;
  const mins = Math.floor(left / 60000);
  const secs = Math.floor((left % 60000) / 1000);

  return (
    <div className="scrim" onMouseDown={(e) => e.target === e.currentTarget && !busy && close()}>
      <section className="sheet" role="dialog" aria-modal="true" aria-labelledby="plan-title">
        {report ? (
          <>
            <div className="body result">
              <Icon name="check" className="check" />
              <h2 id="plan-title" className="big">{bytes(report.freed_bytes)}</h2>
              <p>boşaltıldı.</p>
              <p style={{ color: "var(--ink-2)" }}>
                {report.trashed > 0 && `${report.trashed} öğe çöp kutusuna taşındı. `}
                {report.deleted > 0 && `${report.deleted} öğe kalıcı olarak silindi.`}
              </p>
              {(report.failed.length > 0 || report.skipped.length > 0) && (
                <div className="callout note" style={{ textAlign: "left" }}>
                  {report.skipped.length > 0 && <p style={{ margin: 0 }}>{report.skipped.length} öğe güvenlik denetiminden geçmediği için atlandı.</p>}
                  {report.failed.length > 0 && <p style={{ margin: 0 }}>{report.failed.length} öğe silinemedi: {report.failed[0].error}</p>}
                </div>
              )}
            </div>
            <footer>
              <button className="btn primary" onClick={() => onClose(report)} autoFocus>Tamam</button>
            </footer>
          </>
        ) : (
          <>
            <header>
              <h2 id="plan-title">Temizlik planı</h2>
              <p>{plan.reason ?? "Seçtiğiniz kategoriler aşağıda. Onaylamadan hiçbir şey silinmez."}</p>
            </header>
            <div className="body">
              <div className="big">{bytes(plan.total_bytes)}</div>
              <div style={{ color: "var(--ink-3)" }}>
                {plan.items.length} öğe · {trashOnly ? "çöp kutusundan geri alınabilir" : "bir kısmı kalıcı olarak silinecek"}
              </div>

              {plan.has_permanent && (
                <div className="callout">Çöp kutusu boşaltılacak. Bu dosyalar geri getirilemez.</div>
              )}

              <div style={{ marginTop: 16 }}>
                {plan.summary.map((s) => (
                  <div key={s.category}>
                    <div className="sumrow">
                      <span>
                        {s.title} {s.method === "permanent" && <span className="tag permanent">kalıcı</span>}
                      </span>
                      <span>{bytes(s.bytes)} · {s.items} öğe</span>
                    </div>
                    <ul className="paths">
                      {plan.items
                        .filter((i) => i.category === s.category)
                        .slice(0, 4)
                        .map((i) => (
                          <li key={i.path} title={i.path}>{shortPath(i.path)}</li>
                        ))}
                      {s.items > 4 && <li>ve {s.items - 4} öğe daha</li>}
                    </ul>
                  </div>
                ))}
              </div>

              {plan.blocked.length > 0 && (
                <div style={{ marginTop: 20 }}>
                  <h3 style={{ font: "600 16px var(--font-cond)", margin: "0 0 4px" }}>Bu planda olmayanlar</h3>
                  {plan.blocked.map((b, i) => (
                    <div key={i} style={{ marginBottom: 12 }}>
                      <p style={{ margin: 0, color: "var(--ink-2)", fontSize: 14 }}>{b.reason}</p>
                      {b.manual_command && <Command text={b.manual_command} />}
                    </div>
                  ))}
                </div>
              )}

              {error && <div className="callout">{error}</div>}
            </div>
            <footer>
              <span className="countdown">
                {expired ? "Planın süresi doldu" : `Geçerlilik ${mins}:${String(secs).padStart(2, "0")}`}
              </span>
              <button className="btn ghost" onClick={close} disabled={busy}>Vazgeç</button>
              <button
                className={plan.has_permanent ? "btn danger" : "btn primary"}
                onClick={approve}
                disabled={busy || empty || expired}
                autoFocus
              >
                {busy ? <Icon name="refresh" className="icon spin" /> : <Icon name="trash" />}
                {plan.has_permanent ? `Onayla ve sil (${bytes(plan.total_bytes)})` : `Çöp kutusuna taşı (${bytes(plan.total_bytes)})`}
              </button>
            </footer>
          </>
        )}
      </section>
    </div>
  );
}
