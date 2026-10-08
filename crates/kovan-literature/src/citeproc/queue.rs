// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/queue.js
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

//! `CSL.Output.Queue`: the output queue (`state.output` and `state.dateput`),
//! the tree of blobs the style's nodes append to, and its rendering
//! (`string`, `renderBlobs`) and punctuation-fixing passes
//! (`CSL.Output.Queue.adjust`, `CSL.Output.Queue.purgeEmptyBlobs`).
//!
//! # API shape (read this first)
//!
//! Two queues exist, `state.output` and `state.dateput`, and queue methods
//! also need the rest of the state (`state.tmp`, `state.blobs`,
//! `state.fun.flipflopper`, ...). So every JS method
//! `this.output.<method>(args)` is a **free function** here that takes the
//! state and a [`QueueId`] first:
//!
//! | JS | Rust |
//! |---|---|
//! | `state.output.append(str, tok, notSerious, ...)` | `queue::append(state, QueueId::Output, str, tok, ...)` |
//! | `state.dateput.openLevel(tok)` | `queue::open_level(state, QueueId::Dateput, tok)` |
//! | `state.output.closeLevel(name)` | `queue::close_level(state, QueueId::Output, name)` |
//! | `state.output.startTag` / `endTag` | [`start_tag`] / [`end_tag`] |
//! | `state.output.addToken` / `pushFormats` / `popFormats` | [`add_token`] / [`push_formats`] / [`pop_formats`] |
//! | `state.output.getToken` / `mergeTokenStrings` | [`get_token`] / [`merge_token_strings`] |
//! | `state.output.pop()` / `clearlevel()` | [`pop`] / [`clearlevel`] |
//! | `state.output.string(state, blobs, blob)` | [`string`] |
//! | `state.output.renderBlobs(...)` | [`render_blobs`] |
//! | `state.output.queue` (the array) | [`queue_children`] |
//! | `state.output.current.value()` | [`current`] |
//! | `CSL.Output.Queue.purgeEmptyBlobs(blob)` | [`purge_empty_blobs`] |
//! | `new CSL.Output.Queue.adjust(punctInQuote)` | [`Adjust::new`] (stored in `queue.adjust`) |
//!
//! A format token argument (JS: a token object, a format name, or
//! `undefined`) is a [`FormatRef`]. A blob is a [`BlobId`] into
//! `state.blobs`; the queue's root array is a `BlobKind::RootArray` blob.
//! The queue allocates its root lazily on first use, so `Queue::default()`
//! is a valid, unused queue.
//!
//! # Differences from JS that are not behaviour
//!
//! * `append` with a bare [`FormatRef::Token`] sets the (JS: shared) token's
//!   missing `delimiter` to `""` on a copy; the blob copies the same value.
//! * Calls that would raise a JS TypeError (an unbalanced `closeLevel`, a
//!   bare string inside a list that `string()` walks) return `Err`.
//! * Stand-ins: `getOpt("punctuation-in-quote")` ([`formats::get_opt_flag`]),
//!   the registry `offset` write ([`registry_set_offset`], `PORT-LATER`).

use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::formats::{decorate, get_opt_flag, safe_escape, SafeEscape};
use super::formatters;
use super::js;
use super::obj_blob::{
    drop_first, drop_last, first_char, last_char, romanesque_regexp, Blob, BlobChild, BlobContent,
    BlobId, BlobKind, Blobs, JS_WS_CLASS,
};
use super::obj_number::{self, NumArg, END, SEEN, START, SUCCESSOR, SUPPRESS};
use super::obj_token::{Decoration, Token, TokenType};
use super::stack::Stack;
use super::state::{Area, State};
use super::util_flipflop;
use super::{CslResult, EngineError};

// DUP-CHECK: load.js CSL.TERMINAL_PUNCTUATION
/// `CSL.TERMINAL_PUNCTUATION`.
pub const TERMINAL_PUNCTUATION: [&str; 6] = [":", ".", ";", "!", "?", " "];

/// Which of the engine's two output queues a call addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueId {
    /// `state.output`.
    Output,
    /// `state.dateput`.
    Dateput,
}

/// A format token argument: a token object, a name in the current format
/// store, or JS `undefined`/falsy.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum FormatRef {
    /// `undefined` (or `""`, `null`, `false`).
    #[default]
    None,
    /// A name looked up in `queue.formats.value()`; `"empty"` is the queue's
    /// empty token and `"literal"` has special meaning in [`append`].
    Name(String),
    /// A token object.
    Token(Token),
}

impl From<&str> for FormatRef {
    fn from(s: &str) -> FormatRef {
        FormatRef::Name(s.to_string())
    }
}

impl From<&Token> for FormatRef {
    fn from(t: &Token) -> FormatRef {
        FormatRef::Token(t.clone())
    }
}

/// The first argument of [`append`] (JS `str`).
#[derive(Debug, Clone, PartialEq)]
pub enum AppendArg {
    /// `undefined`: `append` returns `false` and does nothing.
    Undefined,
    /// A string.
    Text(String),
    /// A number (stringified, as `"" + str`).
    Number(f64),
    /// A blob (`append(blob, "literal")`, or wrapped in a new blob for a
    /// named token).
    Blob(BlobId),
}

impl From<&str> for AppendArg {
    fn from(s: &str) -> AppendArg {
        AppendArg::Text(s.to_string())
    }
}

impl From<String> for AppendArg {
    fn from(s: String) -> AppendArg {
        AppendArg::Text(s)
    }
}

/// An element of what [`string`] and [`render_blobs`] return: JS arrays that
/// mix strings and (numeric) blobs.
#[derive(Debug, Clone, PartialEq)]
pub enum Rendered {
    /// A string.
    Str(String),
    /// A blob object (a numeric blob left for later joining).
    Blob(BlobId),
    /// An array (`string()`'s normal return value; nested arrays occur only
    /// in `renderBlobs` with `in_cite`).
    List(Vec<Rendered>),
}

impl Rendered {
    /// JS truthiness: a non-empty string, a blob, or any array.
    pub fn is_truthy(&self) -> bool {
        match self {
            Rendered::Str(s) => !s.is_empty(),
            _ => true,
        }
    }

    /// `"" + value` as JS would compute it (`Array.prototype.toString`
    /// joins with commas; a blob prints as `[object Object]`).
    pub fn to_js_string(&self) -> String {
        match self {
            Rendered::Str(s) => s.clone(),
            Rendered::Blob(_) => "[object Object]".to_string(),
            Rendered::List(l) => l
                .iter()
                .map(Rendered::to_js_string)
                .collect::<Vec<_>>()
                .join(","),
        }
    }

    /// The elements, if this is a list (the usual return of [`string`]).
    pub fn into_list(self) -> Vec<Rendered> {
        match self {
            Rendered::List(l) => l,
            other => vec![other],
        }
    }
}

/// The third argument of [`string`] (JS `blob`): `undefined`, a boolean, or
/// the parent blob whose children are being rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringParent {
    /// `undefined`: top-level call; resets the queue afterwards.
    None,
    /// A boolean (`renderBlobs` calls `string(state, [blob], false)`).
    Bool(bool),
    /// A blob.
    Blob(BlobId),
}

/// `CSL.checkNestedBrace(state)` (load.js:248): flips nested parentheses in
/// affixes to brackets in note styles. `update` is `this.update`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CheckNestedBrace {
    /// `state.opt.xclass === "note"`; otherwise `update` is the identity.
    pub note: bool,
    /// `this.depth`.
    pub depth: i64,
}

impl CheckNestedBrace {
    /// `new CSL.checkNestedBrace(state)`; `xclass_is_note` is
    /// `state.opt.xclass === "note"`.
    pub fn new(xclass_is_note: bool) -> CheckNestedBrace {
        // DUP-CHECK: load.js CSL.checkNestedBrace
        CheckNestedBrace {
            note: xclass_is_note,
            depth: 0,
        }
    }

    /// `this.update(str)`.
    pub fn update(&mut self, s: &str) -> String {
        if !self.note {
            return s.to_string();
        }
        let mut out = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                '(' => {
                    if self.depth % 2 == 1 {
                        out.push('[');
                    } else {
                        out.push('(');
                    }
                    self.depth += 1;
                }
                ')' => {
                    if self.depth % 2 == 0 {
                        out.push(']');
                    } else {
                        out.push(')');
                    }
                    self.depth -= 1;
                }
                other => out.push(other),
            }
        }
        out
    }
}

/// `CSL.Output.Queue`.
#[derive(Debug, Clone, PartialEq)]
pub struct Queue {
    /// `levelname` (`["top"]`; unused upstream beyond construction).
    pub levelname: Vec<String>,
    /// `queue`: the root array, a `BlobKind::RootArray` blob. `None` until
    /// the queue is first used (see the module docs); read the children
    /// with [`queue_children`].
    pub root: Option<BlobId>,
    /// `empty`: `new CSL.Token("empty")`.
    pub empty: Token,
    /// `formats`: the stack of `{name: token}` stores. `empty` is looked up
    /// in `self.empty` when a store has no entry for it (upstream copies
    /// `this.empty` into every store).
    pub formats: Stack<BTreeMap<String, Token>>,
    /// `current`: the stack of open levels (blobs, the root at the bottom).
    pub current: Stack<BlobId>,
    /// `last_char_rendered`.
    pub last_char_rendered: Option<String>,
    /// `checkNestedBrace` (set by the citation API).
    pub check_nested_brace: Option<CheckNestedBrace>,
    /// `adjust` (set by build.js / the citation API).
    pub adjust: Option<Adjust>,
}

impl Default for Queue {
    fn default() -> Queue {
        Queue {
            levelname: vec!["top".to_string()],
            root: None,
            empty: Token::new("empty", TokenType::Start),
            formats: Stack::with(BTreeMap::new()),
            current: Stack::new(),
            last_char_rendered: None,
            check_nested_brace: None,
            adjust: None,
        }
    }
}

impl Queue {
    /// `new CSL.Output.Queue(state)`: a queue with its root allocated in
    /// `blobs`.
    pub fn new(blobs: &mut Blobs) -> Queue {
        let mut q = Queue::default();
        q.init_root(blobs);
        q
    }

    fn init_root(&mut self, blobs: &mut Blobs) {
        let root = new_root(blobs);
        self.root = Some(root);
        self.current = Stack::with(root);
    }
}

fn new_root(blobs: &mut Blobs) -> BlobId {
    let b = Blob {
        kind: BlobKind::RootArray,
        ..Blob::default()
    };
    blobs.add(b)
}

fn q_ref(state: &State, q: QueueId) -> &Queue {
    match q {
        QueueId::Output => &state.output,
        QueueId::Dateput => &state.dateput,
    }
}

fn q_mut(state: &mut State, q: QueueId) -> &mut Queue {
    match q {
        QueueId::Output => &mut state.output,
        QueueId::Dateput => &mut state.dateput,
    }
}

/// Allocate the queue's root array if this is its first use.
fn ensure(state: &mut State, q: QueueId) {
    let needs = q_ref(state, q).root.is_none();
    if needs {
        let root = new_root(&mut state.blobs);
        let qq = q_mut(state, q);
        qq.root = Some(root);
        qq.current = Stack::with(root);
    }
}

