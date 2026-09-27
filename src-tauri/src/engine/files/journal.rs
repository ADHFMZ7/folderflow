//! The journal: `<run id>.jsonl`, one line per event. Each action writes an
//! intent line before it touches the disk and a done line after, flushed to
//! disk each time, so after a crash every action can be finished or known not
//! to have happened, and every finished one can be undone.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::sys::Stamp;

/// What an action is about to do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Action {
    /// A folder created on the way to a Move, Copy or Create file.
    MakeDir { path: PathBuf },
    /// A rename in place, or a move on the same disk: one atomic rename.
    Rename {
        from: PathBuf,
        to: PathBuf,
        inode: u64,
    },
    /// A move to another disk: copied to `part`, renamed to `to`, then the
    /// original removed.
    Move {
        from: PathBuf,
        to: PathBuf,
        part: PathBuf,
        size: u64,
    },
    /// A copy, cloned straight to `to`, or through `part` where cloning can't.
    Copy {
        from: PathBuf,
        to: PathBuf,
        part: Option<PathBuf>,
    },
    /// A new file written to `part`, then renamed to `path`.
    Create { path: PathBuf, part: PathBuf },
    /// A spreadsheet rewritten with one more row: written to `part`, then
    /// swapped in. `backup` holds the file as it was; `None` for a new file.
    AddRow {
        path: PathBuf,
        part: PathBuf,
        backup: Option<PathBuf>,
        /// The file's length once the row is in.
        len: u64,
    },
    /// Finder tags the file didn't have.
    Tag { path: PathBuf, add: Vec<String> },
}

impl Action {
    /// The file the action puts somewhere, if it puts one.
    pub fn target(&self) -> Option<&Path> {
        match self {
            Action::Rename { to, .. } | Action::Move { to, .. } | Action::Copy { to, .. } => {
                Some(to)
            }
            Action::Create { path, .. } | Action::AddRow { path, .. } => Some(path),
            Action::MakeDir { .. } | Action::Tag { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "camelCase")]
enum Line {
    Intent {
        seq: u32,
        action: Action,
    },
    /// The action happened. `stamp` is the file it left, to spot later changes.
    Done {
        seq: u32,
        stamp: Option<Stamp>,
        #[serde(default)]
        added: Vec<String>,
    },
    /// A move to another disk has its copy in place; only removing the
    /// original is left.
    Placed {
        seq: u32,
    },
    /// The action didn't happen, or was found not to have after a crash.
    NotDone {
        seq: u32,
    },
    Undone {
        seq: u32,
    },
    LeftAlone {
        seq: u32,
        reason: String,
    },
}

/// Where an action stands.
#[derive(Debug, Clone, PartialEq)]
pub enum State {
    /// Written down but not known to be done: a crash came in between.
    Pending {
        placed: bool,
    },
    Done {
        stamp: Option<Stamp>,
        added: Vec<String>,
    },
    NotDone,
    Undone,
    LeftAlone,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub seq: u32,
    pub action: Action,
    pub state: State,
}

/// One run's journal, open for writing.
pub struct Journal {
    file: File,
    next: u32,
}

pub fn path(dir: &Path, run: &str) -> PathBuf {
    dir.join(format!("{run}.jsonl"))
}

/// Where a run keeps copies of spreadsheets as they were before a row.
pub fn backups(dir: &Path, run: &str) -> PathBuf {
    dir.join(format!("{run}.backups"))
}

impl Journal {
    pub fn open(dir: &Path, run: &str) -> io::Result<Journal> {
        fs::create_dir_all(dir)?;
        let path = path(dir, run);
        let next = read(&path)?.iter().map(|e| e.seq).max().unwrap_or(0) + 1;
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(Journal { file, next })
    }

    pub fn intent(&mut self, action: Action) -> io::Result<u32> {
        let seq = self.next;
        self.next += 1;
        self.write(&Line::Intent { seq, action })?;
        Ok(seq)
    }

    pub fn done(&mut self, seq: u32, stamp: Option<Stamp>, added: Vec<String>) -> io::Result<()> {
        self.write(&Line::Done { seq, stamp, added })
    }

    pub fn placed(&mut self, seq: u32) -> io::Result<()> {
        self.write(&Line::Placed { seq })
    }

    pub fn not_done(&mut self, seq: u32) -> io::Result<()> {
        self.write(&Line::NotDone { seq })
    }

    pub fn undone(&mut self, seq: u32) -> io::Result<()> {
        self.write(&Line::Undone { seq })
    }

    pub fn left_alone(&mut self, seq: u32, reason: &str) -> io::Result<()> {
        self.write(&Line::LeftAlone {
            seq,
            reason: reason.to_owned(),
        })
    }

    fn write(&mut self, line: &Line) -> io::Result<()> {
        let mut bytes = serde_json::to_vec(line).map_err(io::Error::other)?;
        bytes.push(b'\n');
        self.file.write_all(&bytes)?;
        self.file.sync_data()
    }
}

/// Every action in the journal at `path`, in order. A missing journal is empty;
/// a line cut short by a crash is skipped.
pub fn read(path: &Path) -> io::Result<Vec<Entry>> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut entries: Vec<Entry> = Vec::new();
    for line in text.lines() {
        let Ok(line) = serde_json::from_str::<Line>(line) else {
            continue;
        };
        let (seq, state) = match line {
            Line::Intent { seq, action } => {
                entries.push(Entry {
                    seq,
                    action,
                    state: State::Pending { placed: false },
                });
                continue;
            }
            Line::Done { seq, stamp, added } => (seq, State::Done { stamp, added }),
            Line::Placed { seq } => (seq, State::Pending { placed: true }),
            Line::NotDone { seq } => (seq, State::NotDone),
            Line::Undone { seq } => (seq, State::Undone),
            Line::LeftAlone { seq, .. } => (seq, State::LeftAlone),
        };
        if let Some(entry) = entries.iter_mut().find(|e| e.seq == seq) {
            entry.state = state;
        }
    }
    Ok(entries)
}
