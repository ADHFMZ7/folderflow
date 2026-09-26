//! Paths between the native pickers and the front end. Pickers answer with
//! absolute paths; workflows show them with the home folder written as `~`,
//! which is how people read them and how the templates write them.

use std::path::{Path, PathBuf};

/// `path` with a leading `home` written as `~`. Anything else is unchanged.
pub fn shorten_home(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".into(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

/// Where a picker should open for a path the user already typed: `~` expanded,
/// and only if it's an absolute path. Variables and relative text give `None`.
pub fn expand_home(path: &str, home: &Path) -> Option<PathBuf> {
    if path.contains(['{', '}']) {
        return None;
    }
    let expanded = if path == "~" {
        home.to_path_buf()
    } else if let Some(rest) = path.strip_prefix("~/") {
        home.join(rest)
    } else {
        PathBuf::from(path)
    };
    expanded.is_absolute().then_some(expanded)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = "/Users/ada";

    #[test]
    fn a_path_in_the_home_folder_is_written_with_a_tilde() {
        let home = Path::new(HOME);
        assert_eq!(
            shorten_home(Path::new("/Users/ada/Documents/Receipts"), home),
            "~/Documents/Receipts"
        );
        assert_eq!(shorten_home(Path::new("/Users/ada"), home), "~");
    }

    #[test]
    fn other_paths_and_lookalikes_are_left_alone() {
        let home = Path::new(HOME);
        assert_eq!(
            shorten_home(Path::new("/Volumes/Backup/Scans"), home),
            "/Volumes/Backup/Scans"
        );
        assert_eq!(
            shorten_home(Path::new("/Users/adam/Documents"), home),
            "/Users/adam/Documents"
        );
    }

    #[test]
    fn a_typed_path_is_expanded_for_the_picker_to_start_in() {
        let home = Path::new(HOME);
        assert_eq!(
            expand_home("~/Documents", home),
            Some(PathBuf::from("/Users/ada/Documents"))
        );
        assert_eq!(expand_home("~", home), Some(PathBuf::from("/Users/ada")));
        assert_eq!(
            expand_home("/Volumes/Scans", home),
            Some(PathBuf::from("/Volumes/Scans"))
        );
    }

    #[test]
    fn variables_and_relative_text_give_no_start() {
        let home = Path::new(HOME);
        assert_eq!(expand_home("~/Documents/{year}", home), None);
        assert_eq!(expand_home("Documents", home), None);
        assert_eq!(expand_home("", home), None);
        assert_eq!(expand_home("~ada/x", home), None);
    }
}
