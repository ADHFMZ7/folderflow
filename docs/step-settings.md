# Step settings

What the inspector shows for each step type: the controls, how people give AI steps the context they need, how variables are offered, and the format changes this needs. The format itself is in `docs/workflow-format.md`.

The people we design for are Mac users who are not programmers. Labels say what happens to their files, never how. No "prompt", "schema", "regex", "variable" or "null" on screen; we say "details", "what to look for", "insert", "ends the run".

## Use cases

| # | Scenario | Workflow | Step types |
| --- | --- | --- | --- |
| 1 | **Receipts** from email and shops land in Downloads as PDFs | File added (Downloads, pdf) → Classify (Receipt / Something else) → Extract (date, vendor, amount) → Rename `{date} {vendor} {amount}` → Move `~/Documents/Receipts/{year}` → Add row to `Expenses.csv` | fileAdded, classify, extract, rename, move, addRow |
| 2 | **Invoices to pay**, checking big ones first | Extract (vendor, amount, due) → If `{amount}` is more than 1000 → Ask me "Invoice from {vendor} for {amount}. File it?" → Add row; Notify "{vendor} is due {due}" | extract, if, askMe, addRow, notify |
| 3 | **Screenshots** piling up on the Desktop | File added (Desktop, png) → If `{file}` starts with "Screenshot" → Rename `{dateAdded} screenshot` → Tag "Screenshot" → Move `~/Pictures/Screenshots/{year}` | fileAdded, if, rename, tag, move |
| 4 | **Downloads clean-up**: installers, archives and documents sorted as they arrive, plus a weekly reminder | File added (Downloads, any) → Classify (App installer / Archive / Document / Image) → Move to `~/Downloads/{category}`. Separate workflow: Schedule (Fridays 17:00) → Notify | fileAdded, classify, move, schedule, notify |
| 5 | **Photo sorting**: phone photos of paper (whiteboards, letters) mixed with real photos | File added (`~/Pictures/Imports`, jpg heic, with subfolders) → Classify (Photo of a document / Photo) → document: Write "a short title of a few words" as `title` → Rename `{dateAdded} {title}` → Move `~/Documents/Scans`; photo: Move `~/Pictures/{year}` | fileAdded, classify, write, rename, move |
| 6 | **Meeting notes** from transcripts saved by Zoom or Teams | File added (`~/Documents/Zoom`, txt vtt docx) → Extract (meetingDate, topic) → Write "the decisions and action items, as a bulleted list" as `notes` → Create file `{meetingDate} {topic}.md` in `~/Documents/Meeting notes` → Notify | fileAdded, extract, write, createFile, notify |
| 7 | **Contracts and leases**: know when they end and whether they renew | Classify (Contract / Other) → Agent "note who it's with, when it ends, whether it renews by itself, anything unusual" with outputs party, endDate, autoRenews → Rename `{party} contract` → Move → If `{autoRenews}` is yes → Notify "{party} renews on {endDate}" | classify, agent, rename, move, if, notify |
| 8 | **School work**: a student's downloads sorted by course | File added (Downloads, pdf docx pptx) → Classify (Biology / History / Maths / Not school), each with a description of the course → Rename `{category} - {file}` → Tag `{category}` → Move `~/Documents/School/{category}` | classify, rename, tag, move |
| 9 | **Scanned mail** from a scanner app, where only the person knows where it goes | Run now (chosen files) → Write "a short title" as `title` → Ask me "Where does '{title}' go?" (Taxes / Medical / Home / Leave it) → Taxes etc.: Move `~/Documents/{answer}`; Leave it: Stop | runNow, write, askMe, move, stop |
| 10 | **Job applications**: logging every offer letter and CV sent | Extract (company, role, date) → Add row `Applications.csv` (Company, Role, Date, File) → Tag "Applied" | extract, addRow, tag |

What these teach us:

- **Classify lives or dies by its categories.** "Other", "Personal", "Biology" mean little to a model without a sentence on what belongs there (scenarios 4, 5, 8). Categories need a description.
- **Extract needs "what to look for"**, not just a name: "amount" could be the subtotal, tax or total (1, 2, 6). Fields need a description.
- **Paths use variables** (`{year}`, `{category}`, `{answer}`): the move form must accept text and show where a file would end up.
- **A CSV nobody can read is useless**: rows need column headings (1, 10).
- **Create file needs a folder** other than "next to the file" (6).
- **Merging branches** (2, 7) is where variables disappear; the form must show only what every path provides.

