// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_flipflop.js
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

//! `CSL.Util.FlipFlopper`: parses the markup and quotes inside a text blob
//! (`<i>`, `<b>`, `<sc>`, `<sup>`, `<sub>`, `<span class="nocase">`,
//! `<span class="nodecor">`, small-caps spans, straight and curly quotes,
//! apostrophes) and replaces the blob's text with a tree of styled child
//! blobs, flipping the style against what already surrounds it.
//!
//! Entry point: [`process_tags`] (`state.fun.flipflopper.processTags(blob)`).
//!
//! The JS object is a closure with private state that persists between
//! calls; [`FlipFlopper`] is that state. In particular `_nestingData[...]
//! .outer` for the two straight-quote forms is **mutated** by every call that
//! sees an opening quote (`_setOuterQuoteForm`), and stays mutated.
//!
//! # Integration points
//!
//! * `new CSL.Util.FlipFlopper(state)` reads the four quote terms with
//!   `state.getTerm`; [`FlipFlopper::new`] does that through
//!   [`formats::get_term`](super::formats::get_term) (`State::get_term_no_flag`). If `state.fun.flipflopper` was never built,
//!   [`process_tags`] builds it on first use.
//! * `state[state.tmp.area].opt.layout_decorations` is read through
//!   [`queue::layout_decorations`](super::queue::layout_decorations).

use std::sync::LazyLock;

use regex::Regex;

use super::formats::get_term;
use super::load::ROMANESQUE_REGEXP;
use super::obj_blob::{Blob, BlobChild, BlobContent, BlobId, JS_WS_CLASS};
use super::obj_token::{Decoration, Token};
use super::queue::layout_decorations;
use super::state::State;
use super::{CslResult, EngineError};

/// `_nestingData[...].type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NType {
    Nocase,
    Nodecor,
    Tag,
    Quote,
}

/// One value of `_nestingData` (shared by its aliases).
#[derive(Debug, Clone)]
struct NestEntry {
    ntype: NType,
    closer: String,
    attr: Option<String>,
    outer: Option<String>,
    flipflop: Vec<(String, String)>,
    /// `_nestingParams[...].opener`: the regexp `getNestingOpenerParams`
    /// overwrote the opener string with.
    opener_rex: Regex,
}

/// One entry of `_nestingState`.
#[derive(Debug, Clone)]
struct NestState {
    ntype: NType,
    opener: Regex,
    closer: String,
    pos: usize,
}

/// Everything the constructor computes.
#[derive(Debug, Clone)]
struct Built {
    entries: Vec<NestEntry>,
    /// `Object.keys(_nestingData)` in insertion order, with the entry index.
    keys: Vec<(String, usize)>,
    match_all: Regex,
    split_all: Regex,
    open: Regex,
    close: Regex,
}

/// `CSL.Util.FlipFlopper` (`state.fun.flipflopper`). `Default` is the
/// unbuilt state; see the module docs.
#[derive(Debug, Clone, Default)]
pub struct FlipFlopper {
    built: Option<Built>,
    nesting_state: Vec<NestState>,
}

fn re(p: &str) -> Regex {
    #[allow(clippy::expect_used)]
    Regex::new(p).expect("constant regex")
}

impl FlipFlopper {
    /// `new CSL.Util.FlipFlopper(state)`.
    pub fn new(state: &State) -> FlipFlopper {
        let mut ff = FlipFlopper::default();
        ff.build(state);
        ff
    }

