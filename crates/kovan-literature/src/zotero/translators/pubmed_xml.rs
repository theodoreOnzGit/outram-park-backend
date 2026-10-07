// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "PubMed XML.js" (translatorID
//   fcf41bed-0cbc-3704-85c7-8062a0068a7a, lastUpdated 2026-05-21 14:52:55):
//   `detectImport` :44-47, `processAuthors` :49-90, `doImport` :92-397.
// Copyright (c) 2017-2020 Simon Kornblith, Michael Berkowitz, Avram Lyon,
//   Sebastian Karcher, and Rintze Zelle.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The PubMed XML translator (import): `PubmedArticleSet` (articles and
//! NCBI Bookshelf books and chapters).

use crate::zotero::framework::js;
use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::title_case::capitalize_title;
use crate::zotero::framework::utilities::{clean_author, str_to_iso, trim_internal};
use crate::zotero::framework::xml::{get_xml, XNode, XmlDocument};
use crate::zotero::framework::xpath::{xpath, xpath_text};
use crate::zotero::framework::{
    ImportContext, JsObject, TranslateError, TranslatorCreator, TranslatorItem, TranslatorTag,
};
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "fcf41bed-0cbc-3704-85c7-8062a0068a7a",
    label: "PubMed XML",
    creator: "Simon Kornblith, Michael Berkowitz, Avram Lyon, and Rintze Zelle",
    target: "xml",
    min_version: "2.1.9",
    priority: 100,
    translator_type: translator_type::IMPORT,
    config_options: &[("dataMode", HeaderValue::Str("xml/dom"))],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2026-05-21 14:52:55",
};

const NO_NS: &[(&str, &str)] = &[];

fn re(cell: &'static OnceLock<Regex>, p: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(p).expect("static regex"))
}

/// `detectImport` (:44-47).
pub fn detect_import(ctx: &mut ImportContext) -> bool {
    ctx.read_chars(1000)
        .is_some_and(|t| t.contains("<PubmedArticleSet>"))
}

fn set(item: &mut TranslatorItem, key: &str, v: Option<String>) {
    item.set(key, v.map_or(Value::Null, Value::from));
}

fn text(doc: &XmlDocument, n: XNode, e: &str) -> Result<Option<String>, TranslateError> {
    xpath_text(doc, n, e, NO_NS, None)
}

/// `processAuthors` (:49-90).
fn process_authors(
    doc: &XmlDocument,
    item: &mut TranslatorItem,
    lists: &[XNode],
) -> Result<(), TranslateError> {
    for &list in lists {
        let ty = if doc.get_attribute(list.node(), "Type") == Some("editors") {
            "editor"
        } else {
            "author"
        };
        for author in xpath(doc, list, "Author", NO_NS)? {
            let last = text(doc, author, "LastName")?;
            let mut first = text(doc, author, "FirstName")?.filter(|s| !s.is_empty());
            if first.is_none() {
                first = text(doc, author, "ForeName")?;
            }
            let suffix = text(doc, author, "Suffix")?.filter(|s| !s.is_empty());
            if let (Some(s), Some(f)) = (&suffix, &first) {
                if !f.is_empty() {
                    first = Some(format!("{f}, {s}"));
                }
            }
            let truthy = |o: &Option<String>| o.as_deref().is_some_and(|s| !s.is_empty());
            if truthy(&first) || truthy(&last) {
                // `lastName + ', ' + firstName`: null is "null".
                let s = format!(
                    "{}, {}",
                    last.as_deref().unwrap_or("null"),
                    first.as_deref().unwrap_or("null")
                );
                let c = clean_author(&s, ty, true);
                let mut last_name = c.last_name;
                if last_name.to_uppercase() == last_name {
                    last_name = capitalize_title(&last_name, true);
                }
                // `creator.firstName.toUpperCase()` throws on undefined.
                let mut first_name = c.first_name.ok_or_else(|| {
                    TranslateError::Translator(
                        "Cannot read properties of undefined (reading 'toUpperCase')".into(),
                    )
                })?;
                if first_name.to_uppercase() == first_name {
                    first_name = capitalize_title(&first_name, true);
                }
                item.creators.push(TranslatorCreator {
                    first_name: Some(first_name),
                    last_name: Some(last_name),
                    creator_type: Some(c.creator_type),
                    ..Default::default()
                });
            } else if let Some(l) = text(doc, author, "CollectiveName")?.filter(|s| !s.is_empty()) {
                item.creators.push(TranslatorCreator::single(l, ty));
            }
        }
    }
    Ok(())
}

