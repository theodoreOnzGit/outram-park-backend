//! **Full-text search over the built Pages site** (gh:#876 item 8,
//! maintainer 2026-10-16: "full text index").
//!
//! `kovan-cli site-index <site-dir>` walks the site `scripts/build-pages.sh`
//! has just built and writes `<site-dir>/search-text.json`: every page's
//! title, URL and opening text, and an **inverted index**, term → the pages
//! holding it with a weight (title > heading > body). The landing page's
//! search box (`docs/site/search.js`) loads it lazily on first use and
//! answers a query by prefix-AND over the sorted terms, merged with the
//! pathway entries of `search.json` (`scripts/build-search-index.sh`).
//!
//! - **What is read**: each page's `<main>` (or `<body>`), with `<script>`,
//!   `<style>`, `<nav>` and `<noscript>` removed, so mdBook's sidebar, which
//!   lists every chapter on every page, does not make every page match every
//!   chapter title. HTML parsing and text extraction are
//!   `kovan_literature`'s port of the browser's own (`parse_html_document`,
//!   `query_selector_all`, `textContent`); nothing here re-implements them.
//! - **What is skipped**: `api/` (rustdoc has its own search), `code-review/`
//!   (likewise), and mdBook's `print.html`, `404.html` and `toc.html`.
//! - **One tokeniser** ([`tokenise`]): lower case, split on every character
//!   that is neither alphabetic nor numeric (Unicode), keep tokens of two or
//!   more characters and any number. `search.js` carries the same rule;
//!   `docs/site/search-tokenise-fixture.json` holds cases both must answer
//!   identically (`tests::the_tokeniser_answers_the_shared_fixture` here,
//!   `docs/site/check-search.js` in the site build).
//!
//! Not `no_std` (#876 item 8 asked for a `no_std` + `alloc` query core; that
//! needs a crate of its own, a naming decision left to the maintainer). The
//! query side is JavaScript in the page, as the existing search is.

use kovan_literature::zotero::framework::html_dom::{parse_html_document, query_selector_all};
use kovan_literature::zotero::framework::xml::{NodeId, XNode, XmlDocument};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

/// Characters of opening text kept per page, for the result preview.
pub const PREVIEW_CHARS: usize = 220;

/// Weight of a term in a page's title, in a heading, and per occurrence in
/// the body (body occurrences capped at [`BODY_CAP`]).
pub const W_TITLE: u32 = 12;
pub const W_HEADING: u32 = 4;
pub const W_BODY: u32 = 1;
pub const BODY_CAP: u32 = 6;

/// The search tokens of `s`: lower case, split on every character that is
/// neither alphabetic nor numeric, tokens of one character dropped unless
/// they are a digit.
pub fn tokenise(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !(c.is_alphabetic() || c.is_numeric()))
        .filter(|t| t.chars().count() >= 2 || t.chars().any(char::is_numeric))
        .map(str::to_string)
        .collect()
}

/// One page as read from the site.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    /// From the site root, `dir/` for a directory's `index.html`.
    pub url: String,
    /// `science`, `code` or `demo` (the groups the search box shows).
    pub kind: &'static str,
    pub title: String,
    pub headings: Vec<String>,
    pub text: String,
    /// The first real paragraph (see [`extract_with_preview`]).
    pub preview: String,
}

/// The group a URL belongs to: the lessons and the science pathway are
/// science, the demos demo, everything else (deep dives, code pathway, code
/// map, ICSBEP, schema) code.
pub fn kind_of(url: &str) -> &'static str {
    if url.starts_with("science/") || url.starts_with("tutorials/") {
        "science"
    } else if url.starts_with("demos/") {
        "demo"
    } else {
        "code"
    }
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn text_of(doc: &XmlDocument, n: NodeId) -> String {
    squash(&doc.text_content(XNode::Node(n)).unwrap_or_default())
}

/// Title, headings and text of an HTML page, or `None` for a page with no
/// readable content. The site's banner (`.banner`, the same research-only
/// notice on every page) is not text of the page.
pub fn extract(html: &str) -> Option<(String, Vec<String>, String)> {
    extract_with_preview(html).map(|(t, h, x, _)| (t, h, x))
}

