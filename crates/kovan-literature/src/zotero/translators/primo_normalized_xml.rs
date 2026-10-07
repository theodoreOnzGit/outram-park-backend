// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "Primo Normalized XML.js" (translatorID
//   efd737c9-a227-4113-866e-d57fbc0684ca, lastUpdated 2026-09-01 18:46:48):
//   `detectImport` :41-44, `doImport` :47-384, `stripAuthor` :387-409,
//   `fetchCreators` :411-447, `extractNumPages` :449-466, `dedupeArray`
//   :468-476.
// Copyright (c) 2018 Philipp Zumstein.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The Primo Normalized XML translator (import): Ex Libris Primo PNX
//! records.

use crate::zotero::framework::html::unescape_html;
use crate::zotero::framework::identifiers::{clean_isbn, clean_issn};
use crate::zotero::framework::js;
use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::utilities::{clean_author, trim_internal};
use crate::zotero::framework::xml::{get_xml, XNode};
use crate::zotero::framework::xpath::{xpath, xpath_text};
use crate::zotero::framework::{
    ImportContext, JsObject, TranslateError, TranslatorCreator, TranslatorItem, TranslatorTag,
};
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "efd737c9-a227-4113-866e-d57fbc0684ca",
    label: "Primo Normalized XML",
    creator: "Philipp Zumstein",
    target: "xml",
    min_version: "3.0",
    priority: 100,
    translator_type: translator_type::IMPORT,
    config_options: &[("dataMode", HeaderValue::Str("xml/dom"))],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2026-09-01 18:46:48",
};

const NS: &[(&str, &str)] = &[
    ("p", "http://www.exlibrisgroup.com/xsd/primo/primo_nm_bib"),
    ("sear", "http://www.exlibrisgroup.com/xsd/jaguar/search"),
];

fn re(cell: &'static OnceLock<Regex>, p: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(p).expect("static regex"))
}

/// A JS `\s` class for building patterns.
fn ws() -> &'static str {
    js::WS
}

/// `detectImport` (:41-44).
pub fn detect_import(ctx: &mut ImportContext) -> bool {
    ctx.read_chars(1000)
        .is_some_and(|t| t.contains("http://www.exlibrisgroup.com/xsd/primo/primo_nm_bib"))
}

fn set(item: &mut TranslatorItem, key: &str, v: Option<String>) {
    item.set(key, v.map_or(Value::Null, Value::from));
}

fn nonempty(v: Option<String>) -> Option<String> {
    v.filter(|s| !s.is_empty())
}

/// `stripAuthor` (:387-409).
fn strip_author(s: &str) -> String {
    static Q: OnceLock<Regex> = OnceLock::new();
    static YEAR: OnceLock<Regex> = OnceLock::new();
    static ROLE: OnceLock<Regex> = OnceLock::new();
    static COLON: OnceLock<Regex> = OnceLock::new();
    static NLR: OnceLock<Regex> = OnceLock::new();
    static D0: OnceLock<Regex> = OnceLock::new();
    static D8: OnceLock<Regex> = OnceLock::new();
    let w = ws();
    // `^(.*)\$\$Q(.*)$` (`.` excludes line terminators).
    let s = re(
        &Q,
        r"^([^\n\r\u{2028}\u{2029}]*)\$\$Q([^\n\r\u{2028}\u{2029}]*)$",
    )
    .replace(s, "$2");
    let s = re(
        &YEAR,
        &format!(r"{w}*,?{w}*\(?[0-9]{{4}}-?([0-9]{{4}}|\.{{3}})?\)?"),
    )
    .replace_all(&s, "");
    let s = re(&ROLE, &format!(r"({w}*,?{w}*[\[(][^()]*[\])])+$")).replace(&s, "");
    let s = re(&COLON, &format!(r"{w}*:{w}+")).replace(&s, " ");
    let s = re(&NLR, r"(?-u:\b)NLR10::[^\n\r\u{2028}\u{2029}]*").replace(&s, "");
    let s = re(&D0, r"\$\$0[^\n\r\u{2028}\u{2029}]*").replace(&s, "");
    re(&D8, r"\$\$8[^\n\r\u{2028}\u{2029}]*")
        .replace(&s, "")
        .into_owned()
}

