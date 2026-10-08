// Part of the kovan port of citeproc-js (GitHub #790, #792).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/state.js (CSL.Engine.Opt, .Tmp, .Build, .Fun,
//              .Configure, .Citation, .Bibliography, .BibliographySort,
//              .CitationSort, .InText), and the instance fields that
//              src/build.js's CSL.Engine constructor sets
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

//! The engine's state: what `this` is in `CSL.Engine.prototype.*` and what
//! `state` is in every closure (PORTING.md §3).
//!
//! **Field ownership (PORTING.md §6).** The struct definitions below are
//! shared by every part of the port. Add a field you need inside the block
//! marked with your agent's name; do not reorder or rename other fields. The
//! subsystem structs (`Queue`, `Registry`, `XmlJson`, ...) are defined in their
//! own files and filled in by their owners.
//!
//! The constructors of src/state.js live here: [`new_opt`] (`CSL.Engine.Opt`),
//! [`Tmp::new`], [`Build::new`], [`Fun::new`], [`Configure::new`] and the five
//! [`Area`] constructors. Each reproduces upstream's defaults exactly, since
//! the intermediate dump compares `opt` and the areas' `opt` with citeproc-js.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::disambig_cites::Disambiguation;
use super::js::Obj;
use super::obj_ambigconfig::AmbigConfig;
use super::obj_blob::{BlobId, Blobs};
use super::obj_token::Token;
use super::queue::Queue;
use super::registry::{Comparifier, Registry};
use super::stack::{JsFalsy, Stack};
use super::util::Match;
use super::util_dateparser::DateParser;
use super::util_flipflop::FlipFlopper;
use super::util_locale::Locale;
use super::util_number::{LongOrdinalizer, Ordinalizer, Romanizer, ShadowNumber, Suffixator};
use super::util_modules::Juris;
use super::util_parallel::Parallel;
use super::util_transform::Transform;
use super::xmljson::XmlJson;
use super::Sys;

/// The engine: `new CSL.Engine(sys, style, lang)`'s instance.
#[derive(Debug, Clone, Default)]
pub struct State {
    /// `processor_version` (`CSL.PROCESSOR_VERSION`, "1.4.61" in 2.4.63).
    pub processor_version: String,
    /// `csl_version`.
    pub csl_version: String,
    /// `sys`.
    pub sys: Sys,
    /// `opt` (CSL.Engine.Opt): an open-ended bag, as in JS.
    pub opt: Obj,
    /// `tmp` (CSL.Engine.Tmp).
    pub tmp: Tmp,
    /// `build` (CSL.Engine.Build).
    pub build: Build,
    /// `fun` (CSL.Engine.Fun).
    pub fun: Fun,
    /// `configure` (CSL.Engine.Configure).
    pub configure: Configure,
    /// `citation`, `bibliography`, `intext` (the rendering areas) and
    /// `citation_sort`, `bibliography_sort` (the sort-key areas).
    pub citation: Area,
    pub bibliography: Area,
    pub intext: Area,
    pub citation_sort: Area,
    pub bibliography_sort: Area,
    /// `output`: the main output queue.
    pub output: Queue,
    /// `dateput`: the queue dates are assembled in.
    pub dateput: Queue,
    /// Every blob of both queues (PORTING.md §3).
    pub blobs: Blobs,
    /// `cslXml`: the parsed style.
    pub csl_xml: XmlJson,
    /// `locale`: by language tag.
    pub locale: BTreeMap<String, Locale>,
    /// `locale_opts`, `locale_dates` etc. live inside [`Locale`].
    /// `macros`: each macro's configured token list, by name.
    pub macros: BTreeMap<String, Vec<Token>>,
    /// `registry`.
    pub registry: Registry,
    /// `disambiguate`.
    pub disambiguate: Disambiguation,
    /// `transform`.
    pub transform: Transform,
    /// `parallel` (only when `opt.parallel.enable`).
    pub parallel: Option<Parallel>,
    /// `juris`: the loaded style modules by jurisdiction (src/util_modules.js);
    /// was an `Obj` in the foundation commit, changed because a module holds
    /// token lists.
    pub juris: BTreeMap<String, Juris>,
    /// `splice_delimiter`.
    pub splice_delimiter: Option<String>,

    // ---- fields: wave1-build (load, xmljson, build, util_locale, attributes) ----

    // ---- fields: wave1-nodes ----
    /// `state.intext_sort` (node_intext.js; not in the state.js constructor):
    /// only its `opt.sort_directions` is ever set.
    pub intext_sort: Area,
    /// `state.build_layout_locale_flag` (node_layout.js:259; a misspelling of
    /// `state.build.layout_locale_flag` upstream, kept as its own flag).
    pub build_layout_locale_flag: bool,

