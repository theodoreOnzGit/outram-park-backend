//! Pure metadata helpers used by the review form: author-line parsing, the
//! year/report-number heuristics behind the advisories, and slug/id derivation.
//!
//! Split out of [`super::review`] so each file stays inside the workspace's
//! file-size cap, and because everything here is a pure function of its inputs —
//! no state, no I/O — which makes it the easiest part of the ingestion flow to
//! test and to audit.
//!
//! # Duplication flagged on purpose
//!
//! [`make_slug`] and [`make_id`] **mirror private functions of the same name in
//! `kovan-literature`** (`src/metadata.rs`). That crate derives a document's
//! slug and id exactly once, inside `extract_metadata`, and exposes no public
//! way to re-derive them — so correcting a wrong year in the TUI would otherwise
//! leave the citation key frozen at the extractor's mistake (`2004anl7416`).
//! They are kept byte-for-byte compatible here, and the shared cases are pinned
//! by a test (`slug_matches_the_library_algorithm_on_a_known_case`). The proper
//! fix is a public `kovan_literature::derive_identifiers(&mut KovanDocument)`;
//! see this crate's `DECISIONS.md`.

use kovan_common::Author;

/// Plausible publication-year window used both for validating a typed year and
/// for scanning the document body for candidate years. Deliberately wide — the
/// point is to reject typos (`19777`, `abc`), not to second-guess the user.
pub const YEAR_RANGE: std::ops::RangeInclusive<u32> = 1800..=2100;

/// How much of the generated Markdown body is scanned for candidate publication
/// years (characters). The front matter of a report carries its real date; a
/// full 447-page scan would only add noise (every "in 1953 …" in the text).
const YEAR_SCAN_CHARS: usize = 20_000;

/// Parse the author line into [`Author`]s.
///
/// Authors are separated by `;`. Within one author, a comma splits
/// `Family, Given` (BibTeX name order). An entry with no comma is treated as a
/// **corporate author**: the whole string becomes `family` with an empty
/// `given`, which is the convention `kovan_common::Author` documents for
/// organisations — so typing `Argonne Code Center` yields exactly one corporate
/// author, not three people.
///
/// Blank entries are dropped, so a trailing `;` is harmless.
pub fn parse_authors(text: &str) -> Vec<Author> {
    text.split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|entry| match entry.split_once(',') {
            Some((family, given)) => Author {
                family: family.trim().to_string(),
                given: given.trim().to_string(),
                affiliation: None,
            },
            None => Author {
                family: entry.to_string(),
                given: String::new(),
                affiliation: None,
            },
        })
        .collect()
}

