// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): Wikipedia Citation Templates.js (translatorID
//   3f50aaac-7acc-4350-acd0-59cb77faf620, lastUpdated 2023-11-05 21:29:00):
//   header :1-15, `fieldMap` :40-51, `typeMap` :53-87, `formatAuthors`
//   :89-100, `formatFirstAuthor` :102-109, `formatDate` :111-120,
//   `doExport` :122-428, `escapeWiki` :430-432. The en-US creator-type
//   labels (`ZU.getLocalizedCreatorType`) are the translation-server's:
//   Zotero utilities (commit 4051881d59c6) cachedTypes.js
//   `getLocalizedString` :85-88 over resource/zoteroTypeSchemaData.js
//   `creatorTypes` :1477-.
// Copyright (c) 2017-2019 Simon Kornblith.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The Wikipedia Citation Templates translator: export (`{{Cite ...}}`
//! templates, one per item, separated by CRLF).
//!
//! The item is the legacy export format (minVersion 1.0.0b4.r1 < 4.0.27):
//! dates are SQL, single-field creators have a `lastName` and no
//! `firstName`. JavaScript values are kept as JavaScript would concatenate
//! them (an undefined name is "undefined"; undefined + undefined is "NaN").
//! Where upstream throws (a string method on a value that is not a string;
//! `formatFirstAuthor` of an empty list for a "Cite email") the port
//! returns [`TranslateError::Translator`], as the endpoint answers 500.
//!
//! **Maturity: AI draft (1).** Verified code-to-code against the Zotero
//! translation-server (`tests/zotero_translators.rs`, `wikipedia_*`).

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::utilities::lpad;
use crate::zotero::framework::{js, ExportContext, TranslateError, TranslatorCreator};
use kovan_common::zotero::date::str_to_date;
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "3f50aaac-7acc-4350-acd0-59cb77faf620",
    label: "Wikipedia Citation Templates",
    creator: "Simon Kornblith",
    target: "txt",
    min_version: "1.0.0b4.r1",
    priority: 100,
    translator_type: translator_type::EXPORT,
    config_options: &[],
    display_options: &[("exportCharset", HeaderValue::Str("UTF-8"))],
    hidden_prefs: &[],
    last_updated: "2023-11-05 21:29:00",
};

/// `fieldMap` (:40-51): wiki parameter, Zotero field.
const FIELD_MAP: [(&str, &str); 10] = [
    ("edition", "edition"),
    ("publisher", "publisher"),
    ("doi", "DOI"),
    ("isbn", "ISBN"),
    ("issn", "ISSN"),
    ("conference", "conferenceName"),
    ("volume", "volume"),
    ("issue", "issue"),
    ("pages", "pages"),
    ("number", "episodeNumber"),
];

/// `typeMap` (:53-87).
fn type_map(item_type: &str) -> Option<&'static str> {
    Some(match item_type {
        "book" | "bookSection" | "manuscript" => "Cite book",
        "journalArticle" => "Cite journal",
        "magazineArticle" | "newspaperArticle" => "Cite news",
        "thesis" | "presentation" => "Cite paper",
        "letter" | "artwork" | "bill" | "hearing" | "patent" | "statute" | "map"
        | "instantMessage" | "audioRecording" | "computerProgram" | "document" => "Cite",
        "interview" => "Cite interview",
        "film" | "videoRecording" => "Cite AV media",
        "webpage" | "blogPost" | "forumPost" => "Cite web",
        "report" | "conferencePaper" => "Cite conference",
        "email" => "Cite email",
        "tvBroadcast" | "radioBroadcast" => "Cite episode",
        "podcast" => "Cite podcast",
        "encyclopediaArticle" | "dictionaryEntry" => "Cite encyclopedia",
        _ => return None,
    })
}

