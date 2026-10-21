// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "Open WorldCat.js" (translatorID
//   c73a4a8c-3ef1-4ec8-8229-7531ee384cc4, lastUpdated 2026-08-28 17:44:19):
//   `RELATORS` :38-50, `RECORD_MAPPING` :52-114, `getRecordItemType`
//   :151-163, `scrapeRecords` :200-229, `sanitizeInput` :231-268,
//   `detectSearch` :270-272, `doSearch` :274-327, `getSecureToken`
//   :329-343, `wcHyphenateISBN` :345-424 (with its ISBN ranges table),
//   `MODES`/`METHODS`/`split`/`hexToBytes`/`decrypt`/`decryptResponse`
//   :426-534. The web half (`detectWeb`, `doWeb`, `scrape`) is not ported.
// Copyright (c) 2022 Simon Kornblith, Sebastian Karcher, and Abe Jellinek.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The Open WorldCat search translator (ISBN or OCLC number -> WorldCat's
//! search API, after scraping a session token from its home page).
//!
//! **Not ported: AES-GCM.** WorldCat may answer with an encrypted body
//! (`{p, x, l}`), which upstream decrypts with WebCrypto AES-GCM. The key
//! and ciphertext are split out as upstream does ([`split`]), but the
//! decryption itself needs an AES-GCM implementation, which kovan-literature
//! does not depend on; such a response fails the translator with a
//! [`SearchError::Translator`] naming the reason (the search then moves to
//! the next translator, as upstream does after any failure). A plain JSON
//! answer is handled exactly.
//!
//! WorldCat is scraped behind a session token and was not contacted for
//! the fixtures; `fixtures/synthetic_open_worldcat.json` is kovan-authored.

use crate::zotero::framework::identifiers::{clean_isbn, clean_issn};
use crate::zotero::framework::item::{JsObject, TranslatorCreator, TranslatorItem, TranslatorTag};
use crate::zotero::framework::js;
use crate::zotero::framework::options::TranslatorMetadata;
use crate::zotero::framework::utilities::{clean_doi, get_creators_for_type};
use crate::zotero::search::http::{encode_uri_component, RequestOptions};
use crate::zotero::search::{SearchContext, SearchError, SearchTranslator};
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "c73a4a8c-3ef1-4ec8-8229-7531ee384cc4",
    label: "Open WorldCat",
    creator: "Simon Kornblith, Sebastian Karcher, Abe Jellinek",
    target: "^https?://([^/]+\\.)?worldcat\\.org/",
    min_version: "5.0",
    priority: 100,
    translator_type: 12,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2026-08-28 17:44:19",
};

/// `RELATORS` (:38-50).
fn relator(code: &str) -> Option<&'static str> {
    Some(match code {
        "act" => "castMember",
        "asn" => "contributor",
        "aut" => "author",
        "cmp" => "composer",
        "ctb" => "contributor",
        "drt" => "director",
        "edt" => "editor",
        "pbl" => "SKIP",
        "prf" => "performer",
        "pro" => "producer",
        "pub" => "SKIP",
        "trl" => "translator",
        _ => return None,
    })
}

/// `getRecordItemType` (:151-163).
fn record_item_type(record: &Value) -> &'static str {
    if record.get("generalFormat").and_then(Value::as_str) == Some("ArtChap") {
        if record.get("specificFormat").and_then(Value::as_str) == Some("Artcl") {
            "journalArticle"
        } else {
            "bookSection"
        }
    } else {
        "book"
    }
}

fn s(v: &Value) -> String {
    js::to_js_string(v)
}

fn opt(v: Option<String>) -> Value {
    v.map_or(Value::Bool(false), Value::String)
}

/// One `RECORD_MAPPING` entry applied (:52-114).
fn apply_mapping(ctx: &SearchContext, item: &mut TranslatorItem, key: &str, value: &Value) {
    match key {
        "oclcNumber" => {
            let extra = item
                .get("extra")
                .filter(|v| js::truthy(Some(v)))
                .map(s)
                .unwrap_or_default();
            item.set("extra", format!("{extra}\nOCLC: {}", s(value)));
        }
        "title" => item.set("title", s(value).replacen(" : ", ": ", 1)),
        "edition" => item.set("edition", value.clone()),
        "publisher" => item.set("publisher", value.clone()),
        "publicationPlace" => item.set("place", value.clone()),
        "publicationDate" => item.set("date", opt(ctx.str_to_iso(&s(value)))),
        "catalogingLanguage" => item.set("language", value.clone()),
        "summary" => item.set("abstractNote", value.clone()),
        "physicalDescription" => {
            static PAGES: OnceLock<Regex> = OnceLock::new();
            static DIGITS: OnceLock<Regex> = OnceLock::new();
            let v = s(value);
            let pages = PAGES.get_or_init(|| Regex::new("([0-9]+) page").expect("static regex"));
            let digits = DIGITS.get_or_init(|| Regex::new("[0-9]+").expect("static regex"));
            let n = pages
                .captures(&v)
                .map(|c| c[1].to_owned())
                .or_else(|| digits.find(&v).map(|m| m.as_str().to_owned()));
            match n {
                Some(n) => item.set("numPages", n),
                None => item.set("numPages", Value::Null),
            }
        }
        "series" => item.set("series", value.clone()),
        "subjectsText" => {
            item.tags = match value {
                Value::Array(a) => a.iter().filter_map(TranslatorTag::from_value).collect(),
                other => TranslatorTag::from_value(other).into_iter().collect(),
            }
        }
        "cartographicData" => item.set("scale", value.clone()),
        "doi" => item.set("DOI", opt(clean_doi(&s(value)))),
        "mediumOfPerformance" => item.set("medium", value.clone()),
        "issns" => {
            let text = match value {
                Value::Array(a) => a.iter().map(s).collect::<Vec<_>>().join(" "),
                other => s(other),
            };
            item.set("ISSN", opt(clean_issn(&text)));
        }
        "sourceIssn" => item.set("ISSN", opt(clean_issn(&s(value)))),
        "digitalAccessAndLocations" => {
            if let Some(first) = value.as_array().and_then(|a| a.first()) {
                item.set("url", first.get("uri").cloned().unwrap_or(Value::Null));
            }
        }
        "isbns" => {
            let text = match value {
                Value::Array(a) => a.iter().map(s).collect::<Vec<_>>().join(" "),
                other => s(other),
            };
            item.set("ISBN", opt(clean_isbn(&text, false)));
        }
        "isbn13" => item.set("ISBN", opt(clean_isbn(&s(value), false))),
        "publication" => {
            // try { [, a, b, c, d] = value.match(...) } catch { debug }
            static PUB: OnceLock<Regex> = OnceLock::new();
            let re =
                PUB.get_or_init(|| Regex::new("^(.+), (.+), (.+), (.+)$").expect("static regex"));
            if let Some(c) = re.captures(&s(value)) {
                item.set("publicationTitle", c[1].to_owned());
                item.set("volume", c[2].to_owned());
                item.set("date", opt(ctx.str_to_iso(&c[3])));
                item.set("pages", c[4].to_owned());
            }
        }
        "contributors" => {
            let Some(list) = value.as_array() else { return };
            for contrib in list {
                let code = contrib
                    .get("relatorCodes")
                    .and_then(|r| r.get(0))
                    .filter(|c| js::truthy(Some(c)));
                let creator_type = match code {
                    Some(c) => {
                        let t = relator(&s(c)).unwrap_or("contributor");
                        if t == "SKIP" {
                            continue;
                        }
                        t.to_owned()
                    }
                    None => get_creators_for_type(&item.item_type)
                        .first()
                        .map(|t| (*t).to_owned())
                        .unwrap_or_default(),
                };
                // `contrib.firstName && contrib.firstName.text`
                let name = |k: &str| -> Option<String> {
                    let v = contrib.get(k)?;
                    if !js::truthy(Some(v)) {
                        return None;
                    }
                    v.get("text").map(s)
                };
                let mut c = TranslatorCreator {
                    first_name: name("firstName"),
                    last_name: name("secondName"),
                    creator_type: Some(creator_type),
                    ..Default::default()
                };
                let truthy = |o: &Option<String>| o.as_deref().is_some_and(|x| !x.is_empty());
                if truthy(&c.first_name) && !truthy(&c.last_name) {
                    c.last_name = c.first_name.take();
                    c.field_mode = Some(1);
                }
                item.creators.push(c);
            }
        }
        _ => {}
    }
}

