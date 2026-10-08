//! Code-to-code verification of `kovan_literature::csl` (the citeproc-js
//! port, GitHub #790) against citeproc-js itself, Zotero's CSL engine
//! (GitHub #789, #797).
//!
//! **Methodology.** `scripts/csl-reference.sh` writes the Pages site's `.bib`
//! (`docs/site/references.bib`) as CSL-JSON exactly as this crate produces it
//! (Zotero BibTeX import, then `itemToCSLJSON`) to `tests/data/csl/items.json`,
//! and runs those items and the five pinned styles of `data/csl/` (apa,
//! chicago-author-date, ieee, nature with `en-GB`, vancouver from
//! `nlm-citation-sequence.csl`) with the pinned locales through citeproc-js
//! 2.4.63 (`scripts/csl-reference.cjs`) into `tests/data/csl/reference_<style>.json`.
//! Both engines see one document holding every item (`updateItems` with all
//! ids), so disambiguation works on the same set. For each item three parts
//! are compared: the parenthetical citation (`makeCitationCluster([{id}])`),
//! the narrative citation (the author-only cluster, a space, the
//! suppress-author cluster, as Zotero composes it; the port drives the engine
//! the same way, [`CiteForm::Narrative`]) and the bibliography entry; plus the
//! bibliography order. The test first checks that this crate still produces
//! the CSL-JSON the reference was made from, so a stale reference cannot pass.
//!
//! **Normalisation (both sides, identical).** HTML to text: italic markup
//! (`<i>`, `<em>`, `font-style: italic`) becomes `*`, every other tag is
//! dropped (link text kept), entities are decoded, whitespace is collapsed.
//! Citations also drop the `*` (the port returns citations as plain text).
//! Nothing else is normalised.
//!
//! **Pass criterion.** Every difference must be listed exactly (style, key,
//! part, citeproc-js value, port value) in `tests/data/csl/known_differences.json`
//! with its reason and the registered deviation `Dnn`
//! (`src/citeproc/DEVIATIONS.md`) that causes it; any other difference, or a
//! listed one that no longer occurs, fails. A listed difference is a recorded
//! finding, never a tolerance. Set `CSL_DUMP_DIFFERENCES=<file>` to write the
//! current differences in that file's format.
//!
//! **Results (2026-10-08, #797).** 0 differences: 223 items x 3 parts x 5
//! styles = 3,345 comparisons and 5 bibliography orders all equal citeproc-js
//! 2.4.63 (`known_differences.json` is empty; no registered deviation affects
//! the site set). The test is capable of failing: altering one reference
//! value is reported as an unlisted difference.
//!
//! **History.** The engine under test was hayagriva 0.10.1 until 2026-10-08,
//! APA only: 176 of 669 comparisons differed plus the bibliography order
//! (96 missing given-name disambiguation, 27 stray comma `(2013,)`, 17 missing
//! year suffixes, 17 title-casing of container titles and `arXiv`, 8
//! apostrophes, 4 unclassified, 3 identifier-like numbers read as ranges
//! (`SECY-24-0035`), 2 `?.`, 1 case-sensitive sort, 1 particle placement).
//! That measurement is why citeproc-js was ported (#790); the list is in git
//! history (`known_differences.json` at commit `a4649599f`).

use std::collections::BTreeMap;

use kovan_literature::csl::{CiteForm, CslLibrary, CslStyle};
use serde_json::{json, Value};

const BIB: &str = include_str!("../../../docs/site/references.bib");
const ITEMS: &str = include_str!("data/csl/items.json");
const REFERENCE: &str = include_str!("data/csl/reference_apa.json");
const KNOWN: &str = include_str!("data/csl/known_differences.json");

