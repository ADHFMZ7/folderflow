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
