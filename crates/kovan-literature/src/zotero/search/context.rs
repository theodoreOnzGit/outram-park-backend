// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translate, https://github.com/zotero/translate (commit
//   dd524aea9a55): src/translation/translate.js — the search sandbox
//   (`Zotero.Translate.Sandbox.Search._itemDone` :863-870,
//   `Sandbox.Web._itemDone` :570-727, `Sandbox.Base._itemDone` :83-237 with
//   `libraryID: false`), `loadTranslator` :282-435 (child import and search
//   translators, their default `itemDone` handler), `Zotero.done` :449;
//   src/utilities_translate.js (`request*`, `doGet`, `processDocuments`).
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! [`SearchContext`]: the `Zotero` object of a search translation, and the
//! framework's handling of each item a search translator completes.

use super::http::{
    build_request, check_status, decode_document, decode_json, decode_text, FetchError, HttpCache,
    HttpFailure, HttpRequest, HttpResponse, RequestOptions,
};
use super::SearchTranslator;
use crate::zotero::framework::api_json::type_field_for_base;
use crate::zotero::framework::identifiers::{clean_isbn, to_isbn13};
use crate::zotero::framework::item::{JsObject, TranslatorItem};
use crate::zotero::framework::item_done::item_done_with;
use crate::zotero::framework::js;
use crate::zotero::framework::options::{TranslateOptions, TranslationEnv, TranslatorMetadata};
use crate::zotero::framework::utilities::field_is_valid_for_type;
use crate::zotero::framework::xml::XmlDocument;
use crate::zotero::framework::xpath::XPathError;
use crate::zotero::framework::TranslateError;
use crate::zotero::translators::Translator;
use kovan_common::zotero::schema_generated::{Field, ItemType};
use serde_json::Value;
use std::collections::HashMap;

/// Why a search translator stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchError {
    /// The run needs this request answered before it can go on (not a
    /// failure: the caller fetches it and runs the search again). A
    /// translator must never swallow this one.
    Pending(HttpRequest),
    /// A request failed.
    Http(HttpFailure),
    /// The translator (or a child import translator) threw.
    Translator(String),
}

impl SearchError {
    /// Whether this is [`SearchError::Pending`] (which no `catch` in a
    /// ported translator may swallow).
    pub fn is_pending(&self) -> bool {
        matches!(self, SearchError::Pending(_))
    }
}

impl std::fmt::Display for SearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SearchError::Pending(r) => write!(f, "waiting for {} {}", r.method, r.url),
            SearchError::Http(e) => write!(f, "{e}"),
            SearchError::Translator(m) => write!(f, "{m}"),
        }
    }
}

impl From<HttpFailure> for SearchError {
    fn from(e: HttpFailure) -> Self {
        SearchError::Http(e)
    }
}

impl From<TranslateError> for SearchError {
    fn from(e: TranslateError) -> Self {
        SearchError::Translator(e.to_string())
    }
}

impl From<XPathError> for SearchError {
    fn from(e: XPathError) -> Self {
        SearchError::Translator(e.0)
    }
}

/// What a search runs with.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchOptions {
    /// The environment (dates, the clock: `now_unix_secs` fixes "now" for
    /// the `accessDate` the framework stamps).
    pub env: TranslationEnv,
    /// Hidden preferences (`Z.getHiddenPref`), e.g. `CrossrefREST.email`
    /// (the translation-server's `translators.CrossrefREST.email` config).
    /// Empty by default: nothing user-identifying is sent unless the user
    /// sets it.
    pub hidden_prefs: JsObject,
}

impl Default for SearchOptions {
    fn default() -> Self {
        SearchOptions {
            env: TranslationEnv::default(),
            hidden_prefs: JsObject::new(),
        }
    }
}

/// What a child search translation produced (`Zotero.loadTranslator("search")`):
/// the items its `itemDone` handler received, and the error its `error`
/// handler received (upstream's caller chooses whether to care).
#[derive(Debug, Clone, PartialEq)]
pub struct ChildSearch {
    /// Items, after the child's `_itemDone`.
    pub items: Vec<TranslatorItem>,
    /// The failure, if the child failed or found nothing
    /// ("No items returned from any translator").
    pub error: Option<SearchError>,
}

