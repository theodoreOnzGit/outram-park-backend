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
use super::CslResult;

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
                author_substitute(state, token, item, substitution_name)?;
                Ok(None)
            }
        }
    }
}

/// The subsequent-author-substitute closure of `CSL.Util.substituteEnd`
/// (util_substitute.js:234-342), run on the `cs:names` or `cs:text` token
/// `token` for `Item = item`: replace the rendered names that repeat those of
/// the previous bibliography entry by
/// `bibliography.opt["subsequent-author-substitute"]`.
fn author_substitute(
    state: &mut State,
    token: &Token,
    item: &Value,
    substitution_name: &str,
) -> CslResult<()> {
    use super::obj_blob::{Blob, BlobChild, BlobContent};
    use std::cmp::Ordering;

    if state.tmp.area != "bibliography" {
        return Ok(());
    }
    let Some(Value::String(substitute)) = state
        .bibliography
        .opt
        .get("subsequent-author-substitute")
        .cloned()
    else {
        return Ok(());
    };
    // `this.variables_real` is an array; `Item[array]` looks up the
    // comma-joined key.
    let variables_real: Option<Vec<String>> = token
        .extra
        .get("variables_real")
        .and_then(Value::as_array)
        .map(|a| a.iter().map(js::to_js_string).collect());
    if let Some(vr) = &variables_real {
        if !js::truthy_opt(item.get(vr.join(",").as_str())) {
            return Ok(());
        }
    }
    // The logic of these two is not obvious. The effect is to enable placeholder substitution
    // on a text macro name substitution, without printing both the text macro AND the placeholder.
    // See https://forums.zotero.org/discussion/comment/350407
    if variables_real.is_some() && substitution_name == "names" {
        return Ok(());
    }

    let subrule = state
        .bibliography
        .opt
        .get("subsequent-author-substitute-rule")
        .and_then(Value::as_str)
        .map(str::to_string);
    let printing = !state.tmp.suppress_decorations;
    if printing && state.tmp.subsequent_author_substitute_ok {
        if let Some(rendered) = state.tmp.rendered_name.clone() {
            let new_blob =
                |state: &mut State| state.blobs.add(Blob::new(Some(&substitute), None, None));
            let same = |a: &str, b: &str| js::locale_compare(a, b, "en") == Ordering::Equal;
            if matches!(
                subrule.as_deref(),
                Some("partial-each") | Some("partial-first")
            ) {
                let mut dosub = true;
                let mut rendered_name: Vec<Value> = Vec::new();
                let children = state.tmp.name_node.children.clone();
                let last = state.tmp.last_rendered_name.clone();
                for (i, child) in children.iter().enumerate() {
                    let name = rendered.get(i).cloned();
                    let last_len: Option<usize> = match &last {
                        Value::Array(a) => Some(a.len()),
                        Value::String(t) if !t.is_empty() => Some(js::len(t)),
                        _ => None,
                    };
                    let last_i: String = match &last {
                        Value::Array(a) => a.get(i).map(js::to_js_string),
                        Value::String(t) => {
                            Some(js::char_at(t, i as i64)).filter(|c| !c.is_empty())
                        }
                        _ => None,
                    }
                    .unwrap_or_else(|| "undefined".to_string());
                    let name_s = name
                        .as_ref()
                        .filter(|n| js::truthy(n))
                        .map(js::to_js_string);
                    if dosub
                        && last_len.map(|l| l as i64 > i as i64 - 1).unwrap_or(false)
                        && name_s.as_deref().map(|n| same(n, &last_i)).unwrap_or(false)
                    {
                        let str_ = new_blob(state);
                        let Some(child) = child else {
                            return Err(super::load::type_error(
                                "Cannot create property 'blobs' on boolean 'false'",
                            ));
                        };
                        state.blobs.get_mut(*child).blobs =
                            BlobContent::List(vec![BlobChild::Blob(str_)]);
                        if subrule.as_deref() == Some("partial-first") {
                            dosub = false;
                        }
                    } else {
                        dosub = false;
                    }
                    rendered_name.push(name.unwrap_or(Value::Null));
                }
                // might want to slice this?
                state.tmp.last_rendered_name = Value::Array(rendered_name);
            } else if subrule.as_deref() == Some("complete-each") {
                let rendered_name = rendered
                    .iter()
                    .map(js::to_js_string)
                    .collect::<Vec<_>>()
                    .join(",");
                if !rendered_name.is_empty() {
                    let last = state.tmp.last_rendered_name.clone();
                    if js::truthy(&last) && same(&rendered_name, &js::to_js_string(&last)) {
                        let children = state.tmp.name_node.children.clone();
                        for child in children {
                            let str_ = new_blob(state);
                            let Some(child) = child else {
                                return Err(super::load::type_error(
                                    "Cannot create property 'blobs' on boolean 'false'",
                                ));
                            };
                            state.blobs.get_mut(child).blobs =
                                BlobContent::List(vec![BlobChild::Blob(str_)]);
                        }
                    }
                    state.tmp.last_rendered_name = Value::String(rendered_name);
                }
            } else {
                let rendered_name = rendered
                    .iter()
                    .map(js::to_js_string)
                    .collect::<Vec<_>>()
                    .join(",");
                if !rendered_name.is_empty() {
                    let last = state.tmp.last_rendered_name.clone();
                    if js::truthy(&last) && same(&rendered_name, &js::to_js_string(&last)) {
                        let str_ = new_blob(state);
                        let top = state.tmp.name_node.top.ok_or_else(|| {
                            super::load::type_error(
                                "Cannot read properties of undefined (reading 'blobs')",
                            )
                        })?;
                        if let Some(label_blob) = state.tmp.label_blob {
                            state.blobs.get_mut(top).blobs = BlobContent::List(vec![
                                BlobChild::Blob(str_),
                                BlobChild::Blob(label_blob),
                            ]);
                        } else {
                            let first = match &state.blobs.get(top).blobs {
                                BlobContent::List(l) => l.first().cloned(),
                                BlobContent::Text(_) => None,
                            };
                            match first {
                                Some(BlobChild::Blob(b)) => {
                                    state.blobs.get_mut(b).blobs =
                                        BlobContent::List(vec![BlobChild::Blob(str_)]);
                                }
                                Some(BlobChild::Str(_)) => {}
                                None => {
                                    state.blobs.get_mut(top).blobs =
                                        BlobContent::List(vec![BlobChild::Blob(str_)]);
                                }
                            }
                        }
                        state.tmp.substituted_variable = Some(substitution_name.to_string());
                    }
                    state.tmp.last_rendered_name = Value::String(rendered_name);
                }
            }
            state.tmp.subsequent_author_substitute_ok = false;
        }
    }
    Ok(())
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
