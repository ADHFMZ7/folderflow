import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderApp } from "../../test/render";
import { ASKS, FAILS, runOf } from "./testing";

const home = { settings: { setupComplete: true } };

describe("history", () => {
  it("lists runs newest first and narrows them by workflow and state", async () => {
    const { api, user } = renderApp(home);
    await runOf(api, "Sort", FAILS, "~/Downloads/a.pdf", "failed");
    const { workflow } = await runOf(api, "Log scans", ASKS, "~/Downloads/b.pdf", "waiting");
    window.location.hash = "#/history";

    const list = await screen.findByRole("list", { name: "Runs" });
    await waitFor(() => expect(within(list).getAllByRole("link").map((l) => l.textContent)).toEqual([
      expect.stringMatching(/^b\.pdfLog scans · just nowWaiting for you$/),
      expect.stringMatching(/^a\.pdfSort · just now.*Failed$/),
    ]));

    await user.selectOptions(screen.getByRole("combobox", { name: "Workflow" }), workflow.id);
    await waitFor(() => expect(within(screen.getByRole("list", { name: "Runs" })).getAllByRole("link")).toHaveLength(1));
    await user.selectOptions(screen.getByRole("combobox", { name: "State" }), "done");
    expect(await screen.findByText("No runs match.")).toBeInTheDocument();
  });

  it("opens a run, shows what each step did, and undoes it", async () => {
    const { api, user } = renderApp(home);
    const { runId } = await runOf(api, "Log scans", ASKS, "~/Downloads/Scan.pdf", "waiting");
    await api.answer(runId, "log");
    await waitFor(async () => expect((await api.getRun(runId)).status).toBe("done"));
    window.location.hash = "#/history";

    const list = await screen.findByRole("list", { name: "Runs" });
    await user.click(within(list).getByRole("link", { name: /Scan\.pdf/ }));

    expect(window.location.hash).toBe(`#/history/${runId}`);
    const steps = await screen.findByRole("list", { name: "Steps" });
    expect(within(steps).getAllByRole("listitem").map((li) => li.textContent)).toEqual([
      "Run nowDone", "Log it?DoneYou answered Log it.", "Rename itDoneRenamed Scan.pdf to Logged Scan.pdf.",
    ]);

    await user.click(screen.getByRole("button", { name: "Undo run" }));
    await user.click(within(screen.getByRole("group", { name: "Undo this run?" })).getByRole("button", { name: "Undo run" }));

    expect(await screen.findByRole("region", { name: "Undone" })).toHaveTextContent("This run was undone. Put back 1 change.");
    expect(screen.queryByRole("button", { name: "Undo run" })).not.toBeInTheDocument();
  });

  it("answers a waiting run from its page", async () => {
    const { api, user } = renderApp(home);
    const { runId } = await runOf(api, "Log scans", ASKS, "~/Downloads/Scan.pdf", "waiting");
    window.location.hash = `#/history/${runId}`;
    await screen.findByRole("heading", { name: "Scan.pdf" });

    const waiting = await screen.findByRole("region", { name: "Needs you" });
    expect(waiting).toHaveTextContent("Log Scan?");
    await user.click(within(waiting).getByRole("button", { name: "Skip" }));

    await waitFor(async () => expect((await api.getRun(runId)).status).toBe("done"));
    expect(await screen.findByText("You answered Skip.")).toBeInTheDocument();
  });

  it("says so when the run no longer exists", async () => {
    renderApp(home);
    window.location.hash = "#/history/5d0b6a0e-0000-4000-8000-0000000000aa";
    expect(await screen.findByText("That run no longer exists.")).toBeInTheDocument();
  });
});
