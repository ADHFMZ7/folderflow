// A stand-in backend for the browser, tests and previews. Keeps settings in
// browser storage and fakes detection and key checks. The app uses tauri.ts.

import type { Api } from "./api";
import {
  ApiError, type Activity, type Connection, type Model, type ModelKind, type ModelRef, type Notice, type NoticeKind, type Provider, type Run, type RunChanged,
  type Settings, type SettingsNotice, type Template, type Workflow,
} from "./types";
import { after, execute, needsYouOf, summaryOf } from "./mockRuns";
import {
  blankWorkflow, damagedSummary, roughValidate, sampleWorkflows, summarize, templateWorkflow, UUID,
} from "./mockWorkflows";

export const MODEL_KINDS: ModelKind[] = [
  {
    id: "llm",
    name: "LLM",
    description: "Reads and writes text: pulling out details, writing summaries, open-ended steps.",
    usedBy: ["Extract", "Write", "Agent step"],
  },
  {
    id: "system1",
    name: "System 1",
    description: "Fast, lightweight models that pick one option from a list.",
    usedBy: ["Classify"],
  },
];

export const PROVIDERS: Provider[] = [
  { id: "ollama", name: "Ollama", location: "local", connect: "detect", kinds: ["llm"],
    privacy: "Runs on this Mac. Files never leave it.", helpUrl: "https://ollama.com" },
  { id: "custom", name: "Custom server", location: "local", connect: "endpoint", kinds: ["llm"],
    privacy: "Files go to the server address you enter." },
  { id: "anthropic", name: "Anthropic", location: "cloud", connect: "apiKey", kinds: ["llm"],
    privacy: "Files your workflows run on are sent to Anthropic.", keyUrl: "https://platform.claude.com/settings/keys" },
  { id: "openai", name: "OpenAI", location: "cloud", connect: "apiKey", kinds: ["llm"],
    privacy: "Files your workflows run on are sent to OpenAI.", keyUrl: "https://platform.openai.com/api-keys" },
  { id: "groq", name: "Groq", location: "cloud", connect: "apiKey", kinds: ["llm"],
    privacy: "Files your workflows run on are sent to Groq.", keyUrl: "https://console.groq.com/keys" },
  // Placeholder until Jev's real connection details are known.
  { id: "jev", name: "Jev", location: "cloud", connect: "apiKey", kinds: ["system1"],
    privacy: "Files your workflows run on are sent to Jev." },
];

const MODELS: Record<string, { id: string; name: string; kind: string }[]> = {
  ollama: [
    { id: "qwen3.5:9b", name: "qwen3.5:9b", kind: "llm" },
    { id: "llama3.2:3b", name: "llama3.2:3b", kind: "llm" },
  ],
  custom: [{ id: "default", name: "Server default", kind: "llm" }],
  anthropic: [{ id: "claude", name: "Claude", kind: "llm" }],
  openai: [{ id: "gpt", name: "GPT", kind: "llm" }],
  groq: [{ id: "openai/gpt-oss-120b", name: "gpt-oss-120b", kind: "llm" }],
  jev: [{ id: "jev", name: "Jev", kind: "system1" }],
};

export const TEMPLATES: Template[] = [
  { id: "receipts", name: "Sort receipts", blurb: "Rename receipts by date and vendor, and file them by year", trigger: "File added" },
  { id: "screenshots", name: "Tidy screenshots", blurb: "Move screenshots off the Desktop into a dated folder", trigger: "File added" },
  { id: "summaries", name: "Summarise PDFs", blurb: "Write a one-paragraph summary next to each new PDF", trigger: "File added" },
  { id: "invoices", name: "Log invoices", blurb: "Add each invoice to a spreadsheet, and ask before big ones", trigger: "File added" },
  { id: "cleanup", name: "Weekly clean-up", blurb: "Every Friday at 17:00, a reminder to tidy Downloads", trigger: "Schedule" },
  { id: "paperwork", name: "Paperwork inbox", blurb: "Sort receipts, invoices and contracts from Downloads: rename, file and log them, and ask before big invoices", trigger: "File added" },
];

export const FRESH_SETTINGS: Settings = { setupComplete: false, openAtLogin: true, appearance: "system", connections: [], defaults: {} };

