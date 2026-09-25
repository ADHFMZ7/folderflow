// The only way screens reach the backend. Implemented by the Rust core through
// Tauri commands (tauri.ts) and by the mock for the browser, tests and previews.
// The command-by-command contract is in docs/api-contract.md.

import { createContext, useContext } from "react";
import type {
  ConnectOutcome, Credentials, DetectResult, LoadedSettings, Model, ModelKind, Provider, Settings,
  SettingsChange, Template, WorkflowSummary,
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
}

export const ApiContext = createContext<Api | null>(null);

export function useApi(): Api {
  const api = useContext(ApiContext);
  if (!api) throw new Error("useApi must be used inside <ApiContext.Provider>");
  return api;
}
