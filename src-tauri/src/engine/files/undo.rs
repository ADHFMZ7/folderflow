//! Undo and crash recovery, both read from a run's journal.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;
use ts_rs::TS;

use super::journal::{self, Action, Entry, Journal, State};
use super::sys::{self, Stamp};
use super::{claim, copy_bytes, file_name, numbered, part_path, split, Trash, Writes, MAX_NUMBER};

/// What undoing a run did.
#[derive(Debug, Clone, Default, PartialEq, Serialize, serde::Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UndoReport {
    /// How many actions were reversed.
    pub restored: u32,
    /// The ones that weren't, and why.
    pub left_alone: Vec<LeftAlone>,
}

#[derive(Debug, Clone, PartialEq, Serialize, serde::Deserialize, TS)]
#[ts(export)]
pub struct LeftAlone {
    #[ts(type = "string")]
    pub path: PathBuf,
    pub reason: String,
}

/// Reverses a run's actions, newest first. A file changed since the run (by
/// inode, size or modification time) is left as it is and listed. Files the
/// run made go to the Trash. Undoing again reverses nothing more. `writes`
/// hears of each file put back.
pub fn undo(
    journal_dir: &Path,
    run: &str,
    trash: &dyn Trash,
    writes: &dyn Writes,
) -> io::Result<UndoReport> {
    let entries = journal::read(&journal::path(journal_dir, run))?;
    let mut journal = Journal::open(journal_dir, run)?;
    let mut report = UndoReport::default();
    // Files undo itself rewrote or moved back. Each matches the state an
    // earlier action of the run left, so it still counts as unchanged.
    let mut ours: HashMap<PathBuf, Stamp> = HashMap::new();
    for entry in entries.iter().rev() {
        let State::Done { stamp, added } = &entry.state else {
            continue;
        };
        match reverse(&entry.action, *stamp, added, trash, writes, &mut ours) {
            Ok(Reversed::Counted) => {
                report.restored += 1;
                journal.undone(entry.seq)?;
            }
            Ok(Reversed::Quietly) => journal.undone(entry.seq)?,
            Err((path, reason)) => {
                journal.left_alone(entry.seq, &reason)?;
                report.left_alone.push(LeftAlone { path, reason });
            }
        }
    }
    Ok(report)
}

enum Reversed {
    Counted,
    /// A folder removed, or nothing left to do: not counted as an action.
    Quietly,
}

type Refusal = (PathBuf, String);

fn reverse(
    action: &Action,
    stamp: Option<Stamp>,
    added: &[String],
    trash: &dyn Trash,
    writes: &dyn Writes,
    ours: &mut HashMap<PathBuf, Stamp>,
) -> Result<Reversed, Refusal> {
    let unchanged = |path: &Path| {
        let now = Stamp::of(path).ok();
        stamp.is_some() && (now == stamp || now.is_some() && now.as_ref() == ours.get(path))
    };
    match action {
        Action::MakeDir { path } => {
            // Only if empty: anything put there since stays, and so does the folder.
            let _ = fs::remove_dir(path);
            Ok(Reversed::Quietly)
        }
        Action::Rename { from, to, .. } | Action::Move { from, to, .. } => {
            let name = file_name(from);
            if !unchanged(to) {
                let why = if fs::symlink_metadata(to).is_ok() {
                    "was changed after this run"
                } else {
                    "is no longer where this run left it"
                };
                return Err((
                    to.clone(),
                    format!("{name} {why}, so it wasn't moved back."),
                ));
            }
            let back = move_back(to, from, writes)
                .map_err(|e| (to.clone(), format!("{name} couldn't be moved back: {e}.")))?;
            remember(ours, back);
            Ok(Reversed::Counted)
        }
        Action::Copy { to: path, .. } | Action::Create { path, .. } => {
            to_trash(path, unchanged(path), trash)
        }
        Action::AddRow { path, backup, .. } => {
            let name = file_name(path);
            if !unchanged(path) {
                return Err((
                    path.clone(),
                    format!("{name} was changed after this run, so the row this run added is still in it."),
                ));
            }
            match backup {
                None => to_trash(path, true, trash),
                Some(backup) => {
                    writes.writing(path);
                    let restored = restore(backup, path);
                    writes.finished(path, restored.is_ok());
                    restored.map_err(|e| {
                        (
                            path.clone(),
                            format!("The row couldn't be taken out of {name}: {e}."),
                        )
                    })?;
                    remember(ours, path.clone());
                    Ok(Reversed::Counted)
                }
            }
        }
        Action::Tag { path, .. } => match sys::remove_tags(path, added) {
            Ok(()) => Ok(Reversed::Counted),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Reversed::Quietly),
            Err(e) => Err((
                path.clone(),
                format!("The tags couldn't be taken off {}: {e}.", file_name(path)),
            )),
        },
    }
}

fn to_trash(path: &Path, unchanged: bool, trash: &dyn Trash) -> Result<Reversed, Refusal> {
    let name = file_name(path);
    if fs::symlink_metadata(path).is_err() {
        return Ok(Reversed::Quietly);
    }
    if !unchanged {
        return Err((
            path.to_path_buf(),
            format!("{name} was changed after this run, so it wasn't moved to the Trash."),
        ));
    }
    trash.trash(path).map_err(|e| {
        (
            path.to_path_buf(),
            format!("{name} couldn't be moved to the Trash: {e}."),
        )
    })?;
    Ok(Reversed::Counted)
}

