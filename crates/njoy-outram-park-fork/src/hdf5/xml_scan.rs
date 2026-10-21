// SPDX-License-Identifier: GPL-3.0

//! **A minimal element scanner for the small XML files OpenMC ships beside its
//! HDF5 libraries** — `cross_sections.xml` and the depletion chain.
//!
//! # Why hand-written rather than `serde-xml-rs`
//!
//! The two consumers need four element shapes between them: self-closing
//! elements with attributes, elements whose only content is text, and elements
//! that contain other elements. A scanner that reads exactly those lets each
//! consumer **refuse** what it does not recognise, which a permissive
//! deserialiser cannot: a derive that ignores unknown elements would read a
//! future file and return a silently incomplete result.
//!
//! # What it returns, and what it does not
//!
//! A **flat** list of elements in document order, each with its name,
//! attributes, and — only when the element's entire content is text — that
//! text. Closing tags, comments and the XML declaration are dropped. There is
//! no tree: a consumer that needs nesting reconstructs it from the element
//! order, which works because both formats are unambiguous about which parent
//! each element name belongs to. That is a real limitation and is why this is
//! `pub(crate)` rather than a published API.
//!
//! # Provenance
//!
//! Extracted 2026-09-23 from [`super::cross_sections_xml`], which carried this
//! logic privately. The extraction also **fixed a quadratic-time defect**: that
//! version held the document as `Vec<char>` and converted a char index back to
//! a byte offset with `chars[..i].iter().map(char::len_utf8).sum()` on *every*
//! `<`, making a scan `O(n^2)` in the document length. Unnoticeable on
//! `cross_sections.xml` (a few hundred lines); fatal on an ENDF/B-VIII
//! depletion chain, which is tens of megabytes. This version indexes bytes
//! directly and is `O(n)`. Slicing at `<`, `>` and quote characters is
//! UTF-8-safe because a multi-byte sequence never contains an ASCII byte.
//!
//! **Measured 2026-09-23**, both versions counting the same elements on
//! synthetic chain files (`rustc -O`, this container):
//!
//! | document | elements | this version | the old one | ratio |
//! |---|---|---|---|---|
//! | 0.24 MB | 4,001 | 117 µs | 236 ms | 2,017x |
//! | 0.98 MB | 16,001 | 634 µs | 3.96 s | 6,252x |
//! | 3.94 MB | 64,001 | 1.94 ms | 65.7 s | 33,842x |
//! | 9.92 MB | 160,001 | 5.00 ms | 411.6 s | 82,330x |
//!
//! The ratio quadrupling for each quadrupling of size is the signature of the
//! quadratic term, and it is why this is recorded as a **fixed defect** and not
//! a micro-optimisation: a 4 MB chain file took over a minute to scan, and the
//! full ENDF/B-VIII chain is several times that again.

use std::collections::BTreeMap;

/// One element from the document: its name, its attributes, and its text
/// content when the element holds nothing but text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct XmlElement {
    /// Element name, e.g. `"library"` or `"nuclide"`.
    pub name: String,
    /// Attributes, by name. Both `"` and `'` quoting are accepted.
    pub attrs: BTreeMap<String, String>,
    /// The element's text content, **empty unless the element's whole content
    /// is text** (i.e. the next thing after the text is this element's own
    /// closing tag). An element containing child elements reports empty text
    /// and its children follow it in the returned list.
    pub text: String,
}

impl XmlElement {
    /// An attribute's value, or `None` if the element does not carry it.
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs.get(key).map(String::as_str)
    }
}

/// Scan a document into a flat list of elements in document order.
pub(crate) fn scan(xml: &str) -> Vec<XmlElement> {
    let b = xml.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < b.len() {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        // A comment runs to `-->`; an unterminated one ends the scan.
        if b[i..].starts_with(b"<!--") {
            match xml[i..].find("-->") {
                Some(off) => {
                    i += off + 3;
                    continue;
                }
                None => break,
            }
        }
        if i + 1 >= b.len() {
            break;
        }
        // Declarations (`<?xml …?>`), doctypes (`<!…>`) and closing tags carry
        // nothing this scanner reports.
        if matches!(b[i + 1], b'?' | b'!' | b'/') {
            while i < b.len() && b[i] != b'>' {
                i += 1;
            }
            i += 1;
            continue;
        }
        let start = i + 1;
        let mut j = start;
        while j < b.len() && b[j] != b'>' {
            j += 1;
        }
        if j >= b.len() {
            break; // Unterminated tag: stop rather than invent an element.
        }
        let inner = xml[start..j].trim_end();
        let self_closing = inner.ends_with('/');
        let inner = inner.trim_end_matches('/');
        let mut parts = inner.splitn(2, char::is_whitespace);
        let name = parts.next().unwrap_or("").trim().to_string();
        let attrs = parse_attrs(parts.next().unwrap_or(""));

        let mut text = String::new();
        i = j + 1;
        if !self_closing {
            let mut k = i;
            while k < b.len() && b[k] != b'<' {
                k += 1;
            }
            // Take the text only when what follows closes THIS element;
            // otherwise leave the position alone so the children are scanned.
            if closes(&xml[k..], &name) {
                text.push_str(&xml[i..k]);
                i = k;
            }
        }
        if !name.is_empty() {
            out.push(XmlElement { name, attrs, text });
        }
    }
    out
}

