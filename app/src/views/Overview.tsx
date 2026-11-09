import { useState } from "react";
import type { ContainerSummary, FindingsPayload, Snapshot } from "../api";
import { Command, Icon } from "../components/Icon";
import { Spark } from "../components/Meter";
import { bytes, level, pct, percent, uptime } from "../format";
import { useT, type Key } from "../i18n";

interface Props {
  latest: Snapshot | null;
  history: Snapshot[];
  findings: FindingsPayload | null;
  containers: ContainerSummary | null;
  onGoClean: () => void;
  onAdmin: (action: string) => void;
  onMute: (id: string, muted: boolean) => void;
}

export function Overview({ latest, history, findings, containers, onGoClean, onAdmin, onMute }: Props) {
  const { t } = useT();
  const [sort, setSort] = useState<"cpu" | "mem">("cpu");
  const [showMuted, setShowMuted] = useState(false);
  if (!latest) return <div className="empty">{t("overview.waiting")}</div>;

  const root = latest.disks.find((d) => d.mount === "/") ?? latest.disks[0];
  const diskPct = root ? pct(root.total - root.available, root.total) : 0;
  const memPct = pct(latest.mem_used, latest.mem_total);
  const temp = latest.temps.length ? Math.max(...latest.temps.map((x) => x.celsius)) : null;
  const otherDisks = latest.disks.filter((d) => d !== root);
  const series = (f: (s: Snapshot) => number) => history.map(f);

  const all = findings?.findings ?? [];
  const active = all.filter((f) => !f.muted);
  const muted = all.filter((f) => f.muted);

  const apps = [...(latest.top_apps ?? [])].sort((a, b) => (sort === "cpu" ? b.cpu_percent - a.cpu_percent : b.mem_bytes - a.mem_bytes)).slice(0, 8);

  return (
    <div className="view">
      <div className="cards">
        <div className="card">
          <div className="card-text">
            <h3>{t("overview.cpu")}</h3>
            <b className={`lvl-${level(latest.cpu_percent, 85, 95)}`}>{percent(latest.cpu_percent)}</b>
            <small>{t("overview.cores", { n: latest.cpu_count })}</small>
          </div>
          <Spark values={series((s) => s.cpu_percent)} max={100} />
        </div>
        <div className="card">
          <div className="card-text">
            <h3>{t("overview.memory")}</h3>
            <b className={`lvl-${level(memPct, 85, 92)}`}>{percent(memPct)}</b>
            <small>{t("overview.of", { used: bytes(latest.mem_used), total: bytes(latest.mem_total) })}</small>
            <small>
              {latest.swap_total
                ? t("overview.swapUse", { used: bytes(latest.swap_used), total: bytes(latest.swap_total) })
                : t("overview.noSwap")}
            </small>
          </div>
          <Spark values={series((s) => pct(s.mem_used, s.mem_total))} max={100} />
        </div>
        <div className="card">
          <div className="card-text">
            <h3>{t("overview.disk", { mount: root?.mount ?? "" })}</h3>
            <b className={`lvl-${level(diskPct, 85, 95)}`}>{percent(diskPct)}</b>
            <small>{root ? t("overview.free", { size: bytes(root.available) }) : "—"}</small>
            {otherDisks.length > 0 && <small>{t("overview.moreDisks", { n: otherDisks.length })}</small>}
          </div>
          <div className="bar card-bar"><i style={{ width: `${diskPct}%`, background: diskPct >= 85 ? "var(--flag)" : undefined }} /></div>
        </div>
        <div className="card">
          <div className="card-text">
            <h3>{t("overview.diskIo")}</h3>
            <b>{bytes((latest.disk_read_bps ?? 0) + (latest.disk_write_bps ?? 0))}/s</b>
            <small>{t("overview.read", { v: bytes(latest.disk_read_bps ?? 0) })}</small>
            <small>{t("overview.write", { v: bytes(latest.disk_write_bps ?? 0) })}</small>
          </div>
          <Spark values={series((s) => s.disk_read_bps ?? 0)} second={series((s) => s.disk_write_bps ?? 0)} />
        </div>
        <div className="card">
          <div className="card-text">
            <h3>{t("overview.network")}</h3>
            <b>{bytes((latest.net_rx_bps ?? 0) + (latest.net_tx_bps ?? 0))}/s</b>
            <small>{t("overview.down", { v: bytes(latest.net_rx_bps ?? 0) })}</small>
            <small>{t("overview.up", { v: bytes(latest.net_tx_bps ?? 0) })}</small>
          </div>
          <Spark values={series((s) => s.net_rx_bps ?? 0)} second={series((s) => s.net_tx_bps ?? 0)} />
        </div>
        {latest.gpu && (
          <div className="card">
            <div className="card-text">
              <h3>{t("overview.gpu")}</h3>
              <b>{percent(latest.gpu.util_percent)}</b>
              <small>{latest.gpu.mem_total > 0 ? `${bytes(latest.gpu.mem_used)} / ${bytes(latest.gpu.mem_total)}` : latest.gpu.name}</small>
              {latest.gpu.temp_c != null && <small>{Math.round(latest.gpu.temp_c)} °C</small>}
            </div>
            <Spark values={series((s) => s.gpu?.util_percent ?? 0)} max={100} />
          </div>
        )}
      </div>

      <p className="facts">
        {t("overview.uptime", { uptime: uptime(latest.uptime_secs) })}
        {" · "}
        {t("overview.loadLine", { a: latest.load_avg[0].toFixed(2), b: latest.load_avg[1].toFixed(2), c: latest.load_avg[2].toFixed(2) })}
        {temp !== null && ` · ${t("overview.hottest", { t: Math.round(temp) })}`}
        {latest.battery && (
          <span className={latest.battery.percent <= 15 && latest.battery.status === "Discharging" ? "warn" : ""}>
            {" · "}
            {t("overview.battery")} {percent(latest.battery.percent)} ({t(`battery.${latest.battery.status.toLowerCase().replace(/ /g, "_")}` as Key)}
            {latest.battery.power_w ? `, ${latest.battery.power_w.toFixed(1)} W` : ""})
          </span>
        )}
      </p>

      <div className="alerts">
        {active.map((f) => (
          <div className={`alert ${f.severity}`} key={f.id} role={f.severity === "info" ? undefined : "status"}>
            <Icon name={f.severity === "info" ? "info" : "alert"} />
            <div className="alert-text">
              <b>{f.title}</b>
              <span>{f.detail}</span>
            </div>
            {(f.id === "reclaimable" || f.category) && (
              <button className="btn small" onClick={onGoClean}>{t("overview.reviewCleanup")}</button>
            )}
            {f.severity !== "info" || f.id.startsWith("leak:") ? (
              <button className="btn ghost small" onClick={() => onMute(f.id, true)}>{t("overview.mute")}</button>
            ) : null}
          </div>
        ))}
        {active.length === 0 && (
          <div className="alert ok">
            <Icon name="check" />
            <div className="alert-text">
              <b>{t("overview.allGood")}</b>
              <span>{t("overview.allGoodBody")}</span>
            </div>
          </div>
        )}
        {muted.length > 0 && (
          <button className="link quiet" onClick={() => setShowMuted((v) => !v)}>
            {t("overview.mutedCount", { n: muted.length })}
          </button>
        )}
        {showMuted &&
          muted.map((f) => (
            <div className="alert muted" key={f.id}>
              <div className="alert-text">
                <b>{f.title}</b>
                <span>{f.detail}</span>
              </div>
              <button className="btn ghost small" onClick={() => onMute(f.id, false)}>{t("overview.unmute")}</button>
            </div>
          ))}
      </div>

      <div className="two">
        <section className="box">
          <div className="box-head">
            <h3>{t("overview.busiest")}</h3>
            <div className="seg" role="group" aria-label={t("overview.sortBy")}>
              <button aria-pressed={sort === "cpu"} onClick={() => setSort("cpu")}>{t("overview.cpu")}</button>
              <button aria-pressed={sort === "mem"} onClick={() => setSort("mem")}>{t("overview.memory")}</button>
            </div>
          </div>
          {apps.length === 0 ? (
            <div className="empty">{t("overview.noApps")}</div>
          ) : (
            <table className="table">
              <thead>
                <tr>
                  <th>{t("overview.app")}</th>
                  <th className="num">{t("overview.cpu")}</th>
                  <th className="num">{t("overview.memory")}</th>
                  <th className="num">{t("overview.processes")}</th>
                </tr>
              </thead>
              <tbody>
                {apps.map((a) => (
                  <tr key={a.name}>
                    <td className="name">{a.name}</td>
                    <td className={`num${a.cpu_percent >= 20 ? " hot" : ""}`}>{a.cpu_percent.toFixed(1)}%</td>
                    <td className="num">{bytes(a.mem_bytes)}</td>
                    <td className="num dim">{a.processes}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </section>

        <div className="stack">
          {containers && (
            <section className="box">
              <div className="box-head">
                <h3>{containers.engine === "podman" ? "Podman" : "Docker"}</h3>
                <span className="hint">{t("overview.containersCount", { running: containers.running, total: containers.total })}</span>
              </div>
              {containers.containers.length === 0 ? (
                <div className="empty">{t("overview.noContainers")}</div>
              ) : (
                <table className="table">
                  <tbody>
                    {containers.containers.slice(0, 6).map((c) => (
                      <tr key={c.name}>
                        <td className="name">
                          {c.name}
                          <small>{c.image}</small>
                        </td>
                        <td className="num">{c.cpu_percent.toFixed(1)}%</td>
                        <td className="num">{bytes(c.mem_bytes)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </section>
          )}
          {otherDisks.length > 0 && (
            <section className="box">
              <div className="box-head">
                <h3>{t("overview.otherDisks")}</h3>
              </div>
              <table className="table">
                <tbody>
                  {otherDisks.map((d) => {
                    const p = pct(d.total - d.available, d.total);
                    return (
                      <tr key={d.mount}>
                        <td className="name">
                          {d.mount}
                          <div className="bar"><i style={{ width: `${p}%`, background: p >= 85 ? "var(--flag)" : undefined }} /></div>
                        </td>
                        <td className="num">{percent(p)}</td>
                        <td className="num dim">{t("overview.free", { size: bytes(d.available) })}</td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </section>
          )}
        </div>
      </div>

      <section className="box">
        <div className="box-head">
          <h3>{t("overview.advice")}</h3>
          <span className="hint">{t("overview.adviceHint")}</span>
        </div>
        {!findings || findings.advice.length === 0 ? (
          <div className="empty">{t("overview.noAdvice")}</div>
        ) : (
          <div className="rows flat">
            {findings.advice.map((a) => (
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
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
