// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_number.js (CSL.Util.padding, LongOrdinalizer, Ordinalizer,
//              Romanizer, Suffixator, CSL.Engine.prototype.processNumber,
//              CSL.Util.outputNumericField)
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

//! Port of `src/util_number.js`: number formatters (`Ordinalizer`,
//! `LongOrdinalizer`, `Romanizer`, `Suffixator`) and
//! `CSL.Engine.prototype.processNumber`, the parser that splits a numeric
//! variable (`"12-15"`, `"vol. 3 & 4"`, `"2nd ed."`) into labelled values.
//!
//! # The input path and the rendering path
//!
//! `processNumber(false, Item, variable)` is the INPUT path: it fills
//! `state.tmp.shadow_numbers[variable]` with the parsed values, labels and
//! the plural / numeric / collapsible flags. It is complete here.
//! `processNumber(node, ...)` additionally mangles ranges and builds the
//! styling tokens: [`fix_ranges`] (with `state.fun.page_mangler`) and
//! [`set_styling`] are ported; `CSL.Util.outputNumericField` renders through
//! the output queue and is deferred ([`output_numeric_field`]).
//!
//! # Locale terms
//!
//! citeproc-js reads locale terms through `state.getTerm(...)` and
//! `CSL.Engine.getField(...)` (build.js). Every term read in this file goes
//! through [`input_get_term`] / [`input_get_field`], thin wrappers over
//! `State::get_term` / `State::get_field` (which set
//! `tmp.cite_renders_content` as upstream does, so the readers take
//! `&mut State`). [`Ordinalizer`] reads `locale[lang].ord["1.0.1"]` directly.
//!
//! # Quirks kept on purpose
//!
//! `fixNumericAndCount` tests `val.particle` on a *string* (always
//! undefined); `setPluralsAndNumerics` compares `lastVal.particle` with
//! itself; `parseString`'s "and" handling tests `lst[i]` for both ends. All
//! reproduced.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::js::{self, Obj};
use super::obj_token::{Decoration, Token, TokenType};
use super::state::State;
use super::{CslResult, EngineError};

use super::load::{
    lang_prefs_map, statute_subdiv_string, statute_subdiv_string_reverse, LOOSE, ROMAN_NUMERALS,
    SUFFIX_CHARS,
};

fn rx(src: &str) -> Regex {
    // Static patterns only; a failure here is a programming error caught by
    // the unit tests.
    Regex::new(src).unwrap_or_else(|e| panic!("invalid static regex {src:?}: {e}"))
}

// ---------------------------------------------------------------------------
// Locale access
// ---------------------------------------------------------------------------

/// The arguments of `state.getTerm(term, form, plural, gender, mode,
/// forceDefaultLocale)`, normalised: `form` / `gender` `None` for
/// `undefined` or `false`; `plural` 0 for anything but a number; `mode`
/// `None` for `undefined`/`false`.
#[derive(Debug, Clone, PartialEq)]
pub struct TermQuery {
    /// The term name, e.g. `"and"`, `"month-01"`, `"ordinal-02"`.
    pub name: String,
    /// `"long"`, `"short"`, `"symbol"`, `"verb"`, ... or `None`.
    pub form: Option<String>,
    /// 0 (singular) or 1 (plural).
    pub plural: i64,
    /// `"masculine"`, `"feminine"`, `"neuter"` or `None`.
    pub gender: Option<String>,
    /// `CSL.STRICT` / `CSL.TOLERANT` / `CSL.LOOSE` or `None`.
    pub mode: Option<i64>,
    /// `forceDefaultLocale`.
    pub force_default_locale: bool,
}

impl TermQuery {
    /// A query for `name` with every other argument unset.
    pub fn new(name: &str) -> TermQuery {
        TermQuery {
            name: name.to_string(),
            form: None,
            plural: 0,
            gender: None,
            mode: None,
            force_default_locale: false,
        }
    }

    /// The table key: `name|form|plural|gender|mode|force` with `~` for
    /// unset. `scripts/csl-units/*.cjs` log queries with the same key.
    pub fn key(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}",
            self.name,
            self.form.as_deref().unwrap_or("~"),
            self.plural,
            self.gender.as_deref().unwrap_or("~"),
            self.mode
                .map(|m| m.to_string())
                .unwrap_or_else(|| "~".into()),
            u8::from(self.force_default_locale)
        )
    }
}

/// `state.getTerm(term, form, plural, gender, mode, forceDefaultLocale)` for
/// the query `q`: [`State::get_term`], `None` for JS `undefined`.
pub fn input_get_term(state: &mut State, q: &TermQuery) -> Option<String> {
    // An `Err` is the TypeError of a missing `state.locale[state.opt.lang]`,
    // which the constructor rules out; it reads as `undefined` here.
    state
        .get_term(
            &q.name,
            q.form.as_deref(),
            Some(q.plural),
            q.gender.as_deref(),
            q.mode,
            q.force_default_locale,
        )
        .ok()
        .flatten()
}

/// `getTerm(name)` with only the name given; `None` name is
/// `getTerm(undefined)`, which is `undefined`.
pub fn input_get_term_name(state: &mut State, name: Option<&str>) -> Option<String> {
    name.and_then(|n| input_get_term(state, &TermQuery::new(n)))
}

/// `CSL.Engine.getField(mode, state.locale[state.opt.lang].terms, name, form,
/// plural, gender)` ([`State::get_field`]); `None` is JS `undefined`.
pub fn input_get_field(
    state: &mut State,
    mode: i64,
    name: &str,
    form: &str,
    plural: i64,
    gender: Option<&str>,
) -> Option<String> {
    let lang = opt_lang(state);
    let locale = state.locale.get(&lang)?;
    State::get_field(mode, &locale.terms, name, Some(form), Some(plural), gender)
        .ok()
        .flatten()
        .map(|v| js::to_js_string(&v))
}

