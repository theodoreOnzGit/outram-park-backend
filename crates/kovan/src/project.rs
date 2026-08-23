//! The "kovan folder" project format — `kovan.toml` generation and
//! maintenance (op-b1y5), implementing the design in
//! `docs/kovan-folder-format.md` (op-63u0). See that document for the full
//! rationale; this module implements §3 (schema), §4.1 (section markers)
//! and §5 (regeneration algorithm) of it.
//!
//! ## What belongs here
//!
//! - [`ProjectIndex`]/[`DocumentEntry`]/[`SectionRanges`] — the `kovan.toml`
//!   schema, `serde`-derived for `toml` (de)serialisation.
//! - [`scan_markdown_sections`] — the marker scanner (design doc §5, steps
//!   1–3): finds `<!-- kovan:section NAME -->` lines and computes each
//!   section's inclusive 1-indexed line range.
//! - [`regenerate`]/[`write_index`]/[`regenerate_and_write`] — rescan a
//!   project folder and (re)write its `kovan.toml`, atomically.
//!
//! ## What does not belong here (design doc §3/§7 — read before extending)
//!
//! **`kovan.toml` is generated, never hand-authored, and never read back as
//! an input to anything but a locate-by-line-number lookup** — this module
//! must never grow a "merge my hand edits back in" path; every regeneration
//! fully replaces the file from a fresh scan (design doc §5 step 4),
//! deliberately, so a stale or hand-edited copy can never silently survive.
//!
//! **Known v1 scoping limitation, not yet closed:** the design doc says
//! `document.id` must equal the `.bib` entry's cite key. This module does
//! not parse `.bib` files (this workspace has no BibTeX *parser* today —
//! `kovan_literature::bibtex::to_bibtex` only *renders* a `KovanDocument`
//! into BibTeX, one-way). [`regenerate`] instead joins `pdf/<stem>.pdf` with
//! `markdown/<stem>.md` by shared filename stem and uses the stem as `id`.
//! This is an honest, working v1 — not the bib-key join the design doc
//! describes — and is flagged here deliberately rather than silently
//! diverging from that document. Closing the gap needs a BibTeX parser
//! somewhere in the workspace first (`kovan-literature` is the natural
//! home); tracked as a follow-up, not done in this change.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Current `kovan.toml` schema version (design doc §3).
pub const PROJECT_SCHEMA_VERSION: u32 = 1;

/// The five standard markdown sections, in their fixed order (design doc
/// §4.1) — every generated markdown file contains every marker in this
/// order, even when a section's body is empty.
pub const SECTION_ORDER: [&str; 5] = [
    "ai_summary",
    "author_summary",
    "full_text",
    "table_csvs",
    "graph_csvs",
];

/// Errors from scanning a project folder or reading/writing `kovan.toml`.
#[derive(Debug)]
pub enum ProjectError {
    /// An I/O failure reading/writing a file, with the path it happened on.
    Io { path: PathBuf, source: std::io::Error },
    /// `toml` (de)serialisation failed.
    Toml(String),
    /// A `<!-- kovan:section NAME -->` marker named something outside
    /// [`SECTION_ORDER`] — design doc §5 step 1: "an unknown name is a
    /// parse error, not a silently-ignored line."
    UnknownSection { markdown: PathBuf, name: String, line: usize },
    /// The same section marker appeared twice in one file.
    DuplicateSection { markdown: PathBuf, name: String },
    /// The project root has no `.bib` file, or more than one — design doc
    /// §1 says exactly one (the user-named main bibliography file).
    AmbiguousOrMissingBibFile { root: PathBuf, found: Vec<PathBuf> },
    /// [`write_section`]'s caller-supplied range no longer matches a fresh
    /// scan — the file changed on disk since it was read for editing.
    StaleSectionRange { markdown: PathBuf, name: String },
}

impl std::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Toml(m) => write!(f, "toml error: {m}"),
            Self::UnknownSection { markdown, name, line } => write!(
                f,
                "{}:{line}: unknown section marker {name:?} (expected one of {SECTION_ORDER:?})",
                markdown.display()
            ),
            Self::DuplicateSection { markdown, name } => {
                write!(f, "{}: duplicate section marker {name:?}", markdown.display())
            }
            Self::AmbiguousOrMissingBibFile { root, found } => {
                if found.is_empty() {
                    write!(f, "{}: no .bib file found (need exactly one)", root.display())
                } else {
                    write!(
                        f,
                        "{}: {} .bib files found, need exactly one: {found:?}",
                        root.display(),
                        found.len()
                    )
                }
            }
            Self::StaleSectionRange { markdown, name } => write!(
                f,
                "{}: section {name:?} changed on disk since it was opened for editing \
                 — reload and try again",
                markdown.display()
            ),
        }
    }
}

