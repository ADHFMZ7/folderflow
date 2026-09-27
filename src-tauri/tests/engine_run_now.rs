//! Run now: a workflow runs once per chosen file, follows its steps, and
//! leaves a run record that can be read back. See docs/engine.md, "Runs".

mod common;

use std::fs;

use folderflow_lib::api::types::ErrorCode;
use folderflow_lib::engine::runs::{RunQuery, RunStatus, StepOutcome, TriggerKind, ValueKind};
use serde_json::{json, Value};

use common::engine::{engine, engine_with, EngineHarness, Notes};

/// Steps as the editor saves them; positions don't matter here.
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

/// Run now → If {file} starts with "Screenshot" → yes: Notify.
fn screenshot_check(h: &EngineHarness) -> folderflow_lib::workflow::Workflow {
    h.workflow(
        "Spot screenshots",
        steps(json!([
            { "id": "t", "title": "Run now", "type": "runNow", "next": "i" },
            { "id": "i", "title": "Is it a screenshot?", "type": "if",
              "condition": { "left": "{file}", "op": "startsWith", "right": "screenshot" },
              "branches": { "yes": "n" } },
            { "id": "n", "title": "Say so", "type": "notify",
              "message": "{file}.{extension} in {folder} is a screenshot from {year}", "next": null },
        ])),
    )
}

#[tokio::test]
async fn a_run_now_workflow_runs_on_the_chosen_file_and_can_be_read_back() {
    let mut h = engine();
    let wf = screenshot_check(&h);
    let file = h.file("Desktop/Screenshot 2026-09-14.png");

    let queued = h.engine.run_now(&wf.id, vec![file]).unwrap();
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].status, RunStatus::Queued);
    assert_eq!(queued[0].workflow_id, wf.id);
    assert_eq!(queued[0].file.as_deref(), Some("Screenshot 2026-09-14.png"));

    let run = h.until(&queued[0].id, RunStatus::Done).await;

    assert_eq!(
        h.notes.bodies(),
        ["Screenshot 2026-09-14.png in ~/Desktop is a screenshot from 2026"]
    );
    assert_eq!(run.workflow_id, wf.id);
    assert_eq!(run.revision, 1);
    assert_eq!(run.workflow.steps, wf.steps);
    assert_eq!(run.trigger.kind, TriggerKind::RunNow);
    let trigger_file = run.trigger.file.as_ref().unwrap();
    assert_eq!(
        trigger_file.path,
        h.home.join("Desktop/Screenshot 2026-09-14.png")
    );
    assert!(trigger_file.inode > 0);
    assert!(run.ended_at.is_some());
    assert!(run.error.is_none());

    let taken: Vec<_> = run
        .steps
        .iter()
        .map(|s| (s.step_id.as_str(), s.outcome, s.branch.as_deref()))
        .collect();
    assert_eq!(
        taken,
        [
            ("t", StepOutcome::Done, None),
            ("i", StepOutcome::Done, Some("yes")),
            ("n", StepOutcome::Done, None),
        ]
    );
    assert_eq!(run.steps[1].title, "Is it a screenshot?");

    let value = |name: &str| {
        let v = &run.values[name];
        (v.kind, v.value.as_str())
    };
    assert_eq!(value("file"), (ValueKind::Text, "Screenshot 2026-09-14"));
    assert_eq!(value("extension"), (ValueKind::Text, "png"));
    assert_eq!(value("folder"), (ValueKind::Text, "~/Desktop"));
    assert_eq!(value("dateAdded"), (ValueKind::Date, "2026-09-14"));
    assert_eq!(value("year"), (ValueKind::Number, "2026"));
    // The trigger's step entry holds the values it produced.
    assert_eq!(run.steps[0].values["extension"].value, "png");

    let on_disk = h.runs_dir().join(&wf.id).join(format!("{}.json", run.id));
    let saved: Value = serde_json::from_slice(&fs::read(on_disk).unwrap()).unwrap();
    assert_eq!(saved["status"], "done");
    assert_eq!(saved["workflowId"], json!(wf.id));
}

