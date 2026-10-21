//! CSL citations from a `.bib` file, and the citation pass over the Pages
//! site's markdown and HTML (GitHub #789, #797).
//!
//! # Pipeline
//!
//! 1. [`parse_bib_entries`] reads the `.bib` file and keeps each entry's cite
//!    key.
//! 2. Each entry is re-serialised on its own and imported through the ported
//!    Zotero BibTeX translator ([`Translator::BibTeX`]): TeX unescaping,
//!    `Last, First and ...` name splitting, `--` to an en dash, and the
//!    BibTeX-to-Zotero item-type map, all as Zotero does them. An entry type
//!    the importer drops (`@standard`) is imported as `@misc`.
//! 3. The Zotero item becomes CSL-JSON through the ported `itemToCSLJSON`
//!    ([`item_to_csl_json`]), with the cite key as its `id`. This is the
//!    input Zotero itself hands its CSL engine.
//! 4. The [citeproc-js port](crate::citeproc) (GitHub #790; citeproc-js
//!    2.4.63, Zotero's CSL engine, by Frank Bennett, AGPL-3.0 option of its
//!    "CPAL-1.0 or AGPL-3.0-or-later" licence) renders citations and the
//!    bibliography from that CSL-JSON with a CSL style and locale
//!    ([`CslStyle`]). The Pages site uses APA 7, `data/csl/apa.csl`, pinned
//!    from the official CSL styles repository; the other four pinned styles
//!    are in [`CslStyle::bundled`].
//!
//! **~~hayagriva~~ CORRECTED 2026-10-08 (#797).** Until this change step 4 was
//! [hayagriva](https://github.com/typst/hayagriva) 0.10.1, which differed from
//! citeproc-js in 176 of 669 comparisons on the site's 223 entries (GitHub
//! #790). The dependency is removed.
//!
//! # How the engine is driven
//!
//! Exactly as `scripts/csl-reference.cjs` drives citeproc-js: one engine
//! (HTML output) per document, `updateItems` with every cited work, then per
//! citation `makeCitationCluster([{id}])` (parenthetical) or, for a
//! narrative citation, the author-only cluster, a space and the
//! suppress-author cluster, the composition Zotero's word-processor plugins
//! use; the bibliography is `makeBibliography()` in the engine's order.
//!
//! # Verification
//!
//! `tests/csl_vs_citeproc_js.rs` compares this module's output, on every
//! entry of the site's `.bib` under five styles, with citeproc-js run on the
//! same CSL-JSON, style and locale by `scripts/csl-reference.sh`. See that
//! test's doc comment for the method and the measured result.
//!
//! [`parse_bib_entries`]: crate::parse_bib_entries
//! [`Translator::BibTeX`]: crate::zotero::translators::Translator::BibTeX
//! [`item_to_csl_json`]: kovan_common::zotero::csl::item_to_csl_json

use std::collections::BTreeMap;
use std::sync::Arc;

use kovan_common::zotero::item::ZoteroItem;
use serde_json::{json, Value};

use crate::citeproc::{CitationItem, Engine, OutputFormat, Sys};
use crate::zotero::translators::Translator;

/// The opening marker of the generated reference list on a page.
pub const REFERENCES_BEGIN: &str = "<!-- references:begin -->";
/// The closing marker of the generated reference list on a page.
pub const REFERENCES_END: &str = "<!-- references:end -->";

/// APA 7 (`apa.csl`, CSL styles repository, pinned; see
/// `data/csl/README.md`).
pub const APA_CSL: &str = include_str!("../data/csl/apa.csl");
/// Chicago Manual of Style 18th edition, author-date (pinned).
pub const CHICAGO_AUTHOR_DATE_CSL: &str = include_str!("../data/csl/chicago-author-date.csl");
/// IEEE (pinned).
pub const IEEE_CSL: &str = include_str!("../data/csl/ieee.csl");
/// Nature (pinned; its default locale is `en-GB`).
pub const NATURE_CSL: &str = include_str!("../data/csl/nature.csl");
/// NLM citation-sequence, the parent of the dependent "Vancouver - NLM"
/// style (pinned).
pub const VANCOUVER_CSL: &str = include_str!("../data/csl/nlm-citation-sequence.csl");
/// The CSL `en-US` locale (CSL locales repository, pinned).
pub const LOCALE_EN_US: &str = include_str!("../data/csl/locales-en-US.xml");
/// The CSL `en-GB` locale (CSL locales repository, pinned).
pub const LOCALE_EN_GB: &str = include_str!("../data/csl/locales-en-GB.xml");

