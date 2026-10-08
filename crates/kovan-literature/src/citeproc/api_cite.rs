// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/api_cite.js (the whole file), plus the citation-item input steps of src/util_static_locator.js (moved here: they are the loop bodies of processCitationCluster and makeCitationCluster)
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

//! The citation API of the engine: `previewCitationCluster`,
//! `appendCitationCluster`, `processCitationCluster`, `makeCitationCluster`,
//! `CSL.getAmbiguousCite`, `CSL.getSpliceDelimiter`, `CSL.getCitationCluster`,
//! `CSL.getCite`, `CSL.citeStart` and `CSL.citeEnd`.
//!
//! # How the JS objects map
//!
//! * The citation object the caller passes to `processCitationCluster` (and
//!   which citeproc-js keeps and mutates) is a
//!   [`CitationRec`] in the registry's arena; `citationById`,
//!   `citationByIndex` and `citationsByItemId` hold [`CitId`]s.
//! * `citation.sortedItems` is a list of `[Item, item]` pairs. `item` (the
//!   copy of the caller's citation item, which accumulates `sortkeys`,
//!   `position`, `near-note`, ...) is an `Obj`; `Item` is
//!   `registry.refhash[id]` and is read when needed.
//! * `this.output.queue` is the root array blob of `state.output`
//!   ([`queue::queue_children`]); `getCitationCluster` replaces it with
//!   `[blob]` for each cite exactly as upstream does.
//!

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::build_retrieve_item::retrieve_item;
use super::disambig_citations::{CitId, CitationRec, SortedItem};
use super::formats;
use super::js::{self, Obj};
use super::load::{
    self, check_prefix_space_append, check_suffix_space_prepend, get_safe_escape,
    locator_labels_map, parse_locator, CheckNestedBrace, LOCATOR_LABELS_REGEXP,
    SWAPPING_PUNCTUATION, TERMINAL_PUNCTUATION,
};
use super::obj_ambigconfig::{AmbigConfig, AmbigId};
use super::obj_blob::{BlobChild, BlobContent, BlobId};
use super::queue::{self, QueueId, Rendered, StringParent};
use super::registry::{self, sort_with, ReturnData};
use super::state::{AbbrevTrimmer, GroupContext, State};
use super::util_nodes::{TokenList, NEXT_UNDEFINED};
use super::util_number::input_get_term_name;
use super::util_parallel;
use super::util_static_locator::{remap_section_variable, remap_section_variable_one};
use super::{CslResult, EngineError};

fn type_error(what: &str) -> EngineError {
    EngineError::Csl(format!("TypeError: {what}"))
}

/// The legal item types `remapSectionVariable` leaves alone in the
/// locator-label step.
const LEGAL_TYPES: [&str; 5] = ["bill", "gazette", "legislation", "regulation", "treaty"];

/// A citation as the caller passes it to `processCitationCluster`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CitationInput {
    /// `citationID` (`None`: the engine makes one).
    pub citation_id: Option<String>,
    /// `citationItems`, each an object with at least `id`.
    pub citation_items: Vec<Obj>,
    /// `properties` (`None`: `{noteIndex: 0}`).
    pub properties: Option<Obj>,
}

/// An entry of `citationsPre` / `citationsPost`: `[citationID, noteIndex]`.
#[derive(Debug, Clone, PartialEq)]
pub struct CitationPos {
    /// The citation's ID.
    pub citation_id: String,
    /// Its note index (a number in practice).
    pub note_index: Value,
}

/// The `flag` argument of `processCitationCluster`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClusterFlag {
    /// No flag.
    None,
    /// `CSL.PREVIEW`: render only, leaving the registry as it was.
    Preview,
    /// `CSL.ASSUME_ALL_ITEMS_REGISTERED`: skip `updateItems`.
    AssumeAllItemsRegistered,
}

/// What `processCitationCluster` returns: `[return_data, ret]`. `ret` holds
/// `[index, string, citationID]` triples, or (preview) the one string.
#[derive(Debug, Clone, PartialEq)]
pub struct ClusterResult {
    /// `registry.return_data` (`bibchange`, `citation_errors`).
    pub return_data: ReturnData,
    /// The `[index, text, citationID]` triples of the citations that changed.
    pub updates: Vec<(i64, String, String)>,
    /// With `CSL.PREVIEW`: the rendered string.
    pub preview: Option<String>,
}

fn obj_id(item: &Obj) -> String {
    super::registry::id_key(item.get("id"))
}

/// `Item.id` of a registered item value, as a string.
fn item_value_id(item: &Value) -> String {
    super::registry::id_key(item.get("id"))
}

/// JS `a == b` between two optional values (`undefined == undefined` is true),
/// as the position code compares ids and locator extras.
fn loose_eq(a: Option<&Value>, b: Option<&Value>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(Value::Null), None) | (None, Some(Value::Null)) => true,
        (None, _) | (_, None) => false,
        (Some(x), Some(y)) => {
            if x == y {
                return true;
            }
            let num = |v: &Value| match v {
                Value::Number(n) => n.as_f64(),
                Value::String(s) => js::trim(s).parse::<f64>().ok(),
                Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
                _ => None,
            };
            match (x, y) {
                (Value::String(_), Value::Number(_)) | (Value::Number(_), Value::String(_)) => {
                    matches!((num(x), num(y)), (Some(p), Some(q)) if p == q)
                }
                _ => false,
            }
        }
    }
}

/// The token list `this[area]` of a rendering or sort area name.
fn token_list_for(area: &str) -> CslResult<TokenList> {
    match area {
        "citation" => Ok(TokenList::Citation),
        "bibliography" => Ok(TokenList::Bibliography),
        "intext" => Ok(TokenList::Intext),
        "citation_sort" => Ok(TokenList::CitationSort),
        "bibliography_sort" => Ok(TokenList::BibliographySort),
        other => Err(type_error(&format!(
            "Cannot read properties of undefined (reading 'tokens') [area {other}]"
        ))),
    }
}

// ---------------------------------------------------------------------------
// The citation-item input steps
// ---------------------------------------------------------------------------

/// The locator-label step shared by `processCitationCluster` and
/// `makeCitationCluster`: with `locator_label_parse`, move a label embedded in
/// a plain locator (`"ch. 3"`) into `item.label` when the locale has a term
/// for it (api_cite.js:117-127 and 887-898).
fn locator_label_parse(state: &mut State, item_obj: &Obj, item: &mut Obj) {
    if !state.dev_ext("locator_label_parse") {
        return;
    }
    let is_legal = item_obj
        .get("type")
        .and_then(Value::as_str)
        .map(|t| LEGAL_TYPES.contains(&t))
        .unwrap_or(false);
    let label_is_page_or_none =
        !js::get_truthy(item, "label") || item.get("label").and_then(Value::as_str) == Some("page");
    if js::get_truthy(item, "locator") && !is_legal && label_is_page_or_none {
        let locator = js::to_js_string(item.get("locator").unwrap_or(&Value::Null));
        if let Some(m) = LOCATOR_LABELS_REGEXP.captures(&locator) {
            let m2 = m.get(2).map(|x| x.as_str()).unwrap_or("");
            let m3 = m.get(3).map(|x| x.as_str()).unwrap_or("");
            let try_label = locator_labels_map(m2);
            if input_get_term_name(state, try_label)
                .map(|t| !t.is_empty())
                .unwrap_or(false)
            {
                item.insert(
                    "label".into(),
                    Value::String(try_label.unwrap_or("").to_string()),
                );
                item.insert("locator".into(), Value::String(m3.to_string()));
            }
        }
    }
}

/// The per-citation-item input steps of `processCitationCluster`
/// (api_cite.js:112-127) applied to `item` (already a shallow copy of the
/// caller's citation item) and its `Item`: `CSL.parseLocator`;
/// `remapSectionVariable` when `consolidate_legal_items`; and, with
/// `locator_label_parse`, moving an embedded label such as `"ch. 3"` out of a
/// plain locator into `item.label` when the locale has a term for it.
///
/// `Item` may be mutated (`remapSectionVariable` rewrites `Item.section`).
pub fn citation_item_input(state: &mut State, item_obj: &mut Obj, item: &mut Obj) -> CslResult<()> {
    parse_locator(state, item);
    if state.dev_ext("consolidate_legal_items") {
        remap_section_variable_one(item_obj, item)?;
    }
    locator_label_parse(state, item_obj, item);
    Ok(())
}

// ---------------------------------------------------------------------------
// getCite, citeStart, citeEnd, getAmbiguousCite
// ---------------------------------------------------------------------------

static LANG_HEAD: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new("^([a-zA-Z]+)").expect("static regex")
});

impl State {
    /// `CSL.getCite.call(state, Item, item, prevItemID, blockShadowNumberReset)`:
    /// render a single cite item into the output queue; returns `"" + Item.id`.
    /// `cite_item` is `Value::Null` for `undefined` (bibliography entries).
    pub fn get_cite(
        &mut self,
        item: &Value,
        cite_item: &Value,
        prev_item_id: Option<&str>,
        block_shadow_number_reset: bool,
    ) -> CslResult<String> {
        let area_orig = self.tmp.area.clone();
        if js::truthy_opt(cite_item.get("author-only")) && !self.intext.tokens.is_empty() {
            self.tmp.area = "intext".to_string();
        }
        self.tmp.cite_renders_content = false;
        self.tmp.probably_rendered_something = false;
        self.tmp.prev_item_id = prev_item_id.map(str::to_string);

        self.cite_start(item, cite_item, block_shadow_number_reset)?;
        let mut next: usize = 0;
        self.tmp.name_node = Default::default();
        self.new_name_output(item, cite_item);

        // rerun?
        loop {
            let area = self.tmp.area.clone();
            let list = token_list_for(&area)?;
            let len = self.area_ref(&area).tokens.len();
            if next >= len || next == NEXT_UNDEFINED {
                break;
            }
            next = self.token_exec(&list, next, item, cite_item)?;
        }

        self.cite_end(item, cite_item)?;
        // Odd place for this, but it seems to fit here
        if !self.tmp.cite_renders_content
            && !self.tmp.just_looking
            && self.tmp.area == "bibliography"
        {
            let mut error_object = Obj::new();
            error_object.insert("index".into(), Value::from(self.tmp.bibliography_pos));
            error_object.insert("itemID".into(), Value::String(item_value_id(item)));
            error_object.insert(
                "error_code".into(),
                Value::from(load::ERROR_NO_RENDERED_FORM),
            );
            self.tmp
                .bibliography_errors
                .push(Value::Object(error_object));
        }
        self.tmp.area = area_orig;
        Ok(item_value_id(item))
    }

    /// `CSL.citeStart.call(state, Item, item, blockShadowNumberReset)`.
    pub fn cite_start(
        &mut self,
        item: &Value,
        cite_item: &Value,
        block_shadow_number_reset: bool,
    ) -> CslResult<()> {
        self.tmp.lang_array = Vec::new();
        if let Some(language) = item.get("language").filter(|v| js::truthy(v)) {
            // Guard against garbage locales in user input
            if let Some(m) = LANG_HEAD.captures(&js::to_js_string(language)) {
                self.tmp.lang_array.push(m[1].to_lowercase());
            }
        }
        let lang = js::get_string(&self.opt, "lang").unwrap_or_default();
        self.tmp.lang_array.push(lang);
        if !block_shadow_number_reset {
            self.tmp.shadow_numbers = BTreeMap::new();
        }

        self.tmp.disambiguate_count = 0;
        self.tmp.disambiguate_max_max = 0;
        self.tmp.same_author_as_previous_cite = false;
        self.tmp.subsequent_author_substitute_ok = !self.tmp.suppress_decorations;
        self.tmp.lastchr = String::new();
        let collapse_set = self
            .citation
            .opt
            .get("collapse")
            .map(|c| match c {
                Value::String(s) => !s.is_empty(),
                Value::Array(a) => !a.is_empty(),
                other => js::truthy(other),
            })
            .unwrap_or(false);
        // this.tmp.have_collapsed = "year";
        self.tmp.have_collapsed = self.tmp.area == "citation" && collapse_set;
        self.tmp.render_seen = false;

        let item_id = item_value_id(item);
        let reg_token = self.registry.registry.get(&item_id).cloned();
        if self.tmp.disambig_request.is_some() && !self.tmp.disambig_override {
            self.tmp.disambig_settings = self.tmp.disambig_request;
        } else if reg_token.is_some() && !self.tmp.disambig_override {
            self.tmp.disambig_request = reg_token.as_ref().and_then(|t| t.disambig);
            self.tmp.disambig_settings = self.tmp.disambig_request;
        } else {
            let fresh = self.alloc_ambig(AmbigConfig::default());
            self.tmp.disambig_settings = Some(fresh);
        }
        if self.tmp.area != "citation" {
            match reg_token {
                None => self.tmp.disambig_restore = Some(AmbigConfig::default()),
                Some(token) => {
                    let reg_disambig = token.disambig.ok_or_else(|| {
                        type_error("Cannot read properties of false (reading 'names')")
                    })?;
                    self.tmp.disambig_restore = Some(super::util_disambig::clone_ambig_config(
                        self.ambig(reg_disambig),
                        None,
                    ));
                    if self.tmp.area == "bibliography"
                        && self.tmp.disambig_settings.is_some()
                        && self.tmp.disambig_override
                    {
                        let settings = self.disambig_settings_id();
                        if js::truthy_opt(self.opt.get("disambiguate-add-names")) {
                            let names = self.ambig(reg_disambig).names.clone();
                            self.ambig_mut(settings).names = names.clone();
                            if let Some(request) = self.tmp.disambig_request {
                                self.ambig_mut(request).names = names;
                            }
                        }
                        if js::truthy_opt(self.opt.get("disambiguate-add-givenname")) {
                            // This is weird and delicate and not fully understood
                            self.tmp.disambig_request = Some(settings);
                            let givens = self.ambig(reg_disambig).givens.clone();
                            self.ambig_mut(settings).givens = givens;
                        }
                    }
                }
            }
        }

        self.tmp.names_used = Vec::new();
        self.tmp.nameset_counter = 0;
        self.tmp.years_used = Vec::new();
        self.tmp.names_max.clear();
        if !self.tmp.just_looking {
            let parallel = cite_item.get("parallel");
            if cite_item.is_null()
                || parallel.and_then(Value::as_str) == Some("first")
                || !js::truthy_opt(parallel)
            {
                self.tmp.abbrev_trimmer = Some(AbbrevTrimmer::default());
            }
        }

        let area = self.tmp.area.clone();
        self.tmp.splice_delimiter = self
            .area_ref(&area)
            .opt
            .get("layout_delimiter")
            .map(js::to_js_string);
        //this.tmp.splice_delimiter = this[this.tmp.area].opt.delimiter;

        self.bibliography_sort.keys = Vec::new();
        self.citation_sort.keys = Vec::new();

        self.tmp.has_done_year_suffix = false;
        self.tmp.last_cite_locale = None;
        // SAVE PARAMETERS HERE, IF APPROPRIATE
        // (promiscuous addition of global parameters => death by a thousand cuts)
        if !self.tmp.just_looking
            && !cite_item.is_null()
            && !js::truthy_opt(cite_item.get("position"))
        {
            if let Some(token) = self.registry.registry.get(&item_id) {
                let d = token.disambig.ok_or_else(|| {
                    type_error("Cannot read properties of false (reading 'names')")
                })?;
                self.tmp.disambig_restore = Some(super::util_disambig::clone_ambig_config(
                    self.ambig(d),
                    None,
                ));
            }
        }
        // XXX This only applied to the "number" variable itself? Huh?
        //this.setNumberLabels(Item);
        self.tmp.first_name_string = None;
        self.tmp.authority_stop_last = 0;
        Ok(())
    }

