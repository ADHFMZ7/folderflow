// Each step type's settings, edited as a person would and saved as the format
// says (docs/workflow-format.md, docs/step-settings.md).

import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { Step } from "../../../api/types";
import { fileTrigger, inspector, openWith, saved } from "./testing";

const at = (row: number) => ({ x: 0, y: row * 160 });

describe("File added", () => {
  it("chooses the folder with the picker, and the file types with presets or by typing", async () => {
    const { user, api, wf } = await openWith([fileTrigger(null)], "When a file is added", { chosenFolder: "~/Documents/Scans" });

    await user.click(screen.getByRole("button", { name: "Choose a folder for Folder to watch" }));
    expect(screen.getByRole("textbox", { name: "Folder to watch" })).toHaveValue("~/Documents/Scans");

    await user.click(screen.getByRole("button", { name: "PDFs" }));
    await user.type(screen.getByRole("textbox", { name: "Add a file type" }), ".HEIC{Enter}");
    expect(screen.getByRole("button", { name: "Remove pdf" })).toBeInTheDocument();
    await user.click(screen.getByRole("switch", { name: "Include files in subfolders" }));

    await saved<"fileAdded">(api, wf, "t", (s) => expect(s).toMatchObject({ folder: "~/Documents/Scans", fileTypes: ["pdf", "heic"], subfolders: true }));
  });

  it("keeps what's typed when the picker is cancelled", async () => {
    const { user } = await openWith([fileTrigger(null)], "When a file is added", { chosenFolder: null });
    await user.click(screen.getByRole("button", { name: "Choose a folder for Folder to watch" }));
    expect(screen.getByRole("textbox", { name: "Folder to watch" })).toHaveValue("~/Downloads");
  });

  it("goes back to any file with Any file, and removes a type", async () => {
    const { user, api, wf } = await openWith([fileTrigger(null, { fileTypes: ["pdf", "png"] })], "When a file is added");
    await user.click(screen.getByRole("button", { name: "Remove png" }));
    await saved<"fileAdded">(api, wf, "t", (s) => expect(s.fileTypes).toEqual(["pdf"]));
    await user.click(screen.getByRole("button", { name: "Any file" }));
    await saved<"fileAdded">(api, wf, "t", (s) => expect(s.fileTypes).toEqual([]));
  });
});

describe("Schedule", () => {
  const schedule: Step = { id: "t", type: "schedule", title: "Every weekday at 09:00", position: at(0), schedule: { every: "weekday", time: "09:00" }, next: null };

  it("asks for a day only for weekly runs", async () => {
    const { user, api, wf } = await openWith([schedule], "Every weekday at 09:00");
    expect(screen.queryByRole("combobox", { name: "Day" })).not.toBeInTheDocument();

    await user.click(screen.getByRole("radio", { name: "Once a week" }));
    expect(screen.getByRole("combobox", { name: "Day" })).toHaveValue("5");
    await user.selectOptions(screen.getByRole("combobox", { name: "Day" }), "Monday");
    await saved<"schedule">(api, wf, "t", (s) => expect(s.schedule).toEqual({ every: "week", time: "09:00", weekday: 1 }));

    await user.click(screen.getByRole("radio", { name: "Every day" }));
    await saved<"schedule">(api, wf, "t", (s) => expect(s.schedule).toEqual({ every: "day", time: "09:00" }));
  });
});

describe("Run now and Stop", () => {
  it("explain themselves, with nothing to set", async () => {
    await openWith([{ id: "t", type: "runNow", title: "When I run it", position: at(0), next: "x" },
      { id: "x", type: "stop", title: "Stop", position: at(1) }], "When I run it");
    expect(within(inspector()).getByText(/pick files and choose Run/)).toBeInTheDocument();
  });
});

