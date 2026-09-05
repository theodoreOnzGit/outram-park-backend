//! The **metadata review** step of the ingestion flow — the part that exists
//! because `kovan_literature::extract_metadata` is explicitly best-effort.
//!
//! # Why this screen exists
//!
//! `kovan-literature`'s own module docs say metadata is recovered "best-effort"
//! and that "a human reviewer fills gaps against the source". Until now nothing
//! in KOVAN gave that reviewer a place to stand: the CLI (`kovan lit import`)
//! prints the extracted record and writes it straight to disk. Wrong metadata
//! then flows into the generated BibTeX and from there into a citation, which
//! the workspace's `RESEARCH_INTEGRITY_AND_PROVENANCE.md` treats as a real
//! integrity problem rather than a cosmetic one.
//!
//! A real, observed failure (2026-08-05, a 1977 Argonne benchmark-problem
//! report) motivated the design: extraction produced the correct title
//! (`ANL-7416 Supplement 2`) but `year: 2004` — a digitisation date from the
//! scan, not the publication year — an empty author list (the real corporate
//! author is "Argonne Code Center"), and therefore the slug `2004anl7416`.
//!
//! # What it does
//!
//! Presents every citation-critical field as an editable line, flags the fields
//! that are *typically* wrong ([`ReviewState::advisories`]), and re-derives the
//! slug/id from the **corrected** values before anything is written — so a
//! corrected year really does produce `argonnecodecenter1977anl7416`, not a slug
//! frozen at extraction time.
//!
//! Nothing here mutates the extracted document in place: the pristine record
//! from `kovan-literature` is kept in [`ReviewState::extracted`] so the UI can
//! show what was changed, and a corrected copy is built on demand by
//! [`ReviewState::corrected_document`].

use std::path::{Path, PathBuf};
use std::time::Duration;

use kovan_common::{DocumentType, KovanDocument, Visibility};

use super::metadata::{
    format_authors, looks_like_report_number, make_id, make_slug, parse_authors, years_in_text,
    YEAR_RANGE,
};
use crate::tui::text_input::TextInput;

/// Default literature-archive root used to derive output paths, matching the
/// Literature tab's default (`kovan-tui` is normally launched from the
/// repository root). The generated sub-directories under it follow
/// `docs/kovan.md` § "Storage Layout" via
/// [`kovan_literature::storage::generated_dir_for`].
pub const DEFAULT_ARCHIVE_ROOT: &str = "crates/kovan-literature";

/// One editable row of the review form.
///
/// [`ReviewField::DocType`] is the only non-text row: it cycles through
/// [`DocumentType`] with Left/Right instead of accepting typed characters,
/// because the set of document types is closed (and picking from it cannot be
/// misspelled).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewField {
    /// Full document title (BibTeX `title`).
    Title,
    /// Author list, typed as `Family, Given; Family, Given` — see
    /// [`parse_authors`].
    Authors,
    /// Publication year (BibTeX `year`), or empty for "unknown".
    Year,
    /// Document type; drives the BibTeX entry type (`@techreport`, `@article`…).
    DocType,
    /// Issuing institution / awarding school (BibTeX `institution`/`school`).
    Institution,
    /// Where to write the generated Markdown body (empty = don't write).
    MarkdownOut,
    /// Where to write the canonical `KovanDocument` JSON (empty = don't write).
    JsonOut,
    /// Where to write the generated BibTeX entry (empty = don't write).
    BibtexOut,
}

/// Field order on screen, and the order Up/Down cycles through.
const FIELDS: [ReviewField; 8] = [
    ReviewField::Title,
    ReviewField::Authors,
    ReviewField::Year,
    ReviewField::DocType,
    ReviewField::Institution,
    ReviewField::MarkdownOut,
    ReviewField::JsonOut,
    ReviewField::BibtexOut,
];