    /// `CSL.citeEnd.call(state, Item, item)`.
    pub fn cite_end(&mut self, item: &Value, cite_item: &Value) -> CslResult<()> {
        // RESTORE PARAMETERS IF APPROPRIATE
        let item_id = item_value_id(item);
        if let Some(restore) = self.tmp.disambig_restore.clone() {
            if let Some(d) = self.registry.registry.get(&item_id).map(|t| t.disambig) {
                let d = d.ok_or_else(|| {
                    type_error("Cannot set properties of false (setting 'names')")
                })?;
                let c = self.ambig_mut(d);
                c.names = restore.names.clone();
                c.givens = restore.givens.clone();
            }
        }
        self.tmp.disambig_restore = None;

        self.tmp.last_suffix_used = match cite_item.get("suffix").filter(|v| js::truthy(v)) {
            //this.tmp.last_suffix_used = this.tmp.suffix.value();
            Some(s) if !cite_item.is_null() => js::to_js_string(s),
            _ => String::new(),
        };
        self.tmp.last_years_used = self.tmp.years_used.clone();
        self.tmp.last_names_used = self.tmp.names_used.clone();
        self.tmp.cut_var = false;

        // This is a hack, in a way; I have lost track of where
        // the disambig (name rendering) settings used for rendering work their way
        // into the registry.  This resets defaults to the subsequent form,
        // when first cites are rendered.
        self.tmp.disambig_request = None;

        let cl = match &self.tmp.last_cite_locale {
            Some(l) => Value::String(l.clone()),
            None => Value::Bool(false),
        };
        self.tmp.cite_locales.push(cl);

        if let Some(issued) = self.tmp.issued_date {
            if self.tmp.renders_collection_number {
                let list_len = match &self.blobs.get(issued.list).blobs {
                    BlobContent::List(l) => l.len(),
                    BlobContent::Text(_) => 0,
                };
                let mut buf: Vec<BlobChild> = Vec::new();
                let mut i = list_len as i64 - 1;
                if let BlobContent::List(list) = &mut self.blobs.get_mut(issued.list).blobs {
                    while i > issued.pos as i64 {
                        if let Some(c) = list.pop() {
                            buf.push(c);
                        }
                        i -= 1;
                    }
                    // Throw away the unwanted blob
                    list.pop();
                    // Put the other stuff back
                    while let Some(c) = buf.pop() {
                        list.push(c);
                    }
                }
            }
        }
        self.tmp.issued_date = None;
        self.tmp.renders_collection_number = false;
        Ok(())
    }

    /// `CSL.getAmbiguousCite.call(state, Item, disambig, visualForm, item)`:
    /// the undisambiguated cite of `Item` without decorations (used by the
    /// registry to find ambiguous cites). `disambig` is the config to render
    /// with (it is shared with the name code, which writes into it).
    ///
    /// Returns what `output.string` returned: a string, or, when the cite
    /// rendered nothing, an empty *array* (see `Disambiguation::scan_items`:
    /// two such results are never `===`). Use [`Rendered::to_js_string`] for
    /// the ambiguous-cite key.
    pub fn get_ambiguous_cite(
        &mut self,
        item: &Value,
        disambig: Option<AmbigId>,
        visual_form: bool,
        cite_item: Option<&Obj>,
    ) -> CslResult<Rendered> {
        let flags = self.tmp.group_context.tip().cloned().ok_or_else(|| {
            type_error("Cannot read properties of undefined (reading 'term_intended')")
        })?;
        let old_term_sibling_layer = GroupContext {
            term_intended: flags.term_intended,
            variable_attempt: flags.variable_attempt,
            variable_success: flags.variable_success,
            output_tip: flags.output_tip,
            label_form: flags.label_form.clone(),
            non_parallel: flags.non_parallel.clone(),
            parallel_last: flags.parallel_last.clone(),
            parallel_first: flags.parallel_first.clone(),
            parallel_last_override: flags.parallel_last_override.clone(),
            parallel_delimiter_override: flags.parallel_delimiter_override.clone(),
            parallel_delimiter_override_on_suppress: flags
                .parallel_delimiter_override_on_suppress
                .clone(),
            condition: flags.condition.clone(),
            force_suppress: flags.force_suppress,
            done_vars: flags.done_vars.clone(),
            ..GroupContext::default()
        };
        self.tmp.disambig_request = disambig;
        let mut item_supp = Obj::new();
        item_supp.insert("position".into(), Value::from(load::POSITION_SUBSEQUENT));
        item_supp.insert("near-note".into(), Value::Bool(true));

        if let Some(ci) = cite_item {
            if let Some(l) = ci.get("locator") {
                item_supp.insert("locator".into(), l.clone());
            }
            if let Some(l) = ci.get("label") {
                item_supp.insert("label".into(), l.clone());
            }
        }

        let item_id = item_value_id(item);
        let has_citations = self
            .registry
            .citationreg
            .citations_by_item_id
            .as_ref()
            .and_then(|m| m.get(&item_id))
            .map(|l| !l.is_empty())
            .unwrap_or(false);
        if self.registry.registry.contains_key(&item_id)
            && has_citations
            && visual_form
            && js::get_str(&self.citation.opt, "givenname-disambiguation-rule") == Some("by-cite")
        {
            if let Some(n) = self
                .registry
                .registry
                .get(&item_id)
                .and_then(|t| t.first_reference_note_number)
            {
                item_supp.insert("first-reference-note-number".into(), Value::from(n));
            }
        }
        self.tmp.area = "citation".to_string();
        self.tmp.root = "citation".to_string();
        let orig_suppress_decorations = self.tmp.suppress_decorations;
        self.tmp.suppress_decorations = true;
        self.tmp.just_looking = true;

        let item_supp = Value::Object(item_supp);
        let rendered = self.get_cite(item, &item_supp, None, false);
        if let Err(e) = rendered {
            self.tmp.just_looking = false;
            self.tmp.suppress_decorations = orig_suppress_decorations;
            return Err(e);
        }
        // !!!
        for child in queue::queue_children(self, QueueId::Output) {
            if let BlobChild::Blob(id) = child {
                queue::purge_empty_blobs(&mut self.blobs, id);
            }
        }
        if self.dev_ext("clean_up_csl_flaws") {
            self.adjust_output_queue(false)?;
        }
        let ret = queue::string(
            self,
            QueueId::Output,
            &queue::queue_children(self, QueueId::Output),
            StringParent::None,
        )?;
        self.tmp.just_looking = false;
        self.tmp.suppress_decorations = orig_suppress_decorations;
        // Cache the result.
        self.tmp.group_context.replace(old_term_sibling_layer)?;
        Ok(ret)
    }

    /// The `upward`, `leftward`, `downward`, `fix` passes over every blob of
    /// the output queue (the `clean_up_csl_flaws` loop of `getAmbiguousCite`,
    /// `getCitationCluster` and `getBibliographyEntries`). With `record`, the
    /// last character `fix` saw becomes `tmp.last_chr` (`getCitationCluster`).
    pub fn adjust_output_queue(&mut self, record: bool) -> CslResult<()> {
        let adjust =
            self.output.adjust.clone().ok_or_else(|| {
                type_error("Cannot read properties of undefined (reading 'upward')")
            })?;
        for child in queue::queue_children(self, QueueId::Output) {
            if let BlobChild::Blob(id) = child {
                adjust.upward(&mut self.blobs, id);
                adjust.leftward(&mut self.blobs, id);
                adjust.downward(&mut self.blobs, id);
                let last = adjust.fix(&mut self.blobs, id);
                if record {
                    self.tmp.last_chr = last;
                }
            }
        }
        Ok(())
    }

    /// `CSL.getSpliceDelimiter.call(state, last_locator, last_collapsed, pos)`:
    /// the delimiter to put before cite number `pos`. Evaluation uses three
    /// items of information from the preceding cite: the names used, the
    /// years used, and the suffix appended to the citation.
    pub fn get_splice_delimiter(
        &mut self,
        last_locator: bool,
        last_collapsed: bool,
        pos: usize,
    ) -> Option<String> {
        let after_collapse = self.citation.opt.get("after-collapse-delimiter").cloned();
        if let Some(acd) = after_collapse {
            let acd_s = js::to_js_string(&acd);
            let layout_delimiter = self
                .citation
                .opt
                .get("layout_delimiter")
                .map(js::to_js_string);
            let collapse_is_year_suffix =
                self.citation.opt.get("collapse").and_then(Value::as_str) == Some("year-suffix");
            if last_locator
                || (last_collapsed && !self.tmp.have_collapsed)
                || (!last_collapsed && !self.tmp.have_collapsed && !collapse_is_year_suffix)
            {
                self.tmp.splice_delimiter = Some(acd_s);
            } else {
                self.tmp.splice_delimiter = layout_delimiter;
            }
        } else if self.tmp.use_cite_group_delimiter {
            self.tmp.splice_delimiter = self
                .citation
                .opt
                .get("cite_group_delimiter")
                .map(js::to_js_string);
        } else if self.tmp.have_collapsed
            && js::get_str(&self.opt, "xclass") == Some("in-text")
            && self.opt.get("update_mode").and_then(Value::as_i64) != Some(load::NUMERIC)
        {
            self.tmp.splice_delimiter = Some(", ".to_string());
        } else if let Some(prev_locale) = pos
            .checked_sub(1)
            .and_then(|p| self.tmp.cite_locales.get(p))
            .filter(|v| js::truthy(v))
        {
            //
            // Must have a value to take effect.  Use zero width space to force empty delimiter.
            let alt = self
                .tmp
                .cite_affixes
                .get(&self.tmp.area)
                .and_then(|a| a.get(&js::to_js_string(prev_locale)))
                .and_then(|a| a.get("delimiter"))
                .filter(|d| js::truthy(d))
                .map(js::to_js_string);
            if let Some(d) = alt {
                self.tmp.splice_delimiter = Some(d);
            }
        } else if self
            .tmp
            .splice_delimiter
            .as_deref()
            .map(str::is_empty)
            .unwrap_or(true)
        {
            // This happens when no delimiter is set on cs:layout under cs:citation
            self.tmp.splice_delimiter = Some(String::new());
        }
        self.tmp.splice_delimiter.clone()
    }
}

// ---------------------------------------------------------------------------
// getCitationCluster, process_CitationCluster, makeCitationCluster
// ---------------------------------------------------------------------------

/// What the per-cite pass of `getCitationCluster` records for the cite.
struct CiteParams {
    splice_delimiter: Option<String>,
    suppress_decorations: bool,
    have_collapsed: bool,
}

/// JS `"" + x` where `null` stands for `undefined`.
fn undefined_or_string(v: &Value) -> String {
    match v {
        Value::Null => "undefined".to_string(),
        other => js::to_js_string(other),
    }
}

/// JS `x.slice(-1)` of an optional JSON string.
fn last_char_of(v: Option<&Value>) -> String {
    v.map(|s| js::slice(&js::to_js_string(s), -1, None))
        .unwrap_or_default()
}

impl State {
    /// The children of `state.output.queue`, each a blob.
    pub(crate) fn queue_blobs(&self) -> Vec<BlobId> {
        queue::queue_children(self, QueueId::Output)
            .into_iter()
            .filter_map(|c| match c {
                BlobChild::Blob(id) => Some(id),
                BlobChild::Str(_) => None,
            })
            .collect()
    }

    /// `state.output.queue = blobs`.
    fn set_queue(&mut self, blobs: &[BlobId]) {
        let _ = queue::queue_children(self, QueueId::Output);
        if let Some(root) = self.output.root {
            self.blobs.get_mut(root).blobs =
                BlobContent::List(blobs.iter().map(|b| BlobChild::Blob(*b)).collect());
        }
    }

