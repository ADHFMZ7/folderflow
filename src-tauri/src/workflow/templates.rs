//! The workflows `create_workflow` builds: a blank one, and one per template in
//! the catalog (docs/workflow-format.md, "Templates"). Every build gets a new
//! workflow id, new step ids and new branch ids, as the editor would.

use std::collections::BTreeMap;

use super::format::{
    Branch, Condition, Every, Field, FieldType, IfMissing, MoveMode, Op, Position, Schedule, Step,
    StepKind, Workflow, WORKFLOW_VERSION,
};

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
                    categories: vec![branch(&receipt, "Receipt"), branch(&new_id("c"), "Other")],
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
