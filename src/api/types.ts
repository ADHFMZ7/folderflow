// The vocabulary shared by the screens and whatever backs the api.

/** A category of model, such as "llm" or "system1". Open-ended: new kinds are catalog entries. */
export type ModelKindId = string;

export type ModelKind = {
  id: ModelKindId;
  name: string;
  description: string;
  /** Step types that need a model of this kind, e.g. ["Classify"]. */
  usedBy: string[];
};

/** How a provider is connected: found on this Mac, an API key, or a server address. */
export type ConnectMethod = "detect" | "apiKey" | "endpoint";

export type Provider = {
  id: string;
  name: string;
  location: "local" | "cloud";
  connect: ConnectMethod;
  kinds: ModelKindId[];
  /** One sentence on where files go when this provider runs a step. */
  privacy: string;
  /** Where to download a local provider. */
  helpUrl?: string;
  /** Where to create an API key, for providers connected by "apiKey". */
  keyUrl?: string;
};

export type Credentials = { apiKey?: string; endpoint?: string };

/** A provider the user has set up. Secrets stay in the backend, never here. */
export type Connection = {
  id: string;
  providerId: string;
  /** The server address, for providers connected by address. */
  endpoint?: string;
};

export type Model = { id: string; connectionId: string; name: string; kind: ModelKindId };

export type ModelRef = { connectionId: string; modelId: string };

export type DetectResult = { found: true } | { found: false; reason: string };

/** `ok: false` is an expected refusal (key rejected, provider unreachable, bad input). */
export type ConnectOutcome =
  | { ok: true; connection: Connection; settings: Settings }
  | { ok: false; error: string };

export type Settings = {
  setupComplete: boolean;
  openAtLogin: boolean;
  connections: Connection[];
  /** The model each kind uses unless a step overrides it. */
  defaults: Record<ModelKindId, ModelRef | null>;
};

/** Why the settings aren't simply what was saved last time. */
export type SettingsNotice = { kind: "recovered"; backup: string } | null;

export type LoadedSettings = { settings: Settings; notice: SettingsNotice };

/** The parts of Settings the front end may change. Connections change only through connect and remove. */
export type SettingsChange = Partial<Pick<Settings, "setupComplete" | "openAtLogin" | "defaults">>;

export type ApiErrorCode = "too_new" | "not_found" | "invalid" | "keychain" | "provider" | "io";

/** What a failed api call throws. Messages never contain a key. */
export class ApiError extends Error {
  constructor(readonly code: ApiErrorCode, message: string) {
    super(message);
    this.name = "ApiError";
  }
}

export type WorkflowSummary = {
  id: string;
  name: string;
  trigger: string;
  enabled: boolean;
  lastRun: string | null;
  needsYou: number;
  kindsNeeded: ModelKindId[];
};

export type Template = { id: string; name: string; blurb: string; trigger: string };