impl ReviewField {
    /// Human-readable row label, as rendered in the form.
    pub fn label(self) -> &'static str {
        match self {
            ReviewField::Title => "Title",
            ReviewField::Authors => "Authors",
            ReviewField::Year => "Year",
            ReviewField::DocType => "Type",
            ReviewField::Institution => "Institution",
            ReviewField::MarkdownOut => "Markdown out",
            ReviewField::JsonOut => "JSON out",
            ReviewField::BibtexOut => "BibTeX out",
        }
    }

    /// Move `delta` rows down (negative = up), wrapping at both ends.
    pub fn step(self, delta: i32) -> Self {
        let i = FIELDS.iter().position(|f| *f == self).unwrap_or(0) as i32;
        let n = FIELDS.len() as i32;
        let j = ((i + delta) % n + n) % n;
        FIELDS[j as usize]
    }

    /// Whether this row accepts typed text (everything except
    /// [`ReviewField::DocType`], which is cycled).
    pub fn is_text(self) -> bool {
        self != ReviewField::DocType
    }
}

/// Every [`DocumentType`], in the order Left/Right cycles them.
const DOCUMENT_TYPES: [DocumentType; 7] = [
    DocumentType::Paper,
    DocumentType::Report,
    DocumentType::Standard,
    DocumentType::Benchmark,
    DocumentType::Manual,
    DocumentType::Thesis,
    DocumentType::Other,
];

/// Cycle a [`DocumentType`] by `delta` positions, wrapping at both ends.
pub fn step_document_type(current: DocumentType, delta: i32) -> DocumentType {
    let i = DOCUMENT_TYPES
        .iter()
        .position(|t| *t == current)
        .unwrap_or(0) as i32;
    let n = DOCUMENT_TYPES.len() as i32;
    let j = ((i + delta) % n + n) % n;
    DOCUMENT_TYPES[j as usize]
}

/// The editable review form for one extracted document.
///
/// Holds both the pristine extraction ([`ReviewState::extracted`]) and the
/// user's edits, so the UI can show which fields a human changed and the saved
/// record can be rebuilt from the corrected values.
pub struct ReviewState {
    /// The PDF this document was extracted from.
    pub source_pdf: PathBuf,
    /// The untouched record returned by `kovan_literature::extract_metadata`.
    /// Never mutated — it is the "what the extractor said" reference.
    pub extracted: KovanDocument,
    /// Wall-clock time the extraction took, for display.
    pub elapsed: Duration,

    /// Editable title.
    pub title: TextInput,
    /// Editable author list, in `Family, Given; …` form.
    pub authors: TextInput,
    /// Editable year, as typed (empty = unknown).
    pub year: TextInput,
    /// Selected document type (cycled, not typed).
    pub document_type: DocumentType,
    /// Editable institution (empty = none).
    pub institution: TextInput,

    /// Output path for the generated Markdown body (empty = skip).
    pub markdown_out: TextInput,
    /// Output path for the `KovanDocument` JSON (empty = skip).
    pub json_out: TextInput,
    /// Output path for the generated BibTeX entry (empty = skip).
    pub bibtex_out: TextInput,
    /// `true` once the user has hand-edited any output path, after which the
    /// slug-derived defaults stop overwriting them.
    pub outputs_pinned: bool,

    /// Which row has focus.
    pub field: ReviewField,
    /// Lines written by the last [`ReviewState::save`] call (successes and
    /// per-file errors); empty before the first save.
    pub save_report: Vec<String>,
    /// Vertical scroll offset of the derived-record pane.
    pub preview_scroll: u16,
}

impl ReviewState {
    /// Build the form from a freshly extracted document.
    ///
    /// `elapsed` is the extraction wall time (displayed, not used in any
    /// derivation). Output paths start at the slug-derived defaults; see
    /// [`ReviewState::refresh_output_defaults`].
    pub fn new(source_pdf: PathBuf, extracted: KovanDocument, elapsed: Duration) -> Self {
        let mut state = Self {
            title: TextInput::new(extracted.title.clone()),
            authors: TextInput::new(format_authors(&extracted.authors)),
            year: TextInput::new(extracted.year.map(|y| y.to_string()).unwrap_or_default()),
            document_type: extracted.document_type,
            institution: TextInput::new(extracted.institution.clone().unwrap_or_default()),
            markdown_out: TextInput::default(),
            json_out: TextInput::default(),
            bibtex_out: TextInput::default(),
            outputs_pinned: false,
            field: ReviewField::Title,
            save_report: Vec::new(),
            preview_scroll: 0,
            source_pdf,
            extracted,
            elapsed,
        };
        state.refresh_output_defaults();
        state
    }

