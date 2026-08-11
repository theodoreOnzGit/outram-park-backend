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
//! a human reviewer fills gaps against the source.
//!
//! Two positional heuristics do run over the extracted text, both added for
//! bead `op-szai` and both written to fail by recovering *nothing*:
//! [`title_block`] rejoins a cover title split across lines, and
//! [`authors_from_text`] reads the name block directly beneath it. The latter
//! outranks the Info dictionary in exactly one case — a cover that prints an
//! `Editors:` label — because a PDF has only one `/Author` key and producers
//! fill it with whichever name came first, editor or not.

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

    // Same precedence rule for numbered agency reports: refine `Other` when the
    // front matter carries a report identifier, but never override a type the
    // storage layout already asserted. Without this, a JRC book filed outside a
    // `reports/` directory stayed `Other` despite printing `EUR 28712 EN`.
    if document_type == crate::DocumentType::Other && looks_like_report(&text) {
        document_type = crate::DocumentType::Report;
    }

    // The Info dictionary is NOT authoritative for titles, which is the single
    // biggest source of wrong records in this archive (bead `op-szai`):
    //
    // - Elsevier sets `/Title` to the article's PII, so three separate ingests
    //   were titled `PII: S0029-5493(02)00182-6` and friends.
    // - The JRC HTR book's `/Title` is itself truncated at the cover's first
    //   line break, giving `The High Temperature`.
    //
    // So an Info title is screened for publisher furniture, and is superseded
    // when the text-derived title is a strict extension of it — the signature of
    // a cover title that wraps across lines. Anything else keeps the Info value.
    let info_title = info.title.clone().filter(|t| !is_title_junk(t));
    let text_title = title_from_text(&text);
    let title = cover
        .title
        .clone()
        .or_else(|| match (&info_title, &text_title) {
            (Some(i), Some(t)) if t.chars().count() > i.chars().count() && t.starts_with(i) => {
                Some(t.clone())
            }
            (Some(i), _) => Some(i.clone()),
            (None, t) => t.clone(),
        })
        .unwrap_or_else(|| {
            pdf.file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Untitled".to_string())
        });

    // Author precedence: labelled cover > text block that a cover explicitly
    // separated from an editor list > Info dictionary > text block.
    //
    // The middle rung exists for the JRC HTR book (bead `op-szai`): a PDF has
    // exactly one `/Author` key, and its producer filled it with the first
    // *editor*, `Scheuermann, Walter`. When the cover itself prints an
    // `Editors:` label under the author block, the text is the only source that
    // knows the difference, so it wins. Absent that signal the Info dictionary
    // stays ahead of any positional guess.
    let text_authors = authors_from_text(&text);
    let authors = if !cover.authors.is_empty() {
        cover.authors.clone()
    } else if text_authors.stopped_at_editors && !text_authors.authors.is_empty() {
        text_authors.authors.clone()
    } else if !info.authors.is_empty() {
        info.authors.clone()
    } else {
        text_authors.authors.clone()
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
    // (volume/pages/number) are intentionally left unset: there is no reliable
    // offline heuristic for them, and a confidently wrong locator is worse than
    // an absent one a human can fill in.
    let mut builder = KovanDocument::builder(id, slug, visibility, document_type, title)
        .authors(authors)
        .keywords(keywords)
        .source_path(pdf.to_string_lossy().into_owned())
        .markdown_body(markdown_body);
    // Provenance: hash whatever was actually ingested. DATA_POLICY.md asks every
    // catalogued document to be identifiable, and the tool already has the file
    // open, so leaving this null pushed avoidable manual work onto every import.
    // A read failure is non-fatal — the rest of the record is still worth having.
    if let Some(digest) = sha256_of(pdf) {
        builder = builder.source_sha256(digest);
    }
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
    let rest = line
        .to_ascii_lowercase()
        .strip_prefix(&label.to_ascii_lowercase())?
        .to_string();
    if !rest.is_empty() && !rest.starts_with(' ') {
        return None;
    }
    // `label` is ASCII and matched at the start, so the byte offset is valid.
    Some(line[label.len()..].trim().to_string())
}

