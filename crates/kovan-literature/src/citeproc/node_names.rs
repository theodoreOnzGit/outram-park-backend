// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/node_names.js
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

//! Port of `src/node_names.js`: `CSL.Node.names.build`.
//!
//! Also hosts [`token_to_value`] / [`token_from_value`], the JSON form of a
//! token that the builders use where upstream keeps a *reference* to a token
//! inside another JS object (`state.build.family`, `names.label`,
//! `state.tmp.etal_node`, ...), which cannot be a [`Token`] value in a
//! `serde_json` bag.

use serde_json::Value;

use super::exec::Exec;
use super::js::{self, Obj};
use super::obj_token::{Decoration, Token, TokenType};
use super::state::State;
use super::util_substitute;
use super::{CslResult, EngineError};

/// A token as a JSON object, in the form the intermediate dump gives a
/// *nested* token (scripts/csl-intermediate-reference.cjs `canon`): `name`,
/// `tokentype`, `strings`, `decorations`, `variables`, `execs_n`, `tests_n`
/// (only when the `tests` array exists), `has_test` (only when true),
/// `postponed_macro` and every `extra` property; jump indices are left out.
/// (Closures are counted, not stored.)
pub fn token_to_value(t: &Token) -> Value {
    let mut o = Obj::new();
    o.insert("name".into(), Value::String(t.name.clone()));
    o.insert("tokentype".into(), Value::from(t.tokentype.as_js()));
    o.insert("strings".into(), Value::Object(t.strings.clone()));
    o.insert("decorations".into(), decorations_to_value(&t.decorations));
    o.insert(
        "variables".into(),
        Value::Array(t.variables.iter().cloned().map(Value::String).collect()),
    );
    o.insert("execs_n".into(), Value::from(t.execs.len()));
    if t.tests_defined {
        o.insert("tests_n".into(), Value::from(t.tests.len()));
    }
    if t.test.is_some() {
        o.insert("has_test".into(), Value::Bool(true));
    }
    if let Some(m) = &t.postponed_macro {
        o.insert("postponed_macro".into(), Value::String(m.clone()));
    }
    for (k, v) in &t.extra {
        o.insert(k.clone(), v.clone());
    }
    Value::Object(o)
}

/// The inverse of [`token_to_value`] for the data fields (closures are not
/// restored: `execs`, `tests` and `test` come back empty).
pub fn token_from_value(v: &Value) -> Option<Token> {
    let o = v.as_object()?;
    let name = o.get("name")?.as_str()?;
    let tokentype = match o.get("tokentype")?.as_i64()? {
        0 => TokenType::Start,
        1 => TokenType::End,
        _ => TokenType::Singleton,
    };
    let mut t = Token::new(name, tokentype);
    t.strings = o.get("strings")?.as_object()?.clone();
    t.decorations = decorations_from_value(o.get("decorations")?);
    t.variables = o
        .get("variables")?
        .as_array()?
        .iter()
        .filter_map(|x| x.as_str().map(str::to_string))
        .collect();
    t.postponed_macro = o
        .get("postponed_macro")
        .and_then(Value::as_str)
        .map(str::to_string);
    const OWN: [&str; 8] = [
        "name",
        "tokentype",
        "strings",
        "decorations",
        "variables",
        "execs_n",
        "tests_n",
        "has_test",
    ];
    for (k, x) in o {
        if !OWN.contains(&k.as_str()) && k != "postponed_macro" {
            t.extra.insert(k.clone(), x.clone());
        }
    }
    Some(t)
}

/// `token.decorations` as the JS array of `[name, value(, extra)]` arrays.
pub fn decorations_to_value(ds: &[Decoration]) -> Value {
    Value::Array(
        ds.iter()
            .map(|d| {
                let mut a = vec![
                    Value::String(d.name.clone()),
                    Value::String(d.value.clone()),
                ];
                if let Some(e) = &d.extra {
                    a.push(Value::String(e.clone()));
                }
                Value::Array(a)
            })
            .collect(),
    )
}

