//! Notices new files in the folders File added workflows watch. See
//! docs/engine.md, "File added".
//!
//! A folder change is only a hint to look: what counts is a scan compared
//! against each workflow's record of the files it has seen, by device and
//! inode, so a renamed file is still the same file. A file is recorded before
//! its run is queued, so it runs at most once. Files Vela puts in a
//! watched folder are recorded as they land (`Writes`) and start nothing.

use std::collections::{HashMap, HashSet};
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::os::macos::fs::MetadataExt as _;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};

use super::files::Writes;
use super::runs::RunFile;

/// What a File added workflow watches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Watch {
    /// With links resolved.
    pub folder: PathBuf,
    pub subfolders: bool,
    /// Lowercase extensions; empty means any.
    pub file_types: Vec<String>,
}

impl Watch {
    /// Whether a file at `path` is in what this watches.
    fn holds(&self, path: &Path) -> bool {
        match path.parent() {
            Some(parent) if self.subfolders => parent.starts_with(&self.folder),
            Some(parent) => parent == self.folder,
            None => false,
        }
    }

    /// Whether a change at `path` may have touched what this watches: the
    /// folder, something in it, or a folder it's in.
    fn touched_by(&self, path: &Path) -> bool {
        path.starts_with(&self.folder) || self.folder.starts_with(path)
    }
}

/// One line of a workflow's record, `engine/seen/<workflow id>.jsonl`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Line {
    /// From here on the workflow watched this; `None` when it was turned off.
    Watch(Option<Watch>),
    Seen(Seen),
}

#[derive(Debug, Serialize, Deserialize)]
struct Seen {
    device: u64,
    inode: u64,
    size: u64,
    /// Nanoseconds since 1970.
    modified: i64,
    path: PathBuf,
}

type Key = (u64, u64);

fn key(meta: &Metadata) -> Key {
    (meta.dev(), meta.ino())
}

pub struct Intake {
    dir: PathBuf,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    on: HashMap<String, Watching>,
    /// Paths Vela is putting a file at right now, with how many times.
    writing: HashMap<PathBuf, u32>,
}

struct Watching {
    watch: Watch,
    seen: HashSet<Key>,
    record: File,
}

impl Intake {
    /// Records are kept in `dir`.
    pub fn new(dir: PathBuf) -> Intake {
        Intake {
            dir,
            state: Mutex::default(),
        }
    }

    /// Starts watching for a workflow. If it's newly on, or watches something
    /// new, every file there now is recorded as seen and won't run (decision
    /// 2); returns true. Otherwise it carries on from its record, and files
    /// that arrived since are left for `take`.
    pub fn turn_on(&self, id: &str, watch: Watch) -> io::Result<bool> {
        let mut state = self.lock();
        if state.on.get(id).is_some_and(|w| w.watch == watch) {
            return Ok(false);
        }
        let path = self.record_path(id);
        let (last, mut seen) = read_record(&path)?;
        fs::create_dir_all(&self.dir)?;
        let mut record = OpenOptions::new().create(true).append(true).open(&path)?;
        let fresh = last.as_ref() != Some(&watch);
        if fresh {
            let mut lines = vec![Line::Watch(Some(watch.clone()))];
            for (path, meta) in candidates(&watch) {
                if seen.insert(key(&meta)) {
                    lines.push(seen_line(path, &meta));
                }
            }
            append(&mut record, &lines)?;
        }
        state.on.insert(
            id.to_owned(),
            Watching {
                watch,
                seen,
                record,
            },
        );
        Ok(fresh)
    }

    /// Stops watching for a workflow. Its record is kept, so turning it on
    /// again doesn't run what it already ran.
    pub fn turn_off(&self, id: &str) -> io::Result<()> {
        if let Some(mut watching) = self.lock().on.remove(id) {
            append(&mut watching.record, &[Line::Watch(None)])?;
        }
        Ok(())
    }

