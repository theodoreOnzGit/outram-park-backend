// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/node_label.js
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

//! Port of `src/node_label.js`: `CSL.Node.label`.

use serde_json::Value;

use super::exec::Exec;
use super::js;
use super::node_names::{set_pair, token_to_value};
use super::obj_token::Token;
use super::state::State;
use super::{CslResult, EngineError};

/// The closures `src/node_label.js` stores in `token.execs` (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeLabelExec {
    /// A `cs:label` outside `cs:names`: render the term (node_label.js:6-34).
    Render,
}

impl NodeLabelExec {
    /// Run the closure.
    pub fn run(
        &self,
        _state: &mut State,
        _token: &mut Token,
        _item: &Value,
        _cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            // PORT-LATER(wave2): node_label.js:6-34, needs CSL.evaluateLabel,
            // CSL.UPDATE_GROUP_CONTEXT_CONDITION, state.tmp.group_context,
            // CSL.Output.Formatters["capitalize-first"], state.output.append,
            // and writes item.section_form_override (a &Value here).
            NodeLabelExec::Render => Err(EngineError::NotYetPorted {
                method: "node_label.js:6 closure",
            }),
        }
    }
}

/// `CSL.Node.label.build.call(token, state, target)`.
pub fn build(
    state: &mut State,
    mut token: Token,
    target: &mut Vec<Token>,
    _real_group: Option<bool>,
) -> CslResult<()> {
    if js::truthy_opt(token.strings.get("term")) {
        // Non-names labels
        token.execs.push(Exec::NodeLabel(NodeLabelExec::Render));
    } else {
        if !js::truthy_opt(token.strings.get("form")) {
            token.set_string("form", "long");
        }
        // Names labels
        // Picked up in names END
        let namevars = state.build.names_variables.last().cloned().ok_or_else(|| {
            EngineError::Csl("TypeError: state.build.names_variables[-1] is undefined".into())
        })?;
        let name_flag = state.build.name_flag;
        let tv = token_to_value(&token);
        let namelabels = state.build.name_label.last_mut().ok_or_else(|| {
            EngineError::Csl("TypeError: state.build.name_label[-1] is undefined".into())
        })?;
        for var in &namevars {
            if !namelabels.iter().any(|(k, _)| k == var) {
                namelabels.push((var.clone(), Value::Object(js::Obj::new())));
            }
        }
        let side = if !name_flag { "before" } else { "after" };
        for var in &namevars {
            let mut entry = namelabels
                .iter()
                .find(|(k, _)| k == var)
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| Value::Object(js::Obj::new()));
            if let Value::Object(o) = &mut entry {
                o.insert(side.to_string(), tv.clone());
            }
            set_pair(namelabels, var, entry);
        }
    }
    target.push(token);
    Ok(())
}