    fn build(&mut self, state: &State) {
        let ff_i = |pairs: &[(&str, &str)]| -> Vec<(String, String)> {
            pairs
                .iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect()
        };
        let quote_ff = ff_i(&[("true", "inner"), ("inner", "true"), ("false", "true")]);
        // (key, type, closer, attr, outer, flipflop)
        struct Base {
            key: &'static str,
            ntype: NType,
            closer: &'static str,
            attr: Option<&'static str>,
            outer: Option<&'static str>,
            flipflop: Vec<(String, String)>,
        }
        let bases: Vec<Base> = vec![
            Base {
                key: "<span class=\"nocase\">",
                ntype: NType::Nocase,
                closer: "</span>",
                attr: None,
                outer: None,
                flipflop: vec![],
            },
            Base {
                key: "<span class=\"nodecor\">",
                ntype: NType::Nodecor,
                closer: "</span>",
                attr: Some("@class"),
                outer: Some("nodecor"),
                flipflop: ff_i(&[("nodecor", "nodecor")]),
            },
            Base {
                key: "<span style=\"font-variant:small-caps;\">",
                ntype: NType::Tag,
                closer: "</span>",
                attr: Some("@font-variant"),
                outer: Some("small-caps"),
                flipflop: ff_i(&[("small-caps", "normal"), ("normal", "small-caps")]),
            },
            Base {
                key: "<sc>",
                ntype: NType::Tag,
                closer: "</sc>",
                attr: Some("@font-variant"),
                outer: Some("small-caps"),
                flipflop: ff_i(&[("small-caps", "normal"), ("normal", "small-caps")]),
            },
            Base {
                key: "<i>",
                ntype: NType::Tag,
                closer: "</i>",
                attr: Some("@font-style"),
                outer: Some("italic"),
                // DEVIATION(D3): upstream has no `oblique` entry, so `<i>` inside an oblique context crashes; flip it to normal, as for italic.
                flipflop: ff_i(&[
                    ("italic", "normal"),
                    ("oblique", "normal"),
                    ("normal", "italic"),
                ]),
            },
            Base {
                key: "<b>",
                ntype: NType::Tag,
                closer: "</b>",
                attr: Some("@font-weight"),
                outer: Some("bold"),
                // DEVIATION(D3): upstream has no `light` entry, so `<b>` inside a light context crashes; light is not bold, so the toggle adds bold.
                flipflop: ff_i(&[("bold", "normal"), ("light", "bold"), ("normal", "bold")]),
            },
            Base {
                key: "<sup>",
                ntype: NType::Tag,
                closer: "</sup>",
                attr: Some("@vertical-align"),
                outer: Some("sup"),
                flipflop: ff_i(&[("sub", "sup"), ("sup", "sup")]),
            },
            Base {
                key: "<sub>",
                ntype: NType::Tag,
                closer: "</sub>",
                attr: Some("@vertical-align"),
                outer: Some("sub"),
                flipflop: ff_i(&[("sup", "sub"), ("sub", "sub")]),
            },
            Base {
                key: " \"",
                ntype: NType::Quote,
                closer: "\"",
                attr: Some("@quotes"),
                outer: Some("true"),
                flipflop: quote_ff.clone(),
            },
            Base {
                key: " '",
                ntype: NType::Quote,
                closer: "'",
                attr: Some("@quotes"),
                outer: Some("inner"),
                flipflop: quote_ff.clone(),
            },
        ];
        // Entries are built with a placeholder opener regexp and fixed up
        // below (getNestingOpenerParams).
        let placeholder = re("^(?:)");
        let mut entries: Vec<NestEntry> = Vec::new();
        let mut keys: Vec<(String, usize)> = Vec::new();
        for b in &bases {
            entries.push(NestEntry {
                ntype: b.ntype,
                closer: b.closer.to_string(),
                attr: b.attr.map(str::to_string),
                outer: b.outer.map(str::to_string),
                flipflop: b.flipflop.clone(),
                opener_rex: placeholder.clone(),
            });
            keys.push((b.key.to_string(), entries.len() - 1));
        }
        // _nestingData["(\""] = _nestingData[" \""]; likewise for single.
        keys.push(("(\"".to_string(), 8));
        keys.push(("('".to_string(), 9));

        let local_open = get_term(state, "open-quote");
        let local_close = get_term(state, "close-quote");
        let local_open_inner = get_term(state, "open-inner-quote");
        let local_close_inner = get_term(state, "close-inner-quote");
        let straight = [" \"", " '", "\"", "'"];
        let add_clone = |open: &str,
                         close: &str,
                         from: usize,
                         keys: &mut Vec<(String, usize)>,
                         entries: &mut Vec<NestEntry>| {
            let mut e = entries[from].clone();
            e.closer = close.to_string();
            entries.push(e);
            let idx = entries.len() - 1;
            if let Some(k) = keys.iter_mut().find(|(k, _)| k == open) {
                k.1 = idx;
            } else {
                keys.push((open.to_string(), idx));
            }
        };
        // If locale uses straight quotes, do not register them.
        if !local_open.is_empty()
            && !local_close.is_empty()
            && !straight.contains(&local_open.as_str())
        {
            add_clone(&local_open, &local_close, 8, &mut keys, &mut entries);
        }
        if !local_open_inner.is_empty()
            && !local_close_inner.is_empty()
            && !straight.contains(&local_open_inner.as_str())
        {
            add_clone(
                &local_open_inner,
                &local_close_inner,
                9,
                &mut keys,
                &mut entries,
            );
        }

        // _getNestingOpenerParams for every key, in order. The entry's opener
        // becomes ^(?:<all keys>) unless it is a quote, then ^(?:).
        let all_keys = keys
            .iter()
            .map(|(k, _)| regex::escape(k))
            .collect::<Vec<_>>()
            .join("|");
        let non_quote_rex = re(&format!("^(?:{all_keys})"));
        for (_, idx) in keys.clone() {
            entries[idx].opener_rex = if entries[idx].ntype != NType::Quote {
                non_quote_rex.clone()
            } else {
                re("^(?:)")
            };
        }

        // _tagRex
        let mut closers: Vec<String> = Vec::new();
        for (_, idx) in &keys {
            let c = entries[*idx].closer.clone();
            if !closers.contains(&c) {
                closers.push(c);
            }
        }
        let openers_alt = all_keys;
        let closers_alt = closers
            .iter()
            .map(|c| regex::escape(c))
            .collect::<Vec<_>>()
            .join("|");
        let all = format!("{openers_alt}|{closers_alt}");
        self.built = Some(Built {
            entries,
            keys,
            match_all: re(&format!("((?:{all}))")),
            split_all: re(&format!("(?:{all})")),
            open: re(&format!("(^(?:{openers_alt})$)")),
            close: re(&format!("(^(?:{closers_alt})$)")),
        });
        self.nesting_state.clear();
    }
}