    /// Whether `this.registry` has been assigned yet. `CSL.SET_COURT_CLASSES`
    /// (load.js) tests `state.registry` for "defined" to tell an in-style
    /// declaration (during `localeConfigure`, before the registry exists)
    /// from an in-module one. `CSL.Engine` sets it after `localeConfigure`.
    pub has_registry: bool,
    /// `setParseNames` is a method in JS (build.js); there is no field.
    /// `version` (`this.version = CSL.version` in `configureTokenLists`) is
    /// `undefined` in 2.4.63 and so is not modelled.
    ///
    /// Whether `sys.variableWrapper` exists (which also decides whether the
    /// constructor sets the module global `CSL.VARIABLE_WRAPPER_PREPUNCT_REX`,
    /// `load::variable_wrapper_prepunct_rex()`) is `fun.host_hooks.variable_wrapper`.
    // ---- fields: wave1-input (dates, numbers, name particles, retrieveItem) ----
    /// PROVISIONAL (wave1-input): `registry.refhash`, the normalised items
    /// by id that `retrieveItem` returns on later calls. Belongs to the
    /// registry (wave4); `build_retrieve_item.rs` reads and writes this
    /// field until `Registry::refhash` exists.
    pub item_refhash: BTreeMap<String, serde_json::Value>,
    // ---- fields: wave1-output (queue, formats, formatters, flip-flop, page) ----

    // ---- fields: wave2 (rendering nodes, api_cite core) ----

    // ---- fields: wave3 (names) ----
    /// `state.nameOutput` (`new CSL.NameOutput(state, Item, item)`, api_cite.js:1488):
    /// the names renderer of the cite being rendered. Replace it with
    /// [`State::new_name_output`] where upstream constructs one.
    pub name_output: super::util_names_output::NameOutput,
    /// What `NameOutput.getName` reads from `this.Item` and `this.state` (set
    /// by [`State::new_name_output`]); a separate field so `getName` stays
    /// callable while `name_output` is taken out of the state for a method
    /// call (the registry's `evalname` calls it from inside `disambigNames`).
    pub name_input_ctx: super::util_names_render::NameInputCtx,
    /// `state.publisherOutput` (node_group.js:174; `undefined` until a group
    /// with a publisher/place pair builds it).
    pub publisher_output: Option<super::util_publishers::PublisherOutput>,

    // ---- fields: wave4 (registry, disambiguation, sort) ----

    // ---- fields: wave5 (citations API, bibliography) ----
}

/// A `Value` from a `json!` object literal, as an [`Obj`].
fn obj(v: Value) -> Obj {
    match v {
        Value::Object(m) => m,
        _ => Obj::new(),
    }
}

/// `CSL.Engine.Opt`: the engine's `opt` bag with upstream's defaults.
///
/// Keys are those of src/state.js lines 3-135; `development_extensions` holds
/// the 31 flags upstream initialises. Later code adds more keys (`version`,
/// `lang`, `xclass`, `styleID`, ... from the build).
pub fn new_opt() -> Obj {
    let affix = || {
        json!({
            "locale-orig": {"prefix": "", "suffix": ""},
            "locale-translit": {"prefix": "", "suffix": ""},
            "locale-translat": {"prefix": "", "suffix": ""}
        })
    };
    let mut o = Obj::new();
    let mut put = |k: &str, v: Value| {
        o.insert(k.to_string(), v);
    };
    put("parallel", json!({"enable": false}));
    put("has_disambiguate", json!(false));
    put("mode", json!("html"));
    put("dates", json!({}));
    put("jurisdictions_seen", json!({}));
    put("suppressedJurisdictions", json!({}));
    put("inheritedAttributes", json!({}));
    put("locale-sort", json!([]));
    put("locale-translit", json!([]));
    put("locale-translat", json!([]));
    put(
        "citeAffixes",
        json!({
            "persons": affix(),
            "institutions": affix(),
            "titles": affix(),
            "journals": affix(),
            "publishers": affix(),
            "places": affix()
        }),
    );
    put("default-locale", json!([]));
    put("update_mode", json!(0));
    put("bib_mode", json!(0));
    put("sort_citations", json!(false));
    put("et-al-min", json!(0));
    put("et-al-use-first", json!(1));
    put("et-al-use-last", json!(false));
    put("et-al-subsequent-min", json!(false));
    put("et-al-subsequent-use-first", json!(false));
    put("demote-non-dropping-particle", json!("display-and-sort"));
    put("parse-names", json!(true));
    put("citation_number_slug", json!(false));
    put("trigraph", json!("Aaaa00:AaAa00:AaAA00:AAAA00"));
    put("nodenames", json!([]));
    put("gender", json!({}));
    put(
        "cite-lang-prefs",
        json!({
            "persons": ["orig"],
            "institutions": ["orig"],
            "titles": ["orig"],
            "journals": ["orig"],
            "publishers": ["orig"],
            "places": ["orig"],
            "number": ["orig"]
        }),
    );
    put("has_layout_locale", json!(false));
    put("disable_duplicate_year_suppression", json!([]));
    put("use_context_condition", json!(false));
    put("jurisdiction_fallbacks", json!({}));
    let mut dev = Obj::new();
    for (k, v) in [
        ("field_hack", true),
        ("allow_field_hack_date_override", true),
        ("locator_date_and_revision", true),
        ("locator_label_parse", true),
        ("raw_date_parsing", true),
        ("clean_up_csl_flaws", true),
        ("consolidate_legal_items", false),
        ("csl_reverse_lookup_support", false),
        ("wrap_url_and_doi", false),
        ("thin_non_breaking_space_html_hack", false),
        ("apply_citation_wrapper", false),
        ("main_title_from_short_title", false),
        ("uppercase_subtitles", false),
        ("normalize_lang_keys_to_lowercase", false),
        ("strict_text_case_locales", false),
        ("expect_and_symbol_form", false),
        ("require_explicit_legal_case_title_short", false),
        ("spoof_institutional_affiliations", false),
        ("force_jurisdiction", false),
        ("parse_names", true),
        ("hanging_indent_legacy_number", false),
        ("throw_on_empty", false),
        ("strict_inputs", true),
        ("prioritize_disambiguate_condition", false),
        ("force_short_title_casing_alignment", true),
        ("implicit_short_title", false),
        ("force_title_abbrev_fallback", false),
        ("split_container_title", false),
        ("legacy_institution_name_ordering", false),
        ("etal_min_etal_usefirst_hack", false),
    ] {
        dev.insert(k.to_string(), Value::Bool(v));
    }
    put("development_extensions", Value::Object(dev));
    o
}

