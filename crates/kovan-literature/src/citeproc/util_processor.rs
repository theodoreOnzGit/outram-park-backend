// Part of the kovan port of citeproc-js (GitHub #790, #792).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_processor.js
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

//! Output-mode plumbing (src/util_processor.js): `CSL.substituteOne`,
//! `CSL.substituteTwo`, `CSL.Mode`, `CSL.setDecorations`, `CSL.Doppeler`
//! and `CSL.Engine.prototype.normalDecorIsOrphan`.
//!
//! # `state.fun.decorate`
//!
//! `CSL.Mode(mode)` turns the output format table `CSL.Output.Formats[mode]`
//! (src/formats.js, the output agent's `formats.rs`) into the object the
//! queue calls as `state.fun.decorate[name][value].call(blob, state, str,
//! cslid)`. Its leaves are either the template functions this file
//! builds (`substituteOne`, `substituteTwo`), or the format's own functions
//! (`@quotes/true`, `@display/block`, `text_escape`, ...) which only
//! `formats.rs` can run. [`Decorate`] keeps both: [`DecorFn::Function`] names
//! a format function by its key in the format table.
//!
//! PORT-LATER(formats): [`output_formats`] is the seam to
//! `CSL.Output.Formats[mode]`; until `formats.rs` provides the table it
//! returns `None` (upstream: iterating `undefined` yields an empty
//! `decorate`).

use std::collections::BTreeMap;

use regex::Regex;
use serde_json::Value;

use super::js;
use super::load;
use super::obj_blob::Blob;
use super::obj_token::Decoration;
use super::state::State;
use super::{CslResult, EngineError};

/// One value of the format table `CSL.Output.Formats[mode]`.
#[derive(Debug, Clone, PartialEq)]
pub enum FormatEntry {
    /// A string: a template containing `%%STRING%%` (and perhaps
    /// `%%PARAM%%`), or plain text such as `bibstart`.
    Str(String),
    /// The boolean `false` (`"@font-weight/light": false`).
    False,
    /// A function; the string names it (its key in the table).
    Function(String),
}

/// A leaf of [`Decorate`]: what `decorations[a][b]` holds.
#[derive(Debug, Clone, PartialEq)]
pub enum DecorFn {
    /// `CSL.substituteOne(template)`: replace `%%STRING%%` in the template.
    Substitute1 { template: String },
    /// `CSL.substituteTwo(template)`: a function of a parameter that returns
    /// a [`DecorFn::Substitute1`] with `%%PARAM%%` filled in.
    Substitute2 { template: String },
    /// `CSL.Output.Formatters.passthrough`.
    Passthrough,
    /// A function of the format table, by key.
    Function(String),
}

impl DecorFn {
    /// Call the leaf on `list` (`function (state, list)`) for the kinds
    /// that this file builds; `None` for [`DecorFn::Function`] (needs
    /// `formats.rs`) and [`DecorFn::Substitute2`] (needs a parameter first,
    /// see [`DecorFn::with_param`]).
    pub fn apply(&self, list: &str) -> Option<String> {
        match self {
            DecorFn::Substitute1 { template } => Some(substitute_one(template, list)),
            DecorFn::Passthrough => Some(list.to_string()),
            _ => None,
        }
    }

    /// `substituteTwo(template)(param)`: the one-parameter function's result.
    pub fn with_param(&self, param: &str) -> Option<DecorFn> {
        match self {
            DecorFn::Substitute2 { template } => Some(DecorFn::Substitute1 {
                template: template.replacen("%%PARAM%%", param, 1),
            }),
            _ => None,
        }
    }
}

/// `CSL.substituteOne(template)(state, list)`: `""` for a falsy `list`, else
/// the template with its first `%%STRING%%` replaced.
pub fn substitute_one(template: &str, list: &str) -> String {
    if list.is_empty() {
        String::new()
    } else {
        template.replacen("%%STRING%%", list, 1)
    }
}