/// Whether `n` lies inside a `<blockquote>` below `top`.
fn in_blockquote(doc: &XmlDocument, mut n: NodeId, top: NodeId) -> bool {
    while let Some(p) = doc.parent(n) {
        if p == top {
            return false;
        }
        if doc.local_name(XNode::Node(p)).is_some_and(|l| l.eq_ignore_ascii_case("blockquote")) {
            return true;
        }
        n = p;
    }
    false
}

/// [`extract`], and the page's preview: its first paragraph of 40 or more
/// characters that is not in a quotation block (a lesson's review-status
/// note is one) nor a status line, else the start of its text.
pub fn extract_with_preview(html: &str) -> Option<(String, Vec<String>, String, String)> {
    // `textContent` joins adjacent elements with nothing between them
    // ("<h1>Jemima</h1><p>Godiva" reads "JemimaGodiva"); a space before
    // every tag keeps the words apart, as a rendered page does.
    let mut doc = parse_html_document(&html.replace('<', " <"));
    let root = doc.document_element()?;
    for n in query_selector_all(&doc, root, "script, style, nav, noscript, .banner") {
        doc.detach(n);
    }
    let main = query_selector_all(&doc, root, "main")
        .into_iter()
        .next()
        .or_else(|| query_selector_all(&doc, root, "body").into_iter().next())?;
    let headings: Vec<String> = query_selector_all(&doc, main, "h1, h2, h3, h4")
        .into_iter()
        .map(|h| text_of(&doc, h))
        .filter(|h| !h.is_empty())
        .collect();
    let title = headings
        .first()
        .cloned()
        .or_else(|| query_selector_all(&doc, root, "title").into_iter().next().map(|t| text_of(&doc, t)))
        .unwrap_or_default();
    let text = text_of(&doc, main);
    let preview = query_selector_all(&doc, main, "p:not(.status)")
        .into_iter()
        .filter(|&p| !in_blockquote(&doc, p, main))
        .map(|p| text_of(&doc, p))
        .find(|t| t.chars().count() >= 40)
        .unwrap_or_else(|| text.clone());
    let preview: String = preview.chars().take(PREVIEW_CHARS).collect();
    (!text.is_empty() || !title.is_empty()).then_some((title, headings, text, preview))
}

/// Whether a site path (from the root) is left out of the index.
pub fn skipped(rel: &str) -> bool {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    rel.starts_with("api/") || rel.starts_with("code-review/") || matches!(name, "print.html" | "404.html" | "toc.html")
}

/// Every indexed page of the site at `site`, in path order.
///
/// # Errors
///
/// A directory or file that cannot be read.
pub fn collect(site: &Path) -> std::io::Result<Vec<Page>> {
    let mut files = Vec::new();
    let mut stack = vec![site.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir)? {
            let p = e?.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "html") {
                files.push(p);
            }
        }
    }
    files.sort();
    let mut pages = Vec::new();
    for f in files {
        let rel = f.strip_prefix(site).unwrap_or(&f).to_string_lossy().replace('\\', "/");
        if skipped(&rel) {
            continue;
        }
        let Some((title, headings, text, preview)) = extract_with_preview(&std::fs::read_to_string(&f)?) else { continue };
        let url = rel.strip_suffix("index.html").unwrap_or(&rel).to_string();
        pages.push(Page { kind: kind_of(&url), url, title, headings, text, preview });
    }
    Ok(pages)
}

/// A page as the search box lists it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Doc {
    pub k: &'static str,
    pub t: String,
    pub u: String,
    /// The preview: the page's first real paragraph.
    pub p: String,
}

/// What `search-text.json` holds: the pages, and every term with the pages
/// holding it (`[page index, weight]`, page order), terms sorted so the page
/// finds a prefix by binary search.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SiteIndex {
    pub docs: Vec<Doc>,
    pub terms: Vec<(String, Vec<[u32; 2]>)>,
}

