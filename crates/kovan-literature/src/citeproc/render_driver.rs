// Part of the kovan port of citeproc-js (GitHub #790, #793).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
//              (src/api_cite.js: makeCitationCluster, CSL.getCitationCluster,
//              CSL.getCite, CSL.citeStart, CSL.citeEnd, CSL.getSpliceDelimiter;
//              src/node_layout.js: the closures of cs:layout) and
//              juris-m/citeproc-test-runner 1.1.116 (lib/fixture-parser.js,
//              lib/sys.js), MIT
// Version:     2.4.63, commit 73bc1b44bc7d54d0bfec4e070fd27f5efe024ff9
// Copyright:   (c) 2009-2019 Frank Bennett
// Licence:     AGPL-3.0, taken from upstream's "CPAL-1.0 or AGPL-3.0-or-later"
//              (LICENSE at the commit above; see this crate's NOTICE).
// Modified:    2026-10-08, by the OUTRAM PARK contributors. A test-only
//              Rust translation of the parts of the files named above that
//              the rendering nodes need, modified from the original.
// No warranty: this program is distributed in the hope that it will be
//              useful, but WITHOUT ANY WARRANTY; without even the implied
//              warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
//              PURPOSE. See the GNU Affero General Public License.

//! A TEST-ONLY driver that renders citations end to end with the rendering
//! nodes of wave 2, before the engine agent's `getCite` exists (GitHub #793).
//!
//! It builds the engine for a fixture of the CSL test suite exactly as the
//! runner does, then renders each cluster of `CITATION-ITEMS` the way
//! `makeCitationCluster` / `CSL.getCitationCluster` / `CSL.getCite` do for the
//! non-note, unsorted, non-collapsing, registry-less case, running only the
//! body tokens of `cs:layout` and doing by hand what the layout closures of
//! node_layout.js do (open and close the level, the cite's prefix and suffix).
//! The result is compared with what citeproc-js produced for the same
//! fixture (`tests/data/csl/test_suite_reference.json`).
//!
//! What it does NOT do (those fixtures are counted as skipped or blocked, not
//! as passes): the registry (`updateItems`: sorting of the bibliography,
//! disambiguation, citation numbers, year suffixes), collapsing, sorting of
//! cites, `CITATIONS` (processCitationCluster), bibliographies, multi-layout
//! styles, numeric blobs joining across cites.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use regex::Regex;
use serde_json::Value;

use crate::citeproc::js::{self, Obj};
use crate::citeproc::load::{
    check_ignore_predecessor, check_prefix_space_append, check_suffix_space_prepend,
    get_safe_escape, CheckNestedBrace, TERMINAL_PUNCTUATION,
};
use crate::citeproc::obj_blob::{BlobChild, BlobContent, BlobId};
use crate::citeproc::obj_token::{Token, TokenType};
use crate::citeproc::queue::{self, AppendArg, FormatRef, QueueId, Rendered, StringParent};
use crate::citeproc::build_retrieve_item::retrieve_item as retrieve;
use crate::citeproc::state::State;
use crate::citeproc::util_nodes::TokenList;
use crate::citeproc::{CslResult, Engine, EngineError, OutputFormat, Sys};

pub(super) fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub(super) fn fixture_dir() -> PathBuf {
    workspace_root().join("vendor/csl-test-suite/processor-tests/humans")
}

fn locale_dir() -> PathBuf {
    workspace_root().join("vendor/citeproc-js/locale")
}

// ---------------------------------------------------------------- fixtures

/// lib/sections.js, in the runner's order (it matters for the alternation).
const SECTIONS: [&str; 18] = [
    "CSL",
    "KEYS",
    "DESCRIPTION",
    "INPUT",
    "MODE",
    "RESULT",
    "NAME",
    "PATH",
    "ABBREVIATIONS",
    "BIBENTRIES",
    "BIBSECTION",
    "CITATION-ITEMS",
    "CITATIONS",
    "INPUT2",
    "LANGPARAMS",
    "MULTIAFFIX",
    "OPTIONS",
    "OPTIONZ",
];

/// A parsed fixture (the parts the driver uses).
#[derive(Debug, Clone)]
pub(super) struct Fixture {
    pub name: String,
    pub csl: String,
    pub mode: String,
    pub input: Vec<Value>,
    pub abbreviations: Option<Value>,
    pub has_bibentries: bool,
    pub has_bibsection: bool,
    pub bibentries: Option<Vec<Vec<String>>>,
    pub has_citations: bool,
    pub citation_items: Option<Vec<Vec<Value>>>,
    pub langparams: Option<Value>,
    pub multiaffix: Option<Value>,
    pub options: Option<Obj>,
    /// The engine language (`new CSL.Engine(sys, style, lang)`), `""` for none.
    pub lang: String,
    /// An output format set after construction (`setOutputFormat`).
    pub format: Option<String>,
    /// `{translit: [...], translat: [...]}` language tags.
    pub tags: Option<Value>,
}

fn read_sections(text: &str) -> Result<BTreeMap<String, Vec<String>>, String> {
    let names = SECTIONS.join("|");
    let open = Regex::new(&format!(r"^.*>>===*\s({names})\s.*=>>.*")).map_err(|e| e.to_string())?;
    let close =
        Regex::new(&format!(r"^.*<<===*\s({names})\s.*=<<.*")).map_err(|e| e.to_string())?;
    let mut obj: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut section = String::new();
    let mut state = "";
    for line in text.replace("\r\n", "\n").split('\n') {
        if let Some(m) = open.captures(line) {
            if !state.is_empty() {
                return Err(format!(
                    "attempted to open tag {} before {section} closed",
                    &m[1]
                ));
            }
            section = m[1].to_string();
            state = "opening";
        } else if let Some(m) = close.captures(line) {
            if section != m[1] {
                return Err(format!(
                    "expected closing tag {section} but found {}",
                    &m[1]
                ));
            }
            state = "closing";
            if section == "RESULT" && !obj.contains_key(&section) {
                obj.insert(section.clone(), vec![String::new()]);
            }
        } else if state == "opening" {
            obj.insert(section.clone(), Vec::new());
            state = "reading";
        } else if state == "closing" {
            state = "";
        }
        if state == "reading" {
            if let Some(lines) = obj.get_mut(&section) {
                lines.push(line.to_string());
            }
        }
    }
    Ok(obj)
}

