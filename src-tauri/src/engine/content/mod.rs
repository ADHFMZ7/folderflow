//! Turns a file into text for AI steps, on the Mac: plain text, web pages and
//! rich text, PDFs (with text recognition for scanned pages), photos and
//! screenshots, and Word, PowerPoint and Excel files. Nothing here leaves the
//! Mac. See docs/engine.md, "Reading the file".

mod apple;
mod office;
mod plain;

use std::path::Path;

/// How much text a file gives at most: about the first pages.
pub const LIMIT: usize = 30_000;

/// What was read from a file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Text {
    /// Empty when nothing could be read.
    pub text: String,
    /// Whether the text was cut at `LIMIT`.
    pub cut: bool,
    /// Whether any of it came from text recognition (a scan or a picture).
    pub recognised: bool,
}

impl Text {
    /// What the step should say about the reading, if anything.
    pub fn note(&self, name: &str) -> Option<String> {
        if self.text.trim().is_empty() {
            Some(format!(
                "FolderFlow couldn't read any text in {name}, so the model was given only its name."
            ))
        } else if self.cut {
            Some(format!(
                "{name} is long, so only its first 30,000 characters were read."
            ))
        } else {
            None
        }
    }

    fn plain(text: String) -> Text {
        Text {
            text,
            ..Text::default()
        }
    }
}

/// The file's text, by its extension. A file that can't be read, or of a
/// kind not listed, gives no text; this never fails.
pub fn read(path: &Path) -> Text {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let read = match ext.as_str() {
        "txt" | "md" | "markdown" | "csv" | "tsv" | "json" | "vtt" | "srt" | "log" => {
            plain::text(path).map(Text::plain)
        }
        "html" | "htm" => plain::html(path).map(Text::plain),
        "rtf" => plain::rtf(path).map(Text::plain),
        "docx" => office::docx(path).map(Text::plain),
        "pptx" => office::pptx(path).map(Text::plain),
        "xlsx" => office::xlsx(path).map(Text::plain),
        "pdf" => apple::pdf(path),
        "png" | "jpg" | "jpeg" | "heic" | "tif" | "tiff" | "gif" => {
            apple::picture(path).map(|text| Text {
                text,
                recognised: true,
                ..Text::default()
            })
        }
        _ => None,
    };
    let mut read = read.unwrap_or_default();
    if read.text.trim().is_empty() {
        read.text.clear();
    }
    if let Some((cut_at, _)) = read.text.char_indices().nth(LIMIT) {
        read.text.truncate(cut_at);
        read.cut = true;
    }
    read
}

/// Collects text until it's past the limit, so a huge file stops early.
#[derive(Default)]
struct Collect(String);

impl Collect {
    /// Past what `read` keeps, even for text all in 4-byte characters.
    fn full(&self) -> bool {
        self.0.len() > LIMIT * 4
    }

    fn push(&mut self, s: &str) {
        self.0.push_str(s);
    }

    /// Ends the current line, once.
    fn line(&mut self) {
        if !self.0.is_empty() && !self.0.ends_with('\n') {
            self.0.push('\n');
        }
    }

    /// Leaves one empty line, as between slides or sheets.
    fn gap(&mut self) {
        self.line();
        if !self.0.is_empty() && !self.0.ends_with("\n\n") {
            self.0.push('\n');
        }
    }
}