describe("Classify", () => {
  const classify: Step = {
    id: "c", type: "classify", title: "What is it?", position: at(1), instructions: "",
    categories: [{ id: "c1", label: "Receipt" }, { id: "c2", label: "Invoice" }], branches: { c1: "x" },
  };

  it("adds categories with a description, and keeps at least two", async () => {
    const { user, api, wf } = await openWith([fileTrigger("c"), classify, { id: "x", type: "stop", title: "Stop", position: at(2) }], "What is it?");
    expect(screen.getByRole("button", { name: "Remove category 1" })).toBeDisabled();
    expect(within(inspector()).getByText(/Add one like .Something else./)).toBeInTheDocument();

    await user.type(screen.getByRole("textbox", { name: "Category 1: how to recognise it" }), "Proof I paid");
    await user.click(screen.getByRole("button", { name: "Add a category" }));
    await user.type(screen.getByRole("textbox", { name: "Category 3 name" }), "Something else");
    expect(within(inspector()).queryByText(/Add one like/)).not.toBeInTheDocument();
    await user.type(screen.getByRole("textbox", { name: "Anything else it should know" }), "From work email.");

    await saved<"classify">(api, wf, "c", (s) => {
      expect(s.categories.map((c) => c.label)).toEqual(["Receipt", "Invoice", "Something else"]);
      expect(s.categories[0].description).toBe("Proof I paid");
      expect(new Set(s.categories.map((c) => c.id)).size).toBe(3);
      expect(s.instructions).toBe("From work email.");
      expect(s.branches).toEqual({ c1: "x" });
    });
  });

  it("reorders and removes categories, and removing one drops its link", async () => {
    const three = { ...classify, categories: [...(classify as Extract<Step, { type: "classify" }>).categories, { id: "c3", label: "Other" }] } as Step;
    const { user, api, wf } = await openWith([fileTrigger("c"), three, { id: "x", type: "stop", title: "Stop", position: at(2) }], "What is it?");
    await user.click(screen.getByRole("button", { name: "Move category 3 up" }));
    await user.click(screen.getByRole("button", { name: "Remove category 1" }));
    await saved<"classify">(api, wf, "c", (s) => {
      expect(s.categories.map((c) => c.label)).toEqual(["Other", "Invoice"]);
      expect(s.branches).toEqual({});
    });
  });
});

describe("Extract", () => {
  const extract: Step = { id: "e", type: "extract", title: "Get details", position: at(1), ifMissing: "review", next: null, fields: [{ name: "vendor", type: "text" }] };

  it("adds a detail with a kind and what to look for, making its name usable", async () => {
    const { user, api, wf } = await openWith([fileTrigger("e"), extract], "Get details");
    await user.click(screen.getByRole("button", { name: "Add a detail" }));
    await user.type(screen.getByRole("textbox", { name: "Detail 2 name" }), "Due date!");
    expect(screen.getByRole("textbox", { name: "Detail 2 name" })).toHaveValue("Due_date");
    await user.selectOptions(screen.getByRole("combobox", { name: "Detail 2 kind" }), "Date");
    await user.type(screen.getByRole("textbox", { name: "Detail 2: what to look for" }), "When payment is due");
    await user.click(screen.getByRole("radio", { name: "Stop the run" }));

    await saved<"extract">(api, wf, "e", (s) => {
      expect(s.fields).toEqual([{ name: "vendor", type: "text" }, { name: "Due_date", type: "date", description: "When payment is due" }]);
      expect(s.ifMissing).toBe("fail");
    });
  });
});

describe("Write", () => {
  it("starts from a suggestion and saves the text under a usable name", async () => {
    const write: Step = { id: "w", type: "write", title: "Write the text", position: at(1), instruction: "", saveAs: "text", next: null };
    const { user, api, wf } = await openWith([fileTrigger("w"), write], "Write the text");
    await user.click(screen.getByRole("button", { name: "A one-paragraph summary" }));
    expect(screen.queryByRole("button", { name: "A one-paragraph summary" })).not.toBeInTheDocument();
    const saveAs = screen.getByRole("textbox", { name: "Save the text as" });
    await user.clear(saveAs);
    await user.type(saveAs, "short summary");

    await saved<"write">(api, wf, "w", (s) => {
      expect(s.instruction).toMatch(/one paragraph/);
      expect(s.saveAs).toBe("short_summary");
    });
  });
});