/// True when `line` starts a new labelled field — the terminator for a wrapped
/// value.
fn is_label_line(line: &str) -> bool {
    COVER_LABELS.iter().any(|l| split_label(line, l).is_some())
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

/// Publisher furniture that is never a document title, matched case-insensitively
/// as a prefix or substring of a candidate line.
///
/// Motivated by three real ingest failures recorded in bead `op-szai`:
/// two Elsevier articles whose extracted text led with `PII: S0029-5493(02)00182-6`,
/// and a JRC report carrying `EUR 28712 EN` / `ISSN 1831-9424` boilerplate. Each
/// of these satisfied the old "8–200 chars, not a URL" test and was taken as the
/// title verbatim.
const TITLE_JUNK_MARKERS: &[&str] = &[
    "pii:",
    "issn",
    "isbn",
    "all rights reserved",
    "rights reserved",
    "www.",
    "http",
    "doi:",
    "copyright",
    "(c) 20",
    "elsevier",
    "springer",
    "published by",
    "publications office",
    "downloaded from",
    "see front matter",
    "this publication is",
    "received ",
    "accepted ",
];

/// True when a line is publisher furniture rather than a candidate title.
///
/// Rejects the [`TITLE_JUNK_MARKERS`], journal running headers (see
/// [`looks_like_journal_locator`]), and any line with no alphabetic content
/// (bare page numbers, rule characters, report-number strings).
fn is_title_junk(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    if TITLE_JUNK_MARKERS.iter().any(|m| lower.contains(m)) {
        return true;
    }
    if line.contains('@') {
        return true;
    }
    if looks_like_journal_locator(line) {
        return true;
    }
    // A title has words. Something that is mostly digits and punctuation
    // (`EUR 28712 EN`, `0029-5493/02/$`, `227 – 240`) is not one.
    let alphabetic = line.chars().filter(|c| c.is_alphabetic()).count();
    alphabetic < 4
}

/// True when a line is a journal running header / bibliographic locator rather
/// than a title — journal name, volume, year and page range on one line.
///
/// Motivated directly by the two Elsevier ingests in bead `op-szai`: rejecting
/// the `PII:` string as a title merely exposed the *next* piece of furniture,
/// and both papers were then catalogued as
/// `Nuclear Engineering and Design 218 (2002) 25-32`, which is the running
/// header `pdf-extract` returns as the very first line of the article. The real
/// title sits two lines below it.
///
/// Two forms are recognised, both requiring **several** independent signals so
/// an ordinary title is not caught:
///
/// 1. A parenthesised publication year *and* a page range — the Elsevier form
///    `Nuclear Engineering and Design 218 (2002) 25-32`. Page ranges accept the
///    ASCII hyphen and the en/em dashes that PDF text extraction produces.
/// 2. An explicit `pp.` page marker *and* a bare 4-digit year — the
///    `Vol. 218, No. 1, pp. 25-32, 2002` style used by other publishers.
///
/// A title that legitimately carries both a parenthesised year and a page range
/// would be misread, but no such title has been observed; the alternative —
/// silently titling every Elsevier article after its journal — was measured.
fn looks_like_journal_locator(line: &str) -> bool {
    if has_parenthesised_year(line) && has_page_range(line) {
        return true;
    }
    line.to_ascii_lowercase().contains("pp.") && contains_bare_year(line)
}

/// True when `line` contains `(YYYY)` with `YYYY` a plausible publication year
/// (1900-2099). Part of the [`looks_like_journal_locator`] test.
fn has_parenthesised_year(line: &str) -> bool {
    let chars: Vec<char> = line.chars().collect();
    for i in 0..chars.len().saturating_sub(5) {
        if chars[i] != '(' || chars[i + 5] != ')' {
            continue;
        }
        let digits: Option<String> = chars[i + 1..i + 5]
            .iter()
            .all(char::is_ascii_digit)
            .then(|| chars[i + 1..i + 5].iter().collect());
        if let Some(d) = digits {
            if let Ok(y) = d.parse::<u32>() {
                if (1900..=2099).contains(&y) {
                    return true;
                }
            }
        }
    }
    false
}

/// True when `line` contains a page range: digits, a hyphen or dash, digits
/// (`25-32`, `25–32`). Part of the [`looks_like_journal_locator`] test.
///
/// The dash set covers the ASCII hyphen plus the Unicode hyphen, en dash and em
/// dash, because `pdf-extract` reproduces whatever glyph the typesetter used —
/// the two HTR-10 papers use U+2013.
fn has_page_range(line: &str) -> bool {
    let chars: Vec<char> = line.chars().collect();
    for i in 0..chars.len() {
        if !matches!(chars[i], '-' | '\u{2010}' | '\u{2013}' | '\u{2014}') {
            continue;
        }
        let before = i > 0 && chars[i - 1].is_ascii_digit();
        let after = chars.get(i + 1).is_some_and(char::is_ascii_digit);
        if before && after {
            return true;
        }
    }
    false
}

/// True when `line` contains a standalone 4-digit year in 1900-2099. Part of the
/// `pp.`-form [`looks_like_journal_locator`] test.
fn contains_bare_year(line: &str) -> bool {
    year_from_text(line).is_some()
}

/// True when a line looks like an author/editor block rather than title text.
///
/// Used purely as a *stop* condition when joining a wrapped title, so a false
/// positive costs a truncated title and a false negative glues an author list
/// onto it. Detects the two dominant cover forms: initials-with-periods
/// (`Kugeler, K., Nabielek, H.,`) and an explicit `Editors:` / `Edited by` label.
///
/// Deliberately conservative — it is also the filter that decides a candidate
/// line is *not* the start of the title, where a false positive would skip the
/// real title entirely. The looser [`looks_like_name_list`] test handles
/// given-name-first lists and is used only where truncation is the worst
/// outcome.
fn looks_like_author_line(line: &str) -> bool {
    if is_editor_label(line) {
        return true;
    }
    if line.to_ascii_lowercase().starts_with("by ") {
        return true;
    }
    // "Surname, X." — a comma followed by a single capital and a period.
    let bytes: Vec<char> = line.chars().collect();
    for i in 0..bytes.len().saturating_sub(3) {
        if bytes[i] == ','
            && bytes[i + 1] == ' '
            && bytes[i + 2].is_uppercase()
            && bytes[i + 3] == '.'
        {
            return true;
        }
    }
    false
}

/// True when a line introduces an **editor** list rather than an author list.
///
/// The JRC HTR book's cover prints `Editors: Scheuermann, W., …` directly below
/// its author block, and the PDF Info dictionary's single `/Author` field was
/// filled with that first *editor* — the misattribution reported in bead
/// `op-szai`. Finding this label is therefore both a stop condition for the
/// author-block scan and the signal that the Info dictionary must not be
/// trusted for authorship (see [`authors_from_text`]).
fn is_editor_label(line: &str) -> bool {
    let lower = line.trim().to_ascii_lowercase();
    lower.starts_with("editor") || lower.starts_with("edited by") || lower.starts_with("eds.")
}

/// Footnote/affiliation markers that publishers hang off author names and that
/// `pdf-extract` reproduces verbatim (`Zongxin Wu *`). They are not part of a
/// name and must be removed before parsing or matching.
const AUTHOR_FOOTNOTE_MARKERS: &[char] = &['*', '†', '‡', '§', '¶', '#', '^'];

/// Strip footnote/affiliation markers from one comma-separated name fragment.
///
/// Removes the [`AUTHOR_FOOTNOTE_MARKERS`] anywhere in the fragment and any
/// trailing digits, which are the other common affiliation-superscript form
/// (`Zongxin Wu1`). Motivated by the HTR-10 papers, whose corresponding-author
/// asterisk otherwise ends up inside the recorded family name.
fn strip_author_markers(part: &str) -> String {
    let cleaned: String = part
        .chars()
        .filter(|c| !AUTHOR_FOOTNOTE_MARKERS.contains(c))
        .collect();
    cleaned
        .trim()
        .trim_end_matches(|c: char| c.is_ascii_digit())
        .trim()
        .to_string()
}

/// The most name-like tokens a single author fragment may hold (`Per Fredrik
/// Peterson` is three; anything longer is prose, not a name).
const MAX_NAME_TOKENS: usize = 4;

/// Longest line still considered a plausible author list. Real author lines on
/// a cover or article opening are short; abstracts and affiliations are not.
const MAX_NAME_LIST_CHARS: usize = 120;

/// True when a line looks like a comma-separated list of personal names.
///
/// Complements [`looks_like_author_line`], which only recognises the
/// `Family, I.` initials form. Elsevier sets author lines given-name-first
/// (`Zongxin Wu *, Dengcai Lin, Daxin Zhong`), which carries no initials at all
/// and was therefore invisible to the initials test — so the wrapped-title join
/// ran straight through it.
///
/// A line qualifies when every comma/`and`-separated fragment is name-like
/// (1-[`MAX_NAME_TOKENS`] tokens, each starting with an uppercase letter) *and*
/// either some fragment is a bare initial or every fragment holds at least two
/// tokens. That second clause is what keeps a title-cased title with a comma in
/// it — `Safety, Reliability and Cost` — from being read as three authors,
/// since `Safety` is a lone token with no initials anywhere on the line.
///
/// It can still misfire on a title of the shape `Thermal Hydraulics, Neutron
/// Physics`. The cost there is a truncated title, which a human fills in; the
/// cost of the opposite error is an author list silently glued onto a title, so
/// the bias is deliberate (same trade-off as [`find_doi`]).
fn looks_like_name_list(line: &str) -> bool {
    let line = line.trim();
    if line.chars().count() > MAX_NAME_LIST_CHARS {
        return false;
    }
    let parts = split_name_fragments(line);
    if parts.len() < 2 {
        return false;
    }
    if !parts.iter().all(|p| is_name_like(p)) {
        return false;
    }
    parts.iter().any(|p| is_initials(p)) || parts.iter().all(|p| p.split_whitespace().count() >= 2)
}

/// Split an author line into individual name fragments on `,`, `;` and ` and `,
/// stripping footnote markers and dropping empties.
///
/// Shared by [`looks_like_name_list`] and [`parse_author_block`] so detection
/// and parsing can never disagree about where one name ends and the next begins.
fn split_name_fragments(line: &str) -> Vec<String> {
    line.split([',', ';'])
        .flat_map(|p| p.split(" and "))
        .map(strip_author_markers)
        .filter(|p| !p.is_empty())
        .collect()
}

/// True when a fragment could be a personal name: 1-[`MAX_NAME_TOKENS`]
/// whitespace tokens, each beginning with an uppercase letter.
///
/// The uppercase-start requirement is what rejects affiliations — `Institute of
/// Nuclear Energy and Technology` fails on the lowercase `of`, and
/// `Beijing 100084` fails on the digits. Name particles (`van`, `von`, `de`) are
/// also rejected, which loses a Dutch or German name rather than risking a false
/// author; that is the intended direction of failure.
fn is_name_like(part: &str) -> bool {
    let tokens: Vec<&str> = part.split_whitespace().collect();
    if tokens.is_empty() || tokens.len() > MAX_NAME_TOKENS {
        return false;
    }
    tokens.iter().all(|t| {
        t.chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() && c.is_uppercase())
    })
}

