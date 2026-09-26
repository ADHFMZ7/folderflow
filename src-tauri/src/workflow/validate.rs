//! Validation: every problem that keeps a workflow from being turned on
//! (docs/workflow-format.md, "Problems").
//!
//! The graph passes are iterative and linear or near-linear in the number of
//! exits, so any input, including a hostile one, finishes quickly without
//! recursion.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use super::format::{Every, Field, Problem, ProblemCode, Schedule, Step, StepKind, Workflow};

/// The problems with `workflow`, in step order within each kind of check.
/// `models` holds the model kinds that have a working default model.
pub fn validate(workflow: &Workflow, models: &BTreeSet<String>) -> Vec<Problem> {
    let mut out = Problems::default();

    if workflow.name.trim().is_empty() {
        out.whole(ProblemCode::Required, "Give this workflow a name.");
    }
    check_triggers(workflow, &mut out);

    let graph = Graph::new(&workflow.steps);
    check_ids(workflow, &graph, &mut out);
    check_exits(workflow, &graph, &mut out);
    for step in &workflow.steps {
        check_fields(step, &mut out);
        check_model(step, models, &mut out);
    }
    check_loops(workflow, &graph, &mut out);
    let reachable = check_reachable(workflow, &graph, &mut out);
    check_variables(workflow, &graph, &reachable, &mut out);

    out.0
}

#[derive(Default)]
struct Problems(Vec<Problem>);

impl Problems {
    fn whole(&mut self, code: ProblemCode, message: impl Into<String>) {
        self.0.push(Problem {
            step_id: None,
            code,
            message: message.into(),
            field: None,
        });
    }

    fn whole_step(&mut self, step: &Step, code: ProblemCode, message: impl Into<String>) {
        self.0.push(Problem {
            step_id: Some(step.id.clone()),
            code,
            message: message.into(),
            field: None,
        });
    }

    /// A problem with one field of a step, named by its path in the step's JSON.
    fn at(
        &mut self,
        step: &Step,
        field: impl Into<String>,
        code: ProblemCode,
        message: impl Into<String>,
    ) {
        self.0.push(Problem {
            step_id: Some(step.id.clone()),
            code,
            message: message.into(),
            field: Some(field.into()),
        });
    }
}

/// The steps as a graph. A node is a step index; exits resolve to the first
/// step with that id, and later steps sharing an id stay out of the graph
/// (they are reported as duplicates).
struct Graph {
    /// Step index → the step indexes its exits lead to (existing ones only).
    edges: Vec<Vec<usize>>,
    /// Whether the step is the first with its id, and so part of the graph.
    in_graph: Vec<bool>,
}

impl Graph {
    fn new(steps: &[Step]) -> Self {
        let mut first: HashMap<&str, usize> = HashMap::new();
        for (i, step) in steps.iter().enumerate() {
            first.entry(step.id.as_str()).or_insert(i);
        }
        let in_graph: Vec<bool> = steps
            .iter()
            .enumerate()
            .map(|(i, s)| first[s.id.as_str()] == i)
            .collect();
        let edges = steps
            .iter()
            .enumerate()
            .map(|(i, step)| {
                if !in_graph[i] {
                    return Vec::new();
                }
                let mut targets: Vec<usize> = step
                    .kind
                    .exits()
                    .into_iter()
                    .filter_map(|id| first.get(id).copied())
                    .collect();
                targets.sort_unstable();
                targets.dedup();
                targets
            })
            .collect();
        Self { edges, in_graph }
    }

    fn len(&self) -> usize {
        self.edges.len()
    }
}

fn check_triggers(workflow: &Workflow, out: &mut Problems) {
    let triggers: Vec<&Step> = workflow
        .steps
        .iter()
        .filter(|s| s.kind.is_trigger())
        .collect();
    match triggers.len() {
        0 => out.whole(
            ProblemCode::NoTrigger,
            "Add a trigger so this workflow knows when to run.",
        ),
        1 => {}
        n => {
            for step in triggers {
                out.whole_step(
                    step,
                    ProblemCode::ManyTriggers,
                    format!("A workflow has one trigger, and this one has {n}."),
                );
            }
        }
    }
}

