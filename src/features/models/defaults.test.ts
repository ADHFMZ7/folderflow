import { describe, expect, it } from "vitest";
import type { Model, ModelKind } from "../../api/types";
import { fillDefaults, missingKinds } from "./defaults";

const kinds: ModelKind[] = [
  { id: "llm", name: "LLM", description: "", usedBy: ["Extract"] },
  { id: "system1", name: "System 1", description: "", usedBy: ["Classify"] },
];
const models: Model[] = [
  { id: "a", connectionId: "c1", name: "A", kind: "llm" },
  { id: "b", connectionId: "c1", name: "B", kind: "llm" },
];

describe("fillDefaults", () => {
  it("gives each kind the first model of that kind", () => {
    expect(fillDefaults({}, kinds, models)).toEqual({ llm: { connectionId: "c1", modelId: "a" }, system1: null });
  });

  it("keeps a default that still points at a model", () => {
    const defaults = { llm: { connectionId: "c1", modelId: "b" } };
    expect(fillDefaults(defaults, kinds, models).llm).toEqual({ connectionId: "c1", modelId: "b" });
  });

  it("replaces a default whose model has gone", () => {
    const defaults = { llm: { connectionId: "gone", modelId: "x" } };
    expect(fillDefaults(defaults, kinds, models).llm).toEqual({ connectionId: "c1", modelId: "a" });
  });
});

describe("missingKinds", () => {
  it("lists needed kinds with no working model", () => {
    const defaults = fillDefaults({}, kinds, models);
    expect(missingKinds(["llm", "system1"], kinds, defaults, models).map((k) => k.id)).toEqual(["system1"]);
    expect(missingKinds(["llm"], kinds, defaults, models)).toEqual([]);
  });
});
