// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/attributes.js
// Version:     2.4.63, commit 73bc1b44bc7d54d0bfec4e070fd27f5efe024ff9
// Copyright:   (c) 2009-2019 Frank Bennett
// Licence:     AGPL-3.0, taken from upstream's "CPAL-1.0 or AGPL-3.0-or-later"
//              (LICENSE at the commit above; see this crate's NOTICE).
// Modified:    2026-10-08, by the OUTRAM PARK contributors. This file is a
//              Rust translation (port) of the file named above, modified
//              from the original.
// No warranty: this program is distributed in the hope that it will be
//              useful, but WITHOUT ANY WARRANTY; without even the implied
//              warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
//              PURPOSE. See the GNU Affero General Public License.

//! Port of `src/attributes.js`: `CSL.Attributes["@name"]`, the handlers that
//! turn each CSL attribute into token properties and token closures while the
//! style is compiled (`CSL.XmlToToken`, util_nodes.js, calls [`apply`]).
//!
//! Attributes that only set a token property or an engine option are fully
//! ported. Attributes that add behaviour push [`AttributesExec`] /
//! [`AttributesTest`] variants (same count and order as citeproc-js). Their
//! bodies are ported where they need only state, token and item; the others
//! carry a `PORT-LATER` marker naming the machinery they wait for.
//!
//! **Token mutation at run time.** Several upstream closures mutate `this`
//! (the token) while a style runs: `@variable`'s first closure rewrites
//! `this.variables` in place, `@text-case` rewrites `this.strings`. The
//! foundation's `Exec::run` receives `&Token`, so those bodies are deferred
//! until the integrator decides where run-time token state lives.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::exec::{Exec, Test};
use super::js::{self, Obj};
use super::obj_token::{Token, TokenType};
use super::state::{Area, State};
use super::util_conditions;
use super::{CslResult, EngineError};

// ---------------------------------------------------------------------------
// Constants of load.js that this file and the node builders need.
// STUB(load.rs): the owner of load.js defines these; the integrator replaces
// this block with references to that module.
// ---------------------------------------------------------------------------

/// `CSL.START`.
pub const START: i64 = 0;
/// `CSL.DESCENDING`.
pub const DESCENDING: i64 = 1;
/// `CSL.ASCENDING`.
pub const ASCENDING: i64 = 2;
/// `CSL.NUMERIC` (update_mode / bib_mode).
pub const NUMERIC: i64 = 1;
/// `CSL.POSITION` (update_mode).
pub const POSITION: i64 = 2;
/// `CSL.TRIGRAPH` (bib_mode).
pub const TRIGRAPH: i64 = 3;
/// `CSL.GIVENNAME_DISAMBIGUATION_RULES`.
pub const GIVENNAME_DISAMBIGUATION_RULES: [&str; 5] = [
    "all-names",
    "all-names-with-initials",
    "primary-name",
    "primary-name-with-initials",
    "by-cite",
];
/// `CSL.NUMERIC_VARIABLES`.
pub const NUMERIC_VARIABLES: [&str; 20] = [
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
/// `CSL.DATE_VARIABLES`.
pub const DATE_VARIABLES: [&str; 10] = [
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
/// `CSL.NAME_VARIABLES`.
pub const NAME_VARIABLES: [&str; 28] = [
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
/// `CSL.MULTI_FIELDS`.
pub const MULTI_FIELDS: [&str; 15] = [
    "event",
    "publisher",
    "publisher-place",
    "event-place",
    "title",
    "container-title",
    "collection-title",
    "authority",
    "genre",
    "title-short",
    "medium",
    "country",
    "jurisdiction",
    "archive",
    "archive-place",
];
/// `CSL.CITE_FIELDS`.
pub const CITE_FIELDS: [&str; 4] = [
    "first-reference-note-number",
    "first-container-reference-note-number",
    "locator",
    "locator-extra",
];
/// `CSL.DISPLAY_CLASSES`.
pub const DISPLAY_CLASSES: [&str; 4] = ["block", "left-margin", "right-inline", "indent"];

/// `CSL.STARTSWITH_ROMANESQUE_REGEXP.test(s)`.
pub fn starts_with_romanesque(s: &str) -> bool {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            "^[&a-zA-Z\u{0e01}-\u{0e5b}\u{00c0}-\u{017f}\u{0370}-\u{03ff}\u{0400}-\u{052f}\
             \u{0590}-\u{05d4}\u{05d6}-\u{05ff}\u{1f00}-\u{1fff}\u{0600}-\u{06ff}\u{200c}\
             \u{200d}\u{200e}\u{0218}\u{0219}\u{021a}\u{021b}\u{202a}-\u{202e}]",
        )
        .expect("static regex")
    });
    RE.is_match(s)
}

/// `CSL.POSITION_MAP[n]`.
pub fn position_map(n: i64) -> Option<i64> {
    match n {
        0 => Some(0),
        4 => Some(1),
        1 => Some(2),
        2 => Some(3),
        3 => Some(4),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Small JS helpers.
// ---------------------------------------------------------------------------

static WS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!("[{}]+", js::WS)).expect("static regex"));

/// JS `arg.split(/\s+/)`.
pub fn split_ws(arg: &str) -> Vec<String> {
    js::split(&WS_RE, arg)
}

/// A JS number that may be NaN, as a JSON value (`NaN` serialises as `null`).
pub fn int_or_nan(n: Option<i64>) -> Value {
    match n {
        Some(i) => Value::from(i),
        None => Value::Null,
    }
}

/// JS `parseFloat(s)` as a JSON value: an integer where integral, `null` for
/// NaN.
pub fn parse_float_value(s: &str) -> Value {
    let t = s.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
    let b = t.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let ds = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let mut digits = i - ds;
    if i < b.len() && b[i] == b'.' {
        let mut j = i + 1;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        digits += j - (i + 1);
        i = j;
    }
    if digits == 0 {
        return Value::Null;
    }
    match t[..i].parse::<f64>() {
        Ok(f) if f == f.trunc() && f.abs() < 9.0e15 => Value::from(f as i64),
        Ok(f) => serde_json::Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        Err(_) => Value::Null,
    }
}

/// JS `"" + item[key]`: `"undefined"` when absent.
fn str_of(item: &Value, key: &str) -> String {
    match item.get(key) {
        Some(v) => js::to_js_string(v),
        None => "undefined".to_string(),
    }
}

/// `item[key]` truthiness (`item` may be `null`/absent: falsy).
fn item_truthy(item: &Value, key: &str) -> bool {
    js::truthy_opt(item.get(key))
}

/// JS `x` is an object with truthy-valued own entry (for `for (key in x)`).
fn any_truthy_entry(v: &Value) -> bool {
    match v {
        Value::Array(a) => a.iter().any(js::truthy),
        Value::Object(o) => o.values().any(js::truthy),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// STUBs for engine methods owned by wave1-build (build.js, util_locale.js).
// ---------------------------------------------------------------------------

/// `CSL.localeResolve(langstr, defaultLocale)` result (util_locale.js:3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LocaleSpec {
    /// `base`, e.g. `en-US`.
    pub base: String,
    /// `best`.
    pub best: String,
    /// `bare`, e.g. `en`.
    pub bare: String,
    /// `generic` (only set when the language had no region).
    pub generic: bool,
}

impl LocaleSpec {
    /// The JS object `{base, best, bare[, generic: true]}` as JSON.
    pub fn to_value(&self) -> Value {
        let mut o = Obj::new();
        o.insert("base".into(), Value::String(self.base.clone()));
        o.insert("best".into(), Value::String(self.best.clone()));
        o.insert("bare".into(), Value::String(self.bare.clone()));
        if self.generic {
            o.insert("generic".into(), Value::Bool(true));
        }
        Value::Object(o)
    }
}

/// `CSL.LANG_BASES[lang]`.
fn lang_base(lang: &str) -> Option<&'static str> {
    Some(match lang {
        "af" => "af_ZA",
        "ar" => "ar",
        "bg" => "bg_BG",
        "ca" => "ca_AD",
        "cs" => "cs_CZ",
        "da" => "da_DK",
        "de" => "de_DE",
        "el" => "el_GR",
        "en" => "en_US",
        "es" => "es_ES",
        "et" => "et_EE",
        "eu" => "eu",
        "fa" => "fa_IR",
        "fi" => "fi_FI",
        "fr" => "fr_FR",
        "he" => "he_IL",
        "hr" => "hr-HR",
        "hu" => "hu_HU",
        "is" => "is_IS",
        "it" => "it_IT",
        "ja" => "ja_JP",
        "km" => "km_KH",
        "ko" => "ko_KR",
        "lt" => "lt_LT",
        "lv" => "lv-LV",
        "mn" => "mn_MN",
        "nb" => "nb_NO",
        "nl" => "nl_NL",
        "nn" => "nn-NO",
        "pl" => "pl_PL",
        "pt" => "pt_PT",
        "ro" => "ro_RO",
        "ru" => "ru_RU",
        "sk" => "sk_SK",
        "sl" => "sl_SI",
        "sr" => "sr_RS",
        "sv" => "sv_SE",
        "th" => "th_TH",
        "tr" => "tr_TR",
        "uk" => "uk_UA",
        "vi" => "vi_VN",
        "zh" => "zh_CN",
        _ => return None,
    })
}

/// `CSL.localeResolve(langstr, defaultLocale)` (util_locale.js:3).
/// STUB(util_locale.rs): the owner of util_locale.js defines the real one;
/// this is a faithful copy so the node builders compile and test now.
pub fn locale_resolve(langstr: &str, default_locale: &str) -> LocaleSpec {
    let default_locale = if default_locale.is_empty() {
        "en-US"
    } else {
        default_locale
    };
    let langstr = if langstr.is_empty() {
        default_locale
    } else {
        langstr
    };
    let langlst: Vec<&str> = langstr.split(['-', '_']).collect();
    let Some(base) = lang_base(langlst[0]) else {
        return LocaleSpec {
            base: default_locale.to_string(),
            best: langstr.to_string(),
            bare: langlst[0].to_string(),
            generic: false,
        };
    };
    let generic = langlst.len() == 1;
    let best = if langlst.len() == 1 || langlst[1] == "x" {
        base.replacen('_', "-", 1)
    } else {
        langlst[..2].join("-")
    };
    LocaleSpec {
        base: base.replacen('_', "-", 1),
        best,
        bare: langlst[0].to_string(),
        generic,
    }
}

