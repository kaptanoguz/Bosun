import { describe, expect, it } from "vitest";
import type { Finding } from "./api";
import { translate, type Key } from "./i18n";
import { starters } from "./suggestions";

const t = (k: Key, v?: Record<string, string | number>) => translate("en", k, v);
const f = (id: string, extra: Partial<Finding> = {}): Finding => ({ id, severity: "warning", title: id, detail: "", category: null, ...extra });

describe("chat starters", () => {
  it("asks about what Bosun actually sees", () => {
    const s = starters([f("disk:/"), f("leak:telegram-desktop", { severity: "info" })], t);
    expect(s[0]).toBe("The system disk is almost full. What can I safely remove?");
    expect(s[1]).toContain("telegram-desktop");
    expect(s).toHaveLength(4);
  });

  it("skips muted findings and falls back to general questions", () => {
    const s = starters([f("cpu", { muted: true })], t);
    expect(s).not.toContain(t("chat.askCpu"));
    expect(s[0]).toBe(t("chat.s1"));
  });
});
