// Part of the kovan port of citeproc-js (GitHub #790, #792).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_nodes.js
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

//! Token execution and macro expansion (src/util_nodes.js): `CSL.tokenExec`,
//! `CSL.expandMacro`, `CSL.getMacroTarget`, `CSL.buildMacro`,
//! `CSL.configureMacro` and `CSL.XmlToToken`, plus the two dispatch tables
//! that stand for `CSL.Node[name].build` / `.configure` and
//! `CSL.Attributes[key]`.
//!
//! # Where tokens are built
//!
//! `CSL.Node[name].build.call(token, state, target, true)` takes the list it
//! pushes the token on as `target`. In this port a build is run on a list
//! **taken out of the state** (`std::mem::take`), because the builder also
//! gets `&mut State`. So while an area's tokens are being built, the area's
//! `tokens` field is empty and `target` is the list. Builders that push on a
//! *different* list (`node_sort`, `node_key` write `state[root + "_sort"]`)
//! reach it through the state. The one place where upstream's `target` and
//! `state[root + "_sort"].tokens` are the same list is macro expansion inside
//! a `<sort>` (`build.extension` set): [`State::expand_macro`] then builds the
//! macro straight into `target`, which is what `getMacroTarget` returns in
//! that case.
//!
//! # `undefined` jump targets
//!
//! `CSL.tokenExec` returns `token.next` (or `succeed`/`fail`, or a closure's
//! result); any of them may be `undefined`, which ends the
//! `while (next < tokens.length)` loops that call it. [`NEXT_UNDEFINED`]
//! stands for it: it is never below a token count.

use serde_json::Value;

use super::attributes;
use super::exec::Exec;
use super::js;
use super::load;
use super::obj_token::{Token, TokenType};
use super::state::State;
use super::util_processor;
use super::xmljson::{NodeId, XmlChild};
use super::{CslResult, EngineError};

/// A token index meaning JS `undefined` (see the module docs).
pub const NEXT_UNDEFINED: usize = usize::MAX;

/// The closures `src/util_nodes.js` stores in `token.execs` (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum UtilNodesExec {
    /// The first closure of `expandMacro` for a macro containing a date:
    /// `if (state.tmp.extension) state.tmp["doing-macro-with-date"] = true`
    /// (on the start token), or `= false` (on the end token).
    DoingMacroWithDate(bool),
    /// The closure that runs a macro: `while (next < state.macros[name].length)
    /// next = CSL.tokenExec.call(state, state.macros[name][next], Item, item)`.
    RunMacro {
        /// `macro_name` (`mkey`, with `@locale` appended for date macros).
        macro_name: String,
    },
}

impl UtilNodesExec {
    /// Run the closure.
    pub fn run(
        &self,
        state: &mut State,
        _token: &mut Token,
        item: &Value,
        cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            UtilNodesExec::DoingMacroWithDate(on) => {
                if !state.tmp.extension.is_empty() {
                    state.tmp.doing_macro_with_date = *on;
                }
                Ok(None)
            }
            UtilNodesExec::RunMacro { macro_name } => {
                let list = TokenList::Macro(macro_name.clone());
                let mut next = 0usize;
                loop {
                    let len = state.macros.get(macro_name).map(Vec::len).ok_or_else(|| {
                        EngineError::Csl(
                            "TypeError: Cannot read properties of undefined (reading 'length')"
                                .to_string(),
                        )
                    })?;
                    if next >= len {
                        break;
                    }
                    next = state.token_exec(&list, next, item, cite_item)?;
                }
                Ok(None)
            }
        }
    }
}

/// Where a built token list lives in the state, for `token_exec` (a token is
/// addressed by list and index, never by reference: its closures mutate it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenList {
    /// `state.citation.tokens`.
    Citation,
    /// `state.bibliography.tokens`.
    Bibliography,
    /// `state.intext.tokens`.
    Intext,
    /// `state.citation_sort.tokens`.
    CitationSort,
    /// `state.bibliography_sort.tokens`.
    BibliographySort,
    /// `state.macros[name]`.
    Macro(String),
}

/// What `CSL.getMacroTarget` returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroTarget {
    /// `false`: the macro is already built (not in an extension).
    Built,
    /// `this[root + extension].tokens`: build into the caller's `target`
    /// (see the module docs).
    Extension,
    /// A new list for the macro (`this.macros[mkey] = []`).
    New,
}

