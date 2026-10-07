// Part of the kovan port of citeproc-js (GitHub #790).
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

use std::collections::BTreeMap;
use std::sync::Arc;

use super::disambig_cites::Disambiguation;
use super::js::Obj;
use super::obj_blob::Blobs;
use super::obj_token::Token;
use super::queue::Queue;
use super::registry::{Comparifier, Registry};
use super::util_dateparser::DateParser;
use super::util_flipflop::FlipFlopper;
use super::util_locale::Locale;
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
    pub macros: BTreeMap<String, Arc<Vec<Token>>>,
    /// `registry`.
    pub registry: Registry,
    /// `disambiguate`.
    pub disambiguate: Disambiguation,
    /// `transform`.
    pub transform: Transform,
    /// `parallel` (only when `opt.parallel.enable`).
    pub parallel: Option<Parallel>,
    /// `juris`.
    pub juris: Obj,
    /// `splice_delimiter`.
    pub splice_delimiter: Option<String>,

    // ---- fields: wave1-build (load, xmljson, build, util_locale, attributes) ----

    // ---- fields: wave1-nodes ----
    // DUP-CHECK: state.js. `state.intext_sort` (node_intext.js): only
    // `opt.sort_directions` is ever set.
    pub intext_sort: Area,
    /// `state.build_layout_locale_flag` (node_layout.js:259; a misspelling of
    /// `state.build.layout_locale_flag` upstream, kept as its own flag).
    pub build_layout_locale_flag: bool,
    /// `sys.variableWrapper` is installed (`Engine::set_variable_wrapper`).
    pub sys_variable_wrapper: bool,

    // ---- fields: wave1-input (dates, numbers, name particles, retrieveItem) ----

    // ---- fields: wave1-output (queue, formats, formatters, flip-flop, page) ----

    // ---- fields: wave2 (rendering nodes, api_cite core) ----

    // ---- fields: wave3 (names) ----

    // ---- fields: wave4 (registry, disambiguation, sort) ----

    // ---- fields: wave5 (citations API, bibliography) ----
}

/// One rendering or sorting area: `CSL.Engine.Citation`, `.Bibliography`,
/// `.InText`, `.CitationSort`, `.BibliographySort`.
#[derive(Debug, Clone, Default)]
pub struct Area {
    /// `opt`.
    pub opt: Obj,
    /// `tokens`: the configured token list.
    pub tokens: Arc<Vec<Token>>,
    /// `root`: `"citation"`, `"bibliography"` or `"intext"`.
    pub root: String,
    /// `srt` (citation and bibliography only).
    pub srt: Option<Comparifier>,
    /// `keys` (the sort areas).
    pub keys: Vec<serde_json::Value>,
    /// `tmp` (bibliography_sort only).
    pub tmp: Obj,
}

/// `CSL.Engine.Tmp`: per-render scratch state.
#[derive(Debug, Clone, Default)]
pub struct Tmp {
    // ---- fields: wave1-build ----

    // ---- fields: wave1-nodes ----
    // DUP-CHECK: state.js for every field of this block.
    /// `area`: "citation", "bibliography", "intext", "citation_sort", ...
    pub area: String,
    /// `root`; `None` is JS `undefined` (node_name.js tests for it).
    pub root: Option<String>,
    /// `extension`: "" or "_sort".
    pub extension: String,
    /// `jump`: values are "succeed" / "fail"; `None` is the `undefined`
    /// that cs:choose pushes (node_choose.js).
    pub jump: super::stack::Stack<Option<String>>,
    /// `conditions`: the cs:if/cs:else-if being filled by cs:conditions.
    pub conditions: Option<super::util_conditions::ConditionsEngine>,
    /// `condition_counter`.
    pub condition_counter: i64,
    /// `condition_lang_counter_arr`.
    pub condition_lang_counter_arr: Vec<i64>,
    /// `condition_lang_val_arr`.
    pub condition_lang_val_arr: Vec<String>,
    /// `cite_affixes`: per area `false` or {locale: {delimiter, suffix}}.
    pub cite_affixes: Obj,
    /// `last_cite_locale`.
    pub last_cite_locale: Option<String>,
    /// `etal_node`: the cs:et-al token (as `node_names::token_to_value`).
    pub etal_node: Option<serde_json::Value>,
    /// `etal_term`.
    pub etal_term: Option<String>,
    /// `abort_alternative`.
    pub abort_alternative: bool,
    /// `date_object` (`false` when the date is not rendered).
    pub date_object: serde_json::Value,
    /// `donesies`.
    pub donesies: Vec<String>,
    /// `dateparts`.
    pub dateparts: Vec<String>,
    /// `date_collapse_at`.
    pub date_collapse_at: Vec<String>,
    /// `element_rendered_ok`.
    pub element_rendered_ok: bool,
    /// `date_token` (set at build time by cs:date).
    pub date_token: Option<Token>,
    /// `just_looking`.
    pub just_looking: bool,
    /// `done_vars`.
    pub done_vars: Vec<String>,
    /// `sort_key_flag`.
    pub sort_key_flag: bool,
    /// `nameset_counter`.
    pub nameset_counter: i64,
    /// `strip_periods`.
    pub strip_periods: i64,
    /// `can_substitute`.
    pub can_substitute: super::stack::Stack<bool>,
    /// `can_block_substitute`.
    pub can_block_substitute: bool,
    /// `common_term_match_fail`.
    pub common_term_match_fail: bool,
    /// `value`.
    pub value: Vec<serde_json::Value>,
    /// `element_trace`.
    pub element_trace: super::stack::Stack<String>,
    /// `probably_rendered_something`.
    pub probably_rendered_something: bool,
    /// `container_item_count`, `container_item_pos` (keyed by container_id).
    pub container_item_count: Obj,
    pub container_item_pos: Obj,
    /// `et-al-min`, `et-al-use-first`, `et-al-use-last` (`None` = undefined).
    pub et_al_min: Option<serde_json::Value>,
    pub et_al_use_first: Option<serde_json::Value>,
    pub et_al_use_last: Option<serde_json::Value>,
    /// `lang_sort_hold` (node_sort.js).
    pub lang_sort_hold: Option<String>,

