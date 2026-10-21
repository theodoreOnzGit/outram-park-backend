// Part of the kovan port of citeproc-js (GitHub #790, #791).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/api_control.js (the Engine constructor, setOutputFormat,
//              setLangPrefsForCites, setLangPrefsForCiteAffixes),
//              src/api_update.js (updateItems), src/api_cite.js
//              (makeCitationCluster, processCitationCluster,
//              appendCitationCluster), src/api_bibliography.js
//              (makeBibliography), src/system.js (the sys object)
// Version:     2.4.63, commit 73bc1b44bc7d54d0bfec4e070fd27f5efe024ff9
// Copyright:   (c) 2009-2019 Frank Bennett
// Licence:     AGPL-3.0, taken from upstream's "CPAL-1.0 or AGPL-3.0-or-later"
//              (LICENSE at the commit above; see this crate's NOTICE).
// Modified:    2026-10-08, by the OUTRAM PARK contributors. This file is a
//              Rust translation (port) of the files named above, modified
//              from the original.
// No warranty: this program is distributed in the hope that it will be
//              useful, but WITHOUT ANY WARRANTY; without even the implied
//              warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
//              PURPOSE. See the GNU Affero General Public License.

//! A Rust port of citeproc-js 2.4.63, Zotero's CSL engine (epic #790).
//!
//! **Status: the engine API is ported (#795, #796).** [`Engine::new`] parses
//! the style, merges its locales and builds the token lists, as `new
//! CSL.Engine` does; the processing methods ([`Engine::update_items`],
//! [`Engine::make_citation_cluster`], [`Engine::process_citation_cluster`],
//! [`Engine::append_citation_cluster`], [`Engine::make_bibliography`], ...)
//! run citeproc-js's registry, disambiguation, sorting and cite-position
//! machinery. What they can render depends on the rendering stages: a method
//! reaching a part of citeproc-js that is not ported yet (`grep -rn
//! PORT-LATER src/citeproc/`) returns [`EngineError::NotYetPorted`].
//! `tests/citeproc_test_suite.rs` is the harness the port is verified
//! against: it runs the CSL test suite's fixtures through this [`Engine`] the
//! way `scripts/csl-testsuite-reference.cjs` runs them through citeproc-js,
//! and compares with what citeproc-js produced.
//!
//! # API (names mirror citeproc-js)
//!
//! | citeproc-js | here |
//! |---|---|
//! | the `sys` object (`retrieveItem`, `retrieveLocale`, abbreviations) | [`Sys`] |
//! | `new CSL.Engine(sys, style, lang)` | [`Engine::new`] |
//! | `setOutputFormat` | [`Engine::set_output_format`] |
//! | `opt.development_extensions[k] = v` | [`Engine::set_development_extension`] |
//! | `variableWrapper` on sys | [`Engine::set_variable_wrapper`] |
//! | `citation.opt.suppressTrailingPunctuation` | [`Engine::set_suppress_trailing_punctuation`] |
//! | `setLangPrefsForCites`, `setLangPrefsForCiteAffixes` | [`Engine::set_lang_prefs_for_cites`], [`Engine::set_lang_prefs_for_cite_affixes`] |
//! | `setLangTagsForCslTranslation`, `...Transliteration` | [`Engine::set_lang_tags_for_csl_translation`], [`Engine::set_lang_tags_for_csl_transliteration`] |
//! | `updateItems(ids, nosort)` | [`Engine::update_items`] |
//! | `registry.reflist` ids, `registry.citationreg.citationById` | [`Engine::registry_ids`], [`Engine::citation_registered`] |
//! | the test runner's `preloadAbbreviations` | [`Engine::preload_abbreviations`] |
//! | `makeCitationCluster(items)` | [`Engine::make_citation_cluster`] |
//! | `processCitationCluster(citation, pre, post)` | [`Engine::process_citation_cluster`] |
//! | `appendCitationCluster(citation)` | [`Engine::append_citation_cluster`] |
//! | `makeBibliography(bibsection?)` | [`Engine::make_bibliography`] |
//!
//! Items, citation items and the bibliography parameters stay
//! [`serde_json::Value`]s because CSL-JSON is open-ended and citeproc-js reads
//! and writes them as plain objects.