/// The three in-text forms, chosen on a page by how the link text is written
/// (see [`cite_page`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CiteForm {
    /// `(Romano et al., 2015)`: the style's ordinary citation.
    Parenthetical,
    /// `Romano et al. (2015)`: the authors are part of the sentence
    /// (citeproc-js: the author-only cluster, a space, the suppress-author
    /// cluster).
    Narrative,
    /// `Romano et al., 2015`: the parenthetical citation without its outer
    /// parentheses, for several works inside one pair the author writes,
    /// `([..](#ref-a); [..](#ref-b))`.
    Bare,
}

/// A CSL style with the locales the engine may ask for and the language it
/// starts from.
#[derive(Debug, Clone)]
pub struct CslStyle {
    xml: String,
    lang: String,
    locales: Arc<BTreeMap<String, String>>,
}

impl CslStyle {
    /// A CSL style (independent) and its `en-US` locale file. The pinned
    /// `en-GB` locale is available too, as in `scripts/csl-reference.cjs`
    /// (whose `retrieveLocale` serves every `data/csl/locales-*.xml`).
    ///
    /// # Errors
    ///
    /// The engine refuses the style (citeproc-js is lenient: it raises only
    /// for what `new CSL.Engine` itself rejects, e.g. no CSL root element).
    pub fn from_xml(style_xml: &str, locale_xml: &str) -> Result<Self, String> {
        let mut locales = BTreeMap::new();
        locales.insert("en-GB".to_string(), LOCALE_EN_GB.to_string());
        locales.insert("en-US".to_string(), locale_xml.to_string());
        let style = CslStyle {
            xml: style_xml.to_string(),
            lang: "en-US".to_string(),
            locales: Arc::new(locales),
        };
        // Parse once now so a bad style fails here, not at the first render.
        style
            .engine(Sys::default())
            .map_err(|e| format!("CSL style: {e}"))?;
        Ok(style)
    }

    /// The same style starting from another language (`en-GB` for Nature).
    pub fn with_lang(mut self, lang: &str) -> Self {
        self.lang = lang.to_string();
        self
    }

    /// APA 7 with the `en-US` locale, both pinned in this crate.
    pub fn apa() -> Self {
        Self::from_xml(APA_CSL, LOCALE_EN_US).expect("the pinned APA style and locale parse")
    }

    /// One of the five pinned site styles by name: `apa`,
    /// `chicago-author-date`, `ieee`, `nature` (with `en-GB`), `vancouver`.
    pub fn bundled(name: &str) -> Option<Self> {
        let (xml, lang) = match name {
            "apa" => (APA_CSL, "en-US"),
            "chicago-author-date" => (CHICAGO_AUTHOR_DATE_CSL, "en-US"),
            "ieee" => (IEEE_CSL, "en-US"),
            "nature" => (NATURE_CSL, "en-GB"),
            "vancouver" => (VANCOUVER_CSL, "en-US"),
            _ => return None,
        };
        Self::from_xml(xml, LOCALE_EN_US)
            .ok()
            .map(|s| s.with_lang(lang))
    }

    /// A fresh engine over `sys` (whose locales are replaced by this style's),
    /// HTML output: `new CSL.Engine(sys, style, lang)` and `setOutputFormat`.
    fn engine(&self, mut sys: Sys) -> Result<Engine, String> {
        sys.locales = Arc::clone(&self.locales);
        let mut engine = Engine::new(sys, &self.xml, &self.lang).map_err(|e| e.to_string())?;
        engine.set_output_format(OutputFormat::Html);
        Ok(engine)
    }
}

/// Every entry of a `.bib` file as CSL-JSON, keyed by cite key.
#[derive(Debug, Clone, Default)]
pub struct CslLibrary {
    json: BTreeMap<String, Value>,
}

