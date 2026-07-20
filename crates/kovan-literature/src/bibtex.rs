//! BibTeX rendering from the canonical [`KovanDocument`].
//!
//! Implements `docs/kovan.md`, "Canonical Representation": the Rust struct is
//! authoritative and BibTeX is *generated* from it, never the reverse. This is
//! Agent D's "citation generation / bibliography support" responsibility.
//!
//! The renderer is deterministic and offline — it only reads the fields already
//! on the document and emits a single `.bib` entry. Values are TeX-escaped so a
//! title containing `&`, `%`, `_`, `{`… produces a syntactically valid entry.

use crate::{DocumentType, KovanDocument};

/// Render a [`KovanDocument`] to a single BibTeX entry.
///
/// ## Entry-type mapping (from [`DocumentType`])
///
/// | `DocumentType` | BibTeX entry |
/// |---|---|
/// | `Paper` | `@article` |
/// | `Report` | `@techreport` |
/// | `Manual` | `@manual` |
/// | `Standard` | `@misc` |
/// | `Benchmark` | `@misc` |
/// | `Other` | `@misc` |
///
/// `Standard`/`Benchmark` map to `@misc` for portability — classic BibTeX has no
/// `@standard`/`@benchmark`, and `@misc` accepts arbitrary fields, so no data is
/// lost. (biblatex users can post-process the type.)
///
/// ## Fields
///
/// The citation key is the document `slug`, sanitised to `[A-Za-z0-9:_-]`. Every
/// present field is emitted: `author` (as `Family, Given and …`), `title`,
/// `year`, `journal`, `volume`, `number`, `pages`, `institution`, `publisher`,
/// `doi`, `url` (from `source_url`), `keywords`, and `abstract`. Absent optional
/// fields are simply omitted. All values are TeX-escaped (see `escape_tex`).
///
/// The output is deterministic: field order is fixed and independent of the
/// document's construction path.
pub fn to_bibtex(doc: &KovanDocument) -> String {
    let entry_type = entry_type_for(doc.document_type);
    let key = sanitize_key(&doc.slug);

    let mut out = format!("@{entry_type}{{{key},\n");

    // author — "Family, Given and Family, Given …" (BibTeX name order).
    if !doc.authors.is_empty() {
        let authors = doc
            .authors
            .iter()
            .map(|a| {
                let given = a.given.trim();
                if given.is_empty() {
                    escape_tex(a.family.trim())
                } else {
                    format!("{}, {}", escape_tex(a.family.trim()), escape_tex(given))
                }
            })
            .collect::<Vec<_>>()
            .join(" and ");
        push_field(&mut out, "author", &authors);
    }

    push_field(&mut out, "title", &escape_tex(&doc.title));

    if let Some(year) = doc.year {
        // A bare integer needs no escaping.
        out.push_str(&format!("  year = {{{year}}},\n"));
    }
    if let Some(journal) = non_empty(&doc.journal) {
        push_field(&mut out, "journal", &escape_tex(journal));
    }
    if let Some(volume) = non_empty(&doc.volume) {
        push_field(&mut out, "volume", &escape_tex(volume));
    }
    if let Some(number) = non_empty(&doc.number) {
        push_field(&mut out, "number", &escape_tex(number));
    }
    if let Some(pages) = non_empty(&doc.pages) {
        push_field(&mut out, "pages", &escape_tex(pages));
    }
    if let Some(institution) = non_empty(&doc.institution) {
        push_field(&mut out, "institution", &escape_tex(institution));
    }
    if let Some(publisher) = non_empty(&doc.publisher) {
        push_field(&mut out, "publisher", &escape_tex(publisher));
    }
    if let Some(doi) = non_empty(&doc.doi) {
        push_field(&mut out, "doi", &escape_tex(doi));
    }
    if let Some(url) = non_empty(&doc.source_url) {
        push_field(&mut out, "url", &escape_tex(url));
    }
    if !doc.keywords.is_empty() {
        let kw = doc
            .keywords
            .iter()
            .map(|k| escape_tex(k.trim()))
            .collect::<Vec<_>>()
            .join(", ");
        push_field(&mut out, "keywords", &kw);
    }
    if !doc.abstract_text.trim().is_empty() {
        push_field(&mut out, "abstract", &escape_tex(doc.abstract_text.trim()));
    }

    out.push_str("}\n");
    out
}

/// Map a [`DocumentType`] to its BibTeX entry type. See [`to_bibtex`] for the
/// rationale behind the `@misc` fallbacks.
fn entry_type_for(document_type: DocumentType) -> &'static str {
    match document_type {
        DocumentType::Paper => "article",
        DocumentType::Report => "techreport",
        DocumentType::Manual => "manual",
        DocumentType::Standard | DocumentType::Benchmark | DocumentType::Other => "misc",
    }
}

