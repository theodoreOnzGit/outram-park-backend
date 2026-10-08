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
//!
//! # Group suppression
//!
//! A group renders only if something inside it did. Every `cs:group` START
//! pushes a context onto `state.tmp.group_context` (a [`GroupContext`]:
//! `variable_attempt`, `variable_success`, `term_intended`, `condition`,
//! `force_suppress`, ...) and every END pops it and decides: when no variable
//! succeeded (and no term was intended without an attempted variable), or a
//! conditional group (`require`/`reject`) wants to be suppressed, the group's
//! blob is popped off the output again and the parent context learns of the
//! failed attempt. `cs:text variable`, `cs:number`, `cs:date`, `cs:names` and
//! the label code raise the flags of the tip context as they render.
//!
//! # Differences from JS that are not behaviour
//!
//! * Upstream's `condition` of a nested context is the very object of its
//!   parent's (UPDATE_GROUP_CONTEXT_CONDITION sets `termtxt` on both). Here
//!   each context owns a copy and the END closure copies the child's
//!   condition back into the parent's tip, which is when the parent reads it.
//! * `Object.assign(non_parallel, this.non_parallel)` mutates the parent's
//!   object in place; the merge is done on the parent's tip as well.
//! * `state.output.current.value().parent = ...` is a blob property nothing
//!   reads; it is stored as `blob.extra["parent"]` (the id of the blob).

use serde_json::Value;

use super::exec::{Exec, Test};
use super::js;
use super::load::{self, evaluate_group_condition, update_group_context_condition};
use super::node_choose;
use super::node_else;
use super::node_if;
use super::obj_blob::BlobContent;
use super::obj_token::{Token, TokenType};
use super::queue::{self, QueueId};
use super::state::{GroupCondition, GroupContext, State};
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

/// `state.tmp.group_context.tip`, mutably. An empty stack is the JS
/// `TypeError` of writing to the `tip` of an exhausted stack.
pub fn tip_mut(state: &mut State) -> CslResult<&mut GroupContext> {
    state.tmp.group_context.tip_mut().ok_or_else(|| {
        EngineError::BadInput("Cannot set properties of undefined (group_context.tip)".into())
    })
}

/// `state.tmp.group_context.tip`. An empty stack gives a default context
/// (every read of `tip.x` is then `undefined`, as on JS's `{}`).
pub fn tip(state: &State) -> GroupContext {
    state.tmp.group_context.tip().cloned().unwrap_or_default()
}

/// `Object.assign(base || {}, add)` on JSON objects: `base` is updated in
/// place and returned.
fn assign(base: &mut Value, add: &Value) {
    if !base.is_object() {
        *base = Value::Object(js::Obj::new());
    }
    if let (Value::Object(b), Value::Object(a)) = (&mut *base, add) {
        for (k, v) in a {
            b.insert(k.clone(), v.clone());
        }
    }
}

/// `state.output.current.value().blobs.pop()` when the open level is a list.
fn pop_last_child(state: &mut State) {
    if let Some(cur) = queue::current(state, QueueId::Output) {
        if let BlobContent::List(l) = &mut state.blobs.get_mut(cur).blobs {
            l.pop();
        }
    }
}

/// `state.registry.registry[Item.id]` as upstream reads it: a TypeError when
/// the item is not registered.
fn registry_entry_mut<'a>(
    state: &'a mut State,
    item: &Value,
) -> CslResult<&'a mut super::registry::RegistryItem> {
    let id = item.get("id").map(js::to_js_string).unwrap_or_else(|| "undefined".to_string());
    state.registry.registry.get_mut(&id).ok_or_else(|| {
        EngineError::Csl(format!(
            "TypeError: Cannot read properties of undefined (registry[{id}])"
        ))
    })
}

