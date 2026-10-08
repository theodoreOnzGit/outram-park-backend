// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_substitute.js
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

//! Port of `src/util_substitute.js`: `CSL.Util.substituteStart` and
//! `CSL.Util.substituteEnd`, called by the `cs:text`, `cs:number`, `cs:date`,
//! `cs:names` and `cs:group` builders. They wrap an element's output in the
//! `cs:substitute` conditional, the bibliography display blocks, the
//! `strip-periods` bookkeeping and the subsequent-author-substitute logic.
//!
//! **Ordering.** Upstream calls `substituteEnd` *after* `target.push(this)`
//! and appends closures to `this.execs` while pushing further tokens behind
//! it. Rust tokens move into `target`, so [`substitute_end`] returns the
//! closures for `this` and the extra tokens separately and
//! [`push_with_substitute_end`] applies both in upstream's order.

use serde_json::Value;

use super::load::DISPLAY_CLASSES;
use super::exec::{Exec, Test};
use super::js;
use super::node_choose;
use super::obj_token::{Decoration, Token, TokenType};
use super::queue::{self, QueueId};
use super::state::State;
use super::{CslResult, EngineError};

/// The closures `src/util_substitute.js` stores in `token.execs`
/// (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum UtilSubstituteExec {
    /// `strip-periods` open: `state.tmp.strip_periods += 1` when a
    /// `@strip-periods="true"` decoration exists (util_substitute.js:4-11).
    StripPeriodsIncrement,
    /// The `element_trace` closure (util_substitute.js:25-62).
    ElementTrace,
    /// `bib_first` token, `second-field-align` form (util_substitute.js:79-84).
    BibFirstSecondFieldAlign,
    /// `bib_first` token, `@display` form (util_substitute.js:92-95).
    BibFirstDisplay,
    /// `variable_entry` start (util_substitute.js:~118-170).
    VariableEntryStart,
    /// `variable_entry` end (util_substitute.js:~180).
    VariableEntryEnd,
    /// `strip-periods` close: `state.tmp.strip_periods -= 1`.
    StripPeriodsDecrement,
    /// `state.output.endTag("bib_first")` (util_substitute.js:~200).
    BibFirstEnd,
    /// `bib_first_end` group token's closure (util_substitute.js:~207).
    BibFirstEndGroup,
    /// `bib_other` group token's closure (util_substitute.js:~216).
    BibOtherStart,
    /// The subsequent-author-substitute closure (util_substitute.js:~234-330).
    AuthorSubstitute {
        /// `substitution_name`: the element name (`names` or `text`).
        substitution_name: String,
    },
    /// The `element_trace` pop closure (util_substitute.js:~335-345).
    ElementTracePop,
}

fn has_strip_periods(token: &Token) -> bool {
    token
        .decorations
        .iter()
        .any(|d| d.name == "@strip-periods" && d.value == "true")
}

