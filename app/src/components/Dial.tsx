import { percent } from "../format";

// Köprüüstü göstergesi: 270°'lik kadran, ibre ve renkli doluluk yayı.

const START = 135; // derece; 0 = sağ, saat yönünde
const SWEEP = 270;
const C = 100;
const R = 78;

function polar(deg: number, r: number) {
  const a = (deg * Math.PI) / 180;
  return [C + r * Math.cos(a), C + r * Math.sin(a)];
}

function arc(from: number, to: number, r: number) {
  const [x1, y1] = polar(from, r);
  const [x2, y2] = polar(to, r);
  const large = to - from > 180 ? 1 : 0;
  return `M ${x1} ${y1} A ${r} ${r} 0 ${large} 1 ${x2} ${y2}`;
}

interface Props {
  label: string;
  value: number; // 0–100
  display?: string;
  detail: string;
  warn?: number;
  crit?: number;
}

export function Dial({ label, value, display, detail, warn = 80, crit = 92 }: Props) {
  const v = Math.max(0, Math.min(100, Number.isFinite(value) ? value : 0));
  const color = v >= crit ? "var(--port)" : v >= warn ? "var(--flag)" : "var(--brass)";
  const angle = START + (SWEEP * v) / 100;
  const ticks = Array.from({ length: 21 }, (_, i) => i);
  return (
    <div className="dial">
      <svg viewBox="0 0 200 196" role="img" aria-label={`${label}: ${percent(v)}`}>
        <circle className="face" cx={C} cy={C} r={94} strokeWidth={1} />
        {ticks.map((i) => {
          const d = START + (SWEEP * i) / 20;
          const major = i % 5 === 0;
          const [x1, y1] = polar(d, major ? 62 : 66);
          const [x2, y2] = polar(d, 70);
          return <line key={i} className={major ? "tick major" : "tick"} x1={x1} y1={y1} x2={x2} y2={y2} strokeWidth={major ? 2 : 1} />;
        })}
        {/* uyarı bölgeleri: kadranın dış kenarında ince bantlar */}
        <path d={arc(START + (SWEEP * warn) / 100, START + (SWEEP * crit) / 100, 86)} stroke="var(--flag)" strokeWidth={3} fill="none" opacity={0.55} />
        <path d={arc(START + (SWEEP * crit) / 100, START + SWEEP, 86)} stroke="var(--port)" strokeWidth={3} fill="none" opacity={0.6} />
        <path className="track" d={arc(START, START + SWEEP, R)} strokeWidth={6} fill="none" />
        {v > 0.5 && <path d={arc(START, angle, R)} stroke={color} strokeWidth={6} fill="none" strokeLinecap="round" />}
        <g style={{ transform: `rotate(${angle}deg)`, transformOrigin: `${C}px ${C}px` }} className="needle">
          <line x1={C - 10} y1={C} x2={C + 56} y2={C} strokeWidth={2.5} strokeLinecap="round" />
        </g>
        <circle className="hub" cx={C} cy={C} r={5} />
        <text className="value" x={C} y={C + 44} textAnchor="middle">
          {display ?? percent(v)}
        </text>
      </svg>
      <h3>{label}</h3>
      <small>{detail}</small>
    </div>
  );
}
