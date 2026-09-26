//! Validation: one test per problem code in docs/workflow-format.md, variables
//! that are only produced on some paths, and graphs built to hang or crash it.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use folderflow_lib::workflow::{
    validate, Branch, Condition, Every, Field, FieldType, IfMissing, MoveMode, Op, Position,
    Problem, ProblemCode, Schedule, Step, StepKind, Workflow,
};

use ProblemCode::*;

fn all_models() -> BTreeSet<String> {
    ["llm".to_string(), "system1".to_string()].into()
}

fn no_models() -> BTreeSet<String> {
    BTreeSet::new()
}

fn wf(steps: Vec<Step>) -> Workflow {
    Workflow {
        version: 1,
        id: "0f8c2b1e-6b0a-4c1e-9d6a-2f1f6c0f7a11".into(),
        name: "Test".into(),
        revision: 1,
        enabled: false,
        steps,
    }
}

fn step(id: &str, kind: StepKind) -> Step {
    Step {
        id: id.into(),
        title: format!("Step {id}"),
        position: Position { x: 0.0, y: 0.0 },
        kind,
    }
}

fn next(id: Option<&str>) -> Option<String> {
    id.map(str::to_owned)
}

fn file_added(id: &str, to: Option<&str>) -> Step {
    step(
        id,
        StepKind::FileAdded {
            folder: "~/Downloads".into(),
            file_types: vec!["pdf".into()],
            subfolders: false,
            next: next(to),
        },
    )
}

fn scheduled(id: &str, schedule: Schedule, to: Option<&str>) -> Step {
    step(
        id,
        StepKind::Schedule {
            schedule,
            next: next(to),
        },
    )
}

fn weekly(time: &str, weekday: Option<u8>) -> Schedule {
    Schedule {
        every: Every::Week,
        time: time.into(),
        weekday,
    }
}

fn notify(id: &str, message: &str, to: Option<&str>) -> Step {
    step(
        id,
        StepKind::Notify {
            message: message.into(),
            next: next(to),
        },
    )
}

fn write(id: &str, save_as: &str, to: Option<&str>) -> Step {
    step(
        id,
        StepKind::Write {
            instruction: "Summarise it.".into(),
            save_as: save_as.into(),
            next: next(to),
        },
    )
}

fn if_step(id: &str, left: &str, branches: &[(&str, &str)]) -> Step {
    step(
        id,
        StepKind::If {
            condition: Condition {
                left: left.into(),
                op: Op::Greater,
                right: "500".into(),
            },
            branches: pairs(branches),
        },
    )
}

fn classify(id: &str, categories: &[(&str, &str)], branches: &[(&str, &str)]) -> Step {
    step(
        id,
        StepKind::Classify {
            categories: labels(categories),
            instructions: String::new(),
            branches: pairs(branches),
        },
    )
}

fn ask(id: &str, answers: &[(&str, &str)], branches: &[(&str, &str)]) -> Step {
    step(
        id,
        StepKind::AskMe {
            question: "Log it?".into(),
            answers: labels(answers),
            branches: pairs(branches),
        },
    )
}

fn extract(id: &str, fields: &[&str], to: Option<&str>) -> Step {
    step(
        id,
        StepKind::Extract {
            fields: fields
                .iter()
                .map(|name| Field {
                    name: (*name).into(),
                    kind: FieldType::Text,
                })
                .collect(),
            if_missing: IfMissing::Review,
            next: next(to),
        },
    )
}

fn pairs(pairs: &[(&str, &str)]) -> std::collections::BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn labels(items: &[(&str, &str)]) -> Vec<Branch> {
    items
        .iter()
        .map(|(id, label)| Branch {
            id: (*id).into(),
            label: (*label).into(),
        })
        .collect()
}

/// (step id, code) for each problem, sorted, so tests read as a set.
fn found(workflow: &Workflow) -> Vec<(Option<String>, ProblemCode)> {
    found_with(workflow, &all_models())
}