fn opt_lang(state: &mut State) -> String {
    state
        .opt
        .get("lang")
        .map(js::to_js_string)
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// CSL.Util.padding
// ---------------------------------------------------------------------------

/// `CSL.Util.padding(num)`: left-pad the first integer in `num` to 20
/// digits (negative numbers become `1e20 + n` as a float, which prints as
/// `100000000000000000000`). Text with no digits is returned unchanged.
pub fn padding(num: &str) -> String {
    static RE: LazyLock<Regex> = LazyLock::new(|| rx(&format!("[{}]*(-?[0-9]+)", js::WS)));
    match RE.captures(num).and_then(|c| c.get(1)) {
        Some(m) => {
            // parseInt gives a double: exact for small values, rounded beyond 2^53.
            let n: f64 = m.as_str().parse::<f64>().unwrap_or(0.0);
            let mut s = if n < 0.0 {
                js_integer_string(99999999999999999999.0_f64 + n)
            } else {
                js_integer_string(n)
            };
            while js::len(&s) < 20 {
                s = format!("0{s}");
            }
            s
        }
        None => num.to_string(),
    }
}

/// JS `"" + f` for an integral double below 1e21: the shortest round-trip
/// digits followed by zeros (`123456789012345680000`).
fn js_integer_string(f: f64) -> String {
    if f.abs() < 9.0e15 {
        return format!("{}", f as i64);
    }
    let e = format!("{f:e}");
    let (mant, exp) = e.split_once('e').unwrap_or((&e, "0"));
    let neg = mant.starts_with('-');
    let digits: String = mant.chars().filter(|c| c.is_ascii_digit()).collect();
    let exp: usize = exp.parse().unwrap_or(0);
    let mut out = digits.clone();
    while out.len() < exp + 1 {
        out.push('0');
    }
    if neg {
        out.insert(0, '-');
    }
    out
}

// ---------------------------------------------------------------------------
// Ordinalizers, Romanizer, Suffixator
// ---------------------------------------------------------------------------

/// `CSL.Util.LongOrdinalizer` (`state.fun.long_ordinalizer`): "first",
/// "second" ... from the locale's `long-ordinal-NN` terms.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LongOrdinalizer {}

impl LongOrdinalizer {
    /// `LongOrdinalizer.prototype.format(num, gender)`. Falls back to
    /// [`Ordinalizer::format`] when the locale has no `long-ordinal-NN` term,
    /// and sets `state.tmp.cite_renders_content = true`.
    pub fn format(state: &mut State, num: &Value, gender: Option<&str>) -> CslResult<String> {
        // if (num < 10) num = "0" + num;
        let n_str = js::to_js_string(num);
        let numeric_lt_10 = match num {
            Value::Number(n) => n.as_f64().map(|f| f < 10.0).unwrap_or(false),
            _ => {
                let t = js::trim(&n_str);
                if t.is_empty() {
                    true // "" < 10: Number("") is 0
                } else {
                    t.parse::<f64>().map(|f| f < 10.0).unwrap_or(false)
                }
            }
        };
        let num_s = if numeric_lt_10 {
            format!("0{n_str}")
        } else {
            n_str
        };
        let ret = input_get_field(
            state,
            LOOSE,
            &format!("long-ordinal-{num_s}"),
            "long",
            0,
            gender,
        );
        let ret = match ret {
            Some(r) if !r.is_empty() => r,
            _ => Ordinalizer::default().format(state, &Value::String(num_s), gender)?,
        };
        // Probably too optimistic -- what if only renders in _sort?
        state.tmp.cite_renders_content = true;
        Ok(ret)
    }
}

/// `CSL.Util.Ordinalizer` (`state.fun.ordinalizer`): "1st", "2nd" ...
///
/// Upstream caches the locale's `ordinal-01..04` suffixes per language in
/// `this.suffixes` (filled by `init()`); this port recomputes them on each
/// call (same answers, since the locale does not change).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ordinalizer {}

/// `["masculine", "feminine"]` genders `init()` tries, with `None` for the
/// ungendered set.
const ORD_GENDERS: [Option<&str>; 3] = [None, Some("masculine"), Some("feminine")];

impl Ordinalizer {
    /// `Ordinalizer.prototype.init()` for one gender: the four suffixes
    /// `ordinal-01..04` (long form), or `None` when one is missing (upstream
    /// then deletes the gender's entry).
    fn suffixes(state: &mut State, gender: Option<&str>) -> Option<Vec<String>> {
        let mut out = Vec::new();
        for j in 1..5 {
            let mut q = TermQuery::new(&format!("ordinal-0{j}"));
            q.form = Some("long".into());
            q.gender = gender.map(str::to_string);
            out.push(input_get_term(state, &q)?);
        }
        Some(out)
    }

    /// `Ordinalizer.prototype.format(num, gender)`: `num` with its ordinal
    /// suffix, per the locale's CSL 1.0.1 `ord` rules when present, else
    /// the four-suffix English-style rule.
    pub fn format(&self, state: &mut State, num: &Value, gender: Option<&str>) -> CslResult<String> {
        let parsed = js::parse_int_value(num);
        let mut s = match parsed {
            Some(n) => n.to_string(),
            None => "NaN".to_string(),
        };
        let mut trygenders: Vec<&str> = Vec::new();
        if let Some(g) = gender {
            trygenders.push(g);
        }
        trygenders.push("neuter");
        // `suffix` is a JS value that may stay undefined (then it is appended
        // as the text "undefined", as upstream does).
        let mut suffix: Option<String>;
        // `state.locale[state.opt.lang].ord["1.0.1"]`
        let lang = opt_lang(state);
        let ord_101: Option<Value> = state
            .locale
            .get(&lang)
            .and_then(|l| l.ord.get("1.0.1"))
            .filter(|v| js::truthy(v))
            .cloned();
        if let Some(ordinfo) = ord_101.as_ref() {
            let get = |state: &mut State, name: &str| -> Option<String> {
                let mut q = TermQuery::new(name);
                q.gender = gender.map(str::to_string);
                input_get_term(state, &q)
            };
            suffix = get(state, "ordinal");
            let len = js::len(&s) as i64;
            let two = js::slice(&s, len - 2, None);
            let one = js::slice(&s, len - 1, None);
            let missing = |key: &str| {
                EngineError::BadInput(format!(
                    "Cannot read properties of undefined (reading '{key}')"
                ))
            };
            for tg in &trygenders {
                let pick = |sec: &Value, key: &str| -> Option<String> {
                    sec.get(key)
                        .filter(|e| js::truthy(e))
                        .and_then(|e| e.get(*tg))
                        .filter(|t| js::truthy(t))
                        .map(js::to_js_string)
                };
                let whole = ordinfo.get("whole-number").ok_or_else(|| missing(&s))?;
                if let Some(t) = pick(whole, &s) {
                    suffix = get(state, &t);
                } else if let Some(t) = pick(
                    ordinfo
                        .get("last-two-digits")
                        .ok_or_else(|| missing(&two))?,
                    &two,
                ) {
                    suffix = get(state, &t);
                } else if let Some(t) = pick(
                    ordinfo.get("last-digit").ok_or_else(|| missing(&one))?,
                    &one,
                ) {
                    suffix = get(state, &t);
                }
                if suffix.as_deref().map(|x| !x.is_empty()).unwrap_or(false) {
                    break;
                }
            }
        } else {
            let suffixes = Self::suffixes(state, gender).ok_or_else(|| {
                EngineError::BadInput("Cannot read properties of undefined (reading '3')".into())
            })?;
            let n = parsed.map(|n| n as f64).unwrap_or(f64::NAN);
            let idx = if (n / 10.0) % 10.0 == 1.0 || (n > 10.0 && n < 20.0) {
                3
            } else if n % 10.0 == 1.0 && n % 100.0 != 11.0 {
                0
            } else if n % 10.0 == 2.0 && n % 100.0 != 12.0 {
                1
            } else if n % 10.0 == 3.0 && n % 100.0 != 13.0 {
                2
            } else {
                3
            };
            suffix = Some(suffixes[idx].clone());
        }
        s.push_str(suffix.as_deref().unwrap_or("undefined"));
        Ok(s)
    }

    /// Which genders `init()` would keep suffix tables for (for tests).
    pub fn init_genders(&self, state: &mut State) -> Vec<Option<String>> {
        ORD_GENDERS
            .iter()
            .filter(|g| Self::suffixes(state, **g).is_some())
            .map(|g| g.map(str::to_string))
            .collect()
    }
}

/// `CSL.Util.Romanizer` (`state.fun.romanizer`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Romanizer {}

impl Romanizer {
    /// `Romanizer.prototype.format(num)`: lower-case roman numerals; `""` for
    /// `num >= 6000`. A character that is not a digit (a minus sign, a
    /// decimal point) yields the string `undefined` in its place, as JS does
    /// (`ROMAN_NUMERALS[pos][NaN]`), and a negative number with more than
    /// four digits reaches an undefined row (a TypeError upstream).
    pub fn format(&self, num: &Value) -> CslResult<String> {
        let f = match num {
            Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
            Value::String(s) => {
                let t = js::trim(s);
                if t.is_empty() {
                    0.0
                } else {
                    t.parse::<f64>().unwrap_or(f64::NAN)
                }
            }
            _ => f64::NAN,
        };
        let mut ret = String::new();
        if f < 6000.0 {
            let numstr: Vec<char> = js::to_js_string(num).chars().rev().collect();
            for (pos, ch) in numstr.iter().enumerate() {
                let row = ROMAN_NUMERALS.get(pos).ok_or_else(|| {
                    let n = ch
                        .to_digit(10)
                        .map(|d| d.to_string())
                        .unwrap_or_else(|| "NaN".to_string());
                    EngineError::BadInput(format!(
                        "Cannot read properties of undefined (reading '{n}')"
                    ))
                })?;
                let piece = match ch.to_digit(10).and_then(|d| row.get(d as usize)) {
                    Some(p) => (*p).to_string(),
                    None => "undefined".to_string(),
                };
                ret = format!("{piece}{ret}");
            }
        }
        Ok(ret)
    }
}

/// `CSL.Util.Suffixator` (`state.fun.suffixator`): a, b, ... z, aa, ab ...
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suffixator {
    /// `slist`: the characters to count with.
    pub slist: Vec<String>,
}

impl Default for Suffixator {
    fn default() -> Self {
        Suffixator::new(None)
    }
}

impl Suffixator {
    /// `new CSL.Util.Suffixator(slist)`: `slist` is a comma-separated list;
    /// `None` (or empty) means `CSL.SUFFIX_CHARS`.
    pub fn new(slist: Option<&str>) -> Suffixator {
        let s = match slist {
            Some(s) if !s.is_empty() => s,
            _ => SUFFIX_CHARS,
        };
        Suffixator {
            slist: s.split(',').map(str::to_string).collect(),
        }
    }

    /// `Suffixator.prototype.format(N)`: the (zero-based) N-th suffix:
    /// `0` is `a`, `25` is `z`, `26` is `aa`. An entry beyond the list is
    /// the string `undefined` (JS `undefined + key`).
    pub fn format(&self, n: i64) -> String {
        let mut n = n + 1;
        let mut key = String::new();
        loop {
            let x = if n % 26 == 0 { 26 } else { n % 26 };
            let piece = self
                .slist
                .get((x - 1) as usize)
                .cloned()
                .unwrap_or_else(|| "undefined".to_string());
            key = format!("{piece}{key}");
            n = (n - x) / 26;
            if n == 0 {
                break;
            }
        }
        key
    }
}

// ---------------------------------------------------------------------------
// processNumber: data model
// ---------------------------------------------------------------------------

/// One parsed value of a numeric variable (an element of
/// `shadow_numbers[var].values`): `composeNumberInfo`'s object plus the
/// flags the later passes set. `None` = the JS property is `undefined`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NumberInfo {
    /// `label`: `"p."`, `"vol."`, `"var:volume"`. Absent if no label.
    pub label: Option<String>,
    /// `origLabel`.
    pub orig_label: Option<String>,
    /// `labelSuffix`.
    pub label_suffix: Option<String>,
    /// `plural` (0 or 1).
    pub plural: Option<i64>,
    /// `labelVisibility`.
    pub label_visibility: Option<bool>,
    /// `gotosleepability`.
    pub gotosleepability: Option<bool>,
    /// `particle`: the leading part of a value like `A12`. `None` is JS
    /// `undefined`, which `manglePageNumbers` can leave behind.
    pub particle: Option<String>,
    /// `value`.
    pub value: String,
    /// `joiningSuffix`: the separator that follows (`"-"`, `", "`, `" & "`).
    pub joining_suffix: String,
    /// `numeric`.
    pub numeric: Option<bool>,
    /// `collapsible`.
    pub collapsible: Option<bool>,
    /// `styling` (only when `processNumber` was given a node).
    pub styling: Option<Token>,
}