/// Whether `rest` begins with the closing tag of `name` — `</name>` or
/// `</name   >`, but **not** `</nameLonger>`.
fn closes(rest: &str, name: &str) -> bool {
    let Some(after) = rest.strip_prefix("</") else {
        return false;
    };
    let Some(after) = after.strip_prefix(name) else {
        return false;
    };
    match after.as_bytes().first() {
        Some(b'>') => true,
        Some(c) if c.is_ascii_whitespace() => true,
        _ => false,
    }
}

/// Parse an attribute list. Accepts `"` and `'` quoting, and any amount of
/// whitespace around `=`; both appear in files OpenMC writes and in files
/// people hand-edit.
fn parse_attrs(s: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let b = s.as_bytes();
    let mut i = 0usize;
    while i < b.len() {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let start = i;
        while i < b.len() && b[i] != b'=' && !b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        let key = s[start..i].to_string();
        while i < b.len() && (b[i] == b'=' || b[i].is_ascii_whitespace()) {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        let quote = b[i];
        if quote != b'"' && quote != b'\'' {
            break;
        }
        i += 1;
        let vstart = i;
        while i < b.len() && b[i] != quote {
            i += 1;
        }
        let value = s[vstart..i.min(s.len())].to_string();
        i += 1;
        if !key.is_empty() {
            out.insert(key, value);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three shapes both consumers rely on: a self-closing element with
    /// attributes, a text-only element, and a parent whose children follow it.
    #[test]
    fn the_scanner_reports_attributes_text_and_nesting_order() {
        let els = scan(
            "<?xml version='1.0'?>\n\
             <root>\n\
             <!-- a comment with a < and a > in it -->\n\
             <dir>/data/lib</dir>\n\
             <leaf a=\"1\" b='two' />\n\
             <parent name=\"p\"><kid>3 4</kid></parent>\n\
             </root>",
        );
        let names: Vec<&str> = els.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["root", "dir", "leaf", "parent", "kid"]);
        assert_eq!(els[1].text, "/data/lib");
        assert_eq!(els[2].attr("a"), Some("1"));
        assert_eq!(els[2].attr("b"), Some("two"));
        // A parent with children reports no text; the child carries it.
        assert_eq!(els[3].text, "");
        assert_eq!(els[3].attr("name"), Some("p"));
        assert_eq!(els[4].text, "3 4");
    }

    /// `</nuclide_extra>` must not be read as closing `<nuclide>`. The
    /// prefix-match this replaced would have taken the text.
    #[test]
    fn a_longer_name_does_not_close_a_shorter_one() {
        let els = scan("<a>text</ab></a>");
        assert_eq!(els.len(), 1);
        assert_eq!(els[0].text, "", "`</ab>` must not close `<a>`");
    }

    /// Non-ASCII text must not shift the byte offsets. This is the case the
    /// quadratic char-index conversion existed to handle.
    #[test]
    fn multibyte_text_is_sliced_correctly() {
        let els = scan("<note>naïve — 2.5 µs</note><after x=\"1\"/>");
        assert_eq!(els[0].text, "naïve — 2.5 µs");
        assert_eq!(els[1].attr("x"), Some("1"));
    }

    /// A truncated document ends the scan instead of panicking or inventing an
    /// element with a half-read name.
    #[test]
    fn a_truncated_document_stops_rather_than_panics() {
        assert_eq!(scan("<a><b").len(), 1);
        assert_eq!(scan("<!-- unterminated").len(), 0);
        assert_eq!(scan("<").len(), 0);
    }
}
