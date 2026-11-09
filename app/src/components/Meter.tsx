// Kartların altındaki son bir saatin çizgisi (bağımlılıksız SVG).

/** Kartın altındaki çizgi. `max` verilmezse serinin en büyüğüne göre ölçeklenir. */
export function Spark({ values, max, second }: { values: number[]; max?: number; second?: number[] }) {
  if (values.length < 2) return <div className="spark" />;
  const top = max ?? Math.max(1, ...values, ...(second ?? []));
  const path = (vs: number[]) =>
    vs.map((v, i) => `${i ? "L" : "M"}${((i / (vs.length - 1)) * 100).toFixed(2)},${(40 - (Math.min(v, top) / top) * 38).toFixed(2)}`).join(" ");
  const line = path(values);
  return (
    <svg className="spark" viewBox="0 0 100 40" preserveAspectRatio="none" aria-hidden>
      <path className="spark-area" d={`${line} L100,40 L0,40 Z`} />
      <path className="spark-line" d={line} />
      {second && second.length > 1 && <path className="spark-line second" d={path(second)} />}
    </svg>
  );
}
