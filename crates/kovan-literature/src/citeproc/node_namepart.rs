// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/node_namepart.js
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

//! Port of `src/node_namepart.js`: `CSL.Node["name-part"]`.

use super::node_names::token_to_value;
use super::obj_token::Token;
use super::state::State;
use super::CslResult;

/// `CSL.Node["name-part"].build.call(token, state)`:
/// `state.build[this.strings.name] = this`. The token is kept as its
/// [`token_to_value`] form in `state.build.name_parts` and is consumed by
/// `cs:names` END (node_names.js), which copies it to `this.family` /
/// `this.given`. It is *not* added to the token list.
pub fn build(
    state: &mut State,
    token: Token,
    _target: &mut Vec<Token>,
    _real_group: bool,
) -> CslResult<()> {
    if let Some(name) = token.string_opt("name") {
        state
            .build
            .name_parts
            .insert(name, token_to_value(&token));
    }
    Ok(())
}
