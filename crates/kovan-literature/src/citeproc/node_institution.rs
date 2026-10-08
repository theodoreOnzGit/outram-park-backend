// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/node_institution.js
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

//! Port of `src/node_institution.js`: `CSL.Node.institution`.

use serde_json::Value;

use super::exec::Exec;
use super::obj_token::{Token, TokenType};
use super::state::State;
use super::{CslResult, EngineError};

/// The closures `src/node_institution.js` stores in `token.execs`
/// (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeInstitutionExec {
    /// START / SINGLETON: compute the institution delimiter, `and` term and
    /// prefixes, build the `and` blobs and register the node on
    /// `state.nameOutput` (node_institution.js:5-76).
    Setup,
}

impl NodeInstitutionExec {
    /// Run the closure.
    pub fn run(
        &self,
        _state: &mut State,
        _token: &mut Token,
        _item: &Value,
        _cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            // PORT-LATER(wave3): node_institution.js:5-76, needs state.getTerm,
            // state.output.append/pop and CSL.Blob (queue.rs), CSL.NameOutput
            // (state.nameOutput.institution = this), and it writes
            // this.and_term / and_prefix_* / and on the running token.
            NodeInstitutionExec::Setup => Err(EngineError::NotYetPorted {
                method: "node_institution.js:5 closure",
            }),
        }
    }
}

/// `CSL.Node.institution.build.call(token, state, target)`.
pub fn build(
    _state: &mut State,
    mut token: Token,
    target: &mut Vec<Token>,
    _real_group: bool,
) -> CslResult<()> {
    if token.tokentype == TokenType::Singleton || token.tokentype == TokenType::Start {
        token
            .execs
            .push(Exec::NodeInstitution(NodeInstitutionExec::Setup));
    }
    target.push(token);
    Ok(())
}

/// `CSL.Node.institution.configure.call(tokens[pos], state, pos)`.
pub fn configure(state: &mut State, tokens: &mut [Token], pos: usize) -> CslResult<()> {
    let is_open = tokens
        .get(pos)
        .map(|t| t.tokentype == TokenType::Singleton || t.tokentype == TokenType::Start)
        .unwrap_or(false);
    if is_open {
        state.build.has_institution = true;
    }
    Ok(())
}