/// `state.output.queue` (the array): its children, in order.
pub fn queue_children(state: &State, q: QueueId) -> Vec<BlobChild> {
    match q_ref(state, q).root {
        Some(r) => match &state.blobs.get(r).blobs {
            BlobContent::List(l) => l.clone(),
            BlobContent::Text(_) => Vec::new(),
        },
        None => Vec::new(),
    }
}

/// `state.output.queue.length === 0`.
pub fn queue_is_empty(state: &State, q: QueueId) -> bool {
    queue_children(state, q).is_empty()
}

/// `state.output.current.value()`: the open level (the root array when no
/// level is open); `None` when the stack is empty.
pub fn current(state: &State, q: QueueId) -> Option<BlobId> {
    q_ref(state, q).current.tip().copied()
}

/// The token a format name denotes in the current store.
fn format_token(queue: &Queue, name: &str) -> Option<Token> {
    if let Some(t) = queue.formats.tip().and_then(|m| m.get(name)) {
        return Some(t.clone());
    }
    if name == "empty" {
        return Some(queue.empty.clone());
    }
    None
}

/// `blob` list children of a (list) blob.
fn kids(b: &Blobs, id: BlobId) -> Vec<BlobChild> {
    match &b.get(id).blobs {
        BlobContent::List(l) => l.clone(),
        BlobContent::Text(_) => Vec::new(),
    }
}

fn kid_blob(c: &BlobChild) -> Option<BlobId> {
    match c {
        BlobChild::Blob(i) => Some(*i),
        BlobChild::Str(_) => None,
    }
}

/// `state[state.tmp.area]` by name.
fn area_by_name<'a>(state: &'a State, name: &str) -> Option<&'a Area> {
    match name {
        "citation" => Some(&state.citation),
        "bibliography" => Some(&state.bibliography),
        "intext" => Some(&state.intext),
        "citation_sort" => Some(&state.citation_sort),
        "bibliography_sort" => Some(&state.bibliography_sort),
        _ => None,
    }
}

/// A JSON `[[name, value, extra?], ...]` (how `layout_decorations` is stored
/// in an area's `opt`) as decorations.
pub fn decorations_from_value(v: &Value) -> Option<Vec<Decoration>> {
    let arr = v.as_array()?;
    let mut out = Vec::new();
    for d in arr {
        let a = d.as_array()?;
        let name = a.first().map(js::to_js_string)?;
        let value = a.get(1).map(js::to_js_string).unwrap_or_default();
        let extra = a.get(2).filter(|x| !x.is_null()).map(js::to_js_string);
        out.push(Decoration { name, value, extra });
    }
    Some(out)
}

/// `state[state.tmp.area].opt.layout_decorations` (`None` when unset). The
/// area's `opt` holds it as JSON `[[name, value], ...]`.
pub fn layout_decorations(state: &State) -> Option<Vec<Decoration>> {
    let area = area_by_name(state, &state.tmp.area)?;
    area.opt
        .get("layout_decorations")
        .and_then(decorations_from_value)
}

/// `state.registry.registry[id].offset = offset` if the registry has `id`;
/// returns whether it did (queue.js:421). // STUB(registry.rs)
///
/// PORT-LATER(registry offset): queue.js:420-424, needs `state.registry.registry` (wave4 registry port). Returns `false`, so counting is never switched off here.
pub fn registry_set_offset(_state: &mut State, _id: &str, _offset: usize) -> bool {
    false
}

fn dev_ext(state: &State, key: &str) -> bool {
    state
        .opt
        .get("development_extensions")
        .and_then(Value::as_object)
        .and_then(|o| o.get(key))
        .map(js::truthy)
        .unwrap_or(false)
}

fn tmp_error(msg: &str) -> EngineError {
    EngineError::Csl(msg.to_string())
}

// ---------------------------------------------------------------------------
// The simple methods
// ---------------------------------------------------------------------------

/// `Queue.prototype.pop`: remove and return the last child of the open level.
/// Errors where JS would throw (the open level is a text leaf, or an empty
/// root array).
pub fn pop(state: &mut State, q: QueueId) -> CslResult<Option<BlobChild>> {
    ensure(state, q);
    let cur = current(state, q).ok_or_else(|| tmp_error("pop: no open level"))?;
    let blob = state.blobs.get_mut(cur);
    let is_root = blob.kind == BlobKind::RootArray;
    match &mut blob.blobs {
        BlobContent::List(l) => {
            if l.is_empty() && is_root {
                return Err(tmp_error("pop: empty root array"));
            }
            Ok(l.pop())
        }
        BlobContent::Text(_) => Err(tmp_error("pop: current level is a string")),
    }
}

/// `Queue.prototype.getToken(name)`.
pub fn get_token(state: &State, q: QueueId, name: &str) -> Option<Token> {
    format_token(q_ref(state, q), name)
}

/// `Queue.prototype.mergeTokenStrings(base, modifier)`: a new `base` token
/// carrying `base`'s strings overridden by `modifier`'s, and both
/// decorations; `base` itself when there is no `modifier`.
pub fn merge_token_strings(state: &State, q: QueueId, base: &str, modifier: &str) -> Option<Token> {
    let queue = q_ref(state, q);
    let base_token = format_token(queue, base);
    let modifier_token = format_token(queue, modifier);
    let Some(modifier_token) = modifier_token else {
        return base_token;
    };
    let base_token = match base_token {
        Some(b) => b,
        None => {
            let mut t = Token::new(base, TokenType::Singleton);
            t.decorations = Vec::new();
            t
        }
    };
    let mut ret = Token::new(base, TokenType::Singleton);
    for (k, v) in &base_token.strings {
        ret.strings.insert(k.clone(), v.clone());
    }
    for (k, v) in &modifier_token.strings {
        ret.strings.insert(k.clone(), v.clone());
    }
    let mut decor = base_token.decorations.clone();
    decor.extend(modifier_token.decorations.iter().cloned());
    ret.decorations = decor;
    Some(ret)
}

/// `Queue.prototype.addToken(name, modifier, token)`: store a new output
/// format token, based on `token` (a token or the name of one in the current
/// store), in the current store. `modifier` (a string) becomes the new
/// token's `delimiter`.
pub fn add_token(
    state: &mut State,
    q: QueueId,
    name: &str,
    modifier: Option<&str>,
    token: FormatRef,
) {
    let queue = q_mut(state, q);
    let mut newtok = Token::new("output", TokenType::Start);
    let src: Option<Token> = match token {
        FormatRef::Name(n) => format_token(queue, &n),
        FormatRef::Token(t) => Some(t),
        FormatRef::None => None,
    };
    if let Some(t) = src {
        for (k, v) in &t.strings {
            newtok.strings.insert(k.clone(), v.clone());
        }
        newtok.decorations = t.decorations.clone();
    }
    if let Some(m) = modifier {
        newtok.set_string("delimiter", m);
    }
    if let Some(store) = queue.formats.tip_mut() {
        store.insert(name.to_string(), newtok);
    }
}

/// `Queue.prototype.pushFormats(tokenstore)`: push a new bundle of format
/// tokens (`empty` always resolves to the queue's empty token).
pub fn push_formats(state: &mut State, q: QueueId, tokenstore: Option<BTreeMap<String, Token>>) {
    let mut store = tokenstore.unwrap_or_default();
    store.remove("empty");
    q_mut(state, q).formats.push_literal(store);
}

/// `Queue.prototype.popFormats`.
pub fn pop_formats(state: &mut State, q: QueueId) {
    q_mut(state, q).formats.pop();
}

/// `Queue.prototype.startTag(name, token)`: push a one-token store and open
/// a level named `name`. (Inside a macro-with-date the token is replaced by
/// the empty one.)
pub fn start_tag(
    state: &mut State,
    q: QueueId,
    name: &str,
    token: Option<&Token>,
) -> CslResult<()> {
    let mut name = name.to_string();
    let mut token = token.cloned();
    if state.tmp.doing_macro_with_date
        && !state.tmp.extension.is_empty()
    {
        token = Some(q_ref(state, q).empty.clone());
        name = "empty".to_string();
    }
    let mut store = BTreeMap::new();
    if let Some(t) = token {
        store.insert(name.clone(), t);
    }
    push_formats(state, q, Some(store));
    open_level(state, q, FormatRef::Name(name))
}

/// `Queue.prototype.endTag(name)`.
pub fn end_tag(state: &mut State, q: QueueId, name: Option<&str>) -> CslResult<()> {
    close_level(state, q, name)?;
    pop_formats(state, q);
    Ok(())
}

/// `Queue.prototype.openLevel(token)`: add a new blob to the end of the open
/// level and make it the open level.
pub fn open_level(state: &mut State, q: QueueId, token: FormatRef) -> CslResult<()> {
    ensure(state, q);
    let queue = q_ref(state, q);
    let mut blob = match token {
        FormatRef::Token(t) => Blob::new(None, Some(&t), None),
        FormatRef::None => Blob::new(None, Some(&queue.empty), Some("empty")),
        FormatRef::Name(n) => match format_token(queue, &n) {
            Some(t) => Blob::new(None, Some(&t), Some(&n)),
            None => {
                return Err(EngineError::Csl(format!(
                    "CSL processor error: call to nonexistent format token \"{n}\""
                )))
            }
        },
    };
    if !state.tmp.just_looking {
        if let Some(cnb) = q_mut(state, q).check_nested_brace.as_mut() {
            let p = blob.string("prefix");
            blob.set_string("prefix", &cnb.update(&p));
        }
    }
    let curr = current(state, q).ok_or_else(|| tmp_error("openLevel: no open level"))?;
    let id = state.blobs.add(blob);
    state.blobs.push(curr, id)?;
    q_mut(state, q).current.push_literal(id);
    Ok(())
}

/// `Queue.prototype.closeLevel(name)`: close the open level (checking its
/// name when given).
pub fn close_level(state: &mut State, q: QueueId, name: Option<&str>) -> CslResult<()> {
    ensure(state, q);
    if let Some(n) = name.filter(|n| !n.is_empty()) {
        let cur = current(state, q).ok_or_else(|| tmp_error("closeLevel: no open level"))?;
        let found = state.blobs.get(cur).levelname.clone();
        if Some(n) != found.as_deref() {
            return Err(EngineError::Csl(format!(
                "Level mismatch error:  wanted {} but found {}",
                n,
                found.unwrap_or_else(|| "undefined".to_string())
            )));
        }
    }
    let blob = q_mut(state, q).current.pop();
    if !state.tmp.just_looking {
        if let Some(mut cnb) = q_mut(state, q).check_nested_brace.take() {
            let r = match blob {
                Some(id) => {
                    let s = state.blobs.get(id).string("suffix");
                    let upd = cnb.update(&s);
                    state.blobs.get_mut(id).set_string("suffix", &upd);
                    Ok(())
                }
                None => Err(tmp_error("closeLevel: no open level")),
            };
            q_mut(state, q).check_nested_brace = Some(cnb);
            r?;
        }
    }
    Ok(())
}

