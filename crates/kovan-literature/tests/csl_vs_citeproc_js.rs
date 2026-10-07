//! Code-to-code verification of `kovan_literature::csl` (hayagriva) against
//! citeproc-js, Zotero's CSL engine (GitHub #789).
//!
//! **Methodology.** `scripts/csl-reference.sh` writes the Pages site's `.bib`
//! (`docs/site/references.bib`) as CSL-JSON exactly as this crate produces it
//! (Zotero BibTeX import, then `itemToCSLJSON`) to `tests/data/csl/items.json`,
//! and runs those items, `data/csl/apa.csl` and `data/csl/locales-en-US.xml`
//! through citeproc-js (pinned version in `reference_apa.json`'s `meta`) into
//! `tests/data/csl/reference_apa.json`. Both engines see one document holding
//! every item, so disambiguation works on the same set. For each item three
//! parts are compared: the parenthetical citation, the narrative citation
//! (citeproc-js: author-only + suppress-author, as Zotero composes it;
//! hayagriva: `CitePurpose::Prose`), and the bibliography entry; and the
//! bibliography order. The test first checks that this crate still produces
//! the CSL-JSON the reference was made from, so a stale reference cannot pass.
//!
//! **Normalisation (both sides, identical).** HTML to text: italic markup
//! (`<i>`, `<em>`, `font-style: italic`) becomes `*`, every other tag is
//! dropped (link text kept), entities are decoded, whitespace is collapsed.
//! Citations also drop the `*` (the port returns citations as plain text).
//! Nothing else is normalised.
//!
//! **Pass criterion.** Every difference must be listed exactly (key, part,
//! citeproc-js value, port value) in `tests/data/csl/known_differences.json`
//! with its reason; any other difference, or a listed one that no longer
//! occurs, fails. A listed difference is a recorded finding, never a
//! tolerance. Set `CSL_DUMP_DIFFERENCES=<file>` to write the current
//! differences in that file's format.
//!
//! **Results.** See the summary at the top of `known_differences.json` and
//! `#789`; the counts are asserted below so they cannot drift silently.

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

fn port_outputs(lib: &CslLibrary) -> (BTreeMap<String, [String; 3]>, Vec<String>) {
    let style = CslStyle::apa();
    let keys: Vec<String> = lib.keys().map(str::to_string).collect();
    let paren = lib.render(&style, &keys.iter().map(|k| (k.clone(), CiteForm::Parenthetical)).collect::<Vec<_>>());
    let narr = lib.render(&style, &keys.iter().map(|k| (k.clone(), CiteForm::Narrative)).collect::<Vec<_>>());
    let bib: BTreeMap<&str, &str> = paren.bibliography.iter().map(|(k, h)| (k.as_str(), h.as_str())).collect();
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
    (out, paren.bibliography.iter().map(|(k, _)| k.clone()).collect())
}

#[test]
fn the_reference_was_made_from_the_csl_json_this_crate_produces() {
    let lib = CslLibrary::from_bib(BIB).unwrap();
    let items: Vec<Value> = serde_json::from_str(ITEMS).unwrap();
    assert_eq!(items.len(), lib.len(), "re-run scripts/csl-reference.sh");
    for it in &items {
        let id = it["id"].as_str().unwrap();
        assert_eq!(Some(it), lib.csl_json(id), "{id}: CSL-JSON changed; re-run scripts/csl-reference.sh");
    }
}

#[test]
fn apa_matches_citeproc_js_except_the_recorded_differences() {
    let lib = CslLibrary::from_bib(BIB).unwrap();
    let reference: Value = serde_json::from_str(REFERENCE).unwrap();
    let (port, port_order) = port_outputs(&lib);

    let parts = ["parenthetical", "narrative", "bibliography"];
    let mut diffs: Vec<Value> = Vec::new();
    for (key, got) in &port {
        let r = &reference["items"][key];
        for (n, part) in parts.iter().enumerate() {
            let raw = r[*part].as_str().unwrap_or("");
            let want = if *part == "bibliography" { normalise(raw) } else { normalise(raw).replace('*', "") };
            if want != got[n] {
                diffs.push(json!({ "key": key, "part": part, "citeproc_js": want, "port": got[n] }));
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
        diffs.push(json!({ "key": "*", "part": "bibliography_order", "citeproc_js": ref_order.join(" "), "port": port_order.join(" ") }));
    }

    if let Ok(path) = std::env::var("CSL_DUMP_DIFFERENCES") {
        std::fs::write(&path, serde_json::to_string_pretty(&diffs).unwrap()).unwrap();
    }

    let known: Value = serde_json::from_str(KNOWN).unwrap();
    let listed: Vec<&Value> = known["differences"].as_array().unwrap().iter().collect();
    let strip = |v: &Value| json!({ "key": v["key"], "part": v["part"], "citeproc_js": v["citeproc_js"], "port": v["port"] });
    let listed_core: Vec<Value> = listed.iter().map(|v| strip(v)).collect();
    for l in &listed {
        assert!(
            l["reason"].as_str().is_some_and(|r| !r.trim().is_empty()),
            "a listed difference has no reason: {l}"
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

    // The measured result, pinned (see known_differences.json "summary").
    let total = port.len() * parts.len();
    let summary = &known["summary"];
    assert_eq!(summary["compared"].as_u64(), Some(total as u64));
    assert_eq!(summary["differences"].as_u64(), Some(diffs.len() as u64));
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