/// `CSL.Node[name].build.call(token, state, target, realGroup)`: dispatch to
/// the builder of the node called `name`. `Ok(false)` if there is no such
/// node (upstream: `CSL.Node[name]` undefined).
pub fn node_build(
    state: &mut State,
    name: &str,
    token: Token,
    target: &mut Vec<Token>,
    real_group: Option<bool>,
) -> CslResult<bool> {
    use super::*;
    match name {
        "alternative" => node_alternative::build(state, token, target, real_group)?,
        "alternative-text" => node_alternativetext::build(state, token, target, real_group)?,
        "bibliography" => node_bibliography::build(state, token, target, real_group)?,
        "choose" => node_choose::build(state, token, target, real_group)?,
        "citation" => node_citation::build(state, token, target, real_group)?,
        "#comment" => node_comment::build(state, token, target, real_group)?,
        "condition" => node_condition::build(state, token, target, real_group)?,
        "conditions" => node_conditions::build(state, token, target, real_group)?,
        "date" => node_date::build(state, token, target, real_group)?,
        "date-part" => node_datepart::build(state, token, target, real_group)?,
        "else" => node_else::build(state, token, target, real_group)?,
        "else-if" => node_elseif::build(state, token, target, real_group)?,
        "et-al" => node_etal::build(state, token, target, real_group)?,
        "group" => node_group::build(state, token, target, real_group)?,
        "if" => node_if::build(state, token, target, real_group)?,
        "info" => node_info::build(state, token, target, real_group)?,
        "institution" => node_institution::build(state, token, target, real_group)?,
        "institution-part" => node_institutionpart::build(state, token, target, real_group)?,
        "intext" => node_intext::build(state, token, target, real_group)?,
        "key" => node_key::build(state, token, target, real_group)?,
        "label" => node_label::build(state, token, target, real_group)?,
        "layout" => node_layout::build(state, token, target, real_group)?,
        "macro" => node_macro::build(state, token, target, real_group)?,
        "name" => node_name::build(state, token, target, real_group)?,
        "name-part" => node_namepart::build(state, token, target, real_group)?,
        "names" => node_names::build(state, token, target, real_group)?,
        "number" => node_number::build(state, token, target, real_group)?,
        "sort" => node_sort::build(state, token, target, real_group)?,
        "substitute" => node_substitute::build(state, token, target, real_group)?,
        "text" => node_text::build(state, token, target, real_group)?,
        _ => return Ok(false),
    }
    Ok(true)
}

/// Whether `CSL.Node[name]` exists.
pub fn node_exists(name: &str) -> bool {
    matches!(
        name,
        "alternative"
            | "alternative-text"
            | "bibliography"
            | "choose"
            | "citation"
            | "#comment"
            | "condition"
            | "conditions"
            | "date"
            | "date-part"
            | "else"
            | "else-if"
            | "et-al"
            | "group"
            | "if"
            | "info"
            | "institution"
            | "institution-part"
            | "intext"
            | "key"
            | "label"
            | "layout"
            | "macro"
            | "name"
            | "name-part"
            | "names"
            | "number"
            | "sort"
            | "substitute"
            | "text"
    )
}

/// `CSL.Node[name].configure.call(tokens[pos], state, pos)` for the nodes
/// that define `configure` (`choose`, `else`, `else-if`, `if`,
/// `institution`); a no-op for every other name.
pub fn node_configure(
    state: &mut State,
    name: &str,
    tokens: &mut [Token],
    pos: usize,
) -> CslResult<()> {
    use super::*;
    match name {
        "choose" => node_choose::configure(state, tokens, pos),
        "else" => node_else::configure(state, tokens, pos),
        "else-if" => node_elseif::configure(state, tokens, pos),
        "if" => node_if::configure(state, tokens, pos),
        "institution" => node_institution::configure(state, tokens, pos),
        _ => Ok(()),
    }
}

impl State {
    /// The token list `list` of the state, if it exists.
    pub fn token_list_mut(&mut self, list: &TokenList) -> Option<&mut Vec<Token>> {
        match list {
            TokenList::Citation => Some(&mut self.citation.tokens),
            TokenList::Bibliography => Some(&mut self.bibliography.tokens),
            TokenList::Intext => Some(&mut self.intext.tokens),
            TokenList::CitationSort => Some(&mut self.citation_sort.tokens),
            TokenList::BibliographySort => Some(&mut self.bibliography_sort.tokens),
            TokenList::Macro(name) => self.macros.get_mut(name),
        }
    }

