// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): ReferBibIX.js (translatorID
//   881f60f2-0802-411a-9228-ce5f47b64c7d, lastUpdated 2023-10-27 09:03:42):
//   header :1-15, `detectImport` :17-36, `fieldMap` :38-56, `inputFieldMap`
//   :58-62, `typeMap` :65-100, `inputTypeMap` :104-124, `processTag`
//   :126-209, `doImport` :211-256, `addTag` :258-262, `doExport` :264-313.
// Copyright: no notice upstream; translator by Simon Kornblith (header
//   `creator`).
// Licence: AGPL-3.0 (no licence text upstream; treated as AGPLv3 as part of
//   Zotero, maintainer decision 2026-10-07, #747).

//! The Refer/BibIX translator: import and export.
//!
//! Every routine of the translator is ported. One upstream behaviour worth
//! knowing: an input with no `%` line makes `doImport` throw (`false.replace`
//! on the end of input, :216), reported here as
//! [`TranslateError::Translator`].
//!
//! **Maturity: AI draft (1).** Verified code-to-code against upstream
//! (`tests/zotero_translators.rs`, `refer_*`).

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::utilities::clean_author;
use crate::zotero::framework::{
    js, ExportContext, ImportContext, TranslateError, TranslatorCreator, TranslatorItem,
    TranslatorNote, TranslatorTag,
};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "881f60f2-0802-411a-9228-ce5f47b64c7d",
    label: "Refer/BibIX",
    creator: "Simon Kornblith",
    target: "txt",
    min_version: "2.1",
    priority: 100,
    translator_type: translator_type::IMPORT | translator_type::EXPORT,
    config_options: &[],
    display_options: &[("exportCharset", HeaderValue::Str("UTF-8"))],
    hidden_prefs: &[],
    last_updated: "2023-10-27 09:03:42",
};

/// `fieldMap` (:38-56) in `for...in` order: the integer-like key "7" first,
/// then the others in insertion order.
const FIELD_MAP: [(&str, &str); 17] = [
    ("7", "edition"),
    ("T", "title"),
    ("S", "series"),
    ("V", "volume"),
    ("N", "issue"),
    ("C", "place"),
    ("I", "publisher"),
    ("R", "type"),
    ("P", "pages"),
    ("W", "archiveLocation"),
    ("*", "rights"),
    ("@", "ISBN"),
    ("L", "callNumber"),
    ("M", "accessionNumber"),
    ("U", "url"),
    ("X", "abstractNote"),
    ("G", "language"),
];

/// `inputFieldMap` (:58-62).
const INPUT_FIELD_MAP: [(&str, &str); 3] = [
    ("J", "publicationTitle"),
    ("B", "publicationTitle"),
    ("9", "type"),
];

/// `typeMap` (:65-100), in order.
const TYPE_MAP: [(&str, &str); 34] = [
    ("book", "Book"),
    ("bookSection", "Book Section"),
    ("journalArticle", "Journal Article"),
    ("magazineArticle", "Magazine Article"),
    ("newspaperArticle", "Newspaper Article"),
    ("thesis", "Thesis"),
    ("letter", "Personal Communication"),
    ("manuscript", "Unpublished Work"),
    ("interview", "Personal Communication"),
    ("film", "Film or Broadcast"),
    ("artwork", "Artwork"),
    ("webpage", "Web Page"),
    ("report", "Report"),
    ("bill", "Bill"),
    ("case", "Case"),
    ("hearing", "Hearing"),
    ("patent", "Patent"),
    ("statute", "Statute"),
    ("email", "Personal Communication"),
    ("map", "Map"),
    ("blogPost", "Web Page"),
    ("instantMessage", "Personal Communication"),
    ("forumPost", "Web Page"),
    ("audioRecording", "Audiovisual Material"),
    ("presentation", "Report"),
    ("videoRecording", "Audiovisual Material"),
    ("tvBroadcast", "Film or Broadcast"),
    ("radioBroadcast", "Film or Broadcast"),
    ("podcast", "Audiovisual Material"),
    ("computerProgram", "Computer Program"),
    ("conferencePaper", "Conference Paper"),
    ("document", "Generic"),
    ("encyclopediaArticle", "Encyclopedia"),
    ("dictionaryEntry", "Dictionary"),
];

