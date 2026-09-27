//! Text files, web pages and rich text.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use super::LIMIT;

/// Read at most this much of a text file: enough for LIMIT characters.
const MAX_BYTES: u64 = (LIMIT as u64) * 4 + 4;
/// Web pages and rich text carry markup: read more of them.
const MAX_MARKUP_BYTES: u64 = 8 * 1024 * 1024;

fn bytes(path: &Path, max: u64) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    File::open(path)
        .ok()?
        .take(max)
        .read_to_end(&mut out)
        .ok()?;
    Some(out)
}

/// UTF-8, or Latin-1 if it isn't. When the read stopped at a size limit
/// (`cut`), a character cut in two at the end is dropped.
fn decode(bytes: &[u8], cut: bool) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_owned(),
        Err(e) if cut && e.error_len().is_none() => {
            String::from_utf8_lossy(&bytes[..e.valid_up_to()]).into_owned()
        }
        Err(_) => bytes.iter().map(|&b| char::from(b)).collect(),
    }
}

pub fn text(path: &Path) -> Option<String> {
    let bytes = bytes(path, MAX_BYTES)?;
    let cut = bytes.len() as u64 == MAX_BYTES;
    let text = decode(bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes), cut);
    // A NUL means it isn't text after all.
    (!text.contains('\0')).then_some(text)
}

/// The words of a web page: no tags, scripts, styles or comments, entities
/// decoded, a line per block.
pub fn html(path: &Path) -> Option<String> {
    let bytes = bytes(path, MAX_MARKUP_BYTES)?;
    let page = decode(&bytes, bytes.len() as u64 == MAX_MARKUP_BYTES);
    let lower = page.to_ascii_lowercase();
    let mut out = String::new();
    let mut at = 0;
    while at < page.len() {
        let Some(open) = page[at..].find('<').map(|i| at + i) else {
            out.push_str(&entities(&page[at..]));
            break;
        };
        out.push_str(&entities(&page[at..open]));
        if lower[open..].starts_with("<!--") {
            at = lower[open..]
                .find("-->")
                .map_or(page.len(), |i| open + i + 3);
            continue;
        }
        let close = lower[open..].find('>').map_or(page.len(), |i| open + i + 1);
        let tag: String = lower[open + 1..close]
            .trim_start_matches('/')
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        at = close;
        if matches!(tag.as_str(), "script" | "style" | "noscript" | "template")
            && !lower[open..].starts_with("</")
        {
            let end = format!("</{tag}");
            at = lower[close..].find(&end).map_or(page.len(), |i| close + i);
            at = lower[at..].find('>').map_or(page.len(), |i| at + i + 1);
            continue;
        }
        if BLOCKS.contains(&tag.as_str()) {
            out.push('\n');
        }
    }
    Some(tidy(&out))
}

const BLOCKS: [&str; 22] = [
    "p",
    "br",
    "div",
    "li",
    "tr",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "title",
    "table",
    "ul",
    "ol",
    "section",
    "article",
    "header",
    "footer",
    "blockquote",
    "pre",
    "hr",
];

fn entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let end = rest[..rest.len().min(12)].find(';');
        let decoded = end.and_then(|end| entity(&rest[1..end]).map(|c| (c, end)));
        match decoded {
            Some((c, end)) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn entity(name: &str) -> Option<char> {
    if let Some(num) = name.strip_prefix('#') {
        let code = match num.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => num.parse().ok()?,
        };
        return char::from_u32(code);
    }
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => ' ',
        "mdash" => '—',
        "ndash" => '–',
        "hellip" => '…',
        "rsquo" => '’',
        "lsquo" => '‘',
        "rdquo" => '”',
        "ldquo" => '“',
        "euro" => '€',
        "pound" => '£',
        "copy" => '©',
        _ => return None,
    })
}

/// Runs of spaces become one, each line is trimmed, and empty lines go.
fn tidy(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if !line.is_empty() {
            out.push_str(&line);
            out.push('\n');
        }
    }
    out
}

