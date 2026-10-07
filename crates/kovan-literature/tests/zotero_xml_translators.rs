//! Code-to-code verification of the XML translators (GitHub #749, epic
//! #747) against upstream Zotero: MODS, Endnote XML, TEI, Crossref Unixref
//! XML, MARCXML, MARC, PubMed XML, METS, Primo Normalized XML, DSpace
//! Intermediate Metadata, Citavi 5 XML, XML ContextObject.
//!
//! **Methodology.** As `tests/zotero_translators.rs` (same harness, same
//! server, same commits: `reference/manifest.json`): `scripts/zotero-reference.sh`
//! ran every `testCases` import entry of these translators, and the files in
//! `fixtures/import/<format>/`, through a running Zotero translation-server
//! (`reference/import/<format>.json`); exported every existing export list
//! (`reference/export_inputs/`) as MODS, Endnote XML and TEI
//! (`reference/export/<format>/`); re-imported the MODS and Endnote XML
//! exports (`reference/roundtrip/<format>/`); and exported each MODS and
//! Endnote XML import fixture's items in the same format and re-imported
//! them (`reference/chain_inputs/<format>.json`, `reference/chain/<format>.json`).
//! Each test runs the port on the same input and compares exactly: import
//! output as JSON values (upstream's random item keys normalised by the
//! harness to the port's key sequence, the only normalisation), export
//! output as text, line by line.
//!
//! The helpers below are the ones in `tests/zotero_translators.rs`, copied
//! so that the XML translators' tests live in their own file (three ports
//! ran in parallel, #749); keep the two in step.
//!
//! **Pass criterion.** Every difference between port and upstream must be
//! listed, exactly, in `tests/data/zotero/known_differences/<test>.json`
//! with its traced cause; any other difference, or a listed one that no
//! longer occurs, fails. A listed difference is a recorded finding, never a
//! tolerance.
//!
//! **Detection.** Upstream's `/import` picks the first translator (by
//! priority, then file name) whose `detectImport` accepts the text, among
//! all of Zotero's translators. A case upstream gave to another ported
//! translator (the DSpace test case, a METS document, went to METS) is
//! compared through the port's own detection; [`xml_detection_matches_upstream`]
//! checks the port picks the same translator as upstream on every case.
//!
//! Results are recorded in each test's doc comment.

use kovan_common::zotero::date::DateOptions;
use kovan_literature::zotero::framework::{JsObject, TranslateOptions, TranslationEnv};
use kovan_literature::zotero::translators::{detect_import, Translator};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/zotero")
}

fn read_json(rel: &str) -> Value {
    let p = data_dir().join(rel);
    let s = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    serde_json::from_str(&s).unwrap()
}

/// The options the reference server ran with (as `tests/zotero_translators.rs`).
fn options(t: Translator) -> TranslateOptions {
    let manifest = read_json("reference/manifest.json");
    let offset = manifest["utcOffsetMinutes"].as_i64().unwrap() as i32;
    let mut o = t.default_options();
    o.env = TranslationEnv {
        dates: DateOptions {
            current_year: 2026,
            month_first: true,
            day_suffixes: Vec::new(),
            utc_offset_minutes: offset,
        },
        now_unix_secs: Some(iso_to_unix(manifest["generatedOn"].as_str().unwrap())),
        ..TranslationEnv::default()
    };
    o
}

/// `YYYY-MM-DDThh:mm:ss[.sss]Z` as seconds since the epoch.
fn iso_to_unix(s: &str) -> i64 {
    let n = |a: usize, b: usize| s[a..b].parse::<i64>().unwrap();
    let (y, m, d) = (n(0, 4), n(5, 7), n(8, 10));
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * ((m + 9) % 12) + 2) / 5 + d - 1;
    let days = era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468;
    days * 86_400 + n(11, 13) * 3600 + n(14, 16) * 60 + n(17, 19)
}

/// One difference between upstream and the port.
#[derive(Debug, Clone, PartialEq)]
struct Diff {
    test: String,
    case: String,
    at: String,
    upstream: Value,
    port: Value,
}

impl Diff {
    fn to_json(&self) -> Value {
        json!({"test": self.test, "case": self.case, "at": self.at,
               "upstream": self.upstream, "port": self.port})
    }
}