/// `RECORD_MAPPING`'s keys, in declaration order.
const RECORD_KEYS: [&str; 21] = [
    "oclcNumber",
    "title",
    "edition",
    "publisher",
    "publicationPlace",
    "publicationDate",
    "catalogingLanguage",
    "summary",
    "physicalDescription",
    "series",
    "subjectsText",
    "cartographicData",
    "doi",
    "mediumOfPerformance",
    "issns",
    "sourceIssn",
    "digitalAccessAndLocations",
    "isbns",
    "isbn13",
    "publication",
    "contributors",
];

/// `scrapeRecords(records)` (:200-229).
fn scrape_records(ctx: &mut SearchContext, records: &[Value]) -> Result<(), SearchError> {
    for record in records {
        if let Some(doi) = record.get("doi").filter(|d| js::truthy(Some(d))) {
            // A child DOI Content Negotiation search with the default
            // handlers: its items are this translator's; its error ends
            // this translation (`translate.complete(false, error)`).
            let mut search = JsObject::new();
            search.set("DOI", doi.clone());
            let child = ctx.child_search(SearchTranslator::DoiContentNegotiation, &search)?;
            for item in child.items {
                ctx.complete(item)?;
            }
            if let Some(e) = child.error {
                return Err(e);
            }
            continue;
        }
        let mut item = TranslatorItem::new(record_item_type(record));
        for key in RECORD_KEYS {
            let Some(v) = record.get(key).filter(|v| js::truthy(Some(v))) else {
                continue;
            };
            apply_mapping(ctx, &mut item, key, v);
        }
        static BRACKETS: OnceLock<Regex> = OnceLock::new();
        let re = BRACKETS.get_or_init(|| Regex::new(r"^\[(.+)\]$").expect("static regex"));
        for k in ["title", "publisher", "place"] {
            if let Some(Value::String(v)) = item.get(k).filter(|v| js::truthy(Some(v))).cloned() {
                item.set(k, re.replace(&v, "$1").into_owned());
            }
        }
        ctx.complete(item)?;
    }
    Ok(())
}

/// A sanitised search item (`sanitizeInput`, :231-268): the cleaned ISBN
/// and the trimmed OCLC number, when valid.
struct Clean {
    isbn: Option<String>,
    oclc: Option<String>,
}

fn sanitize_input(search: &JsObject) -> Option<Clean> {
    let isbn = match search.get("ISBN") {
        Some(Value::String(i)) if !i.is_empty() => clean_isbn(i, false),
        _ => None,
    };
    let oclc = search
        .get("identifiers")
        .and_then(|i| i.get("oclc"))
        .and_then(Value::as_str)
        .map(|o| js::trim(o).to_owned())
        .filter(|o| !o.is_empty() && o.bytes().all(|b| b.is_ascii_digit()));
    (isbn.is_some() || oclc.is_some()).then_some(Clean { isbn, oclc })
}

/// `detectSearch` (:270-272).
pub fn detect_search(search: &JsObject) -> bool {
    sanitize_input(search).is_some()
}

/// `doSearch` (:274-327).
pub fn do_search(ctx: &mut SearchContext, search: &JsObject) -> Result<(), SearchError> {
    let Some(clean) = sanitize_input(search) else {
        return Ok(());
    };
    // An item with an OCLC number is looked up by it only.
    let (ids, isbns): (Vec<String>, Vec<String>) = match (clean.oclc, clean.isbn) {
        (Some(o), _) => (vec![o], Vec::new()),
        (None, Some(i)) => (Vec::new(), vec![i]),
        (None, None) => (Vec::new(), Vec::new()),
    };
    let token = get_secure_token(ctx)?;
    let cookie = format!("wc_tkn={}", encode_uri_component(&token));
    let headers = RequestOptions::headers(&[
        ("Referer", "https://search.worldcat.org/search?q="),
        ("Cookie", &cookie),
    ]);
    let mut found = false;
    for isbn in isbns {
        let isbn = wc_hyphenate_isbn(&isbn).unwrap_or(isbn);
        let url = format!(
            "https://search.worldcat.org/api/search?q=bn%3A{}",
            encode_uri_component(&isbn)
        );
        let json = decrypt_response(ctx.request_json(&url, &headers)?)?;
        if let Some(first) = brief_records(&json).and_then(|r| r.first().cloned()) {
            scrape_records(ctx, &[first])?;
            found = true;
        }
    }
    if !ids.is_empty() {
        let q: Vec<String> = ids.iter().map(|i| encode_uri_component(i)).collect();
        let url = format!(
            "https://search.worldcat.org/api/search?q=no%3A{}",
            q.join("+OR+no%3A")
        );
        let json = decrypt_response(ctx.request_json(&url, &headers)?)?;
        if let Some(records) = brief_records(&json) {
            scrape_records(ctx, &records)?;
            found = true;
        }
    }
    // `if (!found) Zotero.done(false)`: a no-op outside detection; the
    // search simply returns no items.
    let _ = found;
    Ok(())
}

/// `json && json.briefRecords && json.briefRecords.length` -> the records.
fn brief_records(json: &Value) -> Option<Vec<Value>> {
    let r = json.get("briefRecords")?;
    if !js::truthy(Some(r)) {
        return None;
    }
    let a = r.as_array()?;
    (!a.is_empty()).then(|| a.clone())
}

/// `getSecureToken` (:329-343).
fn get_secure_token(ctx: &mut SearchContext) -> Result<String, SearchError> {
    let doc = match ctx.request_document("https://search.worldcat.org/", &RequestOptions::default())
    {
        Ok((doc, _)) => doc,
        Err(e) if e.is_pending() => return Err(e),
        // "Initial request to homepage failed; trying archive.org"
        Err(_) => {
            ctx.request_document(
                "https://web.archive.org/web/https://search.worldcat.org/",
                &RequestOptions::default(),
            )?
            .0
        }
    };
    // text(doc, '#__NEXT_DATA__'): its textContent, trimmed ("" if absent).
    let data = doc
        .get_element_by_id("__NEXT_DATA__")
        .map(|n| js::trim(&doc.text(n)).to_owned())
        .unwrap_or_default();
    let next: Value = serde_json::from_str(&data)
        .map_err(|e| SearchError::Translator(format!("SyntaxError: JSON.parse: {e}")))?;
    let build_id = next
        .get("buildId")
        .map(s)
        .unwrap_or_else(|| "undefined".to_owned());
    let json = ctx.request_json(
        &format!("https://search.worldcat.org/_next/data/{build_id}/en/search.json"),
        &RequestOptions::default(),
    )?;
    let token = json
        .get("pageProps")
        .ok_or_else(|| {
            SearchError::Translator("TypeError: json.pageProps is undefined".to_owned())
        })?
        .get("secureToken")
        .map(s)
        .unwrap_or_else(|| "undefined".to_owned());
    Ok(token)
}