pub(super) fn parse_fixture(name: &str, text: &str) -> Result<Fixture, String> {
    let sections = read_sections(text)?;
    let text_of = |key: &str| sections.get(key).map(|l| l.join("\n"));
    let json_of = |key: &str| -> Result<Option<Value>, String> {
        match text_of(key) {
            Some(t) => serde_json::from_str(&t)
                .map(Some)
                .map_err(|e| format!("JSON parse fail for tag {key} in {name}: {e}")),
            None => Ok(None),
        }
    };
    let citation_items = match json_of("CITATION-ITEMS")? {
        Some(Value::Array(sets)) => Some(
            sets.iter()
                .map(|s| s.as_array().cloned().unwrap_or_default())
                .collect(),
        ),
        _ => None,
    };
    let options = match json_of("OPTIONS")? {
        Some(Value::Object(m)) => Some(m),
        _ => None,
    };
    let input = match json_of("INPUT")? {
        Some(Value::Array(a)) => a,
        _ => return Err(format!("missing INPUT in {name}")),
    };
    Ok(Fixture {
        name: name.to_string(),
        csl: text_of("CSL").ok_or_else(|| format!("missing CSL in {name}"))?,
        mode: text_of("MODE").ok_or_else(|| format!("missing MODE in {name}"))?,
        input,
        abbreviations: json_of("ABBREVIATIONS")?,
        has_bibentries: sections.contains_key("BIBENTRIES"),
        has_bibsection: sections.contains_key("BIBSECTION"),
        bibentries: match json_of("BIBENTRIES")? {
            Some(Value::Array(sets)) => Some(
                sets.iter()
                    .map(|s| {
                        s.as_array()
                            .map(|a| a.iter().map(js::to_js_string).collect())
                            .unwrap_or_default()
                    })
                    .collect(),
            ),
            _ => None,
        },
        has_citations: sections.contains_key("CITATIONS"),
        citation_items,
        langparams: json_of("LANGPARAMS")?,
        multiaffix: json_of("MULTIAFFIX")?,
        options,
        lang: String::new(),
        format: None,
        tags: None,
    })
}

/// All fixtures of the vendored suite; `None` when `vendor/` is absent.
pub(super) fn load_fixtures() -> Option<BTreeMap<String, Fixture>> {
    let read = std::fs::read_dir(fixture_dir()).ok()?;
    let mut out = BTreeMap::new();
    let first = Regex::new(r"^[a-z]+_").ok()?;
    for entry in read.flatten() {
        let file = entry.file_name().to_string_lossy().to_string();
        let Some(stem) = file.strip_suffix(".txt") else {
            continue;
        };
        if !first.is_match(stem) {
            continue;
        }
        let text = std::fs::read_to_string(entry.path()).ok()?;
        let fx = parse_fixture(stem, &text).ok()?;
        out.insert(stem.to_string(), fx);
    }
    Some(out)
}

/// Every `locales-<lang>.xml` of `vendor/citeproc-js/locale`.
pub(super) fn load_locales() -> Option<Arc<BTreeMap<String, String>>> {
    let read = std::fs::read_dir(locale_dir()).ok()?;
    let pi = Regex::new(r"\s*<\?[^>]*\?>\s*\n").ok()?;
    let mut map = BTreeMap::new();
    for entry in read.flatten() {
        let file = entry.file_name().to_string_lossy().to_string();
        let Some(lang) = file
            .strip_prefix("locales-")
            .and_then(|f| f.strip_suffix(".xml"))
        else {
            continue;
        };
        let xml = std::fs::read_to_string(entry.path()).ok()?;
        map.insert(lang.to_string(), pi.replace_all(&xml, "").to_string());
    }
    if map.is_empty() {
        None
    } else {
        Some(Arc::new(map))
    }
}

// ------------------------------------------------------------------ engine

/// Why a fixture was not rendered.
#[derive(Debug)]
pub(super) enum Outcome {
    /// The driver does not cover this kind of fixture.
    Skipped(String),
    /// A part of the port that is not there yet (or an error citeproc-js
    /// itself raises) stopped it.
    Blocked(String),
    /// Rendered.
    Rendered(String),
}

fn js_string(v: &Value) -> String {
    js::to_js_string(v)
}

