// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: the Zotero translation-server, https://github.com/zotero/
//   translation-server (commit 3a9d17614896): src/searchEndpoint.js (the
//   identifier search: extractIdentifiers, the PMID rule, every detected
//   translator, itemToAPIJSON), src/translators.js :76-86 (translators
//   sorted by priority, ties in directory order); Zotero translate (commit
//   dd524aea9a55) src/translation/translate.js `Zotero.Translate.Search`
//   :2713-2850 (`getTranslators(true)`: every translator whose
//   `detectSearch` is truthy; `complete`: the next translator when one
//   returns no items), `_detectTranslatorsCollected` :1772-1789.
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! Zotero's "Add Item by Identifier" (GitHub #756): find a DOI, ISBN,
//! arXiv ID, ADS bibcode or PubMed ID in text ([`extract_identifiers`]) and
//! fetch its record with Zotero's search translators ([`SearchTranslator`]).
//!
//! **This module never touches the network.** A search runs until it needs
//! an HTTP answer it does not have, and then stops and returns the request
//! ([`SearchStep::Pending`]); the caller fetches it however its target can
//! (kovan's native `ureq` backend, a browser `fetch`, a recorded fixture, a
//! fake in a test), hands the answer back ([`LookupSession::answer`]) and
//! steps again. The translators are deterministic given their answers, so
//! re-running from the start is exact; a lookup makes a handful of
//! requests, so the repeated work is negligible. Synchronous callers use
//! [`LookupSession::run_with`] with a closure.
//!
//! ```text
//! text --extract_identifiers--> Identifier --setIdentifier--> search item
//!   --detectSearch on every SearchTranslator (priority order)--> candidates
//!   --doSearch on the first; none found -> the next--> items
//!   --Search/Web/Base _itemDone--> itemToAPIJSON --> ZoteroItem
//! ```
//!
//! Every failure is a [`LookupError`] value: offline, connect/DNS, timeout,
//! HTTP status, rate limiting (429 with `Retry-After`), malformed response,
//! not found, unsupported on this target. Nothing panics on network data.
//!
//! Verified code-to-code against upstream run in-process on the same
//! recorded responses: `tests/zotero_search.rs`, fixtures from
//! `scripts/zotero-search-reference.mjs`.

pub mod context;
pub mod extract;
pub mod http;
pub mod translators;

pub use context::{ChildSearch, SearchContext, SearchError, SearchOptions};
pub use extract::{extract_identifiers, identifier_for_search, Identifier};
pub use http::{FetchError, HttpCache, HttpFailure, HttpRequest, HttpResponse, RequestOptions};

use super::framework::api_json::{item_to_api_json, KeyGenerator};
use super::framework::item::{JsObject, TranslatorItem};
use super::framework::options::TranslatorMetadata;
use kovan_common::zotero::{ZoteroItem, ZoteroJsonError};
use serde_json::Value;
use translators as t;

/// A ported Zotero search translator (translatorType 8, or 12 with a
/// search half). No trait objects: one variant per translator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SearchTranslator {
    /// `EIDR.js`.
    Eidr,
    /// `Crossref REST.js`.
    CrossrefRest,
    /// `WHO.js`.
    Who,
    /// `Library of Congress ISBN.js`.
    LibraryOfCongressIsbn,
    /// `BnF ISBN.js`.
    BnfIsbn,
    /// `Camara Brasileira do Livro ISBN.js`.
    CamaraBrasileiraDoLivroIsbn,
    /// `LIBRIS ISBN.js`.
    LibrisIsbn,
    /// `National Library of Poland ISBN.js`.
    NationalLibraryOfPolandIsbn,
    /// `Gemeinsamer Bibliotheksverbund ISBN.js`.
    GemeinsamerBibliotheksverbundIsbn,
    /// `K10plus ISBN.js`.
    K10plusIsbn,
    /// `ADS Bibcode.js`.
    AdsBibcode,
    /// `DOI Content Negotiation.js`.
    DoiContentNegotiation,
    /// `ERIC.js`.
    Eric,
    /// `Open WorldCat.js`.
    OpenWorldCat,
    /// `OpenAlex.js`.
    OpenAlex,
    /// `PubMed.js`.
    PubMed,
    /// `arXiv.org.js`.
    ArXiv,
    /// `Lulu.js`.
    Lulu,
    /// `mEDRA.js`.
    Medra,
}

