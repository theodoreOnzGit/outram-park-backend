//! Identifier lookup fails gracefully (GitHub #756; maintainer, 2026-10-07:
//! "import web requests are okay, just make sure they fail gracefully via
//! result").
//!
//! **Methodology.** A fake fetch backend (no network) answers the lookup of
//! a DOI (which only DOI Content Negotiation handles, so one request is
//! made) with each failure kind, and the test checks the typed
//! [`LookupError`] that comes back: offline, connect/DNS, timeout, 404, 429
//! (with `Retry-After`), 500, a missing Content-Type, a truncated body, a
//! garbage body, and an unsupported target. A further test feeds arbitrary
//! bytes as the response to every search translator's first request and
//! checks that nothing panics. **Pass criterion:** the exact error variant;
//! no panic anywhere.

use kovan_literature::zotero::search::{
    FetchError, HttpRequest, HttpResponse, LookupError, LookupSession, SearchOptions, SearchStep,
    SearchTranslator,
};

const DOI: &str = "10.1109/TPS.1987.4316723";

fn lookup(
    answer: impl Fn(&HttpRequest) -> Result<HttpResponse, FetchError>,
) -> Result<(), LookupError> {
    let s = LookupSession::for_text(DOI, SearchOptions::default()).unwrap();
    s.run_with(|r| answer(r)).map(|_| ())
}

fn resp(status: u16, ct: Option<&str>, body: &[u8]) -> HttpResponse {
    HttpResponse {
        status,
        url: format!("https://doi.org/{DOI}"),
        content_type: ct.map(str::to_owned),
        retry_after: None,
        body: body.to_vec(),
    }
}

#[test]
fn offline() {
    assert_eq!(
        lookup(|_| Err(FetchError::Offline)),
        Err(LookupError::Offline)
    );
}

#[test]
fn connect_failure() {
    assert_eq!(
        lookup(|_| Err(FetchError::Connect("dns: no such host".into()))),
        Err(LookupError::Connect("dns: no such host".into()))
    );
}

#[test]
fn timeout() {
    assert_eq!(
        lookup(|_| Err(FetchError::Timeout)),
        Err(LookupError::Timeout)
    );
}

#[test]
fn not_found_404() {
    let e = lookup(|_| Ok(resp(404, Some("text/html"), b"Not Found"))).unwrap_err();
    assert!(matches!(e, LookupError::NotFound { .. }), "{e:?}");
}

#[test]
fn rate_limited_429() {
    let e = lookup(|_| {
        let mut r = resp(429, Some("text/plain"), b"slow down");
        r.retry_after = Some("120".into());
        Ok(r)
    })
    .unwrap_err();
    assert_eq!(
        e,
        LookupError::RateLimited {
            url: "https://doi.org/10.1109%2FTPS.1987.4316723".into(),
            retry_after: Some("120".into())
        }
    );
}

#[test]
fn server_error_500() {
    let e = lookup(|_| Ok(resp(500, Some("text/html"), b"oops"))).unwrap_err();
    assert_eq!(
        e,
        LookupError::HttpStatus {
            code: 500,
            url: "https://doi.org/10.1109%2FTPS.1987.4316723".into()
        }
    );
}

#[test]
fn missing_content_type() {
    let e = lookup(|_| Ok(resp(200, None, b"{}"))).unwrap_err();
    assert!(matches!(e, LookupError::Malformed(_)), "{e:?}");
}

/// A truncated Crossref Unixref XML answer: the Crossref Unixref XML
/// import cannot parse it, so the response is malformed.
#[test]
fn truncated_body() {
    let e = lookup(|_| {
        Ok(resp(
            200,
            Some("application/vnd.crossref.unixref+xml"),
            br#"<doi_records><doi_record><crossref><journal><journal_metadata><full_title>Bulk"#,
        ))
    })
    .unwrap_err();
    assert!(matches!(e, LookupError::Malformed(_)), "{e:?}");
}

