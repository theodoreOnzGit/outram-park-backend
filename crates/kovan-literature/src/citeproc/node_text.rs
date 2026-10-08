// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/node_text.js
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

//! Port of `src/node_text.js`: `CSL.Node.text`.

use serde_json::Value;

use super::attributes::area_ref;
use super::load::{CITE_FIELDS, MULTI_FIELDS, NUMERIC, TRIGRAPH};
use super::exec::Exec;
use super::js;
use super::node_group;
use super::obj_token::{Token, TokenType};
use super::state::State;
use super::util_substitute;
use super::{CslResult, EngineError};

/// The closures `src/node_text.js` stores in `token.execs` (PORTING.md §4).
/// Each variant is one `func`; the ones that capture build-time locals carry
/// them (`form`, `plural`, the transform parameters).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeTextExec {
    /// `citation-number` (node_text.js:82-130).
    CitationNumber,
    /// `year-suffix` (node_text.js:140-185).
    YearSuffix,
    /// `citation-label` (node_text.js:196-215).
    CitationLabel,
    /// `printterm` (node_text.js:222-275): a term, with `form` and `plural`
    /// captured from the build-time locals.
    PrintTerm {
        /// `form` (default `"long"`).
        form: String,
        /// `plural` (`this.strings.plural` or 0): 0, 1 or `false`.
        plural: Value,
    },
    /// The variable-flags closure that precedes a variable's renderer
    /// (node_text.js:281-299).
    VariableFlags,
    /// `state.transform.getOutputFunction(this.variables, abbrevfam, abbrfall,
    /// altvar, transfall)` (node_text.js:300-335). `variables` is the array
    /// `this.variables` as it was at build time (upstream shares the live array).
    TransformOutput {
        /// `this.variables`.
        variables: Vec<String>,
        /// `abbrevfam` (a variable name, or `false` for none).
        abbrevfam: Option<String>,
        /// `abbrfall`.
        abbrfall: bool,
        /// `altvar` (the `-short` variable name, or `false`).
        altvar: Option<String>,
        /// `transfall`.
        transfall: bool,
    },
    /// Per-cite field (locator etc.) read from `item` (node_text.js:340-362).
    CiteField,
    /// `page`, `volume`, ... via `state.processNumber` (node_text.js:364-371).
    NumberField,
    /// `URL` / `DOI` (node_text.js:372-450). `form` is captured.
    UrlOrDoi {
        /// `form`.
        form: String,
    },
    /// `section` (node_text.js:452-462). `form` is captured.
    Section {
        /// `form`.
        form: String,
    },
    /// `hereinafter` (node_text.js:464-472).
    Hereinafter,
    /// Any other variable, output as is (node_text.js:474-487). `form` is captured.
    PlainVariable {
        /// `form`.
        form: String,
    },
    /// The `value` attribute (node_text.js:491-503).
    Value,
}

impl NodeTextExec {
    /// Run the closure.
    pub fn run(
        &self,
        _state: &mut State,
        _token: &mut Token,
        _item: &Value,
        _cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        // PORT-LATER(wave2): every closure of node_text.js appends to the output
        // queue (state.output.append / NumericBlob / Blob, queue.rs and
        // util_number.js), reads state.registry (wave4), state.getTerm /
        // getVariable (build.js), state.processNumber (util_number.js),
        // state.transform (util_transform.js) or state.tmp.group_context
        // (CSL.UPDATE_GROUP_CONTEXT_CONDITION). The captured build-time values
        // are in the variants.
        let method = match self {
            NodeTextExec::CitationNumber => "node_text.js:82 closure",
            NodeTextExec::YearSuffix => "node_text.js:140 closure",
            NodeTextExec::CitationLabel => "node_text.js:196 closure",
            NodeTextExec::PrintTerm { .. } => "node_text.js:222 closure",
            NodeTextExec::VariableFlags => "node_text.js:281 closure",
            NodeTextExec::TransformOutput { .. } => "node_text.js:335 getOutputFunction",
            NodeTextExec::CiteField => "node_text.js:340 closure",
            NodeTextExec::NumberField => "node_text.js:364 closure",
            NodeTextExec::UrlOrDoi { .. } => "node_text.js:372 closure",
            NodeTextExec::Section { .. } => "node_text.js:452 closure",
            NodeTextExec::Hereinafter => "node_text.js:464 closure",
            NodeTextExec::PlainVariable { .. } => "node_text.js:474 closure",
            NodeTextExec::Value => "node_text.js:491 closure",
        };
        Err(EngineError::NotYetPorted { method })
    }
}