/// The abbreviated page ranges of `MedlinePgn` written out in full
/// (:113-133: "123-9" -> "123-129"), with upstream's `lastIndex` handling.
fn expand_page_ranges(s: &str) -> String {
    static R: OnceLock<Regex> = OnceLock::new();
    let r = re(&R, r"([0-9]+)-([0-9]+)");
    let mut full = s.to_owned();
    let mut last_index = 0usize;
    while last_index <= full.len() {
        let Some(c) = r.captures_at(&full, last_index) else {
            break;
        };
        let m = c.get(0).unwrap();
        let (start, end) = (c[1].to_owned(), c[2].to_owned());
        last_index = m.end();
        if start.len() > end.len() {
            let diff = start.len() - end.len();
            let new_range = format!("{start}-{}{end}", &start[..diff]);
            let grow = new_range.len() - m.as_str().len();
            full = format!("{}{new_range}{}", &full[..m.start()], &full[m.end()..]);
            last_index += grow;
        }
    }
    full
}

fn pubmed_attachment(pmid: &str) -> JsObject {
    let mut a = JsObject::new();
    a.set("title", "PubMed entry");
    a.set("url", format!("http://www.ncbi.nlm.nih.gov/pubmed/{pmid}"));
    a.set("mimeType", "text/html");
    a.set("snapshot", false);
    a
}

/// `title.charAt(title.length - 1) == "."`: drop one trailing period.
fn drop_period(t: String) -> String {
    match t.strip_suffix('.') {
        Some(s) => s.to_owned(),
        None => t,
    }
}