/// One entry of `state.tmp.group_context`: the flags of the group being
/// rendered (the object literal in `CSL.Engine.Tmp`, plus the properties
/// node_group.js and friends add to it later).
///
/// JS `undefined` and `false` mixtures are [`Value`]s (`Null` = `undefined`).
#[derive(Debug, Clone, PartialEq)]
pub struct GroupContext {
    /// `term_intended`.
    pub term_intended: bool,
    /// `variable_attempt`.
    pub variable_attempt: bool,
    /// `variable_success`.
    pub variable_success: bool,
    /// `output_tip` (a blob of the output queue, `undefined` initially).
    pub output_tip: Option<BlobId>,
    /// `label_form`.
    pub label_form: Value,
    /// `label_capitalize_if_first` (added by node_group.js).
    pub label_capitalize_if_first: Value,
    /// `label_static` (added by node_group.js).
    pub label_static: Value,
    /// `parallel_first`.
    pub parallel_first: Value,
    /// `parallel_last`.
    pub parallel_last: Value,
    /// `parallel_delimiter_override`.
    pub parallel_delimiter_override: Value,
    /// `parallel_delimiter_override_on_suppress` (node_group.js).
    pub parallel_delimiter_override_on_suppress: Value,
    /// `non_parallel` (node_group.js).
    pub non_parallel: Value,
    /// `parallel_last_override` (node_group.js).
    pub parallel_last_override: Value,
    /// `variable_success_parent` (node_group.js).
    pub variable_success_parent: Value,
    /// `condition`: `false`, or the conditional-group test (`None` = `false`).
    pub condition: Option<GroupCondition>,
    /// `force_suppress`.
    pub force_suppress: bool,
    /// `done_vars`.
    pub done_vars: Vec<String>,
    /// `value_seen` (set by `UPDATE_GROUP_CONTEXT_CONDITION`).
    pub value_seen: bool,
}

impl Default for GroupContext {
    /// The object literal passed to `new CSL.Stack({...})` in `CSL.Engine.Tmp`.
    fn default() -> Self {
        GroupContext {
            term_intended: false,
            variable_attempt: false,
            variable_success: false,
            output_tip: None,
            label_form: Value::Null,
            label_capitalize_if_first: Value::Null,
            label_static: Value::Null,
            parallel_first: Value::Null,
            parallel_last: Value::Null,
            parallel_delimiter_override: Value::Null,
            parallel_delimiter_override_on_suppress: Value::Null,
            non_parallel: Value::Null,
            parallel_last_override: Value::Null,
            variable_success_parent: Value::Null,
            condition: None,
            force_suppress: false,
            done_vars: Vec::new(),
            value_seen: false,
        }
    }
}

/// `group_context.tip.condition`: the `{test, not, termtxt, valueTerm}`
/// object of a conditional group (`@has-publisher...`/`label` conditions;
/// node_group.js sets `test` and `not`, `UPDATE_GROUP_CONTEXT_CONDITION` the
/// rest).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GroupCondition {
    /// `test`: `"empty-label"`, `"empty-label-no-decor"`, `"comma-safe"`,
    /// `"comma-safe-numbers-only"`, ...
    pub test: String,
    /// `not`.
    pub not: bool,
    /// `termtxt` (`undefined` until the first term is seen).
    pub termtxt: Option<String>,
    /// `valueTerm` (truthiness only).
    pub value_term: bool,
}

impl JsFalsy for GroupContext {
    /// An object is always truthy.
    fn is_truthy(&self) -> bool {
        true
    }
    fn empty_string() -> Self {
        GroupContext::default()
    }
}

/// One rendering or sorting area: `CSL.Engine.Citation`, `.Bibliography`,
/// `.InText`, `.CitationSort`, `.BibliographySort`.
#[derive(Debug, Clone, Default)]
pub struct Area {
    /// `opt`.
    pub opt: Obj,
    /// `tokens`: the configured token list.
    pub tokens: Vec<Token>,
    /// `root`: `"citation"`, `"bibliography"` or `"intext"`.
    pub root: String,
    /// `srt` (citation and bibliography only).
    pub srt: Option<Comparifier>,
    /// `keys` (the sort areas).
    pub keys: Vec<serde_json::Value>,
    /// `tmp` (bibliography_sort only).
    pub tmp: Obj,
}

