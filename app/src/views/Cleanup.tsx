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
}

export function Cleanup({ scan, scanning, onRescan, onPlan }: Props) {
  const { t } = useT();
  // Varsayılan seçim: yalnızca "güvenli" ve toplu seçilebilir kategoriler.
  const [selected, setSelected] = useState<Set<string> | null>(null);
  const [files, setFiles] = useState<Set<string>>(new Set());
  const [open, setOpen] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const cats = scan?.categories ?? [];
  const sel = useMemo(
    () => selected ?? new Set(cats.filter((c) => c.bulk_selectable && c.safety === "safe" && c.bytes > 0).map((c) => c.id)),
    [selected, cats],
  );
  const max = Math.max(1, ...cats.map((c) => c.bytes));
  const large = cats.find((c) => c.id === "large_files");

  const total =
    cats.filter((c) => sel.has(c.id)).reduce((a, c) => a + c.bytes, 0) +
    (large?.items.filter((i) => files.has(i.path)).reduce((a, i) => a + i.bytes, 0) ?? 0);
  const count = sel.size + files.size;

  function toggle(id: string) {
    const next = new Set(sel);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    setSelected(next);
  }
  function toggleFile(p: string) {
    const next = new Set(files);
    if (next.has(p)) next.delete(p);
    else next.add(p);
    setFiles(next);
  }

  async function review() {
    setBusy(true);
    setError(null);
    try {
      onPlan(await api.createPlan([...sel], [...files]));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  const userCats = cats.filter((c) => c.bulk_selectable);
  const adminCats = cats.filter((c) => c.method === "privileged");

  function row(c: CategoryScan) {
    const disabled = !c.bulk_selectable || c.bytes === 0;
    const isOpen = open === c.id;
    return (
      <div className="cat" key={c.id}>
        <input
          type="checkbox"
          aria-label={c.title}
          checked={sel.has(c.id)}
          disabled={disabled}
          onChange={() => toggle(c.id)}
        />
        <div style={{ minWidth: 0 }}>
          <div className="title">
            {c.title}
            <span className={`tag ${c.method === "permanent" ? "permanent" : c.safety}`}>
              {c.method === "permanent" ? t("clean.permanent") : t(`clean.${c.safety}`)}
            </span>
          </div>
          <div className="desc">{c.description}</div>
          {c.items.length > 0 && (
            <button className="link" onClick={() => setOpen(isOpen ? null : c.id)} aria-expanded={isOpen}>
              {isOpen ? t("clean.hide") : t("clean.showItems", { n: num(c.item_count) })}
            </button>
          )}
          {isOpen && (
            <ul className="files">
              {c.items.slice(0, 12).map((i) => (
                <li key={i.path}>
                  <span title={i.path}>{shortPath(i.path)}</span>
                  <em>{bytes(i.bytes)}</em>
                </li>
              ))}
              {c.item_count > 12 && <li><em>{t("clean.more", { n: num(c.item_count - 12) })}</em></li>}
            </ul>
          )}
        </div>
        <div className="size">
          <b>{bytes(c.bytes)}</b>
          <div className="bar"><i style={{ width: `${(c.bytes / max) * 100}%` }} /></div>
        </div>
      </div>
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
            {userCats.map(row)}

            {large && large.items.length > 0 && (
              <>
                <div className="group-title">{t("clean.groupLarge")}</div>
                <div className="cat">
                  <span />
                  <ul className="files" style={{ margin: 0 }}>
                    {large.items.map((i) => (
                      <li key={i.path}>
                        <input type="checkbox" checked={files.has(i.path)} onChange={() => toggleFile(i.path)} aria-label={shortPath(i.path)} style={{ accentColor: "var(--brass)" }} />
                        <span title={i.path}>{shortPath(i.path)}</span>
                        <em>{i.modified ? ago(i.modified) : ""}</em>
                      </li>
                    ))}
                  </ul>
                  <div className="size"><b>{bytes(large.bytes)}</b></div>
                </div>
              </>
            )}

            {adminCats.some((c) => c.bytes > 0) && (
              <>
                <div className="group-title">{t("clean.groupAdmin")}</div>
                {adminCats
                  .filter((c) => c.bytes > 0)
                  .map((c) => (
                    <div className="cat" key={c.id}>
                      <span />
                      <div style={{ minWidth: 0 }}>
                        <div className="title">{c.title}</div>
                        <div className="desc">{c.description}</div>
                        {c.manual_command && <Command text={c.manual_command} />}
                      </div>
                      <div className="size"><b>{bytes(c.bytes)}</b></div>
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
