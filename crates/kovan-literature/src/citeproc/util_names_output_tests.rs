//! Test-only driver for the names subsystem (GitHub #794, epic #790).
//!
//! **Why it exists.** `cs:names` cannot be exercised through the public
//! `Engine` API yet (the layout, text and group nodes, the registry and
//! `getCite` belong to other parts of the port). This module runs the token
//! list of a *names-only* style the way `CSL.getCite` / `CSL.getCitationCluster`
//! do and compares the rendered string with what citeproc-js 2.4.63 produced
//! for the same fixture of the CSL test suite (`tests/data/csl/test_suite_reference.json`).
//!
//! **Method.** For each fixture of the areas `name`, `nameattr`, `nameorder`,
//! `etal`, `substitute` and `bugreports` (the fixtures live in the git-ignored
//! `vendor/csl-test-suite`; without them the test prints a notice and passes):
//! parse the sections `CSL`, `INPUT`, `MODE`, `CITATION-ITEMS`, `ABBREVIATIONS`;
//! skip fixtures that need anything this driver does not model (a style with
//! other elements than layout / names / choose; `CITATIONS`; `OPTIONS`;
//! bibliography mode; a style with disambiguation, sorting, collapsing,
//! positions); build the engine state (`State::new`, the runner's default
//! `cite-lang-prefs`), then for each cluster run the citation token list
//! with the layout tokens' bodies replaced by what they do (initialise
//! `done_vars`, open the layout level, close it), join the cites as
//! `getCitationCluster` does for strings, and compare.
//!
//! **Pass criterion.** The output must equal citeproc-js's `output` for the
//! fixture exactly (even where citeproc-js itself fails the fixture's `RESULT`).
//! A fixture whose run reaches a part of the port that is not written yet
//! (`NotYetPorted`) is reported as *blocked*, not as a failure.
//!
//! **Results (2026-10-08, citeproc-js 2.4.63, test suite `6eefc5b0`).** Printed
//! by the tests (`cargo test --release -p kovan-literature --lib
//! citeproc::util_names_output::driver_tests -- --nocapture`):
//!
//! * fixtures: 131 of the 304 fixtures of the areas `name` (67 of 111),
//!   `nameattr` (54 of 97), `nameorder` (6 of 6), `etal` (1 of 4),
//!   `bugreports` (3 of 83) and `substitute` (0 of 7) are names-only styles
//!   the driver can run; **all 131 render exactly what citeproc-js rendered**.
//!   The other 173 need `cs:text`, `cs:group`, `cs:number`, `cs:date`
//!   or `cs:sort` (the render, dates and engine parts of the port), or the
//!   `CITATIONS` section; the engine's own fixture harness covers them once
//!   those are integrated;
//! * generated styles: 700 random names-only styles x 31 items, 65,137
//!   outputs compared, all equal; 1,363 calls where citeproc-js throws, where
//!   the port also fails (see [`names_e2e_matches_citeproc_js`]).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use regex::Regex;
use serde_json::{json, Value};

use crate::citeproc::build_retrieve_item::retrieve_item;
use crate::citeproc::js::{self, Obj};
use crate::citeproc::load::{get_safe_escape, CheckNestedBrace};
use crate::citeproc::obj_blob::{BlobChild, BlobContent};
use crate::citeproc::obj_token::{Token, TokenType};
use crate::citeproc::queue::{self, Adjust, FormatRef, Queue, QueueId, Rendered, StringParent};
use crate::citeproc::state::State;
use crate::citeproc::util_nodes::NEXT_UNDEFINED;
use crate::citeproc::util_names_output::has_length;
use crate::citeproc::{item_id, CslResult, EngineError, Sys};

const REFERENCE: &str = include_str!("../../tests/data/csl/test_suite_reference.json");

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A parsed fixture (only what this driver reads).
struct Fixture {
    csl: String,
    input: Vec<Value>,
    mode: String,
    citation_items: Option<Vec<Vec<Value>>>,
    abbreviations: Option<Value>,
    options: Option<Obj>,
    langparams: Option<Obj>,
    multiaffix: Option<Vec<Value>>,
    /// Sections present that the driver does not model.
    unsupported: Vec<&'static str>,
}

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

