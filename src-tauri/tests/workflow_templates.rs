//! The blank workflow and the five templates, built exactly as
//! docs/workflow-format.md describes them.

use std::collections::BTreeSet;

use folderflow_lib::api::catalog;
use folderflow_lib::workflow::templates::{blank, build};
use folderflow_lib::workflow::{
    validate, Every, FieldType, MoveMode, Op, ProblemCode, Step, StepKind, Workflow,
};

const TEMPLATES: [&str; 6] = [
    "receipts",
    "screenshots",
    "summaries",
    "invoices",
    "cleanup",
    "paperwork",
];

fn all_models() -> BTreeSet<String> {
    ["llm".to_string(), "system1".to_string()].into()
}

fn template(id: &str) -> Workflow {
    build(id).unwrap_or_else(|| panic!("no template {id}"))
}

fn trigger(w: &Workflow) -> &Step {
    let triggers: Vec<_> = w.steps.iter().filter(|s| s.kind.is_trigger()).collect();
    assert_eq!(triggers.len(), 1);
    triggers[0]
}

fn by_id<'a>(w: &'a Workflow, id: &str) -> &'a Step {
    w.steps.iter().find(|s| s.id == id).unwrap()
}

/// The step a plain step leads to.
fn after<'a>(w: &'a Workflow, step: &Step) -> &'a Step {
    let next = step.kind.next().unwrap().as_ref().expect("a next step");
    by_id(w, next)
}

fn ends(step: &Step) -> bool {
    matches!(step.kind.next(), Some(None))
}

/// The step a branch leads to, found by the branch's label (or yes/no for if).
fn branch<'a>(w: &'a Workflow, step: &Step, label: &str) -> Option<&'a Step> {
    let id = match &step.kind {
        StepKind::Classify { categories, .. } => categories
            .iter()
            .find(|c| c.label == label)
            .unwrap()
            .id
            .clone(),
        StepKind::AskMe { answers, .. } => answers
            .iter()
            .find(|a| a.label == label)
            .unwrap()
            .id
            .clone(),
        StepKind::If { .. } => label.to_string(),
        _ => panic!("{} doesn't branch", step.id),
    };
    step.kind
        .branches()
        .unwrap()
        .get(&id)
        .map(|to| by_id(w, to))
}

fn watches(step: &Step, dir: &str, ext: &str) {
    let StepKind::FileAdded {
        folder,
        file_types,
        subfolders,
        ..
    } = &step.kind
    else {
        panic!("expected fileAdded, got {:?}", step.kind);
    };
    assert_eq!(folder, dir);
    assert_eq!(file_types, &[ext.to_string()]);
    assert!(!subfolders);
}

fn field_names(fields: &[folderflow_lib::workflow::Field]) -> Vec<(&str, FieldType)> {
    fields.iter().map(|f| (f.name.as_str(), f.kind)).collect()
}

#[test]
fn the_blank_workflow_is_one_file_added_step_on_downloads() {
    let w = blank();

    assert_eq!(w.name, "New workflow");
    assert_eq!((w.version, w.revision, w.enabled), (1, 1, false));
    assert!(uuid::Uuid::parse_str(&w.id).is_ok());
    assert_eq!(w.steps.len(), 1);
    let StepKind::FileAdded {
        folder,
        file_types,
        subfolders,
        next,
    } = &w.steps[0].kind
    else {
        panic!("expected fileAdded");
    };
    assert_eq!(folder, "~/Downloads");
    assert!(file_types.is_empty());
    assert!(!subfolders);
    assert_eq!(next, &None);
    assert_eq!(validate(&w, &BTreeSet::new()), []);
}

#[test]
fn every_catalog_template_can_be_built_and_nothing_else() {
    let ids: Vec<String> = catalog::templates().into_iter().map(|t| t.id).collect();
    assert_eq!(ids, TEMPLATES);
    for t in catalog::templates() {
        assert_eq!(template(&t.id).name, t.name);
    }
    assert!(build("nope").is_none());
    assert!(build("").is_none());
}

#[test]
fn every_template_validates_with_no_problems_when_every_kind_has_a_model() {
    for id in TEMPLATES {
        assert_eq!(validate(&template(id), &all_models()), [], "{id}");
    }
}