/// The runner's set-up of an engine for a fixture (lib/sys.js `run`, up to
/// the first `updateItems`).
pub(super) fn build_engine(
    fx: &Fixture,
    locales: &Arc<BTreeMap<String, String>>,
) -> CslResult<(Engine, Vec<String>)> {
    let mut sys = Sys::new(&fx.input, locales.clone())?;
    // The runner copies the fixture's OPTIONS onto `sys` before the Engine is built
    // (`this[option] = OPTIONS[option]`), which is what makes `variableWrapper` visible
    // to the build.
    if let Some(options) = &fx.options {
        for (k, v) in options {
            sys.options.insert(k.clone(), v.clone());
        }
    }
    let mut engine = Engine::new(sys, &fx.csl, &fx.lang)?;
    if fx.options.as_ref().and_then(|o| o.get("variableWrapper")).is_some_and(js::truthy)
        && !engine.state().fun.host_hooks.variable_wrapper
    {
        // build.rs `State::new` sets `host_hooks.variable_wrapper` and then replaces
        // `s.fun` with `Fun::new()`, so the hook is lost before the token lists are
        // built (a defect of the build stage, reported to the integrator).
        return Err(EngineError::BadInput(
            "DRIVER: variableWrapper lost by State::new (build.rs resets s.fun)".into(),
        ));
    }
    if let Some(f) = &fx.format {
        crate::citeproc::formats::set_output_format(engine.state_mut(), f)?;
    }
    if let Some(t) = &fx.tags {
        let strs = |v: Option<&Value>| -> Vec<String> {
            v.and_then(Value::as_array)
                .map(|a| a.iter().map(js_string).collect())
                .unwrap_or_default()
        };
        engine.set_lang_tags_for_csl_translation(strs(t.get("translat")));
        engine.set_lang_tags_for_csl_transliteration(strs(t.get("translit")));
    }
    let mut mode = fx.mode.split('-');
    let _mode_name = mode.next();
    let submodes: Vec<String> = mode.map(str::to_string).collect();
    for name in ["rtf", "plain", "asciidoc", "xslfo"] {
        if submodes.iter().any(|s| s == name) {
            if let Some(f) = OutputFormat::from_name(name) {
                engine.set_output_format(f);
            }
        }
    }
    if submodes
        .iter()
        .any(|s| s == "suppress_trailing_punctuation")
    {
        engine.set_suppress_trailing_punctuation(true);
    }
    if let Some(options) = &fx.options {
        if options.get("variableWrapper").is_some_and(js::truthy) {
            engine.set_variable_wrapper(true);
        }
        for (name, value) in options {
            if name != "variableWrapper" {
                engine.set_development_extension(name, value.clone());
            }
        }
    }
    let mut lang_params: BTreeMap<String, Vec<String>> = [
        ("persons", vec!["translit"]),
        ("institutions", vec!["translit"]),
        ("titles", vec!["translit", "translat"]),
        ("journals", vec!["translit"]),
        ("publishers", vec!["translat"]),
        ("places", vec!["translat"]),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.into_iter().map(str::to_string).collect()))
    .collect();
    if let Some(Value::Object(overrides)) = &fx.langparams {
        for (key, value) in overrides {
            let strings = |v: &Value| -> Vec<String> {
                v.as_array()
                    .map(|a| a.iter().map(js_string).collect())
                    .unwrap_or_default()
            };
            if key == "langs" {
                let translat = value.get("translat").map(strings);
                if let Some(t) = &translat {
                    engine.set_lang_tags_for_csl_translation(t.clone());
                }
                if value.get("translit").is_some() {
                    engine.set_lang_tags_for_csl_transliteration(translat.unwrap_or_default());
                }
                continue;
            }
            lang_params.insert(key.clone(), strings(value));
        }
    }
    engine.set_lang_prefs_for_cites(lang_params);
    if let Some(m) = &fx.multiaffix {
        engine.set_lang_prefs_for_cite_affixes(m.clone());
    }
    if let Some(abbrevs) = &fx.abbreviations {
        engine.set_runner_abbreviations(abbrevs);
    }
    Ok((engine, submodes))
}

// ----------------------------------------------------------- cluster rendering

/// The `[start, end]` indexes of the one `cs:layout` of the citation.
fn layout_range(state: &State, bibliography: bool) -> Option<(usize, usize)> {
    let toks = if bibliography {
        &state.bibliography.tokens
    } else {
        &state.citation.tokens
    };
    let starts: Vec<usize> = toks
        .iter()
        .enumerate()
        .filter(|(_, t)| t.name == "layout" && t.tokentype == TokenType::Start)
        .map(|(i, _)| i)
        .collect();
    let ends: Vec<usize> = toks
        .iter()
        .enumerate()
        .filter(|(_, t)| t.name == "layout" && t.tokentype == TokenType::End)
        .map(|(i, _)| i)
        .collect();
    if starts.len() == 1 && ends.len() == 1 && toks[starts[0]].extra.get("locale_raw").is_none() {
        Some((starts[0], ends[0]))
    } else {
        None
    }
}