/// `Queue.prototype.clearlevel`: empty the open level.
pub fn clearlevel(state: &mut State, q: QueueId) -> CslResult<()> {
    ensure(state, q);
    let cur = current(state, q).ok_or_else(|| tmp_error("clearlevel: no open level"))?;
    if let BlobContent::List(l) = &mut state.blobs.get_mut(cur).blobs {
        l.clear();
    }
    Ok(())
}

static RE_SPACE_BEFORE_PUNCT: LazyLock<Regex> = LazyLock::new(|| re(" ([:;?!\u{00bb}])"));
static RE_WS_APOS: LazyLock<Regex> = LazyLock::new(|| re(&format!("[{JS_WS_CLASS}]+'")));
static RE_STRIP_PERIODS: LazyLock<Regex> = LazyLock::new(|| re(r"\.([^a-z]|$)"));
static RE_TAGS: LazyLock<Regex> = LazyLock::new(|| re("<[^>]*>"));

fn re(p: &str) -> Regex {
    #[allow(clippy::expect_used)]
    Regex::new(p).expect("constant regex")
}

/// `Queue.prototype.append(str, tokname, notSerious, ignorePredecessor,
/// noStripPeriods)`: add a text blob (or a blob) to the open level.
/// Returns `false` when nothing was added (JS `return false`).
///
/// `tokname` is the format token: a [`FormatRef::Token`], a name in the
/// current store, `Name("literal")` (push `str` unwrapped), or `None` for
/// the empty token.
pub fn append(
    state: &mut State,
    q: QueueId,
    s: AppendArg,
    tokname: FormatRef,
    not_serious: bool,
    ignore_predecessor: bool,
    no_strip_periods: bool,
) -> CslResult<bool> {
    ensure(state, q);
    let mut ignore_predecessor = ignore_predecessor;
    let mut tokname = tokname;
    let mut use_blob = true;
    if not_serious {
        ignore_predecessor = true;
    }
    // XXXXX Nasty workaround, but still an improvement
    // over the reverse calls to the cs:date node build
    // function that we had before.
    if state.tmp.doing_macro_with_date && !not_serious {
        match &tokname {
            FormatRef::Name(n) if n == "macro-with-date" => {
                tokname = FormatRef::Name("empty".to_string());
            }
            _ => return Ok(false),
        }
    }
    let arg = match s {
        AppendArg::Undefined => return Ok(false),
        AppendArg::Number(n) => AppendArg::Text(js::number_to_js_string(n)),
        other => other,
    };
    if !not_serious {
        if state.tmp.element_trace.value().map(String::as_str) == Some("suppress-me") {
            return Ok(false);
        }
    }
    // Resolve the token.
    enum Tok {
        Real(Token),
        Literal,
    }
    let mut named: Option<String> = None;
    let mut is_empty_ref = false;
    let tok = match tokname {
        FormatRef::None => {
            is_empty_ref = true;
            Tok::Real(q_ref(state, q).empty.clone())
        }
        FormatRef::Name(n) if n.is_empty() => {
            is_empty_ref = true;
            Tok::Real(q_ref(state, q).empty.clone())
        }
        FormatRef::Name(n) if n == "literal" => {
            use_blob = false;
            Tok::Literal
        }
        FormatRef::Name(n) => match format_token(q_ref(state, q), &n) {
            Some(t) => {
                named = Some(n);
                Tok::Real(t)
            }
            None => {
                return Err(EngineError::Csl(format!(
                    "CSL processor error: unknown format token name: {n}"
                )))
            }
        },
        FormatRef::Token(t) => Tok::Real(t),
    };
    // Unset delimiters must be left undefined until they reach the queue
    // in order to discriminate unset from explicitly empty delimiters
    // when inheriting a default value from a superior node.
    // (The JS mutates the stored token; the store is kept in step.)
    let tok = match tok {
        Tok::Real(mut t) => {
            if !t.strings.contains_key("delimiter") {
                t.set_string("delimiter", "");
                let queue = q_mut(state, q);
                let in_store = match &named {
                    Some(n) => match queue.formats.tip_mut() {
                        Some(store) => match store.get_mut(n) {
                            Some(stored) => {
                                stored.set_string("delimiter", "");
                                true
                            }
                            None => false,
                        },
                        None => false,
                    },
                    None => false,
                };
                if !in_store && (named.as_deref() == Some("empty") || is_empty_ref) {
                    queue.empty.set_string("delimiter", "");
                }
            }
            Some(t)
        }
        Tok::Literal => None,
    };

    // The string handling.
    match arg {
        AppendArg::Text(mut text) => {
            let mut blob: Blob;
            if !text.is_empty() {
                // Source (;?!»«): http://en.wikipedia.org/wiki/Space_(punctuation)#Breaking_and_non-breaking_spaces
                // Source (:): http://forums.zotero.org/discussion/4933/localized-quotes/#Comment_88384
                text = RE_SPACE_BEFORE_PUNCT
                    .replace_all(&text, "\u{202f}${1}")
                    .into_owned()
                    .replace("\u{00ab} ", "\u{00ab}\u{202f}");
                q_mut(state, q).last_char_rendered = Some(last_char(&text).to_string());
                // This, and not the str argument below on flipflop, is the
                // source of the flipflopper string source.
                text = RE_WS_APOS.replace_all(&text, " '").into_owned();
                if !not_serious {
                    // this condition for sort_LeadingApostropheOnNameParticle
                    if text.starts_with('\'') {
                        text = format!(" '{}", &text[1..]);
                    }
                }
                // signal whether we end with terminal punctuation?
                if !ignore_predecessor {
                    state.tmp.term_predecessor = true;
                    state.tmp.in_cite_predecessor = true;
                } else if not_serious {
                    state.tmp.term_predecessor_name = true;
                }
            }
            blob = match &tok {
                Some(t) => Blob::new(Some(&text), Some(t), None),
                None => {
                    // `new CSL.Blob(str, true)`: only prefix and suffix.
                    let mut b = Blob::new(Some(&text), None, None);
                    b.strings.remove("delimiter");
                    b
                }
            };
            // `curr` is undefined with an empty stack: re-seed (queue.js:283-287).
            if q_ref(state, q).current.is_empty() {
                let root = new_root(&mut state.blobs);
                q_mut(state, q).current.push_literal(root);
            }
            let curr = current(state, q).ok_or_else(|| tmp_error("append: no open level"))?;
            if !ignore_predecessor {
                state.tmp.term_predecessor = true;
                state.tmp.in_cite_predecessor = true;
            } else if not_serious {
                state.tmp.term_predecessor_name = true;
            }
            //
            // Caution: The parallel detection machinery will blow up if tracking
            // variables are not properly initialized elsewhere.
            //
            if let BlobContent::Text(bt) = blob.blobs.clone() {
                if !bt.starts_with(' ') {
                    let mut blob_prefix = String::new();
                    let mut blob_blobs = bt.clone();
                    while !blob_blobs.is_empty()
                        && TERMINAL_PUNCTUATION.contains(&first_char(&blob_blobs))
                    {
                        let c = first_char(&blob_blobs).to_string();
                        blob_prefix.push_str(&c);
                        blob_blobs = drop_first(&blob_blobs).to_string();
                    }
                    if !blob_blobs.is_empty() && !blob_prefix.is_empty() {
                        let p = blob.string("prefix");
                        blob.set_string("prefix", &format!("{p}{blob_prefix}"));
                        blob.blobs = BlobContent::Text(blob_blobs);
                    }
                }
            }
            let tc = blob
                .strings
                .get("text-case")
                .filter(|v| js::truthy(v))
                .map(js::to_js_string);
            if let Some(tc) = tc {
                //
                // This one is _particularly_ hard to follow.  It's not obvious,
                // but the blob already contains the input string at this
                // point, as blob.blobs -- it's a terminal node, as it were.
                // The str variable also contains the input string, but
                // that copy is not used for onward processing.  We have to
                // apply our changes to the blob copy.
                //
                blob.blobs = BlobContent::Text(formatters::apply(state, &tc, &text)?);
            }
            if state.tmp.strip_periods != 0 && !no_strip_periods {
                if let BlobContent::Text(bt) = &blob.blobs {
                    let r = RE_STRIP_PERIODS.replace_all(bt, "${1}").into_owned();
                    blob.blobs = BlobContent::Text(r);
                }
            }
            let mut i = blob.decorations.len();
            while i > 0 {
                i -= 1;
                if blob.decorations[i].name == "@quotes" && blob.decorations[i].value != "false" {
                    blob.punctuation_in_quote = Some(get_opt_flag(state, "punctuation-in-quote"));
                }
                let has_roman = match &blob.blobs {
                    BlobContent::Text(t) => romanesque_regexp().is_match(t),
                    BlobContent::List(_) => false,
                };
                if !has_roman && blob.decorations[i].name == "@font-style" {
                    blob.decorations.remove(i);
                }
            }
            //
            // XXX: Beware superfluous code in your code.  str in this
            // case is not the source of the final rendered string.
            // See note above.
            //
            let id = state.blobs.add(blob);
            state.blobs.push(curr, id)?;
            util_flipflop::process_tags(state, id)?;
            Ok(true)
        }
        AppendArg::Blob(child) => {
            if q_ref(state, q).current.is_empty() {
                let root = new_root(&mut state.blobs);
                q_mut(state, q).current.push_literal(root);
            }
            let curr = current(state, q).ok_or_else(|| tmp_error("append: no open level"))?;
            if use_blob {
                let wrapper = match &tok {
                    Some(t) => state.blobs.new_blob_wrapping(child, Some(t), None),
                    None => state.blobs.new_blob_wrapping(child, None, None),
                };
                state.blobs.push(curr, wrapper)?;
            } else {
                state.blobs.push(curr, child)?;
            }
            Ok(true)
        }
        AppendArg::Undefined | AppendArg::Number(_) => Ok(false),
    }
}

/// `append(str, tokname)` with the three trailing flags off.
pub fn append_simple(
    state: &mut State,
    q: QueueId,
    s: impl Into<AppendArg>,
    tokname: impl Into<FormatRef>,
) -> CslResult<bool> {
    append(state, q, s.into(), tokname.into(), false, false, false)
}

// ---------------------------------------------------------------------------
// string / renderBlobs
// ---------------------------------------------------------------------------

fn rendered_num_blob(state: &State, r: &Rendered) -> Option<BlobId> {
    match r {
        Rendered::Blob(id) if state.blobs.get(*id).num_is_number() => Some(*id),
        _ => None,
    }
}