impl NumberInfo {
    /// The JS object, keys of undefined properties omitted (`styling` is a
    /// token and is not part of the data dump).
    pub fn to_value(&self) -> Value {
        let mut o = Obj::new();
        let mut put = |k: &str, v: Option<Value>| {
            if let Some(v) = v {
                o.insert(k.to_string(), v);
            }
        };
        put("collapsible", self.collapsible.map(Value::Bool));
        put("gotosleepability", self.gotosleepability.map(Value::Bool));
        put(
            "joiningSuffix",
            Some(Value::String(self.joining_suffix.clone())),
        );
        put("label", self.label.clone().map(Value::String));
        put("labelSuffix", self.label_suffix.clone().map(Value::String));
        put("labelVisibility", self.label_visibility.map(Value::Bool));
        put("numeric", self.numeric.map(Value::Bool));
        put("origLabel", self.orig_label.clone().map(Value::String));
        put("particle", self.particle.clone().map(Value::String));
        put("plural", self.plural.map(Value::from));
        put("value", Some(Value::String(self.value.clone())));
        Value::Object(o)
    }
}

/// An element of `shadow_numbers[var].values`: a parsed value, or the raw
/// `["Blob", value, false]` array `setNumberLabels` pushes.
#[derive(Debug, Clone, PartialEq)]
pub enum ShadowValue {
    /// A [`NumberInfo`] object.
    Info(NumberInfo),
    /// `["Blob", value, false]`.
    Raw(Vec<Value>),
}

impl ShadowValue {
    /// The JS value.
    pub fn to_value(&self) -> Value {
        match self {
            ShadowValue::Info(i) => i.to_value(),
            ShadowValue::Raw(v) => Value::Array(v.clone()),
        }
    }
}

/// The `label` property of a shadow-numbers entry: `false` (from
/// `setNumberLabels`) or a term name; absent/`undefined` is `None`.
#[derive(Debug, Clone, PartialEq)]
pub enum ShadowLabel {
    /// `false`.
    False,
    /// A term name such as `"page"`, `"volume"`.
    Term(String),
}

/// `state.tmp.shadow_numbers[variable]`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShadowNumber {
    /// `values`.
    pub values: Vec<ShadowValue>,
    /// `plural`.
    pub plural: Option<i64>,
    /// `numeric`.
    pub numeric: Option<bool>,
    /// `collapsible`.
    pub collapsible: Option<bool>,
    /// `label`.
    pub label: Option<ShadowLabel>,
    /// `labelForm` (set by `evaluateLabel`, or `"short"` for sleepy pages).
    pub label_form: Option<String>,
    /// `labelCapitalizeIfFirst`.
    pub label_capitalize_if_first: Option<Value>,
    /// `labelDecorations`.
    pub label_decorations: Option<Vec<Decoration>>,
    /// `masterStyling` (node path only).
    pub master_styling: Option<Token>,
}

impl ShadowNumber {
    /// The JS object as data (keys of undefined properties omitted;
    /// `masterStyling` and `labelDecorations` are tokens/decorations and are
    /// included only when set).
    pub fn to_value(&self) -> Value {
        let mut o = Obj::new();
        if let Some(b) = self.collapsible {
            o.insert("collapsible".into(), Value::Bool(b));
        }
        if let Some(l) = &self.label {
            o.insert(
                "label".into(),
                match l {
                    ShadowLabel::False => Value::Bool(false),
                    ShadowLabel::Term(t) => Value::String(t.clone()),
                },
            );
        }
        if let Some(f) = &self.label_form {
            o.insert("labelForm".into(), Value::String(f.clone()));
        }
        if let Some(c) = &self.label_capitalize_if_first {
            o.insert("labelCapitalizeIfFirst".into(), c.clone());
        }
        if let Some(d) = &self.label_decorations {
            o.insert(
                "labelDecorations".into(),
                Value::Array(
                    d.iter()
                        .map(|x| {
                            let mut a = vec![
                                Value::String(x.name.clone()),
                                Value::String(x.value.clone()),
                            ];
                            if let Some(e) = &x.extra {
                                // the third element of `@showid` is the numeric cslid
                                a.push(
                                    e.parse::<i64>()
                                        .map(Value::from)
                                        .unwrap_or_else(|_| Value::String(e.clone())),
                                );
                            }
                            Value::Array(a)
                        })
                        .collect(),
                ),
            );
        }
        if let Some(b) = self.numeric {
            o.insert("numeric".into(), Value::Bool(b));
        }
        if let Some(p) = self.plural {
            o.insert("plural".into(), Value::from(p));
        }
        o.insert(
            "values".into(),
            Value::Array(self.values.iter().map(ShadowValue::to_value).collect()),
        );
        Value::Object(o)
    }
}

/// `JSON.parse(JSON.stringify(values[groupStartPos]))` plus the fields
/// `setPluralsAndNumerics` adds: the group being examined.
#[derive(Debug, Clone)]
struct LabelInfo {
    pos: usize,
    count: i64,
    numeric: bool,
    collapsible: Option<bool>,
    label: Option<String>,
}

// ---------------------------------------------------------------------------
// processNumber: regexes
// ---------------------------------------------------------------------------

/// JS source text of `\s` etc. turned into a Rust pattern: `\s` outside a
/// class becomes the JS whitespace class.
fn js_pattern_to_rust(src: &str) -> String {
    src.replace("\\s", &format!("[{}]", js::WS))
}

/// The three joiner patterns (`joinerMatchRex` source, `joinerSplitRex`
/// source, `andRex`) for a locale whose `and` term is `locale_and`.
fn joiner_sources(lang: &str, locale_and: Option<&str>) -> (String, String) {
    let symbol_and = "\\s*&\\s*";
    let mut fullform_and = String::from(",\\s+and\\s+|\\s+and\\s+");
    if js::slice(lang, 0, Some(2)) != "en" {
        let t = locale_and.unwrap_or("undefined");
        fullform_and.push_str(&format!("|,\\s+{t}\\s+|\\s+{t}\\s+"));
    }
    let tail = ";\\s+|,\\s+|\\s*\\\\*[\\-\\u2013]+\\s*";
    let jm = format!("({symbol_and}|{fullform_and}|{tail})");
    let js_ = format!("(?:{symbol_and}|{fullform_and}|{tail})");
    (jm, js_)
}

static AND_RE: LazyLock<Regex> = LazyLock::new(|| rx(&js_pattern_to_rust("^\\s*&\\s*$")));
static ALPHA_END_RE: LazyLock<Regex> = LazyLock::new(|| rx("[a-zA-Z]$"));
static ALPHA_START_RE: LazyLock<Regex> = LazyLock::new(|| rx("^[a-zA-Z]"));
static FIRST_WORD_RE: LazyLock<Regex> = LazyLock::new(|| rx("^([^ ]+)"));
static LABEL_PARTS_RE: LazyLock<Regex> =
    LazyLock::new(|| rx(&format!("([{ws}]*)([^{ws}]+)([{ws}]*)", ws = js::WS)));
static PARTICLE_VALUE_RE: LazyLock<Regex> =
    LazyLock::new(|| rx("^([0-9]*[a-zA-Z]+0*)?([0-9]+(?:[a-zA-Z]*|[-,a-zA-Z]+))$"));
static FIRST_DASH_RE: LazyLock<Regex> = LazyLock::new(|| rx(&js_pattern_to_rust("\\s*-\\s*")));
static SUBSECTION_RE: LazyLock<Regex> = LazyLock::new(|| {
    rx("^(?:(?:[a-z]|[a-z][a-z]|[a-z][a-z][a-z]|[a-z][a-z][a-z][a-z])\\.  *)*[0-9]+[,a-zA-Z]+$")
});
static LETTERS_COMMA_RE: LazyLock<Regex> = LazyLock::new(|| rx("^[,a-zA-Z]+$"));
const LABEL_WORDS: &str = "(?:^| )(?:[a-z]|[a-z][a-z]|[a-z][a-z][a-z]|[a-z][a-z][a-z][a-z]|subpara|subch|amend|bibliog|annot|illus|princ|intro|sched|subdiv|subsec)(?:\\.| ) *";
static LABEL_FIND_RE: LazyLock<Regex> = LazyLock::new(|| rx(LABEL_WORDS));
static NUM_TAIL_RE: LazyLock<Regex> = LazyLock::new(|| rx("^[0-9]+([-;,:a-zA-Z]*)$"));
static MVAL_RE: LazyLock<Regex> = LazyLock::new(|| rx("^[0-9]+([-,:a-zA-Z]*)$"));
static MCUR_RE: LazyLock<Regex> =
    LazyLock::new(|| rx("^(?:[0-9]+|[ixv]+)([-,:a-zA-Z]*|\\-[\\-0-9]+)$"));
static ROMAN_ONLY_RE: LazyLock<Regex> = LazyLock::new(|| rx("^[ivxlcmIVXLCM]+$"));
static LEGAL_HYPHEN_RE: LazyLock<Regex> = LazyLock::new(|| rx("[\\\\]*-"));
static FRACTION_RE: LazyLock<Regex> = LazyLock::new(|| rx("^[0-9]+(?:/[0-9]+)+$"));
static PAGE_MANGLE_RE: LazyLock<Regex> = LazyLock::new(|| {
    rx(&js_pattern_to_rust(
        "^((?:[0-9]*[a-zA-Z]+0*))?([0-9]+[a-z]*)(\\s*[^0-9]+\\s*)([-,a-zA-Z]?0*)([0-9]+[a-z]*)$",
    ))
});
static NUM_OR_ROMAN_RE: LazyLock<Regex> = LazyLock::new(|| rx("^([0-9]+|[ivxlcmIVXLCM]+)$"));