impl SearchTranslator {
    /// Every search translator in the order the translation-server tries
    /// them: by `priority`, ties in file-name byte order (translators.js
    /// sorts the directory listing stably by priority).
    pub const ALL: [SearchTranslator; 19] = [
        SearchTranslator::Eidr,                              // 80
        SearchTranslator::CrossrefRest,                      // 90
        SearchTranslator::Who,                               // 96
        SearchTranslator::LibraryOfCongressIsbn,             // 97
        SearchTranslator::BnfIsbn,                           // 98
        SearchTranslator::CamaraBrasileiraDoLivroIsbn,       // 98
        SearchTranslator::LibrisIsbn,                        // 98
        SearchTranslator::NationalLibraryOfPolandIsbn,       // 98
        SearchTranslator::GemeinsamerBibliotheksverbundIsbn, // 99
        SearchTranslator::K10plusIsbn,                       // 99
        SearchTranslator::AdsBibcode,                        // 100
        SearchTranslator::DoiContentNegotiation,             // 100
        SearchTranslator::Eric,                              // 100
        SearchTranslator::OpenWorldCat,                      // 100
        SearchTranslator::OpenAlex,                          // 100
        SearchTranslator::PubMed,                            // 100
        SearchTranslator::ArXiv,                             // 100
        SearchTranslator::Lulu,                              // 101
        SearchTranslator::Medra,                             // 105
    ];

    /// The translator's header.
    pub fn metadata(self) -> &'static TranslatorMetadata {
        match self {
            SearchTranslator::Eidr => &t::eidr::METADATA,
            SearchTranslator::CrossrefRest => &t::crossref_rest::METADATA,
            SearchTranslator::Who => &t::who::METADATA,
            SearchTranslator::LibraryOfCongressIsbn => &t::loc_isbn::METADATA,
            SearchTranslator::BnfIsbn => &t::bnf_isbn::METADATA,
            SearchTranslator::CamaraBrasileiraDoLivroIsbn => &t::camara_isbn::METADATA,
            SearchTranslator::LibrisIsbn => &t::libris_isbn::METADATA,
            SearchTranslator::NationalLibraryOfPolandIsbn => &t::nlp_isbn::METADATA,
            SearchTranslator::GemeinsamerBibliotheksverbundIsbn => &t::gbv_isbn::METADATA,
            SearchTranslator::K10plusIsbn => &t::k10plus_isbn::METADATA,
            SearchTranslator::AdsBibcode => &t::ads_bibcode::METADATA,
            SearchTranslator::DoiContentNegotiation => &t::doi_content_negotiation::METADATA,
            SearchTranslator::Eric => &t::eric::METADATA,
            SearchTranslator::OpenWorldCat => &t::open_worldcat::METADATA,
            SearchTranslator::OpenAlex => &t::openalex::METADATA,
            SearchTranslator::PubMed => &t::pubmed::METADATA,
            SearchTranslator::ArXiv => &t::arxiv::METADATA,
            SearchTranslator::Lulu => &t::lulu::METADATA,
            SearchTranslator::Medra => &t::medra::METADATA,
        }
    }

    /// The translator with this `translatorID`.
    pub fn from_id(id: &str) -> Option<SearchTranslator> {
        SearchTranslator::ALL
            .into_iter()
            .find(|t| t.metadata().id == id)
    }

    /// `detectSearch(search)`.
    pub fn detect(self, search: &JsObject) -> bool {
        match self {
            SearchTranslator::Eidr => t::eidr::detect_search(search),
            SearchTranslator::CrossrefRest => t::crossref_rest::detect_search(search),
            SearchTranslator::Who => t::who::detect_search(search),
            SearchTranslator::LibraryOfCongressIsbn => t::loc_isbn::detect_search(search),
            SearchTranslator::BnfIsbn => t::bnf_isbn::detect_search(search),
            SearchTranslator::CamaraBrasileiraDoLivroIsbn => t::camara_isbn::detect_search(search),
            SearchTranslator::LibrisIsbn => t::libris_isbn::detect_search(search),
            SearchTranslator::NationalLibraryOfPolandIsbn => t::nlp_isbn::detect_search(search),
            SearchTranslator::GemeinsamerBibliotheksverbundIsbn => {
                t::gbv_isbn::detect_search(search)
            }
            SearchTranslator::K10plusIsbn => t::k10plus_isbn::detect_search(search),
            SearchTranslator::AdsBibcode => t::ads_bibcode::detect_search(search),
            SearchTranslator::DoiContentNegotiation => {
                t::doi_content_negotiation::detect_search(search)
            }
            SearchTranslator::Eric => t::eric::detect_search(search),
            SearchTranslator::OpenWorldCat => t::open_worldcat::detect_search(search),
            SearchTranslator::OpenAlex => t::openalex::detect_search(search),
            SearchTranslator::PubMed => t::pubmed::detect_search(search),
            SearchTranslator::ArXiv => t::arxiv::detect_search(search),
            SearchTranslator::Lulu => t::lulu::detect_search(search),
            SearchTranslator::Medra => t::medra::detect_search(search),
        }
    }

    /// `doSearch(search)`, its items going through `ctx.complete`.
    pub fn do_search(self, ctx: &mut SearchContext, search: &JsObject) -> Result<(), SearchError> {
        match self {
            SearchTranslator::Eidr => t::eidr::do_search(ctx, search),
            SearchTranslator::CrossrefRest => t::crossref_rest::do_search(ctx, search),
            SearchTranslator::Who => t::who::do_search(ctx, search),
            SearchTranslator::LibraryOfCongressIsbn => t::loc_isbn::do_search(ctx, search),
            SearchTranslator::BnfIsbn => t::bnf_isbn::do_search(ctx, search),
            SearchTranslator::CamaraBrasileiraDoLivroIsbn => t::camara_isbn::do_search(ctx, search),
            SearchTranslator::LibrisIsbn => t::libris_isbn::do_search(ctx, search),
            SearchTranslator::NationalLibraryOfPolandIsbn => t::nlp_isbn::do_search(ctx, search),
            SearchTranslator::GemeinsamerBibliotheksverbundIsbn => {
                t::gbv_isbn::do_search(ctx, search)
            }
            SearchTranslator::K10plusIsbn => t::k10plus_isbn::do_search(ctx, search),
            SearchTranslator::AdsBibcode => t::ads_bibcode::do_search(ctx, search),
            SearchTranslator::DoiContentNegotiation => {
                t::doi_content_negotiation::do_search(ctx, search)
            }
            SearchTranslator::Eric => t::eric::do_search(ctx, search),
            SearchTranslator::OpenWorldCat => t::open_worldcat::do_search(ctx, search),
            SearchTranslator::OpenAlex => t::openalex::do_search(ctx, search),
            SearchTranslator::PubMed => t::pubmed::do_search(ctx, search),
            SearchTranslator::ArXiv => t::arxiv::do_search(ctx, search),
            SearchTranslator::Lulu => t::lulu::do_search(ctx, search),
            SearchTranslator::Medra => t::medra::do_search(ctx, search),
        }
    }
}