fn found_with(
    workflow: &Workflow,
    models: &BTreeSet<String>,
) -> Vec<(Option<String>, ProblemCode)> {
    let problems = validate(workflow, models);
    for p in &problems {
        assert!(
            p.message.ends_with('.') && p.message.len() > 10,
            "a problem message should be a sentence: {p:?}"
        );
    }
    let mut out: Vec<_> = problems.into_iter().map(|p| (p.step_id, p.code)).collect();
    out.sort();
    out
}

fn at(id: &str, code: ProblemCode) -> (Option<String>, ProblemCode) {
    (Some(id.into()), code)
}

fn problems_of(workflow: &Workflow, code: ProblemCode) -> Vec<Problem> {
    validate(workflow, &all_models())
        .into_iter()
        .filter(|p| p.code == code)
        .collect()
}

#[test]
fn a_simple_workflow_has_no_problems() {
    let w = wf(vec![
        file_added("t", Some("s1")),
        notify("s1", "Got {file}.{extension}", None),
    ]);
    assert_eq!(found(&w), []);
}

#[test]
fn no_trigger_is_a_problem_with_the_whole_workflow() {
    let w = wf(vec![notify("s1", "Hi.", None)]);
    assert_eq!(found(&w), [(None, NoTrigger)]);
}

#[test]
fn an_empty_workflow_has_no_trigger() {
    assert_eq!(found(&wf(vec![])), [(None, NoTrigger)]);
}

#[test]
fn every_trigger_is_marked_when_there_are_several() {
    let w = wf(vec![
        file_added("t1", Some("s1")),
        step(
            "t2",
            StepKind::RunNow {
                next: next(Some("s1")),
            },
        ),
        notify("s1", "Hi.", None),
    ]);
    assert_eq!(found(&w), [at("t1", ManyTriggers), at("t2", ManyTriggers)]);
}

#[test]
fn two_steps_with_one_id_are_a_duplicate() {
    let w = wf(vec![
        file_added("t", Some("s1")),
        notify("s1", "One.", None),
        notify("s1", "Two.", None),
    ]);
    assert_eq!(found(&w), [at("s1", DuplicateId)]);
}

#[test]
fn two_branches_with_one_id_in_a_step_are_a_duplicate() {
    let w = wf(vec![
        file_added("t", Some("c")),
        classify("c", &[("c1", "Receipt"), ("c1", "Other")], &[]),
    ]);
    assert_eq!(found(&w), [at("c", DuplicateId)]);
}

#[test]
fn an_exit_to_a_step_that_does_not_exist_is_missing() {
    let w = wf(vec![file_added("t", Some("ghost"))]);
    let problems = problems_of(&w, MissingStep);
    assert_eq!(found(&w), [at("t", MissingStep)]);
    assert!(problems[0].message.contains("ghost"), "{problems:?}");
}

#[test]
fn a_branch_to_a_step_that_does_not_exist_is_missing() {
    let w = wf(vec![
        file_added("t", Some("i")),
        if_step("i", "{file}", &[("yes", "ghost"), ("no", "gone")]),
    ]);
    assert_eq!(found(&w), [at("i", MissingStep), at("i", MissingStep)]);
}

#[test]
fn an_if_branch_other_than_yes_or_no_is_unknown() {
    let w = wf(vec![
        file_added("t", Some("i")),
        if_step("i", "{file}", &[("maybe", "n")]),
        notify("n", "Hi.", None),
    ]);
    let problems = problems_of(&w, UnknownBranch);
    assert_eq!(found(&w), [at("i", UnknownBranch)]);
    assert!(problems[0].message.contains("maybe"), "{problems:?}");
}

#[test]
fn a_classify_branch_that_is_not_a_category_is_unknown() {
    let w = wf(vec![
        file_added("t", Some("c")),
        classify(
            "c",
            &[("c1", "Receipt"), ("c2", "Other")],
            &[("c1", "n"), ("c9", "n")],
        ),
        notify("n", "Hi.", None),
    ]);
    assert_eq!(found(&w), [at("c", UnknownBranch)]);
}

