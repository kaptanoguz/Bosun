import { useState } from "react";
import type { FindingsPayload, Snapshot } from "../api";
import { Dial } from "../components/Dial";
import { Command } from "../components/Icon";
import { Trend } from "../components/Trend";
import { bytes, pct, uptime } from "../format";

interface Props {
  latest: Snapshot | null;
  history: Snapshot[];
  findings: FindingsPayload | null;
  onGoClean: () => void;
}

export function Overview({ latest, history, findings, onGoClean }: Props) {
  const [series, setSeries] = useState<"cpu" | "mem">("cpu");
  if (!latest) return <div className="empty">İlk ölçüm bekleniyor…</div>;

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
        <Dial label="İşlemci" value={latest.cpu_percent} detail={`${latest.cpu_count} çekirdek · yük ${latest.load_avg[0].toFixed(2)}`} warn={85} crit={95} />
        <Dial label="Bellek" value={memPct} detail={`${bytes(latest.mem_used)} / ${bytes(latest.mem_total)}`} warn={85} crit={92} />
        <Dial label="Swap" value={swapPct} detail={latest.swap_total ? `${bytes(latest.swap_used)} / ${bytes(latest.swap_total)}` : "Swap yok"} warn={50} crit={80} />
        <Dial label={root ? `Disk ${root.mount}` : "Disk"} value={diskPct} detail={root ? `${bytes(root.available)} boş` : "—"} warn={85} crit={95} />
      </div>

      <section className="panel">
        <div className="panel-head">
          <h3>Son bir saat</h3>
          <span className="hint">
            Açık kalma süresi {uptime(latest.uptime_secs)}
            {temp !== null && ` · en sıcak bileşen ${Math.round(temp)} °C`}
          </span>
          <div className="seg" role="group" aria-label="Grafik">
            <button aria-pressed={series === "cpu"} onClick={() => setSeries("cpu")}>İşlemci</button>
            <button aria-pressed={series === "mem"} onClick={() => setSeries("mem")}>Bellek</button>
          </div>
        </div>
        <Trend values={values} times={times} />
      </section>

      <div className="two">
        <section className="panel">
          <div className="panel-head">
            <h3>Bulgular</h3>
          </div>
          <div className="rows">
            {!findings || findings.findings.length === 0 ? (
              <div className="row">
                <span className="lamp ok" />
                <div>
                  <b>Her şey yolunda</b>
                  <p>Disk, bellek ve servislerde dikkat gerektiren bir şey yok.</p>
                </div>
              </div>
            ) : (
              findings.findings.map((f) => (
                <div className="row" key={f.id}>
                  <span className={`lamp ${f.severity}`} />
                  <div style={{ flex: 1 }}>
                    <b>{f.title}</b>
                    <p>{f.detail}</p>
                    {f.id === "reclaimable" && (
                      <button className="link" onClick={onGoClean} style={{ marginTop: 4 }}>Temizliği gözden geçir</button>
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
                    <span style={{ color: "var(--ink-2)", fontSize: 14, whiteSpace: "nowrap" }}>{bytes(d.available)} boş</span>
                  </div>
                );
              })}
            </div>
          )}
        </section>

        <section className="panel">
          <div className="panel-head">
            <h3>Ayar önerileri</h3>
            <span className="hint">Komutları kendi terminalinizde çalıştırırsınız</span>
          </div>
          <div className="rows">
            {!findings || findings.advice.length === 0 ? (
              <div className="empty">Şu an önerilecek bir ayar yok.</div>
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
                    {a.command && <Command text={a.command} />}
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