/// One top-level entry of [`Decorate`].
#[derive(Debug, Clone, PartialEq)]
pub enum DecorEntry {
    /// A non-`@` entry that is text: `bibstart`, `bibend`.
    Text(String),
    /// A non-`@` entry that is a function (`text_escape`), or an `@name`
    /// with no `/value` part.
    Func(DecorFn),
    /// `@name/value` entries: `decorations[name][value]`.
    Group(BTreeMap<String, DecorFn>),
}

/// `CSL.Mode(mode)`'s result, `state.fun.decorate`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Decorate {
    /// The entries by name (`bibstart`, `text_escape`, `@font-style` → ...,
    /// stored without the `@`).
    pub entries: BTreeMap<String, DecorEntry>,
}

impl Decorate {
    /// `decorate[name]` when it is text (`bibstart`, `bibend`).
    pub fn text(&self, name: &str) -> Option<&str> {
        match self.entries.get(name) {
            Some(DecorEntry::Text(s)) => Some(s),
            _ => None,
        }
    }

    /// `decorate[name][value]`.
    pub fn leaf(&self, name: &str, value: &str) -> Option<&DecorFn> {
        match self.entries.get(name) {
            Some(DecorEntry::Group(g)) => g.get(value),
            _ => None,
        }
    }
}

/// The seam to `CSL.Output.Formats[mode]` (src/formats.js, the output
/// agent's `formats.rs`): the format table, in insertion order, or `None`
/// when there is no such format. PORT-LATER(formats): returns `None`.
pub fn output_formats(_mode: &str) -> Option<Vec<(String, FormatEntry)>> {
    None
}

/// `CSL.Mode(mode)` over an explicit format table (see [`output_formats`]):
/// `@a/b` entries become `decorate[a][b]` leaves, other entries are copied.
/// An entry that is neither a template, `false`, nor a function throws in
/// upstream (`Bad <mode> config entry`).
pub fn mode_from_formats(mode: &str, params: &[(String, FormatEntry)]) -> CslResult<Decorate> {
    let mut decorations: BTreeMap<String, DecorEntry> = BTreeMap::new();
    for (param, val) in params {
        if !param.starts_with('@') {
            let entry = match val {
                FormatEntry::Str(s) => DecorEntry::Text(s.clone()),
                FormatEntry::False => DecorEntry::Text("false".to_string()),
                FormatEntry::Function(f) => DecorEntry::Func(DecorFn::Function(f.clone())),
            };
            decorations.insert(param.clone(), entry);
            continue;
        }
        let func = match val {
            FormatEntry::Str(s) if s.contains("%%STRING%%") => {
                if s.contains("%%PARAM%%") {
                    DecorFn::Substitute2 {
                        template: s.clone(),
                    }
                } else {
                    DecorFn::Substitute1 {
                        template: s.clone(),
                    }
                }
            }
            FormatEntry::False => DecorFn::Passthrough,
            FormatEntry::Function(f) => DecorFn::Function(f.clone()),
            FormatEntry::Str(s) => {
                return Err(EngineError::Csl(format!(
                    "Bad {mode} config entry for {param}: {s}"
                )))
            }
        };
        let args: Vec<&str> = param.split('/').collect();
        if args.len() == 1 {
            decorations.insert(args[0].to_string(), DecorEntry::Func(func));
        } else if args.len() == 2 {
            let group = decorations
                .entry(args[0].to_string())
                .or_insert_with(|| DecorEntry::Group(BTreeMap::new()));
            if let DecorEntry::Group(g) = group {
                g.insert(args[1].to_string(), func);
            }
        }
    }
    Ok(Decorate {
        entries: decorations,
    })
}

/// `CSL.Mode(mode)`: [`mode_from_formats`] over [`output_formats`].
pub fn mode(mode: &str) -> CslResult<Decorate> {
    match output_formats(mode) {
        Some(params) => mode_from_formats(mode, &params),
        None => Ok(Decorate::default()),
    }
}