// ---- the port's modules (PORTING.md §2): one per upstream file, plus js and exec ----
// Internal while the port is in progress (epic #790); dead code is expected
// until the stages that call it land.
#[allow(dead_code)]
pub(crate) mod api_bibliography;
#[allow(dead_code)]
pub(crate) mod api_cite;
#[allow(dead_code)]
pub(crate) mod api_control;
#[allow(dead_code)]
pub(crate) mod api_update;
#[allow(dead_code)]
pub(crate) mod attributes;
#[allow(dead_code)]
pub(crate) mod build;
#[allow(dead_code)]
pub(crate) mod build_retrieve_item;
#[allow(dead_code)]
pub(crate) mod disambig_citations;
#[allow(dead_code)]
pub(crate) mod disambig_cites;
#[allow(dead_code)]
pub(crate) mod disambig_names;
#[doc(hidden)]
pub mod dump;
#[allow(dead_code)]
pub(crate) mod exec;
#[allow(dead_code)]
pub(crate) mod formats;
pub(crate) mod greek_upper;
#[allow(dead_code)]
pub(crate) mod formatters;
#[allow(dead_code)]
pub(crate) mod js;
#[allow(dead_code)]
pub(crate) mod load;
#[allow(dead_code)]
pub(crate) mod node_alternative;
#[allow(dead_code)]
pub(crate) mod node_alternativetext;
#[allow(dead_code)]
pub(crate) mod node_bibliography;
#[allow(dead_code)]
pub(crate) mod node_choose;
#[allow(dead_code)]
pub(crate) mod node_citation;
#[allow(dead_code)]
pub(crate) mod node_comment;
#[allow(dead_code)]
pub(crate) mod node_condition;
#[allow(dead_code)]
pub(crate) mod node_conditions;
#[allow(dead_code)]
pub(crate) mod node_date;
#[allow(dead_code)]
pub(crate) mod node_datepart;
#[allow(dead_code)]
pub(crate) mod node_else;
#[allow(dead_code)]
pub(crate) mod node_elseif;
#[allow(dead_code)]
pub(crate) mod node_etal;
#[allow(dead_code)]
pub(crate) mod node_group;
#[allow(dead_code)]
pub(crate) mod node_if;
#[allow(dead_code)]
pub(crate) mod node_info;
#[allow(dead_code)]
pub(crate) mod node_institution;
#[allow(dead_code)]
pub(crate) mod node_institutionpart;
#[allow(dead_code)]
pub(crate) mod node_intext;
#[allow(dead_code)]
pub(crate) mod node_key;
#[allow(dead_code)]
pub(crate) mod node_label;
#[allow(dead_code)]
pub(crate) mod node_layout;
#[allow(dead_code)]
pub(crate) mod node_macro;
#[allow(dead_code)]
pub(crate) mod node_name;
#[allow(dead_code)]
pub(crate) mod node_namepart;
#[allow(dead_code)]
pub(crate) mod node_names;
#[allow(dead_code)]
pub(crate) mod node_number;
#[allow(dead_code)]
pub(crate) mod node_sort;
#[allow(dead_code)]
pub(crate) mod node_substitute;
#[allow(dead_code)]
pub(crate) mod node_text;
#[allow(dead_code)]
pub(crate) mod obj_ambigconfig;
#[allow(dead_code)]
pub(crate) mod obj_blob;
#[allow(dead_code)]
pub(crate) mod obj_number;
#[allow(dead_code)]
pub(crate) mod obj_token;
#[allow(dead_code)]
pub(crate) mod queue;
#[allow(dead_code)]
pub(crate) mod registry;
#[allow(dead_code)]
pub(crate) mod sort;
#[allow(dead_code)]
pub(crate) mod stack;
#[allow(dead_code)]
pub(crate) mod state;
#[allow(dead_code)]
pub(crate) mod system;
#[allow(dead_code)]
pub(crate) mod util;
#[allow(dead_code)]
pub(crate) mod util_citationlabel;
#[allow(dead_code)]
pub(crate) mod util_conditions;
#[allow(dead_code)]
pub(crate) mod util_date;
#[allow(dead_code)]
pub(crate) mod util_datenode;
#[allow(dead_code)]
pub(crate) mod util_dateparser;
#[allow(dead_code)]
pub(crate) mod util_dates;
#[allow(dead_code)]
pub(crate) mod util_disambig;
#[allow(dead_code)]
pub(crate) mod util_flipflop;
#[allow(dead_code)]
pub(crate) mod util_integration;
#[allow(dead_code)]
pub(crate) mod util_label;
#[allow(dead_code)]
pub(crate) mod util_locale;
#[allow(dead_code)]
pub(crate) mod util_locale_sniff;
#[allow(dead_code)]
pub(crate) mod util_modules;
#[allow(dead_code)]
pub(crate) mod util_name_particles;
#[allow(dead_code)]
pub(crate) mod util_names;
#[allow(dead_code)]
pub(crate) mod util_names_common;
#[allow(dead_code)]
pub(crate) mod util_names_constraints;
#[allow(dead_code)]
pub(crate) mod util_names_disambig;
#[allow(dead_code)]
pub(crate) mod util_names_divide;
#[allow(dead_code)]
pub(crate) mod util_names_etal;
#[allow(dead_code)]
pub(crate) mod util_names_etalconfig;
#[allow(dead_code)]
pub(crate) mod util_names_join;
#[allow(dead_code)]
pub(crate) mod util_names_output;
#[allow(dead_code)]
pub(crate) mod util_names_render;
#[allow(dead_code)]
pub(crate) mod util_names_tests;
#[allow(dead_code)]
pub(crate) mod util_names_truncate;
#[allow(dead_code)]
pub(crate) mod util_nodes;
#[allow(dead_code)]
pub(crate) mod util_number;
#[allow(dead_code)]
pub(crate) mod util_page;
#[allow(dead_code)]
pub(crate) mod util_parallel;
#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod deviation_tests_d6_d12;
#[allow(dead_code)]
pub(crate) mod util_processor;
#[allow(dead_code)]
pub(crate) mod util_publishers;
#[allow(dead_code)]
pub(crate) mod util_sort;
#[allow(dead_code)]
pub(crate) mod util_static_locator;
#[allow(dead_code)]
pub(crate) mod util_substitute;
#[allow(dead_code)]
pub(crate) mod util_transform;
#[allow(dead_code)]
pub(crate) mod xmldom;
#[allow(dead_code)]
pub(crate) mod xmljson;

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use serde_json::Value;