/// Structural JSON diff: (path, upstream, port), `null` for absent.
fn json_diff(path: &str, a: &Value, b: &Value, out: &mut Vec<(String, Value, Value)>) {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let mut keys: Vec<&String> = x.keys().chain(y.keys()).collect();
            keys.sort();
            keys.dedup();
            for k in keys {
                let p = format!("{path}/{k}");
                match (x.get(k), y.get(k)) {
                    (Some(u), Some(v)) => json_diff(&p, u, v, out),
                    (u, v) => out.push((
                        p,
                        u.cloned().unwrap_or(Value::Null),
                        v.cloned().unwrap_or(Value::Null),
                    )),
                }
            }
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
            for (i, (u, v)) in x.iter().zip(y).enumerate() {
                json_diff(&format!("{path}/{i}"), u, v, out);
            }
        }
        _ if a == b => {}
        _ => out.push((path.to_owned(), a.clone(), b.clone())),
    }
}

/// Line diff (longest common subsequence), one entry per hunk.
fn text_diff(a: &str, b: &str) -> Vec<(String, Value, Value)> {
    if a == b {
        return Vec::new();
    }
    let (x, y): (Vec<&str>, Vec<&str>) = (a.split('\n').collect(), b.split('\n').collect());
    let (n, m) = (x.len(), y.len());
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if x[i] == y[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && x[i] == y[j] {
            i += 1;
            j += 1;
            continue;
        }
        let start = i;
        let (mut del, mut ins) = (Vec::new(), Vec::new());
        while (i < n || j < m) && !(i < n && j < m && x[i] == y[j]) {
            if j >= m || (i < n && lcs[i + 1][j] >= lcs[i][j + 1]) {
                del.push(x[i]);
                i += 1;
            } else {
                ins.push(y[j]);
                j += 1;
            }
        }
        out.push((
            format!("line {}", start + 1),
            del.join("\n").into(),
            ins.join("\n").into(),
        ));
    }
    out
}

/// Assert `diffs` equal the recorded known differences of `tests`, exactly.
fn assert_known(tests: &[&str], diffs: Vec<Diff>) {
    let mut known: Vec<Value> = Vec::new();
    for t in tests {
        let rel = format!("known_differences/{}.json", t.replace('/', "_"));
        if data_dir().join(&rel).exists() {
            known.extend(read_json(&rel).as_array().unwrap().iter().cloned());
        }
    }
    let mut want: Vec<Value> = known
        .iter()
        .filter(|d| tests.contains(&d["test"].as_str().unwrap()))
        .map(|d| {
            let mut d = d.clone();
            d.as_object_mut().unwrap().remove("reason");
            d
        })
        .collect();
    let mut got: Vec<Value> = diffs.iter().map(Diff::to_json).collect();
    let key = |v: &Value| format!("{}|{}|{}", v["test"], v["case"], v["at"]);
    want.sort_by_key(key);
    got.sort_by_key(key);
    if let Ok(dir) = std::env::var("ZOTERO_DIFF_OUT") {
        let name = tests.join("+").replace('/', "_");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            PathBuf::from(dir).join(format!("{name}.json")),
            serde_json::to_string_pretty(&got).unwrap(),
        )
        .unwrap();
    }
    let unexpected: Vec<&Value> = got.iter().filter(|g| !want.contains(g)).collect();
    let missing: Vec<&Value> = want.iter().filter(|w| !got.contains(w)).collect();
    assert!(
        unexpected.is_empty() && missing.is_empty(),
        "{} unexpected difference(s) (not in known_differences/):\n{}\n\n{} recorded difference(s) that no longer occur:\n{}",
        unexpected.len(),
        serde_json::to_string_pretty(&unexpected).unwrap(),
        missing.len(),
        serde_json::to_string_pretty(&missing).unwrap()
    );
}

/// What the translation-server's `/import` does: detect, then import with
/// the first translator that accepts the text.
fn server_import(text: &str) -> Value {
    match detect_import(text).first() {
        None => json!({ "error": "No suitable translators found" }),
        Some(&t) => match t.import(text, &options(t)) {
            Ok(r) => Value::Array(r.api_json_at("CURRENT_TIMESTAMP")),
            Err(e) => json!({ "error": e.to_string() }),
        },
    }
}

fn upstream_result(c: &Value) -> Value {
    c.get("items")
        .cloned()
        .unwrap_or_else(|| json!({ "error": c["error"] }))
}

