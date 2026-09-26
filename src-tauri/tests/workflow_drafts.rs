//! workflows/<id>.draft.json: edits to a workflow that is on wait in a draft, so
//! a half-finished change never runs. A draft never touches the running file
//! until it's applied, and nothing is ever deleted outright.

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use folderflow_lib::storage::data_dir::DataDir;
use folderflow_lib::storage::workflows::{Entry, WorkflowError, WorkflowStore};
use folderflow_lib::workflow::templates::build;
use folderflow_lib::workflow::Workflow;

const ID: &str = "0f8c2b1e-6b0a-4c1e-9d6a-2f1f6c0f7a11";

fn all_models() -> BTreeSet<String> {
    ["llm".to_string(), "system1".to_string()].into()
}

fn live_path(dir: &DataDir) -> PathBuf {
    dir.workflows_path().join(format!("{ID}.json"))
}

fn draft_path(dir: &DataDir) -> PathBuf {
    dir.workflows_path().join(format!("{ID}.draft.json"))
}

fn trash_files(dir: &DataDir) -> Vec<String> {
    let trash = dir.workflows_path().join(".trash");
    let mut names: Vec<String> = fs::read_dir(trash)
        .map(|r| {
            r.map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// A store holding one saved workflow (the screenshots template), on or off.
fn with_workflow(enabled: bool) -> (tempfile::TempDir, DataDir, WorkflowStore, Workflow) {
    let (tmp, dir) = common::data_dir();
    let store = WorkflowStore::new(&dir);
    let created = store
        .create(Workflow {
            id: ID.into(),
            ..build("screenshots").unwrap()
        })
        .unwrap();
    let live = if enabled {
        store
            .save(
                Workflow {
                    enabled: true,
                    ..created
                },
                &all_models(),
            )
            .unwrap()
            .workflow
    } else {
        created
    };
    (tmp, dir, store, live)
}

fn renamed(w: &Workflow, name: &str) -> Workflow {
    Workflow {
        name: name.into(),
        ..w.clone()
    }
}

/// The workflow with its trigger removed, which is a problem.
fn broken(w: &Workflow) -> Workflow {
    let mut w = w.clone();
    w.steps.remove(0);
    w
}

#[test]
fn a_draft_is_saved_beside_the_running_file_and_never_changes_it() {
    let (_tmp, dir, store, live) = with_workflow(true);
    let before = fs::read(live_path(&dir)).unwrap();

    let saved = store
        .save_draft(renamed(&live, "Tidy screenshots v2"), &all_models())
        .unwrap();

    assert_eq!(saved.workflow.name, "Tidy screenshots v2");
    assert_eq!(
        saved.workflow.revision, live.revision,
        "a draft keeps the revision it started from"
    );
    assert_eq!(fs::read(live_path(&dir)).unwrap(), before);
    assert_eq!(store.get(ID).unwrap(), live);
    assert_eq!(store.get_draft(ID).unwrap(), Some(saved.workflow));
    assert_eq!(common::mode(&draft_path(&dir)), 0o600);
}

#[test]
fn a_draft_can_have_problems_and_reports_them() {
    let (_tmp, _dir, store, live) = with_workflow(true);
    let saved = store.save_draft(broken(&live), &all_models()).unwrap();
    assert!(!saved.problems.is_empty());
}

#[test]
fn a_draft_cannot_turn_a_workflow_on_or_off() {
    let (_tmp, _dir, store, live) = with_workflow(true);
    let saved = store
        .save_draft(
            Workflow {
                enabled: false,
                ..live.clone()
            },
            &all_models(),
        )
        .unwrap();
    assert!(saved.workflow.enabled);
}

#[test]
fn there_is_no_draft_until_one_is_saved() {
    let (_tmp, _dir, store, _live) = with_workflow(true);
    assert_eq!(store.get_draft(ID).unwrap(), None);
}

#[test]
fn a_draft_based_on_an_old_revision_is_refused() {
    let (_tmp, dir, store, live) = with_workflow(true);
    let stale = Workflow {
        revision: live.revision - 1,
        ..live.clone()
    };

    let err = store.save_draft(stale, &all_models()).unwrap_err();

    assert!(matches!(err, WorkflowError::Conflict { on_disk } if on_disk == live.revision));
    assert!(!draft_path(&dir).exists());
}

#[test]
fn drafts_need_an_existing_workflow_and_a_real_id() {
    let (_tmp, dir) = common::data_dir();
    let store = WorkflowStore::new(&dir);
    let wf = Workflow {
        id: ID.into(),
        revision: 1,
        ..build("screenshots").unwrap()
    };

    assert!(matches!(
        store.save_draft(wf.clone(), &all_models()),
        Err(WorkflowError::NotFound)
    ));
    assert!(matches!(
        store.get_draft("../settings"),
        Err(WorkflowError::InvalidId)
    ));
    assert!(matches!(
        store.save_draft(
            Workflow {
                id: "../settings".into(),
                ..wf
            },
            &all_models()
        ),
        Err(WorkflowError::InvalidId)
    ));
    assert!(!draft_path(&dir).exists());
}

#[test]
fn applying_makes_the_draft_the_running_workflow_and_removes_it() {
    let (_tmp, dir, store, live) = with_workflow(true);
    store
        .save_draft(renamed(&live, "Tidy screenshots v2"), &all_models())
        .unwrap();

    let applied = store.apply_draft(ID, &all_models()).unwrap();

    assert_eq!(applied.workflow.name, "Tidy screenshots v2");
    assert_eq!(applied.workflow.revision, live.revision + 1);
    assert!(applied.workflow.enabled);
    assert_eq!(store.get(ID).unwrap(), applied.workflow);
    assert_eq!(store.get_draft(ID).unwrap(), None);
    assert!(!draft_path(&dir).exists());
}

#[test]
fn a_draft_with_problems_is_never_applied_to_a_running_workflow() {
    let (_tmp, dir, store, live) = with_workflow(true);
    store.save_draft(broken(&live), &all_models()).unwrap();
    let (live_bytes, draft_bytes) = (
        fs::read(live_path(&dir)).unwrap(),
        fs::read(draft_path(&dir)).unwrap(),
    );

    let err = store.apply_draft(ID, &all_models()).unwrap_err();

    assert!(matches!(err, WorkflowError::NotClean(_)));
    assert_eq!(fs::read(live_path(&dir)).unwrap(), live_bytes);
    assert_eq!(fs::read(draft_path(&dir)).unwrap(), draft_bytes);
}

#[test]
fn a_workflow_that_is_off_can_apply_a_draft_with_problems() {
    let (_tmp, _dir, store, live) = with_workflow(false);
    store.save_draft(broken(&live), &all_models()).unwrap();

    let applied = store.apply_draft(ID, &all_models()).unwrap();

    assert!(!applied.problems.is_empty());
    assert!(!applied.workflow.enabled);
}

#[test]
fn applying_with_no_draft_is_not_found() {
    let (_tmp, _dir, store, _live) = with_workflow(true);
    assert!(matches!(
        store.apply_draft(ID, &all_models()),
        Err(WorkflowError::NotFound)
    ));
}

#[test]
fn a_draft_is_not_applied_over_a_newer_running_workflow() {
    let (_tmp, dir, store, live) = with_workflow(false);
    store
        .save_draft(renamed(&live, "From the draft"), &all_models())
        .unwrap();
    let newer = store
        .save(renamed(&live, "Saved directly"), &all_models())
        .unwrap()
        .workflow;
    let draft_bytes = fs::read(draft_path(&dir)).unwrap();

    let err = store.apply_draft(ID, &all_models()).unwrap_err();

    assert!(matches!(err, WorkflowError::Conflict { on_disk } if on_disk == newer.revision));
    assert_eq!(store.get(ID).unwrap(), newer);
    assert_eq!(fs::read(draft_path(&dir)).unwrap(), draft_bytes);
}

#[test]
fn a_stale_draft_is_moved_to_the_trash_when_read() {
    let (_tmp, dir, store, live) = with_workflow(false);
    store
        .save_draft(renamed(&live, "From the draft"), &all_models())
        .unwrap();
    let draft_bytes = fs::read(draft_path(&dir)).unwrap();
    store
        .save(renamed(&live, "Saved directly"), &all_models())
        .unwrap();

    assert_eq!(store.get_draft(ID).unwrap(), None);

    assert!(!draft_path(&dir).exists());
    let trashed = trash_files(&dir);
    assert_eq!(trashed.len(), 1);
    assert!(
        trashed[0].starts_with(&format!("{ID}.draft-")),
        "{trashed:?}"
    );
    let kept = fs::read(dir.workflows_path().join(".trash").join(&trashed[0])).unwrap();
    assert_eq!(kept, draft_bytes);
}

#[test]
fn discarding_moves_the_draft_to_the_trash() {
    let (_tmp, dir, store, live) = with_workflow(true);
    store
        .save_draft(renamed(&live, "Never mind"), &all_models())
        .unwrap();

    store.discard_draft(ID).unwrap();
    store.discard_draft(ID).unwrap();

    assert_eq!(store.get_draft(ID).unwrap(), None);
    assert_eq!(store.get(ID).unwrap(), live);
    assert_eq!(trash_files(&dir).len(), 1);
}

#[test]
fn deleting_a_workflow_moves_its_draft_to_the_trash_too() {
    let (_tmp, dir, store, live) = with_workflow(true);
    store
        .save_draft(renamed(&live, "Pending"), &all_models())
        .unwrap();

    store.delete(ID).unwrap();

    assert!(!draft_path(&dir).exists());
    let trashed = trash_files(&dir);
    assert_eq!(trashed.len(), 2, "{trashed:?}");
    assert!(trashed
        .iter()
        .any(|n| n.starts_with(&format!("{ID}.draft-"))));
}

#[test]
fn a_damaged_draft_is_left_alone_and_the_workflow_still_opens() {
    let (_tmp, dir, store, live) = with_workflow(true);
    fs::write(draft_path(&dir), b"{ not json").unwrap();

    assert!(matches!(store.get_draft(ID), Err(WorkflowError::Damaged)));
    assert_eq!(fs::read(draft_path(&dir)).unwrap(), b"{ not json");
    assert_eq!(store.get(ID).unwrap(), live);
}

#[test]
fn a_draft_from_a_newer_version_is_left_alone() {
    let (_tmp, dir, store, live) = with_workflow(true);
    let newer = format!(r#"{{ "version": 9, "id": "{ID}" }}"#);
    fs::write(draft_path(&dir), &newer).unwrap();

    assert!(matches!(
        store.get_draft(ID),
        Err(WorkflowError::TooNew { found: 9 })
    ));
    assert!(matches!(
        store.save_draft(live.clone(), &all_models()),
        Err(WorkflowError::TooNew { found: 9 })
    ));
    assert!(matches!(
        store.discard_draft(ID),
        Err(WorkflowError::TooNew { found: 9 })
    ));
    assert_eq!(fs::read_to_string(draft_path(&dir)).unwrap(), newer);
}

#[test]
fn a_draft_that_is_a_link_is_never_followed() {
    let (tmp, dir, store, live) = with_workflow(true);
    let outside = tmp.path().join("outside.json");
    fs::write(&outside, "not ours").unwrap();
    symlink(&outside, draft_path(&dir)).unwrap();

    assert!(matches!(store.get_draft(ID), Err(WorkflowError::NotAFile)));
    assert!(matches!(
        store.save_draft(live.clone(), &all_models()),
        Err(WorkflowError::NotAFile)
    ));
    assert!(matches!(
        store.apply_draft(ID, &all_models()),
        Err(WorkflowError::NotAFile)
    ));
    assert_eq!(fs::read_to_string(&outside).unwrap(), "not ours");
}

#[test]
fn listing_skips_draft_files_and_says_which_workflows_have_one() {
    let (_tmp, _dir, store, live) = with_workflow(true);
    store
        .save_draft(renamed(&live, "Pending"), &all_models())
        .unwrap();

    let listed = store.list().unwrap();

    assert_eq!(listed.len(), 1);
    assert!(matches!(listed[0].entry, Entry::Ok(_)));
    assert!(listed[0].has_draft);
}

#[test]
fn autosaves_racing_an_apply_never_corrupt_either_file() {
    let (_tmp, dir, store, live) = with_workflow(false);
    let store = Arc::new(store);
    store
        .save_draft(renamed(&live, "start"), &all_models())
        .unwrap();

    thread::scope(|s| {
        for i in 0..8 {
            let (store, live) = (Arc::clone(&store), live.clone());
            s.spawn(move || {
                for j in 0..20 {
                    let _ =
                        store.save_draft(renamed(&live, &format!("draft {i}-{j}")), &all_models());
                }
            });
        }
        let store = Arc::clone(&store);
        s.spawn(move || {
            let _ = store.apply_draft(ID, &all_models());
        });
    });

    // Whatever won, both files read back cleanly and the running one moved forward at most once.
    let running = store.get(ID).unwrap();
    assert!(running.revision == live.revision || running.revision == live.revision + 1);
    if draft_path(&dir).exists() {
        let _ = store.get_draft(ID).unwrap();
    }
}
