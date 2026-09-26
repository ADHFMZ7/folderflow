// Between a workflow and the canvas: turning steps into nodes and edges, and
// applying the user's edits back. Pure functions, so every edit is testable and
// no edit can quietly lose part of a workflow. See docs/workflow-format.md.

import type { Edge, Node, XYPosition } from "@xyflow/react";
import type { Branch, Position, Problem, Step, StepType, Workflow } from "../../api/types";

export const TRIGGERS: StepType[] = ["fileAdded", "schedule", "runNow"];
export const isTrigger = (type: StepType) => TRIGGERS.includes(type);

export type StepNodeData = { step: Step; problems: Problem[] };
export type StepNode = Node<StepNodeData, "step">;

/** One exit of a step: `handle` is the branch id, or null for a plain step's `next`. */
export type Exit = { handle: string | null; label: string | null; target: string | null };

const IF_BRANCHES: Branch[] = [{ id: "yes", label: "Yes" }, { id: "no", label: "No" }];

/** The branches a branching step offers, in display order. */
export function branchesOf(step: Step): Branch[] | null {
  switch (step.type) {
    case "classify": return step.categories;
    case "askMe": return step.answers;
    case "if": return IF_BRANCHES;
    default: return null;
  }
}

export function exitsOf(step: Step): Exit[] {
  const branches = branchesOf(step);
  if (branches && "branches" in step) {
    return branches.map((b) => ({ handle: b.id, label: b.label, target: step.branches[b.id] ?? null }));
  }
  if ("next" in step) return [{ handle: null, label: null, target: step.next }];
  return [];
}

export const edgeId = (from: string, handle: string | null) => `${from}:${handle ?? "next"}`;

export function toFlow(workflow: Workflow, problems: Problem[]): { nodes: StepNode[]; edges: Edge[] } {
  const ids = new Set(workflow.steps.map((s) => s.id));
  const nodes: StepNode[] = workflow.steps.map((step) => ({
    id: step.id,
    type: "step",
    position: step.position,
    data: { step, problems: problems.filter((p) => p.stepId === step.id) },
  }));
  const edges: Edge[] = workflow.steps.flatMap((step) =>
    exitsOf(step)
      .filter((exit) => exit.target && ids.has(exit.target))
      .map((exit) => ({
        id: edgeId(step.id, exit.handle),
        source: step.id,
        sourceHandle: exit.handle,
        target: exit.target!,
        ...(exit.label ? { label: exit.label } : {}),
      })),
  );
  return { nodes, edges };
}

function replaceStep(workflow: Workflow, id: string, change: (step: Step) => Step): Workflow {
  return { ...workflow, steps: workflow.steps.map((s) => (s.id === id ? change(s) : s)) };
}

/** Sets where one exit leads. Returns `step` itself if it has no such exit. */
function withExit(step: Step, handle: string | null, target: string | null): Step {
  if (handle === null) return "next" in step ? { ...step, next: target } as Step : step;
  if (!("branches" in step) || !branchesOf(step)?.some((b) => b.id === handle)) return step;
  const branches = { ...step.branches };
  if (target) branches[handle] = target;
  else delete branches[handle];
  return { ...step, branches } as Step;
}

/** Points one exit at a step. Refuses links into a trigger, to or from unknown steps, and from unknown exits. */
export function connect(workflow: Workflow, from: string, handle: string | null, to: string): Workflow {
  const source = workflow.steps.find((s) => s.id === from);
  const target = workflow.steps.find((s) => s.id === to);
  if (!source || !target || isTrigger(target.type)) return workflow;
  const changed = withExit(source, handle, to);
  return changed === source ? workflow : replaceStep(workflow, from, () => changed);
}

export function disconnect(workflow: Workflow, from: string, handle: string | null): Workflow {
  return replaceStep(workflow, from, (s) => withExit(s, handle, null));
}