fn decorations_from_value(v: &Value) -> Vec<Decoration> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|d| {
                    let d = d.as_array()?;
                    Some(Decoration {
                        name: d.first()?.as_str()?.to_string(),
                        value: d.get(1)?.as_str()?.to_string(),
                        extra: d.get(2).map(js::to_js_string),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The closures `src/node_names.js` stores in `token.execs` (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeNamesExec {
    /// SINGLETON: `state.nameOutput.reinit(this, this.variables_real[0])`
    /// (node_names.js:23-25).
    Reinit,
    /// START: init can-substitute, `name_node`, `nameOutput.init(this)`
    /// (node_names.js:37-43).
    Init,
    /// END: build the et-al/with/label parameters and run
    /// `nameOutput.outputNames()` (node_names.js:66-170).
    Output,
    /// END: the "unsets" closure (node_names.js:173-190).
    Unset,
}

impl NodeNamesExec {
    /// Run the closure.
    pub fn run(
        &self,
        state: &mut State,
        _token: &mut Token,
        _item: &Value,
        _cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            // PORT-LATER(wave3): node_names.js:23, needs CSL.NameOutput
            // (state.nameOutput.reinit).
            NodeNamesExec::Reinit => Err(EngineError::NotYetPorted {
                method: "node_names.js:23 closure",
            }),
            // PORT-LATER(wave3): node_names.js:37, needs CSL.NameOutput
            // (state.nameOutput.init) and state.tmp.name_node {children, top}.
            // Reachable part: state.tmp.can_substitute.push(true).
            NodeNamesExec::Init => Err(EngineError::NotYetPorted {
                method: "node_names.js:37 closure",
            }),
            // PORT-LATER(wave3): node_names.js:66-170, needs state.getTerm,
            // CSL.Blob, state.nameOutput.outputNames(); also mutates
            // this.etal_* on the running token.
            NodeNamesExec::Output => Err(EngineError::NotYetPorted {
                method: "node_names.js:66 closure",
            }),
            NodeNamesExec::Unset => {
                if !state
                    .tmp
                    .can_substitute
                    .pop()
                    .map(|v| js::truthy(&v))
                    .unwrap_or(false)
                {
                    state
                        .tmp
                        .can_substitute
                        .replace_literal(Value::Bool(false))?;
                }
                // For posterity ... (see node_names.js:177-189)
                if state.tmp.can_substitute.len() == 1 {
                    state.tmp.can_block_substitute = false;
                }
                Ok(None)
            }
        }
    }
}

/// `CSL.Node.names.build.call(token, state, target)`.
pub fn build(
    state: &mut State,
    mut token: Token,
    target: &mut Vec<Token>,
    _real_group: Option<bool>,
) -> CslResult<()> {
    if token.tokentype == TokenType::Start || token.tokentype == TokenType::Singleton {
        util_substitute::substitute_start(state, &mut token, target)?;
        state.build.substitute_level.push(1);
    }

    if token.tokentype == TokenType::Singleton {
        // JS: names_variables[last].concat(this.variables) (result discarded).
        if state.build.names_variables.is_empty() {
            return Err(EngineError::Csl(
                "TypeError: state.build.names_variables[-1] is undefined".into(),
            ));
        }
        for variable in token.variables.clone() {
            let Some(name_labels) = state.build.name_label.last_mut() else {
                return Err(EngineError::Csl(
                    "TypeError: state.build.name_label[-1] is undefined".into(),
                ));
            };
            if !name_labels.is_empty() {
                let first = name_labels[0].1.clone();
                set_pair(name_labels, &variable, first);
            }
        }
        token.execs.push(Exec::NodeNames(NodeNamesExec::Reinit));
    }

    if token.tokentype == TokenType::Start {
        state.build.names_flag = true;
        state.build.name_flag = false;
        state.build.names_level += 1;
        state.build.names_variables.push(token.variables.clone());
        state.build.name_label.push(Vec::new());
        // init can substitute
        // init names
        token.execs.push(Exec::NodeNames(NodeNamesExec::Init));
    }

    if token.tokentype == TokenType::End {
        // Set/reset name blobs if they exist, for processing
        // by namesOutput()
        for key in ["family", "given", "et-al"] {
            match state.build.name_parts.get(key) {
                Some(v) => {
                    token.extra.insert(key.to_string(), v.clone());
                }
                None => {
                    token.extra.remove(key);
                }
            }
            if state.build.names_level == 1 {
                state.build.name_parts.remove(key);
            }
        }
        // Labels, if any
        let label = state.build.name_label.last().ok_or_else(|| {
            EngineError::Csl("TypeError: state.build.name_label[-1] is undefined".into())
        })?;
        token.extra.insert("label".into(), pairs_to_value(label));
        state.build.names_level -= 1;
        state.build.names_variables.pop();
        state.build.name_label.pop();

        // "and" and "ellipsis" are set in node_name.js
        token.execs.push(Exec::NodeNames(NodeNamesExec::Output));
        // unsets
        token.execs.push(Exec::NodeNames(NodeNamesExec::Unset));

        state.build.name_flag = false;
    }
    let is_end_or_singleton =
        token.tokentype == TokenType::End || token.tokentype == TokenType::Singleton;
    if is_end_or_singleton {
        state.build.substitute_level.pop();
        // target.push(this); CSL.Util.substituteEnd.call(this, state, target);
        util_substitute::push_with_substitute_end(state, token, target, true)?;
    } else {
        target.push(token);
    }
    Ok(())
}

/// `obj[key] = value` on an insertion-ordered pair list.
pub fn set_pair(pairs: &mut Vec<(String, Value)>, key: &str, value: Value) {
    if let Some(p) = pairs.iter_mut().find(|(k, _)| k == key) {
        p.1 = value;
    } else {
        pairs.push((key.to_string(), value));
    }
}

/// An ordered pair list as a JSON object.
pub fn pairs_to_value(pairs: &[(String, Value)]) -> Value {
    let mut o = Obj::new();
    for (k, v) in pairs {
        o.insert(k.clone(), v.clone());
    }
    Value::Object(o)
}
