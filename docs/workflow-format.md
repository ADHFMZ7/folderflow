# Workflow format

A workflow is one JSON file, `workflows/<id>.json`, in the app's data folder. The Rust core reads, validates and writes it; the front end edits it through the api. Field names are camelCase.

## File

```json
{
  "version": 1,
  "id": "0f8c2b1e-6b0a-4c1e-9d6a-2f1f6c0f7a11",
  "name": "Receipts and invoices",
  "revision": 3,
  "enabled": false,
  "steps": [ ... ]
}
```

| Field | Meaning |
| --- | --- |
| `version` | Format version. This document is version 1. |
| `id` | A UUID, and nothing else. It becomes the file name, so any other value is refused. |
| `name` | Shown in the app. Not empty. |
| `revision` | Goes up by one on every save. Runs will record the revision they used. |
| `enabled` | Whether the workflow runs. It can't be true while the workflow has problems. |
| `steps` | Every step, in any order. |

## Steps

Every step has:

| Field | Meaning |
| --- | --- |
| `id` | Unique within the workflow, 1 to 32 characters from `A-Z a-z 0-9 _ -`. The editor generates it. |
| `type` | One of the types below. |
| `title` | The sentence shown on the card, e.g. "What kind of document is this?". |
| `position` | `{ "x": number, "y": number }` on the canvas. |

### Where a step goes next

Exits live on the step that owns them, so each exit leads to at most one step and deleting a step takes its exits with it.

- **Plain steps** have `"next": "<step id>" | null`. `null` means the run ends there.
- **Branching steps** (`classify`, `if`, `askMe`) have `"branches": { "<branch id>": "<step id>" }`. A branch with no entry ends the run.
- **`stop`** has no exits.

### Stable branch ids

Branches are named by an id that is set once and never changes; the `label` is only display text. Renaming, reordering or fixing a typo in a label never disconnects anything.

- `classify` categories and `askMe` answers are `{ "id": "c1", "label": "Receipt" }`. The editor generates the id (unique within the step) when the branch is added.
- `if` always has exactly two branch ids: `yes` and `no`.

### Step types

Values in `{braces}` inside text fields are variables (see below).

| Type | Kind | Fields | Exits | Produces |
| --- | --- | --- | --- | --- |
| `fileAdded` | Trigger | `folder` (path), `fileTypes` (lowercase extensions without the dot; empty means any), `subfolders` (bool) | `next` | the file variables |
| `schedule` | Trigger | `schedule`: `{ "every": "day" \| "weekday" \| "week", "time": "HH:MM", "weekday"?: 0-6 }` (`weekday` only with `week`; 0 is Sunday) | `next` | `{date}`, `{year}` |
| `runNow` | Trigger | none | `next` | the file variables |
| `classify` | AI (System 1) | `categories` (at least two), `instructions` (optional hint, may be empty) | `branches` | `{category}` (the chosen label) |
| `extract` | AI (LLM) | `fields`: `[{ "name", "type": "text" \| "number" \| "date" \| "yesNo" }]` (at least one), `ifMissing`: `"review"` \| `"fail"` | `next` | one variable per field |
| `write` | AI (LLM) | `instruction`, `saveAs` (variable name) | `next` | `{<saveAs>}` |
| `agent` | AI (LLM) | `instruction`, `abilities` (e.g. `["readFile"]`), `outputs` (fields, like `extract`) | `next` | one variable per output |
| `rename` | Action | `template` (new name without extension) | `next` | `{newName}` |
| `move` | Action | `to` (folder, may use variables), `mode`: `"move"` \| `"copy"` | `next` | `{newFolder}` |
| `createFile` | Action | `name`, `contents` | `next` | none |
| `tag` | Action | `tags` (at least one) | `next` | none |
| `addRow` | Action | `file` (path to a .csv), `columns` (one text per column, at least one) | `next` | none |
| `notify` | Action | `message` | `next` | none |
| `if` | Logic | `condition`: `{ "left", "op": ">" \| "<" \| ">=" \| "<=" \| "=" \| "!=" \| "contains" \| "startsWith", "right" }` | `branches` (`yes`, `no`) | none |
| `stop` | Logic | none | none | none |
| `askMe` | Human | `question`, `answers` (at least two) | `branches` | `{answer}` (the chosen label) |

AI steps don't name a model. Each uses the default model for its kind (Classify: `system1`; Extract, Write, Agent: `llm`).

## Variables

A text field can use `{name}` for a value produced by a step that always runs before it, or by the trigger:

- File triggers (`fileAdded`, `runNow`) give `{file}` (name without extension), `{extension}`, `{folder}`, `{dateAdded}` and `{year}`.
- `schedule` gives `{date}` and `{year}`.
- Other steps give what the table lists.

Names are 1 to 32 characters from `A-Z a-z 0-9 _`, and field names within one step are unique.

## Problems

Validation returns a list of problems. A workflow with problems can be saved but not turned on.

```json
{ "stepId": "s4", "code": "unknown_variable", "message": "{due_date} isn't produced by any step before this one." }
```

`stepId` is `null` for problems with the whole workflow.

| Code | When |
| --- | --- |
| `no_trigger` | No trigger step |
| `many_triggers` | More than one trigger step |
| `duplicate_id` | Two steps share an id, or two branches in one step do |
| `missing_step` | An exit points at a step that doesn't exist |
| `unknown_branch` | A `branches` key isn't one of the step's branch ids |
| `loop` | Following exits can come back to a step already visited |
| `unreachable` | No path from the trigger reaches the step |
| `required` | A required field is empty, or a list has fewer entries than the table requires |
| `unknown_variable` | A `{name}` isn't produced by the trigger or by every path of steps before this one |
| `no_model` | The step's model kind has no default model |
| `invalid_value` | A value is malformed: a bad variable name, time, weekday or extension |