#[test]
fn an_ask_me_branch_that_is_not_an_answer_is_unknown() {
    let w = wf(vec![
        file_added("t", Some("a")),
        ask("a", &[("a1", "Yes"), ("a2", "No")], &[("a3", "n")]),
        notify("n", "Hi.", None),
    ]);
    assert_eq!(found(&w), [at("a", UnknownBranch)]);
}

#[test]
fn a_path_back_to_an_earlier_step_is_a_loop() {
    let w = wf(vec![
        file_added("t", Some("s1")),
        notify("s1", "One.", Some("s2")),
        notify("s2", "Two.", Some("s1")),
    ]);
    assert_eq!(found(&w), [at("s1", Loop), at("s2", Loop)]);
}

#[test]
fn a_step_leading_to_itself_is_a_loop() {
    let w = wf(vec![
        file_added("t", Some("s1")),
        notify("s1", "Again.", Some("s1")),
    ]);
    assert_eq!(found(&w), [at("s1", Loop)]);
}

#[test]
fn a_branch_back_to_the_trigger_is_a_loop() {
    let w = wf(vec![
        file_added("t", Some("i")),
        if_step("i", "{file}", &[("yes", "t")]),
    ]);
    assert_eq!(found(&w), [at("i", Loop), at("t", Loop)]);
}

#[test]
fn two_paths_meeting_again_are_not_a_loop() {
    let w = wf(vec![
        file_added("t", Some("i")),
        if_step("i", "{file}", &[("yes", "a"), ("no", "b")]),
        notify("a", "A.", Some("end")),
        notify("b", "B.", Some("end")),
        notify("end", "End.", None),
    ]);
    assert_eq!(found(&w), []);
}

#[test]
fn a_step_no_path_reaches_is_unreachable() {
    let w = wf(vec![
        file_added("t", None),
        notify("lonely", "Hi.", Some("after")),
        notify("after", "Hi.", None),
    ]);
    assert_eq!(
        found(&w),
        [at("after", Unreachable), at("lonely", Unreachable)]
    );
}

#[test]
fn an_empty_name_is_required() {
    let mut w = wf(vec![file_added("t", None)]);
    w.name = "  ".into();
    assert_eq!(found(&w), [(None, Required)]);
}

#[test]
fn empty_required_fields_are_required() {
    let w = wf(vec![
        step(
            "t",
            StepKind::FileAdded {
                folder: String::new(),
                file_types: vec![],
                subfolders: false,
                next: next(Some("n")),
            },
        ),
        notify("n", "", Some("w")),
        write("w", "", Some("r")),
        step(
            "r",
            StepKind::Rename {
                template: String::new(),
                next: next(Some("m")),
            },
        ),
        step(
            "m",
            StepKind::Move {
                to: String::new(),
                mode: MoveMode::Move,
                next: next(Some("tag")),
            },
        ),
        step(
            "tag",
            StepKind::Tag {
                tags: vec![],
                next: next(Some("row")),
            },
        ),
        step(
            "row",
            StepKind::AddRow {
                file: String::new(),
                columns: vec![],
                next: next(Some("cf")),
            },
        ),
        step(
            "cf",
            StepKind::CreateFile {
                name: String::new(),
                contents: String::new(),
                next: next(Some("ag")),
            },
        ),
        step(
            "ag",
            StepKind::Agent {
                instruction: String::new(),
                abilities: vec![],
                outputs: vec![],
                next: next(Some("if")),
            },
        ),
        step(
            "if",
            StepKind::If {
                condition: Condition {
                    left: String::new(),
                    op: Op::Equal,
                    right: String::new(),
                },
                branches: pairs(&[("yes", "x")]),
            },
        ),
        step(
            "x",
            StepKind::AskMe {
                question: String::new(),
                answers: labels(&[("a1", "Only one")]),
                branches: pairs(&[]),
            },
        ),
    ]);

    assert_eq!(
        found(&w),
        [
            at("ag", Required),
            at("cf", Required),
            at("if", Required),
            at("m", Required),
            at("n", Required),
            at("r", Required),
            at("row", Required),
            at("row", Required),
            at("t", Required),
            at("tag", Required),
            at("w", Required),
            at("x", Required),
            at("x", Required),
        ]
    );
}