    /// `CSL.getCitationCluster.call(state, inputList, citation)`: compose the
    /// cites of one cluster into a single string with flexible inter-cite
    /// splicing. `input_list` is the cluster's `[Item, item]` pairs (the
    /// items are updated in place, as upstream updates them); `citation` is
    /// the citation they belong to, when it has one.
    pub fn get_citation_cluster(
        &mut self,
        input_list: &mut [(Value, Obj)],
        citation: Option<CitId>,
    ) -> CslResult<String> {
        let mut citation_id: Option<String> = None;
        let mut author_only = false;
        let mut suppress_author = false;
        let mut citation_prefix = String::new();
        self.output.check_nested_brace = Some(CheckNestedBrace::new(self));
        if let Some(cid) = citation {
            let rec = self.registry.citationreg.get(cid);
            citation_id = Some(rec.citation_id.clone());
            let mode = js::get_str(&rec.properties, "mode");
            author_only = mode == Some("author-only");
            if js::get_str(&self.opt, "xclass") != Some("note") {
                suppress_author = mode == Some("suppress-author");
            }
            if let Some(prefix) = rec.properties.get("prefix").filter(|v| js::truthy(v)) {
                citation_prefix = check_prefix_space_append(self, &js::to_js_string(prefix));
            }
        }
        self.tmp.last_primary_names_string = None;
        let txt_esc = get_safe_escape(self);
        self.tmp.area = "citation".to_string();
        self.tmp.root = "citation".to_string();
        let mut result = String::new();
        let mut objects: Vec<Rendered> = Vec::new();
        self.tmp.last_suffix_used = String::new();
        self.tmp.last_names_used = Vec::new();
        self.tmp.last_years_used = Vec::new();
        self.tmp.backref_index = Vec::new();
        self.tmp.cite_locales = Vec::new();
        if !self.tmp.just_looking {
            self.tmp.abbrev_trimmer = Some(AbbrevTrimmer::default());
        }

        let layout_prefix = js::get_string(&self.citation.opt, "layout_prefix")
            .unwrap_or_else(|| "undefined".into());
        let use_layout_prefix = self
            .output
            .check_nested_brace
            .as_mut()
            .map(|c| c.update(&format!("{layout_prefix}{citation_prefix}")))
            .unwrap_or_default();

        let mut suppress_trailing_punctuation = false;
        if js::truthy_opt(self.citation.opt.get("suppressTrailingPunctuation")) {
            suppress_trailing_punctuation = true;
        }
        if let (Some(cid), Some(id)) = (citation, &citation_id) {
            if !id.is_empty() {
                let by_id = self.registry.citationreg.by_id(id).unwrap_or(cid);
                if js::truthy_opt(
                    self.registry
                        .citationreg
                        .get(by_id)
                        .properties
                        .get("suppress-trailing-punctuation"),
                ) {
                    suppress_trailing_punctuation = true;
                }
            }
        }

        // Adjust locator positions if that looks like a sensible thing to do.
        if js::get_str(&self.opt, "xclass") == Some("note") {
            let mut parasets: Vec<Vec<usize>> = Vec::new();
            let mut last_title: Option<Value> = None;
            let mut last_id: Option<Value> = None;
            for (i, (item, cite)) in input_list.iter().enumerate() {
                let ty = item.get("type").and_then(Value::as_str);
                let title = item.get("title").cloned();
                let position = cite.get("position");
                let id = item.get("id").cloned();
                if js::truthy_opt(title.as_ref())
                    && ty == Some("legal_case")
                    && id != last_id
                    && js::truthy_opt(position)
                {
                    // Start a fresh sublist if the item title does not match the last one
                    if title != last_title || parasets.is_empty() {
                        parasets.push(Vec::new());
                    }
                    if let Some(l) = parasets.last_mut() {
                        l.push(i);
                    }
                }
                last_title = title;
                last_id = id;
            }
            // We now have a list of sublists, each w/matching titles
            for lst in parasets {
                if lst.len() < 2 {
                    continue;
                }
                // Get the locator in last position, but only if it's the only one in the set.
                let last = lst[lst.len() - 1];
                let mut locator_in_last_position: Option<Value> = input_list[last]
                    .1
                    .get("locator")
                    .filter(|v| js::truthy(v))
                    .cloned();
                if locator_in_last_position.is_some() {
                    for &j in &lst[..lst.len() - 1] {
                        if js::truthy_opt(input_list[j].1.get("locator")) {
                            locator_in_last_position = None;
                        }
                    }
                }
                // move the locator here, if it's called for.
                if let Some(loc) = locator_in_last_position {
                    input_list[lst[0]].1.insert("locator".into(), loc);
                    input_list[last].1.remove("locator");
                    let last_label = input_list[last].1.get("label").cloned();
                    match &last_label {
                        Some(l) => {
                            input_list[lst[0]].1.insert("label".into(), l.clone());
                        }
                        None => {
                            input_list[lst[0]].1.remove("label");
                        }
                    }
                    if js::truthy_opt(last_label.as_ref()) {
                        input_list[last].1.remove("label");
                    }
                }
            }
        }
        let len = input_list.len();
        if let Some((_, first)) = input_list.first_mut() {
            if author_only {
                first.remove("suppress-author");
                first.insert("author-only".into(), Value::Bool(true));
            } else if suppress_author {
                first.remove("author-only");
                first.insert("suppress-author".into(), Value::Bool(true));
            }
        }
        if js::truthy_opt(self.opt.get("parallel").and_then(|p| p.get("enable"))) {
            util_parallel::start_citation(self, input_list)?;
        }
        let mut myparams: Vec<CiteParams> = Vec::new();
        // `item` of the last pass of the loop below (JS `var item`).
        let mut last_item: Option<Obj> = None;
        for pos in 0..len {
            // Also for parallels only
            self.tmp.cite_index = pos as i64;

            let item_data = input_list[pos].0.clone();
            parse_locator(self, &mut input_list[pos].1);
            let item = input_list[pos].1.clone();
            let last_collapsed = self.tmp.have_collapsed;
            let mut last_locator = false;
            if pos > 0 {
                last_locator = js::truthy_opt(input_list[pos - 1].1.get("locator"));
            }

            // Reset shadow_numbers here, suppress reset in getCite()
            self.tmp.shadow_numbers = BTreeMap::new();
            if !self.tmp.just_looking && js::truthy_opt(self.opt.get("hasPlaceholderTerm")) {
                let fresh = queue::Queue::new(&mut self.blobs);
                let saved = std::mem::replace(&mut self.output, fresh);
                self.output.adjust = Some(queue::Adjust::new(false));
                let r = self.get_ambiguous_cite(&item_data, None, false, Some(&item));
                self.output = saved;
                r?;
            }

            self.tmp.in_cite_predecessor = false;
            // true is to block reset of shadow numbers
            let item_value = Value::Object(item.clone());
            if pos > 0 {
                let prev_id = item_value_id(&input_list[pos - 1].0);
                self.get_cite(&item_data, &item_value, Some(&prev_id), true)?;
            } else {
                self.tmp.term_predecessor = false;
                self.get_cite(&item_data, &item_value, None, true)?;
            }

            // Make a note of any errors
            if !self.tmp.cite_renders_content {
                let mut error_object = Obj::new();
                error_object.insert(
                    "citationID".into(),
                    Value::String(undefined_or_string(&self.tmp.citation_id)),
                );
                error_object.insert("index".into(), self.tmp.citation_pos.clone());
                error_object.insert("noteIndex".into(), self.tmp.citation_note_index.clone());
                error_object.insert("itemID".into(), Value::String(item_value_id(&item_data)));
                error_object.insert("citationItems_pos".into(), Value::from(pos));
                error_object.insert(
                    "error_code".into(),
                    Value::from(load::ERROR_NO_RENDERED_FORM),
                );
                self.tmp.citation_errors.push(Value::Object(error_object));
            }
            let mut splice = self.get_splice_delimiter(last_locator, last_collapsed, pos);
            // XXX This appears to be superfluous.
            if js::truthy_opt(item.get("author-only")) {
                self.tmp.suppress_decorations = true;
            }

            if pos > 0 {
                let preceding_item = &input_list[pos - 1].1;
                // XXX OR if preceding suffix is empty, and the current prefix begins with a full stop.
                let preceding_suffix = preceding_item.get("suffix").filter(|v| js::truthy(v));
                let ends_in = |c: &str| [";", ".", ","].contains(&c);
                let preceding_ends_in_period_or_comma = preceding_suffix
                    .map(|s| ends_in(&last_char_of(Some(s))))
                    .unwrap_or(false);
                let current_starts_with_period_or_comma = preceding_suffix.is_none()
                    && item
                        .get("prefix")
                        .filter(|v| js::truthy(v))
                        .map(|p| ends_in(&js::slice(&js::to_js_string(p), 0, Some(1))))
                        .unwrap_or(false);
                if preceding_ends_in_period_or_comma || current_starts_with_period_or_comma {
                    let current = splice.clone().unwrap_or_default();
                    let spaceidx = js::index_of(&current, " ", 0);
                    if spaceidx > -1 && !current_starts_with_period_or_comma {
                        splice = Some(js::slice(&current, spaceidx, None));
                    } else {
                        splice = Some(String::new());
                    }
                }
            }
            myparams.push(CiteParams {
                splice_delimiter: splice,
                suppress_decorations: self.tmp.suppress_decorations,
                have_collapsed: self.tmp.have_collapsed,
            });
            //
            // XXXXX: capture parameters to an array, which
            // will be of the same length as this.output.queue,
            // corresponding to each element.
            //
            let author_only_item = js::truthy_opt(item.get("author-only"));
            last_item = Some(item);
            if author_only_item {
                break;
            }
        }
        //
        // output.queue is a simple array.  do a slice
        // of it to get each cite item, setting params from
        // the array that was built in the preceding loop.
        //
        let mut _empties = 0;
        let myblobs = self.queue_blobs();
        let mut citation_suffix = String::new();
        if let Some(cid) = citation {
            let suffix = self
                .registry
                .citationreg
                .get(cid)
                .properties
                .get("suffix")
                .map(js::to_js_string)
                .unwrap_or_default();
            citation_suffix = check_suffix_space_prepend(self, &suffix);
        }
        let mut suffix = js::get_string(&self.citation.opt, "layout_suffix")
            .unwrap_or_else(|| "undefined".into());
        let last_locale = self.tmp.cite_locales.last().cloned();
        //
        // Must have a value to take effect.  Use zero width space to force empty suffix.
        if let Some(ll) = last_locale.filter(|l| js::truthy(l)) {
            let area_affixes = self.tmp.cite_affixes.get(&self.tmp.area);
            if let Some(s) = area_affixes
                .and_then(|a| a.get(&js::to_js_string(&ll)))
                .and_then(|a| a.get("suffix"))
                .filter(|s| js::truthy(s))
            {
                suffix = js::to_js_string(s);
            }
        }
        let first = js::slice(&suffix, 0, Some(1));
        if TERMINAL_PUNCTUATION[..TERMINAL_PUNCTUATION.len() - 1].contains(&first.as_str()) {
            suffix = first;
        }
        //print("=== FROM CITE ===");
        let suffix = self
            .output
            .check_nested_brace
            .as_mut()
            .map(|c| c.update(&format!("{citation_suffix}{suffix}")))
            .unwrap_or_default();

        for id in self.queue_blobs() {
            queue::purge_empty_blobs(&mut self.blobs, id);
        }
        let qb = self.queue_blobs();
        if !self.tmp.suppress_decorations && !qb.is_empty() {
            let wrapper = self.dev_ext("apply_citation_wrapper")
                && self.fun.host_hooks.wrap_citation_entry
                && !self.tmp.just_looking
                && self.tmp.area == "citation";
            if !wrapper {
                if !suppress_trailing_punctuation {
                    if let Some(last) = qb.last() {
                        self.blobs.get_mut(*last).set_string("suffix", &suffix);
                    }
                }
                if let Some(first) = qb.first() {
                    self.blobs
                        .get_mut(*first)
                        .set_string("prefix", &use_layout_prefix);
                }
            }
        }
        if self.dev_ext("clean_up_csl_flaws") {
            self.adjust_output_queue(true)?;
        }
        //print("this.tmp.last_chr="+this.tmp.last_chr);
        for pos in 0..myblobs.len() {
            let mut buffer: Vec<Rendered> = Vec::new();
            self.set_queue(&[myblobs[pos]]);
            self.tmp.suppress_decorations = myparams[pos].suppress_decorations;
            self.tmp.splice_delimiter = myparams[pos].splice_delimiter.clone();
            //
            // oh, one last second thought on delimiters ...
            //
            if let Some(pd) = self
                .blobs
                .get(myblobs[pos])
                .extra
                .get("parallel_delimiter")
                .filter(|v| js::truthy(v))
            {
                self.tmp.splice_delimiter = Some(js::to_js_string(pd));
            }
            self.tmp.have_collapsed = myparams[pos].have_collapsed;

            let composite = queue::string(
                self,
                QueueId::Output,
                &queue::queue_children(self, QueueId::Output),
                StringParent::None,
            )?;

            self.tmp.suppress_decorations = false;
            // meaningless assignment
            // this.tmp.handle_ranges = false;
            let mut composite = match composite {
                Rendered::Str(s) => {
                    self.tmp.suppress_decorations = false;
                    let mut s = s;
                    if s.is_empty() {
                        if self.dev_ext("throw_on_empty") {
                            return Err(EngineError::Csl(
                                "Citation would render no content".into(),
                            ));
                        } else {
                            s = "[NO_PRINTED_FORM]".to_string();
                        }
                    }
                    return Ok(s);
                }
                other => other.into_list(),
            };
            let item_suppress_author = last_item
                .as_ref()
                .map(|i| js::truthy_opt(i.get("suppress-author")))
                .ok_or_else(|| {
                    type_error("Cannot read properties of undefined (reading 'suppress-author')")
                })?;
            if composite.is_empty() && !item_suppress_author {
                if pos == 0 {
                    let err_str = "[CSL STYLE ERROR: reference with no printed form.]";
                    let layout_prefix =
                        js::get_string(&self.citation.opt, "layout_prefix").unwrap_or_default();
                    let layout_suffix =
                        js::get_string(&self.citation.opt, "layout_suffix").unwrap_or_default();
                    let pre_str = txt_esc.escape(&layout_prefix);
                    let suf_str = if pos == myblobs.len() - 1 {
                        txt_esc.escape(&layout_suffix)
                    } else {
                        String::new()
                    };
                    composite.push(Rendered::Str(format!("{pre_str}{err_str}{suf_str}")));
                } else if pos == myblobs.len() - 1 {
                    let layout_suffix =
                        js::get_string(&self.citation.opt, "layout_suffix").unwrap_or_default();
                    match objects.last_mut() {
                        Some(Rendered::Str(s)) => s.push_str(&txt_esc.escape(&layout_suffix)),
                        Some(Rendered::Blob(b)) => {
                            let cur = self.blobs.get(*b).string("suffix");
                            self.blobs.get_mut(*b).set_string(
                                "suffix",
                                &format!("{cur}{}", txt_esc.escape(&layout_suffix)),
                            );
                        }
                        _ => {}
                    }
                }
            }
            // `buffer` is empty here, so upstream's `buffer.length && "string" ===
            // typeof composite[0]` branch never runs; only the else branch does.
            composite.reverse();
            let compie = composite.pop();
            if let Some(c) = compie {
                buffer.push(c);
            }
            // Seems odd, but this was unnecessary and broken.
            let llen = composite.len();
            for ppos in 0..llen {
                let obj = composite.get(ppos).cloned();
                if let Some(Rendered::Str(s)) = obj {
                    let delim = self.tmp.splice_delimiter.clone().unwrap_or_default();
                    buffer.push(Rendered::Str(format!("{}{}", txt_esc.escape(&delim), s)));
                    continue;
                }
                if let Some(c) = composite.pop() {
                    buffer.push(c);
                }
            }
            if buffer.is_empty() && !js::truthy_opt(input_list[pos].1.get("suppress-author")) {
                _empties += 1;
            }
            if buffer.len() > 1 && !matches!(buffer[0], Rendered::Str(_)) {
                let rendered = queue::render_blobs(self, QueueId::Output, buffer, "", false, None)?;
                buffer = vec![rendered];
            }
            if !buffer.is_empty() {
                let delim = self.tmp.splice_delimiter.clone().unwrap_or_default();
                match &mut buffer[0] {
                    Rendered::Str(s) => {
                        if pos > 0 {
                            *s = format!("{}{}", txt_esc.escape(&delim), s);
                        }
                    }
                    Rendered::Blob(b) => {
                        let sp = if pos > 0 { delim } else { String::new() };
                        self.blobs.get_mut(*b).splice_prefix = Some(sp);
                    }
                    Rendered::List(_) => {}
                }
            }
            objects.extend(buffer);
        }
        // print("OBJECTS="+objects);
        let rendered = queue::render_blobs(self, QueueId::Output, objects, "", false, None)?;
        result.push_str(&rendered.to_js_string());

        if !result.is_empty() && !self.tmp.suppress_decorations {
            let decs = self
                .citation
                .opt
                .get("layout_decorations")
                .and_then(queue::decorations_from_value)
                .unwrap_or_default();
            for params in decs {
                // The "normal" formats in some output modes expect
                // a superior nested decoration environment, and
                // so should produce no output here.
                if params.value == "normal" {
                    continue;
                }
                let item_author_only = last_item
                    .as_ref()
                    .map(|i| js::truthy_opt(i.get("author-only")))
                    .unwrap_or(false);
                if last_item.is_none() || !item_author_only {
                    result = formats::decorate(
                        self,
                        None,
                        &params.name,
                        &params.value,
                        Some(&result),
                        params.extra.as_deref(),
                    )?;
                }
            }
        }
        self.tmp.suppress_decorations = false;
        if result.is_empty() {
            if self.dev_ext("throw_on_empty") {
                return Err(EngineError::Csl("Citation would render no content".into()));
            } else {
                result = "[NO_PRINTED_FORM]".to_string();
            }
        }
        let _ = citation_id;
        Ok(result)
    }
}