/** Removes steps, and clears every exit that led to them. */
export function removeSteps(workflow: Workflow, ids: string[]): Workflow {
  const gone = new Set(ids);
  const steps = workflow.steps
    .filter((s) => !gone.has(s.id))
    .map((s) => exitsOf(s).reduce((step, exit) => (exit.target && gone.has(exit.target) ? withExit(step, exit.handle, null) : step), s));
  return { ...workflow, steps };
}

export function moveStep(workflow: Workflow, id: string, position: XYPosition): Workflow {
  // Whole pixels keep workflow files tidy; dragging gives fractions.
  return replaceStep(workflow, id, (s) => ({ ...s, position: { x: Math.round(position.x), y: Math.round(position.y) } }));
}

/**
 * Replaces a step with an edited version of itself. The id and type can't change
 * this way, and links from branches that no longer exist are dropped.
 */
export function updateStep(workflow: Workflow, edited: Step): Workflow {
  const current = workflow.steps.find((s) => s.id === edited.id);
  if (!current || current.type !== edited.type) return workflow;
  let next = edited;
  if ("branches" in edited) {
    const ids = new Set(branchesOf(edited)?.map((b) => b.id));
    next = { ...edited, branches: Object.fromEntries(Object.entries(edited.branches).filter(([id]) => ids.has(id))) } as Step;
  }
  return replaceStep(workflow, edited.id, () => next);
}

const randomHex = () => Array.from(crypto.getRandomValues(new Uint8Array(4)), (b) => b.toString(16).padStart(2, "0")).join("");

/** A fresh id with the given prefix that isn't in `taken`. */
export function freshId(prefix: string, taken: Iterable<string>): string {
  const used = new Set(taken);
  for (;;) {
    const id = `${prefix}${randomHex()}`;
    if (!used.has(id)) return id;
  }
}

/** A new step of a type, with empty fields ready to fill in. */
export function defaultStep(type: StepType, id: string, position: Position): Step {
  const base = { id, position };
  const branchPair = (prefix: string, a: string, b: string): Branch[] => {
    const first = freshId(prefix, []);
    return [{ id: first, label: a }, { id: freshId(prefix, [first]), label: b }];
  };
  switch (type) {
    case "fileAdded": return { ...base, type, title: "When a file is added", folder: "~/Downloads", fileTypes: [], subfolders: false, next: null };
    case "schedule": return { ...base, type, title: "Every weekday at 09:00", schedule: { every: "weekday", time: "09:00" }, next: null };
    case "runNow": return { ...base, type, title: "When I run it", next: null };
    case "classify": return { ...base, type, title: "What kind of file is this?", instructions: "", categories: branchPair("c", "Category A", "Category B"), branches: {} };
    case "extract": return { ...base, type, title: "Get details", fields: [{ name: "detail", type: "text" }], ifMissing: "review", next: null };
    case "write": return { ...base, type, title: "Write something", instruction: "", saveAs: "text", next: null };
    case "agent": return { ...base, type, title: "Agent step", instruction: "", abilities: ["readFile"], outputs: [], next: null };
    case "rename": return { ...base, type, title: "Rename the file", template: "{file}", next: null };
    case "move": return { ...base, type, title: "Move the file", to: "", mode: "move", next: null };
    case "createFile": return { ...base, type, title: "Create a file", name: "", contents: "", next: null };
    case "tag": return { ...base, type, title: "Add tags", tags: [""], next: null };
    case "addRow": return { ...base, type, title: "Add a row", file: "", columns: [""], next: null };
    case "notify": return { ...base, type, title: "Notify me", message: "", next: null };
    case "if": return { ...base, type, title: "Check a value", condition: { left: "", op: "=", right: "" }, branches: {} };
    case "stop": return { ...base, type, title: "Stop" };
    case "askMe": return { ...base, type, title: "Ask me", question: "", answers: branchPair("a", "Yes", "No"), branches: {} };
  }
}

export function addStep(workflow: Workflow, type: StepType, position: Position): { workflow: Workflow; id: string } {
  const id = freshId("s", workflow.steps.map((s) => s.id));
  return { workflow: { ...workflow, steps: [...workflow.steps, defaultStep(type, id, position)] }, id };
}
