// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): BibTeX.js `parseExtraFields` :177-191,
//   `extraFieldsToString` :193-204, `cleanFilePath` :1196-1199,
//   `tidyAccents` :1214-1239, `citeKeyTitleBannedRe`/`citeKeyConversions`
//   :1241-1272, `buildCiteKey` :1275-1338; and their copies in BibLaTeX.js
//   (:101-129, :340-476, :486-490), which are the same code (BibLaTeX reads
//   its own hidden preference, `BibLaTeX.export.simpleCitekey`).
// Copyright (C) 2019 CHNM, Simon Kornblith, Richard Karnesky, Emiliano
//   heyns and Anders Johansson.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! What the BibTeX and BibLaTeX exporters share: Extra parsing, citation
//! keys, file-path cleaning.

use crate::zotero::framework::js;
use crate::zotero::framework::utilities::remove_diacritics;
use crate::zotero::framework::{ExportContext, TranslatorItem};
use kovan_common::zotero::date::str_to_date;
use regex::Regex;
use std::sync::OnceLock;

fn re(cell: &'static OnceLock<Regex>, pattern: impl FnOnce() -> String) -> &'static Regex {
    cell.get_or_init(|| Regex::new(&pattern()).expect("static regex compiles"))
}

/// One line of Extra as `parseExtraFields` reads it: `{raw}` plus
/// `{field, value}` when the line has a `:` after its second character.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExtraRec {
    /// `raw`.
    pub raw: String,
    /// `field`.
    pub field: Option<String>,
    /// `value`.
    pub value: Option<String>,
}

/// `parseExtraFields(extra)` (BibTeX.js:177-191).
pub(crate) fn parse_extra_fields(extra: &str) -> Vec<ExtraRec> {
    static NL: OnceLock<Regex> = OnceLock::new();
    re(&NL, || "[\r\n]+".into())
        .split(extra)
        .map(|line| {
            let mut rec = ExtraRec {
                raw: line.to_owned(),
                field: None,
                value: None,
            };
            let t = js::trim(line);
            let tc: Vec<char> = t.chars().collect();
            if let Some(at) = tc.iter().position(|c| *c == ':').filter(|at| *at > 1) {
                rec.field = Some(js::trim(&tc[..at].iter().collect::<String>()).to_owned());
                rec.value = Some(js::trim(&tc[at + 1..].iter().collect::<String>()).to_owned());
            }
            rec
        })
        .collect()
}

/// `extraFieldsToString(extra)` (BibTeX.js:193-204): an empty `raw` (falsy)
/// prints `field: value`, which for a line without a colon is
/// "undefined: undefined", as upstream.
pub(crate) fn extra_recs_to_string(extra: &[ExtraRec]) -> String {
    let mut s = String::new();
    for e in extra {
        s.push('\n');
        if e.raw.is_empty() {
            s.push_str(e.field.as_deref().unwrap_or("undefined"));
            s.push_str(": ");
            s.push_str(e.value.as_deref().unwrap_or("undefined"));
        } else {
            s.push_str(&e.raw);
        }
    }
    s.chars().skip(1).collect()
}

/// `cleanFilePath(str)` (BibTeX.js:1196-1199).
pub(crate) fn clean_file_path(s: Option<&str>) -> String {
    static R: OnceLock<Regex> = OnceLock::new();
    match s.filter(|s| !s.is_empty()) {
        None => String::new(),
        Some(s) => re(&R, || format!("(?:{ws}*[{{}}]+)+{ws}*", ws = js::WS))
            .replace_all(s, " ")
            .into_owned(),
    }
}

/// `tidyAccents(s)` (BibTeX.js:1214-1239), on Zotero 3+ (where
/// `ZU.removeDiacritics` exists).
fn tidy_accents(s: &str) -> String {
    remove_diacritics(&s.to_lowercase(), true)
}

/// `citeKeyConversions` %t (BibTeX.js:1257-1262): the title lower-cased,
/// banned words and markup removed, first whitespace-separated word.
fn first_title_word(title: &str) -> String {
    static BANNED: OnceLock<Regex> = OnceLock::new();
    static WSR: OnceLock<Regex> = OnceLock::new();
    let banned = re(&BANNED, || {
        format!(
            "(?-u:\\b)(a|an|the|some|from|on|in|to|of|do|with|der|die|das|ein|eine|einer|eines|einem|einen|un|une|la|le|l'|les|el|las|los|al|uno|una|unos|unas|de|des|del|d')({ws}+|(?-u:\\b))|(</?(i|b|sup|sub|sc|span style=\"small-caps\"|span)>)",
            ws = js::WS
        )
    });
    let t = banned.replace_all(&title.to_lowercase(), "").into_owned();
    re(&WSR, || format!("{}+", js::WS))
        .split(&t)
        .next()
        .unwrap_or("")
        .to_owned()
}

/// Property names every JavaScript object has through its prototype, which
/// `citekeys[citekey]` (a plain object) finds truthy.
const OBJECT_PROTOTYPE: [&str; 12] = [
    "constructor",
    "hasOwnProperty",
    "isPrototypeOf",
    "propertyIsEnumerable",
    "toLocaleString",
    "toString",
    "valueOf",
    "__proto__",
    "__defineGetter__",
    "__defineSetter__",
    "__lookupGetter__",
    "__lookupSetter__",
];

