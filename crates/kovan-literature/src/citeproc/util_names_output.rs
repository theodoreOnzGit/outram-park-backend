// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_names_output.js
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


//! Port of `src/util_names_output.js`: `CSL.NameOutput`, the object that
//! renders the names of one `cs:names` element, with the methods defined in
//! the other `util_names_*.js` files (each in its own Rust file as an
//! `impl NameOutput` block).
//!
//! # Shape
//!
//! `state.nameOutput` is [`State::name_output`]. Its methods take the
//! engine state as a parameter (`NameOutput::method(&mut self, st: &mut
//! State, ..)`); the entry points the `cs:names` closures use
//! ([`State::name_output_init`], [`State::name_output_reinit`],
//! [`State::name_output_output_names`]) take the object out of the state while
//! it runs, so `self` and `st` do not alias.
//!
//! JS keeps lists of names that later *become* blobs (`this.freeters[v]` is a
//! list of name objects until `renderAllNames` replaces it with one blob). The
//! two phases are separate fields here: `freeters`/`persons`/`institutions`
//! hold the names, `freeter_blobs`/`person_blobs`/`institution_blobs` the
//! rendered blobs (`None` is JS `false` or the empty array).
//!
//! Names are kept as [`Value`]s: an object, or `false` for a name a
//! nickname abbreviation suppressed (`_checkNickname`).
//!
//! `getName` and `_normalizeNameInput` need only `this.Item` and the state's
//! options; they read them through [`NameInputCtx`] (`State::name_input_ctx`)
//! so the registry can call them while the object is taken out.

use std::collections::BTreeMap;

use serde_json::Value;

use super::js::{self, Obj};
use super::load::update_group_context_condition;
use super::obj_blob::{Blob, BlobChild, BlobContent, BlobId};
use super::obj_token::Token;
use super::queue::{self, AppendArg, FormatRef, QueueId, Rendered, StringParent};
use super::state::State;
use super::util_names::get_raw_name;
use super::util_names_render::NameInputCtx;
use super::{CslResult, EngineError};

/// `state.tmp.name_node`: `{children, top, string}`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NameNode {
    /// `children`: the blobs of the names rendered by the current `cs:names`
    /// (`None` is a pushed `false`).
    pub children: Vec<Option<BlobId>>,
    /// `top`: the blob of the whole names output.
    pub top: Option<BlobId>,
    /// `string`: the raw names, comma-joined.
    pub string: Option<String>,
}

/// A `{single, multiple}` pair of blobs (`name.and`, `institution.and`,
/// `name.ellipsis`, `["et-al"]`, `["with"]`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BlobPair {
    /// `single`.
    pub single: Option<BlobId>,
    /// `multiple`.
    pub multiple: Option<BlobId>,
}

/// `this.etal_spec[k]`: `{freeters, institutions, persons: []}`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EtalSpec {
    /// `freeters`: 0 none, 1 et-al, 2 ellipsis (`et-al-use-last`).
    pub freeters: i64,
    /// `institutions`.
    pub institutions: i64,
    /// `persons[j]`.
    pub persons: Vec<i64>,
}

/// `this.institutionpart`: the `cs:institution-part` tokens by name.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InstitutionParts {
    /// `["long"]`.
    pub long: Option<Token>,
    /// `["long-with-short"]`.
    pub long_with_short: Option<Token>,
    /// `["short"]`.
    pub short: Option<Token>,
}

/// `this.label[variable]`: `{before, after}` label tokens.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LabelPair {
    /// `before`.
    pub before: Option<Token>,
    /// `after`.
    pub after: Option<Token>,
}

/// `this.etal_style`: the cs:et-al token, or the format name `"empty"`.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum EtalStyle {
    /// `"empty"`.
    #[default]
    Empty,
    /// The `cs:et-al` token.
    Token(Token),
}

impl EtalStyle {
    /// As the format argument of `append`.
    pub fn format_ref(&self) -> FormatRef {
        match self {
            EtalStyle::Empty => FormatRef::Name("empty".to_string()),
            EtalStyle::Token(t) => FormatRef::Token(t.clone()),
        }
    }
}

