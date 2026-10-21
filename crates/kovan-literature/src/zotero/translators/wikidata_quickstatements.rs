// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "Wikidata QuickStatements.js" (translatorID
//   51e5355d-9974-484f-80b9-f84d2b55782e, lastUpdated 2025-08-11 00:00:00):
//   header :1-11, `typeMapping` :39-87, `propertyMapping` :90-98,
//   `nonStringProperties` :101, `languageMapping` :105-141,
//   `identifierMapping` :143-156, `zoteroItemToQuickStatements` :159-311,
//   `doExport` :313-323.
// Copyright (c) 2017-2025 Philipp Zumstein with contributors.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The Wikidata QuickStatements translator: export, one `CREATE` block of
//! QuickStatements commands per item (items whose Extra has a `QID: ` line
//! are skipped).
//!
//! The translator's `minVersion` (3.0) is below 4.0.27, so it reads the
//! legacy export item format the framework prepares.
//!
//! Not ported: lookups of inherited `Object.prototype` names in the mapping
//! tables (`typeMapping["constructor"]`, `"constructor" in
//! languageMapping`), which only an item type, Extra `itemType:` line or
//! language spelled like a JavaScript built-in would reach.
//!
//! **Maturity: AI draft (1).** Verified code-to-code against upstream run
//! in-process (`tests/zotero_translators.rs`,
//! `wikidata_quickstatements_export_matches_upstream`).

use crate::zotero::framework::options::{translator_type, TranslatorMetadata};
use crate::zotero::framework::utilities::str_to_iso;
use crate::zotero::framework::{js, ExportContext, TranslateError, TranslatorItem};
use kovan_common::zotero::date::{str_to_date, DateOptions};
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "51e5355d-9974-484f-80b9-f84d2b55782e",
    label: "Wikidata QuickStatements",
    creator: "Philipp Zumstein with contributors",
    target: "txt",
    min_version: "3.0",
    priority: 100,
    translator_type: translator_type::EXPORT,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2025-08-11 00:00:00",
};

/// `typeMapping` (:39-87).
fn type_mapping(t: &str) -> Option<&'static str> {
    Some(match t {
        "artwork" => "Q838948",
        "audioRecording" => "Q30070318",
        "bill" => "Q686822",
        "blogPost" => "Q17928402",
        "book" => "Q3331189",
        "bookSection" => "Q1980247",
        "case" => "Q2334719",
        "computerProgram" => "Q40056",
        "conferencePaper" => "Q23927052",
        "dictionaryEntry" => "Q30070414",
        "document" => "Q49848",
        "email" => "Q30070439",
        "encyclopediaArticle" => "Q17329259",
        "film" => "Q11424",
        "forumPost" => "Q7216866",
        "hearing" => "Q30070550",
        "instantMessage" => "Q30070565",
        "interview" => "Q178651",
        "journalArticle" => "Q13442814",
        "letter" => "Q133492",
        "magazineArticle" => "Q30070590",
        "manuscript" => "Q87167",
        "map" => "Q4006",
        "newspaperArticle" => "Q5707594",
        "patent" => "Q253623",
        "podcast" => "Q24634210",
        "presentation" => "Q604733",
        "radioBroadcast" => "Q1555508",
        "report" => "Q10870555",
        "statute" => "Q820655",
        "thesis" => "Q1266946",
        "tvBroadcast" => "Q15416",
        "videoRecording" => "Q30070675",
        "webpage" => "Q36774",
        "dataset" => "Q1172284",
        "figure" => "Q30070753",
        "musical_score" => "Q187947",
        "pamphlet" => "Q190399",
        "review" => "Q265158",
        "review-book" => "Q637866",
        "treaty" => "Q131569",
        _ => return None,
    })
}

/// `propertyMapping` (:90-98), in `for...in` order (no integer-like keys).
const PROPERTY_MAPPING: [(&str, &str); 7] = [
    ("P356", "DOI"),
    ("P953", "url"),
    ("P478", "volume"),
    ("P433", "issue"),
    ("P304", "pages"),
    ("P1104", "numPages"),
    ("P393", "edition"),
];

