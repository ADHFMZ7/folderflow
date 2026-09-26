// What the palette offers, and how each step type looks on the canvas.

import type { StepType } from "../../api/types";

export type StepKind = "trigger" | "ai" | "action" | "logic" | "human";

export type StepInfo = { type: StepType; kind: StepKind; label: string; glyph: string; blurb: string };

export const STEP_CATALOG: StepInfo[] = [
  { type: "fileAdded", kind: "trigger", label: "File added", glyph: "⤓", blurb: "Run for each new file in a folder" },
  { type: "schedule", kind: "trigger", label: "Schedule", glyph: "◷", blurb: "Run at set times" },
  { type: "runNow", kind: "trigger", label: "Run now", glyph: "▶", blurb: "Run by hand on chosen files" },
  { type: "classify", kind: "ai", label: "Classify", glyph: "⑂", blurb: "Sort into categories, one branch each" },
  { type: "extract", kind: "ai", label: "Extract", glyph: "⌗", blurb: "Pull out typed, required details" },
  { type: "write", kind: "ai", label: "Write", glyph: "✎", blurb: "Write a summary, reply or description" },
  { type: "agent", kind: "ai", label: "Agent step", glyph: "✦", blurb: "A free-form instruction for anything else" },
  { type: "rename", kind: "action", label: "Rename", glyph: "Aa", blurb: "Rename the file from a template" },
  { type: "move", kind: "action", label: "Move / Copy", glyph: "→", blurb: "Move or copy the file to a folder" },
  { type: "createFile", kind: "action", label: "Create file", glyph: "+", blurb: "Write a new file" },
  { type: "tag", kind: "action", label: "Tag", glyph: "#", blurb: "Add Finder tags" },
  { type: "addRow", kind: "action", label: "Add row", glyph: "▦", blurb: "Append a row to a CSV file" },
  { type: "notify", kind: "action", label: "Notify", glyph: "◉", blurb: "Show a macOS notification" },
  { type: "if", kind: "logic", label: "If", glyph: "◇", blurb: "Branch on a value" },
  { type: "stop", kind: "logic", label: "Stop", glyph: "■", blurb: "End the run" },
  { type: "askMe", kind: "human", label: "Ask me", glyph: "?", blurb: "Pause for my answer" },
];

export const KIND_LABEL: Record<StepKind, string> = { trigger: "Trigger", ai: "AI", action: "Action", logic: "Logic", human: "Human" };

export const infoFor = (type: StepType) => STEP_CATALOG.find((s) => s.type === type)!;