/// `CSL.getCite` + `citeStart` + `citeEnd` for the citation area, with the
/// closures of `cs:layout` done by hand.
fn get_cite(
    state: &mut State,
    item_full: &Value,
    item: &Value,
    prev_item_id: Option<&str>,
    range: (usize, usize),
    bibliography: bool,
) -> CslResult<()> {
    let _ = prev_item_id;
    state.tmp.cite_renders_content = false;
    state.tmp.probably_rendered_something = false;
    // citeStart
    state.tmp.lang_array = Vec::new();
    if let Some(Value::String(lang)) = item_full.get("language") {
        let m: String = lang
            .chars()
            .take_while(|c| c.is_ascii_alphabetic())
            .collect();
        if !m.is_empty() {
            state.tmp.lang_array.push(m.to_lowercase());
        }
    }
    state
        .tmp
        .lang_array
        .push(js::get_str(&state.opt, "lang").unwrap_or("").to_string());
    state.tmp.disambig_settings = Default::default();
    state.tmp.names_used = Vec::new();
    state.tmp.nameset_counter = 0;
    state.tmp.years_used = Vec::new();
    state.tmp.names_max.clear();
    let parallel = item.get("parallel");
    if !state.tmp.just_looking
        && (!js::truthy(item)
            || parallel.and_then(Value::as_str) == Some("first")
            || !js::truthy_opt(parallel))
    {
        state.tmp.abbrev_trimmer = Some(Default::default());
    }
    state.splice_delimiter = (if bibliography {
        &state.bibliography
    } else {
        &state.citation
    })
    .opt
    .get("layout_delimiter")
    .map(js::to_js_string);
    state.bibliography_sort.keys = Vec::new();
    state.citation_sort.keys = Vec::new();
    state.tmp.last_cite_locale = None;
    state.tmp.authority_stop_last = 0;
    state.tmp.have_collapsed = false;
    state.tmp.render_seen = false;

    // The closures of cs:layout START (node_layout.js:63-119)
    state.tmp.done_vars = Vec::new();
    if js::truthy(item) && js::truthy_opt(item.get("author-only")) {
        state.tmp.done_vars.push("locator".to_string());
    }
    state.tmp.sort_key_flag = false;
    state.tmp.nameset_counter = 0;
    queue::open_level(
        state,
        QueueId::Output,
        FormatRef::Token(Token::new("", TokenType::Start)),
    )?;
    // the loop over the tokens (the prefix and suffix text tokens of
    // cs:layout carry closures of node_layout.js, done here by hand with
    // `this` = that token)
    let mut next = range.0 + 1;
    while next < range.1 {
        let kind = layout_closure(state, next, bibliography);
        match kind {
            Some(LayoutKind::Prefix) => {
                let this = area_tokens(state, bibliography)[next].clone();
                if js::truthy(item) && js::truthy_opt(item.get("prefix")) {
                    let prefix = check_prefix_space_append(state, &js_string(&item["prefix"]));
                    let prefix = if !state.tmp.just_looking {
                        match state.output.check_nested_brace.as_mut() {
                            Some(c) => c.update(&prefix),
                            None => prefix,
                        }
                    } else {
                        prefix
                    };
                    let ignore = check_ignore_predecessor(state, &prefix);
                    queue::append(
                        state,
                        QueueId::Output,
                        AppendArg::Text(prefix),
                        FormatRef::Token(this),
                        false,
                        ignore,
                        false,
                    )?;
                }
                next = area_tokens(state, bibliography)[next]
                    .next
                    .unwrap_or(crate::citeproc::util_nodes::NEXT_UNDEFINED);
            }
            Some(LayoutKind::CiteSuffix) => {
                let this = area_tokens(state, bibliography)[next].clone();
                if js::truthy(item) && js::truthy_opt(item.get("suffix")) {
                    let mut suffix = check_suffix_space_prepend(state, &js_string(&item["suffix"]));
                    if !state.tmp.just_looking {
                        if let Some(c) = state.output.check_nested_brace.as_mut() {
                            suffix = c.update(&suffix);
                        }
                    }
                    queue::append(
                        state,
                        QueueId::Output,
                        AppendArg::Text(suffix),
                        FormatRef::Token(this),
                        false,
                        false,
                        false,
                    )?;
                }
                next = area_tokens(state, bibliography)[next]
                    .next
                    .unwrap_or(crate::citeproc::util_nodes::NEXT_UNDEFINED);
            }
            Some(LayoutKind::BibSuffix) => {
                // setSuffix(): Suppress suffix on all but the last item in
                // bibliography parallels (the driver has no parallels).
                let suffix = js::get_str(&state.bibliography.opt, "layout_suffix")
                    .unwrap_or("")
                    .to_string();
                // If @display is used, layout suffix is placed on the last
                // immediate child of the layout, which we assume will be a
                // @display group node.
                if let Some(cur) = queue::current(state, QueueId::Output) {
                    if js::truthy_opt(state.opt.get("using_display")) {
                        let last = match &state.blobs.get(cur).blobs {
                            BlobContent::List(l) => l.last().cloned(),
                            BlobContent::Text(_) => None,
                        };
                        match last {
                            Some(BlobChild::Blob(last)) => {
                                state.blobs.get_mut(last).set_string("suffix", &suffix)
                            }
                            _ => {
                                return Err(EngineError::BadInput(
                                    "Cannot read properties of undefined (reading 'strings')"
                                        .into(),
                                ))
                            }
                        }
                    } else {
                        state.blobs.get_mut(cur).set_string("suffix", &suffix);
                    }
                }
                if js::truthy_opt(state.bibliography.opt.get("second-field-align")) {
                    // closes bib_other
                    queue::end_tag(state, QueueId::Output, Some("bib_other"))?;
                }
                next = area_tokens(state, bibliography)[next]
                    .next
                    .unwrap_or(crate::citeproc::util_nodes::NEXT_UNDEFINED);
            }
            None => {
                next = state.token_exec(
                    if bibliography {
                        &TokenList::Bibliography
                    } else {
                        &TokenList::Citation
                    },
                    next,
                    item_full,
                    item,
                )?;
            }
        }
    }

    // The closure of cs:layout END
    queue::close_level(state, QueueId::Output, None)?;

    // citeEnd
    state.tmp.last_suffix_used = if js::truthy(item) && js::truthy_opt(item.get("suffix")) {
        js_string(&item["suffix"])
    } else {
        String::new()
    };
    state.tmp.last_years_used = state.tmp.years_used.clone();
    state.tmp.last_names_used = state.tmp.names_used.clone();
    state.tmp.disambig_request = None;
    state.tmp.cite_locales.push(
        state
            .tmp
            .last_cite_locale
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Bool(false)),
    );
    Ok(())
}

/// `CSL.getSpliceDelimiter(last_locator, last_collapsed, pos)` for a style
/// without collapsing and without cite-group delimiters.
fn get_splice_delimiter(state: &mut State, last_locator: bool, last_collapsed: bool, pos: usize) {
    let after = state.citation.opt.get("after-collapse-delimiter").cloned();
    let layout_delimiter = state
        .citation
        .opt
        .get("layout_delimiter")
        .map(js::to_js_string);
    let have_collapsed = state.tmp.have_collapsed;
    if let Some(after) = after {
        let a = Some(js::to_js_string(&after));
        let collapse = state.citation.opt.get("collapse").cloned();
        let collapse_ys = collapse.as_ref().and_then(Value::as_str) == Some("year-suffix");
        if last_locator
            || (last_collapsed && !have_collapsed)
            || (!last_collapsed && !have_collapsed && !collapse_ys)
        {
            state.splice_delimiter = a;
        } else {
            state.splice_delimiter = layout_delimiter;
        }
    } else if have_collapsed
        && js::get_str(&state.opt, "xclass") == Some("in-text")
        && state.opt.get("update_mode").and_then(Value::as_i64)
            != Some(crate::citeproc::load::NUMERIC)
    {
        state.splice_delimiter = Some(", ".to_string());
    } else if pos > 0 && js::truthy(state.tmp.cite_locales.get(pos - 1).unwrap_or(&Value::Null)) {
        // alt affixes: not used by the fixtures the driver renders
    } else if !state
        .splice_delimiter
        .as_deref()
        .map(|s| !s.is_empty())
        .unwrap_or(false)
    {
        // This happens when no delimiter is set on cs:layout under cs:citation
        state.splice_delimiter = Some(String::new());
    }
}

