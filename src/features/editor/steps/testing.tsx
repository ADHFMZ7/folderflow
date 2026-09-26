// Helpers for the step settings screen tests: open a workflow through the whole
// app, select a step, and read back what was saved.

import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { expect } from "vitest";
import type { MockOptions } from "../../../api/mock";
import type { Step, Workflow } from "../../../api/types";
import { renderApp } from "../../../test/render";

const pos = (row: number) => ({ x: 0, y: row * 160 });

export const fileTrigger = (next: string | null, extra: Partial<Extract<Step, { type: "fileAdded" }>> = {}): Step => ({
  id: "t", type: "fileAdded", title: "When a file is added", position: pos(0),
  folder: "~/Downloads", fileTypes: [], subfolders: false, next, ...extra,
});

/** Opens the editor on a workflow made of `steps`, with the step titled `select` selected. */
export async function openWith(steps: Step[], select: string, options: MockOptions = {}) {
  const app = renderApp({ settings: { setupComplete: true }, ...options });
  const blank = await app.api.createWorkflow(null);
  const placed = steps.map((s, i) => ({ ...s, position: s.position ?? pos(i) }));
  const { workflow } = await app.api.saveWorkflow({ ...blank, steps: placed });
  window.location.hash = `#/workflows/${workflow.id}`;
  await screen.findByRole("textbox", { name: "Workflow name" });
  fireEvent.click(await within(canvas()).findByText(select));
  await within(inspector()).findByDisplayValue(select);
  return { ...app, wf: workflow };
}

export const canvas = () => screen.getByRole("region", { name: "Canvas" });
export const inspector = () => screen.getByRole("complementary", { name: "Inspector" });

/** Selects a step card by its title. */
export const selectStep = (title: string) => fireEvent.click(within(canvas()).getByText(title));

/** Saves (by the button while there is one; autosave otherwise) and returns the step as stored. */
export async function saved<T extends Step["type"]>(api: { getWorkflow(id: string): Promise<Workflow> }, wf: Workflow, id: string, check: (s: Extract<Step, { type: T }>) => void) {
  const button = screen.queryByRole("button", { name: "Save" });
  if (button && !(button as HTMLButtonElement).disabled) fireEvent.click(button);
  await waitFor(async () => {
    const step = (await api.getWorkflow(wf.id)).steps.find((s) => s.id === id) as Extract<Step, { type: T }>;
    expect(step).toBeDefined();
    check(step);
  });
}