impl std::error::Error for ProjectError {}

/// One document's line-range pointer into its markdown file, per standard
/// section (design doc §3) — `None` for a section whose marker is absent
/// (no digitised tables/plots yet, an author summary not written yet, …),
/// never a fabricated `[0, 0]` (this workspace's data-honesty convention).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionRanges {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_summary: Option<[usize; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_summary: Option<[usize; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_text: Option<[usize; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table_csvs: Option<[usize; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph_csvs: Option<[usize; 2]>,
}

impl SectionRanges {
    fn set(&mut self, name: &str, range: [usize; 2]) {
        match name {
            "ai_summary" => self.ai_summary = Some(range),
            "author_summary" => self.author_summary = Some(range),
            "full_text" => self.full_text = Some(range),
            "table_csvs" => self.table_csvs = Some(range),
            "graph_csvs" => self.graph_csvs = Some(range),
            _ => unreachable!("caller already validated against SECTION_ORDER"),
        }
    }

    /// The range for `name` (one of [`SECTION_ORDER`]), if that section's
    /// marker is present. Used by [`write_section`] to check for a stale
    /// caller-held range before splicing.
    pub fn get(&self, name: &str) -> Option<[usize; 2]> {
        match name {
            "ai_summary" => self.ai_summary,
            "author_summary" => self.author_summary,
            "full_text" => self.full_text,
            "table_csvs" => self.table_csvs,
            "graph_csvs" => self.graph_csvs,
            _ => None,
        }
    }

    fn is_empty(&self) -> bool {
        self.ai_summary.is_none()
            && self.author_summary.is_none()
            && self.full_text.is_none()
            && self.table_csvs.is_none()
            && self.graph_csvs.is_none()
    }
}

/// One document's entry in `kovan.toml` (design doc §3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentEntry {
    /// Join key across the `.bib`/PDF/markdown files — see this module's
    /// doc comment for the v1 filename-stem scoping limitation.
    pub id: String,
    /// Path to the PDF, relative to `kovan.toml`'s own directory.
    pub pdf: String,
    /// Path to the markdown file, relative to `kovan.toml`'s own directory.
    pub markdown: String,
    /// Line-range pointers into `markdown`, one per standard section.
    #[serde(default, skip_serializing_if = "SectionRanges::is_empty")]
    pub sections: SectionRanges,
}

/// The `kovan.toml` index — see this module's doc comment: generated only,
/// never hand-authored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectIndex {
    pub schema_version: u32,
    /// The project's one bibliography file, relative to `kovan.toml`'s own
    /// directory (design doc §1: "a main bibliography file wherein the
    /// user can choose to name it whatever").
    pub bib_file: String,
    #[serde(rename = "document", default)]
    pub documents: Vec<DocumentEntry>,
}

/// Header comment written above the serialised TOML body — makes the
/// generated-not-authoritative rule (design doc §2) visible to anyone who
/// opens the file, the same way `docs/<crate>-api.md`'s own generated
/// mirrors say so.
const GENERATED_HEADER: &str = "\
# GENERATED FILE — do not edit by hand.
# Regenerated by `kovan` (kopi-beans op-b1y5) from the project's PDF/markdown
# files whenever a markdown file changes, or on explicit
# `kovan-cli project regen`. Edits made here are overwritten without warning.
# See crates/kovan/docs/kovan-folder-format.md for the schema.
";

