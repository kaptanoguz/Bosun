import { afterEach, describe, expect, it } from "vitest";
import { bytes, crumbs, level, percent, setLocale, shortPath } from "./format";

afterEach(() => setLocale("en"));

describe("format", () => {
  it("shows sizes with one decimal under 100", () => {
    expect(bytes(0)).toBe("0 B");
    expect(bytes(1536)).toBe("1.5 KB");
    expect(bytes(5.3 * 1024 ** 3)).toBe("5.3 GB");
    expect(bytes(476 * 1024 ** 3)).toBe("476 GB");
  });

  it("puts the percent sign where the language expects it", () => {
    expect(percent(42.4)).toBe("42%");
    setLocale("tr");
    expect(percent(42.4)).toBe("%42");
    expect(bytes(1536)).toBe("1,5 KB");
  });

  it("shortens the home folder", () => {
    expect(shortPath("/home/oguz/.cache/mozilla")).toBe("~/.cache/mozilla");
    expect(shortPath("/var/log")).toBe("/var/log");
  });

  it("maps values to levels", () => {
    expect(level(80, 85, 95)).toBe("ok");
    expect(level(85, 85, 95)).toBe("warning");
    expect(level(99, 85, 95)).toBe("critical");
  });
});

describe("crumbs", () => {
  it("walks from home to the folder", () => {
    expect(crumbs("/home/u/Projects/shop", "/home/u").map((c) => c.path)).toEqual(["/home/u", "/home/u/Projects", "/home/u/Projects/shop"]);
    expect(crumbs("/home/u", "/home/u")).toHaveLength(1);
    expect(crumbs("/home/uber", "/home/u")).toHaveLength(1);
  });
});