/// `CSL.setDecorations.call(token, state, attributes)`: the formatting
/// attributes of a node as `[key, value]` pairs in `CSL.FORMAT_KEY_SEQUENCE`
/// order; each one found (and truthy) is **removed** from `attributes`, so
/// the attribute loop of `CSL.XmlToToken` does not run it again.
pub fn set_decorations(_state: &State, attributes: &mut Vec<(String, Value)>) -> Vec<Decoration> {
    let mut ret = Vec::new();
    // This applies a fixed processing sequence
    for key in load::FORMAT_KEY_SEQUENCE {
        if let Some(pos) = attributes
            .iter()
            .position(|(k, v)| k == key && js::truthy(v))
        {
            let (k, v) = attributes.remove(pos);
            ret.push(Decoration::new(&k, &js::to_js_string(&v)));
        }
    }
    ret
}

/// `new CSL.Doppeler(rexStr, stringMangler)`: splits a string at the matches
/// of a markup pattern, keeping the tags, and joins it back.
#[derive(Debug, Clone)]
pub struct Doppeler {
    match_rex: Regex,
    split_rex: Regex,
    string_mangler: Option<fn(&str) -> String>,
}

/// What `Doppeler.split` returns: `{tags, strings, origStrings}`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DoppelerSplit {
    /// `tags`: the matches, in order.
    pub tags: Vec<String>,
    /// `strings`: the text between them (`tags.len() + 1` pieces).
    pub strings: Vec<String>,
    /// `origStrings`: a copy of `strings` taken before apostrophe handling
    /// (empty when nothing matched, as `undefined` upstream).
    pub orig_strings: Vec<String>,
}

impl Doppeler {
    /// `new CSL.Doppeler(rexStr, stringMangler)`. `rex_str` is a pattern in
    /// `regex` syntax (JS lookaround is not available: the caller adapts it).
    pub fn new(rex_str: &str, string_mangler: Option<fn(&str) -> String>) -> CslResult<Doppeler> {
        let match_rex = Regex::new(&format!("({rex_str})"))
            .map_err(|e| EngineError::Csl(format!("SyntaxError: {e}")))?;
        let split_rex =
            Regex::new(rex_str).map_err(|e| EngineError::Csl(format!("SyntaxError: {e}")))?;
        Ok(Doppeler {
            match_rex,
            split_rex,
            string_mangler,
        })
    }

    /// `this.split(str)`.
    pub fn split(&self, s: &str) -> DoppelerSplit {
        let s = match self.string_mangler {
            Some(f) => f(s),
            None => s.to_string(),
        };
        let mut tags: Vec<String> = self
            .match_rex
            .find_iter(&s)
            .map(|m| m.as_str().to_string())
            .collect();
        if tags.is_empty() {
            return DoppelerSplit {
                tags: Vec::new(),
                strings: vec![s],
                orig_strings: Vec::new(),
            };
        }
        let mut split: Vec<String> = js::split_with_captures(&self.split_rex, &s)
            .into_iter()
            .map(Option::unwrap_or_default)
            .collect();
        for i in (0..tags.len()).rev() {
            let tag = tags[i].clone();
            if tag == "'" && split.get(i + 1).map(|x| !x.is_empty()).unwrap_or(false) {
                // Fixes https://forums.zotero.org/discussion/comment/294317
                split[i + 1] = format!("{}{}", tags[i], split[i + 1]);
                tags[i] = String::new();
            }
        }
        // origStrings is taken after the loop upstream (`split.slice()` in the
        // returned literal), so it already holds the apostrophe moves.
        DoppelerSplit {
            tags,
            orig_strings: split.clone(),
            strings: split,
        }
    }

    /// `this.join(obj)`: `strings[0] tags[0] strings[1] tags[1] ... strings[n]`.
    pub fn join(&self, obj: &DoppelerSplit) -> String {
        let mut lst: Vec<&str> = Vec::new();
        if let Some(last) = obj.strings.last() {
            lst.push(last);
        }
        for i in (0..obj.tags.len()).rev() {
            lst.push(&obj.tags[i]);
            lst.push(obj.strings.get(i).map(String::as_str).unwrap_or(""));
        }
        lst.reverse();
        lst.concat()
    }
}