impl Built {
    fn entry_for(&self, tag: &str) -> Option<&NestEntry> {
        self.keys
            .iter()
            .find(|(k, _)| k == tag)
            .map(|(_, i)| &self.entries[*i])
    }

    fn entry_index(&self, tag: &str) -> Option<usize> {
        self.keys.iter().find(|(k, _)| k == tag).map(|(_, i)| *i)
    }
}

/// The result of `_doppelString`.
#[derive(Debug, Clone, Default)]
struct DoppelStr {
    tags: Vec<String>,
    strings: Vec<String>,
    /// Pushed once per tag that is a `_nestingData` key (so its indices do
    /// NOT line up with `tags`; upstream indexes it as if they did).
    forced_spaces: Vec<bool>,
}

static RE_SMALLCAPS: LazyLock<Regex> = LazyLock::new(|| {
    re(&format!(
        r#"(<span)[{ws}]+(style="font-variant:)[{ws}]*(small-caps);?"[^>]*(>)"#,
        ws = JS_WS_CLASS
    ))
});
static RE_NOCASE: LazyLock<Regex> = LazyLock::new(|| {
    re(&format!(
        r#"(<span)[{ws}]+(class="no(?:case|decor)")[^>]*(>)"#,
        ws = JS_WS_CLASS
    ))
});
static RE_LEADING_WS_QUOTE: LazyLock<Regex> =
    LazyLock::new(|| re(&format!("^[{ws}]+['\"]", ws = JS_WS_CLASS)));
static RE_APOSTROPHE: LazyLock<Regex> = LazyLock::new(|| {
    re(&format!(
        "({r})\u{2019}({r})",
        r = ROMANESQUE_REGEXP.as_str()
    ))
});

/// `_doppelString(str)`.
fn doppel_string(b: &Built, s: &str) -> DoppelStr {
    // Normalize markup
    let s = RE_SMALLCAPS
        .replace_all(s, "${1} ${2}${3};\"${4}")
        .into_owned();
    let s = RE_NOCASE.replace_all(&s, "${1} ${2}${3}").into_owned();
    let mut tags: Vec<String> = b
        .match_all
        .find_iter(&s)
        .map(|m| m.as_str().to_string())
        .collect();
    if tags.is_empty() {
        return DoppelStr {
            tags: Vec::new(),
            strings: vec![s],
            forced_spaces: Vec::new(),
        };
    }
    let strings: Vec<String> = b.split_all.split(&s).map(str::to_string).collect();
    let mut forced_spaces = Vec::new();
    let ilen = tags.len() - 1;
    for i in 0..ilen {
        if b.entry_for(&tags[i]).is_some() {
            if strings[i + 1].is_empty() && (tags[i + 1] == "\"" || tags[i + 1] == "'") {
                tags[i + 1] = format!(" {}", tags[i + 1]);
                forced_spaces.push(true);
            } else {
                forced_spaces.push(false);
            }
        }
    }
    DoppelStr {
        tags,
        strings,
        forced_spaces,
    }
}

/// The result of `_tryOpen` / `_tryClose`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TagInfo {
    /// JS `false`.
    False,
    /// `{fixtag: pos}`.
    Fix(usize),
    /// `{nocase: {open, close}}`.
    Nocase(usize, usize),
}

impl FlipFlopper {
    fn built(&self) -> CslResult<&Built> {
        self.built
            .as_ref()
            .ok_or_else(|| EngineError::Csl("flipflopper not built".into()))
    }

