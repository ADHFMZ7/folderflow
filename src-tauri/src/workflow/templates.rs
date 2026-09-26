//! The workflows `create_workflow` builds: a blank one, and one per template in
//! the catalog (docs/workflow-format.md, "Templates"). Every build gets a new
//! workflow id, new step ids and new branch ids, as the editor would.

use std::collections::BTreeMap;

use super::format::{
    Branch, Category, Condition, Every, Field, FieldType, IfMissing, MoveMode, Op, Position,
    Schedule, Step, StepKind, Workflow, WORKFLOW_VERSION,
};

/// Horizontal distance between columns, for templates laid out in columns.
const COL: f64 = 320.0;
/// Vertical distance between rows of steps on the canvas.
const ROW: f64 = 160.0;
/// Horizontal offset of a step on a side branch.
const SIDE: f64 = 280.0;

/// "New workflow": one `fileAdded` step on Downloads, any file type.
pub fn blank() -> Workflow {
    workflow(
        "New workflow",
        vec![file_added(
            &new_id("s"),
            "When a file is added to Downloads",
            "~/Downloads",
            &[],
            None,
            0,
        )],
    )
}

/// The template's workflow with a new id, or `None` for an unknown template.
pub fn build(template_id: &str) -> Option<Workflow> {
    Some(match template_id {
        "receipts" => receipts(),
        "screenshots" => screenshots(),
        "summaries" => summaries(),
        "invoices" => invoices(),
        "cleanup" => cleanup(),
        "paperwork" => paperwork(),
        _ => return None,
    })
}

fn receipts() -> Workflow {
    let [t, classify, extract, rename, mv] = ids();
    let receipt = new_id("c");
    workflow(
        "Sort receipts",
        vec![
            file_added(
                &t,
                "When a PDF is added to Downloads",
                "~/Downloads",
                &["pdf"],
                Some(&classify),
                0,
            ),
            step(
                &classify,
                "Is this a receipt?",
                1,
                StepKind::Classify {
                    categories: vec![
                        category(&receipt, "Receipt", None),
                        category(&new_id("c"), "Other", None),
                    ],
                    instructions: String::new(),
                    branches: exits(&[(&receipt, &extract)]),
                },
            ),
            step(
                &extract,
                "Pull out the date, vendor and amount",
                2,
                StepKind::Extract {
                    fields: vec![
                        field("date", FieldType::Date),
                        field("vendor", FieldType::Text),
                        field("amount", FieldType::Number),
                    ],
                    if_missing: IfMissing::Review,
                    next: Some(rename.clone()),
                },
            ),
            step(
                &rename,
                "Rename it by date, vendor and amount",
                3,
                StepKind::Rename {
                    template: "{date} - {vendor} - {amount}".into(),
                    next: Some(mv.clone()),
                },
            ),
            step(
                &mv,
                "Move it to Receipts, in a folder for the year",
                4,
                StepKind::Move {
                    to: "~/Documents/Receipts/{year}".into(),
                    mode: MoveMode::Move,
                    next: None,
                },
            ),
        ],
    )
}

fn screenshots() -> Workflow {
    let [t, check, mv] = ids();
    workflow(
        "Tidy screenshots",
        vec![
            file_added(
                &t,
                "When a PNG is added to the Desktop",
                "~/Desktop",
                &["png"],
                Some(&check),
                0,
            ),
            step(
                &check,
                "Is it a screenshot?",
                1,
                StepKind::If {
                    condition: Condition {
                        left: "{file}".into(),
                        op: Op::StartsWith,
                        right: "Screenshot".into(),
                    },
                    branches: exits(&[("yes", &mv)]),
                },
            ),
            step(
                &mv,
                "Move it to Screenshots, in a folder for the year",
                2,
                StepKind::Move {
                    to: "~/Pictures/Screenshots/{year}".into(),
                    mode: MoveMode::Move,
                    next: None,
                },
            ),
        ],
    )
}

fn summaries() -> Workflow {
    let [t, write, create] = ids();
    workflow(
        "Summarise PDFs",
        vec![
            file_added(
                &t,
                "When a PDF is added to Downloads",
                "~/Downloads",
                &["pdf"],
                Some(&write),
                0,
            ),
            step(
                &write,
                "Write a one-paragraph summary",
                1,
                StepKind::Write {
                    instruction: "Summarise this document in one paragraph.".into(),
                    save_as: "summary".into(),
                    next: Some(create.clone()),
                },
            ),
            step(
                &create,
                "Save the summary next to the PDF",
                2,
                StepKind::CreateFile {
                    name: "{file} summary.txt".into(),
                    contents: "{summary}".into(),
                    folder: None,
                    next: None,
                },
            ),
        ],
    )
}