/// The ISBN ranges table of `wcHyphenateISBN` (copied from Zotero's
/// isbn.js into the translator): per prefix, per group, the registrant
/// ranges as (low, high) pairs, shortest first.
static RANGES: &[(&str, &[(&str, &[&str])])] = &[
    (
        "978",
        &[
            (
                "0",
                &[
                    "00", "19", "200", "227", "229", "368", "370", "638", "640", "644", "646",
                    "647", "649", "654", "656", "699", "2280", "2289", "3690", "3699", "6390",
                    "6397", "6550", "6559", "7000", "8499", "85000", "89999", "900000", "900370",
                    "900372", "949999", "6398000", "6399999", "6450000", "6459999", "6480000",
                    "6489999", "9003710", "9003719", "9500000", "9999999",
                ],
            ),
            (
                "1",
                &[
                    "01", "02", "05", "05", "000", "009", "030", "034", "040", "045", "047", "047",
                    "100", "397", "714", "716", "0350", "0399", "0460", "0469", "0480", "0499",
                    "0700", "0999", "3980", "5499", "6500", "6799", "6860", "7139", "7170", "7319",
                    "7620", "7634", "7900", "7999", "8672", "8675", "9730", "9877", "55000",
                    "64999", "68000", "68599", "74000", "76199", "76500", "77499", "77540",
                    "77639", "77650", "77699", "77830", "78999", "80000", "80049", "80050",
                    "80499", "80500", "83799", "83850", "86719", "86760", "86979", "869800",
                    "915999", "916506", "916869", "916908", "919163", "919565", "919599", "919655",
                    "972999", "987800", "991149", "991200", "998989", "0665000", "0665749",
                    "0665750", "0665999", "0666000", "0669999", "0670000", "0699999", "7320000",
                    "7399999", "7635000", "7649999", "7750000", "7753999", "7764000", "7764999",
                    "7770000", "7782999", "8380000", "8384999", "9160000", "9165059", "9168700",
                    "9169079", "9191640", "9195649", "9196000", "9196549", "9911500", "9911999",
                    "9989900", "9999999",
                ],
            ),
            (
                "2",
                &[
                    "00", "19", "200", "349", "400", "486", "495", "495", "497", "527", "530",
                    "699", "4960", "4966", "5280", "5299", "7000", "8399", "35000", "39999",
                    "49670", "49699", "84000", "89999", "91980", "91980", "487000", "494999",
                    "900000", "919799", "919810", "919942", "919969", "949999", "9199430",
                    "9199689", "9500000", "9999999",
                ],
            ),
            (
                "3",
                &[
                    "00", "02", "04", "19", "39", "39", "030", "033", "200", "312", "314", "389",
                    "400", "688", "0340", "0369", "3130", "3139", "6950", "8499", "9996", "9999",
                    "03700", "03999", "68900", "69499", "85000", "89999", "95400", "96999",
                    "98500", "99959", "900000", "949999", "9500000", "9539999", "9700000",
                    "9849999",
                ],
            ),
            (
                "4",
                &[
                    "00", "19", "200", "699", "7000", "8499", "85000", "89999", "900000", "949999",
                    "9500000", "9999999",
                ],
            ),
            (
                "5",
                &[
                    "01", "19", "200", "361", "363", "420", "430", "430", "440", "440", "450",
                    "602", "605", "699", "0050", "0099", "3620", "3623", "4210", "4299", "4310",
                    "4399", "4410", "4499", "7000", "8499", "9200", "9299", "9501", "9799", "9910",
                    "9999", "00000", "00499", "36240", "36299", "85000", "89999", "91000", "91999",
                    "93000", "94999", "98000", "98999", "900000", "909999", "6030000", "6049999",
                    "9500000", "9500999", "9900000", "9909999",
                ],
            ),
            (
                "7",
                &[
                    "00", "09", "100", "499", "5000", "7999", "80000", "89999", "900000", "999999",
                ],
            ),
            (
                "65",
                &[
                    "00", "02", "250", "299", "300", "302", "5000", "6349", "80000", "81824",
                    "82000", "89999", "900000", "902449", "975500", "999999",
                ],
            ),
            ("66", &["30", "30"]),
            (
                "80",
                &[
                    "00", "19", "200", "529", "550", "689", "7000", "8499", "53000", "54999",
                    "69000", "69999", "85000", "89999", "99900", "99999", "900000", "998999",
                ],
            ),
            (
                "81",
                &[
                    "00", "18", "200", "669", "6700", "6799", "7000", "8499", "19000", "19999",
                    "68000", "68499", "69000", "69999", "85000", "89999", "685000", "689999",
                    "900000", "999999",
                ],
            ),
            (
                "82",
                &[
                    "00", "19", "200", "689", "7000", "8999", "90000", "98999", "690000", "699999",
                    "990000", "999999",
                ],
            ),
            (
                "83",
                &[
                    "00", "19", "200", "599", "7000", "8499", "60000", "69999", "85000", "89999",
                    "900000", "999999",
                ],
            ),
            (
                "84",
                &[
                    "00", "09", "140", "149", "200", "699", "1050", "1199", "1300", "1399", "7000",
                    "8499", "9000", "9199", "9700", "9999", "10000", "10499", "15000", "19999",
                    "85000", "89999", "92400", "92999", "95000", "96999", "120000", "129999",
                    "920000", "923999", "930000", "949999",
                ],
            ),
            (
                "85",
                &[
                    "00", "19", "96", "97", "200", "454", "456", "528", "534", "539", "5320",
                    "5339", "5440", "5479", "5500", "5999", "7000", "8499", "9450", "9599",
                    "45530", "45599", "52900", "53199", "54000", "54029", "54030", "54039",
                    "54050", "54089", "54100", "54399", "54800", "54999", "60000", "69999",
                    "85000", "89999", "92500", "94499", "98000", "99999", "455000", "455299",
                    "540400", "540499", "540900", "540999", "900000", "924999",
                ],
            ),
            (
                "86",
                &[
                    "00", "29", "300", "599", "6000", "7999", "80000", "89999", "900000", "999999",
                ],
            ),
            (
                "87",
                &[
                    "00", "29", "400", "649", "7000", "7999", "85000", "94999", "970000", "999999",
                ],
            ),
            (
                "88",
                &[
                    "00", "19", "200", "311", "315", "318", "323", "326", "339", "360", "363",
                    "548", "555", "599", "910", "926", "3270", "3389", "3610", "3629", "5490",
                    "5549", "6000", "8499", "9270", "9399", "31200", "31499", "31900", "32299",
                    "85000", "89999", "94800", "99999", "900000", "909999", "940000", "947999",
                ],
            ),
            (
                "89",
                &[
                    "00", "24", "250", "549", "990", "999", "5500", "8499", "85000", "94999",
                    "97000", "98999", "950000", "969999",
                ],
            ),
            (
                "90",
                &[
                    "00", "19", "90", "90", "94", "94", "200", "499", "5000", "6999", "8500",
                    "8999", "70000", "79999", "800000", "849999",
                ],
            ),
            (
                "91",
                &[
                    "0", "1", "20", "49", "500", "649", "6850", "8199", "85000", "94999", "970000",
                    "999999",
                ],
            ),
            (
                "92",
                &[
                    "0", "5", "60", "79", "800", "899", "9000", "9499", "95000", "98999", "990000",
                    "999999",
                ],
            ),
            (
                "93",
                &[
                    "00", "08", "100", "469", "0900", "0999", "5000", "7999", "47000", "47999",
                    "48000", "49999", "80000", "95999", "960000", "999999",
                ],
            ),
            (
                "94",
                &[
                    "000", "599", "6000", "6387", "6389", "6395", "6397", "6399", "6401", "6406",
                    "6408", "6419", "6421", "6432", "6434", "6435", "6437", "6443", "6445", "6450",
                    "6452", "6458", "6460", "6465", "6467", "6474", "6476", "6476", "6479", "6493",
                    "6495", "6497", "6499", "8999", "63881", "63881", "63884", "63885", "63887",
                    "63889", "63961", "63962", "63964", "63964", "63966", "63969", "64001",
                    "64004", "64006", "64006", "64009", "64009", "64074", "64074", "64076",
                    "64077", "64200", "64201", "64203", "64203", "64205", "64206", "64208",
                    "64208", "64330", "64331", "64333", "64333", "64336", "64336", "64338",
                    "64339", "64361", "64363", "64366", "64366", "64368", "64369", "64441",
                    "64441", "64443", "64443", "64445", "64446", "64449", "64449", "64510",
                    "64512", "64514", "64515", "64591", "64592", "64595", "64596", "64599",
                    "64599", "64661", "64662", "64666", "64666", "64669", "64669", "64750",
                    "64751", "64754", "64754", "64756", "64757", "64759", "64759", "64771",
                    "64771", "64773", "64773", "64777", "64779", "64781", "64781", "64783",
                    "64786", "64788", "64789", "64941", "64942", "64945", "64946", "64948",
                    "64948", "64980", "64980", "64983", "64984", "64987", "64987", "90000",
                    "99999", "638800", "638809", "638820", "638839", "638860", "638869", "639600",
                    "639609", "639630", "639639", "639650", "639659", "640000", "640009", "640050",
                    "640059", "640070", "640089", "640700", "640739", "640750", "640759", "640780",
                    "640799", "642020", "642029", "642040", "642049", "642070", "642079", "642090",
                    "642099", "643320", "643329", "643340", "643359", "643370", "643379", "643600",
                    "643609", "643640", "643659", "643670", "643679", "644400", "644409", "644420",
                    "644429", "644440", "644449", "644470", "644489", "645130", "645139", "645160",
                    "645199", "645900", "645909", "645930", "645949", "645970", "645989", "646600",
                    "646609", "646630", "646659", "646670", "646689", "647520", "647539", "647550",
                    "647559", "647580", "647589", "647700", "647708", "647723", "647729", "647740",
                    "647769", "647800", "647809", "647820", "647829", "647870", "647879", "649400",
                    "649409", "649430", "649449", "649470", "649479", "649490", "649499", "649810",
                    "649829", "649850", "649869", "649880", "649899",
                ],
            ),
            (
                "600",
                &[
                    "00", "09", "100", "499", "993", "995", "5000", "8999", "9868", "9929",
                    "90000", "98679", "99600", "99999",
                ],
            ),
            (
                "601",
                &[
                    "00", "19", "85", "99", "200", "699", "7000", "7999", "80000", "84999",
                ],
            ),
            (
                "602",
                &[
                    "00", "06", "200", "499", "0700", "1399", "1500", "1699", "5400", "5999",
                    "6200", "6999", "7500", "9499", "14000", "14999", "17000", "19999", "50000",
                    "53999", "60000", "61999", "70000", "74999", "95000", "99999",
                ],
            ),
            (
                "603",
                &[
                    "00", "04", "05", "49", "500", "799", "8000", "8999", "90000", "99999",
                ],
            ),
            (
                "604",
                &[
                    "0", "2", "40", "46", "50", "89", "300", "399", "470", "497", "900", "979",
                    "4980", "4999", "9800", "9999",
                ],
            ),
            (
                "605",
                &[
                    "00", "02", "04", "05", "07", "09", "030", "039", "100", "199", "240", "399",
                    "2000", "2399", "4000", "5999", "7500", "7999", "9000", "9999", "06000",
                    "06999", "60000", "74999", "80000", "89999",
                ],
            ),
            (
                "606",
                &[
                    "10", "49", "000", "099", "500", "799", "910", "919", "975", "999", "8000",
                    "9099", "9600", "9749", "92000", "95999",
                ],
            ),
            (
                "607",
                &[
                    "00", "25", "27", "39", "400", "588", "600", "691", "700", "749", "2600",
                    "2649", "5890", "5929", "7500", "9499", "26500", "26999", "59300", "59999",
                    "69200", "69999", "95000", "99999",
                ],
            ),
            (
                "608",
                &[
                    "0", "0", "7", "9", "10", "19", "200", "449", "4500", "6499", "65000", "69999",
                ],
            ),
            (
                "609",
                &["00", "39", "400", "799", "8000", "9499", "95000", "99999"],
            ),
            ("611", &[]),
            (
                "612",
                &[
                    "00", "29", "300", "399", "4000", "4499", "5000", "5299", "45000", "49999",
                    "99000", "99999",
                ],
            ),
            ("613", &["0", "9"]),
            (
                "614",
                &["00", "39", "400", "799", "8000", "9499", "95000", "99999"],
            ),
            (
                "615",
                &["00", "09", "100", "499", "5000", "7999", "80000", "89999"],
            ),
            (
                "616",
                &["00", "19", "200", "699", "7000", "8999", "90000", "99999"],
            ),
            (
                "617",
                &["00", "49", "500", "699", "7000", "8999", "90000", "99999"],
            ),
            (
                "618",
                &["00", "19", "200", "499", "5000", "7999", "80000", "99999"],
            ),
            (
                "619",
                &["00", "14", "150", "699", "7000", "8999", "90000", "99999"],
            ),
            ("620", &["0", "9"]),
            (
                "621",
                &["00", "29", "400", "599", "8000", "8999", "95000", "99999"],
            ),
            (
                "622",
                &[
                    "00", "10", "110", "129", "180", "182", "190", "194", "200", "459", "1300",
                    "1799", "1830", "1899", "4600", "8749", "19500", "19999", "87500", "99999",
                ],
            ),
            (
                "623",
                &["00", "10", "110", "524", "5250", "8799", "88000", "99999"],
            ),
            (
                "624",
                &["00", "04", "200", "249", "4850", "6899", "91000", "99999"],
            ),
            (
                "625",
                &[
                    "00", "01", "320", "442", "445", "449", "5000", "7793", "7795", "8999",
                    "44300", "44499", "77940", "77949", "90000", "99999",
                ],
            ),
            (
                "626",
                &["00", "04", "300", "499", "6500", "7999", "92500", "99999"],
            ),
            (
                "627",
                &["28", "31", "500", "534", "7400", "7999", "94500", "95149"],
            ),
            (
                "628",
                &["00", "09", "500", "549", "7500", "8499", "95000", "99999"],
            ),
            (
                "629",
                &["00", "02", "455", "499", "7500", "7999", "92000", "99999"],
            ),
            ("630", &["300", "399", "6500", "6849", "95000", "99999"]),
            (
                "631",
                &["00", "09", "300", "399", "6500", "7499", "90000", "99999"],
            ),
            ("632", &["00", "11", "600", "679"]),
            (
                "633",
                &["00", "01", "300", "349", "8250", "8999", "99500", "99999"],
            ),
            (
                "634",
                &["00", "05", "200", "349", "7000", "7999", "96000", "99999"],
            ),
            (
                "635",
                &["00", "04", "250", "324", "5800", "6999", "96000", "99999"],
            ),
            (
                "950",
                &["00", "49", "500", "899", "9000", "9899", "99000", "99999"],
            ),
            (
                "951",
                &[
                    "0", "1", "20", "54", "550", "889", "8900", "9499", "95000", "99999",
                ],
            ),
            (
                "952",
                &[
                    "00", "17", "60", "64", "80", "94", "180", "189", "200", "499", "5000", "5999",
                    "6600", "6699", "7000", "7999", "9500", "9899", "19500", "19999", "65000",
                    "65999", "67000", "69999", "99000", "99999",
                ],
            ),
            (
                "953",
                &[
                    "0", "0", "10", "14", "51", "54", "150", "459", "500", "500", "6000", "9499",
                    "46000", "49999", "50100", "50999", "55000", "59999", "95000", "99999",
                ],
            ),
            (
                "954",
                &[
                    "00", "28", "300", "799", "2900", "2999", "8000", "8999", "9300", "9999",
                    "90000", "92999",
                ],
            ),
            (
                "955",
                &[
                    "20", "33", "550", "710", "0000", "1999", "3400", "3549", "3600", "3799",
                    "3900", "4099", "4500", "4999", "7150", "9499", "35500", "35999", "38000",
                    "38999", "41000", "44999", "50000", "54999", "71100", "71499", "95000",
                    "99999",
                ],
            ),
            (
                "956",
                &[
                    "00", "07", "10", "19", "200", "599", "6000", "6999", "7000", "9999", "08000",
                    "08499", "09000", "09999",
                ],
            ),
            (
                "957",
                &[
                    "00", "02", "05", "19", "21", "27", "31", "43", "440", "819", "0300", "0499",
                    "2000", "2099", "8200", "9699", "28000", "30999", "97000", "99999",
                ],
            ),
            (
                "958",
                &[
                    "00", "49", "500", "509", "600", "799", "5100", "5199", "5400", "5599", "8000",
                    "9499", "52000", "53999", "56000", "59999", "95000", "99999",
                ],
            ),
            (
                "959",
                &["00", "19", "200", "699", "7000", "8499", "85000", "99999"],
            ),
            (
                "960",
                &[
                    "00", "19", "93", "93", "200", "659", "690", "699", "6600", "6899", "7000",
                    "8499", "9400", "9799", "85000", "92999", "98000", "99999",
                ],
            ),
            (
                "961",
                &["00", "19", "200", "599", "6000", "8999", "90000", "97999"],
            ),
            (
                "962",
                &[
                    "00", "19", "200", "699", "900", "999", "7000", "8499", "8700", "8999",
                    "85000", "86999",
                ],
            ),
            (
                "963",
                &[
                    "00", "19", "200", "699", "7000", "8499", "9000", "9999", "85000", "89999",
                ],
            ),
            (
                "964",
                &[
                    "00", "14", "150", "249", "300", "549", "970", "989", "2500", "2999", "5500",
                    "8999", "9900", "9999", "90000", "96999",
                ],
            ),
            (
                "965",
                &["00", "19", "200", "599", "7000", "7999", "90000", "99999"],
            ),
            (
                "966",
                &[
                    "00", "12", "14", "14", "130", "139", "170", "199", "279", "289", "300", "699",
                    "910", "949", "980", "999", "1500", "1699", "2000", "2789", "2900", "2999",
                    "7000", "8999", "90000", "90999", "95000", "97999",
                ],
            ),
            (
                "967",
                &[
                    "60", "89", "250", "254", "300", "499", "900", "989", "0000", "0999", "2000",
                    "2499", "2700", "2799", "2800", "2999", "5000", "5999", "9900", "9989",
                    "10000", "19999", "25500", "26999", "99900", "99999",
                ],
            ),
            (
                "968",
                &[
                    "01", "39", "400", "499", "800", "899", "5000", "7999", "9000", "9999",
                ],
            ),
            (
                "969",
                &[
                    "0", "1", "20", "20", "24", "39", "210", "219", "400", "749", "2200", "2299",
                    "7500", "9999", "23000", "23999",
                ],
            ),
            (
                "970",
                &[
                    "01", "59", "600", "899", "9000", "9099", "9700", "9999", "91000", "96999",
                ],
            ),
            (
                "971",
                &[
                    "02", "02", "06", "49", "97", "98", "000", "015", "500", "849", "0160", "0199",
                    "0300", "0599", "8500", "9099", "9600", "9699", "9900", "9999", "91000",
                    "95999",
                ],
            ),
            (
                "972",
                &[
                    "0", "1", "20", "54", "550", "799", "8000", "9499", "95000", "99999",
                ],
            ),
            (
                "973",
                &[
                    "0", "0", "20", "54", "100", "169", "550", "759", "1700", "1999", "7600",
                    "8499", "8900", "9499", "85000", "88999", "95000", "99999",
                ],
            ),
            (
                "974",
                &[
                    "00", "19", "200", "699", "7000", "8499", "9500", "9999", "85000", "89999",
                    "90000", "94999",
                ],
            ),
            (
                "975",
                &[
                    "02", "23", "250", "599", "990", "999", "2400", "2499", "6000", "9199",
                    "00000", "01999", "92000", "98999",
                ],
            ),
            (
                "976",
                &[
                    "0", "3", "40", "59", "600", "799", "8000", "9499", "95000", "99999",
                ],
            ),
            (
                "977",
                &[
                    "00", "19", "90", "95", "200", "499", "700", "849", "890", "894", "970", "999",
                    "5000", "6999", "8740", "8899", "8950", "8999", "9600", "9699", "85000",
                    "87399",
                ],
            ),
            (
                "978",
                &[
                    "67", "68", "000", "199", "690", "699", "765", "799", "900", "999", "2000",
                    "2999", "8000", "8999", "30000", "66999",
                ],
            ),
            (
                "979",
                &[
                    "20", "29", "000", "099", "400", "799", "1000", "1499", "3000", "3999", "8000",
                    "9499", "15000", "19999", "95000", "99999",
                ],
            ),
            ("980", &["00", "19", "200", "599", "6000", "9999"]),
            (
                "981",
                &[
                    "00", "16", "18", "19", "92", "99", "200", "299", "310", "399", "3000", "3099",
                    "4000", "5999", "17000", "17999",
                ],
            ),
            (
                "982",
                &[
                    "00", "09", "70", "89", "100", "699", "9000", "9799", "98000", "99999",
                ],
            ),
            (
                "983",
                &[
                    "00", "01", "45", "49", "50", "79", "020", "199", "800", "899", "2000", "3999",
                    "9000", "9899", "40000", "44999", "99000", "99999",
                ],
            ),
            (
                "984",
                &[
                    "00", "21", "26", "28", "30", "38", "220", "224", "400", "799", "2250", "2599",
                    "3900", "3999", "8000", "8999", "29000", "29999", "90000", "99999",
                ],
            ),
            (
                "985",
                &[
                    "00", "39", "400", "599", "880", "899", "6000", "8799", "90000", "99999",
                ],
            ),
            (
                "986",
                &[
                    "00", "05", "08", "11", "120", "539", "0700", "0799", "5400", "7999", "06000",
                    "06999", "80000", "99999",
                ],
            ),
            (
                "987",
                &[
                    "00", "09", "30", "35", "42", "43", "85", "88", "500", "824", "1000", "1999",
                    "3600", "4199", "4400", "4499", "4900", "4999", "8250", "8279", "8300", "8499",
                    "8900", "9499", "20000", "29999", "45000", "48999", "82800", "82999", "95000",
                    "99999",
                ],
            ),
            (
                "988",
                &[
                    "00", "11", "200", "699", "8000", "9699", "12000", "19999", "70000", "79999",
                    "97000", "99999",
                ],
            ),
            (
                "989",
                &[
                    "0", "0", "20", "34", "37", "48", "50", "52", "550", "799", "8000", "9499",
                    "35000", "36999", "49000", "49999", "53000", "54999", "95000", "99999",
                ],
            ),
            (
                "9905",
                &["0", "0", "20", "23", "600", "624", "9900", "9999"],
            ),
            ("9906", &["20", "22", "700", "724", "9900", "9999"]),
            (
                "9907",
                &["0", "0", "50", "64", "800", "874", "9500", "9999"],
            ),
            (
                "9908",
                &["0", "3", "40", "69", "825", "899", "9700", "9999"],
            ),
            ("9909", &["00", "19", "750", "849", "9800", "9999"]),
            (
                "9910",
                &[
                    "01", "18", "225", "374", "550", "799", "5000", "5499", "8000", "9999",
                ],
            ),
            ("9911", &["20", "24", "550", "749", "9500", "9999"]),
            ("9912", &["40", "44", "750", "799", "9800", "9999"]),
            ("9913", &["00", "09", "600", "709", "9500", "9999"]),
            ("9914", &["27", "55", "700", "799", "9200", "9999"]),
            ("9915", &["40", "59", "650", "799", "9300", "9999"]),
            (
                "9916",
                &[
                    "0", "0", "4", "5", "10", "39", "79", "91", "94", "94", "600", "789", "9200",
                    "9399", "9500", "9999",
                ],
            ),
            (
                "9917",
                &["0", "0", "30", "34", "600", "699", "9625", "9999"],
            ),
            (
                "9918",
                &["0", "0", "20", "29", "600", "799", "9500", "9999"],
            ),
            (
                "9919",
                &["0", "0", "20", "29", "500", "599", "9000", "9999"],
            ),
            (
                "9920",
                &[
                    "00", "02", "23", "42", "130", "199", "200", "229", "430", "799", "8300",
                    "8549", "8550", "9999",
                ],
            ),
            (
                "9921",
                &["0", "0", "30", "39", "700", "899", "9700", "9999"],
            ),
            (
                "9922",
                &["20", "29", "600", "799", "5500", "5999", "8000", "9999"],
            ),
            (
                "9923",
                &["0", "0", "10", "69", "700", "899", "9400", "9999"],
            ),
            ("9924", &["28", "39", "500", "659", "8950", "9999"]),
            (
                "9925",
                &["0", "2", "30", "54", "550", "734", "7350", "9999"],
            ),
            (
                "9926",
                &["0", "1", "20", "39", "400", "799", "8000", "9999"],
            ),
            ("9927", &["00", "09", "100", "399", "4000", "4999"]),
            (
                "9928",
                &[
                    "00", "09", "90", "99", "100", "399", "800", "899", "4000", "4999",
                ],
            ),
            (
                "9929",
                &[
                    "0", "3", "40", "54", "550", "799", "980", "999", "8000", "9799",
                ],
            ),
            ("9930", &["00", "49", "500", "939", "9400", "9999"]),
            ("9931", &["00", "23", "240", "899", "9000", "9999"]),
            ("9932", &["00", "39", "400", "849", "8500", "9999"]),
            (
                "9933",
                &[
                    "0", "0", "10", "39", "87", "89", "400", "869", "9000", "9999",
                ],
            ),
            (
                "9934",
                &["0", "0", "10", "49", "500", "799", "8000", "9999"],
            ),
            (
                "9935",
                &["0", "0", "10", "39", "400", "899", "9000", "9999"],
            ),
            (
                "9936",
                &["0", "1", "20", "39", "400", "799", "8000", "9999"],
            ),
            (
                "9937",
                &["0", "2", "30", "49", "500", "799", "8000", "9999"],
            ),
            (
                "9938",
                &[
                    "00", "79", "800", "949", "975", "990", "9500", "9749", "9910", "9999",
                ],
            ),
            (
                "9939",
                &[
                    "0", "3", "40", "47", "50", "79", "98", "99", "480", "499", "800", "899",
                    "960", "979", "9000", "9599",
                ],
            ),
            (
                "9940",
                &[
                    "0", "1", "20", "49", "84", "86", "500", "839", "8700", "9999",
                ],
            ),
            (
                "9941",
                &[
                    "0", "0", "8", "8", "10", "39", "400", "789", "7900", "7999", "9000", "9999",
                ],
            ),
            (
                "9942",
                &[
                    "00", "55", "560", "699", "750", "849", "900", "984", "7000", "7499", "8500",
                    "8999", "9850", "9999",
                ],
            ),
            (
                "9943",
                &["00", "29", "300", "399", "975", "999", "4000", "9749"],
            ),
            (
                "9944",
                &[
                    "60", "69", "80", "89", "100", "499", "700", "799", "900", "999", "0000",
                    "0999", "5000", "5999",
                ],
            ),
            (
                "9945",
                &[
                    "00", "00", "08", "39", "57", "57", "80", "80", "010", "079", "400", "569",
                    "580", "799", "810", "849", "8500", "9999",
                ],
            ),
            (
                "9946",
                &["0", "1", "20", "39", "400", "899", "9000", "9999"],
            ),
            ("9947", &["0", "1", "20", "79", "800", "999"]),
            ("9948", &["00", "39", "400", "849", "8500", "9999"]),
            (
                "9949",
                &[
                    "00", "08", "10", "39", "70", "71", "75", "89", "090", "099", "400", "699",
                    "7200", "7499", "9000", "9999",
                ],
            ),
            ("9950", &["00", "29", "300", "849", "8500", "9999"]),
            (
                "9951",
                &["00", "38", "390", "849", "980", "999", "8500", "9799"],
            ),
            (
                "9952",
                &["0", "0", "15", "39", "400", "799", "8000", "9999"],
            ),
            (
                "9953",
                &[
                    "0", "0", "10", "39", "60", "89", "93", "96", "400", "599", "970", "999",
                    "9000", "9299",
                ],
            ),
            (
                "9954",
                &[
                    "0", "1", "20", "39", "99", "99", "400", "799", "8000", "9899",
                ],
            ),
            ("9955", &["00", "39", "400", "929", "9300", "9999"]),
            (
                "9956",
                &["0", "0", "10", "39", "400", "899", "9000", "9999"],
            ),
            (
                "9957",
                &[
                    "00", "39", "65", "67", "70", "84", "88", "99", "400", "649", "680", "699",
                    "8500", "8799",
                ],
            ),
            (
                "9958",
                &[
                    "00", "01", "10", "18", "20", "49", "020", "029", "040", "089", "500", "899",
                    "0300", "0399", "0900", "0999", "1900", "1999", "9000", "9999",
                ],
            ),
            (
                "9959",
                &[
                    "0", "1", "20", "79", "98", "99", "800", "949", "970", "979", "9500", "9699",
                ],
            ),
            ("9960", &["00", "59", "600", "899", "9000", "9999"]),
            (
                "9961",
                &["0", "2", "30", "69", "700", "949", "9500", "9999"],
            ),
            (
                "9962",
                &[
                    "00", "54", "56", "59", "600", "849", "5500", "5599", "8500", "9999",
                ],
            ),
            (
                "9963",
                &[
                    "0", "1", "30", "54", "250", "279", "550", "734", "2000", "2499", "2800",
                    "2999", "7350", "7499", "7500", "9999",
                ],
            ),
            ("9964", &["0", "6", "70", "94", "950", "999"]),
            ("9965", &["00", "39", "400", "899", "9000", "9999"]),
            (
                "9966",
                &[
                    "14", "14", "20", "69", "000", "139", "750", "820", "825", "825", "829", "959",
                    "1500", "1999", "7000", "7499", "8210", "8249", "8260", "8289", "9600", "9999",
                ],
            ),
            ("9967", &["00", "39", "400", "899", "9000", "9999"]),
            ("9968", &["00", "49", "500", "939", "9400", "9999"]),
            ("9969", &["00", "19", "500", "749", "9300", "9999"]),
            ("9970", &["00", "39", "400", "899", "9000", "9999"]),
            (
                "9971",
                &["0", "5", "60", "89", "900", "989", "9900", "9999"],
            ),
            (
                "9972",
                &[
                    "1", "1", "00", "09", "30", "59", "200", "249", "600", "899", "2500", "2999",
                    "9000", "9999",
                ],
            ),
            (
                "9973",
                &[
                    "00", "05", "10", "69", "060", "089", "700", "969", "0900", "0999", "9700",
                    "9999",
                ],
            ),
            (
                "9974",
                &[
                    "0", "2", "30", "54", "91", "94", "95", "99", "550", "749", "880", "909",
                    "7500", "8799",
                ],
            ),
            (
                "9975",
                &[
                    "0", "0", "45", "89", "100", "299", "900", "949", "3000", "3999", "4000",
                    "4499", "9500", "9999",
                ],
            ),
            (
                "9976",
                &[
                    "0", "4", "59", "89", "580", "589", "900", "989", "5000", "5799", "9900",
                    "9999",
                ],
            ),
            ("9977", &["00", "89", "900", "989", "9900", "9999"]),
            (
                "9978",
                &[
                    "00", "29", "40", "94", "300", "399", "950", "989", "9900", "9999",
                ],
            ),
            (
                "9979",
                &[
                    "0", "4", "50", "64", "66", "75", "650", "659", "760", "899", "9000", "9999",
                ],
            ),
            (
                "9980",
                &["0", "3", "40", "89", "900", "989", "9900", "9999"],
            ),
            (
                "9981",
                &[
                    "00", "09", "20", "79", "100", "159", "800", "949", "1600", "1999", "9500",
                    "9999",
                ],
            ),
            ("9982", &["00", "79", "800", "989", "9900", "9999"]),
            ("9983", &["80", "94", "950", "989", "9900", "9999"]),
            ("9984", &["00", "49", "500", "899", "9000", "9999"]),
            (
                "9985",
                &["0", "4", "50", "79", "800", "899", "9000", "9999"],
            ),
            (
                "9986",
                &[
                    "00", "39", "97", "99", "400", "899", "940", "969", "9000", "9399",
                ],
            ),
            ("9987", &["00", "39", "400", "879", "8800", "9999"]),
            (
                "9988",
                &["0", "3", "40", "54", "550", "749", "7500", "9999"],
            ),
            (
                "9989",
                &[
                    "0", "0", "30", "59", "100", "199", "600", "949", "2000", "2999", "9500",
                    "9999",
                ],
            ),
            ("69990", &["50", "50", "994", "999"]),
            ("99901", &["00", "49", "80", "99", "500", "799"]),
            ("99902", &[]),
            ("99903", &["0", "1", "20", "89", "900", "999"]),
            ("99904", &["0", "5", "60", "89", "900", "999"]),
            ("99905", &["0", "3", "40", "79", "800", "999"]),
            (
                "99906",
                &[
                    "0", "2", "30", "59", "70", "89", "90", "94", "600", "699", "950", "999",
                ],
            ),
            ("99908", &["0", "0", "10", "89", "900", "999"]),
            ("99909", &["0", "3", "40", "94", "950", "999"]),
            ("99910", &["0", "2", "30", "89", "900", "999"]),
            ("99911", &["00", "59", "600", "999"]),
            ("99912", &["0", "3", "60", "89", "400", "599", "900", "999"]),
            ("99913", &["0", "2", "30", "35", "600", "604"]),
            (
                "99914",
                &[
                    "0", "4", "7", "7", "50", "69", "80", "86", "88", "89", "870", "879", "900",
                    "999",
                ],
            ),
            ("99915", &["0", "4", "50", "79", "800", "999"]),
            ("99916", &["0", "2", "30", "69", "700", "999"]),
            ("99917", &["0", "2", "30", "88", "890", "999"]),
            ("99918", &["0", "3", "40", "79", "800", "999"]),
            ("99919", &["0", "2", "40", "79", "300", "399", "800", "999"]),
            ("99920", &["0", "4", "50", "89", "900", "999"]),
            (
                "99921",
                &["0", "1", "8", "8", "20", "69", "90", "99", "700", "799"],
            ),
            ("99922", &["0", "3", "40", "69", "700", "999"]),
            ("99923", &["0", "1", "20", "79", "800", "999"]),
            ("99924", &["0", "1", "20", "79", "800", "999"]),
            (
                "99925",
                &[
                    "0", "0", "3", "3", "10", "19", "40", "79", "200", "299", "800", "999",
                ],
            ),
            (
                "99926",
                &["0", "0", "10", "59", "87", "89", "90", "99", "600", "869"],
            ),
            ("99927", &["0", "2", "30", "59", "600", "999"]),
            ("99928", &["0", "0", "10", "79", "800", "999"]),
            ("99929", &["0", "4", "50", "79", "800", "999"]),
            ("99930", &["0", "4", "50", "79", "800", "999"]),
            ("99931", &["0", "4", "50", "79", "800", "999"]),
            (
                "99932",
                &["0", "0", "7", "7", "10", "59", "80", "99", "600", "699"],
            ),
            ("99933", &["0", "2", "30", "59", "600", "999"]),
            ("99934", &["0", "1", "20", "79", "800", "999"]),
            (
                "99935",
                &["0", "2", "7", "8", "30", "59", "90", "99", "600", "699"],
            ),
            ("99936", &["0", "0", "10", "59", "600", "999"]),
            ("99937", &["0", "1", "20", "59", "600", "999"]),
            ("99938", &["0", "1", "20", "59", "90", "99", "600", "899"]),
            ("99939", &["0", "2", "30", "59", "60", "89", "900", "999"]),
            ("99940", &["0", "0", "10", "69", "700", "999"]),
            ("99941", &["0", "2", "30", "79", "800", "999"]),
            ("99942", &["0", "4", "50", "79", "800", "999"]),
            ("99943", &["0", "2", "30", "59", "600", "999"]),
            ("99944", &["0", "4", "50", "79", "800", "999"]),
            ("99945", &["0", "4", "50", "89", "98", "99", "900", "979"]),
            ("99946", &["0", "2", "30", "59", "600", "999"]),
            ("99947", &["0", "2", "30", "69", "700", "999"]),
            ("99948", &["0", "4", "50", "79", "800", "999"]),
            (
                "99949",
                &["0", "1", "8", "8", "20", "79", "99", "99", "900", "989"],
            ),
            ("99950", &["0", "4", "50", "79", "800", "999"]),
            ("99951", &[]),
            ("99952", &["0", "4", "50", "79", "800", "999"]),
            ("99953", &["0", "2", "30", "79", "94", "99", "800", "939"]),
            ("99954", &["0", "2", "30", "69", "88", "99", "700", "879"]),
            ("99955", &["0", "1", "20", "59", "80", "99", "600", "799"]),
            ("99956", &["00", "59", "86", "99", "600", "859"]),
            ("99957", &["0", "1", "20", "79", "95", "99", "800", "949"]),
            ("99958", &["0", "4", "50", "93", "940", "949", "950", "999"]),
            ("99959", &["0", "2", "30", "59", "600", "999"]),
            ("99960", &["10", "94", "070", "099", "950", "999"]),
            ("99961", &["0", "2", "37", "89", "300", "369", "900", "999"]),
            ("99962", &["0", "4", "50", "79", "800", "999"]),
            ("99963", &["00", "49", "92", "99", "500", "919"]),
            ("99964", &["0", "1", "20", "79", "800", "999"]),
            ("99965", &["0", "2", "36", "62", "300", "359", "630", "999"]),
            (
                "99966",
                &["0", "2", "30", "69", "80", "96", "700", "799", "970", "999"],
            ),
            ("99967", &["0", "0", "10", "59", "600", "999"]),
            ("99968", &["0", "3", "60", "89", "400", "599", "900", "999"]),
            ("99969", &["0", "4", "50", "79", "95", "99", "800", "949"]),
            ("99970", &["0", "4", "50", "89", "900", "999"]),
            ("99971", &["0", "3", "40", "84", "850", "999"]),
            ("99972", &["0", "4", "50", "89", "900", "999"]),
            ("99973", &["0", "3", "40", "79", "800", "999"]),
            (
                "99974",
                &[
                    "0", "0", "10", "25", "40", "63", "65", "79", "260", "399", "640", "649",
                    "800", "999",
                ],
            ),
            ("99975", &["0", "2", "40", "79", "300", "399", "800", "999"]),
            (
                "99976",
                &[
                    "00", "03", "10", "15", "20", "59", "82", "89", "040", "099", "160", "199",
                    "600", "819", "900", "999",
                ],
            ),
            (
                "99977",
                &[
                    "0", "1", "40", "69", "700", "799", "900", "924", "975", "999",
                ],
            ),
            ("99978", &["0", "4", "50", "69", "700", "999"]),
            ("99979", &["0", "3", "40", "79", "800", "999"]),
            ("99980", &["0", "0", "25", "64", "670", "999"]),
            (
                "99981",
                &[
                    "0", "0", "10", "10", "15", "19", "22", "74", "110", "149", "200", "219",
                    "750", "999",
                ],
            ),
            ("99982", &["0", "4", "50", "79", "845", "999"]),
            ("99983", &["0", "0", "35", "69", "850", "999"]),
            ("99984", &["0", "0", "50", "69", "950", "999"]),
            ("99985", &["0", "1", "23", "79", "200", "229", "800", "999"]),
            ("99986", &["0", "0", "50", "69", "950", "999"]),
            ("99987", &["400", "999"]),
            ("99988", &["0", "0", "10", "10", "50", "54", "800", "824"]),
            ("99989", &["0", "1", "50", "79", "900", "999"]),
            ("99990", &["0", "1", "45", "57", "930", "999"]),
            ("99991", &["0", "0", "50", "60", "960", "999"]),
            ("99992", &["0", "2", "50", "69", "900", "999"]),
            ("99993", &["0", "4", "50", "54", "980", "999"]),
            ("99994", &["0", "0", "50", "56", "960", "999"]),
            ("99995", &["50", "55", "975", "999"]),
            ("99996", &["0", "1", "40", "59", "900", "999"]),
            ("99997", &["0", "0", "40", "61", "920", "999"]),
            ("99998", &["80", "89"]),
        ],
    ),
    (
        "979",
        &[
            (
                "8",
                &[
                    "200", "239", "1800", "1949", "1950", "1999", "2400", "2599", "2600", "2799",
                    "2800", "2999", "3000", "8849", "88500", "89999", "90000", "90999", "950000",
                    "969999", "9850000", "9929999", "9930000", "9959999", "9960000", "9984999",
                    "9985000", "9999999",
                ],
            ),
            (
                "10",
                &[
                    "00", "19", "200", "699", "7000", "8999", "90000", "97599", "976000", "999999",
                ],
            ),
            (
                "11",
                &[
                    "00", "21", "250", "549", "5500", "8499", "23000", "24999", "85000", "94999",
                    "220000", "229999", "950000", "999999",
                ],
            ),
            (
                "12",
                &[
                    "200", "299", "5450", "5999", "80000", "84999", "985000", "999999",
                ],
            ),
            (
                "13",
                &[
                    "00", "00", "600", "604", "7000", "7349", "87500", "89999", "990000", "999999",
                ],
            ),
        ],
    ),
];