impl CslLibrary {
    /// Parse every entry of `bib` through the Zotero BibTeX import and
    /// `itemToCSLJSON`.
    ///
    /// # Errors
    ///
    /// The `.bib` does not parse, two entries share a cite key, or an entry
    /// cannot be imported or converted.
    pub fn from_bib(bib: &str) -> Result<Self, String> {
        let entries = crate::parse_bib_entries(bib).map_err(|e| e.to_string())?;
        let mut lib = CslLibrary::default();
        for e in &entries {
            if lib.json.contains_key(&e.cite_key) {
                return Err(format!("duplicate cite key {}", e.cite_key));
            }
            let csl = entry_to_csl_json(&e.entry_type, &e.cite_key, &e.fields)
                .or_else(|_| entry_to_csl_json("misc", &e.cite_key, &e.fields))
                .map_err(|err| format!("{}: {err}", e.cite_key))?;
            lib.json.insert(e.cite_key.clone(), csl);
        }
        Ok(lib)
    }

    /// Whether the library holds this cite key.
    pub fn contains(&self, key: &str) -> bool {
        self.json.contains_key(key)
    }

    /// The CSL-JSON of one entry (what both this module and the citeproc-js
    /// reference render).
    pub fn csl_json(&self, key: &str) -> Option<&Value> {
        self.json.get(key)
    }

    /// Every cite key, sorted.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.json.keys().map(String::as_str)
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.json.len()
    }

    /// Whether the library is empty.
    pub fn is_empty(&self) -> bool {
        self.json.is_empty()
    }

    /// Render a document: `cites` in order, each one key and form; returns
    /// each citation's text (plain) and the bibliography as `(key, HTML)` in
    /// the style's order.
    ///
    /// One engine holds the document: `updateItems` receives the distinct
    /// keys in order of first citation, so disambiguation (`2015a`, given
    /// names) and a citation-number style see exactly the works cited.
    /// Driven as `scripts/csl-reference.cjs` drives citeproc-js; see the
    /// module docs.
    ///
    /// # Errors
    ///
    /// A key the library does not hold (callers check [`Self::contains`]), or
    /// an error the engine raises (a style the engine cannot run).
    pub fn render(
        &self,
        style: &CslStyle,
        cites: &[(String, CiteForm)],
    ) -> Result<Rendered, String> {
        let mut ids: Vec<String> = Vec::new();
        let mut items = BTreeMap::new();
        for (key, _) in cites {
            let item = self
                .json
                .get(key)
                .ok_or_else(|| format!("unknown cite key `{key}`"))?;
            if !ids.contains(key) {
                ids.push(key.clone());
                items.insert(key.clone(), item.clone());
            }
        }
        if ids.is_empty() {
            return Ok(Rendered {
                citations: Vec::new(),
                bibliography: Vec::new(),
                hanging_indent: false,
            });
        }
        let sys = Sys {
            items: Arc::new(items),
            ..Sys::default()
        };
        let mut engine = style.engine(sys)?;
        engine
            .update_items(&ids, false)
            .map_err(|e| e.to_string())?;

        let cluster = |engine: &mut Engine, item: Value| -> Result<String, String> {
            let item = CitationItem::from_json(&item).map_err(|e| e.to_string())?;
            engine
                .make_citation_cluster(&[item])
                .map_err(|e| e.to_string())
        };
        let mut citations = Vec::with_capacity(cites.len());
        for (key, form) in cites {
            let text = match form {
                CiteForm::Parenthetical => {
                    html_to_text(&cluster(&mut engine, json!({ "id": key }))?)
                }
                CiteForm::Bare => {
                    strip_outer_parens(&html_to_text(&cluster(&mut engine, json!({ "id": key }))?))
                }
                CiteForm::Narrative => {
                    let author = cluster(&mut engine, json!({ "id": key, "author-only": true }))?;
                    let year = cluster(&mut engine, json!({ "id": key, "suppress-author": true }))?;
                    html_to_text(&format!("{author} {year}")).trim().to_string()
                }
            };
            citations.push(text);
        }

        let bib = engine.make_bibliography(None).map_err(|e| e.to_string())?;
        let order: Vec<String> = bib
            .params
            .get("entry_ids")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|e| e.get(0).and_then(Value::as_str).map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let hanging = bib
            .params
            .get("hangingindent")
            .is_some_and(|v| !matches!(v, Value::Bool(false) | Value::Null));
        let bibliography = order
            .into_iter()
            .zip(bib.entries.iter().map(|e| e.trim().to_string()))
            .collect();
        Ok(Rendered {
            citations,
            bibliography,
            hanging_indent: hanging,
        })
    }
}

