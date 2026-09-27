//! File actions: nothing is ever overwritten or deleted, nothing happens
//! outside the granted folders, every action is written down before it
//! happens, a crash at any point is recovered, and undo puts things back.
//! See docs/engine.md, "File safety", and TESTING.md, "Critical risks".

mod common;

use std::fs;
use std::os::unix::fs::symlink;

use vela_lib::engine::files::{self, sys, CrashPoint};

use common::files::{names, place};

const RUN: &str = "5d0b6a0e-0000-4000-8000-000000000001";

// ---- Rename -----------------------------------------------------------------

#[test]
fn rename_keeps_the_extension() {
    let p = place();
    let f = p.file("Downloads/Scan_0042.pdf", "receipt");
    let mut files = p.files(RUN, &[("Downloads", false)]);

    let renamed = files.rename(&f, "2026-09-14 Blue Bottle").unwrap();

    assert_eq!(renamed, p.at("Downloads/2026-09-14 Blue Bottle.pdf"));
    assert_eq!(names(&p.at("Downloads")), ["2026-09-14 Blue Bottle.pdf"]);
    assert_eq!(fs::read_to_string(renamed).unwrap(), "receipt");
}

#[test]
fn a_name_that_is_taken_gets_a_number_and_both_files_are_kept() {
    let p = place();
    p.file("Downloads/Receipt.pdf", "old");
    p.file("Downloads/Receipt 2.pdf", "older");
    let f = p.file("Downloads/Scan.pdf", "new");
    let mut files = p.files(RUN, &[("Downloads", false)]);

    let renamed = files.rename(&f, "Receipt").unwrap();

    assert_eq!(renamed, p.at("Downloads/Receipt 3.pdf"));
    assert_eq!(
        fs::read_to_string(p.at("Downloads/Receipt.pdf")).unwrap(),
        "old"
    );
    assert_eq!(
        fs::read_to_string(p.at("Downloads/Receipt 2.pdf")).unwrap(),
        "older"
    );
    assert_eq!(fs::read_to_string(renamed).unwrap(), "new");
}

#[test]
fn renaming_to_the_same_name_or_only_its_case_works() {
    let p = place();
    let f = p.file("Downloads/invoice.pdf", "x");
    let mut files = p.files(RUN, &[("Downloads", false)]);

    assert_eq!(files.rename(&f, "invoice").unwrap(), f);
    let renamed = files.rename(&f, "Invoice").unwrap();

    assert_eq!(names(&p.at("Downloads")), ["Invoice.pdf"]);
    assert_eq!(renamed, p.at("Downloads/Invoice.pdf"));
}

#[test]
fn a_file_without_an_extension_stays_without_one() {
    let p = place();
    let f = p.file("Downloads/README", "x");
    let mut files = p.files(RUN, &[("Downloads", false)]);
    assert_eq!(files.rename(&f, "Notes").unwrap(), p.at("Downloads/Notes"));
}

// ---- Move and copy ------------------------------------------------------------

#[test]
fn move_creates_missing_folders_and_keeps_both_on_a_clash() {
    let p = place();
    p.file("Documents/Receipts/2026/Receipt.pdf", "already there");
    let f = p.file("Downloads/Receipt.pdf", "new");
    let mut files = p.files(RUN, &[("Downloads", false), ("Documents/Receipts", true)]);

    let moved = files
        .move_file(&f, &p.at("Documents/Receipts/2026"))
        .unwrap();
    assert_eq!(moved, p.at("Documents/Receipts/2026/Receipt 2.pdf"));

    let g = p.file("Downloads/Other.pdf", "other");
    let moved = files
        .move_file(&g, &p.at("Documents/Receipts/2027/09"))
        .unwrap();
    assert_eq!(moved, p.at("Documents/Receipts/2027/09/Other.pdf"));
    assert!(names(&p.at("Downloads")).is_empty());
    assert_eq!(
        fs::read_to_string(p.at("Documents/Receipts/2026/Receipt.pdf")).unwrap(),
        "already there"
    );
}