    /// Stops watching for a deleted workflow and removes its record.
    pub fn forget(&self, id: &str) -> io::Result<()> {
        self.lock().on.remove(id);
        match fs::remove_file(self.record_path(id)) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    pub fn is_on(&self, id: &str) -> bool {
        self.lock().on.contains_key(id)
    }

    /// Every folder watched, with whether its subfolders are too.
    pub fn folders(&self) -> Vec<(PathBuf, bool)> {
        let mut out: Vec<(PathBuf, bool)> = self
            .lock()
            .on
            .values()
            .map(|w| (w.watch.folder.clone(), w.watch.subfolders))
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// The workflows whose folders a change at any of `paths` may have touched.
    pub fn touched_by(&self, paths: &[PathBuf]) -> Vec<String> {
        let mut ids: Vec<String> = self
            .lock()
            .on
            .iter()
            .filter(|(_, w)| paths.iter().any(|p| w.watch.touched_by(p)))
            .map(|(id, _)| id.clone())
            .collect();
        ids.sort();
        ids
    }

    /// The workflow's new files, oldest first, each recorded as seen so it's
    /// never taken again. Files Vela is writing are passed over, and so
    /// are files not ready yet (`ready`); none of those is recorded, so a
    /// later look takes them.
    pub fn take(&self, id: &str) -> io::Result<Vec<RunFile>> {
        let mut state = self.lock();
        let State { on, writing } = &mut *state;
        let Some(watching) = on.get_mut(id) else {
            return Ok(Vec::new());
        };
        let mut new: Vec<(PathBuf, Metadata)> = candidates(&watching.watch)
            .into_iter()
            .filter(|(path, meta)| {
                !writing.contains_key(path)
                    && !watching.seen.contains(&key(meta))
                    && ready(path, meta)
            })
            .collect();
        new.sort_by_key(|(path, meta)| (meta.ctime(), meta.ctime_nsec(), path.clone()));
        let mut lines = Vec::new();
        for (path, meta) in &new {
            watching.seen.insert(key(meta));
            lines.push(seen_line(path.clone(), meta));
        }
        append(&mut watching.record, &lines)?;
        Ok(new
            .into_iter()
            .map(|(path, meta)| RunFile {
                path,
                inode: meta.ino(),
            })
            .collect())
    }

    /// Records the file at `path` as seen by every workflow watching where it
    /// is: Vela put it there.
    pub fn ours(&self, path: &Path) {
        let mut state = self.lock();
        ours(&mut state, path);
    }

    fn record_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.jsonl"))
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn ours(state: &mut State, path: &Path) {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return;
    };
    for watching in state.on.values_mut() {
        if watching.watch.holds(path) && watching.seen.insert(key(&meta)) {
            let line = seen_line(path.to_path_buf(), &meta);
            if let Err(e) = append(&mut watching.record, &[line]) {
                eprintln!("vela: couldn't record {}: {e}", path.display());
            }
        }
    }
}

impl Writes for Intake {
    fn writing(&self, path: &Path) {
        *self.lock().writing.entry(path.to_path_buf()).or_default() += 1;
    }

    fn finished(&self, path: &Path, written: bool) {
        let mut state = self.lock();
        // Recorded before it stops counting as being written, under one lock,
        // so a scan never sees it as neither.
        if written {
            ours(&mut state, path);
        }
        if let Some(count) = state.writing.get_mut(path) {
            *count -= 1;
            if *count == 0 {
                state.writing.remove(path);
            }
        }
    }
}

/// The last watch in a record, and every file it has seen. Damaged lines,
/// from a crash mid-write, are skipped.
fn read_record(path: &Path) -> io::Result<(Option<Watch>, HashSet<Key>)> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok((None, HashSet::new())),
        Err(e) => return Err(e),
    };
    let mut last = None;
    let mut seen = HashSet::new();
    for line in BufReader::new(file).lines() {
        match serde_json::from_str(&line?) {
            Ok(Line::Watch(watch)) => last = watch,
            Ok(Line::Seen(s)) => {
                seen.insert((s.device, s.inode));
            }
            Err(_) => {}
        }
    }
    Ok((last, seen))
}

