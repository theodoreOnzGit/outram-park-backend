//! Code-to-code verification of the RDF translator port (Zotero RDF, RDF,
//! Bibliontology RDF, Unqualified Dublin Core RDF) and the RDF layer under
//! them against upstream Zotero (GitHub #749, #752, epic #747).
//!
//! **Methodology.** `scripts/zotero-reference.sh` recorded upstream's
//! output in `tests/data/zotero/reference/` (commits in
//! `reference/manifest.json`: translation-server with translate dd524aea9a55,
//! utilities 4051881d59c6, zotero-schema b86c79b56479, translators
//! 3d1c78530f42), from two sources:
//!
//! * the running translation-server: `import/rdf.json` and
//!   `import/rdf_bibliontology.json` (RDF.js and Bibliontology RDF.js
//!   `testCases`, `fixtures/import/*.rdf` and `*.bibo.rdf`, through `/import`
//!   with its detection); `export/<fmt>/<set>.json` (`/export` of every list
//!   in `export_inputs/` and `export_inputs_rdf/` as `rdf_zotero`,
//!   `rdf_bibliontology`, `rdf_dc`); `roundtrip/<fmt>/<set>.json` (that text
//!   back through `/import`);
//! * the server's own modules loaded in-process
//!   (`scripts/zotero-reference-inproc.mjs`), for what the endpoints cannot
//!   do: `inproc/forced_import.json` (each import fixture with the translator
//!   forced, as Zotero runs a translator's testCases; the saved items as the
//!   framework hands them to the item saver, the collections, and the Web API
//!   JSON), and `inproc/library.json` (the library round trip: a kovan-authored
//!   library with child notes, attachments, relations, tags and nested
//!   collections exported as Zotero RDF with its collections, as Zotero
//!   desktop does, then imported with RDF.js).
//!
//! Each test runs the port on the same input. Imports compare as JSON values
//! (key order is not meaningful); exports compare as text, line by line,
//! byte for byte.
//!
//! **What is normalised, and why.** (1) `/import` item keys, by the harness,
//! as in `tests/zotero_translators.rs`. (2) Blank-node ids: upstream's RDF
//! library numbers blank nodes from a counter global to the server process,
//! so `rdf:nodeID="n11408"` depends on what the server did before; the ids'
//! length even decides upstream's line packing. The port's counter is
//! started where upstream's was ([`TranslationEnv::first_blank_node_id`]):
//! for server exports, the first `rdf:nodeID` of the reference minus the
//! first of the port's output at 0; for in-process runs, the counter value
//! the harness recorded. The output is then compared verbatim. (3) The
//! saved items of in-process imports lack upstream's random `id`.
//!
//! **Pass criterion.** As in `tests/zotero_translators.rs`: every difference
//! must be listed, with its traced cause, in `known_differences/<test>.json`;
//! any other difference, or a listed one that no longer occurs, fails. An
//! upstream error (HTTP 500) and a port error count as the same outcome.
//!
//! **Prediction (written before the first full run, 2026-10-07):** after
//! the spot checks run while porting (itemJSON, the kovan probes and the
//! import sets through all three exports; the translators' testCases),
//! identical everywhere, including the library round trip.
//!
//! Results are recorded in each test's doc comment.

use kovan_common::zotero::date::DateOptions;
use kovan_literature::zotero::framework::{
    fold_child_notes, CollectionChild, JsObject, TranslateOptions, TranslationEnv,
    TranslatorCollection,
};
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

/// The options the reference server ran with (as `tests/zotero_translators.rs`).
fn options(t: Translator, first_blank_node_id: u64) -> TranslateOptions {
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
        first_blank_node_id,
        ..TranslationEnv::default()
    };
    o
}

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

/// Structural JSON diff (as `tests/zotero_translators.rs`).
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

/// Line diff (as `tests/zotero_translators.rs`).
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