/// `nonStringProperties` (:101).
const NON_STRING_PROPERTIES: [&str; 1] = ["P1104"];

/// `languageMapping` (:105-141).
fn language_mapping(l: &str) -> Option<&'static str> {
    Some(match l {
        "en" => "Q1860",
        "zh" => "Q7850",
        "ru" => "Q7737",
        "fr" => "Q150",
        "ja" => "Q5287",
        "de" => "Q188",
        "es" => "Q1321",
        "sr" => "Q9299",
        "pl" => "Q809",
        "cs" => "Q9056",
        "it" => "Q652",
        "cy" => "Q9309",
        "pt" => "Q5146",
        "nl" => "Q7411",
        "sv" => "Q9027",
        "ar" => "Q13955",
        "ko" => "Q9176",
        "hu" => "Q9067",
        "da" => "Q9035",
        "fi" => "Q1412",
        "eu" => "Q8752",
        "he" => "Q9288",
        "la" => "Q397",
        "nb" => "Q25167",
        "no" => "Q9043",
        "el" => "Q9129",
        "tr" => "Q256",
        "ca" => "Q7026",
        "sl" => "Q9063",
        "ro" => "Q7913",
        "is" => "Q294",
        "grc" => "Q35497",
        "uk" => "Q8798",
        "fa" => "Q9168",
        "hy" => "Q8785",
        "ta" => "Q5885",
        _ => return None,
    })
}

/// `identifierMapping` (:143-156).
fn identifier_mapping(l: &str) -> Option<&'static str> {
    Some(match l {
        "PMID" => "P698",
        "PMCID" => "P932",
        "JSTOR ID" => "P888",
        "arXiv" => "P818",
        "Open Library ID" => "P648",
        "OCLC" => "P243",
        "IMDb ID" => "P345",
        "Google-Books-ID" => "P675",
        "OpenAlex" => "P10283",
        "CorpusID" => "P8299",
        "WOS" => "P8372",
        "MAG" => "P6366",
        _ => return None,
    })
}

/// JavaScript `parseInt(s)` (no radix): leading whitespace, a sign, then
/// hexadecimal after `0x`/`0X`, else decimal digits; `NaN` when there are
/// none.
fn parse_int(s: &str) -> f64 {
    let t = js::trim_start(s);
    let (neg, t) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let (radix, digits) = match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        Some(r) => (16, r),
        None => (10, t),
    };
    let ds: Vec<u32> = digits.chars().map_while(|c| c.to_digit(radix)).collect();
    if ds.is_empty() {
        return f64::NAN;
    }
    let mut n = 0f64;
    for d in ds {
        n = n * f64::from(radix) + f64::from(d);
    }
    if neg {
        -n
    } else {
        n
    }
}

/// `Number#toString` of an integral or NaN value.
fn number_string(n: f64) -> String {
    if n.is_nan() {
        "NaN".to_owned()
    } else if n.fract() == 0.0 && n.abs() < 1e21 {
        format!("{n:.0}")
    } else {
        n.to_string()
    }
}

/// A field value of the (mutable) item copy: JSON, or a number the
/// translator computed.
#[derive(Clone)]
enum Field {
    Json(Value),
    Num(f64),
}

impl Field {
    fn truthy(&self) -> bool {
        match self {
            Field::Json(v) => js::truthy(Some(v)),
            Field::Num(n) => *n != 0.0 && !n.is_nan(),
        }
    }

    fn to_js_string(&self) -> String {
        match self {
            Field::Json(v) => js::to_js_string(v),
            Field::Num(n) => number_string(*n),
        }
    }
}

/// `item[key]` (`None` is `undefined`).
fn field(item: &TranslatorItem, key: &str) -> Option<Field> {
    item.get(key).cloned().map(Field::Json)
}

/// `item[key]` as a string when truthy.
fn truthy_str(item: &TranslatorItem, key: &str) -> Option<String> {
    let v = item.get(key);
    js::truthy(v).then(|| js::to_js_string(v.unwrap()))
}

