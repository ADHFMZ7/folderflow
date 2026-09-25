//! Crash-safe writes: never a half-written file, never a write outside the target.

mod common;

use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::thread;

use folderflow_lib::storage::atomic::write_atomic;

#[test]
fn replaces_the_whole_file() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("settings.json");
    fs::write(
        &path,
        "a much longer original file that must not leave a tail behind",
    )
    .unwrap();

    write_atomic(&path, b"short").unwrap();

    assert_eq!(fs::read_to_string(&path).unwrap(), "short");
}

#[test]
fn new_files_are_readable_only_by_their_owner() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("settings.json");

    write_atomic(&path, b"{}").unwrap();

    assert_eq!(common::mode(&path), 0o600);
}

#[test]
fn a_failed_write_keeps_the_old_file_and_leaves_no_temp_files() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("settings.json");
    fs::write(&path, "original").unwrap();
    // A read-only folder makes the write fail partway, like a full disk would.
    fs::set_permissions(tmp.path(), fs::Permissions::from_mode(0o500)).unwrap();

    let result = write_atomic(&path, b"replacement");
    fs::set_permissions(tmp.path(), fs::Permissions::from_mode(0o700)).unwrap();

    assert!(result.is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "original");
    assert_eq!(common::files_under(tmp.path()), vec![path]);
}

#[test]
fn a_successful_write_leaves_no_temp_files() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("settings.json");

    for i in 0..5 {
        write_atomic(&path, format!("version {i}").as_bytes()).unwrap();
    }

    assert_eq!(common::files_under(tmp.path()), vec![path]);
}

#[test]
fn a_symlink_is_replaced_not_written_through() {
    let tmp = tempfile::tempdir().unwrap();
    let outside = tmp.path().join("outside.txt");
    fs::write(&outside, "not ours to change").unwrap();
    let data = tmp.path().join("data");
    fs::create_dir(&data).unwrap();
    let path = data.join("settings.json");
    symlink(&outside, &path).unwrap();

    write_atomic(&path, b"{}").unwrap();

    assert_eq!(fs::read_to_string(&outside).unwrap(), "not ours to change");
    assert!(!fs::symlink_metadata(&path)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(fs::read_to_string(&path).unwrap(), "{}");
}

#[test]
fn concurrent_writers_never_produce_a_mixed_file() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("settings.json");
    let contents: Vec<Vec<u8>> = (b'a'..=b'h').map(|c| vec![c; 256 * 1024]).collect();

    thread::scope(|s| {
        for bytes in &contents {
            let path = &path;
            s.spawn(move || {
                for _ in 0..10 {
                    write_atomic(path, bytes).unwrap();
                }
            });
        }
    });

    let result = fs::read(&path).unwrap();
    assert!(
        contents.contains(&result),
        "the file mixes bytes from different writers"
    );
    assert_eq!(common::files_under(tmp.path()), vec![path]);
}