/// Assert the differences equal the recorded ones (`known_differences/`).
fn assert_known(test: &str, diffs: Vec<Diff>) {
    let rel = format!("known_differences/{}.json", test.replace('/', "_"));
    let mut want: Vec<Value> = if data_dir().join(&rel).exists() {
        read_json(&rel)
            .as_array()
            .unwrap()
            .iter()
            .map(|d| {
                let mut d = d.clone();
                d.as_object_mut().unwrap().remove("reason");
                d
            })
            .collect()
    } else {
        Vec::new()
    };
    let mut got: Vec<Value> = diffs.iter().map(Diff::to_json).collect();
    let key = |v: &Value| format!("{}|{}|{}", v["test"], v["case"], v["at"]);
    want.sort_by_key(key);
    got.sort_by_key(key);
    if let Ok(dir) = std::env::var("ZOTERO_DIFF_OUT") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            PathBuf::from(dir).join(format!("{}.json", test.replace('/', "_"))),
            serde_json::to_string_pretty(&got).unwrap(),
        )
        .unwrap();
    }
    let unexpected: Vec<&Value> = got.iter().filter(|g| !want.contains(g)).collect();
    let missing: Vec<&Value> = want.iter().filter(|w| !got.contains(w)).collect();
    assert!(
        unexpected.is_empty() && missing.is_empty(),
        "{} unexpected difference(s):\n{}\n\n{} recorded difference(s) that no longer occur:\n{}",
        unexpected.len(),
        serde_json::to_string_pretty(&unexpected).unwrap(),
        missing.len(),
        serde_json::to_string_pretty(&missing).unwrap()
    );
}

/// What the server's `/import` does: detect, import with the first
/// translator that accepts the text. Returns the translator's id (or
/// `None`) and the Web API JSON or an error marker.
fn server_import(text: &str) -> (Option<&'static str>, Value) {
    match detect_import(text).first() {
        None => (None, json!({ "error": true })),
        Some(&t) => match t.import(text, &options(t, 0)) {
            Ok(r) => (
                Some(t.metadata().id),
                Value::Array(r.api_json_at("CURRENT_TIMESTAMP")),
            ),
            Err(_) => (Some(t.metadata().id), json!({ "error": true })),
        },
    }
}

/// Upstream's `/import` record as (translator id, items or error marker).
/// The server answers 500 without a translator id when the translation
/// throws.
fn upstream_import(rec: &Value) -> (Option<String>, Value) {
    match rec.get("items") {
        Some(items) => (
            rec["translatorID"].as_str().map(str::to_owned),
            items.clone(),
        ),
        None => (None, json!({ "error": true })),
    }
}

