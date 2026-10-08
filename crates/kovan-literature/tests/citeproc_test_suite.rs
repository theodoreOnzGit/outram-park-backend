// Part of the kovan port of citeproc-js (GitHub #790, #791).
//
// Upstream:    juris-m/citeproc-test-runner 1.1.116 (commit b1e72d5c, MIT),
//              lib/fixture-parser.js, lib/sections.js, lib/sys.js,
//              lib/templateJS.js: the runner this file replicates, and
//              citeproc-js, https://github.com/juris-m/citeproc-js
//              (2.4.63, commit 73bc1b44bc7d54d0bfec4e070fd27f5efe024ff9),
//              whose Engine API it drives
// Copyright:   (c) Frank Bennett (citeproc-js 2009-2019; the runner)
// Licence:     AGPL-3.0 (citeproc-js: "CPAL-1.0 or AGPL-3.0-or-later"); the
//              runner is MIT, compatible. See this crate's NOTICE.
// Modified:    2026-10-08, by the OUTRAM PARK contributors: a Rust
//              translation of the runner's parsing and driving logic.
// No warranty: distributed in the hope that it will be useful, but WITHOUT
//              ANY WARRANTY; without even the implied warranty of
//              MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.

//! The CSL test suite against the citeproc-js port (GitHub #791, epic #790).
//!
//! **Methodology.** `scripts/csl-reference.sh` ran the 845
//! `processor-tests/humans/*.txt` fixtures of
//! `citation-style-language/test-suite` (commit `6eefc5b0`) through citeproc-js
//! 2.4.63 exactly as citeproc-js's own runner (`juris-m/citeproc-test-runner`)
//! does, and committed what citeproc-js produced in
//! `tests/data/csl/test_suite_reference.json` (method:
//! `scripts/csl-testsuite-reference.cjs`). This file parses the same fixtures
//! with a Rust translation of the runner's parser, drives
//! [`kovan_literature::citeproc::Engine`] with a translation of the runner's
//! `Sys.run` (the same sequence of `updateItems`, `makeCitationCluster`,
//! `processCitationCluster` and `makeBibliography` calls, the same
//! `updateDoc` bookkeeping), and compares with the committed citeproc-js
//! output. **The port is verified against citeproc-js's output, not against
//! the fixture's `RESULT`:** citeproc-js itself fails some fixtures (see
//! Results) and where it does, the port must reproduce citeproc-js.
//!
//! **Fixtures are not committed.** The suite carries no licence file and the
//! #790 licence decision keeps it in the git-ignored `vendor/csl-test-suite`
//! (and the locales in `vendor/citeproc-js/locale`), fetched by
//! `scripts/csl-reference.sh`. When `vendor/` is absent the tests that need
//! the fixtures print a reason and skip; the rest still run.
//!
//! **Which fixtures run.** Only those whose area (the file name before `_`)
//! is listed in `tests/data/csl/ported_areas.json`. Each stage (#792 to #797)
//! adds the areas it makes pass. The list is empty today: nothing is ported,
//! and the test prints "0 areas ported".
//!
//! **Pass criterion.** For a listed area, every difference between the port
//! and citeproc-js must be listed exactly (fixture, citeproc-js output, port
//! output) with a reason in `tests/data/csl/test_suite_known_differences.json`;
//! any other difference, or a listed one that no longer occurs, fails. A listed
//! difference is a recorded finding, never a tolerance.
//!
//! **Results (2026-10-08, stage 0, the harness).** 0 areas ported, 0 fixtures
//! compared. citeproc-js 2.4.63 itself fails 16 of the 845 fixtures
//! (`test_suite_reference.json` `meta`): bugreports 1, collapse 1, date 5,
//! decorations 1, label 1, magic 1, name 3, page 1, punctuation 1, textcase 1.
//! At least nine are the pinned locales being older than the suite
//! (`<term name="ad">` is `AD` there, ` AD` in the suite's `RESULT`;
//! `tran.`/`trans.`); results per stage are recorded as stages land.
//! Not replicated from the runner: JS's non-transitive `Array.sort`
//! comparator corner (`updateDoc` sorts by position, stable here), and
//! `MODE all` (no fixture uses it).

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use kovan_literature::citeproc::{
    Citation, CitationItem, CitationRef, Engine, EngineError, OutputFormat, Sys,
};
use regex::Regex;
use serde_json::{json, Map, Value};

