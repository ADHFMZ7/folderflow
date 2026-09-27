//! Moves to another disk: the copy is whole in place before the original goes,
//! and undo brings it back across. Needs a second disk, so it makes a small
//! disk image with hdiutil; run with `cargo test --test files_other_disk -- --ignored`.

mod common;

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use folderflow_lib::engine::files::{self, Grant, Grants};

use common::files::{names, place};

const RUN: &str = "5d0b6a0e-0000-4000-8000-0000000000dd";

/// A mounted disk image, detached when dropped.
struct Disk {
    mount: PathBuf,
}

impl Drop for Disk {
    fn drop(&mut self) {
        let _ = Command::new("hdiutil")
            .args(["detach", "-force"])
            .arg(&self.mount)
            .status();
    }
}

fn disk(dir: &std::path::Path) -> Disk {
    let image = dir.join("other.dmg");
    let mount = dir.join("mnt");
    let made = Command::new("hdiutil")
        .args([
            "create", "-size", "8m", "-fs", "APFS", "-volname", "FFTest", "-quiet",
        ])
        .arg(&image)
        .status()
        .unwrap();
    assert!(made.success());
    fs::create_dir_all(&mount).unwrap();
    let attached = Command::new("hdiutil")
        .args(["attach", "-nobrowse", "-quiet", "-mountpoint"])
        .arg(&mount)
        .arg(&image)
        .status()
        .unwrap();
    assert!(attached.success());
    Disk { mount }
}

#[test]
#[ignore = "makes and mounts a disk image"]
fn a_move_to_another_disk_copies_first_and_undo_brings_it_back() {
    let p = place();
    let other = tempfile::tempdir().unwrap();
    let disk = disk(other.path());
    let f = p.file("Downloads/Scan.pdf", "a receipt");
    let before = p.tree();

    let grants = Grants::new(vec![
        Grant {
            folder: p.at("Downloads"),
            below: false,
        },
        Grant {
            folder: disk.mount.clone(),
            below: true,
        },
    ]);
    let mut acts = files::Files::open(&p.journal, RUN, grants).unwrap();
    let moved = acts.move_file(&f, &disk.mount.join("Receipts")).unwrap();

    assert_eq!(fs::read_to_string(&moved).unwrap(), "a receipt");
    assert!(!f.exists(), "the original went once the copy was in place");
    assert_eq!(
        names(&disk.mount.join("Receipts")),
        ["Scan.pdf"],
        "no temporary file left"
    );

    let report = files::undo(&p.journal, RUN, &p.trash, &files::NoWrites).unwrap();

    assert!(report.left_alone.is_empty(), "{:?}", report.left_alone);
    assert_eq!(p.tree(), before);
    assert!(!disk.mount.join("Receipts").exists());
}
