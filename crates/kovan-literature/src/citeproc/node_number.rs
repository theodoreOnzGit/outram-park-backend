// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/node_number.js
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

//! Port of `src/node_number.js`: `CSL.Node.number`.

use serde_json::Value;

use super::exec::Exec;
use super::obj_token::Token;
use super::state::State;
use super::util_substitute;
use super::{CslResult, EngineError};

use super::attributes::area_ref;

/// The closures `src/node_number.js` stores in `token.execs` (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeNumberExec {
    /// The one closure of `cs:number`: push a number or text to the output
    /// (node_number.js:41-130).
    Render,
}

impl NodeNumberExec {
    /// Run the closure.
    pub fn run(
        &self,
        _state: &mut State,
        _token: &mut Token,
        _item: &Value,
        _cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            // PORT-LATER(wave2): node_number.js:41-130, needs state.processNumber
            // (util_number.js), CSL.Util.outputNumericField, state.output.append,
            // state.tmp.group_context. The formatter processNumber reads from
            // the node is `token.extra["formatter"]` (a name: "romanizer",
            // "ordinalizer", "long_ordinalizer") -> state.fun.<name>.
            NodeNumberExec::Render => Err(EngineError::NotYetPorted {
                method: "node_number.js:41 closure",
            }),
        }
    }
}

/// `CSL.Node.number.build.call(token, state, target)`.
pub fn build(
    state: &mut State,
    mut token: Token,
    target: &mut Vec<Token>,
    _real_group: bool,
) -> CslResult<()> {
    util_substitute::substitute_start(state, &mut token, target)?;
    //
    // This should push a rangeable object to the queue.
    //
    // The formatter is a function upstream; the intermediate dump records its
    // name, so the name is the token property here.
    let formatter = match token.string_opt("form").as_deref() {
        Some("roman") => Some("romanizer"),
        Some("ordinal") => Some("ordinalizer"),
        Some("long-ordinal") => Some("long_ordinalizer"),
        _ => None,
    };
    if let Some(f) = formatter {
        token
            .extra
            .insert("formatter".into(), serde_json::Value::String(f.to_string()));
    }
    let layout_delimiter = area_ref(state, &state.build.area.clone())?
        .opt
        .get("layout_delimiter")
        .cloned();
    if !token.extra.contains_key("successor_prefix") {
        if let Some(d) = &layout_delimiter {
            token.extra.insert("successor_prefix".into(), d.clone());
        }
    }
    if !token.extra.contains_key("splice_prefix") {
        if let Some(d) = &layout_delimiter {
            token.extra.insert("splice_prefix".into(), d.clone());
        }
    }
    //
    // Whether we actually stick a number object on
    // the output queue depends on whether the field
    // contains a pure number.
    //
    // push number or text
    token.execs.push(Exec::NodeNumber(NodeNumberExec::Render));
    // target.push(this); CSL.Util.substituteEnd.call(this, state, target);
    util_substitute::push_with_substitute_end(state, token, target, true)
}