const REFERENCE: &str = include_str!("data/csl/test_suite_reference.json");
const PORTED_AREAS: &str = include_str!("data/csl/ported_areas.json");
const KNOWN: &str = include_str!("data/csl/test_suite_known_differences.json");

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture_dir() -> PathBuf {
    workspace_root().join("vendor/csl-test-suite/processor-tests/humans")
}

fn locale_dir() -> PathBuf {
    workspace_root().join("vendor/citeproc-js/locale")
}

/// Print why a vendor-dependent test is skipped.
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

/// A parsed fixture (lib/fixture-parser.js).
#[derive(Debug, Clone)]
struct Fixture {
    name: String,
    csl: String,
    mode: String,
    result: String,
    input: Vec<Value>,
    abbreviations: Option<Value>,
    bibentries: Option<Vec<Vec<String>>>,
    bibsection: Option<Value>,
    citation_items: Option<Vec<Vec<CitationItem>>>,
    citations: Option<Vec<Value>>,
    input2: Option<Vec<Value>>,
    langparams: Option<Value>,
    multiaffix: Option<Value>,
    options: Option<Map<String, Value>>,
}

/// The runner's line-by-line section reader. Quirks kept: the line after an
/// opening tag is read as the first content line; the line after a closing tag
/// is skipped; a tag opening while the previous one's closing line is the
/// last thing read is an error; a section with no content lines stays unset
/// (except RESULT, which becomes the empty string); unknown tags (VERSION,
/// TAGS) are ordinary lines outside any section.
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
                    "attempted to open tag \"{}\" before tag \"{section}\" was closed",
                    &m[1]
                ));
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
    let array_of = |key: &str| -> Result<Option<Vec<Value>>, String> {
        match json_of(key)? {
            Some(Value::Array(a)) => Ok(Some(a)),
            Some(other) => Err(format!("tag \"{key}\" in {name} is not an array: {other}")),
            None => Ok(None),
        }
    };
    let req =
        |key: &str| text_of(key).ok_or_else(|| format!("missing required tag \"{key}\" in {name}"));
    let to_ids = |v: &Value| -> Result<Vec<String>, String> {
        v.as_array()
            .ok_or_else(|| format!("BIBENTRIES element not an array in {name}"))?
            .iter()
            .map(|id| {
                id.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| format!("non-string id in {name}"))
            })
            .collect()
    };
    let bibentries = match array_of("BIBENTRIES")? {
        Some(sets) => Some(sets.iter().map(to_ids).collect::<Result<Vec<_>, _>>()?),
        None => None,
    };
    let citation_items = match array_of("CITATION-ITEMS")? {
        Some(sets) => Some(
            sets.iter()
                .map(|set| {
                    set.as_array()
                        .ok_or_else(|| format!("CITATION-ITEMS element not an array in {name}"))?
                        .iter()
                        .map(|c| CitationItem::from_json(c).map_err(|e| e.to_string()))
                        .collect::<Result<Vec<_>, _>>()
                })
                .collect::<Result<Vec<_>, _>>()?,
        ),
        None => None,
    };
    let options = match json_of("OPTIONS")? {
        Some(Value::Object(m)) => Some(m),
        Some(other) => return Err(format!("OPTIONS in {name} is not an object: {other}")),
        None => None,
    };
    Ok(Fixture {
        name: name.to_string(),
        csl: req("CSL")?,
        mode: req("MODE")?,
        result: req("RESULT")?,
        input: array_of("INPUT")?
            .ok_or_else(|| format!("missing required tag \"INPUT\" in {name}"))?,
        abbreviations: json_of("ABBREVIATIONS")?,
        bibentries,
        bibsection: json_of("BIBSECTION")?,
        citation_items,
        citations: array_of("CITATIONS")?,
        input2: array_of("INPUT2")?,
        langparams: json_of("LANGPARAMS")?,
        multiaffix: json_of("MULTIAFFIX")?,
        options,
    })
}

fn area_of(name: &str) -> &str {
    name.split('_').next().unwrap_or(name)
}

