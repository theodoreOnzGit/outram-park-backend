// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "Note Markdown.js" (translatorID
//   1412e9e2-51e1-42ec-aa35-e036a895534b, lastUpdated 2024-07-10 16:00:00):
//   `convert` :1461-1617 and `doExport` :1619-1638; the bundled turndown and
//   turndown-plugin-gfm are in [`turndown`].
// Copyright (c) 2021 Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The Note Markdown translator (export): each note among the exported
//! items (standalone notes and attachments' notes) as Markdown, separated
//! by `---`.

pub mod turndown;

use crate::zotero::framework::html_dom::{
    create_html_element, parse_html_document, query_selector_all, set_inner_html, style_get,
};
use crate::zotero::framework::js;
use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::xml::{NodeId, XmlDocument};
use crate::zotero::framework::{ExportContext, TranslateError};
use crate::zotero::translators::note_html::{
    annotation_link, citation_items, citation_uris, has_citation_items, insert_annotation_link,
};
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;
use turndown::Turndown;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "1412e9e2-51e1-42ec-aa35-e036a895534b",
    label: "Note Markdown",
    creator: "Martynas Bagdonas",
    target: "md",
    min_version: "5.0.97",
    priority: 50,
    translator_type: translator_type::EXPORT,
    config_options: &[("noteTranslator", HeaderValue::Bool(true))],
    display_options: &[("includeAppLinks", HeaderValue::Bool(true))],
    hidden_prefs: &[],
    last_updated: "2024-07-10 16:00:00",
};

fn body(doc: &XmlDocument) -> NodeId {
    query_selector_all(doc, doc.document(), "body")[0]
}

/// `JSON.parse(decodeURIComponent(getAttribute(name)))` in a try.
fn parse_data_attr(doc: &XmlDocument, n: NodeId, name: &str) -> Option<Value> {
    let raw = doc.get_attribute(n, name).unwrap_or("null");
    let decoded = crate::zotero::framework::utilities::decode_uri_component(raw)?;
    serde_json::from_str::<Value>(&decoded).ok()
}

/// `node.replaceWith(...nodes)`.
fn replace_with(doc: &mut XmlDocument, node: NodeId, nodes: &[NodeId]) {
    let Some(parent) = doc.parent(node) else {
        return;
    };
    for &n in nodes {
        doc.insert_before(parent, n, Some(node));
    }
    doc.detach(node);
}

/// `convert(doc)` (:1461-1617).
fn convert(mut doc: XmlDocument, include_app_links: bool) -> Result<String, TranslateError> {
    let root = doc.document();
    // Strike-through spans to <s>.
    for span in query_selector_all(&doc, root, "span") {
        if style_get(&doc, span, "text-decoration") == "line-through" {
            let s = create_html_element(&mut doc, "s");
            for c in doc.children(span).to_vec() {
                doc.append_child(s, c);
            }
            replace_with(&mut doc, span, &[s]);
        }
    }
    // Turndown wants pre content inside a code element.
    for pre in query_selector_all(&doc, root, "pre:not(.math)") {
        let code = create_html_element(&mut doc, "code");
        for c in doc.children(pre).to_vec() {
            doc.append_child(code, c);
        }
        doc.append_child(pre, code);
    }
    // Indented paragraphs.
    static PAD: OnceLock<Regex> = OnceLock::new();
    let pad = PAD.get_or_init(|| Regex::new("padding-(left|right): ([0-9]+)px").expect("regex"));
    for p in query_selector_all(&doc, root, "p") {
        let Some(style) = doc.get_attribute(p, "style").filter(|s| !s.is_empty()) else {
            continue;
        };
        let Some(m) = pad.captures(style) else {
            continue;
        };
        let px: u64 = m[2].parse().unwrap_or(0);
        if px > 0 && px % 40 == 0 {
            let spaces = "\u{A0}".repeat(4 * (px / 40) as usize);
            let t = doc.create_text_node(&spaces);
            let first = doc.first_child(p);
            doc.insert_before(p, t, first);
            for br in query_selector_all(&doc, p, "br") {
                let t = doc.create_text_node(&spaces);
                if let Some(parent) = doc.parent(br) {
                    let next = doc.next_sibling(br);
                    doc.insert_before(parent, t, next);
                }
            }
        }
    }
    // Links after highlight, underline and image annotations.
    for node in query_selector_all(
        &doc,
        root,
        "span[class=\"highlight\"], span[class=\"underline\"], img[data-annotation]",
    ) {
        let Some(annotation) = parse_data_attr(&doc, node, "data-annotation") else {
            continue;
        };
        if !js::truthy(Some(&annotation)) || !include_app_links {
            continue;
        }
        if let Some((href, text)) = annotation_link(&annotation) {
            insert_annotation_link(&mut doc, node, &href, text);
        }
    }
    // A placeholder for embedded note images.
    for img in query_selector_all(&doc, root, "img[data-attachment-key]") {
        let t = doc.create_text_node("[image]");
        replace_with(&mut doc, img, &[t]);
    }
    // Citations to links.
    for span in query_selector_all(&doc, root, "span[class=\"citation\"]") {
        let Some(citation) = parse_data_attr(&doc, span, "data-citation") else {
            continue;
        };
        if !has_citation_items(&citation) {
            continue;
        }
        let uris = citation_uris(&citation);
        let items = citation_items(&doc, span);
        let parts: Vec<String> = items
            .iter()
            .enumerate()
            .map(|(i, it)| {
                if include_app_links {
                    let u = uris.get(i).map_or("undefined", String::as_str);
                    format!("<a href=\"{u}\">{it}</a>")
                } else {
                    it.clone()
                }
            })
            .collect();
        set_inner_html(&mut doc, span, &format!("({})", parts.join("; ")));
    }

    let b = body(&doc);
    let mut td = Turndown::new(doc);
    let text = td.turndown(b)?;
    // Remove lines with just two spaces (`<p><br>test</p>`).
    Ok(text
        .split('\n')
        .filter(|l| *l != "  ")
        .collect::<Vec<_>>()
        .join("\n"))
}

/// `doExport` (:1619-1638).
pub fn do_export(ctx: &mut ExportContext) -> Result<(), TranslateError> {
    let include_app_links = ctx.options.option_truthy("includeAppLinks");
    let mut first = true;
    while let Some(item) = ctx.next_item() {
        if item.item_type != "note" && item.item_type != "attachment" {
            continue;
        }
        let note = match item.get("note") {
            Some(v) if js::truthy(Some(v)) => js::to_js_string(v),
            _ => String::new(),
        };
        let doc = parse_html_document(&note);
        // Skip empty notes.
        if js::trim(&doc.text(body(&doc))).is_empty() {
            continue;
        }
        if !first {
            ctx.write("\n\n---\n\n");
        }
        first = false;
        let md = convert(doc, include_app_links)?;
        ctx.write(&md);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_a_minimal_note() {
        let doc = parse_html_document("<p>Hi <em>there</em></p>");
        assert_eq!(convert(doc, true).unwrap(), "Hi *there*");
    }
}
