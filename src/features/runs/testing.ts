import { waitFor } from "@testing-library/react";
import { expect } from "vitest";
import type { Api } from "../../api/api";
import type { RunStatus, Step } from "../../api/types";

const at = (i: number) => ({ x: 0, y: i * 160 });

/** Run now → Ask "Log {file}?" (Log it / Skip) → Log it: Rename "Logged {file}". */
export const ASKS: Step[] = [
  { id: "t", type: "runNow", title: "Run now", position: at(0), next: "q" },
  { id: "q", type: "askMe", title: "Log it?", position: at(1), question: "Log {file}?",
    answers: [{ id: "log", label: "Log it" }, { id: "skip", label: "Skip" }], branches: { log: "r" } },
  { id: "r", type: "rename", title: "Rename it", position: at(2), template: "Logged {file}", next: null },
];

/** Run now → Rename "Done {file}" → Classify, which the mock can't run: the run fails there. */
export const FAILS: Step[] = [
  { id: "t", type: "runNow", title: "Run now", position: at(0), next: "r" },
  { id: "r", type: "rename", title: "Rename it", position: at(1), template: "Done {file}", next: "c" },
  { id: "c", type: "classify", title: "Sort it", position: at(2), instructions: "",
    categories: [{ id: "a", label: "A" }, { id: "b", label: "B" }], branches: {} },
];

/** Saves a workflow with these steps and runs it on `file`, waiting until the run reaches `status`. */
export async function runOf(api: Api, name: string, steps: Step[], file: string, status: RunStatus) {
  const blank = await api.createWorkflow(null);
  const { workflow } = await api.saveWorkflow({ ...blank, name, steps });
  const [queued] = await api.runNow(workflow.id, [file]);
  await waitFor(async () => expect((await api.getRun(queued.id)).status).toBe(status));
  return { workflow, runId: queued.id };
}
