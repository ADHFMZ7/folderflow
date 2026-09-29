import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { Step } from "../../api/types";
import { renderApp } from "../../test/render";

const home = { settings: { setupComplete: true } };
const titleBar = () => screen.findByRole("banner", { name: "Title bar" });

const say: Step[] = [
  { id: "t", type: "runNow", title: "Run now", position: { x: 0, y: 0 }, next: "n" },
  { id: "n", type: "notify", title: "Say it", position: { x: 0, y: 160 }, message: "Filed {file}", next: null },
];

describe("title bar", () => {
  it("shows runs in progress, and pauses and resumes everything", async () => {
    // Slow runs, so one is still queued while the test looks.
    const { api, user } = renderApp({ ...home, delayMs: 60_000 });
    const blank = await api.createWorkflow(null);
    const { workflow } = await api.saveWorkflow({ ...blank, steps: say });
    await api.runNow(workflow.id, ["~/Downloads/a.pdf"]);

    const running = await within(await titleBar()).findByRole("button", { name: "1 running. Pause all workflows" });
    expect(running).toHaveTextContent("1 running");
    await user.click(running);

    const resume = await within(await titleBar()).findByRole("button", { name: "Workflows are paused. Resume all workflows" });
    expect(resume).toHaveTextContent("Paused · Resume");
    expect(await screen.findByText(/Workflows are paused\. New files wait/)).toBeInTheDocument();
    expect((await api.getActivity()).paused).toBe(true);

    await user.click(resume);
    expect(await within(await titleBar()).findByRole("button", { name: "1 running. Pause all workflows" })).toBeInTheDocument();
    expect(screen.queryByText(/Workflows are paused\. New files wait/)).not.toBeInTheDocument();
  });

  it("says when an update is ready, and opens Settings for it", async () => {
    const { api, user } = renderApp({ ...home, update: { version: "0.9.1", notes: "Faster OCR." } });
    await within(await titleBar()).findByRole("button", { name: "Pause all workflows" });
    expect(within(await titleBar()).queryByRole("link", { name: "Update ready" })).not.toBeInTheDocument();

    await api.checkForUpdates();

    const ready = await within(await titleBar()).findByRole("link", { name: "Update ready" });
    expect(ready).toHaveAttribute("title", "Vela 0.9.1 is ready to install");
    await user.click(ready);
    expect(await screen.findByRole("button", { name: "Restart to update" })).toBeInTheDocument();
  });

  it("follows a pause made from the menu bar", async () => {
    const { api } = renderApp(home);
    await within(await titleBar()).findByRole("button", { name: "Pause all workflows" });

    await api.pauseAll(true);
    expect(await within(await titleBar()).findByRole("button", { name: "Workflows are paused. Resume all workflows" })).toBeInTheDocument();
    await api.pauseAll(false);
    expect(await within(await titleBar()).findByRole("button", { name: "Pause all workflows" })).toBeInTheDocument();
  });

  it("offers Pause all as an icon when nothing is running", async () => {
    const { api, user } = renderApp(home);
    await user.click(await within(await titleBar()).findByRole("button", { name: "Pause all workflows" }));
    await waitFor(async () => expect((await api.getActivity()).paused).toBe(true));
  });

  it("keeps notifications under the bell, opens their runs, and marks them read", async () => {
    const { api, user } = renderApp(home);
    const blank = await api.createWorkflow(null);
    const { workflow } = await api.saveWorkflow({ ...blank, name: "Say", steps: say });
    const [run] = await api.runNow(workflow.id, ["~/Downloads/a.pdf"]);
    await waitFor(async () => expect((await api.getRun(run.id)).status).toBe("done"));

    await user.click(await within(await titleBar()).findByRole("button", { name: "Notifications, 1 unread" }));
    const panel = screen.getByRole("dialog", { name: "Notifications" });
    const item = within(panel).getByRole("link", { name: /Say.*Filed a/ });
    expect(item).toHaveTextContent("Notify step · just now");

    await user.click(within(panel).getByRole("button", { name: "Mark all as read" }));
    expect(await within(await titleBar()).findByRole("button", { name: "Notifications" })).toBeInTheDocument();
    expect((await api.listNotices())[0].read).toBe(true);

    await user.click(within(panel).getByRole("link", { name: /Say.*Filed a/ }));
    expect(window.location.hash).toBe(`#/history/${run.id}`);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Notifications" })).not.toBeInTheDocument());
  });

  it("clears the list, leaving the runs in History", async () => {
    const { api, user } = renderApp(home);
    const blank = await api.createWorkflow(null);
    const { workflow } = await api.saveWorkflow({ ...blank, name: "Say", steps: say });
    const [run] = await api.runNow(workflow.id, ["~/Downloads/a.pdf"]);
    await waitFor(async () => expect((await api.getRun(run.id)).status).toBe("done"));

    await user.click(await within(await titleBar()).findByRole("button", { name: "Notifications, 1 unread" }));
    const panel = screen.getByRole("dialog", { name: "Notifications" });
    await user.click(within(panel).getByRole("button", { name: "Clear all" }));

    expect(await within(panel).findByText(/Nothing yet/)).toBeInTheDocument();
    expect(within(panel).queryByRole("button", { name: "Clear all" })).not.toBeInTheDocument();
    expect(within(await titleBar()).getByRole("button", { name: "Notifications" })).toBeInTheDocument();
    expect(await api.listNotices()).toEqual([]);
    expect((await api.getRun(run.id)).status).toBe("done");
  });

  it("switches between light and dark", async () => {
    const { api, user } = renderApp({ settings: { setupComplete: true, appearance: "light" } });

    await user.click(await within(await titleBar()).findByRole("button", { name: "Switch to dark mode" }));

    await waitFor(async () => expect((await api.getSettings()).settings.appearance).toBe("dark"));
    await waitFor(() => expect(document.documentElement.getAttribute("data-theme")).toBe("dark"));
    await user.click(within(await titleBar()).getByRole("button", { name: "Switch to light mode" }));
    await waitFor(async () => expect((await api.getSettings()).settings.appearance).toBe("light"));
  });
});
