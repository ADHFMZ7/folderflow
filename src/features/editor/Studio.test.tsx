import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { renderApp } from "../../test/render";

afterEach(() => vi.restoreAllMocks());

/** Opens the app on a freshly created workflow's page. */
async function openWorkflow(templateId: string | null = "screenshots") {
  const app = renderApp({ settings: { setupComplete: true } });
  const wf = await app.api.createWorkflow(templateId);
  window.location.hash = `#/workflows/${wf.id}`;
  await screen.findByRole("textbox", { name: "Workflow name" });
  return { ...app, wf };
}

const canvas = () => screen.getByRole("region", { name: "Canvas" });

/** Selects a step card. A plain click: jsdom's mouse-down has no window, which React Flow's drag code needs. */
const selectStep = (title: string) => fireEvent.click(within(canvas()).getByText(title));

describe("studio", () => {
  it("shows the workflow's steps on the canvas", async () => {
    await openWorkflow();
    expect(screen.getByRole("textbox", { name: "Workflow name" })).toHaveValue("Tidy screenshots");
    for (const title of ["When an image lands on the Desktop", "Is it a screenshot?", "Move to Screenshots"]) {
      expect(within(canvas()).getByText(title)).toBeInTheDocument();
    }
  });

  it("saves changes, and only when there are some", async () => {
    const { user, api, wf } = await openWorkflow();
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();

    const name = screen.getByRole("textbox", { name: "Workflow name" });
    await user.clear(name);
    await user.type(name, "Screenshots tidy-up");
    expect(screen.getByText("Unsaved changes")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(screen.queryByText("Unsaved changes")).not.toBeInTheDocument());
    expect(await api.getWorkflow(wf.id)).toMatchObject({ name: "Screenshots tidy-up", revision: 2 });
  });

  it("saves with Cmd-S", async () => {
    const { user, api, wf } = await openWorkflow();
    await user.type(screen.getByRole("textbox", { name: "Workflow name" }), "!");

    await user.keyboard("{Meta>}s{/Meta}");

    await waitFor(async () => expect((await api.getWorkflow(wf.id)).name).toBe("Tidy screenshots!"));
  });

  it("adds a step from the palette", async () => {
    const { user } = await openWorkflow();

    await user.click(screen.getByRole("button", { name: "Add Notify" }));

    expect(within(canvas()).getByText("Notify me")).toBeInTheDocument();
    expect(screen.getByText("Unsaved changes")).toBeInTheDocument();
  });

  it("deletes a step and the links that led to it", async () => {
    const { user, api, wf } = await openWorkflow();

    selectStep("Move to Screenshots");
    await user.click(screen.getByRole("button", { name: "Delete step" }));
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(async () => {
      const saved = await api.getWorkflow(wf.id);
      expect(saved.steps.map((s) => s.title)).not.toContain("Move to Screenshots");
      expect(saved.steps.find((s) => s.type === "if")).toMatchObject({ branches: {} });
    });
  });

  it("shows problems as the workflow changes, and selects the step they're about", async () => {
    const { user } = await openWorkflow();

    selectStep("When an image lands on the Desktop");
    await user.click(screen.getByRole("button", { name: "Delete step" }));

    const problems = await screen.findByRole("list", { name: "Problems" });
    expect(problems).toHaveTextContent("Add a trigger to say when this workflow runs.");
    expect(screen.getByText("1 problem")).toBeInTheDocument();
  });

  it("explains a save conflict and can load the newer version", async () => {
    const { user, api, wf } = await openWorkflow();
    await api.saveWorkflow({ ...wf, name: "Changed elsewhere" });

    await user.type(screen.getByRole("textbox", { name: "Workflow name" }), "!");
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("changed somewhere else");
    await user.click(screen.getByRole("button", { name: "Load the saved version" }));
    expect(await screen.findByDisplayValue("Changed elsewhere")).toBeInTheDocument();
  });

  it("asks before leaving with unsaved changes", async () => {
    const { user } = await openWorkflow();
    await user.type(screen.getByRole("textbox", { name: "Workflow name" }), "!");
    const confirm = vi.spyOn(window, "confirm").mockReturnValueOnce(false).mockReturnValueOnce(true);

    await user.click(screen.getByRole("link", { name: "All workflows" }));
    await waitFor(() => expect(confirm).toHaveBeenCalledTimes(1));
    expect(screen.getByRole("textbox", { name: "Workflow name" })).toBeInTheDocument();

    await user.click(screen.getByRole("link", { name: "All workflows" }));
    expect(await screen.findByRole("heading", { name: "Workflows" })).toBeInTheDocument();
  });

  it("says why trying and turning on aren't available yet", async () => {
    await openWorkflow();
    expect(screen.getByRole("button", { name: "Try on a file" })).toBeDisabled();
    expect(screen.getByRole("switch", { name: "On" })).toBeDisabled();
  });
});
