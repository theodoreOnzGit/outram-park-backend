// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/node_institution.js
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

//! Port of `src/node_institution.js`: `CSL.Node.institution`.

use serde_json::Value;

use super::exec::Exec;
use super::js;
use super::load::STARTSWITH_ROMANESQUE_REGEXP;
use super::obj_blob::Blob;
use super::obj_token::{Token, TokenType};
use super::queue::FormatRef;
use super::state::State;
use super::util_names_output::{q_append_str, q_pop_blob_required, BlobPair};
use super::CslResult;

/// The closures `src/node_institution.js` stores in `token.execs`
/// (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeInstitutionExec {
    /// START / SINGLETON: compute the institution delimiter, `and` term and
    /// prefixes, build the `and` blobs and register the node on
    /// `state.nameOutput` (node_institution.js:5-76).
    Setup,
}

impl NodeInstitutionExec {
    /// Run the closure.
    pub fn run(
        &self,
        state: &mut State,
        token: &mut Token,
        _item: &Value,
        _cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            NodeInstitutionExec::Setup => {
                setup(state, token)?;
                Ok(None)
            }
        }
    }
}

/// The closure `CSL.Node.institution.build` pushes (node_institution.js:5-76).
/// `this.and_term` persists on the token between runs, as in JS (it is kept in
/// `token.extra["and_term"]`).
fn setup(state: &mut State, token: &mut Token) -> CslResult<()> {
    let institution_delimiter: Option<String> = match token.strings.get("delimiter") {
        Some(Value::String(s)) => Some(s.clone()),
        _ => state.tmp.name_delimiter.clone(),
    };
    state.tmp.institution_delimiter = institution_delimiter.clone();

    // This is the same code for the same result as in node_name.js,
    // but when cs:institution comes on stream, it may produce
    // different results.
    let and_opt = state.inherit_opt(token, "and", None, None);
    match and_opt.as_ref().and_then(Value::as_str) {
        Some("text") => {
            set_and_term(token, state.get_term("and", Some("long"), Some(0), None, None, false)?)
        }
        Some("symbol") => {
            let expect = js::truthy_opt(
                state
                    .opt
                    .get("development_extensions")
                    .and_then(|d| d.get("expect_and_symbol_form")),
            );
            if expect {
                set_and_term(token, state.get_term("and", Some("symbol"), Some(0), None, None, false)?)
            } else {
                set_and_term(token, Some("&".to_string()))
            }
        }
        Some("none") => set_and_term(token, institution_delimiter.clone()),
        _ => {}
    }
    if !token.extra.contains_key("and_term") {
        if let Some(t) = state.tmp.and_term.clone().filter(|t| !t.is_empty()) {
            // this.and_term = state.getTerm("and", "long", 0);
            set_and_term(token, Some(t));
        }
    }
    let and_term: Option<String> = token.extra.get("and_term").map(js::to_js_string);
    let (mut and_prefix_single, mut and_prefix_multiple, and_suffix): (String, String, String);
    // `RegExp.test(undefined)` tests the string "undefined".
    if STARTSWITH_ROMANESQUE_REGEXP.is_match(and_term.as_deref().unwrap_or("undefined")) {
        and_prefix_single = " ".to_string();
        and_prefix_multiple = ", ".to_string();
        if let Some(d) = &institution_delimiter {
            and_prefix_multiple = d.clone();
        }
        and_suffix = " ".to_string();
    } else {
        and_prefix_single = String::new();
        and_prefix_multiple = String::new();
        and_suffix = String::new();
    }
    let inst_delim = institution_delimiter.clone().unwrap_or_else(|| "undefined".to_string());
    let dpl = state.inherit_opt(token, "delimiter-precedes-last", None, None);
    match dpl.as_ref().and_then(Value::as_str) {
        Some("always") => and_prefix_single = inst_delim.clone(),
        Some("never") => {
            // Slightly fragile: could test for charset here to make
            // this more certain.
            if !and_prefix_multiple.is_empty() {
                and_prefix_multiple = " ".to_string();
            }
        }
        _ => {}
    }

    let and: BlobPair;
    if and_term.is_some() {
        q_append_str(state, and_term.as_deref(), FormatRef::Name("empty".into()), true)?;
        let single = q_pop_blob_required(state)?;
        state.blobs.get_mut(single).set_string("prefix", &and_prefix_single);
        state.blobs.get_mut(single).set_string("suffix", &and_suffix);
        q_append_str(state, and_term.as_deref(), FormatRef::Name("empty".into()), true)?;
        let multiple = q_pop_blob_required(state)?;
        state.blobs.get_mut(multiple).set_string("prefix", &and_prefix_multiple);
        state.blobs.get_mut(multiple).set_string("suffix", &and_suffix);
        and = BlobPair {
            single: Some(single),
            multiple: Some(multiple),
        };
    } else {
        // `"undefined" !== this.strings.delimiter` is always true.
        let mk = |state: &mut State| {
            let mut b = Blob::new(Some(&inst_delim), None, None);
            b.set_string("prefix", "");
            b.set_string("suffix", "");
            state.blobs.add(b)
        };
        let single = mk(state);
        let multiple = mk(state);
        and = BlobPair {
            single: Some(single),
            multiple: Some(multiple),
        };
    }
    state.name_output.institution = Some(token.clone());
    state.name_output.institution_and = Some(and);
    Ok(())
}

/// `this.and_term = value` (`undefined` removes the property).
fn set_and_term(token: &mut Token, value: Option<String>) {
    match value {
        Some(v) => token.extra.insert("and_term".into(), Value::String(v)),
        None => token.extra.remove("and_term"),
    };
}

/// `CSL.Node.institution.build.call(token, state, target)`.
pub fn build(
    _state: &mut State,
    mut token: Token,
    target: &mut Vec<Token>,
    _real_group: Option<bool>,
) -> CslResult<()> {
    if token.tokentype == TokenType::Singleton || token.tokentype == TokenType::Start {
        token
            .execs
            .push(Exec::NodeInstitution(NodeInstitutionExec::Setup));
    }
    target.push(token);
    Ok(())
}

/// `CSL.Node.institution.configure.call(tokens[pos], state, pos)`.
pub fn configure(state: &mut State, tokens: &mut [Token], pos: usize) -> CslResult<()> {
    let is_open = tokens
        .get(pos)
        .map(|t| t.tokentype == TokenType::Singleton || t.tokentype == TokenType::Start)
        .unwrap_or(false);
    if is_open {
        state.build.has_institution = true;
    }
    Ok(())
}
