import { describe, expect, it } from "vitest";
import type { AuditEntry } from "../api";
import { translate, type Key } from "../i18n";
import { toRows } from "./historyRows";

const t = (k: Key, v?: Record<string, string | number>) => translate("en", k, v);
const GB = 1024 ** 3;
const e = (kind: string, data: Record<string, unknown>, ts = "2026-11-09T10:00:00Z"): AuditEntry => ({ ts, kind, data });

describe("history rows", () => {
  it("collapses one plan into one row", () => {
    const rows = toRows(
      [
        e("plan_executed", { plan_id: "a", freed_bytes: 2 * GB, trashed: 1, summary: [{ title: "npm cache" }] }),
        e("plan_approved", { plan_id: "a", items: 1, total_bytes: 2 * GB }),
        e("plan_proposed", { plan_id: "a", total_bytes: 2 * GB }),
      ],
      t,
    );
    expect(rows).toHaveLength(1);
    expect(rows[0].title).toBe("2.0 GB freed");
    expect(rows[0].detail).toBe("npm cache · 1 item in the trash · suggested in chat");
    expect(rows[0].lamp).toBe("ok");
  });

  it("dims plans that were never applied and names admin actions", () => {
    const rows = toRows(
      [
        e("admin_action", { action: "journal-vacuum", title: "Shrink system logs", ok: true, freed_bytes: 0 }),
        e("plan_cancelled", { plan_id: "b" }),
        e("plan_created", { plan_id: "b", total_bytes: GB }),
      ],
      t,
    );
    expect(rows.map((r) => r.title)).toEqual(["Shrink system logs", "Cleanup plan not applied"]);
    expect(rows[1].dim).toBe(true);
  });
});
