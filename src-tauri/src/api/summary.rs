//! The one-line summaries the workflow list shows.

use crate::storage::workflows::{Entry, Listed};
use crate::workflow::{Every, Schedule, StepKind, Workflow};

use super::catalog;
use super::types::{WorkflowStatus, WorkflowSummary};

pub fn of(listed: Listed) -> WorkflowSummary {
    let (workflow, status) = match listed.entry {
        Entry::Ok(workflow) => (workflow, WorkflowStatus::Ok),
        Entry::Damaged => return unreadable(listed.id, listed.file_name, WorkflowStatus::Damaged),
        Entry::TooNew(_) => return unreadable(listed.id, listed.file_name, WorkflowStatus::TooNew),
    };
    WorkflowSummary {
        trigger: trigger(&workflow),
        kinds_needed: kinds_needed(&workflow),
        id: workflow.id,
        name: workflow.name,
        enabled: workflow.enabled,
        last_run: None,
        needs_you: 0,
        status,
    }
}

/// A file that couldn't be read: named by its file name, everything else empty.
fn unreadable(id: String, file_name: String, status: WorkflowStatus) -> WorkflowSummary {
    WorkflowSummary {
        id,
        name: file_name,
        trigger: String::new(),
        enabled: false,
        last_run: None,
        needs_you: 0,
        kinds_needed: Vec::new(),
        status,
    }
}

/// e.g. "File added · ~/Downloads", "Schedule · Weekdays 09:00" or "Run now".
fn trigger(workflow: &Workflow) -> String {
    let Some(step) = workflow.steps.iter().find(|s| s.kind.is_trigger()) else {
        return "No trigger".into();
    };
    match &step.kind {
        StepKind::FileAdded { folder, .. } if !folder.trim().is_empty() => {
            format!("File added · {folder}")
        }
        StepKind::FileAdded { .. } => "File added".into(),
        StepKind::Schedule { schedule, .. } => format!("Schedule · {}", when(schedule)),
        _ => "Run now".into(),
    }
}

fn when(schedule: &Schedule) -> String {
    const DAYS: [&str; 7] = [
        "Sundays",
        "Mondays",
        "Tuesdays",
        "Wednesdays",
        "Thursdays",
        "Fridays",
        "Saturdays",
    ];
    let days = match schedule.every {
        Every::Day => "Every day",
        Every::Weekday => "Weekdays",
        Every::Week => schedule
            .weekday
            .and_then(|d| DAYS.get(usize::from(d)))
            .copied()
            .unwrap_or("Every week"),
    };
    format!("{days} {}", schedule.time)
}

/// The model kinds the workflow's AI steps use, each once, in catalog order.
fn kinds_needed(workflow: &Workflow) -> Vec<String> {
    catalog::model_kinds()
        .into_iter()
        .map(|k| k.id)
        .filter(|id| {
            workflow
                .steps
                .iter()
                .any(|s| s.kind.model_kind() == Some(id.as_str()))
        })
        .collect()
}