/// Index `pages`: a term in a page's title weighs [`W_TITLE`], in one of its
/// headings [`W_HEADING`], and [`W_BODY`] per body occurrence up to
/// [`BODY_CAP`].
pub fn build(pages: &[Page]) -> SiteIndex {
    let mut terms: BTreeMap<String, BTreeMap<u32, u32>> = BTreeMap::new();
    for (i, p) in pages.iter().enumerate() {
        let i = i as u32;
        let mut body: BTreeMap<String, u32> = BTreeMap::new();
        for t in tokenise(&p.text) {
            *body.entry(t).or_default() += 1;
        }
        let mut add = |t: String, w: u32| *terms.entry(t).or_default().entry(i).or_default() += w;
        for t in tokenise(&p.title).into_iter().collect::<std::collections::BTreeSet<_>>() {
            add(t, W_TITLE);
        }
        for t in p.headings.iter().flat_map(|h| tokenise(h)).collect::<std::collections::BTreeSet<_>>() {
            add(t, W_HEADING);
        }
        for (t, n) in body {
            add(t, W_BODY * n.min(BODY_CAP));
        }
    }
    let docs = pages
        .iter()
        .map(|p| Doc { k: p.kind, t: p.title.clone(), u: p.url.clone(), p: p.preview.clone() })
        .collect();
    let terms = terms.into_iter().map(|(t, m)| (t, m.into_iter().map(|(d, w)| [d, w]).collect())).collect();
    SiteIndex { docs, terms }
}

/// The pages whose terms match every query word as a prefix, best first
/// (summed weight; ties by page order): the rule `search.js` applies,
/// here so the index can be tested end to end.
pub fn query(index: &SiteIndex, q: &str) -> Vec<usize> {
    let words = tokenise(q);
    if words.is_empty() {
        return Vec::new();
    }
    let mut total: BTreeMap<u32, u32> = BTreeMap::new();
    for (n, w) in words.iter().enumerate() {
        let start = index.terms.partition_point(|(t, _)| t.as_str() < w.as_str());
        let mut this: BTreeMap<u32, u32> = BTreeMap::new();
        for (_, posts) in index.terms[start..].iter().take_while(|(t, _)| t.starts_with(w.as_str())) {
            for [d, wt] in posts {
                *this.entry(*d).or_default() += wt;
            }
        }
        total = if n == 0 {
            this
        } else {
            total.into_iter().filter_map(|(d, s)| this.get(&d).map(|x| (d, s + x))).collect()
        };
    }
    let mut hits: Vec<(u32, u32)> = total.into_iter().collect();
    hits.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    hits.into_iter().map(|(d, _)| d as usize).collect()
}