    /// Recompute the three output paths from the *current* slug, unless the user
    /// has pinned them by editing one by hand ([`ReviewState::outputs_pinned`]).
    ///
    /// Markdown and BibTeX go to the storage-layout directories
    /// (`generated/{markdown,bibtex}/{open,proprietary}/`) resolved by
    /// [`kovan_literature::storage::generated_dir_for`]. The JSON record has no
    /// directory defined in `docs/kovan.md` § "Storage Layout", so it is placed
    /// beside the Markdown as a convenience default — flagged here because it is
    /// this crate's choice, not a documented layout rule.
    pub fn refresh_output_defaults(&mut self) {
        if self.outputs_pinned {
            return;
        }
        let slug = self.current_slug();
        let visibility = self.visibility();
        let base = Path::new(DEFAULT_ARCHIVE_ROOT);
        let md_dir = kovan_literature::storage::generated_dir_for(
            base,
            kovan_literature::storage::MARKDOWN_DIR,
            visibility,
        );
        let bib_dir = kovan_literature::storage::generated_dir_for(
            base,
            kovan_literature::storage::BIBTEX_DIR,
            visibility,
        );
        self.markdown_out
            .set(md_dir.join(format!("{slug}.md")).to_string_lossy().as_ref());
        self.json_out.set(
            md_dir
                .join(format!("{slug}.json"))
                .to_string_lossy()
                .as_ref(),
        );
        self.bibtex_out.set(
            bib_dir
                .join(format!("{slug}.bib"))
                .to_string_lossy()
                .as_ref(),
        );
    }

    /// Visibility of the document being reviewed — inherited from the extraction
    /// (which infers it from the source path, so a PDF under `proprietary/`
    /// stays proprietary and its artifacts land in the gitignored half of the
    /// generated tree).
    pub fn visibility(&self) -> Visibility {
        self.extracted.visibility
    }

    /// The slug the corrected record would carry, or the extracted slug when the
    /// form is currently invalid (e.g. a half-typed year).
    pub fn current_slug(&self) -> String {
        match self.corrected_document() {
            Ok(doc) => doc.slug,
            Err(_) => self.extracted.slug.clone(),
        }
    }

    /// Mutable access to the focused text field, or `None` when the focused row
    /// is the cycled [`ReviewField::DocType`].
    pub fn focused_input_mut(&mut self) -> Option<&mut TextInput> {
        match self.field {
            ReviewField::Title => Some(&mut self.title),
            ReviewField::Authors => Some(&mut self.authors),
            ReviewField::Year => Some(&mut self.year),
            ReviewField::DocType => None,
            ReviewField::Institution => Some(&mut self.institution),
            ReviewField::MarkdownOut => Some(&mut self.markdown_out),
            ReviewField::JsonOut => Some(&mut self.json_out),
            ReviewField::BibtexOut => Some(&mut self.bibtex_out),
        }
    }

    /// Current text of `field`, for rendering. The cycled type row renders its
    /// `Debug` name.
    pub fn field_value(&self, field: ReviewField) -> String {
        match field {
            ReviewField::Title => self.title.value().to_string(),
            ReviewField::Authors => self.authors.value().to_string(),
            ReviewField::Year => self.year.value().to_string(),
            ReviewField::DocType => format!("{:?}", self.document_type),
            ReviewField::Institution => self.institution.value().to_string(),
            ReviewField::MarkdownOut => self.markdown_out.value().to_string(),
            ReviewField::JsonOut => self.json_out.value().to_string(),
            ReviewField::BibtexOut => self.bibtex_out.value().to_string(),
        }
    }