/// Scan `markdown_text` for `<!-- kovan:section NAME -->` marker lines and
/// compute each present section's inclusive, 1-indexed line range: from the
/// marker line itself through the line before the next marker (or end of
/// file for the last section) — design doc §5 steps 1–3.
///
/// `markdown_path` is used only to attribute errors to a file; it need not
/// exist on disk (callers with an in-memory string, e.g. tests, can pass any
/// path).
pub fn scan_markdown_sections(
    markdown_path: &Path,
    markdown_text: &str,
) -> Result<SectionRanges, ProjectError> {
    let mut markers: Vec<(String, usize)> = Vec::new(); // (name, 1-indexed line)
    let mut seen: BTreeMap<&str, ()> = BTreeMap::new();
    let total_lines = markdown_text.lines().count();

    for (i, line) in markdown_text.lines().enumerate() {
        let line_no = i + 1;
        let Some(name) = parse_marker(line) else {
            continue;
        };
        let Some(&canonical) = SECTION_ORDER.iter().find(|s| **s == name) else {
            return Err(ProjectError::UnknownSection {
                markdown: markdown_path.to_path_buf(),
                name: name.to_string(),
                line: line_no,
            });
        };
        if seen.insert(canonical, ()).is_some() {
            return Err(ProjectError::DuplicateSection {
                markdown: markdown_path.to_path_buf(),
                name: canonical.to_string(),
            });
        }
        markers.push((canonical.to_string(), line_no));
    }

    let mut ranges = SectionRanges::default();
    for (idx, (name, start)) in markers.iter().enumerate() {
        let end = markers.get(idx + 1).map(|(_, l)| l - 1).unwrap_or(total_lines);
        ranges.set(name, [*start, end]);
    }
    Ok(ranges)
}

/// Parse a `<!-- kovan:section NAME -->` marker out of one line, returning
/// `NAME` if the line (trimmed) matches the exact marker shape.
fn parse_marker(line: &str) -> Option<&str> {
    let line = line.trim();
    let rest = line.strip_prefix("<!-- kovan:section ")?;
    let name = rest.strip_suffix(" -->")?;
    (!name.is_empty()).then_some(name)
}

/// Rescan `root` (a "kovan folder" — design doc §1: `kovan.toml`, one
/// `.bib` file, `pdf/`, `markdown/`) and build a fresh [`ProjectIndex`] —
/// design doc §5's regeneration algorithm, minus the write (see
/// [`write_index`]/[`regenerate_and_write`]).
///
/// Joins `pdf/<stem>.pdf` with `markdown/<stem>.md` by shared filename
/// stem (see this module's doc comment for why, not the design doc's
/// bib-key join). A PDF with no matching markdown file, or vice versa, is
/// silently skipped — not every PDF has been processed into markdown yet,
/// and that is a normal, expected state, not an error.
pub fn regenerate(root: &Path) -> Result<ProjectIndex, ProjectError> {
    let bib_file = find_bib_file(root)?;

    let pdf_dir = root.join("pdf");
    let markdown_dir = root.join("markdown");
    let pdf_stems = list_stems(&pdf_dir, "pdf")?;
    let markdown_stems = list_stems(&markdown_dir, "md")?;

    let mut documents = Vec::new();
    for stem in pdf_stems {
        if !markdown_stems.contains(&stem) {
            continue;
        }
        let markdown_path = markdown_dir.join(format!("{stem}.md"));
        let text = read_to_string(&markdown_path)?;
        let sections = scan_markdown_sections(&markdown_path, &text)?;
        documents.push(DocumentEntry {
            id: stem.clone(),
            pdf: format!("pdf/{stem}.pdf"),
            markdown: format!("markdown/{stem}.md"),
            sections,
        });
    }
    documents.sort_by(|a, b| a.id.cmp(&b.id));

    Ok(ProjectIndex {
        schema_version: PROJECT_SCHEMA_VERSION,
        bib_file,
        documents,
    })
}

/// Serialise `index` and write it to `root/kovan.toml` atomically (temp
/// file + rename — design doc §5 step 5), preceded by
/// `GENERATED_HEADER` (the "do not edit by hand" comment block).
pub fn write_index(root: &Path, index: &ProjectIndex) -> Result<(), ProjectError> {
    let body = toml::to_string_pretty(index).map_err(|e| ProjectError::Toml(e.to_string()))?;
    let full = format!("{GENERATED_HEADER}\n{body}");
    let final_path = root.join("kovan.toml");
    let tmp_path = root.join("kovan.toml.tmp");
    fs::write(&tmp_path, full).map_err(|e| io_err(&tmp_path, e))?;
    fs::rename(&tmp_path, &final_path).map_err(|e| io_err(&final_path, e))?;
    Ok(())
}

/// Convenience: [`regenerate`] then [`write_index`] — what `kovan-cli
/// project regen` and a future markdown-write-triggered call both run.
pub fn regenerate_and_write(root: &Path) -> Result<ProjectIndex, ProjectError> {
    let index = regenerate(root)?;
    write_index(root, &index)?;
    Ok(index)
}

