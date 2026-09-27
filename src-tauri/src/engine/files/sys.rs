//! The few macOS calls file actions need that std doesn't offer: renames and
//! clones that refuse to replace anything, Finder tags, and case sensitivity.

use std::ffi::CString;
use std::fs::{self, File};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

fn c_path(path: &Path) -> io::Result<CString> {
    CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path holds a NUL byte"))
}

fn check(result: libc::c_int) -> io::Result<()> {
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Renames `from` to `to`, failing with `AlreadyExists` if `to` exists. The
/// check and the rename are one step, so nothing can slip in between.
pub fn rename_new(from: &Path, to: &Path) -> io::Result<()> {
    let (from, to) = (c_path(from)?, c_path(to)?);
    check(unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) })
}

/// A copy of `from` at `to` that shares its blocks until either changes (APFS),
/// failing with `AlreadyExists` if `to` exists. Fails with `Unsupported` or a
/// cross-device error where the disk can't clone.
pub fn clone_new(from: &Path, to: &Path) -> io::Result<()> {
    let (from, to) = (c_path(from)?, c_path(to)?);
    check(unsafe { libc::clonefile(from.as_ptr(), to.as_ptr(), 0) })
}

/// Whether names in `dir` ignore case (the APFS default).
pub fn ignores_case(dir: &Path) -> bool {
    let Ok(dir) = c_path(dir) else { return true };
    unsafe { libc::pathconf(dir.as_ptr(), libc::_PC_CASE_SENSITIVE) == 0 }
}

/// Flushes a folder, so a rename or new file in it survives a power cut.
pub fn sync_dir(dir: &Path) -> io::Result<()> {
    File::open(dir)?.sync_all()
}

/// What a file was when FolderFlow last touched it. A file that no longer
/// matches has been changed by someone else since, and is left alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stamp {
    pub inode: u64,
    pub size: u64,
    /// Nanoseconds since 1970.
    pub modified: i64,
}

impl Stamp {
    /// The stamp of the file at `path`, not following a link.
    pub fn of(path: &Path) -> io::Result<Stamp> {
        let meta = fs::symlink_metadata(path)?;
        let modified = meta
            .modified()?
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as i64)
            .unwrap_or_default();
        Ok(Stamp {
            inode: meta.ino(),
            size: meta.len(),
            modified,
        })
    }
}

const TAGS: &str = "com.apple.metadata:_kMDItemUserTags";

/// The file's Finder tags, without their colours.
pub fn tags(path: &Path) -> io::Result<Vec<String>> {
    Ok(raw_tags(path)?
        .into_iter()
        .map(|t| tag_name(&t).to_owned())
        .collect())
}

/// Adds the tags the file doesn't have yet, ignoring case, and returns them.
pub fn add_tags(path: &Path, add: &[String]) -> io::Result<Vec<String>> {
    let mut raw = raw_tags(path)?;
    let mut added = Vec::new();
    for tag in add {
        let has = raw.iter().any(|t| tag_name(t).eq_ignore_ascii_case(tag))
            || added.iter().any(|a: &String| a.eq_ignore_ascii_case(tag));
        if !has {
            added.push(tag.clone());
        }
    }
    if !added.is_empty() {
        raw.extend(added.iter().cloned());
        write_tags(path, &raw)?;
    }
    Ok(added)
}

/// Removes these tags if the file still has them, keeping every other tag.
pub fn remove_tags(path: &Path, remove: &[String]) -> io::Result<()> {
    let raw = raw_tags(path)?;
    let kept: Vec<String> = raw
        .iter()
        .filter(|t| !remove.iter().any(|r| r.eq_ignore_ascii_case(tag_name(t))))
        .cloned()
        .collect();
    if kept.len() != raw.len() {
        write_tags(path, &kept)?;
    }
    Ok(())
}

/// A stored tag is its name, then optionally a newline and a colour number.
fn tag_name(raw: &str) -> &str {
    raw.split('\n').next().unwrap_or(raw)
}

fn raw_tags(path: &Path) -> io::Result<Vec<String>> {
    let (path, name) = (c_path(path)?, CString::new(TAGS).expect("no NUL"));
    let size = unsafe {
        libc::getxattr(
            path.as_ptr(),
            name.as_ptr(),
            std::ptr::null_mut(),
            0,
            0,
            libc::XATTR_NOFOLLOW,
        )
    };
    if size < 0 {
        let e = io::Error::last_os_error();
        return if e.raw_os_error() == Some(libc::ENOATTR) {
            Ok(Vec::new())
        } else {
            Err(e)
        };
    }
    let mut buf = vec![0u8; size as usize];
    let read = unsafe {
        libc::getxattr(
            path.as_ptr(),
            name.as_ptr(),
            buf.as_mut_ptr().cast(),
            buf.len(),
            0,
            libc::XATTR_NOFOLLOW,
        )
    };
    if read < 0 {
        return Err(io::Error::last_os_error());
    }
    buf.truncate(read as usize);
    // Tags another app wrote in a shape FolderFlow doesn't know are kept as none
    // rather than failing the run; they're only replaced if a tag is added.
    Ok(plist::from_bytes::<Vec<String>>(&buf).unwrap_or_default())
}

fn write_tags(path: &Path, tags: &[String]) -> io::Result<()> {
    let (path, name) = (c_path(path)?, CString::new(TAGS).expect("no NUL"));
    if tags.is_empty() {
        let result =
            unsafe { libc::removexattr(path.as_ptr(), name.as_ptr(), libc::XATTR_NOFOLLOW) };
        if result != 0 {
            let e = io::Error::last_os_error();
            if e.raw_os_error() != Some(libc::ENOATTR) {
                return Err(e);
            }
        }
        return Ok(());
    }
    let mut bytes = Vec::new();
    plist::to_writer_binary(&mut bytes, &tags).map_err(io::Error::other)?;
    check(unsafe {
        libc::setxattr(
            path.as_ptr(),
            name.as_ptr(),
            bytes.as_ptr().cast(),
            bytes.len(),
            0,
            libc::XATTR_NOFOLLOW,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rename_never_replaces_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a"), dir.path().join("b"));
        fs::write(&a, "a").unwrap();
        fs::write(&b, "b").unwrap();
        let err = rename_new(&a, &b).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(&b).unwrap(), "b");
        rename_new(&a, &dir.path().join("c")).unwrap();
        assert!(!a.exists());
    }

    #[test]
    fn a_clone_never_replaces_a_file_and_keeps_the_contents() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a"), dir.path().join("b"));
        fs::write(&a, "a").unwrap();
        fs::write(&b, "b").unwrap();
        assert_eq!(
            clone_new(&a, &b).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        clone_new(&a, &dir.path().join("c")).unwrap();
        assert_eq!(fs::read_to_string(dir.path().join("c")).unwrap(), "a");
    }

    #[test]
    fn tags_are_added_once_and_removed_without_touching_others() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("f");
        fs::write(&f, "x").unwrap();
        write_tags(&f, &["Red\n6".into()]).unwrap();

        let added = add_tags(&f, &["Receipts".into(), "red".into(), "receipts".into()]).unwrap();
        assert_eq!(added, ["Receipts"]);
        assert_eq!(tags(&f).unwrap(), ["Red", "Receipts"]);
        // The colour of a tag that was there stays.
        assert_eq!(raw_tags(&f).unwrap()[0], "Red\n6");

        remove_tags(&f, &["Receipts".into()]).unwrap();
        assert_eq!(tags(&f).unwrap(), ["Red"]);
        remove_tags(&f, &["Red".into()]).unwrap();
        assert!(tags(&f).unwrap().is_empty());
    }
}