#[test]
fn classify_needs_two_named_categories_and_extract_one_field() {
    let w = wf(vec![
        file_added("t", Some("c")),
        classify("c", &[("c1", "Receipt")], &[("c1", "e")]),
        extract("e", &[], Some("c2")),
        classify("c2", &[("c1", "Receipt"), ("c2", " ")], &[]),
    ]);
    assert_eq!(
        found(&w),
        [at("c", Required), at("c2", Required), at("e", Required)]
    );
}

#[test]
fn a_weekly_schedule_needs_a_weekday() {
    let w = wf(vec![scheduled("t", weekly("17:00", None), None)]);
    assert_eq!(found(&w), [at("t", Required)]);
}

#[test]
fn an_empty_tag_is_required() {
    let w = wf(vec![
        file_added("t", Some("g")),
        step(
            "g",
            StepKind::Tag {
                tags: vec!["Receipt".into(), "".into()],
                next: None,
            },
        ),
    ]);
    assert_eq!(found(&w), [at("g", Required)]);
}

#[test]
fn a_variable_no_earlier_step_produces_is_unknown() {
    let w = wf(vec![
        file_added("t", Some("n")),
        notify("n", "Due {due_date}.", None),
    ]);
    let problems = validate(&w, &all_models());
    assert_eq!(
        problems,
        [Problem {
            step_id: Some("n".into()),
            code: UnknownVariable,
            message: "{due_date} isn't produced by any step before this one.".into(),
        }]
    );
}

#[test]
fn the_trigger_variables_are_known_to_every_step() {
    let w = wf(vec![
        file_added("t", Some("n")),
        notify("n", "{file} {extension} {folder} {dateAdded} {year}.", None),
    ]);
    assert_eq!(found(&w), []);
}

#[test]
fn a_schedule_gives_date_and_year_but_no_file() {
    let w = wf(vec![
        scheduled("t", weekly("17:00", Some(5)), Some("n")),
        notify("n", "{date} {year} {file}.", None),
    ]);
    let problems = problems_of(&w, UnknownVariable);
    assert_eq!(found(&w), [at("n", UnknownVariable)]);
    assert!(problems[0].message.starts_with("{file}"), "{problems:?}");
}

#[test]
fn each_step_produces_what_the_contract_lists() {
    let w = wf(vec![
        file_added("t", Some("c")),
        classify(
            "c",
            &[("c1", "A"), ("c2", "B")],
            &[("c1", "e"), ("c2", "e")],
        ),
        extract("e", &["vendor", "amount"], Some("w")),
        write("w", "summary", Some("ag")),
        step(
            "ag",
            StepKind::Agent {
                instruction: "Look.".into(),
                abilities: vec!["readFile".into()],
                outputs: vec![Field {
                    name: "total".into(),
                    kind: FieldType::Number,
                }],
                next: next(Some("r")),
            },
        ),
        step(
            "r",
            StepKind::Rename {
                template: "{vendor}".into(),
                next: next(Some("m")),
            },
        ),
        step(
            "m",
            StepKind::Move {
                to: "~/{year}".into(),
                mode: MoveMode::Move,
                next: next(Some("a")),
            },
        ),
        ask("a", &[("a1", "Yes"), ("a2", "No")], &[("a1", "n")]),
        notify(
            "n",
            "{category} {vendor} {amount} {summary} {total} {newName} {newFolder} {answer}.",
            None,
        ),
    ]);
    assert_eq!(found(&w), []);
}

#[test]
fn a_step_cannot_use_what_it_produces_itself() {
    let w = wf(vec![
        file_added("t", Some("r")),
        step(
            "r",
            StepKind::Rename {
                template: "{newName}".into(),
                next: None,
            },
        ),
    ]);
    assert_eq!(found(&w), [at("r", UnknownVariable)]);
}