/// `getTranslators()` of a search: every translator whose `detectSearch`
/// is truthy, in priority order.
pub fn detect_translators(search: &JsObject) -> Vec<SearchTranslator> {
    SearchTranslator::ALL
        .into_iter()
        .filter(|t| t.detect(search))
        .collect()
}

/// Why a lookup failed. Every network path ends in one of these; none
/// panics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LookupError {
    /// No identifier in the text (upstream would run a text search, which
    /// kovan does not).
    NoIdentifier,
    /// No translator handles this identifier (upstream 501 "No translators
    /// available").
    NoTranslator,
    /// Every translator answered and none found a record (upstream 501 "No
    /// items returned from any translator"; an HTTP 404 counts as "no
    /// record").
    NotFound {
        /// Each translator tried, with what happened.
        attempts: Vec<(String, String)>,
    },
    /// No network.
    Offline,
    /// DNS or connection failure.
    Connect(String),
    /// A timeout.
    Timeout,
    /// An HTTP status that is not success.
    HttpStatus {
        /// The code.
        code: u16,
        /// The URL.
        url: String,
    },
    /// HTTP 429.
    RateLimited {
        /// The URL.
        url: String,
        /// `Retry-After`, when sent.
        retry_after: Option<String>,
    },
    /// The response could not be read (bad JSON/XML, unexpected shape,
    /// unsupported content type, too large).
    Malformed(String),
    /// Not possible on this target (e.g. a browser's CORS refusal, a build
    /// without a network backend).
    Unsupported(String),
    /// A translator refused the input or failed for its own reasons.
    Translator(String),
}