/// JS `parseInt(s)` with no radix: like radix 10 except that a `0x`/`0X`
/// prefix means hexadecimal.
fn parse_int_auto(s: &str) -> Option<i64> {
    let t = s.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
    let (neg, rest) = match t.chars().next() {
        Some('-') => (true, &t[1..]),
        Some('+') => (false, &t[1..]),
        _ => (false, t),
    };
    if rest.starts_with("0x") || rest.starts_with("0X") {
        let digits: String = rest[2..]
            .chars()
            .take_while(|c| c.is_ascii_hexdigit())
            .collect();
        if digits.is_empty() {
            return None;
        }
        let n = i64::from_str_radix(&digits, 16).unwrap_or(i64::MAX);
        return Some(if neg { -n } else { n });
    }
    js::parse_int(t)
}

// ---------------------------------------------------------------------------
// processNumber: the parser
// ---------------------------------------------------------------------------

/// `normalizeFieldValue(str)`: prepend the variable's own label to a value
/// that does not start with one (`"12"` for `volume` becomes `"vol. 12"`).
fn normalize_field_value(item: &Value, variable: &str, s: &str) -> String {
    let mut str_ = js::trim(s).to_string();
    if let Some(m) = FIRST_WORD_RE.captures(&str_) {
        let first = m.get(1).map(|x| x.as_str()).unwrap_or("");
        if statute_subdiv_string(first).is_none() {
            let embedded = if ["locator", "locator-extra", "page"].contains(&variable) {
                match item.get("label").filter(|l| js::truthy(l)) {
                    Some(l) => statute_subdiv_string_reverse(&js::to_js_string(l)),
                    None => Some("p."),
                }
            } else {
                statute_subdiv_string_reverse(variable)
            };
            if let Some(e) = embedded {
                str_ = format!("{e} {str_}");
            }
        }
    }
    str_
}

/// `composeNumberInfo(origLabel, label, val, joiningSuffix, parsePosition)`.
fn compose_number_info(
    variable: &str,
    real_variable: &str,
    orig_label: &str,
    label: &str,
    val: &str,
    joining_suffix: Option<&str>,
    parse_position: Option<usize>,
) -> CslResult<NumberInfo> {
    let joining_suffix = joining_suffix.unwrap_or("");
    let mut info = NumberInfo::default();
    let mut label = label.to_string();
    if label.is_empty() && statute_subdiv_string_reverse(variable).is_none() {
        label = format!("var:{variable}");
    }
    if !label.is_empty() {
        let m = LABEL_PARTS_RE.captures(&label).ok_or_else(|| {
            EngineError::BadInput("Cannot read properties of null (reading '2')".into())
        })?;
        let m2 = m.get(2).map(|x| x.as_str()).unwrap_or("");
        let m3 = m.get(3).map(|x| x.as_str()).unwrap_or("");
        if real_variable == "page" && parse_position == Some(0) && !["p.", "pp."].contains(&m2) {
            info.gotosleepability = Some(true);
            info.label_visibility = Some(true);
        } else {
            info.label_visibility = Some(false);
        }
        info.label = Some(m2.to_string());
        info.orig_label = Some(orig_label.to_string());
        info.label_suffix = Some(m3.to_string());
        info.plural = Some(0);
    }
    match PARTICLE_VALUE_RE.captures(val) {
        Some(m) => {
            info.particle = Some(m.get(1).map(|x| x.as_str()).unwrap_or("").to_string());
            info.value = m.get(2).map(|x| x.as_str()).unwrap_or("").to_string();
        }
        None => {
            info.particle = Some(String::new());
            info.value = val.to_string();
        }
    }
    info.joining_suffix = FIRST_DASH_RE.replace(joining_suffix, "-").into_owned();
    Ok(info)
}

/// `fixupSubsections(elems)`: recombine things like `12a-c`, which a hyphen
/// split but which are one numeric value.
fn fixup_subsections(mut elems: Vec<String>) -> Vec<String> {
    let mut i = elems.len() as i64 - 2;
    while i > -1 {
        let iu = i as usize;
        if iu >= 1
            && iu + 1 < elems.len()
            && elems[iu] == "-"
            && SUBSECTION_RE.is_match(&elems[iu - 1])
            && LETTERS_COMMA_RE.is_match(&elems[iu + 1])
        {
            let joined = elems[iu - 1..iu + 2].join("");
            elems[iu - 1] = joined;
            let mut next = elems[..iu].to_vec();
            next.extend_from_slice(&elems[iu + 2..]);
            elems = next;
        }
        i -= 2;
    }
    elems
}

/// The compiled joiner regexps for one call (`joinerMatchRex`,
/// `joinerSplitRex`, and the `\-`-stripped variants).
struct Joiners {
    jm: Regex,
    js: Regex,
    jm_escaped: Option<Regex>,
    js_escaped: Option<Regex>,
}

fn compile_joiner(src: &str) -> CslResult<Regex> {
    Regex::new(&js_pattern_to_rust(src))
        .map_err(|e| EngineError::BadInput(format!("Invalid regular expression: {e}")))
}

fn build_joiners(state: &mut State) -> CslResult<(Joiners, String)> {
    let lang = opt_lang(state);
    let and_term = input_get_term_name(state, Some("and"));
    let (jm_src, js_src) = joiner_sources(&lang, and_term.as_deref());
    // localeAnd / localeAmpersand
    let locale_and = and_term;
    let mut q = TermQuery::new("and");
    q.form = Some("symbol".into());
    let mut locale_amp = input_get_term(state, &q);
    if locale_and == locale_amp {
        locale_amp = Some("&".into());
    }
    let strip = |s: &str| s.replacen("\\-", "", 1);
    Ok((
        Joiners {
            jm: compile_joiner(&jm_src)?,
            js: compile_joiner(&js_src)?,
            jm_escaped: Some(compile_joiner(&strip(&jm_src))?),
            js_escaped: Some(compile_joiner(&strip(&js_src))?),
        },
        locale_amp.unwrap_or_else(|| "undefined".to_string()),
    ))
}

/// `parseString(str, defaultLabel)`.
#[allow(clippy::too_many_arguments)]
fn parse_string(
    state: &mut State,
    joiners: &Joiners,
    locale_amp: &str,
    item: &Value,
    variable: &str,
    real_variable: &str,
    s: &str,
    default_label: &str,
) -> CslResult<Vec<NumberInfo>> {
    let mut str_ = normalize_field_value(item, variable, s);
    if variable == "page" && str_.contains('\u{2013}') {
        str_ = str_.replace('\u{2013}', "-");
    }
    let (jmrex, jsrex, mystr): (&Regex, &Regex, String);
    if str_.contains("\\-") {
        jmrex = joiners.jm_escaped.as_ref().unwrap_or(&joiners.jm);
        jsrex = joiners.js_escaped.as_ref().unwrap_or(&joiners.js);
        let lst: Vec<String> = str_
            .split("\\-")
            .map(|p| p.replace('-', "\u{2013}"))
            .collect();
        mystr = lst.join("\\-").replace('\\', "");
    } else {
        jmrex = &joiners.jm;
        jsrex = &joiners.js;
        mystr = str_.clone();
    }
    // Split chunks and collate delimiters.
    let mut m: Vec<String> = jmrex
        .find_iter(&mystr)
        .map(|x| x.as_str().to_string())
        .collect();
    let mut elems: Vec<String>;
    if !m.is_empty() {
        let lst: Vec<String> = js::split(jsrex, &mystr);
        for i in 0..m.len() {
            if AND_RE.is_match(&m[i]) {
                let piece = lst.get(i).map(String::as_str).unwrap_or("");
                if ALPHA_END_RE.is_match(piece) && ALPHA_START_RE.is_match(piece) {
                    m[i] = locale_amp.to_string();
                } else {
                    m[i] = format!(" {locale_amp} ");
                }
            }
        }
        // (the `recombine` loop upstream never sets its flag; nothing to do)
        elems = Vec::new();
        for i in 0..lst.len().saturating_sub(1) {
            elems.push(lst[i].clone());
            elems.push(m.get(i).cloned().unwrap_or_else(|| "undefined".to_string()));
        }
        elems.push(lst.last().cloned().unwrap_or_default());
        elems = fixup_subsections(elems);
    } else {
        elems = vec![mystr.clone()];
    }
    // Split elements within each chunk, build the list of value objects.
    let mut values: Vec<NumberInfo> = Vec::new();
    let mut label = default_label.to_string();
    let mut orig_label = String::new();
    let mut i = 0usize;
    while i < elems.len() {
        let mut mm: Vec<String> = LABEL_FIND_RE
            .find_iter(&elems[i])
            .map(|x| x.as_str().to_string())
            .collect();
        if !mm.is_empty() {
            let mut lst: Vec<String> = js::split(&LABEL_FIND_RE, &elems[i]);
            // Head off disaster by merging parsed labels on non-numeric
            // values into content.
            let mut j = lst.len() as i64 - 1;
            while j > 0 {
                let ju = j as usize;
                if !lst[ju - 1].is_empty()
                    && (!NUM_TAIL_RE.is_match(&lst[ju]) || !NUM_TAIL_RE.is_match(&lst[ju - 1]))
                {
                    lst[ju - 1] = format!("{}{}{}", lst[ju - 1], mm[ju - 1], lst[ju]);
                    lst.remove(ju);
                    mm.remove(ju - 1);
                }
                j -= 1;
            }
            // merge bad leading label into content
            if !mm.is_empty() {
                let slug = js::trim(&mm[0]).to_string();
                let sub = statute_subdiv_string(&slug);
                let not_a_label = sub.is_none()
                    || input_get_term_name(state, sub).is_none()
                    || (!["locator", "number", "locator-extra", "page"].contains(&variable)
                        && sub != Some(variable));
                if not_a_label {
                    if i == 0 {
                        mm.remove(0);
                        let second = lst.get(1).cloned().unwrap_or_default();
                        lst[0] = format!("{} {} {}", lst[0], slug, second);
                        lst.remove(1);
                    }
                } else {
                    orig_label = slug;
                }
            }
            let n = lst.len();
            for j in 0..n {
                if !lst[j].is_empty() || j == n - 1 {
                    if j >= 1 {
                        if let Some(l) = mm.get(j - 1).filter(|l| !l.is_empty()) {
                            label = l.clone();
                        }
                    }
                    let filtered = if orig_label == js::trim(&label) {
                        String::new()
                    } else {
                        orig_label.clone()
                    };
                    let mystr_j = if lst[j].is_empty() {
                        String::new()
                    } else {
                        js::trim(&lst[j]).to_string()
                    };
                    let joiner = if j == n - 1 {
                        elems.get(i + 1).map(String::as_str)
                    } else {
                        None
                    };
                    values.push(compose_number_info(
                        variable,
                        real_variable,
                        &filtered,
                        &label,
                        &mystr_j,
                        joiner,
                        Some(i),
                    )?);
                }
            }
        } else {
            let filtered = if orig_label == js::trim(&label) {
                String::new()
            } else {
                orig_label.clone()
            };
            values.push(compose_number_info(
                variable,
                real_variable,
                &filtered,
                &label,
                &elems[i],
                elems.get(i + 1).map(String::as_str),
                None,
            )?);
        }
        i += 2;
    }
    Ok(values)
}