fn check_ids(workflow: &Workflow, graph: &Graph, out: &mut Problems) {
    let mut reported: HashSet<&str> = HashSet::new();
    for (i, step) in workflow.steps.iter().enumerate() {
        if !valid_step_id(&step.id) {
            out.whole_step(
                step,
                ProblemCode::InvalidValue,
                format!(
                    "The step id \"{}\" must be 1 to 32 letters, digits, _ or -.",
                    step.id
                ),
            );
        }
        if !graph.in_graph[i] && reported.insert(step.id.as_str()) {
            out.whole_step(
                step,
                ProblemCode::DuplicateId,
                format!("More than one step has the id \"{}\".", step.id),
            );
        }
        if let Some((list, branches)) = own_branches(&step.kind) {
            let mut seen: HashSet<&str> = HashSet::new();
            let mut dup_reported: HashSet<&str> = HashSet::new();
            for (i, (id, label)) in branches.into_iter().enumerate() {
                if id.is_empty() {
                    out.at(
                        step,
                        format!("{list}.{i}"),
                        ProblemCode::InvalidValue,
                        format!("The branch \"{label}\" has no id."),
                    );
                    continue;
                }
                if !seen.insert(id) && dup_reported.insert(id) {
                    out.at(
                        step,
                        format!("{list}.{i}"),
                        ProblemCode::DuplicateId,
                        format!("Two branches of this step share the id \"{id}\"."),
                    );
                }
            }
        }
    }
}

/// The branches a step defines itself, as (id, label): its categories or
/// answers, with the name of the list they're in.
fn own_branches(kind: &StepKind) -> Option<(&'static str, Vec<(&str, &str)>)> {
    match kind {
        StepKind::Classify { categories, .. } => Some((
            "categories",
            categories
                .iter()
                .map(|c| (c.id.as_str(), c.label.as_str()))
                .collect(),
        )),
        StepKind::AskMe { answers, .. } => Some((
            "answers",
            answers
                .iter()
                .map(|a| (a.id.as_str(), a.label.as_str()))
                .collect(),
        )),
        _ => None,
    }
}

fn check_exits(workflow: &Workflow, graph: &Graph, out: &mut Problems) {
    let ids: HashSet<&str> = workflow.steps.iter().map(|s| s.id.as_str()).collect();
    for (i, step) in workflow.steps.iter().enumerate() {
        if !graph.in_graph[i] {
            continue;
        }
        for target in step.kind.exits() {
            if !ids.contains(target) {
                out.whole_step(
                    step,
                    ProblemCode::MissingStep,
                    format!("This step leads to \"{target}\", which isn't in the workflow."),
                );
            }
        }
        let Some(branches) = step.kind.branches() else {
            continue;
        };
        let known: HashSet<&str> = match &step.kind {
            StepKind::If { .. } => ["yes", "no"].into(),
            kind => own_branches(kind)
                .map(|(_, branches)| branches.into_iter().map(|(id, _)| id).collect())
                .unwrap_or_default(),
        };
        for key in branches.keys() {
            if !known.contains(key.as_str()) {
                out.whole_step(
                    step,
                    ProblemCode::UnknownBranch,
                    format!("This step has no branch \"{key}\"."),
                );
            }
        }
    }
}

