import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { renderApp } from "../../test/render";

const home = { settings: { setupComplete: true } };

beforeEach(() => localStorage.clear());

describe("sidebar", () => {
  it("collapses and expands with the menu button", async () => {
    const { user } = renderApp(home);
    const hide = await screen.findByRole("button", { name: "Hide sidebar" });
    expect(hide).toHaveAttribute("aria-expanded", "true");

    await user.click(hide);

    expect(screen.queryByRole("link", { name: "Settings" })).not.toBeInTheDocument();
    const show = screen.getByRole("button", { name: "Show sidebar" });
    expect(show).toHaveAttribute("aria-expanded", "false");
    expect(screen.getByRole("heading", { name: "Workflows" })).toBeInTheDocument();

    await user.click(show);
    expect(screen.getByRole("link", { name: "Settings" })).toBeInTheDocument();
  });

  it("remembers the choice the next time the app opens", async () => {
    const first = renderApp(home);
    await first.user.click(await screen.findByRole("button", { name: "Hide sidebar" }));
    first.unmount();

    renderApp(home);

    expect(await screen.findByRole("button", { name: "Show sidebar" })).toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "Settings" })).not.toBeInTheDocument();
  });

  it("still shows that something needs you while collapsed", async () => {
    const { user } = renderApp({ ...home, sampleWorkflows: true });
    await screen.findByRole("link", { name: /Receipts and invoices/ });

    await user.click(screen.getByRole("button", { name: "Hide sidebar" }));

    const nav = screen.getByRole("navigation", { name: "Main" });
    expect(within(nav).getByLabelText("1 workflow needs you")).toHaveTextContent("1");
  });

  it("works from inside the editor too", async () => {
    const { user, api } = renderApp(home);
    const wf = await api.createWorkflow("screenshots");
    window.location.hash = `#/workflows/${wf.id}`;
    await screen.findByRole("textbox", { name: "Workflow name" }, { timeout: 5000 });

    await user.click(screen.getByRole("button", { name: "Hide sidebar" }));

    expect(screen.queryByRole("link", { name: "Workflows" })).not.toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Workflow name" })).toBeInTheDocument();
  });
});