    // ---- fields: wave1-input ----

    // ---- fields: wave1-output ----

    // ---- fields: wave2 ----

    // ---- fields: wave3 ----

    // ---- fields: wave4 ----

    // ---- fields: wave5 ----
}

/// `CSL.Engine.Build`: state while compiling the style.
#[derive(Debug, Clone, Default)]
pub struct Build {
    // ---- fields: wave1-build ----

    // ---- fields: wave1-nodes ----
    // DUP-CHECK: state.js for every field of this block.
    /// `area`, `root`, `extension`.
    pub area: String,
    pub root: String,
    pub extension: String,
    /// `skip`: `Some("info")` while inside cs:info, `None` is JS `false`.
    pub skip: Option<String>,
    /// `substitute_level` (starts as `Stack(0, LITERAL)`).
    pub substitute_level: super::stack::Stack<i64>,
    /// `names_level`, `render_nesting_level`.
    pub names_level: i64,
    pub render_nesting_level: i64,
    /// `cls` (a truthy string while a display block is open).
    pub cls: Option<String>,
    /// `date_parts`, `date_variables`, `date_key`.
    pub date_parts: Vec<String>,
    pub date_variables: Vec<String>,
    pub date_key: bool,
    /// `names_variables` (a stack of variable lists) and `name_label` (a
    /// stack of {variable: {before, after}} kept as ordered pairs because
    /// `Object.keys` order matters; tokens as `node_names::token_to_value`).
    pub names_variables: Vec<Vec<String>>,
    pub name_label: Vec<Vec<(String, serde_json::Value)>>,
    pub name_flag: bool,
    pub names_flag: bool,
    /// `state.build[this.strings.name] = this` of cs:name-part: "family",
    /// "given" (and "et-al" / "with" if ever set), as token values.
    pub name_parts: Obj,
    /// `layout_flag`, `layout_locale_flag`.
    pub layout_flag: bool,
    pub layout_locale_flag: bool,
    /// `current_default_locale` (a string or the `default-locale` list).
    pub current_default_locale: serde_json::Value,
    /// `publisher-special`.
    pub publisher_special: bool,
    /// `has_institution`.
    pub has_institution: bool,
    /// `term`, `form`, `plural` (cs:text resets them to `false`).
    pub term: serde_json::Value,
    pub form: serde_json::Value,
    pub plural: serde_json::Value,
    /// `lang` (set by the `lang` attribute).
    pub lang: Option<String>,

    // ---- fields: wave2 ----

    // ---- fields: wave3 ----

    // ---- fields: wave4 ----

    // ---- fields: wave5 ----
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

    // ---- fields: wave1-input ----

    // ---- fields: wave1-output ----

    // ---- fields: wave2 ----
}

/// `CSL.Engine.Configure`: the back-to-front jump-index pass.
#[derive(Debug, Clone, Default)]
pub struct Configure {
    /// `tests`, `fail`, `succeed`: stacks of token positions.
    pub tests: Vec<usize>,
    pub fail: Vec<usize>,
    pub succeed: Vec<usize>,
}