/// `CSL.NameOutput`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NameOutput {
    /// `this.Item`.
    pub item: Value,
    /// `this.item` (the citation item; `null` when `undefined`).
    pub cite_item: Value,
    /// `this.nameset_base`.
    pub nameset_base: i64,
    /// `this.nameset_offset` (`undefined` before the first `init`: 0).
    pub nameset_offset: i64,
    /// `this.etal_spec`: the specs, and the keys (variable names and
    /// positions) pointing at them (two keys may share one spec).
    pub etal_specs: Vec<EtalSpec>,
    pub etal_spec_keys: BTreeMap<String, usize>,
    /// `this._first_creator_variable`.
    pub first_creator_variable: Option<String>,
    /// `this._please_chop` (`false` = `None`, else a variable name).
    pub please_chop: Option<String>,
    /// `this.requireMatch`.
    pub require_match: bool,
    /// `this.names`: the `cs:names` token being rendered.
    pub names: Token,
    /// `this.variables`.
    pub variables: Vec<String>,
    /// `this["et-al"]` (`undefined` = `None`).
    pub et_al: Option<BlobPair>,
    /// `this["with"]`.
    pub with: Option<BlobPair>,
    /// `this.name`: the `cs:name` token.
    pub name: Option<Token>,
    /// `this.name.and` (`{}` = `None`) and `this.name.ellipsis`.
    pub name_and: Option<BlobPair>,
    pub name_ellipsis: Option<BlobPair>,
    /// `this.institution`: the `cs:institution` token, and its `.and`.
    pub institution: Option<Token>,
    pub institution_and: Option<BlobPair>,
    /// `this.institutionpart`.
    pub institutionpart: InstitutionParts,
    /// `this.labelVariable`.
    pub label_variable: Option<String>,
    /// `this.family` / `this.given`: the `cs:name-part` tokens.
    pub family: Option<Token>,
    pub given: Option<Token>,
    /// `this.family_decor` / `this.given_decor` (`false` = `None`).
    pub family_decor: Option<Token>,
    pub given_decor: Option<Token>,
    /// `this.label`.
    pub label: BTreeMap<String, LabelPair>,
    /// `this.etal_style`, `etal_term`, `etal_prefix_single`,
    /// `etal_prefix_multiple`, `etal_suffix` (set by the `cs:names` closure).
    pub etal_style: EtalStyle,
    pub etal_term: Option<String>,
    pub etal_prefix_single: String,
    pub etal_prefix_multiple: String,
    pub etal_suffix: String,
    /// `this.etal_min`, `etal_use_first`, `etal_use_last` (`None` is JS
    /// `undefined`).
    pub etal_min: Option<Value>,
    pub etal_use_first: Option<Value>,
    pub etal_use_last: Option<Value>,
    /// `this.variable_offset`.
    pub variable_offset: BTreeMap<String, i64>,
    /// `this.varnames`.
    pub varnames: Vec<String>,
    /// `this.freeters`, `persons`, `institutions` (names) and their counts.
    pub freeters: BTreeMap<String, Vec<Value>>,
    pub persons: BTreeMap<String, Vec<Vec<Value>>>,
    pub institutions: BTreeMap<String, Vec<Value>>,
    pub freeters_count: BTreeMap<String, i64>,
    pub persons_count: BTreeMap<String, Vec<i64>>,
    pub institutions_count: BTreeMap<String, i64>,
    /// The keys of `freeters`/`persons`/`institutions` in insertion order
    /// (JS `for (v in ...)` order).
    pub keys: Vec<String>,
    /// The same three after `renderAllNames` (blobs; `None` is `false`/`[]`).
    pub freeter_blobs: BTreeMap<String, Option<BlobId>>,
    pub person_blobs: BTreeMap<String, Vec<Option<BlobId>>>,
    pub institution_blobs: BTreeMap<String, Vec<Option<BlobId>>>,
    /// `this.names_count`.
    pub names_count: i64,
    /// `this.common_term` (`false` = `None`).
    pub common_term: Option<String>,
}

impl NameOutput {
    /// `new CSL.NameOutput(state, Item, item)` (the part that is not
    /// `state`); use [`State::new_name_output`] to also set the state's
    /// input context.
    pub fn new(item: &Value, cite_item: &Value) -> NameOutput {
        NameOutput {
            item: item.clone(),
            cite_item: cite_item.clone(),
            ..NameOutput::default()
        }
    }

    /// The `cs:name` token (`this.name`); a TypeError where JS would read a
    /// property of `undefined`.
    pub(super) fn name_token(&self) -> CslResult<&Token> {
        self.name.as_ref().ok_or_else(|| {
            super::load::type_error("Cannot read properties of undefined (reading 'strings')")
        })
    }

    /// `this.state.inheritOpt(this.name, attrname, parentname, default)`.
    pub(super) fn inherit_name_opt(
        &self,
        st: &State,
        attrname: &str,
        parentname: Option<&str>,
        default_value: Option<Value>,
    ) -> CslResult<Option<Value>> {
        Ok(st.inherit_opt(self.name_token()?, attrname, parentname, default_value))
    }

    /// `this.etal_spec[key]` (a variable name or a position).
    pub(super) fn etal_spec(&self, key: &str) -> Option<&EtalSpec> {
        self.etal_spec_keys
            .get(key)
            .and_then(|i| self.etal_specs.get(*i))
    }

    /// Mutable `this.etal_spec[key]`.
    pub(super) fn etal_spec_mut(&mut self, key: &str) -> Option<&mut EtalSpec> {
        let i = *self.etal_spec_keys.get(key)?;
        self.etal_specs.get_mut(i)
    }
}

impl State {
    /// `state.nameOutput = new CSL.NameOutput(state, Item, item)`
    /// (api_cite.js:1488, node_alternative.js): a fresh renderer for the cite,
    /// with the input context of its item.
    pub fn new_name_output(&mut self, item: &Value, cite_item: &Value) {
        self.name_input_ctx = NameInputCtx::from_state_item(self, item);
        self.name_output = NameOutput::new(item, cite_item);
    }

    /// `state.nameOutput.init(names)`.
    pub fn name_output_init(&mut self, names: &Token) -> CslResult<()> {
        let mut no = std::mem::take(&mut self.name_output);
        let r = no.init(self, names);
        self.name_output = no;
        r
    }

    /// `state.nameOutput.reinit(names, labelVariable)`.
    pub fn name_output_reinit(
        &mut self,
        names: &Token,
        label_variable: Option<&str>,
    ) -> CslResult<()> {
        let mut no = std::mem::take(&mut self.name_output);
        let r = no.reinit(self, names, label_variable);
        self.name_output = no;
        r
    }

    /// `state.nameOutput.outputNames()`.
    pub fn name_output_output_names(&mut self) -> CslResult<()> {
        let mut no = std::mem::take(&mut self.name_output);
        let r = no.output_names(self);
        self.name_output = no;
        r
    }
}

