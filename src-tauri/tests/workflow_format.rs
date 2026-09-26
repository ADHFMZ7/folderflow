//! The workflow file format (docs/workflow-format.md): exact field names and
//! tagged shapes, so files on disk and the front end agree with the core.

use folderflow_lib::workflow::{
    Branch, Condition, Every, Op, Position, Problem, ProblemCode, SaveResult, Schedule, Step,
    StepKind, Workflow,
};
use serde_json::{json, Value};

/// Every step type exactly as the contract writes it.
fn every_step_type() -> Vec<Value> {
    let pos = json!({ "x": 10.0, "y": 20.5 });
    vec![
        json!({ "id": "t1", "type": "fileAdded", "title": "When a file is added", "position": pos,
                "folder": "~/Downloads", "fileTypes": ["pdf", "png"], "subfolders": true, "next": "s1" }),
        json!({ "id": "t2", "type": "schedule", "title": "Every Friday", "position": pos,
                "schedule": { "every": "week", "time": "17:00", "weekday": 5 }, "next": null }),
        json!({ "id": "t3", "type": "schedule", "title": "Every day", "position": pos,
                "schedule": { "every": "day", "time": "09:30" }, "next": null }),
        json!({ "id": "t4", "type": "runNow", "title": "Run now", "position": pos, "next": null }),
        json!({ "id": "s1", "type": "classify", "title": "What kind of document is this?", "position": pos,
                "categories": [{ "id": "c1", "label": "Receipt" }, { "id": "c2", "label": "Other" }],
                "instructions": "", "branches": { "c1": "s2" } }),
        json!({ "id": "s2", "type": "extract", "title": "Pull out the details", "position": pos,
                "fields": [
                    { "name": "date", "type": "date" }, { "name": "vendor", "type": "text" },
                    { "name": "amount", "type": "number" }, { "name": "paid", "type": "yesNo" }
                ],
                "ifMissing": "review", "next": "s3" }),
        json!({ "id": "s3", "type": "write", "title": "Summarise", "position": pos,
                "instruction": "Summarise it.", "saveAs": "summary", "next": null }),
        json!({ "id": "s4", "type": "agent", "title": "Look into it", "position": pos,
                "instruction": "Find the total.", "abilities": ["readFile"],
                "outputs": [{ "name": "total", "type": "number" }], "next": null }),
        json!({ "id": "s5", "type": "rename", "title": "Rename", "position": pos,
                "template": "{date} - {vendor}", "next": null }),
        json!({ "id": "s6", "type": "move", "title": "Move", "position": pos,
                "to": "~/Documents/{year}", "mode": "copy", "next": null }),
        json!({ "id": "s7", "type": "createFile", "title": "Create", "position": pos,
                "name": "{file} summary.txt", "contents": "{summary}", "next": null }),
        json!({ "id": "s8", "type": "tag", "title": "Tag", "position": pos,
                "tags": ["Receipt"], "next": null }),
        json!({ "id": "s9", "type": "addRow", "title": "Add row", "position": pos,
                "file": "~/Invoices.csv", "columns": ["{vendor}", "{amount}"], "next": null }),
        json!({ "id": "s10", "type": "notify", "title": "Notify", "position": pos,
                "message": "Done.", "next": null }),
        json!({ "id": "s11", "type": "if", "title": "Is it big?", "position": pos,
                "condition": { "left": "{amount}", "op": ">", "right": "500" },
                "branches": { "yes": "s5", "no": "s12" } }),
        json!({ "id": "s12", "type": "stop", "title": "Stop", "position": pos }),
        json!({ "id": "s13", "type": "askMe", "title": "Ask me", "position": pos,
                "question": "Log it?",
                "answers": [{ "id": "a1", "label": "Log it" }, { "id": "a2", "label": "Skip" }],
                "branches": { "a1": "s9" } }),
    ]
}

fn workflow_json(steps: Vec<Value>) -> Value {
    json!({
        "version": 1,
        "id": "0f8c2b1e-6b0a-4c1e-9d6a-2f1f6c0f7a11",
        "name": "Receipts and invoices",
        "revision": 3,
        "enabled": false,
        "steps": steps
    })
}