## The inspector

Top to bottom, for every step:

1. **Kind line**: glyph and label ("AI · Extract"), as on the card.
2. **Title**: the card's sentence. Plain text.
3. **Problems banner** for problems that don't belong to a field (see Validation).
4. **The step's form** (below). Fields in the order the user thinks about them.
5. **For AI steps, a "Runs on" line**: "Your default LLM: Claude Sonnet (Anthropic). This file's contents are sent to Anthropic." or "…runs on this Mac; nothing leaves it." Links to Settings when there's no default.
6. **Delete step**.

Every edit calls `onChange(wholeNewStep)`; nothing else changes. Forms are controlled and stateless apart from UI state (an open autocomplete, an expanded row).

## Controls

Shared controls, built once in `src/features/editor/steps/` (editor-specific) or `src/ui/` (generic):

| Control | Used by | Behaviour |
| --- | --- | --- |
| **TextArea** (`src/ui`) | many | Multi-line text, token styled like TextInput. |
| **VariableText** | every text field that takes `{…}` | A text input or textarea, plus an **Insert** button that opens the list of available details grouped by where they come from ("This file", "Pull out the details"). Typing `{` opens the same list as autocomplete (ARIA combobox; arrows, Enter, Esc). Picking inserts `{name}` at the caret. A `{name}` that isn't available is flagged under the field at once, before Rust answers. |
| **Preview** | rename, move, createFile, notify, askMe | "Example: 2026-09-14 Blue Bottle 4.50.pdf". Fills variables with sample values (below). |
| **ListEditor** | categories, fields, answers, tags, columns, file types | Rows with a remove button (disabled at the minimum count), "Add …" at the bottom, move up/down buttons. New rows focus their first input. |
| **NameInput** | extract/agent field names, write's "save as" | Turns what's typed into a valid name as you type: spaces become `_`, other characters are dropped, 32 max. Shows the result as a chip: `{due_date}`. |
| **FolderField** | fileAdded.folder, move.to, createFile.folder | A VariableText plus **Choose…** that opens the native folder picker (see Pickers). |
| **FileTypes** | fileAdded.fileTypes | Chips for extensions, with presets: Any file, PDFs, Images (png jpg jpeg heic gif webp), Documents (pdf doc docx pages txt rtf md), Spreadsheets (csv xls xlsx numbers). Typing ".PDF" stores `pdf`. |
| **Segmented**, **Select**, **Toggle** (`src/ui`, existing) | modes and small choices | |

## Step forms

Labels are what the user sees. Defaults are what `defaultStep` in graph.ts already creates unless noted.

### Triggers

| Step | Controls | Helper text |
| --- | --- | --- |
| **File added** | **Folder to watch**: FolderField (`~/Downloads`). **Which files**: FileTypes (any). **Include files in subfolders**: Toggle (off). | "Runs once for each new file. Files FolderFlow creates itself never start it." Lists the details it provides: "Gives the next steps: the file's name, extension, folder, the date it was added, and the year." |
| **Schedule** | **How often**: Segmented Every day / Weekdays / Once a week. **Day**: Select Sunday…Saturday, only for "Once a week" (switching to weekly sets Friday; switching away removes `weekday`). **Time**: `<input type="time">` (HH:MM, 24-hour stored). | "Gives the next steps today's date and the year. There's no file in a scheduled run." |
| **Run now** | None. | "Runs when you pick files and choose Run. Gives the same details as File added." |

### AI steps

What makes them work well, and how we ask for it without jargon:

