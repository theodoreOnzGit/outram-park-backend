//! Code-to-code verification of the identifier lookup (GitHub #756, epic
//! #747): `extractIdentifiers` and Zotero's search translators, against
//! upstream Zotero run on the SAME recorded HTTP responses.
//!
//! **Methodology.** `scripts/zotero-search-reference.mjs --record` ran
//! upstream's search (the translation-server's own code, in-process) once
//! against the live services for the cases in its `CASES` table, recording
//! every HTTP exchange raw (`tests/data/zotero/search/fixtures/<case>.json`;
//! 25 requests on 2026-10-07, listed in each fixture). Synthetic fixtures
//! (`fixtures/synthetic_*.json`) are kovan-authored responses for services
//! that were deliberately not contacted. The script's default (offline)
//! mode then re-ran upstream with every request answered from the fixture
//! and wrote what it returned (`reference/<case>.json`). Each test here runs
//! the port on the same fixture, answering requests the same way (a request
//! the fixture does not hold fails as a network error, as in the harness),
//! and compares exactly:
//!
//! * the requests made: method, URL and body, in order, and their headers
//!   (`User-Agent` aside) against the recorded ones;
//! * the outcome: success (and which translator's items were kept) or the
//!   failure upstream reported (501 "No items returned from any translator"
//!   / 500);
//! * the items, as `itemToAPIJSON` returns them, as JSON values. The two
//!   normalisations the harness applies to upstream (random keys renumbered
//!   to the port's key sequence; an `accessDate` stamped during the run
//!   becomes "NOW") are applied to the port's output the same way.
//!
//! **Pass criterion.** Exact equality. No tolerance and no list of accepted
//! differences: a difference means the port is wrong until shown otherwise.
//!
//! Results (2026-10-07): recorded in the hand-off of #756; see each case.

use kovan_common::zotero::date::DateOptions;
use kovan_literature::zotero::framework::{JsObject, TranslationEnv};
use kovan_literature::zotero::search::{
    extract_identifiers, FetchError, HttpRequest, HttpResponse, LookupSession, SearchOptions,
    SearchTranslator,
};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/zotero/search")
}

fn read_json(rel: &str) -> Value {
    let p = dir().join(rel);
    let s = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    serde_json::from_str(&s).unwrap()
}

/// A fixed "now" for the port; its ISO form is replaced by "NOW".
const NOW: i64 = 1_790_000_000;

fn options() -> SearchOptions {
    let manifest = read_json("reference/manifest.json");
    let offset = manifest["utcOffsetMinutes"].as_i64().unwrap_or(0) as i32;
    SearchOptions {
        env: TranslationEnv {
            dates: DateOptions {
                current_year: 2026,
                month_first: true,
                day_suffixes: Vec::new(),
                utc_offset_minutes: offset,
            },
            now_unix_secs: Some(NOW),
            ..TranslationEnv::default()
        },
        // Câmara Brasileira do Livro's api-key is not in the port's source
        // (DATA_POLICY.md: no API keys); the caller supplies it. A
        // placeholder here: its fixture is synthetic and request headers
        // are not compared for it.
        hidden_prefs: [(
            "CamaraBrasileiraDoLivro.apiKey".to_owned(),
            Value::String("kovan-test-placeholder".to_owned()),
        )]
        .into_iter()
        .collect(),
    }
}

/// The fixture's exchanges, answered as the harness answers them: by
/// method + URL + body, in order, the last answer reused.
struct Replay {
    queues: HashMap<String, Vec<(Value, Value)>>,
}

impl Replay {
    fn new(fixture: &Value) -> Replay {
        let mut queues: HashMap<String, Vec<(Value, Value)>> = HashMap::new();
        for ex in fixture["exchanges"].as_array().unwrap() {
            let r = &ex["request"];
            let key = format!(
                "{} {}\n{}",
                r["method"].as_str().unwrap(),
                r["url"].as_str().unwrap(),
                r["body"].as_str().unwrap_or("")
            );
            queues
                .entry(key)
                .or_default()
                .push((r.clone(), ex["response"].clone()));
        }
        Replay { queues }
    }