fn check_fields(step: &Step, out: &mut Problems) {
    let mut required = |field: String, what: &str| {
        out.at(
            step,
            field,
            ProblemCode::Required,
            format!("Fill in {what}."),
        )
    };
    match &step.kind {
        StepKind::FileAdded { folder, .. } => {
            if blank(folder) {
                required("folder".into(), "the folder to watch");
            }
        }
        StepKind::Schedule { schedule, .. } => {
            if blank(&schedule.time) {
                required("schedule.time".into(), "the time");
            }
            if schedule.every == Every::Week && schedule.weekday.is_none() {
                required("schedule.weekday".into(), "the day of the week");
            }
        }
        StepKind::RunNow { .. } | StepKind::Stop {} => {}
        StepKind::Classify { categories, .. } => {
            if categories.len() < 2 {
                required("categories".into(), "at least two categories");
            }
            if let Some(i) = first_blank(categories.iter().map(|c| c.label.as_str())) {
                required(format!("categories.{i}.label"), "every category's name");
            }
        }
        StepKind::Extract { fields, .. } => {
            if fields.is_empty() {
                required("fields".into(), "at least one detail to pull out");
            }
            if let Some(i) = first_blank(fields.iter().map(|f| f.name.as_str())) {
                required(format!("fields.{i}.name"), "every detail's name");
            }
        }
        StepKind::Write {
            instruction,
            save_as,
            ..
        } => {
            if blank(instruction) {
                required("instruction".into(), "what to write");
            }
            if blank(save_as) {
                required("saveAs".into(), "the name to save the text as");
            }
        }
        StepKind::Agent {
            instruction,
            outputs,
            ..
        } => {
            if blank(instruction) {
                required("instruction".into(), "the instruction");
            }
            if let Some(i) = first_blank(outputs.iter().map(|f| f.name.as_str())) {
                required(format!("outputs.{i}.name"), "every output's name");
            }
        }
        StepKind::Rename { template, .. } => {
            if blank(template) {
                required("template".into(), "the new name");
            }
        }
        StepKind::Move { to, .. } => {
            if blank(to) {
                required("to".into(), "the folder to move to");
            }
        }
        StepKind::CreateFile { name, .. } => {
            if blank(name) {
                required("name".into(), "the new file's name");
            }
        }
        StepKind::Tag { tags, .. } => {
            let field = match first_blank(tags.iter().map(String::as_str)) {
                Some(i) => Some(format!("tags.{i}")),
                None if tags.is_empty() => Some("tags".into()),
                None => None,
            };
            if let Some(field) = field {
                required(field, "at least one tag, and no empty tags");
            }
        }
        StepKind::AddRow { file, columns, .. } => {
            if blank(file) {
                required("file".into(), "the spreadsheet file");
            }
            if columns.is_empty() {
                required("columns".into(), "at least one column");
            }
        }
        StepKind::Notify { message, .. } => {
            if blank(message) {
                required("message".into(), "the message");
            }
        }
        StepKind::If { condition, .. } => {
            if blank(&condition.left) {
                required("condition.left".into(), "what to compare");
            }
        }
        StepKind::AskMe {
            question, answers, ..
        } => {
            if blank(question) {
                required("question".into(), "the question");
            }
            if answers.len() < 2 {
                required("answers".into(), "at least two answers");
            }
            if let Some(i) = first_blank(answers.iter().map(|a| a.label.as_str())) {
                required(format!("answers.{i}.label"), "every answer's text");
            }
        }
    }

    check_values(step, out);
}

/// Malformed values: variable names, times, weekdays and extensions.
fn check_values(step: &Step, out: &mut Problems) {
    match &step.kind {
        StepKind::FileAdded { file_types, .. } => {
            for (i, ext) in file_types.iter().enumerate() {
                if !valid_extension(ext) {
                    out.at(
                        step,
                        format!("fileTypes.{i}"),
                        ProblemCode::InvalidValue,
                        format!(
                            "\"{ext}\" isn't a file type. Use lowercase letters and digits without the dot, like pdf."
                        ),
                    );
                }
            }
        }
        StepKind::Schedule { schedule, .. } => check_schedule(step, schedule, out),
        StepKind::Extract { fields, .. } => check_field_names(step, "fields", fields, out),
        StepKind::Agent { outputs, .. } => check_field_names(step, "outputs", outputs, out),
        StepKind::Write { save_as, .. } => {
            if !blank(save_as) && !valid_variable(save_as) {
                out.at(step, "saveAs", ProblemCode::InvalidValue, bad_name(save_as));
            }
        }
        StepKind::AddRow {
            columns,
            headers: Some(headers),
            ..
        } if headers.len() != columns.len() => out.at(
            step,
            "headers",
            ProblemCode::InvalidValue,
            format!(
                "There are {} headings for {} columns. Give each column one heading.",
                headers.len(),
                columns.len()
            ),
        ),
        _ => {}
    }
}