/// The import cases of one format: those upstream gave to `t` run through
/// `t`, those it gave to another ported translator through detection.
/// Returns the differences, the number of cases compared and the number of
/// upstream items in them.
fn import_diffs(t: Translator) -> (Vec<Diff>, usize, usize) {
    let test = format!("import/{}", t.format_name());
    let cases = read_json(&format!("reference/import/{}.json", t.format_name()));
    let mut diffs = Vec::new();
    let (mut n, mut items) = (0, 0);
    for c in cases.as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let input = c["input"].as_str().unwrap();
        let port = if c["translatorID"] == t.metadata().id {
            match t.import(input, &options(t)) {
                Ok(r) => Value::Array(r.api_json_at("CURRENT_TIMESTAMP")),
                Err(e) => json!({ "error": e.to_string() }),
            }
        } else if c["translatorID"]
            .as_str()
            .and_then(Translator::from_id)
            .is_some()
        {
            server_import(input)
        } else {
            continue;
        };
        n += 1;
        let upstream = upstream_result(c);
        items += upstream.as_array().map_or(0, Vec::len);
        let mut d = Vec::new();
        json_diff("", &upstream, &port, &mut d);
        diffs.extend(d.into_iter().map(|(at, u, p)| Diff {
            test: test.clone(),
            case: name.to_owned(),
            at,
            upstream: u,
            port: p,
        }));
    }
    (diffs, n, items)
}

fn export_sets() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(data_dir().join("reference/export_inputs"))
        .unwrap()
        .map(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .trim_end_matches(".json")
                .to_owned()
        })
        .collect();
    v.sort();
    v
}

