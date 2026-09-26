//! The workflow file format, docs/workflow-format.md. The JSON is the contract
//! with the front end and with files already on disk, so field names and tagged
//! shapes here must match that document exactly.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The workflow format this build reads and writes.
pub const WORKFLOW_VERSION: u32 = 1;

fn current_version() -> u32 {
    WORKFLOW_VERSION
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Workflow {
    #[serde(default = "current_version")]
    pub version: u32,
    /// A UUID; it is also the file name.
    pub id: String,
    pub name: String,
    /// Goes up by one on every save.
    pub revision: u32,
    pub enabled: bool,
    pub steps: Vec<Step>,
}

/// One card on the canvas. The fields every step has are here; the rest,
/// including its exits, depend on its `type`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Step {
    pub id: String,
    pub title: String,
    pub position: Position,
    #[serde(flatten)]
    pub kind: StepKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

/// A step id, or `null` where the run ends.
pub type Next = Option<String>;

/// Branch id → step id. A branch with no entry ends the run.
pub type Branches = BTreeMap<String, String>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum StepKind {
    FileAdded {
        folder: String,
        /// Lowercase extensions without the dot; empty means any.
        file_types: Vec<String>,
        subfolders: bool,
        #[serde(default)]
        next: Next,
    },
    Schedule {
        schedule: Schedule,
        #[serde(default)]
        next: Next,
    },
    RunNow {
        #[serde(default)]
        next: Next,
    },
    Classify {
        categories: Vec<Category>,
        #[serde(default)]
        instructions: String,
        #[serde(default)]
        branches: Branches,
    },
    Extract {
        fields: Vec<Field>,
        if_missing: IfMissing,
        #[serde(default)]
        next: Next,
    },
    Write {
        instruction: String,
        save_as: String,
        #[serde(default)]
        next: Next,
    },
    Agent {
        instruction: String,
        abilities: Vec<String>,
        outputs: Vec<Field>,
        #[serde(default)]
        next: Next,
    },
    Rename {
        template: String,
        #[serde(default)]
        next: Next,
    },
    Move {
        to: String,
        mode: MoveMode,
        #[serde(default)]
        next: Next,
    },
    CreateFile {
        name: String,
        contents: String,
        /// Where the new file goes; missing or empty means the file's folder.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        folder: Option<String>,
        #[serde(default)]
        next: Next,
    },
    Tag {
        tags: Vec<String>,
        #[serde(default)]
        next: Next,
    },
    AddRow {
        file: String,
        columns: Vec<String>,
        /// One heading per column, written as the first row of a new file.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        headers: Option<Vec<String>>,
        #[serde(default)]
        next: Next,
    },
    Notify {
        message: String,
        #[serde(default)]
        next: Next,
    },
    If {
        condition: Condition,
        /// Keyed by `yes` and `no`.
        #[serde(default)]
        branches: Branches,
    },
    Stop {},
    AskMe {
        question: String,
        answers: Vec<Branch>,
        #[serde(default)]
        branches: Branches,
    },
}

/// A `classify` category: a branch, plus a description of what belongs in it
/// that is sent to the model with the label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Category {
    pub id: String,
    pub label: String,
    /// "How to recognise it". Guidance for the model, never filled in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub description: Option<String>,
}

/// An `askMe` answer (and the shape of every branch). The id never changes;
/// the label is only display text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Branch {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Schedule {
    pub every: Every,
    /// "HH:MM", 24-hour.
    pub time: String,
    /// 0 is Sunday. Only with `every: "week"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub weekday: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Every {
    Day,
    Weekday,
    Week,
}