/// Set `obj[key]` to `Some(n)` or remove it (`undefined`).
fn set_opt_int(obj: &mut Obj, key: &str, v: Option<i64>) {
    match v {
        Some(n) => {
            obj.insert(key.to_string(), Value::from(n));
        }
        None => {
            obj.remove(key);
        }
    }
}

/// JS `parseInt(x, 10)` of a property value; `None` is `NaN`.
fn note_of(props: &Obj) -> Option<i64> {
    props.get("noteIndex").and_then(js::parse_int_value)
}

impl State {
    /// `registry.refhash[id]`, as an error when upstream would read a
    /// property of `undefined`.
    pub(crate) fn item_data(&self, id: &str) -> CslResult<Value> {
        self.registry.refhash.get(id).cloned().ok_or_else(|| {
            type_error(&format!(
                "Cannot read properties of undefined (reading 'id') [item {id}]"
            ))
        })
    }

    /// The `[Item, item]` pairs of a citation, with `Item` read from the
    /// registry.
    fn sorted_input(&self, cid: CitId) -> CslResult<Vec<(Value, Obj)>> {
        self.registry
            .citationreg
            .get(cid)
            .sorted_items
            .iter()
            .map(|s| Ok((self.item_data(&s.item_id)?, s.item.clone())))
            .collect()
    }

    /// Store the (possibly updated) cite items back into the citation.
    fn store_sorted_items(&mut self, cid: CitId, input: Vec<(Value, Obj)>) {
        let rec = self.registry.citationreg.get_mut(cid);
        for (slot, (_, item)) in rec.sorted_items.iter_mut().zip(input) {
            slot.item = item;
        }
    }

    /// Sort a citation's `sortedItems` with `citation.srt.compareCompositeKeys`.
    pub(crate) fn sort_cite_items(&mut self, cid: CitId) -> CslResult<()> {
        let comparifier = self.citation.srt.clone().ok_or_else(|| {
            type_error("Cannot read properties of undefined (reading 'compareCompositeKeys')")
        })?;
        let mut items = std::mem::take(&mut self.registry.citationreg.get_mut(cid).sorted_items);
        let r = {
            let st: &State = self;
            sort_with(&mut items, |a, b| {
                comparifier.compare_composite_keys(
                    st,
                    &registry::sortkeys_of_item(&a.item),
                    &registry::sortkeys_of_item(&b.item),
                )
            })
        };
        self.registry.citationreg.get_mut(cid).sorted_items = items;
        r
    }

    /// `sortedItems[i][1].sortkeys = CSL.getSortKeys.call(this,
    /// sortedItems[i][0], "citation_sort")` for every cite of a citation.
    fn set_cite_sortkeys(&mut self, cid: CitId) -> CslResult<()> {
        let n = self.registry.citationreg.get(cid).sorted_items.len();
        for i in 0..n {
            let id = self.registry.citationreg.get(cid).sorted_items[i]
                .item_id
                .clone();
            let item = self.item_data(&id)?;
            let keys = self.get_sort_keys(&item, "citation_sort")?;
            self.registry.citationreg.get_mut(cid).sorted_items[i]
                .item
                .insert("sortkeys".into(), Value::Array(keys));
        }
        Ok(())
    }

    /// `CSL.Engine.prototype.process_CitationCluster(sortedItems, citation)`:
    /// the text of one citation, composing the author and the rest when its
    /// `mode` is `"composite"`.
    pub fn process_cluster_text(&mut self, cid: CitId) -> CslResult<String> {
        let mut input = self.sorted_input(cid)?;
        let r = self.process_cluster_text_with(&mut input, cid);
        self.store_sorted_items(cid, input);
        r
    }

    fn process_cluster_text_with(
        &mut self,
        input: &mut [(Value, Obj)],
        cid: CitId,
    ) -> CslResult<String> {
        let set_mode = |s: &mut State, mode: &str| {
            s.registry
                .citationreg
                .get_mut(cid)
                .properties
                .insert("mode".into(), Value::String(mode.to_string()));
        };
        if js::get_str(&self.registry.citationreg.get(cid).properties, "mode") == Some("composite")
        {
            set_mode(self, "author-only");
            let mut first_chunk = self.get_citation_cluster(input, Some(cid))?;
            set_mode(self, "suppress-author");
            let mut second_chunk = String::new();
            let infix = self
                .registry
                .citationreg
                .get(cid)
                .properties
                .get("infix")
                .filter(|v| js::truthy(v))
                .map(js::to_js_string);
            if let Some(infix) = infix {
                queue::append_simple(
                    self,
                    QueueId::Output,
                    infix.as_str(),
                    queue::FormatRef::None,
                )?;
                let r = queue::string(
                    self,
                    QueueId::Output,
                    &queue::queue_children(self, QueueId::Output),
                    StringParent::None,
                )?;
                // Had no idea this could return a single-element array! Go figure.
                second_chunk = match r {
                    Rendered::List(l) => l.iter().map(Rendered::to_js_string).collect::<String>(),
                    other => other.to_js_string(),
                };
            }
            let third_chunk = self.get_citation_cluster(input, Some(cid))?;
            set_mode(self, "composite");
            let mut second_present = true;
            let head = js::slice(&second_chunk, 0, Some(1));
            if !first_chunk.is_empty()
                && !second_chunk.is_empty()
                && (SWAPPING_PUNCTUATION.contains(&head.as_str())
                    || head == "\u{2019}"
                    || head == "'")
            {
                first_chunk.push_str(&second_chunk);
                second_present = false;
            }
            let chunks = [
                first_chunk,
                if second_present {
                    second_chunk
                } else {
                    String::new()
                },
                third_chunk,
            ];
            Ok(chunks
                .iter()
                .filter(|c| !c.is_empty())
                .cloned()
                .collect::<Vec<_>>()
                .join(" "))
        } else {
            self.get_citation_cluster(input, Some(cid))
        }
    }

    /// `makeCitationCluster(rawList)`: the text of a cluster of cites given
    /// as citation-item objects, without registering a citation.
    pub fn make_citation_cluster(&mut self, raw_list: &[Obj]) -> CslResult<String> {
        let mut input_list: Vec<(Value, Obj)> = Vec::new();
        for raw in raw_list {
            let mut item = raw.clone();
            let id = obj_id(&item);
            let item_data = retrieve_item(self, &id)?;
            // Code block is copied from processCitationCluster() above
            let item_obj = item_data.as_object().cloned().unwrap_or_default();
            locator_label_parse(self, &item_obj, &mut item);
            if js::get_truthy(&item, "locator") {
                static TRAILING_WS: LazyLock<Regex> = LazyLock::new(|| {
                    #[allow(clippy::expect_used)]
                    Regex::new(&format!("[{}]+$", js::WS)).expect("static regex")
                });
                let loc = js::to_js_string(item.get("locator").unwrap_or(&Value::Null));
                item.insert(
                    "locator".into(),
                    Value::String(TRAILING_WS.replace(&loc, "").into_owned()),
                );
            }
            input_list.push((item_data, item));
        }
        if self.dev_ext("consolidate_legal_items") {
            // remapSectionVariable(inputList): `Item.section` is rewritten in place.
            let mut pairs: Vec<(Value, Value)> = input_list
                .iter()
                .map(|(i, c)| (i.clone(), Value::Object(c.clone())))
                .collect();
            remap_section_variable(&mut pairs)?;
            for ((item_data, cite), (new_item, new_cite)) in input_list.iter_mut().zip(pairs) {
                if let Value::Object(c) = new_cite {
                    *cite = c;
                }
                if new_item != *item_data {
                    self.registry
                        .refhash
                        .insert(item_value_id(&new_item), new_item.clone());
                    *item_data = new_item;
                }
            }
        }
        if input_list.len() > 1 && !self.citation_sort.tokens.is_empty() {
            for pair in input_list.iter_mut() {
                let keys = self.get_sort_keys(&pair.0, "citation_sort")?;
                pair.1.insert("sortkeys".into(), Value::Array(keys));
            }
            let comparifier = self.citation.srt.clone().ok_or_else(|| {
                type_error("Cannot read properties of undefined (reading 'compareCompositeKeys')")
            })?;
            let st: &State = self;
            sort_with(&mut input_list, |a, b| {
                comparifier.compare_composite_keys(
                    st,
                    &registry::sortkeys_of_item(&a.1),
                    &registry::sortkeys_of_item(&b.1),
                )
            })?;
        }
        self.tmp.citation_errors = Vec::new();
        self.get_citation_cluster(&mut input_list, None)
    }

    /// `appendCitationCluster(citation)`: `processCitationCluster` with the
    /// citation after all the registered ones.
    pub fn append_citation_cluster(&mut self, citation: CitationInput) -> CslResult<ClusterResult> {
        let mut citations_pre = Vec::new();
        for cid in self.registry.citationreg.citation_by_index.clone() {
            let c = self.registry.citationreg.get(cid);
            citations_pre.push(CitationPos {
                citation_id: c.citation_id.clone(),
                note_index: c
                    .properties
                    .get("noteIndex")
                    .cloned()
                    .unwrap_or(Value::Null),
            });
        }
        // Drop the data segment to return a list of pos/string pairs.
        self.process_citation_cluster(citation, &citations_pre, &[], ClusterFlag::None)
    }

    /// `previewCitationCluster(citation, citationsPre, citationsPost, newMode)`:
    /// the text a hypothetical citation would have at this position, leaving
    /// the registry as it was found.
    pub fn preview_citation_cluster(
        &mut self,
        mut citation: CitationInput,
        citations_pre: &[CitationPos],
        citations_post: &[CitationPos],
        new_mode: &str,
    ) -> CslResult<String> {
        let old_mode = js::get_string(&self.opt, "mode").unwrap_or_else(|| "html".into());
        self.set_output_format(new_mode)?;
        // Avoids generating unwanted ibids, if the citationID already exists in document
        citation.citation_id = None;
        let ret = self.process_citation_cluster(
            citation,
            citations_pre,
            citations_post,
            ClusterFlag::Preview,
        );
        self.set_output_format(&old_mode)?;
        Ok(ret?.preview.unwrap_or_default())
    }
}

