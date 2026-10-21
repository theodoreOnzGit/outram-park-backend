// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "MODS.js" (lastUpdated 2022-10-31 14:01:52):
//   `mapProperty` :351-363, `doExport` :365-697; Zotero utilities (commit
//   4051881d59c6) utilities.js `getPageRange` :952-964.
// Copyright (c) 2019-2021 Simon Kornblith, Richard Karnesky, and Abe Jellinek.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! MODS export: a `modsCollection` document built with the DOM and written
//! with `XMLSerializer`.

use super::{lookup, NS, PARTIAL_ITEM_TYPES, TO_MARC_GENRE, TO_TYPE_OF_RESOURCE};
use crate::zotero::framework::js;
use crate::zotero::framework::xml::{NodeId, XmlDocument};
use crate::zotero::framework::{ExportContext, TranslateError, TranslatorItem};
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

/// `mapProperty(parentElement, elementName, property, attributes)`
/// (:351-363): nothing for a falsy property other than 0.
fn map_property(
    doc: &mut XmlDocument,
    parent: NodeId,
    name: &str,
    property: Option<&Value>,
    attrs: &[(&str, &str)],
) -> Option<NodeId> {
    let p = property?;
    let zero = matches!(p, Value::Number(n) if n.as_f64() == Some(0.0));
    if !js::truthy(Some(p)) && !zero {
        return None;
    }
    let el = doc.create_element_ns(Some(NS), name);
    for (k, v) in attrs {
        doc.set_attribute(el, k, v);
    }
    let t = doc.create_text_node(&js::to_js_string(p));
    doc.append_child(el, t);
    doc.append_child(parent, el);
    Some(el)
}

fn s(v: &str) -> Value {
    Value::String(v.to_owned())
}

/// `ZU.getPageRange(pages)` (utilities.js:952-964).
fn get_page_range(pages: &str) -> (String, String) {
    static R: OnceLock<Regex> = OnceLock::new();
    let ws = js::WS;
    let r = R.get_or_init(|| {
        Regex::new(&format!(r"^{ws}*([0-9]+) ?[-\x{{2013}}] ?([0-9]+){ws}*$")).unwrap()
    });
    match r.captures(pages) {
        Some(c) => (c[1].to_owned(), c[2].to_owned()),
        None => (pages.to_owned(), pages.to_owned()),
    }
}

fn truthy(item: &TranslatorItem, k: &str) -> bool {
    item.truthy(k)
}