impl NodeGroupExec {
    /// Run the closure.
    pub fn run(
        &self,
        state: &mut State,
        token: &mut Token,
        item: &Value,
        _cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            NodeGroupExec::Start => {
                group_start(state, token)?;
                Ok(None)
            }
            NodeGroupExec::PublisherSpecialStart => {
                if js::truthy_opt(item.get("publisher"))
                    && js::truthy_opt(item.get("publisher-place"))
                {
                    let split = |v: &Value| -> usize {
                        let s = js::to_js_string(v);
                        let re = regex::Regex::new(&format!(r";[{}]*", js::WS)).ok();
                        match re {
                            Some(re) => js::split(&re, &s).len(),
                            None => 1,
                        }
                    };
                    let publisher_n = item.get("publisher").map(split).unwrap_or(0);
                    let place_n = item.get("publisher-place").map(split).unwrap_or(0);
                    if publisher_n > 1 && publisher_n == place_n {
                        // PORT-LATER(w2-names): `state.publisherOutput = new
                        // CSL.PublisherOutput(state, this)` (util_publishers.js).
                        return Err(EngineError::NotYetPorted {
                            method: "node_group.js:174 new CSL.PublisherOutput",
                        });
                    }
                }
                Ok(None)
            }
            NodeGroupExec::RunJurisTokens => {
                // This will run the juris- token list.
                let mut item_item = item;
                let cite_has_best =
                    js::truthy(_cite_item) && js::truthy_opt(_cite_item.get("best-jurisdiction"));
                let juris = token
                    .extra
                    .get("juris")
                    .map(js::to_js_string)
                    .unwrap_or_default();
                if cite_has_best && juris == "juris-locator" {
                    item_item = _cite_item;
                }
                let best = item_item
                    .get("best-jurisdiction")
                    .map(js::to_js_string)
                    .unwrap_or_else(|| "undefined".to_string());
                let Some(module) = state.juris.get(&best) else {
                    return Err(EngineError::BadInput(
                        "Cannot read properties of undefined (reading 'juris-...')".into(),
                    ));
                };
                if module.macros.contains_key(&juris) {
                    // PORT-LATER(w2-engine): token lists of style modules are
                    // not addressable by `State::token_exec` (util_nodes.rs
                    // `TokenList` has no module variant); the test runner
                    // loads no modules, so this is not reached.
                    return Err(EngineError::NotYetPorted {
                        method: "node_group.js:229 run state.juris[...] tokens",
                    });
                }
                Ok(None)
            }
            NodeGroupExec::PublisherSpecialEnd => {
                // if (state.publisherOutput) { render(); state.publisherOutput = false }
                // PORT-LATER(w2-names): there is never a publisherOutput yet.
                Ok(None)
            }
            NodeGroupExec::End => {
                group_end(state, token, item)?;
                Ok(None)
            }
        }
    }
}

