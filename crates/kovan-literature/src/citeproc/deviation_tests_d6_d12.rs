// Part of the kovan port of citeproc-js (GitHub #790, #808). Tests only.

//! Registered deviations D6 to D12 (`DEVIATIONS.md`, GitHub #808), pinned end
//! to end through the public [`Engine`].
//!
//! **Method.** `scripts/csl-units/deviations_d6_d12.cjs` builds one minimal
//! probe per case (a style, a few items, the language preferences) and records
//! what citeproc-js 2.4.63 gives for it (`results[].citeproc_js`, a value `v`
//! or an error `e`), next to the output the port is *meant* to give
//! (`intended`, written by hand in that script from the measurements in the
//! issue comments of #808). Regenerate with
//! `CITEPROC_MODULE=<repo>/target/csl-reference/node_modules/citeproc/citeproc_commonjs.js
//! node scripts/csl-units/deviations_d6_d12.cjs`
//! (output `tests/data/csl/units/deviations_d6_d12.json`).
//!
//! **Pass criteria.** For every probe the port renders exactly `intended`, and
//! citeproc-js did **not** (a value that differs, or an error): the second
//! half keeps the table honest, because a probe where both agree would not be a
//! deviation. D12's `intended` is the measurement agent's best judgement (no
//! spec text, no fixture).
//!
//! **Results (2026-10-08, citeproc-js 2.4.63):** 11 probes (D6 1, D7 2, D8 1,
//! D9 2, D10 1, D11 3, D12 1); the port renders the intended output for all 11,
//! and citeproc-js differs from it on all 11: 5 with other text (D6 `... Appeals
//! Court 最`, D7 `短 Gendai Long` twice, D8 `The Title`, D10 `Al Smith and Ed
//! Zed, Org One`) and 6 by throwing (D9 twice, D11 three times, D12). D12 uses
//! the year 1700, not the issue's 44: the port reads the newer locales
//! (`data/csl/locales-en-US.xml`, `44 AD`) and the script the pinned ones
//! (`44AD`), see DEVIATIONS.md C4.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{json, Value};

use crate::citeproc::test_support::minimal_locales;
use crate::citeproc::{Abbreviations, CitationItem, Engine, EngineError, Sys};

const REFERENCE: &str = include_str!("../../tests/data/csl/units/deviations_d6_d12.json");

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| a.iter().map(|s| s.as_str().unwrap_or("").to_string()).collect())
        .unwrap_or_default()
}

fn abbreviations(raw: &Value) -> Abbreviations {
    let mut out = Abbreviations::new();
    for (j, segs) in raw.as_object().into_iter().flatten() {
        for (seg, keys) in segs.as_object().into_iter().flatten() {
            for (k, v) in keys.as_object().into_iter().flatten() {
                out.entry(j.clone())
                    .or_default()
                    .entry(seg.clone())
                    .or_default()
                    .insert(k.clone(), v.as_str().unwrap_or("").to_string());
            }
        }
    }
    out
}

fn engine(p: &Value) -> Result<Engine, EngineError> {
    let items: Vec<Value> = p["items"].as_array().cloned().unwrap_or_default();
    let mut sys = Sys::new(&items, Arc::new(minimal_locales()))?;
    if p.get("abbrevs").is_some() {
        sys.abbreviations = abbreviations(&p["abbrevs"]);
    }
    let mut e = Engine::new(sys, p["style"].as_str().unwrap_or(""), "en-US")?;
    let mut prefs: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (k, v) in [
        ("persons", vec!["translit"]),
        ("institutions", vec!["translit"]),
        ("titles", vec!["translit", "translat"]),
        ("journals", vec!["translit"]),
        ("publishers", vec!["translat"]),
        ("places", vec!["translat"]),
    ] {
        prefs.insert(k.into(), v.into_iter().map(str::to_string).collect());
    }
    for (k, v) in p["prefs"].as_object().into_iter().flatten() {
        prefs.insert(k.clone(), strings(v));
    }
    e.set_lang_prefs_for_cites(prefs);
    let tags = if p.get("translation").is_some() {
        strings(&p["translation"])
    } else {
        vec!["en".to_string()]
    };
    e.set_lang_tags_for_csl_translation(tags);
    if p["spoof"].as_bool() == Some(true) {
        e.set_development_extension("spoof_institutional_affiliations", json!(true));
    }
    Ok(e)
}

fn run(p: &Value, op: &str, ids: &[String]) -> Result<String, EngineError> {
    let mut e = engine(p)?;
    match op {
        "cite" => {
            let items: Vec<CitationItem> = ids
                .iter()
                .map(|id| CitationItem::from_json(&json!({ "id": id })))
                .collect::<Result<_, _>>()?;
            e.make_citation_cluster(&items)
        }
        _ => {
            e.update_items(ids, false)?;
            Ok(e.make_bibliography(None)?.joined())
        }
    }
}

#[test]
fn registered_deviations_d6_to_d12_give_the_intended_output() {
    let r: Value = serde_json::from_str(REFERENCE).expect("deviations_d6_d12.json");
    let probes = r["probes"].as_array().expect("probes");
    assert_eq!(probes.len(), 11);
    let mut bad: Vec<String> = Vec::new();
    for p in probes {
        let tag = format!("{} {}", p["dev"], p["name"]);
        let intended = p["intended"].as_str().expect("intended");
        for res in p["results"].as_array().expect("results") {
            let op = res["op"].as_str().expect("op");
            let ids = strings(&res["ids"]);
            // citeproc-js: a different text, or an error; never the intended text.
            let js = &res["citeproc_js"];
            if js["v"].as_str() == Some(intended) {
                bad.push(format!("{tag}: citeproc-js already gives the intended output"));
            }
            if js.get("v").is_none() && js.get("e").is_none() {
                bad.push(format!("{tag}: no citeproc-js result recorded"));
            }
            match run(p, op, &ids) {
                Ok(got) if got == intended => {}
                other => bad.push(format!(
                    "{tag}\n    want: {intended:?}\n    got:  {other:?}\n    citeproc-js: {js}"
                )),
            }
        }
    }
    assert!(bad.is_empty(), "{} probes differ:\n{}", bad.len(), bad.join("\n"));
}
