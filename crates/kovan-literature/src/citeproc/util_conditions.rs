// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_conditions.js, src/util.js (CSL.Util.Match only)
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

//! Port of `src/util_conditions.js` (`CSL.Conditions`) and of the match
//! combinators `CSL.Util.Match` (`state.fun.match.any/none/all/nand`, which
//! live in `src/util.js` upstream; they are only ever used to combine the
//! `tests` of a conditional token, so they are ported here).
//!
//! A conditional token (`cs:if`, `cs:else-if`, `cs:condition`, ...) collects
//! `tests` (attribute handlers push them) and one `test` that combines them
//! with a match mode. Upstream's `match.any(token, state, tests)` (util.js,
//! [`super::util::Match`]) returns a closure over the `tests` *array*; here
//! [`super::util::UtilTest`] carries a copy of the tests at the moment of
//! combination (always after the last test was pushed, so the copy is
//! equivalent).

use serde_json::Value;

use super::exec::Test;
use super::js;
use super::obj_token::{Token, TokenType};
use super::state::State;
use super::{CslResult, EngineError};

/// The closures `src/util_conditions.js` stores in `token.execs`
/// (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum UtilConditionsExec {
    /// The `state.tmp.condition_counter++` closure of `TopNode`
    /// (util_conditions.js:18-20).
    ConditionCounterIncrement,
    /// The closure that decrements the counter and restores the locale
    /// (util_conditions.js:25-42).
    ConditionCounterDecrement,
    /// `closingjump` (util_conditions.js:46-49): returns `this[jump.value()]`.
    ClosingJump,
}

impl UtilConditionsExec {
    /// Run the closure.
    pub fn run(
        &self,
        state: &mut State,
        token: &mut Token,
        _item: &Value,
        _cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            UtilConditionsExec::ConditionCounterIncrement => {
                state.tmp.condition_counter += 1;
                Ok(None)
            }
            UtilConditionsExec::ConditionCounterDecrement => {
                state.tmp.condition_counter -= 1;
                if !state.tmp.condition_lang_counter_arr.is_empty() {
                    let counter = state.tmp.condition_lang_counter_arr.last().copied();
                    if counter == Some(state.tmp.condition_counter) {
                        // JS: state.opt.lang = state.tmp.condition_lang_val_arr.pop()
                        let lang = state.tmp.condition_lang_val_arr.pop();
                        match lang {
                            Some(l) => {
                                state.opt.insert("lang".into(), Value::String(l));
                            }
                            None => {
                                state.opt.remove("lang");
                            }
                        }
                        state.tmp.condition_lang_counter_arr.pop();
                    }
                }
                if js::truthy_opt(token.extra.get("locale_default")) {
                    // PORT-LATER(wave2): util_conditions.js:36-39, needs
                    // state.output.current.value().old_locale = this.locale_default;
                    // state.output.closeLevel("empty") (queue.rs), then
                    // state.opt.lang = this.locale_default.
                    return Err(EngineError::NotYetPorted {
                        method: "util_conditions.js:36 closure (output.closeLevel)",
                    });
                }
                Ok(None)
            }
            UtilConditionsExec::ClosingJump => {
                // JS: var next = this[state.tmp.jump.value()]; return next;
                let which = state
                    .tmp
                    .jump
                    .value()
                    .and_then(|v| v.as_str().map(str::to_string));
                Ok(match which.as_deref() {
                    Some("succeed") => token.succeed,
                    Some("fail") => token.fail,
                    Some("next") => token.next,
                    _ => None,
                })
            }
        }
    }
}

/// `CSL.Conditions.Engine`: collects the tests of a `cs:if` / `cs:else-if`
/// built from `cs:conditions` / `cs:condition` children. Upstream holds the
/// token by reference; the token has been pushed to the target token list
/// when the children arrive, so the engine holds its index in that list.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ConditionsEngine {
    /// Index in the target list of the token being filled.
    pub token_index: usize,
}