#[test]
fn without_models_templates_only_lack_models() {
    for id in TEMPLATES {
        let problems = validate(&template(id), &BTreeSet::new());
        assert!(
            problems.iter().all(|p| p.code == ProblemCode::NoModel),
            "{id}: {problems:?}"
        );
    }
    let needs_models = ["receipts", "summaries", "invoices", "paperwork"];
    for id in needs_models {
        assert!(
            !validate(&template(id), &BTreeSet::new()).is_empty(),
            "{id}"
        );
    }
}

#[test]
fn each_build_gets_a_new_workflow_id_revision_one_and_is_off() {
    for id in TEMPLATES {
        let a = template(id);
        let b = template(id);
        assert!(uuid::Uuid::parse_str(&a.id).is_ok());
        assert_ne!(a.id, b.id);
        assert_eq!((a.version, a.revision, a.enabled), (1, 1, false));
    }
}

#[test]
fn steps_are_laid_out_top_to_bottom() {
    for id in TEMPLATES {
        let w = template(id);
        for step in &w.steps {
            assert!(
                !step.title.trim().is_empty(),
                "{id}: {} has no title",
                step.id
            );
            for to in step.kind.exits() {
                let next = by_id(&w, to);
                assert!(
                    next.position.y > step.position.y,
                    "{id}: {} is not below {}",
                    next.id,
                    step.id
                );
            }
        }
        let mut places: Vec<_> = w
            .steps
            .iter()
            .map(|s| (s.position.x as i64, s.position.y as i64))
            .collect();
        places.sort();
        places.dedup();
        assert_eq!(places.len(), w.steps.len(), "{id}: two steps overlap");
    }
}

#[test]
fn receipts_sorts_receipts_into_a_folder_per_year() {
    let w = template("receipts");
    let t = trigger(&w);
    watches(t, "~/Downloads", "pdf");

    let classify = after(&w, t);
    let StepKind::Classify { categories, .. } = &classify.kind else {
        panic!("expected classify");
    };
    let labels: Vec<_> = categories.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, ["Receipt", "Other"]);
    assert!(branch(&w, classify, "Other").is_none());

    let extract = branch(&w, classify, "Receipt").unwrap();
    let StepKind::Extract { fields, .. } = &extract.kind else {
        panic!("expected extract");
    };
    assert_eq!(
        field_names(fields),
        [
            ("date", FieldType::Date),
            ("vendor", FieldType::Text),
            ("amount", FieldType::Number)
        ]
    );

    let rename = after(&w, extract);
    assert!(
        matches!(&rename.kind, StepKind::Rename { template, .. } if template == "{date} - {vendor} - {amount}")
    );
    let mv = after(&w, rename);
    assert!(
        matches!(&mv.kind, StepKind::Move { to, mode: MoveMode::Move, .. } if to == "~/Documents/Receipts/{year}")
    );
    assert!(ends(mv));
    assert_eq!(w.steps.len(), 5);
}

#[test]
fn screenshots_moves_screenshots_off_the_desktop() {
    let w = template("screenshots");
    let t = trigger(&w);
    watches(t, "~/Desktop", "png");

    let check = after(&w, t);
    let StepKind::If { condition, .. } = &check.kind else {
        panic!("expected if");
    };
    assert_eq!(
        (
            condition.left.as_str(),
            condition.op,
            condition.right.as_str()
        ),
        ("{file}", Op::StartsWith, "Screenshot")
    );
    assert!(branch(&w, check, "no").is_none());
    let mv = branch(&w, check, "yes").unwrap();
    assert!(matches!(&mv.kind, StepKind::Move { to, .. } if to == "~/Pictures/Screenshots/{year}"));
    assert!(ends(mv));
    assert_eq!(w.steps.len(), 3);
}

#[test]
fn summaries_writes_a_summary_file_next_to_each_pdf() {
    let w = template("summaries");
    let t = trigger(&w);
    watches(t, "~/Downloads", "pdf");

    let write = after(&w, t);
    assert!(
        matches!(&write.kind, StepKind::Write { instruction, save_as, .. }
        if instruction == "Summarise this document in one paragraph." && save_as == "summary")
    );
    let create = after(&w, write);
    assert!(
        matches!(&create.kind, StepKind::CreateFile { name, contents, .. }
        if name == "{file} summary.txt" && contents == "{summary}")
    );
    assert!(ends(create));
    assert_eq!(w.steps.len(), 3);
}

