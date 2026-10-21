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
use super::js::{self, Obj};
use super::node_choose;
use super::node_if;
use super::obj_token::{Token, TokenType};
use super::queue::{self, FormatRef, QueueId};
use super::state::State;
use super::util_locale::locale_resolve;
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
        token: &mut Token,
        item: &Value,
        _cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            NodeAlternativeExec::Start => {
                state.tmp.old_item = Some(item.clone());
                state.tmp.old_lang = js::get_string(&state.opt, "lang");
                state.tmp.abort_alternative = true;

                let mut new_item: Option<Value> = None;
                if js::truthy_opt(item.get("language-name"))
                    && js::truthy_opt(item.get("language-name-original"))
                {
                    let mut ni: Obj = item.as_object().cloned().unwrap_or_default();

                    let lang_name = ni.get("language-name").cloned().unwrap_or(Value::Null);
                    ni.insert("language".into(), lang_name.clone());
                    let default_locale = state
                        .opt
                        .get("default-locale")
                        .and_then(|d| d.get(0))
                        .map(js::to_js_string)
                        .unwrap_or_default();
                    let langspec =
                        locale_resolve(&js::to_js_string(&lang_name), Some(&default_locale));

                    if js::truthy_opt(state.opt.get("multi_layout")) {
                        let layouts: Vec<Value> = state
                            .opt
                            .get("multi_layout")
                            .and_then(Value::as_array)
                            .cloned()
                            .unwrap_or_default();
                        for locale_list in &layouts {
                            let mut gotlang: Option<String> = None;
                            for tryspec in locale_list.as_array().into_iter().flatten() {
                                let field = |k: &str| {
                                    tryspec.get(k).and_then(Value::as_str).map(str::to_string)
                                };
                                if Some(langspec.best.clone()) == field("best")
                                    || Some(langspec.base.clone()) == field("base")
                                    || Some(langspec.bare.clone()) == field("bare")
                                {
                                    gotlang = locale_list
                                        .get(0)
                                        .and_then(|l| l.get("best"))
                                        .map(js::to_js_string);
                                    break;
                                }
                            }
                            let gotlang = gotlang.unwrap_or_else(|| default_locale.clone());
                            state.opt.insert("lang".into(), Value::String(gotlang));
                        }
                    }

                    let keys: Vec<String> = ni.keys().cloned().collect();
                    for key in &keys {
                        if !["id", "type", "language", "multi"].contains(&key.as_str())
                            && js::slice(key, 0, Some(4)) != "alt-"
                        {
                            let multi_keys = ni
                                .get("multi")
                                .filter(|m| js::truthy(m))
                                .map(|m| m.get("_keys").cloned().unwrap_or(Value::Null));
                            let multi_has_key = match &multi_keys {
                                Some(Value::Null) => {
                                    return Err(EngineError::BadInput(format!(
                                        "Cannot read properties of undefined (reading '{key}')"
                                    )))
                                }
                                Some(k) => js::truthy_opt(k.get(key.as_str())),
                                None => false,
                            };
                            if multi_has_key {
                                let mut deleteme = true;
                                if let Some(Value::Object(langs)) =
                                    multi_keys.as_ref().and_then(|k| k.get(key.as_str()))
                                {
                                    for lang in langs.keys() {
                                        if langspec.bare == leading_alpha(lang) {
                                            deleteme = false;
                                            break;
                                        }
                                    }
                                }
                                if deleteme {
                                    ni.remove(key);
                                }
                            } else {
                                ni.remove(key);
                            }
                        }
                    }
                    // (for-in does not visit keys added during the loop.)
                    let keys: Vec<String> = ni.keys().cloned().collect();
                    for key in &keys {
                        if js::slice(key, 0, Some(4)) == "alt-" {
                            let v = ni.get(key).cloned().unwrap_or(Value::Null);
                            ni.insert(js::slice(key, 4, None), v);
                            state.tmp.abort_alternative = false;
                        } else {
                            let multi_keys: Option<Value> = ni
                                .get("multi")
                                .filter(|m| js::truthy(m))
                                .and_then(|m| m.get("_keys").cloned())
                                .filter(|k| js::truthy(k));
                            if let Some(mk) = multi_keys {
                                if !js::truthy_opt(ni.get(&format!("alt-{key}")))
                                    && js::truthy_opt(mk.get(key.as_str()))
                                {
                                    let by = |spec: &str| -> Option<Value> {
                                        mk.get(key.as_str())
                                            .and_then(|k| k.get(spec))
                                            .filter(|v| js::truthy(v))
                                            .cloned()
                                    };
                                    let found = by(&langspec.best)
                                        .or_else(|| by(&langspec.base))
                                        .or_else(|| by(&langspec.bare));
                                    if let Some(v) = found {
                                        ni.insert(key.clone(), v);
                                        state.tmp.abort_alternative = false;
                                    }
                                }
                            }
                        }
                    }
                    new_item = Some(Value::Object(ni));
                }

                queue::open_level(state, QueueId::Output, FormatRef::Token(token.clone()))?;
                set_refhash(state, item, new_item.clone());
                new_name_output(state, new_item.as_ref());
                Ok(None)
            }
            NodeAlternativeExec::AbortAlternative => {
                state.tmp.abort_alternative = true;
                Ok(None)
            }
            NodeAlternativeExec::End => {
                queue::close_level(state, QueueId::Output, None)?;
                let old = state.tmp.old_item.clone();
                set_refhash(state, item, old.clone());
                match state.tmp.old_lang.clone() {
                    Some(l) => {
                        state.opt.insert("lang".into(), Value::String(l));
                    }
                    None => {
                        state.opt.remove("lang");
                    }
                }
                new_name_output(state, old.as_ref());
                state.tmp.abort_alternative = false;
                Ok(None)
            }
        }
    }
}