fn invoices() -> Workflow {
    let [t, extract, check, ask, row] = ids();
    let log_it = new_id("a");
    workflow(
        "Log invoices",
        vec![
            file_added(
                &t,
                "When a PDF is added to Downloads",
                "~/Downloads",
                &["pdf"],
                Some(&extract),
                0,
            ),
            step(
                &extract,
                "Pull out the vendor, amount and due date",
                1,
                StepKind::Extract {
                    fields: vec![
                        field("vendor", FieldType::Text),
                        field("amount", FieldType::Number),
                        field("due", FieldType::Date),
                    ],
                    if_missing: IfMissing::Review,
                    next: Some(check.clone()),
                },
            ),
            step(
                &check,
                "Is it over 500?",
                2,
                StepKind::If {
                    condition: Condition {
                        left: "{amount}".into(),
                        op: Op::Greater,
                        right: "500".into(),
                    },
                    branches: exits(&[("yes", &ask), ("no", &row)]),
                },
            ),
            Step {
                position: Position {
                    x: SIDE,
                    y: 3.0 * ROW,
                },
                ..step(
                    &ask,
                    "Ask me before logging it",
                    3,
                    StepKind::AskMe {
                        question: "Log {vendor} {amount}?".into(),
                        answers: vec![branch(&log_it, "Log it"), branch(&new_id("a"), "Skip")],
                        branches: exits(&[(&log_it, &row)]),
                    },
                )
            },
            step(
                &row,
                "Add a row to Invoices",
                4,
                StepKind::AddRow {
                    file: "~/Documents/Invoices.csv".into(),
                    columns: vec!["{vendor}".into(), "{amount}".into(), "{due}".into()],
                    headers: None,
                    next: None,
                },
            ),
        ],
    )
}

fn cleanup() -> Workflow {
    let [t, notify] = ids();
    workflow(
        "Weekly clean-up",
        vec![
            step(
                &t,
                "Every Friday at 17:00",
                0,
                StepKind::Schedule {
                    schedule: Schedule {
                        every: Every::Week,
                        time: "17:00".into(),
                        weekday: Some(5),
                    },
                    next: Some(notify.clone()),
                },
            ),
            step(
                &notify,
                "Remind me to tidy Downloads",
                1,
                StepKind::Notify {
                    message: "Time to tidy Downloads.".into(),
                    next: None,
                },
            ),
        ],
    )
}

