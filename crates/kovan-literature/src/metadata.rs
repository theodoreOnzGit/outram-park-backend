//! Metadata extraction: build a [`KovanDocument`] skeleton from a source PDF.
//!
//! Implements the "Metadata extraction" step of `docs/kovan.md`, "PDF
//! Processing". Everything here is deterministic and offline.
//!
//! ## Heuristics and their limits
//!
//! Metadata is recovered **best-effort**, in this order of trust:
//!
//! 1. **PDF Info dictionary** (`/Title`, `/Author`, `/Keywords`,
//!    `/CreationDate`) — read losslessly via [`lopdf`]. This is the most
//!    reliable source and is used whenever present.
//! 2. **Text heuristics** — only as a fallback. The title falls back to the
//!    first substantial line of extracted text; the year to the first plausible
//!    `19xx`/`20xx` in the opening of the document; the DOI to the first
//!    `10.xxxx/…` match anywhere in the text.
//! 3. **Storage-path hints** — [`Visibility`] and [`DocumentType`] are inferred
//!    from where the file lives (`proprietary/`, `reports/`, …).
//!
//! Author parsing from a free-text `/Author` field is inherently ambiguous;
//! unknown given/family splits fall back to "last whitespace token is the
//! family name". **When a field cannot be recovered it is left `None`/empty
//! rather than guessed** — a wrong DOI or author is worse than a missing one, so
//! a human reviewer fills gaps against the source. No text-only author guessing
//! is attempted at all.

use crate::{Author, KovanDocument, LiteratureError};
use lopdf::{Dictionary, Document, Object};
use std::path::Path;

/// Extract document metadata from a source PDF into a [`KovanDocument`].
///
/// See the module docs for the heuristic order. Returns
/// [`LiteratureError::Io`] only when the PDF can be neither text-extracted nor
/// parsed at all; a partially-readable PDF yields a document with whatever
/// fields were recoverable and empty/`None` for the rest.
///
/// Deterministic and offline. The returned document's `markdown_body` is the
/// generated Markdown (so a single call yields both the metadata and the body).
pub fn extract_metadata(pdf: &Path) -> Result<KovanDocument, LiteratureError> {
    let text_res = crate::pdf_import::extract_pdf_text(pdf);
    let info_res = read_info(pdf);

    // Only a hard failure (unparseable *and* un-extractable) is an error.
    if let (Err(text_err), Err(_)) = (&text_res, &info_res) {
        return Err(text_err.clone());
    }
    let text = text_res.unwrap_or_default();
    let info = info_res.unwrap_or_default();

    let visibility = crate::storage::visibility_from_path(pdf);
    let document_type = crate::storage::document_type_from_path(pdf);

    let title = info
        .title
        .clone()
        .or_else(|| title_from_text(&text))
        .unwrap_or_else(|| {
            pdf.file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Untitled".to_string())
        });

    let authors = info.authors.clone();
    let year = info.year.or_else(|| year_from_text(&text));
    let doi = find_doi(&text);
    let keywords = info.keywords.clone();

    let slug = make_slug(&authors, year, &title);
    let id = make_id(&slug, &title);

    let markdown_body = crate::markdown::text_to_markdown(&text);
    let page_count = page_count_from_text(&text);

    // Build via the KovanDocumentBuilder (kovan-common v2). `source_path`
    // records where this document was ingested from; `page_count` is counted
    // from the extracted text's page breaks. Journal locators
    // (volume/pages/number) and `source_sha256` are intentionally left unset:
    // there is no reliable offline heuristic for the locators, and a SHA-256
    // hash would need a `sha2` dependency (not yet added — see DECISIONS.md).
    let mut builder = KovanDocument::builder(id, slug, visibility, document_type, title)
        .authors(authors)
        .keywords(keywords)
        .source_path(pdf.to_string_lossy().into_owned())
        .markdown_body(markdown_body);
    if let Some(y) = year {
        builder = builder.year(y);
    }
    if let Some(d) = doi {
        builder = builder.doi(d);
    }
    if let Some(pages) = page_count {
        builder = builder.page_count(pages);
    }
    Ok(builder.build())
}

/// Number of pages in the extracted `text`, counted from the form-feed
/// (`U+000C`) page breaks `pdf-extract` emits between pages. `None` for empty
/// text (no pages extracted).
fn page_count_from_text(text: &str) -> Option<u32> {
    if text.is_empty() {
        return None;
    }
    let breaks = text.matches('\u{000C}').count();
    // n page-breaks separate n+1 pages; saturate into u32 for absurd inputs.
    Some(
        u32::try_from(breaks)
            .unwrap_or(u32::MAX - 1)
            .saturating_add(1),
    )
}

/// Recovered PDF Info-dictionary fields. All optional.
#[derive(Debug, Default, Clone)]
struct InfoDict {
    title: Option<String>,
    authors: Vec<Author>,
    year: Option<u32>,
    keywords: Vec<String>,
}

