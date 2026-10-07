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
static NOTE_FIELDS_RE: LazyLock<Regex> =
    LazyLock::new(|| rx("\\{:(?:[-_a-z]+|[A-Z]+):[^}]+\\}"));

// DUP-CHECK: load.js CSL.NOTE_FIELD_REGEXP  /^([\-_a-z]+|[A-Z]+):\s*([^\}]+)$/
static NOTE_FIELD_RE: LazyLock<Regex> = LazyLock::new(|| {
    rx(&format!("^([-_a-z]+|[A-Z]+):[{ws}]*([^}}]+)$", ws = js::WS))
});

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
    state.opt.get("development_extensions").and_then(|d| d.get(name))
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
    EngineError::Csl(msg.to_string())
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
        return Err(EngineError::NotYetPorted { method: "CSL.getAbbrevsDomain" });
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
    item.insert("note".into(), Value::String(js::trim(&lines.join("\n")).to_string()));
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
            lst.insert(iu + 1, m.get(iu).cloned().unwrap_or_else(|| "undefined".into()));
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
            let keys = item.get("multi").and_then(|m| m.get("_keys")).ok_or_else(|| {
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
                        keys.get(&t_title).and_then(|k| k.get(lang.as_str())).cloned(),
                    );
                }
                if js::truthy_opt(keys.get(&t_short)) {
                    vals.insert(
                        t_short.clone(),
                        keys.get(&t_short).and_then(|k| k.get(lang.as_str())).cloned(),
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
                let title = vals_str(&vals, &t_title).ok_or_else(|| {
                    type_error("vals[title.title].toLowerCase is not a function")
                })?;
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
                            set(&mut vals, &t_subjoin, TRAIL_Q_WS.replace(m1, "${1}").into_owned());
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

    if matches!(dev_ext(state, "normalize_lang_keys_to_lowercase"), Some(Value::Bool(true))) {
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
    if dev_ext_truthy(state, "normalize_lang_keys_to_lowercase") && js::truthy_opt(item.get("multi")) {
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
                        _ => return Err(type_error("Item.multi.main[field].toLowerCase is not a function")),
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
            let m = LANG_SPLIT.captures(&language).ok_or_else(|| {
                type_error("Cannot read properties of null (reading '2')")
            })?;
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
                    let v = item.get("language-name-original").cloned().unwrap_or(Value::Null);
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
        static PAGE_SPLIT: LazyLock<Regex> = LazyLock::new(|| {
            rx(&format!("[{ws}]*(?:&|, |-|\u{2013})[{ws}]*", ws = js::WS))
        });
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
        for varname in ["type", "title", "jurisdiction", "genre", "volume", "container-title"] {
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
        item.insert("legislation_id".into(), Value::String(legislation_id.join("::")));
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
                item.insert("container_id".into(), Value::String(container_id.join("::")));
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
    if dev_ext_truthy(state, "force_jurisdiction") && is_legal_type && !js::truthy_opt(item.get("jurisdiction")) {
        item.insert("jurisdiction".into(), Value::String("us".into()));
    }
    let jurisdiction = item
        .get("jurisdiction")
        .filter(|j| js::truthy(j))
        .map(js::to_js_string);
    if !is_legal_type && js::truthy_opt(item.get("title")) {
        let title = js::to_js_string(item.get("title").unwrap_or(&Value::Null));
        let normalized_key = normalize_abbrevs_key("title", Some(&title));
        if let Some(abbr) = abbreviation_lookup(state, jurisdiction.as_deref(), "title", &normalized_key)? {
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
        if let Some(abbr) =
            abbreviation_lookup(state, jurisdiction.as_deref(), "container-title", &normalized_key)?
        {
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

/// The key `o[it.id]` / `o[I.id]` of the dump: the id as a JS property name.
fn id_key(v: Option<&Value>) -> String {
    v.map(js::to_js_string).unwrap_or_else(|| "undefined".to_string())
}

/// The `items` section: `engine.retrieveItem(id)` for every input item, in
/// input order, as `{id: normalised item}`; also returns the normalised
/// items (needed by the other sections). Errors are returned as
/// `{"error": message}`, as the script's `guarded` does.
pub(crate) fn items_section(state: &mut State, inputs: &[Value]) -> (Value, Vec<Value>) {
    let mut out = Obj::new();
    let mut norm = Vec::new();
    for it in inputs {
        let id = js::to_js_string(it.get("id").unwrap_or(&Value::Null));
        match retrieve_item(state, &id) {
            Ok(i) => {
                let mut c = i.clone();
                canon_numbers(&mut c);
                out.insert(id_key(it.get("id")), c);
                norm.push(i);
            }
            Err(e) => {
                let mut o = Obj::new();
                o.insert("error".into(), Value::String(error_message(&e)));
                return (Value::Object(o), norm);
            }
        }
    }
    (Value::Object(out), norm)
}

/// The message JS's `e.message` would carry for an engine error.
pub(crate) fn error_message(e: &EngineError) -> String {
    match e {
        EngineError::Csl(m) => m.clone(),
        other => other.to_string(),
    }
}

/// The `names` section: for every name of every name variable of each
/// normalised item, `{static_ordering, name}` (the input side of the name
/// renderer), or `{error}`.
pub(crate) fn names_section(state: &State, items: &[Value]) -> Value {
    let mut out = Obj::new();
    for item in items {
        let ctx = NameInputCtx::from_state_item(state, item);
        let mut per_item = Obj::new();
        for v in NAME_VARIABLES {
            let Some(Value::Array(list)) = item.get(v) else {
                continue;
            };
            let recs: Vec<Value> = list
                .iter()
                .map(|name| {
                    let mut rec = Obj::new();
                    let res: CslResult<()> = (|| {
                        let Value::Object(n) = name else {
                            return Err(EngineError::Csl(format!(
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
                        rec.insert("static_ordering".into(), Value::Bool(so));
                        let nm = normalize_name_input(&ctx, name)?;
                        let mut nv = Value::Object(nm);
                        canon_numbers(&mut nv);
                        rec.insert("name".into(), nv);
                        Ok(())
                    })();
                    if let Err(e) = res {
                        rec.insert("error".into(), Value::String(error_message(&e)));
                    }
                    Value::Object(rec)
                })
                .collect();
            per_item.insert(v.to_string(), Value::Array(recs));
        }
        out.insert(id_key(item.get("id")), Value::Object(per_item));
    }
    Value::Object(out)
}

/// The `numbers` section: for each numeric variable (and `page-first`) an
/// item has, `processNumber(false, Item, variable)`'s resulting
/// `shadow_numbers` (parsed values, labels, plural / numeric / collapsible
/// flags), or `{error}`.
pub(crate) fn numbers_section(state: &mut State, items: &[Value]) -> Value {
    let mut out = Obj::new();
    for item in items {
        let mut per_item = Obj::new();
        for v in NUMERIC_VARIABLES.iter().copied().chain(std::iter::once("page-first")) {
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
                    Value::Object(o)
                }
                Err(e) => {
                    let mut o = Obj::new();
                    o.insert("error".into(), Value::String(error_message(&e)));
                    Value::Object(o)
                }
            };
            per_item.insert(v.to_string(), val);
        }
        out.insert(id_key(item.get("id")), Value::Object(per_item));
    }
    state.tmp.shadow_numbers = BTreeMap::new();
    Value::Object(out)
}

/// The `citation_items` section: for each list of citation items (a
/// `CITATION-ITEMS` entry or a `CITATIONS` entry's `citationItems`), each
/// item after the input steps `makeCitationCluster` applies (a shallow copy,
/// `parseLocator`, `remapSectionVariable`, `locator_label_parse`), or
/// `{error}`.
pub(crate) fn citation_items_section(state: &mut State, lists: &[Vec<Value>]) -> Value {
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
                Ok(v) => v,
                Err(e) => {
                    let mut o = Obj::new();
                    o.insert("error".into(), Value::String(error_message(&e)));
                    Value::Object(o)
                }
            });
        }
        out.push(Value::Array(row));
    }
    Value::Array(out)
}