impl Area {
    /// The `opt` that `CSL.Engine.Citation` and `CSL.Engine.InText` build.
    fn rendering_opt() -> Obj {
        obj(json!({
            "inheritedAttributes": {},
            "collapse": [],
            "disambiguate-add-names": false,
            "disambiguate-add-givenname": false,
            "disambiguate-add-year-suffix": false,
            "givenname-disambiguation-rule": "by-cite",
            "near-note-distance": 5,
            "topdecor": [],
            "layout_decorations": [],
            "layout_prefix": "",
            "layout_suffix": "",
            "layout_delimiter": "",
            "sort_locales": [],
            "max_number_of_names": 0
        }))
    }

    /// `new CSL.Engine.Citation(state)`.
    ///
    /// PORT-LATER(registry): upstream also sets
    /// `this.srt = new CSL.Registry.Comparifier(state, "citation_sort")`
    /// (registry.js:675, wave4); a default comparifier stands in.
    pub fn new_citation() -> Area {
        Area {
            opt: Area::rendering_opt(),
            tokens: Vec::new(),
            root: "citation".to_string(),
            srt: Some(Comparifier::default()),
            keys: Vec::new(),
            tmp: Obj::new(),
        }
    }

    /// `new CSL.Engine.Bibliography()`.
    pub fn new_bibliography() -> Area {
        Area {
            opt: obj(json!({
                "inheritedAttributes": {},
                "collapse": [],
                "topdecor": [],
                "layout_decorations": [],
                "layout_prefix": "",
                "layout_suffix": "",
                "layout_delimiter": "",
                "line-spacing": 1,
                "entry-spacing": 1,
                "sort_locales": [],
                "max_number_of_names": 0
            })),
            tokens: Vec::new(),
            root: "bibliography".to_string(),
            srt: None,
            keys: Vec::new(),
            tmp: Obj::new(),
        }
    }

    /// `new CSL.Engine.BibliographySort()`.
    pub fn new_bibliography_sort() -> Area {
        Area {
            opt: obj(json!({
                "sort_directions": [],
                "topdecor": [],
                "citation_number_sort_direction": 2,
                "citation_number_secondary": false
            })),
            tokens: Vec::new(),
            root: "bibliography".to_string(),
            srt: None,
            keys: Vec::new(),
            tmp: Obj::new(),
        }
    }

    /// `new CSL.Engine.CitationSort()`.
    pub fn new_citation_sort() -> Area {
        Area {
            opt: obj(json!({
                "sort_directions": [],
                "topdecor": []
            })),
            tokens: Vec::new(),
            root: "citation".to_string(),
            srt: None,
            keys: Vec::new(),
            tmp: Obj::new(),
        }
    }

    /// `new CSL.Engine.InText()`.
    pub fn new_intext() -> Area {
        Area {
            opt: Area::rendering_opt(),
            tokens: Vec::new(),
            root: "intext".to_string(),
            srt: None,
            keys: Vec::new(),
            tmp: Obj::new(),
        }
    }
}