/// One section's content, split into its read-only heading (design doc
/// §4.2: the marker line plus the heading line immediately following it —
/// structure, never shown as editable text) and its editable body (every
/// line after the heading through the end of the section's range).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionContent {
    /// The `<!-- kovan:section NAME -->` marker line, verbatim.
    pub marker_line: String,
    /// The line immediately after the marker (normally the `##` heading).
    pub heading_line: String,
    /// Everything from the line after `heading_line` through the end of the
    /// section's range — the part a GUI editor (op-wr08) may change.
    pub body: String,
}

/// Read one document's section content for editing (op-wr08's GUI editor
/// surface) — the marker+heading (read-only) and the body (editable), split
/// out of `markdown_path` at `range` (a `[start, end]` pair as recorded in
/// `kovan.toml`, 1-indexed inclusive, `start` = the marker's own line).
pub fn read_section(
    markdown_path: &Path,
    range: [usize; 2],
) -> Result<SectionContent, ProjectError> {
    let text = read_to_string(markdown_path)?;
    let lines: Vec<&str> = text.lines().collect();
    let [start, end] = range;
    if start == 0 || end < start || end > lines.len() {
        return Err(ProjectError::Toml(format!(
            "{}: section range {range:?} is out of bounds for a {}-line file",
            markdown_path.display(),
            lines.len()
        )));
    }
    let marker_line = lines[start - 1].to_string();
    let heading_line = lines.get(start).copied().unwrap_or("").to_string();
    // Body: lines after the heading (0-indexed `start + 1`) through `end`
    // (0-indexed exclusive, since `end` is already the 1-indexed inclusive
    // last line).
    let body_start = start + 1;
    let body = if body_start < end {
        lines[body_start..end].join("\n")
    } else {
        String::new()
    };
    Ok(SectionContent { marker_line, heading_line, body })
}

/// Write a new `body` back into `section_name` of `markdown_rel` (relative
/// to `root`), then regenerate `kovan.toml` (design doc §5: any markdown
/// write triggers regeneration).
///
/// `expected_range` must match the section's *current* range — re-scanned
/// fresh from disk before writing — or the write is rejected with
/// [`ProjectError::StaleSectionRange`] rather than silently overwriting
/// whatever is actually there now (design doc §4.2's conflict rule: the
/// file may have changed since the editor opened it, e.g. a fresh
/// digitisation appended a CSV subsection).
pub fn write_section(
    root: &Path,
    markdown_rel: &str,
    section_name: &str,
    expected_range: [usize; 2],
    new_body: &str,
) -> Result<ProjectIndex, ProjectError> {
    let markdown_path = root.join(markdown_rel);
    let text = read_to_string(&markdown_path)?;
    let current = scan_markdown_sections(&markdown_path, &text)?;
    if current.get(section_name) != Some(expected_range) {
        return Err(ProjectError::StaleSectionRange {
            markdown: markdown_path,
            name: section_name.to_string(),
        });
    }

    let lines: Vec<&str> = text.lines().collect();
    let [start, end] = expected_range;
    let heading_idx = start; // 0-indexed index of the heading line
    let mut spliced: Vec<&str> = Vec::with_capacity(lines.len());
    spliced.extend_from_slice(&lines[..=heading_idx]);
    let body_lines: Vec<&str> = new_body.lines().collect();
    spliced.extend_from_slice(&body_lines);
    spliced.extend_from_slice(&lines[end..]);
    let new_text = spliced.join("\n") + "\n";
    fs::write(&markdown_path, new_text).map_err(|e| io_err(&markdown_path, e))?;

    regenerate_and_write(root)
}

fn find_bib_file(root: &Path) -> Result<String, ProjectError> {
    let mut found = Vec::new();
    let entries = fs::read_dir(root).map_err(|e| io_err(root, e))?;
    for entry in entries {
        let entry = entry.map_err(|e| io_err(root, e))?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("bib") {
            found.push(path);
        }
    }
    if found.len() != 1 {
        return Err(ProjectError::AmbiguousOrMissingBibFile {
            root: root.to_path_buf(),
            found,
        });
    }
    let name = found[0]
        .file_name()
        .expect("read_dir entry always has a file name")
        .to_string_lossy()
        .into_owned();
    Ok(name)
}