fn export_lists(rel: &str) -> BTreeMap<String, Vec<JsObject>> {
    let text = std::fs::read_to_string(data_dir().join(rel)).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn export_one(t: Translator, items: &[JsObject], r: &Value) -> (String, String) {
    let upstream = r["output"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| format!("ERROR {}", r["status"]));
    let port = match t.export(items, &options(t)) {
        Ok(s) => s,
        Err(e) => format!("ERROR {e}"),
    };
    (upstream, port)
}

/// Export every list of every set, compared as text; (diffs, lists,
/// identical lists).
fn export_diffs(t: Translator) -> (Vec<Diff>, usize, usize) {
    let test = format!("export/{}", t.format_name());
    let mut diffs = Vec::new();
    let (mut n, mut same) = (0, 0);
    for set in export_sets() {
        let refs = read_json(&format!("reference/export/{}/{set}.json", t.format_name()));
        for (name, items) in export_lists(&format!("reference/export_inputs/{set}.json")) {
            n += 1;
            let (upstream, port) = export_one(t, &items, &refs[&name]);
            let d = text_diff(&upstream, &port);
            if d.is_empty() {
                same += 1;
            }
            diffs.extend(d.into_iter().map(|(at, u, p)| Diff {
                test: test.clone(),
                case: format!("{set}/{name}"),
                at,
                upstream: u,
                port: p,
            }));
        }
    }
    (diffs, n, same)
}

/// Upstream's export text of every list imported again by the port
/// (detection first, as `/import`), against upstream's re-import.
fn roundtrip_diffs(t: Translator) -> (Vec<Diff>, usize) {
    let test = format!("roundtrip/{}", t.format_name());
    let mut diffs = Vec::new();
    let mut n = 0;
    for set in export_sets() {
        let exports = read_json(&format!("reference/export/{}/{set}.json", t.format_name()));
        let rts = read_json(&format!(
            "reference/roundtrip/{}/{set}.json",
            t.format_name()
        ));
        for (name, rt) in rts.as_object().unwrap() {
            if rt["translatorID"]
                .as_str()
                .is_some_and(|id| Translator::from_id(id).is_none())
            {
                continue;
            }
            n += 1;
            let text = exports[name]["output"].as_str().unwrap();
            let port = server_import(text);
            let mut d = Vec::new();
            json_diff("", &upstream_result(rt), &port, &mut d);
            diffs.extend(d.into_iter().map(|(at, u, p)| Diff {
                test: test.clone(),
                case: format!("{set}/{name}"),
                at,
                upstream: u,
                port: p,
            }));
        }
    }
    (diffs, n)
}

/// The import -> export -> import chain on the format's own import
/// fixtures: the port's export of each fixture's items against upstream's
/// (text), and the port's import of upstream's export against upstream's
/// re-import (JSON). (diffs, chains).
fn chain_diffs(t: Translator) -> (Vec<Diff>, usize) {
    let fmt = t.format_name();
    let test = format!("chain/{fmt}");
    let inputs = export_lists(&format!("reference/chain_inputs/{fmt}.json"));
    let chain = read_json(&format!("reference/chain/{fmt}.json"));
    let mut diffs = Vec::new();
    let mut n = 0;
    for (name, items) in &inputs {
        n += 1;
        let c = &chain[name];
        let (upstream, port) = export_one(t, items, &c["export"]);
        for (at, u, p) in text_diff(&upstream, &port) {
            diffs.push(Diff {
                test: test.clone(),
                case: format!("{name}/export"),
                at,
                upstream: u,
                port: p,
            });
        }
        if let Some(re) = c.get("reimport") {
            if re["translatorID"]
                .as_str()
                .is_some_and(|id| Translator::from_id(id).is_none())
            {
                continue;
            }
            let text = c["export"]["output"].as_str().unwrap();
            let mut d = Vec::new();
            json_diff("", &upstream_result(re), &server_import(text), &mut d);
            diffs.extend(d.into_iter().map(|(at, u, p)| Diff {
                test: test.clone(),
                case: format!("{name}/reimport"),
                at,
                upstream: u,
                port: p,
            }));
        }
    }
    (diffs, n)
}

/// The XML translators' formats with import references.
const XML_IMPORT: [Translator; 11] = [
    Translator::Mods,
    Translator::EndnoteXml,
    Translator::CrossrefUnixrefXml,
    Translator::MarcXml,
    Translator::Marc,
    Translator::PubMedXml,
    Translator::Mets,
    Translator::PrimoNormalizedXml,
    Translator::DSpaceIntermediateMetadata,
    Translator::Citavi5Xml,
    Translator::XmlContextObject,
];

/// **Detection** on every XML import case: the port's first accepting
/// translator is the one upstream's `/import` used (whenever upstream's
/// choice is a ported translator; otherwise the port must not claim it with
/// a translator that sorts before upstream's choice, which cannot be checked
/// here and is skipped).
#[test]
fn xml_detection_matches_upstream() {
    let mut diffs = Vec::new();
    for t in XML_IMPORT {
        let cases = read_json(&format!("reference/import/{}.json", t.format_name()));
        for c in cases.as_array().unwrap() {
            let Some(up) = c["translatorID"].as_str().and_then(Translator::from_id) else {
                continue;
            };
            let port = detect_import(c["input"].as_str().unwrap()).first().copied();
            if port != Some(up) {
                diffs.push(Diff {
                    test: "detection".into(),
                    case: format!("{}/{}", t.format_name(), c["name"].as_str().unwrap()),
                    at: String::new(),
                    upstream: up.format_name().into(),
                    port: port.map_or(Value::Null, |p| p.format_name().into()),
                });
            }
        }
    }
    assert_known(&["detection"], diffs);
}

/// **MODS import**: the translator's 23 testCases (58 items upstream).
///
/// **Result:** not yet run.
#[test]
fn mods_import_matches_upstream() {
    let (d, n, _) = import_diffs(Translator::Mods);
    assert_eq!(n, 23);
    assert_known(&["import/mods"], d);
}

/// **MODS export** of every export list.
///
/// **Result:** not yet run.
#[test]
fn mods_export_matches_upstream() {
    let (d, n, _) = export_diffs(Translator::Mods);
    assert_eq!(n, 84);
    assert_known(&["export/mods"], d);
}

/// **MODS round trip**: upstream's MODS export re-imported by the port.
///
/// **Result:** not yet run.
#[test]
fn mods_roundtrip_matches_upstream() {
    let (d, n) = roundtrip_diffs(Translator::Mods);
    assert_eq!(n, 84);
    assert_known(&["roundtrip/mods"], d);
}

/// **MODS import -> export -> import** on the 23 import fixtures.
///
/// **Result:** not yet run.
#[test]
fn mods_chain_matches_upstream() {
    let (d, n) = chain_diffs(Translator::Mods);
    assert_eq!(n, 23);
    assert_known(&["chain/mods"], d);
}

/// **Endnote XML import**: the translator's 3 testCases (14 items).
///
/// **Result:** not yet run.
#[test]
fn endnote_xml_import_matches_upstream() {
    let (d, n, _) = import_diffs(Translator::EndnoteXml);
    assert_eq!(n, 3);
    assert_known(&["import/endnote_xml"], d);
}

/// **Endnote XML export** of every export list.
///
/// **Result:** not yet run.
#[test]
fn endnote_xml_export_matches_upstream() {
    let (d, n, _) = export_diffs(Translator::EndnoteXml);
    assert_eq!(n, 84);
    assert_known(&["export/endnote_xml"], d);
}

/// **Endnote XML round trip** (four exports upstream cannot re-import:
/// "No suitable translators found"; compared as errors).
///
/// **Result:** not yet run.
#[test]
fn endnote_xml_roundtrip_matches_upstream() {
    let (d, n) = roundtrip_diffs(Translator::EndnoteXml);
    assert_eq!(n, 84);
    assert_known(&["roundtrip/endnote_xml"], d);
}

/// **Endnote XML import -> export -> import** on the 3 import fixtures.
///
/// **Result:** not yet run.
#[test]
fn endnote_xml_chain_matches_upstream() {
    let (d, n) = chain_diffs(Translator::EndnoteXml);
    assert_eq!(n, 3);
    assert_known(&["chain/endnote_xml"], d);
}

/// **TEI export** of every export list.
///
/// **Result:** not yet run.
#[test]
fn tei_export_matches_upstream() {
    let (d, n, _) = export_diffs(Translator::Tei);
    assert_eq!(n, 84);
    assert_known(&["export/tei"], d);
}

/// **Crossref Unixref XML import**: the translator's 12 testCases (13
/// items upstream).
///
/// **Result:** not yet run.
#[test]
fn crossref_unixref_xml_import_matches_upstream() {
    let (d, n, _) = import_diffs(Translator::CrossrefUnixrefXml);
    assert_eq!(n, 12);
    assert_known(&["import/crossref_unixref_xml"], d);
}

/// **MARCXML import**: the translator's 6 testCases (12 items upstream).
///
/// **Result:** not yet run.
#[test]
fn marcxml_import_matches_upstream() {
    let (d, n, _) = import_diffs(Translator::MarcXml);
    assert_eq!(n, 6);
    assert_known(&["import/marcxml"], d);
}

/// **MARC import**: the translator's 4 testCases (7 items upstream).
///
/// **Result:** not yet run.
#[test]
fn marc_import_matches_upstream() {
    let (d, n, _) = import_diffs(Translator::Marc);
    assert_eq!(n, 4);
    assert_known(&["import/marc"], d);
}

/// **PubMed XML import**: the translator's 11 testCases (13 items
/// upstream).
///
/// **Result:** not yet run.
#[test]
fn pubmed_xml_import_matches_upstream() {
    let (d, n, _) = import_diffs(Translator::PubMedXml);
    assert_eq!(n, 11);
    assert_known(&["import/pubmed_xml"], d);
}

/// **METS import**: the translator's 4 testCases (5 items upstream; MODS
/// and MARCXML run as child translators).
///
/// **Result:** not yet run.
#[test]
fn mets_import_matches_upstream() {
    let (d, n, _) = import_diffs(Translator::Mets);
    assert_eq!(n, 4);
    assert_known(&["import/mets"], d);
}

/// **Primo Normalized XML import**: the translator's 13 testCases (13
/// items upstream).
///
/// **Result:** not yet run.
#[test]
fn primo_normalized_xml_import_matches_upstream() {
    let (d, n, _) = import_diffs(Translator::PrimoNormalizedXml);
    assert_eq!(n, 13);
    assert_known(&["import/primo_normalized_xml"], d);
}

/// **DSpace Intermediate Metadata import**: its one testCase, a METS
/// document (upstream's `/import` gave it to METS, priority 50, which skips
/// the `OTHER` metadata: 0 items; compared through the port's detection),
/// and `fixtures/import/dspace_intermediate_metadata/`, the same document
/// with the `mets:` prefix renamed `m:` so that METS does not claim it and
/// DSpace does (1 item).
///
/// **Result:** not yet run.
#[test]
fn dspace_intermediate_metadata_import_matches_upstream() {
    let (d, n, _) = import_diffs(Translator::DSpaceIntermediateMetadata);
    assert_eq!(n, 2);
    assert_known(&["import/dspace_intermediate_metadata"], d);
}

/// **Citavi 5 XML import**: the translator has no testCases; two
/// kovan-authored probes (`fixtures/import/citavi5_xml/`, a Citavi 5 and a
/// Citavi 6 project: persons, periodicals, series, publishers, keywords,
/// groups, categories, knowledge items, locations, tasks, a contribution in
/// proceedings; 14 items upstream).
///
/// **Result:** not yet run.
#[test]
fn citavi5_xml_import_matches_upstream() {
    let (d, n, _) = import_diffs(Translator::Citavi5Xml);
    assert_eq!(n, 2);
    assert_known(&["import/citavi5_xml"], d);
}

/// **XML ContextObject import**: the translator's 2 testCases (3 items
/// upstream).
///
/// **Result:** not yet run.
#[test]
fn xml_contextobject_import_matches_upstream() {
    let (d, n, _) = import_diffs(Translator::XmlContextObject);
    assert_eq!(n, 2);
    assert_known(&["import/xml_contextobject"], d);
}
