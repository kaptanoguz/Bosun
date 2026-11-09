import { describe, expect, it } from "vitest";
import { translate } from "./i18n";
import { en } from "./locales/en";
import { tr } from "./locales/tr";

describe("translate", () => {
  it("picks singular or plural", () => {
    expect(translate("en", "plan.items", { n: 1 })).toBe("1 item");
    expect(translate("en", "plan.items", { n: 3 })).toBe("3 items");
    expect(translate("en", "plan.skipped", { n: 1 })).toMatch(/^1 item was skipped/);
  });

  it("has every key in Turkish, with the same placeholders", () => {
    const vars = (s: string) => [...s.matchAll(/\{(\w+)[}|]/g)].map((m) => m[1]).sort();
    for (const k of Object.keys(en) as (keyof typeof en)[]) {
      expect(tr[k], k).toBeTruthy();
      expect(new Set(vars(tr[k])), k).toEqual(new Set(vars(en[k])));
    }
  });
});