/// `Queue.prototype.string(state, myblobs, blob)`: render `myblobs` (the
/// children of `blob`, or of the whole queue when `blob` is
/// [`StringParent::None`]) to an array of strings and numeric blobs.
///
/// `q` is JS `this`: only the final reset of the queue (when `blob` is
/// `None`) happens on it; recursion goes through `state.output` as upstream
/// does.
pub fn string(
    state: &mut State,
    q: QueueId,
    myblobs: &[BlobChild],
    parent: StringParent,
) -> CslResult<Rendered> {
    ensure(state, q);
    let txt_esc: SafeEscape = safe_escape(state);
    let blobs: Vec<BlobChild> = myblobs.to_vec();
    let mut ret: Vec<Rendered> = Vec::new();

    if blobs.is_empty() {
        return Ok(Rendered::List(ret));
    }

    let mut blob_delimiter = String::new();
    let parent_blob: Option<BlobId> = match parent {
        StringParent::Blob(id) => Some(id),
        _ => None,
    };
    match parent {
        StringParent::Blob(id) => {
            blob_delimiter = state.blobs.get(id).string("delimiter");
        }
        StringParent::Bool(true) => {
            return Err(tmp_error("string: blob === true (TypeError upstream)"));
        }
        StringParent::Bool(false) | StringParent::None => {
            state.tmp.count_offset_characters = None;
            state.tmp.offset_characters = 0;
        }
    }

    if let Some(pb) = parent_blob {
        if let Some(nl) = state.blobs.get(pb).new_locale.clone() {
            let old = state.opt.get("lang").map(js::to_js_string);
            state.blobs.get_mut(pb).old_locale = old;
            state.opt.insert("lang".into(), Value::String(nl));
        }
    }

    let mut last_child: Option<BlobId> = None;
    for child in blobs.iter() {
        let Some(blobjr) = kid_blob(child) else {
            return Err(tmp_error(
                "string: bare string inside the output queue (TypeError upstream)",
            ));
        };
        last_child = Some(blobjr);

        let first_blob = state
            .blobs
            .get(blobjr)
            .strings
            .get("first_blob")
            .filter(|v| js::truthy(v))
            .map(js::to_js_string);
        if let Some(fb) = &first_blob {
            // Being the Item.id of the the entry being rendered.
            state.tmp.count_offset_characters = Some(fb.clone());
        }

        if state.blobs.get(blobjr).is_leaf() {
            if state.blobs.get(blobjr).num_is_number() {
                ret.push(Rendered::Blob(blobjr));
            } else if matches!(&state.blobs.get(blobjr).blobs, BlobContent::Text(t) if !t.is_empty())
            {
                {
                    let bj = state.blobs.get_mut(blobjr);
                    if let Some(p) = bj.particle.clone().filter(|p| !p.is_empty()) {
                        if let BlobContent::Text(t) = &bj.blobs {
                            let nt = format!("{p}{t}");
                            bj.blobs = BlobContent::Text(nt);
                        }
                        bj.particle = Some(String::new());
                    }
                }
                // (skips empty strings)
                let text = match &state.blobs.get(blobjr).blobs {
                    BlobContent::Text(t) => t.clone(),
                    BlobContent::List(_) => String::new(),
                };
                let mut b = txt_esc.escape(&text);
                let blen = js::len(&b);

                if !state.tmp.suppress_decorations {
                    let decs = state.blobs.get(blobjr).decorations.clone();
                    for params in decs.iter() {
                        if params.name == "@showid" {
                            continue;
                        }
                        if state.normal_decor_is_orphan(blobjr, params) {
                            continue;
                        }
                        b = decorate(
                            state,
                            Some(state.blobs.get(blobjr)),
                            &params.name,
                            &params.value,
                            Some(&b),
                            params.extra.as_deref(),
                        )?;
                    }
                }
                //
                // because we will rip out portions of the output
                // queue before rendering, group wrappers need
                // to produce no output if they are found to be
                // empty.
                if !b.is_empty() {
                    let prefix = state.blobs.get(blobjr).string("prefix");
                    let suffix = state.blobs.get(blobjr).string("suffix");
                    b = format!(
                        "{}{}{}",
                        txt_esc.escape(&prefix),
                        b,
                        txt_esc.escape(&suffix)
                    );
                    if dev_ext(state, "csl_reverse_lookup_support")
                        && !state.tmp.suppress_decorations
                    {
                        let decs = state.blobs.get(blobjr).decorations.clone();
                        for params in decs.iter() {
                            if params.name == "@showid" {
                                b = decorate(
                                    state,
                                    Some(state.blobs.get(blobjr)),
                                    &params.name,
                                    &params.value,
                                    Some(&b),
                                    params.extra.as_deref(),
                                )?;
                            }
                        }
                    }
                    ret.push(Rendered::Str(b));
                    if state.tmp.count_offset_characters.is_some() {
                        state.tmp.offset_characters += blen + js::len(&suffix) + js::len(&prefix);
                    }
                }
            }
        } else if matches!(&state.blobs.get(blobjr).blobs, BlobContent::List(l) if !l.is_empty()) {
            let list = kids(&state.blobs, blobjr);
            let addtoret = string(state, QueueId::Output, &list, StringParent::Blob(blobjr))?;
            let mut add = addtoret.into_list();
            if parent_blob.is_some() {
                // Patch up world-class weird bug in the ill-constructed code of mine.
                let delim = state.blobs.get(blobjr).string("delimiter");
                if add.len() > 1 && !delim.is_empty() {
                    let mut number_seen = false;
                    for el in add.iter_mut() {
                        match el {
                            Rendered::Str(s) => {
                                if number_seen {
                                    *s = format!("{delim}{s}");
                                }
                            }
                            _ => number_seen = true,
                        }
                    }
                }
            }
            ret.extend(add);
        }
        if let Some(fb) = first_blob {
            // (state.registry.registry[...] existence test and write)
            let off = state.tmp.offset_characters;
            if registry_set_offset(state, &fb, off) {
                // The Item.id of the entry being rendered.
                state.tmp.count_offset_characters = None;
            }
        }
    }

    // Provide delimiters on adjacent numeric blobs
    if ret.len() > 1 {
        for i in 0..ret.len() - 1 {
            if let (Some(a), Some(b)) = (
                rendered_num_blob(state, &ret[i]),
                rendered_num_blob(state, &ret[i + 1]),
            ) {
                if !state.blobs.get(b).ugly_delimiter_suppress_hack {
                    // XXX watch this
                    let suf = state.blobs.get(a).string("suffix");
                    state
                        .blobs
                        .get_mut(a)
                        .set_string("suffix", &format!("{suf}{blob_delimiter}"));
                    let nb = state.blobs.get_mut(b);
                    nb.successor_prefix = Some(String::new());
                    nb.ugly_delimiter_suppress_hack = true;
                }
            }
        }
    }

    let mut span_split: usize = 0;
    let rlen = ret.len();
    for i in 0..rlen {
        if let Rendered::Str(_) = &ret[i] {
            span_split = i + 1;
            if i + 1 < rlen {
                if let Rendered::Blob(nb) = ret[i + 1].clone() {
                    if !blob_delimiter.is_empty()
                        && !state.blobs.get(nb).ugly_delimiter_suppress_hack
                    {
                        if let Rendered::Str(s) = &mut ret[i] {
                            s.push_str(&txt_esc.escape(&blob_delimiter));
                        }
                    }
                    // One bite of the apple
                    state.blobs.get_mut(nb).ugly_delimiter_suppress_hack = true;
                }
            }
        }
    }
    if let Some(pb) = parent_blob {
        let (has_decor, suffix, prefix) = {
            let b = state.blobs.get(pb);
            (
                !b.decorations.is_empty(),
                b.string("suffix"),
                b.string("prefix"),
            )
        };
        if has_decor || !suffix.is_empty() {
            span_split = ret.len();
        } else if !prefix.is_empty() {
            for i in 0..ret.len() {
                if let Rendered::Blob(rb) = &ret[i] {
                    if state.blobs.get(*rb).has_num() {
                        span_split = i;
                        if i == 0 {
                            let rp = state.blobs.get(*rb).string("prefix");
                            state
                                .blobs
                                .get_mut(*rb)
                                .set_string("prefix", &format!("{prefix}{rp}"));
                        }
                        break;
                    }
                }
            }
        }
    }

    let mut blobs_start = render_blobs(
        state,
        QueueId::Output,
        ret[..span_split.min(ret.len())].to_vec(),
        &blob_delimiter,
        false,
        parent_blob,
    )?;
    if let Some(pb) = parent_blob {
        let (decs, suffix, prefix) = {
            let b = state.blobs.get(pb);
            (
                b.decorations.clone(),
                b.string("suffix"),
                b.string("prefix"),
            )
        };
        if blobs_start.is_truthy() && (!decs.is_empty() || !suffix.is_empty() || !prefix.is_empty())
        {
            if !state.tmp.suppress_decorations {
                for params in decs.iter() {
                    if ["@cite", "@bibliography", "@display", "@showid"]
                        .contains(&params.name.as_str())
                    {
                        continue;
                    }
                    // (upstream passes `blobjr`, the last child of the loop)
                    if let Some(lc) = last_child {
                        if state.normal_decor_is_orphan(lc, params) {
                            continue;
                        }
                    }
                    if params.name.is_empty() {
                        continue;
                    }
                    if let Rendered::Str(s) = &blobs_start {
                        let r = decorate(
                            state,
                            Some(state.blobs.get(pb)),
                            &params.name,
                            &params.value,
                            Some(s),
                            params.extra.as_deref(),
                        )?;
                        blobs_start = Rendered::Str(r);
                    }
                }
            }
            //
            // XXXX: cut-and-paste warning.  same as a code block above.
            //
            if let Rendered::Str(b) = &blobs_start {
                if !b.is_empty() {
                    let nb = format!(
                        "{}{}{}",
                        txt_esc.escape(&prefix),
                        b,
                        txt_esc.escape(&suffix)
                    );
                    if state.tmp.count_offset_characters.is_some() {
                        state.tmp.offset_characters += js::len(&prefix) + js::len(&suffix);
                    }
                    blobs_start = Rendered::Str(nb);
                }
            }
            if !state.tmp.suppress_decorations {
                for params in decs.iter() {
                    if !["@cite", "@bibliography", "@display", "@showid"]
                        .contains(&params.name.as_str())
                    {
                        continue;
                    }
                    if let Rendered::Str(s) = &blobs_start {
                        let r = decorate(
                            state,
                            Some(state.blobs.get(pb)),
                            &params.name,
                            &params.value,
                            Some(s),
                            params.extra.as_deref(),
                        )?;
                        blobs_start = Rendered::Str(r);
                    }
                }
            }
        }
    }

    let blobs_end: Vec<Rendered> = ret[span_split.min(ret.len())..].to_vec();
    if blobs_end.is_empty() && blobs_start.is_truthy() {
        ret = vec![blobs_start];
    } else if !blobs_end.is_empty() && !blobs_start.is_truthy() {
        ret = blobs_end;
    } else if blobs_start.is_truthy() && !blobs_end.is_empty() {
        let mut r = vec![blobs_start];
        r.extend(blobs_end);
        ret = r;
    }
    //
    // Blobs is now definitely a string with
    // trailing blobs.  Return it.
    let mut out = Rendered::List(ret);
    match parent {
        StringParent::None => {
            let root = new_root(&mut state.blobs);
            let qq = q_mut(state, q);
            qq.root = Some(root);
            qq.current = Stack::with(root);
            if state.tmp.suppress_decorations {
                let l = out.into_list();
                out = render_blobs(state, QueueId::Output, l, "", false, None)?;
            }
        }
        StringParent::Bool(_) => {
            let l = out.into_list();
            out = render_blobs(state, QueueId::Output, l, "", true, None)?;
        }
        StringParent::Blob(_) => {}
    }

    if let Some(pb) = parent_blob {
        if state.blobs.get(pb).new_locale.is_some() {
            let old = state.blobs.get(pb).old_locale.clone();
            match old {
                Some(o) => {
                    state.opt.insert("lang".into(), Value::String(o));
                }
                None => {
                    state.opt.remove("lang");
                }
            }
        }
    }
    Ok(out)
}

