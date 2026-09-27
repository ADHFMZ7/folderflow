//! The bell and Pause all: everything FolderFlow tells the person is kept in
//! a list as well as sent to macOS, and pausing stops new runs from folders
//! and schedules until resumed. See docs/engine.md, "Notifications" and
//! "Pause all".

mod common;

use std::fs;

use folderflow_lib::engine::notices::NoticeKind;
use folderflow_lib::engine::runs::RunStatus;
use serde_json::{json, Value};

use common::engine::{engine, engine_with, EngineHarness, Notes};
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

fn say(h: &EngineHarness) -> folderflow_lib::workflow::Workflow {
    h.workflow(
        "Say",
        steps(json!([
            { "id": "t", "type": "runNow", "next": "n" },
            { "id": "n", "type": "notify", "message": "Filed {file}", "next": null },
        ])),
    )
}

#[tokio::test]
async fn a_notify_step_is_kept_under_the_bell_even_when_macos_refuses_it() {
    let mut h = engine_with(Notes {
        refuse: true,
        ..Notes::default()
    });
    let wf = say(&h);
    let file = h.file("Downloads/a.pdf");
    let id = h.engine.run_now(&wf.id, vec![file]).unwrap()[0].id.clone();
    h.until(&id, RunStatus::Done).await;

    let list = h.engine.list_notices();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].kind, NoticeKind::Message);
    assert_eq!(list[0].message, "Filed a");
    assert_eq!(list[0].workflow_name, "Say");
    assert_eq!(list[0].run_id.as_deref(), Some(id.as_str()));
    assert!(!list[0].read);
}

#[tokio::test]
async fn questions_and_failures_go_to_the_bell_and_to_macos() {
    let mut h = engine();
    let asks = h.workflow(
        "Asks",
        steps(json!([
            { "id": "t", "type": "runNow", "next": "q" },
            { "id": "q", "type": "askMe", "question": "Keep {file}?",
              "answers": [{ "id": "y", "label": "Yes" }, { "id": "n", "label": "No" }], "branches": {} },
        ])),
    );
    let fails = h.workflow(
        "Fails",
        steps(json!([
            { "id": "t", "type": "runNow", "next": "r" },
            { "id": "r", "title": "Rename it", "type": "rename", "template": "{extension}", "next": null },
        ])),
    );
    let a = h.file("Downloads/a.pdf");
    let b = h.file("Downloads/README");
    let q = h.engine.run_now(&asks.id, vec![a]).unwrap()[0].id.clone();
    h.until(&q, RunStatus::Waiting).await;
    let f = h.engine.run_now(&fails.id, vec![b]).unwrap()[0].id.clone();
    h.until(&f, RunStatus::Failed).await;

    let list = h.engine.list_notices();
    let kinds: Vec<_> = list.iter().map(|n| (n.kind, n.message.as_str())).collect();
    assert_eq!(
        kinds,
        [
            (
                NoticeKind::Failed,
                "Rename it failed on README: {extension} is empty, so it can't be used in a file or folder name."
            ),
            (NoticeKind::Question, "Keep a?"),
        ],
        "newest first"
    );
    assert_eq!(h.notes.bodies().len(), 2, "and each sent to macOS");

    h.engine.mark_notices_read(Some(vec![list[1].id.clone()]));
    let read: Vec<bool> = h.engine.list_notices().iter().map(|n| n.read).collect();
    assert_eq!(read, [false, true]);
    h.engine.mark_notices_read(None);
    assert!(h.engine.list_notices().iter().all(|n| n.read));

    // Kept across a restart.
    let h = h.restart();
    assert_eq!(h.engine.list_notices().len(), 2);
}

#[tokio::test]
async fn an_undo_that_leaves_files_alone_says_so_under_the_bell() {
    let mut h = engine();
    let wf = h.workflow(
        "Rename",
        steps(json!([
            { "id": "t", "type": "runNow", "next": "r" },
            { "id": "r", "type": "rename", "template": "Renamed", "next": null },
        ])),
    );
    let file = h.file("Downloads/a.pdf");
    let id = h.engine.run_now(&wf.id, vec![file]).unwrap()[0].id.clone();
    h.until(&id, RunStatus::Done).await;
    fs::write(h.home.join("Downloads/Renamed.pdf"), "changed since").unwrap();

    h.engine.undo_run(&id).unwrap();

    let list = h.engine.list_notices();
    assert_eq!(list[0].kind, NoticeKind::Undo);
    assert_eq!(
        list[0].message,
        "Undo put back 0 of this run's changes. 1 file changed since, so it was left alone."
    );
}

#[tokio::test]
async fn pausing_holds_new_files_and_schedules_until_resumed() {
    let mut h = engine();
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

    assert!(h.engine.pause_all(true).unwrap().paused);
    h.file("Downloads/a.pdf");
    h.changed("Downloads");
    assert!(h.settle().await.is_empty());

    // Still paused after a restart.
    let mut h = h.restart();
    assert!(h.engine.activity().paused);
    h.changed("Downloads");
    assert!(h.settle().await.is_empty());

    // Resuming looks at once, so the file that waited runs.
    assert!(!h.engine.pause_all(false).unwrap().paused);
    let runs = h.settle().await;
    assert_eq!(runs.len(), 1);
    assert_eq!(names(&h.home.join("Downloads")), ["a.pdf"]);
}

#[tokio::test]
async fn run_now_still_works_while_paused_and_the_count_shows_runs_in_progress() {
    let mut h = engine();
    let wf = say(&h);
    h.engine.pause_all(true).unwrap();
    let file = h.file("Downloads/a.pdf");

    let id = h.engine.run_now(&wf.id, vec![file]).unwrap()[0].id.clone();
    assert_eq!(h.engine.activity().running, 1);
    h.until(&id, RunStatus::Done).await;
    assert_eq!(h.engine.activity().running, 0);
}