#[test]
fn a_variable_produced_on_only_one_incoming_path_is_unknown_at_the_merge() {
    let w = wf(vec![
        file_added("t", Some("i")),
        if_step("i", "{file}", &[("yes", "w"), ("no", "end")]),
        write("w", "summary", Some("end")),
        notify("end", "{summary}.", None),
    ]);
    let problems = problems_of(&w, UnknownVariable);
    assert_eq!(found(&w), [at("end", UnknownVariable)]);
    assert_eq!(
        problems[0].message,
        "{summary} isn't produced by any step before this one."
    );
}

#[test]
fn a_variable_produced_on_every_incoming_path_is_known_at_the_merge() {
    let w = wf(vec![
        file_added("t", Some("i")),
        if_step("i", "{file}", &[("yes", "w1"), ("no", "w2")]),
        write("w1", "summary", Some("end")),
        write("w2", "summary", Some("end")),
        notify("end", "{summary}.", None),
    ]);
    assert_eq!(found(&w), []);
}

#[test]
fn variables_are_checked_in_every_text_field() {
    let w = wf(vec![
        file_added("t", Some("i")),
        step(
            "i",
            StepKind::If {
                condition: Condition {
                    left: "{a}".into(),
                    op: Op::Equal,
                    right: "{b}".into(),
                },
                branches: pairs(&[("yes", "row")]),
            },
        ),
        step(
            "row",
            StepKind::AddRow {
                file: "~/{c}.csv".into(),
                columns: vec!["{d}".into()],
                next: next(Some("cf")),
            },
        ),
        step(
            "cf",
            StepKind::CreateFile {
                name: "{e}".into(),
                contents: "{f}".into(),
                next: next(Some("g")),
            },
        ),
        step(
            "g",
            StepKind::Tag {
                tags: vec!["{g}".into()],
                next: next(Some("q")),
            },
        ),
        step(
            "q",
            StepKind::AskMe {
                question: "{h}?".into(),
                answers: labels(&[("a1", "Yes"), ("a2", "No")]),
                branches: pairs(&[]),
            },
        ),
    ]);
    let names: Vec<String> = problems_of(&w, UnknownVariable)
        .into_iter()
        .map(|p| p.message[..3].to_string())
        .collect();
    assert_eq!(
        names,
        ["{a}", "{b}", "{c}", "{d}", "{e}", "{f}", "{g}", "{h}"]
    );
}

#[test]
fn an_unknown_variable_used_twice_in_a_step_is_reported_once() {
    let w = wf(vec![
        file_added("t", Some("n")),
        notify("n", "{x} and {x}.", None),
    ]);
    assert_eq!(found(&w), [at("n", UnknownVariable)]);
}

#[test]
fn braces_that_do_not_hold_a_variable_name_are_plain_text() {
    let w = wf(vec![
        file_added("t", Some("n")),
        step(
            "n",
            StepKind::CreateFile {
                name: "notes.json".into(),
                contents: "{ \"a\": 1 } {} {not a name}".into(),
                next: None,
            },
        ),
    ]);
    assert_eq!(found(&w), []);
}

#[test]
fn an_ai_step_without_a_default_model_for_its_kind_has_no_model() {
    let w = wf(vec![
        file_added("t", Some("c")),
        classify("c", &[("c1", "A"), ("c2", "B")], &[("c1", "e")]),
        extract("e", &["vendor"], Some("w")),
        write("w", "summary", None),
    ]);

    assert_eq!(
        found_with(&w, &no_models()),
        [at("c", NoModel), at("e", NoModel), at("w", NoModel)]
    );
    assert_eq!(
        found_with(&w, &["llm".to_string()].into()),
        [at("c", NoModel)]
    );
    assert_eq!(found_with(&w, &all_models()), []);
}

#[test]
fn a_bad_variable_name_is_an_invalid_value() {
    let w = wf(vec![
        file_added("t", Some("w")),
        write("w", "due date", Some("e")),
        extract("e", &["ok", "{bad}"], None),
    ]);
    assert_eq!(found(&w), [at("e", InvalidValue), at("w", InvalidValue)]);
}

