//! Every change FolderFlow makes to a person's files goes through here. See
//! docs/engine.md, "File safety".
//!
//! - Nothing is overwritten: names are claimed with `renamex_np(RENAME_EXCL)`,
//!   `clonefile` and `O_EXCL`, which fail rather than replace, and a taken
//!   name gets " 2", " 3"…
//! - Nothing is deleted: what undo removes goes to the Trash. The one file
//!   removed is the original of a move to another disk, once its copy is
//!   whole in place.
//! - Nothing happens outside the granted folders, checked with links resolved
//!   just before each action.
//! - Every action is written to the run's journal before and after, so a crash
//!   at any point is recovered (`recover`) and every action can be undone
//!   (`undo`).

mod journal;
pub mod sys;
mod undo;

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use journal::{Action, Journal};
use sys::Stamp;

use crate::workflow::{paths, validate::variables_in, StepKind, Workflow};

pub use undo::{recover, recover_all, undo, LeftAlone, Recovered, UndoReport};

/// Moves files to the Trash. Behind a trait so tests keep their own.
pub trait Trash: Send + Sync {
    fn trash(&self, path: &Path) -> io::Result<()>;
}

/// Hears of every file FolderFlow is about to put somewhere and whether it
/// did, so a workflow watching that folder doesn't take it for a new file
/// (docs/engine.md, decision 8).
pub trait Writes: Send + Sync {
    /// Before a file is put at `path`.
    fn writing(&self, path: &Path);
    /// After: `written` is whether FolderFlow's file is at `path` now.
    fn finished(&self, path: &Path, written: bool);
}

/// For when nothing is watching.
pub struct NoWrites;

impl Writes for NoWrites {
    fn writing(&self, _: &Path) {}
    fn finished(&self, _: &Path, _: bool) {}
}

/// Why a file action didn't happen, in plain words.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct FileError(pub String);

/// A folder a workflow may change: the folder itself, and the folders inside
/// it when `below`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    pub folder: PathBuf,
    pub below: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Grants(Vec<Grant>);

impl Grants {
    pub fn new(grants: Vec<Grant>) -> Grants {
        Grants(grants)
    }

    pub fn list(&self) -> &[Grant] {
        &self.0
    }

    /// What a run of `workflow` may change: the folder its file is in, the
    /// trigger's folder (and below, with subfolders on), and for each Move,
    /// Create file and Add row, the fixed part of its path, and below. A path
    /// that starts with a variable grants nothing of its own; one that names a
    /// forbidden folder grants nothing at all.
    pub fn for_run(workflow: &Workflow, file: Option<&Path>, home: &Path) -> Grants {
        let mut out = Vec::new();
        if let Some(folder) = file.and_then(Path::parent) {
            out.push(Grant {
                folder: folder.to_path_buf(),
                below: false,
            });
        }
        for step in &workflow.steps {
            let (path, is_file, below) = match &step.kind {
                StepKind::FileAdded {
                    folder, subfolders, ..
                } => (folder.as_str(), false, *subfolders),
                StepKind::Move { to, .. } => (to.as_str(), false, true),
                StepKind::CreateFile {
                    folder: Some(folder),
                    ..
                } => (folder.as_str(), false, true),
                StepKind::AddRow { file, .. } => {
                    (file.as_str(), true, !variables_in(file).is_empty())
                }
                _ => continue,
            };
            let Some(fixed) = paths::fixed_folder(path, is_file) else {
                continue;
            };
            if paths::forbidden(&fixed) {
                continue;
            }
            let folder = match fixed.strip_prefix('~') {
                Some("") => home.to_path_buf(),
                Some(rest) => match rest.strip_prefix('/') {
                    Some(rest) => home.join(rest),
                    None => continue,
                },
                None if fixed.starts_with('/') => PathBuf::from(&fixed),
                None => continue,
            };
            out.push(Grant { folder, below });
        }
        Grants(out)
    }