impl ConditionsEngine {
    fn token<'t>(&self, target: &'t mut [Token]) -> CslResult<&'t mut Token> {
        target
            .get_mut(self.token_index)
            .ok_or_else(|| EngineError::Csl("TypeError: conditions engine token is missing".into()))
    }

    /// `CSL.Conditions.Engine.prototype.addTest`.
    pub fn add_test(&self, target: &mut [Token], test: Test) -> CslResult<()> {
        let tok = self.token(target)?;
        // this.token.tests ? {} : this.token.tests = [];
        tok.tests_defined = true;
        tok.tests.push(test);
        Ok(())
    }

    /// `CSL.Conditions.Engine.prototype.addMatch`.
    /// `None` is an undefined `match` attribute (the property is then absent).
    pub fn add_match(&self, target: &mut [Token], match_: Option<&Value>) -> CslResult<()> {
        let tok = self.token(target)?;
        match match_ {
            Some(m) => {
                tok.extra.insert("match".into(), m.clone());
            }
            None => {
                tok.extra.remove("match");
            }
        }
        Ok(())
    }

    /// `CSL.Conditions.Engine.prototype.matchCombine`.
    pub fn match_combine(&self, state: &State, target: &mut [Token]) -> CslResult<()> {
        let tok = self.token(target)?;
        let combined = match_combine(state, tok, &tok.tests.clone())?;
        tok.test = Some(combined);
        Ok(())
    }
}

/// `state.fun.match[token.match](token, state, tests)`: the combined test.
/// Reads the token's `match` property (`token.extra["match"]`).
pub fn match_combine(state: &State, token: &Token, tests: &[Test]) -> CslResult<Test> {
    let name = token.extra.get("match").map(js::to_js_string);
    state.fun.match_.build(name.as_deref(), token, state, tests)
}

/// `CSL.Conditions.TopNode.call(token, state)`, shared by `cs:if` and
/// `cs:else-if`. `target` is the list the token is about to be pushed to (the
/// caller pushes it right after this returns).
pub fn top_node(state: &mut State, token: &mut Token, target: &[Token]) -> CslResult<()> {
    if token.tokentype == TokenType::Start || token.tokentype == TokenType::Singleton {
        if js::truthy_opt(token.extra.get("locale")) {
            if let Some(l) = token.extra.get("locale") {
                state.opt.insert("lang".into(), l.clone());
            }
        }
        if token.tests.is_empty() {
            // Set up the condition compiler with our current context
            state.tmp.conditions = Some(ConditionsEngine {
                token_index: target.len(),
            });
        } else {
            // The usual.
            token.test = Some(match_combine(state, token, &token.tests.clone())?);
        }
        if state.build.substitute_level.value().copied() == Some(0) {
            token.execs.push(super::exec::Exec::UtilConditions(
                UtilConditionsExec::ConditionCounterIncrement,
            ));
        }
    }
    if token.tokentype == TokenType::End || token.tokentype == TokenType::Singleton {
        if state.build.substitute_level.value().copied() == Some(0) {
            token.execs.push(super::exec::Exec::UtilConditions(
                UtilConditionsExec::ConditionCounterDecrement,
            ));
        }
        // closingjump
        token.execs.push(super::exec::Exec::UtilConditions(
            UtilConditionsExec::ClosingJump,
        ));
        if js::truthy_opt(token.extra.get("locale_default")) {
            if let Some(l) = token.extra.get("locale_default") {
                state.opt.insert("lang".into(), l.clone());
            }
        }
    }
    Ok(())
}

/// `CSL.Conditions.Configure.call(tokens[pos], state, pos)`: the jump-index
/// pass shared by `cs:if` and `cs:else-if`.
pub fn configure(state: &mut State, tokens: &mut [Token], pos: usize) -> CslResult<()> {
    let Some(tok) = tokens.get_mut(pos) else {
        return Ok(());
    };
    match tok.tokentype {
        TokenType::Start => {
            // jump index on failure
            tok.fail = state.configure.fail.last().copied();
            tok.succeed = tok.next;
            if let Some(last) = state.configure.fail.last_mut() {
                *last = pos;
            }
        }
        TokenType::Singleton => {
            // jump index on failure
            tok.fail = tok.next;
            tok.succeed = state.configure.succeed.last().copied();
            if let Some(last) = state.configure.fail.last_mut() {
                *last = pos;
            }
        }
        TokenType::End => {
            // jump index on success
            tok.succeed = state.configure.succeed.last().copied();
            tok.fail = tok.next;
        }
    }
    Ok(())
}
