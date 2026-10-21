// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/node_condition.js
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

//! Port of `src/node_condition.js`: `CSL.Node["condition"]`.

use super::obj_token::{Token, TokenType};
use super::state::State;
use super::util_conditions;
use super::{CslResult, EngineError};

/// `CSL.Node["condition"].build.call(token, state)`: a SINGLETON
/// `cs:condition` inside `cs:conditions` combines its own tests with its
/// `match` and hands the result to the enclosing `cs:if`'s condition engine.
pub fn build(
    state: &mut State,
    token: Token,
    target: &mut Vec<Token>,
    _real_group: Option<bool>,
) -> CslResult<()> {
    if token.tokentype == TokenType::Singleton {
        let test = util_conditions::match_combine(state, &token, &token.tests)?;
        let engine = state.tmp.conditions.clone().ok_or_else(|| {
            EngineError::Csl("TypeError: state.tmp.conditions is undefined".into())
        })?;
        engine.add_test(target, test)?;
    }
    Ok(())
}