    /// `folder` with links resolved, if the workflow may change it. Checked
    /// against the disk as it is now, so a link swapped in earlier is caught.
    fn allow(&self, folder: &Path) -> Result<PathBuf, FileError> {
        let refused = || {
            FileError(format!(
                "{} isn't one this workflow may change.",
                folder.display()
            ))
        };
        if !folder.is_absolute() || folder.components().any(|c| c == Component::ParentDir) {
            return Err(refused());
        }
        let real = resolve(folder).map_err(|_| refused())?;
        for grant in &self.0 {
            let Ok(root) = resolve(&grant.folder) else {
                continue;
            };
            let ignore_case = sys::ignores_case(existing_ancestor(&root));
            if within(&real, &root, grant.below, ignore_case) {
                return Ok(real);
            }
        }
        Err(refused())
    }
}

/// The path with links resolved in the part that exists; the rest as written.
fn resolve(path: &Path) -> io::Result<PathBuf> {
    let existing = existing_ancestor(path);
    let mut out = fs::canonicalize(existing)?;
    if let Ok(rest) = path.strip_prefix(existing) {
        out.push(rest);
    }
    Ok(out)
}

fn existing_ancestor(path: &Path) -> &Path {
    path.ancestors()
        .find(|p| fs::symlink_metadata(p).is_ok())
        .unwrap_or(Path::new("/"))
}

fn within(path: &Path, root: &Path, below: bool, ignore_case: bool) -> bool {
    let key = |p: &Path| -> Vec<String> {
        p.components()
            .map(|c| {
                let s = c.as_os_str().to_string_lossy();
                if ignore_case {
                    s.to_lowercase()
                } else {
                    s.into_owned()
                }
            })
            .collect()
    };
    let (path, root) = (key(path), key(root));
    if below {
        path.starts_with(&root)
    } else {
        path == root
    }
}

/// Where a test makes the next action stop, as if the app had crashed.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashPoint {
    /// After the intent is written, before the disk is touched.
    AfterIntent,
    /// After the disk is changed, before the done line is written.
    BeforeDone,
}

/// The file actions of one run.
pub struct Files {
    grants: Grants,
    journal: Journal,
    backups: PathBuf,
    writes: Arc<dyn Writes>,
    crash: Option<CrashPoint>,
}

/// How many " 2", " 3"… to try before giving up on a name.
const MAX_NUMBER: u32 = 999;

/// What doing an action left behind, for the done line.
struct Did {
    stamp: Option<Stamp>,
    added: Vec<String>,
}

impl Files {
    /// Opens the run's journal in `journal_dir`, carrying on from any earlier
    /// actions of the same run.
    pub fn open(journal_dir: &Path, run: &str, grants: Grants) -> io::Result<Files> {
        Ok(Files {
            journal: Journal::open(journal_dir, run)?,
            backups: journal::backups(journal_dir, run),
            grants,
            writes: Arc::new(NoWrites),
            crash: None,
        })
    }

    /// Tells `writes` of every file these actions put somewhere.
    pub fn telling(mut self, writes: Arc<dyn Writes>) -> Files {
        self.writes = writes;
        self
    }

    #[doc(hidden)]
    pub fn crash_at(&mut self, point: CrashPoint) {
        self.crash = Some(point);
    }

    /// Renames the file, keeping its extension. Returns where it is now.
    pub fn rename(&mut self, file: &Path, new_stem: &str) -> Result<PathBuf, FileError> {
        let (folder, name) = self.source(file)?;
        check_name(new_stem)?;
        let ext = extension(&name);
        let from = folder.join(&name);
        self.place(&from, &folder, new_stem, ext.as_deref())
    }