impl State {
    /// `CSL.Engine.prototype.normalDecorIsOrphan(blob, params)`: whether a
    /// `normal` decoration (`params = [name, "normal"]`) has no enclosing
    /// decoration of the same name that is not `normal`, and so can be
    /// dropped.
    pub fn normal_decor_is_orphan(&self, blob: &Blob, params: &[String]) -> bool {
        if params.get(1).map(String::as_str) == Some("normal") {
            let mut use_param = false;
            let mut all_the_decor: Vec<Vec<(String, String)>> = Vec::new();
            if self.tmp.area == "citation" {
                let layout: Vec<(String, String)> =
                    match self.citation.opt.get("layout_decorations") {
                        Some(Value::Array(a)) => a
                            .iter()
                            .map(|d| {
                                (
                                    d.get(0).map(js::to_js_string).unwrap_or_default(),
                                    d.get(1).map(js::to_js_string).unwrap_or_default(),
                                )
                            })
                            .collect(),
                        _ => Vec::new(),
                    };
                all_the_decor.push(layout);
            }
            for d in &blob.alldecor {
                all_the_decor.push(
                    d.iter()
                        .map(|x| (x.name.clone(), x.value.clone()))
                        .collect(),
                );
            }
            for k in (0..all_the_decor.len()).rev() {
                for n in (0..all_the_decor[k].len()).rev() {
                    if all_the_decor[k][n].0 == params[0] && all_the_decor[k][n].1 != "normal" {
                        use_param = true;
                    }
                }
            }
            if !use_param {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_builds_group_and_text_entries() {
        let params = vec![
            ("bibstart".to_string(), FormatEntry::Str("<div>".into())),
            (
                "text_escape".to_string(),
                FormatEntry::Function("text_escape".into()),
            ),
            (
                "@font-style/italic".to_string(),
                FormatEntry::Str("<i>%%STRING%%</i>".into()),
            ),
            ("@font-weight/light".to_string(), FormatEntry::False),
            (
                "@a/b".to_string(),
                FormatEntry::Str("%%PARAM%%:%%STRING%%".into()),
            ),
        ];
        let d = mode_from_formats("html", &params).unwrap();
        assert_eq!(d.text("bibstart"), Some("<div>"));
        assert_eq!(
            d.leaf("@font-style", "italic")
                .unwrap()
                .apply("x")
                .as_deref(),
            Some("<i>x</i>")
        );
        assert_eq!(d.leaf("@font-weight", "light"), Some(&DecorFn::Passthrough));
        let two = d.leaf("@a", "b").unwrap().with_param("P").unwrap();
        assert_eq!(two.apply("S").as_deref(), Some("P:S"));
        assert_eq!(two.apply("").as_deref(), Some(""));
        assert!(
            mode_from_formats("html", &[("@x/y".into(), FormatEntry::Str("no".into()))]).is_err()
        );
    }

    #[test]
    fn set_decorations_removes_what_it_takes() {
        let st = State::default();
        let mut attrs = vec![
            ("@font-weight".to_string(), Value::String("bold".into())),
            ("@variable".to_string(), Value::String("title".into())),
            ("@font-style".to_string(), Value::String("italic".into())),
            ("@quotes".to_string(), Value::String("".into())),
        ];
        let d = set_decorations(&st, &mut attrs);
        let names: Vec<_> = d.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, vec!["@font-style", "@font-weight"]);
        assert_eq!(
            attrs.len(),
            2,
            "empty @quotes stays, as upstream's `if (attributes[key])`"
        );
    }

    #[test]
    fn doppeler_splits_and_joins() {
        let d = Doppeler::new("<i>|</i>", None).unwrap();
        let s = d.split("a <i>b</i> c");
        assert_eq!(s.tags, vec!["<i>", "</i>"]);
        assert_eq!(s.strings, vec!["a ", "b", " c"]);
        assert_eq!(d.join(&s), "a <i>b</i> c");
        assert_eq!(d.split("plain").strings, vec!["plain"]);
    }
}
