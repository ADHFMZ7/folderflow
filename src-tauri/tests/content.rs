//! Reading a file's text for AI steps: each kind of file the spec lists gives
//! the text a person would see in it. The samples are in tests/fixtures/content,
//! made by make.sh there. See docs/engine.md, "Reading the file".

use std::path::PathBuf;

use folderflow_lib::engine::content::{self, LIMIT};

fn sample(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/content")
        .join(name)
}

fn text(name: &str) -> String {
    content::read(&sample(name)).text
}

/// Text recognition may split or join lines differently between macOS
/// versions: compare words only.
fn words(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn plain_text_is_read_as_utf8_or_else_latin1() {
    assert_eq!(
        text("note.txt"),
        "Receipt from Blue Bottle Coffee\nCafé latte 4.50\nTotal 4.50\n"
    );
    assert_eq!(text("latin1.txt"), "Café crème 3.20\n");
    assert_eq!(
        text("expenses.csv"),
        "Date,Vendor,Amount\n2026-09-14,Blue Bottle,4.50\n"
    );
    assert!(text("notes.md").contains("Budget for **2026** agreed"));
    assert!(text("order.json").contains("\"vendor\": \"Blue Bottle\""));
    assert!(text("lecture.srt").contains("Welcome to the course"));
    assert!(text("lecture.vtt").contains("Welcome back"));
}

#[test]
fn web_pages_and_rich_text_lose_their_markup() {
    let page = text("page.html");
    assert_eq!(
        words(&page),
        "Invoice Invoice 4471 Total & tax: 12 EUR — paid"
    );
    assert!(!page.contains("secret") && !page.contains("color"));

    let letter = text("letter.rtf");
    assert_eq!(
        words(&letter),
        "Dear Sam, The lease for Flat 3 ends on 30 June 2027."
    );
    assert!(
        letter.contains("Dear Sam,\n"),
        "paragraphs stay lines: {letter:?}"
    );
}

#[test]
fn a_pdf_gives_its_own_text() {
    let read = content::read(&sample("invoice.pdf"));
    assert_eq!(
        words(&read.text),
        "INVOICE 4471 Blue Bottle Coffee Total due: 4.50"
    );
    assert!(!read.recognised);
}

#[test]
fn a_scanned_pdf_and_a_photo_are_read_by_text_recognition() {
    for name in ["scan.pdf", "receipt.png"] {
        let read = content::read(&sample(name));
        let seen = words(&read.text);
        for expected in ["INVOICE 4471", "Blue Bottle Coffee", "4.50"] {
            assert!(
                seen.contains(expected),
                "{name}: {expected} not in {seen:?}"
            );
        }
        assert!(read.recognised, "{name}");
    }
}

#[test]
fn office_documents_give_the_text_inside_them() {
    assert_eq!(
        text("report.docx").trim_end(),
        "Dear Sam,\nThe lease for Flat 3 ends on 30 June 2027."
    );
    // Slides in their order (1, 2, 10), each paragraph a line.
    assert_eq!(
        text("slides.pptx").trim_end(),
        "Quarterly review\nSales up 12%\n\nSecond slide: results\n\nTenth slide"
    );
    // Sheet by sheet, a row per line, cells between tabs.
    assert_eq!(
        text("budget.xlsx").trim_end(),
        "Budget\nItem\tCost\nRent (March)\t1200.5\nTotal\t1200.5\n\nNotes\nPaid on time\tTRUE"
    );
}

#[test]
fn anything_else_gives_no_text_and_says_why() {
    let tmp = tempfile::tempdir().unwrap();
    let damaged = tmp.path().join("broken.docx");
    std::fs::write(&damaged, "not a zip").unwrap();
    for path in [sample("data.bin"), damaged, tmp.path().join("missing.pdf")] {
        let read = content::read(&path);
        assert_eq!(read.text, "", "{}", path.display());
        let name = path.file_name().unwrap().to_string_lossy();
        assert_eq!(
            read.note(&name).unwrap(),
            format!("FolderFlow couldn't read any text in {name}, so the model was given only its name.")
        );
    }
}

#[test]
fn long_text_is_cut_and_says_so() {
    let tmp = tempfile::tempdir().unwrap();
    let long = tmp.path().join("long.txt");
    std::fs::write(&long, "é".repeat(LIMIT + 500)).unwrap();

    let read = content::read(&long);

    assert_eq!(read.text.chars().count(), LIMIT);
    assert!(read.cut);
    assert_eq!(
        read.note("long.txt").unwrap(),
        "long.txt is long, so only its first 30,000 characters were read."
    );
    assert_eq!(content::read(&sample("note.txt")).note("note.txt"), None);
}