// ---------------------------------------------------------------------------
// Small helpers shared by the util_names_*.rs files
// ---------------------------------------------------------------------------

/// JS `ToNumber` of an optional value for relational comparisons:
/// `undefined`/`null`-as-NaN → NaN (this port stores a parse failure as
/// `null`), booleans 0/1, numbers as they are, strings parsed.
pub(super) fn js_num(v: Option<&Value>) -> f64 {
    match v {
        None | Some(Value::Null) => f64::NAN,
        Some(Value::Bool(b)) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        Some(Value::Number(n)) => n.as_f64().unwrap_or(f64::NAN),
        Some(Value::String(s)) => {
            let t = js::trim(s);
            if t.is_empty() {
                0.0
            } else {
                t.parse::<f64>().unwrap_or(f64::NAN)
            }
        }
        Some(_) => f64::NAN,
    }
}

/// `list.slice(0, n)` with JS's `ToIntegerOrInfinity` on `n` (NaN is 0,
/// negative counts from the end).
pub(super) fn slice_head<T: Clone>(list: &[T], n: f64) -> Vec<T> {
    let len = list.len() as f64;
    let n = if n.is_nan() { 0.0 } else { n.trunc() };
    let end = if n < 0.0 { (len + n).max(0.0) } else { n.min(len) };
    list[..end as usize].to_vec()
}

/// Whether `value && value.length` is truthy for an `Item[variable]`-like
/// value (a non-empty array or string).
pub(super) fn has_length(v: Option<&Value>) -> bool {
    match v {
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::String(s)) => !s.is_empty(),
        _ => false,
    }
}

/// `state.tmp.value = state.tmp.value.concat(Item[variable])`.
fn concat_into(dest: &mut Vec<Value>, v: &Value) {
    match v {
        Value::Array(a) => dest.extend(a.iter().cloned()),
        other => dest.push(other.clone()),
    }
}

/// `state[area].opt[key]` has a truthy `.length` (`opt.collapse` and
/// `opt.cite_group_delimiter`: arrays or strings).
fn opt_length_truthy(st: &State, key: &str) -> bool {
    has_length(st.area_ref(&st.tmp.area).opt.get(key))
}

/// `state.output.append(arg, tok, notSerious)` with the trailing flags off.
pub(super) fn q_append(
    st: &mut State,
    arg: AppendArg,
    tok: FormatRef,
    not_serious: bool,
) -> CslResult<bool> {
    queue::append(st, QueueId::Output, arg, tok, not_serious, false, false)
}

/// `state.output.append(str, tok, notSerious)` for a string that may be
/// `undefined` (`None`).
pub(super) fn q_append_str(
    st: &mut State,
    s: Option<&str>,
    tok: FormatRef,
    not_serious: bool,
) -> CslResult<bool> {
    let arg = match s {
        Some(s) => AppendArg::Text(s.to_string()),
        None => AppendArg::Undefined,
    };
    q_append(st, arg, tok, not_serious)
}

/// `state.output.append(blob, tok, notSerious)` for a value that is a blob or
/// `false` (`None`). `append(false, "literal")` pushes `false`, which
/// `Blob.prototype.push` drops; `append(false, token)` pushes an empty blob
/// carrying the token's strings and decorations.
pub(super) fn q_append_blob(
    st: &mut State,
    blob: Option<BlobId>,
    tok: FormatRef,
    not_serious: bool,
) -> CslResult<bool> {
    if let Some(id) = blob {
        return q_append(st, AppendArg::Blob(id), tok, not_serious);
    }
    let mut tok = tok;
    if st.tmp.doing_macro_with_date && !not_serious {
        match &tok {
            FormatRef::Name(n) if n == "macro-with-date" => tok = FormatRef::Name("empty".into()),
            _ => return Ok(false),
        }
    }
    if !not_serious && st.tmp.element_trace.value().map(String::as_str) == Some("suppress-me") {
        return Ok(false);
    }
    let token: Token = match tok {
        FormatRef::Name(n) if n == "literal" => return Ok(true),
        FormatRef::None => st.output.empty.clone(),
        FormatRef::Name(n) if n.is_empty() => st.output.empty.clone(),
        FormatRef::Name(n) => queue::get_token(st, QueueId::Output, &n).ok_or_else(|| {
            EngineError::Csl(format!("CSL processor error: unknown format token name: {n}"))
        })?,
        FormatRef::Token(t) => t,
    };
    let mut token = token;
    if !token.strings.contains_key("delimiter") {
        token.set_string("delimiter", "");
    }
    let curr = queue::current(st, QueueId::Output).ok_or_else(|| {
        EngineError::Csl("TypeError: Cannot read properties of undefined (reading 'push')".into())
    })?;
    let id = st.blobs.add(Blob::new(None, Some(&token), None));
    st.blobs.push(curr, id)?;
    Ok(true)
}

/// `state.output.pop()` where the result is used as a blob: `None` for JS
/// `undefined`.
pub(super) fn q_pop_blob(st: &mut State) -> CslResult<Option<BlobId>> {
    match queue::pop(st, QueueId::Output)? {
        Some(BlobChild::Blob(id)) => Ok(Some(id)),
        Some(BlobChild::Str(_)) => Err(EngineError::Csl(
            "TypeError: popped a string where a blob was expected".into(),
        )),
        None => Ok(None),
    }
}

