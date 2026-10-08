// Part of the kovan port of citeproc-js (GitHub #790, #792).
//
// Upstream:    juris-m/citeproc-test-runner 1.1.116 (commit b1e72d5c, MIT),
//              lib/fixture-parser.js, lib/sys.js: the runner whose set-up this
//              file replicates (as scripts/csl-intermediate-reference.cjs
//              does), and citeproc-js, https://github.com/juris-m/citeproc-js
//              (2.4.63, commit 73bc1b44bc7d54d0bfec4e070fd27f5efe024ff9)
// Copyright:   (c) Frank Bennett (citeproc-js 2009-2019; the runner)
// Licence:     AGPL-3.0 (citeproc-js: "CPAL-1.0 or AGPL-3.0-or-later"); the
//              runner is MIT, compatible. See this crate's NOTICE.
// Modified:    2026-10-08, by the OUTRAM PARK contributors: a Rust
//              translation of the runner's parsing logic.
// No warranty: distributed in the hope that it will be useful, but WITHOUT
//              ANY WARRANTY; without even the implied warranty of
//              MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.

//! The citeproc-js port's **build stage** against citeproc-js's intermediate
//! state (GitHub #792, epic #790): `style`, `locale`, `items`, `names`,
//! `numbers` and `citation_items` of the dump, plus the parsed style (`xml`).
//!
//! **Methodology.** `scripts/csl-intermediate-reference.cjs` built an
//! `Engine` for each of the 845 fixtures of the CSL test suite (set up exactly
//! as the test runner does before its first `updateItems`) and for five
//! styles over `tests/data/csl/items.json` (the "site set"), dumped what the
//! engine holds, in a canonical JSON form, and committed the SHA-256 of every
//! section in `tests/data/csl/intermediate_reference.json`. This file builds
//! the same 850 engines with [`kovan_literature::citeproc::Engine`], dumps
//! them with [`kovan_literature::citeproc::dump`] (the same canonical form,
//! see that module) and compares digests.
//!
//! Sections compared per case:
//!
//! * **`locale`**: `engine.locale` (terms, opts, dates, ordinals, noun
//!   genders) for every language loaded, and `opt.gender`.
//! * **`style`** (full): `opt`, the five areas' `opt`/`root`/tokens, the
//!   macros, `tmp.cite_affixes`, `build.names_level`, with each token's
//!   closure counts (`execs_n`, `tests_n`, `has_test`), in the reference
//!   script's property order (see `dump::token_text`).
//! * **`style_reduced`**: the same without the closure counts, against
//!   `tests/data/csl/intermediate_reference_reduced.json`
//!   (`scripts/csl-intermediate-reduced.cjs`).
//! * **`xml`**: the parsed style (`engine.cslXml`) after the constructor's
//!   normalisations, against `intermediate_reference_xml.json`.
//! * **`items`**, **`names`**, **`numbers`**, **`citation_items`**: the
//!   input side (`retrieveItem`, the name parser's input half,
//!   `processNumber(false, ...)`, the `parseLocator` steps of
//!   `makeCitationCluster`), computed on the same engine by
//!   `dump::input_sections` over the fixture's INPUT and citation lists (with
//!   the runner's ABBREVIATIONS and Turkish months applied).
//!
//! **Pass criterion.** Every case whose digest differs from citeproc-js's must
//! be listed, with a reason, in `tests/data/csl/intermediate_known_differences.json`
//! (groups of `{section, reason, cases}`); any other difference fails, and so
//! does a listed one that no longer occurs. A listed difference is a recorded
//! finding, never a tolerance. Where `vendor/` is absent the tests print a
//! reason and skip.
//!
//! **To diff a failing case**, run with `INTERMEDIATE_DUMP=<case>[:section]`
//! (section `style`, `style_reduced`, `locale`, `xml`, `items`, `names`,
//! `numbers`, `citation_items`; default `style`; a site case is `site:<style>`)
//! to build only that case and print the port's section as JSON, and compare
//! with `node scripts/csl-intermediate-reference.cjs --case <case> --section style`.
//! Add `INTERMEDIATE_RAW=1` to print the exact digested text between
//! `RAW>>` and `<<RAW` (compare `... --raw`). `INTERMEDIATE_PRINT_DIFFERENCES=1`
//! prints the list of differing cases per section in the known-differences
//! file's `cases` format.
//!
//! **Results (2026-10-08, wave 1 integrated, branch citeproc/integ1).** All
//! eight sections equal citeproc-js's for all 850 cases (845 fixtures + 5
//! site styles); no known differences. See `RESULTS_NOTE`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use kovan_literature::citeproc::dump;
use kovan_literature::citeproc::{Engine, OutputFormat, Sys};
use regex::Regex;
use serde_json::{Map, Value};