/// Build the index of the site at `site` and write `search-text.json` there.
/// Returns the number of pages and terms.
///
/// # Errors
///
/// The site cannot be read, or the file cannot be written.
pub fn write_index(site: &Path) -> Result<(usize, usize), String> {
    let pages = collect(site).map_err(|e| format!("reading {}: {e}", site.display()))?;
    let index = build(&pages);
    let json = serde_json::to_string(&index).map_err(|e| e.to_string())?;
    let out = site.join("search-text.json");
    std::fs::write(&out, json).map_err(|e| format!("writing {}: {e}", out.display()))?;
    Ok((index.docs.len(), index.terms.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cases `docs/site/check-search.js` also runs through `search.js`'s
    /// tokeniser: the two must split every one identically.
    #[test]
    fn the_tokeniser_answers_the_shared_fixture() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/site/search-tokenise-fixture.json");
        let cases: Vec<(String, Vec<String>)> = serde_json::from_str(&std::fs::read_to_string(path).expect("fixture")).expect("parse");
        assert!(cases.len() >= 8);
        for (input, want) in cases {
            assert_eq!(tokenise(&input), want, "{input:?}");
        }
    }

    /// A page's sidebar and scripts are not its text; the title is its first
    /// heading; the index finds a page by a word only its body holds, by a
    /// prefix, ranks a title hit first, and ANDs the words.
    #[test]
    fn pages_are_read_and_found_by_their_full_text() {
        let a = r#"<html><head><title>T</title><script>var godiva=1</script></head><body>
            <nav>Godiva Jemima every chapter</nav>
            <main><h1>Godiva, a bare sphere</h1><h2>Leakage</h2><p>Fast neutrons leak out of the sphere &amp; are lost.</p></main></body></html>"#;
        let (title, heads, text) = extract(a).expect("page");
        assert_eq!(title, "Godiva, a bare sphere");
        assert_eq!(heads, vec!["Godiva, a bare sphere".to_string(), "Leakage".to_string()]);
        assert!(text.contains("are lost") && text.contains("& are") && !text.contains("Jemima") && !text.contains("var godiva"));
        let b = "<html><body><main><h1>Jemima</h1><p>Godiva is mentioned here; uranium cylinders.</p></main></body></html>";
        let pages: Vec<Page> = [("tutorials/mc/godiva.html", a), ("deep-dives/mc/jemima.html", b)]
            .iter()
            .map(|(u, h)| {
                let (title, headings, text, preview) = extract_with_preview(h).expect("page");
                Page { url: u.to_string(), kind: kind_of(u), title, headings, text, preview }
            })
            .collect();
        let idx = build(&pages);
        assert_eq!(idx.docs[0].k, "science");
        assert_eq!(idx.docs[1].k, "code");
        assert!(idx.terms.windows(2).all(|w| w[0].0 < w[1].0), "terms sorted");
        assert_eq!(query(&idx, "godiva"), vec![0, 1], "the title hit ranks first");
        assert_eq!(query(&idx, "cylind"), vec![1], "a prefix of a body word");
        assert_eq!(query(&idx, "godiva leak"), vec![0], "every word must match");
        assert!(query(&idx, "jemima uranium leak").is_empty());
        assert!(query(&idx, "  ").is_empty());
    }

    /// The banner is not indexed; the preview is the first real paragraph,
    /// not the review-status quote nor a status line.
    #[test]
    fn the_preview_is_the_first_real_paragraph() {
        let h = r#"<html><body><main><p class="banner">Research, education and V&amp;V only, on every page.</p>
            <h1>Delta tracking</h1><blockquote><p>Review status: AI-assisted first draft, not yet reviewed by a human.</p></blockquote>
            <p class="status">Status: first draft, 2026-10-04, tracking issue 123 and more words here.</p>
            <p>Short.</p><p>Woodcock tracking samples flights on a majorant and never looks for a surface.</p></main></body></html>"#;
        let (_, _, text, preview) = extract_with_preview(h).expect("page");
        assert!(!text.contains("on every page"), "{text}");
        assert!(preview.starts_with("Woodcock tracking"), "{preview}");
    }

    /// API pages, Code Review and mdBook's print/404/toc pages are skipped;
    /// a directory's index page is listed by its directory.
    #[test]
    fn the_walk_skips_what_has_its_own_search() {
        for s in ["api/outram_mc_libs/index.html", "code-review/index.html", "deep-dives/x/print.html", "tutorials/y/404.html", "z/toc.html"] {
            assert!(skipped(s), "{s}");
        }
        assert!(!skipped("tutorials/monte-carlo/godiva.html"));
        let dir = std::env::temp_dir().join(format!("site_search_{}", std::process::id()));
        std::fs::create_dir_all(dir.join("api")).expect("mkdir");
        std::fs::create_dir_all(dir.join("demos/monte-carlo")).expect("mkdir");
        std::fs::write(dir.join("api/index.html"), "<main><h1>API</h1></main>").expect("write");
        std::fs::write(dir.join("demos/monte-carlo/index.html"), "<main><h1>Monte Carlo demo</h1><p>Watch neutrons.</p></main>").expect("write");
        let (docs, terms) = write_index(&dir).expect("index");
        let json = std::fs::read_to_string(dir.join("search-text.json")).expect("read");
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(docs, 1);
        assert!(terms >= 4);
        assert!(json.contains(r#""u":"demos/monte-carlo/""#) && json.contains(r#""k":"demo""#), "{json}");
    }
}
