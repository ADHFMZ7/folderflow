//! Try on a file: the workflow as it stands in the editor runs on one file,
//! and every action is worked out, not done. Nothing on disk changes, nothing
//! is recorded, and the file isn't marked as seen. See docs/engine.md, "Try
//! on a file".

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use vela_lib::api::types::ErrorCode;
use vela_lib::engine::files::sys;
use vela_lib::engine::runs::{RunStatus, StepOutcome};
use vela_lib::engine::TryResult;
use vela_lib::workflow::Workflow;

use common::engine::{engine, EngineHarness};

/// A workflow as the editor holds it: never saved.
fn draft(steps: Value) -> Workflow {
    let mut steps = steps;
    for step in steps.as_array_mut().unwrap() {
        step["position"] = json!({ "x": 0, "y": 0 });
        if step.get("title").is_none() {
            step["title"] = step["id"].clone();
        }
    }
    serde_json::from_value(json!({
        "version": 1,
        "id": uuid::Uuid::new_v4().to_string(),
        "name": "Trying",
        "revision": 1,
        "enabled": false,
        "steps": steps,
    }))
    .unwrap()
}

/// A folder (`None`), or a file with its bytes and tags.
type Entry = (PathBuf, Option<(Vec<u8>, Vec<String>)>);

/// Every folder and file under `root`.
fn snapshot(root: &Path) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            let rel = path.strip_prefix(root).unwrap().to_path_buf();
            if entry.file_type().unwrap().is_dir() {
                out.push((rel, None));
                stack.push(path);
            } else {
                let tags = sys::tags(&path).unwrap();
                out.push((rel, Some((fs::read(&path).unwrap(), tags))));
            }
        }
    }
    out.sort();
    out
}

fn messages(result: &TryResult) -> Vec<(&str, Option<&str>)> {
    result
        .steps
        .iter()
        .map(|s| (s.step_id.as_str(), s.message.as_deref()))
        .collect()
}

fn no_answers() -> BTreeMap<String, String> {
    BTreeMap::new()
}

async fn try_on(h: &EngineHarness, workflow: &Workflow, file: &str) -> TryResult {
    h.engine
        .try_on_file(workflow.clone(), file.to_owned(), no_answers())
        .await
        .unwrap()
}

#[tokio::test]
async fn every_action_is_worked_out_and_nothing_on_disk_changes() {
    let h = engine();
    let file = h.file("Downloads/Receipt.pdf");
    fs::create_dir_all(h.home.join("Documents")).unwrap();
    let workflow = draft(json!([
        { "id": "t", "type": "runNow", "next": "r" },
        { "id": "r", "type": "rename", "template": "{dateAdded} {file}", "next": "m" },
        { "id": "m", "type": "move", "to": "~/Documents/Receipts/{year}", "mode": "move", "next": "g" },
        { "id": "g", "type": "tag", "tags": ["Paid"], "next": "a" },
        { "id": "a", "type": "addRow", "file": "~/Documents/Receipts/log.csv",
          "columns": ["{dateAdded}", "{file}"], "headers": ["Date", "Name"], "next": "c" },
        { "id": "c", "type": "createFile", "name": "{newName}.txt", "contents": "Filed", "next": "n" },
        { "id": "n", "type": "notify", "message": "Filed {newName}", "next": null },
    ]));
    let before = snapshot(&h.home);

    let result = try_on(&h, &workflow, &file).await;

    assert_eq!(result.status, RunStatus::Done);
    assert_eq!(
        messages(&result),
        [
            ("t", None),
            (
                "r",
                Some("Would rename Receipt.pdf to 2026-09-14 Receipt.pdf.")
            ),
            (
                "m",
                Some("Would move 2026-09-14 Receipt.pdf to ~/Documents/Receipts/2026 (the folder will be created).")
            ),
            ("g", Some("Would tag 2026-09-14 Receipt.pdf Paid.")),
            (
                "a",
                Some("Would create log.csv, with its headings and the row 2026-09-14, Receipt.")
            ),
            (
                "c",
                Some("Would create 2026-09-14 Receipt.txt in ~/Documents/Receipts/2026.")
            ),
            (
                "n",
                Some("Would show the notification \"Filed 2026-09-14 Receipt\".")
            ),
        ]
    );
    assert_eq!(result.values["newName"].value, "2026-09-14 Receipt");

    assert_eq!(snapshot(&h.home), before);
    assert!(h.engine.list_runs(Default::default()).unwrap().is_empty());
    assert!(h.engine.list_notices().is_empty());
    assert!(h.notes.bodies().is_empty());
}

