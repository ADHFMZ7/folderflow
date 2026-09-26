import { describe, expect, it } from "vitest";
import type { Step } from "../../../api/types";
import { defaultStep } from "../graph";
import { autoTitle, retitle } from "./titles";

const pos = { x: 0, y: 0 };
const fresh = <T extends Step["type"]>(type: T) => defaultStep(type, "s1", pos) as Extract<Step, { type: T }>;
const freshMove = () => fresh("move") as Extract<Step, { type: "move" }>;

describe("autoTitle", () => {
  it.each([
    [{ ...fresh("fileAdded"), folder: "~/Downloads", fileTypes: ["pdf"] }, "When a PDF is added to Downloads"],
    [{ ...fresh("fileAdded"), folder: "~/Desktop", fileTypes: ["png", "jpg"] }, "When a PNG or JPG is added to Desktop"],
    [{ ...fresh("fileAdded"), folder: "~/Documents/Scans", fileTypes: [] }, "When a file is added to Scans"],
    [{ ...fresh("schedule"), schedule: { every: "week", time: "17:00", weekday: 5 } }, "Every Friday at 17:00"],
    [{ ...fresh("schedule"), schedule: { every: "weekday", time: "09:00" } }, "Every weekday at 09:00"],
    [{ ...fresh("extract"), fields: [{ name: "date", type: "date" }, { name: "vendor", type: "text" }, { name: "dueDate", type: "date" }] }, "Pull out the date, vendor and due date"],
    [{ ...fresh("rename"), template: "{date} {vendor}" }, "Rename to {date} {vendor}"],
    [{ ...fresh("move"), to: "~/Documents/Receipts/{year}", mode: "copy" }, "Copy to Receipts/{year}"],
    [{ ...fresh("addRow"), file: "~/Documents/Expenses.csv" }, "Add a row to Expenses.csv"],
    [{ ...fresh("tag"), tags: ["Receipt", "Tax"] }, "Tag it Receipt and Tax"],
    [{ ...fresh("write"), saveAs: "summary" }, "Write the summary"],
    [{ ...fresh("createFile"), name: "{file} notes.md" }, "Create {file} notes.md"],
    [{ ...fresh("classify"), categories: [{ id: "a", label: "Receipt" }, { id: "b", label: "Invoice" }] }, "Receipt or Invoice?"],
    [{ ...fresh("if"), condition: { left: "{amount}", op: ">", right: "500" } }, "Is {amount} more than 500?"],
  ] as [Step, string][])("%# titles a step from its settings", (step, title) => {
    expect(autoTitle(step)).toBe(title);
  });

  it("falls back to the new step's title while the settings are empty", () => {
    expect(autoTitle({ ...fresh("move"), to: "" })).toBe(fresh("move").title);
  });

  it("leaves steps it can't sum up alone", () => {
    expect(autoTitle(fresh("agent"))).toBeNull();
    expect(autoTitle(fresh("notify"))).toBeNull();
  });
});

describe("retitle", () => {
  it("keeps a fresh step's title in step with its settings", () => {
    const step = freshMove();
    const once = retitle(step, { ...step, to: "~/Documents/Receipts" });
    expect(once.title).toBe("Move to Documents/Receipts");
    const twice = retitle(once, { ...once, to: "~/Documents/Invoices" });
    expect(twice.title).toBe("Move to Documents/Invoices");
  });

  it("never touches a title the user wrote", () => {
    const step = { ...freshMove(), title: "File it away" };
    expect(retitle(step, { ...step, to: "~/Documents" }).title).toBe("File it away");
  });

  it("stops following the settings once the user edits the title", () => {
    const step = freshMove();
    const edited = retitle(step, { ...step, title: "Put it somewhere" });
    expect(edited.title).toBe("Put it somewhere");
    expect(retitle(edited, { ...edited, to: "~/Pictures" }).title).toBe("Put it somewhere");
  });
});