/// `state.opt["default-locale"][0]` (`"en-US"` when absent).
pub fn default_locale(state: &State) -> String {
    state
        .opt
        .get("default-locale")
        .and_then(|v| v.get(0))
        .and_then(|v| v.as_str())
        .unwrap_or("en-US")
        .to_string()
}

/// `state.localeConfigure(langspec, beShy)` (util_locale.js:44).
/// STUB(util_locale.rs): reports not-ported; the integrator replaces it.
pub fn locale_configure(_state: &mut State, _spec: &LocaleSpec, _be_shy: bool) -> CslResult<()> {
    // PORT-LATER(wave1-build): util_locale.js:44 localeConfigure, needs locale loading.
    Err(EngineError::NotYetPorted {
        method: "util_locale.js:44 CSL.Engine.prototype.localeConfigure",
    })
}

/// `state[name]` for the five areas (`citation`, `bibliography`, `intext`,
/// `citation_sort`, `bibliography_sort`); any other name is the JS
/// `TypeError` of reading `.opt` of `undefined`.
pub fn area_ref<'s>(state: &'s State, name: &str) -> CslResult<&'s Area> {
    match name {
        "citation" => Ok(&state.citation),
        "bibliography" => Ok(&state.bibliography),
        "intext" => Ok(&state.intext),
        "citation_sort" => Ok(&state.citation_sort),
        "bibliography_sort" => Ok(&state.bibliography_sort),
        other => Err(EngineError::Csl(format!(
            "TypeError: state.{other} is undefined"
        ))),
    }
}

/// Mutable [`area_ref`].
pub fn area_mut<'s>(state: &'s mut State, name: &str) -> CslResult<&'s mut Area> {
    match name {
        "citation" => Ok(&mut state.citation),
        "bibliography" => Ok(&mut state.bibliography),
        "intext" => Ok(&mut state.intext),
        "citation_sort" => Ok(&mut state.citation_sort),
        "bibliography_sort" => Ok(&mut state.bibliography_sort),
        other => Err(EngineError::Csl(format!(
            "TypeError: state.{other} is undefined"
        ))),
    }
}

/// `obj[key]` as a mutable JSON object, created when absent or not an object.
pub fn obj_entry<'o>(obj: &'o mut Obj, key: &str) -> &'o mut Obj {
    let slot = obj
        .entry(key.to_string())
        .or_insert_with(|| Value::Object(Obj::new()));
    if !slot.is_object() {
        *slot = Value::Object(Obj::new());
    }
    slot.as_object_mut().expect("slot was just made an object")
}

/// `obj[key]` as a mutable JSON array, created when absent or not an array.
pub fn arr_entry<'o>(obj: &'o mut Obj, key: &str) -> &'o mut Vec<Value> {
    let slot = obj
        .entry(key.to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    if !slot.is_array() {
        *slot = Value::Array(Vec::new());
    }
    slot.as_array_mut().expect("slot was just made an array")
}

/// `state.setOpt(token, name, value)` (build.js:776).
/// STUB(build.rs): faithful copy; the integrator may point it at the real one.
pub fn set_opt(state: &mut State, token: &mut Token, name: &str, value: Value) {
    if token.name == "style" || token.name == "cslstyle" {
        obj_entry(&mut state.opt, "inheritedAttributes").insert(name.to_string(), value.clone());
        obj_entry(&mut state.citation.opt, "inheritedAttributes")
            .insert(name.to_string(), value.clone());
        obj_entry(&mut state.bibliography.opt, "inheritedAttributes")
            .insert(name.to_string(), value);
    } else if token.name == "citation" || token.name == "bibliography" {
        let area = if token.name == "citation" {
            &mut state.citation
        } else {
            &mut state.bibliography
        };
        obj_entry(&mut area.opt, "inheritedAttributes").insert(name.to_string(), value);
    } else {
        token.strings.insert(name.to_string(), value);
    }
}

/// `state.inheritOpt(token, attrname, parentname, defaultValue)`
/// (build.js:789). `None` is JS `undefined`.
/// STUB(build.rs): faithful copy; the integrator may point it at the real one.
pub fn inherit_opt(
    state: &State,
    token: &Token,
    attrname: &str,
    parentname: Option<&str>,
    default_value: Option<Value>,
) -> CslResult<Option<Value>> {
    if let Some(v) = token.strings.get(attrname) {
        return Ok(Some(v.clone()));
    }
    let root = state.tmp.root.clone().ok_or_else(|| {
        EngineError::Csl("TypeError: state[state.tmp.root] with undefined root".into())
    })?;
    let area = area_ref(state, &root)?;
    let key = match parentname {
        Some(p) if !p.is_empty() => p,
        _ => attrname,
    };
    let parent = area.opt.get("inheritedAttributes").and_then(|o| o.get(key));
    match parent {
        Some(v) => Ok(Some(v.clone())),
        None => Ok(default_value),
    }
}

/// STUB(util_nodes.rs): `CSL.expandMacro.call(state, token, target)`
/// (util_nodes.js). The owner of util_nodes.js defines the real one.
pub fn expand_macro_stub(
    _state: &mut State,
    _token: Token,
    _target: &mut Vec<Token>,
) -> CslResult<()> {
    // PORT-LATER(wave1-build): util_nodes.js CSL.expandMacro.
    Err(EngineError::NotYetPorted {
        method: "util_nodes.js CSL.expandMacro",
    })
}

/// `state.getTerm(...)` at build time. STUB(build.rs): not available yet.
pub fn get_term_stub(_state: &State, _name: &str) -> CslResult<String> {
    // PORT-LATER(wave1-build): build.js getTerm, needs the merged locale terms.
    Err(EngineError::NotYetPorted {
        method: "build.js CSL.Engine.prototype.getTerm",
    })
}

// ---------------------------------------------------------------------------
// Closures.
// ---------------------------------------------------------------------------

/// The closures `src/attributes.js` stores in `token.execs` (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum AttributesExec {
    /// `@variable` on names/date/text/number: "set variable names"
    /// (attributes.js:~150): rewrites `this.variables` from `variables_real`,
    /// skipping `done_vars`.
    VariableSetNames,
    /// `@variable` on names/date/text/number: "check for output"
    /// (attributes.js:~162-300).
    VariableCheckOutput,
    /// `@text-case` (attributes.js:1521): sets `this.strings["text-case"]`.
    TextCase {
        /// The attribute value.
        arg: String,
    },
}

impl AttributesExec {
    /// Run the closure.
    pub fn run(
        &self,
        _state: &mut State,
        _token: &Token,
        _item: &Value,
        _cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            // PORT-LATER(integrator): attributes.js:150-165, rewrites
            // this.variables in place at run time; needs run-time mutable
            // token state (Exec::run receives &Token) and state.tmp.done_vars.
            AttributesExec::VariableSetNames => Err(EngineError::NotYetPorted {
                method: "attributes.js:@variable set-variable-names closure",
            }),
            // PORT-LATER(wave2): attributes.js:166-300, mutates Item (authority,
            // committee split), reads state.transform.abbrevs, writes
            // state.tmp.group_context.tip / name_node / output.current.
            AttributesExec::VariableCheckOutput => Err(EngineError::NotYetPorted {
                method: "attributes.js:@variable check-for-output closure",
            }),
            // PORT-LATER(integrator): attributes.js:1521-1535, rewrites
            // this.strings / this.text_case_normal on the running token.
            AttributesExec::TextCase { .. } => Err(EngineError::NotYetPorted {
                method: "attributes.js:@text-case closure",
            }),
        }
    }
}

/// The condition closures `src/attributes.js` stores in `token.tests` /
/// `token.test` (PORTING.md §4). Each variant is one `maketest(...)` closure
/// with its captured arguments; all take `(Item, item)` upstream.
#[derive(Debug, Clone, PartialEq)]
pub enum AttributesTest {
    /// `@disambiguate="true"` (attributes.js:6-24).
    Disambiguate,
    /// `@disambiguate="check-ambiguity-and-backreference"` (attributes.js:25-31).
    DisambiguateBackref,
    /// `@is-numeric` for one variable (attributes.js:36-62).
    IsNumeric {
        /// The variable.
        variable: String,
    },
    /// `@is-uncertain-date` for one variable.
    IsUncertainDate {
        /// The variable.
        variable: String,
    },
    /// `@locator` for one label.
    Locator {
        /// The label tried.
        trylabel: String,
    },
    /// `testSubsequentNear` (attributes.js:117).
    PositionNear,
    /// `testSubsequentNotNear` (attributes.js:123).
    PositionFar,
    /// `@position` for one position keyword, mapped to `CSL.POSITION_*`
    /// (`None`: a keyword upstream does not map, which never matches).
    Position {
        /// `tryposition`.
        tryposition: Option<i64>,
    },
    /// `@type` for one type (nested inside a `match.any`).
    Type {
        /// The type tried.
        mytype: String,
    },
    /// `@variable` on `cs:if` / `cs:else-if` / `cs:condition`.
    Variable {
        /// The variable.
        variable: String,
    },
    /// `@page` for one label.
    Page {
        /// The label tried.
        trylabel: String,
    },
    /// `@number` for one label.
    Number {
        /// The label tried.
        trylabel: String,
    },
    /// `@jurisdiction` (a list, one closure).
    Jurisdiction {
        /// The jurisdictions tried.
        tryjurisdictions: Vec<String>,
    },
    /// `@country` (a list, one closure).
    Country {
        /// The countries tried.
        trycountries: Vec<String>,
    },
    /// `@context`.
    Context {
        /// The attribute value.
        arg: String,
    },
    /// `@has-year-only` for one date variable.
    HasYearOnly {
        /// The date variable.
        trydate: String,
    },
    /// `@has-to-month-or-season` for one date variable.
    HasToMonthOrSeason {
        /// The date variable.
        trydate: String,
    },
    /// `@has-day` for one date variable.
    HasDay {
        /// The date variable.
        trydate: String,
    },
    /// `@is-plural`.
    IsPlural {
        /// The names variable.
        arg: String,
    },
    /// `@is-multiple`.
    IsMultiple {
        /// The variable.
        arg: String,
    },
    /// `@locale` on cs:if / cs:else-if (attributes.js:~700): the
    /// `maketest(locale_list, locale_default, locale_bares)` closure.
    Locale {
        /// `locale_list`.
        locale_list: Vec<LocaleSpec>,
        /// `locale_default`.
        locale_default: String,
        /// `locale_bares`.
        locale_bares: Vec<String>,
    },
    /// `@alternative-node-internal`.
    AlternativeNodeInternal,
    /// `@locale-internal` (attributes.js:~790): `maketest(me)`.
    LocaleInternal {
        /// `me.locale_list`.
        locale_list: Vec<LocaleSpec>,
        /// `me.locale_bares`.
        locale_bares: Vec<String>,
        /// `me.locale`.
        locale: String,
    },
    /// `@court-class` for one class.
    CourtClass {
        /// The class tried.
        tryclass: String,
    },
    /// `@container-multiple`.
    ContainerMultiple {
        /// `"true" === arg`.
        retval: bool,
    },
    /// `@container-subsequent`.
    ContainerSubsequent {
        /// `"true" === arg`.
        retval: bool,
    },
    /// `@has-subunit`.
    HasSubunit {
        /// The names variable.
        namevar: String,
    },
    /// `@cite-form`.
    CiteForm {
        /// The cite form tried.
        cite_form: String,
    },
}

