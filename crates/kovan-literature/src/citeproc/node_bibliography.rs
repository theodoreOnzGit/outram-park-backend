// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/node_bibliography.js
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

//! Port of `src/node_bibliography.js`: `CSL.Node.bibliography.build`.

use serde_json::Value;

use super::exec::Exec;
use super::obj_token::{Token, TokenType};
use super::state::State;
use super::CslResult;

/// The closures `src/node_bibliography.js` stores in `token.execs`
/// (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeBibliographyExec {
    /// Sets `state.tmp.area/root/extension` for the bibliography
    /// (node_bibliography.js:11-15).
    SetArea,
}

impl NodeBibliographyExec {
    /// Run the closure.
    pub fn run(
        &self,
        state: &mut State,
        _token: &Token,
        _item: &Value,
        _cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            NodeBibliographyExec::SetArea => {
                state.tmp.area = "bibliography".to_string();
                state.tmp.root = Some("bibliography".to_string());
                state.tmp.extension = String::new();
                Ok(None)
            }
        }
    }
}

/// `CSL.Node.bibliography.build.call(token, state, target)`.
pub fn build(
    state: &mut State,
    mut token: Token,
    target: &mut Vec<Token>,
    _real_group: bool,
) -> CslResult<()> {
    if token.tokentype == TokenType::Start {
        state.build.area = "bibliography".to_string();
        state.build.root = "bibliography".to_string();
        state.build.extension = String::new();
        token
            .execs
            .push(Exec::NodeBibliography(NodeBibliographyExec::SetArea));
    }
    target.push(token);
    Ok(())
}
