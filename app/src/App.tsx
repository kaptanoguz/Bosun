import { useCallback, useEffect, useMemo, useState } from "react";
import { APP_VERSION, restartDaemon, startDaemon, type Lang, type Plan, type Report } from "./api";
import { applyTheme, cachedTheme, type Theme } from "./theme";
import { Command, Icon, StatusDot } from "./components/Icon";
import { AdminSheet } from "./components/AdminSheet";
import { PlanSheet } from "./components/PlanSheet";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { ago, bytes } from "./format";
import { applyLang, cachedLang, I18n, translate, type Key } from "./i18n";
import { useDaemon } from "./useDaemon";
import { Chat } from "./views/Chat";
import { Cleanup } from "./views/Cleanup";
import { History } from "./views/History";
import { Overview } from "./views/Overview";
import { Settings } from "./views/Settings";
import { Startup } from "./views/Startup";

type View = "overview" | "cleanup" | "startup" | "chat" | "history" | "settings";

const initialLang = cachedLang();
applyLang(initialLang);
const initialTheme = cachedTheme();
applyTheme(initialTheme);

/** Önerilen temizlik bu boyutu geçince kenar çubuğunda gösterilir. */
const NAV_RECLAIM_MIN = 512 * 1024 ** 2;

export default function App() {
  const [lang, setLang] = useState<Lang>(initialLang);
  const i18n = useMemo(() => ({ lang, t: (k: Key, v?: Record<string, string | number>) => translate(lang, k, v) }), [lang]);
  const { t } = i18n;

  const changeLang = useCallback((l: Lang) => {
    applyLang(l);
    setLang(l);
  }, []);

  const d = useDaemon(changeLang);
  const [view, setView] = useState<View>("overview");
  const [theme, setTheme] = useState<Theme>(initialTheme);
  const [plan, setPlan] = useState<Plan | null>(null);
  const [admin, setAdmin] = useState<{ action: string; items?: string[] } | null>(null);
  const [adminKey, setAdminKey] = useState(0);
  const [toast, setToast] = useState<string | null>(null);
  const [startError, setStartError] = useState<string | null>(null);

  useEffect(() => {
    if (!toast) return;
    const tm = setTimeout(() => setToast(null), 4000);
    return () => clearTimeout(tm);
  }, [toast]);

  function start() {
    setStartError(null);
    startDaemon().catch((e) => setStartError(String(e)));
  }

  function openAdmin(action: string, items?: string[]) {
    setAdmin({ action, items });
  }

  function closeAdmin(done: boolean) {
    setAdmin(null);
    if (done) {
      setAdminKey((k) => k + 1);
      d.bumpHistory();
      d.refreshFindings();
    }
  }

  function closePlan(report?: Report) {
    setPlan(null);
    if (report) {
      setToast(t("toast.freed", { size: bytes(report.freed_bytes) }));
      d.bumpHistory();
    }
  }

  const offline = d.connected === false;
  const level = offline ? "offline" : d.findings?.overall === "critical" ? "critical" : d.findings?.overall === "warning" ? "warning" : "ok";
  const reclaim = d.scan?.categories.filter((c) => c.bulk_selectable && c.safety === "safe").reduce((a, c) => a + c.bytes, 0) ?? 0;
  const warnCount = d.findings?.findings.filter((f) => f.severity !== "info" && !f.muted).length ?? 0;

  const nav: { id: View; label: string; icon: string; count?: string }[] = [
    { id: "overview", label: t("nav.overview"), icon: "gauge", count: warnCount ? String(warnCount) : undefined },
    { id: "cleanup", label: t("nav.cleanup"), icon: "broom", count: reclaim > NAV_RECLAIM_MIN ? bytes(reclaim) : undefined },
    { id: "startup", label: t("nav.startup"), icon: "launch" },
    { id: "chat", label: t("nav.chat"), icon: "chat" },
    { id: "history", label: t("nav.history"), icon: "log" },
    { id: "settings", label: t("nav.settings"), icon: "gear" },
  ];

  return (
    <I18n.Provider value={i18n}>
      <div className="shell">
        <aside className="rail">
          <div className="brand">
            <img src="/bosun-small.svg" alt="" />
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
            <div className="rail-status" role="status">
              <StatusDot level={level} />
              <span>{t(`status.${level}` as Key)}</span>
            </div>
            {d.latest && !offline && <div>{t("status.live", { ago: ago(d.latest.ts) })}</div>}
            {d.hello && (
              <div className="rail-host">
                {d.hello.host.hostname}
                <br />
                {d.hello.host.os}
                <br />
                Bosun {d.hello.version}
              </div>
            )}
          </div>
        </aside>

        <main>
          {d.hello && !offline && d.hello.version !== APP_VERSION && (
            <div className="banner" role="status">
              {t("version.mismatch", { service: d.hello.version, app: APP_VERSION })}
              <button
                className="btn small"
                onClick={() =>
                  void restartDaemon()
                    .then(() => setTimeout(() => void d.reload(), 1500))
                    .catch((e) => setStartError(String(e)))
                }
              >
                <Icon name="refresh" />
                {t("version.restart")}
              </button>
            </div>
          )}
          <ErrorBoundary resetKey={view} title={t("crash.title")} hint={t("crash.hint")} copyLabel={t("crash.copy")}>
            {offline && d.latest && (
              <div className="banner" role="status">
                <StatusDot level="offline" />
                {t("offline.banner")}
                <button className="btn small" onClick={start}>
                  <Icon name="power" />
                  {t("offline.start")}
                </button>
              </div>
            )}
            {startError && offline && d.latest && <p className="error-text">{startError}</p>}
            {offline && !d.latest && view !== "settings" ? (
              <div className="offline">
                <img src="/bosun-small.svg" alt="" />
                <h2>{t("offline.title")}</h2>
                <p>{t("offline.body")}</p>
                <p>
                  <button className="btn primary" onClick={start}>
                    <Icon name="power" />
                    {t("offline.start")}
                  </button>
                </p>
                {startError && <p className="error-text">{startError}</p>}
                <p className="hint-note">{t("offline.orTerminal")}</p>
                <Command text="systemctl --user enable --now bosund" />
              </div>
            ) : view === "overview" ? (
              <Overview
                latest={d.latest}
                history={d.history}
                findings={d.findings}
                containers={d.containers}
                scanTs={d.scan?.ts ?? null}
                onGoClean={() => setView("cleanup")}
                onAdmin={(a) => openAdmin(a)}
                onMute={d.muteFinding}
              />
            ) : view === "cleanup" ? (
              <Cleanup scan={d.scan} scanning={d.scanning} progress={d.progress} onRescan={d.rescan} onPlan={setPlan} onAdmin={openAdmin} />
            ) : view === "startup" ? (
              <Startup onToast={setToast} />
            ) : view === "chat" ? (
              <Chat
                enabled={d.hello?.chat_enabled ?? false}
                live={d.live}
                onSend={() => d.setLive(null)}
                onDone={() => d.setLive(null)}
                messages={d.messages}
                setMessages={d.setMessages}
                onPlan={setPlan}
                onOpenSettings={() => setView("settings")}
              />
            ) : view === "history" ? (
              <History refreshKey={d.historyKey} />
            ) : (
              <Settings
                theme={theme}
                onTheme={(th) => {
                  applyTheme(th);
                  setTheme(th);
                }}
                onLanguage={changeLang}
                onAdmin={(a) => openAdmin(a)}
                refreshKey={adminKey}
                onChatChanged={d.refreshHello}
              />
            )}
          </ErrorBoundary>
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