impl State {
    /// `processCitationCluster(citation, citationsPre, citationsPost, flag)`:
    /// register `citation` at its position in the document and render it,
    /// together with every other citation its registration changed. Returns
    /// `[return_data, ret]` of upstream.
    pub fn process_citation_cluster(
        &mut self,
        input: CitationInput,
        citations_pre: &[CitationPos],
        citations_post: &[CitationPos],
        flag: ClusterFlag,
    ) -> CslResult<ClusterResult> {
        self.tmp.loaded_item_ids = BTreeMap::new();

        self.tmp.citation_errors = Vec::new();
        self.registry.return_data = ReturnData {
            bibchange: false,
            citation_errors: Vec::new(),
        };

        let mut properties = input.properties.clone().unwrap_or_default();
        if input.properties.is_none() {
            properties.insert("noteIndex".into(), Value::from(0));
        }
        let cid = self.registry.citationreg.alloc(CitationRec {
            citation_id: input.citation_id.clone().unwrap_or_default(),
            citation_items: input.citation_items.clone(),
            properties,
            sorted_items: Vec::new(),
        });
        // make sure this citation has a unique ID, and register it in citationById.
        self.set_citation_id(cid, false);
        let citation_id = self.registry.citationreg.get(cid).citation_id.clone();

        let mut old_citation_list: Vec<CitId> = Vec::new();
        let mut old_item_list: Vec<String> = Vec::new();
        let mut old_ambigs: BTreeMap<String, AmbigConfig> = BTreeMap::new();
        if flag == ClusterFlag::Preview {
            // Simplify.

            // Take a slice of existing citations.
            old_citation_list = self.registry.citationreg.citation_by_index.clone();

            // Take a slice of current items, for later use with update.
            old_item_list = self.registry.reflist.clone();

            // Make a list of preview citation ref objects. Omit the current
            // citation, because it will not exist in registry if: (a) this is
            // a new citation; or (b) the calling application is assigning
            // new citationIDs for every transaction.
            let new_citation_list: Vec<&CitationPos> =
                citations_pre.iter().chain(citations_post.iter()).collect();

            // Make a full list of desired ids, for use in preview update,
            // and a hash list of same while we're at it.
            // First step through known citations, then step through
            // the items in the citation for preview.
            let mut new_item_ids: BTreeSet<String> = BTreeSet::new();
            for pos in new_citation_list {
                let c = self
                    .registry
                    .citationreg
                    .by_id(&pos.citation_id)
                    .ok_or_else(|| {
                        type_error("Cannot read properties of undefined (reading 'citationItems')")
                    })?;
                for ci in &self.registry.citationreg.get(c).citation_items {
                    new_item_ids.insert(obj_id(ci));
                }
            }
            for ci in &input.citation_items {
                new_item_ids.insert(obj_id(ci));
            }

            // Clone and save off disambigs of items that will be lost.
            for id in &old_item_list {
                if !new_item_ids.contains(id) {
                    let old_akey = self
                        .registry
                        .registry
                        .get(id)
                        .and_then(|t| t.ambig.clone())
                        .unwrap_or_else(|| "false".to_string());
                    if let Some(ids) = self.registry.ambigcites.get(&old_akey).cloned() {
                        for other in ids {
                            let d = self
                                .registry
                                .registry
                                .get(&other)
                                .and_then(|t| t.disambig)
                                .ok_or_else(|| {
                                    type_error(
                                        "Cannot read properties of undefined (reading 'disambig')",
                                    )
                                })?;
                            old_ambigs.insert(
                                other,
                                super::util_disambig::clone_ambig_config(self.ambig(d), None),
                            );
                        }
                    }
                }
            }
        }

        self.tmp.tainted_citation_ids = BTreeMap::new();
        let mut sorted_items: Vec<SortedItem> = Vec::new();

        // Styles that use note backreferencing with a by-cite
        // givenname disambiguation rule include the note number
        // in the cite for disambiguation purposes. Correct resolution
        // of disambiguate="true" conditions on first-reference cites
        // in certain editing scenarios (e.g. where a cite is moved across
        // notes) requires that disambiguation be rerun on cites
        // affected by the edit.
        let mut rerun_akeys: BTreeSet<String> = BTreeSet::new();

        // retrieve item data and compose items for use in rendering
        // attach pointer to item data to shared copy for good measure
        for ci in &input.citation_items {
            // Protect against caller-side overwrites to locator strings etc
            let mut item = ci.clone();
            let id = obj_id(&item);
            let item_data = retrieve_item(self, &id)?;
            if js::truthy_opt(item_data.get("id")) {
                let hid = item_data.get("id").map(js::to_js_string).unwrap_or_default();
                let lang = item_data
                    .get("language")
                    .filter(|l| js::truthy(l))
                    .map(js::to_js_string);
                super::util_transform::load_abbreviation(
                    self,
                    Some("default"),
                    "hereinafter",
                    &hid,
                    lang.as_deref(),
                );
            }
            let mut item_obj = item_data.as_object().cloned().unwrap_or_default();
            citation_item_input(self, &mut item_obj, &mut item)?;
            let new_item = Value::Object(item_obj);
            if new_item != item_data {
                // `remapSectionVariable` rewrote `Item.section` in place.
                self.registry.refhash.insert(id.clone(), new_item);
            }
            sorted_items.push(SortedItem { item_id: id, item });
        }

        // ZZZ sort stuff moved from here.

        // attach the sorted list to the citation item
        self.registry.citationreg.get_mut(cid).sorted_items = sorted_items;

        // build reconstituted citations list in current document order
        let strict_inputs = self.dev_ext("strict_inputs");
        let mut citation_by_index: Vec<CitId> = Vec::new();
        let mut citation_by_id: BTreeMap<String, CitId> = BTreeMap::new();
        for pre in citations_pre {
            if strict_inputs && citation_by_id.contains_key(&pre.citation_id) {
                return Err(EngineError::Csl(format!(
                    "Previously referenced citationID {} encountered in citationsPre",
                    pre.citation_id
                )));
            }
            let pc = self
                .registry
                .citationreg
                .by_id(&pre.citation_id)
                .ok_or_else(|| {
                    type_error("Cannot read properties of undefined (reading 'properties')")
                })?;
            self.registry
                .citationreg
                .get_mut(pc)
                .properties
                .insert("noteIndex".into(), pre.note_index.clone());
            citation_by_index.push(pc);
            citation_by_id.insert(pre.citation_id.clone(), pc);
        }
        if strict_inputs && citation_by_id.contains_key(&citation_id) {
            return Err(EngineError::Csl(format!(
                "Citation with previously referenced citationID {citation_id}"
            )));
        }
        citation_by_index.push(cid);
        citation_by_id.insert(citation_id.clone(), cid);
        for post in citations_post {
            if strict_inputs && citation_by_id.contains_key(&post.citation_id) {
                return Err(EngineError::Csl(format!(
                    "Previously referenced citationID {} encountered in citationsPost",
                    post.citation_id
                )));
            }
            let pc = self
                .registry
                .citationreg
                .by_id(&post.citation_id)
                .ok_or_else(|| {
                    type_error("Cannot read properties of undefined (reading 'properties')")
                })?;
            self.registry
                .citationreg
                .get_mut(pc)
                .properties
                .insert("noteIndex".into(), post.note_index.clone());
            citation_by_index.push(pc);
            citation_by_id.insert(post.citation_id.clone(), pc);
        }
        self.registry.citationreg.citation_by_index = citation_by_index.clone();
        self.registry.citationreg.citation_by_id = citation_by_id;

        //
        // The processor provides three facilities to support
        // updates following position reevaluation.
        //
        // (1) The updateItems() function reports tainted ItemIDs
        // to state.tmp.taintedItemIDs.
        //
        // (2) The processor memos the type of style referencing as
        // CSL.NONE, CSL.NUMERIC or CSL.POSITION in state.opt.update_mode.
        //
        // XXXX: NO LONGER
        // (3) For citations containing cites with backreference note numbers,
        // a string image of the rendered citation is held in
        // citation.properties.backref_citation, and a list of
        // ItemIDs to be used to update the backreference note numbers
        // is memoed at citation.properties.backref_index.  When such
        // citations change position, they can be updated with a
        // series of simple find and replace operations, without
        // need for rerendering.
        //

        //
        // Position evaluation!
        //
        // set positions in reconstituted list, noting taints
        let update_mode = self.opt.get("update_mode").and_then(Value::as_i64);
        let position_mode = update_mode == Some(load::POSITION);
        let mut by_item: BTreeMap<String, Vec<CitId>> = BTreeMap::new();
        let mut text_citations: Vec<CitId> = Vec::new();
        let mut note_citations: Vec<CitId> = Vec::new();
        let mut update_items: Vec<String> = Vec::new();
        for (i, &c) in citation_by_index.iter().enumerate() {
            self.registry
                .citationreg
                .get_mut(c)
                .properties
                .insert("index".into(), Value::from(i as i64));
            let ids: Vec<String> = self
                .registry
                .citationreg
                .get(c)
                .sorted_items
                .iter()
                .map(|s| obj_id(&s.item))
                .collect();
            for iid in ids {
                if !by_item.contains_key(&iid) {
                    by_item.insert(iid.clone(), Vec::new());
                    update_items.push(iid.clone());
                }
                if let Some(list) = by_item.get_mut(&iid) {
                    if !list.contains(&c) {
                        list.push(c);
                    }
                }
            }
            if position_mode {
                if js::truthy_opt(self.registry.citationreg.get(c).properties.get("noteIndex")) {
                    note_citations.push(c);
                } else {
                    self.registry
                        .citationreg
                        .get_mut(c)
                        .properties
                        .insert("noteIndex".into(), Value::from(0));
                    text_citations.push(c);
                }
            }
        }
        self.registry.citationreg.citations_by_item_id = Some(by_item);
        //
        // update bibliography items here
        //
        if flag != ClusterFlag::AssumeAllItemsRegistered {
            // true signals implicit updateItems (will not rerun sys.retrieveItem())
            self.update_items(&update_items, false, false, true)?;
        }

        let unsorted =
            |s: &State| js::truthy_opt(s.registry.citationreg.get(cid).properties.get("unsorted"));
        let n_sorted = self.registry.citationreg.get(cid).sorted_items.len();
        let citation_number_sort = js::truthy_opt(self.opt.get("citation_number_sort"));
        if !citation_number_sort && n_sorted > 1 && !self.citation_sort.tokens.is_empty() {
            self.set_cite_sortkeys(cid)?;

            /*
             * Grouped sort stuff (start)
             */
            if js::truthy_opt(self.opt.get("grouped_sort")) && !unsorted(self) {
                // Insert authorstring as key.
                for i in 0..n_sorted {
                    let id = self.registry.citationreg.get(cid).sorted_items[i]
                        .item_id
                        .clone();
                    let sortkeys = self.registry.citationreg.get(cid).sorted_items[i]
                        .item
                        .get("sortkeys")
                        .cloned()
                        .unwrap_or(Value::Array(Vec::new()));
                    // Run getAmbiguousCite() with the current disambig
                    // parameters, and pick up authorstring from the registry.
                    let mydisambig = self.registry.registry.get(&id).and_then(|t| t.disambig);
                    self.tmp.authorstring_request = true;
                    let item = self.item_data(&id)?;
                    self.get_ambiguous_cite(&item, mydisambig, false, None)?;
                    let authorstring = self.registry.authorstrings.get(&id).cloned();
                    self.tmp.authorstring_request = false;

                    let mut keys = vec![match authorstring {
                        Some(a) => Value::String(a),
                        None => Value::Null,
                    }];
                    keys.extend(sortkeys.as_array().cloned().unwrap_or_default());
                    self.registry.citationreg.get_mut(cid).sorted_items[i]
                        .item
                        .insert("sortkeys".into(), Value::Array(keys));
                }

                self.sort_cite_items(cid)?;
                // Replace authorstring key in items with same (authorstring) with the
                // keystring of first normal key. This forces grouped sorts,
                // as discussed here:
                // https://github.com/citation-style-language/schema/issues/40
                let mut lastauthor: Option<Value> = None;
                let mut thiskey: Option<Value> = None;
                let mut thisauthor: Option<Value> = None;
                for i in 0..n_sorted {
                    let item = &mut self.registry.citationreg.get_mut(cid).sorted_items[i].item;
                    let keys = item
                        .get("sortkeys")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    let k0 = keys.first().cloned().unwrap_or(Value::Null);
                    if lastauthor.as_ref() != Some(&k0) {
                        thisauthor = Some(k0);
                        thiskey = Some(keys.get(1).cloned().unwrap_or(Value::Null));
                    }
                    let tk = match &thiskey {
                        Some(Value::String(s)) => s.clone(),
                        Some(Value::Null) | None => "undefined".to_string(),
                        Some(other) => js::to_js_string(other),
                    };
                    let mut new_keys = keys;
                    if new_keys.is_empty() {
                        new_keys.push(Value::Null);
                    }
                    new_keys[0] = Value::String(format!("{tk}{i}"));
                    item.insert("sortkeys".into(), Value::Array(new_keys));
                    lastauthor = thisauthor.clone();
                }
            }
            /*
             * Grouped sort stuff (end)
             */

            if !unsorted(self) {
                self.sort_cite_items(cid)?;
            }
        }

        // evaluate parallels

        if js::truthy_opt(self.opt.get("parallel").and_then(|p| p.get("enable"))) {
            let mut input = self.sorted_input(cid)?;
            let r = util_parallel::start_citation(self, &mut input);
            self.store_sorted_items(cid, input);
            r?;
        }

        if position_mode {
            self.evaluate_positions(
                flag,
                cid,
                &citation_id,
                &[text_citations, note_citations],
                &mut rerun_akeys,
            )?;
        }
        if citation_number_sort
            && n_sorted > 1
            && !self.citation_sort.tokens.is_empty()
            && !unsorted(self)
        {
            self.set_cite_sortkeys(cid)?;
            self.sort_cite_items(cid)?;
        }
        let tainted_items: Vec<String> = self.tmp.tainted_item_ids.keys().cloned().collect();
        for key in tainted_items {
            // Current citation may be tainted but will not exist
            // during previewing.
            let citations = self
                .registry
                .citationreg
                .citations_by_item_id
                .as_ref()
                .and_then(|m| m.get(&key))
                .cloned();
            if let Some(citations) = citations {
                for c in citations {
                    let id = self.registry.citationreg.get(c).citation_id.clone();
                    self.tmp.tainted_citation_ids.insert(id, true);
                }
            }
        }

        let mut ret: Vec<(i64, String, String)> = Vec::new();
        let mut preview: Option<String> = None;
        if flag == ClusterFlag::Preview {
            // If previewing, return only a rendered string
            let rendered = self.process_cluster_text(cid).map_err(|e| {
                EngineError::Csl(format!("Error running CSL processor for preview: {e}"))
            })?;
            preview = Some(rendered);

            // Wind out anything related to new items added for the preview.
            // This means (1) names, (2) disambig state for affected items,
            // (3) keys registered in the ambigs pool arrays, and (4) registry
            // items.
            //

            // restore sliced citations
            self.registry.citationreg.citation_by_index = old_citation_list.clone();
            self.registry.citationreg.citation_by_id = BTreeMap::new();
            for c in &old_citation_list {
                let id = self.registry.citationreg.get(*c).citation_id.clone();
                self.registry.citationreg.citation_by_id.insert(id, *c);
            }

            self.update_items(&old_item_list, false, false, true)?;
            // Roll back disambig states
            for (key, config) in old_ambigs {
                let restored = self.alloc_ambig(config);
                if let Some(t) = self.registry.registry.get_mut(&key) {
                    t.disambig = Some(restored);
                } else {
                    return Err(type_error(
                        "Cannot set properties of undefined (setting 'disambig')",
                    ));
                }
            }
        } else {
            // Rerun cites that have moved across citations or had a change
            // in their number of subsequent references, so that disambiguate
            // and subsequent-reference-count conditions are applied
            // correctly in output.
            for rerun_akey in &rerun_akeys {
                super::disambig_cites::run(self, rerun_akey)?;
            }
            // Run taints only if not previewing
            //
            // Push taints to the return object
            //
            let tainted: Vec<String> = self.tmp.tainted_citation_ids.keys().cloned().collect();
            for key in tainted {
                if key == citation_id {
                    continue;
                }
                let mycitation = self.registry.citationreg.by_id(&key).ok_or_else(|| {
                    type_error("Cannot read properties of undefined (reading 'properties')")
                })?;
                if !js::truthy_opt(
                    self.registry
                        .citationreg
                        .get(mycitation)
                        .properties
                        .get("unsorted"),
                ) {
                    self.set_cite_sortkeys(mycitation)?;
                    self.sort_cite_items(mycitation)?;
                }
                // For error reporting
                let (index, note_index) = {
                    let p = &self.registry.citationreg.get(mycitation).properties;
                    (
                        p.get("index").cloned().unwrap_or(Value::Null),
                        p.get("noteIndex").cloned().unwrap_or(Value::Null),
                    )
                };
                self.tmp.citation_pos = index.clone();
                self.tmp.citation_note_index = note_index;
                let mid = self
                    .registry
                    .citationreg
                    .get(mycitation)
                    .citation_id
                    .clone();
                self.tmp.citation_id = Value::String(mid.clone());
                let text = self.process_cluster_text(mycitation)?;
                ret.push((js::parse_int_value(&index).unwrap_or(0), text, mid));
            }
            self.tmp.tainted_item_ids = BTreeMap::new();
            self.tmp.tainted_citation_ids = BTreeMap::new();

            // For error reporting again
            let (index, note_index) = {
                let p = &self.registry.citationreg.get(cid).properties;
                (
                    p.get("index").cloned().unwrap_or(Value::Null),
                    p.get("noteIndex").cloned().unwrap_or(Value::Null),
                )
            };
            self.tmp.citation_pos = index;
            self.tmp.citation_note_index = note_index;
            self.tmp.citation_id = Value::String(citation_id.clone());

            let text = self.process_cluster_text(cid)?;
            ret.push((citations_pre.len() as i64, text, citation_id.clone()));
            //
            // note for posterity: Rhino and Spidermonkey produce different
            // sort results for items with matching keys.  That discrepancy
            // turned up a subtle bug in the parallel detection code, trapped
            // at line 266, above, and in line 94 of util_parallel.js.
            //
            ret.sort_by_key(|r| r.0);
            //
            // In normal rendering, return is a list of two-part arrays, with the first element
            // a citation index number, and the second the text to be inserted.
            //
        }
        self.registry.return_data.citation_errors = self.tmp.citation_errors.clone();
        Ok(ClusterResult {
            return_data: self.registry.return_data.clone(),
            updates: ret,
            preview,
        })
    }