    /// `CSL.tokenExec.call(state, token, Item, item)` for the token at
    /// `list[idx]`: run it and return the index of the next one to run.
    ///
    /// Upstream closures mutate their own token and the mutation persists, so
    /// the token is **taken out** of its list (`std::mem::take`) while its
    /// test and execs run on `&mut Token`, and put back afterwards (also on
    /// error). Execution only recurses into *other* tokens (a macro that
    /// calls itself is an error at build time), so the take is safe.
    ///
    /// If the token has a `test`, evaluate it, record `"succeed"` or
    /// `"fail"` on the `tmp.jump` stack and start from the token's
    /// `succeed`/`fail` index; then run every exec in order, a *truthy*
    /// (non-zero) result of which replaces the next index. [`NEXT_UNDEFINED`]
    /// is returned for a missing index.
    pub fn token_exec(
        &mut self,
        list: &TokenList,
        idx: usize,
        item: &Value,
        cite_item: &Value,
    ) -> CslResult<usize> {
        let missing = || {
            EngineError::Csl("TypeError: Cannot read properties of undefined (token)".to_string())
        };
        let mut token = {
            let tokens = self.token_list_mut(list).ok_or_else(missing)?;
            std::mem::take(tokens.get_mut(idx).ok_or_else(missing)?)
        };
        let r = self.run_token(&mut token, item, cite_item);
        if let Some(slot) = self.token_list_mut(list).and_then(|t| t.get_mut(idx)) {
            *slot = token;
        }
        r
    }

    /// The body of `CSL.tokenExec` on a token that is out of its list.
    fn run_token(
        &mut self,
        token: &mut Token,
        item: &Value,
        cite_item: &Value,
    ) -> CslResult<usize> {
        let mut next = token.next.unwrap_or(NEXT_UNDEFINED);
        if let Some(test) = token.test.clone() {
            if test.eval(self, token, item, cite_item)? {
                self.tmp
                    .jump
                    .replace(Value::String("succeed".to_string()))?;
                next = token.succeed.unwrap_or(NEXT_UNDEFINED);
            } else {
                self.tmp.jump.replace(Value::String("fail".to_string()))?;
                next = token.fail.unwrap_or(NEXT_UNDEFINED);
            }
        }
        let mut i = 0;
        while i < token.execs.len() {
            let exec = token.execs[i].clone();
            if let Some(maybenext) = exec.run(self, token, item, cite_item)? {
                // `if (maybenext)`: index 0 is falsy.
                if maybenext != 0 {
                    next = maybenext;
                }
            }
            i += 1;
        }
        Ok(next)
    }

