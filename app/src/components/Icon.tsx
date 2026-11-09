// Küçük, tek renkli çizgi ikonlar (harici kütüphane yok).
import { useT } from "../i18n";
const paths: Record<string, string> = {
  gauge: "M12 21a9 9 0 1 1 9-9M12 12l4.5-4.5M5.6 16.5h12.8",
  broom: "M14 4l-3.5 8M7 12h8l2 8H5l2-8zM9 16v4M13 16v4",
  chat: "M4 5h16v11H9l-5 4V5z",
  log: "M6 4h12v16H6zM9 8h6M9 12h6M9 16h4",
  refresh: "M20 11a8 8 0 1 0-2.3 5.7M20 5v6h-6",
  copy: "M9 9h11v11H9zM5 15V4h11",
  check: "M5 12.5l4.5 4.5L19 7",
  close: "M6 6l12 12M18 6L6 18",
  send: "M4 12l16-8-6 16-2-7-8-1z",
  trash: "M5 7h14M10 7V4h4v3M7 7l1 13h8l1-13",
  power: "M12 3v9M6.3 6.3a8 8 0 1 0 11.4 0",
  eraser: "M4 20h16M7 16l9-9 3 3-9 9H7l-3-3 3-3",
  shield: "M12 3l7 3v5c0 4.5-3 8.2-7 10-4-1.8-7-5.5-7-10V6l7-3z",
  launch: "M12 15V4M7.5 8.5L12 4l4.5 4.5M5 14v6h14v-6",
  alert: "M12 4l9 16H3L12 4zM12 10v4M12 17.5v.01",
  info: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 11v6M12 7.5v.01",
  gear: "M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6zM12 2v3M12 19v3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M2 12h3M19 12h3M4.9 19.1L7 17M17 7l2.1-2.1",
};

export function Icon({ name, className = "icon", title }: { name: keyof typeof paths | string; className?: string; title?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round" aria-hidden={title ? undefined : true} role={title ? "img" : undefined}>
      {title && <title>{title}</title>}
      <path d={paths[name] ?? ""} />
    </svg>
  );
}

/** Durum noktası: iyi = yeşil, dikkat = sarı, kritik = kırmızı. */
export function Pennant({ level }: { level: "ok" | "warning" | "critical" | "offline" }) {
  return <span className={`status-dot ${level}`} aria-hidden />;
}

export function CopyButton({ text }: { text: string }) {
  const { t } = useT();
  return (
    <button
      className="btn ghost small"
      onClick={(e) => {
        void navigator.clipboard.writeText(text);
        const b = e.currentTarget;
        b.textContent = t("copied");
        setTimeout(() => (b.textContent = t("copy")), 1500);
      }}
    >
      {t("copy")}
    </button>
  );
}

export function Command({ text }: { text: string }) {
  return (
    <div className="cmd">
      <code>{text}</code>
      <CopyButton text={text} />
    </div>
  );
}
