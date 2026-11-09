import { useEffect, useState } from "react";
import { api, diagnostics, helperStatus, type Applied, type HelperStatus, type Lang, type Settings as S, type Toggle } from "../api";
import type { Theme } from "../theme";
import { HelperNotice } from "../components/HelperNotice";
import { AiSettings } from "../components/AiSettings";
import { Command, Icon } from "../components/Icon";
import { useT, type Key } from "../i18n";

const LANGS: { id: Lang; label: string }[] = [
  { id: "en", label: "English" },
  { id: "tr", label: "Türkçe" },
];

function Switch({ label, hint, checked, disabled, onChange }: { label: string; hint: string; checked: boolean; disabled?: boolean; onChange: (v: boolean) => void }) {
  return (
    <label className={`row setting switch-row${disabled ? " muted" : ""}`}>
      <div className="grow">
        <b>{label}</b>
        <p>{hint}</p>
      </div>
      <input type="checkbox" role="switch" className="switch" checked={checked} disabled={disabled} onChange={(e) => onChange(e.target.checked)} />
    </label>
  );
}

interface Props {
  theme: Theme;
  onTheme: (t: Theme) => void;
  onChatChanged: () => void;
  onLanguage: (l: Lang) => void;
  onAdmin: (action: string) => void;
  refreshKey: number;
}