    /// `_tryOpen(tag, pos)`.
    fn try_open(&mut self, tag: &str, pos: usize) -> CslResult<TagInfo> {
        let (ntype, opener, closer) = {
            let b = self.built()?;
            let e = b
                .entry_for(tag)
                .ok_or_else(|| EngineError::Csl(format!("no nesting data for {tag}")))?;
            (e.ntype, e.opener_rex.clone(), e.closer.clone())
        };
        let params = self.nesting_state.last().cloned();
        let ok = match &params {
            None => true,
            Some(p) => p.opener.is_match(tag),
        };
        let st = NestState {
            ntype,
            opener,
            closer,
            pos,
        };
        if ok {
            self.nesting_state.push(st);
            Ok(TagInfo::False)
        } else {
            self.nesting_state.pop();
            self.nesting_state.push(st);
            Ok(TagInfo::Fix(params.map(|p| p.pos).unwrap_or(0)))
        }
    }

    /// `_tryClose(tag, pos)`.
    fn try_close(&mut self, tag: &str, pos: usize) -> TagInfo {
        match self.nesting_state.last().cloned() {
            Some(p) if tag == p.closer => {
                self.nesting_state.pop();
                if p.ntype == NType::Nocase {
                    TagInfo::Nocase(p.pos, pos)
                } else {
                    TagInfo::False
                }
            }
            Some(p) => TagInfo::Fix(p.pos),
            None => TagInfo::Fix(pos),
        }
    }

    /// `_pushNestingState(tag, pos)`.
    fn push_nesting_state(&mut self, tag: &str, pos: usize) -> CslResult<TagInfo> {
        let is_open = self.built()?.open.is_match(tag);
        if is_open {
            self.try_open(tag, pos)
        } else {
            Ok(self.try_close(tag, pos))
        }
    }

    /// `_setOuterQuoteForm(quot)`.
    fn set_outer_quote_form(&mut self, quot: &str) -> CslResult<()> {
        let flip = match quot {
            " '" => " \"",
            " \"" => " '",
            "(\"" => "('",
            "('" => "(\"",
            _ => return Ok(()),
        };
        let b = self
            .built
            .as_mut()
            .ok_or_else(|| EngineError::Csl("flipflopper not built".into()))?;
        if let Some(i) = b.entry_index(quot) {
            b.entries[i].outer = Some("true".into());
        }
        if let Some(i) = b.entry_index(flip) {
            b.entries[i].outer = Some("inner".into());
        }
        Ok(())
    }
}

/// `_apostropheForce(tag, str)`: the replacement text or `None` (JS `false`).
fn apostrophe_force(tag: &str, s: &str) -> Option<&'static str> {
    if tag == "'" {
        if let Some(c) = s.chars().next() {
            if !matches!(c, ',' | '.' | '?' | ':' | ';' | ' ') {
                return Some("\u{2019}");
            }
        }
    } else if tag == " '" && s.starts_with(' ') {
        return Some(" \u{2019}");
    }
    None
}

/// `TagReg`: the per-blob stack of the decorations opened so far.
struct TagReg {
    stack: Vec<(String, Option<String>)>,
}

impl TagReg {
    /// `TagReg.set(tag)`.
    fn set(&mut self, b: &Built, state: &State, blob_alldecor: &[Vec<Decoration>], tag: &str) {
        let Some(entry) = b.entry_for(tag) else {
            return;
        };
        let attr = entry.attr.clone().unwrap_or_default();
        let mut decor: Option<(String, Option<String>)> = None;
        for d in self.stack.iter().rev() {
            if d.0 == attr {
                decor = Some(d.clone());
                break;
            }
        }
        if decor.is_none() {
            let mut all: Vec<Vec<Decoration>> = vec![layout_decorations(state).unwrap_or_default()];
            all.extend(blob_alldecor.iter().cloned());
            'outer: for set in all.iter().rev() {
                for d in set.iter().rev() {
                    if d.name == attr {
                        decor = Some((d.name.clone(), Some(d.value.clone())));
                        break 'outer;
                    }
                }
            }
        }
        let new = match decor {
            None => (attr, entry.outer.clone()),
            Some((_, v)) => {
                let flipped = v.and_then(|v| {
                    entry
                        .flipflop
                        .iter()
                        .find(|(k, _)| *k == v)
                        .map(|(_, to)| to.clone())
                });
                (attr, flipped)
            }
        };
        self.stack.push(new);
    }

    /// `TagReg.pair()`.
    fn pair(&self) -> Option<(String, Option<String>)> {
        self.stack.last().cloned()
    }

    /// `TagReg.pop()`.
    fn pop(&mut self) {
        self.stack.pop();
    }
}