/// The translation-server's en-US creator-type labels
/// (zoteroTypeSchemaData.js `creatorTypes`).
const CREATOR_TYPE_LABELS: [(&str, &str); 37] = [
    ("author", "Author"),
    ("contributor", "Contributor"),
    ("editor", "Editor"),
    ("translator", "Translator"),
    ("seriesEditor", "Series Editor"),
    ("interviewee", "Interview With"),
    ("interviewer", "Interviewer"),
    ("director", "Director"),
    ("scriptwriter", "Scriptwriter"),
    ("producer", "Producer"),
    ("castMember", "Cast Member"),
    ("sponsor", "Sponsor"),
    ("counsel", "Counsel"),
    ("inventor", "Inventor"),
    ("attorneyAgent", "Attorney/Agent"),
    ("recipient", "Recipient"),
    ("performer", "Performer"),
    ("composer", "Composer"),
    ("wordsBy", "Words By"),
    ("cartographer", "Cartographer"),
    ("programmer", "Programmer"),
    ("artist", "Artist"),
    ("commenter", "Commenter"),
    ("presenter", "Presenter"),
    ("guest", "Guest"),
    ("podcaster", "Podcaster"),
    ("reviewedAuthor", "Reviewed Author"),
    ("cosponsor", "Cosponsor"),
    ("bookAuthor", "Book Author"),
    ("originalCreator", "Original Creator"),
    ("host", "Host"),
    ("narrator", "Narrator"),
    ("executiveProducer", "Executive Producer"),
    ("seriesCreator", "Series Creator"),
    ("chair", "Chair"),
    ("organizer", "Organizer"),
    ("creator", "Creator"),
];

/// `Zotero.Utilities.getLocalizedCreatorType(type)` as JavaScript would
/// concatenate it: the label, or "false" for an unknown (or missing) type.
pub fn localized_creator_type(t: Option<&str>) -> &'static str {
    t.and_then(|t| CREATOR_TYPE_LABELS.iter().find(|(n, _)| *n == t))
        .map_or("false", |(_, l)| *l)
}

fn type_error(what: &str) -> TranslateError {
    TranslateError::Translator(format!("TypeError: {what}"))
}

fn truthy_str(s: &Option<String>) -> bool {
    s.as_deref().is_some_and(|s| !s.is_empty())
}

/// `formatAuthors(authors, useTypes)` (:89-100).
fn format_authors(authors: &[TranslatorCreator], use_types: bool) -> String {
    let mut text = String::new();
    for a in authors {
        text.push_str(", ");
        if truthy_str(&a.first_name) {
            text.push_str(a.first_name.as_deref().unwrap());
        }
        if truthy_str(&a.first_name) && truthy_str(&a.last_name) {
            text.push(' ');
        }
        if truthy_str(&a.last_name) {
            text.push_str(a.last_name.as_deref().unwrap());
        }
        if use_types {
            text.push_str(&format!(
                " ({})",
                localized_creator_type(a.creator_type.as_deref())
            ));
        }
    }
    text.chars().skip(2).collect()
}

/// `formatFirstAuthor(authors, useTypes)` (:102-109): shifts the first
/// creator off `authors`; throws when there is none.
fn format_first_author(
    authors: &mut Vec<TranslatorCreator>,
    use_types: bool,
) -> Result<String, TranslateError> {
    if authors.is_empty() {
        return Err(type_error("firstCreator is undefined"));
    }
    let c = authors.remove(0);
    let mut field = match (&c.last_name, &c.first_name) {
        (Some(l), first) => {
            let mut f = l.clone();
            if truthy_str(&c.last_name) && truthy_str(&c.first_name) {
                f.push_str(", ");
            }
            f.push_str(first.as_deref().unwrap_or("undefined"));
            f
        }
        // `undefined + firstName`.
        (None, Some(f)) => format!("undefined{f}"),
        // `undefined + undefined` is NaN.
        (None, None) => "NaN".to_owned(),
    };
    if use_types {
        field.push_str(&format!(
            " ({})",
            localized_creator_type(c.creator_type.as_deref())
        ));
    }
    Ok(field)
}

/// `formatDate(date)` (:111-120).
fn format_date(date: &str) -> String {
    let chars: Vec<char> = date.chars().collect();
    // `date.substr(0, date.indexOf(" "))`: -1 gives a negative length, "".
    let date: String = match chars.iter().position(|&c| c == ' ') {
        Some(i) => chars[..i].iter().collect(),
        None => String::new(),
    };
    if js::substr(&date, 4, 3) == "-00" {
        js::substr(&date, 0, 4)
    } else if js::substr(&date, 7, 3) == "-00" {
        js::substr(&date, 0, 7)
    } else {
        date
    }
}

