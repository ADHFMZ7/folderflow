// Workflow pieces for the mock api: templates, summaries, and a rough validator.
// The Rust core is the authority on validation; this one only covers the basics
// so previews and screen tests have something to show. See docs/workflow-format.md.

import type { ModelKindId, Problem, Step, Workflow, WorkflowSummary } from "./types";

export const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;

const TRIGGERS: Step["type"][] = ["fileAdded", "schedule", "runNow"];
const KIND_OF: Partial<Record<Step["type"], ModelKindId>> = { classify: "system1", extract: "llm", write: "llm", agent: "llm" };

const at = (row: number, col = 0) => ({ x: 300 + col * 320, y: row * 160 });

function workflow(name: string, steps: Step[]): Workflow {
  return { version: 1, id: crypto.randomUUID(), name, revision: 1, enabled: false, steps };
}

const fileAdded = (folder: string, fileTypes: string[], next: string | null, title?: string): Step => ({
  id: "s1", type: "fileAdded", title: title ?? `When a file lands in ${folder}`, position: at(0),
  folder, fileTypes, subfolders: false, next,
});

export function blankWorkflow(): Workflow {
  return workflow("New workflow", [fileAdded("~/Downloads", [], null, "When a file is added")]);
}

const TEMPLATES: Record<string, () => Workflow> = {
  receipts: () => workflow("Sort receipts", [
    fileAdded("~/Downloads", ["pdf"], "s2", "When a PDF lands in Downloads"),
    { id: "s2", type: "classify", title: "Is this a receipt?", position: at(1), instructions: "",
      categories: [{ id: "c1", label: "Receipt" }, { id: "c2", label: "Other" }], branches: { c1: "s3" } },
    { id: "s3", type: "extract", title: "Get receipt details", position: at(2), ifMissing: "review", next: "s4",
      fields: [{ name: "date", type: "date" }, { name: "vendor", type: "text" }, { name: "amount", type: "number" }] },
    { id: "s4", type: "rename", title: "Rename by date and vendor", position: at(3), template: "{date} - {vendor} - {amount}", next: "s5" },
    { id: "s5", type: "move", title: "File under Receipts", position: at(4), to: "~/Documents/Receipts/{year}", mode: "move", next: null },
  ]),
  screenshots: () => workflow("Tidy screenshots", [
    fileAdded("~/Desktop", ["png"], "s2", "When an image lands on the Desktop"),
    { id: "s2", type: "if", title: "Is it a screenshot?", position: at(1),
      condition: { left: "{file}", op: "startsWith", right: "Screenshot" }, branches: { yes: "s3" } },
    { id: "s3", type: "move", title: "Move to Screenshots", position: at(2), to: "~/Pictures/Screenshots/{year}", mode: "move", next: null },
  ]),
  summaries: () => workflow("Summarise PDFs", [
    fileAdded("~/Downloads", ["pdf"], "s2", "When a PDF lands in Downloads"),
    { id: "s2", type: "write", title: "Summarise it", position: at(1), instruction: "Summarise this document in one paragraph.", saveAs: "summary", next: "s3" },
    { id: "s3", type: "createFile", title: "Save the summary", position: at(2), name: "{file} summary.txt", contents: "{summary}", next: null },
  ]),
  invoices: () => workflow("Log invoices", [
    fileAdded("~/Downloads", ["pdf"], "s2", "When a PDF lands in Downloads"),
    { id: "s2", type: "extract", title: "Get invoice details", position: at(1), ifMissing: "review", next: "s3",
      fields: [{ name: "vendor", type: "text" }, { name: "amount", type: "number" }, { name: "due", type: "date" }] },
    { id: "s3", type: "if", title: "Over $500?", position: at(2),
      condition: { left: "{amount}", op: ">", right: "500" }, branches: { yes: "s4", no: "s5" } },
    { id: "s4", type: "askMe", title: "Check big invoices with me", position: at(3, -1), question: "Log {vendor} {amount}?",
      answers: [{ id: "a1", label: "Log it" }, { id: "a2", label: "Skip" }], branches: { a1: "s5" } },
    { id: "s5", type: "addRow", title: "Log the invoice", position: at(4), file: "~/Documents/Invoices.csv",
      columns: ["{vendor}", "{amount}", "{due}"], next: null },
  ]),
  cleanup: () => workflow("Weekly clean-up", [
    { id: "s1", type: "schedule", title: "Every Friday at 17:00", position: at(0),
      schedule: { every: "week", time: "17:00", weekday: 5 }, next: "s2" },
    { id: "s2", type: "notify", title: "Remind me", position: at(1), message: "Time to tidy Downloads.", next: null },
  ]),
};

