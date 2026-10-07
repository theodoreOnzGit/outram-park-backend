// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "DSpace Intermediate Metadata.js" (translatorID
//   2c05e2d1-a533-448f-aa20-e919584864cb, lastUpdated 2022-12-24 19:29:02):
//   `detectImport` :37-40, `getType` :42-59, `doImport` :60-121.
// Copyright (c) 2022 Sebastian Karcher.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The DSpace Intermediate Metadata translator (import): DSpace's DIM
//! (`dim:field` elements, usually inside a METS wrapper).

use crate::zotero::framework::options::{translator_type, TranslatorMetadata};
use crate::zotero::framework::utilities::{clean_author, trim_internal};
use crate::zotero::framework::xml::{get_xml, XNode};
use crate::zotero::framework::xpath::{xpath, xpath_text};
use crate::zotero::framework::{
    ImportContext, JsObject, TranslateError, TranslatorCreator, TranslatorItem, TranslatorTag,
};
use regex::Regex;
use serde_json::Value;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "2c05e2d1-a533-448f-aa20-e919584864cb",
    label: "DSpace Intermediate Metadata",
    creator: "Sebastian Karcher",
    target: "xml",
    min_version: "5.0",
    priority: 100,
    translator_type: translator_type::IMPORT,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2022-12-24 19:29:02",
};

const NS: &[(&str, &str)] = &[
    ("dim", "http://www.dspace.org/xmlns/dspace/dim"),
    ("mets", "http://www.loc.gov/METS/"),
    ("xlink", "http://www.w3.org/TR/xlink/"),
];

/// `detectImport` (:37-40): the first 1000 characters name the DIM
/// namespace.
pub fn detect_import(ctx: &mut ImportContext) -> bool {
    ctx.read_chars(1000)
        .is_some_and(|t| t.contains("http://www.dspace.org/xmlns/dspace/dim"))
}

/// `getType` (:42-59).
fn get_type(s: &str) -> &'static str {
    let s = s.to_lowercase();
    if s.contains("book_section") || s.contains("chapter") {
        "bookSection"
    } else if s.contains("book") || s.contains("monograph") {
        "book"
    } else if s.contains("report") {
        "report"
    } else if s.contains("proceedings") || s.contains("conference") {
        "conferencePaper"
    } else {
        "journalArticle"
    }
}

fn set(item: &mut TranslatorItem, key: &str, v: Option<String>) {
    item.set(key, v.map_or(Value::Null, Value::from));
}

/// `doImport` (:60-121).
pub fn do_import(ctx: &mut ImportContext) -> Result<(), TranslateError> {
    let doc = get_xml(ctx)?;
    let root = XNode::Node(doc.document());
    let text = |e: &str| xpath_text(&doc, root, e, NS, None);

    let ty = text("//dim:field[@element=\"type\"]")?;
    let mut item = TranslatorItem::new("journalArticle");
    if let Some(t) = ty.filter(|t| !t.is_empty()) {
        item.item_type = get_type(&t).to_owned();
    }

    // :71 `ZU.trimInternal(null)` throws.
    let title = text("//dim:field[@element=\"title\"]")?.ok_or_else(|| {
        TranslateError::Translator("Cannot read properties of null (reading 'replace')".into())
    })?;
    item.set("title", trim_internal(&title));
    let abs = xpath(&doc, root, "//dim:field[@qualifier=\"abstract\"]", NS)?;
    if let Some(&a) = abs.first() {
        item.set("abstractNote", doc.text(a));
    }
    set(
        &mut item,
        "date",
        text("//dim:field[@element=\"date\" and @qualifier=\"issued\"]")?,
    );
    set(
        &mut item,
        "language",
        text("//dim:field[@element=\"language\"]")?,
    );
    set(
        &mut item,
        "issue",
        text("//dim:field[@element=\"bibliographicCitation\" and @qualifier=\"issue\"]")?,
    );
    set(
        &mut item,
        "volume",
        text("//dim:field[@element=\"bibliographicCitation\" and @qualifier=\"volume\"]")?,
    );
    set(
        &mut item,
        "publicationTitle",
        text("//dim:field[@element=\"title\" and @qualifier=\"parent\"]")?,
    );
    if !item.truthy("publicationTitle") {
        set(
            &mut item,
            "publicationTitle",
            text("//dim:field[@element=\"bibliographicCitation\" and @qualifier=\"title\"]")?,
        );
    }
    set(
        &mut item,
        "conferenceName",
        text("//dim:field[@element=\"bibliographicCitation\" and @qualifier=\"conferencename\"]")?,
    );
    set(
        &mut item,
        "publisher",
        text("//dim:field[@element=\"publisher\" and not(@qualifier=\"place\")]")?,
    );
    set(
        &mut item,
        "place",
        text("//dim:field[@element=\"publisher\" and @qualifier=\"place\"]")?,
    );
    set(
        &mut item,
        "series",
        text("//dim:field[@element=\"relation\" and @qualifier=\"ispartofseries\"]")?,
    );
    set(
        &mut item,
        "ISSN",
        text("//dim:field[@element=\"identifier\" and @qualifier=\"issn\"]")?,
    );
    let pages = text("//dim:field[@element=\"format\" and @qualifier=\"pagerange\"]")?;
    let stpage = "//dim:field[@element=\"bibliographicCitation\" and @qualifier=\"stpage\"]";
    if let Some(p) = pages.filter(|p| !p.is_empty()) {
        let re = Regex::new(r"(?i)pp?\.").unwrap();
        item.set("pages", re.replace(&p, "").into_owned());
    } else if text(stpage)?.is_some_and(|s| !s.is_empty()) {
        // `a + "-" + b`: a null end page is "null".
        let end =
            text("//dim:field[@element=\"bibliographicCitation\" and @qualifier=\"endpage\"]")?;
        item.set(
            "pages",
            format!(
                "{}-{}",
                text(stpage)?.unwrap_or_default(),
                end.unwrap_or_else(|| "null".into())
            ),
        );
    }
    let num_pages = text("//dim:field[@element=\"format\" and @qualifier=\"pages\"]")?;
    if let Some(n) = num_pages.filter(|n| !n.is_empty()) {
        let re = Regex::new(r"(?i)pp?\.?").unwrap();
        item.set("numPages", re.replace(&n, "").into_owned());
    }
    set(
        &mut item,
        "url",
        text("//dim:field[@element=\"identifier\" and @qualifier=\"uri\"]")?,
    );

    for a in xpath(
        &doc,
        root,
        "//dim:field[@element=\"contributor\" and @qualifier=\"author\"]",
        NS,
    )? {
        let c = clean_author(&doc.text(a), "author", true);
        item.creators.push(TranslatorCreator {
            first_name: c.first_name,
            last_name: Some(c.last_name),
            creator_type: Some(c.creator_type),
            ..Default::default()
        });
    }
    for t in xpath(&doc, root, "//dim:field[@element=\"subject\"]", NS)? {
        item.tags.push(TranslatorTag::new(doc.text(t)));
    }
    let pdf = text("//mets:file[@MIMETYPE=\"application/pdf\"][1]/mets:FLocat/@xlink:href")?;
    if let Some(u) = pdf.filter(|u| !u.is_empty()) {
        let mut a = JsObject::new();
        a.set("url", u);
        a.set("title", "Full Text PDF");
        a.set("mimeType", "application/pdf");
        item.attachments.push(a);
    }
    ctx.item_done(item);
    Ok(())
}
