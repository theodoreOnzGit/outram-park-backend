// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "Endnote XML.js": `doImport`/`startImport`/
//   `importNext` :510-697, `htmlify` :883-942 (with `en2zMap`,
//   `en2zNoteMap` :869-881), `processField` :951-961.
// Copyright (c) Sebastian Karcher (creator named in the header; the file
//   carries no copyright line).
// Licence: AGPL-3.0 (no licence text upstream; treated as AGPLv3 as part of
//   Zotero, maintainer decision 2026-10-07, #747).

//! Endnote XML import.

use super::{get_field, process_item_type, process_number_type};
use crate::zotero::framework::item::{JsObject, TranslatorNote, TranslatorTag};
use crate::zotero::framework::js;
use crate::zotero::framework::utilities::{clean_author, field_is_valid_for_type, trim_internal};
use crate::zotero::framework::xml::{get_xml, node_type, NodeId, XNode, XmlDocument};
use crate::zotero::framework::xpath::{xpath, xpath_text};
use crate::zotero::framework::{ImportContext, TranslateError, TranslatorCreator, TranslatorItem};
use serde_json::Value;

/// `en2zMap` (:869-874) and, for notes, `en2zNoteMap` (:876-877, which adds
/// `underline`).
fn face_tag(face: &str, note: bool) -> Option<&'static str> {
    match face {
        "italic" => Some("i"),
        "bold" => Some("b"),
        "superscript" => Some("sup"),
        "subscript" => Some("sub"),
        "underline" if note => Some("u"),
        _ => None,
    }
}

/// `htmlify(nodes, field)` (:883-942): an Endnote `<style face="...">`
/// sequence as Zotero HTML. A single text child is returned as is; otherwise
/// only the element children count (text directly inside is dropped).
fn htmlify(doc: &XmlDocument, n: NodeId, note: bool) -> String {
    let kids = doc.children(n);
    if kids.len() == 1 && doc.node_type(XNode::Node(kids[0])) == node_type::TEXT {
        return doc.text(n);
    }
    let mut html = String::new();
    let mut formatting: Vec<String> = Vec::new();
    for child in doc.element_children(n) {
        let face: Vec<String> = match doc.get_attribute(child, "face") {
            Some(f) if !f.is_empty() => f
                .split(js::is_space)
                .filter(|x| face_tag(x, note).is_some())
                .map(str::to_owned)
                .collect(),
            _ => Vec::new(),
        };
        // What closes.
        let mut closing: Vec<&str> = Vec::new();
        let mut j = 0;
        while j < formatting.len() {
            if !face.contains(&formatting[j]) {
                closing.push(face_tag(&formatting[j], note).unwrap_or_default());
                formatting.remove(j);
            } else {
                j += 1;
            }
        }
        if !closing.is_empty() {
            closing.reverse();
            html.push_str(&format!("</{}>", closing.join("></")));
        }
        // What opens.
        let mut opening: Vec<&str> = Vec::new();
        for f in &face {
            let Some(t) = face_tag(f, note) else {
                continue;
            };
            if !formatting.contains(f) {
                opening.push(t);
                formatting.push(f.clone());
            }
        }
        if !opening.is_empty() {
            html.push_str(&format!("<{}>", opening.join("><")));
        }
        html.push_str(&doc.text(child));
    }
    let mut closing: Vec<&str> = formatting
        .iter()
        .map(|f| face_tag(f, note).unwrap_or_default())
        .collect();
    if !closing.is_empty() {
        closing.reverse();
        html.push_str(&format!("</{}>", closing.join("></")));
    }
    html
}

/// `processField(node, keepNewlines, field)` (:951-961).
fn process_field(doc: &XmlDocument, n: NodeId, keep_newlines: bool, note: bool) -> String {
    if doc.text(n).is_empty() {
        return String::new();
    }
    let content = htmlify(doc, n, note);
    if keep_newlines {
        content
    } else {
        trim_internal(&content)
    }
}

