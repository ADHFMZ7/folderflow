//! Word, PowerPoint and Excel files: zip archives of XML. Only the text is
//! taken; each part read is capped, so a hostile file can't unpack to gigabytes.

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use quick_xml::escape::resolve_xml_entity;
use quick_xml::events::{BytesRef, BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use zip::ZipArchive;

use super::Collect;

/// The most one part of a document may unpack to.
const MAX_PART: u64 = 32 * 1024 * 1024;

type Archive = ZipArchive<File>;

fn open(path: &Path) -> Option<Archive> {
    ZipArchive::new(File::open(path).ok()?).ok()
}

fn part(zip: &mut Archive, name: &str) -> Option<String> {
    let mut out = String::new();
    zip.by_name(name)
        .ok()?
        .take(MAX_PART)
        .read_to_string(&mut out)
        .ok()?;
    Some(out)
}

fn local(e: &BytesStart) -> String {
    e.local_name().as_ref().to_owned()
}

fn attr(e: &BytesStart, name: &str) -> Option<String> {
    let a = e.try_get_attribute(name).ok()??;
    a.normalized_value(XmlVersion::Implicit1_0)
        .ok()
        .map(|v| v.into_owned())
}

fn reference(r: &BytesRef) -> String {
    if let Ok(Some(c)) = r.resolve_char_ref() {
        return c.to_string();
    }
    resolve_xml_entity(r).unwrap_or_default().to_owned()
}

/// Walks an XML part: text inside a `text` element goes to `out`; `on`
/// hears every start and end tag, by local name, and whether it's an end.
fn walk(xml: &str, text: &str, out: &mut Collect, mut on: impl FnMut(&str, bool, &mut Collect)) {
    let mut reader = Reader::from_str(xml);
    let mut inside = 0usize;
    while !out.full() {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local(&e);
                if name == text {
                    inside += 1;
                }
                on(&name, false, out);
            }
            Ok(Event::Empty(e)) => {
                let name = local(&e);
                on(&name, false, out);
                on(&name, true, out);
            }
            Ok(Event::End(e)) => {
                let name = e.local_name().as_ref().to_owned();
                if name == text {
                    inside = inside.saturating_sub(1);
                }
                on(&name, true, out);
            }
            Ok(Event::Text(t)) if inside > 0 => out.push(&t),
            Ok(Event::CData(t)) if inside > 0 => out.push(&t),
            Ok(Event::GeneralRef(r)) if inside > 0 => out.push(&reference(&r)),
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

/// The document's paragraphs, a line each; tabs and breaks kept.
pub fn docx(path: &Path) -> Option<String> {
    let mut zip = open(path)?;
    let xml = part(&mut zip, "word/document.xml")?;
    let mut out = Collect::default();
    walk(&xml, "t", &mut out, |name, end, out| match (name, end) {
        ("p", true) | ("br", true) | ("cr", true) => out.push("\n"),
        ("tab", true) => out.push("\t"),
        _ => {}
    });
    Some(out.0)
}

/// Each slide in order, a line per paragraph, an empty line between slides.
pub fn pptx(path: &Path) -> Option<String> {
    let mut zip = open(path)?;
    let mut slides: Vec<(u32, String)> = zip
        .file_names()
        .filter_map(|n| {
            let num = n.strip_prefix("ppt/slides/slide")?.strip_suffix(".xml")?;
            Some((num.parse().ok()?, n.to_owned()))
        })
        .collect();
    slides.sort();
    let mut out = Collect::default();
    for (_, name) in slides {
        if out.full() {
            break;
        }
        let Some(xml) = part(&mut zip, &name) else {
            continue;
        };
        out.gap();
        walk(&xml, "t", &mut out, |name, end, out| {
            if end && (name == "p" || name == "br") {
                out.line();
            }
        });
    }
    Some(out.0)
}

/// Each sheet by name, in the workbook's order: a line per row, cells
/// between tabs, formulas as their last computed value.
pub fn xlsx(path: &Path) -> Option<String> {
    let mut zip = open(path)?;
    let shared = part(&mut zip, "xl/sharedStrings.xml")
        .map(|xml| shared_strings(&xml))
        .unwrap_or_default();
    let targets = part(&mut zip, "xl/_rels/workbook.xml.rels")
        .map(|xml| relationships(&xml))
        .unwrap_or_default();
    let workbook = part(&mut zip, "xl/workbook.xml")?;
    let mut out = Collect::default();
    for (sheet, rel) in sheets(&workbook) {
        if out.full() {
            break;
        }
        let Some(target) = targets.get(&rel) else {
            continue;
        };
        let target = target.trim_start_matches('/').trim_start_matches("xl/");
        let Some(xml) = part(&mut zip, &format!("xl/{target}")) else {
            continue;
        };
        out.gap();
        out.push(&sheet);
        out.line();
        cells(&xml, &shared, &mut out);
    }
    Some(out.0)
}

fn shared_strings(xml: &str) -> Vec<String> {
    let mut list = Vec::new();
    let mut reader = Reader::from_str(xml);
    let (mut current, mut in_t) = (String::new(), false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if e.local_name().as_ref() == "si" => current.clear(),
            Ok(Event::Start(e)) if e.local_name().as_ref() == "t" => in_t = true,
            Ok(Event::End(e)) if e.local_name().as_ref() == "t" => in_t = false,
            Ok(Event::End(e)) if e.local_name().as_ref() == "si" => {
                list.push(std::mem::take(&mut current))
            }
            Ok(Event::Text(t)) if in_t => current.push_str(&t),
            Ok(Event::GeneralRef(r)) if in_t => current.push_str(&reference(&r)),
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    list
}

/// Relationship id → target, from `xl/_rels/workbook.xml.rels`.
fn relationships(xml: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut reader = Reader::from_str(xml);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e) | Event::Empty(e)) if e.local_name().as_ref() == "Relationship" => {
                if let (Some(id), Some(target)) = (attr(&e, "Id"), attr(&e, "Target")) {
                    out.insert(id, target);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    out
}

/// (sheet name, relationship id), in the workbook's order.
fn sheets(xml: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut reader = Reader::from_str(xml);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e) | Event::Empty(e)) if e.local_name().as_ref() == "sheet" => {
                if let (Some(name), Some(id)) = (attr(&e, "name"), attr(&e, "r:id")) {
                    out.push((name, id));
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    out
}

fn cells(xml: &str, shared: &[String], out: &mut Collect) {
    let mut reader = Reader::from_str(xml);
    let mut kind = String::new();
    let mut value = String::new();
    // Inside <v> (a value) or <t> (inline text), where text counts.
    let mut in_value = false;
    let mut row: Vec<String> = Vec::new();
    while !out.full() {
        match reader.read_event() {
            Ok(Event::Start(e)) => match e.local_name().as_ref() {
                "row" => row.clear(),
                "c" => {
                    kind = attr(&e, "t").unwrap_or_default();
                    value.clear();
                }
                "v" | "t" => in_value = true,
                _ => {}
            },
            Ok(Event::Text(t)) if in_value => value.push_str(&t),
            Ok(Event::GeneralRef(r)) if in_value => value.push_str(&reference(&r)),
            Ok(Event::End(e)) => match e.local_name().as_ref() {
                "v" | "t" => in_value = false,
                "c" => row.push(match kind.as_str() {
                    "s" => value
                        .trim()
                        .parse::<usize>()
                        .ok()
                        .and_then(|i| shared.get(i))
                        .cloned()
                        .unwrap_or_default(),
                    "b" => if value.trim() == "1" { "TRUE" } else { "FALSE" }.to_owned(),
                    _ => value.clone(),
                }),
                "row" => {
                    while row.last().is_some_and(String::is_empty) {
                        row.pop();
                    }
                    if !row.is_empty() {
                        out.push(&row.join("\t"));
                        out.line();
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}
