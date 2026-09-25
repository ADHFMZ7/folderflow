//! Crash-safe file writes: a reader sees either the old file or the new one,
//! never a mix of the two.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

/// Replaces the file at `path` with `bytes`, readable only by its owner.
///
/// Writes to a temporary file in the same folder, flushes it to disk, then
/// renames it over `path`. If `path` is a symlink, the link itself is replaced.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
    let dir = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    // Unique per write, so concurrent writers never share a temp file.
    let tmp = dir.join(format!(
        ".{}.{}.tmp",
        name.to_string_lossy(),
        uuid::Uuid::new_v4()
    ));

    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&tmp, path)?;
        // Flush the folder too, so the rename itself survives a power cut.
        File::open(dir)?.sync_all()
    })();

    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}
