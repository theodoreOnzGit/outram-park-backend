// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "EIDR.js" (translatorID
//   79c3d292-0afc-42a1-bd86-7e706fc35aa5, lastUpdated 2017-06-03 11:41:00):
//   `typeMap` :14-26, `creatorMap` :28-31, `checkEIDR` :33-49, `getValue`
//   :51-55, `detectSearch` :57-73, `doSearch` :75-142.
// Copyright (c) Aurimas Vinckevicius.
// Licence: AGPL-3.0 (no licence text upstream; treated as AGPLv3 as part
//   of Zotero, maintainer decision 2026-10-07, #747).

//! The EIDR search translator: an EIDR content record (a DOI under
//! 10.5240) resolved at resolve.eidr.org, read as a film, TV broadcast or
//! video recording.
//!
//! One deliberate difference: upstream's credit loop (`while (c) { t =
//! creatorMap[c.nodeName]; if (!t) continue; ... }`) never advances past a
//! credit it has no creator type for, so upstream hangs on one. The port
//! fails the translation there instead (kovan never hangs).

use crate::zotero::framework::item::{JsObject, TranslatorCreator, TranslatorItem};
use crate::zotero::framework::js;
use crate::zotero::framework::options::TranslatorMetadata;
use crate::zotero::framework::utilities::clean_author;
use crate::zotero::framework::xml::{NodeId, XNode, XmlDocument};
use crate::zotero::framework::xpath::xpath_text;
use crate::zotero::search::{SearchContext, SearchError};
use serde_json::Value;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "79c3d292-0afc-42a1-bd86-7e706fc35aa5",
    label: "EIDR",
    creator: "Aurimas Vinckevicius",
    target: "",
    min_version: "1.0",
    priority: 80,
    translator_type: 8,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2017-06-03 11:41:00",
};

/// `typeMap` (:14-26).
fn type_map(t: &str) -> Option<&'static str> {
    match t {
        "TV" => Some("tvBroadcast"),
        "Movie" => Some("film"),
        "Short" | "Web" => Some("videoRecording"),
        _ => None,
    }
}

/// `creatorMap` (:28-31).
fn creator_map(t: &str) -> Option<&'static str> {
    match t {
        "Director" => Some("director"),
        "Actor" => Some("castMember"),
        _ => None,
    }
}

fn type_error(m: &str) -> SearchError {
    SearchError::Translator(format!("TypeError: {m}"))
}

/// `checkEIDR(eidr)` (:33-49): the first match of
/// `/10.5240\/((?:[0-9A-F]{4}-){5})([0-9A-Z])/i` with a valid ISO 7064
/// Mod 37,36 check character.
fn check_eidr(eidr: &str) -> bool {
    let c: Vec<char> = js::trim(eidr).chars().collect();
    let hex = |ch: char| ch.is_ascii_hexdigit();
    let line_terminator = |ch: char| matches!(ch, '\n' | '\r' | '\u{2028}' | '\u{2029}');
    let mut found: Option<(String, char)> = None;
    for p in 0..c.len() {
        // "10" . "5240/" — the `.` is any character but a line terminator.
        if p + 8 + 25 + 1 > c.len() {
            break;
        }
        if c[p] != '1' || c[p + 1] != '0' || line_terminator(c[p + 2]) {
            continue;
        }
        if c[p + 3..p + 8].iter().collect::<String>() != "5240/" {
            continue;
        }
        let groups = &c[p + 8..p + 33];
        let ok = (0..5).all(|g| {
            groups[g * 5..g * 5 + 4].iter().all(|&ch| hex(ch)) && groups[g * 5 + 4] == '-'
        });
        let last = c[p + 33];
        if ok && last.is_ascii_alphanumeric() {
            found = Some((groups.iter().collect(), last));
            break;
        }
    }
    let Some((suffix1, suffix2)) = found else {
        return false;
    };
    let mut sum: i64 = 0;
    for ch in suffix1.chars().filter(|&ch| ch != '-') {
        let ch = ch.to_ascii_uppercase();
        sum += "0123456789ABCDEF".find(ch).map_or(-1, |i| i as i64);
        let r = sum % 36;
        sum = ((if r == 0 { 36 } else { r }) * 2) % 37;
    }
    // indexOf on the check character as matched (a lower-case letter is -1).
    sum += "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ"
        .find(suffix2)
        .map_or(-1, |i| i as i64);
    sum % 36 == 1
}

/// `getValue(parentNode, node)` (:51-55): the first descendant's
/// `textContent`, `None` for undefined.
fn get_value(doc: &XmlDocument, parent: NodeId, node: &str) -> Option<String> {
    doc.get_elements_by_tag_name(parent, node)
        .first()
        .map(|&n| doc.text(n))
}

/// `detectSearch` (:57-73).
pub fn detect_search(search: &JsObject) -> bool {
    let Some(doi) = search.get("DOI").filter(|v| js::truthy(Some(v))) else {
        return false;
    };
    // `item.DOI.split('/')`: a non-string throws, so detection fails.
    let Value::String(doi) = doi else {
        return false;
    };
    let prefix = doi.split('/').next().unwrap_or("");
    ["10.5237", "10.5238", "10.5239", "10.5240"].contains(&prefix)
}