impl UtilSubstituteExec {
    /// Run the closure.
    pub fn run(
        &self,
        state: &mut State,
        token: &mut Token,
        item: &Value,
        cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            UtilSubstituteExec::StripPeriodsIncrement => {
                if has_strip_periods(token) {
                    state.tmp.strip_periods += 1;
                }
                Ok(None)
            }
            UtilSubstituteExec::StripPeriodsDecrement => {
                if has_strip_periods(token) {
                    state.tmp.strip_periods -= 1;
                }
                Ok(None)
            }
            UtilSubstituteExec::ElementTrace => {
                let item = cite_item;
                let has_item = js::truthy(item);
                let author_only = has_item && js::truthy_opt(item.get("author-only"));
                let suppress_author = has_item && js::truthy_opt(item.get("suppress-author"));
                let not_intext = state.tmp.area != "intext";
                let jl = state.tmp.just_looking;
                let top_is_author =
                    state.tmp.element_trace.value().map(String::as_str) == Some("author");
                if top_is_author || token.name == "names" {
                    if !jl && author_only && not_intext && state.tmp.probably_rendered_something {
                        state.tmp.element_trace.push("suppress-me".to_string());
                    }
                    if !jl && suppress_author && !state.tmp.probably_rendered_something {
                        state.tmp.element_trace.push("suppress-me".to_string());
                    }
                } else if token.name == "date" {
                    if !jl && author_only && not_intext && state.tmp.probably_rendered_something {
                        state.tmp.element_trace.push("suppress-me".to_string());
                    }
                } else if !jl && author_only && not_intext {
                    // XXX can_block_substitute probably is doing nothing here.
                    if state.tmp.probably_rendered_something || !state.tmp.can_block_substitute {
                        state.tmp.element_trace.push("suppress-me".to_string());
                    }
                } else if suppress_author {
                    state
                        .tmp
                        .element_trace
                        .push("do-not-suppress-me".to_string());
                }
                Ok(None)
            }
            UtilSubstituteExec::ElementTracePop => {
                if state.tmp.element_trace.len() > 1 {
                    state.tmp.element_trace.pop();
                }
                Ok(None)
            }
            UtilSubstituteExec::BibFirstSecondFieldAlign => {
                // `bib_first` is this very token (the closure captured it).
                if !state.tmp.render_seen {
                    token.strings.insert(
                        "first_blob".into(),
                        item.get("id").cloned().unwrap_or(Value::Null),
                    );
                    queue::start_tag(state, QueueId::Output, "bib_first", Some(token))?;
                }
                Ok(None)
            }
            UtilSubstituteExec::BibFirstDisplay => {
                token.strings.insert(
                    "first_blob".into(),
                    item.get("id").cloned().unwrap_or(Value::Null),
                );
                queue::start_tag(state, QueueId::Output, "bib_first", Some(token))?;
                Ok(None)
            }
            UtilSubstituteExec::VariableEntryStart => {
                if !state.tmp.just_looking && !state.tmp.suppress_decorations {
                    // Attach item data and variable names.
                    // Do with them what you will.
                    let mut variable_entry = Token::new("text", TokenType::Start);
                    variable_entry.decorations = vec![Decoration::new("@showid", "true")];
                    queue::start_tag(
                        state,
                        QueueId::Output,
                        "variable_entry",
                        Some(&variable_entry),
                    )?;
                    let has_cite = js::truthy(cite_item);
                    let cite_num = |key: &str| -> Value {
                        if has_cite && js::truthy_opt(cite_item.get(key)) {
                            cite_item.get(key).cloned().unwrap_or(Value::from(0))
                        } else {
                            Value::from(0)
                        }
                    };
                    let mut position: Value = if has_cite {
                        cite_item.get("position").cloned().unwrap_or(Value::Null)
                    } else {
                        Value::Null
                    };
                    if !js::truthy(&position) {
                        position = Value::from(0);
                    }
                    let position_map = [
                        "first",
                        "container-subsequent",
                        "subsequent",
                        "ibid",
                        "ibid-with-locator",
                    ];
                    let position_name = position
                        .as_u64()
                        .and_then(|p| position_map.get(p as usize))
                        .map(|s| Value::String((*s).to_string()))
                        .unwrap_or(Value::Null);
                    let mut params = js::Obj::new();
                    params.insert("itemData".into(), item.clone());
                    params.insert(
                        "variableNames".into(),
                        Value::Array(
                            token
                                .variables
                                .iter()
                                .map(|v| Value::String(v.clone()))
                                .collect(),
                        ),
                    );
                    params.insert("context".into(), Value::String(state.tmp.area.clone()));
                    params.insert(
                        "xclass".into(),
                        state.opt.get("xclass").cloned().unwrap_or(Value::Null),
                    );
                    params.insert("position".into(), position_name);
                    params.insert("note-number".into(), cite_num("noteIndex"));
                    params.insert(
                        "first-reference-note-number".into(),
                        cite_num("first-reference-note-number"),
                    );
                    params.insert(
                        "first-container-reference-note-number".into(),
                        cite_num("first-container-reference-note-number"),
                    );
                    // XXX Will this EVER happen?
                    params.insert("citation-number".into(), cite_num("citation-number"));
                    params.insert("index".into(), cite_num("index"));
                    params.insert(
                        "mode".into(),
                        state.opt.get("mode").cloned().unwrap_or(Value::Null),
                    );
                    if let Some(cur) = queue::current(state, QueueId::Output) {
                        state
                            .blobs
                            .get_mut(cur)
                            .extra
                            .insert("params".into(), Value::Object(params));
                    }
                }
                Ok(None)
            }
            UtilSubstituteExec::VariableEntryEnd => {
                if !state.tmp.just_looking && !state.tmp.suppress_decorations {
                    queue::end_tag(state, QueueId::Output, Some("variable_entry"))?;
                }
                Ok(None)
            }
            UtilSubstituteExec::BibFirstEnd => {
                queue::end_tag(state, QueueId::Output, Some("bib_first"))?;
                Ok(None)
            }
            UtilSubstituteExec::BibFirstEndGroup => {
                // first func end
                if !state.tmp.render_seen {
                    queue::end_tag(state, QueueId::Output, Some("bib_first"))?; // closes bib_first
                }
                Ok(None)
            }
            UtilSubstituteExec::BibOtherStart => {
                if !state.tmp.render_seen {
                    state.tmp.render_seen = true;
                    queue::start_tag(state, QueueId::Output, "bib_other", Some(token))?;
                }
                Ok(None)
            }
            UtilSubstituteExec::AuthorSubstitute { substitution_name } => {
                // The guards of the closure (util_substitute.js:~250-264).
                if state.tmp.area != "bibliography" {
                    return Ok(None);
                }
                if !matches!(
                    state.bibliography.opt.get("subsequent-author-substitute"),
                    Some(Value::String(_))
                ) {
                    return Ok(None);
                }
                // `this.variables_real` is an array, so `Item[this.variables_real]`
                // reads the key its comma-joined text spells.
                let vr_key = token
                    .extra
                    .get("variables_real")
                    .map(js::to_js_string)
                    .unwrap_or_default();
                if token.extra.contains_key("variables_real")
                    && !js::truthy_opt(item.get(vr_key.as_str()))
                {
                    return Ok(None);
                }
                // The logic of these two is not obvious. The effect is to
                // enable placeholder substitution on a text macro name
                // substitution, without printing both the text macro AND the
                // placeholder.
                if token.extra.contains_key("variables_real") && substitution_name == "names" {
                    return Ok(None);
                }
                // PORT-LATER(w2-names): the rest of the closure works on
                // state.tmp.rendered_name / name_node / last_rendered_name /
                // label_blob (the cs:names code) and
                // state.tmp.subsequent_author_substitute_ok (citeStart).
                Err(EngineError::NotYetPorted {
                    method: "util_substitute.js:~265 subsequent-author-substitute",
                })
            }
        }
    }
}