/// Replace the children of the output queue's root array.
fn set_queue_children(state: &mut State, children: Vec<BlobChild>) {
    if let Some(root) = state.output.root {
        state.blobs.get_mut(root).blobs = BlobContent::List(children);
    }
}

fn root_id(state: &State) -> Option<BlobId> {
    state.output.root
}

/// `makeCitationCluster(rawList)` for the case this driver covers, over the
/// engine's state.
pub(super) fn make_citation_cluster(state: &mut State, raw_list: &[Value]) -> CslResult<String> {
    let Some(range) = layout_range(state, false) else {
        return Err(EngineError::BadInput("DRIVER: unsupported layout".into()));
    };
    if !state.citation_sort.tokens.is_empty() && raw_list.len() > 1 {
        return Err(EngineError::BadInput("DRIVER: sorted citation".into()));
    }
    if !js::get_str(&state.citation.opt, "collapse")
        .unwrap_or("")
        .is_empty()
        || state
            .citation
            .opt
            .get("collapse")
            .map(|c| c.as_array().map(|a| !a.is_empty()).unwrap_or(js::truthy(c)))
            .unwrap_or(false)
    {
        return Err(EngineError::BadInput("DRIVER: collapse".into()));
    }
    let mut input_list: Vec<(Value, Value)> = Vec::new();
    for raw in raw_list {
        let mut item: Obj = raw.as_object().cloned().unwrap_or_default();
        let id = item
            .get("id")
            .map(js::to_js_string)
            .unwrap_or_else(|| "undefined".to_string());
        let item_full = retrieve(state, &id)?;
        let mut item_obj: Obj = item_full.as_object().cloned().unwrap_or_default();
        crate::citeproc::api_cite::citation_item_input(state, &mut item_obj, &mut item)?;
        if js::truthy_opt(item.get("locator")) {
            let l = js::to_js_string(&item["locator"]);
            let trimmed = l.trim_end_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
            item.insert("locator".into(), Value::String(trimmed.to_string()));
        }
        input_list.push((Value::Object(item_obj), Value::Object(item)));
    }

    // CSL.getCitationCluster(inputList)
    let mut cnb = CheckNestedBrace::new(state);
    let citation_prefix = String::new();
    state.output.check_nested_brace = Some(cnb.clone());
    let txt_esc = get_safe_escape(state);
    state.tmp.area = "citation".to_string();
    state.tmp.root = "citation".to_string();
    let mut objects: Vec<Rendered> = Vec::new();
    state.tmp.last_suffix_used = String::new();
    state.tmp.last_names_used = Vec::new();
    state.tmp.last_years_used = Vec::new();
    state.tmp.cite_locales = Vec::new();
    if !state.tmp.just_looking {
        state.tmp.abbrev_trimmer = Some(Default::default());
    }
    let layout_prefix = js::get_str(&state.citation.opt, "layout_prefix")
        .unwrap_or("")
        .to_string();
    let use_layout_prefix = cnb.update(&format!("{layout_prefix}{citation_prefix}"));
    state.output.check_nested_brace = Some(cnb);
    let suppress_trailing_punctuation =
        js::truthy_opt(state.citation.opt.get("suppressTrailingPunctuation"));

    let mut myparams: Vec<(Option<String>, bool, bool)> = Vec::new();
    // `item` of the JS loop is still bound after it (the last cite processed).
    let mut last_item = Value::Null;
    let len = input_list.len();
    for pos in 0..len {
        let (item_full, item) = (input_list[pos].0.clone(), input_list[pos].1.clone());
        last_item = item.clone();
        let last_collapsed = state.tmp.have_collapsed;
        let last_locator = pos > 0 && js::truthy_opt(input_list[pos - 1].1.get("locator"));
        // Reset shadow_numbers here, suppress reset in getCite()
        state.tmp.shadow_numbers = BTreeMap::new();
        state.tmp.in_cite_predecessor = false;
        if pos > 0 {
            let prev = input_list[pos - 1]
                .0
                .get("id")
                .map(js::to_js_string)
                .unwrap_or_default();
            get_cite(state, &item_full, &item, Some(&prev), range, false)?;
        } else {
            state.tmp.term_predecessor = false;
            get_cite(state, &item_full, &item, None, range, false)?;
        }
        get_splice_delimiter(state, last_locator, last_collapsed, pos);
        let mut splice = state.splice_delimiter.clone();
        if pos > 0 {
            let preceding = &input_list[pos - 1].1;
            let psuffix = preceding.get("suffix").map(js_string).unwrap_or_default();
            let ends = !psuffix.is_empty()
                && [";", ".", ","].contains(&js::slice(&psuffix, -1, None).as_str());
            let prefix = item.get("prefix").map(js_string).unwrap_or_default();
            let starts = psuffix.is_empty()
                && !prefix.is_empty()
                && [";", ".", ","].contains(&js::slice(&prefix, 0, Some(1)).as_str());
            if ends || starts {
                let sd = splice.clone().unwrap_or_default();
                let spaceidx = js::index_of(&sd, " ", 0);
                if spaceidx > -1 && !starts {
                    splice = Some(js::slice(&sd, spaceidx, None));
                } else {
                    splice = Some(String::new());
                }
            }
        }
        myparams.push((
            splice,
            state.tmp.suppress_decorations,
            state.tmp.have_collapsed,
        ));
        if js::truthy_opt(item.get("author-only")) {
            break;
        }
    }

    let myblobs: Vec<BlobChild> = queue::queue_children(state, QueueId::Output);
    let suffix_raw = js::get_str(&state.citation.opt, "layout_suffix")
        .unwrap_or("")
        .to_string();
    let mut suffix = suffix_raw;
    if TERMINAL_PUNCTUATION[..TERMINAL_PUNCTUATION.len() - 1]
        .contains(&js::slice(&suffix, 0, Some(1)).as_str())
    {
        suffix = js::slice(&suffix, 0, Some(1));
    }
    let suffix = match state.output.check_nested_brace.as_mut() {
        Some(c) => c.update(&suffix),
        None => suffix,
    };
    let children = queue::queue_children(state, QueueId::Output);
    for c in &children {
        if let BlobChild::Blob(b) = c {
            queue::purge_empty_blobs(&mut state.blobs, *b);
        }
    }
    if !state.tmp.suppress_decorations && !children.is_empty() {
        if !suppress_trailing_punctuation {
            if let Some(BlobChild::Blob(b)) = children.last() {
                state.blobs.get_mut(*b).set_string("suffix", &suffix);
            }
        }
        if let Some(BlobChild::Blob(b)) = children.first() {
            state
                .blobs
                .get_mut(*b)
                .set_string("prefix", &use_layout_prefix);
        }
    }
    if crate::citeproc::load::dev_ext_truthy(state, "clean_up_csl_flaws") {
        let adjust = state.output.adjust.unwrap_or_default();
        for c in &children {
            if let BlobChild::Blob(b) = c {
                adjust.upward(&mut state.blobs, *b);
                adjust.leftward(&mut state.blobs, *b);
                adjust.downward(&mut state.blobs, *b);
                let _ = adjust.fix(&mut state.blobs, *b);
            }
        }
    }
    let mut empties = 0usize;
    for (pos, blob) in myblobs.iter().enumerate() {
        set_queue_children(state, vec![blob.clone()]);
        state.tmp.suppress_decorations = myparams[pos].1;
        state.splice_delimiter = myparams[pos].0.clone();
        state.tmp.have_collapsed = myparams[pos].2;
        let queue_now = queue::queue_children(state, QueueId::Output);
        let composite = queue::string(state, QueueId::Output, &queue_now, StringParent::None)?;
        state.tmp.suppress_decorations = false;
        let mut composite: Vec<Rendered> = composite.into_list();
        if composite.is_empty() && !js::truthy_opt(last_item.get("suppress-author")) {
            if pos == 0 {
                let pre =
                    txt_esc.escape(js::get_str(&state.citation.opt, "layout_prefix").unwrap_or(""));
                let suf = if pos == myblobs.len() - 1 {
                    txt_esc.escape(js::get_str(&state.citation.opt, "layout_suffix").unwrap_or(""))
                } else {
                    String::new()
                };
                composite.push(Rendered::Str(format!(
                    "{pre}[CSL STYLE ERROR: reference with no printed form.]{suf}"
                )));
            } else if pos == myblobs.len() - 1 {
                let esc =
                    txt_esc.escape(js::get_str(&state.citation.opt, "layout_suffix").unwrap_or(""));
                match objects.last_mut() {
                    Some(Rendered::Str(s)) => s.push_str(&esc),
                    Some(Rendered::Blob(b)) => {
                        let b = *b;
                        let cur = state.blobs.get(b).string("suffix");
                        state
                            .blobs
                            .get_mut(b)
                            .set_string("suffix", &format!("{cur}{esc}"));
                    }
                    _ => {}
                }
            }
        }
        // `var buffer = []` re-initialises the (function-scoped) variable on
        // every pass, so the `buffer.length` branches of the JS never run.
        let mut buffer: Vec<Rendered> = Vec::new();
        composite.reverse();
        if let Some(compie) = composite.pop() {
            buffer.push(compie);
        }
        let llen = composite.len();
        for ppos in 0..llen {
            let obj = composite.get(ppos).cloned();
            if let Some(Rendered::Str(s)) = obj {
                let sd = state.splice_delimiter.clone().unwrap_or_default();
                buffer.push(Rendered::Str(format!("{}{}", txt_esc.escape(&sd), s)));
                continue;
            }
            if let Some(compie) = composite.pop() {
                buffer.push(compie);
            }
        }
        if buffer.is_empty() && !js::truthy_opt(input_list[pos].1.get("suppress-author")) {
            empties += 1;
        }
        if buffer.len() > 1 && !matches!(buffer[0], Rendered::Str(_)) {
            let r = queue::render_blobs(state, QueueId::Output, buffer, "", false, None)?;
            buffer = vec![r];
        }
        if !buffer.is_empty() {
            let sd = state.splice_delimiter.clone().unwrap_or_default();
            match &mut buffer[0] {
                Rendered::Str(s) => {
                    if pos > 0 {
                        *s = format!("{}{}", txt_esc.escape(&sd), s);
                    }
                }
                Rendered::Blob(b) => {
                    let b = *b;
                    state.blobs.get_mut(b).splice_prefix =
                        Some(if pos > 0 { sd } else { String::new() });
                }
                Rendered::List(_) => {}
            }
        }
        objects.extend(buffer);
    }
    let _ = (empties, root_id(state));
    let rendered = queue::render_blobs(state, QueueId::Output, objects, "", false, None)?;
    let mut result: String = match rendered {
        Rendered::Str(s) => s,
        other => other.to_js_string(),
    };
    if !result.is_empty() && !state.tmp.suppress_decorations {
        let decs = state
            .citation
            .opt
            .get("layout_decorations")
            .and_then(queue::decorations_from_value)
            .unwrap_or_default();
        for d in decs {
            // The "normal" formats in some output modes expect a superior
            // nested decoration environment, and so should produce no output here.
            if d.value == "normal" {
                continue;
            }
            let any_author_only = js::truthy_opt(last_item.get("author-only"));
            if !any_author_only {
                result = crate::citeproc::formats::decorate(
                    state,
                    None,
                    &d.name,
                    &d.value,
                    Some(&result),
                    None,
                )?;
            }
        }
    }
    state.tmp.suppress_decorations = false;
    if result.is_empty() {
        result = "[NO_PRINTED_FORM]".to_string();
    }
    Ok(result)
}