/// `doSearch` (:75-142).
pub fn do_search(ctx: &mut SearchContext, search: &JsObject) -> Result<(), SearchError> {
    if !search.truthy("DOI") {
        return Err(SearchError::Translator("EIDR not specified.".to_owned()));
    }
    let doi = search.get("DOI").map(js::to_js_string).unwrap_or_default();
    if !check_eidr(&doi) {
        return Err(SearchError::Translator(format!(
            "EIDR not supported: {doi}"
        )));
    }
    let request = format!("https://resolve.eidr.org/EIDR/object/{doi}/?type=Full&followAlias=true");
    let text = ctx.do_get(&request)?;
    let res = XmlDocument::parse_from_string(&text);
    let root = res.document();
    let ns: &[(&str, &str)] = &[
        ("n", "http://www.eidr.org/schema"),
        ("md", "http://www.movielabs.com/schema/md/v2.1/md"),
    ];
    if !res.get_elements_by_tag_name(root, "Response").is_empty() {
        let v = |n: &str| get_value(&res, root, n).unwrap_or_else(|| "undefined".to_owned());
        return Err(SearchError::Translator(format!(
            "Server returned error: ({}) {}",
            v("Code"),
            v("Type")
        )));
    }
    let base = *res
        .get_elements_by_tag_name(root, "BaseObjectData")
        .first()
        .ok_or_else(|| {
            type_error("Cannot read properties of undefined (reading 'getElementsByTagName')")
        })?;
    let referent = get_value(&res, base, "ReferentType");
    let Some(ty) = referent.as_deref().and_then(type_map) else {
        // "Unhandled ReferentType": no item.
        return Ok(());
    };
    let mut item = TranslatorItem::new(ty);
    let opt = |v: Option<String>| v.map_or(Value::Null, Value::String);
    item.set("title", opt(get_value(&res, base, "ResourceName")));
    item.set(
        "language",
        opt(xpath_text(
            &res,
            XNode::from(base),
            "./n:PrimaryLanguage/n:Language",
            ns,
            None,
        )?),
    );
    item.set("date", opt(get_value(&res, base, "ReleaseDate")));
    item.set("place", opt(get_value(&res, base, "CountryOfOrigin")));

    let length = get_value(&res, base, "ApproximateLength")
        .ok_or_else(|| type_error("Cannot read properties of undefined (reading 'match')"))?;
    item.set("runningTime", running_time(&length)?);

    if let Some(&credits) = res.get_elements_by_tag_name(base, "Credits").first() {
        let mut c = res.first_child(credits);
        while let Some(n) = c {
            let Some(t) = creator_map(&res.node_name(XNode::from(n))) else {
                return Err(SearchError::Translator(format!(
                    "EIDR: credit <{}> has no creator type (upstream loops forever here)",
                    res.node_name(XNode::from(n))
                )));
            };
            let name = get_value(&res, n, "md:DisplayName").ok_or_else(|| {
                SearchError::Translator("cleanAuthor: author must be a string".to_owned())
            })?;
            let a = clean_author(&name, t, false);
            item.creators.push(TranslatorCreator {
                first_name: a.first_name,
                last_name: Some(a.last_name),
                creator_type: Some(a.creator_type),
                ..Default::default()
            });
            c = res.next_sibling(n);
        }
    }
    ctx.complete(item)
}

/// The running time (:104-109): `match(/PT(?:(\d+)H)?(?:(\d+)M)?(?:(\d+)S)?/i)`
/// at its first `PT`, then `(h||0)*3600 + (m||0)*60 + (s||0) + 's'`. When
/// the seconds group matched it is a string, so JavaScript concatenates it
/// rather than adding it; kept.
fn running_time(s: &str) -> Result<String, SearchError> {
    let c: Vec<char> = s.chars().collect();
    let start = (0..c.len().saturating_sub(1))
        .find(|&i| c[i].eq_ignore_ascii_case(&'P') && c[i + 1].eq_ignore_ascii_case(&'T'))
        .ok_or_else(|| type_error("Cannot read properties of null (reading '1')"))?;
    let mut i = start + 2;
    let group = |unit: char, i: &mut usize| -> Option<String> {
        let mut j = *i;
        while j < c.len() && c[j].is_ascii_digit() {
            j += 1;
        }
        if j > *i && j < c.len() && c[j].eq_ignore_ascii_case(&unit) {
            let d: String = c[*i..j].iter().collect();
            *i = j + 1;
            Some(d)
        } else {
            None
        }
    };
    let h = group('H', &mut i);
    let m = group('M', &mut i);
    let sec = group('S', &mut i);
    let num =
        |v: &Option<String>| -> f64 { v.as_deref().map_or(0.0, |d| d.parse().unwrap_or(f64::NAN)) };
    let base = num(&h) * 3600.0 + num(&m) * 60.0;
    let base = js_number(base);
    Ok(match sec {
        Some(sec) => format!("{base}{sec}s"),
        None => format!("{base}s"),
    })
}

/// A JavaScript number's `ToString` for the integers this produces.
fn js_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e21 {
        format!("{}", n as i128)
    } else {
        crate::zotero::framework::xpath::js_number_to_string(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_and_lengths_like_upstream() {
        assert!(check_eidr("10.5240/6F7E-EF59-329B-1F0A-8440-2"));
        assert!(!check_eidr("10.5240/6F7E-EF59-329B-1F0A-8440-3"));
        assert_eq!(running_time("PT21M").unwrap(), "1260s");
        assert_eq!(running_time("PT1H30M15S").unwrap(), "540015s");
        assert!(running_time("21 minutes").is_err());
    }
}