/// `buildCiteKey(item, extraFields, citekeys)` (BibTeX.js:1275-1338;
/// BibLaTeX.js:413-476). `simple_pref` is the hidden preference that forces
/// the simple key pattern.
pub(crate) fn build_cite_key(
    item: &TranslatorItem,
    extra: Option<&mut Vec<ExtraRec>>,
    citekeys: &mut Vec<String>,
    simple_pref: &str,
    ctx: &ExportContext,
) -> String {
    static NUMBER: OnceLock<Regex> = OnceLock::new();
    static LEGACY: OnceLock<Regex> = OnceLock::new();
    static SIMPLE: OnceLock<Regex> = OnceLock::new();
    if let Some(extra) = extra {
        let pos = extra.iter().position(|f| {
            f.field.as_deref().is_some_and(|x| !x.is_empty())
                && f.value.as_deref().is_some_and(|x| !x.is_empty())
                && f.field.as_deref().unwrap().to_lowercase() == "citation key"
        });
        if let Some(p) = pos {
            return extra.remove(p).value.unwrap();
        }
    }
    if let Some(k) = item.get("citationKey").filter(|k| js::truthy(Some(k))) {
        return js::to_js_string(k);
    }
    // citeKeyFormat "%a_%t_%y" (BibTeX.js:87).
    let a = match item
        .creators
        .first()
        .and_then(|c| c.last_name.as_deref())
        .filter(|l| !l.is_empty())
    {
        Some(l) => l.to_lowercase().replace(' ', "_").replace(',', ""),
        None => "noauthor".to_owned(),
    };
    let t = match item.get("title").filter(|t| js::truthy(Some(t))) {
        Some(t) => first_title_word(&js::to_js_string(t)),
        None => "notitle".to_owned(),
    };
    let mut y = "nodate".to_owned();
    if let Some(d) = item.get_str("date").filter(|d| !d.is_empty()) {
        let date = str_to_date(d, &ctx.options.env.dates);
        if let Some(year) = date.year.filter(|yr| !yr.is_empty()) {
            if re(&NUMBER, || "^[0-9]+".into()).is_match(&year) {
                y = year;
            }
        }
    }
    let basekey = tidy_accents(&format!("{a}_{t}_{y}"));
    let simple = ctx
        .get_hidden_pref(simple_pref)
        .is_some_and(|v| js::truthy(Some(&v)))
        || item.get_str("dateAdded").is_some_and(|d| {
            let head: String = d.chars().take(4).collect();
            // parseInt(head) >= 2020; parseInt reads leading digits.
            let digits: String = js::trim_start(&head)
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            digits.parse::<i64>().is_ok_and(|n| n >= 2020)
        });
    let clean = if simple {
        re(&SIMPLE, || "[^a-z0-9_-]".into())
    } else {
        re(&LEGACY, || r"[^a-z0-9!$&*+\-./:;<>?\[\]^_`|]+".into())
    };
    let basekey = clean.replace_all(&basekey, "").into_owned();
    let mut citekey = basekey.clone();
    let mut i = 0;
    while citekeys.contains(&citekey) || OBJECT_PROTOTYPE.contains(&citekey.as_str()) {
        i += 1;
        citekey = format!("{basekey}-{i}");
    }
    citekeys.push(citekey.clone());
    citekey
}

/// `creatorString` before escaping (BibTeX.js:1440-1446, BibLaTeX.js:700-707):
/// "Last, First", with a "Jr" part (after a comma in the first name) moved
/// before the first name. `None` when there is no last name and no first
/// name (upstream then throws on `undefined.replace`).
pub(crate) fn creator_string(first: Option<&str>, last: Option<&str>) -> Option<String> {
    static SPLIT: OnceLock<Regex> = OnceLock::new();
    match first.filter(|f| !f.is_empty()) {
        Some(f) => {
            let mut fname: Vec<&str> = re(&SPLIT, || format!("{ws}*,!?{ws}*", ws = js::WS))
                .split(f)
                .collect();
            let head = fname.remove(0);
            fname.push(head);
            Some(format!(
                "{}, {}",
                last.unwrap_or("undefined"),
                fname.join(", ")
            ))
        }
        None => last.map(str::to_owned),
    }
}

/// `.replace(/ (and) /gi, ' {$1} ')`.
pub(crate) fn protect_and(s: &str) -> String {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, || "(?i) (and) ".into())
        .replace_all(s, " {${1}} ")
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extra_round_trip_and_title_word() {
        let recs = parse_extra_fields("Citation Key: x\nfoo\n");
        assert_eq!(recs[0].field.as_deref(), Some("Citation Key"));
        assert_eq!(recs.len(), 3);
        assert_eq!(
            extra_recs_to_string(&recs[1..]),
            "foo\nundefined: undefined"
        );
        assert_eq!(first_title_word("The Art of War"), "art");
        assert_eq!(
            creator_string(Some("John, Jr."), Some("Smith")).unwrap(),
            "Smith, Jr., John"
        );
    }
}