/// Run one fixture through the driver.
pub(super) fn run_fixture(fx: &Fixture, locales: &Arc<BTreeMap<String, String>>) -> Outcome {
    let mut mode = fx.mode.split('-');
    let mode_name = mode.next().unwrap_or("");
    let submodes: Vec<&str> = mode.collect();
    if fx.has_citations {
        return Outcome::Skipped("CITATIONS".into());
    }
    if fx.has_bibsection {
        return Outcome::Skipped("BIBSECTION".into());
    }
    if submodes.contains(&"header") {
        return Outcome::Skipped("bibliography header".into());
    }
    // The registry's order: the items given to the last updateItems, in
    // their order (no bibliography sort, checked below).
    let ids: Vec<String> = match &fx.bibentries {
        Some(sets) => sets.last().cloned().unwrap_or_default(),
        None => fx
            .input
            .iter()
            .map(|i| i.get("id").map(js::to_js_string).unwrap_or_default())
            .collect(),
    };
    let ids: Vec<String> = {
        let mut seen = std::collections::BTreeSet::new();
        ids.into_iter().filter(|i| seen.insert(i.clone())).collect()
    };
    let (mut engine, _submodes) = match build_engine(fx, locales) {
        Ok(e) => e,
        Err(EngineError::BadInput(m)) if m.starts_with("item without an id") => {
            return Outcome::Skipped("DRIVER: item without an id".into())
        }
        Err(e) => return Outcome::Blocked(format!("build: {e}")),
    };
    let bad_driver = |e: EngineError| -> Outcome {
        match e {
            EngineError::BadInput(m) if m.starts_with("DRIVER:") => Outcome::Skipped(m),
            other => Outcome::Blocked(other.to_string()),
        }
    };
    let ret: String;
    if mode_name == "bibliography" {
        match make_bibliography(engine.state_mut(), &ids) {
            Ok(s) => ret = s,
            Err(e) => return bad_driver(e),
        }
    } else if mode_name == "citation" {
        let synthesized: Vec<Vec<Value>>;
        let sets: &Vec<Vec<Value>> = match &fx.citation_items {
            Some(s) => s,
            None => {
                if !engine.state().bibliography_sort.tokens.is_empty() {
                    return Outcome::Skipped(
                        "DRIVER: registry order needs a bibliography sort".into(),
                    );
                }
                synthesized = vec![ids
                    .iter()
                    .map(|id| serde_json::json!({ "id": id }))
                    .collect()];
                &synthesized
            }
        };
        let mut out: Vec<String> = Vec::new();
        for set in sets {
            match make_citation_cluster(engine.state_mut(), set) {
                Ok(s) => out.push(s),
                Err(e) => return bad_driver(e),
            }
        }
        ret = out.join("\n");
    } else {
        return Outcome::Skipped("unknown mode".into());
    }
    Outcome::Rendered(ret.replace("\r\n", "\n").replace('\r', "\n"))
}

