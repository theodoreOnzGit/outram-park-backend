// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/node_datepart.js
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

//! Port of `src/node_datepart.js`: `CSL.Node["date-part"]`.

use serde_json::Value;

use super::exec::Exec;
use super::obj_token::Token;
use super::state::State;
use super::{CslResult, EngineError};

/// The closures `src/node_datepart.js` stores in `token.execs`
/// (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeDatepartExec {
    /// The one closure of `cs:date-part`: render one date part
    /// (node_datepart.js:38-305). `date_variable` is
    /// `state.build.date_variables[0]` at build time.
    Render {
        /// `date_variable`.
        date_variable: Option<String>,
    },
}

impl NodeDatepartExec {
    /// Run the closure.
    pub fn run(
        &self,
        _state: &mut State,
        _token: &Token,
        _item: &Value,
        _cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            // PORT-LATER(wave2): node_datepart.js:38-305, needs CSL.Util.Dates
            // (util_dates.js), state.getTerm, state.output.append,
            // CSL.NumericBlob and CSL.Util.Suffixator (queue/formatters),
            // state.registry.registry (wave4) and state.tmp.group_context. The
            // build-scope locals it shares across calls (first_date, value,
            // value_end, have_collapsed, ...) are re-assigned at each call.
            NodeDatepartExec::Render { .. } => Err(EngineError::NotYetPorted {
                method: "node_datepart.js:38 closure",
            }),
        }
    }
}

/// `CSL.Node["date-part"].build.call(token, state, target)`.
pub fn build(
    state: &mut State,
    mut token: Token,
    target: &mut Vec<Token>,
    _real_group: bool,
) -> CslResult<()> {
    if !super::js::truthy_opt(token.strings.get("form")) {
        token.set_string("form", "long");
    }
    // used in node_date, to send a list of rendering date parts
    // to node_key, for dates embedded in macros.
    state.build.date_parts.push(token.string("name"));
    //
    // Set delimiter here, if poss.
    //
    let date_variable = state.build.date_variables.first().cloned();

    token
        .execs
        .push(Exec::NodeDatepart(NodeDatepartExec::Render {
            date_variable,
        }));
    target.push(token);
    Ok(())
}
