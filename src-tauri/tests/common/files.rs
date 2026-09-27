//! Helpers for the file action tests: a home folder in a temp dir, a trash
//! that is a folder beside it, and a snapshot of every file for comparing
//! trees before and after.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use folderflow_lib::engine::files::{sys, Files, Grant, Grants, Trash};

/// Moves "trashed" files into a folder, so tests never touch the real Trash.
pub struct FolderTrash(pub PathBuf);

impl Trash for FolderTrash {
    fn trash(&self, path: &Path) -> std::io::Result<()> {
        fs::create_dir_all(&self.0)?;
        let to = self.0.join(format!(
            "{}-{}",
            uuid::Uuid::new_v4(),
            path.file_name().unwrap().to_string_lossy()
        ));
        fs::rename(path, to)
    }
}

pub struct Place {
    pub _tmp: tempfile::TempDir,
    pub home: PathBuf,
    pub journal: PathBuf,
    pub trash: FolderTrash,
}

pub fn place() -> Place {
    let tmp = tempfile::tempdir().unwrap();
    // Resolved, so paths compare equal to what the engine reports.
    let root = fs::canonicalize(tmp.path()).unwrap();
    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    Place {
        journal: root.join("journal"),
        trash: FolderTrash(root.join("trash")),
        home,
        _tmp: tmp,
    }
}

impl Place {
    pub fn at(&self, rel: &str) -> PathBuf {
        self.home.join(rel)
    }

    /// Writes a file (and its folders) and returns its path.
    pub fn file(&self, rel: &str, contents: &str) -> PathBuf {
        let path = self.at(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, contents).unwrap();
        path
    }

    /// Grants for these folders under home: (folder, and below).
    pub fn grants(&self, folders: &[(&str, bool)]) -> Grants {
        Grants::new(
            folders
                .iter()
                .map(|(f, below)| Grant {
                    folder: self.at(f),
                    below: *below,
                })
                .collect(),
        )
    }

    /// File actions for one run, allowed in `folders`.
    pub fn files(&self, run: &str, folders: &[(&str, bool)]) -> Files {
        Files::open(&self.journal, run, self.grants(folders)).unwrap()
    }

    /// Every file and folder under home: path → contents and tags (folders
    /// have `None`). Hidden temporary files would show up here too.
    pub fn tree(&self) -> BTreeMap<String, Option<(String, Vec<String>)>> {
        let mut out = BTreeMap::new();
        walk(&self.home, &self.home, &mut out);
        out
    }
}

fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Option<(String, Vec<String>)>>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let rel = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        if path.is_dir() {
            out.insert(format!("{rel}/"), None);
            walk(root, &path, out);
        } else {
            let text = String::from_utf8_lossy(&fs::read(&path).unwrap()).into_owned();
            out.insert(rel, Some((text, sys::tags(&path).unwrap())));
        }
    }
}

/// The names in a folder, sorted.
pub fn names(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    out.sort();
    out
}
