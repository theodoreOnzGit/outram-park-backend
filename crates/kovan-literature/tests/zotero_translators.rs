//! Code-to-code verification of the Zotero translator port against upstream
//! Zotero (GitHub #749, #752, epic #747).
//!
//! **Methodology.** `scripts/zotero-reference.sh` ran every fixture through
//! a running Zotero translation-server (server 3a9d17614896, translate
//! e0fe482b8a07, utilities 1dd38e27edf8, zotero-schema 70c3aa98627 (v44),
//! translators 3d1c78530f42; `reference/manifest.json`) and committed what
//! it returned under `tests/data/zotero/reference/`:
//!
//! * `import/<format>.json`: every `testCases` import case of BibTeX.js,
//!   RIS.js and CSL JSON.js (input verbatim) and the files in
//!   `fixtures/import/`, with the Web API JSON `/import` returned;
//! * `export/<format>/<set>.json`: `/export` of each list in
//!   `export_inputs/<set>.json` (upstream's itemJSON test data, one list per
//!   item type and all together; the items each import case produced; the
//!   kovan probe items), for BibTeX, BibLaTeX, RIS and CSL JSON;
//! * `roundtrip/<format>/<set>.json`: that export text posted back to
//!   `/import`;
//! * `utilities.json`: Zotero.Utilities helpers run directly from the
//!   server's utilities.js.
//!
//! Each test runs the port on the same input and compares. Import output is
//! compared as JSON values (key order is not meaningful in JSON objects);
//! upstream's random item keys were normalised by the harness to the
//! sequence the port generates (the only normalisation). Export output is
//! compared as text, line by line.
//!
//! **Pass criterion.** Every difference between port and upstream must be
//! listed, exactly (case, location, upstream value, port value), in
//! `tests/data/zotero/known_differences/<test>.json` with its reason; any other
//! difference, or a listed one that no longer occurs, fails. A listed
//! difference is a recorded finding, never a tolerance.
//!
//! **Environment.** Dates are parsed as the server does: en-US, no day
//! suffixes (not the Zotero client), "local" time = the server's zone
//! (`utcOffsetMinutes` in the manifest, +480 = Asia/Singapore), current
//! year 2026.
//!
//! Results are recorded in each test's doc comment.

use kovan_common::zotero::date::DateOptions;
use kovan_literature::zotero::framework::{
    fold_child_notes, JsObject, TranslateOptions, TranslationEnv,
};
use kovan_literature::zotero::translators::Translator;
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

/// The options the reference server ran with.
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
        // The server's clock when the references were recorded (RIS reads
        // `new Date()` for an access date without a time of day).
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