/// Read the PDF `/Info` dictionary. `Err` only when the PDF fails to load.
fn read_info(pdf: &Path) -> Result<InfoDict, LiteratureError> {
    let doc = Document::load(pdf)
        .map_err(|e| LiteratureError::Io(format!("load {}: {e}", pdf.display())))?;

    let info_dict: Option<&Dictionary> = doc
        .trailer
        .get(b"Info")
        .ok()
        .and_then(|o| match o {
            Object::Reference(id) => doc.get_object(*id).ok(),
            other => Some(other),
        })
        .and_then(|o| o.as_dict().ok());

    let Some(dict) = info_dict else {
        return Ok(InfoDict::default());
    };

    Ok(InfoDict {
        title: info_string(dict, b"Title"),
        authors: info_string(dict, b"Author")
            .map(|a| parse_authors(&a))
            .unwrap_or_default(),
        year: info_string(dict, b"CreationDate").and_then(|d| year_from_pdf_date(&d)),
        keywords: info_string(dict, b"Keywords")
            .map(|k| parse_keywords(&k))
            .unwrap_or_default(),
    })
}

/// Read a PDF text string from an Info dictionary, decoded and trimmed. `None`
/// when absent or empty.
fn info_string(dict: &Dictionary, key: &[u8]) -> Option<String> {
    let value = match dict.get(key).ok()? {
        Object::String(bytes, _) => decode_pdf_text_string(bytes),
        _ => return None,
    };
    let trimmed = value.trim().to_string();
    (!trimmed.is_empty()).then_some(trimmed)
}

/// Decode a PDF text string. UTF-16BE if it carries a `FE FF` BOM, otherwise
/// PDFDocEncoding (approximated as Latin-1 for the byte range we care about).
fn decode_pdf_text_string(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let units: Vec<u16> = bytes[2..]
            .chunks(2)
            .filter(|c| c.len() == 2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        bytes.iter().map(|&b| b as char).collect()
    }
}

/// Parse an `/Author` field into authors, splitting on ` and ` and `;`.
fn parse_authors(field: &str) -> Vec<Author> {
    field
        .split(" and ")
        .flat_map(|s| s.split(';'))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(parse_one_author)
        .collect()
}

/// Parse a single name. `Family, Given` when a comma is present; otherwise the
/// last whitespace token is taken as the family name.
fn parse_one_author(name: &str) -> Author {
    if let Some((family, given)) = name.split_once(',') {
        return Author {
            family: family.trim().to_string(),
            given: given.trim().to_string(),
            affiliation: None,
        };
    }
    let mut parts: Vec<&str> = name.split_whitespace().collect();
    match parts.len() {
        0 => Author {
            family: String::new(),
            given: String::new(),
            affiliation: None,
        },
        1 => Author {
            family: parts[0].to_string(),
            given: String::new(),
            affiliation: None,
        },
        _ => {
            let family = parts.pop().unwrap().to_string();
            Author {
                family,
                given: parts.join(" "),
                affiliation: None,
            }
        }
    }
}