/// The showcase: every step type but the other triggers, three kinds of AI
/// step, a question, and a merge where a variable drops out.
fn paperwork() -> Workflow {
    let [t, sort, r_extract, r_rename, r_move, r_row] = ids();
    let [i_extract, i_check, i_ask, i_stop, i_rename, i_move, i_notify] = ids();
    let [c_agent, c_write, c_rename, c_move, c_create, c_check, c_notify] = ids();
    let [o_tag, o_notify] = ids();
    let [receipt, invoice, contract, other] = std::array::from_fn(|_| new_id("c"));
    let file_it = new_id("a");
    let leave_it = new_id("a");
    let place = |col: f64, row: u32, s: Step| Step {
        position: Position {
            x: col * COL,
            y: f64::from(row) * ROW,
        },
        ..s
    };
    let next = |id: &String| Some(id.clone());

    workflow(
        "Paperwork inbox",
        vec![
            place(
                2.0,
                0,
                file_added(
                    &t,
                    "When a PDF or photo is added to Downloads",
                    "~/Downloads",
                    &["pdf", "png", "jpg", "jpeg", "heic"],
                    Some(&sort),
                    0,
                ),
            ),
            place(
                2.0,
                1,
                step(
                    &sort,
                    "What kind of paperwork is this?",
                    0,
                    StepKind::Classify {
                        categories: vec![
                            category(
                                &receipt,
                                "Receipt",
                                Some("Proof of a payment I made: shop receipts, order confirmations, card slips"),
                            ),
                            category(
                                &invoice,
                                "Invoice",
                                Some("A bill asking me to pay, with an amount due and usually a due date"),
                            ),
                            category(
                                &contract,
                                "Contract",
                                Some("An agreement to sign or already signed: leases, service contracts, terms of employment"),
                            ),
                            category(
                                &other,
                                "Something else",
                                Some("Anything that isn't one of the above"),
                            ),
                        ],
                        instructions: String::new(),
                        branches: exits(&[
                            (&receipt, &r_extract),
                            (&invoice, &i_extract),
                            (&contract, &c_agent),
                            (&other, &o_tag),
                        ]),
                    },
                ),
            ),
            // Receipts: file them by year and log them.
            place(
                0.0,
                2,
                step(
                    &r_extract,
                    "Pull out the date, shop and total",
                    0,
                    StepKind::Extract {
                        fields: vec![
                            described("date", FieldType::Date, "The date I paid"),
                            described("vendor", FieldType::Text, "The shop or company I paid"),
                            described("amount", FieldType::Number, "The total I paid, including tax"),
                        ],
                        if_missing: IfMissing::Review,
                        next: next(&r_rename),
                    },
                ),
            ),
            place(
                0.0,
                3,
                step(
                    &r_rename,
                    "Rename it by date, shop and total",
                    0,
                    StepKind::Rename {
                        template: "{date} {vendor} {amount}".into(),
                        next: next(&r_move),
                    },
                ),
            ),
            place(
                0.0,
                4,
                step(
                    &r_move,
                    "File it under Receipts for the year",
                    0,
                    StepKind::Move {
                        to: "~/Documents/Paperwork/Receipts/{year}".into(),
                        mode: MoveMode::Move,
                        next: next(&r_row),
                    },
                ),
            ),
            place(
                0.0,
                5,
                step(
                    &r_row,
                    "Log it in Expenses",
                    0,
                    StepKind::AddRow {
                        file: "~/Documents/Paperwork/Expenses.csv".into(),
                        columns: vec![
                            "{date}".into(),
                            "{vendor}".into(),
                            "{amount}".into(),
                            "{newName}".into(),
                        ],
                        headers: Some(vec![
                            "Date".into(),
                            "Vendor".into(),
                            "Amount".into(),
                            "File".into(),
                        ]),
                        next: None,
                    },
                ),
            ),
            // Invoices: ask before filing a big one.
            place(
                1.0,
                2,
                step(
                    &i_extract,
                    "Pull out who it's from, the amount and the due date",
                    0,
                    StepKind::Extract {
                        fields: vec![
                            described("vendor", FieldType::Text, "The company asking to be paid"),
                            described("amount", FieldType::Number, "The total due"),
                            described("due", FieldType::Date, "The date payment is due"),
                            described("number", FieldType::Text, "The invoice number"),
                        ],
                        if_missing: IfMissing::Review,
                        next: next(&i_check),
                    },
                ),
            ),
            place(
                1.0,
                3,
                step(
                    &i_check,
                    "Is it more than 1000?",
                    0,
                    StepKind::If {
                        condition: Condition {
                            left: "{amount}".into(),
                            op: Op::Greater,
                            right: "1000".into(),
                        },
                        branches: exits(&[("yes", &i_ask), ("no", &i_rename)]),
                    },
                ),
            ),
            place(
                2.0,
                4,
                step(
                    &i_ask,
                    "Ask me before filing a big invoice",
                    0,
                    StepKind::AskMe {
                        question: "Invoice from {vendor} for {amount}, due {due}. File it?".into(),
                        answers: vec![
                            branch(&file_it, "File it"),
                            branch(&leave_it, "Leave it in Downloads"),
                        ],
                        branches: exits(&[(&file_it, &i_rename), (&leave_it, &i_stop)]),
                    },
                ),
            ),
            place(
                2.0,
                5,
                step(&i_stop, "Leave it where it is", 0, StepKind::Stop {}),
            ),
            place(
                1.0,
                5,
                step(
                    &i_rename,
                    "Rename it by company and invoice number",
                    0,
                    StepKind::Rename {
                        template: "{vendor} invoice {number}".into(),
                        next: next(&i_move),
                    },
                ),
            ),
            place(
                1.0,
                6,
                step(
                    &i_move,
                    "File it under Invoices for the year",
                    0,
                    StepKind::Move {
                        to: "~/Documents/Paperwork/Invoices/{year}".into(),
                        mode: MoveMode::Move,
                        next: next(&i_notify),
                    },
                ),
            ),
            place(
                1.0,
                7,
                step(
                    &i_notify,
                    "Tell me when it's due",
                    0,
                    StepKind::Notify {
                        message: "Filed the {vendor} invoice for {amount}. It's due {due}.".into(),
                        next: None,
                    },
                ),
            ),
            // Contracts: read them, summarise them, warn about renewals.
            place(
                3.0,
                2,
                step(
                    &c_agent,
                    "Read the contract",
                    0,
                    StepKind::Agent {
                        instruction: "Read this contract and note who it's with, when it ends, whether it renews by itself, and anything unusual someone who isn't a lawyer should know.".into(),
                        abilities: vec!["readFile".into()],
                        outputs: vec![
                            described("party", FieldType::Text, "Who the contract is with"),
                            described("endDate", FieldType::Date, "When it ends"),
                            described("autoRenews", FieldType::YesNo, "Whether it renews by itself"),
                            described("watchOut", FieldType::Text, "Anything unusual, in one sentence"),
                        ],
                        next: next(&c_write),
                    },
                ),
            ),
            place(
                3.0,
                3,
                step(
                    &c_write,
                    "Write a plain-English summary",
                    0,
                    StepKind::Write {
                        instruction: "A short plain-English summary of this contract for my records: who it's with, what I agreed to, when it ends, and this: {watchOut}. Use bullet points.".into(),
                        save_as: "summary".into(),
                        next: next(&c_rename),
                    },
                ),
            ),
            place(
                3.0,
                4,
                step(
                    &c_rename,
                    "Rename it after who it's with",
                    0,
                    StepKind::Rename {
                        template: "{party} contract".into(),
                        next: next(&c_move),
                    },
                ),
            ),
            place(
                3.0,
                5,
                step(
                    &c_move,
                    "File it under Contracts",
                    0,
                    StepKind::Move {
                        to: "~/Documents/Paperwork/Contracts".into(),
                        mode: MoveMode::Move,
                        next: next(&c_create),
                    },
                ),
            ),
            place(
                3.0,
                6,
                step(
                    &c_create,
                    "Save the summary next to it",
                    0,
                    StepKind::CreateFile {
                        name: "{party} contract summary.md".into(),
                        contents: "{summary}".into(),
                        folder: Some("{newFolder}".into()),
                        next: next(&c_check),
                    },
                ),
            ),
            place(
                3.0,
                7,
                step(
                    &c_check,
                    "Does it renew by itself?",
                    0,
                    StepKind::If {
                        condition: Condition {
                            left: "{autoRenews}".into(),
                            op: Op::Equal,
                            right: "yes".into(),
                        },
                        branches: exits(&[("yes", &c_notify)]),
                    },
                ),
            ),
            place(
                3.0,
                8,
                step(
                    &c_notify,
                    "Warn me about the renewal",
                    0,
                    StepKind::Notify {
                        message: "{party} renews by itself on {endDate}. Cancel before then if you don't want that.".into(),
                        next: None,
                    },
                ),
            ),
            // Anything else: leave it, but say so.
            place(
                4.0,
                2,
                step(
                    &o_tag,
                    "Tag it To sort",
                    0,
                    StepKind::Tag {
                        tags: vec!["To sort".into()],
                        next: next(&o_notify),
                    },
                ),
            ),
            place(
                4.0,
                3,
                step(
                    &o_notify,
                    "Tell me I need to sort it",
                    0,
                    StepKind::Notify {
                        message: "I couldn't sort {file}. It's in Downloads, tagged To sort.".into(),
                        next: None,
                    },
                ),
            ),
        ],
    )
}