/// The condition closures `src/util_substitute.js` stores in `token.tests`
/// (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum UtilSubstituteTest {
    /// `function () { return state.tmp.can_substitute.value() ? true : false }`
    /// (util_substitute.js:~123).
    CanSubstitute,
}

impl UtilSubstituteTest {
    /// Evaluate the condition.
    pub fn eval(
        &self,
        state: &mut State,
        _token: &mut Token,
        _item: &Value,
        _cite_item: &Value,
    ) -> CslResult<bool> {
        match self {
            UtilSubstituteTest::CanSubstitute => Ok(state
                .tmp
                .can_substitute
                .value()
                .map(js::truthy)
                .unwrap_or(false)),
        }
    }
}

/// `CSL.Util.substituteStart.call(token, state, target)`.
pub fn substitute_start(
    state: &mut State,
    token: &mut Token,
    target: &mut Vec<Token>,
) -> CslResult<()> {
    token.execs.push(Exec::UtilSubstitute(
        UtilSubstituteExec::StripPeriodsIncrement,
    ));
    let reverse_lookup = state
        .opt
        .get("development_extensions")
        .map(|d| js::truthy_opt(d.get("csl_reverse_lookup_support")))
        .unwrap_or(false);
    if reverse_lookup {
        // this.decorations.reverse(); push(["@showid","true",cslid]); reverse()
        let mut d = Decoration::new("@showid", "true");
        d.extra = token.extra.get("cslid").map(js::to_js_string);
        token.decorations.insert(0, d);
    }
    //
    // Contains body code for both substitute and first-field/remaining-fields
    // formatting.
    //
    let name = token.name.as_str();
    if (name == "text" && token.postponed_macro.is_none())
        || ["number", "date", "names"].contains(&name)
    {
        token
            .execs
            .push(Exec::UtilSubstitute(UtilSubstituteExec::ElementTrace));
    }
    let display: Option<String> = token.strings.get("cls").and_then(|v| match v {
        Value::String(s) => Some(s.clone()),
        _ => None,
    });
    token.strings.insert("cls".into(), Value::Bool(false));
    if state.build.render_nesting_level == 0 {
        //
        // The markup formerly known as @bibliography/first
        //
        // Separate second-field-align from the generic display logic.
        let second_field_align = js::truthy_opt(state.bibliography.opt.get("second-field-align"));
        if state.build.area == "bibliography" && second_field_align {
            let mut bib_first = Token::new("group", TokenType::Start);
            bib_first.decorations = vec![Decoration::new("@display", "left-margin")];
            bib_first.execs.push(Exec::UtilSubstitute(
                UtilSubstituteExec::BibFirstSecondFieldAlign,
            ));
            target.push(bib_first);
        } else if display
            .as_deref()
            .map(|d| DISPLAY_CLASSES.contains(&d))
            .unwrap_or(false)
        {
            let d = display.clone().unwrap_or_default();
            let mut bib_first = Token::new("group", TokenType::Start);
            bib_first.decorations = vec![Decoration::new("@display", &d)];
            bib_first
                .execs
                .push(Exec::UtilSubstitute(UtilSubstituteExec::BibFirstDisplay));
            target.push(bib_first);
        }
        state.build.cls = display;
    }
    state.build.render_nesting_level += 1;
    // Should this be render_nesting_level, with the increment
    // below? ... ?
    if state.build.substitute_level.value().copied() == Some(1) {
        //
        // All top-level elements in a substitute environment get
        // wrapped in conditionals.
        let choose_start = Token::new("choose", TokenType::Start);
        node_choose::build(state, choose_start, target, None)?;
        let mut if_start = Token::new("if", TokenType::Start);
        //
        // Set a test of the shadow if token to skip this
        // macro if we have acquired a name value.
        if_start.tests_defined = true;
        if_start
            .tests
            .push(Test::UtilSubstitute(UtilSubstituteTest::CanSubstitute));
        if_start.test = Some(state.fun.match_.any(&if_start, state, &if_start.tests));
        target.push(if_start);
    }

    if state.fun.host_hooks.variable_wrapper && variables_real_len(token) > 0 {
        token
            .execs
            .push(Exec::UtilSubstitute(UtilSubstituteExec::VariableEntryStart));
    }
    Ok(())
}

