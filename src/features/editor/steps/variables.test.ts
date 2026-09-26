// Which details a step can use must agree with Rust's check_variables
// (src-tauri/src/workflow/validate.rs): produced by the trigger or on every
// path from the trigger to the step.

import { describe, expect, it } from "vitest";
import type { Step, Workflow } from "../../../api/types";
import { availableAt, fillSample, unknownIn, variablesIn } from "./variables";

const pos = { x: 0, y: 0 };
const wf = (steps: Step[]): Workflow => ({ version: 1, id: "w", name: "W", revision: 1, enabled: false, steps });
const trigger = (next: string | null): Step => ({ id: "t", type: "fileAdded", title: "T", position: pos, folder: "~/Downloads", fileTypes: [], subfolders: false, next });
const extract = (id: string, names: string[], next: string | null): Step => ({
  id, type: "extract", title: `Extract ${id}`, position: pos, ifMissing: "review", next,
  fields: names.map((name) => ({ name, type: name === "amount" ? "number" : "text" })),
});
const notify = (id: string, next: string | null = null): Step => ({ id, type: "notify", title: `Notify ${id}`, position: pos, message: "", next });
const names = (w: Workflow, id: string) => availableAt(w, id).map((v) => v.name).sort();

describe("availableAt", () => {
  it("gives every step after a file trigger the file's details", () => {
    const w = wf([trigger("n"), notify("n")]);
    expect(names(w, "n")).toEqual(["dateAdded", "extension", "file", "folder", "year"]);
  });

  it("gives the trigger itself nothing", () => {
    expect(names(wf([trigger("n"), notify("n")]), "t")).toEqual([]);
  });

  it("gives a scheduled run today's date and the year, but no file", () => {
    const w = wf([{ id: "t", type: "schedule", title: "S", position: pos, schedule: { every: "day", time: "09:00" }, next: "n" }, notify("n")]);
    expect(names(w, "n")).toEqual(["date", "year"]);
  });

  it("adds what earlier steps produce, not what the step itself or later ones do", () => {
    const w = wf([trigger("e"), extract("e", ["vendor", "amount"], "w"),
      { id: "w", type: "write", title: "W", position: pos, instruction: "x", saveAs: "summary", next: "n" }, notify("n")]);
    expect(names(w, "e")).not.toContain("vendor");
    expect(names(w, "w")).toContain("vendor");
    expect(names(w, "w")).not.toContain("summary");
    expect(names(w, "n")).toEqual(expect.arrayContaining(["amount", "summary", "vendor"]));
  });

  it("drops a detail made on only one of the paths that meet at a step", () => {
    const w = wf([
      trigger("i"),
      { id: "i", type: "if", title: "I", position: pos, condition: { left: "{file}", op: "=", right: "x" }, branches: { yes: "a", no: "b" } },
      extract("a", ["vendor", "amount"], "m"),
      extract("b", ["amount"], "m"),
      notify("m"),
    ]);
    expect(names(w, "m")).toContain("amount");
    expect(names(w, "m")).not.toContain("vendor");
  });

  it("keeps the answer of an Ask me only where every path went through it", () => {
    const w = wf([
      trigger("q"),
      { id: "q", type: "askMe", title: "Q", position: pos, question: "?", answers: [{ id: "a1", label: "A" }, { id: "a2", label: "B" }], branches: { a1: "n", a2: "m" } },
      notify("n", "m"),
      notify("m"),
    ]);
    expect(names(w, "n")).toContain("answer");
    expect(names(w, "m")).toContain("answer");
  });

  it("gives a step no path reaches nothing, and survives loops", () => {
    expect(names(wf([trigger(null), notify("lost")]), "lost")).toEqual([]);
    const loop = wf([trigger("a"), extract("a", ["x"], "b"), notify("b", "a")]);
    expect(names(loop, "a")).not.toContain("x");
    expect(names(loop, "b")).toContain("x");
  });

  it("says where each detail comes from and what kind it is", () => {
    const w = wf([trigger("e"), extract("e", ["amount"], "n"), notify("n")]);
    const at = availableAt(w, "n");
    expect(at.find((v) => v.name === "file")).toMatchObject({ label: "File name", source: "This file", kind: "text" });
    expect(at.find((v) => v.name === "dateAdded")).toMatchObject({ kind: "date" });
    expect(at.find((v) => v.name === "amount")).toMatchObject({ label: "amount", source: "Extract e", kind: "number" });
  });

  it("names no single source when different steps make a detail on different paths", () => {
    const w = wf([
      trigger("i"),
      { id: "i", type: "if", title: "I", position: pos, condition: { left: "{file}", op: "=", right: "x" }, branches: { yes: "a", no: "b" } },
      extract("a", ["amount"], "m"), extract("b", ["amount"], "m"), notify("m"),
    ]);
    expect(availableAt(w, "m").find((v) => v.name === "amount")).toMatchObject({ source: "Earlier steps" });
  });

  it("lists the nearest step's details first and the file's last", () => {
    const w = wf([trigger("a"), extract("a", ["x"], "b"), extract("b", ["y"], "n"), notify("n")]);
    expect(availableAt(w, "n").map((v) => v.source)).toEqual(["Extract b", "Extract a", "This file", "This file", "This file", "This file", "This file"]);
  });
});

describe("variablesIn and unknownIn", () => {
  it("reads {names} like Rust does, and leaves other braces alone", () => {
    expect(variablesIn("{date} - {vendor}")).toEqual(["date", "vendor"]);
    expect(variablesIn("{ \"a\": 1 } {} {not a name} {{x}")).toEqual(["x"]);
  });

  it("lists each unknown name once", () => {
    const w = wf([trigger("n"), notify("n")]);
    expect(unknownIn("{file} {nope} {nope} {also}", availableAt(w, "n"))).toEqual(["nope", "also"]);
  });
});

describe("fillSample", () => {
  it("fills details with sample values for a preview", () => {
    const w = wf([trigger("e"), extract("e", ["vendor", "amount"], "n"), notify("n")]);
    const out = fillSample("{file} {vendor} {amount} {year} {nope}", availableAt(w, "n"), new Date(2026, 8, 14));
    expect(out).toBe("Scan_0042 vendor 42.50 2026 {nope}");
  });
});
