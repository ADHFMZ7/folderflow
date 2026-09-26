// Titles that follow a step's settings, until the user writes their own. No
// format change: a title counts as untouched while it still equals the title
// its settings (or a fresh step of its type) would give.

import type { ConditionOp, Step, StepType } from "../../../api/types";
import { defaultStep } from "../graph";

const DAYS = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

/** "a", "a and b", "a, b and c". */
export function listing(items: string[], word = "and"): string {
  if (items.length <= 1) return items[0] ?? "";
  return `${items.slice(0, -1).join(", ")} ${word} ${items[items.length - 1]}`;
}

/** dueDate or due_date → "due date". */
export const humanize = (name: string) => name.replace(/_/g, " ").replace(/([a-z0-9])([A-Z])/g, "$1 $2").toLowerCase().trim();

const segments = (path: string) => path.split("/").filter((s) => s && s !== "~");
const lastOf = (path: string, n: number) => segments(path).slice(-n).join("/");

const OP_TITLE: Record<ConditionOp, (l: string, r: string) => string> = {
  ">": (l, r) => `Is ${l} more than ${r}?`,
  "<": (l, r) => `Is ${l} less than ${r}?`,
  ">=": (l, r) => `Is ${l} at least ${r}?`,
  "<=": (l, r) => `Is ${l} at most ${r}?`,
  "=": (l, r) => `Is ${l} ${r}?`,
  "!=": (l, r) => `Isn't ${l} ${r}?`,
  contains: (l, r) => `Does ${l} contain ${r}?`,
  startsWith: (l, r) => `Does ${l} start with ${r}?`,
};

const defaultTitle = (type: StepType) => defaultStep(type, "x", { x: 0, y: 0 }).title;

/** What the settings say, or null for the empty parts. */
function fromSettings(step: Step): string | null | undefined {
  switch (step.type) {
    case "fileAdded": {
      const types = step.fileTypes.filter(Boolean).map((t) => t.toUpperCase());
      const what = types.length === 0 || types.length > 2 ? "file" : listing(types, "or");
      const folder = step.folder.trim() === "~" ? "your home folder" : lastOf(step.folder, 1);
      if (!folder) return null;
      return `When a${/^[AEFILMNORSX]/.test(what) && what !== "file" ? "n" : ""} ${what} is added to ${folder}`;
    }
    case "schedule": {
      const { every, time, weekday } = step.schedule;
      if (!time) return null;
      const when = every === "day" ? "day" : every === "weekday" ? "weekday" : DAYS[weekday ?? 0];
      return `Every ${when} at ${time}`;
    }
    case "classify": {
      const labels = step.categories.map((c) => c.label.trim()).filter(Boolean);
      return labels.length < 2 ? null : `${listing(labels, "or")}?`;
    }
    case "extract": {
      const names = step.fields.map((f) => humanize(f.name)).filter(Boolean);
      return names.length ? `Pull out the ${listing(names)}` : null;
    }
    case "write": return step.saveAs.trim() ? `Write the ${humanize(step.saveAs)}` : null;
    case "rename": return step.template.trim() ? `Rename to ${step.template.trim()}` : null;
    case "move": {
      const to = lastOf(step.to, 2);
      return to ? `${step.mode === "copy" ? "Copy" : "Move"} to ${to}` : null;
    }
    case "createFile": return step.name.trim() ? `Create ${step.name.trim()}` : null;
    case "tag": {
      const tags = step.tags.map((t) => t.trim()).filter(Boolean);
      return tags.length ? `Tag it ${listing(tags)}` : null;
    }
    case "addRow": return lastOf(step.file, 1) ? `Add a row to ${lastOf(step.file, 1)}` : null;
    case "if": {
      const { left, op, right } = step.condition;
      return left.trim() ? OP_TITLE[op](left.trim(), right.trim()) : null;
    }
    default: return undefined;
  }
}

/** The title a step's settings give it, the fresh step's title while they're empty, or null for steps it doesn't title. */
export function autoTitle(step: Step): string | null {
  const title = fromSettings(step);
  if (title === undefined) return null;
  return title ?? defaultTitle(step.type);
}

/** `next` with its title following its settings, if the user hasn't written one. */
export function retitle<S extends Step>(prev: S, next: S): S {
  if (next.title !== prev.title) return next;
  const untouched = prev.title === autoTitle(prev) || prev.title === defaultTitle(prev.type);
  const title = untouched ? autoTitle(next) : null;
  return title && title !== next.title ? { ...next, title } : next;
}