#[tokio::test]
async fn names_already_taken_are_numbered_as_a_real_run_would() {
    let h = engine();
    let file = h.file("Downloads/scan.pdf");
    h.file("Downloads/Invoice.pdf");
    h.file("Documents/Invoice 2.pdf");
    fs::write(h.home.join("Documents/log.csv"), "Date\n2026-09-01\n").unwrap();
    let workflow = draft(json!([
        { "id": "t", "type": "runNow", "next": "r" },
        { "id": "r", "type": "rename", "template": "Invoice", "next": "m" },
        { "id": "m", "type": "move", "to": "~/Documents", "mode": "move", "next": "g" },
        { "id": "g", "type": "tag", "tags": ["Paid", "paid"], "next": "a" },
        { "id": "a", "type": "addRow", "file": "~/Documents/log.csv", "columns": ["{dateAdded}"],
          "next": null },
    ]));

    let result = try_on(&h, &workflow, &file).await;

    assert_eq!(
        messages(&result)[1..],
        [
            ("r", Some("Would rename scan.pdf to Invoice 2.pdf.")),
            (
                "m",
                Some("Would move Invoice 2.pdf to ~/Documents, as Invoice 2 2.pdf.")
            ),
            ("g", Some("Would tag Invoice 2 2.pdf Paid.")),
            ("a", Some("Would add the row 2026-09-14 to log.csv.")),
        ]
    );
}

#[tokio::test]
async fn what_only_the_disk_can_show_stops_the_try_as_it_would_a_run() {
    let h = engine();
    let file = h.file("Downloads/scan.pdf");
    let real = h.file("Elsewhere/log.csv");
    fs::create_dir_all(h.home.join("Documents")).unwrap();
    std::os::unix::fs::symlink(
        h.home.join(real.trim_start_matches("~/")),
        h.home.join("Documents/log.csv"),
    )
    .unwrap();
    let workflow = draft(json!([
        { "id": "t", "type": "runNow", "next": "a" },
        { "id": "a", "type": "addRow", "file": "~/Documents/log.csv", "columns": ["{file}"],
          "next": "n" },
        { "id": "n", "type": "notify", "message": "Logged", "next": null },
    ]));

    let result = try_on(&h, &workflow, &file).await;

    assert_eq!(result.status, RunStatus::Failed);
    let error = result.error.unwrap();
    assert_eq!(error.step_id.as_deref(), Some("a"));
    assert_eq!(error.message, "log.csv is a link, so Vela won't change it.");
    assert_eq!(result.steps.len(), 2);
    assert_eq!(result.steps[1].outcome, StepOutcome::Failed);
}