#[test]
fn moving_to_the_folder_its_in_does_nothing() {
    let p = place();
    let f = p.file("Downloads/a.pdf", "x");
    let mut files = p.files(RUN, &[("Downloads", false)]);
    assert_eq!(files.move_file(&f, &p.at("Downloads")).unwrap(), f);
    assert_eq!(names(&p.at("Downloads")), ["a.pdf"]);
}

#[test]
fn copy_leaves_the_original_and_makes_an_identical_copy() {
    let p = place();
    let f = p.file("Downloads/a.pdf", "contents");
    let mut files = p.files(RUN, &[("Downloads", false), ("Backup", true)]);

    let copy = files.copy_file(&f, &p.at("Backup")).unwrap();
    let again = files.copy_file(&f, &p.at("Backup")).unwrap();

    assert_eq!(fs::read_to_string(&f).unwrap(), "contents");
    assert_eq!(fs::read_to_string(&copy).unwrap(), "contents");
    assert_eq!(names(&p.at("Backup")), ["a 2.pdf", "a.pdf"]);
    assert_eq!(again, p.at("Backup/a 2.pdf"));
}

// ---- Create file ------------------------------------------------------------

#[test]
fn create_file_writes_the_file_and_numbers_a_taken_name_before_its_extension() {
    let p = place();
    p.file("Documents/Notes/Summary.md", "old");
    let mut files = p.files(RUN, &[("Documents/Notes", true)]);

    let made = files
        .create_file(&p.at("Documents/Notes"), "Summary.md", "# New")
        .unwrap();

    assert_eq!(made, p.at("Documents/Notes/Summary 2.md"));
    assert_eq!(fs::read_to_string(made).unwrap(), "# New");
    assert_eq!(
        fs::read_to_string(p.at("Documents/Notes/Summary.md")).unwrap(),
        "old"
    );
    // No temporary files are left behind.
    assert_eq!(
        names(&p.at("Documents/Notes")),
        ["Summary 2.md", "Summary.md"]
    );
}

#[test]
fn a_file_name_with_a_slash_is_refused() {
    let p = place();
    let mut files = p.files(RUN, &[("Documents", true)]);
    let err = files
        .create_file(&p.at("Documents"), "a/b.txt", "x")
        .unwrap_err();
    assert_eq!(err.to_string(), "A file name can't contain / or :.");
}

// ---- Add row ------------------------------------------------------------------

#[test]
fn add_row_starts_a_new_file_with_its_headings() {
    let p = place();
    let csv = p.at("Documents/Expenses.csv");
    let mut files = p.files(RUN, &[("Documents", false)]);

    let headers = ["Date".to_string(), "Vendor".into(), "Amount".into()];
    files
        .add_row(
            &csv,
            &["2026-09-14".into(), "Blue Bottle".into(), "4.50".into()],
            Some(&headers),
        )
        .unwrap();
    files
        .add_row(
            &csv,
            &["2026-09-15".into(), "Acme, Inc.".into(), "12".into()],
            Some(&headers),
        )
        .unwrap();

    assert_eq!(
        fs::read_to_string(&csv).unwrap(),
        "Date,Vendor,Amount\n2026-09-14,Blue Bottle,4.50\n2026-09-15,\"Acme, Inc.\",12\n"
    );
}

#[test]
fn add_row_keeps_the_files_own_line_endings_and_finishes_a_last_line() {
    let p = place();
    let crlf = p.file("Documents/a.csv", "A,B\r\n1,2\r\n");
    let unfinished = p.file("Documents/b.csv", "A,B\n1,2");
    let mut files = p.files(RUN, &[("Documents", false)]);

    files
        .add_row(&crlf, &["3".into(), "4".into()], None)
        .unwrap();
    files
        .add_row(&unfinished, &["3".into(), "4".into()], None)
        .unwrap();

    assert_eq!(fs::read_to_string(crlf).unwrap(), "A,B\r\n1,2\r\n3,4\r\n");
    assert_eq!(fs::read_to_string(unfinished).unwrap(), "A,B\n1,2\n3,4\n");
}

