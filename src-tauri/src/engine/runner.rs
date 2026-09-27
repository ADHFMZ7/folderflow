//! Runs one run: starts at the trigger, follows `next` or the branch each step
//! chose, fills in `{variables}`, and records what every step did.

use std::path::{Path, PathBuf};

use super::files::{sys::Stamp, Files};
use super::notices::{NoticeKind, Notices};
use super::runs::{Question, Run, RunError, RunFile, RunValue, StepOutcome, StepRun, ValueKind};
use super::values::{self, fill, fill_name, Values};
use super::Ports;
use crate::api::pickers::shorten_home;
use crate::workflow::{MoveMode, Step, StepKind};

/// More steps than any real workflow takes. Validation rules out loops; this
/// stops a run for good if one ever gets through.
const MAX_STEPS: usize = 1000;

/// Where a step leaves the run.
enum Exit {
    /// On to this step, or the end of the run.
    Next(Option<String>),
    Stop,
    /// Paused until the person answers.
    Ask(Question),
}

/// How a run that didn't fail ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Ended {
    Done,
    /// Paused on a question, in `run.waiting_for`.
    Waiting,
}

/// What a run works with: the home folder `~` stands for, the ports, and the
/// run's file actions, confined to the folders its workflow names.
pub struct Ctx<'a> {
    pub home: &'a Path,
    pub ports: &'a Ports,
    pub notices: &'a Notices,
    pub files: Files,
}