    fn fetch(&mut self, req: &HttpRequest) -> Result<HttpResponse, FetchError> {
        let Some(q) = self.queues.get_mut(&req.key()) else {
            return Err(FetchError::Connect(format!(
                "kovan replay: no recorded response for {} {}",
                req.method, req.url
            )));
        };
        let (recorded_req, resp) = if q.len() > 1 {
            q.remove(0)
        } else {
            q[0].clone()
        };
        // The port must send the headers Zotero sent (a synthetic fixture
        // may leave `headers` out; then they are not checked).
        if recorded_req.get("headers").is_none() {
            return self.answer(&resp);
        }
        let mut want: Vec<(String, String)> = recorded_req["headers"]
            .as_object()
            .map(|m| {
                m.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_owned()))
                    .collect()
            })
            .unwrap_or_default();
        let mut got = req.headers.clone();
        want.sort();
        got.sort();
        assert_eq!(got, want, "request headers for {}", req.url);
        self.answer(&resp)
    }

    fn answer(&self, resp: &Value) -> Result<HttpResponse, FetchError> {
        if let Some(e) = resp["error"].as_str() {
            return Err(FetchError::Connect(e.to_owned()));
        }
        let body = match resp["bodyEncoding"].as_str() {
            Some("base64") => base64(resp["body"].as_str().unwrap()),
            _ => resp["body"].as_str().unwrap_or("").as_bytes().to_vec(),
        };
        Ok(HttpResponse {
            status: resp["status"].as_u64().unwrap() as u16,
            url: resp["url"].as_str().unwrap().to_owned(),
            content_type: resp["headers"]["content-type"].as_str().map(str::to_owned),
            retry_after: None,
            body,
        })
    }
}

fn base64(s: &str) -> Vec<u8> {
    let val = |c: u8| -> u32 {
        match c {
            b'A'..=b'Z' => u32::from(c - b'A'),
            b'a'..=b'z' => u32::from(c - b'a') + 26,
            b'0'..=b'9' => u32::from(c - b'0') + 52,
            b'+' => 62,
            _ => 63,
        }
    };
    let bytes: Vec<u8> = s.bytes().filter(|&c| c != b'=').collect();
    let mut out = Vec::new();
    for chunk in bytes.chunks(4) {
        let mut n = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            n |= val(c) << (18 - 6 * i);
        }
        for i in 0..chunk.len().saturating_sub(1) {
            out.push((n >> (16 - 8 * i)) as u8);
        }
    }
    out
}

fn iso(secs: i64) -> String {
    kovan_literature::zotero::search::context::epoch_to_iso(secs)
}