#[test]
fn values_that_a_spreadsheet_would_run_as_formulas_are_made_plain_text() {
    let p = place();
    let csv = p.at("Documents/Log.csv");
    let mut files = p.files(RUN, &[("Documents", false)]);

    let values = [
        "=HYPERLINK(\"http://evil\")".to_string(),
        "+1+1".into(),
        "@SUM(A1)".into(),
        "-12.50".into(),
        "He said \"hi\"".into(),
        "line\nbreak".into(),
    ];
    files.add_row(&csv, &values, None).unwrap();

    assert_eq!(
        fs::read_to_string(&csv).unwrap(),
        "\"'=HYPERLINK(\"\"http://evil\"\")\",'+1+1,'@SUM(A1),-12.50,\"He said \"\"hi\"\"\",\"line\nbreak\"\n"
    );
}

#[test]
fn a_spreadsheet_that_is_a_link_is_refused() {
    let p = place();
    let real = p.file("Documents/real.csv", "A\n");
    symlink(&real, p.at("Documents/link.csv")).unwrap();
    let mut files = p.files(RUN, &[("Documents", false)]);

    let err = files
        .add_row(&p.at("Documents/link.csv"), &["1".into()], None)
        .unwrap_err();

    assert_eq!(
        err.to_string(),
        "link.csv is a link, so Vela won't change it."
    );
    assert_eq!(fs::read_to_string(real).unwrap(), "A\n");
}

// ---- Tag --------------------------------------------------------------------

#[test]
fn tag_adds_only_the_tags_the_file_doesnt_have() {
    let p = place();
    let f = p.file("Downloads/a.pdf", "x");
    let mut files = p.files(RUN, &[("Downloads", false)]);

    assert_eq!(
        files.tag(&f, &["Receipts".into(), "2026".into()]).unwrap(),
        ["Receipts", "2026"]
    );
    assert_eq!(
        files.tag(&f, &["receipts".into(), "Paid".into()]).unwrap(),
        ["Paid"]
    );
    assert_eq!(sys::tags(&f).unwrap(), ["Receipts", "2026", "Paid"]);
}

// ---- Granted folders ------------------------------------------------------------

#[test]
fn nothing_happens_outside_the_granted_folders() {
    let p = place();
    let f = p.file("Downloads/a.pdf", "x");
    p.file("Documents/Private/secret.txt", "secret");
    let before = p.tree();
    let mut files = p.files(RUN, &[("Downloads", false), ("Documents/Receipts", true)]);

    let refused = [
        files.move_file(&f, &p.at("Documents/Private")).unwrap_err(),
        files.move_file(&f, &p.at("Documents")).unwrap_err(),
        files
            .move_file(&f, &p.at("Documents/Receipts/../Private"))
            .unwrap_err(),
        files.copy_file(&f, &p.at("Elsewhere")).unwrap_err(),
        files
            .create_file(&p.at("Documents/Private"), "x.txt", "x")
            .unwrap_err(),
        files
            .add_row(&p.at("Documents/Private/log.csv"), &["1".into()], None)
            .unwrap_err(),
        files
            .tag(&p.at("Documents/Private/secret.txt"), &["x".into()])
            .unwrap_err(),
        files
            .rename(&p.at("Documents/Private/secret.txt"), "y")
            .unwrap_err(),
        // Only Downloads itself is granted, not the folders in it.
        files.move_file(&f, &p.at("Downloads/Sub")).unwrap_err(),
    ];

    for err in &refused {
        assert!(
            err.to_string()
                .contains("isn't one this workflow may change"),
            "{err}"
        );
    }
    assert_eq!(p.tree(), before, "nothing on disk changed");
}

