import { useCallback, useEffect, useMemo, useState } from "react";
import { api, onConnection, onEvent, startDaemon, type FindingsPayload, type Hello, type Lang, type Plan, type Report, type ScanResult, type Settings as SettingsT, type Snapshot } from "./api";
import { Command, Icon, Pennant } from "./components/Icon";
import { AdminSheet } from "./components/AdminSheet";
import { PlanSheet } from "./components/PlanSheet";
import { ago, bytes } from "./format";
import { applyLang, cachedLang, I18n, translate, type Key } from "./i18n";
import { Chat, type ChatMessage } from "./views/Chat";
import { Cleanup } from "./views/Cleanup";
import { History } from "./views/History";
import { Overview } from "./views/Overview";
import { Settings } from "./views/Settings";

type View = "overview" | "cleanup" | "chat" | "history" | "settings";

const initialLang = cachedLang();
applyLang(initialLang);

export default function App() {
  const [lang, setLang] = useState<Lang>(initialLang);
  const i18n = useMemo(() => ({ lang, t: (k: Key, v?: Record<string, string | number>) => translate(lang, k, v) }), [lang]);
  const { t } = i18n;

  const [view, setView] = useState<View>("overview");
  const [connected, setConnected] = useState<boolean | null>(null);
  const [hello, setHello] = useState<Hello | null>(null);
  const [latest, setLatest] = useState<Snapshot | null>(null);
  const [history, setHistory] = useState<Snapshot[]>([]);
  const [scan, setScan] = useState<ScanResult | null>(null);
  const [scanning, setScanning] = useState(false);
  const [findings, setFindings] = useState<FindingsPayload | null>(null);
  const [plan, setPlan] = useState<Plan | null>(null);
  const [admin, setAdmin] = useState<{ action: string; items?: string[] } | null>(null);
  const [adminKey, setAdminKey] = useState(0);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [toast, setToast] = useState<string | null>(null);
  const [historyKey, setHistoryKey] = useState(0);
  const [startError, setStartError] = useState<string | null>(null);

  const changeLang = useCallback((l: Lang) => {
    applyLang(l);
    setLang(l);
  }, []);

  const loadAll = useCallback(async () => {
    try {
      const [h, s, f] = await Promise.all([api.hello(), api.snapshot(), api.findings()]);
      setHello(h);
      if (h.language) changeLang(h.language);
      setLatest(s.latest);
      setHistory(s.history);
      setFindings(f);
      setConnected(true);
      api.scan().then(setScan).catch(() => {});
    } catch {
      setConnected(false);
    }
  }, [changeLang]);

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
        case "settings":
          changeLang((e.data as SettingsT).language);
          break;
      }
    });
    return () => {
      offConn();
      offEv();
    };
  }, [loadAll, changeLang]);

  useEffect(() => {
    if (!toast) return;
    const tm = setTimeout(() => setToast(null), 4000);
    return () => clearTimeout(tm);
  }, [toast]);

  async function rescan() {
    setScanning(true);
    try {
      setScan(await api.scan(true));
    } finally {
      setScanning(false);
    }
  }

  function openAdmin(action: string, items?: string[]) {
    setAdmin({ action, items });
  }

  function closeAdmin(done: boolean) {
    setAdmin(null);
    if (done) {
      setAdminKey((k) => k + 1);
      setHistoryKey((k) => k + 1);
      api.findings().then(setFindings).catch(() => {});
    }
  }

  function closePlan(report?: Report) {
    setPlan(null);
    if (report) {
      setToast(t("toast.freed", { size: bytes(report.freed_bytes) }));
      setHistoryKey((k) => k + 1);
    }
  }

  const level = connected === false ? "offline" : findings?.overall === "critical" ? "critical" : findings?.overall === "warning" ? "warning" : "ok";
  const reclaim = scan?.categories.filter((c) => c.bulk_selectable && c.safety === "safe").reduce((a, c) => a + c.bytes, 0) ?? 0;
  const warnCount = findings?.findings.filter((f) => f.severity !== "info").length ?? 0;

  const nav: { id: View; label: string; icon: string; count?: string }[] = [
    { id: "overview", label: t("nav.overview"), icon: "gauge", count: warnCount ? String(warnCount) : undefined },
    { id: "cleanup", label: t("nav.cleanup"), icon: "broom", count: reclaim > 512 * 1024 ** 2 ? bytes(reclaim) : undefined },
    { id: "chat", label: t("nav.chat"), icon: "chat" },
    { id: "history", label: t("nav.history"), icon: "log" },
    { id: "settings", label: t("nav.settings"), icon: "gear" },
  ];

  return (
    <I18n.Provider value={i18n}>
      <div className="shell">
        <aside className="rail">
          <div className="brand">
            <img src="/bosun.svg" alt="" />
            <b>Bosun</b>
          </div>
          <nav className="nav" aria-label={t("nav.sections")}>
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
          <h1>{connected === false ? t("status.offline") : t(`status.${level === "offline" ? "ok" : level}` as Key)}</h1>
          <div className="meta">
            {latest && t("status.live", { ago: ago(latest.ts) })}
            {scan && (
              <>
                <br />
                {t("status.lastScan", { ago: ago(scan.ts) })}
              </>
            )}
          </div>
        </header>

        <main>
          {connected === false && view !== "settings" ? (
            <div className="offline">
              <img src="/bosun.svg" alt="" />
              <h2>{t("offline.title")}</h2>
              <p>{t("offline.body")}</p>
              <p>
                <button
                  className="btn primary"
                  onClick={() => {
                    setStartError(null);
                    startDaemon().catch((e) => setStartError(String(e)));
                  }}
                >
                  <Icon name="power" />
                  {t("offline.start")}
                </button>
              </p>
              {startError && <p style={{ color: "var(--port)" }}>{startError}</p>}
              <p style={{ fontSize: 14, color: "var(--ink-3)" }}>{t("offline.orTerminal")}</p>
              <Command text="systemctl --user enable --now bosund" />
            </div>
          ) : view === "overview" ? (
            <Overview latest={latest} history={history} findings={findings} onGoClean={() => setView("cleanup")} onAdmin={(a) => openAdmin(a)} />
          ) : view === "cleanup" ? (
            <Cleanup scan={scan} scanning={scanning} onRescan={rescan} onPlan={setPlan} onAdmin={openAdmin} />
          ) : view === "chat" ? (
            <Chat enabled={hello?.chat_enabled ?? false} configPath={hello?.config_path ?? "~/.config/bosun/config.toml"} messages={messages} setMessages={setMessages} onPlan={setPlan} />
          ) : view === "history" ? (
            <History refreshKey={historyKey} />
          ) : (
            <Settings onLanguage={changeLang} onAdmin={(a) => openAdmin(a)} refreshKey={adminKey} />
          )}
        </main>

        {plan && <PlanSheet plan={plan} onClose={closePlan} />}
        {admin && <AdminSheet action={admin.action} items={admin.items} onClose={closeAdmin} />}
        {toast && (
          <div className="toast" role="status">
            <Icon name="check" />
            {toast}
          </div>
        )}
      </div>
    </I18n.Provider>
  );
}
