//! Needs you: a run at Ask me does nothing until answered, and failed and
//! interrupted runs wait for the person to retry, resume, undo or dismiss
//! them. See docs/engine.md, "Needs you", and the acting-without-consent risk
//! in TESTING.md.

mod common;

use std::fs;

use folderflow_lib::api::types::ErrorCode;
use folderflow_lib::engine::runs::{NeedsYouKind, RunStatus, RunStore, StepOutcome};
use folderflow_lib::storage::data_dir::DataDir;
use folderflow_lib::workflow::Workflow;
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

/// Run now → Ask "Log {file}?" → Log it: Rename "Logged {file}"; Skip: nothing.
fn asker(h: &EngineHarness) -> Workflow {
    h.workflow(
        "Log scans",
        steps(json!([
            { "id": "t", "type": "runNow", "next": "q" },
            { "id": "q", "type": "askMe", "title": "Log it?", "question": "Log {file}?",
              "answers": [{ "id": "log", "label": "Log it" }, { "id": "skip", "label": "Skip" }],
              "branches": { "log": "r" } },
            { "id": "r", "type": "rename", "template": "Logged {file}", "next": null },
        ])),
    )
}

#[tokio::test]
async fn a_question_pauses_the_run_until_it_is_answered() {
    let mut h = engine();
    let wf = asker(&h);
    let file = h.file("Downloads/Scan.pdf");

    let id = h.engine.run_now(&wf.id, vec![file]).unwrap()[0].id.clone();
    let run = h.until(&id, RunStatus::Waiting).await;

    // Nothing happens to the file while it waits.
    assert_eq!(names(&h.home.join("Downloads")), ["Scan.pdf"]);
    assert_eq!(run.steps.last().unwrap().outcome, StepOutcome::Waiting);
    assert_eq!(h.notes.bodies(), ["Log Scan?"]);
    let needs = h.engine.list_needs_you().unwrap();
    assert_eq!(needs.len(), 1);
    assert_eq!(needs[0].kind, NeedsYouKind::Question);
    assert_eq!(needs[0].message, "Log Scan?");
    assert_eq!(needs[0].step.as_deref(), Some("Log it?"));
    assert_eq!(needs[0].answers.len(), 2);
    assert_eq!(h.engine.workflow_runs()[&wf.id].0, 1);

    let bad = h.engine.answer(&id, "maybe").unwrap_err();
    assert_eq!(bad.code, ErrorCode::Invalid);

    h.engine.answer(&id, "log").unwrap();
    let run = h.until(&id, RunStatus::Done).await;

    assert_eq!(names(&h.home.join("Downloads")), ["Logged Scan.pdf"]);
    let said: Vec<_> = run.steps.iter().filter_map(|s| s.message.clone()).collect();
    assert_eq!(
        said,
        [
            "You answered Log it.",
            "Renamed Scan.pdf to Logged Scan.pdf."
        ]
    );
    assert!(h.engine.list_needs_you().unwrap().is_empty());
    assert_eq!(h.engine.workflow_runs()[&wf.id].0, 0);

    // Answering again, say from another window, is refused.
    let again = h.engine.answer(&id, "skip").unwrap_err();
    assert_eq!(again.code, ErrorCode::Conflict);
}

#[tokio::test]
async fn the_other_answers_branch_never_runs() {
    let mut h = engine();
    let wf = asker(&h);
    let file = h.file("Downloads/Scan.pdf");
    let id = h.engine.run_now(&wf.id, vec![file]).unwrap()[0].id.clone();
    h.until(&id, RunStatus::Waiting).await;

    // Skip leads nowhere: the run is done at once.
    let run = h.engine.answer(&id, "skip").unwrap();

    assert_eq!(run.status, RunStatus::Done);
    assert_eq!(h.settle().await.len(), 1);
    assert_eq!(names(&h.home.join("Downloads")), ["Scan.pdf"]);
}

