// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): BibLaTeX.js (translatorID
//   b6e39b57-8942-4d11-8259-342c46ce395f, lastUpdated 2026-04-01 18:00:00):
//   header :1-21, `fieldMap` :52-72, `revExtraIds` :80-87, `revEprintIds`
//   :90-99, `zotero2biblatexTypeMap` :135-170, `alwaysMap` :173-182,
//   `babelLanguageMap` :187-281, `writeField` :286-319, `mapHTMLmarkup`
//   :321-331, `creatorCheck` :401-411, `encodeFilePathComponent` :478-484,
//   `doExport` :492-883. `parseExtraFields`, `extraFieldsToString`,
//   `tidyAccents`, `buildCiteKey` and `cleanFilePath` (:101-129, :340-476,
//   :486-490) are the same code as BibTeX.js's and shared with it
//   (`bibtex::common`).
// Copyright (C) 2019 Simon Kornblith, Richard Karnesky and Anders Johansson.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The BibLaTeX translator: export.

use super::bibtex::common::{
    build_cite_key, clean_file_path, creator_string, extra_recs_to_string, parse_extra_fields,
    protect_and,
};
use crate::zotero::framework::html::unescape_html;
use crate::zotero::framework::js;
use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::utilities::str_to_iso;
use crate::zotero::framework::{ExportContext, TranslateError, TranslatorItem};
use regex::Regex;
use std::sync::OnceLock;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "b6e39b57-8942-4d11-8259-342c46ce395f",
    label: "BibLaTeX",
    creator: "Simon Kornblith, Richard Karnesky and Anders Johansson",
    target: "bib",
    min_version: "2.1.9",
    priority: 100,
    translator_type: translator_type::EXPORT,
    config_options: &[("getCollections", HeaderValue::Bool(true))],
    display_options: &[
        ("exportCharset", HeaderValue::Str("UTF-8")),
        ("exportNotes", HeaderValue::Bool(false)),
        ("exportFileData", HeaderValue::Bool(false)),
        ("useJournalAbbreviation", HeaderValue::Bool(false)),
    ],
    hidden_prefs: &[],
    last_updated: "2026-04-01 18:00:00",
};

