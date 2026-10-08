// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/node_group.js
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

//! Port of `src/node_group.js`: `CSL.Node.group`.

use serde_json::Value;

use super::exec::{Exec, Test};
use super::js;
use super::node_choose;
use super::node_else;
use super::node_if;
use super::obj_token::{Token, TokenType};
use super::state::State;
use super::util_conditions::{self, MatchKind};
use super::util_substitute;
use super::{CslResult, EngineError};

/// The closures `src/node_group.js` stores in `token.execs` (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeGroupExec {
    /// START "newoutput": open the group tag and push a group context
    /// (node_group.js:20-150). Inserted at the *front* of `execs`.
    Start,
    /// START, with `subgroup-delimiter` and `has-publisher-and-publisher-place`:
    /// build `state.publisherOutput` (node_group.js:160-175).
    PublisherSpecialStart,
    /// The juris text token's closure: run the `juris-` token list
    /// (node_group.js:229-243).
    RunJurisTokens,
    /// END: render the publisher list (node_group.js:~270-278).
    PublisherSpecialEnd,
    /// END "quashnonfields": close the group tag, pop the group context and
    /// decide whether the group's output stays (node_group.js:~282-385).
    End,
}

impl NodeGroupExec {
    /// Run the closure.
    pub fn run(
        &self,
        _state: &mut State,
        _token: &mut Token,
        _item: &Value,
        _cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        // PORT-LATER(wave2): every closure of node_group.js works on the output
        // queue (state.output.startTag/endTag/current, queue.rs) and on
        // state.tmp.group_context (the group-context stack with its flag
        // objects; CSL.UPDATE_GROUP_CONTEXT_CONDITION / EVALUATE_GROUP_CONDITION),
        // state.parallel.checkRepeats, state.tmp.suppress_repeats and
        // CSL.PublisherOutput / CSL.tokenExec. `this.realGroup`, `parallel_*`,
        // `non_parallel` and the label overrides are token data (extra /
        // strings) already set by the builder and attributes.
        let method = match self {
            NodeGroupExec::Start => "node_group.js:20 closure",
            NodeGroupExec::PublisherSpecialStart => "node_group.js:160 closure",
            NodeGroupExec::RunJurisTokens => "node_group.js:229 closure",
            NodeGroupExec::PublisherSpecialEnd => "node_group.js:270 closure",
            NodeGroupExec::End => "node_group.js:282 closure",
        };
        Err(EngineError::NotYetPorted { method })
    }
}

/// The condition closures `src/node_group.js` stores in `token.tests` /
/// `token.test` (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeGroupTest {
    /// `CSL.INIT_JURISDICTION_MACROS(state, Item, item, macroName)` for the
    /// `juris-` macro `macro_name` (node_group.js:215-222).
    InitJurisdictionMacros {
        /// The captured `this.juris`.
        macro_name: String,
    },
}

impl NodeGroupTest {
    /// Evaluate the condition.
    pub fn eval(
        &self,
        _state: &mut State,
        _token: &mut Token,
        _item: &Value,
        _cite_item: &Value,
    ) -> CslResult<bool> {
        match self {
            // PORT-LATER(wave2): node_group.js:215-222, CSL.INIT_JURISDICTION_MACROS
            // (jurisdiction macro loading; not in the files of this wave).
            NodeGroupTest::InitJurisdictionMacros { .. } => Err(EngineError::NotYetPorted {
                method: "node_group.js:215 CSL.INIT_JURISDICTION_MACROS",
            }),
        }
    }
}