/// A value the translator calls a string method on.
fn as_str<'a>(v: &'a Value, what: &str) -> Result<&'a str, TranslateError> {
    v.as_str()
        .ok_or_else(|| type_error(&format!("{what}.replace is not a function")))
}

/// A property of the `properties` object.
#[derive(Debug, Clone)]
enum Prop {
    /// A JavaScript value (`None` is undefined).
    Val(Option<Value>),
    /// `properties.authors`: `{last, first}` of each author.
    Authors(Vec<(Option<String>, Option<String>)>),
}

/// The `properties` object: keys in insertion order (none is an integer).
#[derive(Debug, Default)]
struct Props(Vec<(String, Prop)>);

impl Props {
    fn set(&mut self, k: &str, v: Prop) {
        match self.0.iter_mut().find(|(key, _)| key == k) {
            Some((_, x)) => *x = v,
            None => self.0.push((k.to_owned(), v)),
        }
    }
    fn val(&mut self, k: &str, v: Option<Value>) {
        self.set(k, Prop::Val(v));
    }
    fn s(&mut self, k: &str, v: String) {
        self.set(k, Prop::Val(Some(Value::String(v))));
    }
    fn get(&self, k: &str) -> Option<&Prop> {
        self.0.iter().find(|(key, _)| key == k).map(|(_, v)| v)
    }
    fn get_val(&self, k: &str) -> Option<&Value> {
        match self.get(k) {
            Some(Prop::Val(v)) => v.as_ref(),
            _ => None,
        }
    }
    fn truthy(&self, k: &str) -> bool {
        match self.get(k) {
            Some(Prop::Val(v)) => js::truthy(v.as_ref()),
            Some(Prop::Authors(_)) => true,
            None => false,
        }
    }
}

/// `str.match(re)` for a regex with the `m` flag whose pattern starts with
/// `^`: JavaScript's `^` then matches after any line terminator (LF, CR,
/// U+2028, U+2029); `anchored` is the pattern without `^`, anchored with
/// `\A`. Returns the first capture group.
fn match_multiline(s: &str, anchored: &Regex) -> Option<String> {
    let mut starts = vec![0];
    for (i, c) in s.char_indices() {
        if matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}') {
            starts.push(i + c.len_utf8());
        }
    }
    starts
        .into_iter()
        .find_map(|p| anchored.captures(&s[p..]).map(|c| c[1].to_owned()))
}

