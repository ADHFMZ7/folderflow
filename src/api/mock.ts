// A stand-in backend that keeps settings in browser storage and fakes detection
// and key checks. Replaced by the Tauri implementation in step 2.

import type { Api } from "./api";
import type {
  Connection, Model, ModelKind, Provider, Settings, Template, WorkflowSummary,
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
};

const SETTINGS_KEY = "folderflow.settings";

function memoryStore(): KeyValueStore {
  const data = new Map<string, string>();
  return { getItem: (k) => data.get(k) ?? null, setItem: (k, v) => void data.set(k, v) };
}

export function createMockApi(options: MockOptions = {}): Api {
  const { storage = memoryStore(), delayMs = 400, ollamaRunning = true, workflows = [] } = options;
  const wait = () => new Promise((r) => setTimeout(r, delayMs));
  let connectionCount = 0;

  const readSettings = (): Settings => {
    const raw = storage.getItem(SETTINGS_KEY);
    return raw ? { ...FRESH_SETTINGS, ...JSON.parse(raw) } : FRESH_SETTINGS;
  };

  const newConnection = (providerId: string): Connection =>
    ({ id: `${providerId}-${Date.now()}-${++connectionCount}`, providerId });

  return {
    async getSettings() { return readSettings(); },
    async saveSettings(settings) { storage.setItem(SETTINGS_KEY, JSON.stringify(settings)); },

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
      if (!provider) return { ok: false, error: "Unknown provider." };
      if (provider.connect === "detect" && !ollamaRunning) return { ok: false, error: "Ollama isn't running on this Mac." };
      if (provider.connect === "apiKey" && (credentials.apiKey ?? "").trim().length < 12)
        return { ok: false, error: "That key was rejected. Check it and try again." };
      if (provider.connect === "endpoint" && !/^https?:\/\//.test(credentials.endpoint ?? ""))
        return { ok: false, error: "Enter an address starting with http:// or https://." };
      return { ok: true, connection: newConnection(providerId) };
    },

    async listModels(connectionId) {
      const connection = readSettings().connections.find((c) => c.id === connectionId);
      const providerId = connection?.providerId ?? connectionId.split("-")[0];
      return (MODELS[providerId] ?? []).map((m): Model => ({ ...m, connectionId }));
    },

    async listWorkflows() { return workflows; },
    async listTemplates() { return TEMPLATES; },
  };
}
