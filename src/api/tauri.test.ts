// The Tauri adapter calls the right command with the right arguments, and turns
// command failures into ApiErrors. See docs/api-contract.md.

import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it } from "vitest";
import { createTauriApi } from "./tauri";
import { ApiError } from "./types";

afterEach(() => clearMocks());

function recordCalls(reply: unknown = null) {
  const calls: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    return reply;
  });
  return calls;
}

describe("tauri api", () => {
  it.each([
    ["getSettings", [], "get_settings", {}],
    ["updateSettings", [{ openAtLogin: false }], "update_settings", { change: { openAtLogin: false } }],
    ["listModelKinds", [], "list_model_kinds", {}],
    ["listProviders", [], "list_providers", {}],
    ["detect", ["ollama"], "detect", { providerId: "ollama" }],
    ["connect", ["groq", { apiKey: "gsk-123" }], "connect", { providerId: "groq", credentials: { apiKey: "gsk-123" } }],
    ["removeConnection", ["c1"], "remove_connection", { id: "c1" }],
    ["listModels", ["c1"], "list_models", { connectionId: "c1" }],
    ["listWorkflows", [], "list_workflows", {}],
    ["listTemplates", [], "list_templates", {}],
  ] as const)("%s invokes %s", async (method, args, cmd, expected) => {
    const calls = recordCalls();
    const api = createTauriApi() as unknown as Record<string, (...a: unknown[]) => Promise<unknown>>;

    await api[method](...args);

    expect(calls).toHaveLength(1);
    expect(calls[0].cmd).toBe(cmd);
    expect(calls[0].args ?? {}).toEqual(expected);
  });

  it("returns what the command returns", async () => {
    recordCalls({ settings: { setupComplete: true }, notice: null });
    await expect(createTauriApi().getSettings()).resolves.toEqual({ settings: { setupComplete: true }, notice: null });
  });

  it("turns a command error into an ApiError with its code", async () => {
    mockIPC(() => {
      throw { code: "too_new", message: "the settings file is from a newer version of FolderFlow (format 2)" };
    });

    const err = await createTauriApi().getSettings().catch((e) => e);

    expect(err).toBeInstanceOf(ApiError);
    expect(err.code).toBe("too_new");
    expect(err.message).toContain("newer version");
  });

  it("wraps an unexpected failure as an io ApiError", async () => {
    mockIPC(() => {
      throw "the command panicked";
    });

    const err = await createTauriApi().listProviders().catch((e) => e);

    expect(err).toBeInstanceOf(ApiError);
    expect(err.code).toBe("io");
    expect(err.message).toBe("the command panicked");
  });
});