    /// Whether `field` now differs from what the extractor produced — rendered
    /// as an "edited" marker so a reviewer can see at a glance which values are
    /// human-supplied rather than machine-guessed.
    pub fn is_edited(&self, field: ReviewField) -> bool {
        match field {
            ReviewField::Title => self.title.value() != self.extracted.title,
            ReviewField::Authors => self.authors.value() != format_authors(&self.extracted.authors),
            ReviewField::Year => {
                self.year.value()
                    != self
                        .extracted
                        .year
                        .map(|y| y.to_string())
                        .unwrap_or_default()
            }
            ReviewField::DocType => self.document_type != self.extracted.document_type,
            ReviewField::Institution => {
                self.institution.value() != self.extracted.institution.clone().unwrap_or_default()
            }
            // Output paths are this crate's suggestion, not an extraction, so
            // "edited" has no meaning for them.
            ReviewField::MarkdownOut | ReviewField::JsonOut | ReviewField::BibtexOut => false,
        }
    }

    /// Build the corrected [`KovanDocument`].
    ///
    /// Starts from the extracted record (keeping the Markdown body, page count,
    /// DOI, keywords, source path and visibility), applies the reviewed fields,
    /// and **re-derives the slug and id from the corrected values** so a fixed
    /// year/author really does change the citation key.
    ///
    /// Returns the list of validation problems when the form cannot be turned
    /// into a record (empty title, or a year that is not a plausible 4-digit
    /// number).
    pub fn corrected_document(&self) -> Result<KovanDocument, Vec<String>> {
        let mut problems = Vec::new();

        let title = self.title.value().trim().to_string();
        if title.is_empty() {
            problems.push("title must not be empty".to_string());
        }

        let year_text = self.year.value().trim();
        let year = if year_text.is_empty() {
            None
        } else {
            match year_text.parse::<u32>() {
                Ok(y) if YEAR_RANGE.contains(&y) => Some(y),
                _ => {
                    problems.push(format!(
                        "year '{year_text}' is not a plausible 4-digit year ({}-{})",
                        YEAR_RANGE.start(),
                        YEAR_RANGE.end()
                    ));
                    None
                }
            }
        };

        if !problems.is_empty() {
            return Err(problems);
        }

        let authors = parse_authors(self.authors.value());
        let slug = make_slug(&authors, year, &title);
        let id = make_id(&slug, &title);

        let mut doc = self.extracted.clone();
        doc.title = title;
        doc.authors = authors;
        doc.year = year;
        doc.document_type = self.document_type;
        doc.institution = {
            let inst = self.institution.value().trim();
            if inst.is_empty() {
                None
            } else {
                Some(inst.to_string())
            }
        };
        doc.slug = slug;
        doc.id = id;
        Ok(doc)
    }

    /// Advisory notes about fields that extraction commonly gets wrong.
    ///
    /// These never change the data — they only tell the reviewer where to look.
    /// The checks are:
    ///
    /// - **no authors** — the extractor never guesses authors from body text, so
    ///   an empty list is expected on scanned reports and needs a human (a
    ///   corporate author goes in one field, e.g. `Argonne Code Center`).
    /// - **no year** — nothing to cite by.
    /// - **year later than years found in the body text** — the classic scanned
    ///   report failure: the PDF `/CreationDate` is the *digitisation* date, so
    ///   a 1977 report reads as 2004.
    /// - **`Other` document type** — renders as a bare `@misc` BibTeX entry.
    /// - **title that looks like a bare report number** — usually the cover-page
    ///   identifier rather than the document's real title.
    pub fn advisories(&self) -> Vec<String> {
        let mut out = Vec::new();
        let doc = &self.extracted;

        if doc.authors.is_empty() {
            out.push(
                "authors: extraction found none (it never guesses from body text) — type them, \
                 e.g. a corporate author as 'Argonne Code Center'"
                    .to_string(),
            );
        }
        match doc.year {
            None => out.push("year: not recovered — check the title page".to_string()),
            Some(year) => {
                let found = years_in_text(&doc.markdown_body);
                let earlier: Vec<String> = found
                    .iter()
                    .filter(|y| **y < year)
                    .take(6)
                    .map(|y| y.to_string())
                    .collect();
                if !earlier.is_empty() {
                    out.push(format!(
                        "year {year} may be a digitisation/scan date — earlier years in the text: {}",
                        earlier.join(", ")
                    ));
                }
            }
        }
        if doc.document_type == DocumentType::Other {
            out.push(
                "type: 'Other' renders as BibTeX @misc — set Report/Paper/Benchmark if it is one"
                    .to_string(),
            );
        }
        if looks_like_report_number(&doc.title) {
            out.push(format!(
                "title '{}' looks like a report number, not a title",
                doc.title
            ));
        }
        out
    }

