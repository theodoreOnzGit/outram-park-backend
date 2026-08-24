//! BibTeX rendering from the canonical [`KovanDocument`], and — the other
//! direction — a plain-text `.bib` file **parser** ([`parse_bib_entries`],
//! op-vi1n).
//!
//! Implements `docs/kovan.md`, "Canonical Representation": the Rust struct is
//! authoritative and BibTeX is *generated* from it, never the reverse. This is
//! Agent D's "citation generation / bibliography support" responsibility.
//!
//! The renderer is deterministic and offline — it only reads the fields already
//! on the document and emits a single `.bib` entry. Values are TeX-escaped so a
//! title containing `&`, `%`, `_`, `{`… produces a syntactically valid entry.
//!
//! ## The parser is deliberately one-way and shallow (op-vi1n's scope)
//!
//! [`parse_bib_entries`] reads an arbitrary, user-authored `.bib` file into a
//! flat list of [`BibEntry`] (entry type, cite key, raw field map). It does
//! **not** attempt to build a [`KovanDocument`] from the result — the "kovan
//! folder" project format (`crates/kovan/src/project.rs`, op-b1y5) only needs
//! the cite key to join a `.bib` entry with its `pdf/`/`markdown/` files, not
//! a full document reconstruction, and inventing an author-name/date-field
//! reverse mapping is out of scope here (would need to be exact-inverse of
//! [`to_bibtex`] to round-trip cleanly, which this parser makes no attempt at).
//! Field values are returned exactly as written between the delimiters (brace
//! or quote characters stripped, TeX escapes such as `\&` **not** unescaped)
//! — good enough for exact-match joins and for display, not for re-deriving
//! structured data.

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
/// | `Thesis` | `@phdthesis` |
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
/// One field is spelled per entry type: `institution` becomes **`school`** on a
/// `@phdthesis`, which is the field BibTeX styles read for the awarding
/// university.
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
        // `@phdthesis` names the awarding institution `school`; `institution` is
        // the `@techreport` spelling and BibTeX styles ignore it on a thesis,
        // which would silently drop the university from the rendered citation.
        let field = if doc.document_type == DocumentType::Thesis {
            "school"
        } else {
            "institution"
        };
        push_field(&mut out, field, &escape_tex(institution));
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
        DocumentType::Thesis => "phdthesis",
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

/// One parsed `.bib` entry (op-vi1n) — the entry type (`article`,
/// `techreport`, …, lowercased), its cite key, and every field it declares.
///
/// Field names are lowercased (BibTeX field names are case-insensitive) and
/// values have their delimiters (`{}` or `"..."`) stripped but are otherwise
/// verbatim — see the module doc's "deliberately one-way and shallow" note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BibEntry {
    pub entry_type: String,
    pub cite_key: String,
    pub fields: std::collections::BTreeMap<String, String>,
}

/// Errors from [`parse_bib_entries`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BibParseError {
    /// An `@type{` (or a field's `{`/`"` value) was opened but the input
    /// ended before its matching close.
    UnterminatedEntry { entry_type: String, near: String },
    /// An entry body did not start with `<cite_key> ,` — e.g. `@article{}` or
    /// `@article{, title = {X}}`.
    MissingCiteKey { entry_type: String },
}

impl std::fmt::Display for BibParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BibParseError::UnterminatedEntry { entry_type, near } => {
                write!(f, "unterminated @{entry_type} entry (near {near:?})")
            }
            BibParseError::MissingCiteKey { entry_type } => {
                write!(f, "@{entry_type} entry has no cite key")
            }
        }
    }
}

impl std::error::Error for BibParseError {}

/// Parse a `.bib` file's text into its entries (op-vi1n).
///
/// Handles the common `@type{key, field = {value}, field2 = "value2", year =
/// 2020,}` shape, including nested braces inside a `{…}` value (e.g. `title =
/// {Heat {and} mass transfer}`) and a trailing comma before the closing `}`.
/// Text outside any `@…{…}` block (blank lines, `%` comments) is ignored, the
/// same way BibTeX itself ignores it.
///
/// `@string{…}`, `@preamble{…}` and `@comment{…}` blocks (case-insensitive)
/// are recognised, brace-balanced past, and **dropped** — they are BibTeX
/// macro/comment constructs, not citable entries, and don't have the
/// `key, field = value, …` shape this parser otherwise expects.
///
/// # Errors
///
/// [`BibParseError::UnterminatedEntry`] if a `{` (entry or field value) is
/// never closed. [`BibParseError::MissingCiteKey`] if an ordinary entry's
/// body doesn't start with `<key> ,`.
pub fn parse_bib_entries(input: &str) -> Result<Vec<BibEntry>, BibParseError> {
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0usize;
    let mut entries = Vec::new();

    while i < chars.len() {
        if chars[i] != '@' {
            i += 1;
            continue;
        }
        i += 1; // consume '@'
        let type_start = i;
        while i < chars.len() && chars[i] != '{' && chars[i] != '(' {
            i += 1;
        }
        let entry_type: String = chars[type_start..i].iter().collect::<String>().trim().to_lowercase();
        if i >= chars.len() {
            return Err(BibParseError::UnterminatedEntry {
                near: entry_type.clone(),
                entry_type,
            });
        }
        let open = chars[i];
        let close = if open == '{' { '}' } else { ')' };
        i += 1; // consume opening delimiter

        let body_start = i;
        let body_end = find_matching_close(&chars, i, open, close)
            .ok_or_else(|| BibParseError::UnterminatedEntry {
                entry_type: entry_type.clone(),
                near: chars[body_start..].iter().take(30).collect(),
            })?;
        let body = &chars[body_start..body_end];
        i = body_end + 1; // past the closing delimiter

        if matches!(entry_type.as_str(), "string" | "preamble" | "comment") {
            continue; // macro/comment block — not a citable entry
        }

        entries.push(parse_entry_body(&entry_type, body)?);
    }

    Ok(entries)
}