#[tokio::test]
async fn ask_me_waits_inside_the_try_and_an_answer_carries_it_on() {
    let h = engine();
    let file = h.file("Downloads/scan.pdf");
    let workflow = draft(json!([
        { "id": "t", "type": "runNow", "next": "q" },
        { "id": "q", "type": "askMe", "question": "Log {file}?",
          "answers": [{ "id": "log", "label": "Log it" }, { "id": "skip", "label": "Skip" }],
          "branches": { "log": "r" } },
        { "id": "r", "type": "rename", "template": "Logged {file}", "next": null },
    ]));

    let asked = try_on(&h, &workflow, &file).await;
    assert_eq!(asked.status, RunStatus::Waiting);
    let question = asked.question.unwrap();
    assert_eq!(question.question, "Log scan?");
    assert_eq!(question.step_id, "q");
    assert_eq!(asked.steps.last().unwrap().outcome, StepOutcome::Waiting);

    let answered = h
        .engine
        .try_on_file(
            workflow,
            file,
            BTreeMap::from([("q".to_string(), "log".to_string())]),
        )
        .await
        .unwrap();
    assert_eq!(answered.status, RunStatus::Done);
    assert_eq!(
        messages(&answered)[1..],
        [
            ("q", Some("You answered Log it.")),
            ("r", Some("Would rename scan.pdf to Logged scan.pdf.")),
        ]
    );
    assert_eq!(answered.steps[1].branch.as_deref(), Some("log"));
    assert!(h.engine.list_needs_you().unwrap().is_empty());
}

#[tokio::test]
async fn a_problem_stops_the_try_only_on_the_path_it_takes() {
    let h = engine();
    let workflow = draft(json!([
        { "id": "t", "type": "runNow", "next": "i" },
        { "id": "i", "type": "if", "condition": { "left": "{file}", "op": "=", "right": "urgent" },
          "branches": { "yes": "r", "no": "n" } },
        { "id": "r", "type": "rename", "template": "", "next": null },
        { "id": "n", "type": "notify", "message": "Not urgent", "next": null },
    ]));

    let calm = try_on(&h, &workflow, &h.file("Downloads/calm.pdf")).await;
    assert_eq!(calm.status, RunStatus::Done);
    assert_eq!(calm.steps.last().unwrap().step_id, "n");

    let urgent = try_on(&h, &workflow, &h.file("Downloads/urgent.pdf")).await;
    assert_eq!(urgent.status, RunStatus::Failed);
    let error = urgent.error.unwrap();
    assert_eq!(error.step_id.as_deref(), Some("r"));
    assert!(!error.message.is_empty());
    // It stopped before the step did anything.
    assert_eq!(urgent.steps.last().unwrap().step_id, "r");
    assert_eq!(urgent.steps.last().unwrap().outcome, StepOutcome::Failed);
}

#[tokio::test]
async fn a_file_tried_in_a_watched_folder_still_runs_when_it_arrives() {
    let mut h = engine();
    let watched = h.workflow(
        "Tag",
        serde_json::to_value(
            draft(json!([
                { "id": "t", "type": "fileAdded", "folder": "~/Downloads", "fileTypes": [],
                  "subfolders": false, "next": "g" },
                { "id": "g", "type": "tag", "tags": ["Seen"], "next": null },
            ]))
            .steps,
        )
        .unwrap(),
    );
    fs::create_dir_all(h.home.join("Downloads")).unwrap();
    h.switch(&watched, true);

    let file = h.file("Downloads/new.pdf");
    let tried = try_on(&h, &watched, &file).await;
    assert_eq!(tried.status, RunStatus::Done);

    h.changed("Downloads");
    assert_eq!(h.settle().await.len(), 1);
}

#[tokio::test]
async fn a_scheduled_workflow_has_no_file_to_try_on() {
    let h = engine();
    let workflow = draft(json!([
        { "id": "t", "type": "schedule", "schedule": { "every": "day", "time": "09:00" }, "next": "n" },
        { "id": "n", "type": "notify", "message": "Morning", "next": null },
    ]));
    let err = h
        .engine
        .try_on_file(workflow, h.file("Downloads/a.pdf"), no_answers())
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Invalid);
}

#[tokio::test]
async fn a_missing_file_is_refused() {
    let h = engine();
    let workflow = draft(json!([
        { "id": "t", "type": "runNow", "next": null },
    ]));
    let err = h
        .engine
        .try_on_file(workflow, "~/Downloads/gone.pdf".into(), no_answers())
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Invalid);
    assert!(err.message.contains("gone.pdf no longer exists"));
}