fn push(token: &mut Token, e: NodeTextExec) {
    token.execs.push(Exec::NodeText(e));
}

fn variables_real(token: &Token) -> Vec<String> {
    token
        .extra
        .get("variables_real")
        .and_then(Value::as_array)
        .map(|a| a.iter().map(js::to_js_string).collect())
        .unwrap_or_default()
}

/// `CSL.Node.text.build.call(token, state, target)`.
pub fn build(
    state: &mut State,
    mut token: Token,
    target: &mut Vec<Token>,
    _real_group: Option<bool>,
) -> CslResult<()> {
    if let Some(macro_name) = token.postponed_macro.clone() {
        let mut group_start = token.clone_token();
        group_start.name = "group".to_string();
        group_start.tokentype = TokenType::Start;
        node_group::build(state, group_start, target, None)?;

        state.expand_macro(&token, target)?;

        let mut group_end = token.clone_token();
        group_end.name = "group".to_string();
        group_end.tokentype = TokenType::End;
        if macro_name == "juris-locator-label" {
            group_end
                .extra
                .insert("isJurisLocatorLabel".into(), Value::Bool(true));
        }
        node_group::build(state, group_end, target, None)?;
        return Ok(());
    }

    util_substitute::substitute_start(state, &mut token, target)?;
    // ...
    //
    // Do non-macro stuff

    // Guess again. this.variables is ephemeral, adjusted by an initial
    // function set on the node via @variable attribute setup.
    if !token.extra.contains_key("variables_real") {
        token
            .extra
            .insert("variables_real".into(), Value::Array(Vec::new()));
    }
    // (`if (!this.variables) this.variables = []` is a no-op for a Vec.)

    let mut form = "long".to_string();
    let mut plural = Value::from(0);
    if js::truthy_opt(token.strings.get("form")) {
        form = token.string("form");
    }
    if js::truthy_opt(token.strings.get("plural")) {
        plural = token.strings.get("plural").cloned().unwrap_or(Value::Null);
    }
    let vreal = variables_real(&token);
    let v0 = vreal.first().cloned();
    let area_opt = |state: &State, name: &str, key: &str| -> CslResult<Option<Value>> {
        Ok(area_ref(state, name)?.opt.get(key).cloned())
    };
    if matches!(
        v0.as_deref(),
        Some("citation-number") | Some("year-suffix") | Some("citation-label")
    ) {
        //
        // citation-number and year-suffix are super special,
        // because they are rangeables, and require a completely
        // different set of formatting parameters on the output
        // queue.
        if v0.as_deref() == Some("citation-number") {
            if state.build.root == "citation" {
                state.opt.insert("update_mode".into(), Value::from(NUMERIC));
            }
            if state.build.root == "bibliography" {
                state.opt.insert("bib_mode".into(), Value::from(NUMERIC));
            }
            let tmp_area = state.tmp.area.clone();
            if area_opt(state, &tmp_area, "collapse")?
                .and_then(|v| v.as_str().map(str::to_string))
                .as_deref()
                == Some("citation-number")
            {
                let t =
                    state.get_term("citation-range-delimiter", None, None, None, None, false)?;
                token.extra.insert(
                    "range_prefix".into(),
                    t.map(Value::String).unwrap_or(Value::Null),
                );
            }
            let ld = area_opt(state, &state.build.area.clone(), "layout_delimiter")?;
            set_opt_extra(&mut token, "successor_prefix", ld.clone());
            set_opt_extra(&mut token, "splice_prefix", ld);
            push(&mut token, NodeTextExec::CitationNumber);
        } else if v0.as_deref() == Some("year-suffix") {
            state
                .opt
                .insert("has_year_suffix".into(), Value::Bool(true));

            let tmp_area = state.tmp.area.clone();
            if area_opt(state, &tmp_area, "collapse")?
                .and_then(|v| v.as_str().map(str::to_string))
                .as_deref()
                == Some("year-suffix-ranged")
            {
                let t =
                    state.get_term("citation-range-delimiter", None, None, None, None, false)?;
                token.extra.insert(
                    "range_prefix".into(),
                    t.map(Value::String).unwrap_or(Value::Null),
                );
            }
            let build_area = state.build.area.clone();
            let ld = area_opt(state, &build_area, "layout_delimiter")?;
            set_opt_extra(&mut token, "successor_prefix", ld);
            if let Some(d) = area_opt(state, &tmp_area, "year-suffix-delimiter")? {
                if js::truthy(&d) {
                    // JS reads the delimiter from state[state.build.area].opt here.
                    let d2 = area_opt(state, &build_area, "year-suffix-delimiter")?;
                    set_opt_extra(&mut token, "successor_prefix", d2);
                }
            }
            push(&mut token, NodeTextExec::YearSuffix);
        } else {
            // citation-label
            if state.build.root == "bibliography" {
                state.opt.insert("bib_mode".into(), Value::from(TRIGRAPH));
            }
            state
                .opt
                .insert("has_year_suffix".into(), Value::Bool(true));
            push(&mut token, NodeTextExec::CitationLabel);
        }
    } else if js::truthy_opt(token.strings.get("term")) {
        // printterm
        push(
            &mut token,
            NodeTextExec::PrintTerm {
                form: form.clone(),
                plural,
            },
        );
        state.build.term = Value::Bool(false);
        state.build.form = Value::Bool(false);
        state.build.plural = Value::Bool(false);
    } else if !vreal.is_empty() {
        push(&mut token, NodeTextExec::VariableFlags);

        // plain string fields

        // Deal with multi-fields and ordinary fields separately.
        let v = vreal[0].as_str();
        let func = if MULTI_FIELDS.contains(&v)
            || v.contains("-main")
            || v.contains("-sub")
            || ["language-name", "language-name-original"].contains(&v)
        {
            // multi-fields
            // Initialize transform factory according to whether
            // abbreviation is desired.
            let mut abbrevfam: Option<String> = token.variables.first().cloned();
            let mut altvar: Option<String> = None;
            if form == "short" {
                if js::slice(v, -6, None) != "-short" {
                    altvar = Some(format!("{v}-short"));
                }
            } else {
                abbrevfam = None;
            }
            // multi-fields for sorting get a sort transform,
            // (abbreviated if the short form was selected)
            let transfall = true;
            let abbrfall = state.build.extension.is_empty();
            NodeTextExec::TransformOutput {
                variables: token.variables.clone(),
                abbrevfam,
                abbrfall,
                altvar,
                transfall,
            }
        } else if CITE_FIELDS.contains(&v) {
            // per-cite fields are read from item, rather than Item
            NodeTextExec::CiteField
        } else if [
            "page",
            "page-first",
            "chapter-number",
            "collection-number",
            "edition",
            "issue",
            "number",
            "number-of-pages",
            "number-of-volumes",
            "volume",
        ]
        .contains(&v)
        {
            // page gets mangled with the correct collapsing algorithm
            NodeTextExec::NumberField
        } else if v == "URL" || v == "DOI" {
            NodeTextExec::UrlOrDoi { form: form.clone() }
        } else if v == "section" {
            // Sections for statutes are special.
            NodeTextExec::Section { form: form.clone() }
        } else if v == "hereinafter" {
            NodeTextExec::Hereinafter
        } else {
            // anything left over just gets output in the normal way.
            NodeTextExec::PlainVariable { form: form.clone() }
        };
        push(&mut token, func);
    } else if js::truthy_opt(token.strings.get("value")) {
        // for the text value attribute.
        push(&mut token, NodeTextExec::Value);
        // otherwise no output
    }
    // target.push(this); CSL.Util.substituteEnd.call(this, state, target);
    util_substitute::push_with_substitute_end(state, token, target, true)
}

fn set_opt_extra(token: &mut Token, key: &str, v: Option<Value>) {
    match v {
        Some(v) => {
            token.extra.insert(key.to_string(), v);
        }
        None => {
            token.extra.remove(key);
        }
    }
}
