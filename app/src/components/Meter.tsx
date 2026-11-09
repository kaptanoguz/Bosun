// Kartların altındaki son bir saatin çizgisi (bağımlılıksız SVG). Üzerine gelince o anki
// değer ve saat görünür; en yüksek değer ekran okuyucuya da söylenir.
import { useState, type MouseEvent } from "react";
import { timeOfDay } from "../format";

interface SparkProps {
  values: number[];
  /** İkinci seri (ör. yazma, gönderme); ince mavi çizgi. */
  second?: number[];
  /** Verilmezse serinin en büyüğüne göre ölçeklenir. */
  max?: number;
  /** Her noktanın zamanı (ISO). */
  times?: string[];
  /** Değeri ekranda yazmak için. */
  format: (v: number) => string;
  /** Ekran okuyucu için: "CPU, son bir saat". */
  label: string;
}

export function Spark({ values, second, max, times, format, label }: SparkProps) {
  const [hover, setHover] = useState<number | null>(null);
  if (values.length < 2) return <div className="spark" />;
  const top = max ?? Math.max(1, ...values, ...(second ?? []));
  const x = (i: number, n: number) => (i / (n - 1)) * 100;
  const y = (v: number) => 40 - (Math.min(v, top) / top) * 38;
  const path = (vs: number[]) => vs.map((v, i) => `${i ? "L" : "M"}${x(i, vs.length).toFixed(2)},${y(v).toFixed(2)}`).join(" ");
  const line = path(values);
  const peak = Math.max(...values);

  function move(e: MouseEvent<HTMLDivElement>) {
    const r = e.currentTarget.getBoundingClientRect();
    const i = Math.round(((e.clientX - r.left) / r.width) * (values.length - 1));
    setHover(Math.max(0, Math.min(values.length - 1, i)));
  }

  const at = hover ?? 0;
  const text = second ? `${format(values[at])} · ${format(second[at] ?? 0)}` : format(values[at]);

  return (
    <div className="spark-wrap" onMouseMove={move} onMouseLeave={() => setHover(null)} role="img" aria-label={`${label}: ${format(peak)} max`}>
      <svg className="spark" viewBox="0 0 100 40" preserveAspectRatio="none" aria-hidden>
        <path className="spark-area" d={`${line} L100,40 L0,40 Z`} />
        <path className="spark-line" d={line} />
        {second && second.length > 1 && <path className="spark-line second" d={path(second)} />}
        {hover !== null && <line className="spark-cursor" x1={x(at, values.length)} x2={x(at, values.length)} y1="0" y2="40" />}
      </svg>
      {hover !== null && (
        <div className="spark-tip" style={{ left: `${Math.min(80, Math.max(20, x(at, values.length)))}%` }}>
          {text}
          {times?.[at] && <small>{timeOfDay(times[at])}</small>}
        </div>
      )}
    </div>
  );
}
