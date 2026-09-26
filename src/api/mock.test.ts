// The mock follows the same rules as the Rust core (docs/api-contract.md), so
// screens tested against it behave the same in the app.

import { describe, expect, it } from "vitest";
import { createMockApi } from "./mock";
import { ApiError } from "./types";

const KEY = "sk-live-1234567890abcdef";

function storage() {
  const data = new Map<string, string>();
  return {
    getItem: (k: string) => data.get(k) ?? null,
    setItem: (k: string, v: string) => void data.set(k, v),
    dump: () => [...data.values()].join("\n"),
  };
}

const api = (opts: Parameters<typeof createMockApi>[0] = {}) => createMockApi({ delayMs: 0, ...opts });

describe("getSettings", () => {
  it("starts fresh with no notice", async () => {
    const { settings, notice } = await api().getSettings();
    expect(notice).toBeNull();
    expect(settings).toMatchObject({ setupComplete: false, openAtLogin: true, connections: [] });
  });

  it("reports a recovered settings file", async () => {
    const { notice } = await api({ storedFile: "damaged" }).getSettings();
    expect(notice).toEqual({ kind: "recovered", backup: expect.stringContaining("settings.damaged") });
  });

  it("keeps reporting a recovered file for the whole session", async () => {
    const a = api({ storedFile: "damaged" });
    const first = await a.getSettings();
    await a.updateSettings({ openAtLogin: false });
    const second = await a.getSettings();
    expect(second.notice).toEqual(first.notice);
    expect(second.notice).not.toBeNull();
  });

  it("refuses settings from a newer version", async () => {
    await expect(api({ storedFile: "tooNew" }).getSettings()).rejects.toMatchObject({ code: "too_new" });
  });
});