/// The blob popped just now, which JS then dereferences (`.strings`): a
/// TypeError when there is none.
pub(super) fn q_pop_blob_required(st: &mut State) -> CslResult<BlobId> {
    q_pop_blob(st)?.ok_or_else(|| {
        EngineError::Csl("TypeError: Cannot read properties of undefined (reading 'strings')".into())
    })
}

/// `state.output.openLevel(tok)`.
pub(super) fn q_open_level(st: &mut State, tok: FormatRef) -> CslResult<()> {
    queue::open_level(st, QueueId::Output, tok)
}

/// `state.output.closeLevel()`.
pub(super) fn q_close_level(st: &mut State) -> CslResult<()> {
    queue::close_level(st, QueueId::Output, None)
}

/// `state.output.closeLevel(name)`.
pub(super) fn q_close_level_named(st: &mut State, name: &str) -> CslResult<()> {
    queue::close_level(st, QueueId::Output, Some(name))
}

/// `blob.strings[key] += suffix`-style read of a blob string.
pub(super) fn blob_string(st: &State, id: BlobId, key: &str) -> String {
    st.blobs.get(id).string(key)
}

/// Deep copy of a blob tree, as `JSON.parse(JSON.stringify(blob))` does
/// (the copy shares nothing with the original).
pub(super) fn clone_blob_deep(st: &mut State, id: BlobId) -> BlobId {
    let mut b = st.blobs.get(id).clone();
    if let BlobContent::List(l) = &b.blobs {
        let kids: Vec<BlobChild> = l
            .clone()
            .into_iter()
            .map(|c| match c {
                BlobChild::Blob(k) => BlobChild::Blob(clone_blob_deep(st, k)),
                other => other,
            })
            .collect();
        b.blobs = BlobContent::List(kids);
    }
    st.blobs.add(b)
}

/// Whether a blob counts as empty to `_purgeEmptyBlobs`
/// (`!blobs[i].blobs.length`: no text, no children).
pub(super) fn blob_is_empty(st: &State, id: BlobId) -> bool {
    match &st.blobs.get(id).blobs {
        BlobContent::Text(t) => t.is_empty(),
        BlobContent::List(l) => l.is_empty(),
    }
}

/// A name object from a list entry; a TypeError for `false` (JS strict mode:
/// assigning a property to a primitive throws).
pub(super) fn name_obj_mut(v: &mut Value) -> CslResult<&mut Obj> {
    match v {
        Value::Object(o) => Ok(o),
        other => Err(EngineError::BadInput(format!(
            "Cannot create property 'family' on {} '{}'",
            if other.is_boolean() { "boolean" } else { "string" },
            js::to_js_string(other)
        ))),
    }
}

impl NameOutput {
    // -----------------------------------------------------------------------
    // CSL.NameOutput.prototype.init / reinit
    // -----------------------------------------------------------------------

    /// `CSL.NameOutput.prototype.init(names)`: start a `cs:names` element.
    pub fn init(&mut self, st: &mut State, names: &Token) -> CslResult<()> {
        self.require_match = js::truthy_opt(names.extra.get("requireMatch"));
        if st.tmp.term_predecessor {
            st.tmp.subsequent_author_substitute_ok = false;
        }
        if self.nameset_offset != 0 {
            self.nameset_base += self.nameset_offset;
        }
        self.nameset_offset = 0;
        self.names = names.clone();
        self.variables = names.variables.clone();

        st.tmp.value = Vec::new();
        st.tmp.rendered_name = Some(Vec::new());
        st.tmp.label_blob = None;
        st.tmp.etal_node = None;
        st.tmp.etal_term = None;
        for v in &self.variables {
            let iv = self.item.get(v.as_str());
            if has_length(iv) {
                if let Some(iv) = iv {
                    concat_into(&mut st.tmp.value, iv);
                }
            }
        }
        self.et_al = None;
        // REMOVE THIS
        self.with = None;

        self.name = None;
        self.name_and = None;
        self.name_ellipsis = None;
        // long, long-with-short, short
        self.institutionpart = InstitutionParts::default();

        if let Some(tip) = st.tmp.group_context.tip_mut() {
            tip.variable_attempt = true;
        }

        self.label_variable = self.variables.first().cloned();

        if st.tmp.value.is_empty() {
            return Ok(());
        }

        // Abort and proceed to the next substitution if a match is required,
        // two variables are called, and they do not match.
        let check_common_term = self.check_common_author(st, self.require_match)?;
        if check_common_term {
            st.tmp.can_substitute.pop();
            st.tmp.can_substitute.push(Value::Bool(true));
            self.unwind_done_vars(st);
            st.tmp.common_term_match_fail = true;
            self.variables = Vec::new();
        }
        Ok(())
    }

    /// The loop shared by `init` and `reinit`:
    /// `for (var i in this.variables) { idx = done_vars.indexOf(variables[i]);
    /// if (idx > -1) done_vars = done_vars.slice(0, idx).concat(done_vars.slice(i+1)); }`.
    /// `i` is the *string* key, so `i+1` concatenates: `"0"+1 = "01"` and
    /// `"1"+1 = "11"`; `slice` then reads `1`, `11`, `21`, ... (not `idx+1`).
    fn unwind_done_vars(&self, st: &mut State) {
        for (i, var) in self.variables.iter().enumerate() {
            if let Some(idx) = st.tmp.done_vars.iter().position(|d| d == var) {
                let skip: usize = format!("{i}1").parse().unwrap_or(usize::MAX);
                let mut next: Vec<String> = st.tmp.done_vars[..idx].to_vec();
                if skip < st.tmp.done_vars.len() {
                    next.extend(st.tmp.done_vars[skip..].iter().cloned());
                }
                st.tmp.done_vars = next;
            }
        }
    }