/// `doExport` (:122-428).
pub fn do_export(ctx: &mut ExportContext) -> Result<(), TranslateError> {
    static PMID: OnceLock<Regex> = OnceLock::new();
    static PMC: OnceLock<Regex> = OnceLock::new();
    static URL_PMID: OnceLock<Regex> = OnceLock::new();
    static URL_PMC: OnceLock<Regex> = OnceLock::new();
    static URL_JSTOR: OnceLock<Regex> = OnceLock::new();
    static NON_DIGITS: OnceLock<Regex> = OnceLock::new();
    let ws = js::WS;
    let pmid_re = PMID.get_or_init(|| Regex::new(&format!(r"\APMID{ws}*:{ws}*([0-9]+)")).unwrap());
    let pmc_re =
        PMC.get_or_init(|| Regex::new(&format!(r"\APMCID{ws}*:{ws}*((?:PMC)?[0-9]+)")).unwrap());
    let url_res = [
        (
            "pmid",
            URL_PMID.get_or_init(|| {
                Regex::new(r"(?i-u:www\.ncbi\.nlm\.nih\.gov/pubmed/)([0-9]+)").unwrap()
            }),
        ),
        (
            "pmc",
            URL_PMC.get_or_init(|| {
                Regex::new(r"(?i-u:www\.ncbi\.nlm\.nih\.gov/pmc/articles/)((?i-u:PMC)?[0-9]+)")
                    .unwrap()
            }),
        ),
        (
            "jstor",
            URL_JSTOR
                .get_or_init(|| Regex::new(r"(?i-u:www\.jstor\.org/stable/)([^?#]+)").unwrap()),
        ),
    ];
    let non_digits = NON_DIGITS.get_or_init(|| Regex::new("[^0-9]+").unwrap());

    let mut first = true;
    while let Some(item) = ctx.next_item() {
        let it = item.item_type.as_str();
        let ty = type_map(it).unwrap_or("Cite");
        let get = |k: &str| item.get(k).cloned();
        let mut p = Props::default();
        for (wiki, zotero) in FIELD_MAP {
            if item.truthy(zotero) {
                p.val(wiki, get(zotero));
            }
        }

        let mut creators = item.creators.clone();
        if !creators.is_empty() {
            if ty == "Cite episode" {
                p.s("credits", format_authors(&creators, true));
            } else if ty == "Cite AV media" {
                p.s("people", String::new());
                let mut people = format_first_author(&mut creators, true)?;
                if !creators.is_empty() {
                    people.push_str(", ");
                    people.push_str(&format_authors(&creators, true));
                }
                p.s("people", people);
                if item.truthy("type") {
                    p.val("medium", get("type"));
                }
            } else if ty == "Cite email" {
                creators.retain(|c| c.creator_type.as_deref() == Some("author"));
                let mut author = format_first_author(&mut creators, false)?;
                if !creators.is_empty() {
                    author.push_str(", ");
                    author.push_str(&format_authors(&creators, false));
                }
                p.s("author", author);
            } else if ty == "Cite interview" {
                let mut translators = Vec::new();
                let mut interviewers = Vec::new();
                let mut rest = Vec::new();
                for c in creators.drain(..) {
                    match c.creator_type.as_deref() {
                        Some("translator") => translators.push(c),
                        Some("interviewer") => interviewers.push(c),
                        Some("contributor") => {}
                        _ => rest.push(c),
                    }
                }
                creators = rest;
                if !interviewers.is_empty() {
                    let first_iv = interviewers.remove(0);
                    p.s("interviewer", format_authors(&[first_iv], false));
                    if !interviewers.is_empty() {
                        p.s("cointerviewers", format_authors(&interviewers, false));
                    }
                }
                if !translators.is_empty() {
                    let mut co = match p.get_val("cointerviewers") {
                        Some(v) if js::truthy(Some(v)) => format!("{}, ", js::to_js_string(v)),
                        _ => String::new(),
                    };
                    co.push_str(&format_authors(&translators, false));
                    p.s("cointerviewers", co);
                }
                if !creators.is_empty() {
                    // `while ((interviewee = item.creators.shift()) && i <= 4)`
                    for (n, iv) in creators.iter().take(4).enumerate() {
                        let i = n + 1;
                        let (lk, fk) = if i == 1 {
                            ("last".to_owned(), "first".to_owned())
                        } else {
                            (format!("last{i}"), format!("first{i}"))
                        };
                        p.val(&lk, iv.last_name.clone().map(Value::String));
                        p.val(&fk, iv.first_name.clone().map(Value::String));
                    }
                }
                if item.truthy("medium") {
                    p.val("type", get("medium"));
                }
            } else {
                let mut editors = Vec::new();
                let mut translators = Vec::new();
                let mut rest = Vec::new();
                for c in creators.drain(..) {
                    match c.creator_type.as_deref() {
                        Some("translator") => translators.push(c),
                        Some("editor") => editors.push(c),
                        Some("contributor") => {}
                        _ => rest.push(c),
                    }
                }
                creators = rest;
                let mut others = String::new();
                if !editors.is_empty() {
                    let editor_text = format_authors(&editors, false)
                        + if editors.len() == 1 {
                            " (ed.)"
                        } else {
                            " (eds.)"
                        };
                    if it == "bookSection" || ty == "Cite conference" || ty == "Cite encyclopedia" {
                        p.s("editors", editor_text);
                    } else {
                        others = editor_text;
                    }
                }
                if !translators.is_empty() {
                    if !others.is_empty() {
                        others.push_str(", ");
                    }
                    others.push_str(&format_authors(&translators, false));
                    others.push_str(" (trans.)");
                }
                if !creators.is_empty() {
                    p.set(
                        "authors",
                        Prop::Authors(
                            creators
                                .iter()
                                .map(|c| (c.last_name.clone(), c.first_name.clone()))
                                .collect(),
                        ),
                    );
                }
                if !others.is_empty() {
                    p.s("others", others);
                }
            }
        }

        if it == "bookSection" {
            p.val("title", get("publicationTitle"));
            p.val("chapter", get("title"));
        } else {
            p.val("title", get("title"));
            let key = match ty {
                "Cite journal" => "journal",
                "Cite conference" => "booktitle",
                "Cite encyclopedia" => "encyclopedia",
                _ => "work",
            };
            p.val(key, get("publicationTitle"));
        }

        if ty == "Cite web" && item.truthy("type") {
            p.val("format", get("type"));
        }
        if item.truthy("place") {
            p.val(
                if ty == "Cite episode" {
                    "city"
                } else {
                    "location"
                },
                get("place"),
            );
        }
        if item.truthy("series") {
            p.val("series", get("series"));
        } else if item.truthy("seriesTitle") {
            p.val("series", get("seriesTitle"));
        } else if item.truthy("seriesText") {
            p.val("series", get("seriesText"));
        }

        if item.truthy("accessDate") && !(it == "journalArticle" && !item.truthy("url")) {
            let a = as_str(item.get("accessDate").unwrap(), "date")?;
            p.s("access-date", format_date(a));
        }

        if item.truthy("date") {
            if ty == "Cite email" {
                let d = as_str(item.get("date").unwrap(), "date")?;
                p.s("senddate", format_date(d));
            } else {
                let d = str_to_date(
                    &js::to_js_string(item.get("date").unwrap()),
                    &ctx.options.env.dates,
                );
                if let Some(year) = d.year {
                    let mm = d
                        .month
                        .map_or("00".to_owned(), |m| lpad(&(m + 1).to_string(), "0", 2));
                    let dd = d
                        .day
                        .map_or("00".to_owned(), |day| lpad(&day.to_string(), "0", 2));
                    let yyyy = lpad(&year, "0", 4);
                    p.s("date", format_date(&format!("{yyyy}-{mm}-{dd} ")));
                }
            }
        }

        if item.truthy("runningTime") {
            p.val(
                if ty == "Cite episode" {
                    "minutes"
                } else {
                    "time"
                },
                get("runningTime"),
            );
        }

        if item.truthy("url") && item.truthy("accessDate") {
            p.val(
                if it == "bookSection" {
                    "chapterurl"
                } else {
                    "url"
                },
                get("url"),
            );
        }

        if p.truthy("pages") {
            let pages = as_str(p.get_val("pages").unwrap(), "properties.pages")?;
            let replaced = non_digits.replace(pages, "–").into_owned();
            p.s("pages", replaced);
        }

        if item.truthy("extra") {
            let extra = item
                .get("extra")
                .unwrap()
                .as_str()
                .ok_or_else(|| type_error("item.extra.match is not a function"))?;
            for (f, re) in [("pmid", pmid_re), ("pmc", pmc_re)] {
                if let Some(m) = match_multiline(extra, re) {
                    p.s(f, m);
                }
            }
        }

        if item.truthy("url") {
            let url = item
                .get("url")
                .unwrap()
                .as_str()
                .ok_or_else(|| type_error("item.url.match is not a function"))?;
            for (f, re) in &url_res {
                if p.truthy(f) {
                    continue;
                }
                if let Some(c) = re.captures(url) {
                    p.s(f, c[1].to_owned());
                }
            }
        }

        let mut out = format!("{}{{{{{ty}", if first { "" } else { "\r\n" });
        for (key, v) in &p.0 {
            match v {
                Prop::Authors(authors) => {
                    let index = authors.len() > 1;
                    for (i, (last, first_name)) in authors.iter().enumerate() {
                        let n = if i > 0 || index {
                            (i + 1).to_string()
                        } else {
                            String::new()
                        };
                        out.push_str(&format!(
                            "| last{n} = {}",
                            last.as_deref().unwrap_or("undefined")
                        ));
                        if truthy_str(first_name) {
                            out.push_str(&format!(
                                "| first{n} = {}",
                                first_name.as_deref().unwrap()
                            ));
                        }
                    }
                }
                Prop::Val(v) => {
                    if !js::truthy(v.as_ref()) {
                        continue;
                    }
                    // `escapeWiki(markup)`: `markup.replace('|', '{{!}}')`.
                    let s = as_str(v.as_ref().unwrap(), "markup")?;
                    out.push_str(&format!("| {key} = {}", s.replacen('|', "{{!}}", 1)));
                }
            }
        }
        out.push_str("}}");
        ctx.write(&out);
        first = false;
    }
    Ok(())
}