/// Which closure of node_layout.js a token carries, if any.
enum LayoutKind {
    Prefix,
    CiteSuffix,
    BibSuffix,
}

/// The layout closure of the token at `idx`, `None` for any other token.
fn layout_closure(state: &State, idx: usize, bibliography: bool) -> Option<LayoutKind> {
    use crate::citeproc::exec::Exec;
    use crate::citeproc::node_layout::NodeLayoutExec;
    let tok = area_tokens(state, bibliography).get(idx)?;
    for e in &tok.execs {
        match e {
            Exec::NodeLayout(NodeLayoutExec::CitationPrefix) => return Some(LayoutKind::Prefix),
            Exec::NodeLayout(NodeLayoutExec::CitationSuffix) => {
                return Some(LayoutKind::CiteSuffix)
            }
            Exec::NodeLayout(NodeLayoutExec::BibliographySuffix) => {
                return Some(LayoutKind::BibSuffix)
            }
            _ => {}
        }
    }
    None
}

/// `state[area].tokens` for the citation or the bibliography.
fn area_tokens(state: &State, bibliography: bool) -> &Vec<Token> {
    if bibliography {
        &state.bibliography.tokens
    } else {
        &state.citation.tokens
    }
}

/// `makeBibliography()` (`CSL.getBibliographyEntries`) over the items `ids`
/// in the given order (the registry's order when the style has no
/// bibliography sort), joined as the runner does (`bibstart + entries +
/// bibend`).
pub(super) fn make_bibliography(state: &mut State, ids: &[String]) -> CslResult<String> {
    if state.bibliography.tokens.is_empty() {
        return Err(EngineError::BadInput("DRIVER: no bibliography".into()));
    }
    let Some(range) = layout_range(state, true) else {
        return Err(EngineError::BadInput("DRIVER: unsupported layout".into()));
    };
    if !state.bibliography_sort.tokens.is_empty() {
        return Err(EngineError::BadInput("DRIVER: sorted bibliography".into()));
    }
    if state.bibliography.opt.contains_key("exclude_types")
        || state.bibliography.opt.contains_key("exclude_with_fields")
    {
        return Err(EngineError::BadInput(
            "DRIVER: bibliography exclusions".into(),
        ));
    }
    state.tmp.area = "bibliography".to_string();
    state.tmp.root = "bibliography".to_string();
    let mut entries: Vec<String> = Vec::new();
    let layout_decorations = state
        .bibliography
        .opt
        .get("layout_decorations")
        .and_then(queue::decorations_from_value)
        .unwrap_or_default();
    for id in ids {
        let item_full = retrieve(state, id)?;
        let mut bib_entry = Token::new("group", TokenType::Start);
        bib_entry.decorations = vec![crate::citeproc::obj_token::Decoration::new(
            "@bibliography",
            "entry",
        )];
        bib_entry.decorations.extend(layout_decorations.clone());
        queue::start_tag(state, QueueId::Output, "bib_entry", Some(&bib_entry))?;
        if let Some(cur) = queue::current(state, QueueId::Output) {
            state.blobs.get_mut(cur).extra.insert(
                "system_id".into(),
                item_full.get("id").cloned().unwrap_or(Value::Null),
            );
        }
        state.tmp.term_predecessor = false;
        state.tmp.shadow_numbers = BTreeMap::new();
        get_cite(state, &item_full, &Value::Null, None, range, true)?;
        queue::end_tag(state, QueueId::Output, Some("bib_entry"))?;

        // place layout prefix on first blob of each cite
        let layout_prefix = js::get_str(&state.bibliography.opt, "layout_prefix")
            .unwrap_or("")
            .to_string();
        let root_kids = queue::queue_children(state, QueueId::Output);
        if let Some(BlobChild::Blob(entry)) = root_kids.first() {
            let entry_kids = blob_children(state, *entry);
            if let Some(BlobChild::Blob(level)) = entry_kids.first() {
                let level_kids = blob_children(state, *level);
                if !level_kids.is_empty() {
                    let target = match level_kids.first() {
                        Some(BlobChild::Blob(b)) => *b,
                        _ => *level,
                    };
                    let p = state.blobs.get(target).string("prefix");
                    state
                        .blobs
                        .get_mut(target)
                        .set_string("prefix", &format!("{layout_prefix}{p}"));
                }
            }
        }
        for c in queue::queue_children(state, QueueId::Output) {
            if let BlobChild::Blob(b) = c {
                queue::purge_empty_blobs(&mut state.blobs, b);
            }
        }
        let adjust = state.output.adjust.unwrap_or_default();
        for c in queue::queue_children(state, QueueId::Output) {
            if let BlobChild::Blob(b) = c {
                adjust.upward(&mut state.blobs, b);
                adjust.leftward(&mut state.blobs, b);
                adjust.downward(&mut state.blobs, b);
                let _ = adjust.fix(&mut state.blobs, b);
            }
        }
        let queue_now = queue::queue_children(state, QueueId::Output);
        let res = queue::string(state, QueueId::Output, &queue_now, StringParent::None)?;
        let first = match res.into_list().into_iter().next() {
            Some(Rendered::Str(s)) => s,
            Some(_) => return Err(EngineError::BadInput("DRIVER: numeric blob entry".into())),
            None => String::new(),
        };
        if !first.is_empty() {
            entries.push(first);
        } else if state.opt.get("update_mode").and_then(Value::as_i64)
            == Some(crate::citeproc::load::NUMERIC)
        {
            return Err(EngineError::BadInput("DRIVER: numeric empty entry".into()));
        }
    }
    state.tmp.area = "citation".to_string();
    state.tmp.root = "citation".to_string();
    Ok(format!(
        "{}{}{}",
        state.fun.decorate.bibstart(),
        entries.concat(),
        state.fun.decorate.bibend()
    ))
}