/// The "newoutput" closure (node_group.js:20-150).
fn group_start(state: &mut State, token: &mut Token) -> CslResult<()> {
    queue::start_tag(state, QueueId::Output, "group", Some(token))?;

    if js::truthy_opt(token.strings.get("label_form_override")) {
        let ovr = token
            .strings
            .get("label_form_override")
            .cloned()
            .unwrap_or(Value::Null);
        let t = tip_mut(state)?;
        if !js::truthy(&t.label_form) {
            t.label_form = ovr;
        }
    }

    if js::truthy_opt(token.strings.get("label_capitalize_if_first_override")) {
        let ovr = token
            .strings
            .get("label_capitalize_if_first_override")
            .cloned()
            .unwrap_or(Value::Null);
        let t = tip_mut(state)?;
        if !js::truthy(&t.label_capitalize_if_first) {
            t.label_capitalize_if_first = ovr;
        }
    }

    if js::truthy_opt(token.extra.get("realGroup")) {
        if tip(state).condition.is_some() {
            let prefix = token.string("prefix");
            update_group_context_condition(state, Some(&prefix), false, Some(token), None);
        }

        // XXX Can we do something better for length here?
        if !state.tmp.group_context.is_empty() {
            let parent = tip(state).output_tip;
            if let Some(cur) = queue::current(state, QueueId::Output) {
                let v = parent.map(|b| Value::from(b.0)).unwrap_or(Value::Null);
                state.blobs.get_mut(cur).extra.insert("parent".into(), v);
            }
        }

        // fieldcontextflag
        let mut label_form = tip(state).label_form;
        if !js::truthy(&label_form) {
            label_form = token
                .strings
                .get("label_form_override")
                .cloned()
                .unwrap_or(Value::Null);
        }

        let mut label_capitalize_if_first = tip(state).label_capitalize_if_first;
        if !js::truthy(&label_capitalize_if_first) {
            label_capitalize_if_first = token
                .strings
                .get("label_capitalize_if_first")
                .cloned()
                .unwrap_or(Value::Null);
        }
        let parent_tip = tip(state);
        let condition: Option<GroupCondition>;
        let force_suppress: bool;
        if parent_tip.condition.is_some() {
            condition = parent_tip.condition.clone();
            force_suppress = parent_tip.force_suppress;
            //force_suppress: false;
        } else if js::truthy_opt(token.strings.get("reject")) {
            condition = Some(GroupCondition {
                test: token.string("reject"),
                not: true,
                ..GroupCondition::default()
            });
            force_suppress = false;
        } else if js::truthy_opt(token.strings.get("require")) {
            condition = Some(GroupCondition {
                test: token.string("require"),
                not: false,
                ..GroupCondition::default()
            });
            force_suppress = false;
        } else {
            condition = None;
            force_suppress = false;
        }
        let mut context = GroupContext {
            old_term_predecessor: state.tmp.term_predecessor,
            term_intended: false,
            variable_attempt: false,
            variable_success: false,
            variable_success_parent: Value::Bool(parent_tip.variable_success),
            output_tip: queue::current(state, QueueId::Output),
            label_form,
            label_static: parent_tip.label_static.clone(),
            label_capitalize_if_first,
            parallel_delimiter_override: token
                .strings
                .get("set_parallel_delimiter_override")
                .cloned()
                .unwrap_or(Value::Null),
            parallel_delimiter_override_on_suppress: token
                .strings
                .get("set_parallel_delimiter_override_on_suppress")
                .cloned()
                .unwrap_or(Value::Null),
            condition,
            force_suppress,
            done_vars: parent_tip.done_vars.clone(),
            ..GroupContext::default()
        };
        if js::truthy_opt(token.extra.get("non_parallel")) {
            let add = token
                .extra
                .get("non_parallel")
                .cloned()
                .unwrap_or(Value::Null);
            let t = tip_mut(state)?;
            assign(&mut t.non_parallel, &add);
            context.non_parallel = t.non_parallel.clone();
        }
        if js::truthy_opt(token.extra.get("parallel_first")) {
            let add = token
                .extra
                .get("parallel_first")
                .cloned()
                .unwrap_or(Value::Null);
            let t = tip_mut(state)?;
            assign(&mut t.parallel_first, &add);
            context.parallel_first = t.parallel_first.clone();
        }
        if js::truthy_opt(token.extra.get("parallel_last")) {
            let add = token
                .extra
                .get("parallel_last")
                .cloned()
                .unwrap_or(Value::Null);
            let t = tip_mut(state)?;
            assign(&mut t.parallel_last, &add);
            context.parallel_last = t.parallel_last.clone();
        }
        if let Some(l2f) = state
            .tmp
            .abbrev_trimmer
            .as_ref()
            .and_then(|t| t.last_to_first.clone())
        {
            if js::truthy(&context.parallel_last) {
                if !js::truthy(&context.parallel_first) {
                    context.parallel_first = Value::Object(js::Obj::new());
                }
                for varname in l2f.keys() {
                    if js::truthy_opt(context.parallel_last.get(varname.as_str())) {
                        if let Value::Object(pf) = &mut context.parallel_first {
                            pf.insert(varname.clone(), Value::Bool(true));
                        }
                        if let Value::Object(pl) = &mut context.parallel_last {
                            pl.remove(varname.as_str());
                        }
                    }
                }
            }
        }

        state.tmp.group_context.push_literal(context);

        if state.tmp.abbrev_trimmer.is_some()
            && js::truthy_opt(token.extra.get("parallel_last_to_first"))
        {
            let vars: Vec<String> = token
                .extra
                .get("parallel_last_to_first")
                .and_then(Value::as_object)
                .map(|o| o.keys().cloned().collect())
                .unwrap_or_default();
            if let Some(trimmer) = state.tmp.abbrev_trimmer.as_mut() {
                let l2f = trimmer.last_to_first.get_or_insert_with(Default::default);
                for varname in vars {
                    l2f.insert(varname, true);
                }
            }
        }
    }
    Ok(())
}

