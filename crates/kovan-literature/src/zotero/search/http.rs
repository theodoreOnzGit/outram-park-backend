// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: the Zotero translation-server, https://github.com/zotero/
//   translation-server (commit 3a9d17614896): src/http.js
//   `Zotero.HTTP.request` :90-210 (default headers, POST Content-Type, the
//   success-code and Content-Type checks of `customRequest` :388-449,
//   `getResponseType` :458-476, charset decoding, meta refresh :171-186);
//   Zotero translate (commit dd524aea9a55) src/utilities_translate.js
//   `request` :307-370, `requestText`/`requestJSON`/`requestDocument`
//   :377-397, `doGet` :477-524; the WHATWG URL serializer (`new URL(url)`
//   in `Zotero.HTTP.request` :136, url.spec.whatwg.org percent-encode sets).
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The HTTP layer the search translators see. **No networking happens
//! here**: the library builds [`HttpRequest`]s and interprets
//! [`HttpResponse`]s exactly as Zotero's framework does; who sends the
//! request is the caller's business (see [`super::lookup`]):
//!
//! ```text
//! translator --request_text/json/document/do_get--> SearchContext
//!     --HttpRequest--> [answer cached?] --no--> SearchStep::Pending(request)
//!                            |yes                      | caller fetches it (native
//!                            v                         | ureq in kovan, browser fetch,
//!     status / Content-Type checks, charset decode,    | a recorded fixture, a fake)
//!     JSON parse, HTML/XML document                    v
//!                                           answer cached, search re-run
//! ```
//!
//! Every failure is a value ([`FetchError`] from the caller's backend,
//! [`HttpFailure`] after Zotero's checks); nothing here panics on network
//! data.

use super::super::framework::html_dom::parse_html_document;
use super::super::framework::xml::XmlDocument;
use serde_json::Value;
use std::collections::HashMap;

/// An HTTP request as Zotero's `Zotero.HTTP.request` would send it. The
/// headers are the ones Zotero sets (`Accept: */*` by default, the
/// translator's own, a default `Content-Type` on a POST body), in order;
/// the `User-Agent` is the backend's to add.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HttpRequest {
    /// "GET", "POST", ...
    pub method: String,
    /// The URL as `new URL(url).href` serializes it.
    pub url: String,
    /// Request headers, in order, without `User-Agent`.
    pub headers: Vec<(String, String)>,
    /// The body (POST), as sent.
    pub body: Option<String>,
}

impl HttpRequest {
    /// What identifies a request for caching and replay: method, URL and
    /// body (the recorded-fixture harness, scripts/zotero-search-reference.mjs,
    /// keys exactly so).
    pub fn key(&self) -> String {
        format!(
            "{} {}\n{}",
            self.method,
            self.url,
            self.body.as_deref().unwrap_or("")
        )
    }

    /// A header's value (case-insensitive name).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// What came back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    /// The status code.
    pub status: u16,
    /// The final URL (after redirects).
    pub url: String,
    /// The `Content-Type` header, if any.
    pub content_type: Option<String>,
    /// The `Retry-After` header, if any (for a 429).
    pub retry_after: Option<String>,
    /// The body, decompressed, as bytes.
    pub body: Vec<u8>,
}

/// Why a backend could not produce a response at all. The backend (native,
/// browser, fake) chooses the variant; nothing below a response's status is
/// a `FetchError`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchError {
    /// No network (the machine is offline).
    Offline,
    /// DNS resolution or the TCP/TLS connection failed.
    Connect(String),
    /// The connect or read timeout elapsed.
    Timeout,
    /// The response was larger than the backend accepts.
    TooLarge,
    /// Not possible on this target (e.g. a browser blocking a cross-origin
    /// request, or a build without a network backend).
    Unsupported(String),
    /// Anything else the backend reports.
    Other(String),
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchError::Offline => write!(f, "offline (no network)"),
            FetchError::Connect(m) => write!(f, "could not connect: {m}"),
            FetchError::Timeout => write!(f, "timed out"),
            FetchError::TooLarge => write!(f, "response too large"),
            FetchError::Unsupported(m) => write!(f, "not available on this target: {m}"),
            FetchError::Other(m) => write!(f, "network error: {m}"),
        }
    }
}

