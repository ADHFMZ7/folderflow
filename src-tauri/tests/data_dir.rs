//! The data folder is private to the user.

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt;

use folderflow_lib::storage::data_dir::DataDir;

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