#[derive(Debug, Clone)]
struct Frame {
    meta: &'static TranslatorMetadata,
    child: bool,
    items: Vec<TranslatorItem>,
}

/// The `Zotero` object of a search translation (and of the search
/// translations it loads).
#[derive(Debug, Clone)]
pub struct SearchContext {
    /// The options.
    pub options: SearchOptions,
    cache: HttpCache,
    seen: HashMap<String, usize>,
    requests: Vec<HttpRequest>,
    frames: Vec<Frame>,
}

impl SearchContext {
    /// A context answering requests from `cache`.
    pub fn new(cache: HttpCache, options: SearchOptions) -> Self {
        SearchContext {
            options,
            cache,
            seen: HashMap::new(),
            requests: Vec::new(),
            frames: Vec::new(),
        }
    }

    /// The requests made so far, in order.
    pub fn requests(&self) -> &[HttpRequest] {
        &self.requests
    }

    /// The cache back.
    pub fn into_cache(self) -> HttpCache {
        self.cache
    }

    /// The running translator's header.
    pub fn meta(&self) -> Option<&'static TranslatorMetadata> {
        self.frames.last().map(|f| f.meta)
    }

    // ------------------------------------------------------------- running

    /// Run `t` at top level (or as a child) on `search`; returns its items
    /// and its result. Used by the driver ([`super::run_search`]) and
    /// [`SearchContext::child_search`].
    pub(super) fn run_frame(
        &mut self,
        t: SearchTranslator,
        search: &JsObject,
        child: bool,
    ) -> (Vec<TranslatorItem>, Result<(), SearchError>) {
        self.frames.push(Frame {
            meta: t.metadata(),
            child,
            items: Vec::new(),
        });
        let r = t.do_search(self, search);
        let frame = self.frames.pop().expect("pushed above");
        (frame.items, r)
    }

    /// `Zotero.loadTranslator("search")` + `setTranslator(t)` +
    /// `setSearch(search)` + `translate()`: the child's items (what its
    /// `itemDone` handler gets) and its error (what its `error` handler
    /// gets; a child that returns nothing fails with "No items returned
    /// from any translator", `Zotero.Translate.Search#complete`). A
    /// [`SearchError::Pending`] is returned as `Err` and must be propagated.
    pub fn child_search(
        &mut self,
        t: SearchTranslator,
        search: &JsObject,
    ) -> Result<ChildSearch, SearchError> {
        let (items, r) = self.run_frame(t, search, true);
        match r {
            Err(SearchError::Pending(req)) => Err(SearchError::Pending(req)),
            Err(e) => Ok(ChildSearch {
                items,
                error: Some(e),
            }),
            Ok(()) if items.is_empty() => Ok(ChildSearch {
                items,
                error: Some(SearchError::Translator(
                    "No items returned from any translator".to_owned(),
                )),
            }),
            Ok(()) => Ok(ChildSearch { items, error: None }),
        }
    }

    /// `Zotero.loadTranslator("import")` + `setTranslator(t)` +
    /// `setString(text)` + `translate()`: the items the child completed,
    /// after the child form of `_itemDone`, for the caller's `itemDone`
    /// handler; pass each to [`SearchContext::complete`] for the default
    /// handler. An error in the child is the caller's error.
    pub fn child_import(
        &mut self,
        t: Translator,
        text: &str,
    ) -> Result<Vec<TranslatorItem>, SearchError> {
        let mut options = TranslateOptions::for_translator(t.metadata());
        options.env = self.options.env.clone();
        options.env.parent_translator = self.meta().map(|m| m.id.to_owned());
        // A child import parses the response, so its failure is a
        // malformed response.
        t.import(text, &options).map(|r| r.items).map_err(|e| {
            SearchError::Http(HttpFailure::Malformed(format!(
                "{}: {e}",
                t.metadata().label
            )))
        })
    }

    /// `item.complete()` in a search translator:
    /// `Zotero.Translate.Sandbox.Search._itemDone` (translate.js:863-870),
    /// which sets `libraryCatalog` to the translator's label when it is
    /// undefined, then `Sandbox.Web._itemDone` (:570-727) when this is the
    /// top-level translation, then `Sandbox.Base._itemDone` with
    /// `libraryID: false`. Fails as upstream's `translate.complete(false,
    /// "No title specified for item")` does.
    pub fn complete(&mut self, mut item: TranslatorItem) -> Result<(), SearchError> {
        let now = self.now_iso();
        let Some(frame) = self.frames.last_mut() else {
            return Err(SearchError::Translator(
                "item.complete() outside a translation".to_owned(),
            ));
        };
        if !item.props.contains("libraryCatalog") {
            item.set("libraryCatalog", frame.meta.label);
        }
        if !frame.child && !web_item_done(&mut item, &now)? {
            return Ok(());
        }
        let done = item_done_with(item, frame.child, false);
        frame.items.push(done);
        Ok(())
    }

    /// `Z.getHiddenPref(name)`.
    pub fn get_hidden_pref(&self, name: &str) -> Option<Value> {
        let meta = self.meta()?;
        self.options.hidden_prefs.get(name).cloned().or_else(|| {
            meta.hidden_prefs
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| v.to_value())
        })
    }

    /// "Now" as `Zotero.Date.dateToISO(new Date())`.
    pub fn now_iso(&self) -> String {
        let secs = self.options.env.now_unix_secs.unwrap_or_else(system_now);
        epoch_to_iso(secs)
    }

    /// `ZU.strToISO(s)` in this environment.
    pub fn str_to_iso(&self, s: &str) -> Option<String> {
        crate::zotero::framework::utilities::str_to_iso(s, &self.options.env.dates)
    }

    // --------------------------------------------------------------- HTTP

    /// `request(url, options)` (utilities_translate.js:307-370): the
    /// response, after the success-code check.
    pub fn request(
        &mut self,
        url: &str,
        opts: &RequestOptions,
    ) -> Result<HttpResponse, SearchError> {
        let req = build_request(url, opts)?;
        let key = req.key();
        let n = *self.seen.get(&key).unwrap_or(&0);
        let answer = self.cache.get(&req, n).cloned();
        let Some(answer) = answer else {
            return Err(SearchError::Pending(req));
        };
        self.seen.insert(key, n + 1);
        self.requests.push(req.clone());
        let resp = answer.map_err(|e: FetchError| SearchError::Http(HttpFailure::Fetch(e)))?;
        check_status(&req, &resp, opts.success_codes.as_deref())?;
        Ok(resp)
    }

    /// `requestText(url, options)`.
    pub fn request_text(
        &mut self,
        url: &str,
        opts: &RequestOptions,
    ) -> Result<String, SearchError> {
        let r = self.request(url, opts)?;
        Ok(decode_text(&r)?)
    }

    /// `requestJSON(url, options)`.
    pub fn request_json(&mut self, url: &str, opts: &RequestOptions) -> Result<Value, SearchError> {
        let r = self.request(url, opts)?;
        Ok(decode_json(&r)?)
    }

    /// `requestDocument(url, options)` (HTML or XML; a meta refresh within
    /// 15 s is followed, http.js:171-186). Returns the document and its
    /// final URL.
    pub fn request_document(
        &mut self,
        url: &str,
        opts: &RequestOptions,
    ) -> Result<(XmlDocument, String), SearchError> {
        let mut url = url.to_owned();
        // Bounded: a refresh loop is a failure, not a hang.
        for _ in 0..10 {
            let r = self.request(&url, opts)?;
            let (doc, refresh) = decode_document(&r)?;
            match refresh {
                Some(next) => url = next,
                None => return Ok((doc, r.url)),
            }
        }
        Err(SearchError::Http(HttpFailure::Malformed(
            "too many meta refreshes".to_owned(),
        )))
    }

    /// `ZU.doGet(url, callback)` (utilities_translate.js:477-524): the
    /// response text; any failure ends the translation.
    pub fn do_get(&mut self, url: &str) -> Result<String, SearchError> {
        self.request_text(url, &RequestOptions::default())
    }

    /// `ZU.processDocuments(url, processor)` for one URL: the document and
    /// its URL.
    pub fn process_document(&mut self, url: &str) -> Result<(XmlDocument, String), SearchError> {
        self.request_document(url, &RequestOptions::default())
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn system_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(target_arch = "wasm32")]
fn system_now() -> i64 {
    0
}

