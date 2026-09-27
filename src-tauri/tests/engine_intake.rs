//! File added: a workflow runs once for each file that lands in its folder,
//! never for files that were there before it was on, and never for files
//! Vela itself put there. Folder events are told to the engine by the
//! test, as FSEvents would. See docs/engine.md, "File added", and the
//! running-away risk in TESTING.md.

mod common;

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::Command;

use serde_json::{json, Value};
use vela_lib::engine::runs::{Run, RunStatus, TriggerKind};
use vela_lib::storage::data_dir::DataDir;
use vela_lib::storage::workflows::WorkflowStore;
use vela_lib::workflow::Workflow;

use common::engine::{engine, EngineHarness};
use common::files::names;

fn steps(steps: Value) -> Value {
    let mut steps = steps;
    for step in steps.as_array_mut().unwrap() {
        step["position"] = json!({ "x": 0, "y": 0 });
        step["title"] = step["id"].clone();
    }
    steps
}

/// A workflow on `folder` that tags each file it takes "Seen".
fn tagger(h: &EngineHarness, folder: &str, types: &[&str], subfolders: bool) -> Workflow {
    let wf = h.workflow(
        "Tag new files",
        steps(json!([
            { "id": "t", "type": "fileAdded", "folder": folder, "fileTypes": types,
              "subfolders": subfolders, "next": "g" },
            { "id": "g", "type": "tag", "tags": ["Seen"], "next": null },
        ])),
    );
    fs::create_dir_all(h.home.join(folder.trim_start_matches("~/"))).unwrap();
    h.switch(&wf, true)
}