/// `setSpaces(values)`: a value with no joiner followed by a labelled value
/// is joined with a space.
fn set_spaces(values: &mut [NumberInfo]) {
    for i in 0..values.len().saturating_sub(1) {
        if values[i].joining_suffix.is_empty()
            && values[i + 1]
                .label
                .as_deref()
                .map(|l| !l.is_empty())
                .unwrap_or(false)
        {
            values[i].joining_suffix = " ".to_string();
        }
    }
}

/// `fixNumericAndCount(values, i, currentLabelInfo)`.
fn fix_numeric_and_count(values: &mut [NumberInfo], i: usize, cli: &mut LabelInfo) {
    let master_joining = values[cli.pos].joining_suffix.clone();
    let master_value = values[cli.pos].value.clone();
    let val = values[i].value.clone();
    let is_escaped_hyphen = master_joining == "\\-";
    // `if (val.particle && ...)`: val is a string, so val.particle is
    // undefined and this never fires (upstream quirk).
    let m_val = MVAL_RE.captures(&val);
    let m_cur = MCUR_RE.captures(&master_value);
    if val.is_empty() || m_val.is_none() || m_cur.is_none() || is_escaped_hyphen {
        cli.collapsible = Some(false);
        if val.is_empty() || m_cur.is_none() {
            cli.numeric = false;
        }
        if is_escaped_hyphen {
            cli.count -= 1;
        }
    }
    let g1 = |c: &Option<regex::Captures>| -> bool {
        c.as_ref()
            .and_then(|c| c.get(1))
            .map(|m| !m.as_str().is_empty())
            .unwrap_or(false)
    };
    if g1(&m_val) || g1(&m_cur) {
        cli.collapsible = Some(false);
    }
    if values[i].collapsible.is_none() {
        let end = (i as i64 + cli.count).max(i as i64) as usize;
        for j in i..end {
            if j >= values.len() {
                break;
            }
            let v = &values[j].value;
            let nan = parse_int_auto(v).is_none();
            values[j].collapsible = Some(!(nan && !ROMAN_ONLY_RE.is_match(v)));
        }
        cli.collapsible = values[i].collapsible;
    }
    let is_collapsible = cli.collapsible == Some(true);
    let end = (cli.pos as i64 + cli.count).max(cli.pos as i64) as usize;
    for j in cli.pos..end {
        if j >= values.len() {
            break;
        }
        if cli.count > 1 && is_collapsible {
            values[j].plural = Some(1);
        }
        values[j].numeric = Some(cli.numeric);
        values[j].collapsible = cli.collapsible;
    }
}

/// `fixLabelVisibility(values, groupStartPos, currentLabelInfo)`.
fn fix_label_visibility(
    state: &mut State,
    variable: &str,
    values: &mut [NumberInfo],
    cli: &LabelInfo,
) -> CslResult<()> {
    let label = cli.label.as_deref().ok_or_else(|| {
        EngineError::BadInput("Cannot read properties of undefined (reading 'slice')".into())
    })?;
    if js::slice(label, 0, Some(4)) != "var:" {
        if cli.pos == 0 {
            if ["locator", "number", "locator-extra", "page"].contains(&variable) {
                if input_get_term_name(state, statute_subdiv_string(label)).is_none() {
                    values[cli.pos].label_visibility = Some(true);
                }
            }
            if !["locator", "number", "locator-extra", "page"].contains(&variable)
                && statute_subdiv_string(label) != Some(variable)
            {
                values[0].label_visibility = Some(true);
            }
        } else {
            values[cli.pos].label_visibility = Some(true);
        }
    }
    Ok(())
}

/// `setPluralsAndNumerics(values)`.
fn set_plurals_and_numerics(
    state: &mut State,
    item: &Value,
    variable: &str,
    real_variable: &str,
    values: &mut [NumberInfo],
) -> CslResult<()> {
    if values.is_empty() {
        return Ok(());
    }
    let mut group_start = 0usize;
    let mut group_count: i64 = 1;
    let mk = |values: &[NumberInfo], start: usize, count: i64| LabelInfo {
        pos: start,
        count,
        numeric: true,
        collapsible: values[start].collapsible,
        label: values[start].label.clone(),
    };
    for i in 1..values.len() {
        let same_label = values[i - 1].label == values[i].label;
        // `lastVal.particle === lastVal.particle` is always true (upstream).
        if same_label {
            group_count += 1;
        } else {
            let mut cli = mk(values, group_start, group_count);
            fix_numeric_and_count(values, group_start, &mut cli);
            if values[i - 1].label != values[i].label {
                fix_label_visibility(state, variable, values, &cli)?;
            }
            group_start = i;
            group_count = 1;
        }
    }
    let mut cli = mk(values, group_start, group_count);
    fix_numeric_and_count(values, group_start, &mut cli);
    fix_label_visibility(state, variable, values, &cli)?;
    if values[0].numeric == Some(true) && js::slice(variable, 0, Some(10)) == "number-of-" {
        let n = item
            .get(real_variable)
            .and_then(js::parse_int_value)
            .unwrap_or(i64::MIN);
        if n > 1 {
            values[0].plural = Some(1);
        }
    }
    Ok(())
}

/// `stripHyphenBackslash(joiningSuffix)`: first `\-` becomes `-`.
fn strip_hyphen_backslash(s: &str) -> String {
    s.replacen("\\-", "-", 1)
}

/// `checkTerm(variable, val)`.
fn check_term(state: &mut State, variable: &str, val: &NumberInfo) -> bool {
    if ["locator", "locator-extra", "page"].contains(&variable) {
        let label = match val.orig_label.as_deref().filter(|l| !l.is_empty()) {
            Some(o) => o,
            None => val.label.as_deref().unwrap_or(""),
        };
        input_get_term_name(state, statute_subdiv_string(label))
            .map(|t| !t.is_empty())
            .unwrap_or(false)
    } else {
        true
    }
}

/// `checkPage(variable, val)`.
fn check_page(variable: &str, val: &NumberInfo) -> bool {
    variable == "page"
        || (["locator", "locator-extra"].contains(&variable)
            && (val.label.as_deref() == Some("p.") || val.orig_label.as_deref() == Some("p.")))
}

/// `fixupRangeDelimiter(variable, val, rangeDelimiter, isNumeric)`.
fn fixup_range_delimiter(
    state: &mut State,
    variable: &str,
    val: &NumberInfo,
    range_delimiter: &str,
    is_numeric: bool,
) -> String {
    let is_page = check_page(variable, val);
    let has_term = check_term(state, variable, val);
    let mut rd = range_delimiter.to_string();
    if has_term && rd == "-" && is_numeric {
        if is_page
            || [
                "locator",
                "locator-extra",
                "issue",
                "volume",
                "edition",
                "number",
            ]
            .contains(&variable)
        {
            rd = input_get_term_name(state, Some("page-range-delimiter"))
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| "\u{2013}".to_string());
        }
        if variable == "collection-number" {
            rd = input_get_term_name(state, Some("year-range-delimiter"))
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| "\u{2013}".to_string());
        }
    }
    rd
}

