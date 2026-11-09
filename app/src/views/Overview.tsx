import { useState } from "react";
import type { FindingsPayload, Snapshot } from "../api";
import { Dial } from "../components/Dial";
import { Command, Icon } from "../components/Icon";
import { Trend } from "../components/Trend";
import { bytes, pct, uptime } from "../format";
import { useT } from "../i18n";

interface Props {
  latest: Snapshot | null;
  history: Snapshot[];
  findings: FindingsPayload | null;
  onGoClean: () => void;
  onAdmin: (action: string) => void;
  onMute: (id: string, muted: boolean) => void;
}

export function Overview({ latest, history, findings, onGoClean, onAdmin, onMute }: Props) {
  const { t } = useT();
  const [series, setSeries] = useState<"cpu" | "mem">("cpu");
  if (!latest) return <div className="empty">{t("overview.waiting")}</div>;

  const root = latest.disks.find((d) => d.mount === "/") ?? latest.disks[0];
  const diskPct = root ? pct(root.total - root.available, root.total) : 0;
  const memPct = pct(latest.mem_used, latest.mem_total);
  const swapPct = pct(latest.swap_used, latest.swap_total);
  const temp = latest.temps.length ? Math.max(...latest.temps.map((t) => t.celsius)) : null;

  const values = history.map((h) => (series === "cpu" ? h.cpu_percent : pct(h.mem_used, h.mem_total)));
  const times = history.map((h) => h.ts);
  const otherDisks = latest.disks.filter((d) => d !== root);

  return (
    <div className="view">
      <div className="dials">
        <Dial label={t("overview.cpu")} value={latest.cpu_percent} detail={t("overview.cores", { n: latest.cpu_count, load: latest.load_avg[0].toFixed(2) })} warn={85} crit={95} />
        <Dial label={t("overview.memory")} value={memPct} detail={`${bytes(latest.mem_used)} / ${bytes(latest.mem_total)}`} warn={85} crit={92} />
        <Dial label={t("overview.swap")} value={swapPct} detail={latest.swap_total ? `${bytes(latest.swap_used)} / ${bytes(latest.swap_total)}` : t("overview.noSwap")} warn={50} crit={80} />
        <Dial label={t("overview.disk", { mount: root?.mount ?? "" })} value={diskPct} detail={root ? t("overview.free", { size: bytes(root.available) }) : "—"} warn={85} crit={95} />
      </div>

      <section className="panel">
        <div className="panel-head">
          <h3>{t("overview.lastHour")}</h3>
          <span className="hint">
            {t("overview.uptime", { uptime: uptime(latest.uptime_secs) })}
            {temp !== null && ` · ${t("overview.hottest", { t: Math.round(temp) })}`}
          </span>
          <div className="seg" role="group" aria-label={t("overview.chart")}>
            <button aria-pressed={series === "cpu"} onClick={() => setSeries("cpu")}>{t("overview.cpu")}</button>
            <button aria-pressed={series === "mem"} onClick={() => setSeries("mem")}>{t("overview.memory")}</button>
          </div>
        </div>
        <Trend values={values} times={times} />
      </section>

      <div className="two">
        <section className="panel">
          <div className="panel-head">
            <h3>{t("overview.findings")}</h3>
          </div>
          <div className="rows">
            {!findings || findings.findings.length === 0 ? (
              <div className="row">
                <span className="lamp ok" />
                <div>
                  <b>{t("overview.allGood")}</b>
                  <p>{t("overview.allGoodBody")}</p>
                </div>
              </div>
            ) : (
              findings.findings.map((f) => (
                <div className={`row${f.muted ? " muted" : ""}`} key={f.id}>
                  <span className={`lamp ${f.muted ? "info" : f.severity}`} />
                  <div style={{ flex: 1 }}>
                    <b>{f.title}</b>
                    <p>{f.detail}</p>
                    {f.severity !== "info" && (
                      <button className="link mute" onClick={() => onMute(f.id, !f.muted)}>
                        {f.muted ? t("overview.unmute") : t("overview.mute")}
                      </button>
                    )}
                    {(f.id === "reclaimable" || f.category) && (
                      <button className="link" onClick={onGoClean} style={{ marginTop: 4 }}>{t("overview.reviewCleanup")}</button>
                    )}
                  </div>
                </div>
              ))
            )}
          </div>
          {otherDisks.length > 0 && (
            <div className="rows" style={{ marginTop: 12 }}>
              {otherDisks.map((d) => {
                const p = pct(d.total - d.available, d.total);
                return (
                  <div className="row" key={d.mount} style={{ alignItems: "center" }}>
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <b>{d.mount}</b>
                      <div className="bar"><i style={{ width: `${p}%`, background: p >= 85 ? "var(--flag)" : undefined }} /></div>
                    </div>
                    <span style={{ color: "var(--ink-2)", fontSize: 14, whiteSpace: "nowrap" }}>{t("overview.free", { size: bytes(d.available) })}</span>
                  </div>
                );
              })}
            </div>
          )}
        </section>

        <section className="panel">
          <div className="panel-head">
            <h3>{t("overview.advice")}</h3>
            <span className="hint">{t("overview.adviceHint")}</span>
          </div>
          <div className="rows">
            {!findings || findings.advice.length === 0 ? (
              <div className="empty">{t("overview.noAdvice")}</div>
            ) : (
              findings.advice.map((a) => (
                <div className="row" key={a.id}>
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <b>{a.title}</b>
                    <p>{a.detail}</p>
                    <div className="change">
                      <s>{a.current}</s>
                      <ins>{a.suggested}</ins>
                    </div>
                    <div className="admin-actions">
                      {a.admin_action && (
                        <button className="btn small" onClick={() => onAdmin(a.admin_action!)}>
                          <Icon name="shield" />
                          {t("overview.apply")}
                        </button>
                      )}
                      {a.command && (
                        <details>
                          <summary>{t("clean.orTerminal")}</summary>
                          <Command text={a.command} />
                        </details>
                      )}
                    </div>
                  </div>
                </div>
              ))
            )}
          </div>
        </section>
      </div>
    </div>
  );
}
