// Part of the kovan port of citeproc-js (GitHub #790, #793).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      test-only driver; the behaviour under test is src/node_date.js,
//              src/node_datepart.js, src/util_dates.js and the dateput half of
//              src/queue.js
// Version:     2.4.63, commit 73bc1b44bc7d54d0bfec4e070fd27f5efe024ff9
// Copyright:   (c) 2009-2019 Frank Bennett
// Licence:     AGPL-3.0, taken from upstream's "CPAL-1.0 or AGPL-3.0-or-later"
//              (LICENSE at the commit above; see this crate's NOTICE).
// Modified:    2026-10-08, by the OUTRAM PARK contributors.
// No warranty: this program is distributed in the hope that it will be
//              useful, but WITHOUT ANY WARRANTY; without even the implied
//              warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
//              PURPOSE. See the GNU Affero General Public License.

//! End-to-end check of date rendering against citeproc-js (GitHub #793).
//!
//! **Methodology.** A test-only driver runs the token list of a citation the
//! way `CSL.getCite` does, with two differences: the `layout`/`citation`
//! tokens are skipped (their closures belong to node_layout.js, another
//! agent's file) and replaced by opening one `empty` level around the cite
//! (what the layout closure does), and the per-cite set-up of
//! `CSL.citeStart` is reduced to the `tmp` fields the date code reads. The
//! output queue is then finished as `makeCitationCluster` does
//! (`purgeEmptyBlobs`, `adjust.*`, `output.string`). The same driver, in
//! node, produced the reference (`scripts/csl-units/dates_e2e.cjs` →
//! `tests/data/csl/units/dates_e2e.json`): 79 styles (explicit date-parts,
//! every month/day/year form, affixes, text-case, strip-periods,
//! range-delimiter, `form="text"`/`"numeric"` with every `date-parts`,
//! `year-range-format` values) × 62 items (single dates, ranges sharing
//! day/month/year or nothing, open ranges, seasons, circa, BC/AD, literal,
//! raw strings, legal items for the collection-number case) × 5 locales
//! (en-US, de-DE, fr-FR, ja-JP, zh-CN). A cell where citeproc-js throws must
//! throw here too.
//!
//! **Pass criterion.** Every cell equals citeproc-js's (the JSON of the
//! array `output.string` returns), except cells listed in
//! [`KNOWN_DIFFERENCES`] with a reason.
//!
//! **Results.** See [`RESULTS`].

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;

use crate::citeproc::build_retrieve_item::retrieve_item;
use crate::citeproc::queue::{self, QueueId, Rendered, StringParent};
use crate::citeproc::util_nodes::TokenList;
use crate::citeproc::{Engine, Sys};

const REF: &str = include_str!("../../tests/data/csl/units/dates_e2e.json");

/// Measured results (filled in below, after the run).
const RESULTS: &str = "unmeasured";

/// Cells that differ from citeproc-js on purpose: (style, lang, item, reason).
const KNOWN_DIFFERENCES: &[(&str, &str, &str, &str)] = &[];

fn locale_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vendor/citeproc-js/locale")
}