fn ranges(prefix: &str) -> Option<&'static [(&'static str, &'static [&'static str])]> {
    RANGES.iter().find(|(p, _)| *p == prefix).map(|(_, g)| *g)
}

/// `wcHyphenateISBN(isbn)` (:345-424): the ISBN hyphenated by the ranges
/// table, `None` (upstream '') when it cannot be.
pub fn wc_hyphenate_isbn(isbn: &str) -> Option<String> {
    let c: Vec<char> = isbn.chars().collect();
    let len = c.len();
    let mut parts: Vec<String> = Vec::new();
    let mut i;
    let ucc: String;
    if len == 10 {
        ucc = "978".to_owned();
        i = 0;
    } else {
        ucc = c.iter().take(3).collect();
        ranges(&ucc)?;
        parts.push(ucc.clone());
        i = 3;
    }
    let groups = ranges(&ucc)?;
    let mut group = String::new();
    let mut reg_ranges: Option<&[&str]> = None;
    while i + 3 < len {
        group.push(c[i]);
        if let Some((_, r)) = groups.iter().find(|(g, _)| *g == group) {
            parts.push(group.clone());
            reg_ranges = Some(r);
            break;
        }
        i += 1;
    }
    let reg_ranges = reg_ranges?;
    let mut registrant = String::new();
    let mut found = false;
    i += 1;
    while !found && i + 2 < len {
        registrant.push(c[i]);
        let mut j = 0;
        while j < reg_ranges.len() && registrant.len() >= reg_ranges[j].len() {
            if registrant.len() == reg_ranges[j].len()
                && registrant.as_str() >= reg_ranges[j]
                && reg_ranges
                    .get(j + 1)
                    .is_some_and(|hi| registrant.as_str() <= *hi)
            {
                parts.push(registrant.clone());
                found = true;
                break;
            }
            j += 2;
        }
        i += 1;
    }
    if !found {
        return None;
    }
    parts.push(c[i.min(len - 1)..len - 1].iter().collect());
    parts.push(c[len - 1].to_string());
    Some(parts.join("-"))
}

