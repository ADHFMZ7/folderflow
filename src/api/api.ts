// The only way screens reach the backend. Implemented by the Rust core through
// Tauri commands (tauri.ts) and by the mock for the browser, tests and previews.
// The command-by-command contract is in docs/api-contract.md.

import { createContext, useContext } from "react";
import type {
  ConnectOutcome, Credentials, DetectResult, LoadedSettings, Model, ModelKind, Problem, Provider, Run, RunChanged,
  Activity, NeedsYouItem, Notice, RunQuery, RunSummary, SaveResult, UndoResult, Settings, SettingsChange, Template, Workflow, WorkflowSummary,
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

  // Drafts: edits to a workflow that is on wait here until applied. See "Drafts" in docs/workflow-format.md.
  getDraft(id: string): Promise<Workflow | null>;
  /** Saves the draft; never touches the running workflow. */
  saveDraft(workflow: Workflow): Promise<SaveResult>;
  /** Makes the draft the running workflow, at the next revision. */
  applyDraft(id: string): Promise<SaveResult>;
  /** Moves the draft to the trash. */
  discardDraft(id: string): Promise<void>;

  // Native pickers: a path with the home folder written as ~, or null if cancelled.
  chooseFolder(start?: string): Promise<string | null>;
  /** An existing .csv file. A new file's path is typed instead. */
  chooseCsv(start?: string): Promise<string | null>;
  /** Files to run a workflow on; empty if cancelled. */
  chooseFiles(start?: string): Promise<string[]>;

  // Runs: see docs/engine.md.
  /** Queues one run of the saved workflow per file and returns them queued. All files are checked first. */
  runNow(workflowId: string, files: string[]): Promise<RunSummary[]>;
  /** Newest first. */
  listRuns(query?: RunQuery): Promise<RunSummary[]>;
  getRun(id: string): Promise<Run>;
  /** Questions, failed runs and interrupted runs, newest first. */
  listNeedsYou(): Promise<NeedsYouItem[]>;
  /** Answers a waiting run's question; the run carries on down that branch. "conflict" if it isn't waiting. */
  answer(runId: string, branchId: string): Promise<Run>;
  /** Runs a failed run again from the step that failed. */
  retryRun(runId: string): Promise<Run>;
  /** Carries on a run cut off by FolderFlow quitting; a step that was cut off runs again. */
  resumeRun(runId: string): Promise<Run>;
  /** Reverses the run's file changes, newest first; files changed since are left alone and listed. */
  undoRun(runId: string): Promise<UndoResult>;
  /** Takes a failed or interrupted run out of Needs you, changing nothing else. */
  dismissRun(runId: string): Promise<void>;
  /** Calls `listener` on every run-changed event until the returned function is called. */
  onRunChanged(listener: (change: RunChanged) => void): () => void;

  // The bell and Pause all: see docs/engine.md, "Notifications" and "Pause all".
  /** Newest first, at most 200. */
  listNotices(): Promise<Notice[]>;
  /** Marks these notifications read, or all of them when `ids` is left out. */
  markNoticesRead(ids?: string[]): Promise<void>;
  /** Calls `listener` with the unread count whenever the list changes, until the returned function is called. */
  onNoticesChanged(listener: (unread: number) => void): () => void;
  getActivity(): Promise<Activity>;
  /** While paused, new files wait and scheduled times pass by; runs already going finish, and Run now still works. */
  pauseAll(paused: boolean): Promise<Activity>;
}

export const ApiContext = createContext<Api | null>(null);

export function useApi(): Api {
  const api = useContext(ApiContext);
  if (!api) throw new Error("useApi must be used inside <ApiContext.Provider>");
  return api;
}
