import { useCallback, useEffect, useState } from "react";
import { api, inTauri, onConnection, onEvent, startDaemon, type FindingsPayload, type Hello, type Plan, type Report, type ScanResult, type Snapshot } from "./api";
import { Command, Icon, Pennant } from "./components/Icon";
import { PlanSheet } from "./components/PlanSheet";
import { ago, bytes } from "./format";
import { Chat, type ChatMessage } from "./views/Chat";
import { Cleanup } from "./views/Cleanup";
import { History } from "./views/History";
import { Overview } from "./views/Overview";

type View = "overview" | "cleanup" | "chat" | "history";

const HEADLINE = {
  ok: "Sistem iyi durumda",
  info: "Sistem iyi durumda",
  warning: "Dikkat gerektiren bir şey var",
  critical: "Hemen bakılması gereken bir sorun var",
};

export default function App() {
  const [view, setView] = useState<View>("overview");
  const [connected, setConnected] = useState<boolean | null>(null);
  const [hello, setHello] = useState<Hello | null>(null);
  const [latest, setLatest] = useState<Snapshot | null>(null);
  const [history, setHistory] = useState<Snapshot[]>([]);
  const [scan, setScan] = useState<ScanResult | null>(null);
  const [scanning, setScanning] = useState(false);
  const [findings, setFindings] = useState<FindingsPayload | null>(null);
  const [plan, setPlan] = useState<Plan | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [toast, setToast] = useState<string | null>(null);
  const [historyKey, setHistoryKey] = useState(0);
  const [startError, setStartError] = useState<string | null>(null);

  const loadAll = useCallback(async () => {
    try {
      const [h, s, f] = await Promise.all([api.hello(), api.snapshot(), api.findings()]);
      setHello(h);
      setLatest(s.latest);
      setHistory(s.history);
      setFindings(f);
      setConnected(true);
      api.scan().then(setScan).catch(() => {});
    } catch {
      setConnected(false);
    }
  }, []);

  useEffect(() => {
    void loadAll();
    const offConn = onConnection((up) => {
      setConnected(up);
      if (up) void loadAll();
    });
    const offEv = onEvent((e) => {
      switch (e.event) {
        case "metrics": {
          const s = e.data as Snapshot;
          setLatest(s);
          setHistory((h) => [...h.slice(-239), s]);
          break;
        }
        case "findings":
          setFindings(e.data as FindingsPayload);
          break;
        case "scan_started":
          setScanning(true);
          break;
        case "scan":
          setScan(e.data as ScanResult);
          setScanning(false);
          break;
        case "plan_executed":
          setHistoryKey((k) => k + 1);
          break;
      }
    });
    return () => {
      offConn();
      offEv();
    };
  }, [loadAll]);

  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => setToast(null), 4000);
    return () => clearTimeout(t);
  }, [toast]);

  async function rescan() {
    setScanning(true);
    try {
      setScan(await api.scan(true));
    } finally {
      setScanning(false);
    }
  }

  function closePlan(report?: Report) {
    setPlan(null);
    if (report) {
      setToast(`${bytes(report.freed_bytes)} boşaltıldı`);
      setHistoryKey((k) => k + 1);
      if (!inTauri) return;
    }
  }

  const level = connected === false ? "offline" : findings?.overall === "critical" ? "critical" : findings?.overall === "warning" ? "warning" : "ok";
  const reclaim = scan?.categories.filter((c) => c.bulk_selectable && c.safety === "safe").reduce((a, c) => a + c.bytes, 0) ?? 0;
  const warnCount = findings?.findings.filter((f) => f.severity !== "info").length ?? 0;

  const nav: { id: View; label: string; icon: string; count?: string }[] = [
    { id: "overview", label: "Genel bakış", icon: "gauge", count: warnCount ? String(warnCount) : undefined },
    { id: "cleanup", label: "Temizlik", icon: "broom", count: reclaim > 512 * 1024 ** 2 ? bytes(reclaim) : undefined },
    { id: "chat", label: "Sohbet", icon: "chat" },
    { id: "history", label: "Geçmiş", icon: "log" },
  ];

  return (
    <div className="shell">
      <aside className="rail">
        <div className="brand">
          <img src="/bosun.svg" alt="" />
          <b>Bosun</b>
        </div>
        <nav className="nav" aria-label="Bölümler">
          {nav.map((n) => (
            <button key={n.id} aria-current={view === n.id ? "page" : undefined} onClick={() => setView(n.id)}>
              <Icon name={n.icon} />
              {n.label}
              {n.count && <span className="count">{n.count}</span>}
            </button>
          ))}
        </nav>
        <div className="rail-foot">
          {hello && (
            <>
              {hello.host.hostname}
              <br />
              {hello.host.os}
              <br />
              Bosun {hello.version}
            </>
          )}
        </div>
      </aside>

      <header className="status">
        <Pennant level={level} />
        <h1>{connected === false ? "Bosun servisine ulaşılamıyor" : HEADLINE[level === "offline" ? "ok" : level]}</h1>
        <div className="meta">
          {latest && <>Canlı · son ölçüm {ago(latest.ts)}</>}
          {scan && (
            <>
              <br />
              Son tarama {ago(scan.ts)}
            </>
          )}
        </div>
      </header>

      <main>
        {connected === false ? (
          <div className="offline">
            <img src="/bosun.svg" alt="" />
            <h2>Bosun servisi çalışmıyor</h2>
            <p>Arayüz, sistemi izleyen arka plan servisine bağlanamadı. Servisi başlatın; bağlantı kendiliğinden kurulacak.</p>
            <p>
              <button
                className="btn primary"
                onClick={() => {
                  setStartError(null);
                  startDaemon().catch((e) => setStartError(String(e)));
                }}
              >
                <Icon name="power" />
                Servisi başlat
              </button>
            </p>
            {startError && <p style={{ color: "var(--port)" }}>{startError}</p>}
            <p style={{ fontSize: 14, color: "var(--ink-3)" }}>Ya da terminalde:</p>
            <Command text="systemctl --user enable --now bosund" />
          </div>
        ) : view === "overview" ? (
          <Overview latest={latest} history={history} findings={findings} onGoClean={() => setView("cleanup")} />
        ) : view === "cleanup" ? (
          <Cleanup scan={scan} scanning={scanning} onRescan={rescan} onPlan={setPlan} />
        ) : view === "chat" ? (
          <Chat enabled={hello?.chat_enabled ?? false} configPath={hello?.config_path ?? "~/.config/bosun/config.toml"} messages={messages} setMessages={setMessages} onPlan={setPlan} />
        ) : (
          <History refreshKey={historyKey} />
        )}
      </main>

      {plan && <PlanSheet plan={plan} onClose={closePlan} />}
      {toast && (
        <div className="toast" role="status">
          <Icon name="check" />
          {toast}
        </div>
      )}
    </div>
  );
}
