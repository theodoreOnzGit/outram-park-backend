//! CSL citations from a `.bib` file, and the citation pass over the Pages
//! site's markdown (GitHub #789).
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
//! 4. [hayagriva](https://github.com/typst/hayagriva) renders citations and
//!    the bibliography from that CSL-JSON with a CSL style and locale
//!    ([`CslStyle`]). The Pages site uses APA 7, `data/csl/apa.csl`, pinned
//!    from the official CSL styles repository.
//!
//! # Verification
//!
//! `tests/csl_vs_citeproc_js.rs` compares this module's output, on every
//! entry of the site's `.bib`, with citeproc-js (Zotero's CSL engine) run on
//! the same CSL-JSON, style and locale by `scripts/csl-reference.sh`. See that
//! test's doc comment for the method and the measured result.
//!
//! [`parse_bib_entries`]: crate::parse_bib_entries
//! [`Translator::BibTeX`]: crate::zotero::translators::Translator::BibTeX
//! [`item_to_csl_json`]: kovan_common::zotero::csl::item_to_csl_json

use std::collections::BTreeMap;

use hayagriva::citationberg::json::Item as CslJsonItem;
use hayagriva::citationberg::{IndependentStyle, Locale, LocaleFile};
use hayagriva::{
    BibliographyDriver, BibliographyRequest, BufWriteFormat, CitationItem, CitationRequest,
    CitePurpose, ElemChildren,
};
use kovan_common::zotero::item::ZoteroItem;

use crate::zotero::translators::Translator;

/// The opening marker of the generated reference list on a page.
pub const REFERENCES_BEGIN: &str = "<!-- references:begin -->";
/// The closing marker of the generated reference list on a page.
pub const REFERENCES_END: &str = "<!-- references:end -->";

/// APA 7 (`apa.csl`, CSL styles repository, pinned; see
/// `data/csl/README.md`).
pub const APA_CSL: &str = include_str!("../data/csl/apa.csl");
/// The CSL `en-US` locale (CSL locales repository, pinned).
pub const LOCALE_EN_US: &str = include_str!("../data/csl/locales-en-US.xml");

/// The three in-text forms, chosen on a page by how the link text is written
/// (see [`cite_page`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CiteForm {
    /// `(Romano et al., 2015)`: the style's ordinary citation.
    Parenthetical,
    /// `Romano et al. (2015)`: the authors are part of the sentence
    /// (hayagriva's [`CitePurpose::Prose`]).
    Narrative,
    /// `Romano et al., 2015`: the parenthetical citation without its outer
    /// parentheses, for several works inside one pair the author writes,
    /// `([..](#ref-a); [..](#ref-b))`.
    Bare,
}

/// A parsed CSL style with the locales it needs.
#[derive(Debug, Clone)]
pub struct CslStyle {
    style: IndependentStyle,
    locales: Vec<Locale>,
}

impl CslStyle {
    /// Parse an independent CSL style and a locale file.
    ///
    /// # Errors
    ///
    /// Either file is not valid CSL XML, or the style is a dependent one.
    pub fn from_xml(style_xml: &str, locale_xml: &str) -> Result<Self, String> {
        let style = IndependentStyle::from_xml(style_xml).map_err(|e| format!("CSL style: {e}"))?;
        let locale = LocaleFile::from_xml(locale_xml).map_err(|e| format!("CSL locale: {e}"))?;
        Ok(CslStyle { style, locales: vec![locale.into()] })
    }

    /// APA 7 with the `en-US` locale, both pinned in this crate.
    pub fn apa() -> Self {
        Self::from_xml(APA_CSL, LOCALE_EN_US).expect("the pinned APA style and locale parse")
    }
}

