// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/node_citation.js
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

//! Port of `src/node_citation.js`: `CSL.Node.citation.build`.

use serde_json::Value;

use super::exec::Exec;
use super::js;
use super::obj_token::{Token, TokenType};
use super::registry::Comparifier;
use super::state::State;
use super::CslResult;

/// The closures `src/node_citation.js` stores in `token.execs`
/// (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeCitationExec {
    /// Sets `state.tmp.area/root/extension` for the citation
    /// (node_citation.js:11-15).
    SetArea,
}

impl NodeCitationExec {
    /// Run the closure.
    pub fn run(
        &self,
        state: &mut State,
        _token: &Token,
        _item: &Value,
        _cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            NodeCitationExec::SetArea => {
                state.tmp.area = "citation".to_string();
                state.tmp.root = "citation".to_string();
                state.tmp.extension = String::new();
                Ok(None)
            }
        }
    }
}

/// JS `x && x.length` for a string or array `x`; `None` is `undefined`.
fn and_length(x: Option<&Value>) -> Option<Value> {
    match x {
        None => None,
        Some(v) if js::truthy(v) => match v {
            Value::String(s) => Some(Value::from(js::len(s))),
            Value::Array(a) => Some(Value::from(a.len())),
            _ => None,
        },
        Some(v) => Some(v.clone()),
    }
}

/// `CSL.Node.citation.build.call(token, state, target)`.
pub fn build(
    state: &mut State,
    mut token: Token,
    target: &mut Vec<Token>,
    _real_group: Option<bool>,
) -> CslResult<()> {
    if token.tokentype == TokenType::Start {
        state.build.area = "citation".to_string();
        state.build.root = "citation".to_string();
        state.build.extension = String::new();

        token
            .execs
            .push(Exec::NodeCitation(NodeCitationExec::SetArea));

        state.build.root = "citation".to_string();
    }
    if token.tokentype == TokenType::End {
        // Open an extra key at first position for use in grouped sorts.
        //
        // JS: state.opt.xclass === "in-text" && (collapse && collapse.length)
        //     || (cite_group_delimiter && cite_group_delimiter.length)
        //        && update_mode !== POSITION && update_mode !== NUMERIC;
        // i.e. (A && B) || (C && D && E), with JS's value-returning && and ||.
        let in_text = state.opt.get("xclass").and_then(Value::as_str) == Some("in-text");
        let b = and_length(state.citation.opt.get("collapse"));
        let ab: Option<Value> = if in_text { b } else { Some(Value::Bool(false)) };
        let update_mode = state.opt.get("update_mode").and_then(Value::as_i64);
        let c = and_length(state.citation.opt.get("cite_group_delimiter"));
        let cde: Option<Value> = if !c.as_ref().map(js::truthy).unwrap_or(false) {
            c
        } else if update_mode == Some(super::load::POSITION) {
            Some(Value::Bool(false))
        } else {
            Some(Value::Bool(update_mode != Some(super::load::NUMERIC)))
        };
        let grouped_sort = if ab.as_ref().map(js::truthy).unwrap_or(false) {
            ab
        } else {
            cde
        };
        match &grouped_sort {
            Some(v) => {
                state.opt.insert("grouped_sort".into(), v.clone());
            }
            None => {
                state.opt.remove("grouped_sort");
            }
        }

        let has_directions = state
            .citation_sort
            .opt
            .get("sort_directions")
            .and_then(Value::as_array)
            .map(|a| !a.is_empty())
            .unwrap_or(false);
        if grouped_sort.as_ref().map(js::truthy).unwrap_or(false) && has_directions {
            if let Some(Value::Array(dirs)) = state.citation_sort.opt.get_mut("sort_directions") {
                let firstkey = dirs[0].clone();
                dirs.insert(0, firstkey);
            }
        }
        // PORT-LATER(wave4): node_citation.js:60, `new CSL.Registry.Comparifier(
        // state, "citation_sort")`; needs registry.js Comparifier constructor.
        state.citation.srt = Some(Comparifier::default());
    }
    target.push(token);
    Ok(())
}
