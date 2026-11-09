import { useEffect, useRef, useState } from "react";
import { api, type Finding, type Plan } from "../api";
import { Command, Icon } from "../components/Icon";
import { bytes } from "../format";
import { starters } from "../suggestions";
import { useT, type Key } from "../i18n";

export interface ChatMessage {
  role: "user" | "assistant" | "error";
  text: string;
  plans?: Plan[];
  tools?: string[];
}

const KNOWN_TOOLS = ["get_system_snapshot", "get_cleanup_candidates", "get_findings", "get_large_files", "propose_cleanup"];

interface Props {
  enabled: boolean;
  /** Açılış önerileri bunlardan üretilir. */
  findings: Finding[];
  live: { turn: number; text: string; tool: string | null } | null;
  onSend: () => void;
  onDone: () => void;
  onOpenSettings: () => void;
  messages: ChatMessage[];
  setMessages: (f: (m: ChatMessage[]) => ChatMessage[]) => void;
  onPlan: (p: Plan) => void;
}

/** `kod` parçalarını ve tek başına duran komut satırlarını biçimlendirir. */
function RichText({ text }: { text: string }) {
  return (
    <>
      {text.split("\n").map((line, i) => {
        const solo = line.trim().match(/^`([^`]+)`$/);
        if (solo) return <Command key={i} text={solo[1]} />;
        const parts = line.split(/(`[^`]+`)/g);
        return (
          <div key={i} style={{ minHeight: "1em" }}>
            {parts.map((p, j) => (p.startsWith("`") && p.endsWith("`") ? <code key={j}>{p.slice(1, -1)}</code> : p))}
          </div>
        );
      })}
    </>
  );
}

export function Chat({ enabled, findings, live, onSend, onDone, messages, setMessages, onPlan, onOpenSettings }: Props) {
  const { t } = useT();
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const log = useRef<HTMLDivElement>(null);

  useEffect(() => {
    log.current?.scrollTo({ top: log.current.scrollHeight, behavior: "smooth" });
  }, [messages, busy, live?.text]);

  async function send(text: string) {
    const msg = text.trim();
    if (!msg || busy) return;
    setInput("");
    setMessages((m) => [...m, { role: "user", text: msg }]);
    onSend();
    setBusy(true);
    try {
      const r = await api.chat(msg);
      setMessages((m) => [...m, { role: "assistant", text: r.reply, plans: r.plans, tools: r.tools_used }]);
    } catch (e) {
      setMessages((m) => [...m, { role: "error", text: String(e) }]);
    } finally {
      setBusy(false);
      onDone();
    }
  }

  async function reset() {
    await api.resetChat().catch(() => {});
    setMessages(() => []);
  }

  if (!enabled) {
    return (
      <div className="view">
        <div className="offline" style={{ margin: "6vh 0", textAlign: "left" }}>
          <h2>{t("chat.needKey")}</h2>
          <p>{t("chat.needKeyBody")}</p>
          <p>
            <button className="btn primary" onClick={onOpenSettings}>
              <Icon name="gear" />
              {t("chat.openSettings")}
            </button>
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="view chat">
      <div className="log" ref={log} aria-live="polite">
        {messages.length === 0 && (
          <div className="welcome">
            <h3>{t("chat.welcome")}</h3>
            <p>{t("chat.welcomeBody")}</p>
            <div className="chips">
              {starters(findings, t).map((q) => (
                <button key={q} onClick={() => send(q)}>{q}</button>
              ))}
            </div>
          </div>
        )}
        {messages.map((m, i) =>
          m.role === "user" ? (
            <div key={i} className="msg user">{m.text}</div>
          ) : (
            <div key={i} className={`msg assistant ${m.role === "error" ? "error" : ""}`}>
              <img src="/bosun-small.svg" alt="" />
              <div style={{ minWidth: 0 }}>
                <RichText text={m.text} />
                {m.plans?.map((p) => (
                  <div className="plan-card" key={p.id}>
                    <h4>{t("chat.proposal", { size: bytes(p.total_bytes) })}</h4>
                    {p.reason && <p>{p.reason}</p>}
                    <div style={{ fontSize: 14, color: "var(--ink-2)" }}>
                      {p.summary.map((s) => `${s.title} (${bytes(s.bytes)})`).join(", ")}
                    </div>
                    <div className="actions">
                      <button className="btn primary small" onClick={() => onPlan(p)} disabled={p.items.length === 0}>{t("chat.reviewApprove")}</button>
                    </div>
                  </div>
                ))}
                {m.tools && m.tools.length > 0 && (
                  <div className="tools">
                    {t("chat.looked", {
                      list: [...new Set(m.tools.map((x) => (KNOWN_TOOLS.includes(x) ? t(`chat.tool.${x}` as Key) : x)))].join(", "),
                    })}
                  </div>
                )}
              </div>
            </div>
          ),
        )}
        {busy && (
          <div className="msg assistant">
            <img src="/bosun-small.svg" alt="" />
            <div style={{ minWidth: 0 }}>
              {live?.text ? <RichText text={live.text} /> : null}
              {live?.tool ? (
                <div className="tools live-tool">
                  {t("chat.looking", { what: KNOWN_TOOLS.includes(live.tool) ? t(`chat.tool.${live.tool}` as Key) : live.tool })}
                </div>
              ) : !live?.text ? (
                <span className="typing" aria-label={t("chat.typing")}><i /><i /><i /></span>
              ) : null}
            </div>
          </div>
        )}
      </div>
      <form
        className="composer"
        onSubmit={(e) => {
          e.preventDefault();
          void send(input);
        }}
      >
        <textarea
          value={input}
          placeholder={t("chat.placeholder")}
          aria-label={t("chat.message")}
          rows={1}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              void send(input);
            }
          }}
        />
        <button className="btn primary" disabled={busy || !input.trim()} aria-label={t("chat.send")}>
          <Icon name="send" />
        </button>
        {messages.length > 0 && (
          <button type="button" className="btn ghost" onClick={reset} title={t("chat.new")} aria-label={t("chat.new")}>
            <Icon name="eraser" />
          </button>
        )}
      </form>
    </div>
  );
}
