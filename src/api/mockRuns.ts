// The mock's engine: runs a workflow's steps the way the Rust core does
// (src-tauri/src/engine), for the steps the core can run so far. File steps
// change nothing, since there are no files; they record what they would do.

import type { ConditionOp, NeedsYouItem, Run, RunSummary, RunValue, Step, StepRun, Workflow } from "./types";

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

/** A value fit for a file name, as the core makes it: / and : become -, leading dots and spaces go. */
export function safe(value: string): string {
  return value.replace(/[/:]/g, "-").replace(/[\u0000-\u001f]/g, " ").replace(/^[. ]+/, "").trimEnd().slice(0, 200).trimEnd();
}

/** Like fill, for names and paths; throws the core's message for a value that ends up empty. */
export function fillName(template: string, values: Record<string, RunValue>): string {
  return template.replace(/\{([A-Za-z0-9_]{1,32})\}/g, (whole, name: string) => {
    if (!(name in values)) return whole;
    const value = safe(values[name].value);
    if (!value) throw new Error(`{${name}} is empty, so it can't be used in a file or folder name.`);
    return value;
  });
}

const nameOf = (path: string) => path.slice(path.lastIndexOf("/") + 1);
const folderOf = (path: string) => path.slice(0, Math.max(path.lastIndexOf("/"), 1));
const stemOf = (name: string) => (name.lastIndexOf(".") > 0 ? name.slice(0, name.lastIndexOf(".")) : name);
const extOf = (name: string) => (name.lastIndexOf(".") > 0 ? name.slice(name.lastIndexOf(".")) : "");
const andList = (items: string[]) => (items.length < 2 ? items.join("") : `${items.slice(0, -1).join(", ")} and ${items[items.length - 1]}`);

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

export const summaryOf = (run: Run): RunSummary => ({
  id: run.id, workflowId: run.workflowId, workflowName: run.workflow.name, status: run.status,
  file: run.trigger.file?.path.split("/").pop() ?? null, startedAt: run.startedAt, endedAt: run.endedAt,
  error: run.error?.message ?? null,
});

/** What the run waits on the person for, if anything, as the core words it. */
export function needsYouOf(run: Run): NeedsYouItem | null {
  const titleOf = (id: string | null | undefined) => [...run.steps].reverse().find((s) => s.stepId === id)?.title ?? null;
  if (run.status === "waiting" && run.waitingFor) {
    const q = run.waitingFor;
    return { kind: "question", run: summaryOf(run), step: titleOf(q.stepId), message: q.question, answers: q.answers };
  }
  if (run.dismissed) return null;
  if (run.status === "failed") {
    return { kind: "failed", run: summaryOf(run), step: titleOf(run.error?.stepId), message: run.error?.message ?? "The run failed.", answers: [] };
  }
  if (run.status === "interrupted") {
    const at = run.steps[run.steps.length - 1]?.title ?? null;
    const message = at ? `Stopped at ${at} when FolderFlow quit.` : "FolderFlow quit before this run started.";
    return { kind: "interrupted", run: summaryOf(run), step: at, message, answers: [] };
  }
  return null;
}

/** The step after `stepId`, given the branch it took. */
export function after(workflow: Workflow, stepId: string, branch: string | null): string | null {
  const step = workflow.steps.find((s) => s.id === stepId);
  if (!step || step.type === "stop") return null;
  if ("next" in step) return step.next;
  return branch ? step.branches[branch] ?? null : null;
}

/** Runs `run` to its end or its next question, changing it in place. It starts
    at `continueAt` when a run carries on, otherwise at the trigger. */
export function execute(run: Run, workflow: Workflow, notify: (title: string, body: string) => void) {
  const now = () => new Date().toISOString();
  let current = run.continueAt ?? workflow.steps.find((s) => s.type === "fileAdded" || s.type === "runNow")?.id ?? null;
  run.continueAt = null;
  while (current) {
    const step = workflow.steps.find((s) => s.id === current);
    if (!step) break;
    const entry: StepRun = {
      stepId: step.id, title: step.title, type: step.type, startedAt: now(), endedAt: null,
      outcome: "done", branch: null, values: {}, message: null,
    };
    run.steps.push(entry);
    current = null;
    const file = run.file?.path ?? "";
    try {
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
      case "askMe": {
        const question = fill(step.question, run.values);
        entry.outcome = "waiting";
        run.waitingFor = { stepId: step.id, question, answers: step.answers };
        run.status = "waiting";
        notify(workflow.name, question);
        return;
      }
      case "rename": {
        const to = `${folderOf(file)}/${fillName(step.template, run.values)}${extOf(nameOf(file))}`;
        entry.message = `Renamed ${nameOf(file)} to ${nameOf(to)}.`;
        entry.values = { newName: text(stemOf(nameOf(to))) };
        run.file = { path: to, inode: run.file!.inode };
        current = step.next;
        break;
      }
      case "move": {
        const folder = fillName(step.to, run.values);
        entry.message = `${step.mode === "move" ? "Moved" : "Copied"} ${nameOf(file)} to ${folder}.`;
        entry.values = { newFolder: text(folder) };
        if (step.mode === "move") run.file = { path: `${folder}/${nameOf(file)}`, inode: run.file!.inode };
        current = step.next;
        break;
      }
      case "createFile": {
        const folder = step.folder?.trim() ? fillName(step.folder, run.values) : folderOf(file);
        entry.message = `Created ${fillName(step.name, run.values)} in ${folder}.`;
        current = step.next;
        break;
      }
      case "addRow":
        entry.message = `Added a row to ${nameOf(fillName(step.file, run.values))}.`;
        current = step.next;
        break;
      case "tag":
        entry.message = `Tagged ${nameOf(file)} ${andList(step.tags.map((t) => fill(t, run.values)))}.`;
        current = step.next;
        break;
      default:
        throw new Error(`${NAMES[step.type]} steps can't run in this version of FolderFlow yet.`);
    }
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      entry.outcome = "failed";
      entry.message = message;
      entry.endedAt = now();
      run.error = { stepId: step.id, message };
      run.status = "failed";
      run.endedAt = now();
      return;
    }
    entry.endedAt = now();
    Object.assign(run.values, entry.values);
  }
  run.status = "done";
  run.endedAt = now();
}
