//! The data folder is private to the user.

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt;

use vela_lib::storage::data_dir::DataDir;

#[test]
fn a_missing_folder_is_created_readable_only_by_its_owner() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp
        .path()
        .join("Application Support")
        .join("com.example.app");

    let dir = DataDir::open(&root).unwrap();

    assert_eq!(dir.root(), root);
    assert_eq!(common::mode(&root), 0o700);
}

#[test]
fn an_existing_folder_keeps_its_files_and_loses_loose_permissions() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("data");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("settings.json"), "{}").unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();

    DataDir::open(&root).unwrap();

    assert_eq!(common::mode(&root), 0o700);
    assert_eq!(
        fs::read_to_string(root.join("settings.json")).unwrap(),
        "{}"
    );
}

#[test]
fn the_folder_of_an_earlier_name_moves_over_once() {
    let tmp = tempfile::tempdir().unwrap();
    let old = tmp.path().join("com.example.before");
    let root = tmp.path().join("com.example.after");
    fs::create_dir_all(old.join("workflows")).unwrap();
    fs::write(old.join("workflows/w.json"), "{}").unwrap();

    let dir = DataDir::open_moving(&root, &old).unwrap();

    assert_eq!(dir.root(), root);
    assert!(root.join("workflows/w.json").exists());
    assert!(!old.exists());
    assert_eq!(common::mode(&root), 0o700);
}

#[test]
fn an_existing_folder_is_kept_and_the_earlier_one_left_alone() {
    let tmp = tempfile::tempdir().unwrap();
    let old = tmp.path().join("com.example.before");
    let root = tmp.path().join("com.example.after");
    fs::create_dir_all(&old).unwrap();
    fs::write(old.join("settings.json"), "old").unwrap();
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("settings.json"), "new").unwrap();

    DataDir::open_moving(&root, &old).unwrap();

    assert_eq!(
        fs::read_to_string(root.join("settings.json")).unwrap(),
        "new"
    );
    assert_eq!(
        fs::read_to_string(old.join("settings.json")).unwrap(),
        "old"
    );
}

#[test]
fn with_no_earlier_folder_it_is_simply_created() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("com.example.after");
    DataDir::open_moving(&root, &tmp.path().join("missing")).unwrap();
    assert!(root.is_dir());
}