| Step | What the model needs | How we ask |
| --- | --- | --- |
| **Classify** | Distinct categories, each with what belongs in it; a catch-all so nothing is forced into the wrong one; optional wider context. | Each category is a row: **Name** ("Receipt") and **How to recognise it** ("Proof of a payment I made: shop receipts, order confirmations, card slips"). A tip under the list when no category looks like a catch-all: "Add one like 'Something else' so files that don't fit aren't forced into a category." **Anything else it should know** (VariableText, optional): "These are from my work email; anything from Acme is an invoice." Each category is an exit on the card, so adding one adds a branch. |
| **Extract** | Each detail's name, its kind (so dates and numbers come back usable), and which of several candidates to take. | Rows: **Detail** (NameInput: `vendor`), **Kind** (Select: Text, Number, Date, Yes or no), **What to look for** ("The shop or company I paid, not the payment processor"). **If a detail can't be found**: Segmented "Ask me to check it" (review) / "Stop the run" (fail), with "Nothing is renamed, moved or logged with a missing detail." |
| **Write** | What to write, how long, in what tone and form; a name for the result. | **What should it write?** (VariableText, textarea). Starters shown while it's empty, each fills the box: "A one-paragraph summary", "The action items, as a bulleted list", "A short title of a few words", "A polite reply". Helper: "Say how long and in what tone, e.g. 'three bullet points, friendly'." **Save the text as**: NameInput (`text` → shown as `{text}`), "Use it later as {text}." |
| **Agent step** | A clear goal, what it may do, and what it must hand back. | **What should it do?** (VariableText, textarea). **It may**: checkboxes from a known list; today only "Read the file's contents" (`readFile`). Unknown abilities in a file are kept and shown as "Other: x". **What should it hand back?** The same rows as Extract (optional). Helper: "Use this when no other step fits. For pulling out details, Extract is faster and checks the results." |

### Actions

| Step | Controls | Helper text / preview |
| --- | --- | --- |
| **Rename** | **New name**: VariableText (`{file}`). | Preview "Example: 2026-09-14 Blue Bottle 4.50.pdf". "The extension stays the same. If the name is taken, a number is added so nothing is overwritten." Flags `/` and `:` typed in the text. |
| **Move / Copy** | **Move or copy**: Segmented Move / Copy. **To folder**: FolderField. | Preview "Example: ~/Documents/Receipts/2026". "Folders that don't exist yet are created." |
| **Create file** | **File name**: VariableText. **In folder**: FolderField, empty means "the file's folder" (new field, see Format changes). **Contents**: VariableText textarea. | "Include the extension, like .txt or .md." Preview of the name. |
| **Tag** | **Tags**: ListEditor of VariableText rows (at least one). | "Finder tags. `{category}` tags each file with its category." |
| **Add row** | **Spreadsheet**: FolderField-like file field with **Choose…** (save dialog, .csv). **Columns**: ListEditor rows of **Heading** + **Value** (VariableText). | "A new file starts with a row of headings. Values go in the order shown." Preview of one row. |
| **Notify** | **Message**: VariableText. | Preview. |

### Logic and human

| Step | Controls | Helper text |
| --- | --- | --- |
| **If** | A sentence: **If** [left: Select of available details, or "Custom text…" which reveals a VariableText] [op: Select] [right: VariableText]. Ops in plain words: is (=), is not (!=), is more than (>), is less than (<), is at least (>=), is at most (<=), contains, starts with. For a Number or Date detail the comparisons come first; for Yes or no the right side becomes a Yes / No Segmented. | "Yes and No are the two exits on the card." |
| **Stop** | None. | "The run ends here. Use it to make an ending obvious; a step with nothing after it ends too." |
| **Ask me** | **Question**: VariableText. **Answers**: ListEditor (at least two; each an exit). | "The run waits here until you answer in FolderFlow. `{answer}` holds your choice." Preview of the question. |

## Variables

**Which are available at a step** mirrors `check_variables` in validate.rs exactly: the details produced by the trigger or by a step on **every** path from the trigger to this one (intersection over incoming paths, iterated to a fixed point). A pure function `availableAt(workflow, stepId)` in `src/features/editor/variables.ts`, unit-tested against the same cases as the Rust tests (straight line, two branches merging, a detail made on only one branch, loops, unreachable steps, the trigger itself which gets nothing).

Each available detail carries its **name**, its **kind** (text, number, date, yes/no; trigger details have fixed kinds, Extract/Agent fields their own, everything else text), and **where it comes from**: "This file" for trigger details, else the producing step's title, or "Earlier steps" when different steps make it on different paths.

Plain-language labels for built-ins: `{file}` "File name", `{extension}` "Extension", `{folder}` "Folder", `{dateAdded}` "Date added", `{year}` "Year", `{date}` "Today's date", `{category}` "Category", `{answer}` "My answer", `{newName}` "New name", `{newFolder}` "New folder". Extract/Agent/Write details show their own name.

