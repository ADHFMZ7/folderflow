// The only way screens reach the backend. Implemented by the Rust core through
// Tauri commands (tauri.ts) and by the mock for the browser, tests and previews.
// The command-by-command contract is in docs/api-contract.md.

import { createContext, useContext } from "react";
import type {
  ConnectOutcome, Credentials, DetectResult, LoadedSettings, Model, ModelKind, Problem, Provider, SaveResult,
  Settings, SettingsChange, Template, Workflow, WorkflowSummary,
} from "./types";

/** Every method may reject with an ApiError. */
export interface Api {
  getSettings(): Promise<LoadedSettings>;
  /** Applies a partial change and returns the settings as saved. */
  updateSettings(change: SettingsChange): Promise<Settings>;

  listModelKinds(): Promise<ModelKind[]>;
  listProviders(): Promise<Provider[]>;
  /** For providers connected by "detect": is it running on this Mac? */
  detect(providerId: string): Promise<DetectResult>;
  /** Checks the credentials with the provider, then saves the connection and its key. */
  connect(providerId: string, credentials: Credentials): Promise<ConnectOutcome>;
  /** Deletes the connection and its key, and clears defaults that used it. */
  removeConnection(id: string): Promise<Settings>;
  listModels(connectionId: string): Promise<Model[]>;

  listWorkflows(): Promise<WorkflowSummary[]>;
  listTemplates(): Promise<Template[]>;

  // Workflows: see docs/workflow-format.md.
  getWorkflow(id: string): Promise<Workflow>;
  /** A blank workflow, or a copy of a template. Saved at once, with revision 1. */
  createWorkflow(templateId: string | null): Promise<Workflow>;
  /** Fails with "conflict" if `workflow.revision` isn't the revision on disk. */
  saveWorkflow(workflow: Workflow): Promise<SaveResult>;
  /** Moves the workflow's file to the trash. */
  deleteWorkflow(id: string): Promise<void>;
  validateWorkflow(workflow: Workflow): Promise<Problem[]>;

  // Native pickers: a path with the home folder written as ~, or null if cancelled.
  chooseFolder(start?: string): Promise<string | null>;
  /** A new or existing .csv file. */
  chooseCsv(start?: string): Promise<string | null>;
}

export const ApiContext = createContext<Api | null>(null);

export function useApi(): Api {
  const api = useContext(ApiContext);
  if (!api) throw new Error("useApi must be used inside <ApiContext.Provider>");
  return api;
}
