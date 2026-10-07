// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/node_intext.js
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

//! Port of `src/node_intext.js`: `CSL.Node.intext.build`.

use serde_json::Value;

use super::exec::Exec;
use super::obj_token::{Token, TokenType};
use super::state::State;
use super::CslResult;

/// The closures `src/node_intext.js` stores in `token.execs` (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeIntextExec {
    /// Sets `state.tmp.area/root/extension` for the in-text area
    /// (node_intext.js:11-15).
    SetArea,
}

impl NodeIntextExec {
    /// Run the closure.
    pub fn run(
        &self,
        state: &mut State,
        _token: &Token,
        _item: &Value,
        _cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            NodeIntextExec::SetArea => {
                state.tmp.area = "intext".to_string();
                state.tmp.root = Some("intext".to_string());
                state.tmp.extension = String::new();
                Ok(None)
            }
        }
    }
}

/// `CSL.Node.intext.build.call(token, state, target)`.
pub fn build(
    state: &mut State,
    mut token: Token,
    target: &mut Vec<Token>,
    _real_group: bool,
) -> CslResult<()> {
    if token.tokentype == TokenType::Start {
        state.build.area = "intext".to_string();
        state.build.root = "intext".to_string();
        state.build.extension = String::new();

        token.execs.push(Exec::NodeIntext(NodeIntextExec::SetArea));
    }
    if token.tokentype == TokenType::End {
        // Do whatever cs:citation does with sorting.
        // JS shares the array by reference; a copy is equivalent here because
        // nothing mutates either afterwards.
        let dirs = state.citation_sort.opt.get("sort_directions").cloned();
        state.intext_sort = Default::default();
        if let Some(d) = dirs {
            state.intext_sort.opt.insert("sort_directions".into(), d);
        }
        state.intext.srt = state.citation.srt.clone();
    }
    target.push(token);
    Ok(())
}