/// The locale files, with the runner's clean-up (processing instructions dropped).
fn load_locales() -> Option<Arc<BTreeMap<String, String>>> {
    let read = std::fs::read_dir(locale_dir()).ok()?;
    let pi = regex::Regex::new(r"\s*<\?[^>]*\?>\s*\n").ok()?;
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

fn rendered_json(r: &Rendered) -> Value {
    match r {
        Rendered::Str(s) => Value::String(s.clone()),
        Rendered::Blob(_) => Value::String("[blob]".to_string()),
        Rendered::List(l) => Value::Array(l.iter().map(rendered_json).collect()),
    }
}

/// The driver: render the date(s) of item `id` the way `getCite` does.
fn render_one(engine: &mut Engine, id: &str) -> Result<Value, String> {
    let state = engine.state_mut();
    let item = retrieve_item(state, id).map_err(|e| e.to_string())?;
    state.tmp.area = "citation".to_string();
    state.tmp.root = "citation".to_string();
    state.tmp.years_used = Vec::new();
    state.tmp.done_vars = Vec::new();
    state.tmp.have_collapsed = false;
    state.tmp.has_done_year_suffix = false;
    state.tmp.cite_renders_content = false;
    state.tmp.probably_rendered_something = false;
    let n = state.citation.tokens.len();
    // The `@variable` check-for-output closure of the date token
    // (attributes.js, render agent's): it only maintains group-context flags,
    // which no output here depends on. Removed here and, in the reference
    // script, there.
    for t in state.citation.tokens.iter_mut() {
        t.execs.retain(|e| {
            !matches!(
                e,
                crate::citeproc::exec::Exec::Attributes(
                    crate::citeproc::attributes::AttributesExec::VariableCheckOutput
                )
            )
        });
    }
    // The layout closure opens a level for the cite; its tokens are skipped here.
    queue::open_level(state, QueueId::Output, "empty".into()).map_err(|e| e.to_string())?;
    let mut next = 0usize;
    while next < n {
        let t = &state.citation.tokens[next];
        // Only the date tokens and the helper tokens their `variable` attribute adds
        // (the ones that carry `variables_real`) are run; everything else (layout,
        // its prefix/suffix tokens) is node_layout.js's.
        let runs = t.name == "date"
            || t.name == "date-part"
            || (t.name == "text" && t.extra.contains_key("variables_real"));
        if !runs {
            next = t.next.unwrap_or(usize::MAX);
        } else {
            next = state
                .token_exec(&TokenList::Citation, next, &item, &Value::Null)
                .map_err(|e| e.to_string())?;
        }
    }
    queue::close_level(state, QueueId::Output, None).map_err(|e| e.to_string())?;
    let kids = queue::queue_children(state, QueueId::Output);
    for k in &kids {
        if let crate::citeproc::obj_blob::BlobChild::Blob(b) = k {
            queue::purge_empty_blobs(&mut state.blobs, *b);
        }
    }
    let clean = state
        .opt
        .get("development_extensions")
        .map(|d| crate::citeproc::js::truthy_opt(d.get("clean_up_csl_flaws")))
        .unwrap_or(false);
    if clean {
        let adjust = state.output.adjust.clone();
        if let Some(adj) = adjust {
            for k in queue::queue_children(state, QueueId::Output) {
                if let crate::citeproc::obj_blob::BlobChild::Blob(b) = k {
                    adj.upward(&mut state.blobs, b);
                    adj.leftward(&mut state.blobs, b);
                    adj.downward(&mut state.blobs, b);
                    adj.fix(&mut state.blobs, b);
                }
            }
        }
    }
    let kids = queue::queue_children(state, QueueId::Output);
    let r = queue::string(state, QueueId::Output, &kids, StringParent::None)
        .map_err(|e| e.to_string())?;
    Ok(rendered_json(&r))
}

#[test]
fn date_rendering_matches_citeproc_js() {
    let Some(locales) = load_locales() else {
        println!("SKIP: vendor/citeproc-js/locale is absent (run scripts/csl-reference.sh)");
        return;
    };
    let r: Value = serde_json::from_str(REF).expect("reference json");
    let styles = r["styles"].as_object().expect("styles");
    let items: Vec<Value> = r["items"].as_array().expect("items").clone();
    let langs: Vec<String> = r["langs"]
        .as_array()
        .expect("langs")
        .iter()
        .filter_map(|l| l.as_str().map(str::to_string))
        .collect();
    let results = r["results"].as_object().expect("results");
    let only = std::env::var("DATES_E2E_STYLE").ok();
    let (mut total, mut ok, mut thrown_ok) = (0usize, 0usize, 0usize);
    let mut bad: Vec<String> = Vec::new();
    for (sid, xml) in styles {
        if only.as_deref().is_some_and(|o| o != sid) {
            continue;
        }
        for lang in &langs {
            let want = &results[&format!("{sid}|{lang}")];
            let build = || -> Result<Engine, String> {
                let sys = Sys::new(&items, locales.clone()).map_err(|e| e.to_string())?;
                Engine::new(sys, xml.as_str().unwrap_or(""), lang).map_err(|e| e.to_string())
            };
            let mut engine = match build() {
                Ok(e) => e,
                Err(e) => {
                    total += 1;
                    if want.get("build_error").is_some() {
                        ok += 1;
                    } else {
                        bad.push(format!("{sid}|{lang}: build failed: {e}"));
                    }
                    continue;
                }
            };
            for it in &items {
                let id = it["id"].as_str().unwrap_or("");
                total += 1;
                let known = KNOWN_DIFFERENCES
                    .iter()
                    .any(|(s, l, i, _)| *s == sid && *l == lang && *i == id);
                let w = &want[id];
                let got = render_one(&mut engine, id);
                let same = match (&got, w.get("error")) {
                    (Ok(g), None) => g == w,
                    (Err(_), Some(_)) => {
                        thrown_ok += 1;
                        true
                    }
                    _ => false,
                };
                if got.is_err() {
                    if let Ok(e) = build() {
                        engine = e;
                    }
                }
                if same || known {
                    ok += 1;
                } else {
                    bad.push(format!("{sid}|{lang}|{id}: got {got:?} want {w}"));
                }
            }
        }
    }
    println!(
        "dates_e2e: {ok}/{total} cells equal citeproc-js ({thrown_ok} where both throw); {RESULTS}"
    );
    for b in bad.iter().take(60) {
        println!("MISMATCH {b}");
    }
    assert!(bad.is_empty(), "{} mismatching cells", bad.len());
}

// ------------------------------------------------------------------------
// Fixtures of the CSL test suite whose citation needs nothing but dates.
// ------------------------------------------------------------------------

const SUITE_REF: &str = include_str!("../../tests/data/csl/test_suite_reference.json");

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

/// The runner's line-by-line section reader (see tests/citeproc_test_suite.rs
/// for the quirks it keeps).
fn read_sections(text: &str) -> Option<BTreeMap<String, Vec<String>>> {
    let names = SECTIONS.join("|");
    let open = regex::Regex::new(&format!(r"^.*>>===*\s({names})\s.*=>>.*")).ok()?;
    let close = regex::Regex::new(&format!(r"^.*<<===*\s({names})\s.*=<<.*")).ok()?;
    let mut obj: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut section = String::new();
    let mut state = "";
    for line in text.replace("\r\n", "\n").split('\n') {
        if let Some(m) = open.captures(line) {
            if !state.is_empty() {
                return None;
            }
            section = m[1].to_string();
            state = "opening";
        } else if let Some(m) = close.captures(line) {
            if section != m[1] {
                return None;
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
    Some(obj)
}

fn flatten(r: &Rendered) -> String {
    match r {
        Rendered::Str(s) => s.clone(),
        Rendered::Blob(_) => String::new(),
        Rendered::List(l) => l.iter().map(flatten).collect(),
    }
}

/// [`render_one`] without the JSON step: the cite as one string.
fn render_cite_string(engine: &mut Engine, id: &str) -> Result<String, String> {
    let v = render_one(engine, id)?;
    fn text(v: &Value) -> String {
        match v {
            Value::String(s) => s.clone(),
            Value::Array(a) => a.iter().map(text).collect(),
            _ => String::new(),
        }
    }
    Ok(text(&v))
}

/// Area counters of the fixture sweep: (eligible, equal to citeproc-js's
/// output, equal to the fixture's RESULT).
type AreaCounts = BTreeMap<String, (usize, usize, usize)>;

#[test]
fn date_only_fixtures_render_like_citeproc_js() {
    let Some(locales) = load_locales() else {
        println!("SKIP: vendor/citeproc-js/locale is absent (run scripts/csl-reference.sh)");
        return;
    };
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../vendor/csl-test-suite/processor-tests/humans");
    let Ok(read) = std::fs::read_dir(&dir) else {
        println!("SKIP: {} is absent", dir.display());
        return;
    };
    let suite: Value = serde_json::from_str(SUITE_REF).expect("suite reference");
    let mut files: Vec<(String, PathBuf)> = read
        .flatten()
        .filter_map(|e| {
            let f = e.file_name().to_string_lossy().to_string();
            f.strip_suffix(".txt").map(|s| (s.to_string(), e.path()))
        })
        .collect();
    files.sort();
    let only = std::env::var("DATES_E2E_FIXTURE").ok();
    let mut counts: AreaCounts = BTreeMap::new();
    let mut skipped: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut bad: Vec<String> = Vec::new();
    let mut matched_names: Vec<String> = Vec::new();
    for (name, path) in files {
        if only.as_deref().is_some_and(|o| o != name) {
            continue;
        }
        let area = name.split('_').next().unwrap_or("").to_string();
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Some(sec) = read_sections(&text) else {
            continue;
        };
        let get = |k: &str| sec.get(k).map(|l| l.join("\n"));
        let Some(csl) = get("CSL") else { continue };
        let mode = get("MODE").unwrap_or_default();
        if mode != "citation" {
            *skipped.entry("mode").or_default() += 1;
            continue;
        }
        if sec.contains_key("CITATIONS") || sec.contains_key("INPUT2") || sec.contains_key("OPTIONS")
        {
            *skipped.entry("citations/options").or_default() += 1;
            continue;
        }
        let Some(Value::Array(input)) = get("INPUT").and_then(|t| serde_json::from_str(&t).ok())
        else {
            continue;
        };
        let clusters: Vec<Vec<String>> = match get("CITATION-ITEMS") {
            None => vec![input
                .iter()
                .filter_map(|i| i.get("id").and_then(Value::as_str).map(str::to_string))
                .collect()],
            Some(t) => {
                let Some(Value::Array(sets)) = serde_json::from_str::<Value>(&t).ok() else {
                    continue;
                };
                let mut out = Vec::new();
                let mut plain = true;
                for set in &sets {
                    let mut ids = Vec::new();
                    for c in set.as_array().into_iter().flatten() {
                        let keys_ok = c.as_object().is_some_and(|o| o.len() == 1);
                        match (c.get("id").and_then(Value::as_str), keys_ok) {
                            (Some(id), true) => ids.push(id.to_string()),
                            _ => plain = false,
                        }
                    }
                    out.push(ids);
                }
                if !plain {
                    *skipped.entry("cite properties").or_default() += 1;
                    continue;
                }
                out
            }
        };
        let items: Vec<Value> = input
            .iter()
            .filter(|i| i.get("id").is_some())
            .cloned()
            .collect();
        let Ok(sys) = Sys::new(&items, locales.clone()) else {
            continue;
        };
        let Ok(mut engine) = Engine::new(sys, &csl, "") else {
            *skipped.entry("build error").or_default() += 1;
            continue;
        };
        engine.add_date_parser_months(
            &[
                "ocak", "Şubat", "mart", "nisan", "mayıs", "haziran", "temmuz", "ağustos",
                "eylül", "ekim", "kasım", "aralık", "bahar", "yaz", "sonbahar", "kış",
            ]
            .map(str::to_string),
        );
        // Eligible: nothing in the citation but dates (and the helper tokens
        // of their `variable` attribute), no sort, no collapsing.
        let st = engine.state();
        let only_dates = st.citation.tokens.iter().all(|t| {
            matches!(t.name.as_str(), "citation" | "layout" | "date" | "date-part")
                || (t.name == "text"
                    && !t
                        .execs
                        .iter()
                        .any(|e| matches!(e, crate::citeproc::exec::Exec::NodeText(_))))
        });
        let has_date = st.citation.tokens.iter().any(|t| t.name == "date");
        let collapse = st
            .citation
            .opt
            .get("collapse")
            .map(|c| crate::citeproc::js::truthy(c) && c.as_array().map(|a| !a.is_empty()).unwrap_or(true))
            .unwrap_or(false);
        if !only_dates || !has_date {
            *skipped.entry("style needs other nodes").or_default() += 1;
            continue;
        }
        if !st.citation_sort.tokens.is_empty() || collapse {
            *skipped.entry("sort/collapse").or_default() += 1;
            continue;
        }
        let opt = |k: &str| {
            st.citation
                .opt
                .get(k)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        };
        let (delim, prefix, suffix) = (
            opt("layout_delimiter"),
            opt("layout_prefix"),
            opt("layout_suffix"),
        );
        // The registry has already run every item's cite (getAmbiguousCite)
        // before the first cluster is made; for dates that matters because the
        // first run leaves `dateput` with the build-time date level and later
        // ones with `tmp.date_token`, which has no affixes.
        for it in &items {
            if let Some(id) = it.get("id").and_then(Value::as_str) {
                let _ = render_cite_string(&mut engine, id);
            }
        }
        let mut outs: Vec<String> = Vec::new();
        let mut failed: Option<String> = None;
        for ids in &clusters {
            let mut cites = Vec::new();
            for id in ids {
                match render_cite_string(&mut engine, id) {
                    Ok(s) => cites.push(s),
                    Err(e) => {
                        failed = Some(e);
                        break;
                    }
                }
            }
            if failed.is_some() {
                break;
            }
            let body = cites.join(&delim);
            outs.push(if body.is_empty() {
                "[CSL STYLE ERROR: reference with no printed form.]".to_string()
            } else {
                format!("{prefix}{body}{suffix}")
            });
        }
        let got = outs.join("\n");
        let entry = counts.entry(area.clone()).or_default();
        entry.0 += 1;
        let reference = &suite["fixtures"][&name];
        let want_out = reference["output"].as_str().unwrap_or("");
        let want_res = reference["expected"].as_str().unwrap_or("");
        let same_out = failed.is_none() && got == want_out;
        if same_out {
            entry.1 += 1;
        }
        if failed.is_none() && got == want_res {
            entry.2 += 1;
        }
        if same_out {
            matched_names.push(name.clone());
        } else {
            bad.push(format!(
                "{name}: got {got:?} (err {failed:?}) citeproc-js {want_out:?} RESULT {want_res:?}"
            ));
        }
    }
    println!("date_fixtures: skipped {skipped:?}");
    let (mut e, mut a, mut b) = (0, 0, 0);
    for (area, (el, eq_out, eq_res)) in &counts {
        println!("date_fixtures: area {area}: eligible {el}, equal to citeproc-js {eq_out}, equal to RESULT {eq_res}");
        e += el;
        a += eq_out;
        b += eq_res;
    }
    println!("date_fixtures: TOTAL eligible {e}, equal to citeproc-js {a}, equal to RESULT {b}");
    for m in &bad {
        println!("FIXTURE-MISMATCH {m}");
    }
    assert!(bad.is_empty(), "{} fixtures differ from citeproc-js", bad.len());
}
