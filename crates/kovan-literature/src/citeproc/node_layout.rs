// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/node_layout.js
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

//! Port of `src/node_layout.js`: `CSL.Node.layout`.

use serde_json::Value;

use super::attributes::{self, area_mut};
use super::exec::Exec;
use super::js;
use super::load::{check_ignore_predecessor, check_prefix_space_append, check_suffix_space_prepend};
use super::node_choose;
use super::node_else;
use super::node_elseif;
use super::node_if;
use super::node_names::decorations_to_value;
use super::obj_blob::{BlobChild, BlobContent};
use super::obj_token::{Token, TokenType};
use super::queue::{self, AppendArg, FormatRef, QueueId};
use super::state::State;
use super::{CslResult, EngineError};

/// The closures `src/node_layout.js` stores in `token.execs` (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeLayoutExec {
    /// START: open the citation-entry wrapper (node_layout.js:50-66).
    CiteEntryStart,
    /// START (first layout of an area): initialise `done_vars` etc.
    /// (node_layout.js:78-100).
    InitDoneVars,
    /// START: `state.tmp.sort_key_flag = false` (node_layout.js:104-106).
    ClearSortKeyFlag,
    /// START: `state.tmp.nameset_counter = 0` (node_layout.js:110-112).
    ResetNamesetCounter,
    /// START: `state.output.openLevel(new CSL.Token())` (node_layout.js:114-117).
    OpenLevel,
    /// On the citation prefix text token (node_layout.js:122-131).
    CitationPrefix,
    /// On the bibliography suffix text token (`setSuffix`, node_layout.js:11-33).
    BibliographySuffix,
    /// On the citation suffix text token (node_layout.js:~230-239).
    CitationSuffix,
    /// END: `state.output.closeLevel()` (node_layout.js:~245-247).
    CloseLevel,
    /// END: close the citation-entry wrapper (node_layout.js:~248-257).
    CiteEntryEnd,
}

