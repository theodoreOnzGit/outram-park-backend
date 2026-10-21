// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "Note HTML.js" (translatorID
//   897a81c2-9f60-4bec-ae6b-85a5030b8be5, lastUpdated 2024-07-10 15:30:00):
//   `doExport` :46-206 (itself based on Zotero's quickCopy.js
//   32a5826a1fdc :266-419).
// Copyright (c) 2021 Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The Note HTML translator (export): the notes among the exported items
//! (standalone notes and attachments' notes) as one HTML document.
//!
//! The note HTML is parsed, edited and serialised as jsdom does it
//! ([`crate::zotero::framework::html_dom`]); the XPath steps run on the
//! wicked-good-xpath port over the HTML document.

use crate::zotero::framework::html_dom::{
    class_list_contains, create_html_element, next_element_sibling, outer_html,
    parse_html_document, query_selector_all, set_inner_html, style_get, style_set,
};
use crate::zotero::framework::js;
use crate::zotero::framework::openurl::encode_uri_component;
use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::utilities::decode_uri_component;
use crate::zotero::framework::xml::{NodeId, XmlDocument};
use crate::zotero::framework::xpath::xpath;
use crate::zotero::framework::{ExportContext, TranslateError};
use serde_json::Value;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "897a81c2-9f60-4bec-ae6b-85a5030b8be5",
    label: "Note HTML",
    creator: "Martynas Bagdonas",
    target: "html",
    min_version: "5.0.97",
    priority: 50,
    translator_type: translator_type::EXPORT,
    config_options: &[("noteTranslator", HeaderValue::Bool(true))],
    display_options: &[("includeAppLinks", HeaderValue::Bool(false))],
    hidden_prefs: &[],
    last_updated: "2024-07-10 15:30:00",
};

fn nodes(doc: &XmlDocument, expr: &str) -> Result<Vec<NodeId>, TranslateError> {
    Ok(xpath(doc, doc.document(), expr, &[])?
        .into_iter()
        .filter_map(|x| x.as_node())
        .collect())
}

/// `JSON.parse(decodeURIComponent(attr))` in a try (undefined on failure).
fn parse_data_attr(doc: &XmlDocument, n: NodeId, name: &str) -> Option<Value> {
    // getAttribute gives null when absent; decodeURIComponent(null) is
    // "null", and JSON.parse("null") is null: falsy, as absent.
    let raw = doc.get_attribute(n, name).unwrap_or("null");
    let decoded = decode_uri_component(raw)?;
    serde_json::from_str::<Value>(&decoded).ok()
}

/// The PDF/snapshot/EPUB link inserted after a highlight, underline or
/// image annotation (Note HTML :71-121, Note Markdown :1500-1560): `None`
/// when the annotation has no attachment URI string or no position object.
pub(crate) fn annotation_link(annotation: &Value) -> Option<(String, &'static str)> {
    let uri = match annotation.get("attachmentURI") {
        Some(v) if js::truthy(Some(v)) => v,
        _ => annotation.get("uri")?,
    };
    let uri = uri.as_str()?;
    let position = annotation.get("position")?;
    // `typeof position === 'object'` (null counts as an object).
    if !(position.is_object() || position.is_array() || position.is_null()) {
        return None;
    }
    let parts: Vec<&str> = uri.split('/').collect();
    let library_type = parts.get(3).copied();
    let key = parts.last().copied().unwrap_or("");
    let mut open = if library_type == Some("users") {
        format!("zotero://open-pdf/library/items/{key}")
    } else {
        let group = parts.get(4).copied().unwrap_or("undefined");
        format!("zotero://open-pdf/groups/{group}/items/{key}")
    };
    let ptype = position.get("type").and_then(Value::as_str);
    let value_str = |p: &Value| -> String {
        match p.get("value") {
            Some(Value::String(s)) => s.clone(),
            Some(v) => js::to_js_string(v),
            None => "undefined".into(),
        }
    };
    let text = match ptype {
        Some("FragmentSelector") => {
            open.push_str(&format!(
                "?cfi={}",
                encode_uri_component(&value_str(position))
            ));
            "epub"
        }
        Some("CssSelector") => {
            open.push_str(&format!(
                "?sel={}",
                encode_uri_component(&value_str(position))
            ));
            "snapshot"
        }
        _ => {
            // position.pageIndex + 1 (undefined + 1 is NaN).
            let page = match position.get("pageIndex").and_then(Value::as_f64) {
                Some(p) => crate::zotero::framework::xpath::js_number_to_string(p + 1.0),
                None => "NaN".into(),
            };
            open.push_str(&format!("?page={page}"));
            "pdf"
        }
    };
    if let Some(k) = annotation
        .get("annotationKey")
        .filter(|v| js::truthy(Some(v)))
    {
        open.push_str(&format!("&annotation={}", js::to_js_string(k)));
    }
    Some((open, text))
}

