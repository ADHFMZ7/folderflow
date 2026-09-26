// The mock's engine: runs a workflow's steps the way the Rust core does
// (src-tauri/src/engine), for the steps the core can run so far.

import type { ConditionOp, Run, RunValue, Step, StepRun, Workflow } from "./types";

const NAMES: Record<Step["type"], string> = {
  fileAdded: "File added", schedule: "Schedule", runNow: "Run now", classify: "Classify", extract: "Extract",
  write: "Write", agent: "Agent", rename: "Rename", move: "Move", createFile: "Create file", tag: "Tag",
  addRow: "Add row", notify: "Notify", if: "If", stop: "Stop", askMe: "Ask me",
};

const text = (value: string): RunValue => ({ kind: "text", value });

/** `{file}`, `{extension}`, `{folder}`, `{dateAdded}` and `{year}` for a `~/` path. */
export function fileValues(path: string, added: Date): Record<string, RunValue> {
  const slash = path.lastIndexOf("/");
  const name = path.slice(slash + 1);
  const dot = name.lastIndexOf(".");
  const date = `${added.getFullYear()}-${String(added.getMonth() + 1).padStart(2, "0")}-${String(added.getDate()).padStart(2, "0")}`;
  return {
    file: text(dot > 0 ? name.slice(0, dot) : name),
    extension: text(dot > 0 ? name.slice(dot + 1) : ""),
    folder: text(slash > 0 ? path.slice(0, slash) : path.startsWith("/") ? "/" : "~"),
    dateAdded: { kind: "date", value: date },
    year: { kind: "number", value: String(added.getFullYear()) },
  };
}

export function fill(template: string, values: Record<string, RunValue>): string {
  return template.replace(/\{([A-Za-z0-9_]{1,32})\}/g, (whole, name: string) => values[name]?.value ?? whole);
}

const NUMBER = /^-?(\d+\.?\d*|\.\d+)$/;
const DATE = /^\d{4}-\d{2}-\d{2}$/;

/** Numbers as numbers, YYYY-MM-DD as dates, anything else as text ignoring case and outer spaces. */
export function holds(left: string, op: ConditionOp, right: string): boolean {
  const l = left.trim(), r = right.trim();
  const ll = l.toLowerCase(), rl = r.toLowerCase();
  if (op === "contains") return ll.includes(rl);
  if (op === "startsWith") return ll.startsWith(rl);
  const order = NUMBER.test(l) && NUMBER.test(r) ? Math.sign(Number(l) - Number(r))
    : DATE.test(l) && DATE.test(r) ? Math.sign(l.localeCompare(r))
    : ll < rl ? -1 : ll > rl ? 1 : 0;
  switch (op) {
    case "=": return order === 0;
    case "!=": return order !== 0;
    case ">": return order > 0;
    case "<": return order < 0;
    case ">=": return order >= 0;
    case "<=": return order <= 0;
  }
}

/** Runs `run` to the end, changing it in place. */
export function execute(run: Run, workflow: Workflow, notify: (title: string, body: string) => void) {
  const now = () => new Date().toISOString();
  let current = workflow.steps.find((s) => s.type === "fileAdded" || s.type === "runNow")?.id ?? null;
  while (current) {
    const step = workflow.steps.find((s) => s.id === current);
    if (!step) break;
    const entry: StepRun = {
      stepId: step.id, title: step.title, type: step.type, startedAt: now(), endedAt: null,
      outcome: "done", branch: null, values: {}, message: null,
    };
    run.steps.push(entry);
    current = null;
    switch (step.type) {
      case "fileAdded":
      case "runNow":
        entry.values = fileValues(run.trigger.file!.path, new Date(run.startedAt));
        current = step.next;
        break;
      case "if": {
        const { left, op, right } = step.condition;
        entry.branch = holds(fill(left, run.values), op, fill(right, run.values)) ? "yes" : "no";
        current = step.branches[entry.branch] ?? null;
        break;
      }
      case "notify":
        notify(workflow.name, fill(step.message, run.values));
        current = step.next;
        break;
      case "stop":
        break;
      default: {
        const message = `${NAMES[step.type]} steps can't run in this version of FolderFlow yet.`;
        entry.outcome = "failed";
        entry.message = message;
        entry.endedAt = now();
        run.error = { stepId: step.id, message };
        run.status = "failed";
        run.endedAt = now();
        return;
      }
    }
    entry.endedAt = now();
    Object.assign(run.values, entry.values);
  }
  run.status = "done";
  run.endedAt = now();
}
