import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderApp } from "../../test/render";
import { ASKS, FAILS, runOf } from "./testing";

const home = { settings: { setupComplete: true } };
const needsYou = () => screen.findByRole("region", { name: "Needs you" });

describe("needs you", () => {
  it("asks the question and carries the run on down the answer's branch", async () => {
    const { api, user } = renderApp(home);
    const { runId } = await runOf(api, "Log scans", ASKS, "~/Downloads/Scan.pdf", "waiting");

    const item = within(await needsYou()).getByRole("article", { name: "Log scans: Scan.pdf" });
    expect(item).toHaveTextContent("Log Scan?");
    await user.click(within(item).getByRole("button", { name: "Log it" }));

    await waitFor(async () => expect((await api.getRun(runId)).status).toBe("done"));
    const run = await api.getRun(runId);
    expect(run.steps.map((s) => s.message).filter(Boolean)).toEqual(["You answered Log it.", "Renamed Scan.pdf to Logged Scan.pdf."]);
    await waitFor(() => expect(screen.queryByRole("region", { name: "Needs you" })).not.toBeInTheDocument());
  });

  it("offers Retry, Undo run and Dismiss for a failed run", async () => {
    const { api, user } = renderApp(home);
    const { runId } = await runOf(api, "Sort", FAILS, "~/Downloads/a.pdf", "failed");

    const item = within(await needsYou()).getByRole("article", { name: "Sort: a.pdf" });
    expect(item).toHaveTextContent("Sort it: Classify steps can't run in this version of FolderFlow yet.");
    await user.click(within(item).getByRole("button", { name: "Undo run" }));

    expect(await screen.findByText("Put back 1 change.")).toBeInTheDocument();
    expect((await api.getRun(runId)).status).toBe("undone");
    await waitFor(() => expect(screen.queryByRole("article", { name: "Sort: a.pdf" })).not.toBeInTheDocument());
  });

  it("dismisses a failed run without changing it", async () => {
    const { api, user } = renderApp(home);
    const { runId } = await runOf(api, "Sort", FAILS, "~/Downloads/a.pdf", "failed");

    const item = within(await needsYou()).getByRole("article", { name: "Sort: a.pdf" });
    await user.click(within(item).getByRole("button", { name: "Dismiss" }));

    await waitFor(() => expect(screen.queryByRole("region", { name: "Needs you" })).not.toBeInTheDocument());
    expect((await api.getRun(runId)).status).toBe("failed");
  });

  it("retries a failed run from the step that failed", async () => {
    const { api, user } = renderApp(home);
    const { runId } = await runOf(api, "Sort", FAILS, "~/Downloads/a.pdf", "failed");

    const item = within(await needsYou()).getByRole("article", { name: "Sort: a.pdf" });
    await user.click(within(item).getByRole("button", { name: "Retry" }));

    // The mock still can't classify: it fails again at the same step, and only that step ran again.
    await waitFor(async () => expect((await api.getRun(runId)).steps.map((s) => s.stepId)).toEqual(["t", "r", "c", "c"]));
  });

  it("counts on the sidebar and on the workflow's card", async () => {
    const { api } = renderApp(home);
    await runOf(api, "Log scans", ASKS, "~/Downloads/Scan.pdf", "waiting");

    const nav = screen.getByRole("navigation", { name: "Main" });
    expect(await within(nav).findByText("1")).toBeInTheDocument();
    const card = await screen.findByRole("article", { name: "Log scans" });
    expect(card).toHaveTextContent("1 needs you");
    expect(card).toHaveTextContent("Last run: Scan.pdf · just now · Waiting for you");
  });
});