/// `manglePageNumbers(values, i, currentInfo)`.
fn mangle_page_numbers(
    state: &mut State,
    variable: &str,
    values: &mut [NumberInfo],
    i: usize,
    count: &mut i64,
) -> CslResult<()> {
    if i < 1 || *count != 2 {
        return Ok(());
    }
    if values[i - 1].particle != values[i].particle {
        return Ok(());
    }
    if values[i - 1].joining_suffix != "-" {
        *count = 1;
        return Ok(());
    }
    let page_range_format = js::truthy_opt(state.opt.get("page-range-format"));
    let a = parse_int_auto(&values[i - 1].value);
    let b = parse_int_auto(&values[i].value);
    if !page_range_format
        && matches!((js::parse_int(&values[i - 1].value), js::parse_int(&values[i].value)), (Some(x), Some(y)) if x > y)
    {
        let jd = fixup_range_delimiter(
            state,
            variable,
            &values[i],
            &values[i - 1].joining_suffix,
            true,
        );
        values[i - 1].joining_suffix = jd;
        return Ok(());
    }
    let is_page = check_page(variable, &values[i]);
    let s: String;
    if is_page && a.is_some() && b.is_some() {
        let joined = format!(
            "{}{} - {}{}",
            values[i - 1].particle.as_deref().unwrap_or("undefined"),
            values[i - 1].value,
            values[i].particle.as_deref().unwrap_or("undefined"),
            values[i].value
        );
        // `me.fun.page_mangler(str)` (one argument: `isyear` is undefined).
        let mangler = state.fun.page_mangler.clone();
        s = mangler.mangle(&joined, false)?;
    } else {
        if NUM_OR_ROMAN_RE.is_match(&values[i - 1].value)
            && NUM_OR_ROMAN_RE.is_match(&values[i].value)
        {
            values[i - 1].joining_suffix = input_get_term_name(state, Some("page-range-delimiter"))
                .unwrap_or_else(|| "undefined".into());
        }
        s = format!(
            "{}{}{}",
            values[i - 1].value,
            strip_hyphen_backslash(&values[i - 1].joining_suffix),
            values[i].value
        );
    }
    if let Some(m) = PAGE_MANGLE_RE.captures(&s) {
        let g = |n: usize| m.get(n).map(|x| x.as_str().to_string());
        let range_delimiter = g(3).unwrap_or_default();
        let range_delimiter = fixup_range_delimiter(
            state,
            variable,
            &values[i],
            &range_delimiter,
            values[i].numeric == Some(true),
        );
        values[i - 1].particle = g(1);
        values[i - 1].value = g(2).unwrap_or_default();
        values[i - 1].joining_suffix = range_delimiter;
        values[i].particle = g(4);
        values[i].value = g(5).unwrap_or_default();
    }
    *count = 0;
    Ok(())
}

/// `fixRanges(values)` (only with a node): collapse `12-15` style ranges.
/// Page ranges go through `state.fun.page_mangler`.
pub fn fix_ranges(
    state: &mut State,
    variable: &str,
    node_given: bool,
    values: &mut [NumberInfo],
) -> CslResult<()> {
    if !node_given {
        return Ok(());
    }
    if ![
        "page",
        "chapter-number",
        "collection-number",
        "edition",
        "issue",
        "number",
        "number-of-pages",
        "number-of-volumes",
        "volume",
        "locator",
        "locator-extra",
    ]
    .contains(&variable)
    {
        return Ok(());
    }
    let mut count: i64 = 0;
    // currentInfo.label: None is JS null (never equal to any label, not even undefined).
    let mut label: Option<Option<String>> = None;
    for i in 0..values.len() {
        if values[i].collapsible != Some(true) {
            count = 0;
            label = None;
            let is_numeric = values[i].numeric == Some(true);
            let v = values[i].clone();
            values[i].joining_suffix =
                fixup_range_delimiter(state, variable, &v, &v.joining_suffix, is_numeric);
        } else if label.as_ref() == Some(&values[i].label) && values[i].joining_suffix == "-" {
            count = 1;
        } else if label.as_ref() == Some(&values[i].label) && values[i].joining_suffix != "-" {
            count += 1;
            if count == 2 {
                mangle_page_numbers(state, variable, values, i, &mut count)?;
            }
        } else if label.as_ref() != Some(&values[i].label) {
            label = Some(values[i].label.clone());
            count = 1;
        } else {
            count = 1;
            label = Some(values[i].label.clone());
        }
    }
    if count == 2 {
        let last = values.len() - 1;
        mangle_page_numbers(state, variable, values, last, &mut count)?;
    }
    Ok(())
}

/// `setStyling(values)` (only with a node): attach a styling token to each
/// value and return the master styling token. Quotation marks around the
/// whole value move into the master styling as an `@quotes` decoration.
///
/// `gender` and `formatter` are copied through `Token::extra`.
pub fn set_styling(state: &mut State, node: &Token, values: &mut [NumberInfo]) -> Token {
    let just_looking = state.tmp.just_looking;
    let mut master_node = node.clone_token();
    let mut master_styling = Token::new("", TokenType::Start);
    if !just_looking {
        master_styling.decorations = std::mem::take(&mut master_node.decorations);
        master_styling.strings.insert(
            "prefix".into(),
            master_node
                .strings
                .get("prefix")
                .cloned()
                .unwrap_or(Value::Null),
        );
        master_node.set_string("prefix", "");
        master_styling.strings.insert(
            "suffix".into(),
            master_node
                .strings
                .get("suffix")
                .cloned()
                .unwrap_or(Value::Null),
        );
        master_node.set_string("suffix", "");
    }
    let master_label = values.first().map(|v| v.label.clone());
    if !values.is_empty() {
        for v in values.iter_mut() {
            let mut newnode = master_node.clone_token();
            if let Some(g) = node.extra.get("gender") {
                newnode.extra.insert("gender".into(), g.clone());
            }
            // `newnode.formatter = node.formatter` (the formatter is kept in
            // `extra["formatter"]`, see obj_number.rs).
            if master_label.as_ref() == Some(&v.label) {
                if let Some(f) = node.extra.get("formatter") {
                    newnode.extra.insert("formatter".into(), f.clone());
                }
            }
            // `if (val.numeric) newnode.successor_prefix = val.successor_prefix`:
            // nothing in util_number.js sets `val.successor_prefix`, so this
            // assigns `undefined` to a property `cloneToken` never copies.
            let suffix = newnode.string("suffix") + &strip_hyphen_backslash(&v.joining_suffix);
            newnode.set_string("suffix", &suffix);
            v.styling = Some(newnode);
        }
        if !just_looking {
            let first_quoted = js::slice(&values[0].value, 0, Some(1)) == "\"";
            let last = values.len() - 1;
            let last_quoted = js::slice(&values[last].value, -1, None) == "\"";
            if first_quoted && last_quoted {
                values[0].value = js::slice(&values[0].value, 1, None);
                values[last].value = js::slice(&values[last].value, 0, Some(-1));
                master_styling
                    .decorations
                    .push(Decoration::new("@quotes", "true"));
            }
        }
    }
    master_styling
}

// ---------------------------------------------------------------------------
// processNumber
// ---------------------------------------------------------------------------

/// `JS: normval = this.sys.normalizeAbbrevsKey(realVariable, val)` plus the
/// `loadAbbreviation` / `abbrevs[jurisdiction].number[normval]` lookup:
/// the abbreviation for `val` in the `number` category, or `None`. See
/// [`super::build_retrieve_item::abbreviation_lookup`].
fn number_abbreviation(
    state: &mut State,
    item: &Value,
    real_variable: &str,
    val: &str,
) -> CslResult<Option<String>> {
    let normval = super::build_retrieve_item::normalize_abbrevs_key(real_variable, Some(val));
    super::build_retrieve_item::abbreviation_lookup(
        state,
        item.get("jurisdiction")
            .filter(|j| js::truthy(j))
            .map(js::to_js_string)
            .as_deref(),
        "number",
        &normval,
    )
}

/// `CSL.Engine.prototype.processNumber(node, ItemObject, variable)`.
///
/// Parses `ItemObject[variable]` (`variable` may be `"page-first"`) into
/// `state.tmp.shadow_numbers[variable]`: its `values` (value, particle,
/// label, joining suffix, flags) and the entry's `numeric` / `collapsible`
/// / `plural` / `label`. With `node = None` that is all (the input path the
/// intermediate dump records). With a node, ranges are mangled and styling
/// tokens attached (see the module docs for what is deferred).
///
/// `item` is `None` for JS's falsy `ItemObject` (the entry is created
/// empty). The multilingual transform and the abbreviation step are done
/// in-line, see [`number_abbreviation`] and the `LangPrefsMap` branch.
pub fn process_number(
    state: &mut State,
    node: Option<&Token>,
    item: Option<&Value>,
    variable: &str,
) -> CslResult<()> {
    let real_variable = variable.to_string();
    let variable = if variable == "page-first" {
        "page"
    } else {
        variable
    };
    let mut sn = state
        .tmp
        .shadow_numbers
        .remove(&real_variable)
        .unwrap_or_default();
    let existed_with_values = !sn.values.is_empty();
    let res = process_number_inner(
        state,
        node,
        item,
        variable,
        &real_variable,
        &mut sn,
        existed_with_values,
    );
    state.tmp.shadow_numbers.insert(real_variable, sn);
    res
}