#[test]
fn a_link_out_of_a_granted_folder_is_refused() {
    let p = place();
    let f = p.file("Downloads/a.pdf", "x");
    fs::create_dir_all(p.at("Documents/Receipts")).unwrap();
    fs::create_dir_all(p.at("Private")).unwrap();
    symlink(p.at("Private"), p.at("Documents/Receipts/escape")).unwrap();
    let mut files = p.files(RUN, &[("Downloads", false), ("Documents/Receipts", true)]);

    let err = files
        .move_file(&f, &p.at("Documents/Receipts/escape/2026"))
        .unwrap_err();

    assert!(err
        .to_string()
        .contains("isn't one this workflow may change"));
    assert!(names(&p.at("Private")).is_empty());
    assert!(f.exists());
}

#[test]
fn a_link_swapped_in_after_the_grants_were_worked_out_is_still_refused() {
    let p = place();
    let f = p.file("Downloads/a.pdf", "x");
    fs::create_dir_all(p.at("Documents/Receipts/2026")).unwrap();
    fs::create_dir_all(p.at("Private")).unwrap();
    let mut files = p.files(RUN, &[("Downloads", false), ("Documents/Receipts", true)]);

    // Between working out the grants and moving, the folder becomes a link out.
    fs::remove_dir(p.at("Documents/Receipts/2026")).unwrap();
    symlink(p.at("Private"), p.at("Documents/Receipts/2026")).unwrap();

    assert!(files
        .move_file(&f, &p.at("Documents/Receipts/2026"))
        .is_err());
    assert!(names(&p.at("Private")).is_empty());
}

#[test]
fn a_granted_folder_matches_whatever_case_it_is_written_in() {
    let p = place();
    let f = p.file("Downloads/a.pdf", "x");
    fs::create_dir_all(p.at("Documents/Receipts")).unwrap();
    let mut files = p.files(RUN, &[("Downloads", false), ("documents/receipts", true)]);

    let moved = files.move_file(&f, &p.at("Documents/Receipts")).unwrap();

    assert_eq!(moved, p.at("Documents/Receipts/a.pdf"));
}

// ---- Undo -------------------------------------------------------------------

#[test]
fn undo_puts_every_action_back_newest_first() {
    let p = place();
    let f = p.file("Downloads/Scan.pdf", "receipt");
    let csv = p.file(
        "Documents/Receipts/Expenses.csv",
        "Date,Amount\n2026-01-01,3\n",
    );
    p.file("Documents/Receipts/2026/keep.txt", "keep");
    let before = p.tree();

    let mut files = p.files(RUN, &[("Downloads", false), ("Documents/Receipts", true)]);
    let f = files.rename(&f, "2026-09-14 Blue Bottle").unwrap();
    files.tag(&f, &["Receipts".into()]).unwrap();
    let f = files
        .move_file(&f, &p.at("Documents/Receipts/2026/09"))
        .unwrap();
    files
        .copy_file(&f, &p.at("Documents/Receipts/Copies"))
        .unwrap();
    files
        .create_file(&p.at("Documents/Receipts/2026/09"), "note.txt", "hi")
        .unwrap();
    files
        .add_row(&csv, &["2026-09-14".into(), "4.50".into()], None)
        .unwrap();
    files
        .add_row(&p.at("Documents/Receipts/New.csv"), &["x".into()], None)
        .unwrap();
    assert_ne!(p.tree(), before);

    let report = files::undo(&p.journal, RUN, &p.trash, &files::NoWrites).unwrap();

    assert_eq!(p.tree(), before);
    assert!(report.left_alone.is_empty(), "{:?}", report.left_alone);
    assert_eq!(report.restored, 7);
    // Files Vela made went to the Trash, not away.
    assert_eq!(names(&p.trash.0).len(), 3);
}