    /// `CSL.expandMacro.call(state, macro_key_token, target)`: build the
    /// macro named by `macro_key_token.postponed_macro` as a `group` around a
    /// `text` token that runs it, pushing the group tokens on `target`.
    ///
    /// `postponed_macro` and `strings.sort_direction` are read from
    /// `macro_key_token`; the group tokens are new. The macro body is built
    /// once, into `state.macros[mkey]` (or, inside a `<sort>`, straight into
    /// `target`: see the module docs).
    pub fn expand_macro(
        &mut self,
        macro_key_token: &Token,
        target: &mut Vec<Token>,
    ) -> CslResult<()> {
        let mut mkey = macro_key_token.postponed_macro.clone().unwrap_or_default();
        let sort_direction = macro_key_token.strings.get("sort_direction").cloned();

        // Decorations and affixes are in wrapper applied in cs:text
        let mut macro_key_token = Token::new("group", TokenType::Start);

        let mut has_date = false;
        let mut macroid = Value::Bool(false);
        let macro_nodes = self
            .csl_xml
            .get_nodes_by_name(self.csl_xml.data_obj, "macro", &mkey);
        if !macro_nodes.is_empty() {
            macroid = self.csl_xml.get_attribute_value(macro_nodes[0], "cslid");
            has_date = js::truthy(
                &self
                    .csl_xml
                    .get_attribute_value(macro_nodes[0], "macro-has-date"),
            );
        }
        if has_date {
            mkey = format!(
                "{mkey}@{}",
                match &self.build.current_default_locale {
                    Value::Null => "undefined".to_string(),
                    v => js::to_js_string(v),
                }
            );
            macro_key_token
                .execs
                .push(Exec::UtilNodes(UtilNodesExec::DoingMacroWithDate(true)));
        }

        if self.build.macro_stack.contains(&mkey) {
            return Err(EngineError::Csl(format!(
                "CSL processor error: call to macro \"{mkey}\" would cause an infinite loop"
            )));
        }
        self.build.macro_stack.push(mkey.clone());

        macro_key_token.extra.insert("cslid".into(), macroid);

        let mut juris = false;
        if load::is_module_macro(&mkey) {
            macro_key_token
                .extra
                .insert("juris".into(), Value::String(mkey.clone()));
            juris = true;
            self.opt
                .insert("update_mode".into(), Value::from(load::POSITION));
        }
        // Macro group is treated as a real node in the style
        node_build(self, "group", macro_key_token, target, Some(true))?;

        // `if (!this.cslXml.getNodeValue(macro_nodes)) CSL.error("CSL style
        // error: undefined macro ...")`: getNodeValue of a list is the list,
        // which is always truthy, so this never fires (xmljson.rs docs).

        // Let's macro
        match self.get_macro_target(&mkey) {
            MacroTarget::Built => {}
            MacroTarget::Extension => {
                self.build_macro(target, &macro_nodes)?;
                self.configure_macro(None)?;
            }
            MacroTarget::New => {
                let mut mytarget: Vec<Token> = Vec::new();
                self.build_macro(&mut mytarget, &macro_nodes)?;
                self.configure_macro(Some(&mut mytarget))?;
                self.macros.insert(mkey.clone(), mytarget);
            }
        }
        if self.build.extension.is_empty() {
            let mut text_node = Token::new("text", TokenType::Singleton);
            text_node
                .execs
                .push(Exec::UtilNodes(UtilNodesExec::RunMacro {
                    macro_name: mkey.clone(),
                }));
            target.push(text_node);
        }

        // Decorations and affixes are in wrapper applied in cs:text
        let mut end_of_macro = Token::new("group", TokenType::End);
        if let Some(sd) = sort_direction {
            end_of_macro.strings.insert("sort_direction".into(), sd);
        }
        if has_date {
            end_of_macro
                .execs
                .push(Exec::UtilNodes(UtilNodesExec::DoingMacroWithDate(false)));
        }
        if juris {
            end_of_macro
                .extra
                .insert("juris".into(), Value::String(mkey.clone()));
        }
        // Macro group is treated as a real node in the style
        node_build(self, "group", end_of_macro, target, Some(true))?;

        self.build.macro_stack.pop();
        Ok(())
    }

    /// `CSL.getMacroTarget.call(state, mkey)`: where the macro's tokens go.
    pub fn get_macro_target(&self, mkey: &str) -> MacroTarget {
        if !self.build.extension.is_empty() {
            MacroTarget::Extension
        } else if !self.macros.contains_key(mkey) {
            MacroTarget::New
        } else {
            MacroTarget::Built
        }
    }

    /// `CSL.buildMacro.call(state, mytarget, macro_nodes)`: build the first
    /// of `macro_nodes` (and its subtree) into `mytarget`.
    pub fn build_macro(
        &mut self,
        mytarget: &mut Vec<Token>,
        macro_nodes: &[NodeId],
    ) -> CslResult<()> {
        let mynode: Vec<XmlChild> = macro_nodes
            .first()
            .map(|n| vec![XmlChild::Node(*n)])
            .unwrap_or_default();
        self.run_builder(&mynode, mytarget)
    }

    /// `CSL.configureMacro.call(state, mytarget)`: configure the macro's
    /// token list unless building a sort (`build.extension`). `mytarget` is
    /// `None` for the extension case, where the list is the sort area's and
    /// is configured with the areas.
    pub fn configure_macro(&mut self, mytarget: Option<&mut Vec<Token>>) -> CslResult<()> {
        if self.build.extension.is_empty() {
            if let Some(t) = mytarget {
                self.configure_token_list(t)?;
            }
        }
        Ok(())
    }

