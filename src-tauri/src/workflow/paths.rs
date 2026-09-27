//! Paths as a workflow writes them: `~/…` or `/…`, possibly with `{variables}`.
//! These rules read only the text, so validation and the engine agree on
//! which folder a path names before any value is filled in.

use super::validate::variables_in;

/// How a path in a workflow can go wrong, before any value is filled in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathProblem {
    /// Not `~/…`, `/…` or a path that starts with a variable.
    NotFull,
    /// A `..` component.
    GoesUp,
    /// The folder it names is one FolderFlow never works in.
    NotAllowed(String),
}

impl PathProblem {
    pub fn message(&self) -> String {
        match self {
            PathProblem::NotFull => "Write the full path, starting with ~/ or /.".into(),
            PathProblem::GoesUp => "A path can't use .. to go up a folder.".into(),
            PathProblem::NotAllowed(folder) => format!(
                "FolderFlow doesn't work in {folder}. Choose a folder inside your home folder, like ~/Documents."
            ),
        }
    }
}

/// The folder a path is sure to be inside, whatever its variables turn out
/// to be: the path up to the folder that holds the first variable. `None`
/// when the path starts with a variable, like `{newFolder}`, and so names no
/// folder of its own. For a file path, pass `is_file` to get its folder.
pub fn fixed_folder(path: &str, is_file: bool) -> Option<String> {
    let path = normalize(path);
    let first = variables_in(&path).first().map(|name| {
        path.find(&format!("{{{name}}}"))
            .expect("found by variables_in")
    });
    let fixed = match first {
        Some(0) => return None,
        Some(at) => &path[..path[..at].rfind('/').map_or(0, |slash| slash.max(1))],
        None if is_file => &path[..path.rfind('/').map_or(0, |slash| slash.max(1))],
        None => &path,
    };
    // "~/x" cut at its first slash is "~".
    Some(if fixed.is_empty() {
        path[..1].to_string()
    } else {
        fixed.to_string()
    })
}

/// Whether the path, as written, is one a workflow may use. `is_file` for a
/// file path (Add row), whose folder is what counts.
pub fn check(path: &str, is_file: bool) -> Result<(), PathProblem> {
    let path = normalize(path);
    let starts_with_variable = variables_in(&path)
        .first()
        .is_some_and(|name| path.starts_with(&format!("{{{name}}}")));
    if !(path == "~" || path.starts_with("~/") || path.starts_with('/') || starts_with_variable) {
        return Err(PathProblem::NotFull);
    }
    if path.split('/').any(|part| part == "..") {
        return Err(PathProblem::GoesUp);
    }
    match fixed_folder(&path, is_file) {
        Some(folder) if forbidden(&folder) => Err(PathProblem::NotAllowed(folder)),
        _ => Ok(()),
    }
}

/// `/`, the home folder itself, and anything in `~/Library`, `/System` or
/// `/Applications`. Case never matters: the Mac's disks ignore it by default.
pub fn forbidden(folder: &str) -> bool {
    let folder = normalize(folder).to_lowercase();
    let within = |root: &str| folder == root || folder.starts_with(&format!("{root}/"));
    folder == "/"
        || folder == "~"
        || within("~/library")
        || within("/system")
        || within("/applications")
}

/// Repeated slashes collapsed and a trailing slash dropped: `~//a/` is `~/a`.
fn normalize(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for c in path.trim().chars() {
        if !(c == '/' && out.ends_with('/')) {
            out.push(c);
        }
    }
    if out.len() > 1 && out.ends_with('/') {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fixed_folder_ends_before_the_first_variable() {
        assert_eq!(
            fixed_folder("~/Documents/Receipts/{year}", false).as_deref(),
            Some("~/Documents/Receipts")
        );
        assert_eq!(
            fixed_folder("~/Documents/{category} {year}/x", false).as_deref(),
            Some("~/Documents")
        );
        assert_eq!(
            fixed_folder("~/Documents/Receipts", false).as_deref(),
            Some("~/Documents/Receipts")
        );
        assert_eq!(fixed_folder("~/{category}", false).as_deref(), Some("~"));
        assert_eq!(fixed_folder("/{category}", false).as_deref(), Some("/"));
        assert_eq!(fixed_folder("{newFolder}/notes", false), None);
        assert_eq!(
            fixed_folder("~/Documents/Expenses.csv", true).as_deref(),
            Some("~/Documents")
        );
        assert_eq!(fixed_folder("~/Expenses.csv", true).as_deref(), Some("~"));
        assert_eq!(fixed_folder("/Expenses.csv", true).as_deref(), Some("/"));
        assert_eq!(
            fixed_folder("~/Logs/{year}.csv", true).as_deref(),
            Some("~/Logs")
        );
    }

    #[test]
    fn forbidden_folders_ignore_case_and_extra_slashes() {
        for f in [
            "/",
            "~",
            "~/",
            "~/Library",
            "~/LIBRARY/Mail",
            "/System",
            "/Applications/",
            "~//Library",
        ] {
            assert!(forbidden(f), "{f}");
        }
        for f in [
            "~/Documents",
            "~/Library Books",
            "/Volumes/Scans",
            "/Applications2",
        ] {
            assert!(!forbidden(f), "{f}");
        }
    }
}