const DIGESTS: &str = include_str!("data/csl/intermediate_reference.json");
const REDUCED: &str = include_str!("data/csl/intermediate_reference_reduced.json");
const XML: &str = include_str!("data/csl/intermediate_reference_xml.json");
const KNOWN: &str = include_str!("data/csl/intermediate_known_differences.json");

/// What this branch measured (filled in by the author; printed by the test).
const RESULTS_NOTE: &str = "measured 2026-10-08 on branch citeproc/integ1 (wave 1 integrated): locale 850/850, xml 850/850, style 850/850, style_reduced 850/850, items 850/850, names 850/850, numbers 850/850, citation_items 850/850";

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture_dir() -> PathBuf {
    workspace_root().join("vendor/csl-test-suite/processor-tests/humans")
}

fn locale_dir() -> PathBuf {
    workspace_root().join("vendor/citeproc-js/locale")
}

fn site_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/csl")
}

fn skip(why: &str) {
    println!("SKIP: {why} (run scripts/csl-reference.sh to fetch vendor/)");
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

/// The parts of a fixture the build stage needs.
#[derive(Debug, Clone)]
struct Fixture {
    csl: String,
    mode: String,
    input: Vec<Value>,
    langparams: Option<Value>,
    multiaffix: Option<Value>,
    options: Option<Map<String, Value>>,
    abbreviations: Option<Value>,
    citation_items: Option<Value>,
    citations: Option<Value>,
}

/// The runner's line-by-line section reader (see tests/citeproc_test_suite.rs
/// for the quirks kept).
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
                return Err(format!("attempted to open tag \"{}\" early", &m[1]));
            }
            section = m[1].to_string();
            state = "opening";
        } else if let Some(m) = close.captures(line) {
            if section != m[1] {
                return Err(format!(
                    "expected closing tag \"{section}\" but found \"{}\"",
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

fn parse_fixture_text(name: &str, text: &str) -> Result<Fixture, String> {
    let sections = read_sections(text)?;
    let text_of = |key: &str| sections.get(key).map(|l| l.join("\n"));
    let json_of = |key: &str| -> Result<Option<Value>, String> {
        match text_of(key) {
            Some(t) => serde_json::from_str(&t)
                .map(Some)
                .map_err(|e| format!("JSON parse fail for tag \"{key}\" in {name}: {e}")),
            None => Ok(None),
        }
    };
    let options = match json_of("OPTIONS")? {
        Some(Value::Object(m)) => Some(m),
        Some(other) => return Err(format!("OPTIONS in {name} is not an object: {other}")),
        None => None,
    };
    let input = match json_of("INPUT")? {
        Some(Value::Array(a)) => a,
        _ => return Err(format!("INPUT of {name} is not an array")),
    };
    Ok(Fixture {
        csl: text_of("CSL").ok_or_else(|| format!("missing CSL in {name}"))?,
        mode: text_of("MODE").ok_or_else(|| format!("missing MODE in {name}"))?,
        input,
        langparams: json_of("LANGPARAMS")?,
        multiaffix: json_of("MULTIAFFIX")?,
        options,
        abbreviations: json_of("ABBREVIATIONS")?,
        citation_items: json_of("CITATION-ITEMS")?,
        citations: json_of("CITATIONS")?,
    })
}

fn load_fixtures() -> Option<BTreeMap<String, Fixture>> {
    let dir = fixture_dir();
    let Ok(read) = std::fs::read_dir(&dir) else {
        skip(&format!("{} is absent", dir.display()));
        return None;
    };
    let mut out = BTreeMap::new();
    let name_re = Regex::new(r"^[a-z]+_").expect("static");
    for entry in read.flatten() {
        let file = entry.file_name().to_string_lossy().to_string();
        let Some(stem) = file.strip_suffix(".txt") else {
            continue;
        };
        if !name_re.is_match(stem) {
            continue;
        }
        let text = std::fs::read_to_string(entry.path()).unwrap_or_else(|e| panic!("{file}: {e}"));
        let fx = parse_fixture_text(stem, &text).unwrap_or_else(|e| panic!("{file}: {e}"));
        out.insert(stem.to_string(), fx);
    }
    Some(out)
}

/// Every `locales-<lang>.xml` of `vendor/citeproc-js/locale` with the
/// runner's `retrieveLocale` clean-up (processing instructions dropped).
fn load_locales() -> Option<Arc<BTreeMap<String, String>>> {
    let dir = locale_dir();
    let Ok(read) = std::fs::read_dir(&dir) else {
        skip(&format!("{} is absent", dir.display()));
        return None;
    };
    let pi = Regex::new(r"\s*<\?[^>]*\?>\s*\n").expect("static");
    let mut map = BTreeMap::new();
    for entry in read.flatten() {
        let file = entry.file_name().to_string_lossy().to_string();
        let Some(lang) = file
            .strip_prefix("locales-")
            .and_then(|f| f.strip_suffix(".xml"))
        else {
            continue;
        };
        let xml = std::fs::read_to_string(entry.path()).unwrap_or_else(|e| panic!("{file}: {e}"));
        map.insert(lang.to_string(), pi.replace_all(&xml, "").to_string());
    }
    if map.is_empty() {
        skip(&format!("{} holds no locales", dir.display()));
        return None;
    }
    Some(Arc::new(map))
}

// ------------------------------------------------------------------ engines

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .map(|x| x.as_str().unwrap_or_default().to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// `new CSL.Engine(sys, test.CSL)` and the runner's configuration before the
/// first `updateItems` (`configureLikeRunner` of the reference script).
fn fixture_engine(fx: &Fixture, locales: &Arc<BTreeMap<String, String>>) -> Result<Engine, String> {
    // Some fixtures carry items without an id; the reference runner keys them
    // under `undefined`, and the build never reads items.
    let items: Vec<Value> = fx
        .input
        .iter()
        .filter(|i| i.get("id").is_some())
        .cloned()
        .collect();
    let mut sys = Sys::new(&items, locales.clone()).map_err(|e| e.to_string())?;
    // The runner's cache is keyed by `item.id`: an item without one lands under
    // the key `"undefined"` (the last such item wins), which `retrieveItem("undefined")` finds.
    if let Some(anon) = fx.input.iter().rev().find(|i| i.get("id").is_none()) {
        let mut map = (*sys.items).clone();
        map.insert("undefined".to_string(), anon.clone());
        sys.items = Arc::new(map);
    }
    // `this[option] = this.test.OPTIONS[option]` on the runner's sys.
    if let Some(options) = &fx.options {
        for (k, v) in options {
            sys.options.insert(k.clone(), v.clone());
        }
    }
    let mut engine = Engine::new(sys, &fx.csl, "").map_err(|e| e.to_string())?;
    engine.add_date_parser_months(&TURKISH_MONTHS.map(str::to_string));
    let mode: Vec<&str> = if fx.mode.is_empty() {
        vec!["all"]
    } else {
        fx.mode.split('-').collect()
    };
    let submodes: BTreeSet<&str> = mode.iter().skip(1).copied().collect();
    for fmt in ["rtf", "plain", "asciidoc", "xslfo"] {
        if submodes.contains(fmt) {
            if let Some(f) = OutputFormat::from_name(fmt) {
                engine.set_output_format(f);
            }
        }
    }
    if submodes.contains("suppress_trailing_punctuation") {
        engine.set_suppress_trailing_punctuation(true);
    }
    if let Some(options) = &fx.options {
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
            if key == "langs" {
                if let Some(t) = value.get("translat").filter(|t| !t.is_null()) {
                    engine.set_lang_tags_for_csl_translation(strings(t));
                }
                // The reference runner passes `langsToUse.translat` here too.
                if value.get("translit").is_some_and(|t| !t.is_null()) {
                    engine.set_lang_tags_for_csl_transliteration(strings(
                        value.get("translat").unwrap_or(&Value::Null),
                    ));
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
    if let Some(a) = &fx.abbreviations {
        engine.set_runner_abbreviations(a);
    }
    Ok(engine)
}

/// The runner's Turkish month names (`addDateParserMonths` before the first `updateItems`).
const TURKISH_MONTHS: [&str; 16] = [
    "ocak", "Şubat", "mart", "nisan", "mayıs", "haziran", "temmuz", "ağustos", "eylül", "ekim",
    "kasım", "aralık", "bahar", "yaz", "sonbahar", "kış",
];

/// The site styles of the reference script: (name, file, locale).
const SITE_STYLES: [(&str, &str, &str); 5] = [
    ("apa", "apa.csl", "en-US"),
    ("chicago-author-date", "chicago-author-date.csl", "en-US"),
    ("ieee", "ieee.csl", "en-US"),
    ("nature", "nature.csl", "en-GB"),
    ("vancouver", "nlm-citation-sequence.csl", "en-US"),
];

fn site_engine(file: &str, lang: &str) -> Result<Engine, String> {
    let dir = site_dir();
    let items: Vec<Value> = serde_json::from_str(
        &std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/csl/items.json"),
        )
        .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let mut locales = BTreeMap::new();
    for entry in std::fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .flatten()
    {
        let f = entry.file_name().to_string_lossy().to_string();
        if let Some(l) = f
            .strip_prefix("locales-")
            .and_then(|f| f.strip_suffix(".xml"))
        {
            locales.insert(
                l.to_string(),
                std::fs::read_to_string(entry.path()).map_err(|e| e.to_string())?,
            );
        }
    }
    let sys = Sys::new(&items, Arc::new(locales)).map_err(|e| e.to_string())?;
    let style = std::fs::read_to_string(dir.join(file)).map_err(|e| e.to_string())?;
    let mut engine = Engine::new(sys, &style, lang).map_err(|e| e.to_string())?;
    engine.set_output_format(OutputFormat::Html);
    Ok(engine)
}

// ----------------------------------------------------------------- compare

/// `case[:section]` of `INTERMEDIATE_DUMP`; a site case is `site:<style>`.
fn split_selector(sel: &str) -> (String, String) {
    let skip = usize::from(sel.starts_with("site:"));
    let mut parts = sel.splitn(skip + 2, ':');
    let mut case = parts.next().unwrap_or_default().to_string();
    if skip == 1 {
        case = format!("{case}:{}", parts.next().unwrap_or_default());
    }
    let section = parts.next().unwrap_or("style").to_string();
    (case, section)
}

/// The compared sections.
const SECTION_NAMES: [&str; 8] = [
    "style",
    "style_reduced",
    "locale",
    "xml",
    "items",
    "names",
    "numbers",
    "citation_items",
];

/// The citation-item lists of a fixture: each `CITATION-ITEMS` entry, then
/// each `CITATIONS` entry's `citationItems` (the reference script's order).
fn citation_lists(fx: &Fixture) -> Vec<Vec<Value>> {
    let mut lists: Vec<Vec<Value>> = Vec::new();
    if let Some(Value::Array(a)) = &fx.citation_items {
        for c in a {
            lists.push(c.as_array().cloned().unwrap_or_default());
        }
    }
    if let Some(Value::Array(a)) = &fx.citations {
        for c in a {
            lists.push(
                c.get(0)
                    .and_then(|c0| c0.get("citationItems"))
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default(),
            );
        }
    }
    lists
}

/// What one case produced: the three digests, or the constructor's error.
#[derive(Debug, Clone)]
enum Built {
    Digests {
        style: String,
        style_reduced: String,
        locale: String,
        xml: String,
        items: String,
        names: String,
        numbers: String,
        citation_items: String,
    },
    Error,
}

fn built_of(
    name: &str,
    engine: Result<Engine, String>,
    inputs: &[Value],
    lists: &[Vec<Value>],
    dump_selector: Option<&str>,
) -> Built {
    match engine {
        Err(e) => {
            if std::env::var("INTERMEDIATE_PRINT_DIFFERENCES").is_ok() {
                println!("BUILD ERROR {name}: {e}");
            }
            Built::Error
        }
        Ok(mut engine) => {
            let style = dump::style_text(&engine, true);
            let reduced = dump::style_text(&engine, false);
            let locale = dump::locale_text(&engine);
            let xml = engine
                .state()
                .csl_xml
                .data_obj
                .map(|r| engine.state().csl_xml.to_json_text(r))
                .unwrap_or_default();
            let input = dump::input_sections(&mut engine, inputs, lists);
            if std::env::var("INTERMEDIATE_RAW").is_ok() {
                if let Some(section) = dump_selector {
                    let text = match section {
                        "locale" => &locale,
                        "style_reduced" => &reduced,
                        "xml" => &xml,
                        "items" => &input.items,
                        "names" => &input.names,
                        "numbers" => &input.numbers,
                        "citation_items" => &input.citation_items,
                        _ => &style,
                    };
                    print!("RAW>>{text}<<RAW");
                }
            }
            if let Some(section) = dump_selector {
                let text = match section {
                    "locale" => &locale,
                    "style_reduced" => &reduced,
                    "xml" => &xml,
                    "items" => &input.items,
                    "names" => &input.names,
                    "numbers" => &input.numbers,
                    "citation_items" => &input.citation_items,
                    _ => &style,
                };
                let v: Value = serde_json::from_str(text).unwrap_or(Value::Null);
                println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
            }
            Built::Digests {
                style: dump::digest_text(&style),
                style_reduced: dump::digest_text(&reduced),
                locale: dump::digest_text(&locale),
                xml: dump::digest_text(&xml),
                items: dump::digest_text(&input.items),
                names: dump::digest_text(&input.names),
                numbers: dump::digest_text(&input.numbers),
                citation_items: dump::digest_text(&input.citation_items),
            }
        }
    }
}

fn known_groups() -> BTreeMap<(String, String), String> {
    let v: Value = serde_json::from_str(KNOWN).expect("known differences JSON");
    let mut out = BTreeMap::new();
    for g in v["groups"].as_array().expect("groups") {
        let section = g["section"].as_str().expect("section").to_string();
        let reason = g["reason"].as_str().expect("reason").to_string();
        assert!(
            !reason.trim().is_empty(),
            "a known difference needs a reason"
        );
        for c in g["cases"].as_array().expect("cases") {
            let case = c.as_str().expect("case").to_string();
            let prev = out.insert((section.clone(), case.clone()), reason.clone());
            assert!(prev.is_none(), "{section}/{case} listed twice");
        }
    }
    out
}

#[test]
fn the_intermediate_dump_matches_citeproc_js_except_the_recorded_differences() {
    let Some(fixtures) = load_fixtures() else {
        return;
    };
    let Some(locales) = load_locales() else {
        return;
    };
    let reference: Value = serde_json::from_str(DIGESTS).expect("reference digests");
    let reduced: Value = serde_json::from_str(REDUCED).expect("reduced digests");
    let xml_ref: Value = serde_json::from_str(XML).expect("xml digests");
    // INTERMEDIATE_DUMP=<case>[:section]: build and print that one case only.
    let dump_sel = std::env::var("INTERMEDIATE_DUMP").ok();
    let only: Option<(String, String)> = dump_sel.as_deref().map(split_selector);
    let site_items: Vec<Value> = serde_json::from_str(
        &std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/csl/items.json"),
        )
        .expect("items.json"),
    )
    .expect("items.json parses");

    // case name -> built
    let mut cases: Vec<(String, Built, &Value, &Value, &Value)> = Vec::new();
    for (name, _, file, lang) in SITE_STYLES.iter().map(|(n, f, l)| (n, 0, f, l)) {
        let key = format!("site:{name}");
        if only.as_ref().is_some_and(|(c, _)| *c != key) {
            continue;
        }
        let sel = only.as_ref().map(|(_, s)| s.as_str());
        let b = built_of(&key, site_engine(file, lang), &site_items, &[], sel);
        cases.push((
            key,
            b,
            &reference["site"][*name],
            &reduced["site"][*name],
            &xml_ref["site"][*name],
        ));
    }
    for (name, fx) in &fixtures {
        if only.as_ref().is_some_and(|(c, _)| c != name) {
            continue;
        }
        let sel = only.as_ref().map(|(_, s)| s.as_str());
        let b = built_of(
            name,
            fixture_engine(fx, &locales),
            &fx.input,
            &citation_lists(fx),
            sel,
        );
        cases.push((
            name.clone(),
            b,
            &reference["fixtures"][name],
            &reduced["fixtures"][name],
            &xml_ref["fixtures"][name],
        ));
    }
    if only.is_some() {
        return;
    }
    assert_eq!(cases.len(), 850, "845 fixtures and 5 site styles");

    let mut differing: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for (case, built, r, rr, rx) in &cases {
        match built {
            Built::Error => {
                for s in SECTION_NAMES {
                    differing.entry(s).or_default().push(case.clone());
                }
            }
            Built::Digests {
                style,
                style_reduced,
                locale,
                xml,
                items,
                names,
                numbers,
                citation_items,
            } => {
                for (section, ours, theirs) in [
                    ("style", style, r["style"].as_str()),
                    ("style_reduced", style_reduced, rr["style_reduced"].as_str()),
                    ("locale", locale, r["locale"].as_str()),
                    ("xml", xml, rx.as_str()),
                    ("items", items, r["items"].as_str()),
                    ("names", names, r["names"].as_str()),
                    ("numbers", numbers, r["numbers"].as_str()),
                    ("citation_items", citation_items, r["citation_items"].as_str()),
                ] {
                    if Some(ours.as_str()) == theirs {
                        *counts.entry(section).or_default() += 1;
                    } else {
                        differing.entry(section).or_default().push(case.clone());
                    }
                }
            }
        }
    }
    println!("RESULTS (of 850 cases, equal to citeproc-js): {counts:?}");
    println!("{RESULTS_NOTE}");
    if std::env::var("INTERMEDIATE_PRINT_DIFFERENCES").is_ok() {
        for (s, list) in &differing {
            println!(
                "DIFFERING {s} ({}): {}",
                list.len(),
                serde_json::to_string(list).unwrap_or_default()
            );
        }
    }

    let known = known_groups();
    let mut unexpected = Vec::new();
    for (section, list) in &differing {
        for case in list {
            if !known.contains_key(&(section.to_string(), case.clone())) {
                unexpected.push(format!("{section}/{case}"));
            }
        }
    }
    let mut vanished = Vec::new();
    for (section, case) in known.keys() {
        let still = differing
            .get(section.as_str())
            .is_some_and(|l| l.contains(case));
        if !still {
            vanished.push(format!("{section}/{case}"));
        }
    }
    assert!(
        unexpected.is_empty(),
        "{} unlisted differences from citeproc-js, first 20: {:?}",
        unexpected.len(),
        &unexpected[..unexpected.len().min(20)]
    );
    assert!(
        vanished.is_empty(),
        "{} listed differences no longer occur (remove them), first 20: {:?}",
        vanished.len(),
        &vanished[..vanished.len().min(20)]
    );
}

#[test]
fn the_committed_references_cover_the_same_cases() {
    let reference: Value = serde_json::from_str(DIGESTS).expect("reference digests");
    let reduced: Value = serde_json::from_str(REDUCED).expect("reduced digests");
    for part in ["site", "fixtures"] {
        let a: BTreeSet<&String> = reference[part]
            .as_object()
            .expect("object")
            .keys()
            .collect();
        let b: BTreeSet<&String> = reduced[part].as_object().expect("object").keys().collect();
        assert_eq!(a, b, "{part}: the reduced reference lists different cases");
    }
    assert_eq!(
        reference["fixtures"].as_object().map(|o| o.len()),
        Some(845)
    );
    // Every listed known difference names a real case and section.
    let known = known_groups();
    for (section, case) in known.keys() {
        assert!(
            SECTION_NAMES.contains(&section.as_str()),
            "unknown section {section}"
        );
        let in_site = case
            .strip_prefix("site:")
            .is_some_and(|n| reference["site"].get(n).is_some());
        assert!(
            in_site || reference["fixtures"].get(case).is_some(),
            "unknown case {case}"
        );
    }
}