type KeyValueStore = Pick<Storage, "getItem" | "setItem">;

export type MockOptions = {
  storage?: KeyValueStore;
  delayMs?: number;
  ollamaRunning?: boolean;
  /** Start with sample workflows, one of which needs you. */
  sampleWorkflows?: boolean;
  /** File names of workflow files that can't be read. */
  damagedWorkflows?: string[];
  /** Settings already on disk before the app starts. */
  settings?: Partial<Settings>;
  /** Pretend the settings file on disk was damaged, or written by a newer version. */
  storedFile?: "damaged" | "tooNew";
  /** Providers that answer with something unreadable, like a server error. */
  faultyProviders?: string[];
  /** What the folder picker answers; null is a cancel. A sample folder when unset. */
  chosenFolder?: string | null;
  /** What the CSV picker answers; null is a cancel. A sample file when unset. */
  chosenCsv?: string | null;
  /** What the file picker answers; empty is a cancel. A sample file when unset. */
  chosenFiles?: string[];
};

const SETTINGS_KEY = "vela.settings";

function memoryStore(): KeyValueStore {
  const data = new Map<string, string>();
  return { getItem: (k) => data.get(k) ?? null, setItem: (k, v) => void data.set(k, v) };
}

/** Follows the same rules as the Rust core; see docs/api-contract.md. */
export function createMockApi(options: MockOptions = {}): Api {
  const { storage = memoryStore(), delayMs = 400, ollamaRunning = true } = options;
  const wait = () => new Promise((r) => setTimeout(r, delayMs));
  let connectionCount = 0;
  // Like the Rust core, a recovery is reported by every getSettings for the rest of the session.
  const notice: SettingsNotice = options.storedFile === "damaged"
    ? { kind: "recovered", backup: "~/Library/Application Support/com.adhfmz7.vela/settings.damaged-1790300000-3f2a9c1e.json" }
    : null;
  if (options.settings) write({ ...FRESH_SETTINGS, ...options.settings });

  // Runs, oldest first, and whoever is listening for changes to them.
  const runRecords: Run[] = [];

  // Workflows, keyed by id; sample ones come with made-up past runs for previews.
  const WORKFLOWS_KEY = "vela.workflows";
  const readWorkflows = (): Record<string, Workflow> => JSON.parse(storage.getItem(WORKFLOWS_KEY) ?? "{}");
  const writeWorkflows = (all: Record<string, Workflow>) => storage.setItem(WORKFLOWS_KEY, JSON.stringify(all));
  if (options.sampleWorkflows && !storage.getItem(WORKFLOWS_KEY)) {
    const all: Record<string, Workflow> = {};
    for (const { workflow, runs } of sampleWorkflows()) {
      all[workflow.id] = workflow;
      runRecords.push(...runs);
    }
    writeWorkflows(all);
  }
  const checkId = (id: string) => {
    if (!UUID.test(id)) throw new ApiError("invalid", "That isn't a workflow id.");
  };
  const stored = (id: string) => {
    checkId(id);
    const wf = readWorkflows()[id];
    if (!wf) throw new ApiError("not_found", "That workflow doesn't exist.");
    return wf;
  };
  const put = (wf: Workflow) => writeWorkflows({ ...readWorkflows(), [wf.id]: wf });

  // Drafts, keyed by workflow id, following the same rules as the Rust core.
  const DRAFTS_KEY = "vela.drafts";
  const readDrafts = (): Record<string, Workflow> => JSON.parse(storage.getItem(DRAFTS_KEY) ?? "{}");
  const writeDrafts = (all: Record<string, Workflow>) => storage.setItem(DRAFTS_KEY, JSON.stringify(all));
  const dropDraft = (id: string) => {
    const { [id]: _dropped, ...rest } = readDrafts();
    writeDrafts(rest);
  };
  /** The draft, if it still belongs to the running revision; a stale one is dropped. */
  const draftOf = (live: Workflow): Workflow | null => {
    const draft = readDrafts()[live.id];
    if (!draft) return null;
    if (draft.revision !== live.revision) { dropDraft(live.id); return null; }
    return draft;
  };

  const listeners = new Set<(change: RunChanged) => void>();
  const announce = (run: Run) => {
    activityChanged();
    for (const listener of [...listeners]) listener({ runId: run.id, workflowId: run.workflowId, status: run.status });
  };

  // The bell's list, newest first, and Pause all. There are no folders here,
  // so pausing only changes what the title bar shows.
  const notices: Notice[] = [];
  const noticeListeners = new Set<(unread: number) => void>();
  const noticesChanged = () => {
    const unread = notices.filter((n) => !n.read).length;
    for (const listener of [...noticeListeners]) listener(unread);
  };
  const tell = (kind: NoticeKind, run: Run, message: string) => {
    notices.unshift({
      id: crypto.randomUUID(), kind, workflowId: run.workflowId, workflowName: run.workflow.name, runId: run.id,
      message, at: new Date().toISOString(), read: false,
    });
    notices.splice(200);
    noticesChanged();
  };
  let paused = false;
  const activity = (): Activity => ({
    paused,
    running: runRecords.filter((r) => r.status === "queued" || r.status === "running").length,
    needsYou: runRecords.filter((r) => needsYouOf(r) !== null).length,
  });
  // Like the core, told only when something in it changed.
  const activityListeners = new Set<(activity: Activity) => void>();
  let told = activity();
  function activityChanged() {
    const now = activity();
    if (now.paused === told.paused && now.running === told.running && now.needsYou === told.needsYou) return;
    told = now;
    for (const listener of [...activityListeners]) listener(now);
  }
  // Like the core, a workflow's runs go one at a time, in order.
  let queue = Promise.resolve();
  const later = () => new Promise((r) => setTimeout(r, delayMs));
  const enqueue = (run: Run) => {
    queue = queue.then(async () => {
      await later();
      if (run.status !== "queued") return;
      run.status = "running";
      announce(run);
      execute(run, run.workflow, (kind, message) => tell(kind, run, message));
      announce(run);
    });
  };
  const runOf = (id: string) => {
    const run = runRecords.find((r) => r.id === id);
    if (!run) throw new ApiError("not_found", "That run no longer exists.");
    return run;
  };
  /** Queues a run again, to carry on from `at`. */
  const requeue = (run: Run, at: string | null) => {
    Object.assign(run, { continueAt: at, error: null, endedAt: null, dismissed: false, status: "queued" });
    announce(run);
    enqueue(run);
    return structuredClone(run);
  };
  /** Each workflow's newest run, and how many of its runs need the person. */
  const runsOf = (workflowId: string) => {
    const mine = runRecords.filter((r) => r.workflowId === workflowId);
    const newest = mine.reduce<Run | null>((a, r) => (!a || r.startedAt >= a.startedAt ? r : a), null);
    return { needsYou: mine.filter((r) => needsYouOf(r)).length, lastRun: newest && summaryOf(newest) };
  };

  function read(): Settings {
    if (options.storedFile === "tooNew") {
      throw new ApiError("too_new", "the settings file is from a newer version of Vela (format 2)");
    }
    const raw = storage.getItem(SETTINGS_KEY);
    return raw ? { ...FRESH_SETTINGS, ...JSON.parse(raw) } : FRESH_SETTINGS;
  }

  function write(settings: Settings) {
    storage.setItem(SETTINGS_KEY, JSON.stringify(settings));
  }

  const modelsFor = (connection: Connection): Model[] =>
    (MODELS[connection.providerId] ?? []).map((m) => ({ ...m, connectionId: connection.id }));

  const faulty = (providerId: string) => {
    if (!options.faultyProviders?.includes(providerId)) return;
    const name = PROVIDERS.find((p) => p.id === providerId)?.name ?? providerId;
    throw new ApiError("provider", `${name} sent an answer Vela couldn't read. Try again later.`);
  };

  const isWorking = (settings: Settings, ref: ModelRef | null | undefined) =>
    !!ref && settings.connections.some((c) => c.id === ref.connectionId);

  return {
    async getSettings() {
      return { settings: read(), notice };
    },

    async updateSettings(change) {
      const settings = read();
      for (const ref of Object.values(change.defaults ?? {})) {
        if (ref && !settings.connections.some((c) => c.id === ref.connectionId)) {
          throw new ApiError("invalid", "a default points at a connection that doesn't exist");
        }
      }
      const next = { ...settings, ...change };
      write(next);
      return next;
    },

    async listModelKinds() { return MODEL_KINDS; },
    async listProviders() { return PROVIDERS; },

    async detect(providerId) {
      await wait();
      faulty(providerId);
      if (providerId !== "ollama") return { found: false, reason: "This provider can't be detected." };
      return ollamaRunning ? { found: true } : { found: false, reason: "Ollama isn't running on this Mac." };
    },

    async connect(providerId, credentials) {
      await wait();
      const provider = PROVIDERS.find((p) => p.id === providerId);
      if (!provider) throw new ApiError("not_found", `no provider with id ${providerId}`);
      faulty(providerId);
      if (provider.connect === "detect" && !ollamaRunning) return { ok: false, error: "Couldn't reach Ollama." };
      if (provider.connect === "apiKey" && !(credentials.apiKey ?? "").trim()) return { ok: false, error: "Enter an API key." };
      if (provider.connect === "apiKey" && (credentials.apiKey ?? "").trim().length < 12)
        return { ok: false, error: "That key was rejected. Check it and try again." };
      if (provider.connect === "endpoint" && !/^https?:\/\//.test(credentials.endpoint ?? ""))
        return { ok: false, error: "Enter an address starting with http:// or https://." };

      // The key itself would go to the Keychain; the mock keeps nothing of it.
      const settings = read();
      const connection: Connection = { id: `${providerId}-${Date.now()}-${++connectionCount}`, providerId };
      if (provider.connect === "endpoint") connection.endpoint = credentials.endpoint!.replace(/\/+$/, "");
      const connections = [...settings.connections, connection];
      const withConnection = { ...settings, connections };
      const defaults = { ...settings.defaults };
      for (const kind of MODEL_KINDS) {
        if (isWorking(withConnection, defaults[kind.id])) continue;
        const first = modelsFor(connection).find((m) => m.kind === kind.id);
        defaults[kind.id] = first ? { connectionId: connection.id, modelId: first.id } : null;
      }
      const next = { ...withConnection, defaults };
      write(next);
      return { ok: true, connection, settings: next };
    },

    async removeConnection(id) {
      const settings = read();
      if (!settings.connections.some((c) => c.id === id)) throw new ApiError("not_found", `no connection with id ${id}`);
      const defaults = Object.fromEntries(
        Object.entries(settings.defaults).map(([kind, ref]) => [kind, ref?.connectionId === id ? null : ref]),
      );
      const next = { ...settings, connections: settings.connections.filter((c) => c.id !== id), defaults };
      write(next);
      return next;
    },

    async listModels(connectionId) {
      const connection = read().connections.find((c) => c.id === connectionId);
      if (!connection) throw new ApiError("not_found", `no connection with id ${connectionId}`);
      faulty(connection.providerId);
      return modelsFor(connection);
    },

    async listWorkflows() {
      const saved = Object.values(readWorkflows())
        .map((wf) => ({ ...summarize(wf, runsOf(wf.id)), hasDraft: !!draftOf(wf) }))
        .sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()) || a.id.localeCompare(b.id));
      return [...saved, ...(options.damagedWorkflows ?? []).map(damagedSummary)];
    },
    async listTemplates() { return TEMPLATES; },
    async chooseFolder() { return options.chosenFolder === undefined ? "~/Documents" : options.chosenFolder; },
    async chooseCsv() { return options.chosenCsv === undefined ? "~/Documents/Log.csv" : options.chosenCsv; },
    async chooseFiles() { return options.chosenFiles ?? ["~/Downloads/Scan_0042.pdf"]; },

    async runNow(workflowId, files) {
      const workflow = stored(workflowId);
      if (workflow.steps.some((s) => s.type === "schedule")) {
        throw new ApiError("invalid", "This workflow runs on its schedule, not on files.");
      }
      const problems = roughValidate(workflow).length;
      if (problems) {
        throw new ApiError("invalid", problems === 1
          ? "Fix the problem with this workflow before running it."
          : `Fix the ${problems} problems with this workflow before running it.`);
      }
      if (!files.length) throw new ApiError("invalid", "Choose at least one file to run on.");
      const queued = files.map((path, i): Run => ({
        id: crypto.randomUUID(), workflowId, revision: workflow.revision, workflow: structuredClone(workflow),
        trigger: { kind: "runNow", file: { path, inode: 1000 + runRecords.length + i } },
        file: { path, inode: 1000 + runRecords.length + i }, status: "queued",
        startedAt: new Date().toISOString(), endedAt: null, steps: [], values: {}, error: null, undo: null,
        waitingFor: null, continueAt: null, dismissed: false,
      }));
      for (const run of queued) {
        runRecords.push(run);
        announce(run);
        enqueue(run);
      }
      return queued.map(summaryOf);
    },

    async tryOnFile(workflow, path, answers = {}) {
      const trigger = workflow.steps.find((s) => s.type === "fileAdded" || s.type === "runNow" || s.type === "schedule");
      if (!trigger) throw new ApiError("invalid", "Add a trigger before trying this workflow.");
      if (trigger.type === "schedule") {
        throw new ApiError("invalid", "A scheduled workflow runs without a file, so there's no file to try it on.");
      }
      const problems: Record<string, string> = {};
      for (const p of roughValidate(workflow)) {
        if (p.stepId === null) throw new ApiError("invalid", p.message);
        problems[p.stepId] ??= p.message;
      }
      const file = { path, inode: 1 };
      const run: Run = {
        id: "try", workflowId: workflow.id, revision: workflow.revision, workflow: structuredClone(workflow),
        trigger: { kind: trigger.type, file }, file, status: "running", startedAt: new Date().toISOString(), endedAt: null,
        steps: [], values: {}, error: null, undo: null, waitingFor: null, continueAt: null, dismissed: false,
      };
      execute(run, run.workflow, () => {}, { answers, problems });
      return { status: run.status, steps: run.steps, values: run.values, question: run.waitingFor, error: run.error };
    },

    async listRuns(query = {}) {
      const newest = [...runRecords].reverse()
        .filter((r) => (query.workflowId === undefined || r.workflowId === query.workflowId)
          && (query.status === undefined || r.status === query.status));
      const from = query.before === undefined ? 0 : newest.findIndex((r) => r.id === query.before) + 1;
      if (query.before !== undefined && from === 0) return [];
      return newest.slice(from, from + (query.limit ?? 100)).map(summaryOf);
    },

    async getRun(id) {
      return structuredClone(runOf(id));
    },

    async listNeedsYou() {
      return [...runRecords].reverse().map(needsYouOf).filter((i) => i !== null);
    },

    async answer(runId, branchId) {
      const run = runOf(runId);
      const q = run.waitingFor;
      if (run.status !== "waiting" || !q) throw new ApiError("conflict", "This question was already answered.");
      const answer = q.answers.find((a) => a.id === branchId);
      if (!answer) throw new ApiError("invalid", "That isn't one of the answers to this question.");
      const entry = [...run.steps].reverse().find((s) => s.stepId === q.stepId && s.outcome === "waiting");
      if (entry) Object.assign(entry, { outcome: "done", branch: branchId, endedAt: new Date().toISOString(), message: `You answered ${answer.label}.` });
      run.waitingFor = null;
      const next = after(run.workflow, q.stepId, branchId);
      if (next) return requeue(run, next);
      Object.assign(run, { status: "done", endedAt: new Date().toISOString() });
      announce(run);
      return structuredClone(run);
    },

    async retryRun(runId) {
      const run = runOf(runId);
      if (run.status !== "failed") throw new ApiError("conflict", "Only a failed run can be retried.");
      return requeue(run, run.error?.stepId ?? null);
    },

    async resumeRun(runId) {
      const run = runOf(runId);
      if (run.status !== "interrupted") throw new ApiError("conflict", "Only a run Vela stopped by quitting can be resumed.");
      const last = run.steps[run.steps.length - 1];
      if (last?.outcome === "done") {
        const next = after(run.workflow, last.stepId, last.branch);
        if (!next) {
          Object.assign(run, { status: "done", endedAt: new Date().toISOString() });
          announce(run);
          return structuredClone(run);
        }
        return requeue(run, next);
      }
      return requeue(run, last?.stepId ?? null);
    },

    async undoRun(runId) {
      const run = runOf(runId);
      if (run.status === "undone") throw new ApiError("conflict", "This run was already undone.");
      if (!["done", "failed", "interrupted"].includes(run.status)) {
        throw new ApiError("conflict", "This run hasn't finished, so it can't be undone yet.");
      }
      // No files here: each file step that did something counts as put back.
      const fileSteps = ["rename", "move", "createFile", "addRow", "tag"];
      const report = { restored: run.steps.filter((s) => s.outcome === "done" && fileSteps.includes(s.type)).length, leftAlone: [] };
      Object.assign(run, { status: "undone", undo: report });
      announce(run);
      return { run: structuredClone(run), ...report };
    },

    async dismissRun(runId) {
      const run = runOf(runId);
      if (run.status !== "failed" && run.status !== "interrupted") {
        throw new ApiError("conflict", "Only a failed or stopped run can be dismissed.");
      }
      run.dismissed = true;
      announce(run);
    },

    onRunChanged(listener) {
      listeners.add(listener);
      return () => void listeners.delete(listener);
    },

    async listNotices() { return structuredClone(notices); },
    async markNoticesRead(ids) {
      for (const n of notices) if (!ids || ids.includes(n.id)) n.read = true;
      noticesChanged();
    },
    async clearNotices() {
      notices.splice(0);
      noticesChanged();
    },
    onNoticesChanged(listener) {
      noticeListeners.add(listener);
      return () => void noticeListeners.delete(listener);
    },
    async getActivity() { return activity(); },
    async pauseAll(pause) {
      paused = pause;
      activityChanged();
      return activity();
    },
    onActivityChanged(listener) {
      activityListeners.add(listener);
      return () => void activityListeners.delete(listener);
    },
    // There is no menu bar here to ask for a page.
    onNavigate() {
      return () => {};
    },

    async getWorkflow(id) { return stored(id); },

    async createWorkflow(templateId) {
      const wf = templateId === null ? blankWorkflow() : templateWorkflow(templateId);
      if (!wf) throw new ApiError("not_found", "Vela doesn't have that template.");
      put(wf);
      return wf;
    },

    async saveWorkflow(workflow) {
      const current = stored(workflow.id);
      if (workflow.revision !== current.revision) {
        throw new ApiError("conflict", "This workflow was changed somewhere else since you opened it.");
      }
      const problems = roughValidate(workflow);
      if (workflow.enabled && problems.length) {
        throw new ApiError("invalid", "Fix the problems before turning this workflow on.");
      }
      const saved = { ...workflow, revision: workflow.revision + 1 };
      put(saved);
      return { workflow: saved, problems };
    },

    async deleteWorkflow(id) {
      stored(id);
      const { [id]: _removed, ...rest } = readWorkflows();
      writeWorkflows(rest);
      dropDraft(id);
    },

    async getDraft(id) { return draftOf(stored(id)); },

    async saveDraft(workflow) {
      const live = stored(workflow.id);
      if (workflow.revision !== live.revision) {
        throw new ApiError("conflict", "This workflow was changed somewhere else since you opened it.");
      }
      const draft = { ...workflow, revision: live.revision, enabled: live.enabled };
      writeDrafts({ ...readDrafts(), [draft.id]: draft });
      return { workflow: draft, problems: roughValidate(draft) };
    },

    async applyDraft(id) {
      const live = stored(id);
      const draft = draftOf(live);
      if (!draft) throw new ApiError("not_found", "There are no changes waiting to be applied.");
      const problems = roughValidate(draft);
      if (live.enabled && problems.length) throw new ApiError("invalid", "Fix the problems before applying these changes.");
      const applied = { ...draft, revision: live.revision + 1, enabled: live.enabled };
      put(applied);
      dropDraft(id);
      return { workflow: applied, problems };
    },

    async discardDraft(id) {
      stored(id);
      dropDraft(id);
    },

    async validateWorkflow(workflow) { return roughValidate(workflow); },
  };
}
