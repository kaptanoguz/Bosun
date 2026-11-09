import { useEffect, useState } from "react";
import { api, type StartupApp } from "../api";
import { useT } from "../i18n";

/** Oturum açılışında başlayan uygulamalar. Kapatmak hiçbir şeyi silmez; geri açılabilir. */
export function Startup({ onToast }: { onToast: (s: string) => void }) {
  const { t } = useT();
  const [apps, setApps] = useState<StartupApp[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [showSystem, setShowSystem] = useState(false);

  useEffect(() => {
    api
      .startup()
      .then((r) => setApps(r.apps))
      .catch((e) => setError(String(e)));
  }, []);

  async function toggle(a: StartupApp) {
    setBusy(a.file);
    setError(null);
    try {
      const updated = await api.setStartup(a.file, !a.enabled);
      setApps((list) => list?.map((x) => (x.file === a.file ? updated : x)) ?? null);
      onToast(t(updated.enabled ? "startup.turnedOn" : "startup.turnedOff", { name: updated.name }));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  }

  const shown = apps?.filter((a) => showSystem || !a.essential) ?? [];
  const hidden = (apps?.length ?? 0) - shown.length;
  const on = apps?.filter((a) => a.enabled).length ?? 0;

  return (
    <div className="view">
      <div className="view-head">
        <h2>{t("startup.title")}</h2>
        {apps && <p>{t("startup.count", { on, total: apps.length })}</p>}
      </div>
      <p className="lede">{t("startup.lede")}</p>
      {error && <p className="error-text">{error}</p>}
      <section className="box">
        {!apps ? (
          <div className="empty">{t("startup.loading")}</div>
        ) : shown.length === 0 ? (
          <div className="empty">{t("startup.none")}</div>
        ) : (
          <table className="table startup">
            <thead>
              <tr>
                <th>{t("startup.app")}</th>
                <th>{t("startup.source")}</th>
                <th className="num">{t("startup.runs")}</th>
              </tr>
            </thead>
            <tbody>
              {shown.map((a) => (
                <tr key={a.file} className={a.enabled ? "" : "off"}>
                  <td className="name">
                    {a.name}
                    {a.missing && <span className="tag caution">{t("startup.missing")}</span>}
                    <small title={a.exec}>{a.comment || a.exec}</small>
                  </td>
                  <td className="dim">{a.essential ? t("startup.desktop") : a.system ? t("startup.system") : t("startup.yours")}</td>
                  <td className="num">
                    <input
                      type="checkbox"
                      className="switch"
                      role="switch"
                      aria-label={t("startup.toggle", { name: a.name })}
                      checked={a.enabled}
                      disabled={busy === a.file || (a.essential && a.enabled)}
                      title={a.essential && a.enabled ? t("startup.essentialHint") : undefined}
                      onChange={() => void toggle(a)}
                    />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>
      {hidden > 0 && (
        <button className="link quiet" onClick={() => setShowSystem(true)}>
          {t("startup.showDesktop", { n: hidden })}
        </button>
      )}
    </div>
  );
}