fn re(cell: &'static OnceLock<Regex>, pattern: impl FnOnce() -> String) -> &'static Regex {
    cell.get_or_init(|| Regex::new(&pattern()).expect("static regex compiles"))
}

/// `fieldMap` (:52-72), in property order.
const FIELD_MAP: [(&str, &str); 19] = [
    ("location", "place"),
    ("chapter", "chapter"),
    ("edition", "edition"),
    ("title", "title"),
    ("volume", "volume"),
    ("rights", "rights"),
    ("isbn", "ISBN"),
    ("issn", "ISSN"),
    ("url", "url"),
    ("doi", "DOI"),
    ("series", "series"),
    ("shorttitle", "shortTitle"),
    ("holder", "assignee"),
    ("abstract", "abstractNote"),
    ("volumes", "numberOfVolumes"),
    ("version", "version"),
    ("eventtitle", "conferenceName"),
    ("pages", "pages"),
    ("pagetotal", "numPages"),
];

/// `revExtraIds` (:80-87).
const REV_EXTRA_IDS: [(&str, &str); 6] = [
    ("LCCN", "lccn"),
    ("MR", "mrnumber"),
    ("Zbl", "zmnumber"),
    ("PMCID", "pmcid"),
    ("PMID", "pmid"),
    ("DOI", "doi"),
];

/// `revEprintIds` (:90-99).
const REV_EPRINT_IDS: [(&str, &str); 4] = [
    ("arXiv", "arxiv"),
    ("JSTOR", "jstor"),
    ("HDL", "hdl"),
    ("GoogleBooksID", "googlebooks"),
];

/// `zotero2biblatexTypeMap` (:135-170).
const TYPE_MAP: [(&str, &str); 34] = [
    ("book", "book"),
    ("bookSection", "incollection"),
    ("journalArticle", "article"),
    ("magazineArticle", "article"),
    ("newspaperArticle", "article"),
    ("thesis", "thesis"),
    ("letter", "letter"),
    ("manuscript", "unpublished"),
    ("interview", "misc"),
    ("film", "movie"),
    ("artwork", "artwork"),
    ("webpage", "online"),
    ("conferencePaper", "inproceedings"),
    ("report", "report"),
    ("bill", "legislation"),
    ("case", "jurisdiction"),
    ("hearing", "jurisdiction"),
    ("patent", "patent"),
    ("statute", "legislation"),
    ("email", "letter"),
    ("map", "misc"),
    ("blogPost", "online"),
    ("instantMessage", "misc"),
    ("forumPost", "online"),
    ("audioRecording", "audio"),
    ("presentation", "unpublished"),
    ("videoRecording", "video"),
    ("tvBroadcast", "misc"),
    ("radioBroadcast", "misc"),
    ("podcast", "audio"),
    ("computerProgram", "software"),
    ("document", "misc"),
    ("encyclopediaArticle", "inreference"),
    ("dictionaryEntry", "inreference"),
];

/// A `babelLanguageMap` entry (:187-281): one language, or variants by
/// subtag ("" is the default).
enum Babel {
    One(&'static str),
    Variants(&'static [(&'static str, &'static str)]),
}

/// `babelLanguageMap[code]`.
fn babel(code: &str) -> Option<Babel> {
    use Babel::{One, Variants};
    Some(match code {
        "af" => One("afrikaans"),
        "ar" => One("arabic"),
        "eu" => One("basque"),
        "br" => One("breton"),
        "bg" => One("bulgarian"),
        "ca" => One("catalan"),
        "hr" => One("croatian"),
        "cz" => One("czech"),
        "da" => One("danish"),
        "nl" => One("dutch"),
        "en" => Variants(&[
            ("", "english"),
            ("US", "american"),
            ("GB", "british"),
            ("CA", "canadian"),
            ("AU", "australian"),
            ("NZ", "newzealand"),
        ]),
        "eo" => One("esperanto"),
        "et" => One("estonian"),
        "fa" => One("farsi"),
        "fi" => One("finnish"),
        "fr" => Variants(&[("", "french"), ("CA", "canadien")]),
        "fur" => One("friulan"),
        "gl" => One("galician"),
        "de" => Variants(&[
            ("", "german"),
            ("AT", "austrian"),
            ("DE-1996", "ngerman"),
            ("AT-1996", "naustrian"),
            ("1996", "ngerman"),
        ]),
        "el" => Variants(&[("", "greek"), ("polyton", "polutonikogreek")]),
        "he" => One("hebrew"),
        "hi" => One("hindi"),
        "is" => One("icelandic"),
        "id" => One("indonesian"),
        "ia" => One("interlingua"),
        "ga" => One("irish"),
        "it" => One("italian"),
        "ja" => One("japanese"),
        "la" => One("latin"),
        "lv" => One("latvian"),
        "lt" => One("lithuanian"),
        "dsb" => One("lowersorbian"),
        "hu" => One("magyar"),
        "zlm" => One("malay"),
        "mn" => One("mongolian"),
        "se" => One("samin"),
        "nn" => One("nynorsk"),
        "nb" => One("norsk"),
        "no" => One("norwegian"),
        "zh" => Variants(&[("", "pinyin"), ("Latn", "pinyin")]),
        "pl" => One("polish"),
        "pt" => Variants(&[("", "portuguese"), ("PT", "portuguese"), ("BR", "brazil")]),
        "ro" => One("romanian"),
        "rm" => One("romansh"),
        "ru" => One("russian"),
        "gd" => One("scottish"),
        "sr" => Variants(&[("", "serbian"), ("Cyrl", "serbianc"), ("Latn", "serbian")]),
        "sk" => One("slovak"),
        "sl" => One("slovene"),
        "es" => One("spanish"),
        "sv" => One("swedish"),
        "th" => One("thaicjk"),
        "tr" => One("turkish"),
        "tk" => One("turkmen"),
        "uk" => One("ukrainian"),
        "hsb" => One("uppersorbian"),
        "vi" => One("vietnamese"),
        "cy" => One("welsh"),
        _ => return None,
    })
}

/// `mapEscape` over `alwaysMap` (:173-182, :333-335).
fn map_escape(c: char) -> Option<&'static str> {
    Some(match c {
        '|' => "{\\textbar}",
        '<' => "{\\textless}",
        '>' => "{\\textgreater}",
        '~' => "{\\textasciitilde}",
        '^' => "{\\textasciicircum}",
        '\\' => "{\\textbackslash}",
        '{' => "\\{",
        '}' => "\\}",
        _ => return None,
    })
}

fn escape_chars(s: &str, set: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match map_escape(c).filter(|_| set.contains(c)) {
            Some(m) => out.push_str(m),
            None => out.push(c),
        }
    }
    out
}

/// `.replace(/[#$%&_]/g, "\\$&")`.
fn escape_tex_specials(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if "#$%&_".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn is_line_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// One `mapHTMLmarkup` rule (:325-329):
/// `open (((?!close).)+) close` with the tempered token hand-written (the
/// `regex` crate has no look-ahead). From each position where `open`
/// matches, the inner text runs up to the first `close` (it may not contain
/// one) and may not cross a line terminator or be empty; replacements are
/// global, left to right.
fn tempered_replace(
    s: &str,
    open: impl Fn(&[char], usize) -> Option<usize>,
    close: &str,
    before: &str,
    after: &str,
) -> String {
    let c: Vec<char> = s.chars().collect();
    let close: Vec<char> = close.chars().collect();
    let close_at = |p: usize| c.len() >= p + close.len() && c[p..p + close.len()] == *close;
    let mut out = String::new();
    let mut p = 0;
    let mut copied = 0;
    while p < c.len() {
        if let Some(g0) = open(&c, p) {
            let mut e = g0;
            while e < c.len() && !is_line_terminator(c[e]) && !close_at(e) {
                e += 1;
            }
            if e > g0 && close_at(e) {
                out.extend(c[copied..p].iter());
                out.push_str(before);
                out.extend(c[g0..e].iter());
                out.push_str(after);
                p = e + close.len();
                copied = p;
                continue;
            }
        }
        p += 1;
    }
    out.extend(c[copied..].iter());
    out
}

fn literal_open(tok: &'static str) -> impl Fn(&[char], usize) -> Option<usize> {
    move |c: &[char], p: usize| {
        let t: Vec<char> = tok.chars().collect();
        (c.len() >= p + t.len() && c[p..p + t.len()] == *t).then_some(p + t.len())
    }
}

/// `mapHTMLmarkup(characters)` (:321-331).
fn map_html_markup(s: &str) -> String {
    let s = tempered_replace(
        s,
        literal_open("{\\textless}i{\\textgreater}"),
        "{\\textless}/i{\\textgreater}",
        "\\textit{",
        "}",
    );
    let s = tempered_replace(
        &s,
        literal_open("{\\textless}b{\\textgreater}"),
        "{\\textless}/b{\\textgreater}",
        "\\textbf{",
        "}",
    );
    let s = tempered_replace(
        &s,
        literal_open("{\\textless}sup{\\textgreater}"),
        "{\\textless}/sup{\\textgreater}",
        "$^{\\textrm{",
        "}}$",
    );
    let s = tempered_replace(
        &s,
        literal_open("{\\textless}sub{\\textgreater}"),
        "{\\textless}/sub{\\textgreater}",
        "$_{\\textrm{",
        "}}$",
    );
    // `span\sstyle="small-caps"`: one whitespace character.
    let span_open = |c: &[char], p: usize| {
        let a: Vec<char> = "{\\textless}span".chars().collect();
        let b: Vec<char> = "style=\"small-caps\"{\\textgreater}".chars().collect();
        let n = a.len();
        if c.len() >= p + n + 1 + b.len()
            && c[p..p + n] == *a
            && js::is_space(c[p + n])
            && c[p + n + 1..p + n + 1 + b.len()] == *b
        {
            Some(p + n + 1 + b.len())
        } else {
            None
        }
    };
    let s = tempered_replace(
        &s,
        span_open,
        "{\\textless}/span{\\textgreater}",
        "\\textsc{",
        "}",
    );
    tempered_replace(
        &s,
        literal_open("{\\textless}sc{\\textgreater}"),
        "{\\textless}/sc{\\textgreater}",
        "\\textsc{",
        "}",
    )
}

/// `writeField(field, value, isMacro, noEscape)` (:286-319); an empty
/// `value` writes nothing (`!value`; numbers arrive as their strings).
fn write_field(ctx: &mut ExportContext, field: &str, value: &str, is_macro: bool, no_escape: bool) {
    static CAPS: OnceLock<Regex> = OnceLock::new();
    static PAGES: OnceLock<Regex> = OnceLock::new();
    if value.is_empty() {
        return;
    }
    let mut value = value.to_owned();
    ctx.write(&format!(",\n\t{field} = "));
    if !is_macro {
        ctx.write("{");
    }
    if !no_escape && !is_macro && !matches!(field, "url" | "doi" | "file" | "lccn") {
        value = escape_tex_specials(&escape_chars(&value, "|~^\\{}"));
        value = map_html_markup(&value);
        value = escape_chars(&value, "<>");
        if field != "pages" {
            // `/\b\p{L}+\p{Lu}\p{L}*/gu`: \b is ASCII in /u mode.
            value = re(&CAPS, || r"(?-u:\b)\p{L}+\p{Lu}\p{L}*".into())
                .replace_all(&value, "{${0}}")
                .into_owned();
        }
        if field == "pages" {
            value = re(&PAGES, || "[-\u{2012}-\u{2015}\u{2053}]+".into())
                .replace_all(&value, "--")
                .into_owned();
        }
    }
    if !matches!(field, "url" | "doi" | "file") {
        value = map_html_markup(&value);
    }
    ctx.write(&value);
    if !is_macro {
        ctx.write("}");
    }
}

/// `creatorCheck(item, ctype)` (:401-411).
fn creator_check(item: &TranslatorItem, ctype: &str) -> bool {
    item.creators
        .iter()
        .any(|c| c.creator_type.as_deref() == Some(ctype))
}

/// `encodeFilePathComponent` (:478-484): escapes `\ : ; { } $`.
fn encode_file_path_component(v: &str) -> String {
    let mut out = String::new();
    for c in v.chars() {
        if "\\:;{}$".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// JavaScript `parseInt(s)` (base 10), `None` for NaN.
fn parse_int(s: &str) -> Option<i64> {
    let t = js::trim_start(s);
    let (neg, rest) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let d: String = rest.chars().take_while(char::is_ascii_digit).collect();
    let n: i64 = d.parse().ok()?;
    Some(if neg { -n } else { n })
}

fn get<'a>(item: &'a TranslatorItem, k: &str) -> Option<String> {
    item.get(k)
        .filter(|v| js::truthy(Some(v)))
        .map(js::to_js_string)
}

/// The first truthy of several properties (`a || b || ...`).
fn first_of(item: &TranslatorItem, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|k| get(item, k))
}

/// `doExport` (:492-883).
pub fn do_export(ctx: &mut ExportContext) -> Result<(), TranslateError> {
    static LANG: OnceLock<Regex> = OnceLock::new();
    static PHD: OnceLock<Regex> = OnceLock::new();
    ctx.write("\n");
    let mut first = true;
    let mut citekeys: Vec<String> = Vec::new();
    while let Some(item) = ctx.next_item() {
        if item.item_type == "note" || item.item_type == "attachment" {
            continue;
        }
        let mut noteused = false;
        let mut btype = TYPE_MAP
            .iter()
            .find(|(z, _)| *z == item.item_type)
            .map(|(_, b)| *b);
        if item.item_type == "bookSection" && creator_check(&item, "bookAuthor") {
            btype = Some("inbook");
        }
        if item.item_type == "book"
            && !creator_check(&item, "author")
            && creator_check(&item, "editor")
        {
            btype = Some("collection");
        }
        if btype == Some("book") && item.truthy("numberOfVolumes") {
            btype = Some("mvbook");
        }
        let btype = btype.unwrap_or("misc");

        let mut extra = item
            .get_str("extra")
            .filter(|e| !e.is_empty())
            .map(parse_extra_fields);
        let citekey = build_cite_key(
            &item,
            extra.as_mut(),
            &mut citekeys,
            "BibLaTeX.export.simpleCitekey",
            ctx,
        );
        ctx.write(&format!(
            "{}@{btype}{{{citekey}",
            if first { "" } else { "\n\n" }
        ));
        first = false;

        for (field, zfield) in FIELD_MAP.iter() {
            if let Some(v) = get(&item, zfield) {
                write_field(ctx, field, &v, false, false);
            }
        }

        let has_number = first_of(
            &item,
            &[
                "reportNumber",
                "seriesNumber",
                "billNumber",
                "episodeNumber",
            ],
        )
        .is_some()
            || (item.truthy("number") && !item.truthy("patentNumber"));
        if has_number {
            let n = first_of(
                &item,
                &[
                    "reportNumber",
                    "seriesNumber",
                    "billNumber",
                    "episodeNumber",
                    "number",
                ],
            );
            write_field(ctx, "number", &n.unwrap_or_default(), false, false);
        }

        if let Some(issue) = get(&item, "issue") {
            match parse_int(&issue) {
                Some(n) => write_field(ctx, "number", &n.to_string(), false, false),
                None => write_field(ctx, "issue", &issue, false, false),
            }
        }

        if let Some(pt) = get(&item, "publicationTitle") {
            match item.item_type.as_str() {
                "bookSection" | "conferencePaper" | "dictionaryEntry" | "encyclopediaArticle" => {
                    write_field(ctx, "booktitle", &pt, false, false)
                }
                "magazineArticle" | "newspaperArticle" => {
                    write_field(ctx, "journaltitle", &pt, false, false)
                }
                "journalArticle" => {
                    let ja = get(&item, "journalAbbreviation");
                    if ctx.options.option_truthy("useJournalAbbreviation") && ja.is_some() {
                        write_field(ctx, "journaltitle", &ja.unwrap(), false, false);
                    } else {
                        write_field(ctx, "journaltitle", &pt, false, false);
                        write_field(ctx, "shortjournal", &ja.unwrap_or_default(), false, false);
                    }
                }
                _ => {}
            }
        }

        if let Some(t) = first_of(
            &item,
            &["websiteTitle", "forumTitle", "blogTitle", "programTitle"],
        ) {
            write_field(ctx, "titleaddon", &t, false, false);
        }

        if let Some(p) = get(&item, "publisher") {
            let f = if item.item_type == "thesis" || item.item_type == "report" {
                "institution"
            } else {
                "publisher"
            };
            write_field(ctx, f, &p, false, false);
        }

        let phd = re(&PHD, || r"(?i)ph\.?d".into());
        if item.item_type == "letter" {
            let lt = get(&item, "letterType").unwrap_or_else(|| "Letter".into());
            write_field(ctx, "type", &lt, false, false);
        } else if item.item_type == "email" {
            write_field(ctx, "type", "E-mail", false, false);
        } else if item.item_type == "thesis"
            && get(&item, "thesisType").is_none_or(|t| phd.is_match(&t))
        {
            write_field(ctx, "type", "phdthesis", false, false);
        } else if let Some(t) = first_of(
            &item,
            &[
                "manuscriptType",
                "thesisType",
                "websiteType",
                "presentationType",
                "reportType",
                "mapType",
            ],
        ) {
            write_field(ctx, "type", &t, false, false);
        } else if item.item_type == "patent" {
            match get(&item, "patentNumber") {
                None => write_field(ctx, "type", "patent", false, false),
                Some(pn) => {
                    let mut done = false;
                    for (pre, ty) in [
                        ("US", "patentus"),
                        ("EP", "patenteu"),
                        ("GB", "patentuk"),
                        ("DE", "patentde"),
                        ("FR", "patentfr"),
                    ] {
                        if let Some(rest) = pn.strip_prefix(pre) {
                            write_field(ctx, "type", ty, false, false);
                            write_field(ctx, "number", rest, false, false);
                            done = true;
                            break;
                        }
                    }
                    if !done {
                        write_field(ctx, "type", "patent", false, false);
                        write_field(ctx, "number", &pn, false, false);
                    }
                }
            }
        }

        if let Some(h) = first_of(&item, &["presentationType", "manuscriptType"]) {
            write_field(ctx, "howpublished", &h, false, false);
        }

        if let (Some(archive), Some(loc)) = (get(&item, "archive"), get(&item, "archiveLocation")) {
            let eprinttype = match archive.as_str() {
                "arXiv" | "arxiv" => Some("arxiv"),
                "JSTOR" | "jstor" => Some("jstor"),
                "PubMed" | "pubmed" => Some("pubmed"),
                "HDL" | "hdl" => Some("hdl"),
                "googlebooks" | "Google Books" => Some("googlebooks"),
                _ => None,
            };
            if let Some(et) = eprinttype {
                write_field(ctx, "eprinttype", et, false, false);
                write_field(ctx, "eprint", &loc, false, false);
                if et == "arxiv" {
                    if let Some(cn) = get(&item, "callNumber") {
                        write_field(ctx, "eprintclass", &cn, false, false);
                    }
                }
            }
        }

        if let Some(m) = get(&item, "meetingName") {
            write_field(ctx, "note", &m, false, false);
            noteused = true;
        }

        if !item.creators.is_empty() {
            let mut lists: [(&str, String); 8] = [
                ("author", String::new()),
                ("bookauthor", String::new()),
                ("commentator", String::new()),
                ("editor", String::new()),
                ("editora", String::new()),
                ("editorb", String::new()),
                ("holder", String::new()),
                ("translator", String::new()),
            ];
            let mut no_escape = false;
            for c in &item.creators {
                let Some(s) = creator_string(c.first_name.as_deref(), c.last_name.as_deref())
                else {
                    return Err(TranslateError::Translator(
                        "TypeError: Cannot read properties of undefined (reading 'replace')".into(),
                    ));
                };
                let mut s = escape_tex_specials(&escape_chars(&s, "|<>~^\\{}"));
                if c.field_mode == Some(1) {
                    s = format!("{{{s}}}");
                    no_escape = true;
                } else {
                    s = protect_and(&s);
                }
                let idx = match c.creator_type.as_deref().unwrap_or("") {
                    "author" | "interviewer" | "inventor" | "director" | "programmer"
                    | "artist" | "podcaster" | "presenter" => 0,
                    "bookAuthor" => 1,
                    "commenter" => 2,
                    "editor" => 3,
                    "translator" => 7,
                    "seriesEditor" => 5,
                    _ => 4,
                };
                lists[idx].1.push_str(" and ");
                lists[idx].1.push_str(&s);
            }
            for (name, v) in lists.iter() {
                if v.is_empty() {
                    continue;
                }
                write_field(ctx, name, &v[5..], false, no_escape);
                if *name == "editora" {
                    write_field(ctx, "editoratype", "collaborator", false, false);
                }
                if *name == "editorb" {
                    write_field(ctx, "editorbtype", "redactor", false, false);
                }
            }
        }

        if let Some(ad) = get(&item, "accessDate") {
            if let Some(iso) = str_to_iso(&ad, &ctx.options.env.dates) {
                write_field(ctx, "urldate", &iso, false, false);
            }
        }
        if let Some(d) = get(&item, "date") {
            if let Some(iso) = str_to_iso(&d, &ctx.options.env.dates) {
                write_field(ctx, "date", &iso, false, false);
            }
        }

        if let Some(language) = get(&item, "language") {
            let lre = re(&LANG, || {
                r"(?i)^([a-z]{2,3})(?:[^a-z]([^\n\r\x{2028}\x{2029}]+))?$".into()
            });
            if let Some(m) = lre.captures(&language) {
                match babel(&m[1]) {
                    Some(Babel::One(l)) => write_field(ctx, "langid", l, false, false),
                    Some(Babel::Variants(vs)) => {
                        // lang[langcode[2]]: an absent group is lang["undefined"].
                        let sub = m.get(2).map_or("undefined", |g| g.as_str());
                        let v = vs
                            .iter()
                            .find(|(k, _)| *k == sub)
                            .or_else(|| vs.iter().find(|(k, _)| k.is_empty()));
                        if let Some((_, l)) = v {
                            write_field(ctx, "langid", l, false, false);
                        }
                    }
                    None => {}
                }
            }
        }

        if let Some(mut extra) = extra {
            let mut i = 0;
            while i < extra.len() {
                let rec = extra[i].clone();
                let Some(f) = rec.field.as_deref().filter(|f| !f.is_empty()) else {
                    i += 1;
                    continue;
                };
                let id = REV_EXTRA_IDS.iter().find(|(k, _)| *k == f).map(|(_, v)| *v);
                let ep = REV_EPRINT_IDS
                    .iter()
                    .find(|(k, _)| *k == f)
                    .map(|(_, v)| *v);
                if id.is_none() && ep.is_none() {
                    i += 1;
                    continue;
                }
                let value = js::trim(rec.value.as_deref().unwrap_or("")).to_owned();
                if value.is_empty() {
                    i += 1;
                    continue;
                }
                if let Some(label) = id {
                    write_field(ctx, label, &format!("{{{value}}}"), true, false);
                } else if let Some(label) = ep {
                    write_field(ctx, "eprinttype", label, false, false);
                    write_field(ctx, "eprint", &format!("{{{value}}}"), true, false);
                }
                extra.remove(i);
            }
            let e = extra_recs_to_string(&extra);
            if !e.is_empty() && !noteused {
                write_field(ctx, "note", &e, false, false);
            }
        }

        if !item.tags.is_empty() {
            let mut ts = String::new();
            for t in &item.tags {
                ts.push_str(", ");
                ts.push_str(&t.tag);
            }
            write_field(ctx, "keywords", &ts[2..], false, false);
        }

        if ctx.options.option_truthy("exportNotes") {
            for n in item.notes.clone() {
                write_field(ctx, "annotation", &unescape_html(&n.note), false, false);
            }
        }

        let mut attachment_string = String::new();
        for a in &item.attachments {
            let title = clean_file_path(a.get_str("title"));
            let path = a
                .get_str("localPath")
                .filter(|p| !p.is_empty())
                .map(|p| clean_file_path(Some(p)));
            if let Some(path) = path.filter(|p| !p.is_empty()) {
                attachment_string.push(';');
                attachment_string.push_str(&encode_file_path_component(&title));
                attachment_string.push(':');
                attachment_string.push_str(&encode_file_path_component(&path));
                attachment_string.push(':');
                attachment_string.push_str(&encode_file_path_component(
                    a.get_str("mimeType").unwrap_or(""),
                ));
            }
        }
        if !attachment_string.is_empty() {
            write_field(ctx, "file", &attachment_string[1..], false, false);
        }

        ctx.write(",\n}");
    }
    ctx.write("\n");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markup_and_caps() {
        let s = escape_chars("<i>x</i> y", "<>");
        assert_eq!(map_html_markup(&s), "\\textit{x} y");
        assert_eq!(parse_int("12a"), Some(12));
        assert_eq!(parse_int("a1"), None);
    }
}
