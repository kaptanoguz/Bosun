import { useEffect, useState } from "react";
import { api, type ScanResult, type UsageListing } from "../api";
import { Icon } from "../components/Icon";
import { bytes, crumbs, percent } from "../format";
import { useT } from "../i18n";

/** Bu kadar satırdan sonrası tek satırda özetlenir. */
const MAX_ROWS = 40;

interface Props {
  scan: ScanResult | null;
  onGoClean: () => void;
}

/** Tarama bu klasörün altında temizlenebilir bir şey bulduysa true. */
function hasCleanable(path: string, scan: ScanResult | null): boolean {
  if (!scan) return false;
  const prefix = path.endsWith("/") ? path : `${path}/`;
  return scan.categories.some((c) => c.bulk_selectable && c.bytes > 0 && c.items.some((i) => i.path === path || i.path.startsWith(prefix)));
}

export function Disk({ scan, onGoClean }: Props) {
  const { t } = useT();
  const [listing, setListing] = useState<UsageListing | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  function open(path?: string, refresh = false) {
    setLoading(true);
    setError(null);
    api
      .usage(path, refresh)
      .then(setListing)
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }

  useEffect(() => {
    let live = true;
    api
      .usage()
      .then((l) => live && setListing(l))
      .catch((e) => live && setError(String(e)))
      .finally(() => live && setLoading(false));
    return () => {
      live = false;
    };
  }, []);

  const shown = listing?.entries.slice(0, MAX_ROWS) ?? [];
  const rest = listing?.entries.slice(MAX_ROWS) ?? [];
  const largest = listing?.entries[0]?.bytes ?? 0;

  return (
    <div className="view narrow">
      <div className="view-head">
        <h2>{t("disk.title")}</h2>
        <div className="actions">
          <button className="btn" disabled={loading} onClick={() => open(listing?.path, true)}>
            <Icon name="refresh" className={loading ? "icon spin" : "icon"} />
            {t("disk.refresh")}
          </button>
        </div>
      </div>
      <p className="lede">{t("disk.intro")}</p>

      {listing && (
        <nav className="crumbs" aria-label={t("disk.title")}>
          {crumbs(listing.path, listing.home).map((c, i, all) => (
            <span key={c.path}>
              {i > 0 && <span className="sep">/</span>}
              {i === all.length - 1 ? (
                <b aria-current="page">{c.name || t("disk.home")}</b>
              ) : (
                <button className="link" onClick={() => open(c.path)}>
                  {c.name || t("disk.home")}
                </button>
              )}
            </span>
          ))}
          <span className="hint">{t("disk.total", { size: bytes(listing.bytes), n: listing.entries.length })}</span>
        </nav>
      )}

      {error && <p className="error-text">{error}</p>}

      <section className={`box${loading ? " busy" : ""}`} aria-busy={loading}>
        {!listing && loading ? (
          <div className="empty">{t("disk.measuring")}</div>
        ) : !listing ? null : listing.entries.length === 0 ? (
          <div className="empty">{t("disk.empty")}</div>
        ) : (
          <table className="table usage">
            <thead>
              <tr>
                <th>{t("disk.name")}</th>
                <th className="num">{t("disk.share")}</th>
                <th className="num">{t("disk.size")}</th>
              </tr>
            </thead>
            <tbody>
              {shown.map((e) => {
                const clean = e.is_dir && hasCleanable(e.path, scan);
                return (
                  <tr key={e.path}>
                    <td className="name">
                      <div className="usage-name">
                        <Icon name={e.is_dir ? "folder" : "file"} className="icon small" />
                        {e.is_dir && !e.other_fs ? (
                          <button className="link plain" onClick={() => open(e.path)}>
                            {e.name}
                          </button>
                        ) : (
                          <span>{e.name}</span>
                        )}
                        {e.other_fs && <span className="tag review_only">{t("disk.otherFs")}</span>}
                        {clean && (
                          <button className="tag safe as-button" title={t("nav.cleanup")} onClick={onGoClean}>
                            {t("disk.cleanable")}
                          </button>
                        )}
                      </div>
                      {e.bytes > 0 && (
                        <div className="bar">
                          <i style={{ width: `${largest ? (e.bytes / largest) * 100 : 0}%` }} />
                        </div>
                      )}
                    </td>
                    <td className="num dim">{listing.bytes ? percent((e.bytes / listing.bytes) * 100) : "—"}</td>
                    <td className="num">{e.other_fs ? "—" : bytes(e.bytes)}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
        {rest.length > 0 && (
          <div className="empty">{t("disk.smaller", { n: rest.length, size: bytes(rest.reduce((a, e) => a + e.bytes, 0)) })}</div>
        )}
      </section>
      {listing && listing.unreadable > 0 && <p className="hint-note">{t("disk.unreadable", { n: listing.unreadable })}</p>}
    </div>
  );
}