/// `Zotero.Date.dateToISO` of a Unix time.
pub fn epoch_to_iso(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// `Zotero.Translate.Sandbox.Web._itemDone` (:570-727) for a top-level
/// translation: `Ok(false)` when the item is discarded.
fn web_item_done(item: &mut TranslatorItem, now_iso: &str) -> Result<bool, SearchError> {
    if item.item_type.is_empty() {
        item.item_type = "webpage".to_owned();
    }
    if matches!(item.get_str("type"), Some("attachment") | Some("note")) {
        return Ok(false);
    }
    // libraryCatalog was set by Search._itemDone; drop it when invalid.
    if item.truthy("libraryCatalog") && !field_is_valid_for_type("libraryCatalog", &item.item_type)
    {
        item.remove("libraryCatalog");
    }
    if item.truthy("url") && !item.props.contains("accessDate") {
        item.set("accessDate", now_iso);
    }
    if let Some(t) = ItemType::from_name(&item.item_type) {
        if let Some(alt) = type_field_for_base(t, Field::Title) {
            if let Some(v) = item
                .get(alt.as_str())
                .filter(|v| js::truthy(Some(v)))
                .cloned()
            {
                item.set("title", v);
            }
        }
    }
    if !item.truthy("title") {
        return Err(SearchError::Translator(
            "No title specified for item".to_owned(),
        ));
    }
    if !item.props.contains("shortTitle") && field_is_valid_for_type("shortTitle", &item.item_type)
    {
        if let Some(title) = item.get_str("title").map(str::to_owned) {
            if let Some(short) = short_title(&title) {
                item.set("shortTitle", short);
            }
        }
    }
    if item.truthy("ISBN") {
        let s = item.get_string("ISBN").unwrap_or_default();
        item.set("ISBN", clean_isbns(&s).join(" "));
    }
    item.tags.retain(|t| t.tag.encode_utf16().count() <= 255);
    for a in &mut item.attachments {
        if a.truthy("path") {
            if !a.truthy("url") {
                let p = a.get("path").cloned().unwrap_or(Value::Null);
                a.set("url", p);
            }
            a.remove("path");
        }
    }
    Ok(true)
}

/// The short title Web `_itemDone` makes (:612-662), if it makes one.
fn short_title(title: &str) -> Option<String> {
    let mut t = title.to_owned();
    let mut set = false;
    if let Some(i) = t.find(':') {
        t.truncate(i);
        set = true;
    }
    if let Some(i) = t.find('?') {
        let end = i + 1;
        if end != t.len() {
            t.truncate(end);
            set = true;
        }
    }
    if !set {
        return None;
    }
    let pairs: [(&str, &str); 6] = [
        ("<i>", "</i>"),
        ("<b>", "</b>"),
        ("<sub>", "</sub>"),
        ("<sup>", "</sup>"),
        ("<span style=\"font-variant:small-caps;\">", "</span>"),
        ("<span class=\"nocase\">", "</span>"),
    ];
    let mut stack: Vec<&str> = Vec::new();
    for token in split_tags(&t) {
        if let Some((_, close)) = pairs.iter().find(|(open, _)| *open == token) {
            stack.push(close);
        } else if stack.last() == Some(&token) {
            stack.pop();
        }
    }
    while let Some(c) = stack.pop() {
        t.push_str(c);
    }
    Some(t)
}

/// `title.split(/(<[^>]+>)/)`.
fn split_tags(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'<' {
            if let Some(len) = s[i + 1..].find('>') {
                if len > 0 {
                    out.push(&s[start..i]);
                    out.push(&s[i..i + len + 2]);
                    i += len + 2;
                    start = i;
                    continue;
                }
            }
        }
        i += 1;
    }
    out.push(&s[start..]);
    out
}