#[tokio::test]
async fn the_window_hears_of_each_change_in_order() {
    let mut h = engine();
    let wf = screenshot_check(&h);
    let file = h.file("Desktop/Screenshot.png");

    let id = h.engine.run_now(&wf.id, vec![file]).unwrap()[0].id.clone();

    let mut seen = Vec::new();
    while seen.last() != Some(&RunStatus::Done) {
        let change = h.events.recv().await.unwrap();
        assert_eq!(change.run_id, id);
        assert_eq!(change.workflow_id, wf.id);
        seen.push(change.status);
    }
    assert_eq!(
        seen,
        [RunStatus::Queued, RunStatus::Running, RunStatus::Done]
    );
}

#[tokio::test]
async fn a_branch_with_no_exit_ends_the_run_and_runs_nothing_more() {
    let mut h = engine();
    let wf = screenshot_check(&h);
    let file = h.file("Documents/Invoice.pdf");

    let id = h.engine.run_now(&wf.id, vec![file]).unwrap()[0].id.clone();
    let run = h.until(&id, RunStatus::Done).await;

    assert!(h.notes.bodies().is_empty());
    let last = run.steps.last().unwrap();
    assert_eq!(
        (last.step_id.as_str(), last.branch.as_deref()),
        ("i", Some("no"))
    );
    assert_eq!(run.steps.len(), 2);
}

#[tokio::test]
async fn stop_ends_the_run_as_done() {
    let mut h = engine();
    let wf = h.workflow(
        "Stops",
        steps(json!([
            { "id": "t", "type": "runNow", "next": "i" },
            { "id": "i", "type": "if",
              "condition": { "left": "{extension}", "op": "=", "right": "PDF" },
              "branches": { "yes": "s", "no": "n" } },
            { "id": "s", "type": "stop" },
            { "id": "n", "type": "notify", "message": "Not a PDF", "next": null },
        ])),
    );
    let file = h.file("Invoice.pdf");

    let id = h.engine.run_now(&wf.id, vec![file]).unwrap()[0].id.clone();
    let run = h.until(&id, RunStatus::Done).await;

    let ids: Vec<_> = run.steps.iter().map(|s| s.step_id.as_str()).collect();
    assert_eq!(ids, ["t", "i", "s"]);
    assert!(h.notes.bodies().is_empty());
}

#[tokio::test]
async fn each_file_gets_its_own_run_one_at_a_time_in_the_order_chosen() {
    let mut h = engine();
    let wf = h.workflow(
        "Name them",
        steps(json!([
            { "id": "t", "type": "runNow", "next": "n" },
            { "id": "n", "type": "notify", "message": "{file}", "next": null },
        ])),
    );
    let files: Vec<_> = ["c.txt", "a.txt", "b.txt"]
        .iter()
        .map(|f| h.file(f))
        .collect();

    let queued = h.engine.run_now(&wf.id, files).unwrap();
    assert_eq!(queued.len(), 3);
    for run in &queued {
        h.until(&run.id, RunStatus::Done).await;
    }

    assert_eq!(h.notes.bodies(), ["c", "a", "b"]);
    let started: Vec<_> = queued
        .iter()
        .map(|r| h.engine.get_run(&r.id).unwrap())
        .collect();
    // Each run starts only after the one before it has ended.
    for pair in started.windows(2) {
        assert!(
            pair[0].ended_at.as_ref().unwrap() <= pair[1].steps[0].started_at.as_ref().unwrap()
        );
    }
}

#[tokio::test]
async fn runs_are_listed_newest_first_and_can_be_narrowed() {
    let mut h = engine();
    let one = screenshot_check(&h);
    let two = h.workflow(
        "Two",
        steps(json!([
            { "id": "t", "type": "runNow", "next": null },
        ])),
    );
    let file = h.file("Screenshot.png");

    let mut ids = Vec::new();
    for wf in [&one, &two, &one] {
        let id = h.engine.run_now(&wf.id, vec![file.clone()]).unwrap()[0]
            .id
            .clone();
        h.until(&id, RunStatus::Done).await;
        ids.push(id);
    }

    let all = h.engine.list_runs(RunQuery::default()).unwrap();
    let listed: Vec<_> = all.iter().map(|r| r.id.clone()).collect();
    assert_eq!(listed, [ids[2].clone(), ids[1].clone(), ids[0].clone()]);
    assert_eq!(all[0].workflow_name, "Spot screenshots");

    let only_one = h
        .engine
        .list_runs(RunQuery {
            workflow_id: Some(one.id.clone()),
            ..RunQuery::default()
        })
        .unwrap();
    assert_eq!(
        only_one.iter().map(|r| &r.id).collect::<Vec<_>>(),
        [&ids[2], &ids[0]]
    );

    let page = h
        .engine
        .list_runs(RunQuery {
            before: Some(ids[2].clone()),
            limit: Some(1),
            ..RunQuery::default()
        })
        .unwrap();
    assert_eq!(page.iter().map(|r| &r.id).collect::<Vec<_>>(), [&ids[1]]);

    let failed = h
        .engine
        .list_runs(RunQuery {
            status: Some(RunStatus::Failed),
            ..RunQuery::default()
        })
        .unwrap();
    assert!(failed.is_empty());
}

