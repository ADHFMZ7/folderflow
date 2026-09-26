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

export type ApiErrorCode = "too_new" | "not_found" | "invalid" | "keychain" | "provider" | "io" | "conflict";

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
  /** "damaged": the file can't be read and is kept as is. "tooNew": written by a newer FolderFlow. */
  status: "ok" | "damaged" | "tooNew";
};

// ---- Workflows: see docs/workflow-format.md -------------------------------

export type Position = { x: number; y: number };

/** A branch of a classify or askMe step. The id never changes; the label is display text. */
export type Branch = { id: string; label: string };

export type FieldType = "text" | "number" | "date" | "yesNo";
export type Field = { name: string; type: FieldType };

export type Schedule = { every: "day" | "weekday" | "week"; time: string; weekday?: number };

export type ConditionOp = ">" | "<" | ">=" | "<=" | "=" | "!=" | "contains" | "startsWith";
export type Condition = { left: string; op: ConditionOp; right: string };

type StepBase = { id: string; title: string; position: Position };
/** Where a plain step goes next; null ends the run. */
type Next = { next: string | null };
/** Branch id → step id. A branch with no entry ends the run. */
type Branches = { branches: Record<string, string> };

export type Step = StepBase & (
  | ({ type: "fileAdded"; folder: string; fileTypes: string[]; subfolders: boolean } & Next)
  | ({ type: "schedule"; schedule: Schedule } & Next)
  | ({ type: "runNow" } & Next)
  | ({ type: "classify"; categories: Branch[]; instructions: string } & Branches)
  | ({ type: "extract"; fields: Field[]; ifMissing: "review" | "fail" } & Next)
  | ({ type: "write"; instruction: string; saveAs: string } & Next)
  | ({ type: "agent"; instruction: string; abilities: string[]; outputs: Field[] } & Next)
  | ({ type: "rename"; template: string } & Next)
  | ({ type: "move"; to: string; mode: "move" | "copy" } & Next)
  | ({ type: "createFile"; name: string; contents: string } & Next)
  | ({ type: "tag"; tags: string[] } & Next)
  | ({ type: "addRow"; file: string; columns: string[] } & Next)
  | ({ type: "notify"; message: string } & Next)
  | ({ type: "if"; condition: Condition } & Branches)
  | { type: "stop" }
  | ({ type: "askMe"; question: string; answers: Branch[] } & Branches)
);

export type StepType = Step["type"];

export type Workflow = {
  version: number;
  /** A UUID; it becomes the file name. */
  id: string;
  name: string;
  /** Goes up by one on every save. A save based on an older revision fails with "conflict". */
  revision: number;
  enabled: boolean;
  steps: Step[];
};

export type ProblemCode =
  | "no_trigger" | "many_triggers" | "duplicate_id" | "missing_step" | "unknown_branch" | "loop"
  | "unreachable" | "required" | "unknown_variable" | "no_model" | "invalid_value";

/** Something that stops a workflow from being turned on. stepId is null for the whole workflow. */
export type Problem = { stepId: string | null; code: ProblemCode; message: string };

export type SaveResult = { workflow: Workflow; problems: Problem[] };

export type Template = { id: string; name: string; blurb: string; trigger: string };