/// All fixtures of the vendored suite, by name; `None` (with a printed
/// reason) when `vendor/csl-test-suite` is absent.
fn load_fixtures() -> Option<BTreeMap<String, Fixture>> {
    let dir = fixture_dir();
    let Ok(read) = std::fs::read_dir(&dir) else {
        skip(&format!("{} is absent", dir.display()));
        return None;
    };
    let mut out = BTreeMap::new();
    for entry in read.flatten() {
        let file = entry.file_name().to_string_lossy().to_string();
        let Some(stem) = file.strip_suffix(".txt") else {
            continue;
        };
        if !Regex::new(r"^[a-z]+_").unwrap().is_match(stem) {
            continue;
        }
        let text = std::fs::read_to_string(entry.path()).unwrap_or_else(|e| panic!("{file}: {e}"));
        let fx = parse_fixture_text(stem, &text).unwrap_or_else(|e| panic!("{file}: {e}"));
        out.insert(stem.to_string(), fx);
    }
    Some(out)
}

/// Every `locales-<lang>.xml` of `vendor/citeproc-js/locale`, with the
/// runner's `retrieveLocale` clean-up (processing instructions dropped).
fn load_locales() -> Option<Arc<BTreeMap<String, String>>> {
    let dir = locale_dir();
    let Ok(read) = std::fs::read_dir(&dir) else {
        skip(&format!("{} is absent", dir.display()));
        return None;
    };
    let pi = Regex::new(r"\s*<\?[^>]*\?>\s*\n").unwrap();
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

// ------------------------------------------------------------------- driver

/// JS truthiness of a JSON value (for `OPTIONS.variableWrapper`).
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

/// JS string coercion of a JSON value (for the header-mode dump).
fn js_string(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().map(js_string).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".to_string(),
        other => other.to_string(),
    }
}

/// lib/sys.js `normalizeAbbrevsKey`.
fn normalize_abbrevs_key(variable: &str, key: &str) -> String {
    let key = key.trim();
    if variable == "jurisdiction" || variable == "country" {
        return key.to_uppercase();
    }
    let strip = Regex::new(
        r"(?i)(?:(?-u:\b)|^)(?:and|et|y|und|l[ae]|the|[ld]')(?:(?-u:\b)|$)|[\x21-\x2C.\x2F\x3A-\x40\x5B-\x60\\\x7B\x7D-\x7E]",
    )
    .unwrap();
    let key = strip.replace_all(key, "").to_string();
    let key = Regex::new(r"\s*\x7C\s*")
        .unwrap()
        .replace_all(&key, "\x7C")
        .to_string();
    let key = key.replace('.', " ");
    let key = Regex::new(r"\s+")
        .unwrap()
        .replace_all(&key, " ")
        .to_string();
    key.trim().to_lowercase()
}

/// lib/sys.js: the `ABBREVIATIONS` section into the abbreviation cache.
fn build_abbreviations(raw: &Value) -> kovan_literature::citeproc::Abbreviations {
    let mut out = kovan_literature::citeproc::Abbreviations::new();
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
                    normalize_abbrevs_key("title", key)
                } else {
                    key.clone()
                };
                out.entry(jurisd.clone())
                    .or_default()
                    .entry(segment.clone())
                    .or_default()
                    .insert(
                        norm,
                        abbrev
                            .as_str()
                            .map(str::to_string)
                            .unwrap_or_else(|| js_string(abbrev)),
                    );
            }
        }
    }
    out
}

/// One element of the runner's `this.doc`.
struct DocEntry {
    prefix: String,
    citation_id: String,
    string: String,
}