/// `new CSL.Blob()` (no string, no token): empty list with empty
/// prefix/suffix/delimiter, `alldecor` as given.
fn new_empty_blob(state: &mut State, alldecor: Vec<Vec<Decoration>>) -> BlobId {
    let mut b = Blob::new(None, None, None);
    b.alldecor = alldecor;
    state.blobs.add(b)
}

/// `new CSL.Blob(null, new CSL.Token())`.
fn new_tokened_blob(state: &mut State, alldecor: Vec<Vec<Decoration>>) -> BlobId {
    let tok = Token::new("", super::obj_token::TokenType::Start);
    let mut b = Blob::new(None, Some(&tok), None);
    b.alldecor = alldecor;
    state.blobs.add(b)
}

/// `latest.blobs.push(child)` (a plain array push: no `alldecor` change).
fn array_push(state: &mut State, parent: BlobId, child: BlobId) {
    if let BlobContent::List(l) = &mut state.blobs.get_mut(parent).blobs {
        l.push(BlobChild::Blob(child));
    }
}

/// `Stack` (the one inside `_undoppelToQueue`).
struct StyleStack {
    stack: Vec<BlobId>,
    first_string: bool,
}

impl StyleStack {
    /// `Stack.addStyling(str, decor)`.
    fn add_styling(
        &mut self,
        state: &mut State,
        mut s: String,
        decor: Option<(String, Option<String>)>,
    ) {
        if self.first_string {
            if s.starts_with(' ') {
                s.remove(0);
            }
            if s.starts_with(' ') {
                s.remove(0);
            }
            self.first_string = false;
        }
        let mut latest = match self.stack.last() {
            Some(l) => *l,
            None => return,
        };
        if let Some(decor) = decor {
            // if ("string" === typeof this.latest.blobs): wrap the text
            if let BlobContent::Text(t) = state.blobs.get(latest).blobs.clone() {
                let alld = state.blobs.get(latest).alldecor.clone();
                let child = new_empty_blob(state, alld);
                state.blobs.get_mut(child).blobs = BlobContent::Text(t);
                state.blobs.get_mut(latest).blobs = BlobContent::List(vec![BlobChild::Blob(child)]);
            }
            let latest_alld = state.blobs.get(latest).alldecor.clone();
            let newblob = new_tokened_blob(state, latest_alld);
            if decor.0 == "@class" && decor.1.as_deref() == Some("nodecor") {
                let mut newdecorset: Vec<Decoration> = Vec::new();
                let mut seen: Vec<String> = Vec::new();
                let mut all: Vec<Vec<Decoration>> =
                    vec![layout_decorations(state).unwrap_or_default()];
                all.extend(state.blobs.get(newblob).alldecor.iter().cloned());
                for set in all.iter().rev() {
                    for old in set.iter().rev() {
                        if ["@font-weight", "@font-style", "@font-variant"]
                            .contains(&old.name.as_str())
                            && !seen.contains(&old.name)
                        {
                            // `decor[1] !== "normal"` is always true here
                            // (decor[1] is "nodecor").
                            let nd = Decoration::new(&old.name, "normal");
                            state.blobs.get_mut(newblob).decorations.push(nd.clone());
                            newdecorset.push(nd);
                            seen.push(old.name.clone());
                        }
                    }
                }
                state.blobs.get_mut(newblob).alldecor.push(newdecorset);
            } else {
                let d = Decoration::new(&decor.0, decor.1.as_deref().unwrap_or("undefined"));
                state.blobs.get_mut(newblob).decorations.push(d.clone());
                state.blobs.get_mut(newblob).alldecor.push(vec![d]);
            }
            array_push(state, latest, newblob);
            self.stack.push(newblob);
            latest = newblob;
            if !s.is_empty() {
                let alld = state.blobs.get(latest).alldecor.clone();
                let nb = new_tokened_blob(state, alld);
                state.blobs.get_mut(nb).blobs = BlobContent::Text(s);
                array_push(state, latest, nb);
            }
        } else if !s.is_empty() {
            let alld = state.blobs.get(latest).alldecor.clone();
            let child = new_empty_blob(state, alld);
            state.blobs.get_mut(child).blobs = BlobContent::Text(s);
            array_push(state, latest, child);
        }
    }

    /// `Stack.popStyling()`.
    fn pop_styling(&mut self) {
        self.stack.pop();
    }
}