/// `CSL.Engine.Tmp`: per-render scratch state.
#[derive(Debug, Clone)]
pub struct Tmp {
    // ---- fields: wave1-build ----
    /// `names_max` (a stack of counts).
    pub names_max: Stack<Value>,
    /// `names_base`.
    pub names_base: Stack<Value>,
    /// `givens_base`.
    pub givens_base: Stack<Value>,
    /// `value`: field values collected by `@value`/`@variable`.
    pub value: Vec<Value>,
    /// `namepart_decorations`.
    pub namepart_decorations: Obj,
    /// `namepart_type` (`false` initially).
    pub namepart_type: Value,
    /// `area`: `"citation"` initially.
    pub area: String,
    /// `root`.
    pub root: String,
    /// `extension`.
    pub extension: String,
    /// `can_substitute`: `new CSL.Stack(0, CSL.LITERAL)`; holds `0` and booleans.
    pub can_substitute: Stack<Value>,
    /// `element_rendered_ok`.
    pub element_rendered_ok: bool,
    /// `element_trace`: `new CSL.Stack("style")`.
    pub element_trace: Stack<String>,
    /// `nameset_counter`.
    pub nameset_counter: i64,
    /// `group_context`.
    pub group_context: Stack<GroupContext>,
    /// `term_predecessor`.
    pub term_predecessor: bool,
    /// `in_cite_predecessor`.
    pub in_cite_predecessor: bool,
    /// `jump`: `new CSL.Stack(0, CSL.LITERAL)`; holds `0`, `"succeed"`,
    /// `"fail"` and (`push(undefined, LITERAL)`) `null` for `undefined`.
    pub jump: Stack<Value>,
    /// `decorations`.
    pub decorations: Stack<Value>,
    /// `tokenstore_stack`.
    pub tokenstore_stack: Stack<Value>,
    /// `last_suffix_used`.
    pub last_suffix_used: String,
    /// `last_names_used`.
    pub last_names_used: Vec<Value>,
    /// `last_years_used`.
    pub last_years_used: Vec<Value>,
    /// `years_used`.
    pub years_used: Vec<Value>,
    /// `names_used`.
    pub names_used: Vec<Value>,
    /// `taintedItemIDs`.
    pub tainted_item_ids: BTreeMap<String, bool>,
    /// `taintedCitationIDs`.
    pub tainted_citation_ids: BTreeMap<String, bool>,
    /// `initialize_with`.
    pub initialize_with: Stack<Value>,
    /// `disambig_request` (`false`, later an object).
    pub disambig_request: Value,
    /// `["name-as-sort-order"]`.
    pub name_as_sort_order: Value,
    /// `suppress_decorations`.
    pub suppress_decorations: bool,
    /// `disambig_settings`.
    pub disambig_settings: AmbigConfig,
    /// `bib_sort_keys`.
    pub bib_sort_keys: Vec<Value>,
    /// `prefix`: `new CSL.Stack("", CSL.LITERAL)`.
    pub prefix: Stack<String>,
    /// `suffix`.
    pub suffix: Stack<String>,
    /// `delimiter`.
    pub delimiter: Stack<String>,
    /// `cite_locales`.
    pub cite_locales: Vec<Value>,
    /// `cite_affixes`: `{citation: false, bibliography: false,
    /// citation_sort: false, bibliography_sort: false}`, filled by
    /// node_layout.js with per-locale `{delimiter, suffix}` objects.
    pub cite_affixes: Obj,
    /// `strip_periods`.
    pub strip_periods: i64,
    /// `shadow_numbers`.
    pub shadow_numbers: BTreeMap<String, ShadowNumber>,
    /// `authority_stop_last`.
    pub authority_stop_last: i64,
    /// `loadedItemIDs`.
    pub loaded_item_ids: BTreeMap<String, bool>,
    /// `condition_counter`.
    pub condition_counter: i64,
    /// `condition_lang_val_arr`.
    pub condition_lang_val_arr: Vec<String>,
    /// `condition_lang_counter_arr`.
    pub condition_lang_counter_arr: Vec<i64>,
    /// Not set by the constructor; read/written by later code:
    /// `lang_array` (api_cite.js:1513, read by `CSL.toLocaleUpperCase`).
    pub lang_array: Vec<String>,
    /// `cite_renders_content` (set by `getTerm`).
    pub cite_renders_content: bool,
    /// `tmp["doing-macro-with-date"]` (set by `expandMacro`'s closures).
    pub doing_macro_with_date: bool,
    /// `just_did_number` (group context conditions, load.js).
    pub just_did_number: bool,
    // ---- fields: wave1-nodes (integrated; not in the state.js constructor) ----
    /// `conditions`: the cs:if/cs:else-if being filled by cs:conditions.
    pub conditions: Option<super::util_conditions::ConditionsEngine>,
    /// `last_cite_locale`.
    pub last_cite_locale: Option<String>,
    /// `etal_node`: the cs:et-al token (as `node_names::token_to_value`).
    pub etal_node: Option<Value>,
    /// `etal_term`.
    pub etal_term: Option<String>,
    /// `abort_alternative`.
    pub abort_alternative: bool,
    /// `date_object` (`false` when the date is not rendered).
    pub date_object: Value,
    /// `donesies`.
    pub donesies: Vec<String>,
    /// `dateparts`.
    pub dateparts: Vec<String>,
    /// `date_collapse_at`.
    pub date_collapse_at: Vec<String>,
    /// `date_token` (set at build time by cs:date).
    pub date_token: Option<Token>,
    /// `just_looking` (also read by `processNumber`'s `setStyling` and the
    /// queue; set by the pass that renders "just looking").
    pub just_looking: bool,
    /// `done_vars`.
    pub done_vars: Vec<String>,
    /// `sort_key_flag`.
    pub sort_key_flag: bool,
    /// `can_block_substitute`.
    pub can_block_substitute: bool,
    /// `common_term_match_fail`.
    pub common_term_match_fail: bool,
    /// `probably_rendered_something`.
    pub probably_rendered_something: bool,
    /// `container_item_count`, `container_item_pos` (keyed by container_id).
    pub container_item_count: Obj,
    pub container_item_pos: Obj,
    /// `et-al-min`, `et-al-use-first`, `et-al-use-last` (`None` = undefined).
    pub et_al_min: Option<Value>,
    pub et_al_use_first: Option<Value>,
    pub et_al_use_last: Option<Value>,
    /// `lang_sort_hold` (node_sort.js).
    pub lang_sort_hold: Option<String>,

    // ---- fields: wave1-output (queue.js) ----
    /// `tmp.count_offset_characters`: `false`, or the item id whose
    /// `first_blob` switched counting on (queue.js).
    pub count_offset_characters: Option<String>,
    /// `tmp.offset_characters` (queue.js).
    pub offset_characters: usize,
    /// `tmp.term_predecessor_name` (queue.js).
    pub term_predecessor_name: bool,
    // ---- fields: wave2 ----