/// A request that failed, after Zotero's checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpFailure {
    /// The backend could not fetch it.
    Fetch(FetchError),
    /// A status Zotero does not count as success (2xx by default).
    Status {
        /// The status code.
        code: u16,
        /// The URL requested.
        url: String,
    },
    /// HTTP 429, with `Retry-After` when the server sent one.
    RateLimited {
        /// The URL requested.
        url: String,
        /// The `Retry-After` header.
        retry_after: Option<String>,
    },
    /// No `Content-Type` header, or one the request cannot take
    /// (`Zotero.HTTP.UnsupportedFormatError`).
    UnsupportedFormat(String),
    /// The body could not be read as asked (bad JSON, bad XML, an unknown
    /// charset).
    Malformed(String),
    /// The URL is not one Zotero would request (`new URL` throws, or not
    /// http/https).
    BadUrl(String),
}

impl std::fmt::Display for HttpFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpFailure::Fetch(e) => write!(f, "{e}"),
            HttpFailure::Status { code, url } => write!(f, "HTTP {code} from {url}"),
            HttpFailure::RateLimited { url, retry_after } => match retry_after {
                Some(r) => write!(f, "rate-limited by {url} (retry after {r})"),
                None => write!(f, "rate-limited by {url}"),
            },
            HttpFailure::UnsupportedFormat(m) => write!(f, "unsupported response format: {m}"),
            HttpFailure::Malformed(m) => write!(f, "malformed response: {m}"),
            HttpFailure::BadUrl(m) => write!(f, "bad URL: {m}"),
        }
    }
}

/// Options for [`super::SearchContext::request`] (the translator's
/// `options` object).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RequestOptions {
    /// `method` (default GET).
    pub method: Option<String>,
    /// `headers`, in order.
    pub headers: Vec<(String, String)>,
    /// `body`.
    pub body: Option<String>,
    /// `successCodes`: `None` is 2xx; `Some(vec![])` is `false` (any).
    pub success_codes: Option<Vec<u16>>,
}

impl RequestOptions {
    /// GET with these headers.
    pub fn headers(headers: &[(&str, &str)]) -> Self {
        RequestOptions {
            headers: headers
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
            ..RequestOptions::default()
        }
    }

    /// POST `body` with these headers.
    pub fn post(body: impl Into<String>, headers: &[(&str, &str)]) -> Self {
        RequestOptions {
            method: Some("POST".to_owned()),
            body: Some(body.into()),
            ..RequestOptions::headers(headers)
        }
    }
}

/// Build the request `Zotero.HTTP.request(method, url, options)` sends:
/// `Object.assign({'User-Agent', Accept: '*/*'}, headers)` (the user agent
/// left to the backend), and for a non-GET/HEAD body a default
/// `Content-Type: application/x-www-form-urlencoded` (http.js:104-125).
pub fn build_request(url: &str, opts: &RequestOptions) -> Result<HttpRequest, HttpFailure> {
    let href = url_href(url)?;
    if !(href.starts_with("http://") || href.starts_with("https://")) {
        return Err(HttpFailure::BadUrl(format!(
            "Translator cannot make {} request",
            href.split(':').next().unwrap_or("")
        )));
    }
    let method = opts.method.clone().unwrap_or_else(|| "GET".to_owned());
    let mut headers: Vec<(String, String)> = vec![("Accept".to_owned(), "*/*".to_owned())];
    for (k, v) in &opts.headers {
        if k == "User-Agent" {
            continue;
        }
        match headers.iter_mut().find(|(hk, _)| hk == k) {
            Some(h) => h.1 = v.clone(),
            None => headers.push((k.clone(), v.clone())),
        }
    }
    if (method == "GET" || method == "HEAD") && opts.body.is_some() {
        return Err(HttpFailure::BadUrl(format!(
            "HTTP {method} cannot have a request body"
        )));
    }
    let body = opts.body.clone().filter(|b| !b.is_empty());
    if body.is_some() {
        match headers.iter().position(|(k, _)| k == "Content-Type") {
            None => headers.push((
                "Content-Type".to_owned(),
                "application/x-www-form-urlencoded".to_owned(),
            )),
            Some(i) if headers[i].1 == "multipart/form-data" => {
                headers.remove(i);
            }
            Some(_) => {}
        }
    }
    Ok(HttpRequest {
        method,
        url: href,
        headers,
        body: opts.body.clone(),
    })
}