    /// Moves the file into `folder`, creating it if needed. Returns where it is now.
    pub fn move_file(&mut self, file: &Path, folder: &Path) -> Result<PathBuf, FileError> {
        let (from_folder, name) = self.source(file)?;
        let to_folder = self.grants.allow(folder)?;
        let from = from_folder.join(&name);
        if same_folder(&from_folder, &to_folder) {
            return Ok(from);
        }
        self.make_dirs(&to_folder)?;
        let (stem, ext) = split(&name);
        self.place(&from, &to_folder, &stem, ext.as_deref())
    }

    /// Copies the file into `folder`, creating it if needed. Returns the copy.
    pub fn copy_file(&mut self, file: &Path, folder: &Path) -> Result<PathBuf, FileError> {
        let (from_folder, name) = self.source(file)?;
        let to_folder = self.grants.allow(folder)?;
        let from = from_folder.join(&name);
        self.make_dirs(&to_folder)?;
        let (stem, ext) = split(&name);
        for n in 1..=MAX_NUMBER {
            let to = to_folder.join(numbered(&stem, ext.as_deref(), n));
            if exists(&to) {
                continue;
            }
            let action = Action::Copy {
                from: from.clone(),
                to: to.clone(),
                part: None,
            };
            let cloned = self.step(action, || match sys::clone_new(&from, &to) {
                Ok(()) => Ok(stamped(&to)),
                Err(e) => Err(e),
            });
            match cloned {
                Ok(()) => return Ok(to),
                Err(Failed::Io(e)) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(Failed::Io(_)) => {
                    // This disk can't clone: copy the bytes instead.
                    let part = part_path(&to_folder, &to);
                    let action = Action::Copy {
                        from: from.clone(),
                        to: to.clone(),
                        part: Some(part.clone()),
                    };
                    match self.step(action, || {
                        copy_bytes(&from, &part)?;
                        claim(&part, &to)?;
                        Ok(stamped(&to))
                    }) {
                        Ok(()) => return Ok(to),
                        Err(Failed::Io(e)) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                        Err(e) => return Err(e.into_error(&name, "copy")),
                    }
                }
                Err(e) => return Err(e.into_error(&name, "copy")),
            }
        }
        Err(too_many(&name))
    }