#[tokio::test]
async fn a_waiting_run_steps_out_of_the_line_and_survives_a_restart() {
    let mut h = engine();
    let wf = asker(&h);
    let a = h.file("Downloads/A.pdf");
    let b = h.file("Downloads/B.pdf");
    let ids: Vec<String> = h
        .engine
        .run_now(&wf.id, vec![a, b])
        .unwrap()
        .into_iter()
        .map(|r| r.id)
        .collect();
    // B asks too, although A is still waiting.
    h.until(&ids[1], RunStatus::Waiting).await;
    assert_eq!(h.engine.list_needs_you().unwrap().len(), 2);

    let mut h = h.restart();
    assert_eq!(
        h.engine.get_run(&ids[0]).unwrap().status,
        RunStatus::Waiting
    );
    h.engine.answer(&ids[0], "log").unwrap();
    h.until(&ids[0], RunStatus::Done).await;
    assert_eq!(names(&h.home.join("Downloads")), ["B.pdf", "Logged A.pdf"]);
}

#[tokio::test]
async fn a_file_moved_while_the_run_waits_fails_it_instead_of_acting() {
    let mut h = engine();
    let wf = asker(&h);
    let file = h.file("Downloads/Scan.pdf");
    let id = h.engine.run_now(&wf.id, vec![file]).unwrap()[0].id.clone();
    h.until(&id, RunStatus::Waiting).await;
    fs::rename(h.home.join("Downloads/Scan.pdf"), h.home.join("Scan.pdf")).unwrap();

    h.engine.answer(&id, "log").unwrap();
    let run = h.until(&id, RunStatus::Failed).await;

    assert_eq!(
        run.error.unwrap().message,
        "Scan.pdf was moved or deleted while the run was waiting."
    );
    assert!(h.home.join("Scan.pdf").exists());
}

/// Run now → Rename "Report" → Move to ~/Archive.
fn filer(h: &EngineHarness) -> Workflow {
    h.workflow(
        "File reports",
        steps(json!([
            { "id": "t", "type": "runNow", "next": "r" },
            { "id": "r", "type": "rename", "template": "Report", "next": "m" },
            { "id": "m", "type": "move", "to": "~/Archive", "mode": "move", "next": null },
        ])),
    )
}

#[tokio::test]
async fn a_failed_run_can_be_retried_from_the_step_that_failed() {
    let mut h = engine();
    let wf = filer(&h);
    // ~/Archive is a file, so the move fails.
    fs::write(h.home.join("Archive"), "not a folder").unwrap();
    let file = h.file("Downloads/scan.pdf");
    let id = h.engine.run_now(&wf.id, vec![file]).unwrap()[0].id.clone();
    h.until(&id, RunStatus::Failed).await;

    let needs = h.engine.list_needs_you().unwrap();
    assert_eq!(needs[0].kind, NeedsYouKind::Failed);
    assert_eq!(needs[0].step.as_deref(), Some("m"));

    fs::remove_file(h.home.join("Archive")).unwrap();
    h.engine.retry_run(&id).unwrap();
    let run = h.until(&id, RunStatus::Done).await;

    assert!(h.home.join("Archive/Report.pdf").is_file());
    // The rename isn't done again; the move is tried twice.
    let kinds: Vec<_> = run
        .steps
        .iter()
        .map(|s| (s.step_id.as_str(), s.outcome))
        .collect();
    assert_eq!(
        kinds,
        [
            ("t", StepOutcome::Done),
            ("r", StepOutcome::Done),
            ("m", StepOutcome::Failed),
            ("m", StepOutcome::Done),
        ]
    );
    assert!(run.error.is_none());
    assert!(h.engine.list_needs_you().unwrap().is_empty());
    let done = h.engine.retry_run(&id).unwrap_err();
    assert_eq!(done.code, ErrorCode::Conflict);
}

#[tokio::test]
async fn dismissing_takes_a_run_out_of_needs_you_and_changes_nothing() {
    let mut h = engine();
    let wf = filer(&h);
    fs::write(h.home.join("Archive"), "not a folder").unwrap();
    let file = h.file("Downloads/scan.pdf");
    let id = h.engine.run_now(&wf.id, vec![file]).unwrap()[0].id.clone();
    h.until(&id, RunStatus::Failed).await;

    h.engine.dismiss_run(&id).unwrap();

    assert!(h.engine.list_needs_you().unwrap().is_empty());
    assert_eq!(h.engine.get_run(&id).unwrap().status, RunStatus::Failed);
    assert_eq!(names(&h.home.join("Downloads")), ["Report.pdf"]);
    // Still undoable.
    h.engine.undo_run(&id).unwrap();
    assert_eq!(names(&h.home.join("Downloads")), ["scan.pdf"]);
    assert_eq!(
        h.engine.dismiss_run(&id).unwrap_err().code,
        ErrorCode::Conflict
    );
}