/// True when a fragment is an initials group (`K.`, `M.J.`, `W.`).
///
/// Used by [`parse_author_block`] to pair `Family, I.` fragments back together:
/// splitting `Kugeler, K., Nabielek, H.` on commas yields four fragments, and
/// only the shape of each one says which are families and which are given-name
/// initials. Allows up to three letters, all uppercase, with nothing but
/// periods, hyphens and spaces between them.
fn is_initials(part: &str) -> bool {
    let letters: Vec<char> = part.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.is_empty() || letters.len() > 3 {
        return false;
    }
    if !letters.iter().all(|c| c.is_uppercase()) {
        return false;
    }
    part.chars()
        .all(|c| c.is_alphabetic() || matches!(c, '.' | '-' | ' '))
}

/// Longest title assembled by joining wrapped cover lines, in characters.
const TITLE_JOIN_LIMIT: usize = 250;

/// How many blank lines the title join may step over in total.
///
/// A typeset cover page extracts with a blank line between *every* visual line —
/// the JRC HTR book's four title lines arrive as
/// `"The High Temperature" / "" / "Gas-cooled Reactor" / "" / …`. Without
/// crossing blanks the title truncated at the first one, which is exactly the
/// `The High Temperature` record reported in bead `op-szai`. Four crossings
/// covers a five-line cover title and stops the join running away into the body.
const MAX_TITLE_BLANK_CROSSINGS: usize = 4;

/// How many *consecutive* blank lines the title join may step over at once.
/// One blank is layout; a longer gap is a new block of the page.
const MAX_TITLE_BLANK_RUN: usize = 1;

/// Longest continuation line accepted when the join steps over a blank line.
///
/// Reaching across a blank is the risky case, so it demands a *short* fragment.
/// The JRC cover's continuations are 12-28 characters; the affiliation line that
/// follows the HTR-10 titles (`Institute of Nuclear Energy and Technology,
/// Tsinghua Uniersity, Beijing 100084, China`) is 84 and is refused on length
/// alone, before any name heuristic is consulted.
const TITLE_FRAGMENT_MAX_CHARS: usize = 60;