    /// `CSL.NameOutput.prototype.reinit(names, labelVariable)`: start a
    /// substitute `cs:names` element (the singleton form inside
    /// `cs:substitute`).
    pub fn reinit(
        &mut self,
        st: &mut State,
        names: &Token,
        label_variable: Option<&str>,
    ) -> CslResult<()> {
        self.require_match = js::truthy_opt(names.extra.get("requireMatch"));
        self.label_variable = label_variable.map(str::to_string);

        if st
            .tmp
            .can_substitute
            .value()
            .map(js::truthy)
            .unwrap_or(false)
        {
            self.nameset_offset = 0;
            // What-all should be carried across from the subsidiary
            // names node, and on what conditions? For each attribute,
            // and decoration, is it an override, or is it additive?
            self.variables = names.variables.clone();

            // Not sure why this is necessary. Guards against a memory leak perhaps?
            let oldval = st.tmp.value.clone();
            st.tmp.value = Vec::new();

            for v in &self.variables {
                let iv = self.item.get(v.as_str());
                if has_length(iv) {
                    if let Some(iv) = iv {
                        concat_into(&mut st.tmp.value, iv);
                    }
                }
            }
            if !st.tmp.value.is_empty() {
                st.tmp.can_substitute.replace_literal(Value::Bool(false))?;
            }

            st.tmp.value = oldval;
        }
        // Abort and proceed to the next substitution if a match is required,
        // two variables are called, and they do not match.
        let check_common_term = self.check_common_author(st, self.require_match)?;
        if check_common_term {
            st.tmp.can_substitute.pop();
            st.tmp.can_substitute.push(Value::Bool(true));
            self.unwind_done_vars(st);
            self.variables = Vec::new();
        }
        Ok(())
    }
}

impl NameOutput {
    // -----------------------------------------------------------------------
    // CSL.NameOutput.prototype.outputNames
    // -----------------------------------------------------------------------