/// Find the index of the delimiter that closes the one opened just before
/// `start`, honouring nesting of `open`/`close` within the body.
fn find_matching_close(chars: &[char], start: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 1i32;
    let mut j = start;
    while j < chars.len() {
        if chars[j] == open {
            depth += 1;
        } else if chars[j] == close {
            depth -= 1;
            if depth == 0 {
                return Some(j);
            }
        }
        j += 1;
    }
    None
}

/// Parse the inside of an entry's braces: `key , field = value , …`.
fn parse_entry_body(entry_type: &str, body: &[char]) -> Result<BibEntry, BibParseError> {
    let mut i = 0usize;
    skip_ws(body, &mut i);
    let key_start = i;
    while i < body.len() && body[i] != ',' {
        i += 1;
    }
    let cite_key: String = body[key_start..i].iter().collect::<String>().trim().to_string();
    if cite_key.is_empty() {
        return Err(BibParseError::MissingCiteKey { entry_type: entry_type.to_string() });
    }
    if i < body.len() {
        i += 1; // consume the comma after the key
    }

    let mut fields = std::collections::BTreeMap::new();
    loop {
        skip_ws(body, &mut i);
        if i >= body.len() {
            break;
        }
        if body[i] == ',' {
            i += 1;
            continue;
        }
        let name_start = i;
        while i < body.len() && body[i] != '=' {
            i += 1;
        }
        let name: String = body[name_start..i].iter().collect::<String>().trim().to_lowercase();
        if name.is_empty() {
            break; // trailing comma / stray whitespace before the close
        }
        if i < body.len() {
            i += 1; // consume '='
        }
        skip_ws(body, &mut i);

        let value = if i < body.len() && body[i] == '{' {
            let vstart = i + 1;
            let vend = find_matching_close(body, vstart, '{', '}').ok_or_else(|| {
                BibParseError::UnterminatedEntry {
                    entry_type: entry_type.to_string(),
                    near: name.clone(),
                }
            })?;
            let v: String = body[vstart..vend].iter().collect();
            i = vend + 1;
            v
        } else if i < body.len() && body[i] == '"' {
            let vstart = i + 1;
            let mut depth = 0i32;
            let mut j = vstart;
            let mut vend = None;
            while j < body.len() {
                match body[j] {
                    '{' => depth += 1,
                    '}' => depth -= 1,
                    '"' if depth == 0 => {
                        vend = Some(j);
                        break;
                    }
                    _ => {}
                }
                j += 1;
            }
            let vend = vend.ok_or_else(|| BibParseError::UnterminatedEntry {
                entry_type: entry_type.to_string(),
                near: name.clone(),
            })?;
            let v: String = body[vstart..vend].iter().collect();
            i = vend + 1;
            v
        } else {
            // Bare value (e.g. `year = 2020`) — read up to the next comma.
            let vstart = i;
            while i < body.len() && body[i] != ',' {
                i += 1;
            }
            body[vstart..i].iter().collect::<String>().trim().to_string()
        };

        fields.insert(name, value);
        skip_ws(body, &mut i);
        if i < body.len() && body[i] == ',' {
            i += 1;
        }
    }

    Ok(BibEntry { entry_type: entry_type.to_string(), cite_key, fields })
}

fn skip_ws(chars: &[char], i: &mut usize) {
    while *i < chars.len() && chars[*i].is_whitespace() {
        *i += 1;
    }
}

/// Render one [`BibEntry`] back to BibTeX text — the write side of a
/// JabRef-like entry editor (op-xj8t), sitting alongside [`parse_bib_entries`]
/// rather than [`to_bibtex`] (which needs a full [`KovanDocument`], not a raw
/// entry read back from an arbitrary `.bib` file).
///
/// Fields are written in `entry.fields`' `BTreeMap` order (alphabetical by
/// field name), **not** the original file's field order — round-tripping a
/// parsed-then-rendered entry byte-for-byte is not a goal (see the module
/// doc's "deliberately one-way and shallow" note); the entry's *content* is
/// preserved, its formatting is not.
pub fn render_entry(entry: &BibEntry) -> String {
    let mut out = format!("@{}{{{},\n", entry.entry_type, entry.cite_key);
    for (name, value) in &entry.fields {
        out.push_str(&format!("  {name} = {{{value}}},\n"));
    }
    out.push_str("}\n");
    out
}

