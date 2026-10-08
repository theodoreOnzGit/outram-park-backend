// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/node_name.js
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

//! Port of `src/node_name.js`: `CSL.Node.name`.

use serde_json::Value;

use super::load::POSITION;
use super::exec::Exec;
use super::js;
use super::obj_token::{Token, TokenType};
use super::state::State;
use super::{CslResult, EngineError};

/// The closures `src/node_name.js` stores in `token.execs` (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeNameExec {
    /// START / SINGLETON: set the et-al term, delimiter, `and` term and
    /// prefixes, the ellipsis and `and` blobs, the et-al parameters, and
    /// `state.nameOutput.name = this` (node_name.js:41-170).
    Setup,
}

impl NodeNameExec {
    /// Run the closure.
    pub fn run(
        &self,
        _state: &mut State,
        _token: &mut Token,
        _item: &Value,
        _cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            // PORT-LATER(wave3): node_name.js:41-170, needs state.getTerm,
            // state.output.append/pop and CSL.Blob (queue.rs), CSL.NameOutput
            // (state.nameOutput.name = this); it also writes this.and_term,
            // this.and_prefix_*, this.and, this.ellipsis* on the running token
            // (Exec::run receives &Token) and reads state.inheritOpt at run time
            // (attributes::inherit_opt).
            NodeNameExec::Setup => Err(EngineError::NotYetPorted {
                method: "node_name.js:41 closure",
            }),
        }
    }
}

/// `CSL.Node.name.build.call(token, state, target)`.
pub fn build(
    state: &mut State,
    mut token: Token,
    target: &mut Vec<Token>,
    _real_group: Option<bool>,
) -> CslResult<()> {
    if token.tokentype == TokenType::Singleton || token.tokentype == TokenType::Start {
        // JS: `if ("undefined" === typeof state.tmp.root) { state.tmp.root =
        // "citation" } else { oldTmpRoot = state.tmp.root }`. `Tmp::new` always
        // sets `root`, so only the else-branch is reachable.
        let old_tmp_root: String = state.tmp.root.clone();
        // Many CSL styles set et-al-[min|use-first]
        // and et-al-subsequent-[min|use-first] to the same
        // value.
        // Set state.opt.update_mode = CSL.POSITION if
        // et-al-subsequent-min or et-al-subsequent-use-first
        // are set AND their value differs from their plain
        // counterparts.
        let result = (|| -> CslResult<()> {
            for (sub, plain) in [
                ("et-al-subsequent-min", "et-al-min"),
                ("et-al-subsequent-use-first", "et-al-use-first"),
            ] {
                let a = state.inherit_opt(&token, sub, None, None);
                if let Some(a) = a {
                    if js::truthy(&a) {
                        let b = state.inherit_opt(&token, plain, None, None);
                        if b.as_ref() != Some(&a) || is_nan(&a) {
                            state
                                .opt
                                .insert("update_mode".into(), Value::from(POSITION));
                        }
                    }
                }
            }
            Ok(())
        })();
        state.tmp.root = old_tmp_root;
        result?;

        state.build.name_flag = true;

        token.execs.push(Exec::NodeName(NodeNameExec::Setup));
    }
    target.push(token);
    Ok(())
}

/// A numeric JSON value that is JS NaN (serialised as `null`): never equal to
/// itself under `!==`. A truthy value is never NaN, so this is only a guard.
fn is_nan(v: &Value) -> bool {
    v.is_null()
}
