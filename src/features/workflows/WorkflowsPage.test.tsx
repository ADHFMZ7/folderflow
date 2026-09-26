import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderApp } from "../../test/render";

const home = { settings: { setupComplete: true } };

describe("workflows home", () => {
  it("creates a blank workflow and opens it", async () => {
    const { user, api } = renderApp(home);

    await user.click(await screen.findByRole("button", { name: "+ New workflow" }));

    expect(await screen.findByRole("textbox", { name: "Workflow name" })).toHaveValue("New workflow");
    const [created] = await api.listWorkflows();
    expect(window.location.hash).toBe(`#/workflows/${created.id}`);
  });

  it("creates a workflow from a template and opens it", async () => {
    const { user } = renderApp(home);

    await user.click(await screen.findByRole("button", { name: /Sort receipts/ }));

    expect(await screen.findByRole("textbox", { name: "Workflow name" })).toHaveValue("Sort receipts");
  });

  it("lists saved workflows and opens one", async () => {
    const { user, api } = renderApp(home);
    const wf = await api.createWorkflow("screenshots");

    await user.click(await screen.findByRole("link", { name: /Tidy screenshots/ }));

    expect(await screen.findByRole("textbox", { name: "Workflow name" })).toHaveValue("Tidy screenshots");
    expect(window.location.hash).toBe(`#/workflows/${wf.id}`);
  });

  it("deletes a workflow after confirming", async () => {
    const { user, api } = renderApp(home);
    await api.createWorkflow("screenshots");
    await api.createWorkflow("cleanup");

    await user.click(await screen.findByRole("button", { name: "Delete Tidy screenshots" }));
    await user.click(screen.getByRole("button", { name: "Move to trash" }));

    expect(await api.listWorkflows()).toEqual([expect.objectContaining({ name: "Weekly clean-up" })]);
    expect(screen.queryByRole("link", { name: /Tidy screenshots/ })).not.toBeInTheDocument();
  });

  it("keeps a workflow when deleting is cancelled", async () => {
    const { user, api } = renderApp(home);
    await api.createWorkflow("screenshots");

    await user.click(await screen.findByRole("button", { name: "Delete Tidy screenshots" }));
    await user.click(screen.getByRole("button", { name: "Cancel" }));

    expect(await api.listWorkflows()).toHaveLength(1);
  });

  it("shows a damaged workflow file without opening it", async () => {
    renderApp({ ...home, damagedWorkflows: ["2b7e1c9a-0d3f-4e5a-8b6c-7d8e9f0a1b2c.json"] });

    const card = await screen.findByRole("article", { name: "2b7e1c9a-0d3f-4e5a-8b6c-7d8e9f0a1b2c.json" });
    expect(card).toHaveTextContent("can't be read");
    expect(within(card).queryByRole("link")).not.toBeInTheDocument();
  });

  it("goes back to all workflows from a workflow", async () => {
    const { user } = renderApp(home);
    await user.click(await screen.findByRole("button", { name: "+ New workflow" }));

    await user.click(await screen.findByRole("link", { name: "All workflows" }));

    expect(await screen.findByRole("heading", { name: "Workflows" })).toBeInTheDocument();
  });
});
