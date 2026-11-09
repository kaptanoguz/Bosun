// Sohbet sağlayıcısı: Anthropic, xAI Grok, Google Gemini, OpenAI ya da OpenAI uyumlu özel sunucu.
// API anahtarı yalnızca buradan servise gönderilir; servis onu asla geri göndermez.
import { useEffect, useState } from "react";
import { api, type ProviderId, type Settings } from "../api";
import { useT } from "../i18n";
import { Icon } from "./Icon";

interface Props {
  settings: Settings;
  onSaved: (s: Settings) => void;
}

export function AiSettings({ settings, onSaved }: Props) {
  const { t } = useT();
  const [provider, setProvider] = useState<ProviderId>(settings.provider);
  const [model, setModel] = useState(settings.model_override);
  const [baseUrl, setBaseUrl] = useState(settings.base_url ?? "");
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);

  // Sağlayıcı değişince model alanı o sağlayıcının varsayılanına döner.
  useEffect(() => {
    if (provider !== settings.provider) {
      setModel("");
      setBaseUrl("");
    } else {
      setModel(settings.model_override);
      setBaseUrl(settings.base_url ?? "");
    }
    setKey("");
  }, [provider, settings]);

  const providers = settings.providers ?? [];
  const info = providers.find((p) => p.id === provider) ?? providers[0];
  if (!info) return <div className="empty">{t("version.oldService")}</div>;
  const custom = provider === "custom";
  const dirty =
    provider !== settings.provider || model !== settings.model_override || baseUrl !== (settings.base_url ?? "") || key.trim() !== "";

  async function save(removeKey = false) {
    setBusy(true);
    setMsg(null);
    try {
      const s = await api.setAi({
        provider,
        model,
        base_url: baseUrl,
        ...(removeKey ? { api_key: "" } : key.trim() ? { api_key: key.trim() } : {}),
      });
      setKey("");
      onSaved(s);
      setMsg({ ok: true, text: t("settings.saved") });
    } catch (e) {
      setMsg({ ok: false, text: String(e) });
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="rows ai">
      <div className="row setting">
        <div style={{ flex: 1 }}>
          <b>{t("ai.provider")}</b>
          <p>{t("ai.providerHint")}</p>
        </div>
        <select
          value={provider}
          onChange={(e) => {
            setProvider(e.target.value as ProviderId);
            setMsg(null);
          }}
          aria-label={t("ai.provider")}
        >
          {providers.map((p) => (
            <option key={p.id} value={p.id}>
              {p.id === "custom" ? t("ai.custom") : p.label}
              {p.has_key ? " ✓" : ""}
            </option>
          ))}
        </select>
      </div>

      {custom && (
        <label className="row field">
          <b>{t("ai.baseUrl")}</b>
          <p>{t("ai.baseUrlHint")}</p>
          <input value={baseUrl} onChange={(e) => setBaseUrl(e.target.value)} placeholder="http://localhost:11434/v1" spellCheck={false} />
        </label>
      )}

      <label className="row field">
        <b>{t("ai.model")}</b>
        <p>{t("ai.modelHint", { model: info.default_model })}</p>
        <input value={model} onChange={(e) => setModel(e.target.value)} placeholder={info.default_model} spellCheck={false} />
      </label>

      <label className="row field">
        <b>{t("ai.key")}</b>
        <p>
          {info.has_key ? t("ai.keySaved") : custom ? t("ai.keyOptional") : t("ai.keyMissing")}
          {" "}
          {t("ai.keyPrivacy")}
        </p>
        <div className="key-row">
          <input
            type="password"
            value={key}
            onChange={(e) => setKey(e.target.value)}
            placeholder={info.has_key ? "••••••••••••" : t("ai.keyPlaceholder")}
            autoComplete="off"
            spellCheck={false}
          />
          {info.has_key && (
            <button type="button" className="btn ghost small" onClick={() => save(true)} disabled={busy}>
              {t("ai.removeKey")}
            </button>
          )}
        </div>
      </label>

      <div className="row setting">
        <div style={{ flex: 1, fontSize: 14, color: msg ? (msg.ok ? "var(--starboard)" : "var(--port)") : "var(--ink-3)" }} role="status">
          {msg ? msg.text : provider === settings.provider ? t("ai.active", { label: info.id === "custom" ? t("ai.custom") : info.label, model: settings.model }) : ""}
        </div>
        <button className="btn primary" onClick={() => save()} disabled={busy || !dirty}>
          {busy && <Icon name="refresh" className="icon spin" />}
          {t("ai.save")}
        </button>
      </div>
    </div>
  );
}