/// Split a `/Keywords` field on `,` and `;`.
fn parse_keywords(field: &str) -> Vec<String> {
    field
        .split([',', ';'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// Parse the year from a PDF date string (`D:YYYYMMDD…`).
fn year_from_pdf_date(s: &str) -> Option<u32> {
    let s = s.trim().trim_start_matches("D:");
    let digits: String = s.chars().take_while(char::is_ascii_digit).collect();
    if digits.len() < 4 {
        return None;
    }
    digits[..4]
        .parse()
        .ok()
        .filter(|y| (1900..=2099).contains(y))
}

/// The first line of extracted text that looks like a title (8–200 chars, not a
/// URL or DOI line).
fn title_from_text(text: &str) -> Option<String> {
    for line in text.lines() {
        let t = line.trim();
        let len = t.chars().count();
        if (8..=200).contains(&len)
            && !t.starts_with("http")
            && !t.to_ascii_lowercase().starts_with("doi")
            && !t.contains('@')
        {
            return Some(t.to_string());
        }
    }
    None
}

/// The first plausible 4-digit year (1900–2099) in the opening 3000 chars.
/// Used only when the Info dictionary carries no date; noisy by nature (may pick
/// up a citation year), hence documented as a low-trust fallback.
fn year_from_text(text: &str) -> Option<u32> {
    let window: String = text.chars().take(3000).collect();
    let bytes = window.as_bytes();
    let mut i = 0;
    while i + 4 <= bytes.len() {
        let is_boundary_before = i == 0 || !bytes[i - 1].is_ascii_digit();
        let is_boundary_after = i + 4 == bytes.len() || !bytes[i + 4].is_ascii_digit();
        if is_boundary_before && is_boundary_after && bytes[i..i + 4].iter().all(u8::is_ascii_digit)
        {
            // Safe: the four bytes are ASCII digits.
            let y: u32 = std::str::from_utf8(&bytes[i..i + 4])
                .unwrap()
                .parse()
                .unwrap();
            if (1900..=2099).contains(&y) {
                return Some(y);
            }
        }
        i += 1;
    }
    None
}

/// Find the first DOI (`10.xxxx/…`) in the text, or `None`.
fn find_doi(text: &str) -> Option<String> {
    let mut search_from = 0;
    while let Some(rel) = text[search_from..].find("10.") {
        let start = search_from + rel;
        let rest = &text[start..];
        let after_prefix = &rest[3..];
        let registrant_digits = after_prefix
            .chars()
            .take_while(char::is_ascii_digit)
            .count();
        if registrant_digits >= 4 {
            let candidate: String = rest.chars().take_while(|c| is_doi_char(*c)).collect();
            let trimmed = candidate.trim_end_matches(['.', ',', ';', ')', ']', '>']);
            if trimmed.contains('/') {
                return Some(trimmed.to_string());
            }
        }
        search_from = start + 3;
    }
    None
}

/// Characters permitted in a DOI body.
fn is_doi_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '/' | ':' | '(' | ')' | ';' | '+')
}

/// Build a BibTeX-style slug: `<firstauthorfamily><year><firsttitleword>`,
/// lowercased and alphanumeric-only (e.g. `zweibaum2015ciet`). Falls back to a
/// slugged title when no author/year is known.
fn make_slug(authors: &[Author], year: Option<u32>, title: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(a) = authors.first() {
        let fam = slug_token(&a.family);
        if !fam.is_empty() {
            parts.push(fam);
        }
    }
    if let Some(y) = year {
        parts.push(y.to_string());
    }
    if let Some(word) = title
        .split_whitespace()
        .map(slug_token)
        .find(|w| w.len() >= 3)
    {
        parts.push(word);
    }
    let slug = parts.concat();
    if slug.is_empty() {
        let fallback = slug_token(title);
        if fallback.is_empty() {
            "document".to_string()
        } else {
            fallback
        }
    } else {
        slug
    }
}

/// Lowercase, keep only ASCII alphanumerics.
fn slug_token(s: &str) -> String {
    s.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Build a stable content id from the slug and title, as a 64-bit FNV-1a hash.
/// Deterministic (no timestamps, no randomness) so re-ingesting the same
/// document yields the same id.
fn make_id(slug: &str, title: &str) -> String {
    let mut data = Vec::with_capacity(slug.len() + title.len() + 1);
    data.extend_from_slice(slug.as_bytes());
    data.push(0);
    data.extend_from_slice(title.as_bytes());
    format!("kovan-{:016x}", fnv1a64(&data))
}

/// 64-bit FNV-1a hash. Small, dependency-free, deterministic — used only for
/// generating stable document ids, not for security.
fn fnv1a64(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_pdf::{build_text_pdf, tmp_path};

    #[test]
    fn metadata_from_info_dictionary() {
        let path = tmp_path("kovan_lit_meta.pdf");
        build_text_pdf(&path);
        let doc = extract_metadata(&path).expect("metadata");
        assert_eq!(doc.title, "A Test Steam-Table Report");
        assert_eq!(doc.year, Some(2021));
        assert_eq!(doc.authors.len(), 2);
        assert_eq!(doc.authors[0].family, "Doe");
        assert_eq!(doc.authors[0].given, "Jane");
        assert!(doc.keywords.contains(&"IAPWS".to_string()));
        // slug: firstauthor + year + first title word.
        assert_eq!(doc.slug, "doe2021test");
        assert!(doc.id.starts_with("kovan-"));
        // v2 fields populated via the builder: source pointer + page count.
        assert_eq!(doc.source_path.as_deref(), Some(path.to_str().unwrap()));
        assert!(doc.page_count.is_some(), "page count counted from text");
        // SHA-256 deliberately left unset (no sha2 dep) — see DECISIONS.md.
        assert_eq!(doc.source_sha256, None);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn page_count_from_text_counts_form_feeds() {
        assert_eq!(page_count_from_text(""), None);
        assert_eq!(page_count_from_text("one page only"), Some(1));
        assert_eq!(page_count_from_text("p1\u{000C}p2\u{000C}p3"), Some(3));
    }

    #[test]
    fn id_is_deterministic() {
        assert_eq!(make_id("smith2020x", "T"), make_id("smith2020x", "T"));
        assert_ne!(make_id("smith2020x", "T"), make_id("jones2019y", "T"));
    }

    #[test]
    fn parse_author_comma_and_spaced_forms() {
        let a = parse_one_author("Zweibaum, Nicolas");
        assert_eq!(
            (a.family.as_str(), a.given.as_str()),
            ("Zweibaum", "Nicolas")
        );
        let b = parse_one_author("Per Fredrik Peterson");
        assert_eq!(
            (b.family.as_str(), b.given.as_str()),
            ("Peterson", "Per Fredrik")
        );
    }

    #[test]
    fn find_doi_matches_and_trims() {
        assert_eq!(
            find_doi("see doi 10.1016/j.anucene.2024.110439."),
            Some("10.1016/j.anucene.2024.110439".to_string())
        );
        assert_eq!(find_doi("no identifier here"), None);
        // "10.5" alone (too few registrant digits) is not a DOI.
        assert_eq!(find_doi("version 10.5 of the code"), None);
    }

    #[test]
    fn year_from_pdf_date_parses_prefix() {
        assert_eq!(year_from_pdf_date("D:20210304120000Z"), Some(2021));
        assert_eq!(year_from_pdf_date("garbage"), None);
    }
}