/// `.replace(/\r\n?/g, '\n')`.
fn normalise_cr(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

/// The note markup of :624-630: runs of two or more line breaks
/// (`/(?:\r\n|\r(?!\n)|\n){2,}/`) split paragraphs, joined by `</p><p>`,
/// and every remaining run of `[\r\n]` becomes `<br/>`.
fn note_html(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    let mut out = String::from("<p>");
    let mut i = 0;
    while i < c.len() {
        if c[i] == '\r' || c[i] == '\n' {
            let start = i;
            while i < c.len() && (c[i] == '\r' || c[i] == '\n') {
                i += 1;
            }
            // Count line-break units in the run: \r\n, \r, \n.
            let mut units = 0;
            let mut k = start;
            while k < i {
                if c[k] == '\r' && k + 1 < i && c[k + 1] == '\n' {
                    k += 2;
                } else {
                    k += 1;
                }
                units += 1;
            }
            out.push_str(if units >= 2 { "</p><p>" } else { "<br/>" });
        } else {
            out.push(c[i]);
            i += 1;
        }
    }
    out.push_str("</p>");
    out
}

/// `filepath.replace(/.+\//, "")`: drop through the last `/` of the first
/// line that has a `/` after at least one character (`.` does not match line
/// terminators).
fn strip_dir(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    let is_lt = |x: char| matches!(x, '\n' | '\r' | '\u{2028}' | '\u{2029}');
    let mut s0 = 0;
    while s0 < c.len() {
        if is_lt(c[s0]) {
            s0 += 1;
            continue;
        }
        let mut e = s0;
        while e < c.len() && !is_lt(c[e]) {
            e += 1;
        }
        // Leftmost start: the first non-terminator position of this line
        // with a later '/' on the line.
        if let Some(rel) = c[s0 + 1..e].iter().rposition(|&x| x == '/') {
            let slash = s0 + 1 + rel;
            let mut out: String = c[..s0].iter().collect();
            out.extend(&c[slash + 1..]);
            return out;
        }
        s0 = e;
    }
    s.to_owned()
}

/// `.replace(/\.[^\.]+$/, "")`.
fn strip_ext(s: &str) -> String {
    if let Some(i) = s.rfind('.') {
        if i + 1 < s.len() {
            return s[..i].to_owned();
        }
    }
    s.to_owned()
}

fn creator(value: &str, creator_type: &str, use_comma: bool) -> TranslatorCreator {
    let a = clean_author(value, creator_type, use_comma);
    TranslatorCreator {
        first_name: a.first_name,
        last_name: Some(a.last_name),
        creator_type: Some(a.creator_type),
        ..Default::default()
    }
}

fn type_error(m: &str) -> TranslateError {
    TranslateError::Translator(format!("TypeError: {m}"))
}

/// `doImport` (:510-697): one item per `//record`.
pub fn do_import(ctx: &mut ImportContext) -> Result<(), TranslateError> {
    let doc = get_xml(ctx)?;
    let records = xpath(&doc, doc.document(), "//record", &[])?;
    for rec in records {
        let XNode::Node(record) = rec else {
            continue;
        };
        let item = import_record(&doc, record)?;
        ctx.item_done(item);
    }
    Ok(())
}

fn import_record(doc: &XmlDocument, record: NodeId) -> Result<TranslatorItem, TranslateError> {
    // :540-546
    let mut item_type = xpath_text(doc, record, ".//ref-type/@name", &[], None)?
        .and_then(|n| process_item_type(&n));
    if item_type.is_none() {
        item_type = xpath_text(doc, record, ".//ref-type", &[], None)?
            .and_then(|n| process_number_type(&n));
    }
    let item_type = item_type.unwrap_or("journalArticle");
    let mut item = TranslatorItem::new(item_type);
    let mut notecache: Vec<String> = Vec::new();

    for node in doc.element_children(record) {
        let field = doc.tag_name(node);
        if let Some(zfield) = get_field(&field, item_type) {
            // :554-569
            if zfield.contains("creators") {
                let authortype = zfield.replacen("creators/", "", 1);
                item.creators
                    .push(creator(&doc.text(node), &authortype, false));
            } else if field_is_valid_for_type(zfield, item_type) {
                if zfield == "abstractNote" {
                    item.set(zfield, normalise_cr(&process_field(doc, node, true, false)));
                } else {
                    item.set(zfield, process_field(doc, node, false, false));
                }
            } else {
                notecache.push(format!(
                    "{field}: {}",
                    process_field(doc, node, false, false)
                ));
            }
        } else if field == "titles" || field == "periodical" || field == "alt-periodical" {
            // :570-584
            for sub in doc.element_children(node) {
                let subfield = doc.tag_name(sub);
                match get_field(&subfield, item_type) {
                    Some(z) => {
                        if field_is_valid_for_type(z, item_type) {
                            item.set(z, process_field(doc, sub, false, false));
                        } else {
                            notecache.push(format!(
                                "{field}: {}",
                                process_field(doc, sub, false, false)
                            ));
                        }
                    }
                    None => notecache.push(format!(
                        "{subfield}: {}",
                        process_field(doc, sub, false, false)
                    )),
                }
            }
        } else if field == "contributors" {
            // :585-601
            for sub in doc.element_children(node) {
                let subfield = doc.tag_name(sub);
                match get_field(&subfield, item_type) {
                    Some(authortype) => {
                        for a in doc.get_elements_by_tag_name(sub, "author") {
                            item.creators.push(creator(&doc.text(a), authortype, true));
                        }
                    }
                    None => notecache.push(format!(
                        "{subfield}: {}",
                        process_field(doc, sub, false, false)
                    )),
                }
            }
        } else if field == "dates" {
            // :602-618
            let date = doc.get_elements_by_tag_name(node, "pub-dates");
            let year = doc.get_elements_by_tag_name(node, "year");
            if !date.is_empty() && !year.is_empty() {
                let d = doc
                    .get_elements_by_tag_name(date[0], "date")
                    .first()
                    .copied()
                    .ok_or_else(|| {
                        type_error("Cannot read properties of undefined (reading 'textContent')")
                    })?;
                let d = js::trim(&doc.text(d)).to_owned();
                let y = js::trim(&doc.text(year[0])).to_owned();
                let has_year = d
                    .as_bytes()
                    .windows(4)
                    .any(|w| w.iter().all(u8::is_ascii_digit));
                item.set("date", if has_year { d } else { format!("{d} {y}") });
            } else if !date.is_empty() {
                let first = doc.first_child(date[0]).ok_or_else(|| {
                    type_error("Cannot read properties of null (reading 'textContent')")
                })?;
                item.set("date", doc.text(first));
            } else if !year.is_empty() {
                item.set("date", doc.text(year[0]));
            } else if !js::trim(&doc.text(node)).is_empty() {
                notecache.push(format!("copyright-dates: {}", doc.text(node)));
            }
        } else if field == "notes" || field == "research-notes" {
            // :620-631
            item.notes
                .push(TranslatorNote::new(note_html(&process_field(
                    doc, node, true, true,
                ))));
        } else if field == "keywords" {
            // :632-636
            for sub in doc.element_children(node) {
                item.tags.push(TranslatorTag::new(js::trim(&doc.text(sub))));
            }
        } else if field == "urls" {
            // :637-670
            for sub in doc.element_children(node) {
                let attachment_type = match doc.tag_name(sub).as_str() {
                    "text-urls" | "related-urls" => "text/html",
                    "web-urls" => "url",
                    "pdf-urls" => "application/pdf",
                    _ => "",
                };
                for leaf in doc.element_children(sub) {
                    let filepath = doc.text(leaf);
                    if filepath.is_empty() {
                        continue;
                    }
                    let filepath = strip_internal_pdf(&filepath);
                    let filepath = js::trim(&filepath).to_owned();
                    let filename = strip_ext(&strip_dir(&filepath));
                    if attachment_type == "url" {
                        item.set("url", doc.text(sub));
                    } else {
                        let mut a = JsObject::new();
                        a.set("title", filename);
                        a.set("path", filepath);
                        a.set("mimeType", attachment_type);
                        item.attachments.push(a);
                    }
                }
            }
        } else if (field.contains("custom2")
            || field.contains("custom3")
            || field.contains("custom7"))
            && matches!(item_type, "book" | "bookSection" | "journalArticle")
        {
            // :671-677
            if let Some(m) = find_pmc(&doc.text(node)) {
                let extra = item
                    .get_string("extra")
                    .filter(|s| !s.is_empty())
                    .unwrap_or_default();
                item.set("extra", format!("{extra}PMCID: {m}\n"));
            }
        } else if field == "label" {
            // :678-680
            let extra = extra_or_empty(&item);
            item.set(
                "extra",
                format!("{extra}Citation Key: {}\n", doc.text(node)),
            );
        } else if matches!(
            field.as_str(),
            "database" | "source-app" | "rec-number" | "ref-type" | "foreign-keys"
        ) {
            // skipped (:681-684)
        } else {
            notecache.push(format!(
                "{field}: {}",
                process_field(doc, node, false, false)
            ));
        }
    }
    if !notecache.is_empty() {
        // :687-689
        let mut props = JsObject::new();
        props.set(
            "tags",
            Value::Array(vec![Value::String("_EndnoteXML import".into())]),
        );
        item.notes.push(TranslatorNote {
            note: format!(
                "The following values have no corresponding Zotero field:<br/>{}",
                notecache.join("<br/>")
            ),
            props,
        });
    }
    Ok(item)
}

/// `(newItem.extra || '')`.
fn extra_or_empty(item: &TranslatorItem) -> String {
    if item.truthy("extra") {
        item.get_string("extra").unwrap_or_default()
    } else {
        String::new()
    }
}

/// `filepath.replace(/^internal-pdf:\/\//i, 'PDF/')`.
fn strip_internal_pdf(s: &str) -> String {
    const P: &str = "internal-pdf://";
    if s.len() >= P.len() && s.is_char_boundary(P.len()) && s[..P.len()].eq_ignore_ascii_case(P) {
        format!("PDF/{}", &s[P.len()..])
    } else {
        s.to_owned()
    }
}

/// `text.match(/PMC\d+/i)[0]`.
fn find_pmc(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut i = 0;
    while i + 3 < b.len() {
        if b[i..i + 3].eq_ignore_ascii_case(b"PMC") && b[i + 3].is_ascii_digit() {
            let mut e = i + 3;
            while e < b.len() && b[e].is_ascii_digit() {
                e += 1;
            }
            return Some(s[i..e].to_owned());
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_paragraphs_and_paths() {
        assert_eq!(note_html("a\n\nb\nc"), "<p>a</p><p>b<br/>c</p>");
        assert_eq!(note_html("a\r\nb"), "<p>a<br/>b</p>");
        assert_eq!(strip_dir("PDF/x/y.pdf"), "y.pdf");
        assert_eq!(strip_ext("y.tar.gz"), "y.tar");
        assert_eq!(strip_ext("y."), "y.");
        assert_eq!(find_pmc("see pmc123 x").as_deref(), Some("pmc123"));
    }
}
