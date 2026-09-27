import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { renderApp } from "../../test/render";
import type { Step } from "../../api/types";
import { fileTrigger, openWith } from "./steps/testing";

afterEach(() => vi.restoreAllMocks());

/** Opens the app on a freshly created workflow's page. */
async function openWorkflow(templateId: string | null = "screenshots") {
  const app = renderApp({ settings: { setupComplete: true } });
  const wf = await app.api.createWorkflow(templateId);
  window.location.hash = `#/workflows/${wf.id}`;
  // Generous: the editor loads React Flow, and a busy machine can be slow.
  await screen.findByRole("textbox", { name: "Workflow name" }, { timeout: 5000 });
  return { ...app, wf };
}

const canvas = () => screen.getByRole("region", { name: "Canvas" });

/**
 * Selects a step card once React Flow has drawn it. A plain click: jsdom's
 * mouse-down has no window, which React Flow's drag code needs.
 */
const selectStep = async (title: string) => fireEvent.click(await within(canvas()).findByText(title));

/** jsdom has no layout: place the canvas at x 200-1200, y 0-800. */
const stubCanvasRect = () => {
  canvas().getBoundingClientRect = () => ({ left: 200, top: 0, right: 1200, bottom: 800, width: 1000, height: 800, x: 200, y: 0, toJSON: () => ({}) });
};

