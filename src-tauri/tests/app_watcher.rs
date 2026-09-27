//! The real folder watcher: FSEvents tells of a file written in a watched
//! folder, and nothing after the folder is no longer watched.

use std::time::Duration;

use folderflow_lib::engine::app::AppWatcher;
use folderflow_lib::engine::Watcher;
use tokio::sync::mpsc;

#[tokio::test]
async fn it_tells_of_changes_in_watched_folders_only() {
    let tmp = tempfile::tempdir().unwrap();
    let folder = std::fs::canonicalize(tmp.path()).unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let watcher = AppWatcher::new(tx).unwrap();

    watcher.watch(&[(folder.clone(), false)]);
    std::fs::write(folder.join("new.pdf"), b"x").unwrap();

    let told = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let path = rx.recv().await.unwrap();
            if path.starts_with(&folder) {
                return path;
            }
        }
    })
    .await
    .expect("FSEvents never told of the new file");
    assert_eq!(told, folder.join("new.pdf"));

    watcher.watch(&[]);
    // Anything already on its way is drained; then a new file brings nothing.
    tokio::time::sleep(Duration::from_millis(500)).await;
    while rx.try_recv().is_ok() {}
    std::fs::write(folder.join("later.pdf"), b"x").unwrap();
    let later = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await;
    assert!(later.is_err(), "told of a folder no longer watched");
}
