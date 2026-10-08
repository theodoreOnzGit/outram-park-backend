// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/node_choose.js
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

//! Port of `src/node_choose.js`: `CSL.Node.choose`.

use serde_json::Value;

use super::exec::Exec;
use super::obj_token::{Token, TokenType};
use super::state::State;
use super::CslResult;

/// The closures `src/node_choose.js` stores in `token.execs` (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeChooseExec {
    /// START, "open condition": `state.tmp.jump.push(undefined, CSL.LITERAL)`
    /// (node_choose.js:6-8).
    Open,
    /// END, "close condition": `state.tmp.jump.pop()` (node_choose.js:12-14).
    Close,
    /// A SINGLETON `cs:choose`: upstream pushes the unassigned `func`, i.e.
    /// `undefined` (node_choose.js:16). Counted, never meaningfully run.
    Undefined,
}

impl NodeChooseExec {
    /// Run the closure.
    pub fn run(
        &self,
        state: &mut State,
        _token: &mut Token,
        _item: &Value,
        _cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            NodeChooseExec::Open => {
                state.tmp.jump.push_literal(Value::Null);
                Ok(None)
            }
            NodeChooseExec::Close => {
                state.tmp.jump.pop();
                Ok(None)
            }
            NodeChooseExec::Undefined => Ok(None),
        }
    }
}

/// `CSL.Node.choose.build.call(token, state, target)`.
pub fn build(
    _state: &mut State,
    mut token: Token,
    target: &mut Vec<Token>,
    _real_group: Option<bool>,
) -> CslResult<()> {
    let func = match token.tokentype {
        TokenType::Start => NodeChooseExec::Open,
        TokenType::End => NodeChooseExec::Close,
        TokenType::Singleton => NodeChooseExec::Undefined,
    };
    token.execs.push(Exec::NodeChoose(func));
    target.push(token);
    Ok(())
}

/// `CSL.Node.choose.configure.call(tokens[pos], state, pos)`: the back-to-front
/// jump-index pass (`configureTokenList`).
pub fn configure(state: &mut State, tokens: &mut [Token], pos: usize) -> CslResult<()> {
    let end = tokens
        .get(pos)
        .map(|t| t.tokentype == TokenType::End)
        .unwrap_or(false);
    if end {
        state.configure.fail.push(pos);
        state.configure.succeed.push(pos);
    } else {
        state.configure.fail.pop();
        state.configure.succeed.pop();
    }
    Ok(())
}
