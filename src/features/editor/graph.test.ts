// The pure layer between a workflow file and the canvas. A bug here silently
// loses part of someone's workflow, so every edit is pinned down.

import { describe, expect, it } from "vitest";
import type { Step, Workflow } from "../../api/types";
import {
  addStep, connect, defaultStep, disconnect, exitsOf, moveStep, removeSteps, toFlow, updateStep,
} from "./graph";

const at = { x: 0, y: 0 };

function wf(steps: Step[]): Workflow {
  return { version: 1, id: "0f8c2b1e-6b0a-4c1e-9d6a-2f1f6c0f7a11", name: "Test", revision: 1, enabled: false, steps };
}

const trigger: Step = { id: "t", type: "fileAdded", title: "When a file lands", position: at, folder: "~/Downloads", fileTypes: [], subfolders: false, next: "c" };
const classify: Step = {
  id: "c", type: "classify", title: "What is it?", position: { x: 0, y: 160 }, instructions: "",
  categories: [{ id: "c1", label: "Receipt" }, { id: "c2", label: "Other" }], branches: { c1: "r" },
};
const rename: Step = { id: "r", type: "rename", title: "Rename", position: { x: 0, y: 320 }, template: "{file}", next: null };
const stop: Step = { id: "x", type: "stop", title: "Stop", position: { x: 300, y: 320 } };
const sample = () => wf([trigger, classify, rename, stop]);

const step = (w: Workflow, id: string) => w.steps.find((s) => s.id === id)!;

describe("exitsOf", () => {
  it("gives a plain step one exit and a branching step one per branch, labelled", () => {
    expect(exitsOf(trigger)).toEqual([{ handle: null, label: null, target: "c" }]);
    expect(exitsOf(classify)).toEqual([
      { handle: "c1", label: "Receipt", target: "r" },
      { handle: "c2", label: "Other", target: null },
    ]);
    expect(exitsOf(stop)).toEqual([]);
  });

  it("gives an if step its two fixed branches", () => {
    const iff: Step = { id: "i", type: "if", title: "Big?", position: at, condition: { left: "{amount}", op: ">", right: "5" }, branches: { no: "r" } };
    expect(exitsOf(iff)).toEqual([
      { handle: "yes", label: "Yes", target: null },
      { handle: "no", label: "No", target: "r" },
    ]);
  });
});

describe("toFlow", () => {
  it("makes a node per step and an edge per connected exit", () => {
    const { nodes, edges } = toFlow(sample(), []);
    expect(nodes.map((n) => [n.id, n.position])).toEqual([["t", at], ["c", { x: 0, y: 160 }], ["r", { x: 0, y: 320 }], ["x", { x: 300, y: 320 }]]);
    expect(edges).toEqual([
      expect.objectContaining({ id: "t:next", source: "t", sourceHandle: null, target: "c" }),
      expect.objectContaining({ id: "c:c1", source: "c", sourceHandle: "c1", target: "r", label: "Receipt" }),
    ]);
  });

  it("attaches each step's problems to its node", () => {
    const problem = { stepId: "r", code: "unknown_variable" as const, message: "{x} isn't produced by any step before this one." };
    const { nodes } = toFlow(sample(), [problem]);
    expect(nodes.find((n) => n.id === "r")!.data.problems).toEqual([problem]);
    expect(nodes.find((n) => n.id === "t")!.data.problems).toEqual([]);
  });

  it("draws no edge to a step that doesn't exist", () => {
    const broken = wf([{ ...trigger, next: "gone" }]);
    expect(toFlow(broken, []).edges).toEqual([]);
  });
});

