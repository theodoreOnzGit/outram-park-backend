// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/node_alternative.js
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

//! Port of `src/node_alternative.js`: `CSL.Node.alternative`.

use serde_json::Value;

use super::attributes;
use super::exec::Exec;
use super::node_choose;
use super::node_if;
use super::obj_token::{Token, TokenType};
use super::state::State;
use super::{CslResult, EngineError};

/// The closures `src/node_alternative.js` stores in `token.execs`
/// (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeAlternativeExec {
    /// START: swap in the item's alternative-language variant and open the
    /// output level (node_alternative.js:14-85).
    Start,
    /// On the inner `if` START token: `state.tmp.abort_alternative = true`
    /// (node_alternative.js:94-96).
    AbortAlternative,
    /// END: restore the item, language and name output (node_alternative.js:110-116).
    End,
}

impl NodeAlternativeExec {
    /// Run the closure.
    pub fn run(
        &self,
        state: &mut State,
        _token: &Token,
        _item: &Value,
        _cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            // PORT-LATER(wave2): node_alternative.js:14-85, needs
            // state.output.openLevel (queue.rs), state.registry.refhash and
            // CSL.NameOutput (state.nameOutput = new CSL.NameOutput(state, newItem)).
            NodeAlternativeExec::Start => Err(EngineError::NotYetPorted {
                method: "node_alternative.js:14 closure",
            }),
            NodeAlternativeExec::AbortAlternative => {
                state.tmp.abort_alternative = true;
                Ok(None)
            }
            // PORT-LATER(wave2): node_alternative.js:110-116, needs
            // state.output.closeLevel, state.registry.refhash, CSL.NameOutput.
            NodeAlternativeExec::End => Err(EngineError::NotYetPorted {
                method: "node_alternative.js:110 closure",
            }),
        }
    }
}

/// `CSL.Node.alternative.build.call(token, state, target)`.
pub fn build(
    state: &mut State,
    mut token: Token,
    target: &mut Vec<Token>,
    _real_group: bool,
) -> CslResult<()> {
    if token.tokentype == TokenType::Start {
        let choose_tok = Token::new("choose", TokenType::Start);
        node_choose::build(state, choose_tok, target, false)?;

        let mut if_tok = Token::new("if", TokenType::Start);
        attributes::apply(state, &mut if_tok, "@alternative-node-internal", "")?;
        node_if::build(state, if_tok, target, false)?;

        token
            .execs
            .push(Exec::NodeAlternative(NodeAlternativeExec::Start));
        target.push(token);

        let choose_tok = Token::new("choose", TokenType::Start);
        node_choose::build(state, choose_tok, target, false)?;

        let mut if_tok = Token::new("if", TokenType::Start);
        attributes::apply(state, &mut if_tok, "@alternative-node-internal", "")?;
        if_tok
            .execs
            .push(Exec::NodeAlternative(NodeAlternativeExec::AbortAlternative));
        node_if::build(state, if_tok, target, false)?;
    } else if token.tokentype == TokenType::End {
        let if_tok = Token::new("if", TokenType::End);
        node_if::build(state, if_tok, target, false)?;

        let choose_tok = Token::new("choose", TokenType::End);
        node_choose::build(state, choose_tok, target, false)?;

        token
            .execs
            .push(Exec::NodeAlternative(NodeAlternativeExec::End));
        target.push(token);

        let if_tok = Token::new("if", TokenType::End);
        node_if::build(state, if_tok, target, false)?;

        let choose_tok = Token::new("choose", TokenType::End);
        node_choose::build(state, choose_tok, target, false)?;
    }
    Ok(())
}
