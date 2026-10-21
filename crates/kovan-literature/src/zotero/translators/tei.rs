// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "TEI.js" (translatorID
//   032ae9b7-ab90-9205-a479-baf81f49184a, lastUpdated 2026-05-20 17:56:18):
//   `replaceFormatting` :71-94, `genXMLId` :96-166, `generateItem`
//   :168-523, `generateCollection` :525-547, `generateTEIDocument` :549-558,
//   `doExport` :560-646; Zotero utilities (commit 4051881d59c6)
//   utilities.js `cleanTags` :373-381.
// Copyright (C) 2010 Stefan Majewski <xml@stefanmajewski.eu>.
// Licence: AGPL-3.0 (upstream: GPL-3.0-or-later, which may be combined with
//   AGPL-3.0; this port is distributed under AGPL-3.0 as part of kovan).

//! The TEI translator (export): a TEI P5 `listBibl` of `biblStruct`s.
//!
//! Collections: the translation-server's item getter answers
//! `Zotero.nextCollection()` with `false` (translate_item.js:80-82), so the
//! "Export Collections" path never runs there; [`ExportContext`] has no
//! collections and this port takes the same branch (`generateCollection`
//! is not ported: no input can reach it).

use crate::zotero::framework::html::unescape_html;
use crate::zotero::framework::js;
use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::xml::{NodeId, XmlDocument, XML_NS};
use crate::zotero::framework::{ExportContext, TranslateError, TranslatorItem};
use kovan_common::zotero::date::str_to_date;
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "032ae9b7-ab90-9205-a479-baf81f49184a",
    label: "TEI",
    creator: "Stefan Majewski",
    target: "xml",
    min_version: "4.0.27",
    priority: 25,
    translator_type: translator_type::EXPORT,
    config_options: &[
        ("dataMode", HeaderValue::Str("xml/dom")),
        ("getCollections", HeaderValue::Str("true")),
    ],
    display_options: &[
        ("exportNotes", HeaderValue::Bool(false)),
        ("Export Tags", HeaderValue::Bool(false)),
        ("Generate XML IDs", HeaderValue::Bool(true)),
        ("Full TEI Document", HeaderValue::Bool(false)),
        ("Export Collections", HeaderValue::Bool(false)),
    ],
    hidden_prefs: &[],
    last_updated: "2026-05-20 17:56:18",
};

const TEI_NS: &str = "http://www.tei-c.org/ns/1.0";

