// The details ({variables}) a step can use, worked out exactly as Rust's
// check_variables does (src-tauri/src/workflow/validate.rs): a detail is
// available when the trigger or a step on every path from the trigger to this
// one produces it. The editor offers these, flags the rest as the user types,
// and fills them with sample values for previews.

import type { FieldType, Step, Workflow } from "../../../api/types";
import { isTrigger } from "../graph";

export type Available = {
  name: string;
  /** How the detail is shown: "File name", or the name the user gave it. */
  label: string;
  kind: FieldType;
  /** Where it comes from: "This file", a step's title, or "Earlier steps". */
  source: string;
  /** A value to show in previews, when a fixed one fits better than one by kind. */
  sample?: string;
};

type Made = Omit<Available, "source">;

const FILE: Made[] = [
  { name: "file", label: "File name", kind: "text", sample: "Scan_0042" },
  { name: "extension", label: "Extension", kind: "text", sample: "pdf" },
  { name: "folder", label: "Folder", kind: "text", sample: "~/Downloads" },
  { name: "dateAdded", label: "Date added", kind: "date" },
  { name: "year", label: "Year", kind: "number" },
];

/** What a step makes available to the steps after it. */
export function produces(step: Step): Made[] {
  switch (step.type) {
    case "fileAdded": case "runNow": return FILE;
    case "schedule": return [{ name: "date", label: "Today's date", kind: "date" }, { name: "year", label: "Year", kind: "number" }];
    case "classify": return [{ name: "category", label: "Category", kind: "text", sample: step.categories[0]?.label || "Receipt" }];
    case "extract": return step.fields.map((f) => ({ name: f.name, label: f.name, kind: f.type }));
    case "agent": return step.outputs.map((f) => ({ name: f.name, label: f.name, kind: f.type }));
    case "write": return [{ name: step.saveAs, label: step.saveAs, kind: "text" }];
    case "rename": return [{ name: "newName", label: "New name", kind: "text", sample: "Scan_0042 renamed" }];
    case "move": return [{ name: "newFolder", label: "New folder", kind: "text", sample: "~/Documents" }];
    case "askMe": return [{ name: "answer", label: "My answer", kind: "text", sample: step.answers[0]?.label || "Yes" }];
    default: return [];
  }
}

function exitIds(step: Step): string[] {
  if ("next" in step) return step.next ? [step.next] : [];
  if ("branches" in step) return Object.values(step.branches);
  return [];
}

/** name → the steps that produce it on the paths so far. */
type Known = Map<string, Set<number>>;

/** Details available on entry to `stepId`, nearest producer first, the trigger's last. */
export function availableAt(workflow: Workflow, stepId: string): Available[] {
  const steps = workflow.steps;
  // Like Rust: the first step with an id is the one exits lead to.
  const index = new Map<string, number>();
  steps.forEach((s, i) => { if (!index.has(s.id)) index.set(s.id, i); });
  const edges = steps.map((s, i) => index.get(s.id) === i
    ? [...new Set(exitIds(s).map((id) => index.get(id)).filter((j): j is number => j !== undefined))] : []);

  const known: (Known | null)[] = steps.map(() => null);
  const depth: number[] = steps.map(() => Infinity);
  const queue: number[] = [];
  steps.forEach((s, i) => {
    if (isTrigger(s.type) && index.get(s.id) === i) { known[i] = new Map(); depth[i] = 0; queue.push(i); }
  });
  const queued = new Set(queue);
  while (queue.length) {
    const node = queue.shift()!;
    queued.delete(node);
    const after: Known = new Map([...known[node]!].map(([k, v]) => [k, new Set(v)]));
    for (const made of produces(steps[node])) after.set(made.name, new Set([node]));
    for (const next of edges[node]) {
      depth[next] = Math.min(depth[next], depth[node] + 1);
      let changed = false;
      const cur = known[next];
      if (!cur) {
        known[next] = new Map([...after].map(([k, v]) => [k, new Set(v)]));
        changed = true;
      } else {
        for (const [name, from] of [...cur]) {
          const other = after.get(name);
          if (!other) { cur.delete(name); changed = true; continue; }
          for (const p of other) if (!from.has(p)) { from.add(p); changed = true; }
        }
      }
      if (changed && !queued.has(next)) { queued.add(next); queue.push(next); }
    }
  }

  const at = index.get(stepId);
  const here = at === undefined ? null : known[at];
  if (!here) return [];
  const out: (Available & { order: number })[] = [];
  for (const [name, from] of here) {
    const producers = [...from];
    const first = producers[0];
    const made = produces(steps[first]).find((m) => m.name === name)!;
    const trigger = producers.every((p) => isTrigger(steps[p].type));
    const source = trigger ? "This file" : producers.length > 1 ? "Earlier steps" : steps[first].title;
    const order = trigger ? -1 : Math.max(...producers.map((p) => depth[p]));
    out.push({ ...made, source: source || "An earlier step", order });
  }
  // Nearest first; within one step, in the order it lists them.
  const fieldOrder = (v: Available) => {
    for (const p of here.get(v.name)!) return produces(steps[p]).findIndex((m) => m.name === v.name);
    return 0;
  };
  out.sort((a, b) => b.order - a.order || fieldOrder(a) - fieldOrder(b));
  return out.map(({ order: _, ...v }) => v);
}

const NAME = /^[A-Za-z0-9_]{1,32}$/;

/** Each `{name}` in `text`, as Rust's variables_in reads them: other braces are plain text. */
export function variablesIn(text: string): string[] {
  const out: string[] = [];
  let rest = text;
  for (;;) {
    const open = rest.indexOf("{");
    if (open < 0) break;
    const after = rest.slice(open + 1);
    const close = after.search(/[{}]/);
    if (close < 0) break;
    if (after[close] === "}") {
      const name = after.slice(0, close);
      if (NAME.test(name)) out.push(name);
      rest = after.slice(close + 1);
    } else {
      rest = after.slice(close);
    }
  }
  return out;
}

/** The `{names}` in `text` that aren't available, each once. */
export function unknownIn(text: string, available: Available[]): string[] {
  const known = new Set(available.map((v) => v.name));
  return [...new Set(variablesIn(text).filter((n) => !known.has(n)))];
}

const iso = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;

export function sampleOf(v: Available, today = new Date()): string {
  if (v.sample) return v.sample;
  if (v.name === "year") return String(today.getFullYear());
  switch (v.kind) {
    case "number": return "42.50";
    case "date": return iso(today);
    case "yesNo": return "yes";
    default: return v.label;
  }
}

/** `text` with each available detail replaced by a sample value; unknown ones stay as typed. */
export function fillSample(text: string, available: Available[], today = new Date()): string {
  const byName = new Map(available.map((v) => [v.name, v]));
  return text.replace(/\{([A-Za-z0-9_]{1,32})\}/g, (whole, name: string) => {
    const v = byName.get(name);
    return v ? sampleOf(v, today) : whole;
  });
}