describe("Agent step", () => {
  it("may read the file by default, and hands back details", async () => {
    const agent: Step = { id: "a", type: "agent", title: "Look into it", position: at(1), instruction: "", abilities: ["readFile"], outputs: [], next: null };
    const { user, api, wf } = await openWith([fileTrigger("a"), agent], "Look into it");
    expect(screen.getByRole("checkbox", { name: "Read the file's contents" })).toBeChecked();
    await user.type(screen.getByRole("textbox", { name: "What should it do?" }), "Find who it's with.");
    await user.click(screen.getByRole("button", { name: "Add a detail" }));
    await user.type(screen.getByRole("textbox", { name: "Detail 1 name" }), "party");
    await user.click(screen.getByRole("checkbox", { name: "Read the file's contents" }));

    await saved<"agent">(api, wf, "a", (s) => {
      expect(s).toMatchObject({ instruction: "Find who it's with.", abilities: [], outputs: [{ name: "party", type: "text" }] });
    });
  });
});

describe("Move / Copy", () => {
  it("copies into a folder with details, showing where the file ends up", async () => {
    const move: Step = { id: "m", type: "move", title: "Move the file", position: at(1), to: "", mode: "move", next: null };
    const { user, api, wf } = await openWith([fileTrigger("m"), move], "Move the file", { chosenFolder: "~/Documents/Receipts" });
    await user.click(screen.getByRole("radio", { name: "Copy" }));
    await user.click(screen.getByRole("button", { name: "Choose a folder for To folder" }));
    await user.type(screen.getByRole("textbox", { name: "To folder" }), "/{{year}");
    expect(within(inspector()).getByText(`Example: ~/Documents/Receipts/${new Date().getFullYear()}`)).toBeInTheDocument();

    await saved<"move">(api, wf, "m", (s) => expect(s).toMatchObject({ to: "~/Documents/Receipts/{year}", mode: "copy" }));
  });
});

describe("Create file", () => {
  it("names the file, and puts it in the file's folder unless told otherwise", async () => {
    const create: Step = { id: "f", type: "createFile", title: "Create a file", position: at(1), name: "", contents: "", next: null };
    const { user, api, wf } = await openWith([fileTrigger("f"), create], "Create a file");
    expect(screen.getByRole("textbox", { name: "In folder" })).toHaveAttribute("placeholder", "The file's folder");
    await user.type(screen.getByRole("textbox", { name: "File name" }), "{{file} notes.md");
    await user.type(screen.getByRole("textbox", { name: "Contents" }), "Notes for {{file}");
    await saved<"createFile">(api, wf, "f", (s) => {
      expect(s).toMatchObject({ name: "{file} notes.md", contents: "Notes for {file}" });
      expect(s.folder ?? "").toBe("");
    });

    await user.type(screen.getByRole("textbox", { name: "In folder" }), "~/Documents/Notes");
    await saved<"createFile">(api, wf, "f", (s) => expect(s.folder).toBe("~/Documents/Notes"));
  });
});

describe("Tag", () => {
  it("adds and removes tags, keeping at least one", async () => {
    const tag: Step = { id: "g", type: "tag", title: "Add tags", position: at(1), tags: [""], next: null };
    const { user, api, wf } = await openWith([fileTrigger("g"), tag], "Add tags");
    expect(screen.getByRole("button", { name: "Remove tag 1" })).toBeDisabled();
    await user.type(screen.getByRole("textbox", { name: "Tag 1" }), "Receipt");
    await user.click(screen.getByRole("button", { name: "Add a tag" }));
    await user.type(screen.getByRole("textbox", { name: "Tag 2" }), "{{year}");
    await saved<"tag">(api, wf, "g", (s) => expect(s.tags).toEqual(["Receipt", "{year}"]));
  });
});