/// `_undoppelToQueue(blob, doppel, leadingSpace)`.
fn undoppel_to_queue(
    ff: &FlipFlopper,
    state: &mut State,
    blob: BlobId,
    doppel: &DoppelStr,
    leading_space: bool,
) -> CslResult<()> {
    let b = ff.built()?;
    let blob_alldecor = state.blobs.get(blob).alldecor.clone();
    let mut tag_reg = TagReg { stack: Vec::new() };
    state.blobs.get_mut(blob).blobs = BlobContent::List(Vec::new());
    let mut stack = StyleStack {
        stack: vec![blob],
        first_string: true,
    };
    if !doppel.strings.is_empty() {
        let mut s = doppel.strings[0].clone();
        if leading_space {
            s = format!(" {s}");
        }
        stack.add_styling(state, s, None);
    }
    for i in 0..doppel.tags.len() {
        let tag = &doppel.tags[i];
        let s = doppel.strings[i + 1].clone();
        if b.open.is_match(tag) {
            tag_reg.set(b, state, &blob_alldecor, tag);
            stack.add_styling(state, s, tag_reg.pair());
        } else {
            tag_reg.pop();
            stack.pop_styling();
            stack.add_styling(state, s, None);
        }
    }
    Ok(())
}

/// `state.fun.flipflopper.processTags(blob)`: rewrite the text blob `blob`
/// (a leaf) into a tree of styled blobs. A text without any markup or quote
/// tag is left untouched. `blob`'s text must be a leaf string.
pub fn process_tags(state: &mut State, blob: BlobId) -> CslResult<()> {
    let mut ff = std::mem::take(&mut state.fun.flipflopper);
    if ff.built.is_none() {
        ff.build(state);
    }
    let r = process_tags_with(&mut ff, state, blob);
    if r.is_err() {
        ff.nesting_state.clear();
    }
    state.fun.flipflopper = ff;
    r
}