/// `inputTypeMap` (:104-124).
const INPUT_TYPE_MAP: [(&str, &str); 19] = [
    ("Ancient Text", "book"),
    ("Audio", "audioRecording"),
    ("Audiovisual Material", "videoRecording"),
    ("Generic", "book"),
    ("Chart or Table", "artwork"),
    ("Classical Work", "book"),
    ("Conference Proceedings", "conferencePaper"),
    ("Conference Paper", "conferencePaper"),
    ("Edited Book", "book"),
    ("Electronic Article", "journalArticle"),
    ("Electronic Book", "book"),
    ("Equation", "artwork"),
    ("Figure", "artwork"),
    ("Government Document", "document"),
    ("Grant", "document"),
    ("Legal Rule or Regulation", "statute"),
    ("Online Database", "webpage"),
    ("Online Multimedia", "webpage"),
    ("Electronic Source", "webpage"),
];

fn lookup(map: &[(&'static str, &'static str)], key: &str) -> Option<&'static str> {
    map.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

/// `line.replace(/^\s+/, "")`.
fn strip_leading(line: &str) -> &str {
    js::trim_start(line)
}

/// `/%[A-Z0-9*$] .+/.test(line)` (not anchored; `.` stops at line
/// terminators).
fn is_refer_line(line: &str) -> bool {
    let c: Vec<char> = line.chars().collect();
    (0..c.len()).any(|i| {
        c[i] == '%'
            && c.get(i + 1).is_some_and(|t| {
                t.is_ascii_uppercase() || t.is_ascii_digit() || *t == '*' || *t == '$'
            })
            && c.get(i + 2) == Some(&' ')
            && c.get(i + 3)
                .is_some_and(|x| !matches!(x, '\n' | '\r' | '\u{2028}' | '\u{2029}'))
    })
}

/// `detectImport` (:17-36): two Refer lines before any other non-blank
/// line.
pub fn detect_import(ctx: &mut ImportContext) -> bool {
    let mut matched = 0;
    while let Some(line) = ctx.read_line() {
        let line = strip_leading(&line);
        if !line.is_empty() {
            if is_refer_line(line) {
                matched += 1;
                if matched == 2 {
                    return true;
                }
            } else {
                return false;
            }
        }
    }
    false
}

/// `processTag(item, tag, value)` (:126-209).
fn process_tag(item: &mut TranslatorItem, tag: &str, value: &str) {
    let value = js::trim(value).to_owned();
    if let Some(field) = lookup(&FIELD_MAP, tag) {
        match item
            .get_string(field)
            .filter(|v| item.truthy(field) && !v.is_empty())
        {
            Some(cur) => item.set(field, format!("{cur}, {value}")),
            None => item.set(field, value),
        }
    } else if let Some(field) = lookup(&INPUT_FIELD_MAP, tag) {
        item.set(field, value);
    } else if tag == "0" {
        if let Some(t) = lookup(&INPUT_TYPE_MAP, &value) {
            item.item_type = t.to_owned();
        } else if let Some((t, _)) = TYPE_MAP.iter().find(|(_, v)| *v == value) {
            item.item_type = (*t).to_owned();
        }
    } else if tag == "A" || tag == "E" || tag == "Y" {
        let ty = match tag {
            "A" => "author",
            "E" => "editor",
            _ => "translator",
        };
        let a = clean_author(&value, ty, value.contains(','));
        item.creators.push(TranslatorCreator {
            first_name: a.first_name,
            last_name: Some(a.last_name),
            creator_type: Some(a.creator_type),
            ..Default::default()
        });
    } else if tag == "Q" {
        // `{creatorType: "author", lastName: value, fieldMode: true}`;
        // `fieldMode == 1` holds for `true`.
        item.creators
            .push(TranslatorCreator::single(value, "author"));
    } else if tag == "H" || tag == "O" {
        let extra = match item.get_string("extra").filter(|_| item.truthy("extra")) {
            Some(e) => format!("{e}\n{value}"),
            None => value,
        };
        item.set("extra", extra);
    } else if tag == "Z" {
        item.notes.push(TranslatorNote::new(value));
    } else if tag == "D" {
        match item.get_string("date").filter(|_| item.truthy("date")) {
            Some(d) => {
                if !d.contains(&value) {
                    item.set("date", format!("{d} {value}"));
                }
            }
            None => item.set("date", value),
        }
    } else if tag == "8" {
        match item.get_string("date").filter(|_| item.truthy("date")) {
            Some(d) => {
                if !value.contains(&d) {
                    item.set("date", format!("{d} {value}"));
                }
            }
            None => item.set("date", value),
        }
    } else if tag == "K" {
        item.tags.extend(value.split('\n').map(TranslatorTag::new));
    }
}

