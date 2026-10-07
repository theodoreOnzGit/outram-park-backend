// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/node_substitute.js
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

//! Port of `src/node_substitute.js`: `CSL.Node.substitute`.

use serde_json::Value;

use super::exec::{Exec, Test};
use super::node_choose;
use super::obj_token::{Token, TokenType};
use super::state::State;
use super::util_conditions::{self, MatchKind};
use super::CslResult;

/// The closures `src/node_substitute.js` stores in `token.execs`
/// (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeSubstituteExec {
    /// START: allow blocking and, if a value was rendered, stop further
    /// substitution (node_substitute.js:22-28).
    Start,
}

impl NodeSubstituteExec {
    /// Run the closure.
    pub fn run(
        &self,
        state: &mut State,
        _token: &Token,
        _item: &Value,
        _cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            NodeSubstituteExec::Start => {
                state.tmp.can_block_substitute = true;
                if !state.tmp.value.is_empty() && !state.tmp.common_term_match_fail {
                    state.tmp.can_substitute.replace_literal(false)?;
                }
                state.tmp.common_term_match_fail = false;
                Ok(None)
            }
        }
    }
}

/// The condition closures `src/node_substitute.js` stores in `token.tests`
/// (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeSubstituteTest {
    /// `state.tmp.value.length && !state.tmp.common_term_match_fail`
    /// (node_substitute.js:13-18).
    ValueRendered,
}

impl NodeSubstituteTest {
    /// Evaluate the condition.
    pub fn eval(
        &self,
        state: &mut State,
        _token: &Token,
        _item: &Value,
        _cite_item: &Value,
    ) -> CslResult<bool> {
        match self {
            NodeSubstituteTest::ValueRendered => {
                Ok(!state.tmp.value.is_empty() && !state.tmp.common_term_match_fail)
            }
        }
    }
}

/// `CSL.Node.substitute.build.call(token, state, target)`.
pub fn build(
    state: &mut State,
    mut token: Token,
    target: &mut Vec<Token>,
    _real_group: bool,
) -> CslResult<()> {
    if token.tokentype == TokenType::Start {
        // set conditional
        let choose_start = Token::new("choose", TokenType::Start);
        node_choose::build(state, choose_start, target, false)?;
        let mut if_singleton = Token::new("if", TokenType::Singleton);
        if_singleton.tests_defined = true;
        if_singleton.tests = vec![Test::NodeSubstitute(NodeSubstituteTest::ValueRendered)];
        if_singleton.test = Some(util_conditions::match_test(
            MatchKind::Any,
            &if_singleton.tests,
        ));
        target.push(if_singleton);

        token
            .execs
            .push(Exec::NodeSubstitute(NodeSubstituteExec::Start));
        target.push(token);
    } else if token.tokentype == TokenType::End {
        target.push(token);
        let choose_end = Token::new("choose", TokenType::End);
        node_choose::build(state, choose_end, target, false)?;
    }
    Ok(())
}
