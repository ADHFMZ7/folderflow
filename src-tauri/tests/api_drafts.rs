//! The draft commands: autosaving edits to a running workflow without touching
//! it, applying them, discarding them, and what the api reports.

mod common;

use folderflow_lib::api::types::ErrorCode;
use folderflow_lib::workflow::Workflow;

use common::api::{offline_harness, Harness};

fn running_workflow(h: &Harness) -> Workflow {
    let created = h
        .backend
        .create_workflow(Some("screenshots".into()))
        .unwrap();
    // No model kinds are needed by this template, so it can be turned on.
    h.backend
        .save_workflow(Workflow {
            enabled: true,
            ..created
        })
        .unwrap()
        .workflow
}

#[test]
fn a_draft_is_saved_read_and_applied_through_the_api() {
    let h = offline_harness();
    let live = running_workflow(&h);

    let saved = h
        .backend
        .save_draft(Workflow {
            name: "Tidy screenshots v2".into(),
            ..live.clone()
        })
        .unwrap();
    assert_eq!(h.backend.get_workflow(&live.id).unwrap(), live);
    assert_eq!(h.backend.get_draft(&live.id).unwrap(), Some(saved.workflow));

    let applied = h.backend.apply_draft(&live.id).unwrap();
    assert_eq!(applied.workflow.name, "Tidy screenshots v2");
    assert_eq!(applied.workflow.revision, live.revision + 1);
    assert_eq!(h.backend.get_draft(&live.id).unwrap(), None);
}

#[test]
fn summaries_say_which_workflows_have_changes_waiting() {
    let h = offline_harness();
    let live = running_workflow(&h);
    assert!(!h.backend.list_workflows().unwrap()[0].has_draft);

    h.backend.save_draft(live.clone()).unwrap();

    assert!(h.backend.list_workflows().unwrap()[0].has_draft);
    h.backend.discard_draft(&live.id).unwrap();
    assert!(!h.backend.list_workflows().unwrap()[0].has_draft);
}

#[test]
fn draft_errors_carry_the_contract_codes() {
    let h = offline_harness();
    let live = running_workflow(&h);

    let stale = h
        .backend
        .save_draft(Workflow {
            revision: live.revision + 5,
            ..live.clone()
        })
        .unwrap_err();
    assert_eq!(stale.code, ErrorCode::Conflict);

    let none = h.backend.apply_draft(&live.id).unwrap_err();
    assert_eq!(none.code, ErrorCode::NotFound);

    let mut broken = live.clone();
    broken.steps.remove(0);
    h.backend.save_draft(broken).unwrap();
    let unclean = h.backend.apply_draft(&live.id).unwrap_err();
    assert_eq!(unclean.code, ErrorCode::Invalid);

    let bad_id = h.backend.get_draft("../settings").unwrap_err();
    assert_eq!(bad_id.code, ErrorCode::Invalid);
}