/// File stems (without extension) of every `*.<ext>` file directly under
/// `dir`. Returns an empty set (not an error) when `dir` doesn't exist yet
/// — a fresh project may not have a `pdf/` or `markdown/` folder populated
/// yet, which is a normal state, not a failure.
fn list_stems(dir: &Path, ext: &str) -> Result<std::collections::BTreeSet<String>, ProjectError> {
    let mut stems = std::collections::BTreeSet::new();
    if !dir.exists() {
        return Ok(stems);
    }
    for entry in fs::read_dir(dir).map_err(|e| io_err(dir, e))? {
        let entry = entry.map_err(|e| io_err(dir, e))?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some(ext) {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                stems.insert(stem.to_string());
            }
        }
    }
    Ok(stems)
}

fn read_to_string(path: &Path) -> Result<String, ProjectError> {
    fs::read_to_string(path).map_err(|e| io_err(path, e))
}

fn io_err(path: &Path, source: std::io::Error) -> ProjectError {
    ProjectError::Io { path: path.to_path_buf(), source }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_all_five_sections_in_order() {
        let md = "\
line one before any marker is ignored
<!-- kovan:section ai_summary -->
## AI Summary
body
<!-- kovan:section author_summary -->
## Author's Summary
<!-- kovan:section full_text -->
## Full Text
lots
of
lines
<!-- kovan:section table_csvs -->
## Tables
<!-- kovan:section graph_csvs -->
## Graphs
last line";
        let ranges = scan_markdown_sections(Path::new("doc.md"), md).unwrap();
        assert_eq!(ranges.ai_summary, Some([2, 4]));
        assert_eq!(ranges.author_summary, Some([5, 6]));
        assert_eq!(ranges.full_text, Some([7, 11]));
        assert_eq!(ranges.table_csvs, Some([12, 13]));
        assert_eq!(ranges.graph_csvs, Some([14, 16]));
    }

    #[test]
    fn missing_sections_are_absent_not_zero() {
        let md = "<!-- kovan:section full_text -->\nsome text";
        let ranges = scan_markdown_sections(Path::new("doc.md"), md).unwrap();
        assert_eq!(ranges.full_text, Some([1, 2]));
        assert_eq!(ranges.ai_summary, None);
        assert_eq!(ranges.graph_csvs, None);
        assert!(!ranges.is_empty());
    }

    #[test]
    fn no_markers_at_all_is_not_an_error() {
        let ranges = scan_markdown_sections(Path::new("doc.md"), "just prose\nno markers").unwrap();
        assert!(ranges.is_empty());
    }

    #[test]
    fn unknown_marker_name_is_an_error() {
        let md = "<!-- kovan:section not_a_real_section -->\ntext";
        let err = scan_markdown_sections(Path::new("doc.md"), md).unwrap_err();
        assert!(matches!(err, ProjectError::UnknownSection { .. }), "{err}");
    }

    #[test]
    fn duplicate_marker_is_an_error() {
        let md = "<!-- kovan:section full_text -->\na\n<!-- kovan:section full_text -->\nb";
        let err = scan_markdown_sections(Path::new("doc.md"), md).unwrap_err();
        assert!(matches!(err, ProjectError::DuplicateSection { .. }), "{err}");
    }

    #[test]
    fn round_trips_through_toml() {
        let index = ProjectIndex {
            schema_version: PROJECT_SCHEMA_VERSION,
            bib_file: "my bibliography.bib".to_string(),
            documents: vec![DocumentEntry {
                id: "anl-7416-supplement-2".to_string(),
                pdf: "pdf/anl-7416-supplement-2.pdf".to_string(),
                markdown: "markdown/anl-7416-supplement-2.md".to_string(),
                sections: SectionRanges {
                    full_text: Some([33, 812]),
                    ..Default::default()
                },
            }],
        };
        let text = toml::to_string_pretty(&index).unwrap();
        let back: ProjectIndex = toml::from_str(&text).unwrap();
        assert_eq!(back, index);
    }

    /// Full end-to-end regenerate() + write_index() over a synthetic
    /// project folder, mirroring this crate's "synthetic self-consistency"
    /// testing style (e.g. the digitiser's `tests/digitiser_synthetic.rs`).
    #[test]
    fn regenerate_and_write_over_a_synthetic_project() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("pdf")).unwrap();
        fs::create_dir_all(root.join("markdown")).unwrap();
        fs::write(root.join("my bibliography.bib"), "@article{x, title={X}}").unwrap();

        fs::write(root.join("pdf/report-a.pdf"), b"%PDF-fake").unwrap();
        fs::write(
            root.join("markdown/report-a.md"),
            "<!-- kovan:section full_text -->\n## Full Text\nhello\n",
        )
        .unwrap();
        // A PDF with no markdown yet — must be skipped, not an error.
        fs::write(root.join("pdf/report-b.pdf"), b"%PDF-fake").unwrap();

        let index = regenerate_and_write(root).unwrap();
        assert_eq!(index.bib_file, "my bibliography.bib");
        assert_eq!(index.documents.len(), 1);
        assert_eq!(index.documents[0].id, "report-a");
        assert_eq!(index.documents[0].sections.full_text, Some([1, 3]));

        let written = fs::read_to_string(root.join("kovan.toml")).unwrap();
        assert!(written.starts_with("# GENERATED FILE"));
        let body: String = written
            .lines()
            .skip_while(|l| l.starts_with('#') || l.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        let reparsed: ProjectIndex = toml::from_str(&body).unwrap();
        assert_eq!(reparsed, index);
    }

    #[test]
    fn missing_bib_file_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let err = regenerate(dir.path()).unwrap_err();
        assert!(matches!(err, ProjectError::AmbiguousOrMissingBibFile { .. }), "{err}");
    }

    #[test]
    fn read_section_splits_marker_heading_and_body() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("doc.md");
        fs::write(
            &path,
            "<!-- kovan:section full_text -->\n## Full Text\nline a\nline b\n",
        )
        .unwrap();
        let content = read_section(&path, [1, 4]).unwrap();
        assert_eq!(content.marker_line, "<!-- kovan:section full_text -->");
        assert_eq!(content.heading_line, "## Full Text");
        assert_eq!(content.body, "line a\nline b");
    }

    #[test]
    fn read_section_with_no_body_lines_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("doc.md");
        fs::write(&path, "<!-- kovan:section ai_summary -->\n## AI Summary\n").unwrap();
        let content = read_section(&path, [1, 2]).unwrap();
        assert_eq!(content.body, "");
    }

    #[test]
    fn write_section_replaces_body_and_leaves_heading_and_later_sections_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("pdf")).unwrap();
        fs::create_dir_all(root.join("markdown")).unwrap();
        fs::write(root.join("x.bib"), "@article{x,}").unwrap();
        fs::write(root.join("pdf/doc.pdf"), b"%PDF-fake").unwrap();
        let markdown_path = root.join("markdown/doc.md");
        fs::write(
            &markdown_path,
            "<!-- kovan:section full_text -->\n\
             ## Full Text\n\
             old body\n\
             <!-- kovan:section graph_csvs -->\n\
             ## Graphs\n\
             untouched\n",
        )
        .unwrap();

        let index = write_section(root, "markdown/doc.md", "full_text", [1, 3], "new body\nline 2")
            .unwrap();

        let written = fs::read_to_string(&markdown_path).unwrap();
        assert_eq!(
            written,
            "<!-- kovan:section full_text -->\n\
             ## Full Text\n\
             new body\n\
             line 2\n\
             <!-- kovan:section graph_csvs -->\n\
             ## Graphs\n\
             untouched\n"
        );
        // kovan.toml was refreshed with the new (longer) range.
        let doc = &index.documents[0];
        assert_eq!(doc.sections.full_text, Some([1, 4]));
        assert_eq!(doc.sections.graph_csvs, Some([5, 7]));
    }

    #[test]
    fn write_section_rejects_a_stale_range() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("pdf")).unwrap();
        fs::create_dir_all(root.join("markdown")).unwrap();
        fs::write(root.join("x.bib"), "@article{x,}").unwrap();
        fs::write(root.join("pdf/doc.pdf"), b"%PDF-fake").unwrap();
        // On-disk range is [1, 3] already (an extra line was added by
        // someone/something else since the caller last read it).
        fs::write(
            root.join("markdown/doc.md"),
            "<!-- kovan:section full_text -->\n## Full Text\nalready longer\nthan expected\n",
        )
        .unwrap();

        let err = write_section(root, "markdown/doc.md", "full_text", [1, 3], "clobber")
            .unwrap_err();
        assert!(matches!(err, ProjectError::StaleSectionRange { .. }), "{err}");
        // And the file must be untouched.
        let still_there = fs::read_to_string(root.join("markdown/doc.md")).unwrap();
        assert!(still_there.contains("already longer"));
    }
}