describe("Add row", () => {
  it("pairs each column's heading with its value", async () => {
    const row: Step = { id: "r", type: "addRow", title: "Add a row", position: at(1), file: "", columns: [""], next: null };
    const { user, api, wf } = await openWith([fileTrigger("r"), row], "Add a row", { chosenCsv: "~/Documents/Expenses.csv" });
    await user.click(screen.getByRole("button", { name: "Choose a spreadsheet for Spreadsheet" }));
    await user.type(screen.getByRole("textbox", { name: "Column 1 heading" }), "File");
    await user.type(screen.getByRole("textbox", { name: "Column 1 value" }), "{{file}");
    await user.click(screen.getByRole("button", { name: "Add a column" }));
    await user.type(screen.getByRole("textbox", { name: "Column 2 value" }), "{{dateAdded}");

    await saved<"addRow">(api, wf, "r", (s) => {
      expect(s.file).toBe("~/Documents/Expenses.csv");
      expect(s.columns).toEqual(["{file}", "{dateAdded}"]);
      expect(s.headers).toEqual(["File", ""]);
    });

    await user.click(screen.getByRole("button", { name: "Remove column 1" }));
    await saved<"addRow">(api, wf, "r", (s) => expect(s).toMatchObject({ columns: ["{dateAdded}"], headers: [""] }));
  });
});

describe("Notify", () => {
  it("previews the message", async () => {
    const notify: Step = { id: "n", type: "notify", title: "Notify me", position: at(1), message: "", next: null };
    const { user, api, wf } = await openWith([fileTrigger("n"), notify], "Notify me");
    await user.type(screen.getByRole("textbox", { name: "Message" }), "Filed {{file}.");
    expect(within(inspector()).getByText("Example: Filed Scan_0042.")).toBeInTheDocument();
    await saved<"notify">(api, wf, "n", (s) => expect(s.message).toBe("Filed {file}."));
  });
});

describe("If", () => {
  const extract: Step = { id: "e", type: "extract", title: "Get details", position: at(1), ifMissing: "review", next: "i",
    fields: [{ name: "amount", type: "number" }, { name: "paid", type: "yesNo" }] };
  const check: Step = { id: "i", type: "if", title: "Check a value", position: at(2), condition: { left: "", op: "=", right: "" }, branches: {} };

  it("compares a detail in plain words", async () => {
    const { user, api, wf } = await openWith([fileTrigger("e"), extract, check], "Check a value");
    await user.selectOptions(screen.getByRole("combobox", { name: "Compare" }), "amount");
    await user.selectOptions(screen.getByRole("combobox", { name: "Comparison" }), "is more than");
    await user.type(screen.getByRole("textbox", { name: "Value" }), "500");
    await saved<"if">(api, wf, "i", (s) => expect(s.condition).toEqual({ left: "{amount}", op: ">", right: "500" }));
  });

  it("offers Yes and No for a yes-or-no detail", async () => {
    const { user, api, wf } = await openWith([fileTrigger("e"), extract, check], "Check a value");
    await user.selectOptions(screen.getByRole("combobox", { name: "Compare" }), "paid");
    await user.click(screen.getByRole("radio", { name: "No" }));
    await saved<"if">(api, wf, "i", (s) => expect(s.condition).toEqual({ left: "{paid}", op: "=", right: "no" }));
  });

  it("compares custom text too", async () => {
    const { user, api, wf } = await openWith([fileTrigger("e"), extract, check], "Check a value");
    await user.selectOptions(screen.getByRole("combobox", { name: "Compare" }), "Custom text…");
    await user.type(screen.getByRole("textbox", { name: "Custom text" }), "{{file} ok");
    await saved<"if">(api, wf, "i", (s) => expect(s.condition.left).toBe("{file} ok"));
  });
});

describe("Ask me", () => {
  it("asks a question with at least two answers", async () => {
    const ask: Step = { id: "q", type: "askMe", title: "Check with me", position: at(1), question: "", branches: {},
      answers: [{ id: "a1", label: "Yes" }, { id: "a2", label: "No" }] };
    const { user, api, wf } = await openWith([fileTrigger("q"), ask], "Check with me");
    await user.type(screen.getByRole("textbox", { name: "Question" }), "File {{file}?");
    expect(within(inspector()).getByText("Example: File Scan_0042?")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Remove answer 2" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Add an answer" }));
    await user.type(screen.getByRole("textbox", { name: "Answer 3" }), "Later");
    await saved<"askMe">(api, wf, "q", (s) => {
      expect(s.question).toBe("File {file}?");
      expect(s.answers.map((a) => a.label)).toEqual(["Yes", "No", "Later"]);
    });
  });
});