    /// Writes a new file in `folder`. Returns where it was written.
    pub fn create_file(
        &mut self,
        folder: &Path,
        name: &str,
        contents: &str,
    ) -> Result<PathBuf, FileError> {
        check_name(name)?;
        let folder = self.grants.allow(folder)?;
        self.make_dirs(&folder)?;
        let (stem, ext) = split(name);
        for n in 1..=MAX_NUMBER {
            let path = folder.join(numbered(&stem, ext.as_deref(), n));
            if exists(&path) {
                continue;
            }
            let part = part_path(&folder, &path);
            let action = Action::Create {
                path: path.clone(),
                part: part.clone(),
            };
            match self.step(action, || {
                write_new(&part, contents.as_bytes())?;
                claim(&part, &path)?;
                Ok(stamped(&path))
            }) {
                Ok(()) => return Ok(path),
                Err(Failed::Io(e)) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into_error(name, "create")),
            }
        }
        Err(too_many(name))
    }

    /// Adds one row to a CSV file, starting it with `headers` if it's new.
    pub fn add_row(
        &mut self,
        csv: &Path,
        values: &[String],
        headers: Option<&[String]>,
    ) -> Result<(), FileError> {
        let name = file_name(csv);
        let folder = csv
            .parent()
            .ok_or_else(|| FileError(format!("{name} isn't a file path.")))?;
        let folder = self.grants.allow(folder)?;
        self.make_dirs(&folder)?;
        let path = folder.join(&name);
        // Someone may change the file between reading it and swapping in the
        // new one; if so, read it again.
        for _ in 0..3 {
            let before = match fs::symlink_metadata(&path) {
                Ok(meta) if meta.file_type().is_symlink() => {
                    return Err(FileError(format!(
                        "{name} is a link, so FolderFlow won't change it."
                    )))
                }
                Ok(meta) if !meta.is_file() => {
                    return Err(FileError(format!("{name} isn't a file.")))
                }
                Ok(_) => Some(Stamp::of(&path).map_err(|e| io_error(&name, "read", e))?),
                Err(e) if e.kind() == io::ErrorKind::NotFound => None,
                Err(e) => return Err(io_error(&name, "read", e)),
            };
            let old = match before {
                Some(_) => Some(fs::read(&path).map_err(|e| io_error(&name, "read", e))?),
                None => None,
            };
            let new = with_row(old.as_deref(), values, headers);
            let backup = match &old {
                Some(old) => Some(
                    self.back_up(old)
                        .map_err(|e| io_error(&name, "back up", e))?,
                ),
                None => None,
            };
            let part = part_path(&folder, &path);
            let action = Action::AddRow {
                path: path.clone(),
                part: part.clone(),
                backup,
                len: new.len() as u64,
            };
            let result = self.step(action, || {
                write_new(&part, &new)?;
                match before {
                    Some(stamp) => {
                        if Stamp::of(&path).ok() != Some(stamp) {
                            return Err(io::Error::new(
                                io::ErrorKind::Interrupted,
                                "changed while the row was being added",
                            ));
                        }
                        fs::rename(&part, &path)?;
                        sys::sync_dir(&folder)?;
                    }
                    None => claim(&part, &path)?,
                }
                Ok(stamped(&path))
            });
            match result {
                Ok(()) => return Ok(()),
                Err(Failed::Io(e))
                    if matches!(
                        e.kind(),
                        io::ErrorKind::Interrupted | io::ErrorKind::AlreadyExists
                    ) =>
                {
                    continue
                }
                Err(e) => return Err(e.into_error(&name, "add a row to")),
            }
        }
        Err(FileError(format!(
            "{name} kept changing while FolderFlow was adding a row, so it was left as it is."
        )))
    }

    /// Adds Finder tags the file doesn't have yet. Returns the ones added.
    pub fn tag(&mut self, file: &Path, tags: &[String]) -> Result<Vec<String>, FileError> {
        let (folder, name) = self.source(file)?;
        let path = folder.join(&name);
        let have = sys::tags(&path).map_err(|e| io_error(&name, "read the tags of", e))?;
        let mut add: Vec<String> = Vec::new();
        for tag in tags {
            let tag = tag.trim().replace('\n', " ");
            let known = |t: &String| t.eq_ignore_ascii_case(&tag);
            if !tag.is_empty() && !have.iter().any(known) && !add.iter().any(known) {
                add.push(tag);
            }
        }
        if add.is_empty() {
            return Ok(add);
        }
        let action = Action::Tag {
            path: path.clone(),
            add: add.clone(),
        };
        let added = add.clone();
        self.step_with(action, || {
            let added = sys::add_tags(&path, &added)?;
            Ok(Did { stamp: None, added })
        })
        .map_err(|e| e.into_error(&name, "tag"))?;
        Ok(add)
    }

    /// The folder (links resolved, granted) and name of an existing file.
    fn source(&self, file: &Path) -> Result<(PathBuf, String), FileError> {
        let name = file_name(file);
        let folder = file
            .parent()
            .ok_or_else(|| FileError(format!("{name} isn't a file path.")))?;
        let folder = self.grants.allow(folder)?;
        match fs::symlink_metadata(folder.join(&name)) {
            Ok(meta) if meta.is_file() => Ok((folder, name)),
            Ok(meta) if meta.file_type().is_symlink() => Err(FileError(format!(
                "{name} is a link, so FolderFlow won't change it."
            ))),
            Ok(_) => Err(FileError(format!("{name} isn't a file."))),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                Err(FileError(format!("{name} is no longer there.")))
            }
            Err(e) => Err(io_error(&name, "open", e)),
        }
    }

    /// Moves `from` to the first free name `stem[ n].ext` in `folder`.
    fn place(
        &mut self,
        from: &Path,
        folder: &Path,
        stem: &str,
        ext: Option<&str>,
    ) -> Result<PathBuf, FileError> {
        let name = file_name(from);
        let inode = Stamp::of(from)
            .map_err(|e| io_error(&name, "open", e))?
            .inode;
        for n in 1..=MAX_NUMBER {
            let to = folder.join(numbered(stem, ext, n));
            if to == from {
                return Ok(to);
            }
            match Stamp::of(&to) {
                // The same file under another case: a rename only of its case.
                Ok(there) if there.inode == inode => {
                    let action = Action::Rename {
                        from: from.to_path_buf(),
                        to: to.clone(),
                        inode,
                    };
                    return self
                        .step(action, || {
                            fs::rename(from, &to)?;
                            Ok(stamped(&to))
                        })
                        .map(|()| to)
                        .map_err(|e| e.into_error(&name, "rename"));
                }
                Ok(_) => continue,
                Err(_) => {}
            }
            let action = Action::Rename {
                from: from.to_path_buf(),
                to: to.clone(),
                inode,
            };
            match self.step(action, || {
                sys::rename_new(from, &to)?;
                Ok(stamped(&to))
            }) {
                Ok(()) => return Ok(to),
                Err(Failed::Io(e)) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(Failed::Io(e)) if e.raw_os_error() == Some(libc::EXDEV) => {
                    return self.move_across(from, &to).map(|()| to);
                }
                Err(e) => return Err(e.into_error(&name, "move")),
            }
        }
        Err(too_many(&name))
    }

    /// A move to another disk: copy, claim the name, check, then remove the
    /// original. The original stays until its copy is whole in place.
    fn move_across(&mut self, from: &Path, to: &Path) -> Result<(), FileError> {
        let name = file_name(from);
        let folder = to.parent().expect("a folder was joined");
        let size = Stamp::of(from)
            .map_err(|e| io_error(&name, "open", e))?
            .size;
        let part = part_path(folder, to);
        let action = Action::Move {
            from: from.to_path_buf(),
            to: to.to_path_buf(),
            part: part.clone(),
            size,
        };
        let seq = self
            .journal
            .intent(action)
            .map_err(|e| io_error(&name, "move", e))?;
        self.writes.writing(to);
        let placed = (|| {
            copy_bytes(from, &part)?;
            claim(&part, to)?;
            if Stamp::of(to)?.size != size {
                return Err(io::Error::other("the copy came out a different size"));
            }
            Ok(())
        })();
        if let Err(e) = placed {
            let _ = fs::remove_file(&part);
            self.writes.finished(to, false);
            let _ = self.journal.not_done(seq);
            return Err(io_error(&name, "move", e));
        }
        self.writes.finished(to, true);
        let finish = self
            .journal
            .placed(seq)
            .and_then(|()| fs::remove_file(from))
            .and_then(|()| self.journal.done(seq, Stamp::of(to).ok(), Vec::new()));
        finish.map_err(|e| io_error(&name, "move", e))
    }

    /// Creates each missing folder down to `folder`, writing each one down.
    fn make_dirs(&mut self, folder: &Path) -> Result<(), FileError> {
        let missing: Vec<&Path> = folder
            .ancestors()
            .take_while(|p| fs::symlink_metadata(p).is_err())
            .collect();
        for dir in missing.into_iter().rev() {
            let seq = self
                .journal
                .intent(Action::MakeDir {
                    path: dir.to_path_buf(),
                })
                .map_err(|e| io_error(&file_name(dir), "create", e))?;
            match fs::create_dir(dir) {
                Ok(()) => self.journal.done(seq, None, Vec::new()),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => self.journal.not_done(seq),
                Err(e) => {
                    let _ = self.journal.not_done(seq);
                    return Err(io_error(&file_name(dir), "create the folder", e));
                }
            }
            .map_err(|e| io_error(&file_name(dir), "create", e))?;
        }
        Ok(())
    }

    fn back_up(&self, bytes: &[u8]) -> io::Result<PathBuf> {
        fs::create_dir_all(&self.backups)?;
        let path = self.backups.join(format!("{}.csv", uuid::Uuid::new_v4()));
        write_new(&path, bytes)?;
        Ok(path)
    }

    /// Writes the intent, does the action, and writes done, or not-done if it
    /// failed. Temporary files of a failed action are removed.
    fn step(
        &mut self,
        action: Action,
        act: impl FnOnce() -> io::Result<Option<Stamp>>,
    ) -> Result<(), Failed> {
        self.step_with(action, || {
            Ok(Did {
                stamp: act()?,
                added: Vec::new(),
            })
        })
    }

    fn step_with(
        &mut self,
        action: Action,
        act: impl FnOnce() -> io::Result<Did>,
    ) -> Result<(), Failed> {
        let part = match &action {
            Action::Copy { part, .. } => part.clone(),
            Action::Create { part, .. } | Action::AddRow { part, .. } => Some(part.clone()),
            _ => None,
        };
        let target = action.target().map(Path::to_path_buf);
        let seq = self.journal.intent(action).map_err(Failed::Io)?;
        if self.crash == Some(CrashPoint::AfterIntent) {
            return Err(Failed::Crashed);
        }
        if let Some(target) = &target {
            self.writes.writing(target);
        }
        let result = act();
        if result.is_ok() && self.crash == Some(CrashPoint::BeforeDone) {
            return Err(Failed::Crashed);
        }
        if let Some(target) = &target {
            self.writes.finished(target, result.is_ok());
        }
        match result {
            Ok(did) => self
                .journal
                .done(seq, did.stamp, did.added)
                .map_err(Failed::Io),
            Err(e) => {
                if let Some(part) = part {
                    let _ = fs::remove_file(part);
                }
                let _ = self.journal.not_done(seq);
                Err(Failed::Io(e))
            }
        }
    }
}