/// `doImport` (:92-397).
pub fn do_import(ctx: &mut ImportContext) -> Result<(), TranslateError> {
    let doc = get_xml(ctx)?;
    let d = &doc;
    let root = XNode::Node(doc.document());
    let dates = ctx.options.env.dates.clone();

    for art in xpath(d, root, "/PubmedArticleSet/PubmedArticle", NO_NS)? {
        let citation = xpath(d, art, "MedlineCitation", NO_NS)?
            .first()
            .copied()
            .ok_or_else(|| TranslateError::Translator("no MedlineCitation".into()))?;
        let article = xpath(d, citation, "Article", NO_NS)?
            .first()
            .copied()
            .ok_or_else(|| TranslateError::Translator("no Article".into()))?;
        let is_preprint = !xpath(
            d,
            article,
            "PublicationTypeList/PublicationType[@UI=\"D000076942\"]",
            NO_NS,
        )?
        .is_empty();
        let mut item = TranslatorItem::new(if is_preprint {
            "preprint"
        } else {
            "journalArticle"
        });

        let mut title = text(d, article, "ArticleTitle")?.filter(|s| !s.is_empty());
        if title.is_none() {
            title = text(d, article, "VernacularTitle")?.filter(|s| !s.is_empty());
        }
        if let Some(t) = title {
            item.set("title", drop_period(t));
        }

        if let Some(p) = text(d, article, "Pagination/MedlinePgn")?.filter(|s| !s.is_empty()) {
            item.set("pages", expand_page_ranges(&p));
        }
        if item.item_type == "journalArticle" {
            if !item.truthy("pages") {
                set(
                    &mut item,
                    "pages",
                    text(d, article, "ELocationID[@EIdType=\"pii\"]")?,
                );
            }
        } else if item.item_type == "preprint" {
            set(
                &mut item,
                "archiveID",
                text(d, article, "ELocationID[@EIdType=\"pii\"]")?,
            );
        }

        if let Some(&journal) = xpath(d, article, "Journal", NO_NS)?.first() {
            set(&mut item, "ISSN", text(d, journal, "ISSN")?);
            if let Some(a) = text(d, journal, "ISOAbbreviation")?.filter(|s| !s.is_empty()) {
                item.set("journalAbbreviation", a);
            } else if let Some(a) = text(d, article, "//MedlineTA")?.filter(|s| !s.is_empty()) {
                item.set("journalAbbreviation", a);
            }
            if let Some(t) = text(d, journal, "Title")?.filter(|s| !s.is_empty()) {
                let mut t = trim_internal(&t);
                // :167-171: a parenthesised place at the end goes to place.
                static PLACE: OnceLock<Regex> = OnceLock::new();
                if let Some(c) = re(&PLACE, r" \(([^)]+)\)$").captures(&t) {
                    let place = c[1].to_owned();
                    let start = c.get(0).unwrap().start();
                    item.set("place", place);
                    t.truncate(start);
                }
                static ACR: OnceLock<Regex> = OnceLock::new();
                static WS: OnceLock<Regex> = OnceLock::new();
                let acronym = re(&ACR, r"(?-u:\b)[A-Z]{2}").is_match(&t);
                let has_space = re(&WS, js::WS).is_match(&t);
                if !(acronym && (t.to_uppercase() != t || !has_space)) {
                    t = capitalize_title(&t, true);
                }
                item.set("publicationTitle", t);
            } else if let Some(a) = item
                .get_str("journalAbbreviation")
                .filter(|s| !s.is_empty())
            {
                let a = a.to_owned();
                item.set("publicationTitle", a);
            }
            if let Some(p) = item.get_str("publicationTitle").filter(|s| !s.is_empty()) {
                // Not forced: the server has no `capitalizeTitles` pref.
                let p = capitalize_title(p, false);
                item.set("publicationTitle", p);
            }
            if item.item_type == "preprint" {
                let p = item.get("publicationTitle").cloned().unwrap_or(Value::Null);
                item.set("repository", p);
                item.remove("publicationTitle");
                item.remove("journalAbbreviation");
            }

            if let Some(&issue) = xpath(d, journal, "JournalIssue", NO_NS)?.first() {
                set(&mut item, "volume", text(d, issue, "Volume")?);
                set(&mut item, "issue", text(d, issue, "Issue")?);
                if let Some(&pd) = xpath(d, issue, "PubDate", NO_NS)?.first() {
                    let day = text(d, pd, "Day")?.filter(|s| !s.is_empty());
                    let month = text(d, pd, "Month")?.filter(|s| !s.is_empty());
                    let year = text(d, pd, "Year")?.filter(|s| !s.is_empty());
                    let y = year.clone().unwrap_or_else(|| "null".into());
                    let date = if let Some(day) = day {
                        static NUM: OnceLock<Regex> = OnceLock::new();
                        match &month {
                            Some(m) if re(&NUM, "[0-9]+").is_match(m) => {
                                str_to_iso(&format!("{y}-{m}-{day}"), &dates)
                            }
                            m => str_to_iso(
                                &format!("{} {day}, {y}", m.as_deref().unwrap_or("null")),
                                &dates,
                            ),
                        }
                    } else if let Some(m) = month {
                        str_to_iso(&format!("{m}/{y}"), &dates)
                    } else if year.is_some() {
                        year
                    } else {
                        text(d, pd, "MedlineDate")?
                    };
                    // `strToISO` returns false when nothing parses.
                    match date {
                        Some(v) => item.set("date", v),
                        None => item.set("date", false),
                    }
                }
            }
        }

        let lists = xpath(d, article, "AuthorList", NO_NS)?;
        process_authors(d, &mut item, &lists)?;

        set(&mut item, "language", text(d, article, "Language")?);

        for k in xpath(d, citation, "MeshHeadingList/MeshHeading", NO_NS)? {
            if let Some(t) = text(d, k, "DescriptorName")? {
                item.tags.push(TranslatorTag::new(t));
            }
        }
        for k in xpath(d, citation, "KeywordList/Keyword", NO_NS)? {
            item.tags.push(TranslatorTag::new(d.text(k)));
        }
        let mut abstract_note = String::new();
        for sec in xpath(d, article, "Abstract/AbstractText", NO_NS)? {
            let mut para = js::trim(&d.text(sec)).to_owned();
            if !para.is_empty() {
                para.push('\n');
            }
            if let Some(label) = d.get_attribute(sec.node(), "Label") {
                if !label.is_empty() && label != "UNLABELLED" {
                    para = format!("{label}: {para}");
                }
            }
            abstract_note.push_str(&para);
        }
        item.set("abstractNote", abstract_note);

        set(
            &mut item,
            "DOI",
            text(
                d,
                art,
                "PubmedData/ArticleIdList/ArticleId[@IdType=\"doi\"]",
            )?,
        );
        let pmid = text(d, citation, "PMID")?.filter(|s| !s.is_empty());
        let pmcid = text(
            d,
            art,
            "PubmedData/ArticleIdList/ArticleId[@IdType=\"pmc\"]",
        )?
        .filter(|s| !s.is_empty());
        if let Some(p) = pmid {
            item.set("PMID", p.clone());
            item.attachments.push(pubmed_attachment(&p));
        }
        if let Some(p) = pmcid {
            item.set("PMCID", p);
        }
        ctx.item_done(item);
    }

    for book_article in xpath(d, root, "/PubmedArticleSet/PubmedBookArticle", NO_NS)? {
        let citation = xpath(d, book_article, "BookDocument", NO_NS)?
            .first()
            .copied()
            .ok_or_else(|| TranslateError::Translator("no BookDocument".into()))?;
        let section_title = text(d, citation, "ArticleTitle")?.filter(|s| !s.is_empty());
        let is_section = section_title.is_some();
        let mut item = TranslatorItem::new(if is_section { "bookSection" } else { "book" });
        if let Some(s) = section_title {
            item.set("title", s);
        }
        let book = xpath(d, citation, "Book", NO_NS)?
            .first()
            .copied()
            .ok_or_else(|| TranslateError::Translator("no Book".into()))?;
        if let Some(t) = text(d, book, "BookTitle")?.filter(|s| !s.is_empty()) {
            let t = drop_period(t);
            item.set(
                if is_section {
                    "publicationTitle"
                } else {
                    "title"
                },
                t,
            );
        }
        set(&mut item, "date", text(d, book, "PubDate/Year")?);
        set(&mut item, "edition", text(d, book, "Edition")?);
        set(&mut item, "series", text(d, book, "CollectionTitle")?);
        set(&mut item, "volume", text(d, book, "Volume")?);
        set(
            &mut item,
            "place",
            text(d, book, "Publisher/PublisherLocation")?,
        );
        set(
            &mut item,
            "publisher",
            text(d, book, "Publisher/PublisherName")?,
        );
        if is_section {
            let lists = xpath(d, citation, "AuthorList", NO_NS)?;
            process_authors(d, &mut item, &lists)?;
        }
        let lists = xpath(d, book, "AuthorList", NO_NS)?;
        process_authors(d, &mut item, &lists)?;
        set(&mut item, "language", text(d, citation, "Language")?);
        set(
            &mut item,
            "abstractNote",
            text(d, citation, "Abstract/AbstractText")?,
        );
        set(
            &mut item,
            "rights",
            text(d, citation, "Abstract/CopyrightInformation")?,
        );
        set(&mut item, "ISBN", text(d, book, "Isbn")?);
        let pmid = text(d, citation, "PMID")?.filter(|s| !s.is_empty());
        if let Some(p) = &pmid {
            item.set("extra", format!("PMID: {p}"));
            item.attachments.push(pubmed_attachment(p));
        }
        set(
            &mut item,
            "callNumber",
            text(
                d,
                citation,
                "ArticleIdList/ArticleId[@IdType=\"bookaccession\"]",
            )?,
        );
        if let Some(cn) = item
            .get_str("callNumber")
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
        {
            let url = format!("http://www.ncbi.nlm.nih.gov/books/{cn}/");
            let mut a = JsObject::new();
            if pmid.is_some() {
                item.set("url", url);
                if item.item_type == "bookSection" {
                    a.set("title", "Printable HTML");
                    a.set(
                        "url",
                        format!("http://www.ncbi.nlm.nih.gov/books/{cn}/?report=printable"),
                    );
                    a.set("mimeType", "text/html");
                    a.set("snapshot", true);
                    item.attachments.push(a);
                }
            } else {
                a.set("title", "NCBI Bookshelf entry");
                a.set("url", url);
                a.set("mimeType", "text/html");
                a.set("snapshot", false);
                item.attachments.push(a);
            }
        }
        ctx.item_done(item);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_ranges_expand() {
        assert_eq!(expand_page_ranges("123-9"), "123-129");
        assert_eq!(expand_page_ranges("1234-56, 7-8"), "1234-1256, 7-8");
        assert_eq!(expand_page_ranges("e123"), "e123");
    }
}
