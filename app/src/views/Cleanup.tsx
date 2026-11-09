import { useMemo, useState } from "react";
import { api, type CategoryScan, type Plan, type ScanResult } from "../api";
import { Command, Icon } from "../components/Icon";
import { ago, bytes, num, shortPath } from "../format";
import { useT } from "../i18n";

interface Props {
  scan: ScanResult | null;
  scanning: boolean;
  onRescan: () => void;
  onPlan: (p: Plan) => void;
  onAdmin: (action: string, items?: string[]) => void;
}

export function Cleanup({ scan, scanning, onRescan, onPlan, onAdmin }: Props) {
  const { t } = useT();
  const [selected, setSelected] = useState<Set<string> | null>(null);
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [open, setOpen] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const cats = scan?.categories ?? [];
  // Varsayılan seçim: yalnızca "güvenli" ve toplu seçilebilir, içinde bir şey olan kategoriler.
  const sel = useMemo(
    () => selected ?? new Set(cats.filter((c) => c.bulk_selectable && c.safety === "safe" && c.item_count > 0).map((c) => c.id)),
    [selected, cats],
  );
  const max = Math.max(1, ...cats.map((c) => c.bytes));
  const reviewCats = cats.filter((c) => c.safety === "review_only" && c.items.length > 0);
  const userCats = cats.filter((c) => c.bulk_selectable);
  const adminCats = cats.filter((c) => c.method === "privileged" && c.item_count > 0);

  const pickedBytes = reviewCats.flatMap((c) => c.items).filter((i) => picked.has(i.path)).reduce((a, i) => a + i.bytes, 0);
  const total = cats.filter((c) => sel.has(c.id)).reduce((a, c) => a + c.bytes, 0) + pickedBytes;
  const count = sel.size + picked.size;

  function toggle<T>(set: Set<T>, v: T): Set<T> {
    const next = new Set(set);
    if (next.has(v)) next.delete(v);
    else next.add(v);
    return next;
  }

  async function review() {
    setBusy(true);
    setError(null);
    try {
      onPlan(await api.createPlan([...sel], [...picked]));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  function sizeCell(c: CategoryScan) {
    // Kırık bağlantılar gibi yer kaplamayan öğelerde boyut yerine adet göster.
    if (c.bytes === 0 && c.item_count > 0) return <b>{t("clean.count", { n: num(c.item_count) })}</b>;
    return <b>{bytes(c.bytes)}</b>;
  }

  function tag(c: CategoryScan) {
    if (c.method === "permanent") return <span className="tag permanent">{t("clean.permanent")}</span>;
    if (c.method === "unlink") return <span className="tag safe">{t("clean.linkOnly")}</span>;
    return <span className={`tag ${c.safety}`}>{t(`clean.${c.safety}`)}</span>;
  }

  function itemList(c: CategoryScan, limit = 12) {
    return (
      <ul className="files">
        {c.items.slice(0, limit).map((i) => (
          <li key={i.path}>
            <span title={i.path}>{shortPath(i.path)}</span>
            {i.bytes > 0 && <em>{bytes(i.bytes)}</em>}
          </li>
        ))}
        {c.item_count > limit && <li><em>{t("clean.more", { n: num(c.item_count - limit) })}</em></li>}
      </ul>
    );
  }

  function expander(c: CategoryScan) {
    if (c.items.length === 0) return null;
    const isOpen = open === c.id;
    return (
      <>
        <button className="link" onClick={() => setOpen(isOpen ? null : c.id)} aria-expanded={isOpen}>
          {isOpen ? t("clean.hide") : t("clean.showItems", { n: num(c.item_count) })}
        </button>
        {isOpen && itemList(c)}
      </>
    );
  }

  return (
    <div className="view">
      <div className="view-head">
        <h2>{t("clean.title")}</h2>
        <p>{scan ? t("clean.lastScan", { ago: ago(scan.ts) }) : t("clean.scanning")}</p>
        <div className="actions">
          <button className="btn" onClick={onRescan} disabled={scanning}>
            <Icon name="refresh" className={scanning ? "icon spin" : "icon"} />
            {scanning ? t("clean.scanningBtn") : t("clean.rescan")}
          </button>
        </div>
      </div>

      {!scan ? (
        <div className="empty">{t("clean.scanningLong")}</div>
      ) : (
        <>
          <div className="clean">
            <div className="group-title">{t("clean.groupUser")}</div>
            {userCats.map((c) => (
              <div className="cat" key={c.id}>
                <input
                  type="checkbox"
                  aria-label={c.title}
                  checked={sel.has(c.id)}
                  disabled={c.item_count === 0}
                  onChange={() => setSelected(toggle(sel, c.id))}
                />
                <div style={{ minWidth: 0 }}>
                  <div className="title">{c.title}{tag(c)}</div>
                  <div className="desc">{c.description}</div>
                  {expander(c)}
                </div>
                <div className="size">
                  {sizeCell(c)}
                  <div className="bar"><i style={{ width: `${(c.bytes / max) * 100}%` }} /></div>
                </div>
              </div>
            ))}

            {reviewCats.map((c) => (
              <div key={c.id}>
                <div className="group-title">{c.title} · {t("clean.pickHint")}</div>
                <div className="cat">
                  <span />
                  <div style={{ minWidth: 0 }}>
                    <div className="desc">{c.description}</div>
                    <ul className="files">
                      {c.items.map((i) => (
                        <li key={i.path}>
                          <input type="checkbox" checked={picked.has(i.path)} onChange={() => setPicked(toggle(picked, i.path))} aria-label={shortPath(i.path)} style={{ accentColor: "var(--brass)" }} />
                          <span title={i.path}>{shortPath(i.path)}</span>
                          <em>{bytes(i.bytes)}{i.modified ? ` · ${ago(i.modified)}` : ""}</em>
                        </li>
                      ))}
                    </ul>
                  </div>
                  <div className="size"><b>{bytes(c.bytes)}</b></div>
                </div>
              </div>
            ))}

            {adminCats.length > 0 && (
              <>
                <div className="group-title">{t("clean.groupAdmin")}</div>
                {adminCats.map((c) => (
                  <div className="cat" key={c.id}>
                    <span className="admin-mark" aria-hidden><Icon name="shield" /></span>
                    <div style={{ minWidth: 0 }}>
                      <div className="title">{c.title}</div>
                      <div className="desc">{c.description}</div>
                      {expander(c)}
                      <div className="admin-actions">
                        {c.admin_action && (
                          <button className="btn small" onClick={() => onAdmin(c.admin_action!, c.items.map((i) => i.path))}>
                            {t("clean.adminRun")}
                          </button>
                        )}
                        {c.manual_command && (
                          <details>
                            <summary>{t("clean.orTerminal")}</summary>
                            <Command text={c.manual_command} />
                          </details>
                        )}
                      </div>
                    </div>
                    <div className="size">{c.bytes > 0 ? <b>{bytes(c.bytes)}</b> : <b>{t("clean.packages", { n: c.item_count })}</b>}</div>
                  </div>
                ))}
              </>
            )}
          </div>

          <div className="dock">
            <div>
              <div className="total">{bytes(total)}</div>
              <div className="sub">{count === 0 ? t("clean.nothing") : t("clean.selected", { n: count })}</div>
            </div>
            {error && <span style={{ color: "var(--port)", fontSize: 14 }}>{error}</span>}
            <button className="btn primary" disabled={count === 0 || busy} onClick={review}>
              {busy && <Icon name="refresh" className="icon spin" />}
              {t("clean.review")}
            </button>
          </div>
        </>
      )}
    </div>
  );
}