export function templateWorkflow(id: string): Workflow | null {
  return TEMPLATES[id]?.() ?? null;
}

const DAYS = ["Sundays", "Mondays", "Tuesdays", "Wednesdays", "Thursdays", "Fridays", "Saturdays"];

function triggerText(wf: Workflow): string {
  const trigger = wf.steps.find((s) => TRIGGERS.includes(s.type));
  if (trigger?.type === "fileAdded") return `File added · ${trigger.folder}`;
  if (trigger?.type === "runNow") return "Run now";
  if (trigger?.type === "schedule") {
    const { every, time, weekday } = trigger.schedule;
    const when = every === "day" ? "Every day" : every === "weekday" ? "Weekdays" : DAYS[weekday ?? 0];
    return `Schedule · ${when} ${time}`;
  }
  return "No trigger";
}

export function summarize(wf: Workflow, run?: { needsYou: number; lastRun: string | null }): WorkflowSummary {
  const kinds = [...new Set(wf.steps.map((s) => KIND_OF[s.type]).filter((k): k is string => !!k))];
  return {
    id: wf.id, name: wf.name, trigger: triggerText(wf), enabled: wf.enabled,
    lastRun: run?.lastRun ?? null, needsYou: run?.needsYou ?? 0, kindsNeeded: kinds, status: "ok",
  };
}

export function damagedSummary(fileName: string): WorkflowSummary {
  return { id: fileName, name: fileName, trigger: "", enabled: false, lastRun: null, needsYou: 0, kindsNeeded: [], status: "damaged" };
}

/** A small part of the real rules: triggers and exits that point nowhere. */
export function roughValidate(wf: Workflow): Problem[] {
  const problems: Problem[] = [];
  const triggers = wf.steps.filter((s) => TRIGGERS.includes(s.type));
  if (!triggers.length) problems.push({ stepId: null, code: "no_trigger", message: "Add a trigger to say when this workflow runs." });
  if (triggers.length > 1) problems.push({ stepId: null, code: "many_triggers", message: "A workflow can have only one trigger." });
  const ids = new Set(wf.steps.map((s) => s.id));
  for (const s of wf.steps) {
    const exits = "next" in s ? [s.next] : "branches" in s ? Object.values(s.branches) : [];
    for (const to of exits) {
      if (to && !ids.has(to)) problems.push({ stepId: s.id, code: "missing_step", message: "This step leads to a step that no longer exists." });
    }
  }
  return problems;
}

/** Sample workflows for previews: `folderflow.mock.workflows = "sample"`. */
export function sampleWorkflows(): { workflow: Workflow; run: { needsYou: number; lastRun: string | null } }[] {
  const receipts = { ...templateWorkflow("invoices")!, name: "Receipts and invoices", enabled: true };
  const screenshots = { ...templateWorkflow("screenshots")!, enabled: true };
  return [
    { workflow: receipts, run: { needsYou: 1, lastRun: "Scan_0042.pdf · 2 min ago" } },
    { workflow: screenshots, run: { needsYou: 0, lastRun: "Screenshot 10.02 · 1 h ago" } },
    { workflow: templateWorkflow("cleanup")!, run: { needsYou: 0, lastRun: null } },
  ];
}