/// Whole-line section headings that end the front-matter title block.
///
/// Matched against the entire line, so a title such as *"Introduction to
/// Reactor Physics"* is unaffected — only a line that is nothing but the
/// heading word stops the join.
const SECTION_HEADING_WORDS: &[&str] = &[
    "abstract",
    "introduction",
    "contents",
    "table of contents",
    "keywords",
    "key words",
    "summary",
    "preface",
    "foreword",
    "nomenclature",
    "references",
    "acknowledgement",
    "acknowledgements",
    "acknowledgment",
    "acknowledgments",
];

/// True when the whole line is a section heading (see [`SECTION_HEADING_WORDS`]).
fn is_section_heading(line: &str) -> bool {
    let normalised = line.trim().trim_end_matches(':').to_ascii_lowercase();
    SECTION_HEADING_WORDS.contains(&normalised.as_str())
}

/// Words that mark a line as an institutional affiliation.
///
/// Matched as substrings, case-insensitively. Kept to organisation nouns that do
/// not plausibly appear in a *reactor* title; `reactor`, `laboratory` in the
/// sense of an experiment, and similar are deliberately absent.
const AFFILIATION_MARKERS: &[&str] = &[
    "institute",
    "university",
    "universität",
    "département",
    "department of",
    "school of",
    "college of",
    "academy of",
    "gmbh",
    " inc.",
    " ltd",
    "corporation",
];

/// True when a line looks like an institutional affiliation rather than a title
/// fragment.
///
/// Both HTR-10 papers print their affiliation one blank line below the authors
/// (`Institute of Nuclear Energy and Technology, Tsinghua Uniersity, Beijing
/// 100084, China`). The long real line is already refused by
/// [`TITLE_FRAGMENT_MAX_CHARS`], but a shorter affiliation would slip through
/// and be welded onto the title, so the marker test backs the length test up.
///
/// Applied only to continuation lines the join reaches *across a blank line* —
/// the speculative case. An adjacent wrapped line is left alone so a title that
/// legitimately names an institution is not truncated.
fn looks_like_affiliation(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    AFFILIATION_MARKERS.iter().any(|m| lower.contains(m))
}

/// The front matter of `text` — the leading [`FRONT_MATTER_CHARS`] characters,
/// split into trimmed lines.
///
/// Title and author blocks live on the first page or two, so every positional
/// heuristic works on this window rather than the whole document. That also
/// bounds the cost on a 264-page book.
fn front_matter_lines(text: &str) -> Vec<&str> {
    let end = text
        .char_indices()
        .nth(FRONT_MATTER_CHARS)
        .map(|(i, _)| i)
        .unwrap_or(text.len());
    text[..end].lines().map(str::trim).collect()
}

/// The title assembled from `lines`, plus the index one past the last line it
/// consumed — where an author block, if any, begins.
///
/// Split out of [`title_from_text`] so [`authors_from_text`] can start scanning
/// immediately below the title instead of re-guessing where it ended. See
/// [`title_from_text`] for the heuristic itself.
fn title_block(lines: &[&str]) -> Option<(String, usize)> {
    let start = lines.iter().position(|l| {
        let len = l.chars().count();
        (8..=200).contains(&len) && !is_title_junk(l) && !looks_like_author_line(l)
    })?;

    let mut title = lines[start].to_string();
    let mut end = start + 1;
    let mut cursor = start + 1;
    let mut blanks_crossed = 0usize;

    while title.chars().count() < TITLE_JOIN_LIMIT {
        // A line already ending in sentence punctuation is complete.
        if title.ends_with('.') || title.ends_with('?') || title.ends_with('!') {
            break;
        }

        // Step over the blank lines a typeset cover puts between visual lines.
        let mut probe = cursor;
        let mut blank_run = 0usize;
        while probe < lines.len() && lines[probe].is_empty() {
            probe += 1;
            blank_run += 1;
        }
        if probe >= lines.len() {
            break;
        }
        if blank_run > 0
            && (blank_run > MAX_TITLE_BLANK_RUN || blanks_crossed >= MAX_TITLE_BLANK_CROSSINGS)
        {
            break;
        }

        let next = lines[probe];
        let len = next.chars().count();
        if len > 200
            || is_title_junk(next)
            || looks_like_author_line(next)
            || looks_like_name_list(next)
            || year_only(next)
            || is_section_heading(next)
        {
            break;
        }
        // Crossing a blank is the speculative case: demand a short, unfinished
        // fragment rather than a sentence of body text.
        if blank_run > 0
            && (len > TITLE_FRAGMENT_MAX_CHARS
                || next.ends_with('.')
                || looks_like_affiliation(next))
        {
            break;
        }

        title.push(' ');
        title.push_str(next);
        if blank_run > 0 {
            blanks_crossed += 1;
        }
        cursor = probe + 1;
        end = probe + 1;
    }

    Some((title.trim().to_string(), end))
}

/// The document title recovered from extracted text, skipping publisher
/// furniture and re-joining a title that the cover layout split across lines.
///
/// Used only when neither an eScholarship cover nor the PDF Info dictionary
/// supplies a title.
///
/// Two behaviours, both driven by observed failures (bead `op-szai`):
///
/// 1. **Junk is skipped** rather than accepted — see [`is_title_junk`], which
///    covers both the `PII:` string Elsevier leads with *and* the journal
///    running header (`Nuclear Engineering and Design 218 (2002) 25-32`) sitting
///    behind it. The old implementation returned the first 8–200 char non-URL
///    line, which is one or the other on every Elsevier article.
/// 2. **Wrapped titles are joined, across blank lines.** A cover that sets
///    `"The High Temperature" / "Gas-cooled Reactor" / "Safety considerations of the" /
///    "(V)HTR-Modul"` previously yielded only `"The High Temperature"`, because
///    `pdf-extract` puts a blank line between every visual line of a typeset
///    cover and the join stopped at the first one. Joining now steps over single
///    blank lines (bounded by [`MAX_TITLE_BLANK_CROSSINGS`]) and stops at an
///    author/editor block, a name list, a junk line, a section heading, a
///    year-only line, a line ending in sentence punctuation, or
///    [`TITLE_JOIN_LIMIT`] characters — with the extra guards of
///    [`TITLE_FRAGMENT_MAX_CHARS`] and [`looks_like_affiliation`] applied
///    whenever a blank line is crossed.
///
/// Only the front matter is searched (see [`front_matter_lines`]); a title is
/// never further in than that, and the bound keeps the scan cheap on a
/// 264-page book.
///
/// Returns `None` when no line qualifies, leaving the caller's filename
/// fallback in play.
fn title_from_text(text: &str) -> Option<String> {
    title_block(&front_matter_lines(text)).map(|(title, _)| title)
}

