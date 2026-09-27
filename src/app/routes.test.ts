import { describe, expect, it } from "vitest";
import { hrefFor, parseHash } from "./routes";

describe("routes", () => {
  it("reads pages and workflows from the hash", () => {
    expect(parseHash("#/settings")).toEqual({ page: "settings" });
    expect(parseHash("#/workflows/0f8c2b1e-6b0a-4c1e-9d6a-2f1f6c0f7a11")).toEqual({
      page: "workflow", id: "0f8c2b1e-6b0a-4c1e-9d6a-2f1f6c0f7a11",
    });
    expect(parseHash("#/history/5d0b6a0e-0000-4000-8000-0000000000aa")).toEqual({
      page: "run", id: "5d0b6a0e-0000-4000-8000-0000000000aa",
    });
    expect(parseHash("")).toEqual({ page: "workflows" });
    expect(parseHash("#/nonsense")).toEqual({ page: "workflows" });
  });

  it("writes them back", () => {
    expect(hrefFor({ page: "workflow", id: "abc" })).toBe("#/workflows/abc");
    expect(hrefFor({ page: "history" })).toBe("#/history");
    expect(hrefFor({ page: "run", id: "abc" })).toBe("#/history/abc");
  });
});