impl std::fmt::Display for LookupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LookupError::NoIdentifier => {
                write!(f, "no DOI, ISBN, arXiv ID, ADS bibcode or PMID found")
            }
            LookupError::NoTranslator => write!(f, "no lookup service handles this identifier"),
            LookupError::NotFound { attempts } => {
                write!(f, "no record found")?;
                for (t, why) in attempts {
                    write!(f, "; {t}: {why}")?;
                }
                Ok(())
            }
            LookupError::Offline => write!(f, "offline (no network)"),
            LookupError::Connect(m) => write!(f, "could not connect: {m}"),
            LookupError::Timeout => write!(f, "timed out"),
            LookupError::HttpStatus { code, url } => write!(f, "HTTP {code} from {url}"),
            LookupError::RateLimited { url, retry_after } => match retry_after {
                Some(r) => write!(f, "rate-limited by {url}; retry after {r}"),
                None => write!(f, "rate-limited by {url}"),
            },
            LookupError::Malformed(m) => write!(f, "unexpected response: {m}"),
            LookupError::Unsupported(m) => write!(f, "not available on this target: {m}"),
            LookupError::Translator(m) => write!(f, "lookup failed: {m}"),
        }
    }
}

impl std::error::Error for LookupError {}

impl LookupError {
    fn from_search(e: &SearchError) -> LookupError {
        match e {
            SearchError::Pending(r) => {
                LookupError::Translator(format!("unanswered request {}", r.url))
            }
            SearchError::Translator(m) => LookupError::Translator(m.clone()),
            SearchError::Http(h) => match h {
                HttpFailure::Fetch(f) => match f {
                    FetchError::Offline => LookupError::Offline,
                    FetchError::Connect(m) | FetchError::Other(m) => {
                        LookupError::Connect(m.clone())
                    }
                    FetchError::Timeout => LookupError::Timeout,
                    FetchError::TooLarge => LookupError::Malformed("response too large".to_owned()),
                    FetchError::Unsupported(m) => LookupError::Unsupported(m.clone()),
                },
                HttpFailure::Status { code, url } => LookupError::HttpStatus {
                    code: *code,
                    url: url.clone(),
                },
                HttpFailure::RateLimited { url, retry_after } => LookupError::RateLimited {
                    url: url.clone(),
                    retry_after: retry_after.clone(),
                },
                HttpFailure::UnsupportedFormat(m) | HttpFailure::Malformed(m) => {
                    LookupError::Malformed(m.clone())
                }
                HttpFailure::BadUrl(m) => LookupError::Translator(m.clone()),
            },
        }
    }
}

/// What one translator did in a search.
#[derive(Debug, Clone, PartialEq)]
pub struct Attempt {
    /// The translator.
    pub translator: SearchTranslator,
    /// `None`: it returned no items; `Some`: it failed.
    pub error: Option<SearchError>,
}

/// A finished search.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchRun {
    /// The translators tried, in order.
    pub translators: Vec<SearchTranslator>,
    /// The one whose items were kept.
    pub used: SearchTranslator,
    /// Its items (after `_itemDone`, translator format).
    pub items: Vec<TranslatorItem>,
    /// The translators that found nothing before it.
    pub attempts: Vec<Attempt>,
    /// Every request made, in order.
    pub requests: Vec<HttpRequest>,
}

impl SearchRun {
    /// The items as the translation-server's `/search` returns them
    /// (`itemToAPIJSON` of each; notes follow their parent as child items).
    pub fn api_json(&self, now_iso: &str) -> Vec<Value> {
        let mut keys = KeyGenerator::new(0);
        self.items
            .iter()
            .flat_map(|i| item_to_api_json(i, &mut keys, now_iso))
            .collect()
    }

    /// The items as kovan-common [`ZoteroItem`]s.
    pub fn zotero_items(&self, now_iso: &str) -> Result<Vec<ZoteroItem>, ZoteroJsonError> {
        self.api_json(now_iso)
            .iter()
            .map(ZoteroItem::from_json_value)
            .collect()
    }
}

/// One step of a search.
#[derive(Debug, Clone, PartialEq)]
pub enum SearchStep {
    /// Finished.
    Done(Result<SearchRun, LookupError>),
    /// Needs this request answered first.
    Pending(HttpRequest),
}

/// Run a search from the start with the answers in `cache`
/// (`translate.translate()` with `Search#complete`'s fallback).
pub fn run_search(
    search: &JsObject,
    translators: &[SearchTranslator],
    cache: HttpCache,
    options: &SearchOptions,
) -> SearchStep {
    if translators.is_empty() {
        return SearchStep::Done(Err(LookupError::NoTranslator));
    }
    let mut ctx = SearchContext::new(cache, options.clone());
    let mut attempts = Vec::new();
    for &tr in translators {
        let (items, r) = ctx.run_frame(tr, search, false);
        match r {
            Err(SearchError::Pending(req)) => return SearchStep::Pending(req),
            Ok(()) if !items.is_empty() => {
                return SearchStep::Done(Ok(SearchRun {
                    translators: translators.to_vec(),
                    used: tr,
                    items,
                    attempts,
                    requests: ctx.requests().to_vec(),
                }));
            }
            // An error after items were saved fails the whole search
            // (Search#complete falls back only when newItems is empty).
            Err(e) if !items.is_empty() => {
                return SearchStep::Done(Err(LookupError::from_search(&e)));
            }
            Ok(()) => attempts.push(Attempt {
                translator: tr,
                error: None,
            }),
            Err(e) => attempts.push(Attempt {
                translator: tr,
                error: Some(e),
            }),
        }
    }
    SearchStep::Done(Err(aggregate(&attempts)))
}

