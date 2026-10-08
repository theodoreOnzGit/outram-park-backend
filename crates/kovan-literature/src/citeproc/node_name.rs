// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/node_name.js
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

//! Port of `src/node_name.js`: `CSL.Node.name`.

use serde_json::Value;

use super::exec::Exec;
use super::js;
use super::load::{POSITION, STARTSWITH_ROMANESQUE_REGEXP};
use super::obj_blob::Blob;
use super::obj_token::{Token, TokenType};
use super::queue::FormatRef;
use super::state::State;
use super::util_names_output::{q_append_str, q_pop_blob_required, BlobPair};
use super::CslResult;

/// The closures `src/node_name.js` stores in `token.execs` (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeNameExec {
    /// START / SINGLETON: set the et-al term, delimiter, `and` term and
    /// prefixes, the ellipsis and `and` blobs, the et-al parameters, and
    /// `state.nameOutput.name = this` (node_name.js:41-170).
    Setup,
}

impl NodeNameExec {
    /// Run the closure.
    pub fn run(
        &self,
        state: &mut State,
        token: &mut Token,
        _item: &Value,
        _cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            NodeNameExec::Setup => {
                setup(state, token)?;
                Ok(None)
            }
        }
    }
}

/// A standalone `new CSL.Blob(str)` with the given prefix and suffix, in the
/// arena.
fn plain_blob(state: &mut State, text: &str, prefix: &str, suffix: &str) -> super::obj_blob::BlobId {
    let mut b = Blob::new(Some(text), None, None);
    b.set_string("prefix", prefix);
    b.set_string("suffix", suffix);
    state.blobs.add(b)
}

/// The closure `CSL.Node.name.build` pushes (node_name.js:41-170): resolve the
/// `and` term, build the `and` and ellipsis blobs, set the et-al defaults and
/// make this token `state.nameOutput.name`.
fn setup(state: &mut State, token: &mut Token) -> CslResult<()> {
    // Et-al (onward processing in node_etal.js and node_names.js)
    // XXXXX Why is this necessary? This is available on this.name, right?
    state.tmp.etal_term = Some("et-al".to_string());

    // Use default delimiter as fallback, in a way that allows explicit
    // empty strings.
    let name_delimiter: String = state
        .inherit_opt(token, "delimiter", Some("name-delimiter"), Some(Value::String(", ".into())))
        .map(|v| js::to_js_string(&v))
        .unwrap_or_default();
    state.tmp.name_delimiter = Some(name_delimiter.clone());
    state.tmp.delimiter_precedes_et_al = state
        .inherit_opt(token, "delimiter-precedes-et-al", None, None)
        .and_then(|v| v.as_str().map(str::to_string));

    // And
    let and_opt = state.inherit_opt(token, "and", None, None);
    let and_str = and_opt.as_ref().and_then(Value::as_str).map(str::to_string);
    if and_str.as_deref() == Some("text") {
        set_and_term(token, state.get_term("and", Some("long"), Some(0), None, None, false)?);
    } else if and_str.as_deref() == Some("symbol") {
        let expect = js::truthy_opt(
            state
                .opt
                .get("development_extensions")
                .and_then(|d| d.get("expect_and_symbol_form")),
        );
        if expect {
            set_and_term(token, state.get_term("and", Some("symbol"), Some(0), None, None, false)?);
        } else {
            set_and_term(token, Some("&".to_string()));
        }
    }
    let and_term: Option<String> = token.extra.get("and_term").map(js::to_js_string);
    state.tmp.and_term = and_term.clone();
    let (mut and_prefix_single, mut and_prefix_multiple, and_suffix): (String, String, String);
    // `RegExp.test(undefined)` tests the string "undefined".
    if STARTSWITH_ROMANESQUE_REGEXP.is_match(and_term.as_deref().unwrap_or("undefined")) {
        and_prefix_single = " ".to_string();
        // Workaround to allow explicit empty string
        // on cs:name delimiter ("string" === typeof state.tmp.name_delimiter:
        // always true here).
        and_prefix_multiple = name_delimiter.clone();
        and_suffix = " ".to_string();
    } else {
        and_prefix_single = String::new();
        and_prefix_multiple = String::new();
        and_suffix = String::new();
    }
    let dpl = state.inherit_opt(token, "delimiter-precedes-last", None, None);
    match dpl.as_ref().and_then(Value::as_str) {
        Some("always") => and_prefix_single = name_delimiter.clone(),
        Some("never") => {
            // Slightly fragile: could test for charset here to make
            // this more certain.
            if !and_prefix_multiple.is_empty() {
                and_prefix_multiple = " ".to_string();
            }
        }
        Some("after-inverted-name") => {
            if !and_prefix_single.is_empty() {
                and_prefix_single = name_delimiter.clone();
            }
            if !and_prefix_multiple.is_empty() {
                and_prefix_multiple = " ".to_string();
            }
        }
        _ => {}
    }

    let mut and: Option<BlobPair> = None;
    if and_opt.as_ref().map(js::truthy).unwrap_or(false) {
        q_append_str(state, and_term.as_deref(), FormatRef::Name("empty".into()), true)?;
        let single = q_pop_blob_required(state)?;
        state.blobs.get_mut(single).set_string("prefix", &and_prefix_single);
        state.blobs.get_mut(single).set_string("suffix", &and_suffix);
        q_append_str(state, and_term.as_deref(), FormatRef::Name("empty".into()), true)?;
        let multiple = q_pop_blob_required(state)?;
        state.blobs.get_mut(multiple).set_string("prefix", &and_prefix_multiple);
        state.blobs.get_mut(multiple).set_string("suffix", &and_suffix);
        and = Some(BlobPair {
            single: Some(single),
            multiple: Some(multiple),
        });
    } else if !name_delimiter.is_empty() {
        // This is a little weird, but it works.
        let single = plain_blob(state, &name_delimiter, "", "");
        let multiple = plain_blob(state, &name_delimiter, "", "");
        and = Some(BlobPair {
            single: Some(single),
            multiple: Some(multiple),
        });
    }

    let mut ellipsis: Option<BlobPair> = None;
    if js::truthy_opt(state.inherit_opt(token, "et-al-use-last", None, None).as_ref()) {
        // We use the dedicated Unicode ellipsis character because
        // it is recommended by some editors, and can be more easily
        // identified for find and replace operations.
        // Source: http://en.wikipedia.org/wiki/Ellipsis#Computer_representations
        //
        // Eventually, this should be localized as a term in CSL, with some
        // mechanism for triggering appropriate punctuation handling around
        // the ellipsis placeholder (Polish is a particularly tough case for that).
        let ellipsis_term = "\u{2026}";
        // Similar treatment to "and", above, will be needed
        // here when this becomes a locale term.
        let ellipsis_prefix_single = " ";
        let ellipsis_prefix_multiple = name_delimiter.clone();
        let ellipsis_suffix = " ";
        let single = plain_blob(state, ellipsis_term, ellipsis_prefix_single, ellipsis_suffix);
        let multiple = plain_blob(state, ellipsis_term, &ellipsis_prefix_multiple, ellipsis_suffix);
        ellipsis = Some(BlobPair {
            single: Some(single),
            multiple: Some(multiple),
        });
    }

    // et-al parameters are annoyingly incomprehensible
    // again.
    //
    // Explanation probably just adds a further layer of
    // irritation, but what's INTENDED here is that
    // the state.tmp et-al variables are set from the
    // cs:key element when composing sort keys, and a
    // macro containing a name can be called from cs:key.
    // So when cs:key sets et-al attributes, they are
    // set on state.tmp, and when the key is finished
    // processing, the state.tmp variables are reset to
    // undefined. IN THEORY the state.tmp et-al variables
    // will not be used in other contexts. I hope.
    //
    // Anyway, the current tests now seem to pass.
    if state.tmp.et_al_min.is_none() {
        state.tmp.et_al_min = state.inherit_opt(token, "et-al-min", None, None);
    }
    if state.tmp.et_al_use_first.is_none() {
        state.tmp.et_al_use_first = state.inherit_opt(token, "et-al-use-first", None, None);
    }
    if state.tmp.et_al_use_last.is_none() {
        state.tmp.et_al_use_last = state.inherit_opt(token, "et-al-use-last", None, None);
    }

    state.name_output.name = Some(token.clone());
    state.name_output.name_and = and;
    state.name_output.name_ellipsis = ellipsis;
    Ok(())
}

