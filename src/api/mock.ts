// A stand-in backend for the browser, tests and previews. Keeps settings in
// browser storage and fakes detection and key checks. The app uses tauri.ts.

import type { Api } from "./api";
import {
  ApiError, type Connection, type Model, type ModelKind, type ModelRef, type Provider, type Settings,
  type SettingsNotice, type Template, type WorkflowSummary,
} from "./types";

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
  { id: "cleanup", name: "Weekly clean-up", blurb: "Every Friday, archive Downloads files older than 30 days", trigger: "Schedule" },
];

export const SAMPLE_WORKFLOWS: WorkflowSummary[] = [
  { id: "w1", name: "Receipts and invoices", trigger: "File added · ~/Downloads", enabled: true,
    lastRun: "Scan_0042.pdf · 2 min ago", needsYou: 1, kindsNeeded: ["llm", "system1"] },
  { id: "w2", name: "Tidy screenshots", trigger: "File added · ~/Desktop", enabled: true,
    lastRun: "Screenshot 10.02 · 1 h ago", needsYou: 0, kindsNeeded: ["system1"] },
  { id: "w3", name: "Weekly clean-up", trigger: "Schedule · Fridays 17:00", enabled: false,
    lastRun: null, needsYou: 0, kindsNeeded: [] },
];

export const FRESH_SETTINGS: Settings = { setupComplete: false, openAtLogin: true, connections: [], defaults: {} };

type KeyValueStore = Pick<Storage, "getItem" | "setItem">;

export type MockOptions = {
  storage?: KeyValueStore;
  delayMs?: number;
  ollamaRunning?: boolean;
  workflows?: WorkflowSummary[];
  /** Settings already on disk before the app starts. */
  settings?: Partial<Settings>;
  /** Pretend the settings file on disk was damaged, or written by a newer version. */
  storedFile?: "damaged" | "tooNew";
};

const SETTINGS_KEY = "folderflow.settings";

function memoryStore(): KeyValueStore {
  const data = new Map<string, string>();
  return { getItem: (k) => data.get(k) ?? null, setItem: (k, v) => void data.set(k, v) };
}

/** Follows the same rules as the Rust core; see docs/api-contract.md. */
export function createMockApi(options: MockOptions = {}): Api {
  const { storage = memoryStore(), delayMs = 400, ollamaRunning = true, workflows = [] } = options;
  const wait = () => new Promise((r) => setTimeout(r, delayMs));
  let connectionCount = 0;
  // Like the Rust core, a recovery is reported by every getSettings for the rest of the session.
  const notice: SettingsNotice = options.storedFile === "damaged"
    ? { kind: "recovered", backup: "~/Library/Application Support/com.adhfmz7.folderflow/settings.damaged-1790300000-3f2a9c1e.json" }
    : null;
  if (options.settings) write({ ...FRESH_SETTINGS, ...options.settings });

  function read(): Settings {
    if (options.storedFile === "tooNew") {
      throw new ApiError("too_new", "the settings file is from a newer version of FolderFlow (format 2)");
    }
    const raw = storage.getItem(SETTINGS_KEY);
    return raw ? { ...FRESH_SETTINGS, ...JSON.parse(raw) } : FRESH_SETTINGS;
  }

  function write(settings: Settings) {
    storage.setItem(SETTINGS_KEY, JSON.stringify(settings));
  }

  const modelsFor = (connection: Connection): Model[] =>
    (MODELS[connection.providerId] ?? []).map((m) => ({ ...m, connectionId: connection.id }));

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
      if (providerId !== "ollama") return { found: false, reason: "This provider can't be detected." };
      return ollamaRunning ? { found: true } : { found: false, reason: "Ollama isn't running on this Mac." };
    },

    async connect(providerId, credentials) {
      await wait();
      const provider = PROVIDERS.find((p) => p.id === providerId);
      if (!provider) throw new ApiError("not_found", `no provider with id ${providerId}`);
      if (provider.connect === "detect" && !ollamaRunning) return { ok: false, error: "Couldn't reach Ollama." };
      if (provider.connect === "apiKey" && !(credentials.apiKey ?? "").trim()) return { ok: false, error: "Enter an API key." };
      if (provider.connect === "apiKey" && (credentials.apiKey ?? "").trim().length < 12)
        return { ok: false, error: "That key was rejected. Check it and try again." };
      if (provider.connect === "endpoint" && !/^https?:\/\//.test(credentials.endpoint ?? ""))
        return { ok: false, error: "Enter an address starting with http:// or https://." };

      // The key itself would go to the Keychain; the mock keeps nothing of it.
      const settings = read();
      const connection: Connection = { id: `${providerId}-${Date.now()}-${++connectionCount}`, providerId };
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
      return modelsFor(connection);
    },

    async listWorkflows() { return workflows; },
    async listTemplates() { return TEMPLATES; },
  };
}
