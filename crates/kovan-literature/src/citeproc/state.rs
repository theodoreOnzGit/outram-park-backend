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
use super::util_number::{
    InputLocale, LongOrdinalizer, Ordinalizer, Romanizer, ShadowNumber, Suffixator,
};
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

    // ---- fields: wave1-input (dates, numbers, name particles, retrieveItem) ----
    /// PROVISIONAL (wave1-input): the locale terms and `ord["1.0.1"]` that
    /// `util_number.rs` and `util_dates.rs` read, answered by
    /// `input_get_term` / `input_get_field`. The integrator replaces those
    /// two functions with `State::get_term` / `getField` and removes this.
    pub input_locale: InputLocale,
    /// PROVISIONAL (wave1-input): `registry.refhash`, the normalised items
    /// by id that `retrieveItem` returns on later calls. Belongs to the
    /// registry (wave4); `build_retrieve_item.rs` reads and writes this
    /// field until `Registry::refhash` exists.
    pub item_refhash: BTreeMap<String, serde_json::Value>,

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

    // ---- fields: wave1-input ----
    /// `shadow_numbers`: parsed numeric variables by variable name
    /// (`processNumber`, util_number.js).
    pub shadow_numbers: BTreeMap<String, ShadowNumber>,
    /// `just_looking` (read by `processNumber`'s `setStyling`; set by the
    /// wave that runs the "just looking" pass: another agent may add it too).
    pub just_looking: bool,
    /// `cite_renders_content` (`getTerm` and `LongOrdinalizer.format` set
    /// it; may also be added by the wave that owns citation rendering).
    pub cite_renders_content: bool,
    /// `loadedItemIDs`: ids `retrieveItem` has already loaded.
    pub loaded_item_ids: BTreeMap<String, bool>,
    /// `taintedItemIDs`: ids whose normalised item changed on a re-fetch.
    pub tainted_item_ids: BTreeMap<String, bool>,

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
    /// `ordinalizer`.
    pub ordinalizer: Ordinalizer,
    /// `long_ordinalizer`.
    pub long_ordinalizer: LongOrdinalizer,
    /// `romanizer`.
    pub romanizer: Romanizer,
    /// `suffixator`.
    pub suffixator: Suffixator,

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