    /// `CSL.NameOutput.prototype.outputNames()`: render the names of the
    /// current `cs:names` element into the output queue.
    pub fn output_names(&mut self, st: &mut State) -> CslResult<()> {
        let variables = self.variables.clone();
        if self.institution.is_none() {
            return Err(super::load::type_error(
                "Cannot read properties of undefined (reading 'and')",
            ));
        }
        if let Some(inst_and) = self.institution_and {
            for (mine, theirs) in [
                (inst_and.single, self.name_and.map(|n| n.single)),
                (inst_and.multiple, self.name_and.map(|n| n.multiple)),
            ] {
                let mine_id = mine.ok_or_else(|| {
                    super::load::type_error("Cannot read properties of undefined (reading 'blobs')")
                })?;
                if blob_is_empty(st, mine_id) {
                    let their_id = theirs.flatten().ok_or_else(|| {
                        super::load::type_error(
                            "Cannot read properties of undefined (reading 'blobs')",
                        )
                    })?;
                    let content = st.blobs.get(their_id).blobs.clone();
                    st.blobs.get_mut(mine_id).blobs = content;
                }
            }
        }

        self.variable_offset = BTreeMap::new();
        if let Some(family) = self.family.clone() {
            let mut decor = family.clone_token();
            decor.set_string("prefix", "");
            decor.set_string("suffix", "");
            // Sets text-case value (text-case="title" is suppressed for items
            // non-English with non-English value in Item.language)
            for exec in &family.execs {
                exec.run(st, &mut decor, &self.item.clone(), &Value::Null)?;
            }
            self.family_decor = Some(decor);
        } else {
            self.family_decor = None;
        }

        if let Some(given) = self.given.clone() {
            let mut decor = given.clone_token();
            decor.set_string("prefix", "");
            decor.set_string("suffix", "");
            for exec in &given.execs {
                exec.run(st, &mut decor, &self.item.clone(), &Value::Null)?;
            }
            self.given_decor = Some(decor);
        } else {
            self.given_decor = None;
        }

        // util_names_etalconfig.js
        self.get_et_al_config(st)?;
        // util_names_divide.js
        self.divide_and_transliterate_names(st)?;
        // util_names_truncate.js
        self.truncate_personal_name_lists(st)?;
        // util_names_disambig.js
        self.disambig_names(st)?;
        // util_names_constraints.js
        self.constrain_names(st)?;
        // form="count"
        let form_is_count = self.name_token()?.strings.get("form") == Some(&Value::from("count"));
        if form_is_count {
            if !st.tmp.extension.is_empty() || self.names_count != 0 {
                q_append(
                    st,
                    AppendArg::Number(self.names_count as f64),
                    FormatRef::Name("empty".into()),
                    false,
                )?;
                if let Some(tip) = st.tmp.group_context.tip_mut() {
                    tip.variable_success = true;
                }
            }
            return Ok(());
        }

        self.set_et_al_parameters(st)?;
        self.set_common_term(st)?;
        self.render_all_names(st)?;

        let spoof = js::truthy_opt(
            st.opt
                .get("development_extensions")
                .and_then(|d| d.get("spoof_institutional_affiliations")),
        );
        let mut blob_list: Vec<BlobId> = Vec::new();
        for v in &variables {
            let mut institution_sets: Vec<Option<BlobId>> = Vec::new();
            let mut institutions: Option<BlobId> = None;
            let varblob: Option<BlobId>;
            if !spoof {
                let fb = self.freeter_blobs.get(v).copied().flatten();
                varblob = self.join(st, vec![fb], "", None)?;
            } else {
                let n_inst = self.institution_blobs.get(v).map(Vec::len).unwrap_or(0);
                for j in 0..n_inst {
                    let person = self
                        .person_blobs
                        .get(v)
                        .and_then(|p| p.get(j))
                        .copied()
                        .flatten();
                    let inst = self
                        .institution_blobs
                        .get(v)
                        .and_then(|p| p.get(j))
                        .copied()
                        .flatten();
                    institution_sets.push(self.join_persons_and_institutions(st, vec![person, inst])?);
                }
                if n_inst > 0 {
                    let mut pos = self.nameset_base + self.variable_offset.get(v).copied().unwrap_or(0);
                    // this.freeters[v].length: a blob (or false) has none
                    // after renderAllNames, an unrendered empty list has 0.
                    if self.rendered_freeters_length(v) {
                        pos += 1;
                    }
                    institutions = self.join_institution_sets(st, institution_sets, pos)?;
                }
                let fb = self.freeter_blobs.get(v).copied().flatten();
                varblob = self.join_freeters_and_institution_sets(st, vec![fb, institutions])?;
            }
            if let Some(mut vb) = varblob {
                // Apply labels, if any
                if st.tmp.extension.is_empty() {
                    vb = self.apply_labels(st, vb, v)?;
                }
                blob_list.push(vb);
            }
            if self.common_term.is_some() {
                break;
            }
        }
        q_open_level(st, FormatRef::Name("empty".into()))?;
        let delim = st.inherit_opt(&self.names, "delimiter", Some("names-delimiter"), None);
        if let Some(cur) = queue::current(st, QueueId::Output) {
            match delim {
                Some(d) => st.blobs.get_mut(cur).strings.insert("delimiter".into(), d),
                None => st.blobs.get_mut(cur).strings.remove("delimiter"),
            };
        }
        for b in &blob_list {
            // notSerious
            q_append(st, AppendArg::Blob(*b), FormatRef::Name("literal".into()), true)?;
        }
        if !st.tmp.just_looking && !blob_list.is_empty() {
            st.tmp.probably_rendered_something = true;
        }
        q_close_level_named(st, "empty")?;
        let blob = q_pop_blob_required(st)?;
        st.tmp.name_node.top = Some(blob);

        // Append will drop the names on the floor here if suppress-me is
        // set on element_trace.
        // Need to rescue the value for collapse comparison.
        let names_token = self.names.clone_token();
        if st
            .tmp
            .group_context
            .tip()
            .map(|t| t.condition.is_some())
            .unwrap_or(false)
        {
            let prefix = self.names.string("prefix");
            update_group_context_condition(st, Some(&prefix), false, Some(&self.names), None);
        }
        q_append(st, AppendArg::Blob(blob), FormatRef::Token(names_token), false)?;
        if st.tmp.term_predecessor_name {
            st.tmp.term_predecessor = true;
        }
        // Also used in CSL.Util.substituteEnd (which could do with
        // some cleanup at this writing).
        if variables.first().map(String::as_str) != Some("authority") {
            // Just grab the string values in the name
            let mut name_node_string: Vec<String> = Vec::new();
            if let Some(first) = variables.first() {
                if let Some(Value::Array(nameobjs)) = self.item.get(first.as_str()) {
                    for n in nameobjs {
                        let sub = get_raw_name(n);
                        if !sub.is_empty() {
                            name_node_string.push(sub);
                        }
                    }
                }
            }
            let joined = name_node_string.join(", ");
            if !joined.is_empty() {
                st.tmp.name_node.string = Some(joined);
            }
        }
        // for classic support
        // This may be more convoluted than it needs to be. Or maybe not.
        //
        // Check for classic abbreviation
        //
        // If found, then (1) suppress title rendering, (2) replace the node
        // with the abbreviation output [and (3) do not run this._collapseAuthor() ?]
        if st.tmp.name_node.string.is_some() && st.tmp.first_name_string.is_none() {
            st.tmp.first_name_string = st.tmp.name_node.string.clone();
        }
        if self.item.get("type").and_then(Value::as_str) == Some("classic") {
            if let Some(first) = st.tmp.first_name_string.clone() {
                let mut author_title: Vec<String> = vec![first];
                if let Some(t) = self.item.get("title").filter(|t| js::truthy(t)) {
                    author_title.push(js::to_js_string(t));
                }
                let author_title = author_title.join(", ");
                // `state.sys.getAbbreviation` exists (the host's sys; see
                // build_retrieve_item.rs), as does `normalizeAbbrevsKey`.
                if !author_title.is_empty() {
                    let key = super::build_retrieve_item::normalize_abbrevs_key(
                        "classic",
                        Some(&author_title),
                    );
                    let lang = self.item.get("language").filter(|l| js::truthy(l));
                    let _ = lang;
                    // STUB(util_transform): loadAbbreviation +
                    // abbrevs["default"].classic[key]
                    let abbr = super::build_retrieve_item::abbreviation_lookup(
                        st,
                        Some("default"),
                        "classic",
                        &key,
                    )?;
                    if let Some(abbr) = abbr {
                        st.tmp.done_vars.push("title".to_string());
                        q_append(
                            st,
                            AppendArg::Text(abbr),
                            FormatRef::Name("empty".into()),
                            true,
                        )?;
                        let blob = q_pop_blob_required(st)?;
                        let top = st.tmp.name_node.top.ok_or_else(|| {
                            super::load::type_error(
                                "Cannot read properties of undefined (reading 'blobs')",
                            )
                        })?;
                        if let BlobContent::List(l) = &mut st.blobs.get_mut(top).blobs {
                            l.pop();
                            l.push(BlobChild::Blob(blob));
                        }
                    }
                }
            }
        }

        // Let's try something clever here.
        self.collapse_author(st)?;

        // For name_SubstituteOnNamesSpanNamesSpanFail
        self.variables = Vec::new();

        // Reset stop-last after rendering
        st.tmp.authority_stop_last = 0;
        Ok(())
    }