/// The status check of `customRequest` (http.js:397-413): success codes
/// given, `false` (any), or 2xx.
pub fn check_status(
    req: &HttpRequest,
    resp: &HttpResponse,
    success_codes: Option<&[u16]>,
) -> Result<(), HttpFailure> {
    // http.js:392-395: no Content-Type is an error before the status is looked at.
    if resp.content_type.is_none() {
        return Err(HttpFailure::UnsupportedFormat(
            "Missing Content-Type header".to_owned(),
        ));
    }
    let ok = match success_codes {
        Some([]) => true,
        Some(c) => c.contains(&resp.status),
        None => (200..300).contains(&resp.status),
    };
    if ok {
        return Ok(());
    }
    if resp.status == 429 {
        return Err(HttpFailure::RateLimited {
            url: req.url.clone(),
            retry_after: resp.retry_after.clone(),
        });
    }
    Err(HttpFailure::Status {
        code: resp.status,
        url: req.url.clone(),
    })
}

/// A MIME type's essence (`type/subtype`, lower case) and its `charset`.
pub fn mime(content_type: &str) -> (String, Option<String>) {
    let mut parts = content_type.split(';');
    let essence = parts.next().unwrap_or("").trim().to_ascii_lowercase();
    let charset = parts.find_map(|p| {
        let (k, v) = p.split_once('=')?;
        (k.trim().eq_ignore_ascii_case("charset"))
            .then(|| v.trim().trim_matches('"').to_ascii_lowercase())
    });
    (essence, charset)
}

/// whatwg-mimetype `isHTML`.
pub fn is_html(essence: &str) -> bool {
    essence == "text/html"
}

/// whatwg-mimetype `isXML`.
pub fn is_xml(essence: &str) -> bool {
    essence.ends_with("+xml") || essence == "text/xml" || essence == "application/xml"
}

/// The body as text, as `Zotero.HTTP.request` decodes a `text` response
/// (http.js:191-204: the Content-Type charset through iconv-lite, UTF-8
/// when there is none or it is unknown; iconv-lite drops a leading BOM).
/// Charsets ported: UTF-8, ISO-8859-1/Latin-1, US-ASCII, Windows-1252;
/// another charset iconv-lite knows is reported as
/// [`HttpFailure::Malformed`] rather than mis-decoded.
pub fn decode_text(resp: &HttpResponse) -> Result<String, HttpFailure> {
    let charset = resp
        .content_type
        .as_deref()
        .and_then(|c| mime(c).1)
        .unwrap_or_else(|| "utf8".to_owned());
    let s = match charset.as_str() {
        "utf8" | "utf-8" | "unicode-1-1-utf-8" => String::from_utf8_lossy(&resp.body).into_owned(),
        "iso-8859-1" | "latin1" | "l1" | "iso8859-1" | "binary" => {
            resp.body.iter().map(|&b| char::from(b)).collect()
        }
        "us-ascii" | "ascii" => resp.body.iter().map(|&b| char::from(b & 0x7f)).collect(),
        "windows-1252" | "cp1252" => resp.body.iter().map(|&b| cp1252(b)).collect(),
        other if KNOWN_UNPORTED.contains(&other) => {
            return Err(HttpFailure::Malformed(format!(
                "charset {other} is not supported by this port"
            )));
        }
        // "Unknown charset -- decoding as UTF-8".
        _ => String::from_utf8_lossy(&resp.body).into_owned(),
    };
    Ok(s.strip_prefix('\u{FEFF}').map(str::to_owned).unwrap_or(s))
}

/// Charsets iconv-lite decodes that this port does not.
const KNOWN_UNPORTED: [&str; 8] = [
    "shift_jis", "euc-jp", "gbk", "gb2312", "big5", "euc-kr", "koi8-r", "utf-16",
];

fn cp1252(b: u8) -> char {
    const HI: [u16; 32] = [
        0x20AC, 0x81, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160,
        0x2039, 0x0152, 0x8D, 0x017D, 0x8F, 0x90, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013,
        0x2014, 0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x9D, 0x017E, 0x0178,
    ];
    if (0x80..0xA0).contains(&b) {
        char::from_u32(u32::from(HI[usize::from(b - 0x80)])).unwrap_or('\u{FFFD}')
    } else {
        char::from(b)
    }
}

/// `JSON.parse(body.toString())` (http.js:188-190; UTF-8, a BOM is a
/// syntax error as in JavaScript).
pub fn decode_json(resp: &HttpResponse) -> Result<Value, HttpFailure> {
    let s = String::from_utf8_lossy(&resp.body);
    serde_json::from_str(&s).map_err(|e| HttpFailure::Malformed(format!("JSON: {e}")))
}