/// `lang.replace(/^([a-zA-Z]+).*/, "$1")`: the leading run of ASCII letters
/// (the whole string when there is none to match, as `replace` leaves it).
fn leading_alpha(lang: &str) -> String {
    let n = lang.chars().take_while(|c| c.is_ascii_alphabetic()).count();
    if n == 0 {
        lang.to_string()
    } else {
        lang.chars().take(n).collect()
    }
}

/// `state.registry.refhash[Item.id] = item` (`None` is `undefined`).

fn set_refhash(state: &mut State, item: &Value, value: Option<Value>) {
    let Some(id) = item.get("id").map(js::to_js_string) else {
        return;
    };
    match value {
        Some(v) => {
            state.registry.refhash.insert(id, v);
        }
        None => {
            state.registry.refhash.remove(&id);
        }
    }
}

/// `state.nameOutput = new CSL.NameOutput(state, item)` (no citation item).
fn new_name_output(state: &mut State, item: Option<&Value>) {
    let null = Value::Null;
    state.new_name_output(item.unwrap_or(&null), &null);
}

/// `CSL.Node.alternative.build.call(token, state, target)`.
pub fn build(
    state: &mut State,
    mut token: Token,
    target: &mut Vec<Token>,
    _real_group: Option<bool>,
) -> CslResult<()> {
    if token.tokentype == TokenType::Start {
        let choose_tok = Token::new("choose", TokenType::Start);
        node_choose::build(state, choose_tok, target, None)?;

        let mut if_tok = Token::new("if", TokenType::Start);
        attributes::apply(state, &mut if_tok, "@alternative-node-internal", "")?;
        node_if::build(state, if_tok, target, None)?;

        token
            .execs
            .push(Exec::NodeAlternative(NodeAlternativeExec::Start));
        target.push(token);

        let choose_tok = Token::new("choose", TokenType::Start);
        node_choose::build(state, choose_tok, target, None)?;

        let mut if_tok = Token::new("if", TokenType::Start);
        attributes::apply(state, &mut if_tok, "@alternative-node-internal", "")?;
        if_tok
            .execs
            .push(Exec::NodeAlternative(NodeAlternativeExec::AbortAlternative));
        node_if::build(state, if_tok, target, None)?;
    } else if token.tokentype == TokenType::End {
        let if_tok = Token::new("if", TokenType::End);
        node_if::build(state, if_tok, target, None)?;

        let choose_tok = Token::new("choose", TokenType::End);
        node_choose::build(state, choose_tok, target, None)?;

        token
            .execs
            .push(Exec::NodeAlternative(NodeAlternativeExec::End));
        target.push(token);

        let if_tok = Token::new("if", TokenType::End);
        node_if::build(state, if_tok, target, None)?;

        let choose_tok = Token::new("choose", TokenType::End);
        node_choose::build(state, choose_tok, target, None)?;
    }
    Ok(())
}