/// `fetchCreators` (:411-447).
fn fetch_creators(
    item: &mut TranslatorItem,
    texts: &[String],
    ty: &str,
    guidance: &HashMap<String, String>,
) {
    static SPLIT: OnceLock<Regex> = OnceLock::new();
    static LANG: OnceLock<Regex> = OnceLock::new();
    let split = re(&SPLIT, &format!(r"{}*;{}*", ws(), ws()));
    let lang = re(&LANG, r"\$\$8([A-Za-z0-9_]+)");
    let mut filter_by: Option<String> = None;
    for t in texts {
        let unescaped = unescape_html(t);
        for c in split.split(&unescaped) {
            if let Some(m) = lang.captures(c) {
                match &filter_by {
                    None => filter_by = Some(m[1].to_owned()),
                    Some(f) if f != &m[1] => continue,
                    _ => {}
                }
            }
            let name = strip_author(c).replace('.', "");
            let key = name.to_lowercase();
            let src = guidance
                .get(&key)
                .filter(|g| !g.is_empty())
                .unwrap_or(&name);
            let a = clean_author(src, ty, true);
            let mut creator = TranslatorCreator {
                first_name: a.first_name.clone(),
                last_name: Some(a.last_name),
                creator_type: Some(a.creator_type),
                ..Default::default()
            };
            if !a.first_name.is_some_and(|f| !f.is_empty()) {
                creator.first_name = None;
                creator.field_mode = Some(1);
            }
            item.creators.push(creator);
        }
    }
}

/// `extractNumPages` (:449-466): the first match only.
fn extract_num_pages(s: &str) -> String {
    static R: OnceLock<Regex> = OnceLock::new();
    static SEP: OnceLock<Regex> = OnceLock::new();
    let r = re(
        &R,
        &format!(
            r"(?i)\[?(?-u:\b)((?:[ivxlcdm0-9]+[ \-,]*)+)\]?{}+[fps](?-u:\b)",
            ws()
        ),
    );
    match r.captures(s) {
        Some(c) => re(&SEP, r"[ \-,]+")
            .replace_all(js::trim(&c[1]), "+")
            .to_lowercase(),
        None => String::new(),
    }
}