/// HTML -> text with `*` around italics, entities decoded, spaces collapsed.
fn normalise(html: &str) -> String {
    let mut out = String::new();
    let mut stack: Vec<bool> = Vec::new();
    let mut rest = html;
    while let Some(lt) = rest.find('<') {
        out.push_str(&decode(&rest[..lt]));
        let Some(gt) = rest[lt..].find('>') else {
            out.push_str(&decode(&rest[lt..]));
            rest = "";
            break;
        };
        let tag = &rest[lt + 1..lt + gt];
        if let Some(_name) = tag.strip_prefix('/') {
            if stack.pop() == Some(true) {
                out.push('*');
            }
        } else if !tag.ends_with('/') {
            let lower = tag.to_lowercase();
            let italic = lower == "i"
                || lower == "em"
                || lower.starts_with("i ")
                || lower.contains("font-style: italic")
                || lower.contains("font-style:italic");
            if italic {
                out.push('*');
            }
            stack.push(italic);
        }
        rest = &rest[lt + gt + 1..];
    }
    out.push_str(&decode(rest));
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn decode(s: &str) -> String {
    s.replace("&#38;", "&")
        .replace("&amp;", "&")
        .replace("&#60;", "<")
        .replace("&lt;", "<")
        .replace("&#62;", ">")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#34;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", "\u{a0}")
}

fn port_outputs(
    lib: &CslLibrary,
    style: &CslStyle,
) -> (BTreeMap<String, [String; 3]>, Vec<String>) {
    let keys: Vec<String> = lib.keys().map(str::to_string).collect();
    let paren = lib
        .render(
            style,
            &keys
                .iter()
                .map(|k| (k.clone(), CiteForm::Parenthetical))
                .collect::<Vec<_>>(),
        )
        .unwrap();
    let narr = lib
        .render(
            style,
            &keys
                .iter()
                .map(|k| (k.clone(), CiteForm::Narrative))
                .collect::<Vec<_>>(),
        )
        .unwrap();
    let bib: BTreeMap<&str, &str> = paren
        .bibliography
        .iter()
        .map(|(k, h)| (k.as_str(), h.as_str()))
        .collect();
    let mut out = BTreeMap::new();
    for (i, k) in keys.iter().enumerate() {
        out.insert(
            k.clone(),
            [
                normalise(&paren.citations[i]).replace('*', ""),
                normalise(&narr.citations[i]).replace('*', ""),
                normalise(bib.get(k.as_str()).copied().unwrap_or("")),
            ],
        );
    }
    (
        out,
        paren.bibliography.iter().map(|(k, _)| k.clone()).collect(),
    )
}

/// The port's differences from one style's citeproc-js reference, as the
/// `known_differences.json` entries (with the style name).
fn differences(lib: &CslLibrary, style_name: &str, reference_text: &str) -> Vec<Value> {
    let style = CslStyle::bundled(style_name).unwrap();
    let reference: Value = serde_json::from_str(reference_text).unwrap();
    let (port, port_order) = port_outputs(lib, &style);
    let mut diffs: Vec<Value> = Vec::new();
    for (key, got) in &port {
        let r = &reference["items"][key];
        for (n, part) in PARTS.iter().enumerate() {
            let raw = r[*part].as_str().unwrap_or("");
            let want = if *part == "bibliography" {
                normalise(raw)
            } else {
                normalise(raw).replace('*', "")
            };
            if want != got[n] {
                diffs.push(json!({ "style": style_name, "key": key, "part": part, "citeproc_js": want, "port": got[n] }));
            }
        }
    }
    let ref_order: Vec<String> = reference["bibliography_order"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    if ref_order != port_order {
        diffs.push(json!({ "style": style_name, "key": "*", "part": "bibliography_order", "citeproc_js": ref_order.join(" "), "port": port_order.join(" ") }));
    }
    diffs
}

const PARTS: [&str; 3] = ["parenthetical", "narrative", "bibliography"];

/// Compare `styles` and require the differences to be exactly the listed
/// ones. Returns the number of item-level comparisons made.
fn check_styles(styles: &[(&str, &str)]) -> usize {
    let lib = CslLibrary::from_bib(BIB).unwrap();
    let mut diffs: Vec<Value> = Vec::new();
    for (name, text) in styles {
        diffs.extend(differences(&lib, name, text));
    }
    if let Ok(path) = std::env::var("CSL_DUMP_DIFFERENCES") {
        std::fs::write(&path, serde_json::to_string_pretty(&diffs).unwrap()).unwrap();
    }
    let known: Value = serde_json::from_str(KNOWN).unwrap();
    let wanted: Vec<&str> = styles.iter().map(|(n, _)| *n).collect();
    let listed: Vec<&Value> = known["differences"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|v| wanted.contains(&v["style"].as_str().unwrap_or("apa")))
        .collect();
    let strip = |v: &Value| json!({ "style": v["style"].as_str().unwrap_or("apa"), "key": v["key"], "part": v["part"], "citeproc_js": v["citeproc_js"], "port": v["port"] });
    let listed_core: Vec<Value> = listed.iter().map(|v| strip(v)).collect();
    for l in &listed {
        assert!(
            l["reason"].as_str().is_some_and(|r| !r.trim().is_empty())
                && l["deviation"].as_str().is_some(),
            "a listed difference has no reason and registered deviation Dnn: {l}"
        );
    }
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
    let compared = lib.len() * PARTS.len() * styles.len();
    compared
}

#[test]
fn the_reference_was_made_from_the_csl_json_this_crate_produces() {
    let lib = CslLibrary::from_bib(BIB).unwrap();
    let items: Vec<Value> = serde_json::from_str(ITEMS).unwrap();
    assert_eq!(items.len(), lib.len(), "re-run scripts/csl-reference.sh");
    for it in &items {
        let id = it["id"].as_str().unwrap();
        assert_eq!(
            Some(it),
            lib.csl_json(id),
            "{id}: CSL-JSON changed; re-run scripts/csl-reference.sh"
        );
    }
}

/// APA 7: the port against citeproc-js on every item (see the module docs).
#[test]
fn apa_matches_citeproc_js_except_the_recorded_differences() {
    let compared = check_styles(&[("apa", REFERENCE)]);
    assert_eq!(compared, 669);
}

#[test]
fn normalisation_is_the_same_for_both_engines_markup() {
    assert_eq!(
        normalise("<div class=\"csl-entry\">A, B., &#38; C. <i>J</i>, <i>5</i>. <a href=\"u\">u</a></div>"),
        "A, B., & C. *J*, *5*. u"
    );
    assert_eq!(
        normalise("A, B., & C. <span style=\"font-style: italic;\">J</span>, <span style=\"font-style: italic;\">5</span>. <a href=\"u\">u</a>"),
        "A, B., & C. *J*, *5*. u"
    );
}

/// The five site styles' citeproc-js references (GitHub #791): apa (compared
/// above), chicago-author-date, ieee, nature and vancouver (from
/// nlm-citation-sequence.csl, the parent of the dependent "Vancouver - NLM"
/// style). Every reference was made from the current `items.json`: same ids,
/// a parenthetical, a narrative and a bibliography part for each, and a
/// bibliography order covering every item. Only this structure is checked
/// here; the comparison of the port against the four non-APA references is
/// [`other_styles_match_citeproc_js_except_the_recorded_differences`].
#[test]
fn the_five_style_references_cover_every_item() {
    let items: Vec<Value> = serde_json::from_str(ITEMS).unwrap();
    let ids: Vec<&str> = items.iter().map(|i| i["id"].as_str().unwrap()).collect();
    let references = [
        ("apa", REFERENCE),
        (
            "chicago-author-date",
            include_str!("data/csl/reference_chicago-author-date.json"),
        ),
        ("ieee", include_str!("data/csl/reference_ieee.json")),
        ("nature", include_str!("data/csl/reference_nature.json")),
        (
            "vancouver",
            include_str!("data/csl/reference_vancouver.json"),
        ),
    ];
    for (style, text) in references {
        let reference: Value = serde_json::from_str(text).unwrap();
        assert_eq!(reference["meta"]["engine"], "citeproc-js 2.4.63", "{style}");
        let by_id = reference["items"].as_object().unwrap();
        assert_eq!(
            by_id.len(),
            ids.len(),
            "{style}: re-run scripts/csl-reference.sh"
        );
        for id in &ids {
            for part in ["parenthetical", "narrative", "bibliography"] {
                assert!(
                    by_id[*id][part].as_str().is_some_and(|s| !s.is_empty()),
                    "{style}: {id} has no {part}"
                );
            }
        }
        assert_eq!(
            reference["bibliography_order"].as_array().unwrap().len(),
            ids.len(),
            "{style}"
        );
    }
}

/// Port vs citeproc-js for chicago-author-date, ieee, nature and vancouver
/// (GitHub #797), same method and pass criterion as APA.
#[test]
fn other_styles_match_citeproc_js_except_the_recorded_differences() {
    let compared = check_styles(&[
        (
            "chicago-author-date",
            include_str!("data/csl/reference_chicago-author-date.json"),
        ),
        ("ieee", include_str!("data/csl/reference_ieee.json")),
        ("nature", include_str!("data/csl/reference_nature.json")),
        (
            "vancouver",
            include_str!("data/csl/reference_vancouver.json"),
        ),
    ]);
    assert_eq!(compared, 4 * 669);
}
