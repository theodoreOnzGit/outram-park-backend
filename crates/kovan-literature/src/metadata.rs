//! Metadata extraction: build a [`KovanDocument`] skeleton from a source PDF.
//!
//! Implements the "Metadata extraction" step of `docs/kovan.md`, "PDF
//! Processing". Everything here is deterministic and offline.
//!
//! ## Heuristics and their limits
//!
//! Metadata is recovered **best-effort**, in this order of trust:
//!
//! 0. **A recognised labelled cover page** — currently eScholarship (California
//!    Digital Library) deposits, whose generated first page carries `Title`,
//!    `Permalink`, `Author` and `Publication Date` labels. Explicit labels beat
//!    everything below, and they must: eScholarship sets the PDF `/Title` of
//!    every deposit to the constant `"UC Berkeley"`.
//! 1. **PDF object model** — the Info dictionary (`/Title`, `/Author`,
//!    `/Keywords`, `/CreationDate`) and the page tree (`/Pages` `/Count`), read
//!    losslessly via [`lopdf`]. This is the most reliable source and is used
//!    whenever present. The page count in particular comes from here rather
//!    than from extracted text, because text extraction does not reliably mark
//!    page boundaries.
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
    let mut document_type = crate::storage::document_type_from_path(pdf);

    // An eScholarship deposit cover page carries explicitly *labelled* fields,
    // so it outranks the Info dictionary: eScholarship sets `/Title` to the
    // constant "UC Berkeley" for every deposit, which would otherwise become the
    // title of every UC thesis in the archive.
    let cover = escholarship_cover(&text).unwrap_or_default();

    // The cover states the document is a thesis, so trust it over the fallback
    // path inference — but never override an explicit `theses/`-style directory
    // or any other type the storage layout already asserted.
    if cover.is_thesis && document_type == crate::DocumentType::Other {
        document_type = crate::DocumentType::Thesis;
    }

    let title = cover
        .title
        .clone()
        .or_else(|| info.title.clone())
        .or_else(|| title_from_text(&text))
        .unwrap_or_else(|| {
            pdf.file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Untitled".to_string())
        });

    let authors = if cover.authors.is_empty() {
        info.authors.clone()
    } else {
        cover.authors.clone()
    };
    let year = cover.year.or(info.year).or_else(|| year_from_text(&text));
    let doi = find_doi(&text);
    let keywords = info.keywords.clone();

    let slug = make_slug(&authors, year, &title);
    let id = make_id(&slug, &title);

    let markdown_body = crate::markdown::text_to_markdown(&text);
    // Prefer the PDF page tree over the text-derived count: `pdf-extract` does
    // not emit form-feed page breaks for every producer (notably eScholarship
    // thesis deposits), which would otherwise report every such document as a
    // single page.
    let page_count = info.page_count.or_else(|| page_count_from_text(&text));

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
    if let Some(url) = cover.permalink {
        builder = builder.source_url(url);
    }
    Ok(builder.build())
}

/// Number of pages in the extracted `text`, counted from the form-feed
/// (`U+000C`) page breaks `pdf-extract` emits between pages. `None` for empty
/// text (no pages extracted).
///
/// **Fallback only** — used when the PDF page tree is unreadable. Not every PDF
/// producer leads `pdf-extract` to emit form feeds, and when it does not, this
/// undercounts to 1. Prefer [`InfoDict::page_count`].
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

/// Recovered PDF Info-dictionary fields, plus the true page count from the
/// document's page tree. All optional.
#[derive(Debug, Default, Clone)]
struct InfoDict {
    title: Option<String>,
    authors: Vec<Author>,
    year: Option<u32>,
    keywords: Vec<String>,
    /// Number of pages in the PDF page tree (`/Pages` `/Count`), as reported by
    /// [`lopdf::Document::get_pages`]. Authoritative — unlike a count derived
    /// from extracted text, it does not depend on the text extractor emitting
    /// page-break characters.
    page_count: Option<u32>,
}