/// Run the port on a reference case; compare with upstream.
fn check_case(name: &str) {
    let reference = read_json(&format!("reference/{name}.json"));
    let fixture = read_json(&format!("fixtures/{name}.json"));
    let mut replay = Replay::new(&fixture);
    let opts = options();
    let session = match reference["kind"].as_str().unwrap() {
        "identifier" => {
            let text = reference["text"].as_str().unwrap();
            // The identifiers upstream extracted.
            let ids: Vec<Value> = extract_identifiers(text)
                .iter()
                .map(|i| i.to_value())
                .collect();
            assert_eq!(
                Value::Array(ids),
                reference["identifiers"],
                "{name}: identifiers"
            );
            let s = LookupSession::for_text(text, opts).expect("identifier");
            let detected: Vec<Value> = s
                .translators
                .iter()
                .map(|t| Value::String(t.metadata().id.to_owned()))
                .collect();
            assert_eq!(
                Value::Array(detected),
                reference["detected"],
                "{name}: detected translators"
            );
            s
        }
        _ => {
            let t = SearchTranslator::from_id(reference["translator"].as_str().unwrap())
                .expect("translator ported");
            let search: JsObject = serde_json::from_value(reference["search"].clone()).unwrap();
            LookupSession::forced(search, t, opts)
        }
    };
    // Requests made, recorded by wrapping the replay.
    let mut made: Vec<Value> = Vec::new();
    let result = session.clone().run_with(|req| {
        made.push(serde_json::json!({
            "method": req.method, "url": req.url,
            "body": req.body.clone().map_or(Value::Null, Value::String),
        }));
        replay.fetch(req)
    });
    assert_eq!(
        Value::Array(made),
        reference["requests"],
        "{name}: requests"
    );
    let status = reference["status"].as_u64().unwrap();
    match result {
        Ok(run) => {
            assert_eq!(
                status, 200,
                "{name}: upstream failed ({}), port succeeded",
                reference["error"]
            );
            assert_eq!(
                run.used.metadata().id,
                reference["translatorUsed"].as_str().unwrap(),
                "{name}: translator used"
            );
            let now = iso(NOW);
            let mut items = run.api_json(&now);
            for it in &mut items {
                if it.get("accessDate").and_then(Value::as_str) == Some(now.as_str()) {
                    it["accessDate"] = Value::String("NOW".to_owned());
                }
            }
            let want = reference["items"].as_array().unwrap();
            for (i, (g, w)) in items.iter().zip(want.iter()).enumerate() {
                assert_eq!(g, w, "{name}: item {i}");
            }
            assert_eq!(items.len(), want.len(), "{name}: item count");
        }
        Err(e) => {
            assert_ne!(status, 200, "{name}: port failed ({e}), upstream succeeded");
        }
    }
}

macro_rules! cases {
    ($($name:ident),* $(,)?) => {
        $(
            #[test]
            fn $name() {
                check_case(stringify!($name));
            }
        )*
        const CASES: &[&str] = &[$(stringify!($name)),*];
    };
}

cases!(
    doi_crossref,
    doi_datacite,
    doi_other_ra,
    arxiv_new,
    arxiv_old,
    arxiv_with_doi,
    isbn_loc,
    pmid,
    eidr,
    crossref_rest_article,
    crossref_rest_book,
    crossref_rest_preprint,
    bnf_isbn,
    k10plus_isbn,
    nlp_isbn,
    libris_isbn,
    who_isbn,
    eric_journal,
    eric_report,
    openalex,
    // Synthetic (kovan-authored responses; services not contacted).
    synthetic_ads_bibcode,
    synthetic_camara_isbn,
    synthetic_libris_isbn,
    synthetic_who_isbn,
    synthetic_open_worldcat,
    synthetic_medra,
    synthetic_lulu,
);

/// Every reference case has a test above (a new fixture cannot go
/// unchecked).
#[test]
fn every_reference_case_is_tested() {
    for e in std::fs::read_dir(dir().join("reference")).unwrap() {
        let n = e.unwrap().file_name().into_string().unwrap();
        let Some(stem) = n.strip_suffix(".json") else {
            continue;
        };
        if stem == "manifest" || stem == "extract_identifiers" {
            continue;
        }
        assert!(CASES.contains(&stem), "reference/{n} has no test");
    }
}

/// `extractIdentifiers` on `extract_identifiers_inputs.json` (upstream
/// tests' inputs and kovan probes) against upstream's own utilities.js.
#[test]
fn extract_identifiers_matches_upstream() {
    let r = read_json("reference/extract_identifiers.json");
    for pair in r.as_array().unwrap() {
        let input = pair[0].as_str().unwrap();
        let got: Vec<Value> = extract_identifiers(input)
            .iter()
            .map(|i| i.to_value())
            .collect();
        assert_eq!(Value::Array(got), pair[1], "extractIdentifiers({input:?})");
    }
}