/// A `document` response (http.js:159-187): HTML through the HTML parser,
/// XML through the XML parser; any other type is
/// `UnsupportedFormatError`. Returns the document and, for HTML, a meta
/// refresh target (within 15 s) to follow.
pub fn decode_document(resp: &HttpResponse) -> Result<(XmlDocument, Option<String>), HttpFailure> {
    let ct = resp.content_type.as_deref().unwrap_or("");
    let (essence, _) = mime(ct);
    if is_html(&essence) {
        let text = decode_html_bytes(resp);
        let doc = parse_html_document(&text);
        let refresh = meta_refresh(&doc, &resp.url);
        Ok((doc, refresh))
    } else if is_xml(&essence) {
        let text = decode_text(resp)?;
        let doc = XmlDocument::parse(&text)
            .map_err(|e| HttpFailure::Malformed(format!("XML: {e:?}")))?;
        Ok((doc, None))
    } else {
        Err(HttpFailure::UnsupportedFormat(format!("{ct} is not supported")))
    }
}

/// `decodeContent` (http.js:325-338): html-encoding-sniffer with the
/// transport charset, else a `<meta charset>` prescan, else UTF-8. Ported
/// for the charsets [`decode_text`] knows.
fn decode_html_bytes(resp: &HttpResponse) -> String {
    let transport = resp.content_type.as_deref().and_then(|c| mime(c).1);
    let label = transport.or_else(|| sniff_meta_charset(&resp.body));
    let r = HttpResponse {
        content_type: Some(format!(
            "text/html; charset={}",
            label.unwrap_or_else(|| "utf-8".to_owned())
        )),
        ..resp.clone()
    };
    decode_text(&r).unwrap_or_else(|_| String::from_utf8_lossy(&resp.body).into_owned())
}

fn sniff_meta_charset(body: &[u8]) -> Option<String> {
    let head = String::from_utf8_lossy(&body[..body.len().min(1024)]).to_ascii_lowercase();
    let i = head.find("charset=")?;
    let rest = head[i + 8..].trim_start_matches(['"', '\'']);
    let end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .unwrap_or(rest.len());
    (end > 0).then(|| rest[..end].to_owned())
}

/// The meta refresh `Zotero.HTTP.request` follows (http.js:171-186).
fn meta_refresh(doc: &XmlDocument, base: &str) -> Option<String> {
    use super::super::framework::html_dom::query_selector_all;
    let meta = *query_selector_all(doc, doc.document(), "meta[http-equiv=refresh]").first()?;
    let content = doc.get_attribute(meta, "content")?;
    let parts: Vec<&str> = split_refresh(content);
    if parts.len() != 2 {
        return None;
    }
    let secs: Option<i64> = parse_int(parts[0]);
    if secs.is_some_and(|s| s <= 15) {
        let mut target = parts[1].trim().to_owned();
        if target.len() >= 2 && target.starts_with('\'') && target[1..].contains('\'') {
            // .replace(/^'(.+)'/, '$1'): the first quoted run, greedy.
            let close = target.rfind('\'').unwrap_or(0);
            if close > 1 {
                target = format!("{}{}", &target[1..close], &target[close + 1..]);
            }
        }
        return Some(resolve_url(base, &target));
    }
    None
}

fn split_refresh(s: &str) -> Vec<&str> {
    // .split(/;\s*url=/) (JavaScript \s).
    use std::sync::OnceLock;
    static R: OnceLock<regex::Regex> = OnceLock::new();
    R.get_or_init(|| {
        regex::Regex::new(&format!(
            ";{}*url=",
            super::super::framework::js::WS
        ))
        .expect("static regex")
    })
    .split(s)
    .collect()
}

fn parse_int(s: &str) -> Option<i64> {
    let t = s.trim_start();
    let (neg, d) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let digits: String = d.chars().take_while(char::is_ascii_digit).collect();
    let n: i64 = digits.parse().ok()?;
    Some(if neg { -n } else { n })
}