#[test]
fn every_step_type_round_trips_with_the_exact_json() {
    for step in every_step_type() {
        let parsed: Step = serde_json::from_value(step.clone())
            .unwrap_or_else(|e| panic!("couldn't read {step}: {e}"));
        assert_eq!(serde_json::to_value(&parsed).unwrap(), step);
    }
}

#[test]
fn a_whole_workflow_round_trips() {
    let file = workflow_json(every_step_type());

    let workflow: Workflow = serde_json::from_value(file.clone()).unwrap();

    assert_eq!(workflow.steps.len(), every_step_type().len());
    assert_eq!(serde_json::to_value(&workflow).unwrap(), file);
}

#[test]
fn an_if_step_is_tagged_by_type_with_yes_and_no_branches() {
    let step = Step {
        id: "s4".into(),
        title: "Is it over 500?".into(),
        position: Position { x: 0.0, y: 160.0 },
        kind: StepKind::If {
            condition: Condition {
                left: "{amount}".into(),
                op: Op::Greater,
                right: "500".into(),
            },
            branches: [("yes".to_string(), "s5".to_string())].into(),
        },
    };

    assert_eq!(
        serde_json::to_value(&step).unwrap(),
        json!({
            "id": "s4", "type": "if", "title": "Is it over 500?", "position": { "x": 0.0, "y": 160.0 },
            "condition": { "left": "{amount}", "op": ">", "right": "500" },
            "branches": { "yes": "s5" }
        })
    );
}

#[test]
fn every_condition_operator_is_written_as_the_contract_spells_it() {
    let ops = [
        (Op::Greater, ">"),
        (Op::Less, "<"),
        (Op::GreaterOrEqual, ">="),
        (Op::LessOrEqual, "<="),
        (Op::Equal, "="),
        (Op::NotEqual, "!="),
        (Op::Contains, "contains"),
        (Op::StartsWith, "startsWith"),
    ];
    for (op, text) in ops {
        assert_eq!(serde_json::to_value(op).unwrap(), json!(text));
        assert_eq!(serde_json::from_value::<Op>(json!(text)).unwrap(), op);
    }
}

#[test]
fn a_plain_step_always_writes_next_even_when_the_run_ends() {
    let step = Step {
        id: "s1".into(),
        title: "Notify".into(),
        position: Position { x: 0.0, y: 0.0 },
        kind: StepKind::Notify {
            message: "Hi".into(),
            next: None,
        },
    };

    assert_eq!(serde_json::to_value(&step).unwrap()["next"], Value::Null);
}

#[test]
fn a_missing_next_or_branches_reads_as_no_exit() {
    let notify: Step = serde_json::from_value(json!({
        "id": "s1", "type": "notify", "title": "", "position": { "x": 0, "y": 0 }, "message": "Hi"
    }))
    .unwrap();
    assert_eq!(notify.kind.next(), Some(&None));

    let ask: Step = serde_json::from_value(json!({
        "id": "s1", "type": "askMe", "title": "", "position": { "x": 0, "y": 0 },
        "question": "?", "answers": []
    }))
    .unwrap();
    assert!(ask.kind.branches().unwrap().is_empty());
}

#[test]
fn a_schedule_writes_weekday_only_when_there_is_one() {
    let daily = Schedule {
        every: Every::Weekday,
        time: "09:00".into(),
        weekday: None,
    };
    assert_eq!(
        serde_json::to_value(&daily).unwrap(),
        json!({ "every": "weekday", "time": "09:00" })
    );
}

#[test]
fn an_unknown_step_type_is_refused() {
    let result = serde_json::from_value::<Step>(json!({
        "id": "s1", "type": "deleteEverything", "title": "", "position": { "x": 0, "y": 0 }
    }));
    assert!(result.is_err());
}