/// `"" + item[key]`.
fn concat(item: &TranslatorItem, key: &str) -> String {
    item.get(key)
        .map_or_else(|| "undefined".to_owned(), js::to_js_string)
}

/// `zoteroItemToQuickStatements(item)` (:159-311).
fn item_to_quick_statements(
    item: &TranslatorItem,
    dates: &DateOptions,
) -> Result<String, TranslateError> {
    static PAGES: OnceLock<Regex> = OnceLock::new();
    static ITEM_TYPE: OnceLock<Regex> = OnceLock::new();
    static PNUM: OnceLock<Regex> = OnceLock::new();
    static QNUM: OnceLock<Regex> = OnceLock::new();
    let pages_re = PAGES.get_or_init(|| Regex::new("^([0-9]+)[–-]([0-9]+)$").unwrap());
    let item_type_re =
        ITEM_TYPE.get_or_init(|| Regex::new(r"itemType: ([A-Za-z0-9_-]+)(?:$|\n)").unwrap());
    let pnum = PNUM.get_or_init(|| Regex::new("^P[0-9]+$").unwrap());
    let qnum = QNUM.get_or_init(|| Regex::new("^Q[0-9]+$").unwrap());

    // The fields the function changes on its copy of the item.
    let mut num_pages = field(item, "numPages");
    let mut edition = field(item, "edition");

    // numPages from a page range (:161-166).
    if let Some(pages) = truthy_str(item, "pages") {
        if !num_pages.as_ref().is_some_and(Field::truthy) {
            if let Some(m) = pages_re.captures(&pages) {
                num_pages = Some(Field::Num(parse_int(&m[2]) - parse_int(&m[1]) + 1.0));
            }
        }
    }
    // Edition (:168-173).
    if edition.as_ref().is_some_and(Field::truthy) {
        let e = parse_int(&edition.as_ref().unwrap().to_js_string());
        edition = if e == 1.0 { None } else { Some(Field::Num(e)) };
    }

    let mut statements: Vec<String> = vec!["CREATE".to_owned()];
    let mut add = |args: &[&str]| statements.push(format!("LAST\t{}", args.join("\t")));

    let mut item_type = item.item_type.clone();
    if let Some(extra) = truthy_str(item, "extra") {
        if let Some(m) = item_type_re.captures(&extra) {
            item_type = m[1].to_owned();
        }
    }
    if let Some(q) = type_mapping(&item_type) {
        add(&["P31", q]);
    }

    // `itemType.replace(/([A-Z])/, ...)`: the first capital only.
    let mut description = match item_type
        .char_indices()
        .find(|(_, c)| c.is_ascii_uppercase())
    {
        Some((i, c)) => format!(
            "{} {}{}",
            &item_type[..i],
            c.to_ascii_lowercase(),
            &item_type[i + 1..]
        ),
        None => item_type.clone(),
    };
    for (field_name, types) in [
        (
            "publicationTitle",
            &["journalArticle", "magazineArticle", "newspaperArticle"][..],
        ),
        ("proceedingsTitle", &["conferencePaper"][..]),
        ("bookTitle", &["bookSection"][..]),
        ("encyclopediaTitle", &["encyclopediaArticle"][..]),
        ("university", &["thesis"][..]),
    ] {
        if let Some(v) = truthy_str(item, field_name) {
            if types.contains(&item_type.as_str()) {
                description = format!("{description} from '{v}'");
            }
        }
    }
    if let Some(date) = truthy_str(item, "date") {
        if let Some(year) = str_to_date(&date, dates).year.filter(|y| !y.is_empty()) {
            description = format!("{description} published in {year}");
        }
    }
    add(&["Den", &format!("\"{description}\"")]);

    for (p, zfield) in PROPERTY_MAPPING {
        let v = match zfield {
            "numPages" => num_pages.clone(),
            "edition" => edition.clone(),
            _ => field(item, zfield),
        };
        let Some(v) = v.filter(Field::truthy) else {
            continue;
        };
        if NON_STRING_PROPERTIES.contains(&p) {
            add(&[p, &v.to_js_string()]);
        } else {
            add(&[p, &format!("\"{}\"", v.to_js_string())]);
        }
    }

    // Authors (:225-238).
    let mut index = 1;
    for c in &item.creators {
        let mut value = c.last_name.clone().unwrap_or_else(|| "undefined".into());
        if let Some(f) = c.first_name.as_deref().filter(|f| !f.is_empty()) {
            value = format!("{f} {value}");
        }
        if c.creator_type.as_deref() == Some("author") {
            add(&[
                "P2093",
                &format!("\"{value}\""),
                "P1545",
                &format!("\"{index}\""),
            ]);
            index += 1;
        }
    }

    // Date (:240-257). strToISO's `false` has no length: "false/11".
    if let Some(date) = truthy_str(item, "date") {
        let mut d = str_to_iso(&date, dates).unwrap_or_else(|| "false".into());
        match js::len(&d) {
            4 => d.push_str("-00-00T00:00:00Z/9"),
            7 => d.push_str("-00T00:00:00Z/10"),
            10 => d.push_str("T00:00:00Z/11"),
            _ => d.push_str("/11"),
        }
        add(&["P577", &format!("+{d}")]);
    }

    // ISBN (:259-270).
    let depending = [
        "bookSection",
        "conferencePaper",
        "dictionaryEntry",
        "encyclopediaArticle",
        "journalArticle",
        "magazineArticle",
        "newspaperArticle",
    ]
    .contains(&item_type.as_str());
    if let Some(isbn) = truthy_str(item, "ISBN") {
        if !depending {
            let digits = js::len(&isbn.replace('-', ""));
            if digits == 13 {
                add(&["P212", &format!("\"{isbn}\"")]);
            }
            if digits == 10 {
                add(&["P957", &format!("\"{isbn}\"")]);
            }
        }
    }

    let title = concat(item, "title");
    add(&["Lmul", &format!("\"{title}\"")]);
    let lang = truthy_str(item, "language").map(|l| l.to_lowercase());
    match lang
        .as_deref()
        .and_then(|l| language_mapping(l).map(|q| (l, q)))
    {
        Some((l, q)) => {
            add(&[&format!("L{l}"), &format!("\"{title}\"")]);
            add(&["P1476", &format!("{l}:\"{title}\"")]);
            add(&["P407", q]);
        }
        None => {
            add(&["Len", &format!("\"{title}\"")]);
            add(&["P1476", &format!("und:\"{title}\"")]);
        }
    }

    // Identifiers and P-statements in Extra (:285-308).
    if let Some(extra) = truthy_str(item, "extra") {
        for line in extra.split('\n') {
            let Some(colon) = line.find(':') else {
                continue;
            };
            let label = &line[..colon];
            let value = js::trim(&line[colon + 1..]);
            if let Some(p) = identifier_mapping(label) {
                add(&[p, &format!("\"{value}\"")]);
            }
            if pnum.is_match(label) {
                if qnum.is_match(value) {
                    add(&[label, value]);
                } else {
                    add(&[label, &format!("\"{value}\"")]);
                }
            }
        }
    }

    Ok(statements.join("\n") + "\n")
}

/// `doExport` (:313-323).
pub fn do_export(ctx: &mut ExportContext) -> Result<(), TranslateError> {
    static QID: OnceLock<Regex> = OnceLock::new();
    // `/^QID: /m`: JavaScript's multiline `^` also follows \r, U+2028, U+2029.
    let qid = QID.get_or_init(|| Regex::new(r"(?:^|[\n\r\x{2028}\x{2029}])QID: ").unwrap());
    let dates = ctx.options.env.dates.clone();
    while let Some(item) = ctx.next_item() {
        if truthy_str(&item, "extra").is_some_and(|e| qid.is_match(&e)) {
            continue;
        }
        let s = item_to_quick_statements(&item, &dates)?;
        ctx.write(&s);
    }
    Ok(())
}
