// The only way screens reach the backend. Implemented by the mock for now and by
// Tauri commands once the Rust side exists, so screens never change when it does.

import { createContext, useContext } from "react";
import type {
  ConnectResult, Credentials, DetectResult, Model, ModelKind, Provider, Settings, Template, WorkflowSummary,
} from "./types";

export interface Api {
  getSettings(): Promise<Settings>;
  saveSettings(settings: Settings): Promise<void>;

  listModelKinds(): Promise<ModelKind[]>;
  listProviders(): Promise<Provider[]>;
  /** For providers connected by "detect": is it running on this Mac? */
  detect(providerId: string): Promise<DetectResult>;
  connect(providerId: string, credentials: Credentials): Promise<ConnectResult>;
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
