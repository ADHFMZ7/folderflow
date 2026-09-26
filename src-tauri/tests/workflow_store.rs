//! workflows/<id>.json: ids can't escape the folder, damaged and newer files are
//! never changed, saves never lose someone else's changes, deletes keep a copy.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};
use std::thread;

use folderflow_lib::storage::data_dir::DataDir;
use folderflow_lib::storage::workflows::{Entry, WorkflowError, WorkflowStore};
use folderflow_lib::workflow::templates::{blank, build};
use folderflow_lib::workflow::{ProblemCode, StepKind, Workflow};
use serde_json::json;

const ID: &str = "0f8c2b1e-6b0a-4c1e-9d6a-2f1f6c0f7a11";

fn all_models() -> BTreeSet<String> {
    ["llm".to_string(), "system1".to_string()].into()
}

fn setup() -> (tempfile::TempDir, DataDir, WorkflowStore) {
    let (tmp, dir) = common::data_dir();
    let store = WorkflowStore::new(&dir);
    (tmp, dir, store)
}

fn file(dir: &DataDir, id: &str) -> PathBuf {
    dir.workflows_path().join(format!("{id}.json"))
}

fn trash(dir: &DataDir) -> PathBuf {
    dir.workflows_path().join(".trash")
}

/// Puts raw bytes where the workflow `id` lives.
fn plant(dir: &DataDir, id: &str, bytes: &[u8]) -> PathBuf {
    fs::create_dir_all(dir.workflows_path()).unwrap();
    let path = file(dir, id);
    fs::write(&path, bytes).unwrap();
    path
}

fn with_id(id: &str) -> Workflow {
    Workflow {
        id: id.into(),
        ..blank()
    }
}

/// Every path under `root` (folders too) and each file's bytes.
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    let mut out = BTreeMap::new();
    let mut todo = vec![root.to_path_buf()];
    while let Some(dir) = todo.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let meta = fs::symlink_metadata(&path).unwrap();
            if meta.is_dir() {
                todo.push(path.clone());
                out.insert(path, None);
            } else {
                out.insert(path.clone(), fs::read(&path).ok());
            }
        }
    }
    out
}

#[test]
fn a_created_workflow_is_a_private_file_named_by_its_id() {
    let (_tmp, dir, store) = setup();

    let created = store.create(blank()).unwrap();

    let path = file(&dir, &created.id);
    assert_eq!(common::mode(&path), 0o600);
    assert_eq!(common::mode(&dir.workflows_path()), 0o700);
    let on_disk: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(on_disk["version"], 1);
    assert_eq!(on_disk["revision"], 1);
    assert_eq!(on_disk["id"], created.id.as_str());
    assert_eq!(store.get(&created.id).unwrap(), created);
}

#[test]
fn creating_over_an_existing_id_is_refused() {
    let (_tmp, dir, store) = setup();
    store.create(with_id(ID)).unwrap();
    let before = fs::read(file(&dir, ID)).unwrap();

    let result = store.create(Workflow {
        name: "Other".into(),
        ..with_id(ID)
    });

    assert!(
        matches!(result, Err(WorkflowError::Conflict { .. })),
        "{result:?}"
    );
    assert_eq!(fs::read(file(&dir, ID)).unwrap(), before);
}

#[test]
fn listing_with_no_workflows_folder_is_empty() {
    let (_tmp, dir, store) = setup();
    assert_eq!(store.list().unwrap(), []);
    assert!(!dir.workflows_path().exists());
}

#[test]
fn listing_skips_files_that_are_not_workflows() {
    let (_tmp, dir, store) = setup();
    let created = store.create(blank()).unwrap();
    store.create(with_id(ID)).unwrap();
    store.delete(ID).unwrap();
    fs::write(dir.workflows_path().join("notes.txt"), "hi").unwrap();
    fs::write(dir.workflows_path().join(".x.json.1234.tmp"), "{").unwrap();
    fs::write(dir.workflows_path().join("settings.json"), "{}").unwrap();

    let listed = store.list().unwrap();

    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, created.id);
    assert_eq!(listed[0].entry, Entry::Ok(created));
}