/// lib/sys.js `updateDoc`: run `citations` through `processCitationCluster`
/// and keep the document the way a word-processor plugin would.
fn update_doc(
    engine: &mut Engine,
    citations: &[Value],
    doc: &mut Vec<DocEntry>,
) -> Result<(), EngineError> {
    for triple in citations {
        let parts = triple.as_array().filter(|a| a.len() == 3).ok_or_else(|| {
            EngineError::BadInput(format!("not a [citation, pre, post] triple: {triple}"))
        })?;
        let citation = Citation::from_json(&parts[0])?;
        let refs = |v: &Value| -> Result<Vec<CitationRef>, EngineError> {
            v.as_array()
                .ok_or_else(|| EngineError::BadInput(format!("not a citation list: {v}")))?
                .iter()
                .map(CitationRef::from_json)
                .collect()
        };
        let (pre, post) = (refs(&parts[1])?, refs(&parts[2])?);
        let updates = engine.process_citation_cluster(&citation, &pre, &post)?;
        // Removals first, so the indexes come out right.
        let mut j = doc.len();
        while j > 0 {
            j -= 1;
            if !engine.citation_registered(&doc[j].citation_id)? {
                doc.remove(j);
            }
        }
        // Fix the sequence of citations to reflect that in pre and post.
        let pos_map: BTreeMap<&str, usize> = pre
            .iter()
            .chain(post.iter())
            .enumerate()
            .map(|(n, r)| (r.citation_id.as_str(), n))
            .collect();
        doc.sort_by(|a, b| {
            match (
                pos_map.get(a.citation_id.as_str()),
                pos_map.get(b.citation_id.as_str()),
            ) {
                (Some(x), Some(y)) => x.cmp(y),
                _ => std::cmp::Ordering::Equal,
            }
        });
        for entry in doc.iter_mut() {
            entry.prefix = "..".to_string();
        }
        // A citationID already in the document is replaced in place.
        let mut pending: Vec<Option<_>> = updates.into_iter().map(Some).collect();
        for slot in pending.iter_mut() {
            let Some(insert) = slot.as_ref() else {
                continue;
            };
            if let Some(entry) = doc.iter_mut().find(|e| e.citation_id == insert.citation_id) {
                *entry = DocEntry {
                    prefix: ">>".to_string(),
                    citation_id: entry.citation_id.clone(),
                    string: insert.text.clone(),
                };
                *slot = None;
            }
        }
        // Others go in at the position citeproc-js gave.
        for insert in pending.into_iter().flatten() {
            let at = insert.index.min(doc.len());
            doc.insert(
                at,
                DocEntry {
                    prefix: ">>".to_string(),
                    citation_id: insert.citation_id,
                    string: insert.text,
                },
            );
        }
    }
    Ok(())
}