/// `Queue.prototype.renderBlobs(blobs, delim, in_cite, parent)`: join the
/// rendered pieces into a string, rendering numeric blobs (ranges, collapsed
/// year-suffixes) on the way. With `in_cite`, numeric blobs are left in the
/// result for the citation API to join across items.
pub fn render_blobs(
    state: &mut State,
    q: QueueId,
    blobs: Vec<Rendered>,
    delim: &str,
    in_cite: bool,
    parent: Option<BlobId>,
) -> CslResult<Rendered> {
    ensure(state, q);
    let txt_esc = safe_escape(state);
    let mut blobs = blobs;
    let len = blobs.len();
    let mut ret = Rendered::Str(String::new());
    let mut use_delim = String::new();
    if state.tmp.area == "citation" && !state.tmp.just_looking && len == 1 {
        if let (Rendered::Blob(b0), Some(p)) = (&blobs[0], parent) {
            let b0 = *b0;
            let (pp, ps, pd, params) = {
                let pb = state.blobs.get(p);
                (
                    pb.string("prefix"),
                    pb.string("suffix"),
                    pb.decorations.clone(),
                    pb.extra.get("params").cloned(),
                )
            };
            let bb = state.blobs.get_mut(b0);
            let np = format!("{pp}{}", bb.string("prefix"));
            bb.set_string("prefix", &np);
            let ns = format!("{}{ps}", bb.string("suffix"));
            bb.set_string("suffix", &ns);
            bb.decorations.extend(pd);
            match params {
                Some(v) => {
                    bb.extra.insert("params".into(), v);
                }
                None => {
                    bb.extra.remove("params");
                }
            }
            return Ok(Rendered::Blob(b0));
        }
    }
    let mut start = true;
    for pos in 0..len {
        let this_numeric = match &blobs[pos] {
            Rendered::Blob(id) if state.blobs.get(*id).kind == BlobKind::Numeric => Some(*id),
            _ => None,
        };
        if let Some(id) = this_numeric {
            let next = blobs.get(pos + 1).and_then(|r| match r {
                Rendered::Blob(n) => Some(*n),
                _ => None,
            });
            obj_number::check_next(&mut state.blobs, id, next, start);
            start = false;
        } else if let Some(Rendered::Blob(n)) = blobs.get(pos + 1) {
            if state
                .blobs
                .get(*n)
                .splice_prefix
                .as_deref()
                .map(|s| !s.is_empty())
                .unwrap_or(false)
                && state.blobs.get(*n).kind == BlobKind::Numeric
            {
                start = false;
            } else {
                start = true;
            }
        } else {
            start = true;
        }
    }

    // Fix last non-range join
    let mut doit = true;
    let mut pos = blobs.len();
    while pos > 1 {
        pos -= 1;
        let this_numeric = match &blobs[pos] {
            Rendered::Blob(id) if state.blobs.get(*id).kind == BlobKind::Numeric => Some(*id),
            _ => None,
        };
        if let Some(id) = this_numeric {
            if doit {
                let last = match &blobs[pos - 1] {
                    Rendered::Blob(l) => Some(*l),
                    _ => None,
                };
                if obj_number::check_last(&mut state.blobs, id, last) {
                    doit = false;
                }
            }
        } else {
            doit = true;
        }
    }
    let len = blobs.len();
    for pos in 0..len {
        let blob = std::mem::replace(&mut blobs[pos], Rendered::Str(String::new()));
        if ret.is_truthy() {
            use_delim = delim.to_string();
        }
        match blob {
            Rendered::Str(s) => {
                let mut cur = ret.to_js_string();
                cur.push_str(&txt_esc.escape(&use_delim));
                // XXX Blob should be run through flipflop and flattened here.
                // (I think it must be a fragment of text around a numeric
                // variable)
                cur.push_str(&s);
                ret = Rendered::Str(cur);
                if state.tmp.count_offset_characters.is_some() {
                    state.tmp.offset_characters += js::len(&use_delim);
                }
            }
            other if in_cite => {
                // pass
                // Okay, so this does it -- but we're now not able to return a string!
                if ret.is_truthy() {
                    ret = Rendered::List(vec![ret, other]);
                } else {
                    ret = Rendered::List(vec![other]);
                }
            }
            Rendered::Blob(id) => {
                if state.blobs.get(id).status != Some(SUPPRESS) {
                    let (particle, num, num_text, gender, formatter) = {
                        let b = state.blobs.get(id);
                        (
                            b.particle.clone().filter(|p| !p.is_empty()),
                            b.num,
                            b.num_text.clone(),
                            b.gender.clone(),
                            b.formatter.clone(),
                        )
                    };
                    let num_arg = match (num, &num_text) {
                        (Some(n), _) => NumArg::Number(n),
                        (None, Some(t)) => NumArg::Text(t.clone()),
                        (None, None) => NumArg::Text("undefined".to_string()),
                    };
                    let mut s = match particle {
                        Some(p) => format!(
                            "{p}{}",
                            match &num_arg {
                                NumArg::Number(n) => n.to_string(),
                                NumArg::Text(t) => t.clone(),
                            }
                        ),
                        None => formatter.unwrap_or_default().format(
                            state,
                            &num_arg,
                            gender.as_deref(),
                        )?,
                    };
                    // Workaround to get a more or less accurate value.
                    let strlen = js::len(&RE_TAGS.replace_all(&s, ""));
                    // notSerious
                    append(
                        state,
                        q,
                        AppendArg::Text(s.clone()),
                        FormatRef::Name("empty".into()),
                        true,
                        false,
                        false,
                    )?;
                    let str_blob = pop(state, q)?;
                    let count_offset_characters = state.tmp.count_offset_characters.clone();
                    let children: Vec<BlobChild> = str_blob.into_iter().collect();
                    let rendered = string(state, q, &children, StringParent::Bool(false))?;
                    s = rendered.to_js_string();
                    state.tmp.count_offset_characters = count_offset_characters;
                    let tc = state
                        .blobs
                        .get(id)
                        .strings
                        .get("text-case")
                        .filter(|v| js::truthy(v))
                        .map(js::to_js_string);
                    if let Some(tc) = tc {
                        s = formatters::apply(state, &tc, &s)?;
                    }
                    if !s.is_empty() && state.tmp.strip_periods != 0 {
                        s = RE_STRIP_PERIODS.replace_all(&s, "${1}").into_owned();
                    }
                    if !state.tmp.suppress_decorations {
                        let decs = state.blobs.get(id).decorations.clone();
                        for params in decs.iter() {
                            if state.normal_decor_is_orphan(id, params) {
                                continue;
                            }
                            s = decorate(
                                state,
                                Some(state.blobs.get(id)),
                                &params.name,
                                &params.value,
                                Some(&s),
                                params.extra.as_deref(),
                            )?;
                        }
                    }
                    let (prefix, suffix) = {
                        let b = state.blobs.get(id);
                        (b.string("prefix"), b.string("suffix"))
                    };
                    s = format!(
                        "{}{}{}",
                        txt_esc.escape(&prefix),
                        s,
                        txt_esc.escape(&suffix)
                    );
                    let b = state.blobs.get(id);
                    let addme = match b.status {
                        Some(x) if x == END => {
                            txt_esc.escape(b.range_prefix.as_deref().unwrap_or(""))
                        }
                        Some(x) if x == SUCCESSOR => {
                            txt_esc.escape(b.successor_prefix.as_deref().unwrap_or(""))
                        }
                        Some(x) if x == START => {
                            if pos > 0 && !b.suppress_splice_prefix.unwrap_or(false) {
                                txt_esc.escape(b.splice_prefix.as_deref().unwrap_or(""))
                            } else {
                                String::new()
                            }
                        }
                        Some(x) if x == SEEN => {
                            // THIS IS NOT THE PROPER FUNCTION OF CSL.SEEN, IS IT?
                            txt_esc.escape(b.splice_prefix.as_deref().unwrap_or(""))
                        }
                        _ => String::new(),
                    };
                    let mut cur = ret.to_js_string();
                    cur.push_str(&addme);
                    cur.push_str(&s);
                    ret = Rendered::Str(cur);
                    if state.tmp.count_offset_characters.is_some() {
                        state.tmp.offset_characters +=
                            js::len(&addme) + js::len(&prefix) + strlen + js::len(&suffix);
                    }
                }
            }
            Rendered::List(_) => {}
        }
    }
    Ok(ret)
}

// ---------------------------------------------------------------------------
// purgeEmptyBlobs
// ---------------------------------------------------------------------------