describe("updateSettings", () => {
  it("applies only what changed and returns the saved settings", async () => {
    const a = api({ settings: { openAtLogin: false } });
    const saved = await a.updateSettings({ setupComplete: true });
    expect(saved).toMatchObject({ setupComplete: true, openAtLogin: false });
    expect((await a.getSettings()).settings).toEqual(saved);
  });

  it("rejects a default that points at a missing connection and changes nothing", async () => {
    const a = api();
    const err = await a.updateSettings({ defaults: { llm: { connectionId: "nope", modelId: "x" } } }).catch((e) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect(err.code).toBe("invalid");
    expect((await a.getSettings()).settings.defaults).toEqual({});
  });
});

describe("connect", () => {
  it("saves the connection and gives kinds without a model one of its models", async () => {
    const a = api();
    const outcome = await a.connect("ollama", {});
    if (!outcome.ok) throw new Error(outcome.error);
    expect(outcome.settings.connections).toEqual([outcome.connection]);
    expect(outcome.settings.defaults.llm).toEqual({ connectionId: outcome.connection.id, modelId: "qwen3.5:9b" });
    expect(outcome.settings.defaults.system1 ?? null).toBeNull();
    expect((await a.getSettings()).settings).toEqual(outcome.settings);
  });

  it("leaves a working default alone when another provider is connected", async () => {
    const a = api();
    const first = await a.connect("ollama", {});
    const second = await a.connect("groq", { apiKey: "gsk-1234567890ab" });
    if (!first.ok || !second.ok) throw new Error("connect failed");
    expect(second.settings.defaults.llm?.connectionId).toBe(first.connection.id);
  });

  it("stores nothing when the key is rejected", async () => {
    const a = api();
    const outcome = await a.connect("anthropic", { apiKey: "short" });
    expect(outcome).toEqual({ ok: false, error: "That key was rejected. Check it and try again." });
    expect((await a.getSettings()).settings.connections).toEqual([]);
  });

  it("never writes a key into stored settings", async () => {
    const store = storage();
    const a = api({ storage: store });
    await a.connect("anthropic", { apiKey: KEY });
    await a.updateSettings({ setupComplete: true });
    expect(store.dump()).not.toContain(KEY);
  });

  it("remembers a custom server's address, without a trailing slash", async () => {
    const outcome = await api().connect("custom", { endpoint: "http://localhost:8080/" });
    if (!outcome.ok) throw new Error(outcome.error);
    expect(outcome.connection.endpoint).toBe("http://localhost:8080");
  });

  it("gives other providers no address", async () => {
    const outcome = await api().connect("ollama", {});
    if (!outcome.ok) throw new Error(outcome.error);
    expect(outcome.connection).not.toHaveProperty("endpoint");
  });

  it("fails with a provider error when a provider answers unexpectedly", async () => {
    const a = api({ faultyProviders: ["anthropic"] });
    await expect(a.connect("anthropic", { apiKey: "sk-long-enough-key" })).rejects.toMatchObject({ code: "provider" });
  });

  it("refuses an unknown provider", async () => {
    await expect(api().connect("nope", {})).rejects.toMatchObject({ code: "not_found" });
  });
});

describe("removeConnection", () => {
  it("removes the connection and clears defaults that used it", async () => {
    const a = api();
    const outcome = await a.connect("ollama", {});
    if (!outcome.ok) throw new Error(outcome.error);

    const settings = await a.removeConnection(outcome.connection.id);

    expect(settings.connections).toEqual([]);
    expect(settings.defaults.llm).toBeNull();
  });

  it("refuses an unknown connection", async () => {
    await expect(api().removeConnection("nope")).rejects.toMatchObject({ code: "not_found" });
  });
});

describe("workflows", () => {
  const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;

  it("creates a blank workflow, saved at revision 1 with one trigger", async () => {
    const a = api();
    const wf = await a.createWorkflow(null);

    expect(wf.id).toMatch(UUID);
    expect(wf).toMatchObject({ version: 1, name: "New workflow", revision: 1, enabled: false });
    expect(wf.steps).toEqual([expect.objectContaining({ type: "fileAdded", folder: "~/Downloads", next: null })]);
    expect(await a.listWorkflows()).toEqual([
      expect.objectContaining({ id: wf.id, name: "New workflow", trigger: "File added · ~/Downloads", status: "ok" }),
    ]);
  });

  it("builds a template with stable branch ids that link its steps", async () => {
    const wf = await api().createWorkflow("receipts");
    const classify = wf.steps.find((s) => s.type === "classify");
    if (classify?.type !== "classify") throw new Error("no classify step");

    const receipt = classify.categories.find((c) => c.label === "Receipt")!;
    expect(wf.name).toBe("Sort receipts");
    expect(wf.steps.find((s) => s.id === classify.branches[receipt.id])?.type).toBe("extract");
    expect(await api().validateWorkflow(wf)).toEqual([]);
  });

  it("lists the model kinds a workflow's AI steps need", async () => {
    const a = api();
    await a.createWorkflow("receipts");
    const [summary] = await a.listWorkflows();
    expect(new Set(summary.kindsNeeded)).toEqual(new Set(["system1", "llm"]));
  });

  it("bumps the revision on every save", async () => {
    const a = api();
    const wf = await a.createWorkflow(null);
    const { workflow } = await a.saveWorkflow({ ...wf, name: "Downloads tidy-up" });

    expect(workflow.revision).toBe(2);
    expect(await a.getWorkflow(wf.id)).toEqual(workflow);
  });

  it("refuses a save based on an older revision and keeps what's there", async () => {
    const a = api();
    const wf = await a.createWorkflow(null);
    await a.saveWorkflow({ ...wf, name: "Saved elsewhere" });

    await expect(a.saveWorkflow({ ...wf, name: "Stale edit" })).rejects.toMatchObject({ code: "conflict" });
    expect((await a.getWorkflow(wf.id)).name).toBe("Saved elsewhere");
  });

  it("won't turn on a workflow with problems, and writes nothing", async () => {
    const a = api();
    const wf = await a.createWorkflow(null);
    const broken = { ...wf, enabled: true, steps: [] };

    await expect(a.saveWorkflow(broken)).rejects.toMatchObject({ code: "invalid" });
    expect(await a.getWorkflow(wf.id)).toEqual(wf);
  });

  it("refuses ids that aren't UUIDs and unknown ones", async () => {
    await expect(api().getWorkflow("../settings")).rejects.toMatchObject({ code: "invalid" });
    await expect(api().getWorkflow("0f8c2b1e-6b0a-4c1e-9d6a-2f1f6c0f7a11")).rejects.toMatchObject({ code: "not_found" });
  });

  it("deletes a workflow", async () => {
    const a = api();
    const wf = await a.createWorkflow(null);
    await a.deleteWorkflow(wf.id);

    expect(await a.listWorkflows()).toEqual([]);
    await expect(a.getWorkflow(wf.id)).rejects.toMatchObject({ code: "not_found" });
  });

  it("lists a damaged file without opening it", async () => {
    const [summary] = await api({ damagedWorkflows: ["2b7e1c9a-0d3f-4e5a-8b6c-7d8e9f0a1b2c.json"] }).listWorkflows();
    expect(summary).toMatchObject({ status: "damaged", name: "2b7e1c9a-0d3f-4e5a-8b6c-7d8e9f0a1b2c.json" });
  });

  it("reports a workflow with no trigger", async () => {
    const wf = await api().createWorkflow(null);
    expect((await api().validateWorkflow({ ...wf, steps: [] })).map((p) => p.code)).toEqual(["no_trigger"]);
  });

  it("reports empty required text fields with the field they're about, as Rust does", async () => {
    const wf = await api().createWorkflow(null);
    const t = wf.steps[0];
    const steps = [
      { ...t, folder: " ", next: "m" },
      { id: "m", type: "move", title: "Move", position: { x: 0, y: 0 }, to: "", mode: "move", next: null },
    ] as typeof wf.steps;
    const problems = await api().validateWorkflow({ ...wf, steps });
    expect(problems).toEqual([
      { stepId: t.id, code: "required", message: "Fill in the folder to watch.", field: "folder" },
      { stepId: "m", code: "required", message: "Fill in the folder to move to.", field: "to" },
    ]);
  });
});

describe("templates", () => {
  it("offers the same templates as the Rust catalog, and builds each one", async () => {
    const a = api();
    const list = await a.listTemplates();
    expect(list.map((t) => t.id)).toEqual(["receipts", "screenshots", "summaries", "invoices", "cleanup", "paperwork"]);
    expect(list.find((t) => t.id === "cleanup")!.blurb).toBe("Every Friday at 17:00, a reminder to tidy Downloads");
    for (const t of list) expect((await a.createWorkflow(t.id)).name).toBe(t.name);
  });

  it("builds the Paperwork inbox with every step type but the other triggers", async () => {
    const wf = await api().createWorkflow("paperwork");
    const types = [...new Set(wf.steps.map((s) => s.type))].sort();
    expect(types).toEqual(["addRow", "agent", "askMe", "classify", "createFile", "extract", "fileAdded", "if", "move", "notify", "rename", "stop", "tag", "write"]);
    const classify = wf.steps.find((s) => s.type === "classify");
    expect(classify?.type === "classify" && classify.categories.every((c) => c.description)).toBe(true);
    const ids = new Set(wf.steps.map((s) => s.id));
    for (const s of wf.steps) {
      const exits = "next" in s ? [s.next] : "branches" in s ? Object.values(s.branches) : [];
      for (const to of exits) if (to) expect(ids).toContain(to);
    }
  });
});

describe("pickers", () => {
  it("answers with the folder or file set in the options, or null for a cancel", async () => {
    expect(await api({ chosenFolder: "~/Documents/Receipts" }).chooseFolder()).toBe("~/Documents/Receipts");
    expect(await api({ chosenFolder: null }).chooseFolder()).toBeNull();
    expect(await api({ chosenCsv: "~/Documents/Expenses.csv" }).chooseCsv()).toBe("~/Documents/Expenses.csv");
  });

  it("answers with a sample path in previews", async () => {
    expect(await api().chooseFolder()).toBe("~/Documents");
    expect(await api().chooseCsv()).toBe("~/Documents/Log.csv");
  });
});