/// Why an [`Engine`] call failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    /// The call belongs to a stage that is not ported yet (#792 to #797).
    /// `method` names the citeproc-js function.
    NotYetPorted { method: &'static str },
    /// The input is not what citeproc-js accepts (a citation object without a
    /// `citationID`, an item without an `id`).
    BadInput(String),
    /// citeproc-js threw (`CSL.error(msg)`): a style error, or an internal
    /// error upstream would also raise.
    Csl(String),
}

/// The result of a ported function that can reach `CSL.error` (PORTING.md §3).
pub type CslResult<T> = Result<T, EngineError>;

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::NotYetPorted { method } => {
                write!(f, "citeproc: {method} is not ported yet")
            }
            EngineError::BadInput(why) => write!(f, "citeproc: bad input: {why}"),
            EngineError::Csl(msg) => write!(f, "citeproc-js error: {msg}"),
        }
    }
}

impl std::error::Error for EngineError {}

/// Abbreviations by jurisdiction, then category (`container-title`, `title`,
/// `place`, ...), then normalised key: the shape of the test runner's
/// `_acache`. `"default"` is the jurisdiction-less set.
pub type Abbreviations = BTreeMap<String, BTreeMap<String, BTreeMap<String, String>>>;

/// citeproc-js's `sys` object: where the engine gets items, locales and
/// abbreviations. Read-only data, so shared with [`Arc`].
#[derive(Debug, Clone, Default)]
pub struct Sys {
    /// CSL-JSON items by id (`retrieveItem`).
    pub items: Arc<BTreeMap<String, Value>>,
    /// Locale XML by language tag, e.g. `"en-US"` (`retrieveLocale`). A
    /// missing language is `false` in citeproc-js; here it is absent.
    pub locales: Arc<BTreeMap<String, String>>,
    /// The abbreviation cache (`getAbbreviation`).
    pub abbreviations: Abbreviations,
    /// The other properties the host put on its `sys` object: the test
    /// runner copies the fixture's `OPTIONS` onto it (`this[option] =
    /// OPTIONS[option]`). `CSL.Engine` reads the boolean ones named in
    /// `CSL.SYS_OPTIONS` into `opt.development_extensions`, and
    /// `variableWrapper`.
    pub options: BTreeMap<String, Value>,
}

impl Sys {
    /// A `sys` over `items` (in input order, ids taken from each `id`),
    /// `locales` and no abbreviations.
    pub fn new(
        items: &[Value],
        locales: Arc<BTreeMap<String, String>>,
    ) -> Result<Sys, EngineError> {
        let mut map = BTreeMap::new();
        for item in items {
            let id = item_id(item)?;
            map.insert(id, item.clone());
        }
        Ok(Sys {
            items: Arc::new(map),
            locales,
            abbreviations: Abbreviations::new(),
            options: BTreeMap::new(),
        })
    }

    /// Whether the sys has a `retrieveStyleModule` hook (style modules,
    /// src/util_modules.js). The test runner's hook returns `null` for every
    /// jurisdiction, and this port models the hook as absent: false.
    pub fn has_retrieve_style_module(&self) -> bool {
        false
    }

    /// `sys.retrieveStyleModule(jurisdiction, preference)`: always `None`.
    pub fn retrieve_style_module(&self, _jurisdiction: &str, _preference: &str) -> Option<String> {
        None
    }

    /// `retrieveItem(id)`.
    pub fn retrieve_item(&self, id: &str) -> Option<&Value> {
        self.items.get(id)
    }

    /// `retrieveLocale(lang)`.
    pub fn retrieve_locale(&self, lang: &str) -> Option<&str> {
        self.locales.get(lang).map(String::as_str)
    }
}

/// The `id` of a CSL-JSON item or citation item, as a string. citeproc-js keys
/// its registry by `"" + id`, so a number is accepted and stringified, and an
/// item without an `id` (the test suite has a few) is keyed `"undefined"`.
pub fn item_id(item: &Value) -> Result<String, EngineError> {
    Ok(registry::id_key(item.get("id")))
}

/// `setOutputFormat`: the formats the test suite uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    /// The default.
    #[default]
    Html,
    Rtf,
    Plain,
    Asciidoc,
    Xslfo,
}

impl OutputFormat {
    /// The name citeproc-js's `setOutputFormat` takes.
    pub fn from_name(name: &str) -> Option<OutputFormat> {
        match name {
            "html" => Some(OutputFormat::Html),
            "rtf" => Some(OutputFormat::Rtf),
            "plain" => Some(OutputFormat::Plain),
            "asciidoc" => Some(OutputFormat::Asciidoc),
            "xslfo" => Some(OutputFormat::Xslfo),
            _ => None,
        }
    }
}

/// One element of a citation's `citationItems`: `{ "id": ..., "locator": ...,
/// "label": ..., "prefix": ..., "author-only": true, ... }`.
#[derive(Debug, Clone, PartialEq)]
pub struct CitationItem {
    pub id: String,
    /// Every field, `id` included, as given.
    pub fields: Value,
}

impl CitationItem {
    /// From the JSON object citeproc-js takes.
    pub fn from_json(value: &Value) -> Result<CitationItem, EngineError> {
        Ok(CitationItem {
            id: item_id(value)?,
            fields: value.clone(),
        })
    }
}

/// A citation: `{ "citationID": ..., "citationItems": [...], "properties":
/// { "noteIndex": n } }`.
#[derive(Debug, Clone, PartialEq)]
pub struct Citation {
    pub citation_id: String,
    pub citation_items: Vec<CitationItem>,
    pub note_index: i64,
    /// `properties` as given (`noteIndex`, `mode`, `prefix`, `suffix`,
    /// `infix`, `unsorted`, ...); `None` when the citation has none.
    pub properties: Option<js::Obj>,
}

