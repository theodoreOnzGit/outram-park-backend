// Part of the kovan port of citeproc-js (GitHub #790, #792).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util.js
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

//! `CSL.Util` and `CSL.Util.Match` (src/util.js).
//!
//! `CSL.Util.Match` builds the evaluator of a conditional (`<if match="any">`):
//! a function over the token's `tests` closures that is true when any, all or
//! none of them (or not all) are. Here the evaluator is a [`UtilTest`]
//! (a [`Test`] variant holding the token's tests) and [`Match`] is
//! `state.fun.match`.

use serde_json::Value;

use super::exec::Test;
use super::obj_token::Token;
use super::state::State;
use super::{CslResult, EngineError};

/// `CSL.Util.Match`: `state.fun.match`, the table of `any`, `none`, `all`,
/// `nand`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Match;

impl Match {
    /// `this.any(token, state, tests)`: true if any test is.
    pub fn any(&self, _token: &Token, _state: &State, tests: &[Test]) -> Test {
        Test::Util(UtilTest::Any(tests.to_vec()))
    }

    /// `this.none(token, state, tests)`: true if no test is.
    pub fn none(&self, _token: &Token, _state: &State, tests: &[Test]) -> Test {
        Test::Util(UtilTest::None(tests.to_vec()))
    }

    /// `this.all(token, state, tests)`: true if every test is (also the
    /// default, `this[undefined] = this.all`).
    pub fn all(&self, _token: &Token, _state: &State, tests: &[Test]) -> Test {
        Test::Util(UtilTest::All(tests.to_vec()))
    }

    /// `this.nand(token, state, tests)`: false only if every test is true.
    pub fn nand(&self, _token: &Token, _state: &State, tests: &[Test]) -> Test {
        Test::Util(UtilTest::Nand(tests.to_vec()))
    }

    /// `state.fun.match[name](token, state, tests)`. `name` is the token's
    /// `match` string, `None` for `undefined` (JS looks it up as the key
    /// `"undefined"`, which is `all`). An unknown name is a TypeError
    /// upstream ("state.fun.match[...] is not a function").
    pub fn build(
        &self,
        name: Option<&str>,
        token: &Token,
        state: &State,
        tests: &[Test],
    ) -> CslResult<Test> {
        match name {
            None | Some("undefined") | Some("all") => Ok(self.all(token, state, tests)),
            Some("any") => Ok(self.any(token, state, tests)),
            Some("none") => Ok(self.none(token, state, tests)),
            Some("nand") => Ok(self.nand(token, state, tests)),
            Some(other) => Err(EngineError::Csl(format!(
                "TypeError: state.fun.match[{other:?}] is not a function"
            ))),
        }
    }
}

/// The evaluators `CSL.Util.Match` returns (src/util.js), holding the
/// `tests` array they close over.
#[derive(Debug, Clone, PartialEq)]
pub enum UtilTest {
    /// `any`: true at the first true test.
    Any(Vec<Test>),
    /// `none`: false at the first true test.
    None(Vec<Test>),
    /// `all`: false at the first false test.
    All(Vec<Test>),
    /// `nand`: true at the first false test.
    Nand(Vec<Test>),
}

impl UtilTest {
    /// Evaluate: `function (Item, item) { ... }`.
    pub fn eval(
        &self,
        state: &mut State,
        token: &mut Token,
        item: &Value,
        cite_item: &Value,
    ) -> CslResult<bool> {
        match self {
            UtilTest::Any(tests) => {
                for t in tests {
                    if t.eval(state, &mut *token, item, cite_item)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            UtilTest::None(tests) => {
                for t in tests {
                    if t.eval(state, &mut *token, item, cite_item)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            UtilTest::All(tests) => {
                for t in tests {
                    if !t.eval(state, &mut *token, item, cite_item)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            UtilTest::Nand(tests) => {
                for t in tests {
                    if !t.eval(state, &mut *token, item, cite_item)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
        }
    }
}