/// `this.and_term = value` (`undefined` removes the property).
fn set_and_term(token: &mut Token, value: Option<String>) {
    match value {
        Some(v) => token.extra.insert("and_term".into(), Value::String(v)),
        None => token.extra.remove("and_term"),
    };
}

/// `CSL.Node.name.build.call(token, state, target)`.
pub fn build(
    state: &mut State,
    mut token: Token,
    target: &mut Vec<Token>,
    _real_group: Option<bool>,
) -> CslResult<()> {
    if token.tokentype == TokenType::Singleton || token.tokentype == TokenType::Start {
        // JS: `if ("undefined" === typeof state.tmp.root) { state.tmp.root =
        // "citation" } else { oldTmpRoot = state.tmp.root }`. `Tmp::new` always
        // sets `root`, so only the else-branch is reachable.
        let old_tmp_root: String = state.tmp.root.clone();
        // Many CSL styles set et-al-[min|use-first]
        // and et-al-subsequent-[min|use-first] to the same
        // value.
        // Set state.opt.update_mode = CSL.POSITION if
        // et-al-subsequent-min or et-al-subsequent-use-first
        // are set AND their value differs from their plain
        // counterparts.
        let result = (|| -> CslResult<()> {
            for (sub, plain) in [
                ("et-al-subsequent-min", "et-al-min"),
                ("et-al-subsequent-use-first", "et-al-use-first"),
            ] {
                let a = state.inherit_opt(&token, sub, None, None);
                if let Some(a) = a {
                    if js::truthy(&a) {
                        let b = state.inherit_opt(&token, plain, None, None);
                        if b.as_ref() != Some(&a) || is_nan(&a) {
                            state
                                .opt
                                .insert("update_mode".into(), Value::from(POSITION));
                        }
                    }
                }
            }
            Ok(())
        })();
        state.tmp.root = old_tmp_root;
        result?;

        state.build.name_flag = true;

        token.execs.push(Exec::NodeName(NodeNameExec::Setup));
    }
    target.push(token);
    Ok(())
}

/// A numeric JSON value that is JS NaN (serialised as `null`): never equal to
/// itself under `!==`. A truthy value is never NaN, so this is only a guard.
fn is_nan(v: &Value) -> bool {
    v.is_null()
}