/// `url.resolve(base, href)` for the cases translators produce: an absolute
/// URL is returned as is, a scheme-relative or root-relative one is joined
/// to the base's origin, anything else to the base's directory.
pub fn resolve_url(base: &str, href: &str) -> String {
    if href.contains("://") {
        return href.to_owned();
    }
    let scheme_end = base.find("://").map_or(0, |i| i + 3);
    let origin_end = base[scheme_end..]
        .find('/')
        .map_or(base.len(), |i| scheme_end + i);
    if let Some(rest) = href.strip_prefix("//") {
        return format!("{}{}", &base[..scheme_end], rest);
    }
    if href.starts_with('/') {
        return format!("{}{}", &base[..origin_end], href);
    }
    let path = base.split(['?', '#']).next().unwrap_or(base);
    let dir_end = path.rfind('/').filter(|&i| i >= origin_end).map_or(path.len(), |i| i + 1);
    let dir = if dir_end > path.len() { path } else { &path[..dir_end] };
    if dir_end == path.len() && !path.ends_with('/') {
        return format!("{path}/{href}");
    }
    format!("{dir}{href}")
}

/// `new URL(url).href` for an absolute http(s) URL (WHATWG URL
/// serialization): scheme and host lower-cased, a default port dropped, an
/// empty path made "/", backslashes in the path read as "/", and the path,
/// query and fragment percent-encoded with their WHATWG percent-encode
/// sets (non-ASCII as UTF-8). A host that is not ASCII (IDNA) is refused
/// rather than mis-encoded.
pub fn url_href(url: &str) -> Result<String, HttpFailure> {
    let url = url.trim_matches(|c: char| c <= ' ');
    let url: String = url.chars().filter(|c| !matches!(c, '\t' | '\n' | '\r')).collect();
    let Some((scheme, rest)) = url.split_once(':') else {
        return Err(HttpFailure::BadUrl(format!("Invalid URL: {url}")));
    };
    if scheme.is_empty() || !scheme.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c)) {
        return Err(HttpFailure::BadUrl(format!("Invalid URL: {url}")));
    }
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        // Not a special URL Zotero would fetch; keep it for the protocol check.
        return Ok(format!("{scheme}:{rest}"));
    }
    let rest = rest.trim_start_matches(['/', '\\']);
    let auth_end = rest.find(['/', '\\', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(auth_end);
    let host_port = authority.rsplit('@').next().unwrap_or(authority);
    if host_port.is_empty() {
        return Err(HttpFailure::BadUrl(format!("Invalid URL: {url}")));
    }
    if !host_port.is_ascii() {
        return Err(HttpFailure::BadUrl(format!(
            "internationalised host names are not supported: {host_port}"
        )));
    }
    let (host, port) = match host_port.rsplit_once(':') {
        Some((h, p)) if !h.ends_with(']') || h.starts_with('[') => (h, Some(p)),
        _ => (host_port, None),
    };
    let host = host.to_ascii_lowercase();
    let port = port.filter(|p| {
        !(p.is_empty() || (scheme == "http" && *p == "80") || (scheme == "https" && *p == "443"))
    });
    let (path_part, query, fragment) = {
        let (before_frag, frag) = match tail.split_once('#') {
            Some((a, b)) => (a, Some(b)),
            None => (tail, None),
        };
        let (path, q) = match before_frag.split_once('?') {
            Some((a, b)) => (a, Some(b)),
            None => (before_frag, None),
        };
        (path, q, frag)
    };
    let mut path = path_part.replace('\\', "/");
    if path.is_empty() {
        path.push('/');
    }
    let mut out = format!("{scheme}://");
    // Credentials are kept verbatim (no translator uses them).
    if authority.contains('@') {
        let creds = &authority[..authority.rfind('@').unwrap_or(0)];
        out.push_str(creds);
        out.push('@');
    }
    out.push_str(&host);
    if let Some(p) = port {
        out.push(':');
        out.push_str(p);
    }
    out.push_str(&encode_set(&path, PATH_SET));
    if let Some(q) = query {
        out.push('?');
        out.push_str(&encode_set(q, QUERY_SET_SPECIAL));
    }
    if let Some(f) = fragment {
        out.push('#');
        out.push_str(&encode_set(f, FRAGMENT_SET));
    }
    Ok(out)
}

/// The path percent-encode set, beyond C0 controls and non-ASCII.
const PATH_SET: &str = " \"#<>?`{}";
/// The special-query percent-encode set.
const QUERY_SET_SPECIAL: &str = " \"#<>'";
/// The fragment percent-encode set.
const FRAGMENT_SET: &str = " \"<>`";

fn encode_set(s: &str, set: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii() && !c.is_ascii_control() && c != '\u{7f}' && !set.contains(c) {
            out.push(c);
        } else if c == '\u{7f}' && set == FRAGMENT_SET {
            out.push_str("%7F");
        } else if c == '\u{7f}' {
            out.push(c);
        } else {
            let mut buf = [0u8; 4];
            for b in c.encode_utf8(&mut buf).bytes() {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}

/// JavaScript `encodeURIComponent`.
pub fn encode_uri_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || "-_.!~*'()".contains(c) {
            out.push(c);
        } else {
            let mut buf = [0u8; 4];
            for b in c.encode_utf8(&mut buf).bytes() {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}

/// Answers already obtained, by request key, in the order they were
/// obtained. The `n`-th time a run asks for the same request it gets the
/// `n`-th answer; when there is none, the run stops and asks the caller.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HttpCache {
    answers: HashMap<String, Vec<Result<HttpResponse, FetchError>>>,
}

impl HttpCache {
    /// An empty cache.
    pub fn new() -> Self {
        HttpCache::default()
    }

    /// Record the answer to `req`.
    pub fn insert(&mut self, req: &HttpRequest, answer: Result<HttpResponse, FetchError>) {
        self.answers.entry(req.key()).or_default().push(answer);
    }

    /// The `n`-th answer to `req`, if obtained.
    pub fn get(&self, req: &HttpRequest, n: usize) -> Option<&Result<HttpResponse, FetchError>> {
        self.answers.get(&req.key()).and_then(|v| v.get(n))
    }

    /// How many answers are held.
    pub fn len(&self) -> usize {
        self.answers.values().map(Vec::len).sum()
    }

    /// Whether nothing is held.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hrefs_like_whatwg() {
        assert_eq!(
            url_href("https://lx2.loc.gov/sru/lcdb?query=bath.ISBN=^978&x=1").unwrap(),
            "https://lx2.loc.gov/sru/lcdb?query=bath.ISBN=^978&x=1"
        );
        assert_eq!(url_href("HTTPS://Example.ORG:443").unwrap(), "https://example.org/");
        assert_eq!(
            url_href("https://x.org/a b?q=\"é\"#f g").unwrap(),
            "https://x.org/a%20b?q=%22%C3%A9%22#f%20g"
        );
        assert_eq!(
            url_href("https://api.ies.ed.gov/eric/?search=id:EJ1&fields=*").unwrap(),
            "https://api.ies.ed.gov/eric/?search=id:EJ1&fields=*"
        );
        assert!(url_href("nonsense").is_err());
    }

    #[test]
    fn default_headers_like_zotero() {
        let r = build_request(
            "https://x.org/",
            &RequestOptions::post("{}", &[("Content-Type", "application/json")]),
        )
        .unwrap();
        assert_eq!(
            r.headers,
            vec![
                ("Accept".to_owned(), "*/*".to_owned()),
                ("Content-Type".to_owned(), "application/json".to_owned())
            ]
        );
        let r = build_request("https://x.org/", &RequestOptions::post("a=1", &[])).unwrap();
        assert_eq!(r.header("content-type"), Some("application/x-www-form-urlencoded"));
        assert!(build_request("ftp://x.org/", &RequestOptions::default()).is_err());
    }

    #[test]
    fn status_and_charset() {
        let req = build_request("https://x.org/", &RequestOptions::default()).unwrap();
        let mut resp = HttpResponse {
            status: 429,
            url: req.url.clone(),
            content_type: Some("text/plain; charset=ISO-8859-1".to_owned()),
            retry_after: Some("30".to_owned()),
            body: vec![0xE9],
        };
        assert_eq!(
            check_status(&req, &resp, None),
            Err(HttpFailure::RateLimited {
                url: req.url.clone(),
                retry_after: Some("30".to_owned())
            })
        );
        resp.status = 200;
        assert_eq!(check_status(&req, &resp, None), Ok(()));
        assert_eq!(decode_text(&resp).unwrap(), "é");
        resp.content_type = None;
        assert!(matches!(
            check_status(&req, &resp, None),
            Err(HttpFailure::UnsupportedFormat(_))
        ));
    }

    #[test]
    fn encode_uri_component_like_js() {
        assert_eq!(encode_uri_component("10.1109/TPS.1987"), "10.1109%2FTPS.1987");
        assert_eq!(encode_uri_component("a b'é"), "a%20b'%C3%A9");
    }
}