/// `CSL.Node.group.build.call(token, state, target, realGroup)`.
pub fn build(
    state: &mut State,
    mut token: Token,
    target: &mut Vec<Token>,
    real_group: bool,
) -> CslResult<()> {
    token
        .extra
        .insert("realGroup".into(), Value::Bool(real_group));
    let juris: Option<String> = token
        .extra
        .get("juris")
        .filter(|v| js::truthy(v))
        .map(js::to_js_string);

    if token.tokentype == TokenType::Start {
        util_substitute::substitute_start(state, &mut token, target)?;
        if let Some(level) = state.build.substitute_level.value().copied() {
            if level != 0 {
                state.build.substitute_level.replace_literal(level + 1)?;
            }
        }
        // newoutput
        //
        // Paranoia.  Assure that this init function is the first executed.
        let mut execs = vec![Exec::NodeGroup(NodeGroupExec::Start)];
        execs.append(&mut token.execs);
        token.execs = execs;

        // "Special handling" for nodes that contain only
        // publisher and place, with no affixes. For such
        // nodes only, parallel publisher/place pairs
        // will be parsed out and properly joined, piggybacking on
        // join parameters set on cs:citation or cs:bibliography.
        if js::truthy_opt(token.strings.get("has-publisher-and-publisher-place")) {
            // Pass variable string values to the closing
            // tag via a global, iff they conform to expectations.
            state.build.publisher_special = true;
            if js::truthy_opt(token.strings.get("subgroup-delimiter")) {
                // Set the handling function only if name-delimiter
                // is set on the parent cs:citation or cs:bibliography
                // node.
                token
                    .execs
                    .push(Exec::NodeGroup(NodeGroupExec::PublisherSpecialStart));
            }
        }

        if let Some(juris_name) = juris.clone() {
            // "Special handling" for jurisdiction macros
            // We try to instantiate these as standalone token lists.
            // If available, the token list is executed,
            // the result is written directly into output,
            // and control returns here.
            //
            // `this` is not pushed to the target before the juris scaffolding.
            let choose_start = Token::new("choose", TokenType::Start);
            node_choose::build(state, choose_start, target, false)?;

            let mut if_start = Token::new("if", TokenType::Start);
            if_start.tests_defined = true;
            if_start
                .tests
                .push(Test::NodeGroup(NodeGroupTest::InitJurisdictionMacros {
                    macro_name: juris_name.clone(),
                }));
            if_start.test = Some(util_conditions::match_test(MatchKind::Any, &if_start.tests));
            target.push(if_start);
            let mut text_node = Token::new("text", TokenType::Singleton);
            // This will run the juris- token list.
            text_node
                .extra
                .insert("juris".into(), Value::String(juris_name));
            text_node
                .execs
                .push(Exec::NodeGroup(NodeGroupExec::RunJurisTokens));
            target.push(text_node);

            let if_end = Token::new("if", TokenType::End);
            node_if::build(state, if_end, target, false)?;
            let else_start = Token::new("else", TokenType::Start);
            node_else::build(state, else_start, target, false)?;
        }
    }

    if token.tokentype == TokenType::End {
        // Unbundle and print publisher lists
        // Same constraints on creating the necessary function here
        // as above. The full content of the group formatting token
        // is apparently not available on the closing tag here,
        // hence the global flag on state.build.
        if state.build.publisher_special {
            state.build.publisher_special = false;
            token
                .execs
                .push(Exec::NodeGroup(NodeGroupExec::PublisherSpecialEnd));
        }

        // quashnonfields
        token.execs.push(Exec::NodeGroup(NodeGroupExec::End));

        if juris.is_some() {
            let else_end = Token::new("else", TokenType::End);
            node_else::build(state, else_end, target, false)?;
            let choose_end = Token::new("choose", TokenType::End);
            node_choose::build(state, choose_end, target, false)?;
        }
    }

    if token.tokentype == TokenType::End {
        if let Some(level) = state.build.substitute_level.value().copied() {
            if level != 0 {
                state.build.substitute_level.replace_literal(level - 1)?;
            }
        }
        // if (!this.juris) target.push(this); then substituteEnd.
        util_substitute::push_with_substitute_end(state, token, target, juris.is_none())?;
    } else if juris.is_none() {
        target.push(token);
    }
    Ok(())
}