export function Settings({ onLanguage, onAdmin, refreshKey, onChatChanged, theme, onTheme }: Props) {
  const { t, lang } = useT();
  const [s, setS] = useState<S | null>(null);
  const [saving, setSaving] = useState<Lang | null>(null);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [applied, setApplied] = useState<{ applied: Applied[]; helper_installed: boolean } | null>(null);

  useEffect(() => {
    api.settings().then(setS).catch((e) => setError(String(e)));
  }, []);
  const [helper, setHelper] = useState<HelperStatus>("ok");
  const [diagCopied, setDiagCopied] = useState(false);

  async function toggle(key: Toggle, value: boolean) {
    try {
      setS(await api.setToggle(key, value));
    } catch (e) {
      setError(String(e));
    }
  }
  useEffect(() => {
    api
      .adminApplied()
      .then(async (r) => {
        setApplied(r);
        setHelper(await helperStatus(r.helper_status ?? (r.helper_installed ? "ok" : "missing")));
      })
      .catch(() => {});
  }, [refreshKey, lang]);

  async function choose(l: Lang) {
    if (l === lang || saving) return;
    setSaving(l);
    setError(null);
    try {
      const next = await api.setLanguage(l);
      setS(next);
      onLanguage(next.language);
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(null);
    }
  }

  return (
    <div className="view settings">
      <div className="view-head">
        <h2>{t("settings.title")}</h2>
      </div>

      <div className="rows">
        <div className="row setting">
          <div style={{ flex: 1 }}>
            <b id="lang-label">{t("settings.language")}</b>
            <p>{t("settings.languageHint")}</p>
          </div>
          <div className="seg big-seg" role="radiogroup" aria-labelledby="lang-label">
            {LANGS.map((l) => (
              <button
                key={l.id}
                role="radio"
                aria-checked={lang === l.id}
                aria-pressed={lang === l.id}
                onClick={() => choose(l.id)}
                disabled={!s && !error}
                lang={l.id}
              >
                {saving === l.id && <Icon name="refresh" className="icon spin" />}
                {l.label}
              </button>
            ))}
          </div>
        </div>
        {saved && (
          <div className="row" role="status" style={{ color: "var(--starboard)", fontSize: 14 }}>
            <Icon name="check" />
            {t("settings.saved")}
          </div>
        )}
        {error && <div className="row" style={{ color: "var(--port)", fontSize: 14 }}>{error}</div>}
      </div>

      {applied && (
        <section className="panel">
          <div className="panel-head">
            <h3>{t("settings.applied")}</h3>
            <span className="hint">{t("settings.appliedHint")}</span>
          </div>
          <div className="rows">
            {applied.applied.length === 0 ? (
              <div className="empty">{t("settings.appliedNone")}</div>
            ) : (
              applied.applied.map((a) => (
                <div className="row setting" key={a.id}>
                  <span className="lamp ok" />
                  <div style={{ flex: 1 }}><b>{a.title}</b></div>
                  <button className="btn small" onClick={() => onAdmin(a.undo_action)}>{t("settings.undo")}</button>
                </div>
              ))
            )}
            <div className="row setting">
              <span className={`lamp ${helper === "ok" ? "ok" : "warning"}`} />
              <div style={{ flex: 1, minWidth: 0 }}>
                <b>{t("settings.helper")}</b>
                <p>{helper === "ok" ? t("settings.helperOk") : t("settings.helperMissing")}</p>
                <HelperNotice status={helper} />
              </div>
            </div>
          </div>
        </section>
      )}

      {s && (
        <section className="panel">
          <div className="panel-head">
            <h3>{t("settings.chat")}</h3>
            <span className="hint">{s.chat_enabled ? t("settings.chatOn", { model: s.model }) : t("settings.chatOff")}</span>
          </div>
          <AiSettings settings={s} onSaved={(n) => { setS(n); onChatChanged(); }} />
        </section>
      )}

      {s && (
        <section className="panel">
          <div className="rows">
            <Switch
              label={t("settings.privacy")}
              hint={t("settings.privacyHint")}
              checked={s.share_file_paths}
              onChange={(v) => toggle("share_file_paths", v)}
            />
            <Switch
              label={t("settings.notifications")}
              hint={t("settings.notificationsHint")}
              checked={s.notifications}
              onChange={(v) => toggle("notifications", v)}
            />
            <Switch
              label={t("settings.weeklySummary")}
              hint={t("settings.weeklySummaryHint")}
              checked={(s.weekly_summary ?? true) && s.notifications}
              disabled={!s.notifications}
              onChange={(v) => toggle("weekly_summary", v)}
            />
            <Switch
              label={t("settings.persistChat")}
              hint={t("settings.persistChatHint")}
              checked={s.persist_chat ?? false}
              onChange={(v) => toggle("persist_chat", v)}
            />
            <div className="row setting">
              <div style={{ flex: 1 }}>
                <b id="theme-label">{t("settings.theme")}</b>
              </div>
              <div className="seg big-seg" role="radiogroup" aria-labelledby="theme-label">
                {(["system", "dark", "light"] as Theme[]).map((th) => (
                  <button key={th} role="radio" aria-checked={theme === th} aria-pressed={theme === th} onClick={() => onTheme(th)}>
                    {t(`settings.theme${th[0].toUpperCase()}${th.slice(1)}` as Key)}
                  </button>
                ))}
              </div>
            </div>
          </div>
        </section>
      )}

      <section className="panel">
        <div className="rows">
          {s && (
            <div className="row">
              <div style={{ flex: 1, minWidth: 0 }}>
                <b>{t("settings.configFile")}</b>
                <p>{t("settings.configHint")}</p>
                <Command text={s.config_path} />
                <Command text="systemctl --user restart bosund" />
              </div>
            </div>
          )}
          <div className="row setting">
            <div style={{ flex: 1 }}>
              <b>{t("settings.diagnostics")}</b>
              <p>{t("settings.diagnosticsHint")}</p>
            </div>
            <button
              className="btn small"
              onClick={async () => {
                const text = await diagnostics();
                await navigator.clipboard.writeText(text);
                setDiagCopied(true);
                setTimeout(() => setDiagCopied(false), 2500);
              }}
            >
              <Icon name={diagCopied ? "check" : "copy"} />
              {diagCopied ? t("settings.copiedDiagnostics") : t("settings.copyDiagnostics")}
            </button>
          </div>
        </div>
      </section>
    </div>
  );
}
