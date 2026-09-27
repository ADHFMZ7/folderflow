//! Running in the background: what the menu bar and title bar are told as
//! runs come and go, and what quitting does to watching and to runs in
//! progress. See docs/engine.md, "Background".

mod common;

use std::fs;
use std::time::Duration;

use serde_json::{json, Value};
use vela_lib::engine::runs::{NeedsYouKind, RunStatus, StepOutcome};
use vela_lib::engine::Activity;

use common::engine::{engine, EngineHarness};

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

fn activity(paused: bool, running: u32, needs_you: u32) -> Activity {
    Activity {
        paused,
        running,
        needs_you,
    }
}

fn tag_downloads(h: &EngineHarness) {
    let wf = h.workflow(
        "Tag",
        steps(json!([
            { "id": "t", "type": "fileAdded", "folder": "~/Downloads", "fileTypes": [],
              "subfolders": false, "next": "g" },
            { "id": "g", "type": "tag", "tags": ["Seen"], "next": null },
        ])),
    );
    fs::create_dir_all(h.home.join("Downloads")).unwrap();
    h.switch(&wf, true);
}

#[tokio::test]
async fn activity_is_told_as_runs_come_and_go_wait_for_you_and_pause() {
    let mut h = engine();
    let wf = h.workflow(
        "Ask",
        steps(json!([
            { "id": "t", "type": "runNow", "next": "q" },
            { "id": "q", "type": "askMe", "question": "Keep {file}?",
              "answers": [{ "id": "yes", "label": "Yes" }, { "id": "no", "label": "No" }],
              "branches": { "yes": "n" } },
            { "id": "n", "type": "notify", "message": "Kept", "next": null },
        ])),
    );
    let file = h.file("Downloads/a.pdf");
    let id = h.engine.run_now(&wf.id, vec![file]).unwrap()[0].id.clone();
    h.until(&id, RunStatus::Waiting).await;
    h.engine.pause_all(true).unwrap();
    // Nothing changed, so nothing is told.
    h.engine.pause_all(true).unwrap();
    h.engine.pause_all(false).unwrap();

    assert_eq!(
        *h.activity.lock().unwrap(),
        [
            // Queued, then running: one run in progress throughout.
            activity(false, 1, 0),
            activity(false, 0, 1),
            activity(true, 0, 1),
            activity(false, 0, 1),
        ]
    );
    assert_eq!(h.engine.activity(), activity(false, 0, 1));
}

#[tokio::test]
async fn quitting_stops_watching_and_new_files_wait_for_the_next_start() {
    let mut h = engine();
    tag_downloads(&h);
    assert_eq!(h.watched.0.lock().unwrap().len(), 1);

    h.engine.stop(Duration::from_secs(1));
    assert!(h.watched.0.lock().unwrap().is_empty());
    h.file("Downloads/a.pdf");
    h.changed("Downloads");
    assert!(h.settle().await.is_empty());

    // Not recorded as seen, so the next start runs it.
    let mut h = h.restart();
    let runs = h.settle().await;
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, RunStatus::Done);
}

#[tokio::test]
async fn runs_still_in_the_queue_are_interrupted_when_the_app_quits() {
    let h = engine();
    let wf = h.workflow(
        "Say",
        steps(json!([
            { "id": "t", "type": "runNow", "next": "n" },
            { "id": "n", "type": "notify", "message": "Filed {file}", "next": null },
        ])),
    );
    let files = vec![h.file("Downloads/a.pdf"), h.file("Downloads/b.pdf")];
    h.engine.run_now(&wf.id, files).unwrap();

    // The queue hasn't had a turn yet on this test's one thread.
    h.engine.stop(Duration::from_secs(1));
    tokio::time::sleep(Duration::from_millis(50)).await;

    let runs = h.runs();
    assert_eq!(runs.len(), 2);
    assert!(runs.iter().all(|r| r.status == RunStatus::Interrupted));
    assert!(h.notes.bodies().is_empty());
    let needs = h.engine.list_needs_you().unwrap();
    assert_eq!(needs.len(), 2);
    assert!(needs.iter().all(|n| n.kind == NeedsYouKind::Interrupted));
    assert_eq!(h.engine.activity(), activity(false, 0, 2));
}

#[tokio::test]
async fn a_run_in_progress_stops_after_the_step_it_is_on_and_resumes_from_the_next() {
    let mut h = engine();
    let wf = h.workflow(
        "Say twice",
        steps(json!([
            { "id": "t", "type": "runNow", "next": "one" },
            { "id": "one", "type": "notify", "message": "First", "next": "two" },
            { "id": "two", "type": "notify", "message": "Second", "next": null },
        ])),
    );
    // Quit while the first Notify is being shown.
    let quitting = h.engine.clone();
    *h.notes.then.lock().unwrap() = Some(Box::new(move || quitting.stop(Duration::ZERO)));

    let file = h.file("Downloads/a.pdf");
    let id = h.engine.run_now(&wf.id, vec![file]).unwrap()[0].id.clone();
    let run = h.until(&id, RunStatus::Interrupted).await;
    *h.notes.then.lock().unwrap() = None;

    assert_eq!(h.notes.bodies(), ["First"]);
    let done: Vec<(&str, StepOutcome)> = run
        .steps
        .iter()
        .map(|s| (s.step_id.as_str(), s.outcome))
        .collect();
    assert_eq!(done, [("t", StepOutcome::Done), ("one", StepOutcome::Done)]);

    let mut h = h.restart();
    h.engine.resume_run(&id).unwrap();
    let run = h.until(&id, RunStatus::Done).await;
    assert_eq!(h.notes.bodies(), ["Second"]);
    assert_eq!(run.steps.last().unwrap().step_id, "two");
}