    /// `this.freeters[v].length` in `outputNames` after `renderAllNames`: the
    /// property is `undefined` on a blob and 0 on the unrendered empty list,
    /// so it is never truthy.
    fn rendered_freeters_length(&self, _v: &str) -> bool {
        false
    }

    /// `CSL.NameOutput.prototype._applyLabels(blob, v)`.
    pub fn apply_labels(
        &mut self,
        st: &mut State,
        blob: BlobId,
        v: &str,
    ) -> CslResult<BlobId> {
        let Some(lv) = self.label_variable.clone() else {
            return Ok(blob);
        };
        let Some(label) = self.label.get(&lv).cloned() else {
            return Ok(blob);
        };
        let mut plural: i64 = 0;
        let mut num = self.freeters_count.get(v).copied().unwrap_or(0)
            + self.institutions_count.get(v).copied().unwrap_or(0);
        if num > 1 {
            plural = 1;
        } else {
            let pc = self.persons_count.get(v).cloned().unwrap_or_default();
            let n_persons = self.persons.get(v).map(Vec::len).unwrap_or(0);
            for i in 0..n_persons {
                num += pc.get(i).copied().unwrap_or(0);
            }
            if num > 1 {
                plural = 1;
            }
        }
        // Some code duplication here, should be factored out.
        let mut blob = blob;
        if let Some(before) = &label.before {
            if let Some(Value::Number(n)) = before.strings.get("plural") {
                plural = n.as_i64().unwrap_or(0);
            }
            let txt = self.build_label(st, v, plural, "before", &lv)?;
            q_open_level(st, FormatRef::Name("empty".into()))?;
            q_append_str(st, txt.as_deref(), FormatRef::Token(before.clone()), true)?;
            q_append(st, AppendArg::Blob(blob), FormatRef::Name("literal".into()), true)?;
            q_close_level_named(st, "empty")?;
            blob = q_pop_blob_required(st)?;
        } else if let Some(after) = &label.after {
            if let Some(Value::Number(n)) = after.strings.get("plural") {
                plural = n.as_i64().unwrap_or(0);
            }
            let txt = self.build_label(st, v, plural, "after", &lv)?;
            q_open_level(st, FormatRef::Name("empty".into()))?;
            q_append(st, AppendArg::Blob(blob), FormatRef::Name("literal".into()), true)?;
            q_append_str(st, txt.as_deref(), FormatRef::Token(after.clone()), true)?;
            st.tmp.label_blob = q_pop_blob(st)?;
            let label_blob = st.tmp.label_blob;
            q_append_blob(st, label_blob, FormatRef::Name("literal".into()), true)?;
            q_close_level_named(st, "empty")?;
            blob = q_pop_blob_required(st)?;
        }
        Ok(blob)
    }

    /// `CSL.NameOutput.prototype._buildLabel(term, plural, position, v)`:
    /// the label text, `None` for `false`.
    pub fn build_label(
        &mut self,
        st: &mut State,
        term: &str,
        plural: i64,
        position: &str,
        v: &str,
    ) -> CslResult<Option<String>> {
        let term = match &self.common_term {
            Some(c) => c.clone(),
            None => term.to_string(),
        };
        let node = self
            .label
            .get(v)
            .and_then(|l| if position == "before" { l.before.clone() } else { l.after.clone() });
        match node {
            Some(node) => Ok(Some(cast_label_in_group(st, &node, &term, plural)?)),
            None => Ok(None),
        }
    }

