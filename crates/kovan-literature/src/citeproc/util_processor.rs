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

use regex::Regex;
use serde_json::Value;

use super::js;
use super::load;
use super::formats::Format;
use super::obj_blob::BlobId;
use super::obj_token::Decoration;
use super::state::State;
use super::{CslResult, EngineError};

/// `CSL.substituteOne(template)(state, list)`: `""` for a missing or empty
/// `list`, else `template.replace("%%STRING%%", list)` with JS's `$`-pattern
/// handling of the replacement text.
pub fn substitute_one(template: &str, list: Option<&str>) -> String {
    match list {
        Some(l) if !l.is_empty() => super::formats::js_replace_first(template, "%%STRING%%", l),
        _ => String::new(),
    }
}

/// `CSL.Mode(mode)`: the object `state.fun.decorate` holds. Upstream builds it
/// from the table `CSL.Output.Formats[mode]` (src/formats.js); here the table
/// is the enum [`Format`] (formats.rs, which also carries the per-entry
/// behaviour, `substituteOne`/`substituteTwo` templates included). An unknown
/// `mode` is an `Err` (upstream would iterate `undefined`).
pub fn mode(mode: &str) -> CslResult<Format> {
    Format::from_mode(mode)
        .ok_or_else(|| EngineError::Csl(format!("unknown output format: {mode}")))
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
    /// `CSL.Engine.prototype.normalDecorIsOrphan(blob, params)`
    /// (util_processor.js:161): true when `params` is a `normal` decoration
    /// with no non-`normal` decoration of the same attribute on the blob or
    /// its ancestors (so it can be dropped). `blob` is an arena id.
    pub fn normal_decor_is_orphan(&self, blob: BlobId, params: &Decoration) -> bool {
        if params.value == "normal" {
            let mut use_param = false;
            let mut all: Vec<Vec<Decoration>> = Vec::new();
            if self.tmp.area == "citation" {
                all.push(
                    self.citation
                        .opt
                        .get("layout_decorations")
                        .and_then(super::queue::decorations_from_value)
                        .unwrap_or_default(),
                );
            }
            all.extend(self.blobs.get(blob).alldecor.iter().cloned());
            for set in all.iter().rev() {
                for d in set.iter().rev() {
                    if d.name == params.name && d.value != "normal" {
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