/// `doExport` (:365-697).
pub fn do_export(ctx: &mut ExportContext) -> Result<(), TranslateError> {
    let mut doc = XmlDocument::parse_from_string(
        "<modsCollection xmlns=\"http://www.loc.gov/mods/v3\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:schemaLocation=\"http://www.loc.gov/mods/v3 http://www.loc.gov/standards/mods/v3/mods-3-2.xsd\" />",
    );
    let root = doc.document_element().expect("parsed");
    let export_notes = ctx.options.option_truthy("exportNotes");
    while let Some(item) = ctx.next_item() {
        if item.item_type == "note" || item.item_type == "attachment" {
            continue;
        }
        let d = &mut doc;
        let mods = d.create_element_ns(Some(NS), "mods");
        let is_partial = PARTIAL_ITEM_TYPES.contains(&item.item_type.as_str());
        let record_info = d.create_element_ns(Some(NS), "recordInfo");
        let host = d.create_element_ns(Some(NS), "relatedItem");
        let series = d.create_element_ns(Some(NS), "relatedItem");
        let top_or_host = if is_partial { host } else { mods };
        let ty = item.item_type.as_str();

        if truthy(&item, "title") {
            let ti = d.create_element_ns(Some(NS), "titleInfo");
            map_property(d, ti, "title", item.get("title"), &[]);
            d.append_child(mods, ti);
        }
        if truthy(&item, "shortTitle") {
            let ti = d.create_element_ns(Some(NS), "titleInfo");
            d.set_attribute(ti, "type", "abbreviated");
            map_property(d, ti, "title", item.get("shortTitle"), &[]);
            d.append_child(mods, ti);
        }

        map_property(
            d,
            mods,
            "typeOfResource",
            lookup(TO_TYPE_OF_RESOURCE, ty).map(s).as_ref(),
            &[],
        );
        map_property(d, mods, "genre", Some(&s(ty)), &[("authority", "local")]);
        map_property(
            d,
            top_or_host,
            "genre",
            lookup(TO_MARC_GENRE, ty).map(s).as_ref(),
            &[("authority", "marcgt")],
        );
        if truthy(&item, "thesisType") {
            map_property(d, mods, "genre", item.get("thesisType"), &[]);
        } else if truthy(&item, "type") {
            map_property(d, mods, "genre", item.get("type"), &[]);
        }

        for c in &item.creators {
            let ct = c.creator_type.as_deref().unwrap_or("");
            let role_term = match ct {
                "author" => {
                    if ty == "letter" {
                        "cre"
                    } else {
                        "aut"
                    }
                }
                "editor" => "edt",
                "translator" => "trl",
                "seriesEditor" => "pbd",
                "composer" => "cmp",
                "wordsBy" => "lyr",
                "performer" => "prf",
                "recipient" => "rcp",
                _ => "ctb",
            };
            let name = d.create_element_ns(Some(NS), "name");
            let ln = c.last_name.clone().map(Value::String);
            let fnm = c.first_name.clone().map(Value::String);
            if c.field_mode == Some(1) {
                d.set_attribute(name, "type", "corporate");
                map_property(d, name, "namePart", ln.as_ref(), &[]);
            } else {
                d.set_attribute(name, "type", "personal");
                map_property(d, name, "namePart", ln.as_ref(), &[("type", "family")]);
                map_property(d, name, "namePart", fnm.as_ref(), &[("type", "given")]);
            }
            let role = d.create_element_ns(Some(NS), "role");
            map_property(
                d,
                role,
                "roleTerm",
                Some(&s(role_term)),
                &[("type", "code"), ("authority", "marcrelator")],
            );
            d.append_child(name, role);
            let parent = if ct == "seriesEditor" {
                series
            } else if ct == "editor" {
                top_or_host
            } else {
                mods
            };
            d.append_child(parent, name);
        }

        map_property(
            d,
            record_info,
            "recordContentSource",
            item.get("libraryCatalog"),
            &[],
        );
        map_property(d, mods, "accessCondition", item.get("rights"), &[]);

        let part = d.create_element_ns(Some(NS), "part");
        for detail in ["volume", "issue", "section"] {
            let v = item.get(detail);
            let zero = matches!(v, Some(Value::Number(n)) if n.as_f64() == Some(0.0));
            if js::truthy(v) || zero {
                let de = d.create_element_ns(Some(NS), "detail");
                let num = d.create_element_ns(Some(NS), "number");
                d.set_attribute(de, "type", detail);
                let t = d.create_text_node(&js::to_js_string(v.expect("truthy")));
                d.append_child(num, t);
                d.append_child(de, num);
                d.append_child(part, de);
            }
        }
        if truthy(&item, "pages") {
            let pages = item.get_string("pages").unwrap_or_default();
            let extent = d.create_element_ns(Some(NS), "extent");
            d.set_attribute(extent, "unit", "pages");
            static PR: OnceLock<Regex> = OnceLock::new();
            let pr = PR.get_or_init(|| Regex::new(r"^[0-9]+[-\x{2013}][0-9]+$").unwrap());
            if pr.is_match(&pages) {
                let (a, b) = get_page_range(&pages);
                map_property(d, extent, "start", Some(&s(&a)), &[]);
                map_property(d, extent, "end", Some(&s(&b)), &[]);
            } else {
                d.set_attribute(extent, "unit", "pages");
                map_property(d, extent, "list", Some(&s(&pages)), &[]);
            }
            d.append_child(part, extent);
        }
        if d.has_child_nodes(part) {
            d.append_child(top_or_host, part);
        }

        let origin = d.create_element_ns(Some(NS), "originInfo");
        map_property(d, origin, "edition", item.get("edition"), &[]);
        if truthy(&item, "place") {
            let place = d.create_element_ns(Some(NS), "place");
            let pt = d.create_element_ns(Some(NS), "placeTerm");
            d.set_attribute(pt, "type", "text");
            let t = d.create_text_node(&item.get_string("place").unwrap_or_default());
            d.append_child(pt, t);
            d.append_child(place, pt);
            d.append_child(origin, place);
        }
        if truthy(&item, "publisher") {
            map_property(d, origin, "publisher", item.get("publisher"), &[]);
        } else if truthy(&item, "distributor") {
            // Upstream maps `item.publisher` here (falsy): nothing is added.
            map_property(d, origin, "distributor", item.get("publisher"), &[]);
        }
        if truthy(&item, "date") {
            let date_type = if ["book", "bookSection"].contains(&ty) {
                "copyrightDate"
            } else if ["journalArticle", "magazineArticle", "newspaperArticle"].contains(&ty) {
                "dateIssued"
            } else {
                "dateCreated"
            };
            map_property(d, origin, date_type, item.get("date"), &[]);
        }
        if truthy(&item, "numPages") {
            let pd = d.create_element_ns(Some(NS), "physicalDescription");
            let v = format!("{} p.", item.get_string("numPages").unwrap_or_default());
            map_property(d, pd, "extent", Some(&s(&v)), &[]);
            d.append_child(mods, pd);
        }
        if is_partial {
            if ["journalArticle", "magazineArticle", "newspaperArticle"].contains(&ty) {
                map_property(d, origin, "issuance", Some(&s("continuing")), &[]);
            } else if [
                "bookSection",
                "conferencePaper",
                "dictionaryEntry",
                "encyclopediaArticle",
            ]
            .contains(&ty)
            {
                map_property(d, origin, "issuance", Some(&s("monographic")), &[]);
            }
        } else {
            map_property(d, origin, "issuance", Some(&s("monographic")), &[]);
        }
        if d.has_child_nodes(origin) {
            d.append_child(top_or_host, origin);
        }

        map_property(
            d,
            top_or_host,
            "identifier",
            item.get("ISBN"),
            &[("type", "isbn")],
        );
        map_property(
            d,
            top_or_host,
            "identifier",
            item.get("ISSN"),
            &[("type", "issn")],
        );
        map_property(d, mods, "identifier", item.get("DOI"), &[("type", "doi")]);

        if truthy(&item, "conferenceName") {
            // Upstream builds this element and never appends it.
            let name = d.create_element_ns(Some(NS), "name");
            d.set_attribute(name, "type", "conference");
            map_property(d, name, "namePart", item.get("conferenceName"), &[]);
        }
        if truthy(&item, "publicationTitle") {
            let ti = d.create_element_ns(Some(NS), "titleInfo");
            map_property(d, ti, "title", item.get("publicationTitle"), &[]);
            d.append_child(host, ti);
        }
        if truthy(&item, "journalAbbreviation") {
            let ti = d.create_element_ns(Some(NS), "titleInfo");
            d.set_attribute(ti, "type", "abbreviated");
            map_property(d, ti, "title", item.get("journalAbbreviation"), &[]);
            d.append_child(host, ti);
        }
        map_property(
            d,
            top_or_host,
            "classification",
            item.get("callNumber"),
            &[],
        );

        if truthy(&item, "url") {
            let loc = d.create_element_ns(Some(NS), "location");
            let url = map_property(
                d,
                loc,
                "url",
                item.get("url"),
                &[("usage", "primary display")],
            );
            if let Some(u) = url {
                if truthy(&item, "accessDate") {
                    let ad = item.get_string("accessDate").unwrap_or_default();
                    d.set_attribute(u, "dateLastAccessed", &ad);
                }
            }
            d.append_child(mods, loc);
        }
        if truthy(&item, "archiveLocation") {
            let loc = d.create_element_ns(Some(NS), "location");
            map_property(d, loc, "physicalLocation", item.get("archiveLocation"), &[]);
            d.append_child(top_or_host, loc);
        }
        map_property(d, mods, "abstract", item.get("abstractNote"), &[]);

        let ti = d.create_element_ns(Some(NS), "titleInfo");
        map_property(d, ti, "title", item.get("series"), &[]);
        map_property(d, ti, "title", item.get("seriesTitle"), &[]);
        map_property(d, ti, "subTitle", item.get("seriesText"), &[]);
        if d.has_child_nodes(ti) {
            d.append_child(series, ti);
        }
        if truthy(&item, "seriesNumber") {
            let sp = d.create_element_ns(Some(NS), "part");
            let de = d.create_element_ns(Some(NS), "detail");
            let num = d.create_element_ns(Some(NS), "number");
            d.set_attribute(de, "type", "volume");
            let t = d.create_text_node(&item.get_string("seriesNumber").unwrap_or_default());
            d.append_child(num, t);
            d.append_child(de, num);
            d.append_child(sp, de);
            d.append_child(series, sp);
        }

        if export_notes {
            for n in &item.notes {
                map_property(d, mods, "note", Some(&s(&n.note)), &[]);
            }
        }
        for t in &item.tags {
            let subject = d.create_element_ns(Some(NS), "subject");
            let topic = d.create_element_ns(Some(NS), "topic");
            let tx = d.create_text_node(&t.tag);
            d.append_child(topic, tx);
            d.append_child(subject, topic);
            d.append_child(mods, subject);
        }
        if truthy(&item, "language") {
            let lang = d.create_element_ns(Some(NS), "language");
            map_property(
                d,
                lang,
                "languageTerm",
                item.get("language"),
                &[("type", "text")],
            );
            d.append_child(mods, lang);
        }
        map_property(d, mods, "note", item.get("extra"), &[]);

        if d.has_child_nodes(record_info) {
            d.append_child(mods, record_info);
        }
        if d.has_child_nodes(host) {
            d.set_attribute(host, "type", "host");
            d.append_child(mods, host);
        }
        if d.has_child_nodes(series) {
            d.set_attribute(series, "type", "series");
            d.append_child(top_or_host, series);
        }
        d.append_child(root, mods);
    }
    ctx.write("<?xml version=\"1.0\"?>\n");
    let out = doc.serialize(doc.document());
    ctx.write(&out);
    Ok(())
}
