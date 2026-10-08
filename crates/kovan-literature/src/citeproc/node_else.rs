// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/node_else.js
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

//! Port of `src/node_else.js`: `CSL.Node["else"]`.

use super::obj_token::{Token, TokenType};
use super::state::State;
use super::CslResult;

/// `CSL.Node["else"].build.call(token, state, target)`.
pub fn build(
    _state: &mut State,
    token: Token,
    target: &mut Vec<Token>,
    _real_group: Option<bool>,
) -> CslResult<()> {
    target.push(token);
    Ok(())
}

/// `CSL.Node["else"].configure.call(tokens[pos], state, pos)`: the
/// back-to-front jump-index pass (`configureTokenList`).
pub fn configure(state: &mut State, tokens: &mut [Token], pos: usize) -> CslResult<()> {
    let start = tokens
        .get(pos)
        .map(|t| t.tokentype == TokenType::Start)
        .unwrap_or(false);
    if start {
        if let Some(last) = state.configure.fail.last_mut() {
            *last = pos;
        }
    }
    Ok(())
}