/// Insert ` (<a href="...">pdf</a>) ` after an annotation node, or after
/// the citation that directly follows it.
pub(crate) fn insert_annotation_link(doc: &mut XmlDocument, node: NodeId, href: &str, text: &str) {
    let a = create_html_element(doc, "a");
    doc.set_attribute(a, "href", href);
    let t = doc.create_text_node(text);
    doc.append_child(a, t);
    let open = doc.create_text_node(" (");
    let close = doc.create_text_node(") ");
    let next = next_element_sibling(doc, node);
    let anchor = match next {
        Some(n) if class_list_contains(doc, n, "citation") => n,
        _ => node,
    };
    let Some(parent) = doc.parent(anchor) else {
        return;
    };
    let reference = doc.next_sibling(anchor);
    for n in [open, a, close] {
        doc.insert_before(parent, n, reference);
    }
}

/// The `zotero://select` URIs of a citation's items (`uris[0]` of each, when
/// a string).
pub(crate) fn citation_uris(citation: &Value) -> Vec<String> {
    let mut uris = Vec::new();
    for ci in citation["citationItems"].as_array().into_iter().flatten() {
        let Some(uri) = ci["uris"].get(0).and_then(Value::as_str) else {
            continue;
        };
        let parts: Vec<&str> = uri.split('/').collect();
        let key = parts.last().copied().unwrap_or("");
        if parts.get(3) == Some(&"users") {
            uris.push(format!("zotero://select/library/items/{key}"));
        } else {
            let group = parts.get(4).copied().unwrap_or("undefined");
            uris.push(format!("zotero://select/groups/{group}/items/{key}"));
        }
    }
    uris
}

/// A citation span's item texts: its `.citation-item` texts, or (pre-v5
/// note-editor schema) its text without the parentheses split on "; ".
pub(crate) fn citation_items(doc: &XmlDocument, span: NodeId) -> Vec<String> {
    let items: Vec<String> = query_selector_all(doc, span, ".citation-item")
        .into_iter()
        .map(|x| doc.text(x))
        .collect();
    if !items.is_empty() {
        return items;
    }
    let t: Vec<char> = doc.text(span).chars().collect();
    // slice(1, -1)
    let inner: String = if t.len() >= 2 {
        t[1..t.len() - 1].iter().collect()
    } else {
        String::new()
    };
    inner.split("; ").map(str::to_owned).collect()
}

/// Whether a citation JSON has citation items (`citation && citationItems
/// && citationItems.length`).
pub(crate) fn has_citation_items(c: &Value) -> bool {
    c.get("citationItems")
        .is_some_and(|v| js::truthy(Some(v)) && v.as_array().is_none_or(|a| !a.is_empty()))
}