/// The error for a search where every translator came up empty: "not
/// found" when any translator answered (no items, or HTTP 404), else the
/// first translator's failure (offline, timeout, 429, ...), which is the
/// one the user can act on.
fn aggregate(attempts: &[Attempt]) -> LookupError {
    let answered = attempts.iter().any(|a| match &a.error {
        None => true,
        Some(SearchError::Http(HttpFailure::Status { code: 404, .. })) => true,
        _ => false,
    });
    if !answered {
        if let Some(e) = attempts.iter().find_map(|a| a.error.as_ref()) {
            return LookupError::from_search(e);
        }
    }
    LookupError::NotFound {
        attempts: attempts
            .iter()
            .map(|a| {
                (
                    a.translator.metadata().label.to_owned(),
                    a.error
                        .as_ref()
                        .map_or_else(|| "no items".to_owned(), ToString::to_string),
                )
            })
            .collect(),
    }
}

/// A lookup in progress: what to search, which translators, and the
/// answers so far. Owns everything (no borrows), so an async caller can
/// hold it across awaits.
#[derive(Debug, Clone, PartialEq)]
pub struct LookupSession {
    /// The identifier, when the session came from text.
    pub identifier: Option<Identifier>,
    /// The search item.
    pub search: JsObject,
    /// The translators to try, in order.
    pub translators: Vec<SearchTranslator>,
    /// The options.
    pub options: SearchOptions,
    cache: HttpCache,
}

/// Safety bound on requests per lookup (upstream lookups make one to four).
pub const MAX_REQUESTS: usize = 64;

impl LookupSession {
    /// A session for the identifier the translation-server's `/search`
    /// would take from `text` (searchEndpoint.js:40-47).
    pub fn for_text(text: &str, options: SearchOptions) -> Result<LookupSession, LookupError> {
        let id = identifier_for_search(text).ok_or(LookupError::NoIdentifier)?;
        Ok(LookupSession::for_identifier(id, options))
    }

    /// A session for one identifier.
    pub fn for_identifier(id: Identifier, options: SearchOptions) -> LookupSession {
        let search = id.search_item();
        let translators = detect_translators(&search);
        LookupSession {
            identifier: Some(id),
            search,
            translators,
            options,
            cache: HttpCache::new(),
        }
    }

    /// A session running one translator on a search item
    /// (`setSearch` + `setTranslator`).
    pub fn forced(
        search: JsObject,
        translator: SearchTranslator,
        options: SearchOptions,
    ) -> LookupSession {
        LookupSession {
            identifier: None,
            search,
            translators: vec![translator],
            options,
            cache: HttpCache::new(),
        }
    }

    /// Run as far as the answers allow.
    pub fn step(&self) -> SearchStep {
        run_search(
            &self.search,
            &self.translators,
            self.cache.clone(),
            &self.options,
        )
    }

    /// Give the answer to a pending request.
    pub fn answer(&mut self, req: &HttpRequest, answer: Result<HttpResponse, FetchError>) {
        self.cache.insert(req, answer);
    }

    /// Run to the end, fetching each pending request with `fetch`
    /// (synchronous callers; a browser drives [`LookupSession::step`] and
    /// [`LookupSession::answer`] itself).
    pub fn run_with<F>(mut self, mut fetch: F) -> Result<SearchRun, LookupError>
    where
        F: FnMut(&HttpRequest) -> Result<HttpResponse, FetchError>,
    {
        for _ in 0..=MAX_REQUESTS {
            match self.step() {
                SearchStep::Done(r) => return r,
                SearchStep::Pending(req) => {
                    let a = fetch(&req);
                    self.answer(&req, a);
                }
            }
        }
        Err(LookupError::Translator(format!(
            "more than {MAX_REQUESTS} requests; stopped"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_is_priority_then_file_name() {
        let p: Vec<u32> = SearchTranslator::ALL
            .iter()
            .map(|t| t.metadata().priority)
            .collect();
        let mut sorted = p.clone();
        sorted.sort();
        assert_eq!(p, sorted);
    }
}