    // ---- fields: wave3 ----
    /// `tmp.name_node` (`{children, top, string}`; `{}` at `getCite`, a fresh
    /// object at each `cs:names` START). Also written by attributes.js,
    /// util_transform.js and node_layout.js upstream.
    pub name_node: super::util_names_output::NameNode,
    /// `tmp.rendered_name`: `None` is JS `false`/`undefined`; the strings of
    /// the names rendered so far in the bibliography (and, from
    /// attributes.js:373, the raw `Item[variable]` of a substituted text).
    pub rendered_name: Option<Vec<Value>>,
    /// `tmp.last_rendered_name` (`false`, an array of strings, or one string).
    pub last_rendered_name: Value,
    /// `tmp.label_blob` (`false` = `None`).
    pub label_blob: Option<BlobId>,
    /// `tmp.subsequent_author_substitute_ok`.
    pub subsequent_author_substitute_ok: bool,
    /// `tmp.substituted_variable`.
    pub substituted_variable: Option<String>,
    /// `tmp.first_name_string` (`false` = `None`).
    pub first_name_string: Option<String>,
    /// `tmp.authorstring_request` (set by api_cite.js).
    pub authorstring_request: bool,
    /// `tmp.last_primary_names_string` (`false`/`undefined` = `None`).
    pub last_primary_names_string: Option<String>,
    /// `tmp.have_collapsed`.
    pub have_collapsed: bool,
    /// `tmp.use_cite_group_delimiter`.
    pub use_cite_group_delimiter: bool,
    /// `tmp.name_delimiter` (set by the `cs:name` closure).
    pub name_delimiter: Option<String>,
    /// `tmp.institution_delimiter` (set by the `cs:institution` closure).
    pub institution_delimiter: Option<String>,
    /// `tmp.and_term` (set by the `cs:name` closure).
    pub and_term: Option<String>,
    /// `tmp["delimiter-precedes-et-al"]`.
    pub delimiter_precedes_et_al: Option<String>,
    /// `tmp["publisher-list"]`, `["publisher-place-list"]`,
    /// `["publisher-group-token"]` (`false` after `clearVars`; only ever read
    /// by util_transform.js).
    pub publisher_list: bool,
    /// `tmp["publisher-token"]` and `tmp["publisher-place-token"]`.
    pub publisher_token: Option<Token>,
    pub publisher_place_token: Option<Token>,
    /// The name disambiguation settings `tmp.disambig_settings` carries that
    /// util_names_*.js reads and writes: see
    /// [`super::util_names_disambig::NameDisambigSettings`]. Lives here, not in
    /// `AmbigConfig` (obj_ambigconfig.rs is the engine agent's); the
    /// integrator should merge the two (the JS object is one:
    /// `tmp.disambig_settings.names/.givens/.use_initials`).
    pub name_ambig: super::util_names_disambig::NameDisambigSettings,

    // ---- fields: wave4 ----

    // ---- fields: wave5 ----
}

impl Tmp {
    /// `new CSL.Engine.Tmp()`.
    pub fn new() -> Tmp {
        let cite_affixes = obj(json!({
            "citation": false,
            "bibliography": false,
            "citation_sort": false,
            "bibliography_sort": false
        }));
        Tmp {
            names_max: Stack::new(),
            names_base: Stack::new(),
            givens_base: Stack::new(),
            value: Vec::new(),
            namepart_decorations: Obj::new(),
            namepart_type: Value::Bool(false),
            area: "citation".to_string(),
            root: "citation".to_string(),
            extension: String::new(),
            can_substitute: Stack::with(Value::from(0)),
            element_rendered_ok: false,
            element_trace: Stack::with_nonliteral("style".to_string()),
            nameset_counter: 0,
            group_context: Stack::with(GroupContext::default()),
            term_predecessor: false,
            in_cite_predecessor: false,
            jump: Stack::with(Value::from(0)),
            decorations: Stack::new(),
            tokenstore_stack: Stack::new(),
            last_suffix_used: String::new(),
            last_names_used: Vec::new(),
            last_years_used: Vec::new(),
            years_used: Vec::new(),
            names_used: Vec::new(),
            tainted_item_ids: BTreeMap::new(),
            tainted_citation_ids: BTreeMap::new(),
            initialize_with: Stack::new(),
            disambig_request: Value::Bool(false),
            name_as_sort_order: Value::Bool(false),
            suppress_decorations: false,
            disambig_settings: AmbigConfig::default(),
            bib_sort_keys: Vec::new(),
            prefix: Stack::with(String::new()),
            suffix: Stack::with(String::new()),
            delimiter: Stack::with(String::new()),
            cite_locales: Vec::new(),
            cite_affixes,
            strip_periods: 0,
            shadow_numbers: BTreeMap::new(),
            authority_stop_last: 0,
            loaded_item_ids: BTreeMap::new(),
            condition_counter: 0,
            condition_lang_val_arr: Vec::new(),
            condition_lang_counter_arr: Vec::new(),
            lang_array: Vec::new(),
            cite_renders_content: false,
            doing_macro_with_date: false,
            just_did_number: false,
            conditions: None,
            last_cite_locale: None,
            etal_node: None,
            etal_term: None,
            abort_alternative: false,
            date_object: Value::Bool(false),
            donesies: Vec::new(),
            dateparts: Vec::new(),
            date_collapse_at: Vec::new(),
            date_token: None,
            just_looking: false,
            done_vars: Vec::new(),
            sort_key_flag: false,
            can_block_substitute: false,
            common_term_match_fail: false,
            probably_rendered_something: false,
            container_item_count: Obj::new(),
            container_item_pos: Obj::new(),
            et_al_min: None,
            et_al_use_first: None,
            et_al_use_last: None,
            lang_sort_hold: None,
            count_offset_characters: None,
            offset_characters: 0,
            term_predecessor_name: false,
            name_node: Default::default(),
            rendered_name: None,
            last_rendered_name: Value::Bool(false),
            label_blob: None,
            subsequent_author_substitute_ok: false,
            substituted_variable: None,
            first_name_string: None,
            authorstring_request: false,
            last_primary_names_string: None,
            have_collapsed: false,
            use_cite_group_delimiter: false,
            name_delimiter: None,
            institution_delimiter: None,
            and_term: None,
            delimiter_precedes_et_al: None,
            publisher_list: false,
            publisher_token: None,
            publisher_place_token: None,
            name_ambig: Default::default(),
        }
    }
}

