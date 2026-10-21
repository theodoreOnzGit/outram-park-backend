// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "Endnote XML.js": `doExport` :699-866,
//   `mapProperty` :976-998, `convertZoteroMarkup` :1005-1116; Zotero
//   utilities (commit 4051881d59c6) utilities.js `arrayDiff` :846-863.
// Copyright (c) Sebastian Karcher (creator named in the header; the file
//   carries no copyright line).
// Licence: AGPL-3.0 (no licence text upstream; treated as AGPLv3 as part of
//   Zotero, maintainer decision 2026-10-07, #747).

//! Endnote XML export: a DOM built with `createElement` (no namespace) and
//! serialised with the translation-server's XMLSerializer.

use super::{export_item_type, export_ref_number, get_field, AUTHOR_FIELDS, FIELDS, TITLE_FIELDS};
use crate::zotero::framework::js;
use crate::zotero::framework::xml::{NodeId, XmlDocument};
use crate::zotero::framework::{ExportContext, TranslateError, TranslatorItem};
use kovan_common::zotero::date::str_to_date;
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

fn re(cell: &'static OnceLock<Regex>, pattern: impl FnOnce() -> String) -> &'static Regex {
    cell.get_or_init(|| Regex::new(&pattern()).expect("static regex compiles"))
}

/// `ZU.arrayDiff(a, b)`: the elements of `a` not in `b`.
fn array_diff(a: &[&'static str], b: &[&'static str]) -> Vec<&'static str> {
    a.iter().filter(|x| !b.contains(x)).copied().collect()
}

/// The Zotero-markup -> EndNote `face` map (:1007-1017).
fn markup_format(tag: &str) -> &'static [&'static str] {
    match tag {
        "I" | "EM" => &["italic"],
        "B" | "STRONG" => &["bold"],
        "SUP" => &["superscript"],
        "SUB" => &["subscript"],
        "U" => &["underline"],
        "SPAN" => &["span"],
        _ => &[],
    }
}

/// `createFormattedNode(str, format)` (:1020-1031).
fn formatted_node(doc: &mut XmlDocument, s: &str, format: &[&str]) -> NodeId {
    static NL3: OnceLock<Regex> = OnceLock::new();
    let node = doc.create_element("style");
    let s = re(&NL3, || r"\n{3,}".into()).replace_all(s, "\n\n");
    let face = if format.is_empty() {
        "normal".to_owned()
    } else {
        format.join(" ")
    };
    doc.set_attribute(node, "face", &face);
    let t = doc.create_text_node(&s);
    doc.append_child(node, t);
    node
}

/// `convertZoteroMarkup(str)` (:1033-1115): `style` elements, one per run of
/// equal formatting.
fn convert_zotero_markup(doc: &mut XmlDocument, s: &str) -> Vec<NodeId> {
    static BR: OnceLock<Regex> = OnceLock::new();
    static P_CLOSE: OnceLock<Regex> = OnceLock::new();
    static P_OPEN: OnceLock<Regex> = OnceLock::new();
    static TAG: OnceLock<Regex> = OnceLock::new();
    static STYLE: OnceLock<Regex> = OnceLock::new();
    static UNDERLINE: OnceLock<Regex> = OnceLock::new();
    let ws = js::WS;
    let s = re(&BR, || format!(r"(?i){ws}*<br{ws}*/?>{ws}*")).replace_all(s, "\n");
    let s = re(&P_CLOSE, || format!(r"(?i)(?:{ws}*</p>{ws}*)+")).replace_all(&s, "\n\n");
    let s = re(&P_OPEN, || {
        format!(r"(?i){ws}*<p(?:{ws}[^\n\r\x{{2028}}\x{{2029}}]*?)?>{ws}*")
    })
    .replace_all(&s, "\n\n");
    let s = js::trim(&s).to_owned();

    struct Open {
        tag: String,
        format: Vec<&'static str>,
    }
    let mut tags: Vec<Open> = Vec::new();
    let mut formatting: Vec<&'static str> = Vec::new();
    let mut current = String::new();
    let mut next_start = 0usize;
    let mut nodes = Vec::new();
    let tag_re = re(&TAG, || format!(r"(?i)<(/?)([A-Za-z0-9_]+)({ws}[^>]*)?>"));
    for m in tag_re.captures_iter(&s) {
        let whole = m.get(0).expect("match");
        let tag_name = m[2].to_uppercase();
        let old_formatting;
        let format_diff;
        if m[1].is_empty() {
            // Opening tag.
            let mut format: Vec<&'static str> = markup_format(&tag_name).to_vec();
            if tag_name == "SPAN" {
                if let Some(attrs) = m.get(3) {
                    let style = re(&STYLE, || format!(r"(?i)(?-u:\b)style{ws}*="));
                    if style.is_match(attrs.as_str()) {
                        let ul = re(&UNDERLINE, || {
                            format!("text-decoration{ws}*:[^'\";]*(?-u:\\b)underline(?-u:\\b)")
                        });
                        format = if ul.is_match(attrs.as_str()) {
                            vec!["underline"]
                        } else {
                            Vec::new()
                        };
                    }
                }
            }
            format_diff = array_diff(&format, &formatting);
            tags.push(Open {
                tag: tag_name,
                format: format_diff.clone(),
            });
            old_formatting = formatting.clone();
            formatting.extend(format_diff.iter().copied());
        } else {
            // Closing tag: skip one never opened.
            let Some(j) = tags.iter().rposition(|t| t.tag == tag_name) else {
                continue;
            };
            let mut diff = Vec::new();
            while tags.len() > j {
                let t = tags.pop().expect("non-empty");
                diff.extend(t.format);
            }
            format_diff = diff;
            old_formatting = formatting.clone();
            formatting = array_diff(&formatting, &format_diff);
        }
        if next_start < whole.start() {
            current.push_str(&s[next_start..whole.start()]);
        }
        next_start = whole.end();
        if !format_diff.is_empty() && !current.is_empty() {
            nodes.push(formatted_node(doc, &current, &old_formatting));
            current.clear();
        }
    }
    if next_start < s.len() {
        current.push_str(&s[next_start..]);
    }
    if !current.is_empty() {
        nodes.push(formatted_node(doc, &current, &formatting));
    }
    nodes
}