#[tokio::test]
async fn a_workflow_with_problems_is_refused_and_nothing_runs() {
    let h = engine();
    let wf = h.workflow(
        "Unfinished",
        steps(json!([
            { "id": "t", "type": "runNow", "next": "n" },
            { "id": "n", "type": "notify", "message": "", "next": null },
        ])),
    );
    let file = h.file("a.txt");

    let err = h.engine.run_now(&wf.id, vec![file]).unwrap_err();

    assert_eq!(err.code, ErrorCode::Invalid);
    assert_eq!(
        err.message,
        "Fix the problem with this workflow before running it."
    );
    assert!(h.engine.list_runs(RunQuery::default()).unwrap().is_empty());
}

#[tokio::test]
async fn a_missing_file_or_a_folder_is_refused_and_none_of_the_files_run() {
    let h = engine();
    let wf = screenshot_check(&h);
    let good = h.file("Screenshot.png");
    fs::create_dir_all(h.home.join("Pictures")).unwrap();

    for (bad, message) in [
        ("~/Gone.png", "Gone.png no longer exists."),
        (
            "~/Pictures",
            "Pictures is a folder. Choose files to run on.",
        ),
    ] {
        let err = h
            .engine
            .run_now(&wf.id, vec![good.clone(), bad.to_owned()])
            .unwrap_err();
        assert_eq!(
            (err.code, err.message.as_str()),
            (ErrorCode::Invalid, message)
        );
    }
    let err = h.engine.run_now(&wf.id, vec![]).unwrap_err();
    assert_eq!(err.code, ErrorCode::Invalid);

    assert!(h.engine.list_runs(RunQuery::default()).unwrap().is_empty());
    assert!(h.notes.bodies().is_empty());
}

#[tokio::test]
async fn a_linked_file_is_refused() {
    let h = engine();
    let wf = screenshot_check(&h);
    let target = h.file("Real.png");
    std::os::unix::fs::symlink(h.home.join("Real.png"), h.home.join("Link.png")).unwrap();

    let err = h
        .engine
        .run_now(&wf.id, vec![target, "~/Link.png".into()])
        .unwrap_err();

    assert_eq!(err.code, ErrorCode::Invalid);
    assert_eq!(
        err.message,
        "Link.png is a link. Choose the file it points to."
    );
}

#[tokio::test]
async fn unknown_workflows_and_runs_are_not_found() {
    let h = engine();
    let file = h.file("a.txt");

    let err = h
        .engine
        .run_now("0f8c2b1e-6b0a-4c1e-9d6a-2f1f6c0f7a11", vec![file])
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NotFound);

    let err = h
        .engine
        .get_run("0f8c2b1e-6b0a-4c1e-9d6a-2f1f6c0f7a11")
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NotFound);
    // A run id is never a path.
    let err = h.engine.get_run("../settings").unwrap_err();
    assert_eq!(err.code, ErrorCode::NotFound);
}

#[tokio::test]
async fn a_scheduled_workflow_isnt_run_on_files() {
    let h = engine();
    let wf = h.workflow(
        "Weekly",
        steps(json!([
            { "id": "t", "type": "schedule", "schedule": { "every": "day", "time": "09:00" }, "next": "n" },
            { "id": "n", "type": "notify", "message": "It's {date}", "next": null },
        ])),
    );
    let file = h.file("a.txt");

    let err = h.engine.run_now(&wf.id, vec![file]).unwrap_err();

    assert_eq!(err.code, ErrorCode::Invalid);
    assert_eq!(
        err.message,
        "This workflow runs on its schedule, not on files."
    );
}