impl Default for Tmp {
    fn default() -> Self {
        Tmp::new()
    }
}

/// `CSL.Engine.Build`: state while compiling the style.
#[derive(Debug, Clone)]
pub struct Build {
    // ---- fields: wave1-build ----
    /// `["alternate-term"]`: the localisation key of the alternative et-al
    /// term (`false` initially).
    pub alternate_term: Value,
    /// `in_bibliography`.
    pub in_bibliography: bool,
    /// `in_style`.
    pub in_style: bool,
    /// `skip`: `None` is JS `false`; `Some("info")` while inside cs:info.
    pub skip: Option<String>,
    /// `postponed_macro`.
    pub postponed_macro: Value,
    /// `layout_flag`.
    pub layout_flag: bool,
    /// `name`.
    pub name: Value,
    /// `names_variables`: `[[]]`.
    pub names_variables: Vec<Vec<String>>,
    /// `name_label`: `[{}]`, kept as ordered (variable, {before, after}) pairs
    /// because `Object.keys` order matters (tokens as `node_names::token_to_value`).
    pub name_label: Vec<Vec<(String, Value)>>,
    /// `form`.
    pub form: Value,
    /// `term`.
    pub term: Value,
    /// `macro`: `{}` (the macros themselves are discarded after the build).
    pub macro_: Obj,
    /// `macro_stack`: the macro build stack (infinite-loop guard).
    pub macro_stack: Vec<String>,
    /// `text`.
    pub text: Value,
    /// `lang`.
    pub lang: Option<String>,
    /// `area`: `"citation"` initially.
    pub area: String,
    /// `root`.
    pub root: String,
    /// `extension`.
    pub extension: String,
    /// `substitute_level`: `new CSL.Stack(0, CSL.LITERAL)`.
    pub substitute_level: Stack<i64>,
    /// `names_level`.
    pub names_level: i64,
    /// `render_nesting_level`.
    pub render_nesting_level: i64,
    /// `render_seen`.
    pub render_seen: bool,
    /// `bibliography_key_pos`.
    pub bibliography_key_pos: i64,
    /// Not set by the constructor; written by the builders (a sampling of
    /// what `grep 'build\.' src/*.js` finds; wave1-nodes may add more):
    /// `cslNodeId` (build.js, `csl_reverse_lookup_support`).
    pub csl_node_id: i64,
    /// `current_default_locale` (attributes.js `@default-locale`;
    /// `undefined` until then, and `expandMacro` concatenates it as such).
    pub current_default_locale: Value,
    /// `date_key`.
    pub date_key: bool,
    /// `date_variables` (a copy of the date token's `variables`).
    pub date_variables: Vec<String>,
    /// `date_parts`.
    pub date_parts: Vec<String>,
    /// `layout_locale_flag`.
    pub layout_locale_flag: bool,
    /// `name_flag`.
    pub name_flag: bool,
    /// `names_flag`.
    pub names_flag: bool,
    /// `name_delimiter`.
    pub name_delimiter: Value,
    /// `cls`.
    pub cls: Option<String>,
    /// `has_institution`.
    pub has_institution: bool,
    /// `plural`.
    pub plural: Value,
    /// `["publisher-special"]`.
    pub publisher_special: bool,
    /// `sort_flag` (node_sort.js).
    pub sort_flag: Value,
    /// `area_return` (node_sort.js).
    pub area_return: Option<String>,
    /// `state.build[this.strings.name] = this` of cs:name-part: "family",
    /// "given", as token values (node_namepart.js; wave1-nodes).
    pub name_parts: Obj,
    /// How deep `run_builder` is (`CSL.makeBuilder` calls nested through
    /// macros): 1 is an area's own token list, more is a macro's. Rust-only:
    /// upstream builds straight into `state.<area>.tokens`, which this port
    /// takes out of the state while it builds (see `bibliography_tokens_len`).
    pub builder_depth: usize,
    /// `state.bibliography.tokens.length` as `@display` (attributes.js) reads
    /// it while the bibliography list is out of the state being built:
    /// updated by `xml_to_token` for the bibliography's own list, and set to
    /// the final length once the area is built.
    pub bibliography_tokens_len: usize,
    // ---- fields: wave2 ----