fn check_schedule(step: &Step, schedule: &Schedule, out: &mut Problems) {
    if !blank(&schedule.time) && !valid_time(&schedule.time) {
        out.at(
            step,
            "schedule.time",
            ProblemCode::InvalidValue,
            format!(
                "\"{}\" isn't a time. Use 24-hour HH:MM, like 09:00.",
                schedule.time
            ),
        );
    }
    match schedule.weekday {
        Some(day) if schedule.every != Every::Week => out.at(
            step,
            "schedule.weekday",
            ProblemCode::InvalidValue,
            format!("A day of the week ({day}) only goes with a weekly schedule."),
        ),
        Some(day) if day > 6 => out.at(
            step,
            "schedule.weekday",
            ProblemCode::InvalidValue,
            format!("{day} isn't a day of the week. Use 0 (Sunday) to 6 (Saturday)."),
        ),
        _ => {}
    }
}

fn check_field_names(step: &Step, list: &str, fields: &[Field], out: &mut Problems) {
    let mut seen: HashSet<&str> = HashSet::new();
    for (i, field) in fields.iter().enumerate() {
        if blank(&field.name) {
            continue;
        }
        let path = format!("{list}.{i}.name");
        if !valid_variable(&field.name) {
            out.at(step, path, ProblemCode::InvalidValue, bad_name(&field.name));
        } else if !seen.insert(field.name.as_str()) {
            out.at(
                step,
                path,
                ProblemCode::InvalidValue,
                format!("The name \"{}\" is used twice in this step.", field.name),
            );
        }
    }
}

fn bad_name(name: &str) -> String {
    format!("\"{name}\" isn't a valid name. Use 1 to 32 letters, digits or _.")
}

fn check_model(step: &Step, models: &BTreeSet<String>, out: &mut Problems) {
    let Some(kind) = step.kind.model_kind() else {
        return;
    };
    if !models.contains(kind) {
        let name = match kind {
            "system1" => "System 1",
            _ => "LLM",
        };
        out.whole_step(
            step,
            ProblemCode::NoModel,
            format!("This step needs a default {name} model. Choose one in Settings."),
        );
    }
}

/// Marks every step on a cycle: a step whose exits can lead back to itself.
/// Kosaraju's algorithm with explicit stacks, so deep graphs can't overflow.
fn check_loops(workflow: &Workflow, graph: &Graph, out: &mut Problems) {
    let n = graph.len();

    // Pass 1: finish order of a depth-first search over every node.
    let mut visited = vec![false; n];
    let mut order = Vec::with_capacity(n);
    for start in 0..n {
        if visited[start] {
            continue;
        }
        visited[start] = true;
        let mut stack = vec![(start, 0usize)];
        while let Some((node, edge)) = stack.last_mut() {
            if let Some(&next) = graph.edges[*node].get(*edge) {
                *edge += 1;
                if !visited[next] {
                    visited[next] = true;
                    stack.push((next, 0));
                }
            } else {
                order.push(*node);
                stack.pop();
            }
        }
    }

    // Pass 2: components of the reversed graph, in reverse finish order.
    let mut reverse = vec![Vec::new(); n];
    for (from, targets) in graph.edges.iter().enumerate() {
        for &to in targets {
            reverse[to].push(from);
        }
    }
    let mut component = vec![usize::MAX; n];
    let mut sizes = Vec::new();
    for &start in order.iter().rev() {
        if component[start] != usize::MAX {
            continue;
        }
        let id = sizes.len();
        let mut size = 0;
        component[start] = id;
        let mut stack = vec![start];
        while let Some(node) = stack.pop() {
            size += 1;
            for &prev in &reverse[node] {
                if component[prev] == usize::MAX {
                    component[prev] = id;
                    stack.push(prev);
                }
            }
        }
        sizes.push(size);
    }

    for (i, step) in workflow.steps.iter().enumerate() {
        if !graph.in_graph[i] {
            continue;
        }
        let on_cycle = sizes[component[i]] > 1 || graph.edges[i].contains(&i);
        if on_cycle {
            out.whole_step(
                step,
                ProblemCode::Loop,
                "Following this step's exits leads back to it.",
            );
        }
    }
}