/// Append `  <name> = {<value>},\n` to a growing entry.
fn push_field(out: &mut String, name: &str, value: &str) {
    out.push_str(&format!("  {name} = {{{value}}},\n"));
}

/// `Some(&str)` if the option holds a non-blank string, else `None`.
fn non_empty(opt: &Option<String>) -> Option<&str> {
    opt.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

/// Sanitise a slug into a legal BibTeX citation key (`[A-Za-z0-9:_-]`). Empty or
/// all-illegal input becomes `"unknown"`.
fn sanitize_key(slug: &str) -> String {
    let key: String = slug
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':'))
        .collect();
    if key.is_empty() {
        "unknown".to_string()
    } else {
        key
    }
}

/// Escape the TeX special characters that would otherwise break a BibTeX value.
///
/// Handles `\ { } & % $ # _ ~ ^`. The backslash is escaped first so the
/// replacements introduced for the others are not themselves re-escaped.
pub fn escape_tex(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\\' => out.push_str("\\textbackslash{}"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '&' => out.push_str("\\&"),
            '%' => out.push_str("\\%"),
            '$' => out.push_str("\\$"),
            '#' => out.push_str("\\#"),
            '_' => out.push_str("\\_"),
            '~' => out.push_str("\\textasciitilde{}"),
            '^' => out.push_str("\\textasciicircum{}"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Author, Visibility};

    fn sample() -> KovanDocument {
        let mut doc = KovanDocument::new(
            "kovan-0001",
            "zweibaum2015ciet",
            Visibility::Open,
            DocumentType::Report,
            "CIET facility characterisation",
        );
        doc.authors.push(Author {
            family: "Zweibaum".into(),
            given: "Nicolas".into(),
            affiliation: None,
        });
        doc.year = Some(2015);
        doc.institution = Some("UC Berkeley".into());
        doc.doi = Some("10.1234/ciet".into());
        doc
    }

    #[test]
    fn golden_techreport_entry() {
        let expected = "\
@techreport{zweibaum2015ciet,
  author = {Zweibaum, Nicolas},
  title = {CIET facility characterisation},
  year = {2015},
  institution = {UC Berkeley},
  doi = {10.1234/ciet},
}
";
        assert_eq!(to_bibtex(&sample()), expected);
    }

    #[test]
    fn paper_maps_to_article() {
        let mut doc = sample();
        doc.document_type = DocumentType::Paper;
        assert!(to_bibtex(&doc).starts_with("@article{zweibaum2015ciet,"));
    }

    #[test]
    fn standard_maps_to_misc() {
        let mut doc = sample();
        doc.document_type = DocumentType::Standard;
        assert!(to_bibtex(&doc).starts_with("@misc{"));
    }

    #[test]
    fn special_characters_are_escaped() {
        let mut doc = sample();
        doc.title = "Heat & mass transfer: 50% efficiency in H_2O".into();
        let bib = to_bibtex(&doc);
        assert!(bib.contains("Heat \\& mass transfer: 50\\% efficiency in H\\_2O"));
    }

    #[test]
    fn missing_optional_fields_are_omitted() {
        let doc = KovanDocument::new(
            "id",
            "smith2020",
            Visibility::Open,
            DocumentType::Paper,
            "A Title",
        );
        let bib = to_bibtex(&doc);
        assert!(bib.starts_with("@article{smith2020,"));
        assert!(bib.contains("title = {A Title}"));
        assert!(!bib.contains("author"));
        assert!(!bib.contains("year"));
        assert!(!bib.contains("doi"));
    }

    #[test]
    fn journal_locators_are_emitted_when_present() {
        let doc = KovanDocument::builder(
            "id",
            "smith2020",
            Visibility::Open,
            DocumentType::Paper,
            "A Journal Paper",
        )
        .journal("Nuclear Engineering and Design")
        .volume("410")
        .number("3")
        .pages("112345")
        .build();
        let bib = to_bibtex(&doc);
        assert!(bib.contains("journal = {Nuclear Engineering and Design}"));
        assert!(bib.contains("volume = {410}"));
        assert!(bib.contains("number = {3}"));
        assert!(bib.contains("pages = {112345}"));
    }

    #[test]
    fn multiple_authors_join_with_and() {
        let mut doc = sample();
        doc.authors.push(Author {
            family: "Peterson".into(),
            given: "Per".into(),
            affiliation: None,
        });
        assert!(to_bibtex(&doc).contains("author = {Zweibaum, Nicolas and Peterson, Per}"));
    }

    #[test]
    fn empty_slug_becomes_unknown_key() {
        let doc = KovanDocument::new("id", "!!!", Visibility::Open, DocumentType::Other, "T");
        assert!(to_bibtex(&doc).starts_with("@misc{unknown,"));
    }
}