fn workflow(name: &str, steps: Vec<Step>) -> Workflow {
    Workflow {
        version: WORKFLOW_VERSION,
        id: uuid::Uuid::new_v4().to_string(),
        name: name.into(),
        revision: 1,
        enabled: false,
        steps,
    }
}

/// A step in the main column, `row` rows down.
fn step(id: &str, title: &str, row: u32, kind: StepKind) -> Step {
    Step {
        id: id.into(),
        title: title.into(),
        position: Position {
            x: 0.0,
            y: f64::from(row) * ROW,
        },
        kind,
    }
}

fn file_added(
    id: &str,
    title: &str,
    folder: &str,
    file_types: &[&str],
    next: Option<&str>,
    row: u32,
) -> Step {
    step(
        id,
        title,
        row,
        StepKind::FileAdded {
            folder: folder.into(),
            file_types: file_types.iter().map(|s| s.to_string()).collect(),
            subfolders: false,
            next: next.map(str::to_owned),
        },
    )
}

fn branch(id: &str, label: &str) -> Branch {
    Branch {
        id: id.into(),
        label: label.into(),
    }
}

fn field(name: &str, kind: FieldType) -> Field {
    Field {
        name: name.into(),
        kind,
        description: None,
    }
}

/// A field with "what to look for".
fn described(name: &str, kind: FieldType, description: &str) -> Field {
    Field {
        description: Some(description.into()),
        ..field(name, kind)
    }
}

fn category(id: &str, label: &str, description: Option<&str>) -> Category {
    Category {
        id: id.into(),
        label: label.into(),
        description: description.map(str::to_owned),
    }
}

fn exits(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(from, to)| (from.to_string(), to.to_string()))
        .collect()
}

/// `N` new step ids.
fn ids<const N: usize>() -> [String; N] {
    std::array::from_fn(|_| new_id("s"))
}

/// A short random id such as "s3f9a01bc", within the 32-character limit.
fn new_id(prefix: &str) -> String {
    let hex = uuid::Uuid::new_v4().simple().to_string();
    format!("{prefix}{}", &hex[..8])
}