/// Marks steps no path from a trigger reaches, and returns which steps are
/// reachable. With no trigger nothing is marked: `no_trigger` says it all.
fn check_reachable(workflow: &Workflow, graph: &Graph, out: &mut Problems) -> Vec<bool> {
    let mut reachable = vec![false; graph.len()];
    let mut queue: VecDeque<usize> = VecDeque::new();
    for (i, step) in workflow.steps.iter().enumerate() {
        if graph.in_graph[i] && step.kind.is_trigger() {
            reachable[i] = true;
            queue.push_back(i);
        }
    }
    if queue.is_empty() {
        return reachable;
    }
    while let Some(node) = queue.pop_front() {
        for &next in &graph.edges[node] {
            if !reachable[next] {
                reachable[next] = true;
                queue.push_back(next);
            }
        }
    }
    for (i, step) in workflow.steps.iter().enumerate() {
        if graph.in_graph[i] && !reachable[i] {
            out.whole_step(
                step,
                ProblemCode::Unreachable,
                "No path from the trigger reaches this step.",
            );
        }
    }
    reachable
}

/// A `{name}` is known at a step when the trigger or a step on every path from
/// the trigger to it produces it: the intersection over all incoming paths,
/// found by iterating to a fixed point. Sets only shrink, so it terminates.
fn check_variables(workflow: &Workflow, graph: &Graph, reachable: &[bool], out: &mut Problems) {
    let n = graph.len();
    let steps = &workflow.steps;

    // Known on entry to each reachable step; `None` means "not yet reached".
    let mut known: Vec<Option<BTreeSet<String>>> = vec![None; n];
    let mut queue: VecDeque<usize> = VecDeque::new();
    let mut queued = vec![false; n];
    for i in 0..n {
        if reachable[i] && steps[i].kind.is_trigger() {
            known[i] = Some(BTreeSet::new());
            queue.push_back(i);
            queued[i] = true;
        }
    }
    while let Some(node) = queue.pop_front() {
        queued[node] = false;
        let mut after = known[node].clone().unwrap_or_default();
        after.extend(produces(&steps[node].kind));
        for &next in &graph.edges[node] {
            let changed = match &mut known[next] {
                slot @ None => {
                    *slot = Some(after.clone());
                    true
                }
                Some(set) => {
                    let before = set.len();
                    set.retain(|v| after.contains(v));
                    set.len() != before
                }
            };
            if changed && !queued[next] {
                queued[next] = true;
                queue.push_back(next);
            }
        }
    }

    for (i, step) in steps.iter().enumerate() {
        let Some(available) = known.get(i).and_then(Option::as_ref) else {
            continue;
        };
        if !graph.in_graph[i] || !reachable[i] {
            continue;
        }
        let mut reported: HashSet<&str> = HashSet::new();
        for (field, text) in texts(&step.kind) {
            for name in variables_in(text) {
                if !available.contains(name) && reported.insert(name) {
                    out.at(
                        step,
                        field.clone(),
                        ProblemCode::UnknownVariable,
                        format!("{{{name}}} isn't produced by any step before this one."),
                    );
                }
            }
        }
    }
}