impl Citation {
    /// From the JSON object citeproc-js takes.
    pub fn from_json(value: &Value) -> Result<Citation, EngineError> {
        let citation_id = match value.get("citationID") {
            Some(Value::String(s)) => s.clone(),
            _ => {
                return Err(EngineError::BadInput(format!(
                    "citation without a citationID: {value}"
                )))
            }
        };
        let citation_items = match value.get("citationItems") {
            Some(Value::Array(items)) => items
                .iter()
                .map(CitationItem::from_json)
                .collect::<Result<Vec<_>, _>>()?,
            _ => {
                return Err(EngineError::BadInput(format!(
                    "citation without citationItems: {value}"
                )))
            }
        };
        let note_index = value
            .get("properties")
            .and_then(|p| p.get("noteIndex"))
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let properties = match value.get("properties") {
            Some(Value::Object(o)) => Some(o.clone()),
            _ => None,
        };
        Ok(Citation {
            citation_id,
            citation_items,
            note_index,
            properties,
        })
    }
}

/// An entry of the `pre` or `post` list of `processCitationCluster`:
/// `[citationID, noteIndex]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationRef {
    pub citation_id: String,
    pub note_index: i64,
}

impl CitationRef {
    /// From the `[citationID, noteIndex]` pair citeproc-js takes.
    pub fn from_json(value: &Value) -> Result<CitationRef, EngineError> {
        let pair = value.as_array().filter(|a| a.len() >= 2);
        match pair {
            Some(a) => match (a[0].as_str(), a[1].as_i64()) {
                (Some(id), Some(note)) => Ok(CitationRef {
                    citation_id: id.to_string(),
                    note_index: note,
                }),
                _ => Err(EngineError::BadInput(format!(
                    "not a [citationID, noteIndex] pair: {value}"
                ))),
            },
            None => Err(EngineError::BadInput(format!(
                "not a [citationID, noteIndex] pair: {value}"
            ))),
        }
    }
}

/// One entry of the second element `processCitationCluster` returns:
/// `[index, string, citationID]`, the document position and new text of a
/// citation that changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterUpdate {
    pub index: usize,
    pub text: String,
    pub citation_id: String,
}

/// What `makeBibliography` returns: the parameter object (`bibstart`,
/// `bibend`, `entry_ids`, `hangingindent`, ...) and the formatted entries.
#[derive(Debug, Clone, PartialEq)]
pub struct Bibliography {
    pub params: BTreeMap<String, Value>,
    pub entries: Vec<String>,
}

impl Bibliography {
    /// The string the test runner compares: `bibstart` + entries + `bibend`.
    pub fn joined(&self) -> String {
        let text = |key: &str| {
            self.params
                .get(key)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        };
        format!(
            "{}{}{}",
            text("bibstart"),
            self.entries.concat(),
            text("bibend")
        )
    }
}

/// `new CSL.Engine(sys, style, lang)` and the document it processes.
///
/// Construction runs the ported constructor ([`state::State::new`], build.js):
/// the style is parsed, its locales merged and its token lists built. The
/// processing methods (everything that renders) are not ported yet.
#[derive(Debug, Clone)]
pub struct Engine {
    state: state::State,
    style: String,
    output_format: OutputFormat,
    development_extensions: BTreeMap<String, Value>,
    variable_wrapper: bool,
    suppress_trailing_punctuation: bool,
    lang_prefs_for_cites: BTreeMap<String, Vec<String>>,
    lang_prefs_for_cite_affixes: Option<Value>,
    lang_tags_translation: Vec<String>,
    lang_tags_transliteration: Vec<String>,
}

impl Engine {
    /// `new CSL.Engine(sys, style, lang)`. `style` is the CSL XML (or its
    /// serialized JSON form), `lang` the language tag (`""` for none: the
    /// style's `default-locale`, else `"en-US"`, as in citeproc-js).
    ///
    /// Fails like the constructor does: on a style that cannot be parsed, a
    /// `<style>` attribute with no handler, an unknown node name, a macro
    /// that calls itself, and so on (see [`EngineError::Csl`]).
    pub fn new(sys: Sys, style: &str, lang: &str) -> Result<Engine, EngineError> {
        if style.trim().is_empty() {
            return Err(EngineError::BadInput("empty style".to_string()));
        }
        let variable_wrapper = js::truthy_opt(sys.options.get("variableWrapper"));
        let state = state::State::new(sys, style, lang, false)?;
        Ok(Engine {
            state,
            style: style.to_string(),
            output_format: OutputFormat::default(),
            development_extensions: BTreeMap::new(),
            variable_wrapper,
            suppress_trailing_punctuation: false,
            lang_prefs_for_cites: BTreeMap::new(),
            lang_prefs_for_cite_affixes: None,
            lang_tags_translation: Vec::new(),
            lang_tags_transliteration: Vec::new(),
        })
    }

    /// The ported engine state (what `this` is in citeproc-js). For the
    /// intermediate dump ([`dump`]) and tests.
    #[doc(hidden)]
    pub fn state(&self) -> &state::State {
        &self.state
    }

    /// Mutable access to the engine state. For tests.
    #[doc(hidden)]
    pub fn state_mut(&mut self) -> &mut state::State {
        &mut self.state
    }