fn process_number_inner(
    state: &mut State,
    node: Option<&Token>,
    item: Option<&Value>,
    variable: &str,
    real_variable: &str,
    sn: &mut ShadowNumber,
    existed_with_values: bool,
) -> CslResult<()> {
    let (joiners, locale_amp) = build_joiners(state)?;

    // short-circuit if object exists: if numeric, set styling, no other action
    if let Some(n) = node {
        if existed_with_values {
            let mut infos = take_infos(&mut sn.values);
            fix_ranges(state, variable, true, &mut infos)?;
            sn.master_styling = Some(set_styling(state, n, &mut infos));
            sn.values = infos.into_iter().map(ShadowValue::Info).collect();
            return Ok(());
        }
    }

    let item = match item {
        Some(i) if js::truthy(i) => i,
        _ => return Ok(()),
    };

    // Possibly apply multilingual transform
    let mut val: Value;
    if let Some(role) = lang_prefs_map(variable) {
        let prefs = state
            .opt
            .get("cite-lang-prefs")
            .and_then(|p| p.get(role))
            .ok_or_else(|| {
                EngineError::BadInput("Cannot read properties of undefined (reading '0')".into())
            })?;
        let locale_type = prefs
            .as_array()
            .and_then(|a| a.first())
            .map(js::to_js_string)
            .unwrap_or_else(|| "undefined".to_string());
        val = text_sub_field_name(state, item, real_variable, &format!("locale-{locale_type}"));
    } else {
        val = item.get(real_variable).cloned().unwrap_or(Value::Null);
    }

    if js::truthy(&val)
        && real_variable == "number"
        && item.get("type").and_then(Value::as_str) == Some("legal_case")
    {
        match &val {
            Value::String(s) => {
                val = Value::String(LEGAL_HYPHEN_RE.replace_all(s, "\\-").into_owned());
            }
            _ => {
                return Err(EngineError::BadInput(
                    "val.replace is not a function".into(),
                ));
            }
        }
    }

    // XXX HOLDING THIS: apply short form (abbreviation) to a numeric value.
    if js::truthy(&val) {
        let sval = js::to_js_string(&val);
        if let Some(abbr) = number_abbreviation(state, item, real_variable, &sval)? {
            val = Value::String(abbr);
        }
    }

    // Process only if there is a value.
    if val.is_string() || val.is_number() {
        let vs = js::to_js_string(&val);
        let default_label = statute_subdiv_string_reverse(variable).unwrap_or("");

        if sn.values.is_empty() {
            let mut values = parse_string(
                state,
                &joiners,
                &locale_amp,
                item,
                variable,
                real_variable,
                &vs,
                default_label,
            )?;
            set_spaces(&mut values);
            set_plurals_and_numerics(state, item, variable, real_variable, &mut values)?;
            for o in values.iter_mut() {
                if o.numeric != Some(true) {
                    o.plural = Some(0);
                }
            }
            if let Some(n) = node {
                fix_ranges(state, variable, true, &mut values)?;
                sn.master_styling = Some(set_styling(state, n, &mut values));
            }
            // setVariableParams
            if let Some(v0) = values.first() {
                sn.numeric = v0.numeric;
                sn.collapsible = v0.collapsible;
                sn.plural = v0.plural;
                sn.label = v0
                    .label
                    .as_deref()
                    .and_then(statute_subdiv_string)
                    .map(|t| ShadowLabel::Term(t.to_string()));
                if variable == "number"
                    && sn.label == Some(ShadowLabel::Term("issue".into()))
                    && input_get_term_name(state, Some("number"))
                        .map(|t| !t.is_empty())
                        .unwrap_or(false)
                {
                    sn.label = Some(ShadowLabel::Term("number".into()));
                }
            }
            sn.values = values.into_iter().map(ShadowValue::Info).collect();
        }

        // hack in support for non-numeric numerics like "91 Civ. 5442 (RPP)|91 Civ. 5471"
        if variable == "number" && sn.values.len() == 1 {
            match &mut sn.values[0] {
                ShadowValue::Info(v0) => {
                    if v0.value.contains('|') {
                        v0.value = v0.value.replace('|', ", ");
                        v0.numeric = Some(true);
                        v0.plural = Some(1);
                        v0.collapsible = Some(false);
                        sn.numeric = Some(true);
                        sn.plural = Some(1);
                        sn.collapsible = Some(false);
                    }
                }
                ShadowValue::Raw(_) => {
                    return Err(EngineError::BadInput(
                        "Cannot read properties of undefined (reading 'indexOf')".into(),
                    ));
                }
            }
        }
        if sn.values.len() == 1 {
            match &mut sn.values[0] {
                ShadowValue::Info(v0) => {
                    if FRACTION_RE.is_match(&v0.value) {
                        v0.numeric = Some(true);
                        v0.plural = Some(0);
                        v0.collapsible = Some(false);
                        sn.numeric = Some(true);
                        sn.plural = Some(0);
                        sn.collapsible = Some(false);
                    }
                }
                ShadowValue::Raw(_) => {
                    return Err(EngineError::BadInput(
                        "Cannot read properties of undefined (reading 'match')".into(),
                    ));
                }
            }
        }
        if variable == "page" && !sn.values.is_empty() {
            if let ShadowValue::Info(v0) = &sn.values[0] {
                if v0.gotosleepability == Some(true) {
                    sn.label_form = Some("short".into());
                }
            }
        }
    }
    Ok(())
}

fn take_infos(values: &mut Vec<ShadowValue>) -> Vec<NumberInfo> {
    std::mem::take(values)
        .into_iter()
        .filter_map(|v| match v {
            ShadowValue::Info(i) => Some(i),
            ShadowValue::Raw(_) => None,
        })
        .collect()
}

/// `this.transform.getTextSubField(ItemObject, field, locale_type, true).name`
/// as `processNumber` uses it, for the plain (non-title, non-`-short`)
/// fields `LangPrefsMap` lists for numbers (`number`, `edition`, `issue`,
/// `volume`). `Value::Null` is JS `""`/falsy absence.
///
/// DUP-CHECK: util_transform.js `getTextSubField` (reduced; the full
/// function belongs to the transform port).
fn text_sub_field_name(state: &mut State, item: &Value, field: &str, locale_type: &str) -> Value {
    let item_field = item.get(field).cloned().unwrap_or(Value::Null);
    if !js::truthy(&item_field) {
        return Value::String(String::new());
    }
    let opts: Vec<String> = state
        .opt
        .get(locale_type)
        .and_then(Value::as_array)
        .map(|a| a.iter().map(js::to_js_string).collect())
        .unwrap_or_default();
    let mut name = Value::String(String::new());
    let mut has_val = false;
    if locale_type == "locale-orig" {
        name = item_field.clone();
        has_val = true;
    } else if opts.is_empty() {
        name = item_field.clone();
        has_val = true;
    }
    if !has_val {
        let keys = item
            .get("multi")
            .and_then(|m| m.get("_keys"))
            .and_then(|k| k.get(field));
        for opt in &opts {
            let o = opt.split(|c| c == '-' || c == '_').next().unwrap_or("");
            if let Some(v) = keys
                .and_then(|k| k.get(opt.as_str()))
                .filter(|v| !opt.is_empty() && js::truthy(v))
            {
                name = v.clone();
                break;
            } else if let Some(v) = keys
                .and_then(|k| k.get(o))
                .filter(|v| !o.is_empty() && js::truthy(v))
            {
                name = v.clone();
                break;
            }
        }
        if !js::truthy(&name) {
            name = item_field;
        }
    }
    name
}

/// `CSL.Util.outputNumericField(state, varname, itemID)`: render a parsed
/// numeric variable through the output queue (labels, numeric blobs,
/// styling).
///
/// PORT-LATER(wave2): util_number.js:902-1016, needs `state.output`
/// (`Queue::open_level`, `append`, `close_level`), `NumericBlob`,
/// `CSL.Output.Formatters["capitalize-first"]` and
/// `CSL.UPDATE_GROUP_CONTEXT_CONDITION`.
pub fn output_numeric_field(_state: &mut State, _varname: &str, _item_id: &str) -> CslResult<()> {
    Err(EngineError::NotYetPorted {
        method: "CSL.Util.outputNumericField",
    })
}

#[cfg(test)]
mod tests {
    //! Differential tests against citeproc-js 2.4.63. The reference is
    //! `tests/data/csl/units/numbers.json`, generated by
    //! `scripts/csl-units/numbers.cjs` (node, run by hand):
    //!
    //! * ~5,200 `processNumber` cases: every numeric value in the CSL test
    //!   suite's fixtures plus generated ranges, roman numerals, labels
    //!   (`vol. 3 & 4`, `pp. 12-15`, `2nd ed.`), subsections (`12a-c`),
    //!   escaped hyphens, `|` and fractions, over the numeric variables, in
    //!   seven locales and with a translated (`multi`) value, both the input
    //!   path (`node = false`) and the node path (ranges and styling);
    //! * `Ordinalizer` and `LongOrdinalizer` for -3..130 (plus larger
    //!   numbers and strings) in seven locales and four genders;
    //! * `Romanizer`, `Suffixator` and `padding`.
    //!
    //! The locale terms citeproc-js read while producing each answer are
    //! recorded in the reference (`log`) and replayed through a synthesised
    //! locale (`test_support::logged_locale`), so the tests check the number logic, not the locale
    //! loader. Pass criterion: every output equal to citeproc-js's.
    use super::*;
    use std::collections::BTreeMap;
    use serde_json::json;