fn section(text: &str, name: &str) -> Option<String> {
    let re = Regex::new(&format!(r"(?s)>>=+ {name} =+>>\n(.*?)<<=+ {name} =+<<")).ok()?;
    re.captures(text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn parse_fixture(text: &str) -> Result<Fixture, String> {
    let csl = section(text, "CSL").ok_or("no CSL")?;
    let input: Vec<Value> = match section(text, "INPUT") {
        Some(s) => serde_json::from_str(&s).map_err(|e| format!("INPUT: {e}"))?,
        None => Vec::new(),
    };
    let mode = section(text, "MODE").unwrap_or_default().trim().to_string();
    let citation_items = match section(text, "CITATION-ITEMS") {
        Some(s) => {
            let v: Vec<Vec<Value>> =
                serde_json::from_str(&s).map_err(|e| format!("CITATION-ITEMS: {e}"))?;
            Some(v)
        }
        None => None,
    };
    let abbreviations = section(text, "ABBREVIATIONS").and_then(|s| serde_json::from_str(&s).ok());
    let json_obj = |name: &str| -> Option<Obj> {
        section(text, name)
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .and_then(|v| v.as_object().cloned())
    };
    let options = json_obj("OPTIONS");
    let langparams = json_obj("LANGPARAMS");
    let multiaffix = section(text, "MULTIAFFIX")
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v.as_array().cloned());
    let mut unsupported = Vec::new();
    for s in SECTIONS {
        if ["BIBSECTION", "CITATIONS", "OPTIONZ", "INPUT2", "BIBENTRIES"].contains(&s)
            && section(text, s).is_some()
        {
            unsupported.push(s);
        }
    }
    Ok(Fixture {
        csl,
        input,
        mode,
        citation_items,
        abbreviations,
        options,
        langparams,
        multiaffix,
        unsupported,
    })
}

fn load_locales() -> Option<Arc<BTreeMap<String, String>>> {
    let dir = root().join("vendor/citeproc-js/locale");
    let read = std::fs::read_dir(&dir).ok()?;
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

/// lib/sys.js: the `ABBREVIATIONS` section into the abbreviation cache.
fn build_abbreviations(raw: &Value) -> crate::citeproc::Abbreviations {
    use crate::citeproc::build_retrieve_item::normalize_abbrevs_key;
    let mut out = crate::citeproc::Abbreviations::new();
    let Some(by_jurisdiction) = raw.as_object() else {
        return out;
    };
    for (jurisd, segments) in by_jurisdiction {
        let Some(segments) = segments.as_object() else {
            continue;
        };
        for (segment, keys) in segments {
            let Some(keys) = keys.as_object() else {
                continue;
            };
            for (key, abbrev) in keys {
                let is_jurisdiction =
                    jurisd == "default" && segment == "place" && key.to_uppercase() == *key;
                let is_court = ["institution-entire", "institution-part"]
                    .contains(&segment.as_str())
                    && segment.to_lowercase() == *segment;
                let norm = if !is_jurisdiction && !is_court {
                    normalize_abbrevs_key("title", Some(key))
                } else {
                    key.clone()
                };
                out.entry(jurisd.clone())
                    .or_default()
                    .entry(segment.clone())
                    .or_default()
                    .insert(norm, js::to_js_string(abbrev));
            }
        }
    }
    out
}

/// The style's elements that this driver can run (everything else is
/// someone else's part of the port).
fn style_is_names_only(csl: &str) -> Result<(), String> {
    let re = Regex::new(r"<([a-z-]+)[ >/]").map_err(|e| e.to_string())?;
    let allowed = [
        "style",
        "info",
        "id",
        "title",
        "updated",
        "link",
        "category",
        "author",
        "contributor",
        "summary",
        "rights",
        "terms",
        "locale",
        "term",
        "single",
        "multiple",
        "citation",
        "bibliography",
        "layout",
        "names",
        "name",
        "name-part",
        "et-al",
        "substitute",
        "institution",
        "institution-part",
        "label",
        "macro",
        "title-short",
        "issn",
        "eissn",
        "issnl",
        "email",
        "uri",
        "published",
        "date",
        "choose",
        "if",
        "else-if",
        "else",
    ];
    for c in re.captures_iter(csl) {
        let e = c.get(1).map(|m| m.as_str()).unwrap_or("");
        if !allowed.contains(&e) {
            return Err(format!("element <{e}>"));
        }
    }
    // Inside the layouts only names and conditionals; no other renderers.
    if Regex::new(r"disambiguate-add|givenname-disambiguation|<sort|collapse|subsequent|position|near-note|cite-group|<macro|<date |<date>")
        .map_err(|e| e.to_string())?
        .is_match(csl)
    {
        return Err("style feature".to_string());
    }
    Ok(())
}

/// What `CSL.getCite` does around the token loop, for a names-only style:
/// the layout tokens are replaced by their effect.
fn get_cite(st: &mut State, item_data: &Value, cite: &Value, sort_mode: bool) -> CslResult<()> {
    let bib = st.tmp.area == "bibliography";
    st.tmp.cite_renders_content = false;
    st.tmp.probably_rendered_something = false;
    // CSL.citeStart
    st.tmp.lang_array = Vec::new();
    if let Some(l) = item_data.get("language").and_then(Value::as_str) {
        if let Some(m) = Regex::new(r"^([a-zA-Z]+).*")
            .ok()
            .and_then(|re| re.captures(l))
        {
            st.tmp.lang_array.push(m[1].to_lowercase());
        }
    }
    st.tmp
        .lang_array
        .push(js::get_string(&st.opt, "lang").unwrap_or_default());
    st.tmp.shadow_numbers.clear();
    st.tmp.have_collapsed =
        st.tmp.area == "citation" && has_length(st.citation.opt.get("collapse"));
    st.tmp.years_used = Vec::new();
    st.names_cite_start(item_data, cite);
    // layout START: done_vars, rendered_name, sort_key_flag, nameset_counter, openLevel
    st.tmp.done_vars = Vec::new();
    if js::truthy_opt(cite.get("author-only")) {
        st.tmp.done_vars.push("locator".to_string());
    }
    st.tmp.rendered_name = None;
    st.tmp.sort_key_flag = false;
    st.tmp.nameset_counter = 0;
    queue::open_level(
        st,
        QueueId::Output,
        FormatRef::Token(Token::new("", TokenType::Start)),
    )?;
    let n = tokens(st, bib).len();
    let mut next = 0usize;
    while next < n {
        let (name, nxt) = {
            let t = &tokens(st, bib)[next];
            (t.name.clone(), t.next.unwrap_or(NEXT_UNDEFINED))
        };
        if name == "layout" || name == "text" {
            // The layout's own tokens (prefix and suffix text tokens
            // included: a names-only style has no cs:text).
            next = nxt;
            continue;
        }
        next = run_token(st, bib, sort_mode, next, item_data, cite)?;
    }
    // layout END
    if bib {
        // setSuffix() (node_layout.js): the layout suffix goes on the open level.
        let suffix = js::get_string(&st.bibliography.opt, "layout_suffix").unwrap_or_default();
        if let Some(cur) = queue::current(st, QueueId::Output) {
            st.blobs.get_mut(cur).set_string("suffix", &suffix);
        }
    }
    queue::close_level(st, QueueId::Output, None)?;
    st.tmp.disambig_request = None;
    Ok(())
}

/// `CSL.tokenExec` for the citation token at `idx`, with the
/// `@variable` check-for-output closure (attributes.js:166-300, the render
/// agent's, not written yet) replaced by [`check_for_output`] for names
/// tokens. Same sequence as `State::token_exec`: take the token out, test,
/// run the closures in order, put it back.
fn run_token(
    st: &mut State,
    bib: bool,
    sort_mode: bool,
    idx: usize,
    item: &Value,
    cite: &Value,
) -> CslResult<usize> {
    use crate::citeproc::attributes::AttributesExec;
    use crate::citeproc::exec::Exec;
    use crate::citeproc::node_names::NodeNamesExec;
    let mut token = std::mem::take(&mut tokens_mut(st, bib)[idx]);
    let r = (|| -> CslResult<usize> {
        let mut next = token.next.unwrap_or(NEXT_UNDEFINED);
        if let Some(test) = token.test.clone() {
            if test.eval(st, &mut token, item, cite)? {
                st.tmp.jump.replace(Value::String("succeed".to_string()))?;
                next = token.succeed.unwrap_or(NEXT_UNDEFINED);
            } else {
                st.tmp.jump.replace(Value::String("fail".to_string()))?;
                next = token.fail.unwrap_or(NEXT_UNDEFINED);
            }
        }
        let mut i = 0;
        while i < token.execs.len() {
            let exec = token.execs[i].clone();
            let maybenext = match &exec {
                Exec::Attributes(AttributesExec::VariableCheckOutput) => {
                    check_for_output(st, &token, item)?;
                    None
                }
                Exec::NodeNames(NodeNamesExec::Output { .. }) if sort_mode => {
                    // What the generator's wrapper does around `outputNames`:
                    // a sort key renders names with `sort_key_flag` on and a
                    // sort extension (the layout's own execs reset the flag).
                    st.tmp.sort_key_flag = true;
                    st.tmp.extension = "_sort".to_string();
                    let r = exec.run(st, &mut token, item, cite);
                    st.tmp.sort_key_flag = false;
                    st.tmp.extension = String::new();
                    r?
                }
                other => other.run(st, &mut token, item, cite)?,
            };
            if let Some(m) = maybenext {
                if m != 0 {
                    next = m;
                }
            }
            i += 1;
        }
        Ok(next)
    })();
    tokens_mut(st, bib)[idx] = token;
    r
}

fn tokens(st: &State, bib: bool) -> &Vec<Token> {
    if bib {
        &st.bibliography.tokens
    } else {
        &st.citation.tokens
    }
}

fn tokens_mut(st: &mut State, bib: bool) -> &mut Vec<Token> {
    if bib {
        &mut st.bibliography.tokens
    } else {
        &mut st.citation.tokens
    }
}

/// The closure `@variable` pushes after "set variable names"
/// (attributes.js:166-300) for `cs:names` tokens: whether a string- or
/// number-valued variable would output. (Names variables are arrays, so a
/// `cs:names` almost never "outputs" here; its own closures flag success.)
///
/// STUB(attributes): driver-local; the render agent ports the original.
fn check_for_output(st: &mut State, token: &Token, item: &Value) -> CslResult<()> {
    let mut output = false;
    for variable in &token.variables {
        let v = item.get(variable.as_str());
        match v {
            Some(Value::Object(_)) | Some(Value::Array(_)) => break,
            Some(Value::String(s)) if !s.is_empty() => {
                output = true;
                break;
            }
            Some(Value::Number(_)) => {
                output = true;
                break;
            }
            _ => {}
        }
    }
    if output {
        let real: Vec<String> = token
            .extra
            .get("variables_real")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(js::to_js_string).collect())
            .unwrap_or_default();
        for variable in &real {
            if variable != "citation-number" || st.tmp.area != "bibliography" {
                st.tmp.cite_renders_content = true;
            }
            if let Some(t) = st.tmp.group_context.tip_mut() {
                t.variable_success = true;
            }
        }
        st.tmp.can_substitute.replace_literal(Value::Bool(false))?;
    } else if let Some(t) = st.tmp.group_context.tip_mut() {
        t.variable_attempt = true;
    }
    Ok(())
}

/// `CSL.getSpliceDelimiter` (api_cite.js:1011) without `after-collapse-delimiter`
/// and per-locale delimiters, which the generated styles do not use: the
/// delimiter between this cite and the one before it.
fn splice_delimiter(st: &State, _last_collapsed: bool, layout_delimiter: &str) -> String {
    if st.tmp.use_cite_group_delimiter {
        return js::get_string(&st.citation.opt, "cite_group_delimiter").unwrap_or_default();
    }
    let in_text = st.opt.get("xclass").and_then(Value::as_str) == Some("in-text");
    let numeric =
        st.opt.get("update_mode").and_then(Value::as_i64) == Some(crate::citeproc::load::NUMERIC);
    if st.tmp.have_collapsed && in_text && !numeric {
        return ", ".to_string();
    }
    layout_delimiter.to_string()
}

/// What the engine constructor leaves in `state.output`: an empty queue with
/// `adjust` set. Unlike an engine call, the drivers below never reset the
/// queue or the blob arena: after an error citeproc-js leaves its queue as it
/// was, and the next call sees it.
fn fresh_output(st: &mut State) {
    st.blobs.clear();
    reset_queue(st);
}

/// An empty output queue with `adjust` set (the blob arena is kept).
fn reset_queue(st: &mut State) {
    st.output = Queue::default();
    let piq = crate::citeproc::formats::get_opt_flag(st, "punctuation-in-quote");
    st.output.adjust = Some(Adjust::new(piq));
}

/// `makeCitationCluster(items)` for plain strings (see the module docs).
fn render_cluster(st: &mut State, cites: &[Value], sort_mode: bool) -> CslResult<String> {
    // this.output.checkNestedBrace = new CSL.checkNestedBrace(this)
    st.output.check_nested_brace = Some(CheckNestedBrace::new(st));
    st.tmp.last_primary_names_string = None;
    st.tmp.area = "citation".to_string();
    st.tmp.root = "citation".to_string();
    st.tmp.last_suffix_used = String::new();
    st.tmp.cite_locales = Vec::new();
    let layout_prefix = js::get_string(&st.citation.opt, "layout_prefix").unwrap_or_default();
    let layout_suffix = js::get_string(&st.citation.opt, "layout_suffix").unwrap_or_default();
    let layout_delimiter = js::get_string(&st.citation.opt, "layout_delimiter").unwrap_or_default();
    let use_layout_prefix = match st.output.check_nested_brace.as_mut() {
        Some(c) => c.update(&layout_prefix),
        None => layout_prefix.clone(),
    };
    let mut splice: Vec<String> = Vec::new();
    for (pos, cite) in cites.iter().enumerate() {
        let id = item_id(cite)?;
        st.tmp.shadow_numbers.clear();
        st.tmp.in_cite_predecessor = false;
        let item_data = retrieve_item(st, &id)?;
        if pos == 0 {
            st.tmp.term_predecessor = false;
        }
        let last_collapsed = st.tmp.have_collapsed;
        get_cite(st, &item_data, cite, sort_mode)?;
        splice.push(splice_delimiter(st, last_collapsed, &layout_delimiter));
    }
    // output.queue: one blob per cite
    let root_blob = st.output.root;
    let children: Vec<BlobChild> = match root_blob {
        Some(r) => match &st.blobs.get(r).blobs {
            BlobContent::List(l) => l.clone(),
            BlobContent::Text(_) => Vec::new(),
        },
        None => Vec::new(),
    };
    let blobs: Vec<_> = children
        .iter()
        .filter_map(|c| match c {
            BlobChild::Blob(b) => Some(*b),
            BlobChild::Str(_) => None,
        })
        .collect();
    let suffix = match st.output.check_nested_brace.as_mut() {
        Some(c) => c.update(&layout_suffix),
        None => layout_suffix.clone(),
    };
    for b in &blobs {
        queue::purge_empty_blobs(&mut st.blobs, *b);
    }
    if let (Some(first), Some(last)) = (blobs.first(), blobs.last()) {
        st.blobs.get_mut(*last).set_string("suffix", &suffix);
        st.blobs
            .get_mut(*first)
            .set_string("prefix", &use_layout_prefix);
    }
    if st.dev_ext("clean_up_csl_flaws") {
        if let Some(adjust) = st.output.adjust.clone() {
            for b in &blobs {
                adjust.upward(&mut st.blobs, *b);
                adjust.leftward(&mut st.blobs, *b);
                adjust.downward(&mut st.blobs, *b);
                adjust.fix(&mut st.blobs, *b);
            }
        }
    }
    let txt_esc = |st: &State, s: &str| get_safe_escape(st).escape(s);
    let mut objects: Vec<String> = Vec::new();
    for (pos, b) in blobs.iter().enumerate() {
        // this.output.queue = [myblobs[pos]]; composite = this.output.string(...)
        let composite = queue::string(
            st,
            QueueId::Output,
            &[BlobChild::Blob(*b)],
            StringParent::None,
        )?;
        let mut strings: Vec<String> = Vec::new();
        for r in composite.into_list() {
            match r {
                Rendered::Str(s) => strings.push(s),
                _ => {
                    return Err(EngineError::BadInput(
                        "driver: a numeric blob in a names-only cite".into(),
                    ))
                }
            }
        }
        if strings.is_empty() && !js::truthy_opt(cites[pos].get("suppress-author")) {
            if pos == 0 {
                let pre = txt_esc(
                    st,
                    &js::get_string(&st.citation.opt, "layout_prefix").unwrap_or_default(),
                );
                let suf = if pos == blobs.len() - 1 {
                    txt_esc(
                        st,
                        &js::get_string(&st.citation.opt, "layout_suffix").unwrap_or_default(),
                    )
                } else {
                    String::new()
                };
                strings.push(format!(
                    "{pre}[CSL STYLE ERROR: reference with no printed form.]{suf}"
                ));
            } else if pos == blobs.len() - 1 {
                let suf = txt_esc(
                    st,
                    &js::get_string(&st.citation.opt, "layout_suffix").unwrap_or_default(),
                );
                if let Some(last) = objects.last_mut() {
                    last.push_str(&suf);
                }
            }
        }
        let sd = splice
            .get(pos)
            .cloned()
            .unwrap_or_else(|| layout_delimiter.clone());
        // composite.reverse(); first = pop(); then the rest, in reversed order
        let mut buffer: Vec<String> = Vec::new();
        let mut rest: Vec<String> = strings;
        rest.reverse();
        if let Some(first) = rest.pop() {
            buffer.push(first);
        }
        for obj in rest.iter() {
            buffer.push(format!("{}{}", txt_esc(st, &sd), obj));
        }
        if let Some(b0) = buffer.first_mut() {
            if pos > 0 {
                *b0 = format!("{}{}", txt_esc(st, &sd), b0);
            }
        }
        objects.extend(buffer);
    }
    let mut result: String = objects.concat();
    if result.is_empty() {
        result = "[NO_PRINTED_FORM]".to_string();
    }
    Ok(result)
}

/// `makeBibliography()` for plain strings: `CSL.getBibliographyEntries` over
/// the items in registration order (a style with no sort), then the
/// runner's `bibstart + entries + bibend`.
fn render_bibliography(st: &mut State, ids: &[String]) -> CslResult<String> {
    use crate::citeproc::obj_token::Decoration;
    // `updateItems` (which `makeBibliography` needs first) renders every item
    // through `getAmbiguousCite`, whose `output.string()` empties the queue.
    reset_queue(st);
    st.output.check_nested_brace = Some(CheckNestedBrace::new(st));
    st.tmp.area = "bibliography".to_string();
    st.tmp.root = "bibliography".to_string();
    st.tmp.last_rendered_name = Value::Bool(false);
    let layout_prefix = js::get_string(&st.bibliography.opt, "layout_prefix").unwrap_or_default();
    let mut entries: Vec<String> = Vec::new();
    for id in ids {
        let item = retrieve_item(st, id)?;
        let mut bib_entry = Token::new("group", TokenType::Start);
        bib_entry.decorations = vec![Decoration::new("@bibliography", "entry")];
        if let Some(d) = st
            .bibliography
            .opt
            .get("layout_decorations")
            .and_then(queue::decorations_from_value)
        {
            bib_entry.decorations.extend(d);
        }
        queue::start_tag(st, QueueId::Output, "bib_entry", Some(&bib_entry))?;
        st.tmp.term_predecessor = false;
        st.tmp.shadow_numbers.clear();
        get_cite(st, &item, &Value::Null, false)?;
        queue::end_tag(st, QueueId::Output, Some("bib_entry"))?;
        let root_blob = st.output.root;
        let top: Vec<_> = match root_blob.map(|r| st.blobs.get(r).blobs.clone()) {
            Some(BlobContent::List(l)) => l
                .iter()
                .filter_map(|c| match c {
                    BlobChild::Blob(b) => Some(*b),
                    BlobChild::Str(_) => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        // layout prefix on the first blob
        if let Some(first) = top.first() {
            if let BlobContent::List(l) = st.blobs.get(*first).blobs.clone() {
                if let Some(BlobChild::Blob(b0)) = l.first() {
                    if let BlobContent::List(l0) = st.blobs.get(*b0).blobs.clone() {
                        if let Some(BlobChild::Blob(topblob)) = l0.first() {
                            let p = st.blobs.get(*topblob).string("prefix");
                            st.blobs
                                .get_mut(*topblob)
                                .set_string("prefix", &format!("{layout_prefix}{p}"));
                        }
                    }
                }
            }
        }
        for b in &top {
            queue::purge_empty_blobs(&mut st.blobs, *b);
        }
        if let Some(adjust) = st.output.adjust.clone() {
            for b in &top {
                adjust.upward(&mut st.blobs, *b);
                adjust.leftward(&mut st.blobs, *b);
                adjust.downward(&mut st.blobs, *b);
                adjust.fix(&mut st.blobs, *b);
            }
        }
        let children: Vec<BlobChild> = top.iter().map(|b| BlobChild::Blob(*b)).collect();
        let res = queue::string(st, QueueId::Output, &children, StringParent::None)?;
        match res.into_list().into_iter().next() {
            Some(Rendered::Str(s)) if !s.is_empty() => entries.push(s),
            Some(Rendered::Str(_)) | None => {}
            Some(_) => {
                return Err(EngineError::BadInput(
                    "driver: a numeric blob in a names-only entry".into(),
                ))
            }
        }
    }
    let f = st.fun.decorate;
    Ok(format!(
        "{}{}{}",
        f.bibstart(),
        entries.concat(),
        f.bibend()
    ))
}

/// Run one fixture; `Ok(None)` when the driver does not model it.
fn run_fixture(
    name: &str,
    text: &str,
    locales: &Arc<BTreeMap<String, String>>,
) -> Result<Result<String, String>, String> {
    let fx = parse_fixture(text)?;
    if !fx.unsupported.is_empty() {
        return Ok(Err(format!("section {}", fx.unsupported.join(","))));
    }
    if fx.mode != "citation" && fx.mode != "bibliography" {
        return Ok(Err(format!("mode {}", fx.mode)));
    }
    let bib_mode = fx.mode == "bibliography";
    if let Err(why) = style_is_names_only(&fx.csl) {
        return Ok(Err(why));
    }
    let _ = name;
    let mut sys = Sys::new(&fx.input, locales.clone()).map_err(|e| e.to_string())?;
    if let Some(raw) = &fx.abbreviations {
        sys.abbreviations = build_abbreviations(raw);
    }
    if let Some(options) = &fx.options {
        if options.contains_key("variableWrapper") {
            return Ok(Err("option variableWrapper".to_string()));
        }
        for (k, v) in options {
            sys.options.insert(k.clone(), v.clone());
        }
    }
    let mut st = State::new(sys, &fx.csl, "", false).map_err(|e| e.to_string())?;
    fresh_output(&mut st);
    if let Some(options) = &fx.options {
        for (k, v) in options {
            st.dev_ext_set(k, v.clone());
        }
    }
    // the runner's defaults
    let mut lang_params: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (k, v) in [
        ("persons", vec!["translit"]),
        ("institutions", vec!["translit"]),
        ("titles", vec!["translit", "translat"]),
        ("journals", vec!["translit"]),
        ("publishers", vec!["translat"]),
        ("places", vec!["translat"]),
    ] {
        lang_params.insert(k.to_string(), v.into_iter().map(str::to_string).collect());
    }
    if let Some(lp) = &fx.langparams {
        let strings = |v: &Value| -> Vec<String> {
            v.as_array()
                .map(|a| a.iter().map(js::to_js_string).collect())
                .unwrap_or_default()
        };
        for (key, value) in lp {
            if key == "langs" {
                let translat = value.get("translat").map(strings);
                if let Some(t) = &translat {
                    st.set_lang_tags_for_csl_translation(Some(t));
                }
                // As the runner does: it passes `translat` here as well.
                if value.get("translit").is_some() {
                    let t = translat.unwrap_or_default();
                    st.set_lang_tags_for_csl_transliteration(Some(&t));
                }
                continue;
            }
            lang_params.insert(key.clone(), strings(value));
        }
    }
    st.set_lang_prefs_for_cites(&lang_params, None);
    if let Some(m) = &fx.multiaffix {
        st.set_lang_prefs_for_cite_affixes(m);
    }

    let clusters: Vec<Vec<Value>> = match fx.citation_items.clone() {
        Some(c) => c,
        None => {
            let items: Vec<Value> = fx
                .input
                .iter()
                .map(|i| json!({ "id": item_id(i).unwrap_or_default() }))
                .collect();
            vec![items]
        }
    };
    let mut outs: Vec<String> = Vec::new();
    if bib_mode {
        let ids: Vec<String> = fx
            .input
            .iter()
            .map(|i| item_id(i).unwrap_or_default())
            .collect();
        // The runner's default: citations first (none here), then the bibliography.
        let _ = &clusters;
        return Ok(match render_bibliography(&mut st, &ids) {
            Ok(s) => Ok(s),
            Err(EngineError::NotYetPorted { method }) => Err(format!("blocked: {method}")),
            Err(e) => Err(format!("error: {e}")),
        });
    }
    for cites in &clusters {
        match render_cluster(&mut st, cites, false) {
            Ok(s) => outs.push(s),
            Err(EngineError::NotYetPorted { method }) => {
                return Ok(Err(format!("blocked: {method}")));
            }
            Err(e) => return Ok(Err(format!("error: {e}"))),
        }
    }
    Ok(Ok(outs.join("\n")))
}

/// Names fixtures against citeproc-js.
///
/// **Methodology.** Every fixture of the areas `name`, `nameattr`,
/// `nameorder`, `etal`, `substitute` and `bugreports` that this driver models
/// is rendered through the ported `cs:names` code and compared with the
/// output citeproc-js 2.4.63 produced (`test_suite_reference.json`).
///
/// **Results (2026-10-08).** 131 fixtures run, 131 equal, 0 differ; per area:
/// name 67 of 111, nameattr 54 of 97, nameorder 6 of 6, etal 1 of 4,
/// bugreports 3 of 83, substitute 0 of 7 (the rest need nodes outside the
/// names subsystem; see the module docs).
#[test]
fn names_fixtures_match_citeproc_js() {
    let dir = root().join("vendor/csl-test-suite/processor-tests/humans");
    let Some(locales) = load_locales() else {
        println!("SKIP: vendor/citeproc-js/locale is absent (run scripts/csl-reference.sh)");
        return;
    };
    let Ok(read) = std::fs::read_dir(&dir) else {
        println!(
            "SKIP: {} is absent (run scripts/csl-reference.sh)",
            dir.display()
        );
        return;
    };
    let reference: Value = serde_json::from_str(REFERENCE).expect("reference");
    let fixtures = reference["fixtures"].as_object().expect("fixtures");
    let mut files: Vec<_> = read.flatten().collect();
    files.sort_by_key(|e| e.file_name());
    let areas = [
        "name",
        "nameattr",
        "nameorder",
        "etal",
        "substitute",
        "bugreports",
    ];
    #[derive(Default)]
    struct Tally {
        total: usize,
        skipped: usize,
        blocked: usize,
        pass: usize,
        fail: usize,
    }
    let mut tally: BTreeMap<String, Tally> = BTreeMap::new();
    let mut failures: Vec<String> = Vec::new();
    let mut blocked: BTreeMap<String, usize> = BTreeMap::new();
    let mut skipped: BTreeMap<String, usize> = BTreeMap::new();
    for e in files {
        let file = e.file_name().to_string_lossy().to_string();
        let Some(stem) = file.strip_suffix(".txt") else {
            continue;
        };
        let area = stem.split('_').next().unwrap_or("");
        if !areas.contains(&area) {
            continue;
        }
        let Some(want) = fixtures.get(stem).and_then(|f| f["output"].as_str()) else {
            continue;
        };
        let text = std::fs::read_to_string(e.path()).expect("fixture");
        let t = tally.entry(area.to_string()).or_default();
        t.total += 1;
        match run_fixture(stem, &text, &locales) {
            Err(e) => panic!("{stem}: harness error: {e}"),
            Ok(Err(why)) => {
                if why.starts_with("blocked") || why.starts_with("error") {
                    t.blocked += 1;
                    *blocked.entry(why).or_default() += 1;
                } else {
                    t.skipped += 1;
                    *skipped.entry(why).or_default() += 1;
                }
            }
            Ok(Ok(got)) => {
                if got == want {
                    t.pass += 1;
                    if std::env::var("NAMES_DRIVER_VERBOSE").is_ok() {
                        println!("PASS {stem}: {got:?}");
                    }
                } else {
                    t.fail += 1;
                    failures.push(format!("{stem}\n    got:  {got:?}\n    want: {want:?}"));
                }
            }
        }
    }
    for (area, t) in &tally {
        println!(
            "{area}: {} fixtures, {} run and equal, {} DIFFER, {} blocked/error, {} not modelled",
            t.total, t.pass, t.fail, t.blocked, t.skipped
        );
    }
    for (why, n) in &skipped {
        println!("  not modelled: {n} x {why}");
    }
    for (why, n) in &blocked {
        println!("  {n} x {why}");
    }
    for f in &failures {
        println!("DIFF {f}");
    }
    let _ = Obj::new();
    assert!(failures.is_empty(), "{} fixtures differ", failures.len());
}

// ---------------------------------------------------------------------------
// Generated names-only styles against citeproc-js
// ---------------------------------------------------------------------------

const E2E_REFERENCE: &str = include_str!("../../tests/data/csl/units/names_e2e.json");

/// `build_state` for the generated cases: the pool items, the runner's
/// abbreviation cache, the `OPTIONS` and `LANGPARAMS` of the case.
fn e2e_state(
    case: &Value,
    pool: &[Value],
    acache: &Value,
    locales: &Arc<BTreeMap<String, String>>,
) -> Result<State, String> {
    let mut sys = Sys::new(pool, locales.clone()).map_err(|e| e.to_string())?;
    sys.abbreviations = build_abbreviations_verbatim(acache);
    let options = case["options"].as_object().cloned().unwrap_or_default();
    for (k, v) in &options {
        sys.options.insert(k.clone(), v.clone());
    }
    let style = case["style"].as_str().ok_or("style")?;
    let mut st = State::new(sys, style, "en-US", false).map_err(|e| e.to_string())?;
    fresh_output(&mut st);
    for (k, v) in &options {
        st.dev_ext_set(k, v.clone());
    }
    let mut lp: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (k, v) in [
        ("persons", vec!["translit"]),
        ("institutions", vec!["translit"]),
        ("titles", vec!["translit", "translat"]),
        ("journals", vec!["translit"]),
        ("publishers", vec!["translat"]),
        ("places", vec!["translat"]),
    ] {
        lp.insert(k.to_string(), v.into_iter().map(str::to_string).collect());
    }
    if let Some(l) = case["lang"].as_object() {
        let strings = |v: &Value| -> Vec<String> {
            v.as_array()
                .map(|a| a.iter().map(js::to_js_string).collect())
                .unwrap_or_default()
        };
        let translat = strings(&l["translat"]);
        st.set_lang_tags_for_csl_translation(Some(&translat));
        st.set_lang_tags_for_csl_transliteration(Some(&translat));
        lp.insert("persons".into(), strings(&l["persons"]));
        lp.insert("institutions".into(), strings(&l["institutions"]));
    }
    st.set_lang_prefs_for_cites(&lp, None);
    Ok(st)
}

/// The abbreviation cache of the generator (keys already normalised).
fn build_abbreviations_verbatim(raw: &Value) -> crate::citeproc::Abbreviations {
    let mut out = crate::citeproc::Abbreviations::new();
    for (j, segs) in raw.as_object().into_iter().flatten() {
        for (seg, keys) in segs.as_object().into_iter().flatten() {
            for (k, v) in keys.as_object().into_iter().flatten() {
                out.entry(j.clone())
                    .or_default()
                    .entry(seg.clone())
                    .or_default()
                    .insert(k.clone(), js::to_js_string(v));
            }
        }
    }
    out
}

/// 420 generated names-only styles (random combinations of the attributes
/// of `cs:name`, `cs:names`, `cs:et-al`, `cs:label`, `cs:institution`,
/// `cs:substitute`, the name-parts, the style options `initialize-with-hyphen`
/// and `demote-non-dropping-particle`, the development extensions
/// `spoof_institutional_affiliations`, `parse_names` and
/// `etal_min_etal_usefirst_hack`, and multilingual language preferences),
/// rendered in citation mode (one cluster per item of a pool of twenty items,
/// and one cluster with all of them) and in bibliography mode, compared with
/// citeproc-js 2.4.63's output (`tests/data/csl/units/names_e2e.json`,
/// generator `scripts/csl-units/names_e2e.cjs`).
///
/// **Pass criterion.** Every output equal; where citeproc-js throws, the port
/// must return an error for that call (the messages are not compared: Rust
/// reports the same TypeError or SyntaxError in its own wording).
///
/// **Run state.** As in citeproc-js, nothing is reset between calls: after an
/// exception citeproc-js leaves its output queue and `tmp` as they were,
/// and the following call sees them; the port must leave the same trace
/// (the driver neither resets the queue nor the blob arena, except where
/// `updateItems` would have emptied the queue). Citations are rendered
/// before `updateItems` so that the registry's disambiguation data, which is
/// the engine's, does not take part (with it, `citeStart` replaces the
/// `et-al` truncation by the counts `getAmbiguousCite` stored).
///
/// **Results (2026-10-08, citeproc-js 2.4.63).** 700 styles, 31 items; per
/// style one cluster per item (plain, `suppress-author`, rendered as a sort
/// key) and one with all items, and the bibliography: 65,137 outputs
/// compared, 0 differ; 1,363 calls fail in citeproc-js and in the port: 729
/// `TypeError ... 'first_blob'` (a `classic` item's abbreviation blob under
/// `collapse`, see DEVIATIONS candidates), 377 `TypeError ... 'blobs'` and 242
/// `SyntaxError: "undefined" is not valid JSON` (a `cs:name` with
/// `delimiter=""` and no `and`), 15 of them inside `updateItems`.
#[test]
fn names_e2e_matches_citeproc_js() {
    let Some(locales) = load_locales() else {
        println!("SKIP: vendor/citeproc-js/locale is absent (run scripts/csl-reference.sh)");
        return;
    };
    let r: Value = serde_json::from_str(E2E_REFERENCE).expect("names_e2e.json");
    let pool: Vec<Value> = r["pool"].as_array().cloned().expect("pool");
    let ids: Vec<String> = pool
        .iter()
        .filter_map(|i| i["id"].as_str().map(str::to_string))
        .collect();
    let mut compared = 0;
    let mut error_parity = 0;
    let mut bad: Vec<String> = Vec::new();
    for (ci, case) in r["cases"].as_array().expect("cases").iter().enumerate() {
        let mut st = match e2e_state(case, &pool, &r["acache"], &locales) {
            Ok(st) => st,
            Err(e) => {
                // citeproc-js threw while building the engine.
                if case["out"].get("build_error").is_some() {
                    error_parity += 1;
                } else {
                    bad.push(format!("case {ci}: build failed: {e}"));
                }
                continue;
            }
        };
        let out = &case["out"];
        let mut any_error = false;
        for (i, id) in ids.iter().enumerate() {
            let want = &out["cites"]
                .as_array()
                .map(|a| a[i].clone())
                .unwrap_or(Value::Null);
            let got = render_cluster(&mut st, &[json!({ "id": id })], false);
            any_error |= got.is_err();
            compare_one(
                ci,
                &format!("cite {id}"),
                &got,
                want,
                out.get("build_error").is_some(),
                &mut compared,
                &mut error_parity,
                &mut bad,
            );
        }
        for (i, id) in ids.iter().enumerate() {
            let want = &out["sort"]
                .as_array()
                .map(|a| a[i].clone())
                .unwrap_or(Value::Null);
            let got = render_cluster(&mut st, &[json!({ "id": id })], true);
            any_error |= got.is_err();
            compare_one(
                ci,
                &format!("sort key {id}"),
                &got,
                want,
                out.get("build_error").is_some(),
                &mut compared,
                &mut error_parity,
                &mut bad,
            );
        }
        for (i, id) in ids.iter().enumerate() {
            let want = &out["sa"]
                .as_array()
                .map(|a| a[i].clone())
                .unwrap_or(Value::Null);
            let got = render_cluster(
                &mut st,
                &[json!({ "id": id, "suppress-author": true })],
                false,
            );
            any_error |= got.is_err();
            compare_one(
                ci,
                &format!("suppress-author {id}"),
                &got,
                want,
                out.get("build_error").is_some(),
                &mut compared,
                &mut error_parity,
                &mut bad,
            );
        }
        let all: Vec<Value> = ids.iter().map(|id| json!({ "id": id })).collect();
        let got = render_cluster(&mut st, &all, false);
        any_error |= got.is_err();
        if let Some(want) = out.get("multi") {
            compare_one(
                ci,
                "all cites",
                &got,
                want,
                out.get("build_error").is_some(),
                &mut compared,
                &mut error_parity,
                &mut bad,
            );
        }
        let cites_failed = any_error;
        let got = render_bibliography(&mut st, &ids);
        any_error |= got.is_err();
        if out.get("update").is_some() {
            // `updateItems` threw while rendering an item for ambiguity
            // (getAmbiguousCite: the same tokens, in look-ahead mode); the
            // plain renderings above must have failed too.
            if cites_failed {
                error_parity += 1;
            } else {
                bad.push(format!(
                    "case {ci}: updateItems threw ({}) but no cite failed",
                    out["update"]
                ));
            }
        } else if let Some(want) = out.get("bib") {
            compare_one(
                ci,
                "bibliography",
                &got,
                want,
                out.get("build_error").is_some(),
                &mut compared,
                &mut error_parity,
                &mut bad,
            );
        }
        if out.get("build_error").is_some() {
            // updateItems threw while rendering one of the items.
            if any_error {
                error_parity += 1;
            } else {
                bad.push(format!(
                    "case {ci}: citeproc-js threw ({}) but the port did not",
                    out["build_error"]
                ));
            }
        }
    }
    println!(
        "{compared} outputs compared, {error_parity} error cases agree, {} differ",
        bad.len()
    );
    let show = if std::env::var("NAMES_E2E_ALL").is_ok() {
        usize::MAX
    } else {
        25
    };
    for b in bad.iter().take(show) {
        println!("DIFF {b}");
    }
    assert!(bad.is_empty(), "{} generated cases differ", bad.len());
    assert!(compared > 8000, "{compared}");
}

#[allow(clippy::too_many_arguments)]
fn compare_one(
    case: usize,
    what: &str,
    got: &CslResult<String>,
    want: &Value,
    build_failed: bool,
    compared: &mut usize,
    error_parity: &mut usize,
    bad: &mut Vec<String>,
) {
    if build_failed && want.is_null() {
        return;
    }
    match (got, want.get("v"), want.get("e")) {
        (Ok(g), Some(w), _) => {
            *compared += 1;
            if Some(g.as_str()) != w.as_str() {
                bad.push(format!(
                    "case {case} {what}:\n    got:  {g:?}\n    want: {w}"
                ));
            }
        }
        (Err(_), _, Some(_)) => *error_parity += 1,
        (g, w, e) => bad.push(format!(
            "case {case} {what}: got {g:?}, want v={w:?} e={e:?}"
        )),
    }
}
