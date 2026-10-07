// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "Crossref Unixref XML.js" (translatorID
//   93514073-b541-4e02-9180-c36d2f3bb401, lastUpdated 2024-10-30 12:58:25):
//   `innerXML` :47-61, `removeUnsupportedMarkup` :63-86,
//   `fixAuthorCapitalization` :89-101, `parseCreators` :103-134,
//   `parseDate` :136-158, `detectImport` :161-175, `doImport` :178-477.
//   `fixAuthorCapitalization` calls Zotero utilities (commit 4051881d59c6)
//   utilities.js `capitalizeName` :199-216, ported here.
// Copyright (c) 2019 Sebastian Karcher; utilities: Corporation for Digital
//   Scholarship.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The Crossref Unixref XML translator (import): Crossref's `unixref`
//! records (`doi_records/doi_record/crossref/...`).

use crate::zotero::framework::js;
use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::utilities::{field_is_valid_for_type, trim_internal};
use crate::zotero::framework::xml::{get_xml, XNode, XmlDocument};
use crate::zotero::framework::xpath::{xpath_all, xpath_text, xpath_text_all};
use crate::zotero::framework::{
    ImportContext, TranslateError, TranslatorCreator, TranslatorItem, TranslatorNote,
};
use regex::{Captures, Regex};
use serde_json::Value;
use std::sync::OnceLock;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "93514073-b541-4e02-9180-c36d2f3bb401",
    label: "Crossref Unixref XML",
    creator: "Sebastian Karcher",
    target: "xml",
    min_version: "3.0",
    priority: 100,
    translator_type: translator_type::IMPORT,
    config_options: &[("dataMode", HeaderValue::Str("xml/dom"))],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2024-10-30 12:58:25",
};

const NO_NS: &[(&str, &str)] = &[];

fn re(cell: &'static OnceLock<Regex>, p: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(p).expect("static regex"))
}

/// `innerXML` (:47-61): the element's `innerHTML`, newlines removed, the
/// four entities `&quot; &lt; &gt; &amp;` decoded in one pass.
fn inner_xml(doc: &XmlDocument, n: XNode) -> Result<String, TranslateError> {
    let html = doc
        .inner_html(n.node())
        .map_err(TranslateError::Translator)?
        .replace('\n', "");
    static R: OnceLock<Regex> = OnceLock::new();
    Ok(re(&R, "(&quot;|&lt;|&gt;|&amp;)")
        .replace_all(&html, |c: &Captures| match &c[1] {
            "&quot;" => "\"",
            "&lt;" => "<",
            "&gt;" => ">",
            _ => "&",
        })
        .into_owned())
}

/// `removeUnsupportedMarkup` (:63-86).
fn remove_unsupported_markup(text: &str) -> String {
    static CDATA: OnceLock<Regex> = OnceLock::new();
    static MARKUP: OnceLock<Regex> = OnceLock::new();
    let t = re(&CDATA, r"<!\[CDATA\[((?s:.)*?)\]\]>").replace_all(text, "$1");
    re(&MARKUP, r"(?i)<(/?)([A-Za-z0-9_]+)[^<>]*>")
        .replace_all(&t, |c: &Captures| {
            let close = !c[1].is_empty();
            let name = c[2].to_lowercase();
            if ["i", "b", "sub", "sup", "span", "sc"].contains(&name.as_str()) {
                return c[0].to_owned();
            }
            if name == "scp" {
                return if close {
                    "</span>".to_owned()
                } else {
                    "<span style=\"font-variant:small-caps;\">".to_owned()
                };
            }
            String::new()
        })
        .into_owned()
}