/// Every entry of a `.bib` file as CSL-JSON, keyed by cite key.
#[derive(Debug, Clone, Default)]
pub struct CslLibrary {
    items: BTreeMap<String, CslJsonItem>,
    json: BTreeMap<String, serde_json::Value>,
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
            let item: CslJsonItem = serde_json::from_value(csl.clone())
                .map_err(|err| format!("{}: CSL-JSON not accepted by the engine: {err}", e.cite_key))?;
            lib.items.insert(e.cite_key.clone(), item);
            lib.json.insert(e.cite_key.clone(), csl);
        }
        Ok(lib)
    }

    /// Whether the library holds this cite key.
    pub fn contains(&self, key: &str) -> bool {
        self.items.contains_key(key)
    }

    /// The CSL-JSON of one entry (what both this module and the citeproc-js
    /// reference render).
    pub fn csl_json(&self, key: &str) -> Option<&serde_json::Value> {
        self.json.get(key)
    }

    /// Every cite key, sorted.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.items.keys().map(String::as_str)
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the library is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Render a document: `cites` in order, each one key and form; returns
    /// each citation's text (plain) and the bibliography as `(key, HTML)` in
    /// the style's order.
    ///
    /// # Panics
    ///
    /// A key the library does not hold (callers check [`Self::contains`]).
    pub fn render(&self, style: &CslStyle, cites: &[(String, CiteForm)]) -> Rendered {
        // hayagriva 0.10.1 letters same-year works (`2015a`, `2015b`) in
        // first-citation order; CSL 1.0.2 ("Disambiguation", year-suffix) and
        // citeproc-js letter them in bibliography order. So the document is
        // first rendered once to learn the bibliography order, then again
        // with one primer citation per work in that order ahead of the real
        // ones, whose output is discarded. The style's citations must not
        // depend on position for this to be exact; APA's do not (verified
        // against citeproc-js by tests/csl_vs_citeproc_js.rs).
        let order: Vec<String> = self.render_raw(style, cites).1.into_iter().map(|(k, _)| k).collect();
        let primers: Vec<(String, CiteForm)> =
            order.into_iter().map(|k| (k, CiteForm::Parenthetical)).collect();
        let all: Vec<(String, CiteForm)> = primers.iter().chain(cites).cloned().collect();
        let (citations, bibliography) = self.render_raw(style, &all);
        Rendered { citations: citations[primers.len()..].to_vec(), bibliography }
    }

    /// One pass of the engine, citations in request order.
    fn render_raw(
        &self,
        style: &CslStyle,
        cites: &[(String, CiteForm)],
    ) -> (Vec<String>, Vec<(String, String)>) {
        let mut driver = BibliographyDriver::new();
        for (key, form) in cites {
            let entry = &self.items[key];
            let item = match form {
                CiteForm::Narrative => CitationItem::with_entry(entry).kind(CitePurpose::Prose),
                CiteForm::Parenthetical | CiteForm::Bare => CitationItem::with_entry(entry),
            };
            driver.citation(CitationRequest::from_items(vec![item], &style.style, &style.locales));
        }
        let out = driver.finish(BibliographyRequest {
            style: &style.style,
            locale: None,
            locale_files: &style.locales,
        });
        let citations = out
            .citations
            .iter()
            .zip(cites)
            .map(|(c, (_, form))| {
                let text = plain(&c.citation);
                match form {
                    CiteForm::Bare => strip_outer_parens(&text),
                    _ => text,
                }
            })
            .collect();
        let bibliography = out
            .bibliography
            .map(|b| b.items.iter().map(|i| (i.key.clone(), html(&i.content))).collect())
            .unwrap_or_default();
        (citations, bibliography)
    }
}

/// The output of [`CslLibrary::render`].
#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    /// Each citation, plain text, in request order.
    pub citations: Vec<String>,
    /// The bibliography, `(cite key, entry HTML)`, in the style's order.
    pub bibliography: Vec<(String, String)>,
}

fn plain(c: &ElemChildren) -> String {
    let mut s = String::new();
    c.write_buf(&mut s, BufWriteFormat::Plain).expect("writing to a String");
    s
}