/// A value an `extract` or `agent` step produces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Field {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: FieldType,
    /// "What to look for". Guidance for the model, never filled in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum FieldType {
    Text,
    Number,
    Date,
    YesNo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum IfMissing {
    Review,
    Fail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MoveMode {
    Move,
    Copy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Condition {
    pub left: String,
    pub op: Op,
    pub right: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum Op {
    #[serde(rename = ">")]
    Greater,
    #[serde(rename = "<")]
    Less,
    #[serde(rename = ">=")]
    GreaterOrEqual,
    #[serde(rename = "<=")]
    LessOrEqual,
    #[serde(rename = "=")]
    Equal,
    #[serde(rename = "!=")]
    NotEqual,
    #[serde(rename = "contains")]
    Contains,
    #[serde(rename = "startsWith")]
    StartsWith,
}

/// Something that keeps a workflow from being turned on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Problem {
    /// `null` for problems with the whole workflow.
    pub step_id: Option<String>,
    pub code: ProblemCode,
    pub message: String,
    /// The field the problem is about, as a path into the step's JSON, such as
    /// `"folder"` or `"categories.1.label"`. Missing for step-wide problems.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub field: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ProblemCode {
    NoTrigger,
    ManyTriggers,
    DuplicateId,
    MissingStep,
    UnknownBranch,
    Loop,
    Unreachable,
    Required,
    UnknownVariable,
    NoModel,
    InvalidValue,
}

/// What `save_workflow` returns: the workflow as saved, and its problems.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct SaveResult {
    pub workflow: Workflow,
    pub problems: Vec<Problem>,
}

impl StepKind {
    /// The step's `type` as written in the file.
    pub fn type_name(&self) -> &'static str {
        match self {
            StepKind::FileAdded { .. } => "fileAdded",
            StepKind::Schedule { .. } => "schedule",
            StepKind::RunNow { .. } => "runNow",
            StepKind::Classify { .. } => "classify",
            StepKind::Extract { .. } => "extract",
            StepKind::Write { .. } => "write",
            StepKind::Agent { .. } => "agent",
            StepKind::Rename { .. } => "rename",
            StepKind::Move { .. } => "move",
            StepKind::CreateFile { .. } => "createFile",
            StepKind::Tag { .. } => "tag",
            StepKind::AddRow { .. } => "addRow",
            StepKind::Notify { .. } => "notify",
            StepKind::If { .. } => "if",
            StepKind::Stop {} => "stop",
            StepKind::AskMe { .. } => "askMe",
        }
    }

    pub fn is_trigger(&self) -> bool {
        matches!(
            self,
            StepKind::FileAdded { .. } | StepKind::Schedule { .. } | StepKind::RunNow { .. }
        )
    }

    /// The model kind this step runs on, if it is an AI step.
    pub fn model_kind(&self) -> Option<&'static str> {
        match self {
            StepKind::Classify { .. } => Some("system1"),
            StepKind::Extract { .. } | StepKind::Write { .. } | StepKind::Agent { .. } => {
                Some("llm")
            }
            _ => None,
        }
    }

    /// Where a plain step goes next. `None` for branching steps and `stop`.
    pub fn next(&self) -> Option<&Next> {
        match self {
            StepKind::FileAdded { next, .. }
            | StepKind::Schedule { next, .. }
            | StepKind::RunNow { next }
            | StepKind::Extract { next, .. }
            | StepKind::Write { next, .. }
            | StepKind::Agent { next, .. }
            | StepKind::Rename { next, .. }
            | StepKind::Move { next, .. }
            | StepKind::CreateFile { next, .. }
            | StepKind::Tag { next, .. }
            | StepKind::AddRow { next, .. }
            | StepKind::Notify { next, .. } => Some(next),
            _ => None,
        }
    }

    /// A branching step's exits. `None` for plain steps and `stop`.
    pub fn branches(&self) -> Option<&Branches> {
        match self {
            StepKind::Classify { branches, .. }
            | StepKind::If { branches, .. }
            | StepKind::AskMe { branches, .. } => Some(branches),
            _ => None,
        }
    }

    /// Every step id this step's exits lead to, in a stable order.
    pub fn exits(&self) -> Vec<&str> {
        let mut out = Vec::new();
        if let Some(Some(next)) = self.next() {
            out.push(next.as_str());
        }
        if let Some(branches) = self.branches() {
            out.extend(branches.values().map(String::as_str));
        }
        out
    }
}