/// Authors recovered positionally from the extracted text, plus whether the
/// block that followed them was explicitly labelled as *editors*.
#[derive(Debug, Default, Clone)]
struct TextAuthors {
    /// The parsed author list, empty when nothing plausible was found.
    authors: Vec<Author>,
    /// True when the author-block scan stopped at an `Editors:` label — proof
    /// that this cover separates authors from editors, and therefore that the
    /// PDF's single `/Author` field cannot be trusted to hold an author.
    stopped_at_editors: bool,
}

/// How many blank lines the author-block scan tolerates inside or before the
/// block. The JRC cover puts one blank line between `Kugeler, K., Nabielek, H.,`
/// and `Buckthorpe, D.`; the HTR-10 papers put one between title and authors.
const MAX_AUTHOR_GAP_BLANKS: usize = 2;

/// How many text lines the author block may span before the scan gives up, so a
/// runaway match cannot swallow a page.
const MAX_AUTHOR_BLOCK_LINES: usize = 6;

/// Recover authors from the line block sitting directly beneath the title.
///
/// This is the fallback the extractor previously did not have at all: before
/// bead `op-szai` the only author sources were the PDF Info dictionary and the
/// eScholarship cover parser, so both HTR-10 papers — which carry no `/Author`
/// key — were catalogued with an empty author list.
///
/// The scan starts at the line after [`title_block`] ends and accepts
/// consecutive lines that [`looks_like_author_line`] or [`looks_like_name_list`]
/// recognises, stepping over single blank lines. It stops at the first line that
/// is neither (an affiliation, an abstract, a date line), and it stops
/// *specifically* at an [`is_editor_label`] line, recording that fact so the
/// caller can prefer these authors over the Info dictionary's editor.
///
/// **Fails soft by construction:** any line the name heuristics do not
/// confidently recognise ends the block, so the usual failure is an empty author
/// list a human fills in — never a confidently wrong one.
fn authors_from_text(text: &str) -> TextAuthors {
    let lines = front_matter_lines(text);
    let mut found = TextAuthors::default();
    let Some((_, start)) = title_block(&lines) else {
        return found;
    };

    let mut raw: Vec<&str> = Vec::new();
    let mut blanks = 0usize;
    let mut i = start;
    while i < lines.len() && raw.len() < MAX_AUTHOR_BLOCK_LINES {
        let line = lines[i];
        if line.is_empty() {
            blanks += 1;
            if blanks > MAX_AUTHOR_GAP_BLANKS {
                break;
            }
            i += 1;
            continue;
        }
        if is_editor_label(line) {
            found.stopped_at_editors = true;
            break;
        }
        if !(looks_like_author_line(line) || looks_like_name_list(line)) {
            break;
        }
        raw.push(line);
        blanks = 0;
        i += 1;
    }

    if !raw.is_empty() {
        found.authors = parse_author_block(&raw.join(" "));
    }
    found
}

/// Parse a cover/opening author block into [`Author`]s.
///
/// Two layouts occur in this archive and they need opposite readings of the
/// comma:
///
/// - **Family-first with initials** — `Kugeler, K., Nabielek, H., Buckthorpe, D.`
///   (JRC HTR book). Every comma separates a *fragment*, not an author, so the
///   fragments are paired back up: a non-initials fragment is a family name and
///   an initials fragment immediately after it is that author's given initials.
///   Feeding this whole string to [`parse_one_author`] produced the single
///   nonsense author `Kugeler` / `K., Nabielek, H., Buckthorpe, D.`.
/// - **Given-name-first** — `Zongxin Wu, Dengcai Lin, Daxin Zhong` (Elsevier
///   HTR-10 papers). Here each comma *does* separate an author, and each
///   fragment goes through [`parse_one_author`] unchanged.
///
/// The layouts are told apart by the presence of an [`is_initials`] fragment.
/// Mixed blocks (`Kugeler, Kurt, Nabielek, H.`) are not handled and will
/// under-recover given names rather than mis-assign them.
fn parse_author_block(raw: &str) -> Vec<Author> {
    let parts = split_name_fragments(raw);
    if parts.is_empty() {
        return Vec::new();
    }

    if !parts.iter().any(|p| is_initials(p)) {
        return parts.iter().map(|p| parse_one_author(p)).collect();
    }

    let mut authors = Vec::new();
    let mut i = 0;
    while i < parts.len() {
        // A leading initials fragment has no family name to attach to; skipping
        // it loses one author rather than inventing a family name from initials.
        if is_initials(&parts[i]) {
            i += 1;
            continue;
        }
        let given = parts
            .get(i + 1)
            .filter(|p| is_initials(p))
            .cloned()
            .unwrap_or_default();
        let step = if given.is_empty() { 1 } else { 2 };
        authors.push(Author {
            family: parts[i].clone(),
            given,
            affiliation: None,
        });
        i += step;
    }
    authors
}

/// True when a line is nothing but a 4-digit year — the publication-date line
/// that commonly sits directly under a cover title.
fn year_only(line: &str) -> bool {
    let t = line.trim();
    t.len() == 4 && t.chars().all(|c| c.is_ascii_digit())
}