/// `CSL.Output.Queue.purgeEmptyBlobs(parent)`: recursively remove, back to
/// front, every child that has no content (an empty list or empty text; a
/// bare string child also goes).
pub fn purge_empty_blobs(blobs: &mut Blobs, parent: BlobId) {
    let list = match &blobs.get(parent).blobs {
        BlobContent::List(l) if !l.is_empty() => l.clone(),
        _ => return,
    };
    // back-to-front, bottom-first
    for i in (0..list.len()).rev() {
        let remove;
        match &list[i] {
            BlobChild::Blob(cid) => {
                purge_empty_blobs(blobs, *cid);
                let c = blobs.get(*cid);
                remove = match &c.blobs {
                    BlobContent::Text(t) => t.is_empty(),
                    BlobContent::List(l) => l.is_empty(),
                };
            }
            BlobChild::Str(_) => {
                remove = true;
            }
        }
        if remove {
            if let BlobContent::List(l) = &mut blobs.get_mut(parent).blobs {
                if i < l.len() {
                    l.remove(i);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// adjust
// ---------------------------------------------------------------------------

/// `CSL.Output.Queue.adjust.LtoR_MAP[first][second]`.
fn ltor(first: &str, second: &str) -> Option<&'static str> {
    Some(match (first, second) {
        ("!", ".") => "!",
        ("!", "?") => "!?",
        ("!", ":") => "!",
        ("!", ",") => "!,",
        ("!", ";") => "!;",
        ("?", "!") => "?!",
        ("?", ".") => "?",
        ("?", ":") => "?",
        ("?", ",") => "?,",
        ("?", ";") => "?;",
        (".", "!") => ".!",
        (".", "?") => ".?",
        (".", ":") => ".:",
        (".", ",") => ".,",
        (".", ";") => ".;",
        (":", "!") => "!",
        (":", "?") => "?",
        (":", ".") => ":",
        (":", ",") => ":,",
        (":", ";") => ":;",
        (",", "!") => ",!",
        (",", "?") => ",?",
        (",", ":") => ",:",
        (",", ".") => ",.",
        (",", ";") => ",;",
        (";", "!") => "!",
        (";", "?") => "?",
        (";", ":") => ";",
        (";", ",") => ";,",
        (";", ".") => ";",
        _ => return None,
    })
}

/// `RtoL_MAP[second][first]` (= `LtoR_MAP[first][second]`).
fn rtol(second: &str, first: &str) -> Option<&'static str> {
    ltor(first, second)
}

/// `PUNCT[c]`: one of `! ? . : , ;`.
fn is_punct(c: &str) -> bool {
    matches!(c, "!" | "?" | "." | ":" | "," | ";")
}

/// `PUNCT_OR_SPACE[c]` (punctuation, space, no-break space).
fn is_punct_or_space(c: &str) -> bool {
    is_punct(c) || c == " " || c == "\u{00a0}"
}

/// `SWAP_IN[c]`: punctuation minus `NO_SWAP_IN` (`;` `:`).
fn is_swap_in(c: &str) -> bool {
    matches!(c, "!" | "?" | "." | ",")
}

/// `SWAP_OUT[c]`: punctuation minus `NO_SWAP_OUT` (`.` `!` `?`).
fn is_swap_out(c: &str) -> bool {
    matches!(c, ":" | "," | ";")
}

/// A string slot `mergeChars` reads and writes: a blob's text, or one of its
/// affixes.
#[derive(Debug, Clone, Copy)]
enum Loc {
    /// `blob.blobs` (a text leaf).
    Text(BlobId),
    /// `blob.strings.suffix`.
    Suffix(BlobId),
    /// `blob.strings.prefix`.
    Prefix(BlobId),
}

fn loc_get(b: &Blobs, l: Loc) -> String {
    match l {
        Loc::Text(id) => match &b.get(id).blobs {
            BlobContent::Text(t) => t.clone(),
            BlobContent::List(_) => String::new(),
        },
        Loc::Suffix(id) => b.get(id).string("suffix"),
        Loc::Prefix(id) => b.get(id).string("prefix"),
    }
}

fn loc_set(b: &mut Blobs, l: Loc, v: String) {
    match l {
        Loc::Text(id) => b.get_mut(id).blobs = BlobContent::Text(v),
        Loc::Suffix(id) => b.get_mut(id).set_string("suffix", &v),
        Loc::Prefix(id) => b.get_mut(id).set_string("prefix", &v),
    }
}

/// `mergeChars(First, first, Second, second, merge_right)` (the
/// closure-heavy helper inside `CSL.Output.Queue.adjust`).
fn merge_chars(b: &mut Blobs, first: Loc, second: Loc, merge_right: bool) {
    let first_s = loc_get(b, first);
    let second_s = loc_get(b, second);
    let first_char_ = last_char(&first_s).to_string();
    let second_char = first_char(&second_s).to_string();
    // cullRight / cullLeft / addRight / addLeft
    let cull_right = |b: &mut Blobs| {
        let s = loc_get(b, second);
        loc_set(b, second, drop_first(&s).to_string());
    };
    let cull_left = |b: &mut Blobs| {
        let s = loc_get(b, first);
        loc_set(b, first, drop_last(&s).to_string());
    };
    let add_right = |b: &mut Blobs, chr: &str| {
        let s = loc_get(b, second);
        loc_set(b, second, format!("{chr}{s}"));
    };
    let add_left = |b: &mut Blobs, chr: &str| {
        let s = loc_get(b, first);
        loc_set(b, first, format!("{s}{chr}"));
    };
    let is_duplicate = first_char_ == second_char;
    if is_duplicate {
        if merge_right {
            cull_left(b);
        } else {
            cull_right(b);
        }
    } else {
        // matchOnLeft: LtoR_MAP[firstChar]; matchOnRight: RtoL_MAP[secondChar]
        let matched = if merge_right {
            is_punct(&first_char_)
        } else {
            is_punct(&second_char)
        };
        if matched {
            if merge_right {
                // mergeToRight
                match ltor(&first_char_, &second_char) {
                    Some(chr) => {
                        cull_left(b);
                        cull_right(b);
                        add_right(b, chr);
                    }
                    None => {
                        add_right(b, &first_char_);
                        cull_left(b);
                    }
                }
            } else {
                // mergeToLeft
                match rtol(&second_char, &first_char_) {
                    Some(chr) => {
                        cull_left(b);
                        cull_right(b);
                        add_left(b, chr);
                    }
                    None => {
                        add_left(b, &second_char);
                        cull_right(b);
                    }
                }
            }
        }
    }
}

/// `blobIsNumber(blob)`.
fn blob_is_number(b: &Blobs, id: BlobId) -> bool {
    let blob = b.get(id);
    if blob.num_is_number() {
        return true;
    }
    match &blob.blobs {
        BlobContent::List(l) if l.len() == 1 => match &l[0] {
            BlobChild::Blob(c) => b.get(*c).num_is_number(),
            BlobChild::Str(_) => false,
        },
        _ => false,
    }
}

/// `blobEndsInNumber(blob)`.
fn blob_ends_in_number(b: &Blobs, id: BlobId) -> bool {
    let blob = b.get(id);
    if blob.num_is_number() {
        return true;
    }
    match &blob.blobs {
        BlobContent::List(l) => match l.last() {
            Some(BlobChild::Blob(c)) => blob_ends_in_number(b, *c),
            _ => false,
        },
        BlobContent::Text(_) => false,
    }
}

/// `blobHasDecorations(blob, includeQuotes)`.
fn blob_has_decorations(blob: &Blob, include_quotes: bool) -> bool {
    blob.decorations.iter().any(|d| {
        matches!(
            d.name.as_str(),
            "@font-style"
                | "@font-variant"
                | "@font-weight"
                | "@text-decoration"
                | "@vertical-align"
        ) || (include_quotes && d.name == "@quotes")
    })
}

/// `blobHasDescendantQuotes(blob)`.
fn blob_has_descendant_quotes(b: &Blobs, id: BlobId) -> bool {
    let blob = b.get(id);
    if blob
        .decorations
        .iter()
        .any(|d| d.name == "@quotes" && d.value != "false")
    {
        return true;
    }
    match &blob.blobs {
        BlobContent::List(l) => match l.last() {
            Some(BlobChild::Blob(c)) => blob_has_descendant_quotes(b, *c),
            _ => false,
        },
        BlobContent::Text(_) => false,
    }
}

/// `blobHasDescendantMergingPunctuation(parentChar, blob)`.
fn blob_has_descendant_merging_punctuation(b: &Blobs, parent_char: &str, id: BlobId) -> bool {
    let blob = b.get(id);
    let mut child_char = last_char(&blob.string("suffix")).to_string();
    if child_char.is_empty() {
        if let BlobContent::Text(t) = &blob.blobs {
            child_char = last_char(t).to_string();
        }
    }
    // RtoL_MAP[parentChar][childChar]
    if let Some(merged) = rtol(parent_char, &child_char) {
        if merged.chars().count() == 1 {
            return true;
        }
    }
    match &blob.blobs {
        BlobContent::List(l) => match l.last() {
            Some(BlobChild::Blob(c)) => blob_has_descendant_merging_punctuation(b, parent_char, *c),
            _ => false,
        },
        BlobContent::Text(_) => false,
    }
}

/// `matchLastChar(blob, chr)`.
fn match_last_char(b: &Blobs, id: BlobId, chr: &str) -> bool {
    if !is_punct(chr) {
        return false;
    }
    let blob = b.get(id);
    match &blob.blobs {
        BlobContent::Text(t) => last_char(t) == chr,
        BlobContent::List(l) => match l.last() {
            Some(BlobChild::Blob(c)) => {
                let child_char = last_char(&b.get(*c).string("suffix")).to_string();
                if child_char.is_empty() {
                    match_last_char(b, *c, chr)
                } else {
                    child_char == chr
                }
            }
            _ => false,
        },
    }
}

/// `new CSL.Output.Queue.adjust(punctInQuote)`: the punctuation-moving
/// passes run over the queue before rendering (`upward`, `leftward`,
/// `downward`, `fix`). Each takes a top-level blob of the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Adjust {
    /// `punctInQuote` (`state.getOpt("punctuation-in-quote")`; `false` when
    /// built without the argument).
    pub punct_in_quote: bool,
}

impl Adjust {
    /// `new CSL.Output.Queue.adjust(punctInQuote)`.
    pub fn new(punct_in_quote: bool) -> Adjust {
        Adjust { punct_in_quote }
    }

    /// `adjust.upward(parent)`: migrate leading/trailing space and
    /// punctuation of first/last children into the parent's affixes.
    pub fn upward(&self, b: &mut Blobs, parent: BlobId) {
        // Terminus if no blobs
        if let BlobContent::Text(t) = &b.get(parent).blobs {
            if !t.is_empty() {
                let t = t.clone();
                let suffix = b.get(parent).string("suffix");
                let c0 = first_char(&suffix);
                if is_punct(c0) && c0 == last_char(&t) {
                    let ns = drop_first(&suffix).to_string();
                    b.get_mut(parent).set_string("suffix", &ns);
                }
            }
            return;
        }
        let list = kids(b, parent);
        if list.is_empty() {
            return;
        }
        // back-to-front, bottom-first
        let parent_decorations = blob_has_decorations(b.get(parent), true);
        let n = list.len();
        for i in (0..n).rev() {
            let Some(cid) = kid_blob(&list[i]) else {
                continue;
            };
            self.upward(b, cid);
            if i == 0 {
                let pprefix = b.get(parent).string("prefix");
                let cprefix = b.get(cid).string("prefix");
                // Remove leading space on first-position child node prefix if there is a trailing space on the node prefix above
                if last_char(&pprefix) == " " && first_char(&cprefix) == " " {
                    b.get_mut(cid).set_string("prefix", drop_first(&cprefix));
                }
                // Migrate leading punctuation or space on a first-position prefix upward
                let cprefix = b.get(cid).string("prefix");
                let child_char = first_char(&cprefix).to_string();
                let pprefix = b.get(parent).string("prefix");
                if !parent_decorations && is_punct_or_space(&child_char) && pprefix.is_empty() {
                    b.get_mut(parent)
                        .set_string("prefix", &format!("{pprefix}{child_char}"));
                    b.get_mut(cid).set_string("prefix", drop_first(&cprefix));
                }
            }
            if i == n - 1 {
                // Migrate trailing space ONLY on a last-position suffix upward, controlling for duplicates
                let csuffix = b.get(cid).string("suffix");
                let child_char = last_char(&csuffix).to_string();
                // ZZZ Loosened to fix initialized names wrapped in a span and followed by a period
                if !parent_decorations && child_char == " " {
                    let psuffix = b.get(parent).string("suffix");
                    if first_char(&psuffix) != child_char {
                        b.get_mut(parent)
                            .set_string("suffix", &format!("{child_char}{psuffix}"));
                    }
                    b.get_mut(cid).set_string("suffix", drop_last(&csuffix));
                }
            }
            let delim = b.get(parent).string("delimiter");
            if !delim.is_empty() && i > 0 {
                // Remove leading space on mid-position child node prefix if there is a trailing space on delimiter above
                let cprefix = b.get(cid).string("prefix");
                if is_punct_or_space(last_char(&delim)) && last_char(&delim) == first_char(&cprefix)
                {
                    b.get_mut(cid).set_string("prefix", drop_first(&cprefix));
                }
            }
            // Siblings are handled in adjustNearsideSuffixes()
        }
    }

    /// `adjust.leftward(parent)`: copy a sibling's leading punctuation onto
    /// the preceding sibling's suffix.
    pub fn leftward(&self, b: &mut Blobs, parent: BlobId) {
        // Terminus if no blobs
        let list = match &b.get(parent).blobs {
            BlobContent::List(l) if !l.is_empty() => l.clone(),
            _ => return,
        };
        let n = list.len();
        for i in (0..n).rev() {
            if let Some(cid) = kid_blob(&list[i]) {
                self.leftward(b, cid);
            }
            // This is a delicate one.
            //
            // Migrate if:
            // * there is no umbrella delimiter [ok]
            // * neither the child nor its sibling is a number [ok]
            // * decorations exist neither on the child nor on the sibling [ok]
            // * sibling prefix char is a swapping char [ok]
            //
            // Suppress without migration if:
            // * sibling prefix char matches child suffix char or
            // * child suffix is empty and sibling prefix char match last field char
            if i + 1 < n && b.get(parent).string("delimiter").is_empty() {
                let (Some(child), Some(sibling)) = (kid_blob(&list[i]), kid_blob(&list[i + 1]))
                else {
                    continue;
                };
                let sibling_char = first_char(&b.get(sibling).string("prefix")).to_string();
                let has_decorations = blob_has_decorations(b.get(child), false)
                    || blob_has_decorations(b.get(sibling), false);
                // (`hasNumber` compares a string to "number": always false)
                if !has_decorations && is_punct(&sibling_char) {
                    let csuffix = b.get(child).string("suffix");
                    let suffix_and_prefix_match = sibling_char == last_char(&csuffix);
                    let suffix_and_field_match = csuffix.is_empty()
                        && matches!(&b.get(child).blobs, BlobContent::Text(t) if last_char(t) == sibling_char);
                    if !suffix_and_prefix_match && !suffix_and_field_match {
                        merge_chars(b, Loc::Suffix(child), Loc::Prefix(sibling), false);
                    } else {
                        let sp = b.get(sibling).string("prefix");
                        b.get_mut(sibling).set_string("prefix", drop_first(&sp));
                    }
                }
            }
        }
    }

    /// `adjust.downward(parent)`: push the parent's suffix punctuation and
    /// the delimiter's leading punctuation down into the children.
    pub fn downward(&self, b: &mut Blobs, parent: BlobId) {
        // Terminus if no blobs
        if let BlobContent::Text(t) = &b.get(parent).blobs {
            if !t.is_empty() {
                let t = t.clone();
                let suffix = b.get(parent).string("suffix");
                let c0 = first_char(&suffix);
                if is_punct(c0) && c0 == last_char(&t) {
                    let ns = drop_first(&suffix).to_string();
                    b.get_mut(parent).set_string("suffix", &ns);
                }
            }
            return;
        }
        let list = kids(b, parent);
        if list.is_empty() {
            return;
        }
        let n = list.len();

        // (someChildrenAreNumbers is computed upstream but every use is
        // guarded by `true ||`)

        // If there is a leading swappable character on delimiter, copy it to suffixes IFF none of the targets are numbers
        let delim = b.get(parent).string("delimiter");
        if is_punct(first_char(&delim)) {
            let delim_char = first_char(&delim).to_string();
            if n >= 2 {
                for i in (0..=(n - 2)).rev() {
                    if let Some(cid) = kid_blob(&list[i]) {
                        let cs = b.get(cid).string("suffix");
                        if last_char(&cs) != delim_char {
                            b.get_mut(cid)
                                .set_string("suffix", &format!("{cs}{delim_char}"));
                        }
                    }
                }
            }
            b.get_mut(parent)
                .set_string("delimiter", drop_first(&delim));
        }
        // back-to-front, top-first
        for i in (0..n).rev() {
            let Some(child) = kid_blob(&list[i]) else {
                continue;
            };
            let child_decorations = blob_has_decorations(b.get(child), true);
            let child_is_number = blob_is_number(b, child);

            if i == n - 1 {
                // If we have decorations, drill down to see if there are quotes below.
                // If so, we allow migration anyway.
                // Original discussion is here:
                // https://forums.zotero.org/discussion/37091/citeproc-bug-punctuation-in-quotes/
                let parent_suffix = b.get(parent).string("suffix");
                let parent_char = first_char(&parent_suffix).to_string();

                let mut allow_migration = false;
                if is_punct(&parent_char) {
                    allow_migration =
                        blob_has_descendant_merging_punctuation(b, &parent_char, child);
                    if !allow_migration && self.punct_in_quote {
                        allow_migration = blob_has_descendant_quotes(b, child);
                    }
                }
                if allow_migration && is_punct(&parent_char) && !blob_ends_in_number(b, child) {
                    if matches!(b.get(child).blobs, BlobContent::Text(_)) {
                        merge_chars(b, Loc::Text(child), Loc::Suffix(parent), false);
                    } else {
                        merge_chars(b, Loc::Suffix(child), Loc::Suffix(parent), false);
                    }
                    let ps = b.get(parent).string("suffix");
                    if first_char(&ps) == "." {
                        let cs = b.get(child).string("suffix");
                        b.get_mut(child).set_string("suffix", &format!("{cs}."));
                        b.get_mut(parent).set_string("suffix", drop_first(&ps));
                    }
                }
                let cs = b.get(child).string("suffix");
                let ps = b.get(parent).string("suffix");
                if last_char(&cs) == "\u{00a0}" && first_char(&ps) == " " {
                    b.get_mut(parent).set_string("suffix", drop_first(&ps));
                }
                // More duplicates control
                let cs = b.get(child).string("suffix");
                if is_punct_or_space(first_char(&cs)) {
                    if let BlobContent::Text(t) = &b.get(child).blobs {
                        if last_char(t) == first_char(&cs) {
                            // Remove parent punctuation of it duplicates the last character of a field
                            b.get_mut(child).set_string("suffix", drop_first(&cs));
                        }
                    }
                    let cs = b.get(child).string("suffix");
                    let ps = b.get(parent).string("suffix");
                    if last_char(&cs) == first_char(&ps) {
                        // Remove duplicate punctuation on child suffix
                        b.get_mut(parent).set_string("suffix", drop_last(&ps));
                    }
                }
                // Squash dupes
                let ps = b.get(parent).string("suffix");
                if match_last_char(b, parent, first_char(&ps)) {
                    b.get_mut(parent).set_string("suffix", drop_first(&ps));
                }
            } else if !b.get(parent).string("delimiter").is_empty() {
                // Remove trailing space on mid-position child node suffix if there is a leading space on delimiter above
                let d = b.get(parent).string("delimiter");
                let cs = b.get(child).string("suffix");
                if is_punct_or_space(first_char(&d)) && first_char(&d) == last_char(&cs) {
                    b.get_mut(child).set_string("suffix", drop_last(&cs));
                }
            } else {
                // Otherwise it's a sibling. We don't care about moving spaces here, just suppress a duplicate
                if let Some(sib) = kid_blob(&list[i + 1]) {
                    let cs = b.get(child).string("suffix");
                    let sp = b.get(sib).string("prefix");
                    if !blob_is_number(b, child)
                        && !child_decorations
                        && is_punct_or_space(last_char(&cs))
                        && last_char(&cs) == first_char(&sp)
                    {
                        b.get_mut(sib).set_string("prefix", drop_first(&sp));
                    }
                }
            }
            // If field content ends with swappable punctuation, suppress swappable punctuation in style suffix.
            let cs = b.get(child).string("suffix");
            if !child_is_number
                && !child_decorations
                && is_punct(first_char(&cs))
                && matches!(b.get(child).blobs, BlobContent::Text(_))
            {
                merge_chars(b, Loc::Text(child), Loc::Suffix(child), false);
            }
            self.downward(b, child);
        }
    }

    /// `swapToTheLeft(child)`.
    fn swap_to_the_left(&self, b: &mut Blobs, child: BlobId) {
        let mut child_char = first_char(&b.get(child).string("suffix")).to_string();
        if matches!(b.get(child).blobs, BlobContent::Text(_)) {
            while is_swap_in(&child_char) {
                merge_chars(b, Loc::Text(child), Loc::Suffix(child), false);
                child_char = first_char(&b.get(child).string("suffix")).to_string();
            }
        } else {
            let last = match &b.get(child).blobs {
                BlobContent::List(l) => l.last().and_then(kid_blob),
                BlobContent::Text(_) => None,
            };
            let Some(last) = last else { return };
            while is_swap_in(&child_char) {
                merge_chars(b, Loc::Suffix(last), Loc::Suffix(child), false);
                child_char = first_char(&b.get(child).string("suffix")).to_string();
            }
        }
    }

    /// `swapToTheRight(child)`.
    fn swap_to_the_right(&self, b: &mut Blobs, child: BlobId) {
        if let BlobContent::Text(t) = &b.get(child).blobs {
            let mut child_char = last_char(t).to_string();
            while is_swap_out(&child_char) {
                merge_chars(b, Loc::Text(child), Loc::Suffix(child), true);
                child_char = match &b.get(child).blobs {
                    BlobContent::Text(t) => last_char(t).to_string(),
                    BlobContent::List(_) => String::new(),
                };
            }
        } else {
            let last = match &b.get(child).blobs {
                BlobContent::List(l) => l.last().and_then(kid_blob),
                BlobContent::Text(_) => None,
            };
            let Some(last) = last else { return };
            let mut child_char = last_char(&b.get(last).string("suffix")).to_string();
            while is_swap_out(&child_char) {
                merge_chars(b, Loc::Suffix(last), Loc::Suffix(child), true);
                child_char = last_char(&b.get(last).string("suffix")).to_string();
            }
        }
    }

    /// `adjust.fix(parent)`: swap punctuation across quotation marks
    /// according to `punctuation-in-quote`. Returns the last character of
    /// the last text rendered (JS `lastChar`; `None` is `undefined`).
    pub fn fix(&self, b: &mut Blobs, parent: BlobId) -> Option<String> {
        // Terminus if no blobs
        let list = match &b.get(parent).blobs {
            BlobContent::List(l) if !l.is_empty() => l.clone(),
            _ => return None,
        };
        // Do the swap, front-to-back, bottom-first
        let mut last_char_: Option<String> = None;

        // XXX Two things to fix with this:
        // XXX (1) Stalls after one character
        // XXX (2) Moves colon and semicolon, both of which SHOULD stall
        for c in list.iter() {
            let Some(child) = kid_blob(c) else {
                last_char_ = None;
                continue;
            };
            let quote_swap = b
                .get(child)
                .decorations
                .iter()
                .any(|d| d.name == "@quotes" && d.value != "false");
            if quote_swap {
                if self.punct_in_quote {
                    self.swap_to_the_left(b, child);
                } else {
                    self.swap_to_the_right(b, child);
                }
            }
            last_char_ = self.fix(b, child);
            if let BlobContent::Text(t) = &b.get(child).blobs {
                if !t.is_empty() {
                    last_char_ = Some(last_char(t).to_string());
                }
            }
        }
        last_char_
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::citeproc::obj_blob::testing::{ser_blob, ser_child};
    use crate::citeproc::obj_number::{new_numeric_blob, NumArg, NumFormatter};
    use serde_json::json;

    /// Reference results of replaying operation sequences on citeproc-js
    /// 2.4.63's `engine.output` / `engine.dateput`, generated by
    /// `scripts/csl-units/queue.cjs` (hand-written sequences for quotes and
    /// punctuation swapping, affix/delimiter migration, append edge cases,
    /// format stores, macro-with-date, note-style braces and numeric-blob
    /// ranges, plus seeded random group trees).
    const DATA: &str = include_str!("../../tests/data/csl/units/queue.json");

    fn mk_tok(sp: &Value) -> Token {
        let name = sp["n"].as_str().unwrap_or("x");
        let mut t = Token::new(name, TokenType::Singleton);
        if let Some(s) = sp["s"].as_object() {
            for (k, v) in s {
                t.strings.insert(k.clone(), v.clone());
            }
        }
        if let Some(d) = sp["d"].as_array() {
            for x in d {
                t.decorations.push(Decoration::new(
                    x[0].as_str().unwrap(),
                    x[1].as_str().unwrap(),
                ));
            }
        }
        if let Some(x) = sp["x"].as_object() {
            for (k, v) in x {
                if k == "formatter" {
                    let f = match v.as_str().unwrap() {
                        "suffixator" => NumFormatter::suffixator(None),
                        "romanizer" => NumFormatter::Romanizer,
                        _ => NumFormatter::Default,
                    };
                    t.extra.insert("formatter".into(), f.to_value());
                } else {
                    t.extra.insert(k.clone(), v.clone());
                }
            }
        }
        t
    }

    fn fmt_ref(r: &Value) -> FormatRef {
        match r {
            Value::Null => FormatRef::None,
            Value::String(s) => FormatRef::Name(s.clone()),
            other => FormatRef::Token(mk_tok(other)),
        }
    }

    fn rendered_json(st: &State, r: &Rendered) -> Value {
        match r {
            Rendered::Str(s) => json!({ "str": s }),
            Rendered::Blob(id) => json!({ "blob": ser_blob(&st.blobs, *id, false) }),
            Rendered::List(l) => {
                json!({ "arr": l.iter().map(|x| rendered_json(st, x)).collect::<Vec<_>>() })
            }
        }
    }

    fn child_res(st: &State, c: &BlobChild) -> Value {
        match c {
            BlobChild::Str(s) => json!({ "str": s }),
            BlobChild::Blob(_) => json!({ "blob": ser_child(&st.blobs, c, false) }),
        }
    }

    fn tok_json(t: &Option<Token>) -> Value {
        match t {
            None => Value::Null,
            Some(t) => {
                let mut s = serde_json::Map::new();
                for (k, v) in &t.strings {
                    s.insert(k.clone(), v.clone());
                }
                json!({ "s": s, "d": t.decorations.iter().map(|d| json!([d.name, d.value])).collect::<Vec<_>>() })
            }
        }
    }

    fn run_op(st: &mut State, q: QueueId, op: &[Value]) -> CslResult<Value> {
        let name = op[0].as_str().unwrap();
        let tok_arg = |i: usize| op.get(i).cloned().unwrap_or(Value::Null);
        let flag = |i: usize| op.get(i).and_then(Value::as_bool).unwrap_or(false);
        Ok(match name {
            "append" => {
                let arg = match &op[1] {
                    Value::Null => AppendArg::Undefined,
                    v => AppendArg::Text(v.as_str().unwrap().to_string()),
                };
                json!(append(
                    st,
                    q,
                    arg,
                    fmt_ref(&tok_arg(2)),
                    flag(3),
                    flag(4),
                    flag(5)
                )?)
            }
            "appendBlobLiteral" => {
                let empty = st.output.empty.clone();
                let inner = Blob::new(Some(op[1].as_str().unwrap()), Some(&empty), None);
                let id = st.blobs.add(inner);
                json!(append(
                    st,
                    q,
                    AppendArg::Blob(id),
                    FormatRef::Name("literal".into()),
                    false,
                    false,
                    false
                )?)
            }
            "appendNum" => {
                let tok = mk_tok(&op[2]);
                let particle = op.get(3).and_then(Value::as_str);
                let idv = op.get(4).and_then(Value::as_str).unwrap_or("ID");
                let id = new_numeric_blob(
                    st,
                    particle,
                    NumArg::Number(op[1].as_i64().unwrap()),
                    Some(&tok),
                    Some(idv),
                )?;
                json!(append(
                    st,
                    q,
                    AppendArg::Blob(id),
                    FormatRef::Name("literal".into()),
                    false,
                    false,
                    false
                )?)
            }
            "open" => {
                open_level(st, q, fmt_ref(&tok_arg(1)))?;
                Value::Null
            }
            "close" => {
                close_level(st, q, op[1].as_str())?;
                Value::Null
            }
            "startTag" => {
                let t = if op[2].is_null() {
                    None
                } else {
                    Some(mk_tok(&op[2]))
                };
                start_tag(st, q, op[1].as_str().unwrap(), t.as_ref())?;
                Value::Null
            }
            "endTag" => {
                end_tag(st, q, op[1].as_str())?;
                Value::Null
            }
            "addToken" => {
                add_token(
                    st,
                    q,
                    op[1].as_str().unwrap(),
                    op[2].as_str(),
                    fmt_ref(&tok_arg(3)),
                );
                Value::Null
            }
            "pushFormats" => {
                let store = op.get(1).and_then(Value::as_object).map(|o| {
                    o.iter()
                        .map(|(k, v)| (k.clone(), mk_tok(v)))
                        .collect::<BTreeMap<_, _>>()
                });
                push_formats(st, q, store);
                Value::Null
            }
            "popFormats" => {
                pop_formats(st, q);
                Value::Null
            }
            "clearlevel" => {
                clearlevel(st, q)?;
                Value::Null
            }
            "pop" => match pop(st, q)? {
                Some(c) => child_res(st, &c),
                None => Value::Null,
            },
            "getToken" => tok_json(&get_token(st, q, op[1].as_str().unwrap())),
            "merge" => tok_json(&merge_token_strings(
                st,
                q,
                op[1].as_str().unwrap(),
                op[2].as_str().unwrap(),
            )),
            "purge" => {
                for c in queue_children(st, q) {
                    if let BlobChild::Blob(id) = c {
                        purge_empty_blobs(&mut st.blobs, id);
                    }
                }
                Value::Null
            }
            "adjust" => {
                let adj = st.output.adjust.ok_or_else(|| tmp_error("no adjust"))?;
                for c in queue_children(st, q) {
                    if let BlobChild::Blob(id) = c {
                        for f in op[1].as_array().unwrap() {
                            match f.as_str().unwrap() {
                                "upward" => adj.upward(&mut st.blobs, id),
                                "leftward" => adj.leftward(&mut st.blobs, id),
                                "downward" => adj.downward(&mut st.blobs, id),
                                "fix" => {
                                    adj.fix(&mut st.blobs, id);
                                }
                                other => panic!("pass {other}"),
                            }
                        }
                    }
                }
                Value::Null
            }
            "dump" => Value::Array(
                queue_children(st, q)
                    .iter()
                    .map(|c| child_res(st, c))
                    .collect(),
            ),
            "string" => {
                let kids = queue_children(st, q);
                let r = string(st, q, &kids, StringParent::None)?;
                rendered_json(st, &r)
            }
            "tmp" => {
                for (k, v) in op[1].as_object().unwrap() {
                    match k.as_str() {
                        "doing-macro-with-date" => st.tmp.doing_macro_with_date = js::truthy(v),
                        "extension" => st.tmp.extension = v.as_str().unwrap_or_default().to_string(),
                        other => panic!("tmp {other}"),
                    }
                }
                Value::Null
            }
            other => panic!("unknown op {other}"),
        })
    }

    fn run_seq(seq: &Value) -> Vec<Value> {
        let cfg = &seq["cfg"];
        let mut st = State::default();
        let piq = cfg["piq"].as_bool().unwrap_or(false);
        crate::citeproc::test_support::install_output_locale(&mut st, piq);
        crate::citeproc::formats::set_output_format(
            &mut st,
            cfg["mode"].as_str().unwrap_or("html"),
        )
        .unwrap();
        st.tmp.area = cfg["area"].as_str().unwrap_or("citation").to_string();
        st.output.adjust = Some(Adjust::new(piq));
        st.tmp.strip_periods = i64::from(js::truthy(&cfg["strip"]));
        st.tmp.just_looking = js::truthy(&cfg["just_looking"]);
        st.tmp.suppress_decorations = js::truthy(&cfg["suppress"]);
        if js::truthy(&cfg["note"]) {
            st.output.check_nested_brace = Some(CheckNestedBrace::new(true));
        }
        let q = if cfg["q"].as_str() == Some("dateput") {
            QueueId::Dateput
        } else {
            QueueId::Output
        };
        seq["ops"]
            .as_array()
            .unwrap()
            .iter()
            .map(|op| match run_op(&mut st, q, op.as_array().unwrap()) {
                Ok(v) => v,
                Err(e) => json!({ "error": e.to_string() }),
            })
            .collect()
    }

    fn same(got: &Value, want: &Value) -> bool {
        if want.get("error").is_some() {
            return got.get("error").is_some();
        }
        got == want
    }

    #[test]
    fn queue_sequences_match_citeproc_js() {
        let v: Value = serde_json::from_str(DATA).unwrap();
        let seqs = v["seqs"].as_array().unwrap();
        assert!(seqs.len() > 2000);
        let mut bad = Vec::new();
        for (n, seq) in seqs.iter().enumerate() {
            let got = run_seq(seq);
            let want = seq["res"].as_array().unwrap();
            for (i, (g, w)) in got.iter().zip(want.iter()).enumerate() {
                if !same(g, w) {
                    bad.push(format!(
                        "seq {n} op {i} {}\n  cfg {}\n  got  {g}\n  want {w}",
                        seq["ops"][i], seq["cfg"]
                    ));
                    break;
                }
            }
        }
        assert!(
            bad.is_empty(),
            "{} of {} sequences differ, first 5:\n{}",
            bad.len(),
            seqs.len(),
            bad.iter().take(5).cloned().collect::<Vec<_>>().join("\n")
        );
    }

    #[test]
    fn punctuation_tables_are_the_inverse_of_each_other() {
        assert_eq!(ltor("!", "."), Some("!"));
        assert_eq!(rtol(".", "!"), Some("!"));
        assert_eq!(ltor(".", "."), None);
        assert!(is_punct_or_space("\u{a0}"));
        assert!(!is_swap_in(":") && is_swap_in("."));
        assert!(is_swap_out(",") && !is_swap_out("."));
    }

    #[test]
    fn nested_brace_flips_in_notes() {
        let mut c = CheckNestedBrace::new(true);
        assert_eq!(c.update("(a (b))"), "(a [b])");
        let mut d = CheckNestedBrace::new(false);
        assert_eq!(d.update("(x"), "(x");
    }
}