    /// The position evaluation loop of `processCitationCluster` (api_cite.js
    /// 268-577): sets `position`, `first-reference-note-number`,
    /// `first-container-reference-note-number` and `near-note` on every cite,
    /// noting the citations and items whose values changed.
    fn evaluate_positions(
        &mut self,
        flag: ClusterFlag,
        current: CitId,
        current_id: &str,
        passes: &[Vec<CitId>; 2],
        rerun_akeys: &mut BTreeSet<String>,
    ) -> CslResult<()> {
        let near_note_distance = self
            .citation
            .opt
            .get("near-note-distance")
            .and_then(Value::as_i64);
        // Declared (not initialised) in the JS loops, so they keep their
        // values from iteration to iteration.
        let mut citations_in_note: BTreeMap<i64, i64> = BTreeMap::new();
        let mut oldlastid: Option<Value> = None;
        let mut oldlastxloc: Option<Value> = None;
        let mut incitationid: Option<Value> = None;
        let mut incitationxloc: Option<Value> = None;
        for citations in passes {
            let mut first_ref: BTreeMap<String, Option<i64>> = BTreeMap::new();
            let mut last_ref: BTreeMap<String, Option<i64>> = BTreeMap::new();
            let mut first_container_ref: BTreeMap<String, Option<i64>> = BTreeMap::new();
            for j in 0..citations.len() {
                let onecitation = citations[j];
                // citations[j].properties.noteIndex = parseInt(...)
                {
                    let props = &mut self.registry.citationreg.get_mut(onecitation).properties;
                    if !js::truthy_opt(props.get("noteIndex")) {
                        props.insert("noteIndex".into(), Value::from(0));
                    }
                    let n = note_of(props);
                    match n {
                        Some(n) => props.insert("noteIndex".into(), Value::from(n)),
                        None => props.insert("noteIndex".into(), Value::Null),
                    };
                }
                let note_index = note_of(&self.registry.citationreg.get(onecitation).properties);
                if j > 0 && note_index.map(|n| n != 0).unwrap_or(false) {
                    let prev_note =
                        note_of(&self.registry.citationreg.get(citations[j - 1]).properties);
                    if prev_note
                        .zip(note_index)
                        .map(|(p, n)| p > n)
                        .unwrap_or(false)
                    {
                        citations_in_note = BTreeMap::new();
                        first_ref = BTreeMap::new();
                        last_ref = BTreeMap::new();
                        first_container_ref = BTreeMap::new();
                    }
                }
                let n_items = self
                    .registry
                    .citationreg
                    .get(onecitation)
                    .sorted_items
                    .len();
                for k in 0..n_items {
                    let parallel = self.registry.citationreg.get(onecitation).sorted_items[k]
                        .item
                        .get("parallel")
                        .cloned();
                    if js::truthy_opt(parallel.as_ref())
                        && parallel.as_ref().and_then(Value::as_str) != Some("first")
                    {
                        continue;
                    }
                    let key = note_index.unwrap_or(i64::MIN);
                    *citations_in_note.entry(key).or_insert(0) += 1;
                }
                // Set the following:
                //
                // (1) position as required (as per current Zotero)
                // (2) first-reference-note-number as required (on onecitation item)
                // (3) near-note as required (on onecitation item, according to
                //     state.opt["near-note-distance"] parameter)
                // (4) state.registry.citationreg.citationsByItemId.
                //
                // Any state changes caused by unsetting or resetting should
                // trigger a single entry for the citations in
                // state.tmp.taintedCitationIDs (can block on presence of
                // state.registry.citationreg.citationsByItemId).
                //
                for k in 0..n_items {
                    let sorted = self.registry.citationreg.get(onecitation).sorted_items[k].clone();
                    let item_data = self.item_data(&sorted.item_id)?;
                    // Okay ...
                    // We set up three IDs for use in position evaluation.
                    // item_id is the real Item.id
                    // first_id is the legislation_id or Item.id (so statutes backref to first in set, chapters to specific chapter)
                    // last_id is the legislation_id or container_id (so statute AND chapter distance is from any ref in set)
                    // (replaces myid)
                    let item_id = item_value_id(&item_data);
                    let legislation_id = item_data.get("legislation_id").filter(|v| js::truthy(v));
                    let first_id = legislation_id
                        .map(js::to_js_string)
                        .unwrap_or_else(|| item_id.clone());
                    let last_id = if let Some(l) = legislation_id {
                        js::to_js_string(l)
                    } else if let Some(c) = item_data.get("container_id").filter(|v| js::truthy(v))
                    {
                        js::to_js_string(c)
                    } else {
                        item_id.clone()
                    };
                    let myxloc = sorted.item.get("locator-extra").cloned();
                    let mylocator = sorted.item.get("locator").cloned();
                    let mylabel = sorted.item.get("label").cloned();
                    if k > 0 {
                        // incitationid is only reached in the else branch
                        // following "undefined" === typeof first_ref[myid]
                        // below
                        let prev_sorted =
                            &self.registry.citationreg.get(onecitation).sorted_items[k - 1];
                        let prev_item = self.item_data(&prev_sorted.item_id)?;
                        if let Some(l) = prev_item.get("legislation_id").filter(|v| js::truthy(v)) {
                            incitationid = Some(l.clone());
                        } else {
                            let items = &self.registry.citationreg.get(onecitation).sorted_items;
                            incitationid = items[k - 1].item.get("id").cloned();
                            incitationxloc = items[k - 1].item.get("locator-extra").cloned();
                            //if (onecitation.sortedItems[k-1][1].parallel === "last") {
                            let mut l = k as i64 - 2;
                            while l > -1 {
                                let it = &items[l as usize].item;
                                if it.get("parallel").and_then(Value::as_str) == Some("first") {
                                    incitationid = it.get("id").cloned();
                                    incitationxloc = it.get("locator-extra").cloned();
                                }
                                l -= 1;
                            }
                            //}
                        }
                    }
                    let onecitation_id = self
                        .registry
                        .citationreg
                        .get(onecitation)
                        .citation_id
                        .clone();
                    let one_note = note_of(&self.registry.citationreg.get(onecitation).properties);
                    let one_mode = js::get_str(
                        &self.registry.citationreg.get(onecitation).properties,
                        "mode",
                    )
                    .map(str::to_string);
                    // Don't touch item data of other cites when previewing
                    if flag == ClusterFlag::Preview && onecitation_id != current_id {
                        let cite_item_id = obj_id(&sorted.item);
                        if !first_ref.contains_key(&cite_item_id) {
                            first_ref.insert(first_id.clone(), one_note);
                            last_ref.insert(last_id.clone(), one_note);
                        } else {
                            last_ref.insert(last_id.clone(), one_note);
                        }
                        continue;
                    }
                    let old_position = sorted.item.get("position").cloned();
                    let old_frnn = sorted.item.get("first-reference-note-number").cloned();
                    let old_fcrnn = sorted
                        .item
                        .get("first-container-reference-note-number")
                        .cloned();
                    let old_near = sorted.item.get("near-note").cloned();
                    {
                        let item = &mut self.registry.citationreg.get_mut(onecitation).sorted_items
                            [k]
                            .item;
                        item.insert("first-reference-note-number".into(), Value::from(0));
                        item.insert(
                            "first-container-reference-note-number".into(),
                            Value::from(0),
                        );
                        item.insert("near-note".into(), Value::Bool(false));
                    }
                    let by_item_len = self
                        .registry
                        .citationreg
                        .citations_by_item_id
                        .as_ref()
                        .and_then(|m| m.get(&item_id))
                        .map(|l| l.len());
                    if let Some(new_count) = by_item_len {
                        if js::get_str(&self.opt, "xclass") == Some("note")
                            && js::truthy_opt(self.opt.get("has_disambiguate"))
                        {
                            let token = self.registry.registry.get_mut(&item_id).ok_or_else(|| {
                                type_error("Cannot read properties of undefined (reading 'citation-count')")
                            })?;
                            let old_count = token.citation_count;
                            token.citation_count = Some(new_count as i64);
                            let ambig = token.ambig.clone().unwrap_or_else(|| "false".to_string());
                            let rerun = match old_count {
                                Some(old) => (old < 2) != ((new_count as i64) < 2),
                                None => true,
                            };
                            if rerun {
                                let citations_of_item = self
                                    .registry
                                    .citationreg
                                    .citations_by_item_id
                                    .as_ref()
                                    .and_then(|m| m.get(&item_id))
                                    .cloned()
                                    .unwrap_or_default();
                                for c in citations_of_item {
                                    rerun_akeys.insert(ambig.clone());
                                    let id = self.registry.citationreg.get(c).citation_id.clone();
                                    self.tmp.tainted_citation_ids.insert(id, true);
                                }
                            }
                        }
                    }

                    // Okay, chill.
                    // The first test needs to be for presence of last_ref[last_id]. Everything
                    // after in subsequent evaluation depends on that.

                    // HOWEVER, despite starting with this test, we need to catch every member
                    // of the set, and set its first-container-reference-note-number to point at the
                    // first.

                    // ALSO, despite starting with this test, we need to set first-reference-note-number
                    // on every item.

                    // So ... we run an independent test on first_ref[first_id]], and let this ride.
                    let position: i64;
                    if !last_ref.contains_key(&last_id)
                        && one_mode.as_deref() != Some("author-only")
                    {
                        first_ref.insert(first_id.clone(), one_note);
                        last_ref.insert(last_id.clone(), one_note);
                        first_container_ref.insert(last_id.clone(), one_note);
                        position = load::POSITION_FIRST;
                    } else {
                        //
                        // backward-looking position evaluation happens here.
                        //
                        let mut ibidme = false;
                        let mut suprame = false;
                        let prev_citation: Option<CitId> =
                            if j > 0 { Some(citations[j - 1]) } else { None };
                        let this_citation = citations[j];
                        // XXX Ugly, but This is used in the second else-if branch condition below.
                        if let Some(prev) = prev_citation {
                            let mut old_last_id_offset = 1;
                            if js::get_str(&self.registry.citationreg.get(prev).properties, "mode")
                                == Some("author-only")
                                && j > 1
                            {
                                old_last_id_offset = 2;
                            }
                            let adjusted_offset = j - old_last_id_offset;
                            let adj = self.registry.citationreg.get(citations[adjusted_offset]);
                            if let Some(last) = adj.sorted_items.last() {
                                oldlastid = last.item.get("id").cloned();
                                oldlastxloc = last.item.get("locator-extra").cloned();
                            }
                            let prev_rec = self.registry.citationreg.get(prev);
                            if let Some(first) = prev_rec.sorted_items.first() {
                                // `prevCitation.sortedItems[0].slice(-1)[0]` is the cite item
                                if let Some(l) =
                                    first.item.get("legislation_id").filter(|v| js::truthy(v))
                                {
                                    oldlastid = Some(l.clone());
                                }
                            }
                        }
                        let prev_note = prev_citation
                            .and_then(|p| note_of(&self.registry.citationreg.get(p).properties));
                        let this_note =
                            note_of(&self.registry.citationreg.get(this_citation).properties);
                        let first_id_value = Value::String(first_id.clone());
                        if j > 0 && k == 0 && prev_note != this_note {
                            // Case 1: source in previous onecitation
                            // (1) Threshold conditions
                            //     (a) there must be a previous onecitation with one item
                            //     (b) this item must be the first in this onecitation
                            //     (c) the previous onecitation must contain a reference
                            //         to the same item ...
                            //     (d) the note numbers must be the same or consecutive.
                            // (this has some jiggery-pokery in it for parallels)
                            let mut useme = false;
                            let prev_rec = self
                                .registry
                                .citationreg
                                .get(prev_citation.ok_or_else(|| type_error("prevCitation"))?);
                            // XXX Can oldid be equated with oldlastid, I wonder ...
                            let first_prev = prev_rec.sorted_items.first().ok_or_else(|| {
                                type_error("Cannot read properties of undefined (reading '0')")
                            })?;
                            let prev_item_data = self.item_data(&first_prev.item_id)?;
                            let mut oldid = prev_item_data.get("id").cloned();
                            if let Some(l) = prev_item_data
                                .get("legislation_id")
                                .filter(|v| js::truthy(v))
                            {
                                oldid = Some(l.clone());
                            }
                            if loose_eq(oldid.as_ref(), Some(&first_id_value))
                                && prev_note
                                    .zip(this_note)
                                    .map(|(p, t)| p >= t - 1)
                                    .unwrap_or(false)
                            {
                                let prevxloc = first_prev.item.get("locator-extra");
                                let this_first = self
                                    .registry
                                    .citationreg
                                    .get(this_citation)
                                    .sorted_items
                                    .first()
                                    .ok_or_else(|| {
                                        type_error(
                                            "Cannot read properties of undefined (reading '1')",
                                        )
                                    })?;
                                let thisxloc = this_first.item.get("locator-extra");
                                let count_prev =
                                    prev_note.and_then(|p| citations_in_note.get(&p)).copied();
                                if (count_prev == Some(1) || prev_note == Some(0))
                                    && prevxloc == thisxloc
                                {
                                    useme = true;
                                }
                            }
                            if useme {
                                ibidme = true;
                            } else {
                                suprame = true;
                            }
                        } else if k > 0
                            && loose_eq(incitationid.as_ref(), Some(&first_id_value))
                            && loose_eq(incitationxloc.as_ref(), myxloc.as_ref())
                        {
                            // Case 2: immediately preceding source in this onecitation
                            // (1) Threshold conditions
                            //     (a) there must be an imediately preceding reference to  the
                            //         same item in this onecitation; and
                            ibidme = true;
                        } else if k == 0
                            && j > 0
                            && prev_note == this_note
                            && prev_citation
                                .map(|p| !self.registry.citationreg.get(p).sorted_items.is_empty())
                                .unwrap_or(false)
                            && loose_eq(oldlastid.as_ref(), Some(&first_id_value))
                            && loose_eq(oldlastxloc.as_ref(), myxloc.as_ref())
                        {
                            // ... in case there are separate citations in the same note ...
                            // Case 2 [take 2]: immediately preceding source in this onecitation
                            // (1) Threshold conditions
                            //     (a) there must be an imediately preceding reference to  the
                            //         same item in this onecitation; and
                            ibidme = true;
                        } else {
                            // everything else is definitely subsequent
                            suprame = true;
                        }
                        // conditions
                        let mut prev_locator = String::new();
                        let mut curr_locator = String::new();
                        if ibidme {
                            let prev_obj: Obj = if k > 0 {
                                self.registry.citationreg.get(onecitation).sorted_items[k - 1]
                                    .item
                                    .clone()
                            } else {
                                let p = citations[j - 1];
                                self.registry
                                    .citationreg
                                    .get(p)
                                    .sorted_items
                                    .first()
                                    .ok_or_else(|| {
                                        type_error(
                                            "Cannot read properties of undefined (reading '1')",
                                        )
                                    })?
                                    .item
                                    .clone()
                            };
                            if js::truthy_opt(prev_obj.get("locator")) {
                                let prev_label = prev_obj
                                    .get("label")
                                    .filter(|v| js::truthy(v))
                                    .map(js::to_js_string)
                                    .unwrap_or_default();
                                prev_locator = format!(
                                    "{}{}",
                                    js::to_js_string(
                                        prev_obj.get("locator").unwrap_or(&Value::Null)
                                    ),
                                    prev_label
                                );
                            }
                            if js::truthy_opt(mylocator.as_ref()) {
                                let curr_label = mylabel
                                    .as_ref()
                                    .filter(|v| js::truthy(v))
                                    .map(js::to_js_string)
                                    .unwrap_or_default();
                                curr_locator = format!(
                                    "{}{}",
                                    js::to_js_string(mylocator.as_ref().unwrap_or(&Value::Null)),
                                    curr_label
                                );
                            }
                        }
                        // triage
                        if ibidme && !prev_locator.is_empty() && curr_locator.is_empty() {
                            ibidme = false;
                            suprame = true;
                        }
                        let mut pos_value: Option<i64> = None;
                        if ibidme {
                            if prev_locator.is_empty() && !curr_locator.is_empty() {
                                //     (a) if the previous onecitation had no locator
                                //         and this onecitation has one, use ibid+pages
                                pos_value = Some(load::POSITION_IBID_WITH_LOCATOR);
                            } else if prev_locator.is_empty() && curr_locator.is_empty() {
                                //     (b) if the previous onecitation had no locator
                                //         and this onecitation also has none, use ibid
                                pos_value = Some(load::POSITION_IBID);
                            } else if !prev_locator.is_empty() && curr_locator == prev_locator {
                                //     (c) if the previous onecitation had a locator
                                //         (page number, etc.) and this onecitation has
                                //         a locator that is identical, use ibid
                                pos_value = Some(load::POSITION_IBID);
                            } else if !prev_locator.is_empty()
                                && !curr_locator.is_empty()
                                && curr_locator != prev_locator
                            {
                                //     (d) if the previous onecitation had a locator,
                                //         and this onecitation has one that differs,
                                //         use ibid+pages
                                pos_value = Some(load::POSITION_IBID_WITH_LOCATOR);
                            } else {
                                //     (e) if the previous onecitation had a locator
                                //         and this onecitation has none, use subsequent
                                //
                                //     ... and everything else would be subsequent also
                                ibidme = false; // just to be clear
                                suprame = true;
                            }
                        }
                        let mut position_now = pos_value;
                        if suprame {
                            position_now = Some(load::POSITION_CONTAINER_SUBSEQUENT);
                            if !first_ref.contains_key(&first_id) {
                                first_ref.insert(first_id.clone(), one_note);
                            } else {
                                position_now = Some(load::POSITION_SUBSEQUENT);
                            }
                        }
                        if suprame || ibidme {
                            if one_mode.as_deref() == Some("author-only") {
                                position_now = Some(load::POSITION_FIRST);
                            }
                            let fcr = first_container_ref.get(&last_id).copied().flatten();
                            if fcr != one_note {
                                let item = &mut self
                                    .registry
                                    .citationreg
                                    .get_mut(onecitation)
                                    .sorted_items[k]
                                    .item;
                                set_opt_int(item, "first-container-reference-note-number", fcr);
                                if let Some(t) = self.registry.registry.get_mut(&item_id) {
                                    t.first_container_reference_note_number = fcr;
                                }
                            }
                            let fr = first_ref.get(&first_id).copied().flatten();
                            if fr != one_note {
                                let item = &mut self
                                    .registry
                                    .citationreg
                                    .get_mut(onecitation)
                                    .sorted_items[k]
                                    .item;
                                set_opt_int(item, "first-reference-note-number", fr);
                                if let Some(t) = self.registry.registry.get_mut(&item_id) {
                                    // Try this instead?
                                    t.first_reference_note_number = fr;
                                }
                            }
                        }
                        position = position_now.unwrap_or(load::POSITION_SUBSEQUENT);
                    }
                    self.registry.citationreg.get_mut(onecitation).sorted_items[k]
                        .item
                        .insert("position".into(), Value::from(position));
                    if one_note.map(|n| n != 0).unwrap_or(false) {
                        let last = last_ref.get(&last_id).copied().flatten();
                        let note_distance = one_note.zip(last).map(|(a, b)| a - b);
                        if position != load::POSITION_FIRST
                            && note_distance
                                .zip(near_note_distance)
                                .map(|(d, n)| d <= n)
                                .unwrap_or(false)
                        {
                            self.registry.citationreg.get_mut(onecitation).sorted_items[k]
                                .item
                                .insert("near-note".into(), Value::Bool(true));
                        }
                        last_ref.insert(last_id.clone(), one_note);
                    } else if position != load::POSITION_FIRST {
                        self.registry.citationreg.get_mut(onecitation).sorted_items[k]
                            .item
                            .insert("near-note".into(), Value::Bool(true));
                    }
                    if onecitation_id != current_id {
                        let now = self.registry.citationreg.get(onecitation).sorted_items[k]
                            .item
                            .clone();
                        for (param, old) in [
                            ("position", &old_position),
                            ("first-reference-note-number", &old_frnn),
                            ("near-note", &old_near),
                        ] {
                            if now.get(param) != old.as_ref() {
                                if let Some(t) = self.registry.registry.get(&item_id) {
                                    if param == "first-reference-note-number" {
                                        rerun_akeys.insert(
                                            t.ambig.clone().unwrap_or_else(|| "false".into()),
                                        );
                                        self.tmp.tainted_item_ids.insert(item_id.clone(), true);
                                    }
                                }
                                self.tmp
                                    .tainted_citation_ids
                                    .insert(onecitation_id.clone(), true);
                            }
                        }
                    }
                    let _ = &old_fcrnn;
                    if self.fun.host_hooks.variable_wrapper {
                        let index = self
                            .registry
                            .citationreg
                            .get(onecitation)
                            .properties
                            .get("index")
                            .cloned();
                        let note = self
                            .registry
                            .citationreg
                            .get(onecitation)
                            .properties
                            .get("noteIndex")
                            .cloned();
                        let item = &mut self.registry.citationreg.get_mut(onecitation).sorted_items
                            [k]
                            .item;
                        match index {
                            Some(v) => item.insert("index".into(), v),
                            None => item.remove("index"),
                        };
                        match note {
                            Some(v) => item.insert("noteIndex".into(), v),
                            None => item.remove("noteIndex"),
                        };
                    }
                }
            }
        }
        let _ = current;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! Differential tests against citeproc-js 2.4.63. The citation-item input
    //! steps are checked against `tests/data/csl/units/locator.json`
    //! (generator `scripts/csl-units/locator.cjs`, shared with
    //! `util_static_locator`): ~1,000 items x locators x labels, in two
    //! locales and five option sets, with the locale terms citeproc-js read
    //! replayed. The position machinery and the rest of the cluster API are
    //! checked against `engine.json` (`scripts/csl-units/engine.cjs`) in
    //! `tests/citeproc_engine_units.rs`.
    use std::sync::Arc;

    use serde_json::json;

    use super::*;

    const REF: &str = include_str!("../../tests/data/csl/units/locator.json");

    fn obj(v: &Value) -> Obj {
        v.as_object().cloned().unwrap_or_default()
    }

    fn err_text(e: &EngineError) -> String {
        match e {
            EngineError::BadInput(m) => m.clone(),
            o => o.to_string(),
        }
    }

    #[test]
    fn citation_item_input_matches_citeproc_js() {
        let r: Value = serde_json::from_str(REF).expect("json");
        let mut n = 0;
        for c in r["citation_item_input"].as_array().expect("cases") {
            let e = &r["cii_engines"][c["engine"].as_str().unwrap_or("")];
            let mut st = State::default();
            st.opt = obj(&e["opt"]);
            super::super::test_support::install_locale(
                &mut st,
                super::super::test_support::logged_locale(&e["log"], None, None),
            );
            let (mut item_obj, mut item) = (obj(&c["Item"]), obj(&c["ci"]));
            let res = citation_item_input(&mut st, &mut item_obj, &mut item);
            n += 1;
            match (res, c.get("error")) {
                (Ok(()), None) => {
                    assert_eq!(Value::Object(item), c["item_out"], "item {c}");
                    assert_eq!(Value::Object(item_obj), c["Item_out"], "Item {c}");
                }
                (Err(e), Some(w)) => assert_eq!(w.as_str(), Some(err_text(&e).as_str()), "{c}"),
                (g, w) => panic!("{c}: {g:?} vs {w:?}"),
            }
        }
        assert!(n > 800);
    }

    // ---- the engine against citeproc-js: tests/data/csl/units/engine.json ----

    use super::super::{Citation, CitationItem, CitationRef, Engine, Sys};

    const ENGINE_REF: &str = include_str!("../../tests/data/csl/units/engine.json");

    /// citeproc-js generates a random `citationID` ("a" + base 32) for a
    /// citation without one; the reference calls them all `GEN`.
    fn gen(id: &str) -> String {
        let b = id.as_bytes();
        let generated = b.len() >= 7
            && b[0] == b'a'
            && b[1..]
                .iter()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'v').contains(c));
        if generated {
            "GEN".to_string()
        } else {
            id.to_string()
        }
    }

    /// What `scripts/csl-units/engine.cjs`'s `snapshot` records, from the state.
    fn snapshot(st: &State) -> Value {
        let reg = &st.registry;
        let keep = |o: &Obj, from: &str| o.get(from).cloned();
        let mut tokens = Obj::new();
        for (id, t) in &reg.registry {
            let mut o = Obj::new();
            o.insert("seq".into(), Value::from(t.seq));
            o.insert("offset".into(), Value::from(t.offset));
            if let Some(a) = &t.ambig {
                o.insert("ambig".into(), Value::String(a.clone()));
            }
            if let Some(k) = &t.sortkeys {
                o.insert("sortkeys".into(), registry::sortkeys_to_value(k));
            }
            if t.new_item {
                o.insert("newItem".into(), Value::Bool(true));
            }
            if let Some(d) = t.disambig {
                let d = st.ambig(d);
                let mut dd = Obj::new();
                dd.insert("names".into(), json!(d.names));
                dd.insert("givens".into(), json!(d.givens));
                if d.year_suffix != Value::Bool(false) {
                    dd.insert("year_suffix".into(), d.year_suffix.clone());
                }
                dd.insert("disambiguate".into(), d.disambiguate.clone());
                o.insert("disambig".into(), Value::Object(dd));
            }
            for (k, v) in [
                ("frnn", t.first_reference_note_number),
                ("fcrnn", t.first_container_reference_note_number),
                ("count", t.citation_count),
            ] {
                if let Some(n) = v {
                    o.insert(k.into(), Value::from(n));
                }
            }
            tokens.insert(id.clone(), Value::Object(o));
        }
        let mut citations = Vec::new();
        for cid in &reg.citationreg.citation_by_index {
            let c = reg.citationreg.get(*cid);
            let mut o = Obj::new();
            o.insert("id".into(), Value::String(gen(&c.citation_id)));
            for (k, from) in [("noteIndex", "noteIndex"), ("index", "index")] {
                if let Some(v) = keep(&c.properties, from) {
                    o.insert(k.into(), v);
                }
            }
            let mut sorted = Vec::new();
            for si in &c.sorted_items {
                let mut so = Obj::new();
                so.insert("id".into(), Value::String(si.item_id.clone()));
                so.insert(
                    "cid".into(),
                    si.item.get("id").cloned().unwrap_or(Value::Null),
                );
                for (k, from) in [
                    ("position", "position"),
                    ("frnn", "first-reference-note-number"),
                    ("fcrnn", "first-container-reference-note-number"),
                    ("near", "near-note"),
                    ("sortkeys", "sortkeys"),
                    ("locator", "locator"),
                    ("label", "label"),
                    ("xloc", "locator-extra"),
                ] {
                    if let Some(v) = keep(&si.item, from) {
                        so.insert(k.into(), v);
                    }
                }
                sorted.push(Value::Object(so));
            }
            o.insert("sorted".into(), Value::Array(sorted));
            citations.push(Value::Object(o));
        }
        let mut snap = Obj::new();
        snap.insert("reflist".into(), json!(reg.reflist));
        snap.insert("tokens".into(), Value::Object(tokens));
        snap.insert("citations".into(), Value::Array(citations));
        if let Some(by) = &reg.citationreg.citations_by_item_id {
            let mut m = Obj::new();
            for (k, list) in by {
                let ids: Vec<String> = list
                    .iter()
                    .map(|c| gen(&reg.citationreg.get(*c).citation_id))
                    .collect();
                m.insert(k.clone(), json!(ids));
            }
            snap.insert("byItem".into(), Value::Object(m));
        }
        let mut ids: Vec<String> = reg
            .citationreg
            .citation_by_id
            .keys()
            .map(|k| gen(k))
            .collect();
        ids.sort();
        snap.insert("citationById".into(), json!(ids));
        Value::Object(snap)
    }

    /// The first place two JSON values differ, as a path and the two values.
    fn first_diff(a: &Value, b: &Value, path: &str) -> Option<String> {
        match (a, b) {
            (Value::Object(x), Value::Object(y)) => {
                for k in x.keys().chain(y.keys()) {
                    let sub = format!("{path}.{k}");
                    match (x.get(k), y.get(k)) {
                        (Some(p), Some(q)) => {
                            if let Some(d) = first_diff(p, q, &sub) {
                                return Some(d);
                            }
                        }
                        (p, q) => return Some(format!("{sub}: port {p:?} citeproc-js {q:?}")),
                    }
                }
                None
            }
            (Value::Array(x), Value::Array(y)) => {
                for i in 0..x.len().max(y.len()) {
                    let sub = format!("{path}[{i}]");
                    match (x.get(i), y.get(i)) {
                        (Some(p), Some(q)) => {
                            if let Some(d) = first_diff(p, q, &sub) {
                                return Some(d);
                            }
                        }
                        (p, q) => return Some(format!("{sub}: port {p:?} citeproc-js {q:?}")),
                    }
                }
                None
            }
            _ if a == b => None,
            _ => Some(format!("{path}: port {a} citeproc-js {b}")),
        }
    }

    fn citation_refs(v: &Value) -> Vec<CitationRef> {
        v.as_array()
            .map(|a| {
                a.iter()
                    .map(|r| CitationRef::from_json(r).expect("ref"))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Replays every scenario of `engine.json` (97 scenarios, 908 steps:
    /// bibliography ordering, `makeCitationCluster`, and `processCitationCluster`
    /// over random documents with note and in-text styles, edits, deletions,
    /// previews and appends) and compares after each step the registry, the
    /// citations' cite positions and sort keys, and the call's result with the
    /// state citeproc-js holds.
    #[test]
    fn the_engine_matches_citeproc_js_scenario_by_scenario() {
        let r: Value = serde_json::from_str(ENGINE_REF).expect("engine.json");
        let locales = Arc::new(super::super::test_support::minimal_locales());
        let (mut steps, mut compared) = (0, 0);
        for sc in r["scenarios"].as_array().expect("scenarios") {
            let name = sc["name"].as_str().unwrap_or("?");
            let items: Vec<Value> = sc["items"].as_array().cloned().unwrap_or_default();
            let sys = Sys::new(&items, locales.clone()).expect("sys");
            let mut e =
                Engine::new(sys, sc["style"].as_str().unwrap_or(""), "en-US").expect("engine");
            if sc["result"].get("error").is_some() {
                continue;
            }
            for (n, (op, want)) in sc["ops"]
                .as_array()
                .expect("ops")
                .iter()
                .zip(sc["result"]["steps"].as_array().expect("steps"))
                .enumerate()
            {
                steps += 1;
                let at = format!("{name} step {n} ({})", op["op"]);
                let mut got = Obj::new();
                let outcome: Result<(), EngineError> = (|| {
                    match op["op"].as_str().unwrap_or("") {
                        "update" => {
                            let ids: Vec<String> = op["ids"]
                                .as_array()
                                .map(|a| a.iter().map(|i| js::to_js_string(i)).collect())
                                .unwrap_or_default();
                            e.update_items(&ids, op["nosort"].as_bool().unwrap_or(false))?;
                        }
                        "process" | "append" | "preview" => {
                            let c = Citation::from_json(&op["citation"]).or_else(|_| {
                                // a citation without a citationID or properties
                                let mut v = op["citation"].clone();
                                if v.get("citationID").is_none() {
                                    v["citationID"] = Value::String(String::new());
                                }
                                Citation::from_json(&v)
                            })?;
                            let c = if op["citation"].get("properties").is_none() {
                                Citation {
                                    properties: None,
                                    ..c
                                }
                            } else {
                                c
                            };
                            let (pre, post) =
                                (citation_refs(&op["pre"]), citation_refs(&op["post"]));
                            match op["op"].as_str().unwrap_or("") {
                                "process" => {
                                    let updates = e.process_citation_cluster(&c, &pre, &post)?;
                                    got.insert(
                                        "ret".into(),
                                        json!(updates
                                            .iter()
                                            .map(|u| json!([u.index, u.text, gen(&u.citation_id)]))
                                            .collect::<Vec<_>>()),
                                    );
                                }
                                "append" => {
                                    let updates = e.append_citation_cluster(&c)?;
                                    got.insert(
                                        "ret".into(),
                                        json!(updates
                                            .iter()
                                            .map(|u| json!([u.index, u.text, gen(&u.citation_id)]))
                                            .collect::<Vec<_>>()),
                                    );
                                }
                                _ => {
                                    let text = e.preview_citation_cluster(
                                        &c,
                                        &pre,
                                        &post,
                                        crate::citeproc::OutputFormat::Html,
                                    )?;
                                    got.insert("text".into(), Value::String(text));
                                }
                            }
                        }
                        "make" => {
                            let items: Vec<CitationItem> = op["items"]
                                .as_array()
                                .map(|a| {
                                    a.iter()
                                        .map(|i| CitationItem::from_json(i).expect("item"))
                                        .collect()
                                })
                                .unwrap_or_default();
                            got.insert(
                                "text".into(),
                                Value::String(e.make_citation_cluster(&items)?),
                            );
                        }
                        "bib" if want.get("bib") == Some(&Value::Bool(false)) => {
                            // `makeBibliography()` returns `false` for a style without a
                            // bibliography; the Engine reports it as the TypeError the runner hits.
                            assert!(e.make_bibliography(op.get("section")).is_err(), "{at}");
                        }
                        "bib" => {
                            let b = e.make_bibliography(op.get("section"))?;
                            got.insert(
                                "bib".into(),
                                json!({"params": b.params, "entries": b.entries}),
                            );
                        }
                        "replace" => {
                            e.replace_items(&op["items"].as_array().cloned().unwrap_or_default())?;
                        }
                        other => panic!("{at}: unknown op {other}"),
                    }
                    Ok(())
                })();
                match (&outcome, want.get("error")) {
                    (Ok(()), None) => {}
                    (Err(err), Some(msg)) => {
                        // Both threw. JS TypeError texts are not reproduced; CSL.error texts are.
                        let m = msg.as_str().unwrap_or("");
                        if let Some(rest) = m.strip_prefix("citeproc-js error: ") {
                            assert_eq!(
                                err.to_string(),
                                format!("citeproc-js error: {rest}"),
                                "{at}"
                            );
                        }
                        // state after an exception is not compared
                        break;
                    }
                    (Ok(()), Some(msg)) => {
                        panic!("{at}: citeproc-js threw {msg} and the port did not")
                    }
                    (Err(err), None) => {
                        panic!("{at}: the port failed ({err}) and citeproc-js did not")
                    }
                }
                for key in ["ret", "text", "bib"] {
                    let w = want.get(key).cloned();
                    // citeproc-js `bib: false` has no counterpart (the port errors); both are checked above.
                    if let Some(w) = w.filter(|w| *w != Value::Bool(false)) {
                        let g = got.get(key).cloned().unwrap_or(Value::Null);
                        if let Some(d) = first_diff(&g, &w, key) {
                            panic!("{at}: {d}");
                        }
                    }
                }
                let snap = snapshot(e.state());
                if let Some(d) = first_diff(&snap, &want["snap"], "snap") {
                    panic!("{at}: {d}");
                }
                compared += 1;
            }
        }
        assert!(steps > 800, "{steps} steps");
        println!("engine.json: {steps} steps, {compared} compared with citeproc-js");
    }

    #[test]
    fn loose_equality_follows_javascript() {
        assert!(loose_eq(None, None));
        assert!(!loose_eq(Some(&json!("a")), None));
        assert!(loose_eq(Some(&json!("5")), Some(&json!(5))));
        assert!(!loose_eq(Some(&json!("a")), Some(&json!("b"))));
        assert!(loose_eq(Some(&json!("a")), Some(&json!("a"))));
    }

    #[test]
    fn the_splice_delimiter_follows_the_collapse_options() {
        let mut s = State::default();
        s.citation
            .opt
            .insert("layout_delimiter".into(), json!("; "));
        // no cite-group or after-collapse delimiter: keep the layout delimiter
        s.tmp.splice_delimiter = Some("; ".into());
        assert_eq!(s.get_splice_delimiter(false, false, 1), Some("; ".into()));
        // in-text styles that collapsed use ", "
        s.opt.insert("xclass".into(), json!("in-text"));
        s.tmp.have_collapsed = true;
        assert_eq!(s.get_splice_delimiter(false, false, 1), Some(", ".into()));
        s.tmp.have_collapsed = false;
        // after-collapse-delimiter after a cite with a locator
        s.citation
            .opt
            .insert("after-collapse-delimiter".into(), json!(" | "));
        assert_eq!(s.get_splice_delimiter(true, false, 1), Some(" | ".into()));
        s.citation
            .opt
            .insert("collapse".into(), json!("year-suffix"));
        assert_eq!(s.get_splice_delimiter(false, false, 1), Some("; ".into()));
    }
}