/// Render a whole `.bib` file's worth of entries — one blank line between
/// entries, matching common `.bib` file style.
pub fn render_entries(entries: &[BibEntry]) -> String {
    entries.iter().map(render_entry).collect::<Vec<_>>().join("\n")
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

    #[test]
    fn parses_a_single_brace_delimited_entry() {
        let bib = "@techreport{zweibaum2015ciet,\n  author = {Zweibaum, Nicolas},\n  title = {CIET facility characterisation},\n  year = {2015},\n  institution = {UC Berkeley},\n}\n";
        let entries = parse_bib_entries(bib).unwrap();
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert_eq!(e.entry_type, "techreport");
        assert_eq!(e.cite_key, "zweibaum2015ciet");
        assert_eq!(e.fields.get("author").unwrap(), "Zweibaum, Nicolas");
        assert_eq!(e.fields.get("title").unwrap(), "CIET facility characterisation");
        assert_eq!(e.fields.get("year").unwrap(), "2015");
        assert_eq!(e.fields.get("institution").unwrap(), "UC Berkeley");
    }

    #[test]
    fn round_trips_through_to_bibtex_then_parse_bib_entries() {
        let doc = sample();
        let rendered = to_bibtex(&doc);
        let entries = parse_bib_entries(&rendered).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].cite_key, "zweibaum2015ciet");
        assert_eq!(entries[0].fields.get("title").unwrap(), "CIET facility characterisation");
        assert_eq!(entries[0].fields.get("doi").unwrap(), "10.1234/ciet");
    }

    #[test]
    fn quote_delimited_values_and_bare_numeric_year_parse() {
        let bib = r#"@article{smith2020, title = "A Title", year = 2020, pages = "1--10"}"#;
        let entries = parse_bib_entries(bib).unwrap();
        let e = &entries[0];
        assert_eq!(e.fields.get("title").unwrap(), "A Title");
        assert_eq!(e.fields.get("year").unwrap(), "2020");
        assert_eq!(e.fields.get("pages").unwrap(), "1--10");
    }

    #[test]
    fn nested_braces_inside_a_value_are_preserved() {
        let bib = "@article{k, title = {Heat {and} mass transfer}}";
        let entries = parse_bib_entries(bib).unwrap();
        assert_eq!(entries[0].fields.get("title").unwrap(), "Heat {and} mass transfer");
    }

    #[test]
    fn multiple_entries_and_surrounding_comments_all_parse() {
        let bib = "% a leading comment\n@article{a, title = {A}}\n\n@misc{b, title = {B}}\n";
        let entries = parse_bib_entries(bib).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].cite_key, "a");
        assert_eq!(entries[1].cite_key, "b");
    }

    #[test]
    fn string_preamble_and_comment_blocks_are_skipped_not_returned() {
        let bib = r#"@string{anl = "Argonne National Laboratory"}
@comment{this whole block is ignored}
@preamble{"% some latex preamble"}
@article{real2020, title = {Real Entry}}
"#;
        let entries = parse_bib_entries(bib).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].cite_key, "real2020");
    }

    #[test]
    fn missing_cite_key_is_an_error() {
        let err = parse_bib_entries("@article{, title = {X}}").unwrap_err();
        assert!(matches!(err, BibParseError::MissingCiteKey { .. }), "{err}");
    }

    #[test]
    fn unterminated_entry_is_an_error() {
        let err = parse_bib_entries("@article{k, title = {X}").unwrap_err();
        assert!(matches!(err, BibParseError::UnterminatedEntry { .. }), "{err}");
    }

    #[test]
    fn field_names_are_lowercased() {
        let entries = parse_bib_entries("@article{k, TITLE = {X}}").unwrap();
        assert_eq!(entries[0].fields.get("title").unwrap(), "X");
    }

    #[test]
    fn empty_input_produces_no_entries() {
        assert!(parse_bib_entries("").unwrap().is_empty());
        assert!(parse_bib_entries("just some text, no entries here").unwrap().is_empty());
    }

    #[test]
    fn render_entry_round_trips_through_parse_bib_entries() {
        let mut fields = std::collections::BTreeMap::new();
        fields.insert("title".to_string(), "A Title".to_string());
        fields.insert("year".to_string(), "2020".to_string());
        let entry = BibEntry { entry_type: "article".to_string(), cite_key: "smith2020".to_string(), fields };
        let rendered = render_entry(&entry);
        assert!(rendered.starts_with("@article{smith2020,"));
        let back = parse_bib_entries(&rendered).unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0], entry);
    }

    #[test]
    fn render_entries_joins_multiple_with_a_blank_line() {
        let e = |key: &str| BibEntry {
            entry_type: "misc".to_string(),
            cite_key: key.to_string(),
            fields: std::collections::BTreeMap::new(),
        };
        let rendered = render_entries(&[e("a"), e("b")]);
        assert!(rendered.contains("@misc{a,"));
        assert!(rendered.contains("@misc{b,"));
        let back = parse_bib_entries(&rendered).unwrap();
        assert_eq!(back.len(), 2);
    }
}