/// Garbage that is not JSON to a translator that asks for JSON (ERIC's
/// `requestJSON`): malformed.
#[test]
fn garbage_body_to_json_request() {
    let mut search = kovan_literature::zotero::framework::JsObject::new();
    search.set("ericNumber", "EJ1125432");
    let s = LookupSession::forced(search, SearchTranslator::Eric, SearchOptions::default());
    let e = s
        .run_with(|r| {
            Ok(HttpResponse {
                status: 200,
                url: r.url.clone(),
                content_type: Some("application/json".into()),
                retry_after: None,
                body: b"\x00\xff<<<not json>>>\xfe".to_vec(),
            })
        })
        .unwrap_err();
    assert!(matches!(e, LookupError::Malformed(_)), "{e:?}");
}

/// Garbage to DOI Content Negotiation: upstream's CSL JSON import (the
/// fallback branch) swallows a JSON parse error and returns no items
/// (CSL JSON.js `parseInput` :48-54, `startImport` :104-113), so the
/// faithful outcome is "no record found", not a malformed-response error.
#[test]
fn garbage_body_to_doi_is_not_found_as_upstream() {
    let e = lookup(|_| {
        Ok(resp(
            200,
            Some("text/plain"),
            b"\x00\xff<<<not a record>>>\xfe",
        ))
    })
    .unwrap_err();
    assert!(matches!(e, LookupError::NotFound { .. }), "{e:?}");
}

#[test]
fn unsupported_target() {
    assert_eq!(
        lookup(|_| Err(FetchError::Unsupported("CORS".into()))),
        Err(LookupError::Unsupported("CORS".into()))
    );
}

#[test]
fn no_identifier() {
    assert_eq!(
        LookupSession::for_text("no identifiers here", SearchOptions::default()).unwrap_err(),
        LookupError::NoIdentifier
    );
}

/// Arbitrary bodies under every content type, to every translator's first
/// request: a typed result every time, never a panic.
#[test]
fn arbitrary_responses_never_panic() {
    let searches = [
        ("journalArticle", "DOI", DOI),
        ("book", "ISBN", "9780521779241"),
        ("journalArticle", "arXiv", "1706.03762"),
        (
            "journalArticle",
            "contextObject",
            "rft_id=info:pmid/31978945",
        ),
        ("journalArticle", "adsBibcode", "2022MSSP..16208070W"),
        ("journalArticle", "openAlex", "W2741809807"),
        ("journalArticle", "ericNumber", "EJ1125432"),
    ];
    let bodies: [&[u8]; 6] = [
        b"",
        b"\xff\xfe\x00garbage",
        b"{\"value\": [1, {\"x\": null}]",
        b"<?xml version=\"1.0\"?><zs:x xmlns:zs=\"http://www.loc.gov/zing/srw/\"><unclosed>",
        b"<html><body><div class='x'>",
        b"TY  - JOUR\nTI  - half",
    ];
    let types = [
        "text/plain",
        "application/json",
        "application/xml",
        "text/html",
        "application/x-research-info-systems",
    ];
    for t in SearchTranslator::ALL {
        for (it, k, v) in searches {
            let mut search = kovan_literature::zotero::framework::JsObject::new();
            search.set("itemType", it);
            search.set(k, v);
            for body in bodies {
                for ct in types {
                    let s = LookupSession::forced(search.clone(), t, SearchOptions::default());
                    // Answer every request (up to the session's bound) alike.
                    let _ = s.run_with(|r| {
                        Ok(HttpResponse {
                            status: 200,
                            url: r.url.clone(),
                            content_type: Some(ct.to_owned()),
                            retry_after: None,
                            body: body.to_vec(),
                        })
                    });
                }
            }
        }
    }
}

/// The step API a browser drives: nothing is fetched by the library.
#[test]
fn step_returns_the_request_instead_of_fetching() {
    let mut s = LookupSession::for_text(DOI, SearchOptions::default()).unwrap();
    let SearchStep::Pending(req) = s.step() else {
        panic!("expected a pending request");
    };
    assert_eq!(req.url, "https://doi.org/10.1109%2FTPS.1987.4316723");
    s.answer(&req, Err(FetchError::Offline));
    assert_eq!(s.step(), SearchStep::Done(Err(LookupError::Offline)));
}