fn process_tags_with(ff: &mut FlipFlopper, state: &mut State, blob: BlobId) -> CslResult<()> {
    let text = match &state.blobs.get(blob).blobs {
        BlobContent::Text(t) => t.clone(),
        BlobContent::List(_) => {
            return Err(EngineError::Csl(
                "processTags: blob has no string content".into(),
            ))
        }
    };
    let leading_space = text.starts_with(' ') && !RE_LEADING_WS_QUOTE.is_match(&text);
    let s = format!(" {}", RE_APOSTROPHE.replace_all(&text, "${1}'${2}"));
    let mut doppel = doppel_string(ff.built()?, &s);
    if doppel.tags.is_empty() {
        return Ok(());
    }
    let mut quote_form_seen = false;

    let ilen = doppel.tags.len();
    for i in 0..ilen {
        let tag = doppel.tags[i].clone();
        let s = doppel.strings[i + 1].clone();
        if let Some(apostrophe) = apostrophe_force(&tag, &s) {
            doppel.strings[i + 1] = format!("{apostrophe}{}", doppel.strings[i + 1]);
            doppel.tags[i] = String::new();
        } else {
            let mut tag_info;
            loop {
                tag_info = ff.push_nesting_state(&tag, i)?;
                match tag_info {
                    TagInfo::Fix(fixtag) => {
                        if ff.built()?.close.is_match(&tag) && tag == "'" {
                            doppel.strings[i + 1] = format!("\u{2019}{}", doppel.strings[i + 1]);
                            doppel.tags[i] = String::new();
                        } else {
                            let mut failed_tag = doppel.tags[fixtag].clone();
                            let forced = fixtag
                                .checked_sub(1)
                                .and_then(|k| doppel.forced_spaces.get(k))
                                .copied()
                                .unwrap_or(false);
                            if forced {
                                failed_tag = super::obj_blob::drop_first(&failed_tag).to_string();
                            }
                            doppel.strings[fixtag + 1] =
                                format!("{failed_tag}{}", doppel.strings[fixtag + 1]);
                            doppel.tags[fixtag] = String::new();
                        }
                        if !ff.nesting_state.is_empty() {
                            if tag != "'" {
                                ff.nesting_state.pop();
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                    TagInfo::Nocase(open, close) => {
                        doppel.tags[open] = String::new();
                        doppel.tags[close] = String::new();
                        break;
                    }
                    TagInfo::False => break,
                }
            }
            if let TagInfo::Fix(_) = tag_info {
                doppel.strings[i + 1] = format!("{}{}", doppel.tags[i], doppel.strings[i + 1]);
                doppel.tags[i] = String::new();
            }
        }
    }
    // Stray tags are neutralized here
    while let Some(st) = ff.nesting_state.last().cloned() {
        let tag_pos = st.pos;
        let tag = doppel.tags[tag_pos].clone();
        if tag == " '" || tag == "'" {
            doppel.strings[tag_pos + 1] = format!(" \u{2019}{}", doppel.strings[tag_pos + 1]);
        } else {
            doppel.strings[tag_pos + 1] =
                format!("{}{}", doppel.tags[tag_pos], doppel.strings[tag_pos + 1]);
        }
        doppel.tags[tag_pos] = String::new();
        ff.nesting_state.pop();
    }
    for i in (0..doppel.tags.len()).rev() {
        if doppel.tags[i].is_empty() {
            doppel.tags.remove(i);
            let next = doppel.strings.remove(i + 1);
            doppel.strings[i].push_str(&next);
        }
    }
    // Sniff initial (outer) quote form (single or double) and configure parser
    // Also add leading spaces.
    for i in 0..doppel.tags.len() {
        let tag = doppel.tags[i].clone();
        let forced_space = i
            .checked_sub(1)
            .and_then(|k| doppel.forced_spaces.get(k))
            .copied()
            .unwrap_or(false);
        if [" \"", " '", "(\"", "('"].contains(&tag.as_str()) {
            if !quote_form_seen {
                ff.set_outer_quote_form(&tag)?;
                quote_form_seen = true;
            }
            if !forced_space {
                let first = super::obj_blob::first_char(&tag).to_string();
                doppel.strings[i].push_str(&first);
            }
        }
    }
    undoppel_to_queue(ff, state, blob, &doppel, leading_space)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::citeproc::obj_blob::testing::ser_blob;
    use serde_json::Value;

    /// Reference trees from citeproc-js 2.4.63's `processTags`, generated by
    /// `scripts/csl-units/flipflop.cjs`: edge cases, the CSL test suite's
    /// INPUT strings and 1500 generated mixtures of tags, quotes and
    /// apostrophes, each under a plain context and a rotating decorated one
    /// (blob decorations and `layout_decorations`). The cases run in order on
    /// one engine because the flip-flopper keeps its outer-quote state.
    const DATA: &str = include_str!("../../tests/data/csl/units/flipflop.json");

    fn decor_list(v: &Value) -> Vec<Decoration> {
        v.as_array()
            .map(|a| {
                a.iter()
                    .map(|d| {
                        Decoration::new(d[0].as_str().unwrap_or(""), d[1].as_str().unwrap_or(""))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn process_tags_matches_citeproc_js() {
        let v: Value = serde_json::from_str(DATA).unwrap();
        let cases = v["cases"].as_array().unwrap();
        assert!(cases.len() > 4000);
        let mut st = State::default();
        crate::citeproc::test_support::install_output_locale(&mut st, false);
        st.tmp.area = "citation".to_string();
        let empty = Token::new("empty", crate::citeproc::obj_token::TokenType::Start);
        let mut deviations = 0;
        let mut bad = Vec::new();
        for c in cases {
            let s = c["s"].as_str().unwrap();
            let mut blob = Blob::new(Some(s), Some(&empty), None);
            if !c["d"].is_null() {
                blob.decorations = decor_list(&c["d"]);
                blob.alldecor = vec![blob.decorations.clone()];
            }
            if c["l"].is_null() {
                st.citation.opt.remove("layout_decorations");
            } else {
                st.citation
                    .opt
                    .insert("layout_decorations".into(), c["l"].clone());
            }
            let id = st.blobs.add(blob);
            let r = process_tags(&mut st, id);
            let got = match r {
                Ok(()) => ser_blob(&st.blobs, id, true),
                Err(e) => serde_json::json!({ "error": e.to_string() }),
            };
            // citeproc-js can produce a decoration whose value is `undefined`
            // (an `oblique` outer style flipped by `<i>`); decorations here
            // hold strings, so that value reads "undefined" (it would make
            // `decorate` fail either way).
            let want: Value =
                serde_json::from_str(&c["out"].to_string().replace(",null]", ",\"undefined\"]"))
                    .unwrap();
            // DEVIATION(D3): citeproc-js flips `<i>` inside an `oblique` context to
            // `undefined` (above); the port flips it to `normal`, so such a tree
            // differs by design. Pin that it is exactly the oblique contexts, that
            // the port produces no `undefined`, and count them.
            if want.to_string().contains("\"undefined\"") {
                let ctx = format!("{} {}", c["d"], c["l"]);
                assert!(
                    ctx.contains("oblique") || ctx.contains("light"),
                    "{s:?}: {ctx}"
                );
                assert!(!got.to_string().contains("undefined"), "{s:?}: {got}");
                assert!(got.get("error").is_none(), "{s:?}: {got}");
                deviations += 1;
                st.blobs.clear();
                continue;
            }
            if got != want {
                bad.push(format!(
                    "{s:?} ctx d={} l={}\n   got  {}\n   want {}",
                    c["d"], c["l"], got, want
                ));
            }
            st.blobs.clear();
        }
        assert_eq!(deviations, 4, "D3: reference trees with an undefined flip");
        assert!(
            bad.is_empty(),
            "{} mismatches, first 6:\n{}",
            bad.len(),
            bad.iter().take(6).cloned().collect::<Vec<_>>().join("\n")
        );
    }
}

/// Registered deviation D3 (`<i>` inside `oblique`, `<b>` inside `light`),
/// GitHub #801.
///
/// **Methodology.** `scripts/csl-units/deviations_dqa.cjs` renders a layout with
/// `font-style="oblique"` and the title `a <i>b</i> c`, and a layout with
/// `font-weight="light"` and the title `a <b>b</b> c`, with citeproc-js 2.4.63
/// (`CITEPROC_MODULE=.../citeproc_commonjs.js node
/// scripts/csl-units/deviations_dqa.cjs`; recorded in
/// `tests/data/csl/units/deviations_dqa.json`). Controls: `bold` + `<b>` and
/// `italic` + `<i>`, which flip to `normal` in both engines.
///
/// **Results (2026-10-08).** citeproc-js throws `Cannot read properties of
/// undefined (reading 'call')` for both probes (the flip table has no
/// `oblique` or `light` entry, so the decorator lookup is on `undefined`); the
/// port gave "no html decorator for @font-style/undefined" before D3 and now
/// gives `<em>a <span style="font-style:normal;">b</span> c</em>` and
/// `a <b>b</b> c` (the html format has no wrapper for `light`).
#[cfg(test)]
mod deviation_tests {
    use std::sync::Arc;

    use serde_json::{json, Value};

    use crate::citeproc::test_support::minimal_locales;
    use crate::citeproc::{CitationItem, Engine, Sys};

    const REF: &str = include_str!("../../tests/data/csl/units/deviations_dqa.json");

    fn render(case: &Value, items: &[Value]) -> Result<String, String> {
        let sys = Sys::new(items, Arc::new(minimal_locales())).map_err(|e| e.to_string())?;
        let mut e = Engine::new(sys, case["style"].as_str().unwrap_or(""), "en-US")
            .map_err(|e| e.to_string())?;
        let cite =
            CitationItem::from_json(&json!({"id": case["id"]})).map_err(|e| e.to_string())?;
        e.make_citation_cluster(&[cite]).map_err(|e| e.to_string())
    }

    #[test]
    fn italic_in_oblique_and_bold_in_light_flip_instead_of_crashing() {
        let r: Value = serde_json::from_str(REF).expect("deviations_dqa.json");
        let items: Vec<Value> = r["items"].as_array().cloned().expect("items");
        // (case, citeproc-js output or error, intended output)
        let table: [(&str, Result<&str, &str>, &str); 4] = [
            (
                "oblique-i",
                Err("Cannot read properties of undefined (reading 'call')"),
                "<em>a <span style=\"font-style:normal;\">b</span> c</em>",
            ),
            (
                "light-b",
                Err("Cannot read properties of undefined (reading 'call')"),
                "a <b>b</b> c",
            ),
            (
                "bold-b",
                Ok("<b>a <span style=\"font-weight:normal;\">b</span> c</b>"),
                "<b>a <span style=\"font-weight:normal;\">b</span> c</b>",
            ),
            (
                "italic-i",
                Ok("<i>a <span style=\"font-style:normal;\">b</span> c</i>"),
                "<i>a <span style=\"font-style:normal;\">b</span> c</i>",
            ),
        ];
        for (name, js, intended) in table {
            let case = r["cases"]
                .as_array()
                .and_then(|a| a.iter().find(|c| c["name"] == name))
                .unwrap_or_else(|| panic!("no probe {name}"));
            match js {
                Ok(v) => assert_eq!(case["js"]["v"].as_str(), Some(v), "{name}: citeproc-js"),
                Err(e) => assert_eq!(case["js"]["e"].as_str(), Some(e), "{name}: citeproc-js"),
            }
            assert_eq!(
                render(case, &items).unwrap_or_else(|e| panic!("{name}: {e}")),
                intended,
                "{name} (D3): port output"
            );
        }
    }
}
