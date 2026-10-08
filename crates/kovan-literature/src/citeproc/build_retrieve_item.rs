// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/build.js (CSL.Engine.prototype.retrieveItem and the item-normalisation
//              helpers it calls). Split from build.rs so input normalisation can
//              be ported apart from style building. Also ported here, because
//              retrieveItem is their only caller: CSL.parseNoteFieldHacks,
//              CSL.extractTitleAndSubtitle, CSL.TITLE_SPLIT and
//              CSL.TITLE_SPLIT_REGEXP (src/load.js), and the test runner's
//              sys.normalizeAbbrevsKey / sys.getAbbreviation behaviour
//              (scripts/csl-intermediate-reference.cjs, itself a copy of the
//              juris-m/citeproc-test-runner)
// Version:     2.4.63, commit 73bc1b44bc7d54d0bfec4e070fd27f5efe024ff9
// Copyright:   (c) 2009-2019 Frank Bennett
// Licence:     AGPL-3.0, taken from upstream's "CPAL-1.0 or AGPL-3.0-or-later"
//              (LICENSE at the commit above; see this crate's NOTICE).
// Modified:    2026-10-08, by the OUTRAM PARK contributors. This file is a
//              Rust translation (port) of the file(s) named above, modified
//              from the original.
// No warranty: this program is distributed in the hope that it will be
//              useful, but WITHOUT ANY WARRANTY; without even the implied
//              warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
//              PURPOSE. See the GNU Affero General Public License.

//! Port of `CSL.Engine.prototype.retrieveItem` (`src/build.js`): the wrapper
//! around `sys.retrieveItem` that normalises an input item (a copy) before
//! the engine uses it: language-name split, `page-first`, note-field hacks,
//! date objects, legal / container ids, `authority` shape for
//! `force_jurisdiction`, short titles, abbreviations, `country`.
//!
//! # Integration points
//!
//! * `registry.refhash` (the cache `retrieveItem` returns from on later
//!   calls) is [`State::item_refhash`], a provisional field in the
//!   `wave1-input` block of `state.rs`; point it at the registry when that
//!   exists.
//! * Abbreviations (`transform.loadAbbreviation` / `sys.getAbbreviation`):
//!   [`abbreviation_lookup`] answers from `state.sys.abbreviations` the way
//!   the CSL test runner's `getAbbreviation` does (jurisdiction fallback
//!   chain, no `@domain` support: `availableAbbrevDomains` is
//!   `NotYetPorted`). It stands in until `util_transform.rs` exists.
//! * [`normalize_abbrevs_key`] is the test runner's `sys.normalizeAbbrevsKey`
//!   (the engine's `sys` has it in the runner and in the intermediate
//!   reference script, not in the site sys; harmless there since the
//!   abbreviation cache is empty).
//! * `CSL.DateParser` (the process-wide singleton upstream) is
//!   `state.fun.dateparser`.
//!
//! # Intermediate-dump sections
//!
//! [`items_section`], [`names_section`], [`numbers_section`] and
//! [`citation_items_section`] produce the `items`, `names`, `numbers` and
//! `citation_items` sections of `scripts/csl-intermediate-reference.cjs`
//! (canonical form: keys sorted, undefined omitted), for
//! `tests/citeproc_intermediate.rs`.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::js::{self, Obj};
use super::state::State;
use super::util_name_particles::parse_particles;
use super::util_names_render::{get_static_order, normalize_name_input, NameInputCtx};
use super::util_number::process_number;
use super::util_static_locator::citation_item_input;
use super::{CslResult, EngineError};

// DUP-CHECK: load.js CSL.NAME_VARIABLES
/// `CSL.NAME_VARIABLES`.
const NAME_VARIABLES: [&str; 28] = [
    "author",
    "chair",
    "collection-editor",
    "compiler",
    "composer",
    "container-author",
    "contributor",
    "curator",
    "director",
    "editor",
    "editor-translator",
    "editorial-director",
    "executive-producer",
    "guest",
    "host",
    "illustrator",
    "interviewer",
    "narrator",
    "organizer",
    "original-author",
    "performer",
    "producer",
    "recipient",
    "reviewed-author",
    "script-writer",
    "series-creator",
    "translator",
    "commenter",
];
// DUP-CHECK: load.js CSL.NUMERIC_VARIABLES
/// `CSL.NUMERIC_VARIABLES`.
const NUMERIC_VARIABLES: [&str; 20] = [
    "call-number",
    "chapter-number",
    "collection-number",
    "division",
    "edition",
    "page",
    "issue",
    "locator",
    "locator-extra",
    "number",
    "number-of-pages",
    "number-of-volumes",
    "part-number",
    "printing-number",
    "section",
    "supplement-number",
    "version",
    "volume",
    "supplement",
    "citation-number",
];
// DUP-CHECK: load.js CSL.DATE_VARIABLES
/// `CSL.DATE_VARIABLES`.
const DATE_VARIABLES: [&str; 10] = [
    "locator-date",
    "issued",
    "event-date",
    "accessed",
    "original-date",
    "publication-date",
    "available-date",
    "submitted",
    "alt-issued",
    "alt-event",
];

fn rx(src: &str) -> Regex {
    Regex::new(src).unwrap_or_else(|e| panic!("invalid static regex {src:?}: {e}"))
}

/// JS `.`: anything but `\n`, `\r`, U+2028, U+2029.
const DOT: &str = "[^\\n\\r\\u{2028}\\u{2029}]";

// DUP-CHECK: load.js CSL.NOTE_FIELDS_REGEXP  /\{:(?:[\-_a-z]+|[A-Z]+):[^\}]+\}/g
static NOTE_FIELDS_RE: LazyLock<Regex> = LazyLock::new(|| rx("\\{:(?:[-_a-z]+|[A-Z]+):[^}]+\\}"));

// DUP-CHECK: load.js CSL.NOTE_FIELD_REGEXP  /^([\-_a-z]+|[A-Z]+):\s*([^\}]+)$/
static NOTE_FIELD_RE: LazyLock<Regex> =
    LazyLock::new(|| rx(&format!("^([-_a-z]+|[A-Z]+):[{ws}]*([^}}]+)$", ws = js::WS)));

// DUP-CHECK: load.js CSL.TITLE_SPLIT_REGEXP
const TITLE_SPLITS: [&str; 7] = [
    "\\.\\s+",
    "\\!\\s+",
    "\\?\\s+",
    "\\s*::*\\s+",
    "\\s*\u{2014}\\s*",
    "\\s+\\-\\s+",
    "\\s*\\-\\-\\-*\\s*",
];

fn title_split_source() -> String {
    js_ws(&TITLE_SPLITS.join("|"))
}

/// Turn the JS `\s` of a pattern source into the JS whitespace class.
fn js_ws(src: &str) -> String {
    src.replace("\\s", &format!("[{}]", js::WS))
}

/// `CSL.TITLE_SPLIT_REGEXP.match` (global): `(…)`.
static TITLE_SPLIT_MATCH: LazyLock<Regex> =
    LazyLock::new(|| rx(&format!("({})", title_split_source())));
/// `CSL.TITLE_SPLIT_REGEXP.matchfirst`: `^(…)`.
static TITLE_SPLIT_MATCHFIRST: LazyLock<Regex> =
    LazyLock::new(|| rx(&format!("^({})", title_split_source())));
/// `CSL.TITLE_SPLIT_REGEXP.split`: `(?:…)`.
static TITLE_SPLIT_SPLIT: LazyLock<Regex> =
    LazyLock::new(|| rx(&format!("(?:{})", title_split_source())));

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

/// `development_extensions[name]` of `state.opt`, `None` when absent.
fn dev_ext<'a>(state: &'a State, name: &str) -> Option<&'a Value> {
    state
        .opt
        .get("development_extensions")
        .and_then(|d| d.get(name))
}

/// Truthiness of `state.opt.development_extensions[name]`.
fn dev_ext_truthy(state: &State, name: &str) -> bool {
    dev_ext(state, name).map(js::truthy).unwrap_or(false)
}

/// `obj[key] = value` where `value` may be JS `undefined` (the key is then
/// dropped from this data model).
fn set_or_remove(obj: &mut Obj, key: &str, value: Option<Value>) {
    match value {
        Some(v) => {
            obj.insert(key.to_string(), v);
        }
        None => {
            obj.remove(key);
        }
    }
}

