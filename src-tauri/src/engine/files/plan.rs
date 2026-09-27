//! File actions worked out, not done, for Try on a file. Each action is
//! checked as a real one is (granted folders, names, links) and numbered
//! against the disk as the earlier actions would have left it, but the disk
//! is only read. See docs/engine.md, "Try on a file".

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::{
    check_name, exists, extension, file_name, io_error, new_tags, numbered, resolve, same_folder,
    split, sys, too_many, Actions, FileError, Grants, MAX_NUMBER,
};

pub struct Plan {
    grants: Grants,
    /// Each path the plan put a file at, with the file on disk it stands for
    /// (`None` for a file it would create).
    placed: HashMap<PathBuf, Option<PathBuf>>,
    /// Paths on disk the plan moved a file away from.
    vacated: HashSet<PathBuf>,
    /// Folders the plan would create.
    folders: HashSet<PathBuf>,
    /// Tags the plan would add, by where the file would be.
    tags: HashMap<PathBuf, Vec<String>>,
}

impl Plan {
    pub fn new(grants: Grants) -> Plan {
        Plan {
            grants,
            placed: HashMap::new(),
            vacated: HashSet::new(),
            folders: HashSet::new(),
            tags: HashMap::new(),
        }
    }

    /// Whether something would be at `path` (links already resolved).
    fn there(&self, path: &Path) -> bool {
        self.placed.contains_key(path)
            || self.folders.contains(path)
            || (exists(path) && !self.vacated.contains(path))
    }

    /// The folder (links resolved, granted) and name of the file at `file`,
    /// and the file on disk it stands for, if any.
    fn source(&self, file: &Path) -> Result<(PathBuf, String, Option<PathBuf>), FileError> {
        let name = file_name(file);
        let folder = file
            .parent()
            .ok_or_else(|| FileError(format!("{name} isn't a file path.")))?;
        let folder = self.grants.allow(folder)?;
        let path = folder.join(&name);
        if let Some(real) = self.placed.get(&path) {
            return Ok((folder, name, real.clone()));
        }
        if self.vacated.contains(&path) {
            return Err(FileError(format!("{name} is no longer there.")));
        }
        match fs::symlink_metadata(&path) {
            Ok(meta) if meta.is_file() => Ok((folder, name, Some(path))),
            Ok(meta) if meta.file_type().is_symlink() => Err(FileError(format!(
                "{name} is a link, so Vela won't change it."
            ))),
            Ok(_) => Err(FileError(format!("{name} isn't a file."))),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                Err(FileError(format!("{name} is no longer there.")))
            }
            Err(e) => Err(io_error(&name, "open", e)),
        }
    }

    /// The file at `from` would move to the first free `stem[ n].ext` in `folder`.
    fn place(
        &mut self,
        from: &Path,
        real: Option<PathBuf>,
        folder: &Path,
        stem: &str,
        ext: Option<&str>,
    ) -> Result<PathBuf, FileError> {
        for n in 1..=MAX_NUMBER {
            let to = folder.join(numbered(stem, ext, n));
            if to == from {
                return Ok(to);
            }
            // The same file under another case: a rename only of its case.
            let same_file = real.is_some()
                && !self.placed.contains_key(&to)
                && sys::Stamp::of(&to).ok().map(|s| s.inode)
                    == real
                        .as_deref()
                        .and_then(|r| sys::Stamp::of(r).ok())
                        .map(|s| s.inode);
            if self.there(&to) && !same_file {
                continue;
            }
            if self.placed.remove(from).is_none() {
                self.vacated.insert(from.to_path_buf());
            }
            if let Some(tags) = self.tags.remove(from) {
                self.tags.insert(to.clone(), tags);
            }
            self.placed.insert(to.clone(), real);
            return Ok(to);
        }
        Err(too_many(&file_name(from)))
    }

    fn make_dirs(&mut self, folder: &Path) {
        let missing: Vec<PathBuf> = folder
            .ancestors()
            .take_while(|p| !self.there(p))
            .map(Path::to_path_buf)
            .collect();
        self.folders.extend(missing);
    }

    /// The first free `stem[ n].ext` in `folder`, taken for a new file.
    fn claim_new(&mut self, folder: &Path, name: &str) -> Result<PathBuf, FileError> {
        let (stem, ext) = split(name);
        for n in 1..=MAX_NUMBER {
            let path = folder.join(numbered(&stem, ext.as_deref(), n));
            if !self.there(&path) {
                self.placed.insert(path.clone(), None);
                return Ok(path);
            }
        }
        Err(too_many(name))
    }
}

