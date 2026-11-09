import { useEffect, useState } from "react";
import { api, type Applied, type Lang, type Settings as S } from "../api";
import { Command, Icon } from "../components/Icon";
import { useT } from "../i18n";

const LANGS: { id: Lang; label: string }[] = [
  { id: "en", label: "English" },
  { id: "tr", label: "Türkçe" },
];

interface Props {
  onLanguage: (l: Lang) => void;
  onAdmin: (action: string) => void;
  refreshKey: number;
}

export function Settings({ onLanguage, onAdmin, refreshKey }: Props) {
  const { t, lang } = useT();
  const [s, setS] = useState<S | null>(null);
  const [saving, setSaving] = useState<Lang | null>(null);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [applied, setApplied] = useState<{ applied: Applied[]; helper_installed: boolean } | null>(null);

  useEffect(() => {
    api.settings().then(setS).catch((e) => setError(String(e)));
  }, []);
  useEffect(() => {
    api.adminApplied().then(setApplied).catch(() => {});
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
              <span className={`lamp ${applied.helper_installed ? "ok" : "warning"}`} />
              <div style={{ flex: 1 }}>
                <b>{t("settings.helper")}</b>
                <p>{applied.helper_installed ? t("settings.helperOk") : t("settings.helperMissing")}</p>
                {!applied.helper_installed && <Command text="./install.sh --helper" />}
              </div>
            </div>
          </div>
        </section>
      )}

      {s && (
        <div className="rows" style={{ marginTop: 18 }}>
          <div className="row setting">
            <div style={{ flex: 1 }}>
              <b>{t("settings.chat")}</b>
            </div>
            <span className="value">{s.chat_enabled ? t("settings.chatOn", { model: s.model }) : t("settings.chatOff")}</span>
          </div>
          <div className="row setting">
            <div style={{ flex: 1 }}>
              <b>{t("settings.notifications")}</b>
            </div>
            <span className="value">{s.notifications ? t("settings.on") : t("settings.off")}</span>
          </div>
          <div className="row setting">
            <div style={{ flex: 1 }}>
              <b>{t("settings.privacy")}</b>
            </div>
            <span className="value">{s.share_file_paths ? t("settings.privacyOn") : t("settings.privacyOff")}</span>
          </div>
          <div className="row">
            <div style={{ flex: 1, minWidth: 0 }}>
              <b>{t("settings.configFile")}</b>
              <p>{t("settings.configHint")}</p>
              <Command text={s.config_path} />
              <Command text="systemctl --user restart bosund" />
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
