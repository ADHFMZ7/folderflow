//! Runs one run: starts at the trigger, follows `next` or the branch each step
//! chose, fills in `{variables}`, and records what every step did.

use std::path::Path;

use super::runs::{Run, RunError, StepOutcome, StepRun};
use super::values::{self, fill, Values};
use super::Ports;
use crate::workflow::{Step, StepKind};

/// More steps than any real workflow takes. Validation rules out loops; this
/// stops a run for good if one ever gets through.
const MAX_STEPS: usize = 1000;

/// Where a step leaves the run.
enum Exit {
    /// On to this step, or the end of the run.
    Next(Option<String>),
    Stop,
}

/// Runs `run` to the end, calling `save` after each step.
/// Returns the error it failed with, if it did.
pub async fn run(
    run: &mut Run,
    home: &Path,
    ports: &Ports,
    mut save: impl FnMut(&Run),
) -> Result<(), RunError> {
    let workflow = run.workflow.clone();
    let mut current = workflow
        .steps
        .iter()
        .find(|s| s.kind.is_trigger())
        .map(|s| s.id.clone());

    let mut taken = 0;
    while let Some(id) = current.take() {
        taken += 1;
        if taken > MAX_STEPS {
            return Err(RunError {
                step_id: Some(id),
                message: "This workflow went round in a loop, so the run was stopped.".into(),
            });
        }
        let Some(step) = workflow.steps.iter().find(|s| s.id == id) else {
            return Err(RunError {
                step_id: None,
                message: format!("The workflow has no step {id}."),
            });
        };

        run.steps.push(StepRun {
            step_id: step.id.clone(),
            title: step.title.clone(),
            kind: step.kind.type_name().into(),
            started_at: Some(ports.now()),
            ended_at: None,
            outcome: StepOutcome::Running,
            branch: None,
            values: Values::new(),
            message: None,
        });

        let result = do_step(step, run, home, ports);
        let entry = run.steps.last_mut().expect("pushed above");
        entry.ended_at = Some(ports.now());
        match result {
            Ok(done) => {
                entry.outcome = StepOutcome::Done;
                entry.branch = done.branch;
                entry.message = done.message;
                run.values.extend(done.values.clone());
                entry.values = done.values;
                save(run);
                current = match done.exit {
                    Exit::Next(next) => next,
                    Exit::Stop => None,
                };
            }
            Err(message) => {
                entry.outcome = StepOutcome::Failed;
                entry.message = Some(message.clone());
                // The caller saves the failed run.
                return Err(RunError {
                    step_id: Some(step.id.clone()),
                    message,
                });
            }
        }
    }
    Ok(())
}

struct Done {
    exit: Exit,
    branch: Option<String>,
    values: Values,
    message: Option<String>,
}

impl Done {
    fn next(next: &Option<String>) -> Self {
        Self {
            exit: Exit::Next(next.clone()),
            branch: None,
            values: Values::new(),
            message: None,
        }
    }
}

fn do_step(step: &Step, run: &Run, home: &Path, ports: &Ports) -> Result<Done, String> {
    let filled = |text: &str| {
        fill(text, &run.values).map_err(|name| format!("{{{name}}} has no value at this step."))
    };
    match &step.kind {
        StepKind::RunNow { next } | StepKind::FileAdded { next, .. } => {
            let file = run.trigger.file.as_ref().ok_or("This run has no file.")?;
            let added = chrono::DateTime::parse_from_rfc3339(&run.started_at)
                .map_err(|_| "This run's start time can't be read.")?;
            Ok(Done {
                values: values::file_values(&file.path, home, added),
                ..Done::next(next)
            })
        }
        StepKind::If {
            condition,
            branches,
        } => {
            let yes = values::holds(
                &filled(&condition.left)?,
                condition.op,
                &filled(&condition.right)?,
            );
            let branch = if yes { "yes" } else { "no" };
            Ok(Done {
                exit: Exit::Next(branches.get(branch).cloned()),
                branch: Some(branch.into()),
                values: Values::new(),
                message: None,
            })
        }
        StepKind::Notify { message, next } => {
            let body = filled(message)?;
            let shown = ports.notifier.notify(&run.workflow.name, &body);
            Ok(Done {
                message: shown
                    .err()
                    .map(|e| format!("The notification wasn't shown: {e}")),
                ..Done::next(next)
            })
        }
        StepKind::Stop {} => Ok(Done {
            exit: Exit::Stop,
            ..Done::next(&None)
        }),
        other => Err(format!(
            "{} steps can't run in this version of FolderFlow yet.",
            display_name(other)
        )),
    }
}

/// The step type as the editor names it.
fn display_name(kind: &StepKind) -> &'static str {
    match kind {
        StepKind::FileAdded { .. } => "File added",
        StepKind::Schedule { .. } => "Schedule",
        StepKind::RunNow { .. } => "Run now",
        StepKind::Classify { .. } => "Classify",
        StepKind::Extract { .. } => "Extract",
        StepKind::Write { .. } => "Write",
        StepKind::Agent { .. } => "Agent",
        StepKind::Rename { .. } => "Rename",
        StepKind::Move { .. } => "Move",
        StepKind::CreateFile { .. } => "Create file",
        StepKind::Tag { .. } => "Tag",
        StepKind::AddRow { .. } => "Add row",
        StepKind::Notify { .. } => "Notify",
        StepKind::If { .. } => "If",
        StepKind::Stop {} => "Stop",
        StepKind::AskMe { .. } => "Ask me",
    }
}