/// `doImport` (:47-384).
pub fn do_import(ctx: &mut ImportContext) -> Result<(), TranslateError> {
    let doc = get_xml(ctx)?;
    let d = &doc;
    let root = XNode::Node(doc.document());
    let t = |e: &str| xpath_text(d, root, e, NS, None);
    let x = |e: &str| xpath(d, root, e, NS);

    let mut item = TranslatorItem::new("");
    let item_type = match nonempty(t("//p:display/p:type")?) {
        Some(s) => Some(s),
        None => match nonempty(t("//p:facets/p:rsrctype")?) {
            Some(s) => Some(s),
            None => nonempty(t("//p:search/p:rsrctype")?),
        },
    };
    let Some(item_type) = item_type else {
        return Err(TranslateError::Translator(
            "Could not locate item type".into(),
        ));
    };
    item.item_type = match item_type.to_lowercase().as_str() {
        "book" | "buch" | "ebook" | "pbook" | "pbooks" | "print_book" | "books" | "score"
        | "journal" => "book",
        "book_chapter" => "bookSection",
        "audio" | "audios" | "sound_recording" => "audioRecording",
        "video" | "videos" | "dvd" => "videoRecording",
        "computer_file" => "computerProgram",
        "report" => "report",
        "webpage" => "webpage",
        "article" | "review" => "journalArticle",
        "thesis" | "dissertation" | "dissertations" => "thesis",
        "archive_manuscript"
        | "archival_materials"
        | "manuscripts"
        | "archival_material_manuscript"
        | "object" => "manuscript",
        "map" | "maps" => "map",
        "reference_entry" => "encyclopediaArticle",
        "image" | "images" => "artwork",
        "newspaper_article" => "newspaperArticle",
        "conference_proceeding" | "conference_proceedings" => "conferencePaper",
        _ => {
            let ris = nonempty(t("//p:addata/p:ristype")?);
            if ris.is_some_and(|r| r.to_uppercase() == "THES") {
                "thesis"
            } else {
                "document"
            }
        }
    }
    .to_owned();

    set(&mut item, "title", t("//p:display/p:title")?);
    if let Some(title) = item
        .get_str("title")
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
    {
        static COLON: OnceLock<Regex> = OnceLock::new();
        static SLASH: OnceLock<Regex> = OnceLock::new();
        let title = unescape_html(&title);
        let title = re(&COLON, &format!(r"{}*:", ws())).replace(&title, ":");
        let title = re(&SLASH, r" / [^/]+$").replace(&title, "");
        item.set("title", title.into_owned());
    }
    let mut creators = x("//p:display/p:creator")?;
    let mut contributors = x("//p:display/p:contributor")?;
    if creators.is_empty() && !contributors.is_empty() {
        creators = std::mem::take(&mut contributors);
    }

    let mut guidance: HashMap<String, String> = HashMap::new();
    for au in x("//p:addata/p:addau|//p:addata/p:au")? {
        let author = strip_author(&d.text(au));
        if author.contains(',') {
            let parts: Vec<&str> = author.split(',').collect();
            if parts.len() > 2 {
                continue;
            }
            let name = format!(
                "{} {}",
                js::trim(parts[1]).to_lowercase(),
                js::trim(parts[0]).to_lowercase()
            );
            guidance.insert(name.replace('.', ""), author.clone());
        }
    }
    let texts = |v: &[XNode]| v.iter().map(|&n| d.text(n)).collect::<Vec<_>>();
    fetch_creators(&mut item, &texts(&creators), "author", &guidance);
    fetch_creators(&mut item, &texts(&contributors), "contributor", &guidance);

    set(&mut item, "place", t("//p:addata/p:cop")?);
    let mut publisher = nonempty(t("//p:addata/p:pub")?);
    if publisher.is_none() {
        publisher = nonempty(t("//p:display/p:publisher")?);
    }
    if let Some(p) = publisher {
        static PUB: OnceLock<Regex> = OnceLock::new();
        static QUOTE: OnceLock<Regex> = OnceLock::new();
        let w = ws();
        let p = re(&PUB, &format!(r",{w}*c?[0-9]+|[()\[\]]"))
            .replace_all(&p, "")
            .into_owned();
        let quote = re(&QUOTE, &format!(r#"^{w}*"|,?"{w}*$"#));
        item.set("publisher", quote.replace_all(&p, "").into_owned());
        let unesc = unescape_html(&p);
        let pubplace: Vec<&str> = unesc.split(" : ").collect();
        if let Some(second) = pubplace.get(1).filter(|s| !s.is_empty()) {
            let possible = pubplace[0].to_owned();
            if !item.truthy("place") {
                item.set("publisher", quote.replace_all(second, "").into_owned());
                item.set("place", possible.clone());
            }
            if item.truthy("place") && item.get_str("place") == Some(possible.as_str()) {
                item.set("publisher", quote.replace_all(second, "").into_owned());
            }
        }
        if let Some(place) = item
            .get_str("place")
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
        {
            let publ = item.get_string("publisher").unwrap_or_default();
            if publ.starts_with(&place) {
                let rest: String = publ.chars().skip(place.chars().count()).collect();
                item.set("publisher", rest);
            }
        }
    }
    let mut date = nonempty(t("//p:addata/p:date")?);
    if date.is_none() {
        date = nonempty(t("//p:addata/p:risdate")?);
    }
    static EIGHT: OnceLock<Regex> = OnceLock::new();
    match date {
        Some(dt) if re(&EIGHT, "[0-9]{8}").is_match(&dt) => {
            let c: Vec<char> = dt.chars().collect();
            let s =
                |a: usize, b: usize| c[a.min(c.len())..b.min(c.len())].iter().collect::<String>();
            item.set("date", format!("{}-{}-{}", s(0, 4), s(4, 6), s(6, 8)));
        }
        _ => {
            let dt = nonempty(t("//p:display/p:creationdate|//p:search/p:creationdate")?);
            static NUM: OnceLock<Regex> = OnceLock::new();
            if let Some(m) = dt.as_deref().and_then(|s| re(&NUM, "[0-9]+").find(s)) {
                item.set("date", m.as_str().to_owned());
            }
        }
    }

    set(
        &mut item,
        "language",
        t("(//p:display/p:language|//p:facets/p:language)[1]")?,
    );

    let pages = nonempty(t("//p:display/p:format")?);
    if item.item_type == "book" {
        if let Some(p) = pages.filter(|p| p.chars().any(|c| c.is_ascii_digit())) {
            item.set("numPages", extract_num_pages(&p));
        }
    }

    set(&mut item, "series", t("(//p:addata/p:seriestitle)[1]")?);
    if let Some(series) = item
        .get_str("series")
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
    {
        static SER: OnceLock<Regex> = OnceLock::new();
        let r = re(
            &SER,
            &format!(r"^([^\n\r\u{{2028}}\u{{2029}}]*);{}*([0-9]+)", ws()),
        );
        if let Some(c) = r.captures(&series) {
            item.set("series", js::trim(&c[1]).to_owned());
            item.set("seriesNumber", c[2].to_owned());
        }
    }

    let isbn = nonempty(t("//p:addata/p:isbn")?);
    let issn = nonempty(t("//p:addata/p:issn")?);
    let js_isbn = |s: &str| clean_isbn(s, false).map_or(Value::Bool(false), Value::from);
    let js_issn = |s: &str| clean_issn(s).map_or(Value::Bool(false), Value::from);
    if let Some(i) = &isbn {
        item.set("ISBN", js_isbn(i));
    }
    if let Some(i) = &issn {
        item.set("ISSN", js_issn(i));
    }
    let locators = nonempty(t("//p:display/p:identifier")?);
    if !(item.truthy("ISBN") || item.truthy("ISSN")) {
        if let Some(l) = locators {
            item.set("ISBN", js_isbn(&l));
            item.set("ISSN", js_issn(&l));
        }
    }

    set(&mut item, "edition", t("//p:display/p:edition")?);

    let mut subjects = x("//p:display/p:subject")?;
    if subjects.is_empty() {
        subjects = x("//p:search/p:subject")?;
    }
    static TAGSPLIT: OnceLock<Regex> = OnceLock::new();
    let tag_split = re(&TAGSPLIT, r" (?:/|--|;) ");
    for s in subjects {
        let chain = trim_internal(&d.text(s));
        for tag in tag_split.split(&chain) {
            item.tags.push(TranslatorTag::new(tag));
        }
    }

    let mut abs = nonempty(t("//p:display/p:description")?);
    if abs.is_none() {
        abs = t("//p:addata/p:abstract")?;
    }
    set(&mut item, "abstractNote", abs);
    if let Some(a) = item.get_str("abstractNote").filter(|s| !s.is_empty()) {
        let a = unescape_html(a);
        item.set("abstractNote", a);
    }

    set(&mut item, "DOI", t("//p:addata/p:doi")?);
    set(&mut item, "issue", t("//p:addata/p:issue")?);
    set(&mut item, "volume", t("//p:addata/p:volume")?);
    set(&mut item, "publicationTitle", t("//p:addata/p:jtitle")?);
    if item.item_type != "book" {
        set(&mut item, "bookTitle", t("//p:addata/p:btitle")?);
    }

    let start = nonempty(t("//p:addata/p:spage")?);
    let end = nonempty(t("//p:addata/p:epage")?);
    let overall = nonempty(t("//p:addata/p:pages")?);
    const RANGE_TYPES: [&str; 6] = [
        "journalArticle",
        "magazineArticle",
        "newspaperArticle",
        "dictionaryEntry",
        "encyclopediaArticle",
        "conferencePaper",
    ];
    match (start, end, overall) {
        (Some(s), Some(e), _) => item.set("pages", format!("{s}–{e}")),
        (_, _, Some(o)) => {
            if RANGE_TYPES.contains(&item.item_type.as_str()) {
                item.set("pages", o);
            } else {
                item.set("numPages", o);
            }
        }
        (Some(s), None, None) => item.set("pages", s),
        (None, Some(e), None) => item.set("pages", e),
        _ => {}
    }

    static ULINK: OnceLock<Regex> = OnceLock::new();
    let ulink = re(&ULINK, r"\$\$U((?:[^\n\r\u{2028}\u{2029}])+?)\$\$");
    if let Some(u) = nonempty(t("//p:links/p:linktorsrc")?) {
        if let Some(c) = ulink.captures(&u) {
            item.set("url", c[1].to_owned());
        }
    }
    if let Some(fa) = nonempty(t("//p:links/p:linktofa")?) {
        if let Some(c) = ulink.captures(&fa) {
            let mut a = JsObject::new();
            a.set("url", c[1].to_owned());
            a.set("title", "Finding Aid");
            a.set("snapshot", false);
            item.attachments.push(a);
        }
    }

    let mut calls: Vec<String> = Vec::new();
    static DCALL: OnceLock<Regex> = OnceLock::new();
    let dcall = re(&DCALL, r"\$\$D([^\n\r\u{2028}\u{2029}]+?)\$");
    for c in x("//p:browse/p:callnumber")? {
        if let Some(m) = dcall.captures(&d.text(c)) {
            calls.push(m[1].to_owned());
        }
    }
    if calls.is_empty() {
        static SUB2: OnceLock<Regex> = OnceLock::new();
        let sub2 = re(
            &SUB2,
            &format!(r"\$\$2\(?([^\n\r\u{{2028}}\u{{2029}}]+?)(?:{}*\))?\$", ws()),
        );
        for c in x("//p:display/p:availlibrary|//p:delivery/p:bestlocation/p:callNumber")? {
            let tc = d.text(c);
            match sub2.captures(&tc) {
                Some(m) => calls.push(m[1].to_owned()),
                None => calls.push(tc),
            }
        }
    }
    if !calls.is_empty() {
        let mut dedup: Vec<String> = Vec::new();
        for c in calls {
            if !dedup.contains(&c) {
                dedup.push(c);
            }
        }
        item.set("callNumber", dedup.join(", "));
    } else {
        // :336 evaluated and discarded.
        t("//p:enrichment/p:classificationlcc")?;
    }

    let mut source = nonempty(t("//p:delivery/p:bestlocation/p:organization")?);
    if source.is_none() {
        source = nonempty(t("//p:control/p:sourceid")?);
    }
    static LIB: OnceLock<Regex> = OnceLock::new();
    let library = source.and_then(|s| {
        re(&LIB, r"^(?:\$\$V)?(?:[0-9]+)?([^\n\r\u{2028}\u{2029}]+?)_")
            .captures(&s)
            .map(|c| c[1].to_owned())
    });
    if library.as_deref() == Some("HVD") {
        if let Some(l) = nonempty(t("//p:display/p:lds01")?) {
            item.set("extra", format!("HOLLIS number: {l}"));
        }
        static HREF: OnceLock<Regex> = OnceLock::new();
        let href = re(&HREF, r#"href="([^\n\r\u{2028}\u{2029}]+?)""#);
        for l in x("//p:display/p:lds03")? {
            if let Some(c) = href.captures(&d.text(l)) {
                let mut a = JsObject::new();
                a.set("url", c[1].to_owned());
                a.set("title", "HOLLIS Permalink");
                a.set("mimeType", "text/html");
                a.set("snapshot", false);
                item.attachments.push(a);
            }
        }
    }
    ctx.item_done(item);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_author_and_num_pages() {
        assert_eq!(
            strip_author("Wheaton, Barbara Ketcham [former owner]$$QWheaton, Barbara Ketcham"),
            "Wheaton, Barbara Ketcham"
        );
        assert_eq!(
            strip_author("Smith, John, 1950-2010 (illustrator)"),
            "Smith, John"
        );
        assert_eq!(extract_num_pages("x-109 p., 510 p."), "x+109");
        assert_eq!(extract_num_pages("123 S."), "123");
    }
}