/// Render an author list back into the editable form [`parse_authors`] accepts.
/// Round-trips: `parse_authors(&format_authors(&a)) == a` for any list built by
/// `parse_authors`.
pub fn format_authors(authors: &[Author]) -> String {
    authors
        .iter()
        .map(|a| {
            if a.given.trim().is_empty() {
                a.family.clone()
            } else {
                format!("{}, {}", a.family, a.given)
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// Distinct plausible publication years appearing in `text`, ascending.
///
/// Scans only the first [`YEAR_SCAN_CHARS`] characters (a report's front matter)
/// and accepts any 4-digit run inside [`YEAR_RANGE`] that is not part of a longer
/// digit run — so `ANL-7416` and `19770` contribute nothing.
pub fn years_in_text(text: &str) -> Vec<u32> {
    let head: Vec<char> = text.chars().take(YEAR_SCAN_CHARS).collect();
    let mut years: Vec<u32> = Vec::new();
    let mut i = 0;
    while i < head.len() {
        if !head[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < head.len() && head[i].is_ascii_digit() {
            i += 1;
        }
        if i - start == 4 {
            let value: u32 = head[start..i]
                .iter()
                .collect::<String>()
                .parse()
                .unwrap_or(0);
            if YEAR_RANGE.contains(&value) && !years.contains(&value) {
                years.push(value);
            }
        }
    }
    years.sort_unstable();
    years
}

/// Heuristic: does `title` look like a bare report identifier (e.g.
/// `ANL-7416 Supplement 2`) rather than a descriptive title?
///
/// True when the string is short (at most four whitespace tokens) and contains a
/// token that mixes letters with digits or hyphens — the shape of a report
/// number. Advisory only; it never changes the data.
pub fn looks_like_report_number(title: &str) -> bool {
    let tokens: Vec<&str> = title.split_whitespace().collect();
    if tokens.is_empty() || tokens.len() > 4 {
        return false;
    }
    tokens.iter().any(|t| {
        let has_digit = t.chars().any(|c| c.is_ascii_digit());
        let has_alpha = t.chars().any(|c| c.is_ascii_alphabetic());
        has_digit && (has_alpha || t.contains('-'))
    })
}

/// Build the BibTeX-style slug `<firstauthorfamily><year><firsttitleword>`,
/// lowercased and alphanumeric-only (e.g. `argonnecodecenter1977anl7416`),
/// falling back to a slugged title when neither author nor year is known.
///
/// **Mirrors `kovan-literature`'s private `make_slug`** (see the module docs and
/// this crate's `DECISIONS.md`): that crate derives the slug once, inside
/// `extract_metadata`, and exposes no way to re-derive it after a correction.
/// Kept byte-for-byte compatible so a document corrected here carries the same
/// slug the library would have produced had extraction been right first time.
pub fn make_slug(authors: &[Author], year: Option<u32>, title: &str) -> String {
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

/// Lowercase a token, keeping only ASCII alphanumerics.
fn slug_token(s: &str) -> String {
    s.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Build the stable content id `kovan-<fnv1a64 hex>` from slug and title.
///
/// Mirrors `kovan-literature`'s private `make_id` for the same reason as
/// [`make_slug`]. Deterministic — no timestamps, no randomness — so re-ingesting
/// a document with the same corrections yields the same id.
pub fn make_id(slug: &str, title: &str) -> String {
    let mut data = Vec::with_capacity(slug.len() + title.len() + 1);
    data.extend_from_slice(slug.as_bytes());
    data.push(0);
    data.extend_from_slice(title.as_bytes());
    format!("kovan-{:016x}", fnv1a64(&data))
}

/// 64-bit FNV-1a hash — small, dependency-free, deterministic. Used only for
/// document ids, never for security.
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

    #[test]
    fn authors_round_trip_through_the_edit_form() {
        let authors = parse_authors("Smith, John; Doe, Jane; Argonne Code Center");
        assert_eq!(authors.len(), 3);
        assert_eq!(authors[0].given, "John");
        assert_eq!(authors[2].given, "", "no comma means a corporate author");
        assert_eq!(
            parse_authors(&format_authors(&authors)),
            authors,
            "format → parse must be lossless"
        );
    }

    #[test]
    fn author_parsing_ignores_blank_entries() {
        assert_eq!(parse_authors("  ").len(), 0);
        assert_eq!(parse_authors("Doe, Jane;").len(), 1);
    }

    #[test]
    fn years_in_text_finds_distinct_four_digit_years_only() {
        let years = years_in_text("ANL-7416, June 1977, reprinted 1977 and 1981; id 19770");
        assert_eq!(years, vec![1977, 1981]);
    }

    #[test]
    fn years_outside_the_plausible_window_are_ignored() {
        assert!(years_in_text("part 4321 of 9999").is_empty());
    }

    #[test]
    fn report_number_heuristic_separates_identifiers_from_titles() {
        assert!(looks_like_report_number("ANL-7416 Supplement 2"));
        assert!(looks_like_report_number("NEACRP-L-330"));
        assert!(!looks_like_report_number(
            "A Study of Steam Table Accuracy in Reactor Analysis"
        ));
        assert!(!looks_like_report_number(""));
    }

    #[test]
    fn slug_matches_the_library_algorithm_on_a_known_case() {
        // Same expectation as kovan-literature's own `make_slug` unit test.
        let authors = parse_authors("Doe, Jane");
        assert_eq!(
            make_slug(&authors, Some(2021), "Test Report"),
            "doe2021test"
        );
    }

    #[test]
    fn slug_uses_a_corrected_corporate_author_and_year() {
        let authors = parse_authors("Argonne Code Center");
        assert_eq!(
            make_slug(&authors, Some(1977), "ANL-7416 Supplement 2"),
            "argonnecodecenter1977anl7416"
        );
    }

    #[test]
    fn slug_falls_back_when_nothing_is_known() {
        assert_eq!(make_slug(&[], None, "!!!"), "document");
        assert_eq!(make_slug(&[], None, "Untitled"), "untitled");
    }

    #[test]
    fn ids_are_deterministic_and_slug_sensitive() {
        assert_eq!(make_id("a", "T"), make_id("a", "T"));
        assert_ne!(make_id("a", "T"), make_id("b", "T"));
        assert!(make_id("a", "T").starts_with("kovan-"));
    }
}