fn blob_children(state: &State, id: BlobId) -> Vec<BlobChild> {
    match &state.blobs.get(id).blobs {
        BlobContent::List(l) => l.clone(),
        BlobContent::Text(_) => Vec::new(),
    }
}

/// A case of `tests/data/csl/units/render.json` (generated by
/// `scripts/csl-units/render.cjs`) as a fixture.
pub(super) fn case_fixture(c: &Value) -> Fixture {
    let items = c["citation_items"].as_array().cloned().unwrap_or_default();
    Fixture {
        name: c["name"].as_str().unwrap_or("").to_string(),
        csl: c["csl"].as_str().unwrap_or("").to_string(),
        mode: "citation".to_string(),
        input: c["input"].as_array().cloned().unwrap_or_default(),
        abbreviations: c.get("abbreviations").cloned(),
        has_bibentries: false,
        has_bibsection: false,
        bibentries: None,
        has_citations: false,
        citation_items: Some(vec![items]),
        langparams: c.get("langparams").cloned(),
        multiaffix: c.get("multiaffix").cloned(),
        options: c.get("options").and_then(|o| o.as_object().cloned()),
        lang: c["lang"].as_str().unwrap_or("").to_string(),
        format: c["format"]
            .as_str()
            .filter(|f| *f != "html")
            .map(str::to_string),
        tags: c.get("tags").cloned(),
    }
}

/// Render a generated case both ways: `(citation, bibliography)` results.
pub(super) fn run_case(
    c: &Value,
    locales: &Arc<BTreeMap<String, String>>,
) -> (Result<String, String>, Result<String, String>) {
    let fx = case_fixture(c);
    let ids: Vec<String> = fx
        .input
        .iter()
        .map(|i| i.get("id").map(js::to_js_string).unwrap_or_default())
        .collect();
    let cite = (|| -> Result<String, String> {
        let (mut engine, _) = build_engine(&fx, locales).map_err(|e| e.to_string())?;
        apply_registry(&mut engine, c);
        let set = fx.citation_items.clone().unwrap_or_default();
        let set = set.first().cloned().unwrap_or_default();
        make_citation_cluster(engine.state_mut(), &set)
            .map(|s| s.replace("\r\n", "\n").replace('\r', "\n"))
            .map_err(|e| e.to_string())
    })();
    let bib = (|| -> Result<String, String> {
        let (mut engine, _) = build_engine(&fx, locales).map_err(|e| e.to_string())?;
        apply_registry(&mut engine, c);
        make_bibliography(engine.state_mut(), &ids)
            .map(|s| s.replace("\r\n", "\n").replace('\r', "\n"))
            .map_err(|e| e.to_string())
    })();
    (cite, bib)
}

/// Fill the real registry (`registry.registry[id].seq` and
/// `disambig.year_suffix`) from the case's `registry` (citeproc-js's values).
fn apply_registry(engine: &mut Engine, c: &Value) {
    if let Some(Value::Object(reg)) = c.get("registry") {
        for (id, v) in reg {
            let st = engine.state_mut();
            let mut cfg = crate::citeproc::obj_ambigconfig::AmbigConfig::default();
            cfg.year_suffix = v.get("year_suffix").cloned().unwrap_or(Value::Bool(false));
            st.registry.ambig_pool.push(cfg);
            let mut item = crate::citeproc::registry::RegistryItem::new(id);
            item.seq = v.get("seq").and_then(Value::as_i64).unwrap_or(0);
            item.disambig = Some(crate::citeproc::obj_ambigconfig::AmbigId(
                st.registry.ambig_pool.len() - 1,
            ));
            st.registry.registry.insert(id.clone(), item);
        }
    }
}
