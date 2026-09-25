import { describe, expect, it } from "vitest";
import type { Model, ModelKind } from "../../api/types";
import { kindStatuses, missingKinds } from "./defaults";

const kinds: ModelKind[] = [
  { id: "llm", name: "LLM", description: "", usedBy: ["Extract"] },
  { id: "system1", name: "System 1", description: "", usedBy: ["Classify"] },
];
const models: Model[] = [{ id: "a", connectionId: "c1", name: "A", kind: "llm" }];
const defaults = { llm: { connectionId: "c1", modelId: "a" }, system1: null };

describe("kindStatuses", () => {
  it("pairs each kind with its model, or null", () => {
    expect(kindStatuses(kinds, defaults, models).map((s) => s.model?.id ?? null)).toEqual(["a", null]);
  });

  it("treats a default whose model is gone as unset", () => {
    const stale = { llm: { connectionId: "gone", modelId: "a" } };
    expect(kindStatuses(kinds, stale, models)[0].model).toBeNull();
  });
});

describe("missingKinds", () => {
  it("lists needed kinds with no working model", () => {
    expect(missingKinds(["llm", "system1"], kinds, defaults, models).map((k) => k.id)).toEqual(["system1"]);
    expect(missingKinds(["llm"], kinds, defaults, models)).toEqual([]);
  });
});
