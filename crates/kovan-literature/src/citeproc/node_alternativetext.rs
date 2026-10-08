// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/node_alternativetext.js
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

//! Port of `src/node_alternativetext.js`: `CSL.Node["alternative-text"]`.

use serde_json::Value;

use super::exec::Exec;
use super::obj_token::{Token, TokenType};
use super::state::State;
use super::{CslResult, EngineError};

/// The closures `src/node_alternativetext.js` stores in `token.execs`
/// (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeAlternativetextExec {
    /// `Item = state.refetchItem(Item.id); CSL.getCite.call(state, Item)`
    /// (node_alternativetext.js:5-8).
    RefetchAndGetCite,
}

impl NodeAlternativetextExec {
    /// Run the closure.
    pub fn run(
        &self,
        _state: &mut State,
        _token: &mut Token,
        _item: &Value,
        _cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            // PORT-LATER(wave2): node_alternativetext.js:5-8, needs
            // state.refetchItem (build.js) and CSL.getCite (api_cite.js).
            NodeAlternativetextExec::RefetchAndGetCite => Err(EngineError::NotYetPorted {
                method: "node_alternativetext.js:5 closure",
            }),
        }
    }
}

/// `CSL.Node["alternative-text"].build.call(token, state, target)`.
pub fn build(
    _state: &mut State,
    mut token: Token,
    target: &mut Vec<Token>,
    _real_group: bool,
) -> CslResult<()> {
    if token.tokentype == TokenType::Singleton {
        // do stuff
        token.execs.push(Exec::NodeAlternativetext(
            NodeAlternativetextExec::RefetchAndGetCite,
        ));
    }
    target.push(token);
    Ok(())
}