describe("connect", () => {
  it("points a plain step's exit at the target, replacing what was there", () => {
    const w = connect(sample(), "t", null, "r");
    expect(step(w, "t")).toMatchObject({ next: "r" });
  });

  it("points one branch at the target and leaves the others", () => {
    const w = connect(sample(), "c", "c2", "x");
    expect(step(w, "c")).toMatchObject({ branches: { c1: "r", c2: "x" } });
  });

  it("refuses links into a trigger, to unknown steps and from unknown branches", () => {
    const w = sample();
    expect(connect(w, "r", null, "t")).toBe(w);
    expect(connect(w, "r", null, "nope")).toBe(w);
    expect(connect(w, "c", "c9", "x")).toBe(w);
    expect(connect(w, "x", null, "r")).toBe(w);
  });

  it("changes nothing else", () => {
    const before = sample();
    const after = connect(before, "c", "c2", "x");
    expect(after.steps.filter((s) => s.id !== "c")).toEqual(before.steps.filter((s) => s.id !== "c"));
    expect(before.steps[1]).toEqual(classify);
  });
});

describe("disconnect", () => {
  it("clears a plain exit and a branch", () => {
    expect(step(disconnect(sample(), "t", null), "t")).toMatchObject({ next: null });
    expect(step(disconnect(sample(), "c", "c1"), "c")).toMatchObject({ branches: {} });
  });
});

describe("removeSteps", () => {
  it("removes the steps and every exit that pointed at them", () => {
    const w = removeSteps(sample(), ["r"]);
    expect(w.steps.map((s) => s.id)).toEqual(["t", "c", "x"]);
    expect(step(w, "c")).toMatchObject({ branches: {} });
  });

  it("leaves exits to other steps alone", () => {
    const w = removeSteps(sample(), ["x"]);
    expect(step(w, "t")).toMatchObject({ next: "c" });
    expect(step(w, "c")).toMatchObject({ branches: { c1: "r" } });
  });
});

describe("addStep", () => {
  it("adds a step of the chosen type with a new unique id at the position", () => {
    const { workflow, id } = addStep(sample(), "notify", { x: 40, y: 500 });
    expect(id).toMatch(/^s[0-9a-f]{8}$/);
    expect(workflow.steps.filter((s) => s.id === id)).toEqual([
      expect.objectContaining({ type: "notify", position: { x: 40, y: 500 }, next: null }),
    ]);
    expect(workflow.steps).toHaveLength(5);
  });
});

describe("defaultStep", () => {
  it("gives branching steps two branches with different ids", () => {
    const c = defaultStep("classify", "s1", at);
    const a = defaultStep("askMe", "s2", at);
    if (c.type !== "classify" || a.type !== "askMe") throw new Error("wrong type");
    expect(c.categories).toHaveLength(2);
    expect(new Set(c.categories.map((b) => b.id)).size).toBe(2);
    expect(a.answers).toHaveLength(2);
    expect(c.branches).toEqual({});
  });

  it("gives plain steps an open exit and stop none", () => {
    expect(defaultStep("move", "s1", at)).toMatchObject({ next: null, mode: "move" });
    expect(defaultStep("stop", "s1", at)).not.toHaveProperty("next");
  });
});

describe("updateStep", () => {
  it("keeps a branch connected when its label is renamed", () => {
    const renamed = { ...classify, categories: [{ id: "c1", label: "Receipts" }, classify.categories[1]] };
    const w = updateStep(sample(), renamed);
    expect(step(w, "c")).toMatchObject({ branches: { c1: "r" } });
    expect(toFlow(w, []).edges.find((e) => e.id === "c:c1")?.label).toBe("Receipts");
  });

  it("drops the link of a branch that was removed", () => {
    const fewer = { ...classify, categories: [classify.categories[1]] };
    expect(step(updateStep(sample(), fewer), "c")).toMatchObject({ branches: {} });
  });

  it("never changes a step's id or type through an update", () => {
    const sneaky = { ...rename, id: "other" } as Step;
    expect(updateStep(sample(), sneaky)).toEqual(sample());
  });
});

describe("moveStep", () => {
  it("changes nothing when the step is already there", () => {
    const w = sample();
    expect(moveStep(w, "r", { x: 0, y: 320 })).toBe(w);
    expect(moveStep(w, "r", { x: 0.3, y: 319.8 })).toBe(w);
  });

  it("changes only the position", () => {
    expect(step(moveStep(sample(), "r", { x: 9, y: 9 }), "r")).toEqual({ ...rename, position: { x: 9, y: 9 } });
  });
});