fn re(cell: &'static OnceLock<Regex>, pattern: impl FnOnce() -> String) -> &'static Regex {
    cell.get_or_init(|| Regex::new(&pattern()).expect("static regex compiles"))
}

/// `replaceFormatting(title)` (:71-94).
fn replace_formatting(title: &str) -> String {
    static SC_SPAN: OnceLock<Regex> = OnceLock::new();
    static NOCASE: OnceLock<Regex> = OnceLock::new();
    let t = title
        .replace("<i>", "<hi rend=\"italics\">")
        .replace("</i>", "</hi>")
        .replace("<b>", "<hi rend=\"bold\">")
        .replace("</b>", "</hi>")
        .replace("<sub>", "<hi rend=\"sub\">")
        .replace("</sub>", "</hi>")
        .replace("<sup>", "<hi rend=\"sup\">")
        .replace("</sup>", "</hi>");
    let t = re(&SC_SPAN, || {
        format!(
            "<span style=\"font-variant:{}*small-caps;\">([^\\n\\r\\x{{2028}}\\x{{2029}}]*?)</span>",
            js::WS
        )
    })
    .replace_all(&t, "<hi rend=\"smallcaps\">$1</hi>")
    .into_owned();
    let t = t
        .replace("<sc>", "<hi rend=\"smallcaps\">")
        .replace("</sc>", "</hi>");
    re(&NOCASE, || {
        "<span class=\"nocase\">([^\\n\\r\\x{2028}\\x{2029}]*?)</span>".to_owned()
    })
    .replace_all(&t, "<hi rend=\"nocase\">$1</hi>")
    .into_owned()
}

/// Whether a UTF-16 code unit is in genXMLId's character classes as
/// JavaScript (non-Unicode mode) reads them: `က0-F` there is
/// `က`, the range `0`-`` and `F`, so everything from U+0030 to
/// U+EFFF passes (checked with node, 2026-10-07). Astral characters are
/// surrogate pairs, both halves inside that range.
fn id_char_ok(c: char, name_char: bool) -> bool {
    let u = c as u32;
    if u > 0xFFFF {
        return true;
    }
    if name_char && (c == '-' || c == '.') {
        return true;
    }
    if u < 0x30 {
        return matches!(c, 'A'..='Z' | '_' | 'a'..='z');
    }
    !((0xF000..=0xF8FF).contains(&u) || (0xFDD0..=0xFDEF).contains(&u) || u >= 0xFFFE)
}

/// The state `genXMLId` and `generateItem` share across items.
struct State {
    /// `exportedXMLIds`: id -> its `biblStruct`, in insertion order.
    exported: Vec<(String, NodeId)>,
    /// `generatedItems`: uri -> `biblStruct`.
    generated: Vec<(String, NodeId)>,
    generate_ids: bool,
    export_notes: bool,
    export_tags: bool,
}

impl State {
    fn exported(&self, id: &str) -> Option<NodeId> {
        self.exported.iter().find(|(k, _)| k == id).map(|(_, v)| *v)
    }

    fn set_exported(&mut self, id: String, n: NodeId) {
        match self.exported.iter_mut().find(|(k, _)| *k == id) {
            Some(e) => e.1 = n,
            None => self.exported.push((id, n)),
        }
    }
}

/// `item.uri` as JavaScript would make it a string (`undefined` when the
/// item had no key).
fn uri(item: &TranslatorItem) -> String {
    match item.get("uri") {
        None | Some(Value::Null) => "undefined".into(),
        Some(v) => js::to_js_string(v),
    }
}

/// `item[key]` when truthy, as a string.
fn prop(item: &TranslatorItem, key: &str) -> Option<String> {
    item.get(key)
        .filter(|v| js::truthy(Some(v)))
        .map(js::to_js_string)
}

/// `genXMLId(item)` (:96-166).
fn gen_xml_id(
    item: &mut TranslatorItem,
    doc: &mut XmlDocument,
    st: &mut State,
    ctx: &ExportContext,
) -> Result<String, TranslateError> {
    static KEY: OnceLock<Regex> = OnceLock::new();
    static PUNCT: OnceLock<Regex> = OnceLock::new();
    // Better BibTeX citation key in Extra (:98-104).
    if let Some(extra) = prop(item, "extra") {
        let r = re(&KEY, || {
            format!(
                "(?:^|\\n)[cC][iI][tT][aA][tT][iI][oO][nN] [kK][eE][yY]{ws}*:{ws}*({nws}+)(?:\\n|$)",
                ws = js::WS,
                nws = js::NOT_WS
            )
        });
        let mut key = None;
        let replaced = r
            .replacen(&extra, 1, |c: &regex::Captures| {
                key = Some(c[1].to_owned());
                "\n"
            })
            .into_owned();
        if let Some(k) = key {
            item.set("citationKey", k);
        }
        item.set("extra", js::trim(&replaced).to_owned());
    }
    if let Some(k) = prop(item, "citationKey") {
        return Ok(k);
    }

    let mut xmlid = String::new();
    let first = item.creators.first();
    let first_last = first
        .and_then(|c| c.last_name.clone())
        .filter(|s| !s.is_empty());
    let first_name = first
        .and_then(|c| c.other.get("name"))
        .filter(|v| js::truthy(Some(v)))
        .map(js::to_js_string);
    if first_last.is_some() || first_name.is_some() {
        if let Some(l) = first_last {
            xmlid = l;
        }
        if let Some(n) = first_name {
            xmlid = n;
        }
        if let Some(d) = prop(item, "date") {
            let date = str_to_date(&d, &ctx.options.env.dates);
            if let Some(y) = date.year.filter(|y| !y.is_empty()) {
                xmlid.push_str(&y);
            }
        }
        // :118: spaces, tabs, colon, punctuation, parentheses, apostrophes.
        let r = re(&PUNCT, || {
            "(?:[ \\t\\[\\]:\\x{AD}\\x{21}-\\x{2C}\\x{2010}-\\x{2021}])+".to_owned()
        });
        xmlid = r.replace_all(&xmlid, "_").into_owned();
        // :132-133: a first character that cannot start an NCName, then any
        // character that cannot be in one.
        if let Some(c) = xmlid.chars().next() {
            if !id_char_ok(c, false) {
                xmlid = xmlid[c.len_utf8()..].to_owned();
            }
        }
        xmlid = xmlid.chars().filter(|&c| id_char_ok(c, true)).collect();
    } else {
        // :137-140: `item.uri` undefined throws.
        if matches!(item.get("uri"), None | Some(Value::Null)) {
            return Err(TranslateError::Translator(
                "TypeError: Cannot read properties of undefined (reading 'lastIndexOf')".into(),
            ));
        }
        let s = uri(item);
        let result = match s.rfind('/') {
            Some(n) => s[n + 1..].to_owned(),
            None => s,
        };
        xmlid.push_str("zoteroItem_");
        xmlid.push_str(&result);
    }
    // :142-162: make the id unique.
    let mut cur = xmlid.clone();
    if let Some(prev) = st.exported(&cur) {
        let first_id = format!("{xmlid}a");
        if st.exported(&first_id).is_none() {
            doc.set_attribute_ns(prev, Some(XML_NS), "xml:id", &first_id);
            st.set_exported(first_id, prev);
        }
        let (a, z) = (97u32, 122u32);
        let mut i = a + 1;
        while st.exported(&cur).is_some() {
            cur = format!("{xmlid}{}", char::from_u32(i).expect("ascii"));
            if i == z {
                i = a;
                xmlid.push('a');
            }
            i += 1;
        }
        xmlid = cur;
    }
    Ok(xmlid)
}

fn el(doc: &mut XmlDocument, name: &str) -> NodeId {
    doc.create_element_ns(Some(TEI_NS), name)
}

fn text_el(doc: &mut XmlDocument, name: &str, attrs: &[(&str, &str)], text: &str) -> NodeId {
    let e = el(doc, name);
    for (k, v) in attrs {
        doc.set_attribute(e, k, v);
    }
    let t = doc.create_text_node(text);
    doc.append_child(e, t);
    e
}

/// `Zotero.Utilities.cleanTags(x)` (utilities.js:373-381).
fn clean_tags(x: &str) -> String {
    static BR: OnceLock<Regex> = OnceLock::new();
    static P: OnceLock<Regex> = OnceLock::new();
    static TAG: OnceLock<Regex> = OnceLock::new();
    let x = re(&BR, || "(?i)<br[^>]*>".into()).replace_all(x, "\n");
    let x = re(&P, || "(?i)</p>".into()).replace_all(&x, "\n\n");
    re(&TAG, || "<[^>]+>".into())
        .replace_all(&x, "")
        .into_owned()
}

/// `generateItem(item, teiDoc)` (:168-523).
fn generate_item(
    item: &mut TranslatorItem,
    doc: &mut XmlDocument,
    st: &mut State,
    ctx: &ExportContext,
) -> Result<NodeId, TranslateError> {
    let is_analytic = matches!(
        item.item_type.as_str(),
        "journalArticle"
            | "bookSection"
            | "magazineArticle"
            | "newspaperArticle"
            | "conferencePaper"
            | "encyclopediaArticle"
            | "dictionaryEntry"
            | "webpage"
    );
    let bibl = el(doc, "biblStruct");
    let t = item.item_type.clone();
    doc.set_attribute(bibl, "type", &t);
    let item_uri = uri(item);

    if st.generate_ids {
        match st
            .generated
            .iter()
            .find(|(k, _)| *k == item_uri)
            .map(|(_, v)| *v)
        {
            None => {
                let xmlid = gen_xml_id(item, doc, st, ctx)?;
                doc.set_attribute_ns(bibl, Some(XML_NS), "xml:id", &xmlid);
                st.set_exported(xmlid, bibl);
            }
            Some(prev) => {
                let xmlid = format!(
                    "#{}",
                    doc.get_attribute_ns(prev, Some(XML_NS), "id").unwrap_or("")
                );
                let my = format!("zoteroItem_{item_uri}");
                doc.set_attribute(bibl, "sameAs", &xmlid);
                doc.set_attribute_ns(bibl, Some(XML_NS), "xml:id", &my);
                st.set_exported(my, bibl);
            }
        }
        doc.set_attribute(bibl, "corresp", &item_uri);
    }
    match st.generated.iter_mut().find(|(k, _)| *k == item_uri) {
        Some(e) => e.1 = bibl,
        None => st.generated.push((item_uri.clone(), bibl)),
    }

    let monogr = el(doc, "monogr");
    let mut analytic = None;
    let mut series = None;
    if is_analytic {
        let a = el(doc, "analytic");
        analytic = Some(a);
        doc.append_child(bibl, a);
        doc.append_child(bibl, monogr);
        let at = el(doc, "title");
        doc.set_attribute(at, "level", "a");
        doc.append_child(a, at);
        if let Some(title) = prop(item, "title") {
            let tn = doc.create_text_node(&replace_formatting(&title));
            doc.append_child(at, tn);
        }
        if let Some(doi) = prop(item, "DOI") {
            let idno = text_el(doc, "idno", &[("type", "DOI")], &doi);
            doc.append_child(a, idno);
        }
        let pub_title = [
            "bookTitle",
            "proceedingsTitle",
            "encyclopediaTitle",
            "dictionaryTitle",
            "publicationTitle",
            "websiteTitle",
        ]
        .iter()
        .find_map(|k| prop(item, k));
        if let Some(p) = pub_title {
            let level = if t == "journalArticle" { "j" } else { "m" };
            let pt = text_el(doc, "title", &[("level", level)], &replace_formatting(&p));
            doc.append_child(monogr, pt);
        }
        if let Some(s) = prop(item, "shortTitle") {
            let st_el = text_el(doc, "title", &[("type", "short")], &s);
            doc.append_child(a, st_el);
        }
    } else {
        doc.append_child(bibl, monogr);
        if let Some(title) = prop(item, "title") {
            let te = text_el(doc, "title", &[("level", "m")], &replace_formatting(&title));
            doc.append_child(monogr, te);
        } else if !item.truthy("conferenceName") {
            let te = el(doc, "title");
            doc.append_child(monogr, te);
        }
        if let Some(s) = prop(item, "shortTitle") {
            let st_el = text_el(doc, "title", &[("type", "short")], &s);
            doc.append_child(monogr, st_el);
        }
        if let Some(doi) = prop(item, "DOI") {
            let idno = text_el(doc, "idno", &[("type", "DOI")], &doi);
            doc.append_child(monogr, idno);
        }
    }

    if let Some(c) = prop(item, "conferenceName") {
        let e = text_el(
            doc,
            "title",
            &[("type", "conferenceName")],
            &replace_formatting(&c),
        );
        doc.append_child(monogr, e);
    }

    if item.truthy("series") || item.truthy("seriesTitle") {
        let s = el(doc, "series");
        series = Some(s);
        doc.append_child(bibl, s);
        if let Some(v) = prop(item, "series") {
            let e = text_el(doc, "title", &[("level", "s")], &replace_formatting(&v));
            doc.append_child(s, e);
        }
        if let Some(v) = prop(item, "seriesTitle") {
            let e = text_el(
                doc,
                "title",
                &[("level", "s"), ("type", "alternative")],
                &replace_formatting(&v),
            );
            doc.append_child(s, e);
        }
        if let Some(v) = prop(item, "seriesText") {
            let e = text_el(doc, "note", &[("type", "description")], &v);
            doc.append_child(s, e);
        }
        if let Some(v) = prop(item, "seriesNumber") {
            let e = text_el(doc, "biblScope", &[("unit", "volume")], &v);
            doc.append_child(s, e);
        }
    }

    for (key, ty) in [
        ("ISBN", "ISBN"),
        ("ISSN", "ISSN"),
        ("callNumber", "callNumber"),
    ] {
        if let Some(v) = prop(item, key) {
            let e = text_el(doc, "idno", &[("type", ty)], &v);
            doc.append_child(monogr, e);
        }
    }
    if let Some(v) = prop(item, "numberOfVolumes") {
        let e = text_el(doc, "extent", &[], &v);
        doc.append_child(monogr, e);
    }

    // Creators (:362-427).
    for creator in &item.creators {
        let ty = creator
            .creator_type
            .clone()
            .unwrap_or_else(|| "undefined".into());
        let mut resp_stmt = None;
        let cur = match ty.as_str() {
            "author" | "bookAuthor" => el(doc, "author"),
            "editor" | "seriesEditor" => el(doc, "editor"),
            _ => {
                let rs = el(doc, "respStmt");
                let resp = text_el(doc, "resp", &[], &ty);
                doc.append_child(rs, resp);
                let pn = el(doc, "persName");
                doc.append_child(rs, pn);
                resp_stmt = Some(rs);
                pn
            }
        };
        let first = creator.first_name.clone().filter(|s| !s.is_empty());
        if let Some(f) = &first {
            let e = text_el(doc, "forename", &[], f);
            doc.append_child(cur, e);
        }
        if let Some(l) = creator.last_name.clone().filter(|s| !s.is_empty()) {
            let name = if first.is_some() { "surname" } else { "name" };
            let e = text_el(doc, name, &[], &l);
            doc.append_child(cur, e);
        }
        if let Some(n) = creator
            .other
            .get("name")
            .filter(|v| js::truthy(Some(v)))
            .map(js::to_js_string)
        {
            let e = text_el(doc, "name", &[], &n);
            doc.append_child(cur, e);
        }
        let node = resp_stmt.unwrap_or(cur);
        if ty == "seriesEditor" && series.is_some() {
            doc.append_child(series.expect("checked"), node);
        } else if is_analytic && ty != "editor" && ty != "bookAuthor" {
            doc.append_child(analytic.expect("analytic item"), node);
        } else {
            doc.append_child(monogr, node);
        }
    }

    if let Some(v) = prop(item, "edition").or_else(|| prop(item, "versionNumber")) {
        let e = text_el(doc, "edition", &[], &v);
        doc.append_child(monogr, e);
    }

    // The imprint (:440-500).
    let imprint = el(doc, "imprint");
    doc.append_child(monogr, imprint);
    if let Some(v) = prop(item, "place") {
        let e = text_el(doc, "pubPlace", &[], &v);
        doc.append_child(imprint, e);
    }
    for (key, unit) in [
        ("volume", "volume"),
        ("issue", "issue"),
        ("section", "chapter"),
        ("pages", "page"),
    ] {
        if let Some(v) = prop(item, key) {
            let e = text_el(doc, "biblScope", &[("unit", unit)], &v);
            doc.append_child(imprint, e);
        }
    }
    if let Some(v) = prop(item, "publisher") {
        let e = text_el(doc, "publisher", &[], &v);
        doc.append_child(imprint, e);
    }
    if let Some(d) = prop(item, "date") {
        let date = str_to_date(&d, &ctx.options.env.dates);
        let text = date.year.filter(|y| !y.is_empty()).unwrap_or(d);
        let e = text_el(doc, "date", &[], &text);
        doc.append_child(imprint, e);
    } else {
        let e = el(doc, "date");
        doc.append_child(imprint, e);
    }
    for (key, ty) in [
        ("accessDate", "accessed"),
        ("url", "url"),
        ("thesisType", "thesisType"),
    ] {
        if let Some(v) = prop(item, key) {
            let e = text_el(doc, "note", &[("type", ty)], &v);
            doc.append_child(imprint, e);
        }
    }

    // Notes (:502-512).
    if st.export_notes {
        for n in &item.notes {
            let text = unescape_html(&clean_tags(&n.note));
            let e = text_el(doc, "note", &[], &text);
            doc.append_child(bibl, e);
        }
    }
    // Tags (:514-524).
    if st.export_tags && !item.tags.is_empty() {
        let tags = el(doc, "note");
        doc.set_attribute(tags, "type", "tags");
        for tag in &item.tags {
            let e = text_el(doc, "note", &[("type", "tag")], &tag.tag);
            doc.append_child(tags, e);
        }
        doc.append_child(bibl, tags);
    }
    Ok(bibl)
}

/// Whether a property name is an array index (`ToString(ToUint32(P)) == P`,
/// below 2^32 - 1): JavaScript enumerates those keys first, ascending.
fn array_index(k: &str) -> Option<u32> {
    let n: u32 = k.parse().ok()?;
    (n != u32::MAX && n.to_string() == k).then_some(n)
}

/// `doExport` (:560-646).
pub fn do_export(ctx: &mut ExportContext) -> Result<(), TranslateError> {
    let mut doc = XmlDocument::parse_from_string(
        "<TEI xmlns=\"http://www.tei-c.org/ns/1.0\"><teiHeader><fileDesc><titleStmt><title>Exported from Zotero</title></titleStmt><publicationStmt><p>unpublished</p></publicationStmt><sourceDesc><p>Generated from Zotero database</p></sourceDesc></fileDesc></teiHeader></TEI>",
    );
    // `allItems[item.uri] = item` (:575-581): a JS object, so a repeated uri
    // keeps its first position and its last item.
    let mut all: Vec<(String, TranslatorItem)> = Vec::new();
    while let Some(item) = ctx.next_item() {
        if item.item_type == "note" {
            continue;
        }
        let u = uri(&item);
        match all.iter_mut().find(|(k, _)| *k == u) {
            Some(e) => e.1 = item,
            None => all.push((u, item)),
        }
    }
    // `for (let i in allItems)`: integer-like keys first, ascending.
    let mut order: Vec<usize> = (0..all.len()).collect();
    order.sort_by_key(|&i| match array_index(&all[i].0) {
        Some(n) => (0, n, i),
        None => (1, 0, i),
    });

    let mut st = State {
        exported: Vec::new(),
        generated: Vec::new(),
        generate_ids: ctx.options.option_truthy("Generate XML IDs"),
        export_notes: ctx.options.option_truthy("exportNotes"),
        export_tags: ctx.options.option_truthy("Export Tags"),
    };
    // `Zotero.nextCollection()` is false on the translation-server, so the
    // collection branch (:584-596) is never taken.
    let list_bibl = el(&mut doc, "listBibl");
    for i in order {
        let mut item = all[i].1.clone();
        if item.item_type == "attachment" {
            continue;
        }
        let b = generate_item(&mut item, &mut doc, &mut st, ctx)?;
        doc.append_child(list_bibl, b);
    }
    let output = if ctx.options.option_truthy("Full TEI Document") {
        // `generateTEIDocument` (:549-558).
        let text = el(&mut doc, "text");
        let body = el(&mut doc, "body");
        let root = doc.document_element().expect("TEI root");
        doc.append_child(root, text);
        doc.append_child(text, body);
        doc.append_child(body, list_bibl);
        doc.document()
    } else {
        list_bibl
    };
    ctx.write("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let s = doc.serialize(output);
    ctx.write(&s);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatting_and_ids() {
        assert_eq!(
            replace_formatting("<i>a</i> <span class=\"nocase\">B</span>"),
            "<hi rend=\"italics\">a</hi> <hi rend=\"nocase\">B</hi>"
        );
        assert!(!id_char_ok('/', true));
        assert!(id_char_ok('-', true));
        assert!(!id_char_ok('-', false));
        assert!(id_char_ok('×', false));
        assert!(!id_char_ok('\u{F000}', true));
        assert_eq!(clean_tags("a<br/>b</p><x>c"), "a\nb\n\nc");
    }
}