/// The variables a step makes available to the steps after it.
fn produces(kind: &StepKind) -> Vec<String> {
    let names = |v: &[&str]| v.iter().map(|s| s.to_string()).collect();
    match kind {
        StepKind::FileAdded { .. } | StepKind::RunNow { .. } => {
            names(&["file", "extension", "folder", "dateAdded", "year"])
        }
        StepKind::Schedule { .. } => names(&["date", "year"]),
        StepKind::Classify { .. } => names(&["category"]),
        StepKind::Extract { fields, .. } => fields.iter().map(|f| f.name.clone()).collect(),
        StepKind::Write { save_as, .. } => vec![save_as.clone()],
        StepKind::Agent { outputs, .. } => outputs.iter().map(|f| f.name.clone()).collect(),
        StepKind::Rename { .. } => names(&["newName"]),
        StepKind::Move { .. } => names(&["newFolder"]),
        StepKind::AskMe { .. } => names(&["answer"]),
        _ => Vec::new(),
    }
}

/// The text fields of a step that may use `{variables}`, in display order,
/// each with its path in the step's JSON.
fn texts(kind: &StepKind) -> Vec<(String, &str)> {
    let mut out: Vec<(String, &str)> = Vec::new();
    match kind {
        StepKind::FileAdded { folder, .. } => out.push(("folder".into(), folder)),
        StepKind::Classify { instructions, .. } => out.push(("instructions".into(), instructions)),
        StepKind::Write { instruction, .. } | StepKind::Agent { instruction, .. } => {
            out.push(("instruction".into(), instruction))
        }
        StepKind::Rename { template, .. } => out.push(("template".into(), template)),
        StepKind::Move { to, .. } => out.push(("to".into(), to)),
        StepKind::CreateFile {
            name,
            folder,
            contents,
            ..
        } => {
            out.push(("name".into(), name));
            if let Some(folder) = folder {
                out.push(("folder".into(), folder));
            }
            out.push(("contents".into(), contents));
        }
        StepKind::Tag { tags, .. } => {
            for (i, tag) in tags.iter().enumerate() {
                out.push((format!("tags.{i}"), tag));
            }
        }
        StepKind::AddRow { file, columns, .. } => {
            out.push(("file".into(), file));
            for (i, column) in columns.iter().enumerate() {
                out.push((format!("columns.{i}"), column));
            }
        }
        StepKind::Notify { message, .. } => out.push(("message".into(), message)),
        StepKind::If { condition, .. } => {
            out.push(("condition.left".into(), &condition.left));
            out.push(("condition.right".into(), &condition.right));
        }
        StepKind::AskMe { question, .. } => out.push(("question".into(), question)),
        StepKind::Schedule { .. }
        | StepKind::RunNow { .. }
        | StepKind::Extract { .. }
        | StepKind::Stop {} => {}
    }
    out
}

/// Each `{name}` in `text` whose name is a valid variable name. Other braces
/// are plain text.
pub fn variables_in(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        match after.find(['{', '}']) {
            Some(close) if after.as_bytes()[close] == b'}' => {
                let name = &after[..close];
                if valid_variable(name) {
                    out.push(name);
                }
                rest = &after[close + 1..];
            }
            Some(close) => rest = &after[close..],
            None => break,
        }
    }
    out
}

/// The index of the first blank text.
fn first_blank<'a>(items: impl IntoIterator<Item = &'a str>) -> Option<usize> {
    items.into_iter().position(blank)
}

fn blank(s: &str) -> bool {
    s.trim().is_empty()
}

/// 1 to 32 characters from A-Z a-z 0-9 _.
pub fn valid_variable(name: &str) -> bool {
    (1..=32).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// 1 to 32 characters from A-Z a-z 0-9 _ -.
pub fn valid_step_id(id: &str) -> bool {
    (1..=32).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn valid_extension(ext: &str) -> bool {
    (1..=32).contains(&ext.len())
        && ext
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

/// "HH:MM", 00:00 to 23:59.
fn valid_time(time: &str) -> bool {
    let b = time.as_bytes();
    if b.len() != 5 || b[2] != b':' {
        return false;
    }
    let digits = [b[0], b[1], b[3], b[4]];
    if !digits.iter().all(u8::is_ascii_digit) {
        return false;
    }
    let hours = (b[0] - b'0') * 10 + (b[1] - b'0');
    let minutes = (b[3] - b'0') * 10 + (b[4] - b'0');
    hours < 24 && minutes < 60
}