impl NodeLayoutExec {
    /// Run the closure.
    pub fn run(
        &self,
        state: &mut State,
        token: &mut Token,
        item: &Value,
        cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            NodeLayoutExec::CiteEntryStart => {
                if wraps_citation_entry(state, item) {
                    // PORT-LATER(sys.wrapCitationEntry): the host's
                    // `wrapCitationEntry` callback is not modelled (`Sys` is data
                    // only); `HostHooks::wrap_citation_entry` is never set.
                    return Err(EngineError::NotYetPorted {
                        method: "sys.wrapCitationEntry",
                    });
                }
                Ok(None)
            }
            NodeLayoutExec::InitDoneVars => {
                //
                // done_vars is used to prevent the repeated
                // rendering of variables
                //
                // initalize done vars
                state.tmp.done_vars = Vec::new();
                if js::truthy_opt(cite_item.get("author-only")) {
                    state.tmp.done_vars.push("locator".to_string());
                }
                let country = item.get("country").filter(|c| js::truthy(c));
                let suppressed = country
                    .map(|c| {
                        js::truthy_opt(
                            state
                                .opt
                                .get("suppressedJurisdictions")
                                .and_then(|s| s.get(js::to_js_string(c))),
                        )
                    })
                    .unwrap_or(false);
                let item_type = item.get("type").and_then(Value::as_str);
                if suppressed && item_type != Some("treaty") && item_type != Some("patent") {
                    state.tmp.done_vars.push("country".to_string());
                }
                let id = super::registry::id_key(item.get("id"));
                if !state.tmp.just_looking
                    && state
                        .registry
                        .registry
                        .get(&id)
                        .map(|t| js::truthy_opt(t.parallel.as_ref()))
                        .unwrap_or(false)
                {
                    state
                        .tmp
                        .done_vars
                        .push("first-reference-note-number".to_string());
                }
                // trimmer is not available in getAmbiguousCite
                if !state.tmp.just_looking && js::truthy_opt(item.get("jurisdiction")) {
                    if let Some(trimmer) = &state.tmp.abbrev_trimmer {
                        let jurisdiction =
                            js::to_js_string(item.get("jurisdiction").unwrap_or(&Value::Null));
                        let fields: Vec<String> = trimmer
                            .quashes
                            .get(&jurisdiction)
                            .map(|q| q.keys().cloned().collect())
                            .unwrap_or_default();
                        state.tmp.done_vars.extend(fields);
                    }
                }

                //CSL.debug(" === init rendered_name === ");
                state.tmp.rendered_name = Value::Bool(false);
                Ok(None)
            }
            NodeLayoutExec::ClearSortKeyFlag => {
                // just in case
                state.tmp.sort_key_flag = false;
                Ok(None)
            }
            NodeLayoutExec::ResetNamesetCounter => {
                state.tmp.nameset_counter = 0;
                Ok(None)
            }
            NodeLayoutExec::OpenLevel => {
                // `var tok = new CSL.Token(); state.output.openLevel(tok)`
                let tok = Token::new("", TokenType::Start);
                queue::open_level(state, QueueId::Output, FormatRef::Token(tok))?;
                Ok(None)
            }
            NodeLayoutExec::CitationPrefix => {
                if let Some(prefix) = cite_item.get("prefix").filter(|p| js::truthy(p)) {
                    let mut prefix = check_prefix_space_append(state, &js::to_js_string(prefix));
                    if !state.tmp.just_looking {
                        prefix = update_nested_brace(state, &prefix)?;
                    }
                    let ignore_predecessor = check_ignore_predecessor(state, &prefix);
                    queue::append(
                        state,
                        QueueId::Output,
                        AppendArg::Text(prefix),
                        FormatRef::Token(token.clone()),
                        false,
                        ignore_predecessor,
                        false,
                    )?;
                }
                Ok(None)
            }
            NodeLayoutExec::BibliographySuffix => {
                // Suppress suffix on all but the last item in bibliography parallels
                if !state.tmp.parallel_and_not_last {
                    let locale = state
                        .tmp
                        .last_cite_locale
                        .clone()
                        .unwrap_or_else(|| "false".to_string());
                    let own = state
                        .tmp
                        .cite_affixes
                        .get(&state.tmp.area)
                        .and_then(|a| a.get(&locale))
                        .filter(|a| js::truthy(a))
                        .map(|a| a.get("suffix").cloned().unwrap_or(Value::Null));
                    let suffix = match own {
                        Some(s) => s,
                        None => state
                            .bibliography
                            .opt
                            .get("layout_suffix")
                            .cloned()
                            .unwrap_or(Value::Null),
                    };
                    let suffix = match suffix {
                        Value::Null => String::new(),
                        other => js::to_js_string(&other),
                    };

                    // If @display is used, layout suffix is placed on the last
                    // immediate child of the layout, which we assume will be a
                    // @display group node.
                    let top = queue::current(state, QueueId::Output).ok_or_else(|| {
                        EngineError::Csl(
                            "TypeError: Cannot read properties of undefined (reading 'strings')"
                                .into(),
                        )
                    })?;
                    if js::truthy_opt(state.opt.get("using_display")) {
                        let last = match &state.blobs.get(top).blobs {
                            BlobContent::List(l) => l.last().cloned(),
                            BlobContent::Text(_) => None,
                        };
                        match last {
                            Some(BlobChild::Blob(b)) => state.blobs.get_mut(b).set_string("suffix", &suffix),
                            _ => {
                                return Err(EngineError::Csl(
                                    "TypeError: Cannot read properties of undefined (reading 'strings')".into(),
                                ))
                            }
                        }
                    } else {
                        state.blobs.get_mut(top).set_string("suffix", &suffix);
                    }
                }
                if js::truthy_opt(state.bibliography.opt.get("second-field-align")) {
                    // closes bib_other
                    queue::end_tag(state, QueueId::Output, Some("bib_other"))?;
                }
                Ok(None)
            }
            NodeLayoutExec::CitationSuffix => {
                if let Some(suffix) = cite_item.get("suffix").filter(|s| js::truthy(s)) {
                    let mut suffix = check_suffix_space_prepend(state, &js::to_js_string(suffix));
                    if !state.tmp.just_looking {
                        suffix = update_nested_brace(state, &suffix)?;
                    }
                    queue::append_simple(
                        state,
                        QueueId::Output,
                        suffix.as_str(),
                        FormatRef::Token(token.clone()),
                    )?;
                }
                Ok(None)
            }
            NodeLayoutExec::CloseLevel => {
                queue::close_level(state, QueueId::Output, None)?;
                Ok(None)
            }
            NodeLayoutExec::CiteEntryEnd => {
                if wraps_citation_entry(state, item) {
                    // closes citation link wrapper
                    queue::end_tag(state, QueueId::Output, None)?;
                }
                Ok(None)
            }
        }
    }
}