/// Line diff of two texts (longest common subsequence): one entry per hunk,
/// `("line N", upstream lines, port lines)` with N the hunk's first upstream
/// line (1-based) and the lines joined by "\n".
fn text_diff(a: &str, b: &str) -> Vec<(String, Value, Value)> {
    if a == b {
        return Vec::new();
    }
    let (x, y): (Vec<&str>, Vec<&str>) = (a.split('\n').collect(), b.split('\n').collect());
    let (n, m) = (x.len(), y.len());
    // lcs[i][j]: LCS length of x[i..] and y[j..].
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

/// Assert `diffs` (for the tests named in `tests`) equal the recorded known
/// differences for those tests, exactly.
fn assert_known(tests: &[&str], diffs: Vec<Diff>) {
    // One file per test: known_differences/<test with "/" as "_">.json.
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
    // For reviewing: ZOTERO_DIFF_OUT=<dir> writes the observed differences
    // of each test to <dir>/<test>.json.
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

/// The import cases of one format, compared; returns the differences and
/// the number of cases compared.
fn import_diffs(t: Translator) -> (Vec<Diff>, usize) {
    let test = format!("import/{}", t.format_name());
    let cases = read_json(&format!("reference/import/{}.json", t.format_name()));
    let mut diffs = Vec::new();
    let mut n = 0;
    for c in cases.as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        // Only cases the server imported with this translator.
        assert_eq!(
            c["translatorID"],
            t.metadata().id,
            "{test} {name}: server used another translator"
        );
        n += 1;
        let input = c["input"].as_str().unwrap();
        let port: Value = match t.import(input, &options(t)) {
            Ok(r) => Value::Array(r.api_json_at("CURRENT_TIMESTAMP")),
            Err(e) => json!({ "error": e.to_string() }),
        };
        let upstream = c
            .get("items")
            .cloned()
            .unwrap_or_else(|| json!({ "error": c["error"] }));
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
    (diffs, n)
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

/// Item lists of an export set, each list's objects in file order.
fn export_lists(set: &str) -> BTreeMap<String, Vec<JsObject>> {
    let text =
        std::fs::read_to_string(data_dir().join(format!("reference/export_inputs/{set}.json")))
            .unwrap();
    serde_json::from_str(&text).unwrap()
}

/// Export every list of every set in one format, compared as text.
fn export_diffs(t: Translator) -> (Vec<Diff>, usize) {
    let test = format!("export/{}", t.format_name());
    let mut diffs = Vec::new();
    let mut n = 0;
    for set in export_sets() {
        let refs = read_json(&format!("reference/export/{}/{set}.json", t.format_name()));
        for (name, items) in export_lists(&set) {
            let r = &refs[&name];
            n += 1;
            let upstream = r["output"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("ERROR {}", r["status"]));
            let port = match t.export(&items, &options(t)) {
                Ok(s) => s,
                Err(e) => format!("ERROR {e}"),
            };
            diffs.extend(
                text_diff(&upstream, &port)
                    .into_iter()
                    .map(|(at, u, p)| Diff {
                        test: test.clone(),
                        case: format!("{set}/{name}"),
                        at,
                        upstream: u,
                        port: p,
                    }),
            );
        }
    }
    (diffs, n)
}

/// What the translation-server's `/import` does (importEndpoint.js:31-47):
/// detect, import with the first translator that accepts the text, or fail
/// with "No suitable translators found".
fn server_import(text: &str) -> Value {
    match kovan_literature::zotero::translators::detect_import(text).first() {
        None => json!({ "error": "No suitable translators found" }),
        Some(&t) => match t.import(text, &options(t)) {
            Ok(r) => Value::Array(r.api_json_at("CURRENT_TIMESTAMP")),
            Err(e) => json!({ "error": e.to_string() }),
        },
    }
}

/// Import upstream's export text again and compare with upstream's
/// re-import (the round trip's second leg; the first leg is
/// [`export_diffs`]).
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
            // A translator the port does not have claimed the text upstream:
            // nothing to compare.
            if rt["translatorID"]
                .as_str()
                .is_some_and(|id| Translator::from_id(id).is_none())
            {
                continue;
            }
            n += 1;
            let text = exports[name]["output"].as_str().unwrap();
            let port = server_import(text);
            let upstream = rt
                .get("items")
                .cloned()
                .unwrap_or_else(|| json!({ "error": rt["error"] }));
            let mut d = Vec::new();
            json_diff("", &upstream, &port, &mut d);
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

/// **Import → export → import stability, port vs upstream.** For every
/// import fixture of a format with both directions, upstream's chain is
/// stable when the re-import equals the first import (both normalised, the
/// first folded the way the export input was). The port must be stable on
/// exactly the cases where upstream is.
fn stability(t: Translator) -> (Vec<String>, Vec<String>) {
    let fmt = t.format_name();
    let first = read_json(&format!("reference/export_inputs/import-{fmt}.json"));
    let rts = read_json(&format!("reference/roundtrip/{fmt}/import-{fmt}.json"));
    let mut upstream_stable = Vec::new();
    let mut port_stable = Vec::new();
    for (name, items) in first.as_object().unwrap() {
        let again = rts[name]
            .get("items")
            .map(|v| fold_child_notes(v.as_array().unwrap()));
        if again.as_ref() == items.as_array() {
            upstream_stable.push(name.clone());
        }
        // The port's own chain.
        let list: Vec<JsObject> = serde_json::from_value(items.clone()).unwrap();
        let exported = t.export(&list, &options(t)).unwrap_or_default();
        let re = server_import(&exported)
            .as_array()
            .map(|a| fold_child_notes(a));
        if re.as_ref() == items.as_array() {
            port_stable.push(name.clone());
        }
    }
    (upstream_stable, port_stable)
}

/// **CSL JSON import** against upstream: 2 cases (the translator's 1
/// testCase, `fixtures/import/csl_ranges.json`).
#[test]
fn csl_json_import_matches_upstream() {
    let (d, n) = import_diffs(Translator::CslJson);
    assert_eq!(n, 2);
    assert_known(&["import/csljson"], d);
}

/// **CSL JSON export** against upstream, every export list.
#[test]
fn csl_json_export_matches_upstream() {
    let (d, n) = export_diffs(Translator::CslJson);
    assert!(n > 80, "{n}");
    assert_known(&["export/csljson"], d);
}

/// **CSL JSON round trip**: upstream's export re-imported by the port.
#[test]
fn csl_json_roundtrip_matches_upstream() {
    let (d, _) = roundtrip_diffs(Translator::CslJson);
    assert_known(&["roundtrip/csljson"], d);
}

/// **CSL JSON import → export → import** stable on the same cases as
/// upstream.
#[test]
fn csl_json_chain_stable_where_upstream_is() {
    let (u, p) = stability(Translator::CslJson);
    assert_eq!(p, u);
}

/// **BibTeX import** against upstream: the translator's 24 testCases and
/// `fixtures/import/*.bib`.
#[test]
fn bibtex_import_matches_upstream() {
    let (d, n) = import_diffs(Translator::BibTeX);
    assert_eq!(n, 25);
    assert_known(&["import/bibtex"], d);
}

/// **BibTeX export** against upstream, every export list.
#[test]
fn bibtex_export_matches_upstream() {
    let (d, n) = export_diffs(Translator::BibTeX);
    assert!(n > 80, "{n}");
    assert_known(&["export/bibtex"], d);
}

/// **BibTeX round trip**: upstream's export re-imported by the port.
#[test]
fn bibtex_roundtrip_matches_upstream() {
    let (d, _) = roundtrip_diffs(Translator::BibTeX);
    assert_known(&["roundtrip/bibtex"], d);
}

/// **BibTeX import → export → import** stable on the same cases as
/// upstream.
#[test]
fn bibtex_chain_stable_where_upstream_is() {
    let (u, p) = stability(Translator::BibTeX);
    assert_eq!(p, u);
}

/// **BibLaTeX export** against upstream, every export list.
#[test]
fn biblatex_export_matches_upstream() {
    let (d, n) = export_diffs(Translator::BibLaTeX);
    assert!(n > 80, "{n}");
    assert_known(&["export/biblatex"], d);
}

/// **RIS import** against upstream: the translator's 12 testCases (testCase02
/// and testCase04 cover every RIS type and tag: 143 and 285 items) and
/// `fixtures/import/*.ris` (upstream's book_and_child_note.ris, the kovan
/// probe dates_and_entities.ris); 14 cases, 452 Web API items in all.
///
/// **Prediction (written before the first run, 2026-10-07):** exact on all
/// 14 cases, since RIS import calls only `strToDate`, `cleanDOI` and
/// `unescapeHTML`, and `strToDate` is identical in the server's utilities
/// (1dd38e27) and kovan-common's port (4051881d). The one clock-dependent
/// path (an access date without a time, :1663, local time of day of
/// `new Date()`) runs with "now" = the manifest's `generatedOn`.
///
/// **Result (2026-10-07):** pass, 14/14 cases identical, no known
/// differences. Prediction confirmed.
#[test]
fn ris_import_matches_upstream() {
    let (d, n) = import_diffs(Translator::Ris);
    assert_eq!(n, 14);
    assert_known(&["import/ris"], d);
}

/// **RIS export** against upstream, every export list (84: itemJSON's 37
/// types one by one and together, the items of every BibTeX, RIS and CSL
/// JSON import case, the kovan probes). Upstream runs RIS in legacy mode
/// (minVersion 3.0.4 < 4.0.27), so the framework's
/// `itemToLegacyExportFormat` and SQL access dates are exercised.
///
/// **Prediction (before the first run, 2026-10-07):** exact on all 84.
///
/// **Result (2026-10-07):** pass, 84/84 texts byte-identical, no known
/// differences. This includes `DA  - 2021/05//undefined` for the probe date
/// "2021 May" (#748 open item (b): upstream's strToDate does produce a
/// literal "undefined" part) and `Y2  - 2020/03/04/05:06:07` from a legacy
/// SQL access date.
#[test]
fn ris_export_matches_upstream() {
    let (d, n) = export_diffs(Translator::Ris);
    assert_eq!(n, 84);
    assert_known(&["export/ris"], d);
}

/// **RIS round trip**: upstream's RIS export of each of the 84 lists,
/// imported by the port the way the server's `/import` does (detection,
/// then the first translator), compared with upstream's re-import.
///
/// **Prediction (before the first run, 2026-10-07):** exact.
///
/// **Result (2026-10-07):** pass, 84/84 cases identical (581 items).
#[test]
fn ris_roundtrip_matches_upstream() {
    let (d, n) = roundtrip_diffs(Translator::Ris);
    assert_eq!(n, 84);
    assert_known(&["roundtrip/ris"], d);
}

/// **RIS import → export → import** stable on the same cases as upstream.
///
/// **Result (2026-10-07):** upstream's chain is stable on 10 of the 14 RIS
/// import cases, and the port's on exactly the same 10. What changes in the
/// four unstable cases (checked in the references, 2026-10-07): testCase01
/// loses Extra (RIS export has no tag for it); testCase02/04 change item
/// types (degenerate types, e.g. `document` -> GEN -> journalArticle),
/// dates and access dates (placeholders such as "0000 Year Date" and
/// "Access Date" are not dates on export), Extra, creators, pages;
/// dates_and_entities loses the markup in its title (re-import runs
/// `unescapeHTML`) and "2021 May" comes back as "2021-05" (PY/DA).
#[test]
fn ris_chain_stable_where_upstream_is() {
    let (u, p) = stability(Translator::Ris);
    assert_eq!(p, u);
    assert_eq!(
        u,
        [
            "book_and_child_note",
            "testCase00",
            "testCase03",
            "testCase05",
            "testCase06",
            "testCase07",
            "testCase08",
            "testCase09",
            "testCase10",
            "testCase11"
        ]
    );
}

/// **Zotero.Utilities helpers** against upstream's utilities.js run
/// directly (`reference/utilities.json`).
#[test]
fn utilities_match_upstream() {
    use kovan_literature::zotero::framework::{html, utilities as zu};
    let r = read_json("reference/utilities.json");
    let mut diffs = Vec::new();
    let mut push = |f: &str, case: String, u: Value, p: Value| {
        if u != p {
            diffs.push(Diff {
                test: "utilities".into(),
                case: format!("{f}: {case}"),
                at: String::new(),
                upstream: u,
                port: p,
            });
        }
    };
    for e in r["unescapeHTML"].as_array().unwrap() {
        let s = e[0].as_str().unwrap();
        push(
            "unescapeHTML",
            s.into(),
            e[1].clone(),
            html::unescape_html(s).into(),
        );
    }
    for e in r["cleanAuthor"].as_array().unwrap() {
        let (s, ty, comma) = (
            e[0].as_str().unwrap(),
            e[1].as_str().unwrap(),
            e[2].as_bool().unwrap(),
        );
        let a = zu::clean_author(s, ty, comma);
        let mut o = serde_json::Map::new();
        if let Some(f) = a.first_name {
            o.insert("firstName".into(), f.into());
        }
        o.insert("lastName".into(), a.last_name.into());
        o.insert("creatorType".into(), a.creator_type.into());
        push(
            "cleanAuthor",
            format!("{s} {comma}"),
            e[3].clone(),
            Value::Object(o),
        );
    }
    for e in r["cleanDOI"].as_array().unwrap() {
        let s = e[0].as_str().unwrap();
        push(
            "cleanDOI",
            s.into(),
            e[1].clone(),
            zu::clean_doi(s).map_or(Value::Null, Value::from),
        );
    }
    for e in r["text2html"].as_array().unwrap() {
        let (s, p) = (e[0].as_str().unwrap(), e[1].as_bool().unwrap());
        push(
            "text2html",
            s.into(),
            e[2].clone(),
            zu::text2html(s, p).into(),
        );
    }
    for e in r["trimInternal"].as_array().unwrap() {
        let s = e[0].as_str().unwrap();
        push(
            "trimInternal",
            s.into(),
            e[1].clone(),
            zu::trim_internal(s).into(),
        );
    }
    for e in r["removeDiacritics"].as_array().unwrap() {
        let (s, lc) = (e[0].as_str().unwrap(), e[1].as_bool().unwrap());
        push(
            "removeDiacritics",
            s.into(),
            e[2].clone(),
            zu::remove_diacritics(s, lc).into(),
        );
    }
    assert_known(&["utilities"], diffs);
}