fn is_isbn_sep(c: char) -> bool {
    js::is_space(c)
        || matches!(
            c,
            '\u{2D}' | '\u{AD}' | '\u{2010}'..='\u{2015}' | '\u{2043}' | '\u{2212}'
        )
}

fn is_dash(c: char) -> bool {
    matches!(
        c,
        '\u{2D}' | '\u{AD}' | '\u{2010}'..='\u{2015}' | '\u{2043}' | '\u{2212}'
    )
}

/// Web `_itemDone`'s ISBN clean-up (:671-688): every match of
/// `/\b(?:97[89][\s\x2D\xAD‐-―⁃−]*)?(?:\d[...]*){9}[\dx](?![\x2D\xAD‐-―⁃−])\b/gi`
/// that `cleanISBN` accepts, as ISBN-13, without duplicates (an invalid
/// match restarts the search one character later).
pub fn clean_isbns(s: &str) -> Vec<String> {
    let c: Vec<char> = s.chars().collect();
    let word = |i: usize| i < c.len() && js::is_word_char(c[i]);
    let digit = |i: usize| i < c.len() && c[i].is_ascii_digit();
    // The end of `(?:\d[sep]*){9}[\dx]` + lookahead + \b from `g`.
    let body = |mut g: usize| -> Option<usize> {
        for _ in 0..9 {
            if !digit(g) {
                return None;
            }
            g += 1;
            while g < c.len() && is_isbn_sep(c[g]) {
                g += 1;
            }
        }
        if !(digit(g) || (g < c.len() && (c[g] == 'x' || c[g] == 'X'))) {
            return None;
        }
        g += 1;
        if g < c.len() && is_dash(c[g]) {
            return None;
        }
        (!word(g)).then_some(g)
    };
    let match_at = |p: usize| -> Option<usize> {
        if p > 0 && word(p - 1) {
            return None;
        }
        if p + 3 <= c.len()
            && c[p] == '9'
            && c[p + 1] == '7'
            && (c[p + 2] == '8' || c[p + 2] == '9')
        {
            let mut g = p + 3;
            while g < c.len() && is_isbn_sep(c[g]) {
                g += 1;
            }
            if let Some(e) = body(g) {
                return Some(e);
            }
        }
        body(p)
    };
    let mut out: Vec<String> = Vec::new();
    let mut last = 0;
    while last < c.len() {
        let Some((p, e)) = (last..c.len()).find_map(|p| match_at(p).map(|e| (p, e))) else {
            break;
        };
        let m: String = c[p..e].iter().collect();
        match clean_isbn(&m, false) {
            None => last = p + 1,
            Some(valid) => {
                if let Some(i13) = to_isbn13(&valid) {
                    if !out.contains(&i13) {
                        out.push(i13);
                    }
                }
                last = e;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_titles_like_upstream() {
        assert_eq!(short_title("A: b"), Some("A".to_owned()));
        assert_eq!(short_title("Why? Because"), Some("Why?".to_owned()));
        assert_eq!(short_title("Why?"), None);
        assert_eq!(short_title("<i>A: b</i>"), Some("<i>A</i>".to_owned()));
        assert_eq!(short_title("Plain"), None);
    }

    #[test]
    fn isbn_cleanup_like_upstream() {
        assert_eq!(clean_isbns("0838985890"), vec!["9780838985892"]);
        assert_eq!(
            clean_isbns("978-0-8389-8589-2 0838985890 9781479347711"),
            vec!["9780838985892", "9781479347711"]
        );
        assert!(clean_isbns("12345").is_empty());
    }

    #[test]
    fn web_item_done_requires_title() {
        let mut i = TranslatorItem::new("book");
        assert!(web_item_done(&mut i, "NOW").is_err());
        i.set("title", "T: sub");
        i.set("url", "https://x.org/");
        assert!(web_item_done(&mut i, "NOW").unwrap());
        assert_eq!(i.get_str("accessDate"), Some("NOW"));
        assert_eq!(i.get_str("shortTitle"), Some("T"));
    }
}