    // ---- fields: wave3 ----
    /// The closures (`execs`) of the `cs:name-part` tokens held in
    /// [`Build::name_parts`] (which keeps only their JSON form, where closures
    /// are counted, not stored): `family` and `given`. node_names.js END copies
    /// them into the `Output` closure it builds (they run on the name-part
    /// clones in `outputNames`). Rust-only: upstream keeps the tokens.
    pub name_part_execs: BTreeMap<String, Vec<super::exec::Exec>>,

    // ---- fields: wave4 ----

    // ---- fields: wave5 ----
}

impl Build {
    /// `new CSL.Engine.Build()`.
    pub fn new() -> Build {
        Build {
            alternate_term: Value::Bool(false),
            in_bibliography: false,
            in_style: false,
            skip: None,
            postponed_macro: Value::Bool(false),
            layout_flag: false,
            name: Value::Bool(false),
            names_variables: vec![Vec::new()],
            name_label: vec![Vec::new()],
            form: Value::Bool(false),
            term: Value::Bool(false),
            macro_: Obj::new(),
            macro_stack: Vec::new(),
            text: Value::Bool(false),
            lang: None,
            area: "citation".to_string(),
            root: "citation".to_string(),
            extension: String::new(),
            substitute_level: Stack::with(0),
            names_level: 0,
            render_nesting_level: 0,
            render_seen: false,
            bibliography_key_pos: 0,
            csl_node_id: 0,
            current_default_locale: Value::Null,
            date_key: false,
            date_variables: Vec::new(),
            date_parts: Vec::new(),
            layout_locale_flag: false,
            name_flag: false,
            names_flag: false,
            name_delimiter: Value::Null,
            cls: None,
            has_institution: false,
            plural: Value::Null,
            publisher_special: false,
            sort_flag: Value::Null,
            area_return: None,
            name_parts: Obj::new(),
            builder_depth: 0,
            bibliography_tokens_len: 0,
            name_part_execs: BTreeMap::new(),
        }
    }
}

impl Default for Build {
    fn default() -> Self {
        Build::new()
    }
}

/// `CSL.Engine.Fun`: helper objects (`match`, `suffixator`, `romanizer`,
/// `ordinalizer`, `long_ordinalizer`, `dateparser`, `flipflopper`,
/// `page_mangler`, `year_mangler`, `decorate`).
#[derive(Debug, Clone, Default)]
pub struct Fun {
    /// `dateparser` (one per engine; PORTING.md §8).
    pub dateparser: DateParser,
    /// `flipflopper`.
    pub flipflopper: FlipFlopper,
    // ---- fields: wave1-build ----
    /// `match`: `new CSL.Util.Match()` (src/util.js).
    pub match_: Match,
    // ---- fields: wave1-input ----
    /// `ordinalizer`.
    pub ordinalizer: Ordinalizer,
    /// `long_ordinalizer`.
    pub long_ordinalizer: LongOrdinalizer,
    /// `romanizer`.
    pub romanizer: Romanizer,
    /// `suffixator`.
    pub suffixator: Suffixator,

    // ---- fields: wave1-output ----
    /// `decorate` (`CSL.Mode(mode)`, set by `setOutputFormat`): the output
    /// format whose decorators and escaping are used. Install with
    /// `formats::set_output_format`.
    pub decorate: super::formats::Format,
    /// `page_mangler` (`CSL.Util.PageRangeMangler.getFunction(state, "page")`).
    pub page_mangler: super::util_page::PageRangeMangler,
    /// `year_mangler` (`getFunction(state, "year")`).
    pub year_mangler: super::util_page::PageRangeMangler,
    /// `state.locale[lang].opts["skip-words-regexp"]`, see
    /// `formatters::make_skip_words_regex`; `None` means the default list.
    pub skip_words_rex: Option<regex::Regex>,
    /// Which optional `sys` callbacks the host provides (JS tests
    /// `state.sys.variableWrapper` etc. for existence).
    pub host_hooks: super::formats::HostHooks,
    // ---- fields: wave2 ----
}

impl Fun {
    /// `new CSL.Engine.Fun(state)`: the helper objects with their defaults.
    /// `State::new` then builds the ones that read the locale (`flipflopper`,
    /// `page_mangler`, `year_mangler`, `skip_words_rex`) and sets `decorate`
    /// (`setOutputFormat`), as the `CSL.Engine` constructor does.
    pub fn new() -> Fun {
        Fun::default()
    }
}

/// `CSL.Engine.Configure`: the back-to-front jump-index pass.
#[derive(Debug, Clone, Default)]
pub struct Configure {
    /// `tests`, `fail`, `succeed`: stacks of token positions.
    pub tests: Vec<usize>,
    pub fail: Vec<usize>,
    pub succeed: Vec<usize>,
}

impl Configure {
    /// `new CSL.Engine.Configure()`.
    pub fn new() -> Configure {
        Configure::default()
    }
}