/// `mapProperty(parent, name, property, attributes)` (:976-998); `None` and
/// "" are upstream's falsy properties (no element).
fn map_property(
    doc: &mut XmlDocument,
    parent: NodeId,
    name: &str,
    property: Option<&str>,
    attributes: &[(&str, &str)],
) {
    let Some(property) = property.filter(|p| !p.is_empty()) else {
        return;
    };
    let el = doc.create_element(name);
    for (k, v) in attributes {
        doc.set_attribute(el, k, v);
    }
    let nodes = convert_zotero_markup(doc, property);
    if nodes.len() == 1 && doc.get_attribute(nodes[0], "face") == Some("normal") {
        let first = doc.first_child(nodes[0]).expect("style has its text");
        doc.append_child(el, first);
    } else {
        for n in nodes {
            doc.append_child(el, n);
        }
    }
    doc.append_child(parent, el);
}

/// `item[zfield]` when truthy, as a string.
fn field(item: &TranslatorItem, zfield: Option<&str>) -> Option<String> {
    let z = zfield?;
    let v = item.get(z)?;
    js::truthy(Some(v)).then(|| js::to_js_string(v))
}

/// `doExport` (:699-866).
pub fn do_export(ctx: &mut ExportContext) -> Result<(), TranslateError> {
    let mut doc = XmlDocument::parse_from_string("<xml/>");
    let records = doc.create_element("records");
    let dates_opts = ctx.options.env.dates.clone();
    let export_notes = ctx.options.option_truthy("exportNotes");
    let export_file_data = ctx.options.option_truthy("exportFileData");

    while let Some(item) = ctx.next_item() {
        if item.item_type == "note" || item.item_type == "attachment" {
            continue;
        }
        let t = item.item_type.as_str();
        let record = doc.create_element("record");
        for &f in FIELDS {
            match f {
                "database" => map_property(
                    &mut doc,
                    record,
                    "database",
                    Some("MyLibrary"),
                    &[("name", "MyLibrary")],
                ),
                "source-app" => map_property(
                    &mut doc,
                    record,
                    "source-app",
                    Some("Zotero"),
                    &[("name", "Zotero")],
                ),
                "ref-type" => {
                    // `{name: type}` with type undefined sets "undefined"; the
                    // element is only made when the number exists, and both
                    // tables cover the same types except `mail`/`email`.
                    let name = export_item_type(t).unwrap_or("undefined");
                    map_property(
                        &mut doc,
                        record,
                        "ref-type",
                        export_ref_number(t),
                        &[("name", name)],
                    );
                }
                "titles" => {
                    let titles = doc.create_element("titles");
                    for tf in TITLE_FIELDS {
                        let v = field(&item, get_field(tf, t));
                        map_property(&mut doc, titles, tf, v.as_deref(), &[]);
                    }
                    doc.append_child(record, titles);
                }
                "contributors" => {
                    if !item.creators.is_empty() {
                        let contributors = doc.create_element("contributors");
                        let mut custom4: Vec<String> = Vec::new();
                        for af in AUTHOR_FIELDS {
                            custom4.clear();
                            let creatornode = doc.create_element(af);
                            let ty = get_field(af, t);
                            for c in &item.creators {
                                let name = || {
                                    let mut n =
                                        c.last_name.clone().unwrap_or_else(|| "undefined".into());
                                    if let Some(fnm) =
                                        c.first_name.as_deref().filter(|s| !s.is_empty())
                                    {
                                        n.push_str(", ");
                                        n.push_str(fnm);
                                    }
                                    n
                                };
                                if ty.is_some() && c.creator_type.as_deref() == ty {
                                    map_property(
                                        &mut doc,
                                        creatornode,
                                        "author",
                                        Some(&name()),
                                        &[],
                                    );
                                } else if c.creator_type.as_deref() == Some("attorneyAgent") {
                                    custom4.push(name());
                                }
                            }
                            if doc.has_child_nodes(creatornode) {
                                doc.append_child(contributors, creatornode);
                            }
                        }
                        if !custom4.is_empty() {
                            map_property(
                                &mut doc,
                                record,
                                "custom4",
                                Some(&custom4.join("; ")),
                                &[],
                            );
                        }
                        doc.append_child(record, contributors);
                    }
                }
                "dates" => {
                    let dates = doc.create_element("dates");
                    if let Some(d) = field(&item, get_field("pub-dates", t)) {
                        let o = str_to_date(&d, &dates_opts);
                        map_property(&mut doc, dates, "year", o.year.as_deref(), &[]);
                        let pubdates = doc.create_element("pub-dates");
                        doc.append_child(dates, pubdates);
                        map_property(&mut doc, pubdates, "date", Some(&d), &[]);
                    }
                    doc.append_child(record, dates);
                }
                "periodical" => {
                    let periodical = doc.create_element("periodical");
                    for pf in ["full-title", "abbr-1"] {
                        let v = field(&item, get_field(pf, t));
                        map_property(&mut doc, periodical, pf, v.as_deref(), &[]);
                    }
                    if !doc.element_children(periodical).is_empty() {
                        doc.append_child(record, periodical);
                    }
                }
                "keywords" => {
                    if !item.tags.is_empty() {
                        let keywords = doc.create_element("keywords");
                        for tag in &item.tags {
                            map_property(&mut doc, keywords, "keyword", Some(&tag.tag), &[]);
                        }
                        doc.append_child(record, keywords);
                    }
                }
                "research-notes" => {
                    if !item.notes.is_empty() && export_notes {
                        let s: String = item
                            .notes
                            .iter()
                            .map(|n| format!("<p>{}</p>", n.note))
                            .collect();
                        map_property(&mut doc, record, "research-notes", Some(&s), &[]);
                    }
                }
                "urls" => {
                    let urls = doc.create_element("urls");
                    if let Some(u) = field(&item, Some("url")) {
                        let weburls = doc.create_element("web-urls");
                        doc.append_child(urls, weburls);
                        map_property(&mut doc, weburls, "url", Some(&u), &[]);
                    }
                    if !item.attachments.is_empty() {
                        let pdfurls = doc.create_element("pdf-urls");
                        let texturls = doc.create_element("text-urls");
                        for a in &item.attachments {
                            // `exportFileData && attachment.saveFile`: the
                            // translation-server's attachments have no
                            // `saveFile`, so the path is always the local
                            // path or URL.
                            let _ = export_file_data;
                            let path = ["localPath", "url"]
                                .iter()
                                .find_map(|k| a.get(k).filter(|v| js::truthy(Some(v))))
                                .map(js::to_js_string);
                            let Some(path) = path else {
                                continue;
                            };
                            if a.get("mimeType") == Some(&Value::String("application/pdf".into())) {
                                map_property(&mut doc, pdfurls, "url", Some(&path), &[]);
                            } else {
                                map_property(&mut doc, texturls, "url", Some(&path), &[]);
                            }
                        }
                        if !doc.element_children(pdfurls).is_empty() {
                            doc.append_child(urls, pdfurls);
                        }
                        if !doc.element_children(texturls).is_empty() {
                            doc.append_child(urls, texturls);
                        }
                    }
                    if !doc.element_children(urls).is_empty() {
                        doc.append_child(record, urls);
                    }
                }
                _ => {
                    let v = field(&item, get_field(f, t));
                    map_property(&mut doc, record, f, v.as_deref(), &[]);
                }
            }
        }
        doc.append_child(records, record);
    }
    let root = doc.document_element().expect("<xml/> has a root");
    doc.append_child(root, records);
    ctx.write("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    // Follow EndNote's convention for newlines (:864-865).
    let out = doc
        .serialize(doc.document())
        .replace("\r\n", "\n")
        .replace(['\r', '\n'], "&#xD;");
    ctx.write(&out);
    Ok(())
}