#[tokio::test]
async fn an_interrupted_run_resumes_where_it_stopped() {
    let mut h = engine();
    let wf = filer(&h);
    let file = h.file("Downloads/scan.pdf");
    let id = h.engine.run_now(&wf.id, vec![file]).unwrap()[0].id.clone();
    h.until(&id, RunStatus::Done).await;
    // As if the app had quit after the rename, before the move: put the file
    // back where the rename left it, and the record back to running.
    fs::rename(
        h.home.join("Archive/Report.pdf"),
        h.home.join("Downloads/Report.pdf"),
    )
    .unwrap();
    let record = h.runs_dir().join(&wf.id).join(format!("{id}.json"));
    let mut saved: Value = serde_json::from_slice(&fs::read(&record).unwrap()).unwrap();
    saved["status"] = json!("running");
    saved["steps"].as_array_mut().unwrap().pop();
    let renamed = h.home.join("Downloads/Report.pdf");
    saved["file"]["path"] = json!(renamed);
    fs::write(&record, serde_json::to_vec(&saved).unwrap()).unwrap();

    let mut h = h.restart();
    let needs = h.engine.list_needs_you().unwrap();
    assert_eq!(needs[0].kind, NeedsYouKind::Interrupted);
    assert_eq!(needs[0].message, "Stopped at r when FolderFlow quit.");

    h.engine.resume_run(&id).unwrap();
    let run = h.until(&id, RunStatus::Done).await;

    assert!(h.home.join("Archive/Report.pdf").is_file());
    assert_eq!(run.steps.iter().filter(|s| s.step_id == "r").count(), 1);
}

#[tokio::test]
async fn the_workflow_list_knows_each_workflows_newest_run() {
    let mut h = engine();
    let wf = filer(&h);
    let a = h.file("Downloads/a.pdf");
    let b = h.file("Downloads/b.pdf");
    h.engine.run_now(&wf.id, vec![a]).unwrap();
    h.settle().await;
    h.engine.run_now(&wf.id, vec![b]).unwrap();
    h.settle().await;

    let (needs, last) = h.engine.workflow_runs()[&wf.id].clone();
    assert_eq!(needs, 0);
    assert_eq!(last.file.as_deref(), Some("b.pdf"));

    // Also after a restart, from the records.
    let h = h.restart();
    assert_eq!(
        h.engine.workflow_runs()[&wf.id].1.file.as_deref(),
        Some("b.pdf")
    );
}

#[tokio::test]
async fn old_finished_runs_are_removed_but_runs_that_need_you_are_kept() {
    let mut h = engine();
    let wf = h.workflow(
        "Tag",
        steps(json!([
            { "id": "t", "type": "runNow", "next": "g" },
            { "id": "g", "type": "tag", "tags": ["Seen"], "next": null },
        ])),
    );
    let files: Vec<String> = (0..4)
        .map(|i| h.file(&format!("Downloads/{i}.pdf")))
        .collect();
    let ids: Vec<String> = h
        .engine
        .run_now(&wf.id, files)
        .unwrap()
        .into_iter()
        .map(|r| r.id)
        .collect();
    h.settle().await;
    // The oldest one is failed: it's kept however old.
    let record = h.runs_dir().join(&wf.id).join(format!("{}.json", ids[0]));
    let mut saved: Value = serde_json::from_slice(&fs::read(&record).unwrap()).unwrap();
    saved["status"] = json!("failed");
    fs::write(&record, serde_json::to_vec(&saved).unwrap()).unwrap();

    let mut removed = Vec::new();
    RunStore::new(&DataDir::open(&h.root).unwrap())
        .prune(&wf.id, 2, |id| removed.push(id.to_string()))
        .unwrap();

    assert_eq!(removed, [ids[1].clone()]);
    let left: Vec<String> = h.runs().into_iter().map(|r| r.id).collect();
    assert_eq!(left, [ids[0].clone(), ids[2].clone(), ids[3].clone()]);
}