fn variables_real_len(token: &Token) -> usize {
    token
        .extra
        .get("variables_real")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0)
}

/// What `CSL.Util.substituteEnd.call(this, state, target)` adds.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SubstituteEnd {
    /// Closures appended to `this.execs`, in order.
    pub execs: Vec<Exec>,
    /// Tokens pushed to the target after `this`, in order.
    pub tokens: Vec<Token>,
}

/// `CSL.Util.substituteEnd.call(token, state, target)`, as the closures for
/// `this` and the tokens that follow it (see the module docs).
pub fn substitute_end(state: &mut State, token: &Token) -> CslResult<SubstituteEnd> {
    let mut out = SubstituteEnd::default();
    let has_variable = js::truthy_opt(token.extra.get("hasVariable"));
    if state.fun.host_hooks.variable_wrapper && (has_variable || variables_real_len(token) > 0) {
        out.execs
            .push(Exec::UtilSubstitute(UtilSubstituteExec::VariableEntryEnd));
    }

    out.execs.push(Exec::UtilSubstitute(
        UtilSubstituteExec::StripPeriodsDecrement,
    ));

    state.build.render_nesting_level -= 1;
    if state.build.render_nesting_level == 0 {
        let second_field_align = js::truthy_opt(state.bibliography.opt.get("second-field-align"));
        if state
            .build
            .cls
            .as_deref()
            .map(|c| !c.is_empty())
            .unwrap_or(false)
        {
            out.execs
                .push(Exec::UtilSubstitute(UtilSubstituteExec::BibFirstEnd));
            state.build.cls = None;
        } else if state.build.area == "bibliography" && second_field_align {
            let mut bib_first_end = Token::new("group", TokenType::End);
            // first func end
            bib_first_end
                .execs
                .push(Exec::UtilSubstitute(UtilSubstituteExec::BibFirstEndGroup));
            out.tokens.push(bib_first_end);
            let mut bib_other = Token::new("group", TokenType::Start);
            bib_other.decorations = vec![Decoration::new("@display", "right-inline")];
            bib_other
                .execs
                .push(Exec::UtilSubstitute(UtilSubstituteExec::BibOtherStart));
            out.tokens.push(bib_other);
        }
    }
    if state.build.substitute_level.value().copied() == Some(1) {
        let if_end = Token::new("if", TokenType::End);
        out.tokens.push(if_end);
        let choose_end = Token::new("choose", TokenType::End);
        node_choose::build(state, choose_end, &mut out.tokens, None)?;
    }

    // `this.variables_real !== "title"` compares an array to a string: always
    // true upstream.
    if token.name == "names" || token.name == "text" {
        out.execs
            .push(Exec::UtilSubstitute(UtilSubstituteExec::AuthorSubstitute {
                substitution_name: token.name.clone(),
            }));
    }

    if (token.name == "text" && token.postponed_macro.is_none())
        || ["number", "date", "names"].contains(&token.name.as_str())
    {
        // element trace
        out.execs
            .push(Exec::UtilSubstitute(UtilSubstituteExec::ElementTracePop));
    }
    Ok(out)
}

/// `target.push(this)` (when `push_this`) followed by
/// `CSL.Util.substituteEnd.call(this, state, target)`.
pub fn push_with_substitute_end(
    state: &mut State,
    mut token: Token,
    target: &mut Vec<Token>,
    push_this: bool,
) -> CslResult<()> {
    let end = substitute_end(state, &token)?;
    token.execs.extend(end.execs);
    if push_this {
        target.push(token);
    }
    target.extend(end.tokens);
    Ok(())
}
