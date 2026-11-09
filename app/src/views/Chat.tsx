import { useEffect, useRef, useState } from "react";
import { api, type Plan } from "../api";
import { Command, Icon } from "../components/Icon";
import { bytes } from "../format";

export interface ChatMessage {
  role: "user" | "assistant" | "error";
  text: string;
  plans?: Plan[];
  tools?: string[];
}

const TOOL_LABEL: Record<string, string> = {
  get_system_snapshot: "ölçümler",
  get_cleanup_candidates: "temizlik taraması",
  get_findings: "bulgular",
  get_large_files: "büyük dosyalar",
  propose_cleanup: "plan",
};

const STARTERS = [
  "Sistem nasıl görünüyor?",
  "Diskte yer açmama yardım et",
  "Bilgisayar neden yavaş olabilir?",
  "Hangi ayarları değiştirmemi önerirsin?",
];

interface Props {
  enabled: boolean;
  configPath: string;
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

export function Chat({ enabled, configPath, messages, setMessages, onPlan }: Props) {
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const log = useRef<HTMLDivElement>(null);

  useEffect(() => {
    log.current?.scrollTo({ top: log.current.scrollHeight, behavior: "smooth" });
  }, [messages, busy]);

  async function send(text: string) {
    const msg = text.trim();
    if (!msg || busy) return;
    setInput("");
    setMessages((m) => [...m, { role: "user", text: msg }]);
    setBusy(true);
    try {
      const r = await api.chat(msg);
      setMessages((m) => [...m, { role: "assistant", text: r.reply, plans: r.plans, tools: r.tools_used }]);
    } catch (e) {
      setMessages((m) => [...m, { role: "error", text: String(e) }]);
    } finally {
      setBusy(false);
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
          <h2>Sohbet için bir API anahtarı gerekiyor</h2>
          <p>
            İzleme, bulgular ve temizlik anahtar olmadan da çalışır. Bosun ile konuşmak için Anthropic API anahtarınızı
            yapılandırma dosyasına ekleyip servisi yeniden başlatın.
          </p>
          <Command text={`mkdir -p ~/.config/bosun && echo 'api_key = "sk-ant-..."' >> ${configPath} && chmod 600 ${configPath}`} />
          <Command text="systemctl --user restart bosund" />
        </div>
      </div>
    );
  }

  return (
    <div className="view chat">
      <div className="log" ref={log} aria-live="polite">
        {messages.length === 0 && (
          <div className="welcome">
            <h3>Ne öğrenmek istersiniz?</h3>
            <p>Bosun sisteminizin güncel ölçümlerine bakarak yanıt verir. Bir şey silmeyi önerirse önce size onay kartı gösterir.</p>
            <div className="chips">
              {STARTERS.map((s) => (
                <button key={s} onClick={() => send(s)}>{s}</button>
              ))}
            </div>
          </div>
        )}
        {messages.map((m, i) =>
          m.role === "user" ? (
            <div key={i} className="msg user">{m.text}</div>
          ) : (
            <div key={i} className={`msg assistant ${m.role === "error" ? "error" : ""}`}>
              <img src="/bosun.svg" alt="" />
              <div style={{ minWidth: 0 }}>
                <RichText text={m.text} />
                {m.plans?.map((p) => (
                  <div className="plan-card" key={p.id}>
                    <h4>Temizlik önerisi · {bytes(p.total_bytes)}</h4>
                    {p.reason && <p>{p.reason}</p>}
                    <div style={{ fontSize: 14, color: "var(--ink-2)" }}>
                      {p.summary.map((s) => `${s.title} (${bytes(s.bytes)})`).join(", ")}
                    </div>
                    <div className="actions">
                      <button className="btn primary small" onClick={() => onPlan(p)} disabled={p.items.length === 0}>Gözden geçir ve onayla</button>
                    </div>
                  </div>
                ))}
                {m.tools && m.tools.length > 0 && (
                  <div className="tools">Baktığı yerler: {[...new Set(m.tools.map((t) => TOOL_LABEL[t] ?? t))].join(", ")}</div>
                )}
              </div>
            </div>
          ),
        )}
        {busy && (
          <div className="msg assistant">
            <img src="/bosun.svg" alt="" />
            <span className="typing" aria-label="Yanıt hazırlanıyor"><i /><i /><i /></span>
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
          placeholder="Bosun'a sorun…"
          aria-label="Mesaj"
          rows={1}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              void send(input);
            }
          }}
        />
        <button className="btn primary" disabled={busy || !input.trim()} aria-label="Gönder">
          <Icon name="send" />
        </button>
        {messages.length > 0 && (
          <button type="button" className="btn ghost" onClick={reset} title="Yeni sohbet" aria-label="Yeni sohbet">
            <Icon name="eraser" />
          </button>
        )}
      </form>
    </div>
  );
}
