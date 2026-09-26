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

    // Collapsed to a rail of icons: the links stay, named by their labels, but the text goes.
    expect(screen.queryByText("Settings")).not.toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Settings" })).toHaveAttribute("title", "Settings");
    const show = screen.getByRole("button", { name: "Show sidebar" });
    expect(show).toHaveAttribute("aria-expanded", "false");
    expect(screen.getByRole("heading", { name: "Workflows" })).toBeInTheDocument();

    await user.click(show);
    expect(screen.getByText("Settings")).toBeInTheDocument();
  });

  it("remembers the choice the next time the app opens", async () => {
    const first = renderApp(home);
    await first.user.click(await screen.findByRole("button", { name: "Hide sidebar" }));
    first.unmount();

    renderApp(home);

    expect(await screen.findByRole("button", { name: "Show sidebar" })).toBeInTheDocument();
    expect(screen.queryByText("Settings")).not.toBeInTheDocument();
  });

  it("still shows that something needs you while collapsed", async () => {
    const { user } = renderApp({ ...home, sampleWorkflows: true });
    await screen.findByRole("link", { name: /Receipts and invoices/ });

    await user.click(screen.getByRole("button", { name: "Hide sidebar" }));

    const nav = screen.getByRole("navigation", { name: "Main" });
    expect(within(nav).getByLabelText("1 workflow needs you")).toHaveTextContent("1");
  });

  it("keeps the menu button in the title bar, right of the traffic lights, open or collapsed", async () => {
    const { user } = renderApp(home);
    const nav = await screen.findByRole("navigation", { name: "Main" });
    const hide = screen.getByRole("button", { name: "Hide sidebar" });
    const titleBar = hide.closest("header");
    expect(titleBar).toHaveAttribute("data-tauri-drag-region");
    expect(nav).not.toContainElement(hide);

    await user.click(hide);

    const show = screen.getByRole("button", { name: "Show sidebar" });
    expect(show.closest("header")).toBe(titleBar);
    expect(nav).not.toContainElement(show);
  });

  it("lets the window be moved from its top row, since the title bar is hidden", async () => {
    renderApp(home);
    const hide = await screen.findByRole("button", { name: "Hide sidebar" });
    expect(hide.parentElement).toHaveAttribute("data-tauri-drag-region");
  });

  it("works from inside the editor too", async () => {
    const { user, api } = renderApp(home);
    const wf = await api.createWorkflow("screenshots");
    window.location.hash = `#/workflows/${wf.id}`;
    await screen.findByRole("textbox", { name: "Workflow name" }, { timeout: 5000 });

    await user.click(screen.getByRole("button", { name: "Hide sidebar" }));

    expect(screen.queryByText("Workflows")).not.toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Workflow name" })).toBeInTheDocument();
  });
});