#[test]
fn a_variable_name_longer_than_32_characters_is_invalid() {
    let long = "a".repeat(33);
    let w = wf(vec![file_added("t", Some("w")), write("w", &long, None)]);
    assert_eq!(found(&w), [at("w", InvalidValue)]);
}

#[test]
fn field_names_in_one_step_must_be_unique() {
    let w = wf(vec![
        file_added("t", Some("e")),
        extract("e", &["vendor", "vendor"], None),
    ]);
    assert_eq!(found(&w), [at("e", InvalidValue)]);
}

#[test]
fn a_malformed_time_is_an_invalid_value() {
    for time in ["24:00", "9:00", "09:60", "0900", "nine", "09:00:00"] {
        let w = wf(vec![scheduled("t", weekly(time, Some(1)), None)]);
        assert_eq!(found(&w), [at("t", InvalidValue)], "{time}");
    }
    for time in ["00:00", "09:05", "23:59"] {
        let w = wf(vec![scheduled("t", weekly(time, Some(1)), None)]);
        assert_eq!(found(&w), [], "{time}");
    }
}

#[test]
fn an_empty_time_is_required() {
    let w = wf(vec![scheduled("t", weekly("", Some(1)), None)]);
    assert_eq!(found(&w), [at("t", Required)]);
}

#[test]
fn a_weekday_out_of_range_or_without_week_is_an_invalid_value() {
    let w = wf(vec![scheduled("t", weekly("09:00", Some(7)), None)]);
    assert_eq!(found(&w), [at("t", InvalidValue)]);

    let daily = Schedule {
        every: Every::Day,
        time: "09:00".into(),
        weekday: Some(1),
    };
    let w = wf(vec![scheduled("t", daily, None)]);
    assert_eq!(found(&w), [at("t", InvalidValue)]);
}

#[test]
fn an_extension_must_be_lowercase_without_the_dot() {
    for ext in [".pdf", "PDF", "", "p df", "tar.gz"] {
        let w = wf(vec![step(
            "t",
            StepKind::FileAdded {
                folder: "~/Downloads".into(),
                file_types: vec!["png".into(), ext.into()],
                subfolders: false,
                next: None,
            },
        )]);
        assert_eq!(found(&w), [at("t", InvalidValue)], "{ext:?}");
    }
}

#[test]
fn a_malformed_step_id_is_an_invalid_value() {
    let long = "s".repeat(33);
    for id in ["", "s 1", "../x", long.as_str()] {
        let w = wf(vec![file_added("t", Some(id)), notify(id, "Hi.", None)]);
        assert_eq!(found(&w), [at(id, InvalidValue)], "{id:?}");
    }
}

#[test]
fn an_empty_branch_id_is_an_invalid_value() {
    let w = wf(vec![
        file_added("t", Some("c")),
        classify("c", &[("", "A"), ("c2", "B")], &[]),
    ]);
    assert_eq!(found(&w), [at("c", InvalidValue)]);
}

// Graphs built to hang or crash the validator. Each must finish quickly.

fn quickly<T>(f: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let out = f();
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "took {:?}",
        start.elapsed()
    );
    out
}

fn chain(n: usize, back_to_start: bool) -> Workflow {
    let mut steps = vec![file_added("t", Some("s0"))];
    for i in 0..n {
        let to = if i + 1 < n {
            Some(format!("s{}", i + 1))
        } else if back_to_start {
            Some("s0".to_string())
        } else {
            None
        };
        steps.push(notify(&format!("s{i}"), "{file}.", to.as_deref()));
    }
    wf(steps)
}

#[test]
fn a_chain_of_a_thousand_steps_is_fine() {
    let w = chain(1000, false);
    assert_eq!(quickly(|| found(&w)), []);
}

#[test]
fn a_long_cycle_marks_every_step_in_it() {
    let w = chain(1000, true);
    let problems = quickly(|| found(&w));
    assert_eq!(problems.len(), 1000);
    assert!(problems.iter().all(|(_, code)| *code == Loop));
}

#[test]
fn a_ten_thousand_step_chain_does_not_overflow_the_stack() {
    let w = chain(10_000, true);
    let problems = quickly(|| validate(&w, &all_models()));
    assert_eq!(problems.len(), 10_000);
}