/// Read the PDF `/Info` dictionary and the page-tree page count. `Err` only when
/// the PDF fails to load.
///
/// The page count is read even when the document carries no `/Info` dictionary,
/// since the two come from independent parts of the file.
fn read_info(pdf: &Path) -> Result<InfoDict, LiteratureError> {
    let doc = Document::load(pdf)
        .map_err(|e| LiteratureError::Io(format!("load {}: {e}", pdf.display())))?;

    let page_count = u32::try_from(doc.get_pages().len()).ok().filter(|n| *n > 0);

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
        return Ok(InfoDict {
            page_count,
            ..InfoDict::default()
        });
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
        page_count,
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

/// Labelled bibliographic fields recovered from an eScholarship (California
/// Digital Library) deposit cover page.
///
/// eScholarship prepends a generated cover page to every deposited UC thesis or
/// paper, carrying explicitly *labelled* fields (`Title`, `Permalink`, `Author`,
/// `Publication Date`). Because the labels are machine-readable, this is
/// higher-trust than either the PDF Info dictionary — which eScholarship sets to
/// the useless constant `"UC Berkeley"` — or any positional text heuristic.
#[derive(Debug, Default, Clone)]
struct CoverPage {
    title: Option<String>,
    authors: Vec<Author>,
    year: Option<u32>,
    /// The `escholarship.org/uc/item/…` permalink — the citable location of the
    /// deposit, recorded for provenance.
    permalink: Option<String>,
    /// True when the cover identifies the deposit as a thesis/dissertation
    /// ("Electronic Theses and Dissertations"), which the cover states outright
    /// rather than leaving to be guessed.
    is_thesis: bool,
}

impl CoverPage {
    /// True when no field was recovered.
    fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.authors.is_empty()
            && self.year.is_none()
            && self.permalink.is_none()
            && !self.is_thesis
    }
}

/// How many leading characters are searched for an eScholarship cover page.
const COVER_PAGE_CHARS: usize = 4000;

/// Parse an eScholarship deposit cover page out of the front of `text`.
///
/// Returns `None` unless the text actually looks like an eScholarship deposit —
/// detection requires an `escholarship.org` mention, so an ordinary paper that
/// merely happens to contain the word "Title" is never misread as one.
///
/// Handles both layouts `pdf-extract` produces for these covers: `Label value`
/// on one line, and a bare `Label` line whose value is the next non-empty line.
fn escholarship_cover(text: &str) -> Option<CoverPage> {
    let end = text
        .char_indices()
        .nth(COVER_PAGE_CHARS)
        .map(|(i, _)| i)
        .unwrap_or(text.len());
    let head = &text[..end];
    if !head.to_ascii_lowercase().contains("escholarship.org") {
        return None;
    }

    let lines: Vec<&str> = head.lines().map(str::trim).collect();
    let mut cover = CoverPage::default();

    let head_lower = head.to_ascii_lowercase();
    cover.is_thesis = head_lower.contains("theses and dissertations")
        || head_lower.contains("thesis/dissertation");

    for i in 0..lines.len() {
        // "Publication Date" is probed before "Permalink"/"Author"/"Title"
        // because the labels are distinct prefixes; order only matters in that
        // each line yields at most one field.
        if let Some(v) = labelled_value(&lines, i, "Publication Date", false) {
            if cover.year.is_none() {
                cover.year = year_from_text(&v);
            }
        } else if let Some(v) = labelled_value(&lines, i, "Permalink", false) {
            if cover.permalink.is_none() {
                cover.permalink = v
                    .split_whitespace()
                    .find(|t| t.contains("escholarship.org"))
                    .map(str::to_string);
            }
        } else if let Some(v) = labelled_value(&lines, i, "Author", false) {
            if cover.authors.is_empty() {
                cover.authors = parse_authors(&v);
            }
        } else if let Some(v) = labelled_value(&lines, i, "Title", true) {
            if cover.title.is_none() {
                cover.title = (!v.is_empty()).then_some(v);
            }
        }
    }

    (!cover.is_empty()).then_some(cover)
}

/// The cover-page field labels. Used both to find values and to know where a
/// wrapped value ends.
const COVER_LABELS: [&str; 4] = ["Title", "Permalink", "Author", "Publication Date"];

/// Strip a Markdown heading marker and surrounding whitespace, so the label
/// probes work on raw extracted text and on generated Markdown alike.
fn clean_line(line: &str) -> &str {
    line.trim_start_matches('#').trim()
}

/// If `line` begins with one of [`COVER_LABELS`], return the rest of the line.
///
/// The label must be followed by end-of-line or a space, so `Title` does not
/// match `Titles of nobility`.
fn split_label(line: &str, label: &str) -> Option<String> {
    let line = clean_line(line);
    let rest = line.to_ascii_lowercase().strip_prefix(
        &label.to_ascii_lowercase(),
    )?.to_string();
    if !rest.is_empty() && !rest.starts_with(' ') {
        return None;
    }
    // `label` is ASCII and matched at the start, so the byte offset is valid.
    Some(line[label.len()..].trim().to_string())
}