/// lib/sys.js `Sys.run` (the non-"all" branch) over the port's [`Engine`].
fn run_fixture(
    fx: &Fixture,
    locales: &Arc<BTreeMap<String, String>>,
) -> Result<String, EngineError> {
    let mut sys = Sys::new(&fx.input, locales.clone())?;
    // `Sys.run` copies OPTIONS.variableWrapper onto sys BEFORE `new CSL.Engine`.
    if fx.options.as_ref().and_then(|o| o.get("variableWrapper")).is_some_and(truthy) {
        sys.options.insert("variableWrapper".to_string(), Value::Bool(true));
    }
    if let Some(raw) = &fx.abbreviations {
        sys.abbreviations = build_abbreviations(raw);
    }
    let mut engine = Engine::new(sys, &fx.csl, "")?;

    let mut mode = fx.mode.split('-');
    let mode_name = mode.next().unwrap_or("").to_string();
    let submodes: BTreeSet<&str> = mode.collect();
    if mode_name != "citation" && mode_name != "bibliography" {
        return Err(EngineError::BadInput(format!(
            "invalid mode in test file {}: {}",
            fx.name, fx.mode
        )));
    }
    for name in ["rtf", "plain", "asciidoc", "xslfo"] {
        if submodes.contains(name) {
            if let Some(f) = OutputFormat::from_name(name) {
                engine.set_output_format(f);
            }
        }
    }
    if submodes.contains("suppress_trailing_punctuation") {
        engine.set_suppress_trailing_punctuation(true);
    }
    if let Some(options) = &fx.options {
        if options.get("variableWrapper").is_some_and(truthy) {
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
                // As the runner does: it passes `translat` here as well.
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

    let nosort = submodes.contains("nosort");
    if let Some(sets) = &fx.bibentries {
        for ids in sets {
            engine.update_items(ids, nosort)?;
        }
    } else if fx.citations.is_none() {
        let ids = fx
            .input
            .iter()
            .map(kovan_literature::citeproc::item_id)
            .collect::<Result<Vec<_>, _>>()?;
        engine.update_items(&ids, nosort)?;
    }
    let mut citation_items = fx.citation_items.clone();
    if citation_items.is_none() && fx.citations.is_none() {
        let ids = engine.registry_ids()?;
        let items = ids
            .iter()
            .map(|id| CitationItem::from_json(&json!({ "id": id })))
            .collect::<Result<Vec<_>, _>>()?;
        citation_items = Some(vec![items]);
    }
    if fx.abbreviations.is_none() {
        if let Some(sets) = &citation_items {
            for items in sets {
                engine.preload_abbreviations(items)?;
            }
        } else if let Some(citations) = &fx.citations {
            for triple in citations {
                let citation = Citation::from_json(triple.get(0).unwrap_or(&Value::Null))?;
                engine.preload_abbreviations(&citation.citation_items)?;
            }
        }
    }

    let mut citations: Vec<String> = Vec::new();
    if let Some(sets) = &citation_items {
        for items in sets {
            citations.push(engine.make_citation_cluster(items)?);
        }
    } else if let Some(cites) = &fx.citations {
        let mut doc: Vec<DocEntry> = Vec::new();
        update_doc(&mut engine, cites, &mut doc)?;
        if let Some(input2) = &fx.input2 {
            // The runner swaps INPUT for INPUT2 and rebuilds its item cache,
            // under the same engine.
            engine.replace_items(input2)?;
            update_doc(&mut engine, cites, &mut doc)?;
        }
        citations = doc
            .iter()
            .enumerate()
            .map(|(n, e)| format!("{}[{}] {}", e.prefix, n, e.string))
            .collect();
    }
    let mut ret = citations.join("\n");
    if mode_name == "bibliography" && !submodes.contains("header") {
        ret = engine.make_bibliography(fx.bibsection.as_ref())?.joined();
    } else if mode_name == "bibliography" {
        let params = engine.make_bibliography(None)?.params;
        let mut rows: Vec<(String, String)> = params
            .iter()
            .map(|(k, v)| (k.clone(), js_string(v)))
            .collect();
        rows.sort_by(|a, b| format!("{},{}", a.0, a.1).cmp(&format!("{},{}", b.0, b.1)));
        ret = rows
            .iter()
            .map(|(k, v)| format!("{k}: {v}\n"))
            .collect::<String>()
            .trim()
            .to_string();
    }
    Ok(ret.replace("\r\n", "\n").replace('\r', "\n"))
}

// -------------------------------------------------------------------- tests

fn reference() -> Value {
    serde_json::from_str(REFERENCE).expect("test_suite_reference.json")
}

fn ported_areas() -> BTreeSet<String> {
    let v: Value = serde_json::from_str(PORTED_AREAS).expect("ported_areas.json");
    v["areas"]
        .as_array()
        .expect("areas")
        .iter()
        .map(|a| a.as_str().expect("area").to_string())
        .collect()
}

#[test]
fn the_ported_fixtures_match_citeproc_js_except_the_recorded_differences() {
    let areas = ported_areas();
    println!("{} areas ported: {:?}", areas.len(), areas);
    let reference = reference();
    let fixtures_ref = reference["fixtures"].as_object().unwrap();
    let all_areas: BTreeSet<&str> = fixtures_ref.keys().map(|n| area_of(n)).collect();
    for a in &areas {
        assert!(
            all_areas.contains(a.as_str()),
            "ported_areas.json lists \"{a}\", which is no fixture area"
        );
    }

    let known: Value = serde_json::from_str(KNOWN).expect("test_suite_known_differences.json");
    let listed = known["differences"].as_array().unwrap();
    for l in listed {
        assert!(
            l["reason"].as_str().is_some_and(|r| !r.trim().is_empty()),
            "a listed difference has no reason: {l}"
        );
        let fixture = l["fixture"].as_str().unwrap();
        assert!(
            areas.contains(area_of(fixture)),
            "known difference for {fixture}, whose area is not ported"
        );
    }
    if areas.is_empty() {
        assert!(
            listed.is_empty(),
            "differences are listed but no area is ported"
        );
        return;
    }

    let (Some(fixtures), Some(locales)) = (load_fixtures(), load_locales()) else {
        return;
    };
    let mut diffs: Vec<Value> = Vec::new();
    let mut compared = 0;
    for (name, fx) in fixtures.iter().filter(|(n, _)| areas.contains(area_of(n))) {
        compared += 1;
        let want = fixtures_ref[name]["output"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let got = match run_fixture(fx, &locales) {
            Ok(s) => s,
            Err(e) => format!("ERROR: {e}"),
        };
        if got != want {
            diffs.push(json!({ "fixture": name, "citeproc_js": want, "port": got }));
        }
    }
    println!("compared {compared} fixtures, {} differ", diffs.len());
    let core = |v: &Value| json!({ "fixture": v["fixture"], "citeproc_js": v["citeproc_js"], "port": v["port"] });
    let listed_core: Vec<Value> = listed.iter().map(core).collect();
    let unlisted: Vec<&Value> = diffs.iter().filter(|d| !listed_core.contains(d)).collect();
    let stale: Vec<&Value> = listed_core.iter().filter(|l| !diffs.contains(l)).collect();
    assert!(
        unlisted.is_empty() && stale.is_empty(),
        "{} unlisted difference(s), {} listed difference(s) that no longer occur.\nunlisted: {}\nstale: {}",
        unlisted.len(),
        stale.len(),
        serde_json::to_string_pretty(&unlisted).unwrap(),
        serde_json::to_string_pretty(&stale).unwrap()
    );
}

#[test]
fn the_committed_reference_matches_the_vendored_fixtures() {
    let reference = reference();
    let meta = &reference["meta"];
    let fixtures_ref = reference["fixtures"].as_object().unwrap();
    assert_eq!(meta["fixtures"].as_u64(), Some(fixtures_ref.len() as u64));
    let fails: Vec<&String> = fixtures_ref
        .iter()
        .filter(|(_, r)| r["passes"] == json!(false))
        .map(|(n, _)| n)
        .collect();
    assert_eq!(
        meta["citeproc_js_fails"].as_u64(),
        Some(fails.len() as u64),
        "meta disagrees with the fixtures"
    );
    for (name, rec) in fixtures_ref {
        assert_eq!(
            rec["passes"] == json!(true),
            rec["output"] == rec["expected"],
            "{name}: passes disagrees with output/expected"
        );
        assert!(
            rec["error"].is_null() || rec["output"].is_null(),
            "{name}: has both an error and an output"
        );
    }
    let Some(fixtures) = load_fixtures() else {
        return;
    };
    let ours: BTreeSet<&String> = fixtures.keys().collect();
    let theirs: BTreeSet<&String> = fixtures_ref.keys().collect();
    assert_eq!(ours, theirs, "the vendored suite and the reference list different fixtures; re-run scripts/csl-reference.sh");
    for (name, fx) in &fixtures {
        assert_eq!(
            fixtures_ref[name]["expected"].as_str(),
            Some(fx.result.as_str()),
            "{name}: RESULT differs from the reference's"
        );
        assert_eq!(
            fixtures_ref[name]["mode"].as_str(),
            Some(fx.mode.as_str()),
            "{name}: MODE differs"
        );
    }
}

#[test]
fn the_parser_follows_the_runner_on_synthetic_text() {
    let text = "\
>>===== MODE =====>>
citation
<<===== MODE =====<<

>>===== RESULT =====>>
(Doe 2000)
second line
<<===== RESULT =====<<

>>===== CSL =====>>
<style/>
<<===== CSL =====<<

>>===== VERSION =====>>
1.0
<<===== VERSION =====<<

>>===== INPUT =====>>
[{\"id\": \"ITEM-1\"}]
<<===== INPUT =====<<

>>===== CITATION-ITEMS =====>>
[[{\"id\": \"ITEM-1\"}]]
<<===== CITATION-ITEMS =====<<
";
    let fx = parse_fixture_text("x_y", text).unwrap();
    assert_eq!(fx.mode, "citation");
    assert_eq!(fx.result, "(Doe 2000)\nsecond line");
    assert_eq!(fx.csl, "<style/>");
    assert_eq!(fx.input.len(), 1);
    assert_eq!(fx.citation_items.as_ref().unwrap()[0][0].id, "ITEM-1");
    assert!(fx.citations.is_none() && fx.bibentries.is_none());
    // Empty RESULT is the empty string; missing CSL is an error; an unclosed tag opening another is an error.
    let empty = "\
>>===== MODE =====>>
citation
<<===== MODE =====<<

>>===== RESULT =====>>
<<===== RESULT =====<<
";
    assert_eq!(read_sections(empty).unwrap()["RESULT"], vec![String::new()]);
    assert!(parse_fixture_text("x_y", empty)
        .unwrap_err()
        .contains("missing required tag"));
    // The runner needs a line between a closing tag and the next opening one.
    assert!(read_sections(
        empty
            .replace("<<===== MODE =====<<\n\n", "<<===== MODE =====<<\n")
            .as_str()
    )
    .is_err());
    assert!(read_sections(">>===== MODE =====>>\n>>===== CSL =====>>\n").is_err());
    assert!(read_sections(">>===== MODE =====>>\na\n<<===== CSL =====<<\n").is_err());
}

#[test]
fn the_parser_reads_real_fixtures() {
    let dir = fixture_dir();
    if !dir.is_dir() {
        skip(&format!("{} is absent", dir.display()));
        return;
    }
    let read = |n: &str| {
        std::fs::read_to_string(dir.join(format!("{n}.txt"))).unwrap_or_else(|e| panic!("{n}: {e}"))
    };
    // A plain citation fixture, a bibliography one, and one with CITATIONS.
    let a = parse_fixture_text("affix_CommaAfterQuote", &read("affix_CommaAfterQuote")).unwrap();
    assert_eq!(a.mode, "citation");
    assert!(!a.input.is_empty() && !a.result.is_empty() && a.csl.contains("<style"));
    let b = parse_fixture_text(
        "sort_AuthorDateWithYearSuffix",
        &read("sort_AuthorDateWithYearSuffix"),
    )
    .unwrap();
    assert!(b.mode.starts_with("bibliography") || b.mode.starts_with("citation"));
    let c = parse_fixture_text(
        "position_IbidWithLocator",
        &read("position_IbidWithLocator"),
    )
    .unwrap();
    assert!(c.citations.is_some() || c.citation_items.is_some());
    let ref_fixtures = reference();
    assert_eq!(
        ref_fixtures["fixtures"]["affix_CommaAfterQuote"]["expected"].as_str(),
        Some(a.result.as_str())
    );
}

#[test]
fn the_driver_reaches_the_engine_and_reports_errors_instead_of_panicking() {
    // The driver ported from the runner is exercised end to end on a style
    // with no rendering: every call returns a result, none panics.
    let sample = "\
>>===== MODE =====>>
citation
<<===== MODE =====<<

>>===== RESULT =====>>
x
<<===== RESULT =====<<

>>===== CSL =====>>
<style/>
<<===== CSL =====<<

>>===== INPUT =====>>
[{\"id\": \"ITEM-1\", \"type\": \"book\"}]
<<===== INPUT =====<<
";
    let locales = Arc::new(BTreeMap::new());
    let fx = parse_fixture_text("x_y", sample).unwrap();
    let _ = run_fixture(&fx, &locales);

    let bib = sample.replace("citation\n", "bibliography-header-rtf-nosort\n");
    let fx = parse_fixture_text("x_y", &bib).unwrap();
    let _ = run_fixture(&fx, &locales);

    let with_citations = format!(
        "{sample}\n>>===== CITATIONS =====>>\n[[{{\"citationID\":\"C1\",\"citationItems\":[{{\"id\":\"ITEM-1\"}}],\"properties\":{{\"noteIndex\":0}}}},[],[]]]\n<<===== CITATIONS =====<<\n\n>>===== OPTIONS =====>>\n{{\"variableWrapper\": true, \"x\": 1}}\n<<===== OPTIONS =====<<\n\n>>===== LANGPARAMS =====>>\n{{\"langs\": {{\"translat\": [\"de\"], \"translit\": [\"ja\"]}}, \"titles\": [\"translit\"]}}\n<<===== LANGPARAMS =====<<\n\n>>===== ABBREVIATIONS =====>>\n{{\"default\": {{\"container-title\": {{\"The J. of Things.\": \"JT\"}}}}}}\n<<===== ABBREVIATIONS =====<<\n"
    );
    let fx = parse_fixture_text("x_y", &with_citations).unwrap();
    let _ = run_fixture(&fx, &locales);

    let mut bad_mode = fx.clone();
    bad_mode.mode = "all".to_string();
    assert!(matches!(
        run_fixture(&bad_mode, &locales),
        Err(EngineError::BadInput(_))
    ));
}

#[test]
fn abbreviation_keys_are_normalised_as_the_runner_does() {
    assert_eq!(
        normalize_abbrevs_key("title", "The J. of Things."),
        "j of things"
    );
    assert_eq!(normalize_abbrevs_key("country", "us"), "US");
    let abbrevs = build_abbreviations(
        &json!({"default": {"container-title": {"The J. of Things.": "JT"}, "place": {"US": "United States"}}}),
    );
    assert_eq!(abbrevs["default"]["container-title"]["j of things"], "JT");
    assert_eq!(abbrevs["default"]["place"]["US"], "United States");
}

#[test]
fn update_doc_keeps_the_document_the_way_the_runner_does() {
    // The cluster names a "pre" citation the engine never registered: the call
    // fails (citeproc-js: a TypeError) and the document is left untouched.
    let mut engine = Engine::new(Sys::default(), "<style/>", "en-US").unwrap();
    let mut doc = vec![DocEntry {
        prefix: "..".into(),
        citation_id: "C0".into(),
        string: "old".into(),
    }];
    let cites = vec![
        json!([{"citationID": "C1", "citationItems": [], "properties": {"noteIndex": 0}}, [["C0", 0]], []]),
    ];
    assert!(update_doc(&mut engine, &cites, &mut doc).is_err());
    assert_eq!(doc.len(), 1);
    assert!(matches!(
        update_doc(&mut engine, &[json!([1])], &mut doc),
        Err(EngineError::BadInput(_))
    ));
    assert_eq!(
        (doc[0].citation_id.as_str(), doc[0].string.as_str()),
        ("C0", "old")
    );
}

#[test]
fn js_coercions_match_javascript() {
    assert!(truthy(&json!(true)) && truthy(&json!("x")) && truthy(&json!([])) && truthy(&json!(2)));
    assert!(
        !truthy(&json!(false))
            && !truthy(&json!(""))
            && !truthy(&json!(0))
            && !truthy(&Value::Null)
    );
    assert_eq!(js_string(&json!([1, "a", [2]])), "1,a,2");
    assert_eq!(js_string(&json!({"a": 1})), "[object Object]");
    assert_eq!(js_string(&Value::Null), "");
    assert_eq!(js_string(&json!(true)), "true");
}

/// `CITEPROC_SUITE_REPORT=1 cargo test --release -p kovan-literature --test
/// citeproc_test_suite -- --nocapture report_per_area`: run EVERY fixture
/// (whatever `ported_areas.json` says) and print, per area, how many fixtures
/// the port reproduces citeproc-js on, how many also equal the fixture's own
/// `RESULT`, and how many stop at an error; then the most common error
/// messages. `CITEPROC_SUITE_REPORT=2` also lists each differing fixture
/// with the first line of its error or of both outputs. Without the variable
/// the test does nothing, so the ordinary run stays quiet.
#[test]
fn report_per_area() {
    let Ok(mode) = std::env::var("CITEPROC_SUITE_REPORT") else {
        return;
    };
    let detail = mode == "2" || mode == "3";
    let full = mode == "3";
    let (Some(fixtures), Some(locales)) = (load_fixtures(), load_locales()) else {
        return;
    };
    let reference = reference();
    let fixtures_ref = reference["fixtures"].as_object().unwrap();
    // area -> (total, same as citeproc-js, same as RESULT, errors)
    let mut by_area: BTreeMap<String, (usize, usize, usize, usize)> = BTreeMap::new();
    let mut errors: BTreeMap<String, usize> = BTreeMap::new();
    let only = std::env::var("CITEPROC_SUITE_ONLY").ok();
    for (name, fx) in &fixtures {
        if let Some(o) = &only {
            if !name.starts_with(o.as_str()) {
                continue;
            }
        }
        let want = fixtures_ref[name]["output"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let got = std::panic::catch_unwind(|| run_fixture(fx, &locales));
        let (got, is_error) = match got {
            Ok(Ok(s)) => (s, false),
            Ok(Err(e)) => (format!("ERROR: {e}"), true),
            Err(_) => ("ERROR: panic".to_string(), true),
        };
        let slot = by_area.entry(area_of(name).to_string()).or_default();
        slot.0 += 1;
        if got == want {
            slot.1 += 1;
        }
        if got == fx.result {
            slot.2 += 1;
        }
        if is_error {
            slot.3 += 1;
            let first = got.lines().next().unwrap_or("").to_string();
            *errors.entry(first.clone()).or_default() += 1;
            if detail {
                println!("  ERR  {name}: {first}");
            }
        } else if got != want && detail {
            println!(
                "  DIFF {name}\n    citeproc-js: {:?}\n    port:        {:?}",
                if full { want.as_str() } else { want.lines().next().unwrap_or("") },
                if full { got.as_str() } else { got.lines().next().unwrap_or("") }
            );
        }
    }
    let (mut t, mut a, mut b, mut c) = (0, 0, 0, 0);
    println!(
        "{:<14} {:>5} {:>8} {:>8} {:>7}",
        "area", "total", "=cslJS", "=RESULT", "errors"
    );
    for (area, (total, same, result, err)) in &by_area {
        println!("{area:<14} {total:>5} {same:>8} {result:>8} {err:>7}");
        t += total;
        a += same;
        b += result;
        c += err;
    }
    println!("{:<14} {:>5} {:>8} {:>8} {:>7}", "ALL", t, a, b, c);
    let mut top: Vec<(&String, &usize)> = errors.iter().collect();
    top.sort_by(|x, y| y.1.cmp(x.1));
    println!("most common errors:");
    for (msg, n) in top.iter().take(25) {
        println!("  {n:>4}  {msg}");
    }
}