/// The words of an RTF document: control words and their groups dropped,
/// paragraphs as lines, `\'hh` and `\uN` characters decoded.
pub fn rtf(path: &Path) -> Option<String> {
    let doc = bytes(path, MAX_MARKUP_BYTES)?;
    if !doc.starts_with(b"{\\rtf") {
        return None;
    }
    let mut out = String::new();
    // For each open group: whether its text is skipped.
    let mut skip = vec![false];
    // Characters to skip after a \uN (its fallback).
    let mut fallback = 0usize;
    let mut i = 0;
    while i < doc.len() {
        let skipping = *skip.last().unwrap_or(&false);
        match doc[i] {
            b'{' => {
                skip.push(skipping);
                i += 1;
            }
            b'}' => {
                skip.pop();
                i += 1;
            }
            b'\\' => {
                i += 1;
                let Some(&c) = doc.get(i) else { break };
                if c == b'\'' {
                    let hex = doc
                        .get(i + 1..i + 3)
                        .and_then(|h| std::str::from_utf8(h).ok());
                    if let Some(code) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                        if fallback > 0 {
                            fallback -= 1;
                        } else if !skipping {
                            out.push(cp1252(code));
                        }
                    }
                    i += 3;
                    continue;
                }
                if !c.is_ascii_alphabetic() {
                    // \\ \{ \} are the characters; \* marks a group to skip.
                    match c {
                        b'*' => {
                            if let Some(top) = skip.last_mut() {
                                *top = true;
                            }
                        }
                        b'\\' | b'{' | b'}' if !skipping => out.push(char::from(c)),
                        b'~' if !skipping => out.push(' '),
                        // A backslash before a line break is a paragraph, as Apple writes it.
                        b'\n' | b'\r' if !skipping => out.push('\n'),
                        _ => {}
                    }
                    i += 1;
                    continue;
                }
                let start = i;
                while i < doc.len() && doc[i].is_ascii_alphabetic() {
                    i += 1;
                }
                let word = std::str::from_utf8(&doc[start..i]).unwrap_or_default();
                let num_start = i;
                if i < doc.len() && (doc[i] == b'-' || doc[i].is_ascii_digit()) {
                    i += 1;
                    while i < doc.len() && doc[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                let num: Option<i32> = std::str::from_utf8(&doc[num_start..i])
                    .ok()
                    .and_then(|n| n.parse().ok());
                if i < doc.len() && doc[i] == b' ' {
                    i += 1;
                }
                match word {
                    "fonttbl" | "colortbl" | "stylesheet" | "info" | "pict" | "header"
                    | "footer" | "expandedcolortbl" | "listtable" | "listoverridetable" => {
                        if let Some(top) = skip.last_mut() {
                            *top = true;
                        }
                    }
                    "par" | "line" | "sect" | "row" if !skipping => out.push('\n'),
                    "tab" | "cell" if !skipping => out.push('\t'),
                    "u" => {
                        if let Some(code) = num {
                            let code = if code < 0 { code + 65536 } else { code } as u32;
                            if !skipping {
                                out.extend(char::from_u32(code));
                            }
                            fallback = 1;
                        }
                    }
                    _ => {}
                }
            }
            b'\r' | b'\n' => i += 1,
            c => {
                if fallback > 0 {
                    fallback -= 1;
                } else if !skipping {
                    out.push(char::from(c));
                }
                i += 1;
            }
        }
    }
    Some(
        out.lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_owned()
            + "\n",
    )
}

/// Windows-1252, which RTF's \'hh uses on the Mac too; close enough for text.
fn cp1252(code: u8) -> char {
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž',
        '\u{8f}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}',
        'ž', 'Ÿ',
    ];
    match code {
        0x80..=0x9f => HIGH[usize::from(code - 0x80)],
        _ => char::from(code),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entities_and_stray_ampersands() {
        assert_eq!(
            entities("A &amp; B &#233; &#x41; & C &bogus;"),
            "A & B é A & C &bogus;"
        );
    }

    #[test]
    fn a_cut_character_at_the_end_is_dropped_not_latin1() {
        let mut bytes = "café".as_bytes().to_vec();
        bytes.pop();
        assert_eq!(decode(&bytes, true), "caf");
        assert_eq!(decode(b"caf\xe9", false), "café");
    }
}