#[test]
fn undo_leaves_alone_a_file_changed_since_and_says_so() {
    let p = place();
    let f = p.file("Downloads/Invoice.pdf", "v1");
    let mut files = p.files(RUN, &[("Downloads", false), ("Documents", true)]);
    let moved = files.move_file(&f, &p.at("Documents/Invoices")).unwrap();
    let made = files
        .create_file(&p.at("Documents"), "notes.txt", "mine")
        .unwrap();

    fs::write(&moved, "v2, edited by the person").unwrap();
    fs::write(&made, "edited too").unwrap();
    let report = files::undo(&p.journal, RUN, &p.trash, &files::NoWrites).unwrap();

    assert_eq!(
        fs::read_to_string(&moved).unwrap(),
        "v2, edited by the person"
    );
    assert_eq!(fs::read_to_string(&made).unwrap(), "edited too");
    let reasons: Vec<_> = report
        .left_alone
        .iter()
        .map(|l| l.reason.as_str())
        .collect();
    assert_eq!(
        reasons,
        [
            "notes.txt was changed after this run, so it wasn't moved to the Trash.",
            "Invoice.pdf was changed after this run, so it wasn't moved back.",
        ]
    );
    // The folder the move created still holds the file, so it stays.
    assert!(p.at("Documents/Invoices").is_dir());
}

#[test]
fn undo_brings_a_file_back_under_a_new_number_if_its_old_name_was_taken() {
    let p = place();
    let f = p.file("Downloads/Scan.pdf", "mine");
    let mut files = p.files(RUN, &[("Downloads", false)]);
    files.rename(&f, "Receipt").unwrap();
    p.file("Downloads/Scan.pdf", "someone else's");

    files::undo(&p.journal, RUN, &p.trash, &files::NoWrites).unwrap();

    assert_eq!(
        fs::read_to_string(p.at("Downloads/Scan.pdf")).unwrap(),
        "someone else's"
    );
    assert_eq!(
        fs::read_to_string(p.at("Downloads/Scan 2.pdf")).unwrap(),
        "mine"
    );
}

#[test]
fn undo_keeps_a_row_when_the_spreadsheet_was_edited_since() {
    let p = place();
    let csv = p.file("Documents/Log.csv", "A\n1\n");
    let mut files = p.files(RUN, &[("Documents", false)]);
    files.add_row(&csv, &["2".into()], None).unwrap();
    fs::write(&csv, "A\n1\n2\n3 typed by the person\n").unwrap();

    let report = files::undo(&p.journal, RUN, &p.trash, &files::NoWrites).unwrap();

    assert_eq!(
        fs::read_to_string(&csv).unwrap(),
        "A\n1\n2\n3 typed by the person\n"
    );
    assert_eq!(
        report.left_alone[0].reason,
        "Log.csv was changed after this run, so the row this run added is still in it."
    );
}

#[test]
fn undo_removes_only_the_tags_the_run_added() {
    let p = place();
    let f = p.file("Downloads/a.pdf", "x");
    let mut files = p.files(RUN, &[("Downloads", false)]);
    files.tag(&f, &["Mine".into()]).unwrap();
    files.tag(&f, &["Receipts".into()]).unwrap();
    // Tagged by the person after the run, with a tag the run also added.
    sys::add_tags(&f, &["Keep".into()]).unwrap();

    files::undo(&p.journal, RUN, &p.trash, &files::NoWrites).unwrap();

    assert_eq!(sys::tags(&f).unwrap(), ["Keep"]);
}

#[test]
fn undoing_twice_changes_nothing_more() {
    let p = place();
    let f = p.file("Downloads/a.pdf", "x");
    let mut files = p.files(RUN, &[("Downloads", false)]);
    files.rename(&f, "b").unwrap();

    let first = files::undo(&p.journal, RUN, &p.trash, &files::NoWrites).unwrap();
    let second = files::undo(&p.journal, RUN, &p.trash, &files::NoWrites).unwrap();

    assert_eq!((first.restored, second.restored), (1, 0));
    assert_eq!(names(&p.at("Downloads")), ["a.pdf"]);
}

