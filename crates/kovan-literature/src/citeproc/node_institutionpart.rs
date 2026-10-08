// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/node_institutionpart.js
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

//! Port of `src/node_institutionpart.js`: `CSL.Node["institution-part"]`.

use serde_json::Value;

use super::exec::Exec;
use super::obj_token::Token;
use super::state::State;
use super::CslResult;

/// The closures `src/node_institutionpart.js` stores in `token.execs`
/// (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeInstitutionpartExec {
    /// `strings.name === "long"` with `if-short`:
    /// `state.nameOutput.institutionpart["long-with-short"] = this`.
    LongWithShort,
    /// `strings.name === "long"`: `institutionpart["long"] = this`.
    Long,
    /// `strings.name === "short"`: `institutionpart["short"] = this`.
    Short,
    /// Any other `name`: upstream pushes the unassigned `func` (`undefined`).
    Undefined,
}

impl NodeInstitutionpartExec {
    /// Run the closure.
    pub fn run(
        &self,
        state: &mut State,
        token: &mut Token,
        _item: &Value,
        _cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            NodeInstitutionpartExec::LongWithShort => {
                state.name_output.institutionpart.long_with_short = Some(token.clone());
                Ok(None)
            }
            NodeInstitutionpartExec::Long => {
                state.name_output.institutionpart.long = Some(token.clone());
                Ok(None)
            }
            NodeInstitutionpartExec::Short => {
                state.name_output.institutionpart.short = Some(token.clone());
                Ok(None)
            }
            NodeInstitutionpartExec::Undefined => Ok(None),
        }
    }
}

/// `CSL.Node["institution-part"].build.call(token, state, target)`.
pub fn build(
    _state: &mut State,
    mut token: Token,
    target: &mut Vec<Token>,
    _real_group: Option<bool>,
) -> CslResult<()> {
    let name = token.string_opt("name");
    let func = if name.as_deref() == Some("long") {
        if super::js::truthy_opt(token.strings.get("if-short")) {
            NodeInstitutionpartExec::LongWithShort
        } else {
            NodeInstitutionpartExec::Long
        }
    } else if name.as_deref() == Some("short") {
        NodeInstitutionpartExec::Short
    } else {
        NodeInstitutionpartExec::Undefined
    };
    token.execs.push(Exec::NodeInstitutionpart(func));
    target.push(token);
    Ok(())
}
