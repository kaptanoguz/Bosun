// Son bir saatin çizgi grafiği (bağımlılıksız SVG). Genişlik kapsayıcıdan ölçülür ki yazılar bozulmasın.
import { useEffect, useRef, useState } from "react";
import { percent, timeOfDay } from "../format";
import { useT } from "../i18n";

interface Props {
  values: number[];
  times: string[];
  max?: number;
  unit?: string;
}

const H = 150;
const PAD_L = 34;
const PAD_B = 18;

export function Trend({ values, times, max = 100, unit = "%" }: Props) {
  const { t } = useT();
  const ref = useRef<HTMLDivElement>(null);
  const [W, setW] = useState(900);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const ro = new ResizeObserver(([e]) => setW(Math.max(200, e.contentRect.width)));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  if (values.length < 2) return <div className="trend empty" ref={ref}>{t("trend.collecting")}</div>;
  const innerW = W - PAD_L;
  const innerH = H - PAD_B - 6;
  const x = (i: number) => PAD_L + (innerW * i) / (values.length - 1);
  const y = (v: number) => 6 + innerH - (innerH * Math.min(v, max)) / max;
  const line = values.map((v, i) => `${i ? "L" : "M"}${x(i).toFixed(1)},${y(v).toFixed(1)}`).join(" ");
  const area = `${line} L${x(values.length - 1)},${y(0)} L${x(0)},${y(0)} Z`;
  const fmt = (v: number) => (unit === "%" ? percent(v) : `${Math.round(v)}${unit}`);
  const peak = Math.max(...values);
  const avg = values.reduce((a, b) => a + b, 0) / values.length;
  return (
    <div className="trend" ref={ref}>
      <svg viewBox={`0 0 ${W} ${H}`} width={W} height={H} role="img" aria-label={`${t("trend.avg", { v: fmt(avg) })}, ${t("trend.peak", { v: fmt(peak) })}`}>
        {[0, 50, 100].map((g) => (
          <g key={g}>
            <line className="grid" x1={PAD_L} x2={W} y1={y((g / 100) * max)} y2={y((g / 100) * max)} />
            <text className="axis" x={0} y={y((g / 100) * max) + 4}>
              {fmt((g / 100) * max)}
            </text>
          </g>
        ))}
        <path className="area" d={area} />
        <path className="line" d={line} />
        <text className="axis" x={PAD_L} y={H - 2}>{timeOfDay(times[0])}</text>
        <text className="axis" x={W} y={H - 2} textAnchor="end">{timeOfDay(times[times.length - 1])}</text>
      </svg>
      <div style={{ display: "flex", gap: 18, fontSize: 13, color: "var(--ink-3)", padding: "4px 0 4px " + PAD_L / 2 + "px" }}>
        <span>{t("trend.avg", { v: fmt(avg) })}</span>
        <span>{t("trend.peak", { v: fmt(peak) })}</span>
      </div>
    </div>
  );
}