/// `state.opt.development_extensions.apply_citation_wrapper &&
/// state.sys.wrapCitationEntry && !state.tmp.just_looking && Item.system_id &&
/// state.tmp.area === "citation"`.
fn wraps_citation_entry(state: &State, item: &Value) -> bool {
    state.dev_ext("apply_citation_wrapper")
        && state.fun.host_hooks.wrap_citation_entry
        && !state.tmp.just_looking
        && js::truthy_opt(item.get("system_id"))
        && state.tmp.area == "citation"
}

/// `state.output.checkNestedBrace.update(s)`.
fn update_nested_brace(state: &mut State, s: &str) -> CslResult<String> {
    state
        .output
        .check_nested_brace
        .as_mut()
        .map(|c| c.update(s))
        .ok_or_else(|| {
            EngineError::Csl(
                "TypeError: Cannot read properties of undefined (reading 'update')".into(),
            )
        })
}

fn cite_affixes_truthy(state: &State, area: &str) -> bool {
    js::truthy_opt(state.tmp.cite_affixes.get(area))
}

/// `function setSuffix()` (node_layout.js:8-34).
fn set_suffix(state: &State, target: &mut Vec<Token>) {
    if state.build.area == "bibliography" {
        let mut suffix_token = Token::new("text", TokenType::Singleton);
        suffix_token
            .execs
            .push(Exec::NodeLayout(NodeLayoutExec::BibliographySuffix));
        target.push(suffix_token);
    }
}