#[test]
fn invoices_asks_before_logging_big_ones() {
    let w = template("invoices");
    let t = trigger(&w);
    watches(t, "~/Downloads", "pdf");

    let extract = after(&w, t);
    let StepKind::Extract { fields, .. } = &extract.kind else {
        panic!("expected extract");
    };
    let names: Vec<_> = fields.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["vendor", "amount", "due"]);

    let check = after(&w, extract);
    let StepKind::If { condition, .. } = &check.kind else {
        panic!("expected if");
    };
    assert_eq!(
        (
            condition.left.as_str(),
            condition.op,
            condition.right.as_str()
        ),
        ("{amount}", Op::Greater, "500")
    );

    let ask = branch(&w, check, "yes").unwrap();
    let StepKind::AskMe {
        question, answers, ..
    } = &ask.kind
    else {
        panic!("expected askMe");
    };
    assert_eq!(question, "Log {vendor} {amount}?");
    let labels: Vec<_> = answers.iter().map(|a| a.label.as_str()).collect();
    assert_eq!(labels, ["Log it", "Skip"]);
    assert!(branch(&w, ask, "Skip").is_none());

    let row = branch(&w, ask, "Log it").unwrap();
    assert_eq!(branch(&w, check, "no").unwrap().id, row.id);
    assert!(matches!(&row.kind, StepKind::AddRow { file, columns, .. }
        if file == "~/Documents/Invoices.csv" && columns == &["{vendor}", "{amount}", "{due}"]));
    assert!(ends(row));
    assert_eq!(w.steps.len(), 5);
}

#[test]
fn cleanup_reminds_every_friday_evening() {
    let w = template("cleanup");
    let t = trigger(&w);
    let StepKind::Schedule { schedule, .. } = &t.kind else {
        panic!("expected schedule");
    };
    assert_eq!(schedule.every, Every::Week);
    assert_eq!(schedule.time, "17:00");
    assert_eq!(schedule.weekday, Some(5));

    let notify = after(&w, t);
    assert!(
        matches!(&notify.kind, StepKind::Notify { message, .. } if message == "Time to tidy Downloads.")
    );
    assert!(ends(notify));
    assert_eq!(w.steps.len(), 2);
}

#[test]
fn paperwork_shows_off_every_step_type_but_the_other_triggers() {
    let w = template("paperwork");
    assert_eq!(w.name, "Paperwork inbox");
    let mut types: Vec<&str> = w.steps.iter().map(|s| s.kind.type_name()).collect();
    types.sort_unstable();
    types.dedup();
    assert_eq!(
        types,
        [
            "addRow",
            "agent",
            "askMe",
            "classify",
            "createFile",
            "extract",
            "fileAdded",
            "if",
            "move",
            "notify",
            "rename",
            "stop",
            "tag",
            "write"
        ]
    );
}

#[test]
fn paperwork_guides_its_ai_steps_with_descriptions() {
    let w = template("paperwork");
    let classify = w
        .steps
        .iter()
        .find_map(|s| match &s.kind {
            StepKind::Classify { categories, .. } => Some(categories),
            _ => None,
        })
        .unwrap();
    let labels: Vec<&str> = classify.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, ["Receipt", "Invoice", "Contract", "Something else"]);
    assert!(classify.iter().all(|c| c.description.is_some()));

    for step in &w.steps {
        if let StepKind::Extract { fields, .. } = &step.kind {
            assert!(
                fields.iter().all(|f| f.description.is_some()),
                "{}",
                step.title
            );
        }
    }
    assert!(w.steps.iter().any(|s| matches!(
        &s.kind,
        StepKind::AddRow { headers: Some(h), columns, .. } if h.len() == columns.len()
    )));
    assert!(w.steps.iter().any(|s| matches!(
        &s.kind,
        StepKind::CreateFile { folder: Some(f), .. } if f == "{newFolder}"
    )));
}