fn html(c: &ElemChildren) -> String {
    let mut s = String::new();
    c.write_buf(&mut s, BufWriteFormat::Html).expect("writing to a String");
    s
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
    let result = Translator::BibTeX.import(&s, &opts).map_err(|e| e.to_string())?;
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
    // Drop the old generated list first, so its anchors are not read back.
    const SLOT: &str = "\u{0}REFS\u{0}";
    let (body, tail) = match (page.find(REFERENCES_BEGIN), page.find(REFERENCES_END)) {
        (Some(b), Some(e)) if e > b => {
            (format!("{}{SLOT}", &page[..b]), page[e + REFERENCES_END.len()..].to_string())
        }
        _ => (page.to_string(), String::new()),
    };

    // Pass 1: find every citation link outside code.
    let mut links: Vec<Link> = Vec::new();
    let mut errors = Vec::new();
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
                if lib.contains(&l.key) {
                    links.push(l);
                } else {
                    errors.push(format!("unknown cite key `{}`", l.key));
                }
            }
        }
        offset += line.len();
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    // Pass 2: render the page as one document, then splice.
    let cites: Vec<(String, CiteForm)> = links.iter().map(|l| (l.key.clone(), l.form)).collect();
    let rendered = lib.render(style, &cites);
    let mut out = String::with_capacity(body.len());
    let mut at = 0;
    for (l, text) in links.iter().zip(&rendered.citations) {
        out.push_str(&body[at..l.start]);
        out.push_str(&format!("[{text}](#ref-{})", l.key));
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
        let mut s = format!("{REFERENCES_BEGIN}\n## References\n\n");
        for (key, entry) in &rendered.bibliography {
            s.push_str(&format!(
                "<p class=\"csl-entry\" id=\"ref-{key}\" style=\"padding-left: 2em; text-indent: -2em;\">{entry}</p>\n\n"
            ));
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
                    let form = if t.is_empty() || t.starts_with('(') {
                        CiteForm::Parenthetical
                    } else if t.ends_with(')') {
                        CiteForm::Narrative
                    } else {
                        CiteForm::Bare
                    };
                    out.push(Link { start: i, end, key: key.to_string(), form });
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
    let valid =
        !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || "_-:.+".contains(c));
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
        let r = lib.render(
            &style,
            &[
                ("openmc".into(), CiteForm::Parenthetical),
                ("coolprop".into(), CiteForm::Narrative),
                ("iapws97".into(), CiteForm::Bare),
            ],
        );
        assert_eq!(r.citations, ["(Romano et al., 2015)", "Bell & Lemort (2014)", "Wagner & Kretzschmar, 2008"]);
        let keys: Vec<&str> = r.bibliography.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["coolprop", "openmc", "iapws97"], "APA sorts by author");
        let openmc = &r.bibliography[1].1;
        // hayagriva 0.10.1 writes `&` unescaped in its HTML output (citeproc-js
        // writes `&#38;`). Browsers render both the same; it is recorded as a
        // finding in tests/csl_vs_citeproc_js.rs, not hidden here.
        assert!(openmc.contains("Romano, P. K., Horelik, N. E., Herman, B. R., Nelson, A. G., Forget, B., & Smith, K. (2015)."), "{openmc}");
        assert!(openmc.contains("https://doi.org/10.1016/j.anucene.2014.07.048"), "{openmc}");
    }

    #[test]
    fn a_page_is_one_document_so_same_year_works_get_letters() {
        let (lib, style) = setup();
        let page = "A [](#ref-same1) and B [](#ref-same2).\n";
        let out = cite_page(page, &lib, &style).unwrap();
        assert!(out.text.contains("[(Smith et al., 2020a)](#ref-same2)"), "{}", out.text);
        assert!(out.text.contains("[(Smith et al., 2020b)](#ref-same1)"), "{}", out.text);
    }

    #[test]
    fn a_page_gets_its_citations_and_a_reference_list_and_is_idempotent() {
        let (lib, style) = setup();
        let page = "# Title\n\nTransport follows [](#ref-openmc). \
                    [Bell and Lemort ()](#ref-coolprop) fit it ([x](#ref-iapws97)).\n\n\
                    ```rust\nlet s = \"[](#ref-missing)\";\n```\n\nIn code: `[](#ref-missing)`.\n";
        let out = cite_page(page, &lib, &style).unwrap();
        assert_eq!(out.keys, ["openmc", "coolprop", "iapws97"]);
        assert!(out.text.contains("Transport follows [(Romano et al., 2015)](#ref-openmc)."));
        assert!(out.text.contains("[Bell & Lemort (2014)](#ref-coolprop) fit"));
        assert!(out.text.contains("([Wagner & Kretzschmar, 2008](#ref-iapws97))"));
        assert!(out.text.contains("\"[](#ref-missing)\""), "fenced code is left alone");
        assert!(out.text.contains("`[](#ref-missing)`"), "inline code is left alone");
        let list = &out.text[out.text.find(REFERENCES_BEGIN).unwrap()..];
        assert!(list.contains("## References"));
        assert!(list.contains("id=\"ref-openmc\""));
        assert_eq!(cite_page(&out.text, &lib, &style).unwrap().text, out.text);
    }

    #[test]
    fn unknown_keys_fail_and_an_uncited_page_loses_its_list() {
        let (lib, style) = setup();
        let err = cite_page("See [](#ref-nope) and [](#ref-also-nope).\n", &lib, &style).unwrap_err();
        assert_eq!(err, ["unknown cite key `nope`", "unknown cite key `also-nope`"]);

        let cited = cite_page("A [](#ref-openmc).\n\nMore.\n", &lib, &style).unwrap().text;
        let uncited = cited.replace("[(Romano et al., 2015)](#ref-openmc)", "nothing");
        let out = cite_page(&uncited, &lib, &style).unwrap();
        assert!(out.keys.is_empty());
        assert_eq!(out.text, "A nothing.\n\nMore.\n");
        assert_eq!(cite_page("Plain.\n", &lib, &style).unwrap().text, "Plain.\n");
    }
}