static IS_MULTIPLE_RE: LazyLock<Regex> = LazyLock::new(|| {
    let ws = js::WS;
    Regex::new(&format!(
        "(?:,[{ws}]|[{ws}](?:tot[{ws}]en[{ws}]met|l\u{012b}dz|oraz|and|bis|\u{03ad}\u{03c9}\u{03c2}|\
         \u{03ba}\u{03b1}\u{03b9}|och|a\u{017e}|do|en|et|in|ir|ja|og|sa|to|un|und|\u{00e9}s|\
         \u{0219}i|i|u|y|\u{00e0}|e|a|\u{0438}|-|\u{2013})[{ws}]|\u{2014}|&)"
    ))
    .expect("static regex")
});

impl AttributesTest {
    /// Evaluate the condition: `test(Item, item)`.
    pub fn eval(
        &self,
        state: &mut State,
        _token: &Token,
        item: &Value,
        cite_item: &Value,
    ) -> CslResult<bool> {
        match self {
            // PORT-LATER(wave4): attributes.js:6-24, needs
            // state.registry.registry[Item.id].disambig and
            // state.tmp.disambig_settings / disambiguate_count (registry.rs).
            AttributesTest::Disambiguate => Err(EngineError::NotYetPorted {
                method: "attributes.js:@disambiguate closure",
            }),
            // PORT-LATER(wave4): attributes.js:25-31, needs
            // state.registry.registry[Item.id].disambig / citation-count.
            AttributesTest::DisambiguateBackref => Err(EngineError::NotYetPorted {
                method: "attributes.js:@disambiguate backreference closure",
            }),
            // PORT-LATER(wave1-input): attributes.js:36-62, needs
            // state.processNumber and state.tmp.shadow_numbers.
            AttributesTest::IsNumeric { .. } => Err(EngineError::NotYetPorted {
                method: "attributes.js:@is-numeric closure",
            }),
            AttributesTest::IsUncertainDate { variable } => {
                // Item[v] && Item[v].circa
                Ok(item
                    .get(variable.as_str())
                    .map(|d| js::truthy(d) && js::truthy_opt(d.get("circa")))
                    .unwrap_or(false))
            }
            // PORT-LATER(wave1-input): attributes.js:78-90, needs
            // state.processNumber(false, item, "locator") / shadow_numbers.
            AttributesTest::Locator { .. } => Err(EngineError::NotYetPorted {
                method: "attributes.js:@locator closure",
            }),
            AttributesTest::PositionNear => {
                // item && MAP[item.position] >= MAP[SUBSEQUENT(1)] && item["near-note"]
                let pos = cite_item.get("position").and_then(Value::as_i64);
                Ok(js::truthy(cite_item)
                    && position_map_ge(pos, 1)
                    && item_truthy(cite_item, "near-note"))
            }
            AttributesTest::PositionFar => {
                let pos = cite_item.get("position").and_then(Value::as_i64);
                Ok(js::truthy(cite_item)
                    && pos.and_then(position_map) == position_map(1)
                    && pos.is_some()
                    && !item_truthy(cite_item, "near-note"))
            }
            AttributesTest::Position { tryposition } => {
                if state.tmp.area == "bibliography" {
                    return Ok(false);
                }
                // JS sets `item.position = 0` when undefined; the result below is
                // the same whether or not that write happens (see module docs).
                let has_item = js::truthy(cite_item);
                let pos_value = cite_item.get("position");
                let numeric_pos = match pos_value {
                    None => Some(0),
                    Some(Value::Number(n)) => n.as_i64(),
                    _ => None,
                };
                let pos_is_number = has_item && matches!(pos_value, None | Some(Value::Number(_)));
                if pos_is_number {
                    let p = numeric_pos;
                    if p == Some(0) && *tryposition == Some(0) {
                        return Ok(true);
                    }
                    if let (Some(t), Some(pp)) = (tryposition, p) {
                        if *t > 0 {
                            if let (Some(a), Some(b)) = (position_map(pp), position_map(*t)) {
                                if a >= b {
                                    return Ok(true);
                                }
                            }
                        }
                    }
                } else if *tryposition == Some(0) {
                    return Ok(true);
                }
                Ok(false)
            }
            AttributesTest::Type { mytype } => Ok(item
                .get("type")
                .map(|t| t.as_str() == Some(mytype.as_str()))
                .unwrap_or(false)),
            AttributesTest::Variable { variable } => {
                let use_cite = matches!(
                    variable.as_str(),
                    "locator"
                        | "locator-extra"
                        | "first-reference-note-number"
                        | "first-container-reference-note-number"
                        | "locator-date"
                ) && js::truthy(cite_item);
                let myitem = if use_cite { cite_item } else { item };
                if variable == "hereinafter" && js::truthy_opt(myitem.get("id")) {
                    // PORT-LATER(wave1-build): attributes.js:~410, needs
                    // state.sys.getAbbreviation and state.transform.abbrevs.
                    return Err(EngineError::NotYetPorted {
                        method: "attributes.js:@variable hereinafter test",
                    });
                }
                match myitem.get(variable.as_str()) {
                    Some(v) if js::truthy(v) => match v {
                        Value::Number(_) | Value::String(_) => Ok(true),
                        Value::Array(_) | Value::Object(_) => Ok(any_truthy_entry(v)),
                        _ => Ok(false),
                    },
                    _ => Ok(false),
                }
            }
            // PORT-LATER(wave1-input): attributes.js:428-450, needs
            // state.processNumber and state.tmp.shadow_numbers.page.
            AttributesTest::Page { .. } => Err(EngineError::NotYetPorted {
                method: "attributes.js:@page closure",
            }),
            // PORT-LATER(wave1-input): attributes.js:458-472, needs
            // state.processNumber and state.tmp.shadow_numbers.number.
            AttributesTest::Number { .. } => Err(EngineError::NotYetPorted {
                method: "attributes.js:@number closure",
            }),
            AttributesTest::Jurisdiction { tryjurisdictions } => {
                Ok(match item.get("jurisdiction").filter(|v| js::truthy(v)) {
                    Some(Value::String(j)) => tryjurisdictions.iter().any(|t| t == j),
                    _ => false,
                })
            }
            AttributesTest::Country { trycountries } => {
                Ok(match item.get("country").filter(|v| js::truthy(v)) {
                    Some(Value::String(c)) => trycountries.iter().any(|t| t == c),
                    _ => false,
                })
            }
            AttributesTest::Context { arg } => {
                if arg == "bibliography" || arg == "citation" {
                    let area = js::slice(&state.tmp.area, 0, Some(js::len(arg) as i64));
                    Ok(area == *arg)
                } else if arg == "alternative" {
                    Ok(state.tmp.abort_alternative)
                } else {
                    Ok(false)
                }
            }
            AttributesTest::HasYearOnly { trydate } => {
                let date = item.get(trydate.as_str());
                Ok(match date {
                    Some(d) if js::truthy(d) => {
                        !(js::truthy_opt(d.get("month")) || js::truthy_opt(d.get("season")))
                    }
                    _ => false,
                })
            }
            AttributesTest::HasToMonthOrSeason { trydate } => {
                let date = item.get(trydate.as_str());
                Ok(match date {
                    Some(d) if js::truthy(d) => {
                        (js::truthy_opt(d.get("month")) || js::truthy_opt(d.get("season")))
                            && !js::truthy_opt(d.get("day"))
                    }
                    _ => false,
                })
            }
            AttributesTest::HasDay { trydate } => Ok(match item.get(trydate.as_str()) {
                Some(d) if js::truthy(d) => js::truthy_opt(d.get("day")),
                _ => false,
            }),
            AttributesTest::IsPlural { arg } => {
                let spoof = state
                    .opt
                    .get("development_extensions")
                    .map(|d| js::truthy_opt(d.get("spoof_institutional_affiliations")))
                    .unwrap_or(false);
                if let Some(Value::Array(list)) = item.get(arg.as_str()) {
                    let mut persons = 0;
                    let mut institutions = 0;
                    let mut last_is_person = false;
                    for n in list {
                        if spoof
                            && (js::truthy_opt(n.get("literal"))
                                || (js::truthy_opt(n.get("isInstitution"))
                                    && js::truthy_opt(n.get("family"))
                                    && !js::truthy_opt(n.get("given"))))
                        {
                            institutions += 1;
                            last_is_person = false;
                        } else {
                            persons += 1;
                            last_is_person = true;
                        }
                    }
                    if persons > 1 || institutions > 1 || (institutions > 0 && last_is_person) {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            AttributesTest::IsMultiple { arg } => {
                // JS: ("" + Item[arg]).split(re).length > 1; the pattern has no
                // captures and cannot match empty, so that is "re matches".
                let val = str_of(item, arg);
                Ok(IS_MULTIPLE_RE.is_match(&val))
            }
            AttributesTest::Locale {
                locale_list,
                locale_default,
                locale_bares,
            } => {
                let lang = match item.get("language").filter(|v| js::truthy(v)) {
                    None => locale_default.clone(),
                    Some(v) => js::to_js_string(v),
                };
                let langspec = locale_resolve(&lang, locale_default);
                let mut res = false;
                for l in locale_list {
                    if langspec.best == l.best {
                        push_lang_state(state);
                        set_lang(state, &locale_list[0].best);
                        res = true;
                        break;
                    }
                }
                if !res && locale_bares.contains(&langspec.bare) {
                    push_lang_state(state);
                    set_lang(state, &locale_list[0].best);
                    res = true;
                }
                Ok(res)
            }
            AttributesTest::AlternativeNodeInternal => Ok(!state.tmp.abort_alternative),
            // PORT-LATER(wave1-output): attributes.js:~800-835, needs
            // state.output.openLevel("empty") and
            // state.output.current.value().new_locale (queue.rs). Captured:
            // locale_list, locale_bares, locale.
            AttributesTest::LocaleInternal { .. } => Err(EngineError::NotYetPorted {
                method: "attributes.js:@locale-internal closure",
            }),
            // PORT-LATER(wave2): attributes.js:~845, needs CSL.GET_COURT_CLASS.
            AttributesTest::CourtClass { .. } => Err(EngineError::NotYetPorted {
                method: "attributes.js:@court-class closure",
            }),
            AttributesTest::ContainerMultiple { retval } => {
                let key = str_of(item, "container_id");
                let n = state
                    .tmp
                    .container_item_count
                    .get(&key)
                    .filter(|v| js::truthy(v))
                    .and_then(Value::as_f64);
                Ok(match n {
                    None => !*retval,
                    Some(c) if c > 1.0 => *retval,
                    Some(_) => !*retval,
                })
            }
            AttributesTest::ContainerSubsequent { retval } => {
                let key = str_of(item, "container_id");
                let n = state
                    .tmp
                    .container_item_pos
                    .get(&key)
                    .and_then(Value::as_f64);
                Ok(match n {
                    Some(p) if p > 1.0 => *retval,
                    _ => !*retval,
                })
            }
            AttributesTest::HasSubunit { namevar } => {
                let mut subunit_count: usize = 0;
                let names: Vec<&Value> = match item.get(namevar.as_str()) {
                    Some(Value::Array(a)) => a.iter().collect(),
                    Some(Value::Object(o)) => o.values().collect(),
                    Some(Value::String(s)) if !s.is_empty() => {
                        return Err(EngineError::Csl(
                            "TypeError: cannot read properties of undefined (reading 'split')"
                                .into(),
                        ))
                    }
                    _ => Vec::new(),
                };
                for name in names {
                    if !js::truthy_opt(name.get("given")) {
                        let inst = if js::truthy_opt(name.get("literal")) {
                            name.get("literal")
                        } else {
                            name.get("family")
                        };
                        let Some(inst) = inst else {
                            return Err(EngineError::Csl(
                                "TypeError: cannot read properties of undefined (reading 'split')"
                                    .into(),
                            ));
                        };
                        let length = js::to_js_string(inst).split('|').count();
                        if subunit_count == 0 || length < subunit_count {
                            subunit_count = length;
                        }
                    }
                }
                Ok(subunit_count > 1)
            }
            AttributesTest::CiteForm { cite_form } => Ok(item
                .get("cite-form")
                .map(|v| v.as_str() == Some(cite_form.as_str()))
                .unwrap_or(false)),
        }
    }
}

/// `CSL.POSITION_MAP[pos] >= CSL.POSITION_MAP[target]` with undefined = false.
fn position_map_ge(pos: Option<i64>, target: i64) -> bool {
    match (pos.and_then(position_map), position_map(target)) {
        (Some(a), Some(b)) => a >= b,
        _ => false,
    }
}

/// `state.tmp.condition_lang_counter_arr.push(state.tmp.condition_counter);
/// state.tmp.condition_lang_val_arr.push(state.opt.lang);`
fn push_lang_state(state: &mut State) {
    state
        .tmp
        .condition_lang_counter_arr
        .push(state.tmp.condition_counter);
    let lang = state
        .opt
        .get("lang")
        .map(js::to_js_string)
        .unwrap_or_default();
    state.tmp.condition_lang_val_arr.push(lang);
}

/// `state.opt.lang = lang`.
fn set_lang(state: &mut State, lang: &str) {
    state
        .opt
        .insert("lang".into(), Value::String(lang.to_string()));
}

// ---------------------------------------------------------------------------
// The attribute handlers.
// ---------------------------------------------------------------------------

fn atest(t: AttributesTest) -> Test {
    Test::Attributes(t)
}

fn strings_vec(v: &[String]) -> Value {
    Value::Array(v.iter().map(|s| Value::String(s.clone())).collect())
}

fn flag_map(vars: &[String]) -> Value {
    let mut o = Obj::new();
    for v in vars {
        o.insert(v.clone(), Value::Bool(true));
    }
    Value::Object(o)
}

fn set_opt_flag(state: &mut State, key: &str, value: Value) {
    state.opt.insert(key.to_string(), value);
}

/// `state.opt.parallel.enable = true`.
fn enable_parallel(state: &mut State) {
    obj_entry(&mut state.opt, "parallel").insert("enable".into(), Value::Bool(true));
}

/// `if (!state.opt.track_repeat) {...}; state.opt.track_repeat[v] = true`.
fn track_repeat(state: &mut State, vars: &[String]) {
    let tr = obj_entry(&mut state.opt, "track_repeat");
    for v in vars {
        tr.insert(v.clone(), Value::Bool(true));
    }
}

/// The attribute handlers that start with `if (!this.tests) {this.tests = []; }`.
const TESTS_INITIALISING: [&str; 25] = [
    "@disambiguate",
    "@is-numeric",
    "@is-uncertain-date",
    "@locator",
    "@position",
    "@type",
    "@variable",
    "@page",
    "@number",
    "@jurisdiction",
    "@country",
    "@context",
    "@has-year-only",
    "@has-to-month-or-season",
    "@has-day",
    "@is-plural",
    "@is-multiple",
    "@locale",
    "@alternative-node-internal",
    "@locale-internal",
    "@court-class",
    "@container-multiple",
    "@container-subsequent",
    "@has-subunit",
    "@cite-form",
];

/// `CSL.Attributes[key].call(token, state, "" + arg)`. Returns `Ok(false)`
/// for an attribute upstream does not define (it only warns). Entry point
/// called by the build loop (`CSL.XmlToToken`) and `setStyleAttributes`.
/// `key` includes the leading `@` (`"@variable"`).
pub fn apply(state: &mut State, token: &mut Token, key: &str, arg: &str) -> CslResult<bool> {
    if TESTS_INITIALISING.contains(&key) {
        // `if (!this.tests) {this.tests = []; }` at the top of the handler.
        token.tests_defined = true;
    }
    match key {
        "@disambiguate" => {
            if arg == "true" {
                set_opt_flag(state, "has_disambiguate", Value::Bool(true));
                token.tests.push(atest(AttributesTest::Disambiguate));
            } else if arg == "check-ambiguity-and-backreference" {
                token.tests.push(atest(AttributesTest::DisambiguateBackref));
            }
        }
        "@is-numeric" => {
            for variable in split_ws(arg) {
                token
                    .tests
                    .push(atest(AttributesTest::IsNumeric { variable }));
            }
        }
        "@is-uncertain-date" => {
            for variable in split_ws(arg) {
                token
                    .tests
                    .push(atest(AttributesTest::IsUncertainDate { variable }));
            }
        }
        "@locator" => {
            let trylabels = arg.replacen("sub verbo", "sub-verbo", 1);
            for trylabel in split_ws(&trylabels) {
                token
                    .tests
                    .push(atest(AttributesTest::Locator { trylabel }));
            }
        }
        "@position" => {
            set_opt_flag(state, "update_mode", Value::from(POSITION));
            for tryposition in split_ws(arg) {
                // Keywords upstream maps to CSL.POSITION_*; anything else stays a
                // string upstream, which no comparison below can match
                // (numeric-looking strings are not supported).
                let mapped = match tryposition.as_str() {
                    "first" => Some(0),
                    "container-subsequent" => Some(4),
                    "subsequent" => Some(1),
                    "ibid" => Some(2),
                    "ibid-with-locator" => Some(3),
                    _ => None,
                };
                if tryposition == "near-note" {
                    token.tests.push(atest(AttributesTest::PositionNear));
                } else if tryposition == "far-note" {
                    token.tests.push(atest(AttributesTest::PositionFar));
                } else {
                    token.tests.push(atest(AttributesTest::Position {
                        tryposition: mapped,
                    }));
                }
            }
        }
        "@type" => {
            // XXX This is ALWAYS composed as an "any" match
            let tests: Vec<Test> = split_ws(arg)
                .into_iter()
                .map(|mytype| atest(AttributesTest::Type { mytype }))
                .collect();
            token.tests.push(util_conditions::match_test(
                util_conditions::MatchKind::Any,
                &tests,
            ));
        }
        "@variable" => {
            token.variables = split_ws(arg);
            token
                .extra
                .insert("variables_real".into(), strings_vec(&token.variables));
            // First the non-conditional code.
            if token.name == "label" && !token.variables[0].is_empty() {
                let term = token.variables[0].clone();
                token.set_string("term", &term);
            } else if ["names", "date", "text", "number"].contains(&token.name.as_str()) {
                token
                    .execs
                    .push(Exec::Attributes(AttributesExec::VariableSetNames));
                token
                    .execs
                    .push(Exec::Attributes(AttributesExec::VariableCheckOutput));
            } else if ["if", "else-if", "condition"].contains(&token.name.as_str()) {
                for variable in token.variables.clone() {
                    token
                        .tests
                        .push(atest(AttributesTest::Variable { variable }));
                }
            }
        }
        "@page" => {
            let trylabels = arg.replacen("sub verbo", "sub-verbo", 1);
            for trylabel in split_ws(&trylabels) {
                token.tests.push(atest(AttributesTest::Page { trylabel }));
            }
        }
        "@number" => {
            for trylabel in split_ws(arg) {
                token.tests.push(atest(AttributesTest::Number { trylabel }));
            }
        }
        "@jurisdiction" => {
            token.tests.push(atest(AttributesTest::Jurisdiction {
                tryjurisdictions: split_ws(arg),
            }));
        }
        "@country" => {
            token.tests.push(atest(AttributesTest::Country {
                trycountries: split_ws(arg),
            }));
        }
        "@context" => {
            token.tests.push(atest(AttributesTest::Context {
                arg: arg.to_string(),
            }));
        }
        "@has-year-only" => {
            for trydate in split_ws(arg) {
                token
                    .tests
                    .push(atest(AttributesTest::HasYearOnly { trydate }));
            }
        }
        "@has-to-month-or-season" => {
            for trydate in split_ws(arg) {
                token
                    .tests
                    .push(atest(AttributesTest::HasToMonthOrSeason { trydate }));
            }
        }
        "@has-day" => {
            for trydate in split_ws(arg) {
                token.tests.push(atest(AttributesTest::HasDay { trydate }));
            }
        }
        "@is-plural" => {
            token.tests.push(atest(AttributesTest::IsPlural {
                arg: arg.to_string(),
            }));
        }
        "@is-multiple" => {
            token.tests.push(atest(AttributesTest::IsMultiple {
                arg: arg.to_string(),
            }));
        }
        "@locale" => apply_locale(state, token, arg)?,
        "@alternative-node-internal" => {
            token
                .tests
                .push(atest(AttributesTest::AlternativeNodeInternal));
        }
        "@locale-internal" => apply_locale_internal(state, token, arg)?,
        "@court-class" => {
            for tryclass in split_ws(arg) {
                token
                    .tests
                    .push(atest(AttributesTest::CourtClass { tryclass }));
            }
        }
        "@container-multiple" => {
            token.tests.push(atest(AttributesTest::ContainerMultiple {
                retval: arg == "true",
            }));
        }
        "@container-subsequent" => {
            token.tests.push(atest(AttributesTest::ContainerSubsequent {
                retval: arg == "true",
            }));
        }
        "@has-subunit" => {
            token.tests.push(atest(AttributesTest::HasSubunit {
                namevar: arg.to_string(),
            }));
        }
        "@cite-form" => {
            token.tests.push(atest(AttributesTest::CiteForm {
                cite_form: arg.to_string(),
            }));
        }
        "@disable-duplicate-year-suppression" => {
            set_opt_flag(
                state,
                "disable_duplicate_year_suppression",
                strings_vec(&split_ws(arg)),
            );
        }
        "@consolidate-containers" => {
            track_containers(state, arg);
            state
                .bibliography
                .opt
                .insert("consolidate_containers".into(), strings_vec(&split_ws(arg)));
        }
        "@track-containers" => track_containers(state, arg),
        // These are not evaluated as conditions immediately: they only
        // set parameters that are picked up during processing.
        "@parallel-first" => {
            enable_parallel(state);
            let vars = split_ws(arg);
            track_repeat(state, &vars);
            token.extra.insert("parallel_first".into(), flag_map(&vars));
        }
        "@parallel-last" => {
            enable_parallel(state);
            let vars = split_ws(arg);
            track_repeat(state, &vars);
            token.extra.insert("parallel_last".into(), flag_map(&vars));
        }
        "@parallel-last-to-first" => {
            enable_parallel(state);
            let vars = split_ws(arg);
            token
                .extra
                .insert("parallel_last_to_first".into(), flag_map(&vars));
        }
        "@parallel-delimiter-override" => {
            enable_parallel(state);
            token.set_string("set_parallel_delimiter_override", arg);
        }
        "@parallel-delimiter-override-on-suppress" => {
            enable_parallel(state);
            token.set_string("set_parallel_delimiter_override_on_suppress", arg);
        }
        "@no-repeat" => {
            enable_parallel(state);
            let vars = split_ws(arg);
            track_repeat(state, &vars);
            token.extra.insert("non_parallel".into(), flag_map(&vars));
        }
        "@require" => {
            set_opt_flag(state, "use_context_condition", Value::Bool(true));
            token.set_string("require", arg);
        }
        "@reject" => {
            set_opt_flag(state, "use_context_condition", Value::Bool(true));
            token.set_string("reject", arg);
        }
        "@require-comma-on-symbol" => {
            set_opt_flag(
                state,
                "require_comma_on_symbol",
                Value::String(arg.to_string()),
            );
        }
        "@gender" => {
            token
                .extra
                .insert("gender".into(), Value::String(arg.to_string()));
        }
        "@cslid" => {
            // @cslid is a noop at run time; the id is kept for reverse lookup.
            token
                .extra
                .insert("cslid".into(), int_or_nan(js::parse_int(arg)));
        }
        "@capitalize-if-first" => token.set_string("capitalize_if_first_override", arg),
        "@label-capitalize-if-first" => token.set_string("label_capitalize_if_first_override", arg),
        "@label-form" => token.set_string("label_form_override", arg),
        "@part-separator" => token.set_string("part-separator", arg),
        "@leading-noise-words" => {
            token
                .extra
                .insert("leading-noise-words".into(), Value::String(arg.to_string()));
        }
        "@name-never-short" => {
            token
                .extra
                .insert("name-never-short".into(), Value::String(arg.to_string()));
        }
        "@class" => set_opt_flag(state, "class", Value::String(arg.to_string())),
        "@version" => set_opt_flag(state, "version", Value::String(arg.to_string())),
        "@value" => token.set_string("value", arg),
        "@name" => token.set_string("name", arg),
        "@form" => token.set_string("form", arg),
        "@date-parts" => token.set_string("date-parts", arg),
        "@range-delimiter" => token.set_string("range-delimiter", arg),
        "@macro" => token.postponed_macro = Some(arg.to_string()),
        "@term" => {
            if arg == "sub verbo" {
                token.set_string("term", "sub-verbo");
            } else {
                token.set_string("term", arg);
            }
        }
        "@xmlns" => {}
        "@lang" => {
            if !arg.is_empty() {
                state.build.lang = Some(arg.to_string());
            }
        }
        // Used as a flag during dates processing
        "@lingo" => {}
        "@macro-has-date" => {
            token
                .extra
                .insert("macro-has-date".into(), Value::Bool(true));
        }
        "@suffix" => token.set_string("suffix", arg),
        "@prefix" => token.set_string("prefix", arg),
        "@delimiter" => token.set_string("delimiter", arg),
        "@match" => {
            token
                .extra
                .insert("match".into(), Value::String(arg.to_string()));
        }
        "@names-min" => {
            let val = js::parse_int(arg);
            bump_max_names(state, val)?;
            token.strings.insert("et-al-min".into(), int_or_nan(val));
        }
        "@names-use-first" => {
            token
                .strings
                .insert("et-al-use-first".into(), int_or_nan(js::parse_int(arg)));
        }
        "@names-use-last" => {
            token
                .strings
                .insert("et-al-use-last".into(), Value::Bool(arg == "true"));
        }
        "@sort" => {
            if arg == "descending" {
                token
                    .strings
                    .insert("sort_direction".into(), Value::from(DESCENDING));
            }
        }
        "@plural" => {
            // Accepted values of plural attribute differ on cs:text
            // and cs:label nodes.
            if arg == "always" || arg == "true" {
                token.strings.insert("plural".into(), Value::from(1));
            } else if arg == "never" || arg == "false" {
                token.strings.insert("plural".into(), Value::from(0));
            } else if arg == "contextual" {
                token.strings.insert("plural".into(), Value::Bool(false));
            }
        }
        "@has-publisher-and-publisher-place" => {
            token.strings.insert(
                "has-publisher-and-publisher-place".into(),
                Value::Bool(true),
            );
        }
        "@publisher-delimiter-precedes-last" => {
            token.set_string("publisher-delimiter-precedes-last", arg)
        }
        "@publisher-delimiter" => token.set_string("publisher-delimiter", arg),
        "@publisher-and" => token.set_string("publisher-and", arg),
        "@givenname-disambiguation-rule" => {
            if GIVENNAME_DISAMBIGUATION_RULES.contains(&arg) {
                state.citation.opt.insert(
                    "givenname-disambiguation-rule".into(),
                    Value::String(arg.to_string()),
                );
            }
        }
        "@collapse" => {
            // only one collapse value will be honoured.
            if !arg.is_empty() {
                let name = token.name.clone();
                area_mut(state, &name)?
                    .opt
                    .insert("collapse".into(), Value::String(arg.to_string()));
            }
        }
        "@cite-group-delimiter" => {
            if !arg.is_empty() {
                let area = state.tmp.area.clone();
                area_mut(state, &area)?.opt.insert(
                    "cite_group_delimiter".into(),
                    Value::String(arg.to_string()),
                );
            }
        }
        "@names-delimiter" => set_opt(state, token, "names-delimiter", Value::String(arg.into())),
        "@name-form" => set_opt(state, token, "name-form", Value::String(arg.into())),
        "@subgroup-delimiter" => token.set_string("subgroup-delimiter", arg),
        "@subgroup-delimiter-precedes-last" => {
            token.set_string("subgroup-delimiter-precedes-last", arg)
        }
        "@name-delimiter" => set_opt(state, token, "name-delimiter", Value::String(arg.into())),
        "@et-al-min" => {
            let val = js::parse_int(arg);
            bump_max_names(state, val)?;
            set_opt(state, token, "et-al-min", int_or_nan(val));
        }
        "@et-al-use-first" => {
            set_opt(
                state,
                token,
                "et-al-use-first",
                int_or_nan(js::parse_int(arg)),
            );
        }
        "@et-al-use-last" => {
            set_opt(state, token, "et-al-use-last", Value::Bool(arg == "true"));
        }
        "@et-al-subsequent-min" => {
            let val = js::parse_int(arg);
            bump_max_names(state, val)?;
            set_opt(state, token, "et-al-subsequent-min", int_or_nan(val));
        }
        "@et-al-subsequent-use-first" => {
            set_opt(
                state,
                token,
                "et-al-subsequent-use-first",
                int_or_nan(js::parse_int(arg)),
            );
        }
        "@suppress-min" => {
            token
                .strings
                .insert("suppress-min".into(), int_or_nan(js::parse_int(arg)));
        }
        "@suppress-max" => {
            token
                .strings
                .insert("suppress-max".into(), int_or_nan(js::parse_int(arg)));
        }
        "@and" => set_opt(state, token, "and", Value::String(arg.into())),
        "@delimiter-precedes-last" => set_opt(
            state,
            token,
            "delimiter-precedes-last",
            Value::String(arg.into()),
        ),
        "@delimiter-precedes-et-al" => set_opt(
            state,
            token,
            "delimiter-precedes-et-al",
            Value::String(arg.into()),
        ),
        "@initialize-with" => set_opt(state, token, "initialize-with", Value::String(arg.into())),
        "@initialize" => {
            if arg == "false" {
                set_opt(state, token, "initialize", Value::Bool(false));
            }
        }
        "@name-as-reverse-order" => {
            token
                .extra
                .insert("name-as-reverse-order".into(), Value::String(arg.into()));
        }
        "@name-as-sort-order" => {
            if token.name == "style-options" {
                token
                    .extra
                    .insert("name-as-sort-order".into(), Value::String(arg.into()));
            } else {
                set_opt(
                    state,
                    token,
                    "name-as-sort-order",
                    Value::String(arg.into()),
                );
            }
        }
        "@sort-separator" => set_opt(state, token, "sort-separator", Value::String(arg.into())),
        "@require-match" => {
            if arg == "true" {
                token.extra.insert("requireMatch".into(), Value::Bool(true));
            }
        }
        "@exclude-types" => {
            state
                .bibliography
                .opt
                .insert("exclude_types".into(), strings_vec(&split_ws(arg)));
        }
        "@exclude-with-fields" => {
            state
                .bibliography
                .opt
                .insert("exclude_with_fields".into(), strings_vec(&split_ws(arg)));
        }
        "@year-suffix-delimiter" => {
            let name = token.name.clone();
            area_mut(state, &name)?
                .opt
                .insert("year-suffix-delimiter".into(), Value::String(arg.into()));
        }
        "@after-collapse-delimiter" => {
            let name = token.name.clone();
            area_mut(state, &name)?
                .opt
                .insert("after-collapse-delimiter".into(), Value::String(arg.into()));
        }
        "@subsequent-author-substitute" => {
            let name = token.name.clone();
            area_mut(state, &name)?.opt.insert(
                "subsequent-author-substitute".into(),
                Value::String(arg.into()),
            );
        }
        "@subsequent-author-substitute-rule" => {
            let name = token.name.clone();
            area_mut(state, &name)?.opt.insert(
                "subsequent-author-substitute-rule".into(),
                Value::String(arg.into()),
            );
        }
        "@disambiguate-add-names" => {
            if arg == "true" {
                set_opt_flag(state, "disambiguate-add-names", Value::Bool(true));
            }
        }
        "@disambiguate-add-givenname" => {
            if arg == "true" {
                set_opt_flag(state, "disambiguate-add-givenname", Value::Bool(true));
            }
        }
        "@disambiguate-add-year-suffix" => {
            if arg == "true" && state.opt.get("xclass").and_then(Value::as_str) != Some("numeric") {
                set_opt_flag(state, "disambiguate-add-year-suffix", Value::Bool(true));
            }
        }
        "@second-field-align" => {
            if arg == "flush" || arg == "margin" {
                let name = token.name.clone();
                area_mut(state, &name)?
                    .opt
                    .insert("second-field-align".into(), Value::String(arg.into()));
            }
        }
        "@hanging-indent" => {
            if arg == "true" {
                let legacy = state
                    .opt
                    .get("development_extensions")
                    .map(|d| js::truthy_opt(d.get("hanging_indent_legacy_number")))
                    .unwrap_or(false);
                let name = token.name.clone();
                area_mut(state, &name)?.opt.insert(
                    "hangingindent".into(),
                    if legacy {
                        Value::from(2)
                    } else {
                        Value::Bool(true)
                    },
                );
            }
        }
        "@line-spacing" => {
            if !arg.is_empty() && arg.chars().all(|c| c == '.' || c.is_ascii_digit()) {
                let name = token.name.clone();
                area_mut(state, &name)?
                    .opt
                    .insert("line-spacing".into(), parse_float_value(arg));
            }
        }
        "@entry-spacing" => {
            if !arg.is_empty() && arg.chars().all(|c| c == '.' || c.is_ascii_digit()) {
                let name = token.name.clone();
                area_mut(state, &name)?
                    .opt
                    .insert("entry-spacing".into(), parse_float_value(arg));
            }
        }
        "@near-note-distance" => {
            let name = token.name.clone();
            area_mut(state, &name)?
                .opt
                .insert("near-note-distance".into(), int_or_nan(js::parse_int(arg)));
        }
        "@substring" => {
            token
                .extra
                .insert("substring".into(), int_or_nan(js::parse_int(arg)));
        }
        "@text-case" => {
            token.execs.push(Exec::Attributes(AttributesExec::TextCase {
                arg: arg.to_string(),
            }));
        }
        "@page-range-format" => set_opt_flag(state, "page-range-format", Value::String(arg.into())),
        "@year-range-format" => set_opt_flag(state, "year-range-format", Value::String(arg.into())),
        "@default-locale" => apply_default_locale(state, token, arg)?,
        "@default-locale-sort" => {
            set_opt_flag(state, "default-locale-sort", Value::String(arg.into()))
        }
        "@demote-non-dropping-particle" => {
            set_opt_flag(
                state,
                "demote-non-dropping-particle",
                Value::String(arg.into()),
            );
        }
        "@initialize-with-hyphen" => {
            if arg == "false" {
                set_opt_flag(state, "initialize-with-hyphen", Value::Bool(false));
            }
        }
        "@institution-parts" => token.set_string("institution-parts", arg),
        "@if-short" => {
            if arg == "true" {
                token.strings.insert("if-short".into(), Value::Bool(true));
            }
        }
        "@substitute-use-first" => {
            token.strings.insert(
                "substitute-use-first".into(),
                int_or_nan(js::parse_int(arg)),
            );
        }
        "@use-first" => {
            token
                .strings
                .insert("use-first".into(), int_or_nan(js::parse_int(arg)));
        }
        "@use-last" => {
            token
                .strings
                .insert("use-last".into(), int_or_nan(js::parse_int(arg)));
        }
        "@stop-first" => {
            token
                .strings
                .insert("stop-first".into(), int_or_nan(js::parse_int(arg)));
        }
        "@stop-last" => {
            token.strings.insert(
                "stop-last".into(),
                int_or_nan(js::parse_int(arg).map(|n| -n)),
            );
        }
        "@reverse-order" => {
            if arg == "true" {
                token
                    .strings
                    .insert("reverse-order".into(), Value::Bool(true));
            }
        }
        "@display" => {
            if state.bibliography.tokens.len() == 2 {
                set_opt_flag(state, "using_display", Value::Bool(true));
            }
            token.set_string("cls", arg);
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/// `state[state.build.area].opt.max_number_of_names < val` then assign.
fn bump_max_names(state: &mut State, val: Option<i64>) -> CslResult<()> {
    let Some(val) = val else {
        return Ok(());
    };
    let area = state.build.area.clone();
    let opt = &mut area_mut(state, &area)?.opt;
    let cur = opt.get("max_number_of_names").and_then(Value::as_i64);
    if cur.map(|c| c < val).unwrap_or(false) {
        opt.insert("max_number_of_names".into(), Value::from(val));
    }
    Ok(())
}

/// `@track-containers` (also called by `@consolidate-containers`).
fn track_containers(state: &mut State, arg: &str) {
    let args = split_ws(arg);
    let opt = &mut state.bibliography.opt;
    let tci = arr_entry(opt, "track_container_items");
    for a in &args {
        tci.push(Value::String(a.clone()));
    }
    arr_entry(opt, "consolidate_containers");
}

/// `@locale` (attributes.js:~690).
fn apply_locale(state: &mut State, token: &mut Token, arg: &str) -> CslResult<()> {
    // Style default
    let locale_default = default_locale(state);
    if token.name == "layout" {
        // For layout
        token
            .extra
            .insert("locale_raw".into(), Value::String(arg.to_string()));
        if token.tokentype == TokenType::Start {
            arr_entry(&mut state.opt, "multi_layout");
            let mut locale_data: Vec<Value> = Vec::new();
            // Register the primary locale in the set, and others that "map" to it,
            // so that they can be used when generating sort keys. See node_sort.js.
            let locales = split_ws(arg);
            let mut sort_locale = Obj::new();
            let master = locale_resolve(&locales[0], &locale_default);
            locale_data.push(master.to_value());
            // Upstream keys by `localeMaster.generic`, a boolean: the key is
            // the string "true". Kept as is.
            let key = if master.generic {
                "true".to_string()
            } else {
                master.best.clone()
            };
            sort_locale.insert(key, Value::String(master.best.clone()));
            for l in &locales[1..] {
                let servant = locale_resolve(l, &locale_default);
                locale_data.push(servant.to_value());
                let key = if servant.generic {
                    "true".to_string()
                } else {
                    servant.best.clone()
                };
                sort_locale.insert(key, Value::String(master.best.clone()));
            }
            let area = state.build.area.clone();
            arr_entry(&mut area_mut(state, &area)?.opt, "sort_locales")
                .push(Value::Object(sort_locale));
            arr_entry(&mut state.opt, "multi_layout").push(Value::Array(locale_data));
        }
        set_opt_flag(state, "has_layout_locale", Value::Bool(true));
    } else {
        // For if and if-else
        let lst = split_ws(arg);
        let mut locale_bares: Vec<String> = Vec::new();
        let mut locale_list: Vec<LocaleSpec> = Vec::new();
        for lang in &lst {
            let langspec = locale_resolve(lang, &locale_default);
            if js::len(lang) == 2 {
                // For fallback
                locale_bares.push(langspec.bare.clone());
            }
            // Load the locale terms etc.
            // (second argument causes immediate return if locale already exists)
            locale_configure(state, &langspec, true)?;
            locale_list.push(langspec);
        }
        token.tests.push(atest(AttributesTest::Locale {
            locale_list,
            locale_default,
            locale_bares,
        }));
    }
    Ok(())
}

/// `@locale-internal` (attributes.js:~760).
fn apply_locale_internal(state: &mut State, token: &mut Token, arg: &str) -> CslResult<()> {
    let lst = split_ws(arg);
    let dl = default_locale(state);
    let mut locale_bares: Vec<String> = Vec::new();
    let mut locale_list: Vec<LocaleSpec> = Vec::new();
    for lang in &lst {
        let langspec = locale_resolve(lang, &dl);
        if js::len(lang) == 2 {
            // For fallback
            locale_bares.push(langspec.bare.clone());
        }
        // Load the locale terms etc.
        locale_configure(state, &langspec, false)?;
        locale_list.push(langspec);
    }
    // Set locale tag on node
    token
        .extra
        .insert("locale_bares".into(), strings_vec(&locale_bares));
    token
        .extra
        .insert("locale_default".into(), Value::String(dl));
    // The locale to set on node children if match is successful
    let locale = locale_list[0].best.clone();
    token
        .extra
        .insert("locale".into(), Value::String(locale.clone()));
    // Locales to test
    token.extra.insert(
        "locale_list".into(),
        Value::Array(locale_list.iter().map(LocaleSpec::to_value).collect()),
    );
    token.tests.push(atest(AttributesTest::LocaleInternal {
        locale_list,
        locale_bares,
        locale,
    }));
    Ok(())
}

/// `@default-locale` (attributes.js:~1545).
fn apply_default_locale(state: &mut State, token: &mut Token, arg: &str) -> CslResult<()> {
    if token.name == "style" {
        static SPLIT_RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new("-x-(?:sort|translit|translat)-").expect("static regex"));
        let ms: Vec<String> = SPLIT_RE
            .find_iter(arg)
            .map(|m| {
                let s = m.as_str();
                // .replace(/^-x-/, "").replace(/-$/, "")
                s.trim_start_matches("-x-")
                    .trim_end_matches('-')
                    .to_string()
            })
            .collect();
        let lst0 = js::split(&SPLIT_RE, arg);
        let mut ret: Vec<String> = vec![lst0[0].clone()];
        for pos in 1..lst0.len() {
            ret.push(ms.get(pos - 1).cloned().unwrap_or_default());
            ret.push(lst0[pos].clone());
        }
        let len = ret.len();
        let mut pos = 1;
        while pos < len {
            let key = format!("locale-{}", ret[pos]);
            let val = ret.get(pos + 1).cloned().unwrap_or_default();
            // .replace(/^\s*/g, "").replace(/\s*$/g, "")
            let val = js::trim(&val).to_string();
            arr_entry(&mut state.opt, &key).push(Value::String(val));
            pos += 2;
        }
        // `if (lst.length)` is always true: lst has at least one element.
        state.opt.insert(
            "default-locale".into(),
            Value::Array(vec![Value::String(ret[0].clone())]),
        );
    } else if arg == "true" {
        token
            .extra
            .insert("default_locale".into(), Value::Bool(true));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Differential test of the attribute handlers and node builders against
    //! citeproc-js 2.4.63 (`scripts/csl-units/build_tokens.cjs`, data in
    //! `tests/data/csl/units/build_tokens.json`).
    //!
    //! **Method.** Each case is a tree of CSL elements. `scripts/csl-units/
    //! build_tokens.cjs` serialises it to a style, lets citeproc-js build it
    //! (with the XML name/institution/publisher normalisations switched off, so
    //! the token lists are what the node builders make of exactly that tree),
    //! and dumps the five token lists plus the options the handlers write.
    //! This test replays the tree the way `CSL.makeBuilder` / `CSL.XmlToToken`
    //! do (attributes applied in document order by [`apply`], then the node
    //! builder), runs the jump-index pass (`configureTokenList`), and compares
    //! per token: name, tokentype, `execs_n`, `tests_n` (present only when the
    //! `tests` array exists), `has_test`, `strings`, `variables`, every other
    //! data property, `next`/`succeed`/`fail`; plus the compared option keys,
    //! `cite_affixes` and `date_key`. Decorations are not compared (the driver
    //! does not run `CSL.setDecorations`).
    //!
    //! **Pass criterion.** Every compared case matches exactly. Cases that need
    //! machinery outside this wave (macro expansion, locale loading, date
    //! template merging, `getTerm`) end in `NotYetPorted` and are skipped; the
    //! exact skip list is asserted so a newly failing case cannot hide there.
    //!
    //! **Results (2026-10-08).** 84 cases generated; 1 rejected by citeproc-js
    //! itself (`intext_basic`, upstream throws); 7 skipped (see `EXPECTED_SKIPS`);
    //! the rest match.
    use super::*;
    use crate::citeproc::stack::Stack;
    use crate::citeproc::{
        node_alternative, node_alternativetext, node_bibliography, node_choose, node_citation,
        node_comment, node_condition, node_conditions, node_date, node_datepart, node_else,
        node_elseif, node_etal, node_group, node_if, node_info, node_institution,
        node_institutionpart, node_intext, node_key, node_label, node_layout, node_macro,
        node_name, node_namepart, node_names, node_number, node_sort, node_substitute, node_text,
    };
    use std::sync::Arc;

    const DATA: &str = include_str!("../../tests/data/csl/units/build_tokens.json");

    /// Cases that end in `NotYetPorted` today, and why.
    const EXPECTED_SKIPS: [&str; 7] = [
        "macro_in_text",                    // CSL.expandMacro (util_nodes.js)
        "layout_locale",                    // localeConfigure (util_locale.js)
        "if_locale",                        // localeConfigure
        "sort_key_macro",                   // CSL.expandMacro
        "date_with_form",                   // CSL.Util.fixDateNode host (xmljson.js)
        "text_collapse_citation_number",    // state.getTerm (build.js)
        "text_collapse_year_suffix_ranged", // state.getTerm
    ];

    fn obj(v: Value) -> Obj {
        match v {
            Value::Object(o) => o,
            _ => Obj::new(),
        }
    }

    /// `CSL.Engine` constructor defaults (state.js) for the fields the
    /// handlers and builders touch.
    fn fresh_state(class: &str) -> CslResult<State> {
        let mut s = State::default();
        s.opt = obj(serde_json::json!({
            "parallel": {"enable": false},
            "has_disambiguate": false,
            "inheritedAttributes": {},
            "locale-sort": [], "locale-translit": [], "locale-translat": [],
            "update_mode": 0, "bib_mode": 0, "sort_citations": false,
            "has_layout_locale": false, "disable_duplicate_year_suppression": [],
            "use_context_condition": false,
            "initialize-with-hyphen": true,
            "demote-non-dropping-particle": "display-and-sort",
            "default-locale": ["en-US"], "lang": "en-US", "xclass": class,
            "development_extensions": {},
        }));
        let common = |a: &mut Area| {
            a.opt = obj(serde_json::json!({
                "inheritedAttributes": {}, "collapse": [], "topdecor": [],
                "layout_decorations": [], "layout_prefix": "", "layout_suffix": "",
                "layout_delimiter": "", "sort_locales": [], "max_number_of_names": 0,
            }));
        };
        common(&mut s.citation);
        s.citation
            .opt
            .insert("givenname-disambiguation-rule".into(), "by-cite".into());
        s.citation.opt.insert("near-note-distance".into(), 5.into());
        common(&mut s.intext);
        s.intext
            .opt
            .insert("givenname-disambiguation-rule".into(), "by-cite".into());
        s.intext.opt.insert("near-note-distance".into(), 5.into());
        common(&mut s.bibliography);
        s.bibliography.opt.insert("line-spacing".into(), 1.into());
        s.bibliography.opt.insert("entry-spacing".into(), 1.into());
        for a in [&mut s.citation_sort, &mut s.bibliography_sort] {
            a.opt = obj(serde_json::json!({"sort_directions": [], "topdecor": []}));
        }
        s.citation.root = "citation".into();
        s.intext.root = "intext".into();
        s.bibliography.root = "bibliography".into();
        s.citation_sort.root = "citation".into();
        s.bibliography_sort.root = "bibliography".into();
        s.tmp.area = "citation".into();
        s.tmp.root = Some("citation".into());
        s.tmp.cite_affixes = obj(serde_json::json!({
            "citation": false, "bibliography": false,
            "citation_sort": false, "bibliography_sort": false,
        }));
        s.build.area = "citation".into();
        s.build.root = "citation".into();
        s.build.substitute_level = Stack::with(0);
        // setStyleAttributes: class, version, default-locale of <style>.
        let mut style = Token::new("style", TokenType::Start);
        apply(&mut s, &mut style, "@class", class)?;
        apply(&mut s, &mut style, "@version", "1.0")?;
        apply(&mut s, &mut style, "@default-locale", "en-US")?;
        // build.js:121: a style without @sort-separator gets ", ".
        apply(&mut s, &mut style, "@sort-separator", ", ")?;
        s.opt
            .insert("default-locale-sort".into(), Value::String("en-US".into()));
        Ok(s)
    }

    fn dispatch_build(state: &mut State, token: Token, target: &mut Vec<Token>) -> CslResult<()> {
        macro_rules! go {
            ($m:ident) => {
                $m::build(state, token, target, true)
            };
        }
        match token.name.clone().as_str() {
            "alternative" => go!(node_alternative),
            "alternative-text" => go!(node_alternativetext),
            "bibliography" => go!(node_bibliography),
            "choose" => go!(node_choose),
            "citation" => go!(node_citation),
            "#comment" => go!(node_comment),
            "condition" => go!(node_condition),
            "conditions" => go!(node_conditions),
            "date" => go!(node_date),
            "date-part" => go!(node_datepart),
            "else" => go!(node_else),
            "else-if" => go!(node_elseif),
            "et-al" => go!(node_etal),
            "group" => go!(node_group),
            "if" => go!(node_if),
            "info" => go!(node_info),
            "institution" => go!(node_institution),
            "institution-part" => go!(node_institutionpart),
            "intext" => go!(node_intext),
            "key" => go!(node_key),
            "label" => go!(node_label),
            "layout" => go!(node_layout),
            "macro" => go!(node_macro),
            "name" => go!(node_name),
            "name-part" => go!(node_namepart),
            "names" => go!(node_names),
            "number" => go!(node_number),
            "sort" => go!(node_sort),
            "substitute" => go!(node_substitute),
            "text" => go!(node_text),
            other => Err(EngineError::Csl(format!(
                "Undefined node name \"{other}\"."
            ))),
        }
    }

    fn dispatch_configure(state: &mut State, tokens: &mut [Token], pos: usize) -> CslResult<()> {
        match tokens[pos].name.clone().as_str() {
            "choose" => node_choose::configure(state, tokens, pos),
            "else" => node_else::configure(state, tokens, pos),
            "if" => node_if::configure(state, tokens, pos),
            "else-if" => node_elseif::configure(state, tokens, pos),
            "institution" => node_institution::configure(state, tokens, pos),
            _ => Ok(()),
        }
    }

    /// `CSL.Engine.prototype.configureTokenList`.
    fn configure_token_list(state: &mut State, tokens: &mut [Token]) -> CslResult<()> {
        let mut dateparts: Vec<String> = Vec::new();
        for ppos in (0..tokens.len()).rev() {
            if tokens[ppos].name == "date" && tokens[ppos].tokentype == TokenType::End {
                dateparts = Vec::new();
            }
            if tokens[ppos].name == "date-part" && js::truthy_opt(tokens[ppos].strings.get("name"))
            {
                let name = tokens[ppos].string("name");
                for part in ["year", "month", "day"] {
                    if part == name {
                        dateparts.push(name.clone());
                    }
                }
            }
            if tokens[ppos].name == "date" && tokens[ppos].tokentype == TokenType::Start {
                dateparts.reverse();
                tokens[ppos].extra.insert(
                    "dateparts".into(),
                    Value::Array(dateparts.iter().cloned().map(Value::String).collect()),
                );
            }
            tokens[ppos].next = Some(ppos + 1);
            dispatch_configure(state, tokens, ppos)?;
        }
        Ok(())
    }

    fn with_area_list<R>(
        state: &mut State,
        area: &str,
        f: impl FnOnce(&mut State, &mut Vec<Token>) -> CslResult<R>,
    ) -> CslResult<R> {
        let mut list = std::mem::take(Arc::make_mut(&mut area_mut(state, area)?.tokens));
        let r = f(state, &mut list);
        *Arc::make_mut(&mut area_mut(state, area)?.tokens) = list;
        r
    }

    /// `CSL.XmlToToken` for one element.
    fn xml_to_token(
        state: &mut State,
        node: &Value,
        tokentype: TokenType,
        area: &str,
        var_stack: &mut Vec<Vec<String>>,
    ) -> CslResult<()> {
        let name = node["n"].as_str().unwrap_or_default().to_string();
        if let Some(skip) = state.build.skip.clone() {
            if skip != name {
                return Ok(());
            }
        }
        let attrs: Vec<(String, String)> = node["a"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|p| {
                        (
                            format!("@{}", p[0].as_str().unwrap_or_default()),
                            p[1].as_str().unwrap_or_default().to_string(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let attr = |k: &str| attrs.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone());
        let mut token = Token::new(&name, tokentype);
        if tokentype != TokenType::End || ["if", "else-if", "layout"].contains(&name.as_str()) {
            for (key, val) in &attrs {
                if tokentype == TokenType::End && key != "@language" && key != "@locale" {
                    continue;
                }
                apply(state, &mut token, key, val)?;
            }
            if attr("@variable")
                .map(|v| DATE_VARIABLES.contains(&v.as_str()))
                .unwrap_or(false)
            {
                var_stack.push(token.variables.clone());
            }
        } else if tokentype == TokenType::End && attr("@variable").is_some() {
            token.extra.insert("hasVariable".into(), Value::Bool(true));
            if DATE_VARIABLES.contains(&attr("@variable").unwrap_or_default().as_str()) {
                token.variables = var_stack.pop().unwrap_or_default();
            }
        }
        with_area_list(state, area, |state, list| {
            dispatch_build(state, token, list)
        })
    }

    /// `buildStyle` of `CSL.makeBuilder`.
    fn build_style(
        state: &mut State,
        nodes: &[Value],
        parent: bool,
        area: &str,
        var_stack: &mut Vec<Vec<String>>,
    ) -> CslResult<()> {
        for node in nodes {
            let name = node["n"].as_str().unwrap_or_default();
            if parent && name == "date" {
                // CSL.Util.fixDateNode: raises date_key; with a `form` it merges the
                // locale template (not available here), without one it returns early.
                state.build.date_key = true;
                if node["a"]
                    .as_array()
                    .map(|a| a.iter().any(|p| p[0] == "form"))
                    .unwrap_or(false)
                {
                    return Err(EngineError::NotYetPorted {
                        method: "util_datenode.js fixDateNode host",
                    });
                }
            }
            let children = node["c"].as_array().cloned().unwrap_or_default();
            if !children.is_empty() {
                xml_to_token(state, node, TokenType::Start, area, var_stack)?;
                build_style(state, &children, true, area, var_stack)?;
                xml_to_token(state, node, TokenType::End, area, var_stack)?;
            } else {
                xml_to_token(state, node, TokenType::Singleton, area, var_stack)?;
            }
        }
        Ok(())
    }

    fn build_case(case: &Value) -> CslResult<State> {
        let class = case["class"].as_str().unwrap_or("in-text");
        let mut state = fresh_state(class)?;
        let nodes = case["nodes"].as_array().cloned().unwrap_or_default();
        if case.get("macros").is_some() {
            return Err(EngineError::NotYetPorted {
                method: "util_nodes.js CSL.expandMacro",
            });
        }
        for area in ["citation", "bibliography", "intext"] {
            state.build.area = area.to_string();
            let Some(node) = nodes.iter().find(|n| n["n"] == area) else {
                continue;
            };
            let mut var_stack = Vec::new();
            build_style(
                &mut state,
                std::slice::from_ref(node),
                false,
                area,
                &mut var_stack,
            )?;
        }
        for area in [
            "citation",
            "citation_sort",
            "bibliography",
            "bibliography_sort",
            "intext",
        ] {
            let mut list = std::mem::take(Arc::make_mut(&mut area_mut(&mut state, area)?.tokens));
            let r = configure_token_list(&mut state, &mut list);
            *Arc::make_mut(&mut area_mut(&mut state, area)?.tokens) = list;
            r?;
        }
        Ok(state)
    }

    fn strip_decorations(v: &mut Value) {
        match v {
            Value::Object(o) => {
                o.remove("decorations");
                o.values_mut().for_each(strip_decorations);
            }
            Value::Array(a) => a.iter_mut().for_each(strip_decorations),
            _ => {}
        }
    }

    fn dump_token(t: &Token) -> Value {
        let mut v = super::super::node_names::token_to_value(t);
        strip_decorations(&mut v);
        if let Value::Object(o) = &mut v {
            for (k, x) in [("next", t.next), ("succeed", t.succeed), ("fail", t.fail)] {
                if let Some(i) = x {
                    o.insert(k.into(), Value::from(i));
                }
            }
        }
        v
    }

    fn pick(opt: &Obj, keys: &[Value]) -> Obj {
        let mut out = Obj::new();
        for k in keys {
            if let Some(k) = k.as_str() {
                if let Some(v) = opt.get(k) {
                    out.insert(k.to_string(), v.clone());
                }
            }
        }
        out
    }

    #[test]
    fn token_lists_match_citeproc_js() {
        let data: Value = serde_json::from_str(DATA).expect("build_tokens.json parses");
        let opt_keys = data["meta"]["opt_keys"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let area_opt_keys = data["meta"]["area_opt_keys"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let cases = data["cases"].as_array().cloned().unwrap_or_default();
        assert!(cases.len() >= 80);
        let (mut compared, mut skipped, mut rejected) = (0, Vec::new(), 0);
        let mut failures: Vec<String> = Vec::new();
        for case in &cases {
            let id = case["id"].as_str().unwrap_or("?").to_string();
            if case.get("error").is_some() {
                rejected += 1;
                continue;
            }
            let state = match build_case(case) {
                Ok(s) => s,
                Err(EngineError::NotYetPorted { .. }) => {
                    skipped.push(id);
                    continue;
                }
                Err(e) => {
                    failures.push(format!("{id}: Rust build failed: {e}"));
                    continue;
                }
            };
            compared += 1;
            let exp = &case["expected"];
            for area in [
                "citation",
                "citation_sort",
                "bibliography",
                "bibliography_sort",
                "intext",
            ] {
                let rust: Vec<Value> = area_ref(&state, area)
                    .map(|a| a.tokens.iter().map(dump_token).collect())
                    .unwrap_or_default();
                let want = exp["areas"][area]["tokens"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                if rust.len() != want.len() {
                    failures.push(format!(
                        "{id}: {area}: {} tokens, citeproc-js has {}",
                        rust.len(),
                        want.len()
                    ));
                    continue;
                }
                for (i, (r, w)) in rust.iter().zip(&want).enumerate() {
                    if r != w {
                        failures.push(format!("{id}: {area}[{i}]\n   rust {r}\n   js   {w}"));
                    }
                }
                let ropt = Value::Object(pick(
                    &area_ref(&state, area)
                        .map(|a| a.opt.clone())
                        .unwrap_or_default(),
                    &area_opt_keys,
                ));
                if ropt != exp["areas"][area]["opt"] {
                    failures.push(format!(
                        "{id}: {area}.opt\n   rust {ropt}\n   js   {}",
                        exp["areas"][area]["opt"]
                    ));
                }
            }
            let ropt = Value::Object(pick(&state.opt, &opt_keys));
            if ropt != exp["opt"] {
                failures.push(format!("{id}: opt\n   rust {ropt}\n   js   {}", exp["opt"]));
            }
            let ca = Value::Object(state.tmp.cite_affixes.clone());
            if ca != exp["cite_affixes"] {
                failures.push(format!(
                    "{id}: cite_affixes\n   rust {ca}\n   js   {}",
                    exp["cite_affixes"]
                ));
            }
            if state.build.date_key != exp["date_key"].as_bool().unwrap_or(false) {
                failures.push(format!("{id}: build.date_key differs"));
            }
        }
        assert!(
            failures.is_empty(),
            "{} mismatches:\n{}",
            failures.len(),
            failures.join("\n")
        );
        assert_eq!(skipped, EXPECTED_SKIPS.to_vec());
        assert_eq!(rejected, 1);
        assert!(compared >= 75, "compared {compared}");
    }
}