#[test]
fn ids_that_are_not_uuids_are_refused_without_touching_disk() {
    let (tmp, dir, store) = setup();
    fs::write(dir.settings_path(), r#"{"version":1}"#).unwrap();
    let bad = [
        "../../settings",
        "../settings",
        "settings",
        "/etc/passwd",
        "",
        ".",
        "..",
        ".trash",
        "0F8C2B1E-6B0A-4C1E-9D6A-2F1F6C0F7A11",
        "0f8c2b1e6b0a4c1e9d6a2f1f6c0f7a11",
        "{0f8c2b1e-6b0a-4c1e-9d6a-2f1f6c0f7a11}",
        "urn:uuid:0f8c2b1e-6b0a-4c1e-9d6a-2f1f6c0f7a11",
        "0f8c2b1e-6b0a-4c1e-9d6a-2f1f6c0f7a11.json",
        "0f8c2b1e-6b0a-4c1e-9d6a-2f1f6c0f7a11/../../settings",
        "../workflows/0f8c2b1e-6b0a-4c1e-9d6a-2f1f6c0f7a11",
    ];
    let before = snapshot(tmp.path());

    for id in bad {
        assert!(
            matches!(store.get(id), Err(WorkflowError::InvalidId)),
            "get {id:?}"
        );
        assert!(
            matches!(store.delete(id), Err(WorkflowError::InvalidId)),
            "delete {id:?}"
        );
        let saved = store.save(with_id(id), &all_models());
        assert!(
            matches!(saved, Err(WorkflowError::InvalidId)),
            "save {id:?}"
        );
        let created = store.create(with_id(id));
        assert!(
            matches!(created, Err(WorkflowError::InvalidId)),
            "create {id:?}"
        );
    }

    assert_eq!(snapshot(tmp.path()), before);
    assert!(!dir.workflows_path().exists());
}

#[test]
fn a_symlinked_workflow_file_is_refused_and_its_target_left_alone() {
    let (tmp, dir, store) = setup();
    let outside = tmp.path().join("outside.json");
    let good = serde_json::to_vec(&with_id(ID)).unwrap();
    fs::write(&outside, &good).unwrap();
    fs::create_dir_all(dir.workflows_path()).unwrap();
    symlink(&outside, file(&dir, ID)).unwrap();
    let before = snapshot(tmp.path());

    assert!(matches!(store.get(ID), Err(WorkflowError::NotAFile)));
    let saved = store.save(with_id(ID), &all_models());
    assert!(matches!(saved, Err(WorkflowError::NotAFile)), "{saved:?}");
    assert!(matches!(store.delete(ID), Err(WorkflowError::NotAFile)));
    let listed = store.list().unwrap();
    assert_eq!(listed[0].entry, Entry::Damaged);

    assert_eq!(snapshot(tmp.path()), before);
    assert!(fs::symlink_metadata(file(&dir, ID))
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn a_folder_named_like_a_workflow_is_refused() {
    let (_tmp, dir, store) = setup();
    fs::create_dir_all(file(&dir, ID)).unwrap();

    assert!(matches!(store.get(ID), Err(WorkflowError::NotAFile)));
    assert!(matches!(store.delete(ID), Err(WorkflowError::NotAFile)));
    assert_eq!(store.list().unwrap()[0].entry, Entry::Damaged);
    assert!(file(&dir, ID).is_dir());
}

#[test]
fn a_damaged_file_is_listed_as_damaged_and_left_byte_for_byte() {
    let (_tmp, dir, store) = setup();
    let ok = store.create(blank()).unwrap();
    let garbage = b"{ \"version\": 1, \"id\": oops";
    let path = plant(&dir, ID, garbage);

    let listed = store.list().unwrap();

    assert_eq!(listed.len(), 2);
    let damaged = listed.iter().find(|l| l.id == ID).unwrap();
    assert_eq!(damaged.entry, Entry::Damaged);
    assert_eq!(damaged.file_name, format!("{ID}.json"));
    assert!(listed.iter().any(|l| l.entry == Entry::Ok(ok.clone())));

    assert!(matches!(store.get(ID), Err(WorkflowError::Damaged)));
    let saved = store.save(with_id(ID), &all_models());
    assert!(matches!(saved, Err(WorkflowError::Damaged)), "{saved:?}");
    assert_eq!(fs::read(&path).unwrap(), garbage);
}

#[test]
fn well_formed_json_in_the_wrong_shape_is_damaged() {
    let (_tmp, dir, store) = setup();
    let wrong_shape = json!({ "version": 1, "id": ID, "name": "x", "revision": 1,
                              "enabled": false, "steps": [{ "type": "explode" }] });
    let path = plant(&dir, ID, wrong_shape.to_string().as_bytes());

    assert_eq!(store.list().unwrap()[0].entry, Entry::Damaged);
    assert!(matches!(store.get(ID), Err(WorkflowError::Damaged)));
    assert_eq!(fs::read(&path).unwrap(), wrong_shape.to_string().as_bytes());
}

#[test]
fn a_file_whose_id_is_not_its_name_is_damaged() {
    let (_tmp, dir, store) = setup();
    let other = with_id("11111111-2222-4333-8444-555555555555");
    plant(&dir, ID, &serde_json::to_vec(&other).unwrap());

    assert_eq!(store.list().unwrap()[0].entry, Entry::Damaged);
    assert!(matches!(store.get(ID), Err(WorkflowError::Damaged)));
}

#[test]
fn a_damaged_file_can_be_deleted_and_is_kept_in_the_trash() {
    let (_tmp, dir, store) = setup();
    plant(&dir, ID, b"garbage");

    let kept = store.delete(ID).unwrap();

    assert_eq!(fs::read(kept).unwrap(), b"garbage");
    assert!(!file(&dir, ID).exists());
}

#[test]
fn a_too_new_file_is_refused_everywhere_and_never_changed_or_moved() {
    let (_tmp, dir, store) = setup();
    let newer = json!({ "version": 2, "id": ID, "name": "From the future", "revision": 7,
                        "enabled": true, "steps": [], "somethingNew": true });
    let bytes = serde_json::to_vec_pretty(&newer).unwrap();
    let path = plant(&dir, ID, &bytes);

    assert_eq!(store.list().unwrap()[0].entry, Entry::TooNew(2));
    assert!(matches!(
        store.get(ID),
        Err(WorkflowError::TooNew { found: 2 })
    ));
    let saved = store.save(
        Workflow {
            revision: 7,
            ..with_id(ID)
        },
        &all_models(),
    );
    assert!(
        matches!(saved, Err(WorkflowError::TooNew { found: 2 })),
        "{saved:?}"
    );
    assert!(matches!(
        store.delete(ID),
        Err(WorkflowError::TooNew { found: 2 })
    ));
    let created = store.create(with_id(ID));
    assert!(created.is_err());

    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert!(!trash(&dir).exists() || fs::read_dir(trash(&dir)).unwrap().count() == 0);
}

#[test]
fn saving_bumps_the_revision_and_returns_problems() {
    let (_tmp, dir, store) = setup();
    let created = store.create(blank()).unwrap();

    let mut changed = created.clone();
    changed.name = "Renamed".into();
    changed.steps.push(folderflow_lib::workflow::Step {
        id: "n".into(),
        title: "Tell me".into(),
        position: folderflow_lib::workflow::Position { x: 0.0, y: 160.0 },
        kind: StepKind::Notify {
            message: "{nope}".into(),
            next: None,
        },
    });
    let result = store.save(changed, &all_models()).unwrap();

    assert_eq!(result.workflow.revision, 2);
    assert_eq!(result.workflow.name, "Renamed");
    let codes: Vec<_> = result.problems.iter().map(|p| p.code).collect();
    assert_eq!(codes, [ProblemCode::Unreachable]);
    assert_eq!(store.get(&created.id).unwrap(), result.workflow);
    assert_eq!(common::mode(&file(&dir, &created.id)), 0o600);
}

#[test]
fn saving_always_writes_the_current_version() {
    let (_tmp, dir, store) = setup();
    let created = store.create(blank()).unwrap();

    store
        .save(
            Workflow {
                version: 0,
                ..created.clone()
            },
            &all_models(),
        )
        .unwrap();

    let on_disk: serde_json::Value =
        serde_json::from_slice(&fs::read(file(&dir, &created.id)).unwrap()).unwrap();
    assert_eq!(on_disk["version"], 1);
}

#[test]
fn a_stale_revision_is_a_conflict_and_the_file_is_unchanged() {
    let (_tmp, dir, store) = setup();
    let created = store.create(blank()).unwrap();
    store.save(created.clone(), &all_models()).unwrap();
    let before = fs::read(file(&dir, &created.id)).unwrap();

    let stale = Workflow {
        name: "Mine".into(),
        ..created.clone()
    };
    let result = store.save(stale, &all_models());

    assert!(
        matches!(result, Err(WorkflowError::Conflict { on_disk: 2 })),
        "{result:?}"
    );
    assert_eq!(fs::read(file(&dir, &created.id)).unwrap(), before);

    let ahead = Workflow {
        revision: 9,
        ..created.clone()
    };
    assert!(matches!(
        store.save(ahead, &all_models()),
        Err(WorkflowError::Conflict { on_disk: 2 })
    ));
}

#[test]
fn saving_a_workflow_that_is_not_on_disk_is_not_found_and_creates_nothing() {
    let (_tmp, dir, store) = setup();

    let result = store.save(with_id(ID), &all_models());

    assert!(matches!(result, Err(WorkflowError::NotFound)), "{result:?}");
    assert!(!file(&dir, ID).exists());
}

#[test]
fn turning_on_a_workflow_with_problems_writes_nothing() {
    let (_tmp, dir, store) = setup();
    let created = store.create(build("summaries").unwrap()).unwrap();
    let before = fs::read(file(&dir, &created.id)).unwrap();

    let result = store.save(
        Workflow {
            enabled: true,
            ..created.clone()
        },
        &BTreeSet::new(),
    );

    let Err(WorkflowError::NotClean(problems)) = result else {
        panic!("expected NotClean, got {result:?}");
    };
    assert_eq!(problems[0].code, ProblemCode::NoModel);
    assert_eq!(fs::read(file(&dir, &created.id)).unwrap(), before);
}

#[test]
fn a_clean_workflow_can_be_turned_on() {
    let (_tmp, _dir, store) = setup();
    let created = store.create(build("summaries").unwrap()).unwrap();

    let result = store
        .save(
            Workflow {
                enabled: true,
                ..created.clone()
            },
            &all_models(),
        )
        .unwrap();

    assert!(result.workflow.enabled);
    assert_eq!(result.problems, []);
    assert!(store.get(&created.id).unwrap().enabled);
}

#[test]
fn a_workflow_with_problems_can_still_be_saved_while_off() {
    let (_tmp, _dir, store) = setup();
    let created = store.create(build("summaries").unwrap()).unwrap();

    let result = store.save(created.clone(), &BTreeSet::new()).unwrap();

    assert_eq!(result.workflow.revision, 2);
    assert_eq!(result.problems.len(), 1);
}

#[test]
fn deleting_moves_the_file_to_the_trash() {
    let (_tmp, dir, store) = setup();
    let created = store.create(blank()).unwrap();
    let bytes = fs::read(file(&dir, &created.id)).unwrap();

    let kept = store.delete(&created.id).unwrap();

    assert!(!file(&dir, &created.id).exists());
    assert_eq!(kept.parent().unwrap(), trash(&dir));
    let name = kept.file_name().unwrap().to_string_lossy().to_string();
    let stamp = name
        .strip_prefix(&format!("{}-", created.id))
        .and_then(|rest| rest.strip_suffix(".json"))
        .unwrap_or_else(|| panic!("unexpected trash name {name}"));
    assert!(stamp.parse::<u64>().unwrap() > 1_700_000_000);
    assert_eq!(fs::read(&kept).unwrap(), bytes);
    assert_eq!(common::mode(&trash(&dir)), 0o700);
    assert!(matches!(
        store.get(&created.id),
        Err(WorkflowError::NotFound)
    ));
    assert!(store.list().unwrap().is_empty());
}

#[test]
fn deleting_an_unknown_workflow_is_not_found() {
    let (_tmp, _dir, store) = setup();
    assert!(matches!(store.delete(ID), Err(WorkflowError::NotFound)));
}

#[test]
fn a_second_delete_of_a_recreated_id_never_overwrites_the_first_copy() {
    let (_tmp, dir, store) = setup();
    store.create(with_id(ID)).unwrap();
    let first = store.delete(ID).unwrap();
    let first_bytes = fs::read(&first).unwrap();
    store
        .create(Workflow {
            name: "Second".into(),
            ..with_id(ID)
        })
        .unwrap();

    let second = store.delete(ID).unwrap();

    assert_ne!(first, second);
    assert_eq!(fs::read(&first).unwrap(), first_bytes);
    assert!(String::from_utf8(fs::read(&second).unwrap())
        .unwrap()
        .contains("Second"));
    assert_eq!(fs::read_dir(trash(&dir)).unwrap().count(), 2);
}

#[test]
fn concurrent_saves_of_different_workflows_all_land() {
    let (_tmp, dir, store) = setup();
    let store = Arc::new(store);
    let ids: Vec<String> = (0..8).map(|_| store.create(blank()).unwrap().id).collect();
    let barrier = Arc::new(Barrier::new(ids.len()));

    let handles: Vec<_> = ids
        .iter()
        .map(|id| {
            let id = id.clone();
            let store = store.clone();
            let barrier = barrier.clone();
            thread::spawn(move || {
                barrier.wait();
                for i in 0..8 {
                    let mut w = store.get(&id).unwrap();
                    w.name = format!("{id} save {i}");
                    store.save(w, &all_models()).unwrap();
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }

    for id in &ids {
        let w = store.get(id).unwrap();
        assert_eq!(w.revision, 9);
        assert_eq!(w.name, format!("{id} save 7"));
    }
    let leftovers: Vec<_> = fs::read_dir(dir.workflows_path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".tmp"))
        .collect();
    assert_eq!(leftovers, Vec::<String>::new());
}

#[test]
fn concurrent_saves_of_one_workflow_never_lose_a_change() {
    let (_tmp, dir, store) = setup();
    let store = Arc::new(store);
    let id = store.create(blank()).unwrap().id;
    let writers = 6;
    let saves_each = 6;
    let barrier = Arc::new(Barrier::new(writers + 1));

    let handles: Vec<_> = (0..writers)
        .map(|n| {
            let store = store.clone();
            let barrier = barrier.clone();
            let id = id.clone();
            thread::spawn(move || {
                barrier.wait();
                let mut done = 0;
                let mut conflicts = 0;
                while done < saves_each {
                    let mut w = store.get(&id).unwrap();
                    w.name = format!("{} writer{n}", w.name);
                    match store.save(w, &all_models()) {
                        Ok(_) => done += 1,
                        Err(WorkflowError::Conflict { .. }) => conflicts += 1,
                        Err(e) => panic!("unexpected {e:?}"),
                    }
                }
                conflicts
            })
        })
        .collect();

    // Meanwhile, every read sees a whole file.
    let reader = {
        let path = file(&dir, &id);
        let barrier = barrier.clone();
        thread::spawn(move || {
            barrier.wait();
            for _ in 0..300 {
                let bytes = fs::read(&path).unwrap();
                serde_json::from_slice::<Workflow>(&bytes).unwrap();
            }
        })
    };
    for h in handles {
        h.join().unwrap();
    }
    reader.join().unwrap();

    let w = store.get(&id).unwrap();
    let total = (writers * saves_each) as u32;
    assert_eq!(w.revision, 1 + total);
    for n in 0..writers {
        let count = w.name.matches(&format!(" writer{n}")).count();
        assert_eq!(count, saves_each, "writer{n}'s changes were lost");
    }
}