/// The names of the files the runs were for, oldest run first.
fn taken(runs: &[Run]) -> Vec<String> {
    runs.iter()
        .map(|r| {
            let file = r.trigger.file.as_ref().expect("a file run");
            file.path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

fn put(h: &EngineHarness, rel: &str) {
    let path = h.home.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, rel).unwrap();
}

#[tokio::test]
async fn a_new_file_runs_the_workflow_once() {
    let mut h = engine();
    let wf = tagger(&h, "~/Downloads", &["pdf"], false);

    put(&h, "Downloads/Invoice.pdf");
    h.changed("Downloads");
    let runs = h.settle().await;

    assert_eq!(taken(&runs), ["Invoice.pdf"]);
    let run = &runs[0];
    assert_eq!(run.workflow_id, wf.id);
    assert_eq!(run.trigger.kind, TriggerKind::FileAdded);
    assert_eq!(run.status, RunStatus::Done);
    assert_eq!(run.values["dateAdded"].value, "2026-09-14");

    // More events for the same folder, and the person renaming the file: the
    // same file, so nothing more.
    h.changed("Downloads");
    h.changed("Downloads/Invoice.pdf");
    fs::rename(
        h.home.join("Downloads/Invoice.pdf"),
        h.home.join("Downloads/Invoice March.pdf"),
    )
    .unwrap();
    h.changed("Downloads");
    assert_eq!(h.settle().await.len(), 1);

    // A copy is a new file.
    fs::copy(
        h.home.join("Downloads/Invoice March.pdf"),
        h.home.join("Downloads/Invoice copy.pdf"),
    )
    .unwrap();
    h.changed("Downloads");
    assert_eq!(
        taken(&h.settle().await),
        ["Invoice.pdf", "Invoice copy.pdf"]
    );
}

#[tokio::test]
async fn files_already_there_when_it_is_turned_on_are_left_alone() {
    let mut h = engine();
    put(&h, "Downloads/Old.pdf");
    let wf = tagger(&h, "~/Downloads", &[], false);
    h.changed("Downloads");
    assert!(h.settle().await.is_empty(), "decision 2");

    // Turned off, a file arrives, and it's turned on again: that file is
    // left alone too, but the next one runs.
    h.switch(&wf, false);
    assert!(h.watched.0.lock().unwrap().is_empty());
    put(&h, "Downloads/While off.pdf");
    h.changed("Downloads");
    assert!(h.settle().await.is_empty());
    h.switch(&wf, true);
    h.changed("Downloads");
    assert!(h.settle().await.is_empty());

    put(&h, "Downloads/New.pdf");
    h.changed("Downloads");
    assert_eq!(taken(&h.settle().await), ["New.pdf"]);
}

#[tokio::test]
async fn only_finished_plain_files_of_the_chosen_types_are_taken() {
    let mut h = engine();
    tagger(&h, "~/Downloads", &["pdf", "png"], false);

    for rel in [
        "Downloads/Wanted.pdf",
        "Downloads/Shot.PNG",
        "Downloads/Notes.txt",
        "Downloads/.hidden.pdf",
        "Downloads/.DS_Store",
        "Downloads/Big.pdf.crdownload",
        "Downloads/Big.pdf.part",
        "Downloads/Big.pdf.download/Big.pdf",
        "Downloads/~$Budget.pdf",
        "Downloads/Folder/Inside.pdf",
        "Elsewhere/Linked.pdf",
        "Downloads/Hidden in Finder.pdf",
    ] {
        put(&h, rel);
    }
    std::os::unix::fs::symlink(
        h.home.join("Elsewhere/Linked.pdf"),
        h.home.join("Downloads/Link.pdf"),
    )
    .unwrap();
    let hidden = h.home.join("Downloads/Hidden in Finder.pdf");
    assert!(Command::new("chflags")
        .arg("hidden")
        .arg(&hidden)
        .status()
        .unwrap()
        .success());
    h.changed("Downloads");

    assert_eq!(taken(&h.settle().await), ["Wanted.pdf", "Shot.PNG"]);
}

/// The file each run acted on, by inode.
fn inodes(runs: &[Run]) -> Vec<u64> {
    runs.iter()
        .map(|r| r.trigger.file.as_ref().expect("a file run").inode)
        .collect()
}

fn inode(path: &Path) -> u64 {
    fs::metadata(path).unwrap().ino()
}

// Each browser's steps when downloading Invoice.pdf into a watched folder
// (issue #27). Every one runs the workflow once, on the finished file.

#[tokio::test]
async fn a_firefox_download_runs_once_on_the_finished_file_not_its_placeholder() {
    let mut h = engine();
    tagger(&h, "~/Downloads", &["pdf"], false);
    let done = h.home.join("Downloads/Invoice.pdf");
    let part = h.home.join("Downloads/Invoice.pdf.part");

    // An empty placeholder under the final name, and the download beside it.
    fs::write(&done, b"").unwrap();
    fs::write(&part, b"%PDF-1.7 half").unwrap();
    h.changed("Downloads");
    assert!(h.settle().await.is_empty());

    fs::write(&part, b"%PDF-1.7 half and the rest").unwrap();
    fs::rename(&part, &done).unwrap();
    h.changed("Downloads");
    let runs = h.settle().await;
    assert_eq!(taken(&runs), ["Invoice.pdf"]);
    assert_eq!(inodes(&runs), [inode(&done)]);
}

#[tokio::test]
async fn a_file_waits_while_its_download_is_still_going_beside_it() {
    let mut h = engine();
    tagger(&h, "~/Downloads", &["pdf"], false);
    // Not empty this time: only the .part beside it says it isn't done.
    put(&h, "Downloads/Invoice.pdf");
    put(&h, "Downloads/Invoice.pdf.part");
    h.changed("Downloads");
    assert!(h.settle().await.is_empty());

    fs::remove_file(h.home.join("Downloads/Invoice.pdf.part")).unwrap();
    h.changed("Downloads");
    assert_eq!(taken(&h.settle().await), ["Invoice.pdf"]);
}

#[tokio::test]
async fn a_chrome_download_runs_once_when_renamed_from_crdownload() {
    let mut h = engine();
    tagger(&h, "~/Downloads", &["pdf"], false);
    let partial = h.home.join("Downloads/Invoice.pdf.crdownload");
    fs::write(&partial, b"%PDF-1.7 half").unwrap();
    h.changed("Downloads");
    assert!(h.settle().await.is_empty());

    fs::write(&partial, b"%PDF-1.7 half and the rest").unwrap();
    fs::rename(&partial, h.home.join("Downloads/Invoice.pdf")).unwrap();
    h.changed("Downloads");
    assert_eq!(taken(&h.settle().await), ["Invoice.pdf"]);
}

#[tokio::test]
async fn a_safari_download_runs_once_when_it_leaves_its_download_package() {
    let mut h = engine();
    tagger(&h, "~/Downloads", &["pdf"], false);
    put(&h, "Downloads/Invoice.pdf.download/Invoice.pdf");
    h.changed("Downloads");
    assert!(h.settle().await.is_empty());

    fs::rename(
        h.home.join("Downloads/Invoice.pdf.download/Invoice.pdf"),
        h.home.join("Downloads/Invoice.pdf"),
    )
    .unwrap();
    fs::remove_dir(h.home.join("Downloads/Invoice.pdf.download")).unwrap();
    h.changed("Downloads");
    assert_eq!(taken(&h.settle().await), ["Invoice.pdf"]);
}

#[tokio::test]
async fn a_file_created_empty_runs_once_it_is_written() {
    let mut h = engine();
    tagger(&h, "~/Downloads", &["txt"], false);
    let notes = h.home.join("Downloads/Notes.txt");
    fs::write(&notes, b"").unwrap();
    h.changed("Downloads");
    assert!(h.settle().await.is_empty());

    // The same file, written in place.
    fs::write(&notes, b"Call the bank").unwrap();
    h.changed("Downloads");
    let runs = h.settle().await;
    assert_eq!(taken(&runs), ["Notes.txt"]);
    assert_eq!(inodes(&runs), [inode(&notes)]);
    h.changed("Downloads");
    assert_eq!(h.settle().await.len(), 1);
}

#[tokio::test]
async fn with_subfolders_on_it_looks_inside_folders_but_not_packages() {
    let mut h = engine();
    tagger(&h, "~/Documents/Scans", &["pdf"], true);

    put(&h, "Documents/Scans/2026/March/Scan.pdf");
    put(&h, "Documents/Scans/Report.pages/Preview.pdf");
    put(&h, "Documents/Scans/.cache/Temp.pdf");
    // FSEvents may name only the deepest folder.
    h.changed("Documents/Scans/2026/March");

    assert_eq!(taken(&h.settle().await), ["Scan.pdf"]);
}

#[tokio::test]
async fn files_that_arrive_while_the_app_is_quit_run_at_start_oldest_first() {
    let mut h = engine();
    tagger(&h, "~/Downloads", &[], false);
    put(&h, "Downloads/Before.pdf");
    h.changed("Downloads");
    assert_eq!(h.settle().await.len(), 1);

    let h = h.restart();
    // Quit: files arrive, and the one already run is renamed.
    put(&h, "Downloads/First.pdf");
    std::thread::sleep(std::time::Duration::from_millis(20));
    put(&h, "Downloads/Second.pdf");
    fs::rename(
        h.home.join("Downloads/Before.pdf"),
        h.home.join("Downloads/Before renamed.pdf"),
    )
    .unwrap();
    let mut h = h.restart();

    assert_eq!(
        taken(&h.settle().await),
        ["Before.pdf", "First.pdf", "Second.pdf"]
    );
}

#[tokio::test]
async fn files_vela_moves_writes_or_copies_into_a_watched_folder_start_nothing() {
    let mut h = engine();
    // Inbox is watched by a workflow that tags whatever arrives.
    let inbox = tagger(&h, "~/Documents/Inbox", &[], false);
    // Downloads is watched by one that files into the Inbox.
    let filer = h.workflow(
        "File downloads",
        steps(json!([
            { "id": "t", "type": "fileAdded", "folder": "~/Downloads", "fileTypes": [],
              "subfolders": false, "next": "c" },
            { "id": "c", "type": "move", "to": "~/Documents/Inbox", "mode": "copy", "next": "m" },
            { "id": "m", "type": "move", "to": "~/Documents/Inbox/Moved", "mode": "move", "next": "n" },
            { "id": "n", "type": "createFile", "name": "{file} note.txt", "contents": "from {folder}",
              "folder": "~/Documents/Inbox", "next": "a" },
            { "id": "a", "type": "addRow", "file": "~/Documents/Inbox/Log.csv",
              "columns": ["{file}"], "next": null },
        ])),
    );
    h.switch(&filer, true);

    put(&h, "Downloads/Receipt.pdf");
    h.changed("Downloads");
    h.settle().await;
    h.changed("Documents/Inbox");
    h.changed("Documents/Inbox/Moved");
    let runs = h.settle().await;

    assert_eq!(runs.len(), 1, "only the filing run: {:?}", taken(&runs));
    assert_eq!(runs[0].workflow_id, filer.id);
    assert_eq!(runs[0].status, RunStatus::Done);
    assert_eq!(
        names(&h.home.join("Documents/Inbox")),
        ["Log.csv", "Moved", "Receipt note.txt", "Receipt.pdf"]
    );

    // Still nothing after a restart: they're in the Inbox workflow's record.
    let mut h = h.restart();
    h.changed("Documents/Inbox");
    assert_eq!(h.settle().await.len(), 1);
    let _ = inbox;
}

#[tokio::test]
async fn a_workflow_that_writes_into_its_own_folder_runs_once() {
    let mut h = engine();
    let wf = h.workflow(
        "Notes",
        steps(json!([
            { "id": "t", "type": "fileAdded", "folder": "~/Downloads", "fileTypes": [],
              "subfolders": false, "next": "n" },
            { "id": "n", "type": "createFile", "name": "{file} note.txt", "contents": "x",
              "folder": "~/Downloads", "next": null },
        ])),
    );
    fs::create_dir_all(h.home.join("Downloads")).unwrap();
    h.switch(&wf, true);

    put(&h, "Downloads/a.pdf");
    for _ in 0..3 {
        h.changed("Downloads");
        h.settle().await;
    }

    assert_eq!(h.runs().len(), 1);
    assert_eq!(names(&h.home.join("Downloads")), ["a note.txt", "a.pdf"]);
}

#[tokio::test]
async fn undo_putting_a_file_back_into_a_watched_folder_starts_nothing() {
    let mut h = engine();
    let archive = h.workflow(
        "Archive",
        steps(json!([
            { "id": "t", "type": "runNow", "next": "m" },
            { "id": "m", "type": "move", "to": "~/Archive", "mode": "move", "next": null },
        ])),
    );
    let file = h.file("Desktop/Plan.pdf");
    let id = h.engine.run_now(&archive.id, vec![file]).unwrap()[0]
        .id
        .clone();
    h.until(&id, RunStatus::Done).await;
    // The Desktop becomes watched while Plan.pdf is away.
    tagger(&h, "~/Desktop", &[], false);

    h.engine.undo_run(&id).unwrap();
    h.changed("Desktop");

    assert_eq!(names(&h.home.join("Desktop")), ["Plan.pdf"]);
    assert_eq!(h.settle().await.len(), 1, "only the archive run");
}

#[tokio::test]
async fn a_crash_between_writing_a_file_and_recording_it_still_starts_nothing() {
    let mut h = engine();
    let inbox = tagger(&h, "~/Inbox", &[], false);
    let filer = h.workflow(
        "File downloads",
        steps(json!([
            { "id": "t", "type": "fileAdded", "folder": "~/Downloads", "fileTypes": [],
              "subfolders": false, "next": "m" },
            { "id": "m", "type": "move", "to": "~/Inbox", "mode": "move", "next": null },
        ])),
    );
    fs::create_dir_all(h.home.join("Downloads")).unwrap();
    h.switch(&filer, true);
    put(&h, "Downloads/a.pdf");
    h.changed("Downloads");
    let run = h.settle().await.remove(0);

    // As if the app had quit right after the move: not in the journal as
    // done, not in the Inbox workflow's record, and the run still running.
    let inode = fs::metadata(h.home.join("Inbox/a.pdf")).unwrap().ino();
    let journal = h
        .root
        .join("engine/journal")
        .join(format!("{}.jsonl", run.id));
    drop_lines(&journal, "\"state\":\"done\"");
    let record = h
        .root
        .join("engine/seen")
        .join(format!("{}.jsonl", inbox.id));
    drop_lines(&record, &format!("\"inode\":{inode},"));
    let path = h
        .runs_dir()
        .join(&filer.id)
        .join(format!("{}.json", run.id));
    let mut saved: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    saved["status"] = json!("running");
    fs::write(&path, serde_json::to_vec(&saved).unwrap()).unwrap();

    let mut h = h.restart();
    h.changed("Inbox");

    let runs = h.settle().await;
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, RunStatus::Interrupted);
}

#[tokio::test]
async fn changing_the_folder_leaves_the_files_in_the_new_one_alone() {
    let mut h = engine();
    let wf = tagger(&h, "~/Downloads", &[], false);
    put(&h, "Desktop/Already.pdf");

    let mut moved = wf.clone();
    moved.steps[0] = serde_json::from_value(
        steps(json!([
            { "id": "t", "type": "fileAdded", "folder": "~/Desktop", "fileTypes": [],
              "subfolders": false, "next": "g" },
        ]))[0]
            .clone(),
    )
    .unwrap();
    h.save(moved);
    assert_eq!(
        *h.watched.0.lock().unwrap(),
        [(h.home.join("Desktop"), false)]
    );

    h.changed("Desktop");
    assert!(h.settle().await.is_empty());
    put(&h, "Desktop/New.pdf");
    h.changed("Desktop");
    assert_eq!(taken(&h.settle().await), ["New.pdf"]);
}

#[tokio::test]
async fn deleting_a_workflow_stops_watching_and_removes_its_record() {
    let h = engine();
    let wf = tagger(&h, "~/Downloads", &[], false);
    let record = h.root.join("engine/seen").join(format!("{}.jsonl", wf.id));
    assert!(record.exists());
    assert_eq!(
        *h.watched.0.lock().unwrap(),
        [(h.home.join("Downloads"), false)]
    );

    WorkflowStore::new(&DataDir::open(&h.root).unwrap())
        .delete(&wf.id)
        .unwrap();
    h.engine.reload(&wf.id);

    assert!(h.watched.0.lock().unwrap().is_empty());
    assert!(!record.exists());
}

fn drop_lines(path: &Path, containing: &str) {
    let text = fs::read_to_string(path).unwrap();
    let kept: Vec<&str> = text.lines().filter(|l| !l.contains(containing)).collect();
    assert_ne!(kept.len(), text.lines().count(), "nothing to drop");
    fs::write(path, kept.join("\n") + "\n").unwrap();
}

#[tokio::test]
async fn the_screenshots_template_files_new_screenshots_only() {
    let mut h = engine();
    let store = WorkflowStore::new(&DataDir::open(&h.root).unwrap());
    let wf = store
        .create(vela_lib::workflow::templates::build("screenshots").unwrap())
        .unwrap();
    fs::create_dir_all(h.home.join("Desktop")).unwrap();
    put(&h, "Desktop/Screenshot 2026-09-01 at 08.00.00.png");
    h.switch(&wf, true);

    put(&h, "Desktop/Screenshot 2026-09-14 at 10.00.00.png");
    put(&h, "Desktop/Holiday.png");
    h.changed("Desktop");
    let runs = h.settle().await;

    assert_eq!(runs.len(), 2);
    assert!(runs.iter().all(|r| r.status == RunStatus::Done));
    assert_eq!(
        names(&h.home.join("Pictures/Screenshots/2026")),
        ["Screenshot 2026-09-14 at 10.00.00.png"]
    );
    assert_eq!(
        names(&h.home.join("Desktop")),
        ["Holiday.png", "Screenshot 2026-09-01 at 08.00.00.png"]
    );
}