/// Runs `run` to its end or its next question, calling `save` after each
/// step. It starts at the trigger, or at `continue_at` for a run carrying on
/// after an answer, a retry or a resume; such a run first checks its file is
/// still where it left it. Returns the error it failed with, if it did.
pub async fn run(
    run: &mut Run,
    ctx: &mut Ctx<'_>,
    mut save: impl FnMut(&Run),
) -> Result<Ended, RunError> {
    let ports = ctx.ports;
    let workflow = run.workflow.clone();
    let mut current = match run.continue_at.take() {
        Some(at) => {
            still_there(run).map_err(|message| RunError {
                step_id: Some(at.clone()),
                message,
            })?;
            Some(at)
        }
        None => workflow
            .steps
            .iter()
            .find(|s| s.kind.is_trigger())
            .map(|s| s.id.clone()),
    };

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

        let result = do_step(step, run, ctx);
        let entry = run.steps.last_mut().expect("pushed above");
        entry.ended_at = Some(ports.now());
        match result {
            Ok(done) => {
                entry.outcome = StepOutcome::Done;
                entry.branch = done.branch;
                entry.message = done.message;
                run.values.extend(done.values.clone());
                entry.values = done.values;
                current = match done.exit {
                    Exit::Next(next) => next,
                    Exit::Stop => None,
                    Exit::Ask(question) => {
                        entry.outcome = StepOutcome::Waiting;
                        entry.ended_at = None;
                        run.waiting_for = Some(question);
                        // The caller saves the waiting run.
                        return Ok(Ended::Waiting);
                    }
                };
                save(run);
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
    Ok(Ended::Done)
}

/// A run carrying on must find its file where it left it, by inode.
fn still_there(run: &Run) -> Result<(), String> {
    let Some(file) = &run.file else {
        return Ok(());
    };
    match Stamp::of(&file.path) {
        Ok(now) if now.inode == file.inode => Ok(()),
        _ => Err(format!(
            "{} was moved or deleted while the run was waiting.",
            name_of(&file.path)
        )),
    }
}

/// The step after `step_id`, given the branch it took: where a run cut off
/// between two steps carries on.
pub fn after(run: &Run, step_id: &str, branch: Option<&str>) -> Option<String> {
    let step = run.workflow.steps.iter().find(|s| s.id == step_id)?;
    if matches!(step.kind, StepKind::Stop {}) {
        return None;
    }
    match (step.kind.next(), step.kind.branches(), branch) {
        (Some(next), _, _) => next.clone(),
        (_, Some(branches), Some(branch)) => branches.get(branch).cloned(),
        _ => None,
    }
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

fn do_step(step: &Step, run: &mut Run, ctx: &mut Ctx) -> Result<Done, String> {
    let filled = |text: &str, run: &Run| {
        fill(text, &run.values).map_err(|name| format!("{{{name}}} has no value at this step."))
    };
    let home = ctx.home;
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
        StepKind::Schedule { next, .. } => {
            let at = chrono::DateTime::parse_from_rfc3339(&run.started_at)
                .map_err(|_| "This run's start time can't be read.")?;
            Ok(Done {
                values: values::schedule_values(at),
                ..Done::next(next)
            })
        }
        StepKind::If {
            condition,
            branches,
        } => {
            let yes = values::holds(
                &filled(&condition.left, run)?,
                condition.op,
                &filled(&condition.right, run)?,
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
            let body = filled(message, run)?;
            let shown = ctx
                .notices
                .tell(NoticeKind::Message, run, &body, ctx.ports.now());
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
        StepKind::AskMe {
            question, answers, ..
        } => Ok(Done {
            exit: Exit::Ask(Question {
                step_id: step.id.clone(),
                question: filled(question, run)?,
                answers: answers.clone(),
            }),
            ..Done::next(&None)
        }),
        StepKind::Rename { template, next } => {
            let file = current_file(run)?;
            let stem = fill_name(template, &run.values)?;
            let to = ctx.files.rename(&file, &stem).map_err(|e| e.0)?;
            moved_to(run, &to);
            Ok(Done {
                values: text_value("newName", stem_of(&to)),
                message: Some(format!("Renamed {} to {}.", name_of(&file), name_of(&to))),
                ..Done::next(next)
            })
        }
        StepKind::Move { to, mode, next } => {
            let file = current_file(run)?;
            let folder = full_path(&fill_name(to, &run.values)?, home)?;
            let (placed, verb) = match mode {
                MoveMode::Move => {
                    let placed = ctx.files.move_file(&file, &folder).map_err(|e| e.0)?;
                    moved_to(run, &placed);
                    (placed, "Moved")
                }
                MoveMode::Copy => (
                    ctx.files.copy_file(&file, &folder).map_err(|e| e.0)?,
                    "Copied",
                ),
            };
            let shown = shown_folder(&placed, home);
            Ok(Done {
                message: Some(format!("{verb} {} to {shown}.", name_of(&file))),
                values: text_value("newFolder", shown),
                ..Done::next(next)
            })
        }
        StepKind::CreateFile {
            name,
            contents,
            folder,
            next,
        } => {
            let folder = match folder.as_deref().filter(|f| !f.trim().is_empty()) {
                Some(folder) => full_path(&fill_name(folder, &run.values)?, home)?,
                None => current_file(run)?
                    .parent()
                    .map(Path::to_path_buf)
                    .ok_or("This run's file has no folder.")?,
            };
            let name = fill_name(name, &run.values)?;
            let contents = filled(contents, run)?;
            let made = ctx
                .files
                .create_file(&folder, &name, &contents)
                .map_err(|e| e.0)?;
            Ok(Done {
                message: Some(format!(
                    "Created {} in {}.",
                    name_of(&made),
                    shown_folder(&made, home)
                )),
                ..Done::next(next)
            })
        }
        StepKind::AddRow {
            file,
            columns,
            headers,
            next,
        } => {
            let csv = full_path(&fill_name(file, &run.values)?, home)?;
            let row = columns
                .iter()
                .map(|c| filled(c, run))
                .collect::<Result<Vec<_>, _>>()?;
            ctx.files
                .add_row(&csv, &row, headers.as_deref())
                .map_err(|e| e.0)?;
            Ok(Done {
                message: Some(format!("Added a row to {}.", name_of(&csv))),
                ..Done::next(next)
            })
        }
        StepKind::Tag { tags, next } => {
            let file = current_file(run)?;
            let tags = tags
                .iter()
                .map(|t| filled(t, run))
                .collect::<Result<Vec<_>, _>>()?;
            let added = ctx.files.tag(&file, &tags).map_err(|e| e.0)?;
            let message = if added.is_empty() {
                format!("{} already had those tags.", name_of(&file))
            } else {
                format!("Tagged {} {}.", name_of(&file), and_list(&added))
            };
            Ok(Done {
                message: Some(message),
                ..Done::next(next)
            })
        }
        other => Err(format!(
            "{} steps can't run in this version of FolderFlow yet.",
            display_name(other)
        )),
    }
}

/// Where the run's file is now.
fn current_file(run: &Run) -> Result<PathBuf, String> {
    run.file
        .as_ref()
        .map(|f| f.path.clone())
        .ok_or_else(|| "This run has no file.".to_string())
}

fn moved_to(run: &mut Run, path: &Path) {
    let inode = Stamp::of(path).map(|s| s.inode).unwrap_or_default();
    run.file = Some(RunFile {
        path: path.to_path_buf(),
        inode,
    });
}

/// `~` expanded. Anything that isn't a full path is refused.
fn full_path(path: &str, home: &Path) -> Result<PathBuf, String> {
    if path == "~" {
        Ok(home.to_path_buf())
    } else if let Some(rest) = path.strip_prefix("~/") {
        Ok(home.join(rest))
    } else if path.starts_with('/') {
        Ok(PathBuf::from(path))
    } else {
        Err(format!("{path} isn't a full path, starting with ~/ or /."))
    }
}

fn text_value(name: &str, value: String) -> Values {
    Values::from([(
        name.to_owned(),
        RunValue {
            kind: ValueKind::Text,
            value,
        },
    )])
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn stem_of(path: &Path) -> String {
    let name = name_of(path);
    match name.rfind('.') {
        Some(dot) if dot > 0 => name[..dot].to_owned(),
        _ => name,
    }
}

/// The folder a file is in, with the home folder as `~`.
fn shown_folder(file: &Path, home: &Path) -> String {
    file.parent()
        .map(|p| shorten_home(p, home))
        .unwrap_or_default()
}

/// "A", "A and B", "A, B and C".
fn and_list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
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
