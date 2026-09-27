//! File actions in runs: a workflow renames, tags, moves, copies, writes and
//! logs its file for real, only inside the folders it names, and the whole run
//! can be undone. See docs/engine.md, "File safety".

mod common;

use std::fs;

use folderflow_lib::api::types::ErrorCode;
use folderflow_lib::engine::files::sys;
use folderflow_lib::engine::runs::{RunStatus, StepOutcome};
use serde_json::{json, Value};

use common::engine::{engine, EngineHarness};
use common::files::names;

fn steps(steps: Value) -> Value {
    let mut steps = steps;
    for step in steps.as_array_mut().unwrap() {
        step["position"] = json!({ "x": 0, "y": 0 });
        if step.get("title").is_none() {
            step["title"] = step["id"].clone();
        }
    }
    steps
}

/// Run now → Rename "{dateAdded} {file}" → Tag "Screenshot" → Move to
/// ~/Pictures/Screenshots/{year} → Copy to ~/Backup → Create file in
/// {newFolder} → Add row to ~/Documents/Log.csv.
fn tidy(h: &EngineHarness) -> folderflow_lib::workflow::Workflow {
    h.workflow(
        "Tidy screenshots",
        steps(json!([
            { "id": "t", "type": "runNow", "next": "r" },
            { "id": "r", "type": "rename", "template": "{dateAdded} {file}", "next": "g" },
            { "id": "g", "type": "tag", "tags": ["Screenshot", "{year}"], "next": "m" },
            { "id": "m", "type": "move", "to": "~/Pictures/Screenshots/{year}", "mode": "move", "next": "c" },
            { "id": "c", "type": "move", "to": "~/Backup", "mode": "copy", "next": "n" },
            { "id": "n", "type": "createFile", "name": "{newName}.txt", "contents": "Moved to {newFolder}",
              "folder": "~/Pictures/Screenshots/{year}", "next": "a" },
            { "id": "a", "type": "addRow", "file": "~/Documents/Log.csv", "columns": ["{newName}", "{year}"],
              "headers": ["Name", "Year"], "next": null },
        ])),
    )
}

#[tokio::test]
async fn a_run_renames_tags_moves_copies_writes_and_logs_its_file() {
    let mut h = engine();
    let wf = tidy(&h);
    let file = h.file("Desktop/Screenshot.png");

    let id = h.engine.run_now(&wf.id, vec![file]).unwrap()[0].id.clone();
    let run = h.until(&id, RunStatus::Done).await;

    let moved = h
        .home
        .join("Pictures/Screenshots/2026/2026-09-14 Screenshot.png");
    assert!(moved.is_file());
    assert!(names(&h.home.join("Desktop")).is_empty());
    assert_eq!(sys::tags(&moved).unwrap(), ["Screenshot", "2026"]);
    assert!(h.home.join("Backup/2026-09-14 Screenshot.png").is_file());
    assert_eq!(
        fs::read_to_string(
            h.home
                .join("Pictures/Screenshots/2026/2026-09-14 Screenshot.txt")
        )
        .unwrap(),
        "Moved to ~/Backup"
    );
    assert_eq!(
        fs::read_to_string(h.home.join("Documents/Log.csv")).unwrap(),
        "Name,Year\n2026-09-14 Screenshot,2026\n"
    );

    // The run knows where its file is now, and says what each step did.
    assert_eq!(run.file.as_ref().unwrap().path, moved);
    assert_eq!(run.values["newName"].value, "2026-09-14 Screenshot");
    let said: Vec<_> = run.steps.iter().filter_map(|s| s.message.clone()).collect();
    assert_eq!(
        said,
        [
            "Renamed Screenshot.png to 2026-09-14 Screenshot.png.",
            "Tagged 2026-09-14 Screenshot.png Screenshot and 2026.",
            "Moved 2026-09-14 Screenshot.png to ~/Pictures/Screenshots/2026.",
            "Copied 2026-09-14 Screenshot.png to ~/Backup.",
            "Created 2026-09-14 Screenshot.txt in ~/Pictures/Screenshots/2026.",
            "Added a row to Log.csv.",
        ]
    );
}

#[tokio::test]
async fn undoing_a_run_puts_everything_back() {
    let mut h = engine();
    fs::create_dir_all(h.home.join("Documents")).unwrap();
    fs::write(h.home.join("Documents/Log.csv"), "Name,Year\nold,2025\n").unwrap();
    let wf = tidy(&h);
    let file = h.file("Desktop/Screenshot.png");
    let before = fs::read_to_string(h.home.join("Documents/Log.csv")).unwrap();

    let id = h.engine.run_now(&wf.id, vec![file]).unwrap()[0].id.clone();
    h.until(&id, RunStatus::Done).await;
    let undone = h.engine.undo_run(&id).unwrap();

    assert_eq!(names(&h.home.join("Desktop")), ["Screenshot.png"]);
    assert!(sys::tags(&h.home.join("Desktop/Screenshot.png"))
        .unwrap()
        .is_empty());
    assert!(
        !h.home.join("Pictures").exists(),
        "folders the run made are gone"
    );
    assert!(!h.home.join("Backup").exists());
    assert_eq!(
        fs::read_to_string(h.home.join("Documents/Log.csv")).unwrap(),
        before
    );
    assert_eq!(names(&h.trash_dir()).len(), 2, "the copy and the new file");

    assert_eq!(undone.run.status, RunStatus::Undone);
    assert_eq!(undone.report.restored, 6);
    assert!(undone.report.left_alone.is_empty());
    assert_eq!(h.engine.get_run(&id).unwrap().status, RunStatus::Undone);
    assert_eq!(
        h.engine.undo_run(&id).unwrap_err().code,
        ErrorCode::Conflict
    );
}