## Commands

| Api method | Command | Arguments | Returns |
| --- | --- | --- | --- |
| `listWorkflows()` | `list_workflows` | none | `WorkflowSummary[]` |
| `getWorkflow(id)` | `get_workflow` | `id` | `Workflow` |
| `createWorkflow(templateId)` | `create_workflow` | `templateId: string \| null` | `Workflow` (saved, revision 1) |
| `saveWorkflow(workflow)` | `save_workflow` | `workflow` | `{ workflow, problems }` |
| `deleteWorkflow(id)` | `delete_workflow` | `id` | nothing |
| `validateWorkflow(workflow)` | `validate_workflow` | `workflow` | `Problem[]` |

`WorkflowSummary` is `{ id, name, trigger, enabled, lastRun, needsYou, kindsNeeded, status }`:

- `trigger` is a short description such as `"File added · ~/Downloads"`, `"Schedule · Weekdays 09:00"` or `"Run now"`.
- `kindsNeeded` lists the model kinds its AI steps use.
- `status` is `"ok"`, `"damaged"` (the file can't be read; it's kept as is) or `"tooNew"` (written by a newer FolderFlow). For those two, `name` is the file name and the other fields are empty.
- `lastRun` is `null` and `needsYou` is `0` until workflows run.

Rules:

- **Saves never lose someone else's changes.** `save_workflow` fails with `conflict` if the `revision` sent isn't the revision on disk. On success the saved workflow has `revision + 1`.
- **Turning on needs a clean workflow.** Saving with `enabled: true` while there are problems fails with `invalid`, and nothing is written.
- **Newer files are never overwritten**: getting, saving or deleting a `tooNew` workflow fails with `too_new`.
- **Deleting keeps a copy**: the file moves to `workflows/.trash/<id>-<unix time>.json`.
- **Unknown ids** fail with `not_found`; ids that aren't UUIDs fail with `invalid` and never touch the disk.
- `create_workflow(null)` makes a blank workflow named "New workflow" with one `fileAdded` step (folder `~/Downloads`, any file type, no subfolders). A template id makes a copy of that template with a new id.
- `conflict` joins the error codes in `docs/api-contract.md`.

## Drafts

The editor saves automatically. For a workflow that is **off**, it saves straight to the workflow file with `save_workflow`. For a workflow that is **on**, a half-finished change must never run, so edits go to a **draft** instead, and only **Apply** makes them live.

- The draft is `workflows/<id>.draft.json`, in the same format. Its `revision` is the revision of the running workflow it was started from, and its `enabled` always matches the running workflow.
- Only the running workflow runs. A draft never changes the running file or its revision until it's applied.

| Api method | Command | Arguments | Returns |
| --- | --- | --- | --- |
| `getDraft(id)` | `get_draft` | `id` | `Workflow \| null` |
| `saveDraft(workflow)` | `save_draft` | `workflow` | `{ workflow, problems }` (the draft as saved) |
| `applyDraft(id)` | `apply_draft` | `id` | `{ workflow, problems }` (the running workflow, revision + 1) |
| `discardDraft(id)` | `discard_draft` | `id` | nothing |

Rules:

- **`save_draft`** fails with `conflict` if the draft's `revision` isn't the running workflow's, and with `not_found` if the workflow doesn't exist. Problems don't stop a draft from being saved.
- **`apply_draft`** replaces the running workflow with the draft at `revision + 1` and removes the draft. It fails with `not_found` if there's no draft, with `conflict` if the running workflow changed since the draft was started, and with `invalid` if the workflow is on and the draft has problems. When it fails, nothing is written.
- **`discard_draft`** moves the draft to the trash. With no draft it does nothing.
- **A stale draft** (its revision is behind the running workflow, for example after a crash right after applying) is moved to the trash by `get_draft`, which then returns `null`.
- **A damaged or newer draft** is left untouched: `get_draft` fails with `io` or `too_new`. The running workflow still opens.
- **`delete_workflow`** moves the draft to the trash too. Drafts in the trash are named `<id>.draft-<unix time>.json`.
- `WorkflowSummary` gains `hasDraft: boolean`: whether the workflow has changes that aren't live yet.

## Templates

`list_templates` keeps returning `{ id, name, blurb, trigger }`. `create_workflow(templateId)` builds the full workflow:

- `receipts` "Sort receipts": File added (Downloads, pdf) → Classify (Receipt / Other) → Receipt: Extract (date, vendor, amount) → Rename `{date} - {vendor} - {amount}` → Move to `~/Documents/Receipts/{year}`. Other: no exit.
- `screenshots` "Tidy screenshots": File added (Desktop, png) → If `{file}` startsWith `Screenshot` → yes: Move to `~/Pictures/Screenshots/{year}`.
- `summaries` "Summarise PDFs": File added (Downloads, pdf) → Write "Summarise this document in one paragraph." saveAs `summary` → Create file `{file} summary.txt` with contents `{summary}`.
- `invoices` "Log invoices": File added (Downloads, pdf) → Extract (vendor, amount, due) → If `{amount}` > 500 → yes: Ask me "Log {vendor} {amount}?" (Log it / Skip) → Log it: Add row; no: Add row. Add row: `~/Documents/Invoices.csv`, columns `{vendor}`, `{amount}`, `{due}`.
- `cleanup` "Weekly clean-up": Schedule (week, Friday 17:00) → Notify "Time to tidy Downloads."

Every template must validate without problems, except `no_model` when no model is connected.