/// Import cases of one reference file through detection, compared with
/// upstream: the translator chosen and the items. Returns (differences,
/// cases compared, Web API items compared).
fn import_diffs(format: &str) -> (Vec<Diff>, usize, usize) {
    let test = format!("import/{format}");
    let cases = read_json(&format!("reference/import/{format}.json"));
    let mut diffs = Vec::new();
    let (mut n, mut items) = (0, 0);
    for c in cases.as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let (up_id, up) = upstream_import(c);
        let (port_id, port) = server_import(c["input"].as_str().unwrap());
        n += 1;
        items += up.as_array().map_or(0, Vec::len);
        let mut d = Vec::new();
        if up_id.is_some() && up_id.as_deref() != port_id {
            d.push(("translator".to_owned(), json!(up_id), json!(port_id)));
        }
        json_diff("", &up, &port, &mut d);
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

fn export_sets(dir: &str) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(data_dir().join(format!("reference/{dir}")))
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

fn export_lists(dir: &str, set: &str) -> BTreeMap<String, Vec<JsObject>> {
    let text =
        std::fs::read_to_string(data_dir().join(format!("reference/{dir}/{set}.json"))).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// The first `rdf:nodeID="nN"` number in a text.
fn first_node_id(s: &str) -> Option<u64> {
    let i = s.find("rdf:nodeID=\"n")? + "rdf:nodeID=\"n".len();
    s[i..].split('"').next()?.parse().ok()
}

/// Export one list with the port, its blank-node counter started where
/// upstream's was (module docs).
fn port_export(
    t: Translator,
    items: &[JsObject],
    upstream: Option<&str>,
) -> Result<String, String> {
    let first = t.export(items, &options(t, 0)).map_err(|e| e.to_string())?;
    let base = match (upstream.and_then(first_node_id), first_node_id(&first)) {
        (Some(u), Some(p)) if u >= p => u - p,
        _ => return Ok(first),
    };
    t.export(items, &options(t, base))
        .map_err(|e| e.to_string())
}

const ERROR: &str = "ERROR (translation failed)";

/// Every export list of every set in one RDF format, compared as text;
/// returns (differences, lists, lists that failed on both sides, upstream
/// lines).
fn export_diffs(t: Translator) -> (Vec<Diff>, usize, usize, usize) {
    let fmt = t.format_name();
    let test = format!("export/{fmt}");
    let mut diffs = Vec::new();
    let (mut n, mut both_failed, mut lines) = (0, 0, 0);
    for dir in ["export_inputs", "export_inputs_rdf"] {
        for set in export_sets(dir) {
            let refs = read_json(&format!("reference/export/{fmt}/{set}.json"));
            for (name, items) in export_lists(dir, &set) {
                let r = &refs[&name];
                n += 1;
                let upstream = r["output"].as_str().map(str::to_owned);
                let port = port_export(t, &items, upstream.as_deref()).ok();
                if upstream.is_none() && port.is_none() {
                    both_failed += 1;
                }
                let up = upstream.unwrap_or_else(|| ERROR.to_owned());
                lines += up.lines().count();
                let port = port.unwrap_or_else(|| ERROR.to_owned());
                diffs.extend(text_diff(&up, &port).into_iter().map(|(at, u, p)| Diff {
                    test: test.clone(),
                    case: format!("{set}/{name}"),
                    at,
                    upstream: u,
                    port: p,
                }));
            }
        }
    }
    (diffs, n, both_failed, lines)
}

/// Upstream's export text of each list re-imported by the port through
/// detection, compared with upstream's re-import. Returns (differences,
/// texts compared, items compared).
fn roundtrip_diffs(t: Translator) -> (Vec<Diff>, usize, usize) {
    let fmt = t.format_name();
    let test = format!("roundtrip/{fmt}");
    let mut diffs = Vec::new();
    let (mut n, mut items) = (0, 0);
    for dir in ["export_inputs", "export_inputs_rdf"] {
        for set in export_sets(dir) {
            let exports = read_json(&format!("reference/export/{fmt}/{set}.json"));
            let rts = read_json(&format!("reference/roundtrip/{fmt}/{set}.json"));
            for (name, rt) in rts.as_object().unwrap() {
                let text = exports[name]["output"].as_str().unwrap();
                let (up_id, up) = upstream_import(rt);
                if up_id
                    .as_deref()
                    .is_some_and(|id| Translator::from_id(id).is_none())
                {
                    continue;
                }
                n += 1;
                items += up.as_array().map_or(0, Vec::len);
                let (port_id, port) = server_import(text);
                let mut d = Vec::new();
                if up_id.is_some() && up_id.as_deref() != port_id {
                    d.push(("translator".to_owned(), json!(up_id), json!(port_id)));
                }
                json_diff("", &up, &port, &mut d);
                diffs.extend(d.into_iter().map(|(at, u, p)| Diff {
                    test: test.clone(),
                    case: format!("{set}/{name}"),
                    at,
                    upstream: u,
                    port: p,
                }));
            }
        }
    }
    (diffs, n, items)
}

/// A library fixture's items and collections, each object in its own key
/// order (parsed from the text: a `serde_json::Value` would sort the keys,
/// and key order decides Zotero RDF's output order).
fn read_library(rel: &str) -> (Vec<JsObject>, Vec<JsObject>) {
    #[derive(serde::Deserialize)]
    struct Lib {
        items: Vec<JsObject>,
        collections: Vec<JsObject>,
    }
    let text = std::fs::read_to_string(data_dir().join(rel)).unwrap();
    let lib: Lib = serde_json::from_str(&text).unwrap();
    (lib.items, lib.collections)
}

/// A collection as the harness writes upstream's (`{name, children}`).
fn collection_json(c: &TranslatorCollection) -> Value {
    json!({
        "name": c.name,
        "children": c.children.iter().map(|ch| match ch {
            CollectionChild::Item { id } => json!({"type": "item", "id": id}),
            CollectionChild::Collection(sub) => {
                let mut v = collection_json(sub);
                v["type"] = "collection".into();
                v
            }
        }).collect::<Vec<_>>(),
    })
}

/// An in-process import record compared with the port importing the same
/// input with the same translator and the same first blank-node id.
fn inproc_import_diffs(t: Translator, rec: &Value, test: &str, case: &str) -> Vec<Diff> {
    let input = rec["input"].as_str().map(str::to_owned).unwrap_or_default();
    import_record_diffs(t, &input, rec, test, case)
}

fn import_record_diffs(
    t: Translator,
    input: &str,
    rec: &Value,
    test: &str,
    case: &str,
) -> Vec<Diff> {
    let first = rec["firstBlankNodeId"].as_u64().unwrap();
    let up = if rec.get("error").is_some() {
        json!({"error": true})
    } else {
        json!({"items": rec["items"], "collections": rec["collections"], "apiItems": rec["apiItems"]})
    };
    let port = match t.import(input, &options(t, first)) {
        Err(_) => json!({"error": true}),
        Ok(r) => json!({
            "items": r.items.iter().map(|i| i.to_value()).collect::<Vec<_>>(),
            "collections": r.collections.iter().map(collection_json).collect::<Vec<_>>(),
            "apiItems": r.api_json_at("CURRENT_TIMESTAMP"),
        }),
    };
    let mut d = Vec::new();
    json_diff("", &up, &port, &mut d);
    d.into_iter()
        .map(|(at, u, p)| Diff {
            test: test.to_owned(),
            case: case.to_owned(),
            at,
            upstream: u,
            port: p,
        })
        .collect()
}

/// **RDF import** (RDF.js testCases and `fixtures/import/*.rdf`) through
/// detection, as the server's `/import` does: the translator detected and
/// the Web API items.
///
/// **Result (2026-10-07):** 11 cases (6 testCases, 5 fixtures), 19 Web API
/// items (the count first asserted, 26, was a guess written before the run
/// and wrong; corrected to the measured 19); identical, translator choice included (testCase00 is detected as
/// Bibliontology RDF upstream and in the port: it has BIBO classes;
/// `rdf_container_hole` fails on both sides, as upstream's RDF.js throws on
/// a hole in a creator `rdf:Seq`).
#[test]
fn rdf_import_matches_upstream() {
    let (d, n, items) = import_diffs("rdf");
    assert_eq!((n, items), (11, 19));
    assert_known("import/rdf", d);
}

/// **Bibliontology RDF import** (its testCase and
/// `fixtures/import/*.bibo.rdf`) through detection.
///
/// **Result (2026-10-07):** 2 cases, 3 items, identical.
#[test]
fn bibliontology_import_matches_upstream() {
    let (d, n, items) = import_diffs("rdf_bibliontology");
    assert_eq!((n, items), (2, 3));
    assert_known("import/rdf_bibliontology", d);
}

/// **Forced-translator imports** (`inproc/forced_import.json`): every RDF.js
/// and Bibliontology RDF import fixture run with that translator, as
/// Zotero's translator tests run testCases. Compared: the saved items in the
/// translator format (after `_itemDone`), the collections, the Web API JSON.
///
/// **Result (2026-10-07):** RDF.js 11 cases (RDF.js on testCase00, which
/// the server sends to Bibliontology RDF, included; `rdf_container_hole`
/// throws on both sides), Bibliontology RDF 7 (its testCase and all six
/// fixtures; it imports nothing from four of them); 18 in all (19 was a
/// pre-run guess, corrected); identical.
#[test]
fn forced_imports_match_upstream() {
    let forced = read_json("reference/inproc/forced_import.json");
    let mut diffs = Vec::new();
    let mut n = 0;
    for (fmt, t) in [
        ("rdf", Translator::Rdf),
        ("rdf_bibliontology", Translator::BibliontologyRdf),
    ] {
        for c in forced[fmt].as_array().unwrap() {
            n += 1;
            diffs.extend(inproc_import_diffs(
                t,
                c,
                &format!("forced_import/{fmt}"),
                c["name"].as_str().unwrap(),
            ));
        }
    }
    assert_eq!(n, 18);
    assert_known("forced_import", diffs);
}

/// **Zotero RDF export** against upstream: every list of `export_inputs/`
/// (upstream's itemJSON one type at a time and together, the items of every
/// BibTeX/RIS/CSL JSON import case, the kovan probes) and
/// `export_inputs_rdf/` (the RDF imports' items, the kovan library), as text.
///
/// **Result (2026-10-07):** 97 lists, 9837 upstream lines, every list
/// byte-identical; after merging the XML and tagged-text translators' export
/// sets (develop dbb9e26eb1): 158 lists, 14831 lines, identical (27 fail on
/// both sides). The 13 lists (first run) upstream fails on (folded child notes
/// without a `tags` array: Zotero RDF's `generateTags(note.tags)` reads
/// `.length` of undefined) fail in the port too.
///
/// First full run: 0 differences here, but Bibliontology RDF's export showed
/// `versionNumber` out of place, traced to the port rebuilding the order of
/// `uniqueFields` from the converted item, from which the legacy conversion
/// removes `versionNumber`. Fixed in the port (the order is now replayed on
/// the export input object, `rdf_support::unique_fields_order`), not
/// recorded.
#[test]
fn zotero_rdf_export_matches_upstream() {
    let (d, n, failed, lines) = export_diffs(Translator::ZoteroRdf);
    eprintln!(
        "zotero rdf export: {n} lists, {failed} failed on both sides, {lines} upstream lines"
    );
    assert!(n > 90, "{n}");
    assert_known("export/rdf_zotero", d);
}

/// **Bibliontology RDF export** against upstream, every list.
///
/// **Result (2026-10-07):** 97 lists, 6130 upstream lines (after the merge
/// with develop: 158 lists, 11696 lines, 11 failing on both sides), byte-identical
/// after the `uniqueFields` order fix described under the Zotero RDF export
/// test (first run: 3 differences, all `versionNumber`'s place). The 7 lists
/// upstream fails on (an item type its `TYPES` table lacks: preprint,
/// dataset, standard) fail in the port too.
#[test]
fn bibliontology_export_matches_upstream() {
    let (d, n, failed, lines) = export_diffs(Translator::BibliontologyRdf);
    eprintln!(
        "bibliontology export: {n} lists, {failed} failed on both sides, {lines} upstream lines"
    );
    assert!(n > 90, "{n}");
    assert_known("export/rdf_bibliontology", d);
}

/// **Unqualified Dublin Core RDF export** against upstream, every list.
///
/// **Result (2026-10-07):** 97 lists, 2815 upstream lines, byte-identical;
/// after the merge with develop: 158 lists, 4004 lines, identical.
#[test]
fn dc_rdf_export_matches_upstream() {
    let (d, n, failed, lines) = export_diffs(Translator::DcRdf);
    eprintln!("dc rdf export: {n} lists, {failed} failed on both sides, {lines} upstream lines");
    assert!(n > 90, "{n}");
    assert_known("export/rdf_dc", d);
}

/// **Round trips through the server**: upstream's Zotero RDF, Bibliontology
/// RDF and Dublin Core RDF export of every list, imported by the port
/// through detection (RDF.js or Bibliontology RDF), compared with upstream's
/// re-import.
///
/// **Result (2026-10-07):** Zotero RDF 84 texts (151 items), Bibliontology
/// RDF 90 (109), Dublin Core RDF 97 (132); identical, detection included.
/// After the merge with develop (its export sets added, all 38 ported
/// translators in detection): 131 (232), 147 (199), 158 (226); identical.
#[test]
fn rdf_roundtrips_match_upstream() {
    let mut all = Vec::new();
    for t in [
        Translator::ZoteroRdf,
        Translator::BibliontologyRdf,
        Translator::DcRdf,
    ] {
        let (d, n, items) = roundtrip_diffs(t);
        eprintln!("roundtrip {}: {n} texts, {items} items", t.format_name());
        assert!(n > 80, "{n}");
        all.extend(d);
    }
    assert_known("roundtrip/rdf", all);
}

/// **The library round trip** (`inproc/library.json`): the kovan library
/// (`fixtures/library/kovan_library.json`: seven items of five types, a
/// child note with tags and a relation, an attachment with its own note,
/// tags and file path, a standalone note and attachment, related items,
/// manual and automatic tags, three collections, one nested) exported as
/// Zotero RDF with its collections, then imported with RDF.js; both legs
/// compared with upstream's own.
///
/// **Result (2026-10-07):** first run: the export differed in field order
/// throughout; cause: this test parsed the fixture through
/// `serde_json::Value`, which sorts keys (fixed in the test with
/// [`read_library`]). Then: the export is byte-identical and the import
/// (saved items with notes, attachments, `seeAlso`, tags; the collection
/// tree; Web API JSON) identical. What the round trip keeps and loses is
/// upstream's: recorded in the asserts below.
#[test]
fn library_round_trip_matches_upstream() {
    let lib = read_json("reference/inproc/library.json");
    let mut diffs = Vec::new();
    for (name, rec) in lib.as_object().unwrap() {
        let (items, collections) = read_library(rec["source"].as_str().unwrap());
        let first = rec["export"]["firstBlankNodeId"].as_u64().unwrap();
        let t = Translator::ZoteroRdf;
        let port_text = t
            .export_with_collections(&items, &collections, &options(t, first))
            .map_err(|e| e.to_string());
        let up_text = rec["export"]["output"].as_str().unwrap();
        let pt = port_text.clone().unwrap_or_else(|e| format!("ERROR {e}"));
        diffs.extend(text_diff(up_text, &pt).into_iter().map(|(at, u, p)| Diff {
            test: "library/export".into(),
            case: name.clone(),
            at,
            upstream: u,
            port: p,
        }));
        diffs.extend(import_record_diffs(
            Translator::Rdf,
            up_text,
            &rec["import"],
            "library/import",
            name,
        ));

        // What survives, upstream and port alike (the comparison above makes
        // them equal; these pin the facts down).
        let imported = &rec["import"]["items"];
        let cols = &rec["import"]["collections"];
        assert_eq!(
            imported.as_array().unwrap().len(),
            7,
            "{name}: seven items back"
        );
        assert_eq!(
            cols.as_array().unwrap().len(),
            2,
            "{name}: two top-level collections back"
        );
        assert_eq!(
            cols[0]["children"][0]["type"], "collection",
            "{name}: nested collection kept"
        );
        let article = &imported[0];
        assert_eq!(
            article["notes"].as_array().unwrap().len(),
            1,
            "{name}: child note kept"
        );
        assert_eq!(
            article["attachments"].as_array().unwrap().len(),
            1,
            "{name}: attachment kept"
        );
        assert!(
            !article["seeAlso"].as_array().unwrap().is_empty(),
            "{name}: relation kept as seeAlso"
        );
    }
    assert_known("library", diffs);
}

/// The port's own chain (export with collections, then import) equals
/// importing upstream's export text, so a kovan library survives the trip
/// to Zotero RDF and back exactly as it would through Zotero.
#[test]
fn library_round_trip_port_chain_equals_upstream_chain() {
    let lib = read_json("reference/inproc/library.json");
    for (name, rec) in lib.as_object().unwrap() {
        let (items, collections) = read_library(rec["source"].as_str().unwrap());
        let t = Translator::ZoteroRdf;
        let first = rec["export"]["firstBlankNodeId"].as_u64().unwrap();
        let text = t
            .export_with_collections(&items, &collections, &options(t, first))
            .unwrap();
        let r = Translator::Rdf
            .import(
                &text,
                &options(
                    Translator::Rdf,
                    rec["import"]["firstBlankNodeId"].as_u64().unwrap(),
                ),
            )
            .unwrap();
        let api = fold_child_notes(&r.api_json_at("CURRENT_TIMESTAMP"));
        let up_api = fold_child_notes(rec["import"]["apiItems"].as_array().unwrap());
        assert_eq!(api, up_api, "{name}");
    }
}