// ---- Crashes ------------------------------------------------------------------

/// Runs one action of each kind with a crash at `point`, then recovers as the
/// next start of the app does, and checks the files are whole and undo works.
fn crash_each_action_at(point: CrashPoint) {
    type Act = fn(&mut files::Files, &common::files::Place) -> Result<(), files::FileError>;
    let actions: [(&str, Act); 7] = [
        ("rename", |f, p| {
            f.rename(&p.at("Downloads/a.pdf"), "b").map(drop)
        }),
        ("move", |f, p| {
            f.move_file(&p.at("Downloads/a.pdf"), &p.at("Documents/New"))
                .map(drop)
        }),
        ("copy", |f, p| {
            f.copy_file(&p.at("Downloads/a.pdf"), &p.at("Documents"))
                .map(drop)
        }),
        ("create", |f, p| {
            f.create_file(&p.at("Documents"), "n.txt", "n").map(drop)
        }),
        ("add row", |f, p| {
            f.add_row(&p.at("Documents/log.csv"), &["2".into()], None)
        }),
        ("new sheet", |f, p| {
            f.add_row(&p.at("Documents/new.csv"), &["2".into()], None)
        }),
        ("tag", |f, p| {
            f.tag(&p.at("Downloads/a.pdf"), &["T".into()]).map(drop)
        }),
    ];
    for (name, act) in actions {
        let p = place();
        p.file("Downloads/a.pdf", "original");
        p.file("Documents/log.csv", "A\n1\n");
        let before = p.tree();

        let mut files = p.files(RUN, &[("Downloads", false), ("Documents", true)]);
        files.crash_at(point);
        assert!(act(&mut files, &p).is_err(), "{name}: the crash happened");
        drop(files);

        files::recover(&p.journal).unwrap();
        // Whatever state it was left in, no file was lost or half-written.
        let after = p.tree();
        let originals: Vec<_> = after
            .iter()
            .filter(|(_, v)| matches!(v, Some((text, _)) if text == "original"))
            .map(|(k, _)| k.as_str())
            .filter(|k| k.starts_with("Downloads/") || k.starts_with("Documents/New/"))
            .collect();
        assert_eq!(
            originals.len(),
            1,
            "{name}: the file is in exactly one place: {after:?}"
        );
        assert!(
            !after.keys().any(|k| k.contains(".ffpart")),
            "{name}: no temporary file is left: {after:?}"
        );
        if let Some(Some((log, _))) = after.get("Documents/log.csv") {
            assert!(
                log == "A\n1\n" || log == "A\n1\n2\n",
                "{name}: the sheet is whole: {log:?}"
            );
        }

        files::undo(&p.journal, RUN, &p.trash, &files::NoWrites).unwrap();
        assert_eq!(
            p.tree(),
            before,
            "{name}: undo after recovery restores the start"
        );
    }
}

#[test]
fn a_crash_after_writing_down_an_action_but_before_doing_it_loses_nothing() {
    crash_each_action_at(CrashPoint::AfterIntent);
}

#[test]
fn a_crash_after_doing_an_action_but_before_writing_it_down_is_recovered() {
    crash_each_action_at(CrashPoint::BeforeDone);
}

#[test]
fn recovery_says_which_runs_were_cut_off() {
    let p = place();
    let f = p.file("Downloads/a.pdf", "x");
    let mut files = p.files(RUN, &[("Downloads", false)]);
    files.crash_at(CrashPoint::BeforeDone);
    let _ = files.rename(&f, "b");
    drop(files);

    assert_eq!(files::recover(&p.journal).unwrap(), [RUN]);
    assert!(files::recover(&p.journal).unwrap().is_empty(), "only once");
}
