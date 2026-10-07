// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/node_etal.js
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

//! Port of `src/node_etal.js`: `CSL.Node["et-al"]`.

use serde_json::Value;

use super::exec::Exec;
use super::node_names::token_to_value;
use super::obj_token::Token;
use super::state::State;
use super::CslResult;

/// The closures `src/node_etal.js` stores in `token.execs` (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeEtalExec {
    /// `state.tmp.etal_node = this; state.tmp.etal_term = this.strings.term`
    /// (node_etal.js:5-10).
    SetEtalNode,
}

impl NodeEtalExec {
    /// Run the closure.
    pub fn run(
        &self,
        state: &mut State,
        token: &Token,
        _item: &Value,
        _cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            NodeEtalExec::SetEtalNode => {
                state.tmp.etal_node = Some(token_to_value(token));
                if let Some(Value::String(term)) = token.strings.get("term") {
                    state.tmp.etal_term = Some(term.clone());
                }
                Ok(None)
            }
        }
    }
}

/// `CSL.Node["et-al"].build.call(token, state, target)`.
pub fn build(
    state: &mut State,
    mut token: Token,
    target: &mut Vec<Token>,
    _real_group: bool,
) -> CslResult<()> {
    if state.build.area == "citation" || state.build.area == "bibliography" {
        token.execs.push(Exec::NodeEtal(NodeEtalExec::SetEtalNode));
    }
    target.push(token);
    Ok(())
}