fn seen_line(path: PathBuf, meta: &Metadata) -> Line {
    Line::Seen(Seen {
        device: meta.dev(),
        inode: meta.ino(),
        size: meta.size(),
        modified: meta.mtime() * 1_000_000_000 + meta.mtime_nsec(),
        path,
    })
}

fn append(record: &mut File, lines: &[Line]) -> io::Result<()> {
    if lines.is_empty() {
        return Ok(());
    }
    let mut out = Vec::new();
    for line in lines {
        serde_json::to_writer(&mut out, line).map_err(io::Error::other)?;
        out.push(b'\n');
    }
    record.write_all(&out)?;
    record.sync_data()
}

/// Whether a new file is ready to run. An empty file isn't: Firefox puts an
/// empty placeholder under the final name while it downloads, and some apps
/// create a file before writing it. Nor is a file with its download still
/// going beside it (`Invoice.pdf.part` next to `Invoice.pdf`).
fn ready(path: &Path, meta: &Metadata) -> bool {
    if meta.len() == 0 {
        return false;
    }
    let Some(name) = path.file_name() else {
        return true;
    };
    !["part", "crdownload"].iter().any(|ext| {
        let mut beside = name.to_os_string();
        beside.push(format!(".{ext}"));
        fs::symlink_metadata(path.with_file_name(beside)).is_ok()
    })
}

/// Names of files still being downloaded.
const PARTIAL: [&str; 4] = ["download", "crdownload", "part", "tmp"];

/// Folders that Finder shows as one file.
const PACKAGES: [&str; 16] = [
    "app",
    "bundle",
    "pages",
    "numbers",
    "key",
    "rtfd",
    "photoslibrary",
    "musiclibrary",
    "imovielibrary",
    "fcpbundle",
    "logicx",
    "band",
    "framework",
    "plugin",
    "xcodeproj",
    "download",
];

/// Hidden in Finder.
const UF_HIDDEN: u32 = 0x8000;
/// In iCloud Drive but not downloaded.
const SF_DATALESS: u32 = 0x4000_0000;

/// The files in what `watch` watches that a workflow may take.
fn candidates(watch: &Watch) -> Vec<(PathBuf, Metadata)> {
    let mut out = Vec::new();
    walk(&watch.folder, watch, &mut out);
    out
}

fn walk(folder: &Path, watch: &Watch, out: &mut Vec<(PathBuf, Metadata)>) {
    let Ok(listed) = fs::read_dir(folder) else {
        return;
    };
    for entry in listed.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if name.starts_with('.') || meta.st_flags() & UF_HIDDEN != 0 {
            continue;
        }
        let ext = extension(&name);
        if meta.is_dir() {
            let package = ext.as_deref().is_some_and(|e| PACKAGES.contains(&e));
            if watch.subfolders && !package {
                walk(&entry.path(), watch, out);
            }
            continue;
        }
        // Links (DirEntry::metadata doesn't follow them), and anything else
        // that isn't a plain file.
        if !meta.is_file() {
            continue;
        }
        let partial = ext.as_deref().is_some_and(|e| PARTIAL.contains(&e));
        let wanted = watch.file_types.is_empty()
            || ext.as_ref().is_some_and(|e| watch.file_types.contains(e));
        if partial || !wanted || name.starts_with("~$") || meta.st_flags() & SF_DATALESS != 0 {
            continue;
        }
        out.push((entry.path(), meta));
    }
}

/// The lowercase extension, unless the only dot starts the name.
fn extension(name: &str) -> Option<String> {
    match name.rfind('.') {
        Some(dot) if dot > 0 => Some(name[dot + 1..].to_lowercase()),
        _ => None,
    }
}