/// The "quashnonfields" closure (node_group.js:~282-385).
fn group_end(state: &mut State, token: &mut Token, item: &Value) -> CslResult<()> {
    if tip(state).condition.is_none() {
        let cur = queue::current(state, QueueId::Output);
        if let Some(cur) = cur {
            if js::truthy_opt(state.blobs.get(cur).strings.get("suffix")) {
                state.tmp.just_did_number = false;
            }
        }
    }
    queue::end_tag(state, QueueId::Output, None)?;
    if js::truthy_opt(token.extra.get("realGroup")) {
        let mut flags = state.tmp.group_context.pop().ok_or_else(|| {
            EngineError::BadInput("Cannot read properties of undefined (group_context.pop)".into())
        })?;
        if js::truthy(&flags.parallel_delimiter_override) {
            tip_mut(state)?.parallel_delimiter_override = flags.parallel_delimiter_override.clone();
            if !state.tmp.just_looking {
                let entry = registry_entry_mut(state, item)?;
                if entry.master {
                    entry.parallel_delimiter_override =
                        Some(flags.parallel_delimiter_override.clone());
                }
            }
        }
        if js::truthy(&flags.parallel_delimiter_override_on_suppress) {
            tip_mut(state)?.parallel_delimiter_override_on_suppress =
                flags.parallel_delimiter_override_on_suppress.clone();
        }
        if state.tmp.area == "bibliography_sort" {
            let citation_number_idx = flags.done_vars.iter().position(|v| v == "citation-number");
            if js::truthy_opt(token.strings.get("sort_direction"))
                && citation_number_idx.is_some()
                && state.tmp.group_context.len() == 1
            {
                let dir = if token.strings.get("sort_direction").and_then(Value::as_i64)
                    == Some(load::DESCENDING)
                {
                    load::DESCENDING
                } else {
                    load::ASCENDING
                };
                state
                    .bibliography_sort
                    .opt
                    .insert("citation_number_sort_direction".into(), Value::from(dir));
                if let Some(i) = citation_number_idx {
                    flags.done_vars.remove(i);
                }
            }
        }
        if flags.condition.is_some() {
            // `undefined` (the context condition is not enabled) is falsy.
            flags.force_suppress = evaluate_group_condition(state, &flags).unwrap_or(false);
        }
        // The popped context shared its condition object with the parent
        // context upstream (see the module docs): hand its state back.
        if let Some(t) = state.tmp.group_context.tip_mut() {
            if t.condition.is_some() {
                t.condition = flags.condition.clone();
                t.force_suppress = flags.force_suppress;
            }
        }

        if !flags.force_suppress
            && (flags.variable_success || (flags.term_intended && !flags.variable_attempt))
        {
            if !js::truthy_opt(token.extra.get("isJurisLocatorLabel")) {
                tip_mut(state)?.variable_success = true;
            }
            if !state.tmp.just_looking
                && (js::truthy(&flags.non_parallel)
                    || js::truthy(&flags.parallel_last)
                    || js::truthy(&flags.parallel_first)
                    || js::truthy(&flags.parallel_delimiter_override)
                    || js::truthy(&flags.parallel_delimiter_override_on_suppress))
            {
                // Returns true ONLY if all variables listed on this group are repeats.
                let has_repeat = super::util_parallel::check_repeats(state, &flags)?;
                if has_repeat {
                    pop_last_child(state);
                }
                if state.tmp.cite_index > 0
                    && (has_repeat
                        || (!js::truthy(&flags.parallel_first)
                            && !js::truthy(&flags.parallel_last)
                            && !js::truthy(&flags.non_parallel)))
                {
                    let at = (state.tmp.cite_index - 1) as usize;
                    let info = state
                        .tmp
                        .suppress_repeats
                        .as_ref()
                        .and_then(|v| v.get(at))
                        .cloned()
                        .ok_or_else(|| {
                            EngineError::Csl(
                                "TypeError: Cannot read properties of undefined (suppress_repeats[cite_index-1])"
                                    .into(),
                            )
                        })?;
                    let flag = |k: &str| info.get(k).copied().unwrap_or(false);
                    let new_delim = if has_repeat
                        && js::truthy(&flags.parallel_delimiter_override_on_suppress)
                        && (flag("SIBLING") || flag("ORPHAN"))
                    {
                        Some(flags.parallel_delimiter_override_on_suppress.clone())
                    } else if js::truthy(&flags.parallel_delimiter_override) && flag("SIBLING") {
                        Some(flags.parallel_delimiter_override.clone())
                    } else {
                        None
                    };
                    if let Some(d) = new_delim {
                        // `state.output.queue.slice(-1)[0].parallel_delimiter = d`
                        // (a property of a bare string is lost).
                        if let Some(super::obj_blob::BlobChild::Blob(b)) =
                            queue::queue_children(state, QueueId::Output).last().cloned()
                        {
                            state.blobs.get_mut(b).extra.insert("parallel_delimiter".to_string(), d);
                        }
                    }
                }
            }
        } else {
            state.tmp.term_predecessor = flags.old_term_predecessor;
            {
                let t = tip_mut(state)?;
                t.variable_attempt = flags.variable_attempt;
                if flags.force_suppress && t.condition.is_none() {
                    t.variable_attempt = true;
                    t.variable_success = js::truthy(&flags.variable_success_parent);
                }
            }
            if flags.force_suppress {
                // 2019-04-15
                // This is removing variables done within the group we're
                // leaving from global done_vars? How does that make sense?
                // Ah. This is a FAILURE. So removing from done_vars allows it
                // to re-render later in the cite if desired.
                // Currently no tests fail from removing the condition, but
                // leaving it in.
                for done_var in &flags.done_vars {
                    // `jlen` is fixed before the loop while the array shrinks
                    // under it, as upstream's is.
                    let jlen = state.tmp.done_vars.len();
                    for j in 0..jlen {
                        if state.tmp.done_vars.get(j) == Some(done_var) {
                            state.tmp.done_vars.remove(j);
                        }
                    }
                }
            }
            pop_last_child(state);
        }
    }
    Ok(())
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
        state: &mut State,
        _token: &mut Token,
        item: &Value,
        cite_item: &Value,
    ) -> CslResult<bool> {
        match self {
            NodeGroupTest::InitJurisdictionMacros { macro_name } => {
                // `Item` is mutated (`best-jurisdiction`) upstream; the copy
                // carries it for this call.
                let mut item_full = item.clone();
                let cite = if js::truthy(cite_item) {
                    Some(cite_item)
                } else {
                    None
                };
                load::init_jurisdiction_macros(state, &mut item_full, cite, macro_name)
            }
        }
    }
}

/// `CSL.Node.group.build.call(token, state, target, realGroup)`.
pub fn build(
    state: &mut State,
    mut token: Token,
    target: &mut Vec<Token>,
    real_group: Option<bool>,
) -> CslResult<()> {
    // `this.realGroup = realGroup`: `undefined` (omitted from the dump) when the
    // caller passes no third argument.
    if let Some(rg) = real_group {
        token.extra.insert("realGroup".into(), Value::Bool(rg));
    }
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
            node_choose::build(state, choose_start, target, None)?;

            let mut if_start = Token::new("if", TokenType::Start);
            if_start.tests_defined = true;
            if_start
                .tests
                .push(Test::NodeGroup(NodeGroupTest::InitJurisdictionMacros {
                    macro_name: juris_name.clone(),
                }));
            if_start.test = Some(state.fun.match_.any(&if_start, state, &if_start.tests));
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
            node_if::build(state, if_end, target, None)?;
            let else_start = Token::new("else", TokenType::Start);
            node_else::build(state, else_start, target, None)?;
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
            node_else::build(state, else_end, target, None)?;
            let choose_end = Token::new("choose", TokenType::End);
            node_choose::build(state, choose_end, target, None)?;
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