    /// `CSL.XmlToToken.call(node, state, tokentype, explicitTarget,
    /// var_stack)`: turn one XML node (START, END or SINGLETON) into a token
    /// and hand it to its node builder.
    ///
    /// The node's formatting attributes become the token's `decorations`
    /// ([`util_processor::set_decorations`]); each other attribute, in
    /// document order, runs its `CSL.Attributes` handler (`attributes::apply`)
    /// on the token. An END token only runs `@language`/`@locale` (and only
    /// for `if`, `else-if` and `layout` at all). A START token whose
    /// `@variable` is a date variable pushes its `variables` on `var_stack`
    /// for the END token to take back.
    pub fn xml_to_token(
        &mut self,
        node: NodeId,
        tokentype: TokenType,
        explicit_target: &mut Vec<Token>,
        var_stack: &mut Vec<Vec<String>>,
    ) -> CslResult<()> {
        let name = self.csl_xml.nodename(node);
        if let Some(skip) = self.build.skip.as_deref() {
            if skip != name {
                return Ok(());
            }
        }
        if name.is_empty() {
            let txt = self.csl_xml.content(node);
            if !txt.is_empty() {
                self.build.text = Value::String(txt);
            }
            return Ok(());
        }
        if !node_exists(&name) {
            return Err(EngineError::Csl(format!("Undefined node name \"{name}\".")));
        }
        if self.build.builder_depth == 1 && self.build.area == "bibliography" {
            // `state.bibliography.tokens.length` for `@display`.
            self.build.bibliography_tokens_len = explicit_target.len();
        }
        let mut attributes = self.csl_xml.attributes(node);
        let decorations = util_processor::set_decorations(self, &mut attributes);
        let mut token = Token::new(&name, tokentype);
        let variable = attributes
            .iter()
            .find(|(k, _)| k == "@variable")
            .map(|(_, v)| v.clone());
        let variable_is_date = variable
            .as_ref()
            .and_then(Value::as_str)
            .map(|v| load::DATE_VARIABLES.contains(&v))
            .unwrap_or(false);
        if tokentype != TokenType::End || name == "if" || name == "else-if" || name == "layout" {
            for (key, value) in &attributes {
                if tokentype == TokenType::End && key != "@language" && key != "@locale" {
                    continue;
                }
                let arg = js::to_js_string(value);
                match attributes::apply(self, &mut token, key, &arg) {
                    Ok(_defined) => {
                        // `CSL.debug("warning: undefined attribute ...")` when
                        // !_defined: dropped.
                    }
                    Err(EngineError::NotYetPorted { method }) => {
                        return Err(EngineError::NotYetPorted { method })
                    }
                    Err(EngineError::Csl(e)) => {
                        return Err(EngineError::Csl(format!(
                            "{key} attribute: citeproc-js error: {e}"
                        )))
                    }
                    Err(other) => return Err(other),
                }
            }
            token.decorations = decorations;
            if variable_is_date {
                var_stack.push(token.variables.clone());
            }
        } else if tokentype == TokenType::End && js::truthy_opt(variable.as_ref()) {
            token.extra.insert("hasVariable".into(), Value::Bool(true));
            if variable_is_date {
                token.variables = var_stack.pop().unwrap_or_default();
            }
        }
        //
        // !!!!!: eliminate diversion of tokens to separate
        // token list (formerly used for reading in macros
        // and terms).
        //
        // True flags real nodes in the style
        node_build(self, &name, token, explicit_target, Some(true))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_exec_runs_a_macro_and_puts_the_token_back() {
        let mut st = State::default();
        let mut a = Token::new("group", TokenType::Start);
        a.next = Some(1);
        let mut b = Token::new("group", TokenType::End);
        b.next = Some(2);
        st.macros.insert("m".to_string(), vec![a, b]);
        let mut caller = Token::new("text", TokenType::Singleton);
        caller.next = Some(5);
        caller.execs.push(Exec::UtilNodes(UtilNodesExec::RunMacro {
            macro_name: "m".to_string(),
        }));
        st.citation.tokens = vec![caller];
        let next = st
            .token_exec(&TokenList::Citation, 0, &Value::Null, &Value::Null)
            .unwrap();
        assert_eq!(
            next, 5,
            "the macro run returns nothing, so token.next stands"
        );
        assert_eq!(st.citation.tokens[0].execs.len(), 1, "token put back");
        assert!(st
            .token_exec(&TokenList::Citation, 3, &Value::Null, &Value::Null)
            .is_err());
        assert_eq!(
            st.token_exec(&TokenList::Macro("m".into()), 1, &Value::Null, &Value::Null)
                .unwrap(),
            2
        );
    }

    #[test]
    fn dispatch_knows_exactly_the_csl_node_names() {
        for n in [
            "text",
            "if",
            "else-if",
            "date-part",
            "#comment",
            "name-part",
            "alternative-text",
        ] {
            assert!(node_exists(n), "{n}");
        }
        assert!(!node_exists("style"));
        assert!(!node_exists("term"));
    }
}
