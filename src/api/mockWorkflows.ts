// Workflow pieces for the mock api: templates, summaries, and a rough validator.
// The Rust core is the authority on validation; this one only covers the basics
// so previews and screen tests have something to show. See docs/workflow-format.md.

import type { Field, ModelKindId, Problem, Step, Workflow, WorkflowSummary } from "./types";

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
  paperwork,
};

/** The showcase, as in src-tauri/src/workflow/templates.rs. Columns: receipts, invoices, questions, contracts, the rest. */
function paperwork(): Workflow {
  const d = (name: string, type: Field["type"], description: string): Field => ({ name, type, description });
  return workflow("Paperwork inbox", [
    { ...fileAdded("~/Downloads", ["pdf", "png", "jpg", "jpeg", "heic"], "sort", "When a PDF or photo is added to Downloads"), position: at(0, 0) },
    { id: "sort", type: "classify", title: "What kind of paperwork is this?", position: at(1, 0), instructions: "",
      categories: [
        { id: "c1", label: "Receipt", description: "Proof of a payment I made: shop receipts, order confirmations, card slips" },
        { id: "c2", label: "Invoice", description: "A bill asking me to pay, with an amount due and usually a due date" },
        { id: "c3", label: "Contract", description: "An agreement to sign or already signed: leases, service contracts, terms of employment" },
        { id: "c4", label: "Something else", description: "Anything that isn't one of the above" },
      ],
      branches: { c1: "r1", c2: "i1", c3: "k1", c4: "o1" } },

    { id: "r1", type: "extract", title: "Pull out the date, shop and total", position: at(2, -2), ifMissing: "review", next: "r2",
      fields: [d("date", "date", "The date I paid"), d("vendor", "text", "The shop or company I paid"), d("amount", "number", "The total I paid, including tax")] },
    { id: "r2", type: "rename", title: "Rename it by date, shop and total", position: at(3, -2), template: "{date} {vendor} {amount}", next: "r3" },
    { id: "r3", type: "move", title: "File it under Receipts for the year", position: at(4, -2), to: "~/Documents/Paperwork/Receipts/{year}", mode: "move", next: "r4" },
    { id: "r4", type: "addRow", title: "Log it in Expenses", position: at(5, -2), file: "~/Documents/Paperwork/Expenses.csv",
      columns: ["{date}", "{vendor}", "{amount}", "{newName}"], headers: ["Date", "Vendor", "Amount", "File"], next: null },

    { id: "i1", type: "extract", title: "Pull out who it's from, the amount and the due date", position: at(2, -1), ifMissing: "review", next: "i2",
      fields: [d("vendor", "text", "The company asking to be paid"), d("amount", "number", "The total due"),
        d("due", "date", "The date payment is due"), d("number", "text", "The invoice number")] },
    { id: "i2", type: "if", title: "Is it more than 1000?", position: at(3, -1),
      condition: { left: "{amount}", op: ">", right: "1000" }, branches: { yes: "i3", no: "i5" } },
    { id: "i3", type: "askMe", title: "Ask me before filing a big invoice", position: at(4, 0),
      question: "Invoice from {vendor} for {amount}, due {due}. File it?",
      answers: [{ id: "a1", label: "File it" }, { id: "a2", label: "Leave it in Downloads" }], branches: { a1: "i5", a2: "i4" } },
    { id: "i4", type: "stop", title: "Leave it where it is", position: at(5, 0) },
    { id: "i5", type: "rename", title: "Rename it by company and invoice number", position: at(5, -1), template: "{vendor} invoice {number}", next: "i6" },
    { id: "i6", type: "move", title: "File it under Invoices for the year", position: at(6, -1), to: "~/Documents/Paperwork/Invoices/{year}", mode: "move", next: "i7" },
    { id: "i7", type: "notify", title: "Tell me when it's due", position: at(7, -1), message: "Filed the {vendor} invoice for {amount}. It's due {due}.", next: null },

    { id: "k1", type: "agent", title: "Read the contract", position: at(2, 1), abilities: ["readFile"], next: "k2",
      instruction: "Read this contract and note who it's with, when it ends, whether it renews by itself, and anything unusual someone who isn't a lawyer should know.",
      outputs: [d("party", "text", "Who the contract is with"), d("endDate", "date", "When it ends"),
        d("autoRenews", "yesNo", "Whether it renews by itself"), d("watchOut", "text", "Anything unusual, in one sentence")] },
    { id: "k2", type: "write", title: "Write a plain-English summary", position: at(3, 1), saveAs: "summary", next: "k3",
      instruction: "A short plain-English summary of this contract for my records: who it's with, what I agreed to, when it ends, and this: {watchOut}. Use bullet points." },
    { id: "k3", type: "rename", title: "Rename it after who it's with", position: at(4, 1), template: "{party} contract", next: "k4" },
    { id: "k4", type: "move", title: "File it under Contracts", position: at(5, 1), to: "~/Documents/Paperwork/Contracts", mode: "move", next: "k5" },
    { id: "k5", type: "createFile", title: "Save the summary next to it", position: at(6, 1),
      name: "{party} contract summary.md", contents: "{summary}", folder: "{newFolder}", next: "k6" },
    { id: "k6", type: "if", title: "Does it renew by itself?", position: at(7, 1),
      condition: { left: "{autoRenews}", op: "=", right: "yes" }, branches: { yes: "k7" } },
    { id: "k7", type: "notify", title: "Warn me about the renewal", position: at(8, 1),
      message: "{party} renews by itself on {endDate}. Cancel before then if you don't want that.", next: null },

    { id: "o1", type: "tag", title: "Tag it To sort", position: at(2, 2), tags: ["To sort"], next: "o2" },
    { id: "o2", type: "notify", title: "Tell me I need to sort it", position: at(3, 2), message: "I couldn't sort {file}. It's in Downloads, tagged To sort.", next: null },
  ]);
}

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
    lastRun: run?.lastRun ?? null, needsYou: run?.needsYou ?? 0, kindsNeeded: kinds, status: "ok", hasDraft: false,
  };
}

export function damagedSummary(fileName: string): WorkflowSummary {
  return { id: fileName, name: fileName, trigger: "", enabled: false, lastRun: null, needsYou: 0, kindsNeeded: [], status: "damaged", hasDraft: false };
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