**Offered** through VariableText: the Insert list and `{` autocomplete, grouped by source, nearest step first. An unreachable step says "Connect this step to use details from earlier steps."

**Sample values** for previews: file `Scan_0042`, extension `pdf`, folder `~/Downloads`, dateAdded/date today as `YYYY-MM-DD`, year this year; category and answer the first category/answer of the producing step; Extract/Agent fields by kind (Text: the field's name, Number `42.50`, Date today, Yes or no `yes`); Write `…`.

## Pickers

Folder and CSV pickers need `tauri-plugin-dialog`.

- **Rust**: add the plugin, and two commands `choose_folder(start)` and `choose_csv(start)` that open the native panel from Rust and return the path with the home folder shortened to `~`, or `null` if cancelled. Opening from Rust keeps the path handling in one place and needs no dialog permission in the window's capability.
- **Api**: `chooseFolder(start?: string): Promise<string | null>` and `chooseCsv(start?: string): Promise<string | null>` in `Api`, `tauri.ts` and `mock.ts`.
- **Browser and mock**: the mock answers with a value set in `MockOptions` (tests) or a fixed sample (`~/Documents`) in previews. The text field always works, so nothing depends on the picker.
- Later, the folder picked is the natural moment to grant access to it (confinement, "nothing outside the granted folders").

## Validation feedback

Problems arrive per step from Rust after edits pause. Shown:

- **Next to the field** they belong to, in the danger colour, under the control.
- **In the banner** at the top of the form when they have no field (loop, unreachable, no model, many triggers). "No model" links to Settings.
- **Instantly, client-side**, for unknown `{names}` in a VariableText, so the user sees it while typing; the Rust problem then agrees.

To place a problem on a field without parsing messages, `Problem` gains an optional `field` (see Format changes). Until a problem has one it goes in the banner.

## Format changes

All are additive and optional in the JSON (`serde(default)`, `skip_serializing_if`), so existing files load unchanged and `graph.ts` `defaultStep` still compiles. ts-rs exports them as optional (`?:`) and `types.ts` follows, keeping `contract.check.ts` green.

| Change | Why | Use case | When |
| --- | --- | --- | --- |
| Classify categories: `description?` ("How to recognise it"). Categories become their own `Category { id, label, description? }` type; Ask me answers stay `Branch`. | Names alone are ambiguous to the model; the description is sent with the name. | 4, 5, 8 | **Now** |
| `Field.description?` ("What to look for"), for Extract and Agent outputs. | Picks the right candidate (total vs subtotal), explains the format wanted. | 1, 2, 6, 7 | **Now** |
| Add row: `headers?: string[]`, one per column, written as the first row of a new file. | A CSV without headings is unreadable in Numbers or Excel. A parallel list keeps `columns: string[]` unchanged; the form edits them as pairs. Validation: `invalid_value` if lengths differ. | 1, 10 | **Now** |
| Create file: `folder?: string` (variables allowed; empty means the file's folder). | Notes and summaries often belong elsewhere; also scheduled runs have no file folder. | 6, showcase | **Now** |
| `Problem.field?: string`, the field the problem is about (`"folder"`, `"categories"`, `"categories.2.label"`, `"condition.left"`). | Puts problems next to their field without parsing messages. | all | **Now** |
| Classify and Ask me: an optional name for their result (default `category` / `answer`). | Two Classify steps on one path both produce `{category}`; the second hides the first. | 8 plus a second sort | Later |
| Dates and numbers: a display format per field (e.g. "14 Sep 2026", two decimals, currency). | Names people like vs. sortable ISO dates. Until then dates are `YYYY-MM-DD` and numbers as read. | 1, 6 | Later |
| If: "is empty" / "is not empty", and "is before/after N days from today" for dates. | "Contracts ending in 30 days", "invoices with no due date". | 2, 7 | Later |
| Tag colours (Finder's seven). | Nice, not needed to sort files. | 3 | Later |
| Rename collision policy, Move "create folder if missing". | **Not proposed.** Collisions always keep both (TESTING.md) by adding " 2"; Move always creates missing folders inside the granted area. One right behaviour beats a choice nobody understands; the helper text says what happens. | | No |
| Write "length" and "tone" fields. | **Not proposed**: the starters and helper put this in the instruction, where models handle it well. | | No |

The Rust side of "Now": `format.rs` (the new fields and `Category`), `validate.rs` (headers length, Create file's `folder` checked for variables, `field` on every problem; descriptions are guidance sent to the model, not filled in, so they aren't checked for variables), templates, the format doc, ts-rs exports, and the mock.

## Showcase template: "Paperwork inbox"

Shows off every non-trigger step type, three kinds of AI step, a question, a merge where variables drop out, and details flowing into names, folders, rows and messages. Blurb: "Sorts receipts, invoices and contracts from Downloads: renames, files and logs them, and asks before big invoices."

```
 1  File added    ~/Downloads, pdf png jpg jpeg heic
 2  Classify      "What kind of paperwork is this?"
                  Receipt: "Proof of a payment I made: shop receipts, order confirmations, card slips"
                  Invoice: "A bill asking me to pay, with an amount due and usually a due date"
                  Contract: "An agreement to sign or signed: leases, service contracts, terms of employment"
                  Something else: "Anything that isn't one of the above"

 Receipt →
 3  Extract       date (Date, "the date I paid"), vendor (Text, "the shop or company I paid"),
                  amount (Number, "the total I paid, including tax"); if missing: ask me
 4  Rename        {date} {vendor} {amount}
 5  Move          ~/Documents/Paperwork/Receipts/{year}
 6  Add row       ~/Documents/Paperwork/Expenses.csv
                  Date {date} | Vendor {vendor} | Amount {amount} | File {newName}

 Invoice →
 7  Extract       vendor, amount (Number, "the total due"), due (Date, "the date payment is due"),
                  number (Text, "the invoice number")
 8  If            {amount} is more than 1000      yes → 9, no → 11
 9  Ask me        "Invoice from {vendor} for {amount}, due {due}. File it?"
                  File it → 11, Leave it in Downloads → 10
10  Stop
11  Rename        {vendor} invoice {number}       (the merge: {answer} isn't offered here, one path lacks it)
12  Move          ~/Documents/Paperwork/Invoices/{year}
13  Notify        "Filed the {vendor} invoice for {amount}. It's due {due}."

 Contract →
14  Agent step    "Read this contract and note who it's with, when it ends, whether it renews by itself,
                  and anything unusual someone who isn't a lawyer should know."
                  May read the file. Hands back: party (Text), endDate (Date), autoRenews (Yes or no),
                  watchOut (Text, "anything unusual, in one sentence")
15  Write         "A short plain-English summary of this contract for my records: who it's with, what
                  I agreed to, when it ends, and this: {watchOut}. Use bullet points."  saved as summary
16  Rename        {party} contract
17  Move          ~/Documents/Paperwork/Contracts
18  Create file   "{party} contract summary.md" in {newFolder}, contents {summary}
19  If            {autoRenews} is yes             yes → 20, no: ends
20  Notify        "{party} renews by itself on {endDate}. Cancel before then if you don't want that."

 Something else →
21  Tag           To sort
22  Notify        "I couldn't sort {file}. It's in Downloads, tagged To sort."
```

Every step type except the other two triggers appears. Needs the category and field descriptions and Create file's folder from "Now"; without them it still works, with less guidance for the models.

## Open questions

1. **Scheduled runs have no file**, but nothing stops Rename or Move after a Schedule trigger. Should validation refuse file steps after `schedule` (a new problem code), or should Schedule gain a folder to run over (which the "Weekly clean-up" blurb, "archive Downloads files older than 30 days", already promises)?
2. **Add row headings**: parallel `headers` (additive, proposed) or `columns: [{ heading, value }]` (cleaner, but a breaking change to v1 and to `defaultStep` in graph.ts)?
3. **`Problem.field`** now, or map problems to fields by code and message first and add it later?
4. **Pickers from Rust commands** (proposed) or the dialog plugin's JavaScript API with a capability?
5. **Titles**: keep them hand-written, or offer to fill the title from the settings ("Move to Receipts/{year}") until the user edits it?
6. **"Runs on" line** in AI forms needs settings and providers in the inspector; in scope for Phase 2 or later?
7. **Agent abilities**: is "Read the file's contents" the only one for now, and should it be on by default and possible to turn off?