/// Lowercase hex SHA-256 of a file, or `None` if it cannot be read.
///
/// Recorded as `source_sha256` so a catalogued document can be matched back to
/// the exact bytes ingested — required by `DATA_POLICY.md`'s provenance rules and
/// the only way to tell two revisions of the same report apart. Streams the file
/// in 64 KiB blocks rather than reading it whole: ingested PDFs run to tens of
/// megabytes, and this also runs on Android/Termux where memory is tighter.
fn sha256_of(path: &Path) -> Option<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;

    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Some(format!("{:x}", hasher.finalize()))
}

/// True when the front matter carries a numbered-report identifier.
///
/// Recognises the agency report-number forms this archive actually meets — EU
/// `EUR 28712 EN`, JRC catalogue numbers, US lab `NUREG` / `INL/EXT` / `ORNL/TM`
/// series, and IAEA `TECDOC`. A document bearing one of these is a technical
/// report even when the enclosing directory did not say so, which is what
/// misfiled the JRC HTR book as [`crate::DocumentType::Other`].
///
/// Scans only the front matter, for the same reason [`find_doi`] does: a report
/// number deep in the body belongs to a *cited* report.
fn looks_like_report(text: &str) -> bool {
    let end = text
        .char_indices()
        .nth(FRONT_MATTER_CHARS)
        .map(|(i, _)| i)
        .unwrap_or(text.len());
    let front = text[..end].to_ascii_lowercase();
    front.contains("nureg")
        || front.contains("tecdoc")
        || front.contains("inl/ext")
        || front.contains("ornl/tm")
        || front.contains("jrc1")
        || (front.contains("eur ") && front.contains(" en"))
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

/// Title words that carry no identifying information and are skipped when
/// choosing the slug's title word (already lowercased and alphanumeric-only, to
/// match [`slug_token`] output). Kept to articles, prepositions and
/// conjunctions — anything domain-specific risks discarding the one word that
/// makes a citation key recognisable.
const SLUG_STOP_WORDS: &[&str] = &[
    "the", "and", "for", "with", "from", "into", "its", "this", "that", "these", "those", "are",
    "was", "were", "not", "but", "our", "using", "toward", "towards", "about", "over", "under",
    "between", "among", "upon", "onto", "via",
];

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
    let words: Vec<String> = title
        .split_whitespace()
        .map(slug_token)
        .filter(|w| w.len() >= 3)
        .collect();
    // Prefer the first *content* word. Taking the literal first word gave
    // `wu2002the` for "The design features of the HTR-10" and `kugeler2017the`
    // for the JRC book — technically stable, but useless as a citation key. If
    // a title is nothing but stop words the first word is used anyway, so a
    // slug is always produced.
    if let Some(word) = words
        .iter()
        .find(|w| !SLUG_STOP_WORDS.contains(&w.as_str()))
        .or_else(|| words.first())
    {
        parts.push(word.clone());
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
        // Provenance hash of the ingested bytes (bead `op-szai`, item 4). It was
        // previously left unset; it is now always computed, so assert its shape
        // rather than a literal digest, which would pin the synthetic fixture's
        // exact byte layout.
        let digest = doc
            .source_sha256
            .as_deref()
            .expect("source_sha256 recorded");
        assert_eq!(digest.len(), 64, "SHA-256 is 64 lowercase hex characters");
        assert!(digest
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
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

    /// The first 12 lines `pdf-extract` returns for
    /// `open/reports/kjna28712enn.pdf` (JRC, EUR 28712 EN), reproduced exactly —
    /// including the blank line between every visual line, which is what
    /// truncated the title, and the trailing spaces.
    ///
    /// Ground truth for this document: title *"The High Temperature Gas-cooled
    /// Reactor: Safety considerations of the (V)HTR-Modul"*, authors Kugeler,
    /// Nabielek, Buckthorpe; Scheuermann/Haneklaus/Fütterer are **editors**.
    const JRC_COVER: &str = "\n\
        \n\
        The High Temperature \n\
        \n\
        Gas-cooled Reactor\n\
        \n\
        Safety considerations of the \n\
        \n\
        (V)HTR-Modul \n\
         Kugeler, K., Nabielek, H., \n\
        \n\
        Buckthorpe, D. \n\
        \n\
        Editors: Scheuermann, W., \n\
        Haneklaus, N., Fütterer, M. \n\
        \n\
        2017 \n\
         EUR 28712 EN \n";

    /// The first 11 lines `pdf-extract` returns for
    /// `proprietary/papers/wu2002htr10.pdf` (Elsevier, *Nucl. Eng. Des.* 218),
    /// reproduced exactly — note the running header on the first content line
    /// and the corresponding-author asterisk.
    ///
    /// Ground truth: title *"The design features of the HTR-10"*, authors Wu,
    /// Zongxin / Lin, Dengcai / Zhong, Daxin.
    const ELSEVIER_WU_OPENING: &str = "\n\
        \n\
        Nuclear Engineering and Design 218 (2002) 25\u{2013}32\n\
        \n\
        The design features of the HTR-10\n\
        \n\
        Zongxin Wu *, Dengcai Lin, Daxin Zhong\n\
        \n\
        Institute of Nuclear Energy and Technology, Tsinghua Uniersity, Beijing 100084, China\n\
        \n\
        Received 11 July 2001; received in revised form 24 February 2002; accepted 11 March 2002\n\
        \n\
        Abstract\n";

    /// As [`ELSEVIER_WU_OPENING`] but for
    /// `proprietary/papers/gao2002htr10th.pdf`, whose title wraps onto a second
    /// line with **no** blank line between the two — the other cover layout.
    ///
    /// Ground truth: title *"Thermal hydraulic calculation of the HTR-10 for the
    /// initial and equilibrium core"*, authors Gao, Zuying / Shi, Lei.
    const ELSEVIER_GAO_OPENING: &str = "\n\
        \n\
        Nuclear Engineering and Design 218 (2002) 51\u{2013}64\n\
        \n\
        Thermal hydraulic calculation of the HTR-10 for the initial\n\
        and equilibrium core\n\
        \n\
        Zuying Gao *, Lei Shi\n\
        \n\
        Institute of Nuclear Energy Technology, Tsinghua Uniersity, Beijing 100084, China\n\
        \n\
        Received 11 July 2001; received in revised form 24 February 2002; accepted 11 March 2002\n";

    /// A journal running header is publisher furniture, not a title.
    ///
    /// **Methodology.** Feed [`looks_like_journal_locator`] the exact first
    /// content line of each HTR-10 paper, plus the `Vol./pp.` variant, and
    /// require `true`; feed it the real titles of all three test documents and
    /// require `false`. Pass criterion: exact boolean match on every case.
    ///
    /// **Result (2026-08-11).** All cases pass. Against the real PDFs, `lit
    /// import` previously titled both Elsevier papers
    /// `Nuclear Engineering and Design 218 (2002) 25-32` / `… 51-64`; it now
    /// reports the true titles (see [`elsevier_opening_yields_title_and_authors`]).
    #[test]
    fn journal_running_headers_are_rejected_as_titles() {
        for header in [
            "Nuclear Engineering and Design 218 (2002) 25\u{2013}32",
            "Nuclear Engineering and Design 218 (2002) 51\u{2013}64",
            "Annals of Nuclear Energy 145 (2020) 107-118",
            "Nucl. Eng. Des., Vol. 218, No. 1, pp. 25-32, 2002",
        ] {
            assert!(
                looks_like_journal_locator(header),
                "{header:?} must be recognised as a journal locator"
            );
            assert!(is_title_junk(header), "{header:?} must not be a title");
        }

        for title in [
            "The design features of the HTR-10",
            "Thermal hydraulic calculation of the HTR-10 for the initial",
            "The High Temperature Gas-cooled Reactor",
            "Safety considerations of the (V)HTR-Modul",
        ] {
            assert!(
                !looks_like_journal_locator(title),
                "{title:?} is a real title and must survive"
            );
        }
    }

    /// Author lines must be recognised in both the initials-first and
    /// given-name-first layouts, and affiliations/titles must not be.
    #[test]
    fn name_list_detection_separates_authors_from_prose() {
        for authors in [
            "Zongxin Wu *, Dengcai Lin, Daxin Zhong",
            "Zuying Gao *, Lei Shi",
            "Kugeler, K., Nabielek, H.,",
            "Buckthorpe, D.",
        ] {
            assert!(
                looks_like_name_list(authors) || looks_like_author_line(authors),
                "{authors:?} must read as an author line"
            );
        }

        for not_authors in [
            // The affiliation that follows both HTR-10 author lines.
            "Institute of Nuclear Energy and Technology, Tsinghua Uniersity, Beijing 100084, China",
            // A title-cased title with a comma — the false positive this guards.
            "Safety, Reliability and Cost",
            "The design features of the HTR-10",
            "Thermal hydraulic calculation of the HTR-10 for the initial",
            "Gas-cooled Reactor",
        ] {
            assert!(
                !looks_like_name_list(not_authors),
                "{not_authors:?} must not read as an author list"
            );
        }
    }

    /// Regression test for the truncated JRC title (bead `op-szai`, item 1).
    ///
    /// **Methodology.** Run [`title_block`] over [`JRC_COVER`] — the verbatim
    /// extracted lines of `kjna28712enn.pdf` — and require the four
    /// blank-separated cover lines to be rejoined and the author block *not* to
    /// be absorbed. Reference: the printed cover title. Pass criterion: exact
    /// string equality.
    ///
    /// **Result (2026-08-11).** Returns *"The High Temperature Gas-cooled
    /// Reactor Safety considerations of the (V)HTR-Modul"*. Before the fix,
    /// `lit import` on the real PDF recorded `The High Temperature`, because the
    /// join stopped at the first blank line. The recovered string still lacks
    /// the printed colon after *Reactor* — the cover expresses the title/subtitle
    /// break with layout, and the extracted text carries no punctuation for it,
    /// so no offline heuristic can recover it. That is a known, documented
    /// residual for a human reviewer, not a silent error.
    #[test]
    fn jrc_cover_title_is_rejoined_across_blank_lines() {
        let lines = front_matter_lines(JRC_COVER);
        let (title, end) = title_block(&lines).expect("a title");
        assert_eq!(
            title,
            "The High Temperature Gas-cooled Reactor Safety considerations of the (V)HTR-Modul"
        );
        assert_eq!(
            lines[end], "Kugeler, K., Nabielek, H.,",
            "the title block must end exactly where the author block starts"
        );
    }

    /// Regression test for the editor-as-author misattribution (bead `op-szai`,
    /// item 2) and for the missing text author fallback (item 3).
    ///
    /// **Methodology.** Run [`authors_from_text`] over [`JRC_COVER`] and require
    /// the three authors printed *above* the `Editors:` label, in order, with
    /// `stopped_at_editors` set. Reference: the printed cover. Pass criterion:
    /// exact family/given match and correct list length.
    ///
    /// **Result (2026-08-11).** Returns Kugeler/K., Nabielek/H., Buckthorpe/D.
    /// with `stopped_at_editors == true`. Before the fix the record held the
    /// single author `Scheuermann, Walter` — the first *editor*, taken from the
    /// PDF `/Author` key. Given names are recovered only as the initials the
    /// cover prints; the hand-verified record spells them Kurt and Heinz, which
    /// the document itself does not state.
    #[test]
    fn jrc_cover_authors_come_from_above_the_editors_label() {
        let found = authors_from_text(JRC_COVER);
        assert!(
            found.stopped_at_editors,
            "the Editors: label must be seen, or the Info dictionary would win"
        );
        let names: Vec<(String, String)> = found
            .authors
            .iter()
            .map(|a| (a.family.clone(), a.given.clone()))
            .collect();
        assert_eq!(
            names,
            vec![
                ("Kugeler".to_string(), "K.".to_string()),
                ("Nabielek".to_string(), "H.".to_string()),
                ("Buckthorpe".to_string(), "D.".to_string()),
            ]
        );
    }

    /// Regression test for the Elsevier running-header title and the empty
    /// author lists (bead `op-szai`, items 1 and 3), on both HTR-10 papers.
    ///
    /// **Methodology.** Run [`title_from_text`] and [`authors_from_text`] over
    /// the verbatim extracted openings of `wu2002htr10.pdf` and
    /// `gao2002htr10th.pdf`. References are the hand-verified records
    /// `proprietary/papers/wu2002htr10.json` and `…/gao2002htr10th.json`. Pass
    /// criterion: exact title equality and exact family/given equality for every
    /// author.
    ///
    /// **Result (2026-08-11).** Both titles and all five authors match the
    /// hand-verified records exactly. Before the fix both documents were titled
    /// after the journal running header and carried **no** authors at all, since
    /// neither PDF has an `/Author` key and no text fallback existed. The
    /// affiliation line immediately below the authors is correctly refused, so
    /// no institution leaks into the author list.
    #[test]
    fn elsevier_opening_yields_title_and_authors() {
        assert_eq!(
            title_from_text(ELSEVIER_WU_OPENING).as_deref(),
            Some("The design features of the HTR-10")
        );
        let wu = authors_from_text(ELSEVIER_WU_OPENING);
        assert!(!wu.stopped_at_editors);
        let wu_names: Vec<(String, String)> = wu
            .authors
            .iter()
            .map(|a| (a.family.clone(), a.given.clone()))
            .collect();
        assert_eq!(
            wu_names,
            vec![
                ("Wu".to_string(), "Zongxin".to_string()),
                ("Lin".to_string(), "Dengcai".to_string()),
                ("Zhong".to_string(), "Daxin".to_string()),
            ]
        );

        assert_eq!(
            title_from_text(ELSEVIER_GAO_OPENING).as_deref(),
            Some(
                "Thermal hydraulic calculation of the HTR-10 for the initial and equilibrium core"
            )
        );
        let gao_names: Vec<(String, String)> = authors_from_text(ELSEVIER_GAO_OPENING)
            .authors
            .iter()
            .map(|a| (a.family.clone(), a.given.clone()))
            .collect();
        assert_eq!(
            gao_names,
            vec![
                ("Gao".to_string(), "Zuying".to_string()),
                ("Shi".to_string(), "Lei".to_string()),
            ]
        );
    }

    /// The two author-block layouts must be parsed by opposite readings of the
    /// comma; feeding either to [`parse_one_author`] whole is wrong.
    #[test]
    fn author_block_handles_initials_and_given_name_first() {
        let jrc = parse_author_block("Kugeler, K., Nabielek, H., Buckthorpe, D.");
        assert_eq!(jrc.len(), 3, "commas here separate fragments, not authors");
        assert_eq!(
            (jrc[0].family.as_str(), jrc[0].given.as_str()),
            ("Kugeler", "K.")
        );
        assert_eq!(
            (jrc[2].family.as_str(), jrc[2].given.as_str()),
            ("Buckthorpe", "D.")
        );

        let elsevier = parse_author_block("Zongxin Wu *, Dengcai Lin, Daxin Zhong");
        assert_eq!(elsevier.len(), 3, "commas here do separate authors");
        assert_eq!(
            (elsevier[0].family.as_str(), elsevier[0].given.as_str()),
            ("Wu", "Zongxin"),
            "the corresponding-author asterisk must not enter the name"
        );
    }

    /// The title join must not step over a blank line into body text.
    ///
    /// Guards the risk introduced by [`MAX_TITLE_BLANK_CROSSINGS`]: a long
    /// sentence, an affiliation, or a section heading below the title must all
    /// end the join.
    #[test]
    fn title_join_stops_at_body_text_across_a_blank_line() {
        let text = "A Short Cover Title\n\n\
            This is an ordinary sentence of body text that runs well past the fragment limit.\n";
        assert_eq!(
            title_from_text(text).as_deref(),
            Some("A Short Cover Title")
        );

        let heading = "A Short Cover Title\n\nAbstract\n\nSome prose.\n";
        assert_eq!(
            title_from_text(heading).as_deref(),
            Some("A Short Cover Title")
        );

        let affiliation =
            "A Short Cover Title\n\nInstitute of Nuclear Energy and Technology, Beijing, China\n";
        assert_eq!(
            title_from_text(affiliation).as_deref(),
            Some("A Short Cover Title")
        );
    }

    /// Footnote markers and affiliation superscripts are not part of a name.
    #[test]
    fn author_markers_are_stripped() {
        assert_eq!(strip_author_markers(" Zongxin Wu * "), "Zongxin Wu");
        assert_eq!(strip_author_markers("Lei Shi\u{2020}"), "Lei Shi");
        assert_eq!(strip_author_markers("Dengcai Lin2"), "Dengcai Lin");
        assert!(is_initials("K."));
        assert!(is_initials("M.J."));
        assert!(!is_initials("Wu"));
        assert!(!is_initials("Zongxin"));
    }

    /// The slug's title word must be a content word, not an article.
    ///
    /// Before the fix the JRC book slugged to `kugeler2017the` and the Wu paper
    /// to `wu2002the`, since both titles begin with "The".
    #[test]
    fn slug_skips_leading_stop_words() {
        let wu = vec![Author {
            family: "Wu".to_string(),
            given: "Zongxin".to_string(),
            affiliation: None,
        }];
        assert_eq!(
            make_slug(&wu, Some(2002), "The design features of the HTR-10"),
            "wu2002design"
        );
        // A title made only of stop words still yields a slug.
        assert_eq!(make_slug(&wu, Some(2002), "The and for"), "wu2002the");
    }
}