/// The output of [`CslLibrary::render`].
#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    /// Each citation, plain text, in request order.
    pub citations: Vec<String>,
    /// The bibliography, `(cite key, entry HTML)`, in the style's order. The
    /// entry is citeproc-js's, whole: `<div class="csl-entry">...</div>`.
    pub bibliography: Vec<(String, String)>,
    /// Whether the style asks for a hanging indent (`hangingindent` in the
    /// `makeBibliography` parameters).
    pub hanging_indent: bool,
}

/// citeproc-js's HTML to plain text: tags dropped, the entities its HTML
/// output uses decoded.
fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&#38;", "&")
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

/// Escape plain text for HTML.
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn strip_outer_parens(s: &str) -> String {
    match s.strip_prefix('(').and_then(|t| t.strip_suffix(')')) {
        Some(inner) => inner.to_string(),
        None => s.to_string(),
    }
}

/// One `.bib` entry -> Zotero BibTeX import -> `itemToCSLJSON`, `id` = key.
fn entry_to_csl_json(
    entry_type: &str,
    key: &str,
    fields: &BTreeMap<String, String>,
) -> Result<serde_json::Value, String> {
    let mut s = format!("@{entry_type}{{{key},\n");
    for (k, v) in fields {
        s.push_str(&format!("  {k} = {{{v}}},\n"));
    }
    s.push_str("}\n");
    let opts = Translator::BibTeX.default_options();
    let result = Translator::BibTeX
        .import(&s, &opts)
        .map_err(|e| e.to_string())?;
    let api = result.api_json();
    let first = api.first().ok_or("the BibTeX importer produced no item")?;
    let zitem = ZoteroItem::from_json_value(first).map_err(|e| e.to_string())?;
    let mut csl = kovan_common::zotero::csl::item_to_csl_json(&zitem).map_err(|e| e.to_string())?;
    csl.insert("id".into(), key.into());
    Ok(serde_json::Value::Object(csl))
}

/// The outcome of [`cite_page`] on one page.
#[derive(Debug, Clone, PartialEq)]
pub struct CitedPage {
    /// The page with every citation and the reference list regenerated.
    pub text: String,
    /// The cite keys the page cites, in first-citation order.
    pub keys: Vec<String>,
}

/// Regenerate the citations of one markdown page.
///
/// A citation is a link whose target is `#ref-KEY`. Its form comes from how
/// its text is written: empty or starting with `(` is
/// [`CiteForm::Parenthetical`], ending with `)` is [`CiteForm::Narrative`],
/// anything else is [`CiteForm::Bare`]. The text is replaced by the
/// citation the style renders. Links in fenced code blocks and inline code
/// are left alone.
///
/// The page is one CSL document: its citations are rendered together, in
/// reading order, so disambiguation (APA's `2015a`/`2015b`) holds across the
/// page. The reference list, between [`REFERENCES_BEGIN`] and
/// [`REFERENCES_END`], is a `## References` heading and one paragraph per
/// work in the style's order, each with the `id="ref-KEY"` the links point
/// at. A page that cites nothing loses its list; a page that cites something
/// and has no markers gets the list appended.
///
/// # Errors
///
/// Every cite key the library does not hold, one message each.
pub fn cite_page(page: &str, lib: &CslLibrary, style: &CslStyle) -> Result<CitedPage, Vec<String>> {
    cite_document(page, lib, style, PageKind::Markdown)
}

/// [`cite_page`] for an HTML page (GitHub #789: the landing page and the
/// demos are HTML, not markdown).
///
/// A citation is `<a href="#ref-KEY">text</a>`, the text read as in
/// [`cite_page`] and replaced by the rendered citation (HTML-escaped).
/// Anything inside `<pre>`, `<code>`, `<script>` or `<style>` is left alone.
/// The reference list between the same markers is an `<h2>References</h2>`
/// and one entry per work, with the `id="ref-KEY"` the links point at.
///
/// # Errors
///
/// Every cite key the library does not hold, one message each.
pub fn cite_html_page(
    page: &str,
    lib: &CslLibrary,
    style: &CslStyle,
) -> Result<CitedPage, Vec<String>> {
    cite_document(page, lib, style, PageKind::Html)
}

/// Which markup a page is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PageKind {
    Markdown,
    Html,
}

/// The form a citation link's text asks for.
fn form_of(text: &str) -> CiteForm {
    let t = text.trim();
    if t.is_empty() || t.starts_with('(') {
        CiteForm::Parenthetical
    } else if t.ends_with(')') {
        CiteForm::Narrative
    } else {
        CiteForm::Bare
    }
}

