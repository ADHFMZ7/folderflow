// The Api implemented by Tauri commands in the Rust core. One command per
// method; see docs/api-contract.md.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Api } from "./api";
import { ApiError, type ApiErrorCode, type RunChanged } from "./types";

const CODES: ApiErrorCode[] = ["too_new", "not_found", "invalid", "keychain", "provider", "io", "conflict"];

function toApiError(e: unknown): ApiError {
  if (e && typeof e === "object" && "code" in e && "message" in e && CODES.includes(e.code as ApiErrorCode)) {
    return new ApiError(e.code as ApiErrorCode, String(e.message));
  }
  return new ApiError("io", typeof e === "string" ? e : "FolderFlow's core didn't respond as expected.");
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (e) {
    throw toApiError(e);
  }
}

export function createTauriApi(): Api {
  return {
    getSettings: () => call("get_settings"),
    updateSettings: (change) => call("update_settings", { change }),
    listModelKinds: () => call("list_model_kinds"),
    listProviders: () => call("list_providers"),
    detect: (providerId) => call("detect", { providerId }),
    connect: (providerId, credentials) => call("connect", { providerId, credentials }),
    removeConnection: (id) => call("remove_connection", { id }),
    listModels: (connectionId) => call("list_models", { connectionId }),
    listWorkflows: () => call("list_workflows"),
    listTemplates: () => call("list_templates"),
    getWorkflow: (id) => call("get_workflow", { id }),
    createWorkflow: (templateId) => call("create_workflow", { templateId }),
    saveWorkflow: (workflow) => call("save_workflow", { workflow }),
    deleteWorkflow: (id) => call("delete_workflow", { id }),
    validateWorkflow: (workflow) => call("validate_workflow", { workflow }),
    getDraft: (id) => call("get_draft", { id }),
    saveDraft: (workflow) => call("save_draft", { workflow }),
    applyDraft: (id) => call("apply_draft", { id }),
    discardDraft: (id) => call("discard_draft", { id }),
    chooseFolder: (start) => call("choose_folder", { start: start ?? null }),
    chooseCsv: (start) => call("choose_csv", { start: start ?? null }),
    chooseFiles: (start) => call("choose_files", { start: start ?? null }),
    runNow: (workflowId, files) => call("run_now", { workflowId, files }),
    listRuns: (query = {}) => call("list_runs", { query }),
    getRun: (id) => call("get_run", { id }),
    onRunChanged(listener) {
      // listen() resolves later; a stop that comes first unlistens as soon as it does.
      let stopped = false;
      let unlisten: (() => void) | null = null;
      listen<RunChanged>("run-changed", (event) => listener(event.payload)).then((fn) => {
        if (stopped) fn();
        else unlisten = fn;
      }).catch(() => {});
      return () => { stopped = true; unlisten?.(); };
    },
  };
}

export const inTauri = () => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