    /// Write the corrected record to whichever of the three output paths are
    /// non-empty, creating parent directories as needed.
    ///
    /// Never panics and never propagates an error: every outcome (including a
    /// validation failure or a per-file I/O error) is recorded in
    /// [`ReviewState::save_report`] for the UI to display. Returns `true` when at
    /// least one file was written.
    pub fn save(&mut self) -> bool {
        self.save_report.clear();
        let doc = match self.corrected_document() {
            Ok(doc) => doc,
            Err(problems) => {
                for p in problems {
                    self.save_report.push(format!("cannot save: {p}"));
                }
                return false;
            }
        };

        let mut wrote_any = false;
        let markdown_out = self.markdown_out.value().trim().to_string();
        let json_out = self.json_out.value().trim().to_string();
        let bibtex_out = self.bibtex_out.value().trim().to_string();

        if !markdown_out.is_empty() {
            wrote_any |= self.write_file(&markdown_out, &doc.markdown_body, "markdown");
        }
        if !json_out.is_empty() {
            match serde_json::to_string_pretty(&doc) {
                Ok(json) => wrote_any |= self.write_file(&json_out, &json, "json"),
                Err(e) => self
                    .save_report
                    .push(format!("json: serialise failed: {e}")),
            }
        }
        if !bibtex_out.is_empty() {
            let bib = kovan_literature::to_bibtex(&doc);
            wrote_any |= self.write_file(&bibtex_out, &bib, "bibtex");
        }
        if self.save_report.is_empty() {
            self.save_report
                .push("nothing to save — all three output paths are empty".to_string());
        }
        wrote_any
    }