enum Failed {
    Io(io::Error),
    /// A test's crash point was reached.
    Crashed,
}

impl Failed {
    fn into_error(self, name: &str, verb: &str) -> FileError {
        match self {
            Failed::Io(e) => io_error(name, verb, e),
            Failed::Crashed => FileError("Stopped as if FolderFlow had quit.".into()),
        }
    }
}

fn io_error(name: &str, verb: &str, e: io::Error) -> FileError {
    if e.kind() == io::ErrorKind::PermissionDenied {
        return FileError(format!(
            "FolderFlow isn't allowed to {verb} {name}. If it's in Desktop, Documents or Downloads, allow FolderFlow in System Settings › Privacy & Security › Files and Folders."
        ));
    }
    FileError(format!("Couldn't {verb} {name}: {e}."))
}

fn too_many(name: &str) -> FileError {
    FileError(format!(
        "There are already {MAX_NUMBER} files named like {name}, so FolderFlow stopped numbering them."
    ))
}

/// A name for a file: not empty, no `/` or `:`, and not too long for macOS.
fn check_name(name: &str) -> Result<(), FileError> {
    if name.trim().is_empty() || name == "." || name == ".." {
        return Err(FileError("The new name is empty.".into()));
    }
    if name.contains(['/', ':']) {
        return Err(FileError("A file name can't contain / or :.".into()));
    }
    if name.len() > 240 {
        return Err(FileError("That name is too long for a file.".into()));
    }
    Ok(())
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// The extension after the last dot, unless the dot starts the name.
fn extension(name: &str) -> Option<String> {
    split(name).1
}

fn split(name: &str) -> (String, Option<String>) {
    match name.rfind('.') {
        Some(dot) if dot > 0 => (name[..dot].to_owned(), Some(name[dot + 1..].to_owned())),
        _ => (name.to_owned(), None),
    }
}

/// `stem.ext`, or `stem n.ext` from 2 on.
fn numbered(stem: &str, ext: Option<&str>, n: u32) -> String {
    let stem = if n == 1 {
        stem.to_owned()
    } else {
        format!("{stem} {n}")
    };
    match ext {
        Some(ext) => format!("{stem}.{ext}"),
        None => stem,
    }
}

fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn same_folder(a: &Path, b: &Path) -> bool {
    within(a, b, false, sys::ignores_case(b))
}

fn stamped(path: &Path) -> Option<Stamp> {
    Stamp::of(path).ok()
}

/// A hidden temporary name beside `target`.
fn part_path(folder: &Path, target: &Path) -> PathBuf {
    folder.join(format!(
        ".{}.ffpart-{}",
        file_name(target),
        uuid::Uuid::new_v4()
    ))
}

/// Writes a file that mustn't exist yet, flushed to disk.
fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn copy_bytes(from: &Path, to: &Path) -> io::Result<()> {
    let mut src = fs::File::open(from)?;
    let mut dst = OpenOptions::new().write(true).create_new(true).open(to)?;
    io::copy(&mut src, &mut dst)?;
    dst.set_permissions(src.metadata()?.permissions())?;
    dst.sync_all()
}

/// Renames the finished temporary file to its name, which mustn't be taken.
fn claim(part: &Path, to: &Path) -> io::Result<()> {
    sys::rename_new(part, to)?;
    sys::sync_dir(to.parent().expect("a folder was joined"))
}

/// The CSV with one more row. A new file starts with the headings.
fn with_row(old: Option<&[u8]>, values: &[String], headers: Option<&[String]>) -> Vec<u8> {
    let crlf = old.is_some_and(|o| o.windows(2).any(|w| w == b"\r\n"));
    let ending: &[u8] = if crlf { b"\r\n" } else { b"\n" };
    let mut out = old.map(<[u8]>::to_vec).unwrap_or_default();
    if !out.is_empty() && !out.ends_with(b"\n") {
        out.extend_from_slice(ending);
    }
    let mut line = |cells: &[String]| {
        let row: Vec<String> = cells.iter().map(|c| csv_cell(c)).collect();
        out.extend_from_slice(row.join(",").as_bytes());
        out.extend_from_slice(ending);
    };
    if old.is_none() {
        if let Some(headers) = headers {
            line(headers);
        }
    }
    line(values);
    out
}

/// One CSV cell. A value a spreadsheet would run as a formula starts with `'`,
/// unless it's a plain number like -12.50.
fn csv_cell(value: &str) -> String {
    let formula = value.starts_with(['=', '+', '-', '@', '\t', '\r'])
        && value
            .trim()
            .parse::<f64>()
            .map_or(true, |n| !n.is_finite() || value.contains(['e', 'E']));
    let value = if formula {
        format!("'{value}")
    } else {
        value.to_owned()
    };
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_go_before_the_extension() {
        assert_eq!(numbered("Receipt", Some("pdf"), 1), "Receipt.pdf");
        assert_eq!(numbered("Receipt", Some("pdf"), 3), "Receipt 3.pdf");
        assert_eq!(numbered("README", None, 2), "README 2");
        assert_eq!(split(".hidden"), (".hidden".to_string(), None));
        assert_eq!(
            split("a.tar.gz"),
            ("a.tar".to_string(), Some("gz".to_string()))
        );
    }

    #[test]
    fn only_plain_numbers_may_start_with_a_sign() {
        assert_eq!(csv_cell("-12.50"), "-12.50");
        assert_eq!(csv_cell("+3"), "+3");
        assert_eq!(csv_cell("-1e3"), "'-1e3");
        assert_eq!(csv_cell("-inf"), "'-inf");
        assert_eq!(csv_cell("=1+1"), "'=1+1");
        assert_eq!(csv_cell("\tx"), "'\tx");
    }
}