    /// `CSL.NameOutput.prototype._collapseAuthor()`: suppress the names of a
    /// cite whose author string repeats the previous one's when collapsing.
    pub fn collapse_author(&mut self, st: &mut State) -> CslResult<()> {
        // collapse can be undefined, an array of length zero, and probably
        // other things ... ugh.
        let top = st.tmp.name_node.top.ok_or_else(|| {
            super::load::type_error("Cannot read properties of undefined (reading 'blobs')")
        })?;
        if top_len(st, top)? == 0 {
            return Ok(());
        }
        if self.nameset_base == 0
            && has_value(self.item.get(self.variables.first().map(String::as_str).unwrap_or("")))
            && self.first_creator_variable.is_none()
        {
            self.first_creator_variable = self.variables.first().cloned();
        }
        let collapse = opt_length_truthy(st, "collapse");
        let cgd = opt_length_truthy(st, "cite_group_delimiter");
        if !(collapse || cgd) {
            return Ok(());
        }
        if st.tmp.authorstring_request {
            // Avoid running this on every call to getAmbiguousCite()?
            let oldchars = st.tmp.offset_characters;
            let mystr = last_child_string(st, top)?;
            // Avoid side-effects on character counting: we're only interested
            // in the final rendering.
            st.tmp.offset_characters = oldchars;
            // STUB(registry): this.state.registry.authorstrings[this.Item.id] = mystr
            // (registry.rs is the engine agent's).
            registry_set_authorstring(st, &self.item, mystr);
        } else if !st.tmp.just_looking && !st.tmp.suppress_decorations && (collapse || cgd) {
            let oldchars = st.tmp.offset_characters;
            let mystr = last_child_string(st, top)?;
            if mystr.is_some() && mystr == st.tmp.last_primary_names_string {
                if js::truthy_opt(self.cite_item.get("suppress-author")) || collapse {
                    pop_top_blob(st, top);
                    st.tmp.name_node.children = Vec::new();
                    // If popped, avoid side-effects on character counting: we're only interested
                    // in things that actually render.
                    st.tmp.offset_characters = oldchars;
                }
                // Needed
                if cgd {
                    st.tmp.use_cite_group_delimiter = true;
                }
            } else {
                st.tmp.last_primary_names_string = mystr;
                // XXXXX A little more precision would be nice.
                // This will clobber variable="author editor" as well as variable="author".
                let fcv = self.first_creator_variable.clone();
                let is_first_creator = fcv
                    .as_ref()
                    .map(|f| self.variables.contains(f))
                    .unwrap_or(false);
                if is_first_creator
                    && js::truthy(&self.cite_item)
                    && js::truthy_opt(self.cite_item.get("suppress-author"))
                    && self.item.get("type").and_then(Value::as_str) != Some("legal_case")
                {
                    pop_top_blob(st, top);
                    st.tmp.name_node.children = Vec::new();
                    // If popped, avoid side-effects on character counting: we're only interested
                    // in things that actually render.
                    st.tmp.offset_characters = oldchars;

                    // A wild guess, but will usually be correct
                    st.tmp.term_predecessor = false;
                }
                // Arcane and probably unnecessarily complicated?
                st.tmp.have_collapsed = false;
                // Needed
                if cgd {
                    st.tmp.use_cite_group_delimiter = false;
                }
            }
        }
        Ok(())
    }
}

/// `value !== undefined && truthy`: `this.Item[this.variables[0]]` as a
/// condition.
fn has_value(v: Option<&Value>) -> bool {
    js::truthy_opt(v)
}

/// `state.tmp.name_node.top.blobs.length`.
fn top_len(st: &State, top: BlobId) -> CslResult<usize> {
    match &st.blobs.get(top).blobs {
        BlobContent::List(l) => Ok(l.len()),
        BlobContent::Text(t) => Ok(js::len(t)),
    }
}

/// `state.tmp.name_node.top.blobs.pop()`.
fn pop_top_blob(st: &mut State, top: BlobId) {
    if let BlobContent::List(l) = &mut st.blobs.get_mut(top).blobs {
        l.pop();
    }
}

/// `mystr = ""; myqueue = top.blobs.slice(-1)[0].blobs; if (myqueue) mystr =
/// output.string(state, myqueue, false)`: the rendered string of the last
/// blob of `top`, `None` when the result is not a string (an array never
/// equals a string).
fn last_child_string(st: &mut State, top: BlobId) -> CslResult<Option<String>> {
    let last = match &st.blobs.get(top).blobs {
        BlobContent::List(l) => l.last().cloned(),
        BlobContent::Text(_) => None,
    };
    let Some(BlobChild::Blob(last)) = last else {
        return Err(super::load::type_error(
            "Cannot read properties of undefined (reading 'blobs')",
        ));
    };
    let children = match &st.blobs.get(last).blobs {
        BlobContent::List(l) => l.clone(),
        BlobContent::Text(_) => {
            return Err(super::load::type_error(
                "Cannot read properties of undefined (reading 'strings')",
            ))
        }
    };
    let r = queue::string(st, QueueId::Output, &children, StringParent::Bool(false))?;
    Ok(match r {
        Rendered::Str(s) => Some(s),
        Rendered::List(l) if l.is_empty() => Some(String::new()),
        _ => None,
    })
}

/// STUB(registry): `state.registry.authorstrings[Item.id] = mystr`
/// (api_cite.js `getAmbiguousCite` reads it). `registry.rs` belongs to the
/// engine agent; the integrator replaces this body.
fn registry_set_authorstring(_st: &mut State, _item: &Value, _mystr: Option<String>) {}

/// `CSL.castLabel(state, node, term, plural, CSL.TOLERANT)` with
/// `state.tmp.group_context.tip` supplying and receiving the label flags.
pub(super) fn cast_label_in_group(
    st: &mut State,
    node: &Token,
    term: &str,
    plural: i64,
) -> CslResult<String> {
    use super::util_label::{cast_label, LabelContext};
    let (form, cap) = match st.tmp.group_context.tip() {
        Some(t) => (t.label_form.clone(), t.label_capitalize_if_first.clone()),
        None => (Value::Null, Value::Null),
    };
    let mut ctx = LabelContext {
        tip_label_form: match &form {
            Value::String(s) => Some(s.clone()),
            _ => None,
        },
        tip_label_capitalize_if_first: if js::truthy(&cap) { Some(cap) } else { None },
        tip_label_static: false,
        strip_periods: st.tmp.strip_periods != 0,
    };
    let r = cast_label(st, node, &mut ctx, Some(term), Some(plural), super::load::TOLERANT)?;
    if ctx.tip_label_static {
        if let Some(t) = st.tmp.group_context.tip_mut() {
            t.label_static = Value::Bool(true);
        }
    }
    Ok(r)
}