#[tokio::test]
async fn a_step_the_engine_cant_run_yet_fails_the_run_and_says_so() {
    let mut h = engine();
    let wf = h.workflow(
        "Asks",
        steps(json!([
            { "id": "t", "type": "runNow", "next": "q" },
            { "id": "q", "title": "Sum up first", "type": "write", "instruction": "Sum up {file}",
              "saveAs": "summary", "next": "m" },
            { "id": "m", "type": "notify", "message": "Filed", "next": null },
        ])),
    );
    h.set_models();
    let file = h.file("a.txt");

    let id = h.engine.run_now(&wf.id, vec![file]).unwrap()[0].id.clone();
    let run = h.until(&id, RunStatus::Failed).await;

    let failed = run.steps.last().unwrap();
    assert_eq!(failed.step_id, "q");
    assert_eq!(failed.outcome, StepOutcome::Failed);
    let error = run.error.unwrap();
    assert_eq!(error.step_id.as_deref(), Some("q"));
    assert_eq!(
        error.message,
        "Write steps can't run in this version of FolderFlow yet."
    );
    // The failure is announced; the Notify after it never ran.
    assert_eq!(
        h.notes.shown.lock().unwrap().clone(),
        [(
            "Asks".to_string(),
            "Sum up first failed on a.txt: Write steps can't run in this version of FolderFlow yet."
                .to_string()
        )]
    );
}

#[tokio::test]
async fn a_refused_notification_doesnt_fail_the_run() {
    let mut h = engine_with(Notes {
        refuse: true,
        ..Notes::default()
    });
    let wf = screenshot_check(&h);
    let file = h.file("Screenshot.png");

    let id = h.engine.run_now(&wf.id, vec![file]).unwrap()[0].id.clone();
    let run = h.until(&id, RunStatus::Done).await;

    let notify = run.steps.last().unwrap();
    assert_eq!(notify.outcome, StepOutcome::Done);
    assert_eq!(
        notify.message.as_deref(),
        Some("The notification wasn't shown: Notifications are off for FolderFlow")
    );
}

#[tokio::test]
async fn the_run_keeps_the_workflow_it_started_with() {
    let mut h = engine();
    let wf = screenshot_check(&h);
    let file = h.file("Screenshot.png");

    let id = h.engine.run_now(&wf.id, vec![file]).unwrap()[0].id.clone();
    // Deleting the workflow afterwards changes nothing about the run.
    fs::remove_file(h.root.join("workflows").join(format!("{}.json", wf.id))).unwrap();
    let run = h.until(&id, RunStatus::Done).await;

    assert_eq!(run.workflow.name, "Spot screenshots");
    assert_eq!(h.notes.bodies().len(), 1);
}

#[tokio::test]
async fn a_run_cut_off_by_quitting_is_interrupted_at_the_next_start() {
    let mut h = engine();
    let wf = screenshot_check(&h);
    let file = h.file("Screenshot.png");
    let id = h.engine.run_now(&wf.id, vec![file]).unwrap()[0].id.clone();
    h.until(&id, RunStatus::Done).await;

    // As if the app had quit mid-run: the record still says running.
    let path = h.runs_dir().join(&wf.id).join(format!("{id}.json"));
    let mut record: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    record["status"] = json!("running");
    record["endedAt"] = Value::Null;
    fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();

    let h = h.restart();

    let run = h.engine.get_run(&id).unwrap();
    assert_eq!(run.status, RunStatus::Interrupted);
    assert!(run.ended_at.is_none(), "an interrupted run hasn't ended");
    // Nothing was run again on its own.
    assert!(h.notes.bodies().is_empty());
}

#[tokio::test]
async fn a_damaged_run_record_doesnt_hide_the_others() {
    let mut h = engine();
    let wf = screenshot_check(&h);
    let file = h.file("Screenshot.png");
    let id = h.engine.run_now(&wf.id, vec![file]).unwrap()[0].id.clone();
    h.until(&id, RunStatus::Done).await;

    fs::write(
        h.runs_dir()
            .join(&wf.id)
            .join("5d0b6a0e-0000-4000-8000-000000000000.json"),
        b"{ not json",
    )
    .unwrap();
    let h = h.restart();

    let listed = h.engine.list_runs(RunQuery::default()).unwrap();
    assert_eq!(listed.iter().map(|r| &r.id).collect::<Vec<_>>(), [&id]);
}
