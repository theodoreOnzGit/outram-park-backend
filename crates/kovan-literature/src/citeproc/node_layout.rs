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
use super::node_choose;
use super::node_else;
use super::node_elseif;
use super::node_if;
use super::node_names::decorations_to_value;
use super::obj_token::{Token, TokenType};
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
        _token: &Token,
        _item: &Value,
        _cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            // PORT-LATER(wave1-output): node_layout.js:50-66, needs
            // state.output.startTag + current.value() fields (queue.rs) and
            // state.sys.wrapCitationEntry.
            NodeLayoutExec::CiteEntryStart => Err(EngineError::NotYetPorted {
                method: "node_layout.js:50 closure",
            }),
            // PORT-LATER(wave4): node_layout.js:78-100, needs
            // state.registry.registry[Item.id].parallel (registry.rs),
            // state.tmp.abbrev_trimmer and state.opt.suppressedJurisdictions.
            NodeLayoutExec::InitDoneVars => Err(EngineError::NotYetPorted {
                method: "node_layout.js:78 closure",
            }),
            NodeLayoutExec::ClearSortKeyFlag => {
                // just in case
                state.tmp.sort_key_flag = false;
                Ok(None)
            }
            NodeLayoutExec::ResetNamesetCounter => {
                state.tmp.nameset_counter = 0;
                Ok(None)
            }
            // PORT-LATER(wave1-output): node_layout.js:114-117, needs
            // state.output.openLevel(new CSL.Token()) (queue.rs).
            NodeLayoutExec::OpenLevel => Err(EngineError::NotYetPorted {
                method: "node_layout.js:114 closure",
            }),
            // PORT-LATER(wave1-output): node_layout.js:122-131, needs
            // CSL.checkPrefixSpaceAppend, state.output.checkNestedBrace,
            // CSL.checkIgnorePredecessor and state.output.append.
            NodeLayoutExec::CitationPrefix => Err(EngineError::NotYetPorted {
                method: "node_layout.js:122 closure",
            }),
            // PORT-LATER(wave1-output): node_layout.js:11-33, needs
            // state.output.current.value() and endTag("bib_other").
            NodeLayoutExec::BibliographySuffix => Err(EngineError::NotYetPorted {
                method: "node_layout.js:11 closure",
            }),
            // PORT-LATER(wave1-output): node_layout.js:~230-239, needs
            // CSL.checkSuffixSpacePrepend, checkNestedBrace and output.append.
            NodeLayoutExec::CitationSuffix => Err(EngineError::NotYetPorted {
                method: "node_layout.js:230 closure",
            }),
            // PORT-LATER(wave1-output): node_layout.js:~245, needs output.closeLevel.
            NodeLayoutExec::CloseLevel => Err(EngineError::NotYetPorted {
                method: "node_layout.js:245 closure",
            }),
            // PORT-LATER(wave1-output): node_layout.js:~248-257, needs output.endTag.
            NodeLayoutExec::CiteEntryEnd => Err(EngineError::NotYetPorted {
                method: "node_layout.js:248 closure",
            }),
        }
    }
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
    _real_group: bool,
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
        token.execs.push(Exec::NodeLayout(NodeLayoutExec::InitDoneVars));
        // set opt delimiter
        token
            .execs
            .push(Exec::NodeLayout(NodeLayoutExec::ClearSortKeyFlag));
        // reset nameset counter [all nodes]
        token
            .execs
            .push(Exec::NodeLayout(NodeLayoutExec::ResetNamesetCounter));
        token.execs.push(Exec::NodeLayout(NodeLayoutExec::OpenLevel));
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
        my_tok.extra.insert("locale".into(), Value::String(l.clone()));
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
                node_else::build(state, tok, target, false)?;
            }
        } // !this.locale_raw

        // Conditionals
        if let Some(raw) = &locale_raw {
            if !state.build.layout_locale_flag {
                // if layout_locale_flag is untrue,
                // write cs:choose START and cs:if START
                // to the token list.
                let choose_tok = Token::new("choose", TokenType::Start);
                node_choose::build(state, choose_tok, target, false)?;
                my_tok.name = "if".to_string();
                attributes::apply(state, &mut my_tok, "@locale-internal", raw)?;
                node_if::build(state, my_tok.clone(), target, false)?;
            } else {
                // if build_layout_locale_flag is true,
                // write cs:else-if START to the token list.
                my_tok.name = "else-if".to_string();
                attributes::apply(state, &mut my_tok, "@locale-internal", raw)?;
                node_elseif::build(state, my_tok.clone(), target, false)?;
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
                node_if::build(state, my_tok, target, false)?;
                state.build.layout_locale_flag = true;
            } else {
                // If layout_locale_flag is true, write cs:else-if END
                // to the token list.
                my_tok.name = "else-if".to_string();
                my_tok.tokentype = TokenType::End;
                attributes::apply(state, &mut my_tok, "@locale-internal", raw)?;
                node_elseif::build(state, my_tok, target, false)?;
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
                    node_else::build(state, tok, target, false)?;
                    let tok = Token::new("choose", TokenType::End);
                    node_choose::build(state, tok, target, false)?;
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
            token.execs.push(Exec::NodeLayout(NodeLayoutExec::CloseLevel));
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