/// True when `line` starts a new labelled field — the terminator for a wrapped
/// value.
fn is_label_line(line: &str) -> bool {
    COVER_LABELS
        .iter()
        .any(|l| split_label(line, l).is_some())
}

/// Read the value belonging to `label` at `lines[i]`.
///
/// The value is the rest of the label's own line, or — when the label stands
/// alone on its line — the following non-empty lines.
///
/// When `join_wrapped` is set, continuation lines are appended until the next
/// labelled field. Cover pages hard-wrap long titles, so without this a title
/// like *"…for pebble-bed Fluoride-Salt-Cooled, High-Temperature Reactor (FHR)"*
/// is silently truncated at the comma. Single-line fields (`Permalink`,
/// `Author`, `Publication Date`) pass `false`, so they cannot swallow the
/// eScholarship footer that follows them.
fn labelled_value(lines: &[&str], i: usize, label: &str, join_wrapped: bool) -> Option<String> {
    let inline = split_label(lines.get(i)?, label)?;

    let mut parts: Vec<String> = Vec::new();
    if !inline.is_empty() {
        parts.push(inline);
    }

    for line in lines.iter().skip(i + 1) {
        let line = clean_line(line);
        if line.is_empty() {
            if parts.is_empty() {
                continue; // still looking for the value
            }
            break;
        }
        if is_label_line(line) {
            break;
        }
        parts.push(line.to_string());
        if !join_wrapped {
            break;
        }
    }

    (!parts.is_empty()).then(|| parts.join(" "))
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

/// How many leading characters of the extracted text count as "front matter"
/// for [`find_doi`].
///
/// A document's *own* DOI is printed on its title page or first-page footer. A
/// DOI appearing deep in the body is almost always a *cited* work's. 5000 chars
/// covers a cover page plus a title page with room to spare.
const FRONT_MATTER_CHARS: usize = 5000;

/// Find the first DOI (`10.xxxx/…`) in the document's front matter, or `None`.
///
/// Deliberately scans only the first [`FRONT_MATTER_CHARS`] characters. Scanning
/// the whole text picks up DOIs from the bibliography and attributes a cited
/// paper's identifier to the document itself — e.g. the Xin Wang (2018) FHR
/// dissertation was assigned `10.1016/j.nucengdes.2018.02.003`, which belongs to
/// reference [6] of its own reference list. The trade-off is that a DOI printed
/// only on a late page is missed; a missing DOI is recoverable by a human
/// reviewer, a confidently wrong one is not.
fn find_doi(text: &str) -> Option<String> {
    let end = text
        .char_indices()
        .nth(FRONT_MATTER_CHARS)
        .map(|(i, _)| i)
        .unwrap_or(text.len());
    let text = &text[..end];
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
    use crate::test_pdf::{build_multipage_pdf, build_text_pdf, tmp_path};

    /// Regression test: the page count must come from the PDF page tree, not
    /// from form-feed breaks in the extracted text.
    ///
    /// **Methodology.** Synthesise a 7-page PDF with [`build_multipage_pdf`]
    /// (one text line per page, no Info dictionary), run [`extract_metadata`],
    /// and require `page_count == Some(7)`. The reference is the synthetic
    /// document's own `/Pages` `/Count`, which is exact by construction. Pass
    /// criterion: exact equality.
    ///
    /// **Result (2026-07-30).** `page_count == Some(7)`, exact, no uncertainty
    /// — the value is an integer read from the page tree. Before the fix this
    /// returned `Some(1)`, because the three UC Berkeley theses in
    /// `kovan_import/` extract as text with no `U+000C` page breaks at all and
    /// the old text-derived count therefore saw a single page. Implication: page
    /// counts are now trustworthy for `split_markdown_by_page_limit` decisions
    /// and for provenance records, independent of the text extractor's
    /// behaviour.
    /// The two eScholarship cover layouts `pdf-extract` produces: `Label value`
    /// on one line, and a bare `Label` line followed by its value.
    #[test]
    fn escholarship_cover_parses_both_label_layouts() {
        // The title wraps onto a second line, exactly as on the real deposit.
        let inline = "UC Berkeley UC Berkeley Electronic Theses and Dissertations\n\
             Title Coupled neutronics and thermal-hydraulics modeling for pebble-bed Fluoride-Salt-Cooled,\n\
             High-Temperature Reactor (FHR)\n\
             Permalink https://escholarship.org/uc/item/40q3985m\n\
             Author Wang, Xin\n\
             Publication Date 2018\n";
        let cover = escholarship_cover(inline).expect("inline layout recognised");
        assert_eq!(
            cover.title.as_deref(),
            Some(
                "Coupled neutronics and thermal-hydraulics modeling for pebble-bed \
                 Fluoride-Salt-Cooled, High-Temperature Reactor (FHR)"
            ),
            "a wrapped title must be rejoined, not truncated at the line break"
        );
        assert_eq!(cover.year, Some(2018));
        assert_eq!(
            cover.permalink.as_deref(),
            Some("https://escholarship.org/uc/item/40q3985m")
        );
        assert_eq!(cover.authors.len(), 1);
        assert_eq!(cover.authors[0].family, "Wang");
        assert_eq!(cover.authors[0].given, "Xin");

        // Heading-per-line layout, as seen on the Alivisatos deposit.
        let split = "# UC Berkeley\n\
             ## UC Berkeley Electronic Theses and Dissertations\n\
             ### Title\n\
             #### Evaluating Remote Operations for Advanced Nuclear Reactor Control: Feasibility, Benefits,\n\
             #### and Implementation Criteria\n\
             ### Permalink\n\
             #### https://escholarship.org/uc/item/1wt929p1\n\
             ### Author\n\
             #### Alivisatos, Clara\n\
             ### Publication Date\n\
             #### 2023\n";
        let cover = escholarship_cover(split).expect("split layout recognised");
        assert_eq!(
            cover.title.as_deref(),
            Some(
                "Evaluating Remote Operations for Advanced Nuclear Reactor Control: \
                 Feasibility, Benefits, and Implementation Criteria"
            )
        );
        assert_eq!(cover.year, Some(2023));
        assert_eq!(cover.authors[0].family, "Alivisatos");
        assert_eq!(cover.authors[0].given, "Clara");
    }

    /// A document with no eScholarship marker must not be parsed as a deposit,
    /// even when it contains lines beginning with the same labels.
    #[test]
    fn non_escholarship_text_is_not_treated_as_a_cover_page() {
        let text = "Title of Nobility Clause\nAuthor unknown\nPublication Date 1787\n";
        assert!(escholarship_cover(text).is_none());
    }

    /// Regression test for the false-positive DOI: a DOI that appears only in the
    /// bibliography must not be adopted as the document's own.
    ///
    /// **Methodology.** Build a text whose front matter holds no DOI and whose
    /// body, past [`FRONT_MATTER_CHARS`], holds a real reference-list DOI — the
    /// exact one wrongly attributed to the Xin Wang (2018) dissertation. Require
    /// `find_doi` to return `None`. Pass criterion: `None`, not the cited DOI.
    ///
    /// **Result (2026-07-30).** Returns `None`. Against the real PDF, `lit
    /// import` previously reported `doi: 10.1016/j.nucengdes.2018.02.003`, which
    /// belongs to reference [6] (Xingwei Chen et al., *Nucl. Eng. Design* 331,
    /// 2018) found at line 1967 of the generated markdown; it now reports no DOI,
    /// which is correct — the dissertation has none, only an eScholarship
    /// permalink.
    #[test]
    fn find_doi_ignores_dois_in_the_bibliography() {
        let mut text = String::from("A Dissertation\nby Someone\nBerkeley, 2018\n");
        text.push_str(&"filler body text. ".repeat(400));
        assert!(
            text.chars().count() > FRONT_MATTER_CHARS,
            "filler must push the reference past the front-matter window"
        );
        text.push_str("[6] Xingwei Chen et al. DOI: 10.1016/j.nucengdes.2018.02.003.\n");
        assert_eq!(find_doi(&text), None);

        // A DOI in the front matter is still found.
        let front = "Some Paper\nDOI: 10.1016/j.nucengdes.2018.02.003\n";
        assert_eq!(
            find_doi(front).as_deref(),
            Some("10.1016/j.nucengdes.2018.02.003")
        );
    }

    #[test]
    fn page_count_comes_from_the_page_tree_not_the_text() {
        let path = tmp_path("kovan_lit_pagetree.pdf");
        build_multipage_pdf(&path, 7);
        let doc = extract_metadata(&path).expect("metadata");
        assert_eq!(
            doc.page_count,
            Some(7),
            "page count must come from /Pages /Count"
        );
        let _ = std::fs::remove_file(&path);
    }

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
        // Page count read from the page tree — this fixture is a single page.
        assert_eq!(doc.page_count, Some(1));
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