impl Actions for Plan {
    fn rename(&mut self, file: &Path, new_stem: &str) -> Result<PathBuf, FileError> {
        let (folder, name, real) = self.source(file)?;
        check_name(new_stem)?;
        let ext = extension(&name);
        self.place(&folder.join(&name), real, &folder, new_stem, ext.as_deref())
    }

    fn move_file(&mut self, file: &Path, folder: &Path) -> Result<PathBuf, FileError> {
        let (from_folder, name, real) = self.source(file)?;
        let to_folder = self.grants.allow(folder)?;
        let from = from_folder.join(&name);
        if same_folder(&from_folder, &to_folder) {
            return Ok(from);
        }
        self.make_dirs(&to_folder);
        let (stem, ext) = split(&name);
        self.place(&from, real, &to_folder, &stem, ext.as_deref())
    }

    fn copy_file(&mut self, file: &Path, folder: &Path) -> Result<PathBuf, FileError> {
        let (_, name, _) = self.source(file)?;
        let to_folder = self.grants.allow(folder)?;
        self.make_dirs(&to_folder);
        self.claim_new(&to_folder, &name)
    }

    fn create_file(
        &mut self,
        folder: &Path,
        name: &str,
        _contents: &str,
    ) -> Result<PathBuf, FileError> {
        check_name(name)?;
        let folder = self.grants.allow(folder)?;
        self.make_dirs(&folder);
        self.claim_new(&folder, name)
    }

    fn add_row(
        &mut self,
        csv: &Path,
        _values: &[String],
        _headers: Option<&[String]>,
    ) -> Result<(), FileError> {
        let name = file_name(csv);
        let folder = csv
            .parent()
            .ok_or_else(|| FileError(format!("{name} isn't a file path.")))?;
        let folder = self.grants.allow(folder)?;
        let path = folder.join(&name);
        if self.placed.contains_key(&path) {
            return Ok(());
        }
        if !self.vacated.contains(&path) {
            match fs::symlink_metadata(&path) {
                Ok(meta) if meta.file_type().is_symlink() => {
                    return Err(FileError(format!(
                        "{name} is a link, so Vela won't change it."
                    )))
                }
                Ok(meta) if !meta.is_file() => {
                    return Err(FileError(format!("{name} isn't a file.")))
                }
                Ok(_) => return Ok(()),
                Err(e) if e.kind() != io::ErrorKind::NotFound => {
                    return Err(io_error(&name, "read", e))
                }
                Err(_) => {}
            }
        }
        self.make_dirs(&folder);
        self.placed.insert(path, None);
        Ok(())
    }

    fn tag(&mut self, file: &Path, tags: &[String]) -> Result<Vec<String>, FileError> {
        let (folder, name, real) = self.source(file)?;
        let path = folder.join(&name);
        let mut have = match &real {
            Some(real) => sys::tags(real).map_err(|e| io_error(&name, "read the tags of", e))?,
            None => Vec::new(),
        };
        have.extend(self.tags.get(&path).cloned().unwrap_or_default());
        let add = new_tags(&have, tags);
        self.tags.entry(path).or_default().extend(add.clone());
        Ok(add)
    }

    fn exists(&self, path: &Path) -> bool {
        // Links resolved in the folder, as the actions resolve them.
        let resolved = match (path.parent(), path.file_name()) {
            (Some(folder), Some(name)) => resolve(folder).map(|f| f.join(name)),
            _ => resolve(path),
        };
        self.there(&resolved.unwrap_or_else(|_| path.to_path_buf()))
    }
}