/// `line[i]` as a one-character string (`None` is `undefined`).
fn char_at(line: &str, i: usize) -> Option<String> {
    line.chars().nth(i).map(String::from)
}

/// `doImport` (:211-256).
pub fn do_import(ctx: &mut ImportContext) -> Result<(), TranslateError> {
    // `do { line = Zotero.read(); line = line.replace(...) } while (line !== false && line[0] != "%")`
    let line = loop {
        let Some(l) = ctx.read_line() else {
            return Err(TranslateError::Translator(
                "TypeError: line.replace is not a function".into(),
            ));
        };
        let l = strip_leading(&l).to_owned();
        if l.starts_with('%') {
            break l;
        }
    };

    // Default to a book (`inputTypeMap.Generic`).
    let mut item = TranslatorItem::new("book");
    let mut tag: Option<String> = char_at(&line, 1);
    let mut data: String = js::substr(&line, 3, usize::MAX);
    while let Some(l) = ctx.read_line() {
        let line = strip_leading(&l).to_owned();
        if line.is_empty() {
            if let Some(t) = tag.take().filter(|t| !t.is_empty()) {
                process_tag(&mut item, &t, &data);
                data.clear();
                ctx.item_done(std::mem::replace(&mut item, TranslatorItem::new("book")));
            }
        } else if line.starts_with('%') && char_at(&line, 2).as_deref() == Some(" ") {
            if let Some(t) = tag.as_deref().filter(|t| !t.is_empty()) {
                process_tag(&mut item, t, &data);
            }
            tag = char_at(&line, 1);
            data = js::substr(&line, 3, usize::MAX);
        } else if tag.as_deref().is_some_and(|t| !t.is_empty()) {
            data.push('\n');
            data.push_str(&line);
        }
    }
    if let Some(t) = tag.filter(|t| !t.is_empty()) {
        process_tag(&mut item, &t, &data);
        ctx.item_done(item);
    }
    Ok(())
}

/// `addTag(tag, value)` (:258-262).
fn add_tag(ctx: &mut ExportContext, tag: &str, value: &str) {
    if !value.is_empty() {
        ctx.write(&format!("%{tag} {value}\r\n"));
    }
}

/// `item[field]` as a string when truthy.
fn truthy_string(item: &TranslatorItem, field: &str) -> Option<String> {
    item.truthy(field)
        .then(|| item.get(field).map(js::to_js_string))
        .flatten()
}

/// `doExport` (:264-313).
pub fn do_export(ctx: &mut ExportContext) -> Result<(), TranslateError> {
    while let Some(item) = ctx.next_item() {
        if item.item_type == "note" || item.item_type == "attachment" {
            continue;
        }
        add_tag(
            ctx,
            "0",
            lookup(&TYPE_MAP, &item.item_type).unwrap_or("Generic"),
        );
        for (tag, field) in FIELD_MAP {
            if let Some(v) = truthy_string(&item, field) {
                add_tag(ctx, tag, &v);
            }
        }
        if let Some(p) = truthy_string(&item, "publicationTitle") {
            let tag = if item.item_type == "journalArticle" {
                "J"
            } else {
                "B"
            };
            add_tag(ctx, tag, &p);
        }
        for c in &item.creators {
            let refer_tag = match c.creator_type.as_deref() {
                Some("editor") => "E",
                Some("translator") => "?",
                _ => "A",
            };
            let last = c
                .last_name
                .clone()
                .unwrap_or_else(|| "undefined".to_owned());
            let name = match c.first_name.as_deref().filter(|f| !f.is_empty()) {
                Some(f) => format!("{last}, {f}"),
                None => last,
            };
            add_tag(ctx, refer_tag, &name);
        }
        // `addTag("D", item.date)`: written when truthy.
        if let Some(d) = truthy_string(&item, "date") {
            add_tag(ctx, "D", &d);
        }
        let mut keyword_tag = String::new();
        for t in &item.tags {
            keyword_tag.push_str("\r\n");
            keyword_tag.push_str(&t.tag);
        }
        add_tag(ctx, "K", &js::substr(&keyword_tag, 2, usize::MAX));
        ctx.write("\r\n");
    }
    Ok(())
}