/// What `JSON.parse(JSON.stringify(v))` does to numbers: an integral float
/// becomes an integer (JS has one number type).
pub(crate) fn canon_numbers(v: &mut Value) {
    match v {
        Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                if n.is_f64() && f.is_finite() && f == f.trunc() && f.abs() < 9.0e15 {
                    *v = Value::from(f as i64);
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(canon_numbers),
        Value::Object(o) => o.values_mut().for_each(canon_numbers),
        _ => {}
    }
}

fn type_error(msg: &str) -> EngineError {
    EngineError::BadInput(msg.to_string())
}

// ---------------------------------------------------------------------------
// The test runner's abbreviation behaviour
// ---------------------------------------------------------------------------

/// `sys.normalizeAbbrevsKey(variable, key)` of the CSL test runner (copied
/// into `scripts/csl-intermediate-reference.cjs`): strip noise words and
/// punctuation from `key` and lower-case it; for `jurisdiction` / `country`
/// just upper-case. `key = None` is the runner's one-argument call
/// (`normalizeAbbrevsKey(Item["container-title"])`), which yields `""`.
pub fn normalize_abbrevs_key(variable: &str, key: Option<&str>) -> String {
    static NOISE: LazyLock<Regex> = LazyLock::new(|| {
        rx("(?i-u:(?:\\b|^)(?:and|et|y|und|l[ae]|the|[ld]')(?:\\b|$))|[\\x21-\\x2C./\\x3A-\\x40\\x5B-\\x60\\\\\\x7B\\x7D-\\x7E]")
    });
    static BAR: LazyLock<Regex> =
        LazyLock::new(|| rx(&format!("[{ws}]*\\x7C[{ws}]*", ws = js::WS)));
    static WS_RUN: LazyLock<Regex> = LazyLock::new(|| rx(&format!("[{}]+", js::WS)));
    let key = match key {
        Some(k) if !k.is_empty() => js::trim(k).to_string(),
        _ => String::new(),
    };
    if variable == "jurisdiction" || variable == "country" {
        return key.to_uppercase();
    }
    let k = NOISE.replace_all(&key, "").into_owned();
    let k = BAR.replace_all(&k, "|").into_owned();
    let k = k.replace('.', " ");
    let k = WS_RUN.replace_all(&k, " ").into_owned();
    js::trim(&k).to_lowercase()
}

/// `transform.loadAbbreviation(jurisdiction, category, orig, lang)` followed
/// by a read of `transform.abbrevs[jurisdiction][category][orig]`, with the
/// test runner's `sys.getAbbreviation`: the first of the jurisdiction's
/// prefixes (most specific first, then `default`) whose cache has a truthy
/// entry for `orig`. `orig = ""` loads nothing. `None` when there is none.
///
/// `availableAbbrevDomains` (jurisdiction `@domain` suffixes) is not
/// ported: an engine with it set gets `NotYetPorted`.
pub fn abbreviation_lookup(
    state: &State,
    jurisdiction: Option<&str>,
    category: &str,
    orig: &str,
) -> CslResult<Option<String>> {
    let jurisdiction = match jurisdiction {
        Some(j) if !j.is_empty() => j,
        _ => "default",
    };
    let country = jurisdiction.split(':').next().unwrap_or("");
    if js::truthy_opt(state.opt.get("availableAbbrevDomains")) && country != "default" {
        return Err(EngineError::NotYetPorted {
            method: "CSL.getAbbrevsDomain",
        });
    }
    if orig.is_empty() {
        return Ok(None);
    }
    let mut chain: Vec<String> = vec!["default".to_string()];
    if jurisdiction != "default" {
        let parts: Vec<&str> = jurisdiction.split(':').collect();
        for i in 1..=parts.len() {
            chain.push(parts[..i].join(":"));
        }
    }
    chain.reverse();
    for j in &chain {
        if let Some(v) = state
            .sys
            .abbreviations
            .get(j)
            .and_then(|c| c.get(category))
            .and_then(|m| m.get(orig))
            .filter(|v| !v.is_empty())
        {
            return Ok(Some(v.clone()));
        }
    }
    Ok(None)
}

// ---------------------------------------------------------------------------
// load.js helpers used only by retrieveItem
// ---------------------------------------------------------------------------

/// `CSL.parseNoteFieldHacks(Item, validFieldsForType, allowDateOverride)`
/// (with `validFieldsForType = false`, the only way `retrieveItem` calls it):
/// lines of the `note` field such as `original-date: 1999` or
/// `author: Smith || John` become item fields, and are removed from the note.
/// `Item.type` and date fields are set through the date parser.
pub fn parse_note_field_hacks(
    state: &State,
    item: &mut Obj,
    allow_date_override: bool,
) -> CslResult<()> {
    let note = match item.get("note") {
        Some(Value::String(s)) => s.clone(),
        _ => return Ok(()),
    };
    let mut lines: Vec<String> = note.split('\n').map(str::to_string).collect();
    // Normalize entries
    for i in 0..lines.len() {
        let line = lines[i].clone();
        let m: Vec<String> = NOTE_FIELDS_RE
            .find_iter(&line)
            .map(|x| x.as_str().to_string())
            .collect();
        if !m.is_empty() {
            let splt = js::split(&NOTE_FIELDS_RE, &line);
            let mut elems: Vec<String> = Vec::new();
            for j in 0..splt.len().saturating_sub(1) {
                elems.push(splt[j].clone());
                elems.push(m.get(j).cloned().unwrap_or_default());
            }
            elems.push(splt.last().cloned().unwrap_or_default());
            let mut j = 1;
            while j < elems.len() {
                // Abort conversions if preceded by unparseable text
                if !js::trim(&elems[j - 1]).is_empty()
                    && (i > 0 || j > 1)
                    && !NOTE_FIELD_RE.is_match(&elems[j - 1])
                {
                    break;
                }
                let inner = js::slice(&elems[j], 2, Some(-1));
                elems[j] = format!("\n{}\n", js::trim(&inner));
                j += 2;
            }
            lines[i] = elems.join("");
        }
    }
    // Resplit
    let joined = lines.join("\n");
    lines = joined.split('\n').map(str::to_string).collect();
    let mut names: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for i in 0..lines.len() {
        let line = lines[i].clone();
        let mm = NOTE_FIELD_RE.captures(&line);
        if js::trim(&line).is_empty() {
            continue;
        }
        let Some(mm) = mm else {
            if i == 0 {
                continue;
            } else {
                break;
            }
        };
        let key = mm.get(1).map(|x| x.as_str()).unwrap_or("").to_string();
        let val = js::trim(mm.get(2).map(|x| x.as_str()).unwrap_or("")).to_string();
        let base_key = key.strip_prefix("alt-").unwrap_or(&key);
        if key == "type" {
            item.insert("type".into(), Value::String(val));
            lines[i] = String::new();
        } else if DATE_VARIABLES.contains(&base_key) {
            if !js::truthy_opt(item.get(&key)) || allow_date_override {
                item.insert(
                    key.clone(),
                    Value::Object(state.fun.dateparser.parse_date_to_array(&val)),
                );
                // `!validFieldsForType` is true: the line is consumed
                lines[i] = String::new();
            }
        } else if !js::truthy_opt(item.get(&key)) {
            if NAME_VARIABLES.contains(&base_key) {
                let lst = js::split(&rx(&format!("[{ws}]*\\|\\|[{ws}]*", ws = js::WS)), &val);
                if lst.len() == 1 {
                    let mut n = Obj::new();
                    n.insert("literal".into(), Value::String(lst[0].clone()));
                    names.entry(key.clone()).or_default().push(Value::Object(n));
                } else if lst.len() == 2 {
                    let mut n = Obj::new();
                    n.insert("family".into(), Value::String(lst[0].clone()));
                    n.insert("given".into(), Value::String(lst[1].clone()));
                    parse_particles(&mut n)?;
                    names.entry(key.clone()).or_default().push(Value::Object(n));
                } else {
                    names.entry(key.clone()).or_default();
                }
            } else {
                item.insert(key.clone(), Value::String(val));
            }
            lines[i] = String::new();
        }
    }
    for (key, list) in names {
        item.insert(key, Value::Array(list));
    }
    item.insert(
        "note".into(),
        Value::String(js::trim(&lines.join("\n")).to_string()),
    );
    Ok(())
}

/// `CSL.TITLE_SPLIT(str)`: split a title at sentence / subtitle breaks into
/// `[main, join, sub, join, sub, ...]`, recombining breaks that follow an
/// upper-case character. A falsy `str` is returned as a one-element list.
pub fn title_split(s: &str) -> Vec<String> {
    if s.is_empty() {
        return vec![s.to_string()];
    }
    let m: Vec<String> = TITLE_SPLIT_MATCH
        .find_iter(s)
        .map(|x| x.as_str().to_string())
        .collect();
    let mut lst = js::split(&TITLE_SPLIT_SPLIT, s);
    let mut i = lst.len() as i64 - 2;
    while i > -1 {
        let iu = i as usize;
        lst[iu] = js::trim(&lst[iu]).to_string();
        let last = js::slice(&lst[iu], -1, None);
        if !lst[iu].is_empty() && last.to_lowercase() != last {
            // recombine
            let joined = format!(
                "{}{}{}",
                lst[iu],
                m.get(iu).cloned().unwrap_or_else(|| "undefined".into()),
                lst[iu + 1]
            );
            lst[iu] = joined;
            lst.remove(iu + 1);
        } else {
            // merge
            lst.insert(
                iu + 1,
                m.get(iu).cloned().unwrap_or_else(|| "undefined".into()),
            );
        }
        i -= 1;
    }
    lst
}

/// A JS object of title parts whose values may be `undefined` (`None`),
/// `false` or strings.
type TitleVals = BTreeMap<String, Option<Value>>;

fn vals_str(vals: &TitleVals, key: &str) -> Option<String> {
    match vals.get(key) {
        Some(Some(Value::String(s))) => Some(s.clone()),
        _ => None,
    }
}

fn vals_truthy(vals: &TitleVals, key: &str) -> bool {
    matches!(vals.get(key), Some(Some(v)) if js::truthy(v))
}

/// `CSL.extractTitleAndSubtitle.call(state, Item, narrowSpaceLocale)`
/// (`main_title_from_short_title`): fill `title-main`, `title-sub`,
/// `title-subjoin` (and `container-` variants with `split_container_title`)
/// from `title` and `title-short`, per language of `Item.multi`.
pub fn extract_title_and_subtitle(
    state: &State,
    item: &mut Obj,
    narrow_space_locale: bool,
) -> CslResult<()> {
    let narrow_space = if narrow_space_locale { "\u{202f}" } else { "" };
    let mut segments = vec![""];
    if dev_ext_truthy(state, "split_container_title") {
        segments.push("container-");
    }
    for seg in segments {
        let t_title = format!("{seg}title");
        let t_short = format!("{seg}title-short");
        let t_main = format!("{seg}title-main");
        let t_sub = format!("{seg}title-sub");
        let t_subjoin = format!("{seg}title-subjoin");
        let mut langs: Vec<Option<String>> = vec![None];
        let has_multi = js::truthy_opt(item.get("multi"));
        if has_multi {
            let keys = item
                .get("multi")
                .and_then(|m| m.get("_keys"))
                .ok_or_else(|| {
                    type_error(&format!(
                        "Cannot read properties of undefined (reading '{t_short}')"
                    ))
                })?;
            if let Some(Value::Object(o)) = keys.get(&t_short) {
                for lang in o.keys() {
                    langs.push(Some(lang.clone()));
                }
            }
        }
        for lang in langs {
            let mut vals: TitleVals = BTreeMap::new();
            if let Some(lang) = &lang {
                let keys = item
                    .get("multi")
                    .and_then(|m| m.get("_keys"))
                    .cloned()
                    .unwrap_or(Value::Null);
                if js::truthy_opt(keys.get(&t_title)) {
                    vals.insert(
                        t_title.clone(),
                        keys.get(&t_title)
                            .and_then(|k| k.get(lang.as_str()))
                            .cloned(),
                    );
                }
                if js::truthy_opt(keys.get(&t_short)) {
                    vals.insert(
                        t_short.clone(),
                        keys.get(&t_short)
                            .and_then(|k| k.get(lang.as_str()))
                            .cloned(),
                    );
                }
            } else {
                vals.insert(t_title.clone(), item.get(&t_title).cloned());
                vals.insert(t_short.clone(), item.get(&t_short).cloned());
            }
            vals.insert(t_main.clone(), vals.get(&t_title).cloned().flatten());
            vals.insert(t_sub.clone(), Some(Value::Bool(false)));
            let short_title = vals_str(&vals, &t_short);
            if vals_truthy(&vals, &t_title) {
                let title = vals_str(&vals, &t_title)
                    .ok_or_else(|| type_error("vals[title.title].toLowerCase is not a function"))?;
                let set = |vals: &mut TitleVals, k: &str, v: String| {
                    vals.insert(k.to_string(), Some(Value::String(v)));
                };
                match short_title.as_deref().filter(|s| !s.is_empty()) {
                    Some(st) if st.to_lowercase() == title.to_lowercase() => {
                        set(&mut vals, &t_main, title.clone());
                        set(&mut vals, &t_subjoin, String::new());
                        set(&mut vals, &t_sub, String::new());
                    }
                    Some(st) => {
                        // check for valid match to shortTitle
                        static TRAIL_Q: LazyLock<Regex> = LazyLock::new(|| rx("[?!]+$"));
                        static LEAD_Q: LazyLock<Regex> = LazyLock::new(|| rx("^[?!]+"));
                        static TRAIL_Q_WS: LazyLock<Regex> =
                            LazyLock::new(|| rx(&format!("[?!]+([{}]*)$", js::WS)));
                        let cut = TRAIL_Q.replace(st, "").into_owned();
                        let tail = js::slice(&title, js::len(&cut) as i64, None);
                        let tail_nolead = LEAD_Q.replace(&tail, "").into_owned();
                        let top = js::trim(&title.replacen(&tail_nolead, "", 1)).to_string();
                        let m = TITLE_SPLIT_MATCHFIRST.captures(&tail);
                        if m.is_some() && top.to_lowercase() == st.to_lowercase() {
                            let m1 = m
                                .as_ref()
                                .and_then(|c| c.get(1))
                                .map(|x| x.as_str())
                                .unwrap_or("");
                            set(&mut vals, &t_main, top.clone());
                            set(
                                &mut vals,
                                &t_subjoin,
                                TRAIL_Q_WS.replace(m1, "${1}").into_owned(),
                            );
                            set(
                                &mut vals,
                                &t_sub,
                                TITLE_SPLIT_MATCHFIRST.replace(&tail, "").into_owned(),
                            );
                            if dev_ext_truthy(state, "force_short_title_casing_alignment") {
                                vals.insert(t_short.clone(), vals.get(&t_main).cloned().flatten());
                            }
                        } else {
                            let split_title = title_split(&title);
                            if split_title.len() == 3 {
                                set(&mut vals, &t_main, split_title[0].clone());
                                set(&mut vals, &t_subjoin, split_title[1].clone());
                                set(&mut vals, &t_sub, split_title[2].clone());
                            } else {
                                set(&mut vals, &t_main, title.clone());
                                set(&mut vals, &t_subjoin, String::new());
                                set(&mut vals, &t_sub, String::new());
                            }
                        }
                    }
                    None => {
                        let split_title = title_split(&title);
                        if split_title.len() == 3 {
                            set(&mut vals, &t_main, split_title[0].clone());
                            set(&mut vals, &t_subjoin, split_title[1].clone());
                            set(&mut vals, &t_sub, split_title[2].clone());
                            if dev_ext_truthy(state, "implicit_short_title")
                                && item.get("type").and_then(Value::as_str) != Some("legal_case")
                            {
                                static MAIN_PUNCT_ONLY: LazyLock<Regex> =
                                    LazyLock::new(|| rx("^[-.\\[0-9]+$"));
                                let main = vals_str(&vals, &t_main).unwrap_or_default();
                                if !js::truthy_opt(item.get(&t_short))
                                    && !MAIN_PUNCT_ONLY.is_match(&main)
                                {
                                    let mut punct =
                                        js::trim(&vals_str(&vals, &t_subjoin).unwrap_or_default())
                                            .to_string();
                                    if punct != "?" && punct != "!" {
                                        punct = String::new();
                                    }
                                    set(&mut vals, &t_short, format!("{main}{punct}"));
                                }
                            }
                        } else {
                            set(&mut vals, &t_main, title.clone());
                            set(&mut vals, &t_subjoin, String::new());
                            set(&mut vals, &t_sub, String::new());
                        }
                    }
                }
                if vals_truthy(&vals, &t_subjoin) {
                    let subjoin = vals_str(&vals, &t_subjoin).unwrap_or_default();
                    static QE: LazyLock<Regex> = LazyLock::new(|| rx("([?!])"));
                    static TRAIL_WS: LazyLock<Regex> =
                        LazyLock::new(|| rx(&format!("([{}]*)$", js::WS)));
                    if QE.is_match(&subjoin) {
                        let m1 = TRAIL_WS
                            .captures(&subjoin)
                            .and_then(|c| c.get(1))
                            .map(|x| x.as_str())
                            .unwrap_or("")
                            .to_string();
                        let main = vals_str(&vals, &t_main).unwrap_or_default();
                        vals.insert(
                            t_main.clone(),
                            Some(Value::String(format!(
                                "{main}{narrow_space}{}",
                                js::trim(&subjoin)
                            ))),
                        );
                        vals.insert(t_subjoin.clone(), Some(Value::String(m1)));
                    }
                }
            }
            if vals_truthy(&vals, &t_subjoin) {
                let mut subjoin = vals_str(&vals, &t_subjoin).unwrap_or_default();
                if subjoin.contains(':') {
                    subjoin = format!("{narrow_space}: ");
                }
                if subjoin.contains('-') || subjoin.contains('\u{2014}') {
                    subjoin = "\u{2014}".to_string();
                }
                vals.insert(t_subjoin.clone(), Some(Value::String(subjoin)));
            }
            if let Some(lang) = &lang {
                for (key, v) in &vals {
                    let multi = item
                        .get_mut("multi")
                        .and_then(Value::as_object_mut)
                        .ok_or_else(|| type_error("Item.multi is undefined"))?;
                    let keys = multi
                        .get_mut("_keys")
                        .and_then(Value::as_object_mut)
                        .ok_or_else(|| type_error("Item.multi._keys is undefined"))?;
                    let slot = keys
                        .entry(key.clone())
                        .or_insert_with(|| Value::Object(Obj::new()));
                    if !js::truthy(slot) {
                        *slot = Value::Object(Obj::new());
                    }
                    if let Some(o) = slot.as_object_mut() {
                        set_or_remove(o, lang, v.clone());
                    }
                }
            } else {
                for (key, v) in &vals {
                    set_or_remove(item, key, v.clone());
                }
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// retrieveItem
// ---------------------------------------------------------------------------

/// `date_obj` as the object `dateParseArray` iterates with `for…in`: an
/// object as is, a string or array by index, anything else empty.
fn date_obj_for_parse(v: &Value) -> Obj {
    match v {
        Value::Object(o) => o.clone(),
        Value::String(s) => s
            .chars()
            .enumerate()
            .map(|(i, c)| (i.to_string(), Value::String(c.to_string())))
            .collect(),
        Value::Array(a) => a
            .iter()
            .enumerate()
            .map(|(i, x)| (i.to_string(), x.clone()))
            .collect(),
        _ => Obj::new(),
    }
}

/// `CSL.Engine.prototype.retrieveItem(id)`: the normalised item for `id`,
/// loaded from `state.sys` the first time and returned from the cache
/// (`registry.refhash`) afterwards.
///
/// Normalisations, in upstream's order: lower-case language keys
/// (`normalize_lang_keys_to_lowercase`), split `language` at `<` / `>` into
/// `language-name` / `language-name-original`, `page-first`, note-field
/// hacks (`field_hack`), date objects via the date parser
/// (`raw_date_parsing`) and `dateParseArray`, `legislation_id`
/// (`consolidate_legal_items`), `container_id` (`track_container_items`),
/// `authority` as a name list (`force_jurisdiction`), `title-short` /
/// `container-title-short` (also from abbreviations),
/// `main_title_from_short_title`, a default `jurisdiction` for legal types,
/// `country`.
///
/// Returns the item as a new JSON object. Errors where upstream throws (an
/// unknown id is `JSON.parse(undefined)`).
pub fn retrieve_item(state: &mut State, id: &str) -> CslResult<Value> {
    if !state.tmp.loaded_item_ids.get(id).copied().unwrap_or(false) {
        state.tmp.loaded_item_ids.insert(id.to_string(), true);
    } else {
        return Ok(state.item_refhash.get(id).cloned().unwrap_or(Value::Null));
    }

    if matches!(
        dev_ext(state, "normalize_lang_keys_to_lowercase"),
        Some(Value::Bool(true))
    ) {
        // This is a hack. Should properly be configured by a processor method after build.
        for key in ["default-locale", "locale-translit", "locale-translat"] {
            match state.opt.get_mut(key) {
                Some(Value::Array(a)) => {
                    for x in a.iter_mut() {
                        let l = js::to_js_string(x).to_lowercase();
                        *x = Value::String(l);
                    }
                }
                _ => {
                    return Err(type_error(&format!(
                        "Cannot read properties of undefined (reading 'length') [opt.{key}]"
                    )))
                }
            }
        }
        if let Some(Value::Object(d)) = state.opt.get_mut("development_extensions") {
            d.insert("normalize_lang_keys_to_lowercase".into(), Value::from(100));
        }
    }

    // Item = JSON.parse(JSON.stringify(this.sys.retrieveItem("" + id)));
    let mut item: Obj = match state.sys.items.get(id) {
        Some(Value::Object(o)) => o.clone(),
        _ => return Err(type_error("\"undefined\" is not valid JSON")),
    };
    item.values_mut().for_each(canon_numbers);

    // Optionally normalize keys to lowercase()
    if dev_ext_truthy(state, "normalize_lang_keys_to_lowercase")
        && js::truthy_opt(item.get("multi"))
    {
        if let Some(Value::Object(multi)) = item.get_mut("multi") {
            if let Some(Value::Object(keys)) = multi.get_mut("_keys") {
                for (_field, per_field) in keys.iter_mut() {
                    if let Value::Object(m) = per_field {
                        let ks: Vec<String> = m.keys().cloned().collect();
                        for key in ks {
                            let lk = key.to_lowercase();
                            if key != lk {
                                if let Some(v) = m.remove(&key) {
                                    m.insert(lk, v);
                                }
                            }
                        }
                    }
                }
            }
            if let Some(Value::Object(main)) = multi.get_mut("main") {
                for (_f, v) in main.iter_mut() {
                    match v {
                        Value::String(s) => *s = s.to_lowercase(),
                        _ => {
                            return Err(type_error(
                                "Item.multi.main[field].toLowerCase is not a function",
                            ))
                        }
                    }
                }
            }
        }
        // (the loop over NAME_VARIABLES upstream tests `i > ilen`: never runs)
    }

    // Normalize language field into "language" and "language-original"
    if js::truthy_opt(item.get("language")) {
        let language = match item.get("language") {
            Some(Value::String(s)) => s.clone(),
            _ => return Err(type_error("Item.language.match is not a function")),
        };
        if language.contains('>') || language.contains('<') {
            static LANG_SPLIT: LazyLock<Regex> =
                LazyLock::new(|| rx(&format!("({DOT}*?)([<>])({DOT}*)")));
            let m = LANG_SPLIT
                .captures(&language)
                .ok_or_else(|| type_error("Cannot read properties of null (reading '2')"))?;
            let g = |n: usize| m.get(n).map(|x| x.as_str().to_string()).unwrap_or_default();
            if g(2) == "<" {
                item.insert("language-name".into(), Value::String(g(1)));
                item.insert("language-name-original".into(), Value::String(g(3)));
            } else {
                item.insert("language-name".into(), Value::String(g(3)));
                item.insert("language-name-original".into(), Value::String(g(1)));
            }
            if js::truthy_opt(state.opt.get("multi_layout")) {
                if js::truthy_opt(item.get("language-name-original")) {
                    let v = item
                        .get("language-name-original")
                        .cloned()
                        .unwrap_or(Value::Null);
                    item.insert("language".into(), v);
                }
            } else if js::truthy_opt(item.get("language-name")) {
                let v = item.get("language-name").cloned().unwrap_or(Value::Null);
                item.insert("language".into(), v);
            }
        }
    }

    if js::truthy_opt(item.get("page")) {
        let page = item.get("page").cloned().unwrap_or(Value::Null);
        item.insert("page-first".into(), page.clone());
        let num = js::to_js_string(&page);
        static PAGE_SPLIT: LazyLock<Regex> =
            LazyLock::new(|| rx(&format!("[{ws}]*(?:&|, |-|\u{2013})[{ws}]*", ws = js::WS)));
        let m = js::split(&PAGE_SPLIT, &num);
        let m0 = m.first().cloned().unwrap_or_default();
        if js::slice(&m0, -1, None) != "\\" {
            item.insert("page-first".into(), Value::String(m0));
        }
    }

    // Optional development extensions
    if dev_ext_truthy(state, "field_hack") && js::truthy_opt(item.get("note")) {
        let allow = dev_ext_truthy(state, "allow_field_hack_date_override");
        parse_note_field_hacks(state, &mut item, allow)?;
    }

    // not including locator-date
    let keys: Vec<String> = item.keys().cloned().collect();
    for key in keys {
        let base = key.strip_prefix("alt-").unwrap_or(&key);
        if !DATE_VARIABLES.contains(&base) {
            continue;
        }
        let mut dateobj = item.get(&key).cloned().unwrap_or(Value::Null);
        if js::truthy(&dateobj) {
            // raw date parsing is harmless, but can be disabled if desired
            if dev_ext_truthy(state, "raw_date_parsing") {
                let raw = dateobj.get("raw").cloned();
                let dp_empty = match dateobj.get("date-parts") {
                    None => true,
                    Some(v) if !js::truthy(v) => true,
                    Some(Value::Array(a)) => a.is_empty(),
                    Some(Value::String(s)) => s.is_empty(),
                    Some(_) => false,
                };
                if let Some(raw) = raw.filter(js::truthy) {
                    if dp_empty {
                        let raw_s = match &raw {
                            Value::String(s) => s.clone(),
                            _ => return Err(type_error("txt.replace is not a function")),
                        };
                        dateobj = Value::Object(state.fun.dateparser.parse_date_to_object(&raw_s));
                    }
                }
            }
            let parsed = state.date_parse_array(&date_obj_for_parse(&dateobj))?;
            item.insert(key.clone(), Value::Object(parsed));
        }
    }

    let item_type = item.get("type").cloned();
    let type_in = |list: &[&str]| -> bool {
        item_type
            .as_ref()
            .and_then(Value::as_str)
            .map(|t| list.contains(&t))
            .unwrap_or(false)
    };
    if dev_ext_truthy(state, "consolidate_legal_items")
        && js::truthy_opt(item.get("type"))
        && type_in(&["bill", "gazette", "legislation", "regulation", "treaty"])
    {
        let mut legislation_id: Vec<String> = Vec::new();
        for varname in [
            "type",
            "title",
            "jurisdiction",
            "genre",
            "volume",
            "container-title",
        ] {
            if let Some(v) = item.get(varname).filter(|v| js::truthy(v)) {
                legislation_id.push(js::to_js_string(v));
            }
        }
        for varname in ["original-date", "issued"] {
            if let Some(y) = item
                .get(varname)
                .and_then(|d| d.get("year"))
                .filter(|y| js::truthy(y))
            {
                legislation_id.push(js::to_js_string(y));
                break;
            }
        }
        item.insert(
            "legislation_id".into(),
            Value::String(legislation_id.join("::")),
        );
    }
    if let Some(track) = state.bibliography.opt.get("track_container_items") {
        if js::truthy(track) {
            let found = match track {
                Value::Array(a) => a.iter().any(|x| Some(x) == item.get("type")),
                _ => false,
            };
            if found {
                let mut container_id: Vec<String> = Vec::new();
                for varname in ["type", "container-title", "publisher", "edition"] {
                    if let Some(v) = item.get(varname).filter(|v| js::truthy(v)) {
                        container_id.push(js::to_js_string(v));
                    }
                }
                item.insert(
                    "container_id".into(),
                    Value::String(container_id.join("::")),
                );
            }
        }
    }
    // For authority to name shape in legal styles
    if dev_ext_truthy(state, "force_jurisdiction") {
        if let Some(Value::String(auth)) = item.get("authority").cloned() {
            let mut name = Obj::new();
            name.insert("literal".into(), Value::String(auth));
            let mut key_obj = Obj::new();
            if let Some(Value::Object(ak)) = item
                .get("multi")
                .and_then(|m| m.get("_keys"))
                .and_then(|k| k.get("authority"))
            {
                for (k, v) in ak {
                    let mut lit = Obj::new();
                    lit.insert("literal".into(), v.clone());
                    key_obj.insert(k.clone(), Value::Object(lit));
                }
            }
            let mut multi = Obj::new();
            multi.insert("_key".into(), Value::Object(key_obj));
            name.insert("multi".into(), Value::Object(multi));
            item.insert("authority".into(), Value::Array(vec![Value::Object(name)]));
        }
    }
    // Add getAbbreviation() call for title-short and container-title-short
    if !js::truthy_opt(item.get("title-short")) {
        let short = item.get("shortTitle").cloned();
        set_or_remove(&mut item, "title-short", short);
    }
    // Add support for main_title_from_short_title
    if dev_ext_truthy(state, "main_title_from_short_title") {
        let dl0 = state
            .opt
            .get("default-locale")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .map(js::to_js_string)
            .ok_or_else(|| type_error("Cannot read properties of undefined (reading 'slice')"))?;
        let narrow_space_locale = js::slice(&dl0, 0, Some(2)).to_lowercase() == "fr";
        extract_title_and_subtitle(state, &mut item, narrow_space_locale)?;
    }
    let item_type = item.get("type").cloned();
    let is_legal_type = item_type
        .as_ref()
        .and_then(Value::as_str)
        .map(|t| ["bill", "legal_case", "legislation", "gazette", "regulation"].contains(&t))
        .unwrap_or(false);
    if dev_ext_truthy(state, "force_jurisdiction")
        && is_legal_type
        && !js::truthy_opt(item.get("jurisdiction"))
    {
        item.insert("jurisdiction".into(), Value::String("us".into()));
    }
    let jurisdiction = item
        .get("jurisdiction")
        .filter(|j| js::truthy(j))
        .map(js::to_js_string);
    if !is_legal_type && js::truthy_opt(item.get("title")) {
        let title = js::to_js_string(item.get("title").unwrap_or(&Value::Null));
        let normalized_key = normalize_abbrevs_key("title", Some(&title));
        if let Some(abbr) =
            abbreviation_lookup(state, jurisdiction.as_deref(), "title", &normalized_key)?
        {
            item.insert("title-short".into(), Value::String(abbr));
        }
    }
    if !js::truthy_opt(item.get("container-title-short")) {
        let abbr = item.get("journalAbbreviation").cloned();
        set_or_remove(&mut item, "container-title-short", abbr);
    }
    if js::truthy_opt(item.get("container-title")) {
        // The runner's one-argument call: normalizeAbbrevsKey(Item["container-title"])
        // takes the title as `variable` and an undefined key, which is "".
        let normalized_key = normalize_abbrevs_key(
            &js::to_js_string(item.get("container-title").unwrap_or(&Value::Null)),
            None,
        );
        if let Some(abbr) = abbreviation_lookup(
            state,
            jurisdiction.as_deref(),
            "container-title",
            &normalized_key,
        )? {
            item.insert("container-title-short".into(), Value::String(abbr));
        }
    }
    if let Some(j) = &jurisdiction {
        item.insert(
            "country".into(),
            Value::String(j.split(':').next().unwrap_or("").to_string()),
        );
    }

    let new_item = Value::Object(item);
    if let Some(existing) = state.item_refhash.get(id).cloned() {
        if existing != new_item {
            let item_id = js::to_js_string(new_item.get("id").unwrap_or(&Value::Null));
            state.tmp.tainted_item_ids.insert(item_id, true);
            state.item_refhash.insert(id.to_string(), new_item);
        }
    } else {
        state.item_refhash.insert(id.to_string(), new_item);
    }
    Ok(state.item_refhash.get(id).cloned().unwrap_or(Value::Null))
}

// ---------------------------------------------------------------------------
// Intermediate-dump sections (scripts/csl-intermediate-reference.cjs)
// ---------------------------------------------------------------------------

/// A JSON tree whose objects remember their key order, for the dump sections
/// whose outer objects are NOT key-sorted in the reference (`items`, `names`
/// and `numbers` are built as `o[id] = ...`, so their keys follow input
/// order, with integer-like ids first). Sub-trees the reference passes
/// through `canon` are [`OJson::Val`] (keys sorted).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum OJson {
    /// A canonical (key-sorted) JSON value.
    Val(Value),
    /// A JS object, keys in insertion order.
    Obj(Vec<(String, OJson)>),
    /// A JS array.
    Arr(Vec<OJson>),
}

/// Whether `k` is a JS array-index key (`"0"`, `"17"`; not `"01"`), which
/// JS objects enumerate first, in ascending order.
fn is_array_index(k: &str) -> bool {
    !k.is_empty()
        && k.bytes().all(|b| b.is_ascii_digit())
        && (k == "0" || !k.starts_with('0'))
        && k.parse::<u64>().map(|n| n < 4_294_967_295).unwrap_or(false)
}

/// `o[key] = value` on a JS object model: a repeated key keeps its first
/// position and takes the new value.
fn obj_put(pairs: &mut Vec<(String, OJson)>, key: String, value: OJson) {
    if let Some(slot) = pairs.iter_mut().find(|p| p.0 == key) {
        slot.1 = value;
    } else {
        pairs.push((key, value));
    }
}

impl OJson {
    /// A JS-style error record `{error: message}`.
    fn error(message: String) -> OJson {
        let mut o = Obj::new();
        o.insert("error".into(), Value::String(message));
        OJson::Val(Value::Object(o))
    }

    /// As a plain [`Value`] (object keys sorted, which is how a `Value`
    /// compares).
    pub(crate) fn to_value(&self) -> Value {
        match self {
            OJson::Val(v) => v.clone(),
            OJson::Arr(a) => Value::Array(a.iter().map(OJson::to_value).collect()),
            OJson::Obj(pairs) => {
                let mut o = Obj::new();
                for (k, v) in pairs {
                    o.insert(k.clone(), v.to_value());
                }
                Value::Object(o)
            }
        }
    }

    /// `JSON.stringify(x)` of the JS value this models: compact, objects in
    /// JS enumeration order (array-index keys ascending, then insertion
    /// order), sorted sub-trees as given.
    pub(crate) fn to_js_string(&self) -> String {
        match self {
            OJson::Val(v) => serde_json::to_string(v).unwrap_or_default(),
            OJson::Arr(a) => {
                format!(
                    "[{}]",
                    a.iter()
                        .map(OJson::to_js_string)
                        .collect::<Vec<_>>()
                        .join(",")
                )
            }
            OJson::Obj(pairs) => {
                let mut idx: Vec<&(String, OJson)> =
                    pairs.iter().filter(|p| is_array_index(&p.0)).collect();
                idx.sort_by_key(|p| p.0.parse::<u64>().unwrap_or(0));
                let rest = pairs.iter().filter(|p| !is_array_index(&p.0));
                let body: Vec<String> = idx
                    .into_iter()
                    .chain(rest)
                    .map(|(k, v)| {
                        format!(
                            "{}:{}",
                            serde_json::to_string(k).unwrap_or_default(),
                            v.to_js_string()
                        )
                    })
                    .collect();
                format!("{{{}}}", body.join(","))
            }
        }
    }
}

/// The key `o[it.id]` / `o[I.id]` of the dump: the id as a JS property name.
fn id_key(v: Option<&Value>) -> String {
    v.map(js::to_js_string)
        .unwrap_or_else(|| "undefined".to_string())
}

/// The message JS's `e.message` would carry for an engine error.
pub(crate) fn error_message(e: &EngineError) -> String {
    match e {
        // CSL.error throws the string "citeproc-js error: ..." (no .message)
        EngineError::Csl(m) => format!("citeproc-js error: {m}"),
        // a JS TypeError: its message
        EngineError::BadInput(m) => m.clone(),
        other => other.to_string(),
    }
}

/// The `items` section: `engine.retrieveItem(id)` for every input item, in
/// input order, as `{id: normalised item}`; also returns the normalised
/// items (needed by the other sections). A failure is returned as
/// `{"error": message}`, as the script's `guarded` does.
pub(crate) fn items_section(state: &mut State, inputs: &[Value]) -> (OJson, Vec<Value>) {
    let mut out: Vec<(String, OJson)> = Vec::new();
    let mut norm = Vec::new();
    for it in inputs {
        let id = js::to_js_string(it.get("id").unwrap_or(&Value::Null));
        match retrieve_item(state, &id) {
            Ok(i) => {
                let mut c = i.clone();
                canon_numbers(&mut c);
                obj_put(&mut out, id_key(it.get("id")), OJson::Val(c));
                norm.push(i);
            }
            Err(e) => return (OJson::error(error_message(&e)), norm),
        }
    }
    (OJson::Obj(out), norm)
}

/// The `names` section: for every name of every name variable of each
/// normalised item, `{static_ordering, name}` (the input side of the name
/// renderer), or `{error}`.
pub(crate) fn names_section(state: &State, items: &[Value]) -> OJson {
    let mut out: Vec<(String, OJson)> = Vec::new();
    for item in items {
        let ctx = NameInputCtx::from_state_item(state, item);
        let mut per_item: Vec<(String, OJson)> = Vec::new();
        for v in NAME_VARIABLES {
            let Some(Value::Array(list)) = item.get(v) else {
                continue;
            };
            let recs: Vec<OJson> = list
                .iter()
                .map(|name| {
                    let mut rec: Vec<(String, OJson)> = Vec::new();
                    let res: CslResult<()> = (|| {
                        let Value::Object(n) = name else {
                            return Err(EngineError::BadInput(format!(
                                "Cannot create property 'family' on {} '{}'",
                                if name.is_string() { "string" } else { "value" },
                                js::to_js_string(name)
                            )));
                        };
                        let mut for_static = n.clone();
                        for k in ["family", "given"] {
                            if !js::truthy_opt(for_static.get(k)) {
                                for_static.insert(k.into(), Value::String(String::new()));
                            }
                        }
                        let so = get_static_order(&ctx, &for_static, false)?;
                        rec.push(("static_ordering".into(), OJson::Val(Value::Bool(so))));
                        let nm = normalize_name_input(&ctx, name)?;
                        let mut nv = Value::Object(nm);
                        canon_numbers(&mut nv);
                        rec.push(("name".into(), OJson::Val(nv)));
                        Ok(())
                    })();
                    if let Err(e) = res {
                        rec.push(("error".into(), OJson::Val(Value::String(error_message(&e)))));
                    }
                    OJson::Obj(rec)
                })
                .collect();
            obj_put(&mut per_item, v.to_string(), OJson::Arr(recs));
        }
        obj_put(&mut out, id_key(item.get("id")), OJson::Obj(per_item));
    }
    OJson::Obj(out)
}

/// The `numbers` section: for each numeric variable (and `page-first`) an
/// item has, `processNumber(false, Item, variable)`'s resulting
/// `shadow_numbers` (parsed values, labels, plural / numeric / collapsible
/// flags), or `{error}`.
pub(crate) fn numbers_section(state: &mut State, items: &[Value]) -> OJson {
    let mut out: Vec<(String, OJson)> = Vec::new();
    for item in items {
        let mut per_item: Vec<(String, OJson)> = Vec::new();
        for v in NUMERIC_VARIABLES
            .iter()
            .copied()
            .chain(std::iter::once("page-first"))
        {
            if item.get(v).is_none() {
                continue;
            }
            state.tmp.shadow_numbers = BTreeMap::new();
            let val = match process_number(state, None, Some(item), v) {
                Ok(()) => {
                    let mut o = Obj::new();
                    for (k, sn) in &state.tmp.shadow_numbers {
                        o.insert(k.clone(), sn.to_value());
                    }
                    OJson::Val(Value::Object(o))
                }
                Err(e) => OJson::error(error_message(&e)),
            };
            obj_put(&mut per_item, v.to_string(), val);
        }
        obj_put(&mut out, id_key(item.get("id")), OJson::Obj(per_item));
    }
    state.tmp.shadow_numbers = BTreeMap::new();
    OJson::Obj(out)
}

/// The `citation_items` section: for each list of citation items (a
/// `CITATION-ITEMS` entry or a `CITATIONS` entry's `citationItems`), each
/// item after the input steps `makeCitationCluster` applies (a shallow copy,
/// `parseLocator`, `remapSectionVariable`, `locator_label_parse`), or
/// `{error}`.
pub(crate) fn citation_items_section(state: &mut State, lists: &[Vec<Value>]) -> OJson {
    let mut out = Vec::new();
    for list in lists {
        let mut row = Vec::new();
        for ci in list {
            let r: CslResult<Value> = (|| {
                let mut item: Obj = match ci {
                    Value::Object(o) => o.clone(),
                    _ => Obj::new(),
                };
                let id = js::to_js_string(item.get("id").unwrap_or(&Value::Null));
                let item_val = retrieve_item(state, &id)?;
                let mut item_obj = match item_val {
                    Value::Object(o) => o,
                    _ => Obj::new(),
                };
                citation_item_input(state, &mut item_obj, &mut item)?;
                // `Item` is the cached object: remapSectionVariable edits it in place.
                state.item_refhash.insert(id, Value::Object(item_obj));
                let mut v = Value::Object(item);
                canon_numbers(&mut v);
                Ok(v)
            })();
            row.push(match r {
                Ok(v) => OJson::Val(v),
                Err(e) => OJson::error(error_message(&e)),
            });
        }
        out.push(OJson::Arr(row));
    }
    OJson::Arr(out)
}

#[cfg(test)]
mod tests {
    //! Verification of `retrieveItem` and the four input-side dump sections
    //! against citeproc-js 2.4.63, by the intermediate-dump method
    //! (`scripts/csl-intermediate-reference.cjs`, GitHub #792).
    //!
    //! * `sample_fixtures_and_site_sections_equal_the_full_reference`:
    //!   the `items`, `names`, `numbers` and `citation_items` sections for
    //!   the 12 sample fixtures and the 5 site styles are compared, value by
    //!   value, with `tests/data/csl/intermediate_reference_full.json`.
    //! * `all_fixture_section_digests_equal_the_reference`: for all 845
    //!   fixtures the SHA-256 of each section's `JSON.stringify` (key order as
    //!   JS enumerates it) equals `tests/data/csl/intermediate_reference.json`.
    //!   Needs `vendor/csl-test-suite` (skipped, with a message, when absent).
    //! * `retrieve_item_normalisations`: hand-written cases for the options
    //!   no fixture turns on (`field_hack`, `main_title_from_short_title`,
    //!   `consolidate_legal_items`, `force_jurisdiction`, ...), checked against
    //!   values recorded from citeproc-js in `retrieve_options.json`.
    //!
    //! The engine state that the style builder will provide (`engine.opt`, the
    //! locale terms read, the abbreviation cache) is replayed from
    //! `tests/data/csl/units/fixture_contexts.json` (generator
    //! `scripts/csl-units/fixture_contexts.cjs`), so these tests need neither
    //! the style loader nor the locale loader.
    use std::path::PathBuf;
    use std::sync::Arc;

    use sha2::{Digest, Sha256};

    use super::*;

    const CONTEXTS: &str = include_str!("../../tests/data/csl/units/fixture_contexts.json");
    const DIGESTS: &str = include_str!("../../tests/data/csl/intermediate_reference.json");
    const FULL: &str = include_str!("../../tests/data/csl/intermediate_reference_full.json");
    const SITE_ITEMS: &str = include_str!("../../tests/data/csl/items.json");

    const TURKISH_MONTHS: [&str; 16] = [
        "ocak", "Şubat", "mart", "nisan", "mayıs", "haziran", "temmuz", "ağustos", "eylül", "ekim",
        "kasım", "aralık", "bahar", "yaz", "sonbahar", "kış",
    ];

    fn fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../vendor/csl-test-suite/processor-tests/humans")
    }

    /// The sections of one fixture file (the runner's `fixture-parser.js`).
    fn parse_fixture(text: &str) -> BTreeMap<String, String> {
        let names = "CSL|KEYS|DESCRIPTION|INPUT|MODE|RESULT|NAME|PATH|ABBREVIATIONS|BIBENTRIES|BIBSECTION|CITATION-ITEMS|CITATIONS|INPUT2|LANGPARAMS|MULTIAFFIX|OPTIONS|OPTIONZ";
        let open = Regex::new(&format!(r"^.*>>===*\s({names})\s.*=>>.*")).expect("regex");
        let close = Regex::new(&format!(r"^.*<<===*\s({names})\s.*=<<.*")).expect("regex");
        let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut section = String::new();
        let mut state = 0u8; // 0 none, 1 opening, 2 reading, 3 closing
        for line in text.split("\r\n").flat_map(|l| l.split('\n')) {
            if let Some(m) = open.captures(line) {
                section = m[1].to_string();
                state = 1;
            } else if close.is_match(line) {
                state = 3;
            } else if state == 1 {
                out.insert(section.clone(), Vec::new());
                state = 2;
            } else if state == 3 {
                state = 0;
            }
            if state == 2 {
                out.entry(section.clone())
                    .or_default()
                    .push(line.to_string());
            }
        }
        out.into_iter().map(|(k, v)| (k, v.join("\n"))).collect()
    }

    fn context_state(
        ctx: &Value,
        terms: Option<&Value>,
        items: &[Value],
        abbrevs: Option<&Value>,
        turkish: bool,
    ) -> State {
        let mut st = State::default();
        st.opt = ctx["opt"].as_object().cloned().unwrap_or_default();
        if !ctx["track"].is_null() {
            st.bibliography
                .opt
                .insert("track_container_items".into(), ctx["track"].clone());
        }
        if let Some(t) = terms.and_then(Value::as_object) {
            st.input_locale.terms = t
                .iter()
                .map(|(k, v)| (k.clone(), v.as_str().map(str::to_string)))
                .collect();
        }
        let mut map = BTreeMap::new();
        for it in items {
            map.insert(
                js::to_js_string(it.get("id").unwrap_or(&Value::Null)),
                it.clone(),
            );
        }
        st.sys.items = Arc::new(map);
        if let Some(a) = abbrevs.and_then(Value::as_object) {
            for (jur, cats) in a {
                for (cat, entries) in cats.as_object().into_iter().flatten() {
                    for (k, v) in entries.as_object().into_iter().flatten() {
                        if let Some(s) = v.as_str() {
                            st.sys
                                .abbreviations
                                .entry(jur.clone())
                                .or_default()
                                .entry(cat.clone())
                                .or_default()
                                .insert(k.clone(), s.to_string());
                        }
                    }
                }
            }
        }
        if turkish {
            let l: Vec<String> = TURKISH_MONTHS.iter().map(|s| s.to_string()).collect();
            st.fun.dateparser.add_date_parser_months(&l);
        }
        st
    }

    struct Sections {
        items: OJson,
        names: OJson,
        numbers: OJson,
        citation_items: OJson,
    }

    fn run_sections(st: &mut State, inputs: &[Value], lists: &[Vec<Value>]) -> Sections {
        let (items, norm) = items_section(st, inputs);
        let names = names_section(st, &norm);
        let numbers = numbers_section(st, &norm);
        let citation_items = citation_items_section(st, lists);
        Sections {
            items,
            names,
            numbers,
            citation_items,
        }
    }

    fn lists_of(sections: &BTreeMap<String, String>) -> Vec<Vec<Value>> {
        let mut lists: Vec<Vec<Value>> = Vec::new();
        if let Some(t) = sections.get("CITATION-ITEMS") {
            if let Ok(Value::Array(a)) = serde_json::from_str::<Value>(t) {
                for c in a {
                    lists.push(c.as_array().cloned().unwrap_or_default());
                }
            }
        }
        if let Some(t) = sections.get("CITATIONS") {
            if let Ok(Value::Array(a)) = serde_json::from_str::<Value>(t) {
                for c in a {
                    let ci = c
                        .get(0)
                        .and_then(|c0| c0.get("citationItems"))
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    lists.push(ci);
                }
            }
        }
        lists
    }

    fn sha(s: &str) -> String {
        let mut h = Sha256::new();
        h.update(s.as_bytes());
        h.finalize().iter().map(|b| format!("{b:02x}")).collect()
    }

    const OPTIONS_REF: &str = include_str!("../../tests/data/csl/units/retrieve_options.json");

    /// JS `JSON.stringify` of a value with sorted keys (the reference's `canonJSON`).
    fn canon_json(v: &Value) -> String {
        let mut v = v.clone();
        canon_numbers(&mut v);
        serde_json::to_string(&v).unwrap_or_default()
    }

    #[test]
    fn retrieve_item_normalisations_match_citeproc_js_under_every_option_set() {
        let r: Value = serde_json::from_str(OPTIONS_REF).expect("json");
        let items: Vec<Value> = r["items"].as_array().cloned().unwrap_or_default();
        let mut n = 0;
        for variant in r["variants"].as_array().expect("variants") {
            let name = variant["name"].as_str().unwrap_or("");
            let ctx = serde_json::json!({"opt": variant["opt"], "track": variant["track"]});
            let mut st = context_state(&ctx, None, &items, Some(&r["abbrevs"]), false);
            for c in variant["cases"].as_array().expect("cases") {
                let id = c["id"].as_str().unwrap_or("");
                let got = retrieve_item(&mut st, id);
                n += 1;
                match (got, c.get("error")) {
                    (Ok(v), None) => {
                        let mut want = c["out"].clone();
                        canon_numbers(&mut want);
                        assert_eq!(v, want, "[{name}] item {id}: {}", st.sys.items[id]);
                        // a second call returns the cached item
                        assert_eq!(
                            retrieve_item(&mut st, id).ok(),
                            Some(v),
                            "[{name}] cached {id}"
                        );
                    }
                    (Err(e), Some(w)) => {
                        assert_eq!(
                            w.as_str(),
                            Some(error_message(&e).as_str()),
                            "[{name}] {id}"
                        )
                    }
                    (g, w) => panic!("[{name}] item {id}: {g:?} vs {w:?}"),
                }
            }
            for k in ["default-locale", "locale-translit", "locale-translat"] {
                assert_eq!(st.opt[k], variant["opt_after"][k], "[{name}] opt {k} after");
            }
            assert_eq!(
                st.opt["development_extensions"]["normalize_lang_keys_to_lowercase"],
                variant["dev_after"],
                "[{name}] normalize_lang_keys_to_lowercase after"
            );
        }
        assert!(n > 3000, "{n}");
    }

    #[test]
    fn every_fixture_item_matches_citeproc_js_under_every_option_set() {
        let dir = fixture_dir();
        if !dir.exists() {
            eprintln!(
                "skipped: {} is absent (run scripts/csl-reference.sh)",
                dir.display()
            );
            return;
        }
        let r: Value = serde_json::from_str(OPTIONS_REF).expect("json");
        let mut files: Vec<_> = std::fs::read_dir(&dir)
            .expect("dir")
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().map(|x| x == "txt").unwrap_or(false))
            .collect();
        files.sort();
        let mut all: Vec<(String, Value)> = Vec::new();
        for p in files {
            let sections = parse_fixture(&std::fs::read_to_string(&p).expect("fixture"));
            if let Some(Ok(Value::Array(a))) = sections
                .get("INPUT")
                .map(|t| serde_json::from_str::<Value>(t))
            {
                for it in a {
                    all.push((format!("FX{}", all.len()), it));
                }
            }
        }
        assert!(all.len() > 1500, "{}", all.len());
        for variant in r["variants"].as_array().expect("variants") {
            let name = variant["name"].as_str().unwrap_or("");
            let ctx = serde_json::json!({"opt": variant["opt"], "track": variant["track"]});
            let mut st = context_state(&ctx, None, &[], Some(&r["abbrevs"]), false);
            let mut map = BTreeMap::new();
            for (k, it) in &all {
                let mut it = it.clone();
                canon_numbers(&mut it);
                map.insert(k.clone(), it);
            }
            st.sys.items = Arc::new(map);
            let mut h = Sha256::new();
            let mut errors = 0;
            for (k, _) in &all {
                match retrieve_item(&mut st, k) {
                    Ok(v) => h.update(format!("{}\n", canon_json(&v)).as_bytes()),
                    Err(e) => {
                        errors += 1;
                        h.update(format!("ERR {}\n", error_message(&e)).as_bytes());
                    }
                }
            }
            let digest: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
            assert_eq!(
                Value::String(digest),
                variant["fixtures_digest"],
                "[{name}] all fixture items"
            );
            assert_eq!(
                Value::from(errors),
                variant["fixtures_errors"],
                "[{name}] errors"
            );
        }
    }

    #[test]
    fn all_fixture_section_digests_equal_the_reference() {
        let dir = fixture_dir();
        if !dir.exists() {
            eprintln!(
                "skipped: {} is absent (run scripts/csl-reference.sh)",
                dir.display()
            );
            return;
        }
        let ctxs: Value = serde_json::from_str(CONTEXTS).expect("contexts");
        let digests: Value = serde_json::from_str(DIGESTS).expect("digests");
        let mut compared = 0usize;
        let mut bad: Vec<String> = Vec::new();
        for name in ctxs["order"].as_array().expect("order") {
            let name = name.as_str().unwrap_or("");
            let text = std::fs::read_to_string(dir.join(format!("{name}.txt"))).expect("fixture");
            let sections = parse_fixture(&text);
            let mut input: Vec<Value> = serde_json::from_str(&sections["INPUT"]).expect("INPUT");
            for i in input.iter_mut() {
                canon_numbers(i);
            }
            let f = &ctxs["fixtures"][name];
            let ctx = &ctxs["contexts"][f["ctx"].as_str().unwrap_or("")];
            let mut st = context_state(ctx, f.get("terms"), &input, f.get("abbrevs"), true);
            let lists = lists_of(&sections);
            let got = run_sections(&mut st, &input, &lists);
            let want = &digests["fixtures"][name];
            for (sec, v) in [
                ("items", &got.items),
                ("names", &got.names),
                ("numbers", &got.numbers),
                ("citation_items", &got.citation_items),
            ] {
                compared += 1;
                if sha(&v.to_js_string()) != want[sec].as_str().unwrap_or("") {
                    bad.push(format!("{name}/{sec}"));
                }
            }
        }
        assert!(
            bad.is_empty(),
            "{} of {compared} section digests differ from citeproc-js; first: {:?}",
            bad.len(),
            &bad[..bad.len().min(25)]
        );
        assert!(compared >= 845 * 4, "{compared}");
    }

    #[test]
    fn sample_fixtures_and_site_sections_equal_the_full_reference() {
        let ctxs: Value = serde_json::from_str(CONTEXTS).expect("contexts");
        let full: Value = serde_json::from_str(FULL).expect("full");
        // site styles over items.json
        let site_items: Vec<Value> = serde_json::from_str(SITE_ITEMS).expect("items");
        let mut site_first: Option<(Value, Value, Value)> = None;
        let mut n = 0;
        for (name, s) in ctxs["site"].as_object().expect("site") {
            let ctx = &ctxs["contexts"][s["ctx"].as_str().unwrap_or("")];
            let mut st = context_state(ctx, Some(&s["terms"]), &site_items, None, false);
            let got = run_sections(&mut st, &site_items, &[]);
            let have = [
                ("items", got.items.to_value()),
                ("names", got.names.to_value()),
                ("numbers", got.numbers.to_value()),
            ];
            for (sec, v) in have {
                let mut want = full["site"][name.as_str()][sec].clone();
                if want.get("same_as").is_some() {
                    want = full["site"][want["same_as"].as_str().unwrap_or("")][sec].clone();
                }
                assert_eq!(v, want, "site {name} {sec}");
                n += 1;
            }
            let _ = &mut site_first;
        }
        // sample fixtures
        let dir = fixture_dir();
        if !dir.exists() {
            eprintln!("fixture part skipped: {} is absent", dir.display());
            return;
        }
        for (name, want) in full["fixtures"].as_object().expect("fixtures") {
            let text = std::fs::read_to_string(dir.join(format!("{name}.txt"))).expect("fixture");
            let sections = parse_fixture(&text);
            let mut input: Vec<Value> = serde_json::from_str(&sections["INPUT"]).expect("INPUT");
            for i in input.iter_mut() {
                canon_numbers(i);
            }
            let f = &ctxs["fixtures"][name.as_str()];
            let ctx = &ctxs["contexts"][f["ctx"].as_str().unwrap_or("")];
            let mut st = context_state(ctx, f.get("terms"), &input, f.get("abbrevs"), true);
            let got = run_sections(&mut st, &input, &lists_of(&sections));
            for (sec, v) in [
                ("items", got.items.to_value()),
                ("names", got.names.to_value()),
                ("numbers", got.numbers.to_value()),
                ("citation_items", got.citation_items.to_value()),
            ] {
                assert_eq!(v, want[sec], "fixture {name} {sec}");
                n += 1;
            }
        }
        assert!(n >= 12 * 4 + 15, "{n}");
    }
}