    /// The `sys` the engine was built with.
    pub fn sys(&self) -> &Sys {
        &self.state.sys
    }

    /// Replace the items `retrieveItem` serves, keeping the engine's state:
    /// the test runner's `INPUT2` step (`this.test.INPUT = INPUT2;
    /// this._setCache()`) changes the item database under a live engine.
    pub fn replace_items(&mut self, items: &[Value]) -> Result<(), EngineError> {
        let fresh = Sys::new(items, self.state.sys.locales.clone())?;
        self.state.sys = Sys {
            abbreviations: std::mem::take(&mut self.state.sys.abbreviations),
            options: std::mem::take(&mut self.state.sys.options),
            ..fresh
        };
        Ok(())
    }

    /// The style XML.
    pub fn style(&self) -> &str {
        &self.style
    }

    /// The engine's language (`opt.lang`, the best locale chosen).
    pub fn lang(&self) -> &str {
        js::get_str(&self.state.opt, "lang").unwrap_or("")
    }

    /// `setOutputFormat`.
    pub fn set_output_format(&mut self, format: OutputFormat) {
        self.output_format = format;
        // `setOutputFormat` only fails if the format table is malformed.
        let _ = self.state.set_output_format(Engine::mode_name(format));
    }

    /// The `CSL.Output.Formats` key of an output format (`plain` is `text`,
    /// `xslfo` is `fo`).
    fn mode_name(format: OutputFormat) -> &'static str {
        match format {
            OutputFormat::Html => "html",
            OutputFormat::Rtf => "rtf",
            OutputFormat::Plain => "text",
            OutputFormat::Asciidoc => "asciidoc",
            OutputFormat::Xslfo => "fo",
        }
    }

    /// The current output format.
    pub fn output_format(&self) -> OutputFormat {
        self.output_format
    }

    /// `opt.development_extensions[name] = value`, as the test runner sets the
    /// fixture's `OPTIONS`.
    pub fn set_development_extension(&mut self, name: &str, value: Value) {
        self.state.dev_ext_set(name, value.clone());
        self.development_extensions.insert(name.to_string(), value);
    }

    /// The development extensions set so far.
    pub fn development_extensions(&self) -> &BTreeMap<String, Value> {
        &self.development_extensions
    }

    /// Install the test runner's `variableWrapper` (links the title of a first
    /// citation to the item's URL, bolds a back-reference number).
    pub fn set_variable_wrapper(&mut self, on: bool) {
        self.variable_wrapper = on;
        self.state.fun.host_hooks.variable_wrapper = on;
    }

    /// Whether the `variableWrapper` is installed.
    pub fn variable_wrapper(&self) -> bool {
        self.variable_wrapper
    }

    /// `citation.opt.suppressTrailingPunctuation = on`.
    pub fn set_suppress_trailing_punctuation(&mut self, on: bool) {
        self.state.set_suppress_trailing_punctuation(on);
        self.suppress_trailing_punctuation = on;
    }

    /// Whether trailing punctuation is suppressed.
    pub fn suppress_trailing_punctuation(&self) -> bool {
        self.suppress_trailing_punctuation
    }

    /// `setLangPrefsForCites({ persons: [...], titles: [...], ... })`.
    pub fn set_lang_prefs_for_cites(&mut self, prefs: BTreeMap<String, Vec<String>>) {
        self.state.set_lang_prefs_for_cites(&prefs, None);
        self.lang_prefs_for_cites = prefs;
    }

    /// The language preferences set for cites.
    pub fn lang_prefs_for_cites(&self) -> &BTreeMap<String, Vec<String>> {
        &self.lang_prefs_for_cites
    }

    /// `setLangPrefsForCiteAffixes(multiaffix)`.
    pub fn set_lang_prefs_for_cite_affixes(&mut self, affixes: Value) {
        if let Value::Array(list) = &affixes {
            self.state.set_lang_prefs_for_cite_affixes(list);
        }
        self.lang_prefs_for_cite_affixes = Some(affixes);
    }

    /// The cite-affix language preferences, if set.
    pub fn lang_prefs_for_cite_affixes(&self) -> Option<&Value> {
        self.lang_prefs_for_cite_affixes.as_ref()
    }

    /// `setLangTagsForCslTranslation(tags)`.
    pub fn set_lang_tags_for_csl_translation(&mut self, tags: Vec<String>) {
        self.state.set_lang_tags_for_csl_translation(Some(&tags));
        self.lang_tags_translation = tags;
    }

    /// `setLangTagsForCslTransliteration(tags)`.
    pub fn set_lang_tags_for_csl_transliteration(&mut self, tags: Vec<String>) {
        self.state
            .set_lang_tags_for_csl_transliteration(Some(&tags));
        self.lang_tags_transliteration = tags;
    }

    /// `style.fun.dateparser.addDateParserMonths(months)`: extra month names
    /// for the date parser (the test runner adds Turkish ones).
    pub fn add_date_parser_months(&mut self, months: &[String]) {
        self.state.fun.dateparser.add_date_parser_months(months);
    }

    /// The test runner's abbreviation set-up (`this._acache = Object.assign(
    /// this._acache, abbrevs)` after normalising every key with
    /// `sys.normalizeAbbrevsKey("title", key)`, except jurisdiction places and
    /// court segments): `abbreviations` is the fixture's `ABBREVIATIONS`
    /// object `{jurisdiction: {segment: {key: abbreviation}}}`.
    pub fn set_runner_abbreviations(&mut self, abbreviations: &Value) {
        let Some(jurisdictions) = abbreviations.as_object() else {
            return;
        };
        for (jurisd, segments) in jurisdictions {
            let mut by_segment = BTreeMap::new();
            for (segment, keys) in segments.as_object().into_iter().flatten() {
                let mut by_key = BTreeMap::new();
                for (key, value) in keys.as_object().into_iter().flatten() {
                    let is_jurisdiction =
                        jurisd == "default" && segment == "place" && key.to_uppercase() == *key;
                    let is_court = ["institution-entire", "institution-part"]
                        .contains(&segment.as_str())
                        && segment.to_lowercase() == *segment;
                    let normkey = if !is_jurisdiction && !is_court {
                        build_retrieve_item::normalize_abbrevs_key("title", Some(key))
                    } else {
                        key.clone()
                    };
                    if let Some(s) = value.as_str() {
                        by_key.insert(normkey, s.to_string());
                    }
                }
                by_segment.insert(segment.clone(), by_key);
            }
            self.state
                .sys
                .abbreviations
                .insert(jurisd.clone(), by_segment);
        }
    }

    /// The tags set for CSL translation and transliteration.
    pub fn lang_tags(&self) -> (&[String], &[String]) {
        (&self.lang_tags_translation, &self.lang_tags_transliteration)
    }

    /// `updateItems(ids, nosort)`: register `ids` as the bibliography's items.
    pub fn update_items(&mut self, ids: &[String], nosort: bool) -> Result<(), EngineError> {
        self.state.update_items(ids, nosort, false, false)?;
        Ok(())
    }

    /// The ids of `registry.reflist`, in bibliography order.
    pub fn registry_ids(&self) -> Result<Vec<String>, EngineError> {
        Ok(self.state.registry.get_sorted_ids())
    }

    /// Whether `registry.citationreg.citationById[id]` exists.
    pub fn citation_registered(&self, citation_id: &str) -> Result<bool, EngineError> {
        Ok(self.state.registry.citationreg.by_id(citation_id).is_some())
    }

    /// The test runner's `preloadAbbreviations(CSL, style, citation, acache)`
    /// for one citation's items (lib/preload.js).
    ///
    /// The runner registers, for the values of each item, the abbreviations
    /// of its cache in the style's `transform.abbrevs`; this port's
    /// abbreviation lookup reads [`Sys::abbreviations`] directly
    /// ([`build_retrieve_item::abbreviation_lookup`]), which gives the same
    /// answers, so only the runner's other effect remains: it sets
    /// `language-name` and `language-name-original` on the raw item
    /// (`sys.retrieveItem(id)` returns the runner's own object). Like
    /// `scripts/csl-testsuite-reference.cjs`, an item with a `jurisdiction`
    /// is an error (the runner would read abbreviation files).
    pub fn preload_abbreviations(&mut self, items: &[CitationItem]) -> Result<(), EngineError> {
        for citation_item in items {
            let id = citation_item.id.clone();
            let Some(raw) = Arc::make_mut(&mut self.state.sys.items).get_mut(&id) else {
                return Err(EngineError::Csl(
                    "TypeError: Cannot read properties of undefined (reading 'jurisdiction')"
                        .to_string(),
                ));
            };
            let Value::Object(item) = raw else {
                continue;
            };
            if js::truthy_opt(item.get("jurisdiction")) {
                return Err(EngineError::Csl(
                    "fixture item with a jurisdiction: abbreviation files not available"
                        .to_string(),
                ));
            }
            if let Some(language) = item.get("language").filter(|l| js::truthy(l)) {
                let lang = js::to_js_string(language).to_lowercase();
                let parts: Vec<&str> = lang.split('<').collect();
                item.insert("language-name".into(), Value::String(parts[0].to_string()));
                if parts.len() == 2 {
                    item.insert(
                        "language-name-original".into(),
                        Value::String(parts[1].to_string()),
                    );
                }
            }
        }
        Ok(())
    }

    /// `makeCitationCluster(citationItems)`.
    pub fn make_citation_cluster(&mut self, items: &[CitationItem]) -> Result<String, EngineError> {
        let objs: Vec<js::Obj> = items
            .iter()
            .map(|i| i.fields.as_object().cloned().unwrap_or_default())
            .collect();
        self.state.make_citation_cluster(&objs)
    }

    /// The `processCitationCluster` input for a [`Citation`].
    fn citation_input(citation: &Citation) -> api_cite::CitationInput {
        api_cite::CitationInput {
            citation_id: Some(citation.citation_id.clone()),
            citation_items: citation
                .citation_items
                .iter()
                .map(|i| i.fields.as_object().cloned().unwrap_or_default())
                .collect(),
            properties: citation.properties.clone(),
        }
    }

    /// The `[citationID, noteIndex]` lists of `processCitationCluster`.
    fn citation_positions(refs: &[CitationRef]) -> Vec<api_cite::CitationPos> {
        refs.iter()
            .map(|r| api_cite::CitationPos {
                citation_id: r.citation_id.clone(),
                note_index: Value::from(r.note_index),
            })
            .collect()
    }

    /// The `[index, string, citationID]` triples of a result.
    fn cluster_updates(result: api_cite::ClusterResult) -> Vec<ClusterUpdate> {
        result
            .updates
            .into_iter()
            .map(|(index, text, citation_id)| ClusterUpdate {
                index: index.max(0) as usize,
                text,
                citation_id,
            })
            .collect()
    }

    /// `processCitationCluster(citation, pre, post)`: the updates to the
    /// document, `[index, string, citationID]` each.
    pub fn process_citation_cluster(
        &mut self,
        citation: &Citation,
        pre: &[CitationRef],
        post: &[CitationRef],
    ) -> Result<Vec<ClusterUpdate>, EngineError> {
        let result = self.state.process_citation_cluster(
            Engine::citation_input(citation),
            &Engine::citation_positions(pre),
            &Engine::citation_positions(post),
            api_cite::ClusterFlag::None,
        )?;
        Ok(Engine::cluster_updates(result))
    }

    /// `appendCitationCluster(citation)`: `processCitationCluster` with the
    /// citation at the end of the document.
    pub fn append_citation_cluster(
        &mut self,
        citation: &Citation,
    ) -> Result<Vec<ClusterUpdate>, EngineError> {
        let result = self
            .state
            .append_citation_cluster(Engine::citation_input(citation))?;
        Ok(Engine::cluster_updates(result))
    }

    /// `previewCitationCluster(citation, pre, post, newMode)`: the text the
    /// citation would have at this position, leaving the registry as found.
    pub fn preview_citation_cluster(
        &mut self,
        citation: &Citation,
        pre: &[CitationRef],
        post: &[CitationRef],
        format: OutputFormat,
    ) -> Result<String, EngineError> {
        self.state.preview_citation_cluster(
            Engine::citation_input(citation),
            &Engine::citation_positions(pre),
            &Engine::citation_positions(post),
            Engine::mode_name(format),
        )
    }

    /// `makeBibliography(bibsection?)`.
    pub fn make_bibliography(
        &mut self,
        bibsection: Option<&Value>,
    ) -> Result<Bibliography, EngineError> {
        match self.state.make_bibliography(bibsection)? {
            Some(result) => Ok(Bibliography {
                params: result.params.into_iter().collect(),
                entries: result.entry_strings,
            }),
            // `makeBibliography()` returns `false` for a style without a bibliography.
            None => Err(EngineError::Csl(
                "TypeError: Cannot read properties of undefined (reading 'bibstart')".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn engine() -> Engine {
        let sys = Sys::new(
            &[json!({"id": "ITEM-1", "type": "book"})],
            Arc::new(BTreeMap::new()),
        )
        .unwrap();
        Engine::new(sys, "<style/>", "").unwrap()
    }

    #[test]
    fn sys_retrieves_items_by_string_or_numeric_id() {
        let sys = Sys::new(
            &[json!({"id": "a"}), json!({"id": 7})],
            Arc::new(BTreeMap::new()),
        )
        .unwrap();
        assert!(sys.retrieve_item("a").is_some());
        assert!(sys.retrieve_item("7").is_some());
        assert!(sys.retrieve_item("b").is_none());
        assert_eq!(sys.retrieve_locale("en-US"), None);
        // an item without an id is keyed "undefined", as `"" + undefined` is in JS
        let sys = Sys::new(&[json!({"type": "book"})], Arc::new(BTreeMap::new())).unwrap();
        assert!(sys.retrieve_item("undefined").is_some());
    }

    #[test]
    fn engine_keeps_what_the_runner_sets_on_it() {
        let mut e = engine();
        assert_eq!(e.lang(), "en-US");
        assert_eq!(e.style(), "<style/>");
        assert!(e.sys().retrieve_item("ITEM-1").is_some());
        e.replace_items(&[json!({"id": "ITEM-2"})]).unwrap();
        assert!(
            e.sys().retrieve_item("ITEM-1").is_none() && e.sys().retrieve_item("ITEM-2").is_some()
        );
        // an item without an id is keyed "undefined"
        assert!(e.replace_items(&[json!({})]).is_ok());
        e.set_output_format(OutputFormat::from_name("rtf").unwrap());
        assert_eq!(e.output_format(), OutputFormat::Rtf);
        assert_eq!(OutputFormat::from_name("nope"), None);
        e.set_development_extension("wrap_url_and_doi", json!(true));
        assert_eq!(e.development_extensions()["wrap_url_and_doi"], json!(true));
        e.set_variable_wrapper(true);
        assert!(e.variable_wrapper());
        e.set_suppress_trailing_punctuation(true);
        assert!(e.suppress_trailing_punctuation());
        let mut prefs = BTreeMap::new();
        prefs.insert("persons".to_string(), vec!["translit".to_string()]);
        e.set_lang_prefs_for_cites(prefs.clone());
        assert_eq!(e.lang_prefs_for_cites(), &prefs);
        e.set_lang_prefs_for_cite_affixes(json!({"x": 1}));
        assert_eq!(e.lang_prefs_for_cite_affixes(), Some(&json!({"x": 1})));
        e.set_lang_tags_for_csl_translation(vec!["de".to_string()]);
        e.set_lang_tags_for_csl_transliteration(vec!["ja".to_string()]);
        assert_eq!(
            e.lang_tags(),
            (&["de".to_string()][..], &["ja".to_string()][..])
        );
        assert!(matches!(
            Engine::new(Sys::default(), "  ", "en-US"),
            Err(EngineError::BadInput(_))
        ));
    }

    /// A style whose layouts render nothing (so the tests need no rendering
    /// stage) and whose bibliography sorts by publisher, then by edition
    /// descending.
    const SORT_STYLE: &str = r#"<style xmlns="http://purl.org/net/xbiblio/csl" class="in-text" version="1.0">
  <citation><layout/></citation>
  <bibliography>
    <sort><key variable="publisher"/><key variable="edition" sort="descending"/></sort>
    <layout/>
  </bibliography>
</style>"#;

    fn sorting_engine() -> Engine {
        let items = [
            json!({"id": "a", "type": "book", "publisher": "Zed", "edition": "1"}),
            json!({"id": "b", "type": "book", "publisher": "alpha", "edition": "2"}),
            json!({"id": "c", "type": "book", "publisher": "Alpha", "edition": "10"}),
            json!({"id": "d", "type": "book", "publisher": "The Beta"}),
        ];
        let sys = Sys::new(&items, Arc::new(BTreeMap::new())).unwrap();
        // The test locales are tiny; give the engine the en-US the build needs.
        let locales = crate::citeproc::test_support::minimal_locales();
        let sys = Sys {
            locales: Arc::new(locales),
            ..sys
        };
        Engine::new(sys, SORT_STYLE, "").unwrap()
    }

    #[test]
    fn update_items_registers_and_sorts_by_the_bibliography_keys() {
        let mut e = sorting_engine();
        let ids = ["a", "b", "c", "d"].map(String::from);
        e.update_items(&ids, false).unwrap();
        // publisher (case-insensitively, `The` stripped): alpha/Alpha, Beta, Zed;
        // then edition, descending as numbers padded for sorting: c (10) before b (2).
        assert_eq!(e.registry_ids().unwrap(), vec!["c", "b", "d", "a"]);
        // nosort keeps the order given
        e.update_items(&["d".to_string(), "a".to_string()], true)
            .unwrap();
        assert_eq!(e.registry_ids().unwrap(), vec!["d", "a"]);
        assert!(!e.citation_registered("C1").unwrap());
    }

    #[test]
    fn a_style_without_a_bibliography_gives_an_error_not_a_panic() {
        let sys = Sys::new(
            &[json!({"id": "x", "type": "book"})],
            Arc::new(crate::citeproc::test_support::minimal_locales()),
        )
        .unwrap();
        let mut e = Engine::new(
            sys,
            r#"<style xmlns="http://purl.org/net/xbiblio/csl" class="in-text" version="1.0"><citation><layout/></citation></style>"#,
            "",
        )
        .unwrap();
        assert!(matches!(
            e.make_bibliography(None),
            Err(EngineError::Csl(_))
        ));
        assert_eq!(
            e.update_items(&["nope".to_string()], false).is_err(),
            true,
            "an unknown item id is an error, as JSON.parse(undefined) is upstream"
        );
        let citation = Citation::from_json(&json!({
            "citationID": "C1",
            "citationItems": [{"id": "x"}],
            "properties": {"noteIndex": 0}
        }))
        .unwrap();
        assert!(!e.citation_registered("C1").unwrap());
        e.process_citation_cluster(&citation, &[], &[]).unwrap();
        assert!(e.citation_registered("C1").unwrap());
    }

    #[test]
    fn preload_abbreviations_sets_language_names_and_refuses_jurisdictions() {
        let items = [
            json!({"id": "a", "language": "Ja<En"}),
            json!({"id": "b", "jurisdiction": "us"}),
        ];
        let sys = Sys::new(
            &items,
            Arc::new(crate::citeproc::test_support::minimal_locales()),
        )
        .unwrap();
        let mut e = Engine::new(sys, SORT_STYLE, "").unwrap();
        let ci = |id: &str| CitationItem::from_json(&json!({"id": id})).unwrap();
        e.preload_abbreviations(&[ci("a")]).unwrap();
        let a = e.sys().retrieve_item("a").unwrap();
        assert_eq!(a["language-name"], json!("ja"));
        assert_eq!(a["language-name-original"], json!("en"));
        assert!(e.preload_abbreviations(&[ci("b")]).is_err());
        assert!(e.preload_abbreviations(&[ci("zz")]).is_err());
    }

    #[test]
    fn the_processing_error_type_prints() {
        assert_eq!(
            EngineError::NotYetPorted {
                method: "updateItems"
            }
            .to_string(),
            "citeproc: updateItems is not ported yet"
        );
        assert!(EngineError::BadInput("x".into())
            .to_string()
            .contains("bad input"));
    }

    #[test]
    fn citation_inputs_are_checked_not_unwrapped() {
        assert!(Citation::from_json(&json!({"citationItems": []})).is_err());
        assert!(Citation::from_json(&json!({"citationID": "c"})).is_err());
        assert_eq!(
            CitationRef::from_json(&json!(["c1", 2])).unwrap(),
            CitationRef {
                citation_id: "c1".to_string(),
                note_index: 2
            }
        );
        assert!(CitationRef::from_json(&json!(["c1"])).is_err());
        assert!(CitationRef::from_json(&json!([1, 2])).is_err());
    }

    #[test]
    fn bibliography_joins_as_the_runner_does() {
        let mut params = BTreeMap::new();
        params.insert("bibstart".to_string(), json!("<div>"));
        params.insert("bibend".to_string(), json!("</div>"));
        let b = Bibliography {
            params,
            entries: vec!["a".to_string(), "b".to_string()],
        };
        assert_eq!(b.joined(), "<div>ab</div>");
        assert_eq!(
            Bibliography {
                params: BTreeMap::new(),
                entries: vec![]
            }
            .joined(),
            ""
        );
    }
}
