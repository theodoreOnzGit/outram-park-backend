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
use super::formatters::capitalize_first;
use super::js;
use super::load::update_group_context_condition;
use super::node_group;
use super::node_names::{set_pair, token_to_value};
use super::obj_token::Token;
use super::queue::{self, AppendArg, FormatRef, QueueId};
use super::state::State;
use super::util_label::{evaluate_label, LabelContext};
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
        state: &mut State,
        token: &mut Token,
        item: &Value,
        cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            NodeLabelExec::Render => {
                // Must accomplish this without touching strings shared with
                // the calling application: "sub verbo" and "sub-verbo" must
                // both pass, as they stand.
                //
                // This is abstracted away, because the same logic must be
                // run in cs:names.
                //
                // `evaluateLabel` reads and writes `group_context.tip`
                // (`label_form`, `label_capitalize_if_first`, `label_static`)
                // and `tmp.strip_periods`; the context is copied out and back.
                let tip = node_group::tip(state);
                let mut ctx = LabelContext {
                    tip_label_form: js::truthy(&tip.label_form)
                        .then(|| js::to_js_string(&tip.label_form)),
                    tip_label_capitalize_if_first: js::truthy(&tip.label_capitalize_if_first)
                        .then(|| tip.label_capitalize_if_first.clone()),
                    tip_label_static: js::truthy(&tip.label_static),
                    strip_periods: state.tmp.strip_periods != 0,
                };
                let cite = if js::truthy(cite_item) {
                    Some(cite_item)
                } else {
                    None
                };
                let mut termtxt = evaluate_label(state, token, &mut ctx, item, cite)?;
                if ctx.tip_label_static && !js::truthy(&tip.label_static) {
                    node_group::tip_mut(state)?.label_static = Value::Bool(true);
                }
                // `item.section_form_override = this.strings.form` (for
                // `locator`) is a property nothing in citeproc-js reads.
                if !termtxt.is_empty() {
                    node_group::tip_mut(state)?.term_intended = true;
                }
                update_group_context_condition(state, Some(&termtxt), false, Some(token), None);
                if !termtxt.contains("%s") {
                    // ^ Suppress output here if we have an embedded term
                    if js::truthy_opt(token.strings.get("capitalize_if_first"))
                        && !state.tmp.term_predecessor
                        && !(js::get_str(&state.opt, "class") == Some("in-text")
                            && state.tmp.area == "citation")
                    {
                        termtxt = capitalize_first(state, &termtxt);
                    }
                    queue::append(
                        state,
                        QueueId::Output,
                        AppendArg::Text(termtxt),
                        FormatRef::Token(token.clone()),
                        false,
                        false,
                        false,
                    )?;
                }
                Ok(None)
            }
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