/// `Zotero.Utilities.capitalizeName` (utilities.js:199-216): each
/// space-separated part that is all upper or all lower case is lower-cased
/// and every letter that starts the part or follows a non-letter is
/// upper-cased (`XRegExp('(^|[^\\pL])\\pL', 'g')`, the whole match
/// upper-cased).
fn capitalize_name(s: &str) -> String {
    static R: OnceLock<Regex> = OnceLock::new();
    let r = re(&R, r"(^|[^\p{L}])\p{L}");
    s.split(' ')
        .map(|part| {
            if part.to_uppercase() == part || part.to_lowercase() == part {
                r.replace_all(&part.to_lowercase(), |c: &Captures| c[0].to_uppercase())
                    .into_owned()
            } else {
                part.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `fixAuthorCapitalization` (:89-101): `ZU.capitalizeName` exists in the
/// server's utilities, so that branch is taken; a non-string (null) is
/// returned as it is.
fn fix_author_capitalization(s: Option<String>) -> Option<String> {
    s.map(|s| capitalize_name(&s))
}

/// A `typeOverrideMap` (:103): `None` is the map absent (or the string
/// `"author"` upstream passes, which has no role keys); entries map a role to
/// a creator type, or to `None` (JS `null`: skip the contributor).
type Overrides = &'static [(&'static str, Option<&'static str>)];

/// `parseCreators` (:103-134).
fn parse_creators(
    doc: &XmlDocument,
    nodes: &[XNode],
    item: &mut TranslatorItem,
    overrides: Overrides,
) -> Result<(), TranslateError> {
    let mut contributors = xpath_all(
        doc,
        nodes,
        "contributors/organization | contributors/person_name",
        NO_NS,
    )?;
    if contributors.is_empty() {
        contributors = xpath_all(doc, nodes, "organization | person_name", NO_NS)?;
    }
    for c in contributors {
        let n = c.node();
        let role = doc.get_attribute(n, "contributor_role");
        let creator_type: Option<String> = match overrides.iter().find(|(r, _)| Some(*r) == role) {
            Some((_, t)) => t.map(str::to_owned),
            None => match role {
                Some(r @ ("author" | "editor" | "translator")) => Some(r.to_owned()),
                _ => Some("contributor".to_owned()),
            },
        };
        let Some(creator_type) = creator_type else {
            continue;
        };
        let mut creator = TranslatorCreator {
            creator_type: Some(creator_type),
            ..Default::default()
        };
        let name = doc.node_name(c);
        if name == "organization" {
            creator.field_mode = Some(1);
            creator.last_name = Some(doc.text(c));
        } else if name == "person_name" {
            creator.first_name =
                fix_author_capitalization(xpath_text(doc, c, "given_name", NO_NS, None)?);
            creator.last_name =
                fix_author_capitalization(xpath_text(doc, c, "surname", NO_NS, None)?);
            if !creator.first_name.as_deref().is_some_and(|f| !f.is_empty()) {
                creator.field_mode = Some(1);
            }
        }
        item.creators.push(creator);
    }
    Ok(())
}

/// `parseDate` (:136-158).
fn parse_date(doc: &XmlDocument, nodes: &[XNode]) -> Result<Option<String>, TranslateError> {
    let Some(&n) = nodes.first() else {
        return Ok(None);
    };
    let year = xpath_text(doc, n, "year", NO_NS, None)?.filter(|s| !s.is_empty());
    let month = xpath_text(doc, n, "month", NO_NS, None)?.filter(|s| !s.is_empty());
    let day = xpath_text(doc, n, "day", NO_NS, None)?.filter(|s| !s.is_empty());
    Ok(year.map(|y| match (month, day) {
        (Some(m), Some(d)) => format!("{y}-{m}-{d}"),
        (Some(m), None) => format!("{m}/{y}"),
        _ => y,
    }))
}

/// `detectImport` (:161-175): `<crossref>` on one of the first nine
/// non-empty lines.
pub fn detect_import(ctx: &mut ImportContext) -> bool {
    let mut i = 0;
    while let Some(line) = ctx.read_line() {
        if !line.is_empty() {
            if line.contains("<crossref>") {
                return true;
            }
            let old = i;
            i += 1;
            if old > 7 {
                return false;
            }
        }
    }
    false
}

fn set(item: &mut TranslatorItem, key: &str, v: Option<String>) {
    item.set(key, v.map_or(Value::Null, Value::from));
}

/// JavaScript `decodeURIComponent(escape(s))` (:466): `escape` writes a
/// UTF-16 unit below U+0100 as that byte (`%XX` or itself) and any other as
/// `%uXXXX`, which `decodeURIComponent` rejects; so the result is the
/// characters read as Latin-1 bytes decoded as UTF-8, or an error.
fn decode_escape(s: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(s.len());
    for c in s.chars() {
        let u = c as u32;
        if u > 0xFF {
            return None;
        }
        bytes.push(u as u8);
    }
    String::from_utf8(bytes).ok()
}

/// `doImport` (:178-477).
pub fn do_import(ctx: &mut ImportContext) -> Result<(), TranslateError> {
    let doc = get_xml(ctx)?;
    let d = &doc;
    let root = XNode::Node(doc.document());
    let x = |nodes: &[XNode], e: &str| xpath_all(d, nodes, e, NO_NS);
    let t = |nodes: &[XNode], e: &str| xpath_text_all(d, nodes, e, NO_NS, None);

    let doi_record = xpath_all(d, &[root], "//doi_records/doi_record", NO_NS)?;
    // :186-190
    if let Some(e) = t(&doi_record, "crossref/error")? {
        return Err(TranslateError::Translator(e));
    }

    let mut item: TranslatorItem;
    let mut ref_xml: Vec<XNode> = Vec::new();
    let mut metadata_xml: Vec<XNode> = Vec::new();
    let mut series_xml: Vec<XNode> = Vec::new();
    let mut item_xml: Vec<XNode>;

    if {
        item_xml = x(&doi_record, "crossref/journal")?;
        !item_xml.is_empty()
    } {
        item = TranslatorItem::new("journalArticle");
        ref_xml = x(&item_xml, "journal_article")?;
        metadata_xml = x(&item_xml, "journal_metadata")?;
        set(
            &mut item,
            "publicationTitle",
            t(&metadata_xml, "full_title[1]")?,
        );
        set(
            &mut item,
            "journalAbbreviation",
            t(&metadata_xml, "abbrev_title[1]")?,
        );
        set(
            &mut item,
            "volume",
            t(&item_xml, "journal_issue/journal_volume/volume")?,
        );
        set(
            &mut item,
            "issue",
            t(&item_xml, "journal_issue/journal_volume/issue")?,
        );
        if !item.truthy("issue") {
            set(&mut item, "issue", t(&item_xml, "journal_issue/issue")?);
        }
    } else if {
        item_xml = x(&doi_record, "crossref/report-paper")?;
        !item_xml.is_empty()
    } {
        item = TranslatorItem::new("report");
        ref_xml = x(&item_xml, "report-paper_metadata")?;
        if ref_xml.is_empty() {
            ref_xml = x(&item_xml, "report-paper_series_metadata")?;
            series_xml = x(&ref_xml, "series_metadata")?;
        }
        metadata_xml = ref_xml.clone();
        set(
            &mut item,
            "reportNumber",
            t(&ref_xml, "publisher_item/item_number")?,
        );
        if !item.truthy("reportNumber") {
            set(&mut item, "reportNumber", t(&ref_xml, "volume")?);
        }
        set(
            &mut item,
            "institution",
            t(&ref_xml, "publisher/publisher_name")?,
        );
        set(
            &mut item,
            "place",
            t(&ref_xml, "publisher/publisher_place")?,
        );
    } else if {
        item_xml = x(&doi_record, "crossref/book")?;
        !item_xml.is_empty()
    } {
        let b0 = item_xml[0].node();
        let book_type = d.get_attribute(b0, "book_type").map(str::to_owned);
        let component_type =
            xpath_text(d, item_xml[0], "content_item/@component_type", NO_NS, None)?;
        let ct = component_type.as_deref();
        let bt = book_type.as_deref();
        let is_reference = (bt == Some("reference")
            && matches!(ct, Some("chapter" | "reference_entry" | "other")))
            || (bt == Some("other") && matches!(ct, Some("chapter" | "reference_entry")));
        if (bt == Some("edited_book") && ct.is_some_and(|c| !c.is_empty())) || is_reference {
            item = TranslatorItem::new("bookSection");
            ref_xml = x(&item_xml, "content_item")?;
            if is_reference {
                metadata_xml = x(&item_xml, "book_metadata")?;
                if metadata_xml.is_empty() {
                    metadata_xml = x(&item_xml, "book_series_metadata")?;
                }
                set(
                    &mut item,
                    "bookTitle",
                    t(&metadata_xml, "titles[1]/title[1]")?,
                );
                set(
                    &mut item,
                    "seriesTitle",
                    t(&metadata_xml, "series_metadata/titles[1]/title[1]")?,
                );
                let msx = x(&metadata_xml, "series_metadata")?;
                if !msx.is_empty() {
                    parse_creators(d, &msx, &mut item, &[("editor", Some("seriesEditor"))])?;
                }
            } else {
                metadata_xml = x(&item_xml, "book_series_metadata")?;
                if metadata_xml.is_empty() {
                    metadata_xml = x(&item_xml, "book_metadata")?;
                }
                set(
                    &mut item,
                    "bookTitle",
                    t(&metadata_xml, "series_metadata/titles[1]/title[1]")?,
                );
                if !item.truthy("bookTitle") {
                    set(
                        &mut item,
                        "bookTitle",
                        t(&metadata_xml, "titles[1]/title[1]")?,
                    );
                }
            }
            parse_creators(
                d,
                &metadata_xml,
                &mut item,
                &[("author", Some("bookAuthor"))],
            )?;
        } else {
            item = TranslatorItem::new("book");
            ref_xml = x(&item_xml, "book_metadata")?;
            if ref_xml.is_empty() {
                ref_xml = x(&item_xml, "book_series_metadata")?;
            }
            if ref_xml.is_empty() {
                ref_xml = x(&item_xml, "book_set_metadata")?;
            }
            metadata_xml = ref_xml.clone();
            series_xml = x(&ref_xml, "series_metadata")?;
        }
        set(
            &mut item,
            "place",
            t(&metadata_xml, "publisher/publisher_place")?,
        );
    } else if {
        item_xml = x(&doi_record, "crossref/standard")?;
        !item_xml.is_empty()
    } {
        item = TranslatorItem::new("standard");
        ref_xml = x(&item_xml, "standard_metadata")?;
        metadata_xml = x(&item_xml, "standard_metadata")?;
    } else if {
        item_xml = x(&doi_record, "crossref/conference")?;
        !item_xml.is_empty()
    } {
        item = TranslatorItem::new("conferencePaper");
        ref_xml = x(&item_xml, "conference_paper")?;
        metadata_xml = x(&item_xml, "proceedings_metadata")?;
        series_xml = x(&metadata_xml, "proceedings_metadata")?;
        set(
            &mut item,
            "publicationTitle",
            t(&metadata_xml, "proceedings_title")?,
        );
        set(
            &mut item,
            "place",
            t(&item_xml, "event_metadata/conference_location")?,
        );
        set(
            &mut item,
            "conferenceName",
            t(&item_xml, "event_metadata/conference_name")?,
        );
    } else if {
        item_xml = x(&doi_record, "crossref/database")?;
        !item_xml.is_empty()
    } {
        item = TranslatorItem::new("dataset");
        ref_xml = x(&item_xml, "dataset")?;
        metadata_xml = x(&item_xml, "database_metadata")?;
        let mut pub_date = x(&ref_xml, "database_date/publication_date")?;
        if pub_date.is_empty() {
            pub_date = x(&metadata_xml, "database_date/publication_date")?;
        }
        set(&mut item, "date", parse_date(d, &pub_date)?);
        if !js::truthy(t(&ref_xml, "contributors")?.map(Value::from).as_ref()) {
            parse_creators(d, &metadata_xml, &mut item, &[])?;
        }
        if !js::truthy(t(&metadata_xml, "publisher")?.map(Value::from).as_ref()) {
            set(
                &mut item,
                "institution",
                t(&metadata_xml, "institution/institution_name")?,
            );
        }
    } else if {
        item_xml = x(&doi_record, "crossref/dissertation")?;
        !item_xml.is_empty()
    } {
        item = TranslatorItem::new("thesis");
        let ad = x(&item_xml, "approval_date[1]")?;
        set(&mut item, "date", parse_date(d, &ad)?);
        set(
            &mut item,
            "university",
            t(&item_xml, "institution/institution_name")?,
        );
        set(
            &mut item,
            "place",
            t(&item_xml, "institution/institution_place")?,
        );
        if let Some(ty) = t(&item_xml, "degree")?.filter(|s| !s.is_empty()) {
            static R: OnceLock<Regex> = OnceLock::new();
            item.set(
                "thesisType",
                re(&R, r"\(.+\)").replace(&ty, "").into_owned(),
            );
        }
    } else if {
        item_xml = x(&doi_record, "crossref/posted_content")?;
        !item_xml.is_empty()
    } {
        let ty = t(&item_xml, "./@type")?;
        if ty.as_deref() == Some("preprint") {
            item = TranslatorItem::new("preprint");
            set(&mut item, "repository", t(&item_xml, "group_title")?);
        } else {
            item = TranslatorItem::new("blogPost");
            set(
                &mut item,
                "blogTitle",
                t(&item_xml, "institution/institution_name")?,
            );
        }
        let pd = x(&item_xml, "posted_date")?;
        set(&mut item, "date", parse_date(d, &pd)?);
    } else if {
        item_xml = x(&doi_record, "crossref/peer_review")?;
        !item_xml.is_empty()
    } {
        item = TranslatorItem::new("manuscript");
        let rd = x(&item_xml, "reviewed_date")?;
        set(&mut item, "date", parse_date(d, &rd)?);
        // :363 `if (ZU.xpath(...))`: an array, always truthy.
        x(&item_xml, "/contributors/anonymous")?;
        let mut anon = TranslatorCreator::single("Anonymous Reviewer", "author");
        anon.first_name = None;
        item.creators.push(anon);
        item.set("type", "peer review");
        if let Some(review_of) =
            t(&item_xml, "//related_item/inter_work_relation")?.filter(|s| !s.is_empty())
        {
            let it = t(
                &item_xml,
                "//related_item/inter_work_relation/@identifier-type",
            )?;
            let identifier = match it.as_deref() {
                Some("doi") => format!(
                    "<a href=\"https://doi.org/{review_of}\">https://doi.org/{review_of}</a>"
                ),
                Some("url") => format!("<a href=\"{review_of}\">{review_of}</a>"),
                _ => review_of,
            };
            item.notes
                .push(TranslatorNote::new(format!("Review of {identifier}")));
        }
    } else {
        item = TranslatorItem::new("document");
    }

    if ref_xml.is_empty() {
        ref_xml = item_xml.clone();
    }
    if metadata_xml.is_empty() {
        metadata_xml = ref_xml.clone();
    }

    set(
        &mut item,
        "abstractNote",
        t(&ref_xml, "description|abstract")?,
    );
    set(&mut item, "language", t(&metadata_xml, "./@language")?);
    set(&mut item, "ISBN", t(&metadata_xml, "isbn")?);
    set(&mut item, "ISSN", t(&metadata_xml, "issn")?);
    set(
        &mut item,
        "publisher",
        t(&metadata_xml, "publisher/publisher_name")?,
    );
    set(&mut item, "edition", t(&metadata_xml, "edition_number")?);
    if !item.truthy("volume") {
        set(&mut item, "volume", t(&metadata_xml, "volume")?);
    }

    // :421 `typeOverrideMap` is `{editor: null}` for a book section, else the
    // string "author" (no role keys).
    if item.item_type == "bookSection" {
        parse_creators(d, &ref_xml, &mut item, &[("editor", None)])?;
    } else {
        parse_creators(d, &ref_xml, &mut item, &[])?;
    }

    if !series_xml.is_empty() {
        parse_creators(
            d,
            &series_xml,
            &mut item,
            &[("editor", Some("seriesEditor"))],
        )?;
        set(&mut item, "series", t(&series_xml, "titles[1]/title[1]")?);
        set(&mut item, "seriesNumber", t(&series_xml, "series_number")?);
        set(
            &mut item,
            "reportType",
            t(&series_xml, "titles[1]/title[1]")?,
        );
    }
    let mut pub_date = x(&ref_xml, "publication_date[@media_type=\"print\"]")?;
    if pub_date.is_empty() {
        pub_date = x(&ref_xml, "publication_date")?;
    }
    if pub_date.is_empty() {
        pub_date = x(&metadata_xml, "publication_date[@media_type=\"print\"]")?;
    }
    if pub_date.is_empty() {
        pub_date = x(&metadata_xml, "publication_date")?;
    }
    if !pub_date.is_empty() {
        set(&mut item, "date", parse_date(d, &pub_date)?);
    }

    let pages = x(&ref_xml, "pages[1]")?;
    if !pages.is_empty() {
        let first = t(&pages, "first_page[1]")?;
        let last = t(&pages, "last_page[1]")?.filter(|s| !s.is_empty());
        match last {
            // `null + "-" + last` is "null-last".
            Some(l) => item.set(
                "pages",
                format!("{}-{l}", first.unwrap_or_else(|| "null".into())),
            ),
            None => set(&mut item, "pages", first),
        }
    } else {
        set(
            &mut item,
            "pages",
            t(&ref_xml, "publisher_item/item_number")?,
        );
    }

    set(&mut item, "DOI", t(&ref_xml, "doi_data/doi")?);
    if let Some(doi) = item
        .get_str("DOI")
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
    {
        if !field_is_valid_for_type("DOI", &item.item_type) {
            match item
                .get_str("extra")
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
            {
                Some(e) => item.set("extra", format!("{e}\nDOI: {doi}")),
                None => item.set("extra", format!("DOI: {doi}")),
            }
        }
    }
    set(&mut item, "rights", t(&ref_xml, "program/license_ref[1]")?);
    set(&mut item, "url", t(&ref_xml, "doi_data/resource")?);
    let mut title = x(&ref_xml, "titles[1]/title[1]")?.first().copied();
    if title.is_none() {
        title = x(&metadata_xml, "titles[1]/title[1]")?.first().copied();
    }
    if let Some(tn) = title {
        let mut tt = trim_internal(&remove_unsupported_markup(&inner_xml(d, tn)?));
        if let Some(&sub) = x(&ref_xml, "titles[1]/subtitle[1]")?.first() {
            let tt0 = tt.strip_suffix(':').unwrap_or(&tt).to_owned();
            tt = format!(
                "{tt0}: {}",
                trim_internal(&remove_unsupported_markup(&inner_xml(d, sub)?))
            );
        }
        static SUB: OnceLock<Regex> = OnceLock::new();
        let r = re(&SUB, &format!(r"{}+<(sub|sup)>", js::WS));
        item.set("title", r.replace_all(&tt, "<$1>").into_owned());
    }
    if !item.truthy("title") {
        item.set("title", "[No title found]");
    }

    // :455-475: repair mojibake (UTF-8 read as Latin-1) in string fields.
    static CTRL: OnceLock<Regex> = OnceLock::new();
    static STRIP: OnceLock<Regex> = OnceLock::new();
    let ctrl = re(&CTRL, r"[\x{7F}-\x{9F}]");
    let strip = re(&STRIP, r"[\x{00}-\x{1F}\x{7F}-\x{9F}]");
    let keys: Vec<String> = item.props.keys().map(str::to_owned).collect();
    for k in keys {
        let Some(s) = item.get_str(&k).map(str::to_owned) else {
            continue;
        };
        if ctrl.is_match(&s) {
            let v = decode_escape(&s).unwrap_or_else(|| strip.replace_all(&s, "").into_owned());
            item.set(k, v);
        }
    }
    if ctrl.is_match(&item.item_type) {
        item.item_type = decode_escape(&item.item_type)
            .unwrap_or_else(|| strip.replace_all(&item.item_type, "").into_owned());
    }
    ctx.item_done(item);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capitalize_name_like_upstream() {
        // utilities.js :185-195 examples.
        assert_eq!(capitalize_name("JOHN"), "John");
        assert_eq!(capitalize_name("GUTIÉRREZ-ALBILLA"), "Gutiérrez-Albilla");
        assert_eq!(capitalize_name("O'NEAL"), "O'Neal");
        assert_eq!(capitalize_name("O'neal"), "O'neal");
        assert_eq!(
            capitalize_name("martha McMiddlename WASHINGTON"),
            "Martha McMiddlename Washington"
        );
    }

    #[test]
    fn markup_and_mojibake() {
        assert_eq!(
            remove_unsupported_markup("a <i>b</i> <jats:p>c</jats:p> <scp>d</scp>"),
            "a <i>b</i> c <span style=\"font-variant:small-caps;\">d</span>"
        );
        assert_eq!(decode_escape("\u{e2}\u{80}\u{93}").as_deref(), Some("–"));
    }
}