/// `doExport` (:46-206).
pub fn do_export(ctx: &mut ExportContext) -> Result<(), TranslateError> {
    let mut doc = parse_html_document("<div class=\"zotero-notes\"/>");
    let body = query_selector_all(&doc, doc.document(), "body")[0];
    let container = doc.first_child(body).expect("the zotero-notes div");
    while let Some(item) = ctx.next_item() {
        if item.item_type != "note" && item.item_type != "attachment" {
            continue;
        }
        let div = create_html_element(&mut doc, "div");
        doc.set_attribute(div, "class", "zotero-note");
        let note = match item.get("note") {
            Some(v) if js::truthy(Some(v)) => js::to_js_string(v),
            _ => String::new(),
        };
        set_inner_html(&mut doc, div, &note);
        // Skip empty notes.
        if js::trim(&doc.text(div)).is_empty() {
            continue;
        }
        // Unwrap the ProseMirror note metadata container.
        if let Some(inner) = doc.element_children(div).first().copied() {
            if doc
                .get_attribute(inner, "data-schema-version")
                .is_some_and(|v| !v.is_empty())
            {
                for c in doc.children(inner).to_vec() {
                    doc.insert_before(div, c, Some(inner));
                }
                doc.detach(inner);
            }
        }
        doc.append_child(container, div);
    }

    if ctx.options.option_truthy("includeAppLinks") {
        let root = doc.document();
        for node in query_selector_all(
            &doc,
            root,
            "span[class=\"highlight\"], span[class=\"underline\"], img[data-annotation]",
        ) {
            let Some(annotation) = parse_data_attr(&doc, node, "data-annotation") else {
                continue;
            };
            if !js::truthy(Some(&annotation)) {
                continue;
            }
            if let Some((href, text)) = annotation_link(&annotation) {
                insert_annotation_link(&mut doc, node, &href, text);
            }
        }
        for span in query_selector_all(&doc, root, "span[class=\"citation\"]") {
            let Some(citation) = parse_data_attr(&doc, span, "data-citation") else {
                continue;
            };
            if !has_citation_items(&citation) {
                continue;
            }
            let uris = citation_uris(&citation);
            let items = citation_items(&doc, span);
            let html: Vec<String> = items
                .iter()
                .enumerate()
                .map(|(i, it)| {
                    let u = uris.get(i).map_or("undefined", String::as_str);
                    format!("<a href=\"{u}\">{it}</a>")
                })
                .collect();
            set_inner_html(&mut doc, span, &format!("({})", html.join("; ")));
        }
    }

    // Remove annotation and citation data.
    for (expr, attr) in [
        ("//span[@data-citation]", "data-citation"),
        ("//span[@data-annotation]", "data-annotation"),
        ("//img[@data-annotation]", "data-annotation"),
    ] {
        for n in nodes(&doc, expr)? {
            doc.remove_attribute(n, attr);
        }
    }

    // Horizontal rules between notes.
    for el in doc.element_children(container).into_iter().skip(1) {
        let hr = create_html_element(&mut doc, "hr");
        doc.insert_before(container, hr, Some(el));
    }

    // Quotes around blockquote paragraphs.
    for p in nodes(&doc, "//blockquote/p[1]")? {
        let t = doc.create_text_node("\u{201c}");
        let first = doc.first_child(p);
        doc.insert_before(p, t, first);
    }
    for p in nodes(&doc, "//blockquote/p[last()]")? {
        let t = doc.create_text_node("\u{201d}");
        doc.append_child(p, t);
    }

    // `Zotero.Utilities.xpath(doc, 'p')`: the document's own p children
    // (none: its child is <html>), ported as written.
    for p in nodes(&doc, "p")? {
        let pl = style_get(&doc, p, "padding-left");
        if !pl.is_empty() {
            style_set(&mut doc, p, "margin-left", &pl);
            style_set(&mut doc, p, "padding-left", "");
        }
    }

    for p in nodes(&doc, "//blockquote/p")? {
        style_set(&mut doc, p, "margin-left", "30px");
    }

    let meta = create_html_element(&mut doc, "meta");
    doc.set_attribute(meta, "charset", "utf-8");
    let head = query_selector_all(&doc, doc.document(), "head")[0];
    doc.append_child(head, meta);

    let root = doc.document_element().expect("html");
    ctx.write(&format!("<!DOCTYPE html>{}", outer_html(&doc, root)));
    Ok(())
}