describe("studio", () => {
  it("shows the workflow's steps on the canvas", async () => {
    await openWorkflow();
    expect(screen.getByRole("textbox", { name: "Workflow name" })).toHaveValue("Tidy screenshots");
    for (const title of ["When an image lands on the Desktop", "Is it a screenshot?", "Move to Screenshots"]) {
      // React Flow draws cards after the first render.
      expect(await within(canvas()).findByText(title)).toBeInTheDocument();
    }
  });

  it("adds a step from the palette", async () => {
    const { user } = await openWorkflow();

    await user.click(screen.getByRole("button", { name: "Add Notify" }));

    expect(within(canvas()).getByText("Notify me")).toBeInTheDocument();
  });

  it("deletes a step and the links that led to it", async () => {
    const { user, api, wf } = await openWorkflow();

    await selectStep("Move to Screenshots");
    await user.click(screen.getByRole("button", { name: "Delete step" }));

    await waitFor(async () => {
      const saved = await api.getWorkflow(wf.id);
      expect(saved.steps.map((s) => s.title)).not.toContain("Move to Screenshots");
      expect(saved.steps.find((s) => s.type === "if")).toMatchObject({ branches: {} });
    });
  });

  it("keeps the canvas clear until a step is selected, and closes the step's card again", async () => {
    const { user } = await openWorkflow();
    expect(screen.queryByRole("complementary", { name: "Inspector" })).not.toBeInTheDocument();

    await selectStep("Move to Screenshots");
    expect(await screen.findByRole("complementary", { name: "Inspector" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(screen.queryByRole("complementary", { name: "Inspector" })).not.toBeInTheDocument();
  });

  it("says nothing about problems until there are some, then lists them from the toolbar", async () => {
    const { user } = await openWorkflow();
    expect(screen.queryByText(/problem/i)).not.toBeInTheDocument();

    await selectStep("When an image lands on the Desktop");
    await user.click(screen.getByRole("button", { name: "Delete step" }));
    await user.click(await screen.findByRole("button", { name: "1 problem" }));

    const problems = await screen.findByRole("list", { name: "Problems" });
    expect(problems).toHaveTextContent("Add a trigger to say when this workflow runs.");
  });

  it("opens a step from the problem list", async () => {
    const { user } = await openWith([fileTrigger(null, { folder: "" })], "When a file is added");
    await user.click(screen.getByRole("button", { name: "Close" }));

    await user.click(await screen.findByRole("button", { name: /problem/ }));
    await user.click(within(screen.getByRole("list", { name: "Problems" })).getByRole("button", { name: /When a file is added/ }));

    expect(await within(screen.getByRole("complementary", { name: "Inspector" })).findByDisplayValue("When a file is added")).toBeInTheDocument();
  });

  it("leaves without asking when nothing changed", async () => {
    const { user } = await openWorkflow();
    await selectStep("Move to Screenshots");

    await user.click(screen.getByRole("link", { name: "All workflows" }));

    expect(await screen.findByRole("heading", { name: "Workflows" })).toBeInTheDocument();
  });

  it("drags a step from the palette onto the canvas with the pointer", async () => {
    await openWorkflow();
    stubCanvasRect();
    const item = screen.getByRole("button", { name: "Add Notify" });

    fireEvent.pointerDown(item, { clientX: 20, clientY: 400, button: 0 });
    fireEvent.pointerMove(window, { clientX: 300, clientY: 300 });
    expect(screen.getByTestId("drag-ghost")).toHaveTextContent("Notify");
    fireEvent.pointerUp(window, { clientX: 500, clientY: 300 });

    expect(await within(canvas()).findByText("Notify me")).toBeInTheDocument();
    expect(screen.queryByTestId("drag-ghost")).not.toBeInTheDocument();
  });

  it("adds nothing when a palette drag ends outside the canvas", async () => {
    await openWorkflow();
    stubCanvasRect();
    const item = screen.getByRole("button", { name: "Add Notify" });

    fireEvent.pointerDown(item, { clientX: 20, clientY: 400, button: 0 });
    fireEvent.pointerMove(window, { clientX: 60, clientY: 420 });
    fireEvent.pointerUp(window, { clientX: 60, clientY: 420 });

    expect(within(canvas()).queryByText("Notify me")).not.toBeInTheDocument();
  });

  it("saves on its own once edits pause, with no Save button", async () => {
    const { user, api, wf } = await openWorkflow();
    expect(screen.queryByRole("button", { name: "Save" })).not.toBeInTheDocument();

    const name = screen.getByRole("textbox", { name: "Workflow name" });
    await user.clear(name);
    await user.type(name, "Screenshots tidy-up");

    await waitFor(async () => expect(await api.getWorkflow(wf.id)).toMatchObject({ name: "Screenshots tidy-up" }));
    expect(await screen.findByText("Saved")).toBeInTheDocument();
  });

  it("says nothing about saving while there's nothing to save, and \"Saved\" only briefly", async () => {
    const { user } = await openWorkflow();
    expect(screen.queryByText(/saved|saving/i)).not.toBeInTheDocument();

    await user.type(screen.getByRole("textbox", { name: "Workflow name" }), "!");

    expect(await screen.findByText("Saved")).toBeInTheDocument();
    await waitFor(() => expect(screen.queryByText("Saved")).not.toBeInTheDocument(), { timeout: 4000 });
  });

  it("saves at once with Cmd-S", async () => {
    const { user, api, wf } = await openWorkflow();
    const save = vi.spyOn(api, "saveWorkflow");
    await user.type(screen.getByRole("textbox", { name: "Workflow name" }), "!");

    await user.keyboard("{Meta>}s{/Meta}");

    expect(save).toHaveBeenCalledTimes(1);
    await waitFor(async () => expect((await api.getWorkflow(wf.id)).name).toBe("Tidy screenshots!"));
  });

  it("undoes typing in one step, and redoes it", async () => {
    const { user, api, wf } = await openWorkflow();
    const name = screen.getByRole("textbox", { name: "Workflow name" });
    await user.type(name, " v2");

    await user.keyboard("{Meta>}z{/Meta}");
    expect(name).toHaveValue("Tidy screenshots");
    await user.keyboard("{Meta>}{Shift>}z{/Shift}{/Meta}");
    expect(name).toHaveValue("Tidy screenshots v2");

    await waitFor(async () => expect((await api.getWorkflow(wf.id)).name).toBe("Tidy screenshots v2"));
  });

  it("undoes adding and deleting steps, with the toolbar buttons too", async () => {
    const { user } = await openWorkflow();
    const undo = screen.getByRole("button", { name: "Undo" });
    expect(undo).toBeDisabled();

    await user.click(screen.getByRole("button", { name: "Add Notify" }));
    await selectStep("Move to Screenshots");
    await user.click(screen.getByRole("button", { name: "Delete step" }));

    await user.click(undo);
    expect(within(canvas()).getByText("Move to Screenshots")).toBeInTheDocument();
    await user.click(undo);
    expect(within(canvas()).queryByText("Notify me")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Redo" }));
    expect(within(canvas()).getByText("Notify me")).toBeInTheDocument();
  });

  it("can undo past a save without a false conflict", async () => {
    const { user, api, wf } = await openWorkflow();
    const name = screen.getByRole("textbox", { name: "Workflow name" });
    await user.type(name, "!");
    await waitFor(async () => expect((await api.getWorkflow(wf.id)).name).toBe("Tidy screenshots!"));

    await user.keyboard("{Meta>}z{/Meta}");

    await waitFor(async () => expect((await api.getWorkflow(wf.id)).name).toBe("Tidy screenshots"));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("explains a conflict with a change made elsewhere and can load it", async () => {
    const { user, api, wf } = await openWorkflow();
    await api.saveWorkflow({ ...wf, name: "Changed elsewhere" });

    await user.type(screen.getByRole("textbox", { name: "Workflow name" }), "!");

    expect(await screen.findByRole("alert")).toHaveTextContent("changed somewhere else");
    await user.click(screen.getByRole("button", { name: "Load the latest version" }));
    expect(await screen.findByDisplayValue("Changed elsewhere")).toBeInTheDocument();
  });

  it("finishes saving before leaving, without asking", async () => {
    const { user, api, wf } = await openWorkflow();
    const confirm = vi.spyOn(window, "confirm");
    await user.type(screen.getByRole("textbox", { name: "Workflow name" }), "!");

    await user.click(screen.getByRole("link", { name: "All workflows" }));

    expect(await screen.findByRole("heading", { name: "Workflows" })).toBeInTheDocument();
    expect((await api.getWorkflow(wf.id)).name).toBe("Tidy screenshots!");
    expect(confirm).not.toHaveBeenCalled();
  });

  it("asks before leaving if the last changes couldn't be saved", async () => {
    const { user, api } = await openWorkflow();
    api.saveWorkflow = () => Promise.reject(new Error("The disk is full."));
    await user.type(screen.getByRole("textbox", { name: "Workflow name" }), "!");

    await user.click(screen.getByRole("link", { name: "All workflows" }));

    const dialog = await screen.findByRole("dialog", { name: "Your last changes aren't saved" });
    expect(dialog).toHaveTextContent("The disk is full.");
    await user.click(within(dialog).getByRole("button", { name: "Keep editing" }));
    expect(screen.getByRole("textbox", { name: "Workflow name" })).toHaveValue("Tidy screenshots!");
  });

  describe("a workflow that is on", () => {
    async function openRunning() {
      const app = renderApp({ settings: { setupComplete: true } });
      const created = await app.api.createWorkflow("screenshots");
      const { workflow: live } = await app.api.saveWorkflow({ ...created, enabled: true });
      window.location.hash = `#/workflows/${live.id}`;
      await screen.findByRole("textbox", { name: "Workflow name" }, { timeout: 5000 });
      return { ...app, live };
    }

    it("saves edits to a draft and keeps the running version", async () => {
      const { user, api, live } = await openRunning();

      await user.type(screen.getByRole("textbox", { name: "Workflow name" }), " v2");

      expect(await screen.findByRole("status", { name: "Changes not live" })).toHaveTextContent("aren't live yet");
      await waitFor(async () => expect((await api.getDraft(live.id))?.name).toBe("Tidy screenshots v2"));
      expect(await api.getWorkflow(live.id)).toEqual(live);
    });

    it("applies the draft to make it live", async () => {
      const { user, api, live } = await openRunning();
      await user.type(screen.getByRole("textbox", { name: "Workflow name" }), " v2");
      await waitFor(async () => expect(await api.getDraft(live.id)).not.toBeNull());

      await user.click(screen.getByRole("button", { name: "Apply" }));

      await waitFor(async () => expect(await api.getWorkflow(live.id)).toMatchObject({ name: "Tidy screenshots v2", revision: live.revision + 1 }));
      expect(screen.queryByRole("status", { name: "Changes not live" })).not.toBeInTheDocument();
    });

    it("won't apply while there are problems", async () => {
      const { user } = await openRunning();
      await selectStep("When an image lands on the Desktop");
      await user.click(screen.getByRole("button", { name: "Delete step" }));

      await waitFor(() => expect(screen.getByRole("button", { name: "Apply" })).toBeDisabled());
    });

    it("discards the draft and goes back to the running version", async () => {
      const { user, api, live } = await openRunning();
      await user.type(screen.getByRole("textbox", { name: "Workflow name" }), " v2");
      await waitFor(async () => expect(await api.getDraft(live.id)).not.toBeNull());

      await user.click(screen.getByRole("button", { name: "Discard changes" }));

      expect(await screen.findByDisplayValue("Tidy screenshots")).toBeInTheDocument();
      await waitFor(async () => expect(await api.getDraft(live.id)).toBeNull());
    });

    it("opens with the waiting draft when there is one", async () => {
      const app = renderApp({ settings: { setupComplete: true } });
      const created = await app.api.createWorkflow("screenshots");
      const { workflow: live } = await app.api.saveWorkflow({ ...created, enabled: true });
      await app.api.saveDraft({ ...live, name: "Waiting" });

      window.location.hash = `#/workflows/${live.id}`;

      expect(await screen.findByDisplayValue("Waiting")).toBeInTheDocument();
      expect(screen.getByRole("status", { name: "Changes not live" })).toBeInTheDocument();
    });
  });
});

describe("the On switch", () => {
  const on = () => screen.getByRole("switch", { name: "On" });

  it("turns the workflow on, with the edits made so far", async () => {
    const { user, api, wf } = await openWorkflow();
    await user.type(screen.getByRole("textbox", { name: "Workflow name" }), " v2");

    await user.click(on());

    await waitFor(async () => expect(await api.getWorkflow(wf.id)).toMatchObject({ enabled: true, name: "Tidy screenshots v2" }));
    expect(on()).toBeChecked();
    // From now on, edits wait in a draft.
    await user.type(screen.getByRole("textbox", { name: "Workflow name" }), "!");
    expect(await screen.findByRole("status", { name: "Changes not live" })).toBeInTheDocument();
  });

  it("can't turn on a workflow with problems, and says why", async () => {
    const { user } = await openWorkflow();
    await selectStep("When an image lands on the Desktop");
    await user.click(screen.getByRole("button", { name: "Delete step" }));

    await waitFor(() => expect(on()).toBeDisabled());
    expect(on().closest("[title]")).toHaveAttribute("title", "Fix the problems before turning this workflow on.");
  });

  it("turns the workflow off, keeping changes that weren't live", async () => {
    const app = renderApp({ settings: { setupComplete: true } });
    const created = await app.api.createWorkflow("screenshots");
    const { workflow: live } = await app.api.saveWorkflow({ ...created, enabled: true });
    await app.api.saveDraft({ ...live, name: "Waiting" });
    window.location.hash = `#/workflows/${live.id}`;
    await screen.findByDisplayValue("Waiting", undefined, { timeout: 5000 });

    await app.user.click(on());

    await waitFor(async () => expect(await app.api.getWorkflow(live.id)).toMatchObject({ enabled: false, name: "Waiting" }));
    expect(await app.api.getDraft(live.id)).toBeNull();
    expect(on()).not.toBeChecked();
    expect(screen.queryByRole("status", { name: "Changes not live" })).not.toBeInTheDocument();
  });

  it("isn't offered for a workflow that runs only when chosen", async () => {
    await openWith([{ id: "t", type: "runNow", title: "When I choose", position: { x: 0, y: 0 }, next: null }], "When I choose");
    expect(on()).toBeDisabled();
    expect(on().closest("[title]")).toHaveAttribute("title", "This workflow runs when you choose Run…, so there's nothing to turn on.");
  });
});

describe("run now", () => {
  const runNow = (next: string | null): Step => ({ id: "t", type: "runNow", title: "Run now", position: { x: 0, y: 0 }, next });
  const notify: Step = { id: "n", type: "notify", title: "Say hello", position: { x: 0, y: 160 }, message: "Hello {file}", next: null };
  const run = () => fireEvent.click(screen.getByRole("button", { name: "Run…" }));

  it("runs the saved workflow on the chosen file and says how it went", async () => {
    const { api, wf } = await openWith([runNow("n"), notify], "Say hello", { chosenFiles: ["~/Downloads/Scan_0042.pdf"] });

    run();

    expect(await screen.findByText("Ran on Scan_0042.pdf")).toBeInTheDocument();
    const [done] = await api.listRuns({ workflowId: wf.id });
    expect(done).toMatchObject({ status: "done", file: "Scan_0042.pdf" });
  });

  it("counts the files, and says why a run failed", async () => {
    const write: Step = { id: "q", type: "write", title: "Sum up first", position: { x: 0, y: 160 }, instruction: "Sum up", saveAs: "summary", next: null };
    await openWith([fileTrigger("q"), write], "Sum up first", { chosenFiles: ["~/Downloads/a.pdf", "~/Downloads/b.pdf"] });

    run();

    expect(await screen.findByText("Ran on 2 files, 2 failed: Write steps can't run in this version of FolderFlow yet.")).toBeInTheDocument();
  });

  it("says when the run is waiting for an answer", async () => {
    const ask: Step = { id: "q", type: "askMe", title: "Ask first", position: { x: 0, y: 160 }, question: "File it?",
      answers: [{ id: "y", label: "Yes" }, { id: "n", label: "No" }], branches: {} };
    await openWith([fileTrigger("q"), ask], "Ask first");

    run();

    expect(await screen.findByText("It's waiting for your answer in Needs you")).toBeInTheDocument();
  });

  it("runs nothing when the picker is cancelled", async () => {
    const { api, wf } = await openWith([runNow("n"), notify], "Say hello", { chosenFiles: [] });

    run();

    await waitFor(async () => expect(await api.listRuns({ workflowId: wf.id })).toEqual([]));
    expect(screen.queryByText(/^Ran on|^Running on/)).not.toBeInTheDocument();
  });

  it("waits for the problems to be fixed", async () => {
    await openWith([runNow("n"), { ...notify, message: "" }], "Say hello");
    expect(screen.getByRole("button", { name: "Run…" })).toBeDisabled();
  });

  it("isn't offered for a workflow that runs on a schedule", async () => {
    await openWorkflow("cleanup");
    expect(screen.queryByRole("button", { name: "Run…" })).not.toBeInTheDocument();
  });
});

describe("try on a file", () => {
  const runNow = (next: string | null): Step => ({ id: "t", type: "runNow", title: "Run now", position: { x: 0, y: 0 }, next });
  const rename: Step = { id: "r", type: "rename", title: "Name it", position: { x: 0, y: 160 }, template: "Filed {file}", next: "n" };
  const notify: Step = { id: "n", type: "notify", title: "Say hello", position: { x: 0, y: 320 }, message: "Hello {file}", next: null };
  const tryIt = () => fireEvent.click(screen.getByRole("button", { name: "Try on a file" }));
  const panel = () => screen.findByRole("region", { name: "Try on Scan_0042.pdf" });

  it("shows what each step would do, and changes and records nothing", async () => {
    const { api, wf } = await openWith([runNow("r"), rename, notify], "Name it", { chosenFiles: ["~/Downloads/Scan_0042.pdf"] });

    tryIt();

    const tried = await panel();
    expect(within(tried).getByText("Nothing was changed. This is what a run would do.")).toBeInTheDocument();
    expect(within(tried).getByText("Would rename Scan_0042.pdf to Filed Scan_0042.pdf.")).toBeInTheDocument();
    expect(within(tried).getByText("Would show the notification \"Hello Scan_0042\".")).toBeInTheDocument();
    expect(within(canvas()).getAllByText("Tried")).toHaveLength(3);
    expect(await api.listRuns({ workflowId: wf.id })).toEqual([]);
    expect(await api.listNotices()).toEqual([]);

    fireEvent.click(within(tried).getByRole("button", { name: "Close the try" }));
    expect(screen.queryByRole("region", { name: "Try on Scan_0042.pdf" })).not.toBeInTheDocument();
    expect(within(canvas()).queryByText("Tried")).not.toBeInTheDocument();
  });

  it("shows the values a step produced", async () => {
    await openWith([runNow("r"), rename, notify], "Name it");
    tryIt();
    const tried = await panel();

    fireEvent.click(within(tried).getByRole("button", { name: "Values of Name it" }));

    expect(within(tried).getByText("newName")).toBeInTheDocument();
    expect(within(tried).getByText("Filed Scan_0042")).toBeInTheDocument();
  });

  it("asks inside the try, and carries on down the answer's branch", async () => {
    const ask: Step = { id: "q", type: "askMe", title: "Log it?", position: { x: 0, y: 160 }, question: "Log {file}?",
      answers: [{ id: "log", label: "Log it" }, { id: "skip", label: "Skip" }], branches: { log: "r" } };
    const { api } = await openWith([runNow("q"), ask, { ...rename, next: null }], "Log it?");

    tryIt();
    const tried = await panel();
    expect(within(tried).getByText("Log Scan_0042?")).toBeInTheDocument();
    fireEvent.click(within(tried).getByRole("button", { name: "Log it" }));

    expect(await within(tried).findByText("You answered Log it.")).toBeInTheDocument();
    expect(within(tried).getByText("Would rename Scan_0042.pdf to Filed Scan_0042.pdf.")).toBeInTheDocument();
    expect(await api.listNeedsYou()).toEqual([]);
  });

  it("stops at a problem on the path it takes, and says where", async () => {
    await openWith([runNow("r"), { ...rename, template: "" }, notify], "Name it");

    tryIt();

    const tried = await panel();
    expect(within(tried).getByText(/^Stopped at Name it: /)).toBeInTheDocument();
    expect(within(canvas()).getByText("Stopped here")).toBeInTheDocument();
    expect(within(tried).queryByText(/Would show the notification/)).not.toBeInTheDocument();
  });

  it("does nothing when the picker is cancelled", async () => {
    await openWith([runNow("r"), rename, notify], "Name it", { chosenFiles: [] });
    tryIt();
    await new Promise((r) => setTimeout(r, 20));
    expect(screen.queryByRole("region", { name: /^Try on/ })).not.toBeInTheDocument();
  });

  it("isn't offered for a workflow that runs on a schedule", async () => {
    await openWorkflow("cleanup");
    expect(screen.queryByRole("button", { name: "Try on a file" })).not.toBeInTheDocument();
  });
});

describe("window", () => {
  it("can be moved by the editor's toolbar, since the title bar is hidden", async () => {
    await openWorkflow();
    expect(screen.getByRole("textbox", { name: "Workflow name" }).closest("header")).toHaveAttribute("data-tauri-drag-region");
  });
});