/// `MODES` (:426-436): a mode digit's split.
enum Mode {
    Slice(&'static [(i64, Option<i64>)], &'static [(i64, Option<i64>)]),
    SplitInterlaced(bool),
}

fn mode(index: char) -> Option<Mode> {
    Some(match index {
        '1' => Mode::Slice(&[(0, Some(64))], &[(64, None)]),
        '2' => Mode::Slice(&[(-64, None)], &[(0, Some(-64))]),
        '3' => Mode::Slice(&[(0, Some(32)), (-32, None)], &[(32, Some(-32))]),
        '4' => Mode::SplitInterlaced(true),
        '5' => Mode::SplitInterlaced(false),
        '6' => Mode::Slice(&[(0, Some(22)), (-42, None)], &[(22, Some(-42))]),
        '7' => Mode::Slice(&[(0, Some(16)), (-48, None)], &[(16, Some(-48))]),
        '8' => Mode::Slice(&[(0, Some(48)), (-16, None)], &[(48, Some(-16))]),
        '9' => Mode::Slice(&[(0, Some(42)), (-22, None)], &[(42, Some(-22))]),
        _ => return None,
    })
}

/// `String.prototype.slice(start, end)` on UTF-16 units (here ASCII hex).
fn js_slice(s: &[char], start: i64, end: Option<i64>) -> String {
    let len = s.len() as i64;
    let norm = |x: i64| if x < 0 { (len + x).max(0) } else { x.min(len) };
    let a = norm(start);
    let b = end.map_or(len, norm);
    if b <= a {
        return String::new();
    }
    s[a as usize..b as usize].iter().collect()
}

