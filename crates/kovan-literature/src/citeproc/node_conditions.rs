// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/node_conditions.js
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

//! Port of `src/node_conditions.js`: `CSL.Node["conditions"]`.

use super::obj_token::{Token, TokenType};
use super::state::State;
use super::{CslResult, EngineError};

fn engine(state: &State) -> CslResult<super::util_conditions::ConditionsEngine> {
    state
        .tmp
        .conditions
        .clone()
        .ok_or_else(|| EngineError::Csl("TypeError: state.tmp.conditions is undefined".into()))
}

/// `CSL.Node["conditions"].build.call(token, state)`: `cs:conditions START`
/// records its `match` on the enclosing `cs:if`; `END` combines the collected
/// tests.
pub fn build(
    state: &mut State,
    token: Token,
    target: &mut Vec<Token>,
    _real_group: bool,
) -> CslResult<()> {
    if token.tokentype == TokenType::Start {
        engine(state)?.add_match(target, token.extra.get("match"))?;
    }
    if token.tokentype == TokenType::End {
        engine(state)?.match_combine(target)?;
    }
    Ok(())
}