    /// Write one artifact, recording success or the exact error. Returns whether
    /// the write succeeded.
    fn write_file(&mut self, path: &str, contents: &str, kind: &str) -> bool {
        let path = Path::new(path);
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    self.save_report
                        .push(format!("{kind}: mkdir {} failed: {e}", parent.display()));
                    return false;
                }
            }
        }
        match std::fs::write(path, contents) {
            Ok(()) => {
                self.save_report.push(format!(
                    "{kind}: wrote {} ({} bytes)",
                    path.display(),
                    contents.len()
                ));
                true
            }
            Err(e) => {
                self.save_report
                    .push(format!("{kind}: write {} failed: {e}", path.display()));
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in for the real 1977 Argonne report that motivated this screen:
    /// correct title, wrong (digitisation) year, no authors, `Other` type.
    fn misextracted_report() -> KovanDocument {
        let mut doc = KovanDocument::new(
            "kovan-0000000000000000",
            "2004anl7416",
            Visibility::Open,
            DocumentType::Other,
            "ANL-7416 Supplement 2",
        );
        doc.year = Some(2004);
        doc.page_count = Some(447);
        doc.markdown_body =
            "Argonne Code Center: Benchmark Problem Book\nJune 1977\nRevised 1977\n".to_string();
        doc
    }

    fn review() -> ReviewState {
        ReviewState::new(
            PathBuf::from("/tmp/anl-7416.pdf"),
            misextracted_report(),
            Duration::from_secs(42),
        )
    }

    #[test]
    fn form_starts_populated_from_the_extracted_record() {
        let state = review();
        assert_eq!(state.title.value(), "ANL-7416 Supplement 2");
        assert_eq!(state.authors.value(), "");
        assert_eq!(state.year.value(), "2004");
        assert_eq!(state.document_type, DocumentType::Other);
    }

    #[test]
    fn corrections_regenerate_the_slug_and_id() {
        let mut state = review();
        state.authors.set("Argonne Code Center");
        state.year.set("1977");
        let doc = state.corrected_document().expect("valid form");

        assert_eq!(doc.slug, "argonnecodecenter1977anl7416");
        assert_ne!(doc.slug, state.extracted.slug, "slug must not stay stale");
        assert_ne!(doc.id, state.extracted.id, "id is derived from the slug");
        assert_eq!(doc.year, Some(1977));
        assert_eq!(doc.authors.len(), 1, "corporate author is one author");
        assert_eq!(doc.authors[0].family, "Argonne Code Center");
        assert!(doc.authors[0].given.is_empty());
    }

    #[test]
    fn corrected_document_preserves_the_extracted_body_and_page_count() {
        let mut state = review();
        state.year.set("1977");
        let doc = state.corrected_document().expect("valid form");
        assert_eq!(doc.page_count, Some(447));
        assert_eq!(doc.markdown_body, state.extracted.markdown_body);
        assert_eq!(doc.visibility, Visibility::Open);
    }

    #[test]
    fn empty_title_is_a_validation_error_not_a_panic() {
        let mut state = review();
        state.title.set("   ");
        let problems = state.corrected_document().expect_err("must reject");
        assert!(problems.iter().any(|p| p.contains("title")));
    }

    #[test]
    fn implausible_year_is_rejected_with_a_readable_message() {
        let mut state = review();
        state.year.set("19777");
        let problems = state.corrected_document().expect_err("must reject");
        assert!(problems.iter().any(|p| p.contains("year")));
    }

    #[test]
    fn empty_year_means_unknown_rather_than_invalid() {
        let mut state = review();
        state.year.set("");
        let doc = state.corrected_document().expect("empty year is allowed");
        assert_eq!(doc.year, None);
    }

    #[test]
    fn advisories_flag_the_real_world_failure_modes() {
        let state = review();
        let notes = state.advisories();
        assert!(
            notes.iter().any(|n| n.starts_with("authors:")),
            "empty author list must be flagged: {notes:?}"
        );
        assert!(
            notes.iter().any(|n| n.contains("digitisation")),
            "2004 vs 1977 in the body must be flagged: {notes:?}"
        );
        assert!(
            notes.iter().any(|n| n.contains("@misc")),
            "Other document type must be flagged: {notes:?}"
        );
        assert!(
            notes.iter().any(|n| n.contains("report number")),
            "report-number title must be flagged: {notes:?}"
        );
    }

    #[test]
    fn a_clean_extraction_raises_no_advisories() {
        let mut doc = KovanDocument::new(
            "kovan-1",
            "doe2021steam",
            Visibility::Open,
            DocumentType::Paper,
            "A Study of Steam Table Accuracy in Reactor Analysis",
        );
        doc.year = Some(2021);
        doc.authors = parse_authors("Doe, Jane");
        doc.markdown_body = "Published 2021. Earlier work in this area.".to_string();
        let state = ReviewState::new(PathBuf::from("/tmp/x.pdf"), doc, Duration::from_secs(1));
        assert!(state.advisories().is_empty(), "{:?}", state.advisories());
    }

    #[test]
    fn output_defaults_follow_the_storage_layout_and_track_the_slug() {
        let mut state = review();
        state.authors.set("Argonne Code Center");
        state.year.set("1977");
        state.refresh_output_defaults();
        assert!(state
            .markdown_out
            .value()
            .contains("generated/markdown/open"));
        assert!(state.bibtex_out.value().contains("generated/bibtex/open"));
        assert!(
            state
                .markdown_out
                .value()
                .ends_with("argonnecodecenter1977anl7416.md"),
            "default path must follow the corrected slug: {}",
            state.markdown_out.value()
        );
    }

    #[test]
    fn pinned_output_paths_are_not_overwritten_by_slug_changes() {
        let mut state = review();
        state.outputs_pinned = true;
        state.markdown_out.set("/tmp/mine.md");
        state.year.set("1977");
        state.refresh_output_defaults();
        assert_eq!(state.markdown_out.value(), "/tmp/mine.md");
    }

    #[test]
    fn save_writes_all_three_artifacts_with_corrected_metadata() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut state = review();
        state.authors.set("Argonne Code Center");
        state.year.set("1977");
        state.document_type = DocumentType::Report;
        state.institution.set("Argonne National Laboratory");
        state.outputs_pinned = true;
        state
            .markdown_out
            .set(dir.path().join("out/doc.md").to_string_lossy().as_ref());
        state
            .json_out
            .set(dir.path().join("out/doc.json").to_string_lossy().as_ref());
        state
            .bibtex_out
            .set(dir.path().join("out/doc.bib").to_string_lossy().as_ref());

        assert!(
            state.save(),
            "save must report success: {:?}",
            state.save_report
        );

        let bib = std::fs::read_to_string(dir.path().join("out/doc.bib")).expect("bib written");
        assert!(
            bib.starts_with("@techreport{argonnecodecenter1977anl7416,"),
            "{bib}"
        );
        assert!(bib.contains("author = {Argonne Code Center}"), "{bib}");
        assert!(bib.contains("year = {1977}"), "{bib}");
        assert!(
            bib.contains("institution = {Argonne National Laboratory}"),
            "{bib}"
        );

        let json = std::fs::read_to_string(dir.path().join("out/doc.json")).expect("json written");
        let round: KovanDocument = serde_json::from_str(&json).expect("json parses back");
        assert_eq!(round.year, Some(1977));
        assert_eq!(round.slug, "argonnecodecenter1977anl7416");

        let md = std::fs::read_to_string(dir.path().join("out/doc.md")).expect("markdown written");
        assert_eq!(md, state.extracted.markdown_body);
    }

    #[test]
    fn save_reports_validation_failure_without_writing_anything() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("never.md");
        let mut state = review();
        state.title.set("");
        state.outputs_pinned = true;
        state.markdown_out.set(target.to_string_lossy().as_ref());
        state.json_out.clear();
        state.bibtex_out.clear();

        assert!(!state.save());
        assert!(
            !target.exists(),
            "nothing may be written when the form is invalid"
        );
        assert!(state.save_report.iter().any(|l| l.contains("cannot save")));
    }

    #[test]
    fn save_reports_an_unwritable_path_instead_of_panicking() {
        let mut state = review();
        state.outputs_pinned = true;
        state
            .markdown_out
            .set("/proc/definitely/not/writable/doc.md");
        state.json_out.clear();
        state.bibtex_out.clear();
        assert!(!state.save());
        assert!(
            state.save_report.iter().any(|l| l.contains("failed")),
            "{:?}",
            state.save_report
        );
    }

    #[test]
    fn empty_output_paths_save_nothing_and_say_so() {
        let mut state = review();
        state.outputs_pinned = true;
        state.markdown_out.clear();
        state.json_out.clear();
        state.bibtex_out.clear();
        assert!(!state.save());
        assert!(state
            .save_report
            .iter()
            .any(|l| l.contains("nothing to save")));
    }

    #[test]
    fn field_navigation_wraps_in_both_directions() {
        assert_eq!(ReviewField::Title.step(-1), ReviewField::BibtexOut);
        assert_eq!(ReviewField::BibtexOut.step(1), ReviewField::Title);
        assert!(!ReviewField::DocType.is_text());
        assert!(ReviewField::Title.is_text());
    }

    #[test]
    fn document_type_cycles_and_wraps() {
        assert_eq!(
            step_document_type(DocumentType::Paper, -1),
            DocumentType::Other
        );
        assert_eq!(
            step_document_type(DocumentType::Other, 1),
            DocumentType::Paper
        );
    }

    #[test]
    fn edited_marker_tracks_divergence_from_the_extraction() {
        let mut state = review();
        assert!(!state.is_edited(ReviewField::Year));
        state.year.set("1977");
        assert!(state.is_edited(ReviewField::Year));
        state.year.set("2004");
        assert!(
            !state.is_edited(ReviewField::Year),
            "restored value is not an edit"
        );
    }
}
