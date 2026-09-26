// The mock's If comparisons match the core's (src-tauri/src/engine/values.rs),
// case for case, so screens behave the same against either.

import { describe, expect, it } from "vitest";
import { fileValues, fill, holds } from "./mockRuns";

describe("holds", () => {
  it("compares numbers as numbers", () => {
    expect(holds("1200", ">", "1000")).toBe(true);
    expect(holds("1000", "=", "1000.00")).toBe(true);
    expect(holds(" 10 ", ">=", "9")).toBe(true);
    expect(holds("9", "<", "10")).toBe(true);
  });

  it("compares dates as dates", () => {
    expect(holds("2026-09-14", ">", "2026-01-31")).toBe(true);
    expect(holds("2025-12-31", "<=", "2026-01-01")).toBe(true);
  });

  it("compares anything else as text, ignoring case and outer spaces", () => {
    expect(holds("  Receipt ", "=", "receipt")).toBe(true);
    expect(holds("Screenshot 2026-09-14", "startsWith", "screenshot")).toBe(true);
    expect(holds("Invoice from ACME", "contains", "acme")).toBe(true);
    expect(holds("12 apples", "!=", "12")).toBe(true);
    expect(holds("1e3", "=", "1000")).toBe(false);
    expect(holds("2026-044", "startsWith", "2026")).toBe(true);
  });
});

describe("values", () => {
  it("fills variables and leaves other braces", () => {
    expect(fill("{a} {b} {not a var}", { a: { kind: "text", value: "{b}" }, b: { kind: "text", value: "x" } }))
      .toBe("{b} x {not a var}");
  });

  it("gives a file's name, extension and folder", () => {
    const v = fileValues("~/Downloads/my.scan.PDF", new Date(2026, 0, 2));
    expect([v.file.value, v.extension.value, v.folder.value, v.dateAdded.value, v.year.value])
      .toEqual(["my.scan", "PDF", "~/Downloads", "2026-01-02", "2026"]);
  });
});