    const REF: &str = include_str!("../../tests/data/csl/units/numbers.json");

    fn reference() -> Value {
        serde_json::from_str(REF).expect("reference json")
    }

    fn terms_from(log: &Value) -> BTreeMap<String, Option<String>> {
        log.as_object()
            .map(|o| {
                o.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().map(str::to_string)))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn state_for(engine: &Value) -> State {
        let mut st = State::default();
        if let Some(o) = engine["opt"].as_object() {
            st.opt = o.clone();
        }
        super::super::test_support::install_locale(
            &mut st,
            super::super::test_support::logged_locale(&engine["log"], None, None),
        );
        st.fun.page_mangler = super::super::util_page::PageRangeMangler::get_function(&st, "page");
        st
    }

    fn deco_json(d: &[Decoration]) -> Value {
        Value::Array(
            d.iter()
                .map(|x| {
                    let v = if x.name == "@quotes" && x.value == "true" {
                        Value::Bool(true)
                    } else {
                        Value::String(x.value.clone())
                    };
                    json!([x.name, v])
                })
                .collect(),
        )
    }

    fn node_out(st: &State) -> Value {
        let mut out = Obj::new();
        for (k, sn) in &st.tmp.shadow_numbers {
            let mut o = match sn.to_value() {
                Value::Object(o) => o,
                _ => Obj::new(),
            };
            if let Some(m) = &sn.master_styling {
                o.insert(
                    "_master".into(),
                    json!({"strings": Value::Object(m.strings.clone()), "decorations": deco_json(&m.decorations)}),
                );
            }
            let styl: Vec<Value> = sn
                .values
                .iter()
                .map(|v| match v {
                    ShadowValue::Info(NumberInfo { styling: Some(t), .. }) => {
                        json!({"strings": Value::Object(t.strings.clone()), "decorations": deco_json(&t.decorations)})
                    }
                    _ => Value::Null,
                })
                .collect();
            o.insert("_styling".into(), Value::Array(styl));
            out.insert(k.clone(), Value::Object(o));
        }
        Value::Object(out)
    }

    fn input_out(st: &State) -> Value {
        let mut o = Obj::new();
        for (k, sn) in &st.tmp.shadow_numbers {
            o.insert(k.clone(), sn.to_value());
        }
        Value::Object(o)
    }

    fn node_token() -> Token {
        let mut t = Token::new("number", TokenType::Singleton);
        t.set_string("prefix", "(");
        t.set_string("suffix", ")");
        t.decorations = vec![Decoration::new("@font-style", "italic")];
        t
    }

    #[test]
    fn process_number_matches_citeproc_js() {
        let r = reference();
        let engines = r["engines"].as_object().expect("engines");
        let mut states: BTreeMap<String, State> = BTreeMap::new();
        for (name, e) in engines {
            states.insert(name.clone(), state_for(e));
        }
        let mut bad: Vec<String> = Vec::new();
        let mut n_input = 0usize;
        let mut n_node = 0usize;
        for c in r["cases"].as_array().expect("cases") {
            let st = states
                .get_mut(c["engine"].as_str().unwrap_or(""))
                .expect("engine");
            let variable = c["variable"].as_str().unwrap_or("");
            let item = &c["item"];
            // input path
            st.tmp.shadow_numbers = BTreeMap::new();
            let res = process_number(st, None, Some(item), variable);
            n_input += 1;
            match (&res, c.get("error")) {
                (Ok(()), None) => {
                    let got = input_out(st);
                    if got != c["out"] {
                        bad.push(format!(
                            "[{}] {variable} {item}: want {} got {got}",
                            c["engine"], c["out"]
                        ));
                    }
                }
                (Err(e), Some(want)) => {
                    if want.as_str() != Some(&error_text(e)) {
                        bad.push(format!(
                            "[{}] {variable} {item}: want error {want} got {e}",
                            c["engine"]
                        ));
                    }
                }
                (r, w) => bad.push(format!(
                    "[{}] {variable} {item}: result {r:?} vs reference error {w:?}",
                    c["engine"]
                )),
            }
            // node path (reference has it only for the main engine)
            if c.get("node_out").is_some() || c.get("node_error").is_some() {
                st.tmp.shadow_numbers = BTreeMap::new();
                let tok = node_token();
                let res = process_number(st, Some(&tok), Some(item), variable);
                n_node += 1;
                match (&res, c.get("node_error")) {
                    (Ok(()), None) => {
                        let got = node_out(st);
                        if got != c["node_out"] {
                            bad.push(format!(
                                "node [{}] {variable} {item}: want {} got {got}",
                                c["engine"], c["node_out"]
                            ));
                        }
                    }
                    (Err(e), Some(want)) => {
                        if want.as_str() != Some(&error_text(e)) {
                            bad.push(format!(
                                "node [{}] {variable} {item}: want error {want} got {e}",
                                c["engine"]
                            ));
                        }
                    }
                    (r, w) => bad.push(format!(
                        "node [{}] {variable} {item}: result {r:?} vs reference error {w:?}",
                        c["engine"]
                    )),
                }
            }
        }
        assert!(
            bad.is_empty(),
            "{} mismatches of {n_input} input + {n_node} node cases, first:\n{}",
            bad.len(),
            bad[..bad.len().min(12)].join("\n")
        );
        assert!(n_input > 4000 && n_node > 2000, "{n_input} {n_node}");
    }

    fn error_text(e: &EngineError) -> String {
        match e {
            EngineError::BadInput(m) => m.clone(),
            other => other.to_string(),
        }
    }

    #[test]
    fn ordinalizers_match_citeproc_js() {
        let r = reference();
        let mut states: BTreeMap<String, State> = BTreeMap::new();
        for (lang, e) in r["ord_engines"].as_object().expect("engines") {
            let mut st = state_for(e);
            super::super::test_support::install_locale(
                &mut st,
                super::super::test_support::logged_locale(
                    &e["log"],
                    Some(&e["fields"]),
                    Some(&e["ord_101"]),
                ),
            );
            states.insert(lang.clone(), st);
        }
        let mut bad = Vec::new();
        let mut n = 0;
        for c in r["ordinal"].as_array().expect("cases") {
            let st = states
                .get_mut(c["lang"].as_str().unwrap_or(""))
                .expect("engine");
            let gender = c["gender"].as_str();
            n += 1;
            let got = if c["kind"] == "ordinal" {
                Ordinalizer::default().format(st, &c["num"], gender)
            } else {
                st.tmp.cite_renders_content = false;
                LongOrdinalizer::format(st, &c["num"], gender)
            };
            match (got, c.get("error")) {
                (Ok(s), None) => {
                    if Value::String(s.clone()) != c["out"] {
                        bad.push(format!(
                            "{} {} {} {:?}: want {} got {s}",
                            c["lang"], c["kind"], c["num"], gender, c["out"]
                        ));
                    }
                    if c["kind"] == "long"
                        && c.get("crc").is_some()
                        && c["crc"].as_bool() != Some(st.tmp.cite_renders_content)
                    {
                        bad.push(format!(
                            "{} long {}: cite_renders_content",
                            c["lang"], c["num"]
                        ));
                    }
                }
                (Err(e), Some(want)) => {
                    if want.as_str() != Some(&error_text(&e)) {
                        bad.push(format!(
                            "{} {} {}: want error {want} got {e}",
                            c["lang"], c["kind"], c["num"]
                        ));
                    }
                }
                (g, w) => bad.push(format!(
                    "{} {} {}: {g:?} vs {w:?}",
                    c["lang"], c["kind"], c["num"]
                )),
            }
        }
        assert!(
            bad.is_empty(),
            "{} of {n} differ, first:\n{}",
            bad.len(),
            bad[..bad.len().min(12)].join("\n")
        );
        assert!(n > 7000);
    }

    #[test]
    fn romanizer_suffixator_padding_match_citeproc_js() {
        let r = reference();
        for c in r["roman"].as_array().expect("roman") {
            match (Romanizer::default().format(&c["num"]), c.get("error")) {
                (Ok(s), None) => assert_eq!(Value::String(s), c["out"], "romanizer {}", c["num"]),
                (Err(e), Some(w)) => assert_eq!(
                    w.as_str(),
                    Some(error_text(&e).as_str()),
                    "romanizer {}",
                    c["num"]
                ),
                (g, w) => panic!("romanizer {}: {g:?} vs {w:?}", c["num"]),
            }
        }
        for c in r["suffix"].as_array().expect("suffix") {
            let s = Suffixator::new(c["slist"].as_str());
            assert_eq!(
                Value::String(s.format(c["n"].as_i64().unwrap_or(0))),
                c["out"],
                "suffixator {} {}",
                c["slist"],
                c["n"]
            );
        }
        for c in r["padding"].as_array().expect("padding") {
            assert_eq!(
                Value::String(padding(c["in"].as_str().unwrap_or(""))),
                c["out"],
                "padding {}",
                c["in"]
            );
        }
    }
}