/// `split(input, mode)` (:476-485): (secretKey, encryptedData).
pub fn split(input: &str, mode_str: &str) -> Option<(String, String)> {
    let m: Vec<char> = mode_str.chars().collect();
    let flip = m.get(1) == Some(&'1');
    let mut chars: Vec<char> = input.chars().collect();
    if flip {
        chars.reverse();
    }
    match mode(*m.get(2)?)? {
        Mode::Slice(key, data) => {
            let k: String = key.iter().map(|(a, b)| js_slice(&chars, *a, *b)).collect();
            let d: String = data.iter().map(|(a, b)| js_slice(&chars, *a, *b)).collect();
            Some((k, d))
        }
        Mode::SplitInterlaced(even) => {
            let mut k = String::new();
            let mut d = String::new();
            for (i, ch) in chars.iter().enumerate() {
                let is_key = i < 128 && i % 2 == usize::from(!even);
                if is_key {
                    k.push(*ch);
                } else {
                    d.push(*ch);
                }
            }
            Some((k, d))
        }
    }
}

/// `decryptResponse(json)` (:528-533): a plain answer as it is; an
/// encrypted one (`p`, `x`, `l`) fails, AES-GCM not being ported.
fn decrypt_response(json: Value) -> Result<Value, SearchError> {
    let has = |k: &str| json.as_object().is_some_and(|o| o.contains_key(k));
    if has("p") && has("x") && has("l") {
        return Err(SearchError::Translator(
            "Open WorldCat: encrypted response (AES-GCM) is not supported by this port".to_owned(),
        ));
    }
    Ok(json)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Expected values from upstream's own `wcHyphenateISBN`, run with Node
    /// on vendor/translators/Open WorldCat.js (2026-10-07).
    #[test]
    fn hyphenates_like_upstream() {
        let cases = [
            ("9780585030159", Some("978-0-585-03015-9")),
            ("0838985890", Some("0-8389-8589-0")),
            ("9798218450144", Some("979-8-218-45014-4")),
            ("9791234567896", None),
            ("9786110000001", None),
            ("0000000000", Some("0-00-000000-0")),
        ];
        for (i, want) in cases {
            assert_eq!(wc_hyphenate_isbn(i).as_deref(), want, "{i}");
        }
    }

    #[test]
    fn split_modes() {
        let input: String = (0..100)
            .map(|i| char::from(b'a' + (i % 26) as u8))
            .collect();
        let (k, d) = split(&input, "x01").unwrap();
        assert_eq!(k.len(), 64);
        assert_eq!(d.len(), 36);
        let (k, d) = split(&input, "x04").unwrap();
        assert_eq!(k.len(), 50);
        assert_eq!(d.len(), 50);
    }
}