#[test]
fn every_step_pointing_at_every_other_finishes() {
    let n = 200;
    let categories: Vec<Branch> = (0..n)
        .map(|i| Branch {
            id: format!("b{i}"),
            label: format!("Label {i}"),
        })
        .collect();
    let exits: std::collections::BTreeMap<String, String> =
        (0..n).map(|i| (format!("b{i}"), format!("s{i}"))).collect();
    let mut steps = vec![file_added("t", Some("s0"))];
    for i in 0..n {
        steps.push(step(
            &format!("s{i}"),
            StepKind::Classify {
                categories: categories.clone(),
                instructions: String::new(),
                branches: exits.clone(),
            },
        ));
    }
    let w = wf(steps);

    let problems = quickly(|| validate(&w, &all_models()));
    assert!(problems.iter().all(|p| p.code == Loop), "{problems:?}");
    assert_eq!(problems.len(), n);
}

#[test]
fn branches_to_missing_steps_and_duplicate_ids_do_not_crash() {
    let w = wf(vec![
        file_added("t", Some("s1")),
        if_step("s1", "{x}", &[("yes", "s1"), ("no", "nowhere")]),
        if_step("s1", "{y}", &[("yes", "t"), ("no", "s1")]),
        notify("", "", Some("")),
        step("t", StepKind::Stop {}),
    ]);
    let problems = quickly(|| validate(&w, &all_models()));
    assert!(problems.iter().any(|p| p.code == DuplicateId));
    assert!(problems.iter().any(|p| p.code == MissingStep));
    assert!(problems.iter().any(|p| p.code == Loop));
}

#[test]
fn a_graph_with_no_trigger_and_a_cycle_finishes() {
    let w = wf(vec![
        notify("a", "{x}.", Some("b")),
        notify("b", "{y}.", Some("a")),
    ]);
    let problems = quickly(|| found(&w));
    assert!(problems.contains(&(None, NoTrigger)));
    assert!(problems.contains(&at("a", Loop)));
}

/// A small deterministic random source, so the test is repeatable.
struct Lcg(u64);

impl Lcg {
    fn below(&mut self, n: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) as usize) % n.max(1)
    }
}

#[test]
fn random_graphs_never_panic_or_hang() {
    let mut rng = Lcg(42);
    let start = Instant::now();
    for _ in 0..500 {
        let n = 1 + rng.below(30);
        let id = |rng: &mut Lcg| format!("s{}", rng.below(n + 3));
        let mut steps = Vec::new();
        for i in 0..n {
            let this = if rng.below(10) == 0 {
                id(&mut rng)
            } else {
                format!("s{i}")
            };
            let to = if rng.below(4) == 0 {
                None
            } else {
                Some(id(&mut rng))
            };
            let kind = match rng.below(6) {
                0 => StepKind::FileAdded {
                    folder: "~/x".into(),
                    file_types: vec![],
                    subfolders: false,
                    next: to,
                },
                1 => StepKind::Write {
                    instruction: "{v1}".into(),
                    save_as: format!("v{}", rng.below(3)),
                    next: to,
                },
                2 => StepKind::If {
                    condition: Condition {
                        left: "{v0}".into(),
                        op: Op::Equal,
                        right: "{v2}".into(),
                    },
                    branches: [
                        ("yes".to_string(), id(&mut rng)),
                        ("no".to_string(), id(&mut rng)),
                    ]
                    .into(),
                },
                3 => StepKind::Stop {},
                4 => StepKind::AskMe {
                    question: "{answer}?".into(),
                    answers: labels(&[("a", "A"), ("b", "B")]),
                    branches: [("a".to_string(), id(&mut rng))].into(),
                },
                _ => StepKind::Notify {
                    message: "{v0} {v1} {v2} {file}".into(),
                    next: to,
                },
            };
            steps.push(step(&this, kind));
        }
        validate(&wf(steps), &all_models());
    }
    assert!(start.elapsed() < Duration::from_secs(5));
}