#[test]
fn a_branch_is_an_id_and_a_label() {
    let branch = Branch {
        id: "c1".into(),
        label: "Receipt".into(),
    };
    assert_eq!(
        serde_json::to_value(&branch).unwrap(),
        json!({ "id": "c1", "label": "Receipt" })
    );
}

#[test]
fn a_problem_has_a_step_id_a_snake_case_code_and_a_message() {
    let problem = Problem {
        step_id: Some("s4".into()),
        code: ProblemCode::UnknownVariable,
        message: "{due_date} isn't produced by any step before this one.".into(),
    };
    assert_eq!(
        serde_json::to_value(&problem).unwrap(),
        json!({ "stepId": "s4", "code": "unknown_variable",
                "message": "{due_date} isn't produced by any step before this one." })
    );

    let codes = [
        (ProblemCode::NoTrigger, "no_trigger"),
        (ProblemCode::ManyTriggers, "many_triggers"),
        (ProblemCode::DuplicateId, "duplicate_id"),
        (ProblemCode::MissingStep, "missing_step"),
        (ProblemCode::UnknownBranch, "unknown_branch"),
        (ProblemCode::Loop, "loop"),
        (ProblemCode::Unreachable, "unreachable"),
        (ProblemCode::Required, "required"),
        (ProblemCode::UnknownVariable, "unknown_variable"),
        (ProblemCode::NoModel, "no_model"),
        (ProblemCode::InvalidValue, "invalid_value"),
    ];
    for (code, text) in codes {
        assert_eq!(serde_json::to_value(code).unwrap(), json!(text));
    }
}

#[test]
fn a_whole_workflow_problem_has_a_null_step_id() {
    let problem = Problem {
        step_id: None,
        code: ProblemCode::NoTrigger,
        message: "Add a trigger.".into(),
    };
    assert_eq!(
        serde_json::to_value(&problem).unwrap()["stepId"],
        Value::Null
    );
}

#[test]
fn a_save_result_is_the_workflow_and_its_problems() {
    let workflow: Workflow = serde_json::from_value(workflow_json(vec![])).unwrap();
    let result = SaveResult {
        workflow,
        problems: vec![],
    };
    let json = serde_json::to_value(&result).unwrap();
    let mut keys: Vec<_> = json.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, ["problems", "workflow"]);
}

#[test]
fn a_file_without_a_version_is_read_as_the_current_version() {
    let mut file = workflow_json(vec![]);
    file.as_object_mut().unwrap().remove("version");

    let workflow: Workflow = serde_json::from_value(file).unwrap();

    assert_eq!(workflow.version, 1);
}

/// The committed TypeScript in src/api/generated matches these types. (CI also
/// fails if `cargo test` changes anything there.)
#[test]
fn the_generated_typescript_is_up_to_date() {
    use folderflow_lib::api::types::{ErrorCode, WorkflowStatus, WorkflowSummary};
    use ts_rs::{Config, TS};

    fn check<T: TS + 'static>(cfg: &Config) {
        let path = cfg.out_dir().join(T::output_path().unwrap());
        let committed =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(
            committed,
            T::export_to_string(cfg).unwrap(),
            "{} is out of date; run cargo test and commit it",
            path.display()
        );
    }

    let cfg = Config::from_env();
    check::<Workflow>(&cfg);
    check::<Step>(&cfg);
    check::<Branch>(&cfg);
    check::<Position>(&cfg);
    check::<Schedule>(&cfg);
    check::<Every>(&cfg);
    check::<folderflow_lib::workflow::Field>(&cfg);
    check::<folderflow_lib::workflow::FieldType>(&cfg);
    check::<folderflow_lib::workflow::IfMissing>(&cfg);
    check::<folderflow_lib::workflow::MoveMode>(&cfg);
    check::<Condition>(&cfg);
    check::<Op>(&cfg);
    check::<Problem>(&cfg);
    check::<ProblemCode>(&cfg);
    check::<SaveResult>(&cfg);
    check::<WorkflowSummary>(&cfg);
    check::<WorkflowStatus>(&cfg);
    check::<ErrorCode>(&cfg);
}
