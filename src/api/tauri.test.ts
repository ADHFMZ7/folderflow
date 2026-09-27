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
    ["getWorkflow", ["w1"], "get_workflow", { id: "w1" }],
    ["createWorkflow", ["receipts"], "create_workflow", { templateId: "receipts" }],
    ["createWorkflow", [null], "create_workflow", { templateId: null }],
    ["saveWorkflow", [{ id: "w1" }], "save_workflow", { workflow: { id: "w1" } }],
    ["deleteWorkflow", ["w1"], "delete_workflow", { id: "w1" }],
    ["validateWorkflow", [{ id: "w1" }], "validate_workflow", { workflow: { id: "w1" } }],
    ["getDraft", ["w1"], "get_draft", { id: "w1" }],
    ["saveDraft", [{ id: "w1" }], "save_draft", { workflow: { id: "w1" } }],
    ["applyDraft", ["w1"], "apply_draft", { id: "w1" }],
    ["discardDraft", ["w1"], "discard_draft", { id: "w1" }],
    ["chooseFolder", ["~/Documents"], "choose_folder", { start: "~/Documents" }],
    ["chooseFolder", [], "choose_folder", { start: null }],
    ["chooseCsv", [], "choose_csv", { start: null }],
    ["chooseFiles", [], "choose_files", { start: null }],
    ["runNow", ["w1", ["~/a.pdf"]], "run_now", { workflowId: "w1", files: ["~/a.pdf"] }],
    ["listRuns", [], "list_runs", { query: {} }],
    ["listRuns", [{ workflowId: "w1", limit: 5 }], "list_runs", { query: { workflowId: "w1", limit: 5 } }],
    ["getRun", ["r1"], "get_run", { id: "r1" }],
  ] as const)("%s invokes %s", async (method, args, cmd, expected) => {
    const calls = recordCalls();
    const api = createTauriApi() as unknown as Record<string, (...a: unknown[]) => Promise<unknown>>;

    await api[method](...args);

    expect(calls).toHaveLength(1);
    expect(calls[0].cmd).toBe(cmd);
    expect(calls[0].args ?? {}).toEqual(expected);
  });

  it("hears run-changed events until told to stop", async () => {
    mockIPC(() => null, { shouldMockEvents: true });
    const heard: unknown[] = [];
    const stop = createTauriApi().onRunChanged((change) => heard.push(change));
    await new Promise((r) => setTimeout(r, 0));
    const { emit } = await import("@tauri-apps/api/event");

    await emit("run-changed", { runId: "r1", workflowId: "w1", status: "done" });
    stop();
    await emit("run-changed", { runId: "r2", workflowId: "w1", status: "done" });

    expect(heard).toEqual([{ runId: "r1", workflowId: "w1", status: "done" }]);
  });

  it("hears the activity and the menu bar's requests for a page", async () => {
    mockIPC(() => null, { shouldMockEvents: true });
    const activity: unknown[] = [];
    const pages: unknown[] = [];
    createTauriApi().onActivityChanged((a) => activity.push(a));
    createTauriApi().onNavigate((hash) => pages.push(hash));
    await new Promise((r) => setTimeout(r, 0));
    const { emit } = await import("@tauri-apps/api/event");

    await emit("activity-changed", { paused: true, running: 1, needsYou: 2 });
    await emit("navigate", "#/workflows");

    expect(activity).toEqual([{ paused: true, running: 1, needsYou: 2 }]);
    expect(pages).toEqual(["#/workflows"]);
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

  it("passes on a conflict", async () => {
    mockIPC(() => {
      throw { code: "conflict", message: "this workflow was saved somewhere else" };
    });
    await expect(createTauriApi().saveWorkflow({} as never)).rejects.toMatchObject({ code: "conflict" });
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