/// `CSL.Node.layout.build.call(token, state, target)`.
pub fn build(
    state: &mut State,
    mut token: Token,
    target: &mut Vec<Token>,
    _real_group: Option<bool>,
) -> CslResult<()> {
    let locale_raw: Option<String> = token
        .extra
        .get("locale_raw")
        .filter(|v| js::truthy(v))
        .map(js::to_js_string);

    if token.tokentype == TokenType::Start {
        match &locale_raw {
            Some(l) => state.build.current_default_locale = Value::String(l.clone()),
            None => {
                state.build.current_default_locale = state
                    .opt
                    .get("default-locale")
                    .cloned()
                    .unwrap_or(Value::Null)
            }
        }

        token
            .execs
            .push(Exec::NodeLayout(NodeLayoutExec::CiteEntryStart));
    }

    // XXX Works, but using state.tmp looks wrong here? We're in the build layer ...
    if token.tokentype == TokenType::Start && !cite_affixes_truthy(state, &state.build.area) {
        //
        // done_vars is used to prevent the repeated
        // rendering of variables
        //
        // initalize done vars
        token
            .execs
            .push(Exec::NodeLayout(NodeLayoutExec::InitDoneVars));
        // set opt delimiter
        token
            .execs
            .push(Exec::NodeLayout(NodeLayoutExec::ClearSortKeyFlag));
        // reset nameset counter [all nodes]
        token
            .execs
            .push(Exec::NodeLayout(NodeLayoutExec::ResetNamesetCounter));
        token
            .execs
            .push(Exec::NodeLayout(NodeLayoutExec::OpenLevel));
        target.push(token.clone());

        if state.build.area == "citation" {
            let mut prefix_token = Token::new("text", TokenType::Singleton);
            prefix_token
                .execs
                .push(Exec::NodeLayout(NodeLayoutExec::CitationPrefix));
            target.push(prefix_token);
        }
    }

    // Cast token to be used in one of the configurations below.
    let mut my_tok = Token::new("dummy", TokenType::Start);
    if let Some(l) = &locale_raw {
        my_tok
            .extra
            .insert("locale".into(), Value::String(l.clone()));
        if let Some(d) = token.strings.get("delimiter") {
            my_tok.strings.insert("delimiter".into(), d.clone());
        }
        if let Some(s) = token.strings.get("suffix") {
            my_tok.strings.insert("suffix".into(), s.clone());
        }
        if !cite_affixes_truthy(state, &state.build.area) {
            state
                .tmp
                .cite_affixes
                .insert(state.build.area.clone(), Value::Object(js::Obj::new()));
        }
    }

    if token.tokentype == TokenType::Start {
        state.build.layout_flag = true;

        // Only run the following once, to set up the final layout node ...
        if locale_raw.is_none() {
            //
            // save out decorations for flipflop processing [final node only]
            //
            let deco = Value::Array(vec![decorations_to_value(&token.decorations)]);
            let tmp_area = state.tmp.area.clone();
            area_mut(state, &tmp_area)?
                .opt
                .insert("topdecor".into(), deco.clone());
            area_mut(state, &format!("{tmp_area}_sort"))?
                .opt
                .insert("topdecor".into(), deco);

            let build_area = state.build.area.clone();
            let opt = &mut area_mut(state, &build_area)?.opt;
            opt.insert(
                "layout_prefix".into(),
                token.strings.get("prefix").cloned().unwrap_or(Value::Null),
            );
            opt.insert(
                "layout_suffix".into(),
                token.strings.get("suffix").cloned().unwrap_or(Value::Null),
            );
            match token.strings.get("delimiter") {
                Some(d) => {
                    opt.insert("layout_delimiter".into(), d.clone());
                }
                None => {
                    opt.remove("layout_delimiter");
                }
            }
            opt.insert(
                "layout_decorations".into(),
                decorations_to_value(&token.decorations),
            );

            // Only do this if we're running conditionals
            if cite_affixes_truthy(state, &build_area) {
                // if build_layout_locale_flag is true,
                // write cs:else START to the token list.
                let tok = Token::new("else", TokenType::Start);
                node_else::build(state, tok, target, None)?;
            }
        } // !this.locale_raw

        // Conditionals
        if let Some(raw) = &locale_raw {
            if !state.build.layout_locale_flag {
                // if layout_locale_flag is untrue,
                // write cs:choose START and cs:if START
                // to the token list.
                let choose_tok = Token::new("choose", TokenType::Start);
                node_choose::build(state, choose_tok, target, None)?;
                my_tok.name = "if".to_string();
                attributes::apply(state, &mut my_tok, "@locale-internal", raw)?;
                node_if::build(state, my_tok.clone(), target, None)?;
            } else {
                // if build_layout_locale_flag is true,
                // write cs:else-if START to the token list.
                my_tok.name = "else-if".to_string();
                attributes::apply(state, &mut my_tok, "@locale-internal", raw)?;
                node_elseif::build(state, my_tok.clone(), target, None)?;
            }
            // cite_affixes for this node
            let locale = my_tok
                .extra
                .get("locale")
                .map(js::to_js_string)
                .unwrap_or_default();
            let mut entry = js::Obj::new();
            if let Some(d) = token.strings.get("delimiter") {
                entry.insert("delimiter".into(), d.clone());
            }
            if let Some(s) = token.strings.get("suffix") {
                entry.insert("suffix".into(), s.clone());
            }
            let build_area = state.build.area.clone();
            let slot = state
                .tmp
                .cite_affixes
                .entry(build_area)
                .or_insert_with(|| Value::Object(js::Obj::new()));
            if let Value::Object(o) = slot {
                o.insert(locale, Value::Object(entry));
            }
        }
    }
    if token.tokentype == TokenType::End {
        if let Some(raw) = &locale_raw {
            set_suffix(state, target);
            if !state.build.layout_locale_flag {
                // If layout_locale_flag is untrue, write cs:if END
                // to the token list.
                my_tok.name = "if".to_string();
                my_tok.tokentype = TokenType::End;
                attributes::apply(state, &mut my_tok, "@locale-internal", raw)?;
                node_if::build(state, my_tok, target, None)?;
                state.build.layout_locale_flag = true;
            } else {
                // If layout_locale_flag is true, write cs:else-if END
                // to the token list.
                my_tok.name = "else-if".to_string();
                my_tok.tokentype = TokenType::End;
                attributes::apply(state, &mut my_tok, "@locale-internal", raw)?;
                node_elseif::build(state, my_tok, target, None)?;
            }
        }
        if locale_raw.is_none() {
            set_suffix(state, target);
            // Only add this if we're running conditionals
            if cite_affixes_truthy(state, &state.build.area) {
                // If layout_locale_flag is true, write cs:else END
                // and cs:choose END to the token list.
                if state.build.layout_locale_flag {
                    let tok = Token::new("else", TokenType::End);
                    node_else::build(state, tok, target, None)?;
                    let tok = Token::new("choose", TokenType::End);
                    node_choose::build(state, tok, target, None)?;
                }
            }
            state.build_layout_locale_flag = true;
            if state.build.area == "citation" {
                let mut suffix_token = Token::new("text", TokenType::Singleton);
                suffix_token
                    .execs
                    .push(Exec::NodeLayout(NodeLayoutExec::CitationSuffix));
                target.push(suffix_token);
            }

            // Closes wrapper token
            token
                .execs
                .push(Exec::NodeLayout(NodeLayoutExec::CloseLevel));
            token
                .execs
                .push(Exec::NodeLayout(NodeLayoutExec::CiteEntryEnd));
            target.push(token);
            state.build.layout_flag = false;
            state.build.layout_locale_flag = false;
        } // !this.layout_raw
    }
    Ok(())
}