fn remember(ours: &mut HashMap<PathBuf, Stamp>, path: PathBuf) {
    if let Ok(now) = Stamp::of(&path) {
        ours.insert(path, now);
    }
}

/// Moves `from` back to `to`, or to `to` numbered if that name is taken now.
/// Returns where it went.
fn move_back(from: &Path, to: &Path, writes: &dyn Writes) -> io::Result<PathBuf> {
    let folder = to.parent().expect("a file path");
    fs::create_dir_all(folder)?;
    let (stem, ext) = split(&file_name(to));
    for n in 1..=MAX_NUMBER {
        let target = folder.join(numbered(&stem, ext.as_deref(), n));
        writes.writing(&target);
        let moved = move_to(from, &target, folder);
        writes.finished(&target, moved.is_ok());
        match moved {
            Ok(()) => return Ok(target),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::other("every numbered name is taken"))
}

/// One try at moving `from` to `target`, which mustn't be taken.
fn move_to(from: &Path, target: &Path, folder: &Path) -> io::Result<()> {
    match sys::rename_new(from, target) {
        Err(e) if e.raw_os_error() == Some(libc::EXDEV) => {
            let part = part_path(folder, target);
            copy_bytes(from, &part)?;
            match claim(&part, target) {
                Ok(()) => fs::remove_file(from),
                Err(e) => {
                    let _ = fs::remove_file(&part);
                    Err(e)
                }
            }
        }
        other => other,
    }
}

/// Puts a spreadsheet back as it was, in one swap.
fn restore(backup: &Path, path: &Path) -> io::Result<()> {
    let folder = path.parent().expect("a file path");
    let part = part_path(folder, path);
    fs::copy(backup, &part)?;
    fs::File::open(&part)?.sync_all()?;
    fs::rename(&part, path)?;
    sys::sync_dir(folder)
}

/// What `recover_all` settled.
#[derive(Debug, Default)]
pub struct Recovered {
    /// The runs that had actions a crash left half-known; each was cut off mid-run.
    pub runs: Vec<String>,
    /// Where those actions put, or may have put, a file.
    pub touched: Vec<PathBuf>,
}

/// Settles every action a crash left half-known: done if the disk shows it
/// happened, not done if it didn't, and temporary files removed. A move to
/// another disk whose copy was in place is finished. Returns the runs that
/// had such actions.
pub fn recover(journal_dir: &Path) -> io::Result<Vec<String>> {
    recover_all(journal_dir).map(|r| r.runs)
}

/// `recover`, also saying where the settled actions put files.
pub fn recover_all(journal_dir: &Path) -> io::Result<Recovered> {
    let mut out = Recovered::default();
    let listed = match fs::read_dir(journal_dir) {
        Ok(listed) => listed,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e),
    };
    for item in listed {
        let path = item?.path();
        if path.extension().is_none_or(|e| e != "jsonl") {
            continue;
        }
        let run = path
            .file_stem()
            .expect("has a stem")
            .to_string_lossy()
            .into_owned();
        let pending: Vec<Entry> = journal::read(&path)?
            .into_iter()
            .filter(|e| matches!(e.state, State::Pending { .. }))
            .collect();
        if pending.is_empty() {
            continue;
        }
        let mut journal = Journal::open(journal_dir, &run)?;
        for entry in pending {
            out.touched
                .extend(entry.action.target().map(Path::to_path_buf));
            let placed = matches!(entry.state, State::Pending { placed: true });
            match settle(&entry.action, placed) {
                Some((stamp, added)) => journal.done(entry.seq, stamp, added)?,
                None => journal.not_done(entry.seq)?,
            }
        }
        out.runs.push(run);
    }
    out.runs.sort();
    Ok(out)
}

/// Whether the action happened, judged from the disk, with what done records.
fn settle(action: &Action, placed: bool) -> Option<(Option<Stamp>, Vec<String>)> {
    let drop_part = |part: &Path| {
        let _ = fs::remove_file(part);
    };
    let there = |path: &Path| Stamp::of(path).ok();
    match action {
        Action::MakeDir { path } => path.is_dir().then(|| (None, Vec::new())),
        Action::Rename { to, inode, .. } => there(to)
            .filter(|s| s.inode == *inode)
            .map(|s| (Some(s), Vec::new())),
        Action::Move { from, to, part, .. } => {
            drop_part(part);
            if placed {
                // The copy is whole in place: finish by removing the original.
                let _ = fs::remove_file(from);
                return there(to).map(|s| (Some(s), Vec::new()));
            }
            None
        }
        Action::Copy { to, part, .. } => {
            if let Some(part) = part {
                drop_part(part);
            }
            there(to).map(|s| (Some(s), Vec::new()))
        }
        Action::Create { path, part } => {
            drop_part(part);
            there(path).map(|s| (Some(s), Vec::new()))
        }
        Action::AddRow {
            path, part, len, ..
        } => {
            drop_part(part);
            there(path)
                .filter(|s| s.size == *len)
                .map(|s| (Some(s), Vec::new()))
        }
        Action::Tag { path, add } => {
            let tags = sys::tags(path).ok()?;
            add.iter()
                .all(|a| tags.iter().any(|t| t.eq_ignore_ascii_case(a)))
                .then(|| (None, add.clone()))
        }
    }
}