#[tokio::test]
async fn a_failed_run_keeps_what_it_did_and_can_be_undone() {
    let mut h = engine();
    let wf = h.workflow(
        "Half done",
        steps(json!([
            { "id": "t", "type": "runNow", "next": "r" },
            { "id": "r", "type": "rename", "template": "Renamed", "next": "x" },
            { "id": "x", "type": "classify", "categories": [{ "id": "a", "label": "A" }, { "id": "b", "label": "B" }],
              "instructions": "", "branches": {} },
        ])),
    );
    // Classify needs a model to pass validation.
    set_models(&h);
    let file = h.file("Downloads/a.pdf");

    let id = h.engine.run_now(&wf.id, vec![file]).unwrap()[0].id.clone();
    let run = h.until(&id, RunStatus::Failed).await;

    assert_eq!(run.steps[1].outcome, StepOutcome::Done);
    assert_eq!(
        names(&h.home.join("Downloads")),
        ["Renamed.pdf"],
        "decision 4: kept"
    );
    h.engine.undo_run(&id).unwrap();
    assert_eq!(names(&h.home.join("Downloads")), ["a.pdf"]);
}

#[tokio::test]
async fn values_cant_steer_a_file_out_of_its_folder() {
    let mut h = engine();
    let wf = h.workflow(
        "Sneaky",
        steps(json!([
            { "id": "t", "type": "runNow", "next": "r" },
            { "id": "r", "type": "rename", "template": "{folder} {file}", "next": "m" },
            { "id": "m", "type": "move", "to": "~/Documents/{file}", "mode": "move", "next": null },
        ])),
    );
    let file = h.file("Downloads/..report.pdf");

    let id = h.engine.run_now(&wf.id, vec![file]).unwrap()[0].id.clone();
    h.until(&id, RunStatus::Done).await;

    // "/" in ~/Downloads became "-", and "..report" lost its leading dots, so
    // it can't climb out of ~/Documents or make a hidden folder.
    assert!(h
        .home
        .join("Documents/report/~-Downloads report.pdf")
        .is_file());
}

#[tokio::test]
async fn an_empty_value_fails_the_step_before_anything_moves() {
    let mut h = engine();
    let wf = h.workflow(
        "Empty",
        steps(json!([
            { "id": "t", "type": "runNow", "next": "r" },
            { "id": "r", "type": "rename", "template": "{extension}", "next": null },
        ])),
    );
    let file = h.file("Downloads/README");

    let id = h.engine.run_now(&wf.id, vec![file]).unwrap()[0].id.clone();
    let run = h.until(&id, RunStatus::Failed).await;

    assert_eq!(
        run.error.unwrap().message,
        "{extension} is empty, so it can't be used in a file or folder name."
    );
    assert_eq!(names(&h.home.join("Downloads")), ["README"]);
}

#[tokio::test]
async fn only_finished_runs_can_be_undone() {
    let h = engine();
    let err = h
        .engine
        .undo_run("0f8c2b1e-6b0a-4c1e-9d6a-2f1f6c0f7a11")
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NotFound);
}

#[tokio::test]
async fn a_crash_mid_action_is_settled_at_the_next_start() {
    let mut h = engine();
    let wf = h.workflow(
        "Renames",
        steps(json!([
            { "id": "t", "type": "runNow", "next": "r" },
            { "id": "r", "type": "rename", "template": "b", "next": null },
        ])),
    );
    let file = h.file("Downloads/a.pdf");
    let id = h.engine.run_now(&wf.id, vec![file]).unwrap()[0].id.clone();
    h.until(&id, RunStatus::Done).await;

    // As if the app had quit after renaming but before writing it down: the
    // journal's last line gone, and the run still running.
    let journal = h.root.join("engine/journal").join(format!("{id}.jsonl"));
    let text = fs::read_to_string(&journal).unwrap();
    let kept: Vec<&str> = text
        .lines()
        .filter(|l| !l.contains("\"state\":\"done\""))
        .collect();
    fs::write(&journal, kept.join("\n") + "\n").unwrap();
    let record = h.runs_dir().join(&wf.id).join(format!("{id}.json"));
    let mut run: Value = serde_json::from_slice(&fs::read(&record).unwrap()).unwrap();
    run["status"] = json!("running");
    fs::write(&record, serde_json::to_vec(&run).unwrap()).unwrap();

    let h = h.restart();

    assert_eq!(
        h.engine.get_run(&id).unwrap().status,
        RunStatus::Interrupted
    );
    // The rename was found done, so undo can reverse it.
    h.engine.undo_run(&id).unwrap();
    assert_eq!(names(&h.home.join("Downloads")), ["a.pdf"]);
}

fn set_models(h: &EngineHarness) {
    use folderflow_lib::storage::data_dir::DataDir;
    use folderflow_lib::storage::settings::{Connection, ModelRef, Settings, SettingsStore};
    let model = Some(ModelRef {
        connection_id: "c1".into(),
        model_id: "m".into(),
    });
    let settings = Settings {
        connections: vec![Connection {
            id: "c1".into(),
            provider_id: "ollama".into(),
            endpoint: None,
        }],
        defaults: [
            ("llm".to_string(), model.clone()),
            ("system1".to_string(), model),
        ]
        .into(),
        ..Settings::default()
    };
    SettingsStore::new(&DataDir::open(&h.root).unwrap())
        .save(&settings)
        .unwrap();
}
