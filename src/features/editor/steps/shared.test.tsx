// What every step form shares: inserting details, previews, problems next to
// their field, titles that follow the settings, and where AI steps run.

import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { Step } from "../../../api/types";
import { canvas, fileTrigger, inspector, openWith, saved, selectStep } from "./testing";

const rename = (template: string, title = "Rename the file"): Step => ({ id: "r", type: "rename", title, position: { x: 0, y: 160 }, template, next: null });
const extract: Step = {
  id: "e", type: "extract", title: "Get details", position: { x: 0, y: 160 }, ifMissing: "review", next: "r",
  fields: [{ name: "vendor", type: "text" }, { name: "amount", type: "number" }],
};

describe("details in text fields", () => {
  it("inserts a detail from the list, grouped by where it comes from", async () => {
    const { user, api, wf } = await openWith([fileTrigger("e"), extract, { ...rename(""), position: { x: 0, y: 320 } }], "Rename the file");
    const name = screen.getByRole("textbox", { name: "New name" });

    await user.click(screen.getByRole("button", { name: "Insert a detail into New name" }));
    const list = screen.getByRole("listbox", { name: "Details you can use" });
    expect(within(list).getByRole("group", { name: "Get details" })).toBeInTheDocument();
    expect(within(list).getByRole("group", { name: "This file" })).toBeInTheDocument();
    await user.click(within(list).getByRole("option", { name: "vendor" }));

    expect(name).toHaveValue("{vendor}");
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    await saved<"rename">(api, wf, "r", (s) => expect(s.template).toBe("{vendor}"));
  });

  it("offers matching details as you type after {", async () => {
    const { user } = await openWith([fileTrigger("r"), rename("")], "Rename the file");
    const name = screen.getByRole("textbox", { name: "New name" });

    await user.type(name, "Scan {{fi");
    const list = screen.getByRole("listbox", { name: "Details you can use" });
    expect(within(list).getAllByRole("option").map((o) => o.getAttribute("aria-label"))).toEqual(["File name"]);
    await user.keyboard("{Enter}");

    expect(name).toHaveValue("Scan {file}");
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });

  it("closes the suggestions on Escape and keeps what was typed", async () => {
    const { user } = await openWith([fileTrigger("r"), rename("")], "Rename the file");
    const name = screen.getByRole("textbox", { name: "New name" });
    await user.type(name, "{{ye");
    expect(screen.getByRole("listbox")).toBeInTheDocument();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    expect(name).toHaveValue("{ye");
  });

  it("flags a detail nothing before the step gives, as you type", async () => {
    const { user } = await openWith([fileTrigger("r"), rename("")], "Rename the file");
    const name = screen.getByRole("textbox", { name: "New name" });
    await user.type(name, "{{vendor}");
    expect(name).toHaveAccessibleDescription(expect.stringContaining("Nothing before this step gives {vendor}."));
  });

  it("shows an example of the result", async () => {
    await openWith([fileTrigger("e"), extract, { ...rename("{vendor} {amount}"), position: { x: 0, y: 320 } }], "Rename the file");
    expect(within(inspector()).getByText("Example: vendor 42.50.pdf")).toBeInTheDocument();
  });

  it("says a step no path reaches can't use earlier details", async () => {
    const { user } = await openWith([fileTrigger(null), rename("")], "Rename the file");
    await user.click(screen.getByRole("button", { name: "Insert a detail into New name" }));
    expect(screen.getByText("Connect this step to use details from earlier steps.")).toBeInTheDocument();
  });

  it("asks to connect a step no path reaches, rather than blaming earlier steps", async () => {
    const { user } = await openWith([fileTrigger(null), rename("")], "Rename the file");
    const name = screen.getByRole("textbox", { name: "New name" });
    await user.type(name, "{{year}");
    expect(name).toHaveAccessibleDescription(expect.stringContaining("Connect this step to the workflow to use {year}."));
    expect(name).not.toHaveAccessibleDescription(expect.stringContaining("Nothing before this step"));
  });
});

describe("problems", () => {
  it("shows a problem next to the field it's about", async () => {
    await openWith([fileTrigger("r", { folder: "" }), rename("{file}")], "When a file is added");
    await waitFor(() => expect(screen.getByRole("textbox", { name: "Folder to watch" }))
      .toHaveAccessibleDescription(expect.stringContaining("Fill in the folder to watch.")));
  });

  it("lists problems that aren't about one field at the top", async () => {
    await openWith([fileTrigger("r"), { ...rename("{file}"), next: "gone" } as Step], "Rename the file");
    const list = await screen.findByRole("list", { name: "Problems with this step" });
    expect(list).toHaveTextContent("leads to a step that no longer exists");
  });
});

describe("titles", () => {
  it("follows the settings until you write your own", async () => {
    const { user } = await openWith([fileTrigger("r"), rename("")], "Rename the file");
    const name = screen.getByRole("textbox", { name: "New name" });
    await user.type(name, "Old");
    expect(within(canvas()).getByText("Rename to Old")).toBeInTheDocument();

    const title = screen.getByRole("textbox", { name: "Title" });
    await user.clear(title);
    await user.type(title, "Mark it old");
    await user.type(name, "er");
    expect(within(canvas()).getByText("Mark it old")).toBeInTheDocument();
    expect(title).toHaveValue("Mark it old");
  });

  it("leaves a template's own title alone", async () => {
    const { user } = await openWith([fileTrigger("r"), rename("{file}", "Tidy the name")], "Tidy the name");
    await user.type(screen.getByRole("textbox", { name: "New name" }), "!");
    expect(within(canvas()).getByText("Tidy the name")).toBeInTheDocument();
  });
});

describe("runs on", () => {
  const write: Step = { id: "w", type: "write", title: "Write the summary", position: { x: 0, y: 160 }, instruction: "Summarise it.", saveAs: "summary", next: null };

  it("names the model an AI step uses and where the file goes", async () => {
    await openWith([fileTrigger("w"), write], "Write the summary", {
      settings: {
        setupComplete: true, connections: [{ id: "k1", providerId: "anthropic" }],
        defaults: { llm: { connectionId: "k1", modelId: "claude" } },
      },
    });
    const line = await within(inspector()).findByText(/Runs on Claude from Anthropic/);
    expect(line).toHaveTextContent("Files your workflows run on are sent to Anthropic.");
  });

  it("points to Settings when there's no model for it", async () => {
    await openWith([fileTrigger("w"), write], "Write the summary");
    expect(await within(inspector()).findByText(/No default LLM model yet/)).toBeInTheDocument();
    expect(within(inspector()).getByRole("link", { name: "Choose one in Settings" })).toHaveAttribute("href", "#/settings");
  });

  it("isn't shown for steps without a model", async () => {
    await openWith([fileTrigger("r"), rename("{file}")], "Rename the file");
    expect(within(inspector()).queryByText(/Runs on|No default/)).not.toBeInTheDocument();
  });
});

describe("switching steps", () => {
  it("shows the selected step's settings", async () => {
    await openWith([fileTrigger("r"), rename("{file}")], "Rename the file");
    selectStep("When a file is added");
    expect(await screen.findByRole("textbox", { name: "Folder to watch" })).toHaveValue("~/Downloads");
  });
});