fn cite_document(
    page: &str,
    lib: &CslLibrary,
    style: &CslStyle,
    kind: PageKind,
) -> Result<CitedPage, Vec<String>> {
    // Drop the old generated list first, so its anchors are not read back.
    const SLOT: &str = "\u{0}REFS\u{0}";
    let (body, tail) = match (page.find(REFERENCES_BEGIN), page.find(REFERENCES_END)) {
        (Some(b), Some(e)) if e > b => (
            format!("{}{SLOT}", &page[..b]),
            page[e + REFERENCES_END.len()..].to_string(),
        ),
        _ => (page.to_string(), String::new()),
    };

    // Pass 1: find every citation link outside code.
    let mut links: Vec<Link> = Vec::new();
    let mut errors = Vec::new();
    let found: Vec<Link> = match kind {
        PageKind::Markdown => {
            let mut v = Vec::new();
            let mut offset = 0;
            let mut in_fence = false;
            for line in body.split_inclusive('\n') {
                let t = line.trim_start();
                if t.starts_with("```") || t.starts_with("~~~") {
                    in_fence = !in_fence;
                } else if !in_fence {
                    for mut l in find_links(line) {
                        l.start += offset;
                        l.end += offset;
                        v.push(l);
                    }
                }
                offset += line.len();
            }
            v
        }
        PageKind::Html => find_html_links(&body),
    };
    for l in found {
        if lib.contains(&l.key) {
            links.push(l);
        } else {
            errors.push(format!("unknown cite key `{}`", l.key));
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    // Pass 2: render the page as one document, then splice.
    let cites: Vec<(String, CiteForm)> = links.iter().map(|l| (l.key.clone(), l.form)).collect();
    let rendered = lib.render(style, &cites).map_err(|e| vec![e])?;
    let mut out = String::with_capacity(body.len());
    let mut at = 0;
    for (l, text) in links.iter().zip(&rendered.citations) {
        out.push_str(&body[at..l.start]);
        match kind {
            PageKind::Markdown => out.push_str(&format!("[{text}](#ref-{})", l.key)),
            PageKind::Html => out.push_str(&format!(
                "<a href=\"#ref-{}\">{}</a>",
                l.key,
                escape_html(text)
            )),
        }
        at = l.end;
    }
    out.push_str(&body[at..]);
    let mut keys: Vec<String> = Vec::new();
    for l in &links {
        if !keys.contains(&l.key) {
            keys.push(l.key.clone());
        }
    }

    let block = if keys.is_empty() {
        String::new()
    } else {
        let heading = match kind {
            PageKind::Markdown => "## References",
            PageKind::Html => "<h2>References</h2>",
        };
        let mut s = format!("{REFERENCES_BEGIN}\n{heading}\n\n");
        for (key, entry) in &rendered.bibliography {
            s.push_str(&reference_entry(key, entry, rendered.hanging_indent));
            s.push_str("\n\n");
        }
        s.push_str(REFERENCES_END);
        s
    };

    let text = if out.contains(SLOT) {
        if block.is_empty() {
            let head = out.replace(SLOT, "");
            let mut t = head.trim_end().to_string();
            let rest = tail.trim_start_matches('\n');
            if !rest.trim().is_empty() {
                t.push_str("\n\n");
                t.push_str(rest);
            }
            if !t.ends_with('\n') {
                t.push('\n');
            }
            t
        } else {
            let mut t = out.replace(SLOT, &block);
            t.push_str(&tail);
            t
        }
    } else if block.is_empty() {
        out
    } else {
        let mut t = out.trim_end().to_string();
        t.push_str("\n\n");
        t.push_str(&block);
        t.push('\n');
        t
    };
    Ok(CitedPage { text, keys })
}

/// One reference-list entry carrying the `id="ref-KEY"` the citations link
/// to. An entry citeproc-js wraps as `<div class="csl-entry">inner</div>`
/// with no division inside becomes a paragraph; one with divisions inside
/// (the `csl-left-margin` of a numbered style) stays a division.
fn reference_entry(key: &str, entry: &str, hanging: bool) -> String {
    const OPEN: &str = "<div class=\"csl-entry\">";
    let style = if hanging {
        " style=\"padding-left: 2em; text-indent: -2em;\""
    } else {
        ""
    };
    match entry
        .strip_prefix(OPEN)
        .and_then(|r| r.strip_suffix("</div>"))
    {
        Some(inner) if !inner.contains("<div") => {
            format!("<p class=\"csl-entry\" id=\"ref-{key}\"{style}>{inner}</p>")
        }
        Some(inner) => format!("<div class=\"csl-entry\" id=\"ref-{key}\"{style}>{inner}</div>"),
        None => format!("<div id=\"ref-{key}\"{style}>{entry}</div>"),
    }
}

/// The citation links of an HTML page: `<a href="#ref-KEY">text</a>` outside
/// `<pre>`, `<code>`, `<script>` and `<style>`.
fn find_html_links(html: &str) -> Vec<Link> {
    let lower = html.to_ascii_lowercase();
    let mut skips: Vec<(usize, usize)> = Vec::new();
    for tag in ["pre", "code", "script", "style"] {
        let (open, close) = (format!("<{tag}"), format!("</{tag}>"));
        let mut from = 0;
        while let Some(s) = lower[from..].find(&open).map(|i| i + from) {
            let after = lower.as_bytes().get(s + open.len()).copied();
            if !matches!(after, Some(b'>' | b' ' | b'\n' | b'\t')) {
                from = s + open.len();
                continue;
            }
            let e = lower[s..]
                .find(&close)
                .map(|i| s + i + close.len())
                .unwrap_or(lower.len());
            skips.push((s, e));
            from = e;
        }
    }
    const HREF: &str = "<a href=\"#ref-";
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(s) = html[from..].find(HREF).map(|i| i + from) {
        from = s + HREF.len();
        if skips.iter().any(|&(a, b)| s >= a && s < b) {
            continue;
        }
        let rest = &html[s + HREF.len()..];
        let Some(q) = rest.find('"') else { continue };
        let key = &rest[..q];
        let Some(gt) = rest[q..].find('>').map(|i| i + q) else {
            continue;
        };
        let Some(end_a) = rest[gt..].find("</a>").map(|i| i + gt) else {
            continue;
        };
        let text = &rest[gt + 1..end_a];
        let valid = !key.is_empty()
            && key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "_-:.+".contains(c));
        if !valid || text.contains('<') {
            continue;
        }
        let end = s + HREF.len() + end_a + "</a>".len();
        out.push(Link {
            start: s,
            end,
            key: key.to_string(),
            form: form_of(&html_to_text(text)),
        });
        from = end;
    }
    out
}

/// One `[text](#ref-KEY)` link in a page.
#[derive(Debug, Clone)]
struct Link {
    start: usize,
    end: usize,
    key: String,
    form: CiteForm,
}

/// The citation links of one non-fenced line, skipping inline code.
fn find_links(line: &str) -> Vec<Link> {
    let b = line.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let mut in_code = false;
    while i < b.len() {
        match b[i] {
            b'`' => in_code = !in_code,
            b'[' if !in_code => {
                if let Some((close, key, end)) = parse_ref_link(line, i) {
                    let t = line[i + 1..close].trim();
                    let form = form_of(t);
                    out.push(Link {
                        start: i,
                        end,
                        key: key.to_string(),
                        form,
                    });
                    i = end;
                    continue;
                }
            }
            _ => {}
        }
        i += 1;
    }
    out
}

/// At `start` (a `[`), match `[text](#ref-KEY)`: the index of the closing
/// `]`, the key, and the index just past the `)`.
fn parse_ref_link(line: &str, start: usize) -> Option<(usize, &str, usize)> {
    let b = line.as_bytes();
    let mut depth = 0usize;
    let mut j = start;
    let close = loop {
        match *b.get(j)? {
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    break j;
                }
            }
            b'\n' => return None,
            _ => {}
        }
        j += 1;
    };
    let target = line[close + 1..].strip_prefix("(#ref-")?;
    let end = target.find(')')?;
    let key = &target[..end];
    let valid = !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_-:.+".contains(c));
    valid.then_some((close, key, close + 1 + "(#ref-".len() + end + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BIB: &str = r#"
@article{openmc,
  author  = {Romano, Paul K. and Horelik, Nicholas E. and Herman, Bryan R. and Nelson, Adam G. and Forget, Benoit and Smith, Kord},
  title   = {{OpenMC}: A State-of-the-Art {M}onte {C}arlo Code for Research and Development},
  journal = {Annals of Nuclear Energy},
  volume  = {82},
  pages   = {90--97},
  year    = {2015},
  doi     = {10.1016/j.anucene.2014.07.048}
}
@article{coolprop,
  author  = {Bell, Ian H. and Lemort, Vincent},
  title   = {Pure and Pseudo-Pure Fluid Properties},
  journal = {Industrial \& Engineering Chemistry Research},
  volume  = {53},
  number  = {6},
  pages   = {2498--2508},
  year    = {2014}
}
@book{iapws97,
  author    = {Wagner, Wolfgang and Kretzschmar, Hans-Joachim},
  title     = {International Steam Tables},
  publisher = {Springer},
  year      = {2008}
}
@standard{nqa1,
  author = {{American Society of Mechanical Engineers}},
  title  = {Quality Assurance Requirements for Nuclear Facility Applications},
  year   = {2024}
}
@article{same1,
  author = {Smith, Ann and Jones, Bob and Brown, Cy},
  title  = {Beta work},
  journal = {J},
  year   = {2020}
}
@article{same2,
  author = {Smith, Ann and Jones, Bob and Brown, Cy},
  title  = {Alpha work},
  journal = {J},
  year   = {2020}
}
"#;

    fn setup() -> (CslLibrary, CslStyle) {
        (CslLibrary::from_bib(BIB).unwrap(), CslStyle::apa())
    }

    #[test]
    fn the_bib_goes_through_the_zotero_import_to_csl_json() {
        let (lib, _) = setup();
        assert_eq!(lib.len(), 6);
        let j = lib.csl_json("openmc").unwrap();
        assert_eq!(j["id"], "openmc");
        assert_eq!(j["type"], "article-journal");
        assert_eq!(j["container-title"], "Annals of Nuclear Energy");
        assert_eq!(j["page"], "90–97");
        assert_eq!(j["author"][0]["family"], "Romano");
        assert_eq!(j["author"][0]["given"], "Paul K.");
        // @standard is not a BibTeX type the importer knows: imported as @misc.
        assert!(lib.contains("nqa1"));
    }

    #[test]
    fn apa_citations_in_all_three_forms() {
        let (lib, style) = setup();
        let r = lib
            .render(
                &style,
                &[
                    ("openmc".into(), CiteForm::Parenthetical),
                    ("coolprop".into(), CiteForm::Narrative),
                    ("iapws97".into(), CiteForm::Bare),
                ],
            )
            .unwrap();
        assert_eq!(
            r.citations,
            [
                "(Romano et al., 2015)",
                "Bell & Lemort (2014)",
                "Wagner & Kretzschmar, 2008"
            ]
        );
        let keys: Vec<&str> = r.bibliography.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            keys,
            ["coolprop", "openmc", "iapws97"],
            "APA sorts by author"
        );
        let openmc = &r.bibliography[1].1;
        // citeproc-js writes `&` as `&#38;` in HTML output.
        assert!(openmc.contains("Romano, P. K., Horelik, N. E., Herman, B. R., Nelson, A. G., Forget, B., &#38; Smith, K. (2015)."), "{openmc}");
        assert!(
            openmc.contains("https://doi.org/10.1016/j.anucene.2014.07.048"),
            "{openmc}"
        );
    }

    #[test]
    fn a_page_is_one_document_so_same_year_works_get_letters() {
        let (lib, style) = setup();
        let page = "A [](#ref-same1) and B [](#ref-same2).\n";
        let out = cite_page(page, &lib, &style).unwrap();
        assert!(
            out.text.contains("[(Smith et al., 2020a)](#ref-same2)"),
            "{}",
            out.text
        );
        assert!(
            out.text.contains("[(Smith et al., 2020b)](#ref-same1)"),
            "{}",
            out.text
        );
    }

    #[test]
    fn a_page_gets_its_citations_and_a_reference_list_and_is_idempotent() {
        let (lib, style) = setup();
        let page = "# Title\n\nTransport follows [](#ref-openmc). \
                    [Bell and Lemort ()](#ref-coolprop) fit it ([x](#ref-iapws97)).\n\n\
                    ```rust\nlet s = \"[](#ref-missing)\";\n```\n\nIn code: `[](#ref-missing)`.\n";
        let out = cite_page(page, &lib, &style).unwrap();
        assert_eq!(out.keys, ["openmc", "coolprop", "iapws97"]);
        assert!(out
            .text
            .contains("Transport follows [(Romano et al., 2015)](#ref-openmc)."));
        assert!(out
            .text
            .contains("[Bell & Lemort (2014)](#ref-coolprop) fit"));
        assert!(out
            .text
            .contains("([Wagner & Kretzschmar, 2008](#ref-iapws97))"));
        assert!(
            out.text.contains("\"[](#ref-missing)\""),
            "fenced code is left alone"
        );
        assert!(
            out.text.contains("`[](#ref-missing)`"),
            "inline code is left alone"
        );
        let list = &out.text[out.text.find(REFERENCES_BEGIN).unwrap()..];
        assert!(list.contains("## References"));
        assert!(list.contains("id=\"ref-openmc\""));
        assert_eq!(cite_page(&out.text, &lib, &style).unwrap().text, out.text);
    }

    #[test]
    fn unknown_keys_fail_and_an_uncited_page_loses_its_list() {
        let (lib, style) = setup();
        let err =
            cite_page("See [](#ref-nope) and [](#ref-also-nope).\n", &lib, &style).unwrap_err();
        assert_eq!(
            err,
            ["unknown cite key `nope`", "unknown cite key `also-nope`"]
        );

        let cited = cite_page("A [](#ref-openmc).\n\nMore.\n", &lib, &style)
            .unwrap()
            .text;
        let uncited = cited.replace("[(Romano et al., 2015)](#ref-openmc)", "nothing");
        let out = cite_page(&uncited, &lib, &style).unwrap();
        assert!(out.keys.is_empty());
        assert_eq!(out.text, "A nothing.\n\nMore.\n");
        assert_eq!(
            cite_page("Plain.\n", &lib, &style).unwrap().text,
            "Plain.\n"
        );
    }

    #[test]
    fn the_five_site_styles_are_bundled_and_render() {
        let (lib, _) = setup();
        for name in ["apa", "chicago-author-date", "ieee", "nature", "vancouver"] {
            let style = CslStyle::bundled(name).unwrap_or_else(|| panic!("{name}"));
            let r = lib
                .render(&style, &[("openmc".into(), CiteForm::Parenthetical)])
                .unwrap();
            assert_eq!(r.bibliography.len(), 1, "{name}");
            assert!(!r.citations[0].is_empty(), "{name}");
        }
        assert!(CslStyle::bundled("nope").is_none());
    }

    #[test]
    fn an_unknown_key_is_an_error_not_a_panic() {
        let (lib, style) = setup();
        assert!(lib
            .render(&style, &[("nope".into(), CiteForm::Parenthetical)])
            .is_err());
        assert!(lib.render(&style, &[]).unwrap().citations.is_empty());
    }

    #[test]
    fn an_html_page_gets_citations_and_a_list_and_is_idempotent() {
        let (lib, style) = setup();
        let page = "<html><body><p>Transport follows <a href=\"#ref-openmc\"></a>. \
                    <a href=\"#ref-coolprop\">Bell and Lemort ()</a> fit it.</p>\n\
                    <pre><a href=\"#ref-missing\"></a></pre>\n<code><a href=\"#ref-missing\"></a></code>\n</body></html>\n";
        let out = cite_html_page(page, &lib, &style).unwrap();
        assert_eq!(out.keys, ["openmc", "coolprop"]);
        assert!(
            out.text
                .contains("<a href=\"#ref-openmc\">(Romano et al., 2015)</a>"),
            "{}",
            out.text
        );
        assert!(
            out.text
                .contains("<a href=\"#ref-coolprop\">Bell &amp; Lemort (2014)</a>"),
            "{}",
            out.text
        );
        assert!(out
            .text
            .contains("<pre><a href=\"#ref-missing\"></a></pre>"));
        assert!(out.text.contains("<h2>References</h2>"));
        assert!(out.text.contains("id=\"ref-openmc\""));
        assert_eq!(
            cite_html_page(&out.text, &lib, &style).unwrap().text,
            out.text
        );
        let err = cite_html_page("<a href=\"#ref-nope\">x</a>", &lib, &style).unwrap_err();
        assert_eq!(err, ["unknown cite key `nope`"]);
    }

    #[test]
    fn a_numbered_style_keeps_its_division_entries() {
        let (lib, _) = setup();
        let ieee = CslStyle::bundled("ieee").unwrap();
        let out = cite_page("A [](#ref-openmc).\n", &lib, &ieee).unwrap();
        assert!(out.text.contains("[[1]](#ref-openmc)"), "{}", out.text);
        assert!(out.text.contains("id=\"ref-openmc\""), "{}", out.text);
    }
}
