//! Any sequence of file actions, undone, gives back exactly the starting tree:
//! names, places, contents and tags (TESTING.md, "Losing or corrupting files").
//! Names come from small pools so clashes, numbering and case-only renames
//! happen often.

mod common;

use std::path::PathBuf;

use folderflow_lib::engine::files;
use proptest::prelude::*;

use common::files::place;

const RUN: &str = "5d0b6a0e-0000-4000-8000-0000000000aa";
const NAMES: [&str; 4] = ["Receipt", "receipt", "Scan", "Receipt 2"];
const FOLDERS: [&str; 4] = [
    "Downloads",
    "Documents/Receipts",
    "Documents/Receipts/2026",
    "Documents/Other",
];
const SHEETS: [&str; 2] = ["Documents/Receipts/Log.csv", "Documents/Other/New.csv"];
const TAGS: [&str; 3] = ["Paid", "paid", "Receipts"];

#[derive(Debug, Clone)]
enum Op {
    Rename(usize, usize),
    Move(usize, usize),
    Copy(usize, usize),
    Create(usize, usize, String),
    AddRow(usize, String),
    Tag(usize, usize),
}

fn op() -> impl Strategy<Value = Op> {
    let text = "[a-z,\"=+ ]{0,8}";
    prop_oneof![
        (0..8usize, 0..NAMES.len()).prop_map(|(f, n)| Op::Rename(f, n)),
        (0..8usize, 0..FOLDERS.len()).prop_map(|(f, d)| Op::Move(f, d)),
        (0..8usize, 0..FOLDERS.len()).prop_map(|(f, d)| Op::Copy(f, d)),
        (0..FOLDERS.len(), 0..NAMES.len(), text).prop_map(|(d, n, c)| Op::Create(d, n, c)),
        (0..SHEETS.len(), text).prop_map(|(s, v)| Op::AddRow(s, v)),
        (0..8usize, 0..TAGS.len()).prop_map(|(f, t)| Op::Tag(f, t)),
    ]
}

/// Starting files: a few in Downloads and one in Receipts, plus a spreadsheet.
fn start() -> impl Strategy<Value = Vec<(usize, String)>> {
    prop::collection::vec((0..NAMES.len(), "[a-z]{0,6}"), 1..4)
}

proptest! {
    // 40 cases in the main suite; set PROPTEST_CASES for a longer hunt.
    #![proptest_config(ProptestConfig::with_cases(
        std::env::var("PROPTEST_CASES").ok().and_then(|n| n.parse().ok()).unwrap_or(40)
    ))]

    #[test]
    fn undoing_any_sequence_of_actions_restores_the_starting_tree(
        files_at_start in start(),
        ops in prop::collection::vec(op(), 1..10),
    ) {
        let p = place();
        let mut tracked: Vec<PathBuf> = Vec::new();
        for (i, (name, contents)) in files_at_start.iter().enumerate() {
            let path = p.at(&format!("Downloads/{} {i}.pdf", NAMES[*name]));
            if !path.exists() {
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(&path, contents).unwrap();
                tracked.push(path);
            }
        }
        p.file("Documents/Receipts/Log.csv", "Date,Amount\n");
        let before = p.tree();

        let mut acts = p.files(RUN, &[("Downloads", false), ("Documents", true)]);
        for op in &ops {
            let pick = |i: usize| i % tracked.len();
            // Actions may be refused (a file already moved away, say); a refusal
            // must change nothing, which the final comparison also checks.
            match op {
                Op::Rename(f, n) => {
                    let i = pick(*f);
                    if let Ok(to) = acts.rename(&tracked[i], NAMES[*n]) { tracked[i] = to; }
                }
                Op::Move(f, d) => {
                    let i = pick(*f);
                    if let Ok(to) = acts.move_file(&tracked[i], &p.at(FOLDERS[*d])) { tracked[i] = to; }
                }
                Op::Copy(f, d) => { let _ = acts.copy_file(&tracked[pick(*f)], &p.at(FOLDERS[*d])); }
                Op::Create(d, n, c) => {
                    let _ = acts.create_file(&p.at(FOLDERS[*d]), &format!("{}.txt", NAMES[*n]), c);
                }
                Op::AddRow(s, v) => { let _ = acts.add_row(&p.at(SHEETS[*s]), std::slice::from_ref(v), None); }
                Op::Tag(f, t) => { let _ = acts.tag(&tracked[pick(*f)], &[TAGS[*t].to_string()]); }
            }
        }

        let report = files::undo(&p.journal, RUN, &p.trash).unwrap();

        prop_assert!(report.left_alone.is_empty(), "{:?}", report.left_alone);
        prop_assert_eq!(p.tree(), before);
    }
}
