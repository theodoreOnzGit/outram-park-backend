//! Fine-grained classification — below paper level (§11, §15, §16,
//! `op-9vo6.14`).
//!
//! A paper's `kovan.toml` records *broad* classification (§7); this module
//! builds the artifacts that record classification of a **part** of a
//! paper — a page, a page range, a PDF rectangle, a note, a formula, a
//! digitised table/graph, or a plain source reference — using the schema
//! `crate::artifact` already defines (`op-9vo6.12`) and inserting through
//! `PaperSession::append_block` (§32's stale-buffer guard, `op-9vo6.10`).
//!
//! # The single artifact writer
//!
//! Everything that writes a `[kovan]` artifact into a paper's Markdown goes
//! through this module, so id disambiguation, `[kovan]`/`[source]`/
//! `[extraction]` construction and the §13 Markdown rendering live in one
//! place:
//!
//! - [`classify_selection`] / [`insert_artifact`] — from a PDF text/region
//!   selection, or the PDF reader's "Save page annotations" flow.
//! - [`save_digitised_csv`] — from either digitiser tab's "save into notes".
//! - [`replace_artifact_body`] — the page-context panel's inline block
//!   editor, and a re-digitise replacing its source block in place.
//!
//! The interactive triggers live in `crate::app` (the PDF reader's
//! annotate/crop canvas and the digitiser tabs); this module is UI-free.

use crate::artifact::{
    block_span, parse_document, render_artifact_block, Artifact, ArtifactKind, ArtifactMeta,
    ArtifactToml, Extraction, Region, SourceAnchor,
};
use crate::digitiser::dataset::utc_now_iso8601;
use crate::entity::Classification;
use crate::graph::artifact_node;
use crate::index::KnowledgeIndex;
use crate::relation::{delete_incident, RelationError};
use crate::research_record::ResearchRecordIndex;
use crate::root::KovanRoot;
use crate::session::{PaperSession, SessionError};

/// Errors building or inserting a fine-grained classification artifact.
#[derive(Debug)]
pub enum ClassifyError {
    /// `heading` produced no usable id (e.g. it was empty or entirely
    /// punctuation).
    NoUsableId,
    /// [`replace_artifact_body`] was asked for an id no artifact in the
    /// document has.
    UnknownId(String),
    /// The `[source]` anchor violates §15's invariants — see
    /// `SourceAnchor::validate`'s own error text.
    BadAnchor(String),
    /// Rendering the artifact to Markdown failed (TOML serialisation).
    Render(String),
    /// The artifact was written into the buffer but could not be read back
    /// out of it — the document's Markdown structure swallowed it.
    ///
    /// In practice this means an **unbalanced code fence**: a stray or
    /// unterminated ``` earlier in the file puts everything after it inside
    /// a code block, so the new artifact's fenced TOML is content rather
    /// than metadata. Found in the wild (GH issue #35, 2026-09-08) in a
    /// paper whose second digitiser dataset had been written with no
    /// heading and no opening fence, leaving a dangling closing fence at
    /// the end of the file.
    ///
    /// This used to be an `expect("just inserted")`, i.e. a panic that took
    /// the whole GUI down on a file the user could not have known was
    /// malformed. It is an error now so the caller can say so and carry on.
    NotReadableBack {
        /// The id that was written and could not be found again.
        id: String,
        /// How many ``` fences the document has, when that is odd — the
        /// actionable detail, since it names the actual defect.
        fences: usize,
    },
}

impl std::fmt::Display for ClassifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoUsableId => write!(f, "the heading has no usable characters for an id"),
            Self::UnknownId(id) => write!(f, "no artifact with id {id:?}"),
            Self::BadAnchor(msg) => write!(f, "{msg}"),
            Self::Render(msg) => write!(f, "{msg}"),
            Self::NotReadableBack { id, fences } => write!(
                f,
                "wrote artifact {id:?} but could not read it back: the document has \
                 {fences} ``` fences, an odd number, so an unterminated code block is \
                 swallowing it — close the stray fence and try again"
            ),
        }
    }
}

impl std::error::Error for ClassifyError {}

/// Errors from [`delete_artifact_cascade`].
#[derive(Debug)]
pub enum CascadeError {
    /// The artifact named is the paper's own header block
    /// ([`ArtifactKind::Paper`]), which is never deletable — removing it
    /// would take the document's identity, its BibTeX record and the
    /// heading every other artifact hangs beneath.
    CannotDeletePaper {
        /// The paper whose header was targeted.
        citekey: String,
    },
    /// No artifact with this id exists in `citekey`'s paper. Returned
    /// *before* anything is deleted — see the function docs.
    ArtifactNotFound { citekey: String, artifact_id: String },
    /// Opening/reading/saving a paper failed.
    Session(SessionError),
    /// Removing an incident relation failed.
    Relation(RelationError),
}

impl std::fmt::Display for CascadeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CannotDeletePaper { citekey } => write!(
                f,
                "{citekey:?}'s title block is the paper itself and is never deleted \
                 — delete the artifacts under it instead"
            ),
            Self::ArtifactNotFound {
                citekey,
                artifact_id,
            } => write!(f, "paper {citekey:?} has no artifact with id {artifact_id:?}"),
            Self::Session(e) => write!(f, "{e}"),
            Self::Relation(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for CascadeError {}

/// A lowercase-kebab-case slug from free text, e.g. "Coupled neutronics
/// methodology" -> `"coupled-neutronics-methodology"` — the §13/§40 style
/// every artifact example in the issue uses. Non-ASCII-alphanumeric runs
/// become a single `-`; leading/trailing `-` are trimmed.
pub(crate) fn slugify(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_was_dash = true; // suppresses a leading '-'
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            out.push('-');
            last_was_dash = true;
        }
    }
    if out.ends_with('-') {
        out.pop();
    }
    out
}

/// A slug from `heading`, disambiguated against `existing` ids by
/// appending `-2`, `-3`, … — §40: "avoid visible UUID soup where readable
/// stable ids suffice."
fn unique_id(heading: &str, existing: &ResearchRecordIndex) -> Result<String, ClassifyError> {
    let base = slugify(heading);
    if base.is_empty() {
        return Err(ClassifyError::NoUsableId);
    }
    if existing.get(&base).is_none() {
        return Ok(base);
    }
    for n in 2.. {
        let candidate = format!("{base}-{n}");
        if existing.get(&candidate).is_none() {
            return Ok(candidate);
        }
    }
    unreachable!("id space is unbounded")
}

/// The heading depth every inserted artifact gets — one level under the
/// paper's own `#` title (see the §13 examples).
const ARTIFACT_HEADING_LEVEL: u8 = crate::artifact::ARTIFACT_LEVEL;

/// Build and insert a fine-grained classification artifact into `session`'s
/// buffer (never straight to disk — call `session.save_document()`
/// afterwards to persist it, per §37's Save Document/Save Repository
/// split).
///
/// `heading` becomes both the artifact's display heading and, slugified,
/// its stable id (disambiguated against `index` if it collides). `anchor`
/// is validated against §15's invariants before anything is written.
///
/// This is [`insert_artifact`] with no `[extraction]` block — the shape a
/// note / annotation / source-reference takes.
pub fn classify_selection(
    session: &mut PaperSession,
    index: &ResearchRecordIndex,
    heading: &str,
    kind: ArtifactKind,
    anchor: SourceAnchor,
    classification: Classification,
    body: &str,
) -> Result<Artifact, ClassifyError> {
    insert_artifact(
        session,
        index,
        heading,
        kind,
        Some(anchor),
        classification,
        None,
        body,
    )
}

/// Build and append one fenced-TOML artifact (§13/§14) to `session`'s
/// buffer — the single writer every artifact-producing flow goes through
/// (text selection, PDF annotation save, digitiser CSV save), so id
/// disambiguation, `[kovan]`/`[source]`/`[extraction]` construction and the
/// §13 Markdown rendering live in exactly one place.
///
/// `anchor` (when `Some`) is validated against §15's invariants first.
/// `extraction` is set for `DigitisedTable`/`DigitisedGraph` and `None`
/// otherwise. Returns the artifact as re-parsed from the updated buffer, so
/// callers get its final `line`.
#[allow(clippy::too_many_arguments)]
pub fn insert_artifact(
    session: &mut PaperSession,
    index: &ResearchRecordIndex,
    heading: &str,
    kind: ArtifactKind,
    anchor: Option<SourceAnchor>,
    classification: Classification,
    extraction: Option<Extraction>,
    body: &str,
) -> Result<Artifact, ClassifyError> {
    if let Some(a) = &anchor {
        a.validate().map_err(ClassifyError::BadAnchor)?;
    }
    let id = unique_id(heading, index)?;
    let now = utc_now_iso8601();
    let before = session.markdown().to_string();

    let toml = ArtifactToml {
        kovan: ArtifactMeta {
            id: id.clone(),
            kind,
            created: now.clone(),
            modified: now,
            reviewed: None,
        },
        source: anchor,
        classification,
        extraction,
        relation: None,
            connections: Vec::new(),
    };
    let rendered = render_artifact_block(ARTIFACT_HEADING_LEVEL, heading, &toml, body)
        .map_err(ClassifyError::Render)?;
    insert_block_in_page_order(
        session,
        &rendered,
        toml.source.as_ref().and_then(|s| s.first_page()),
    );

    let refreshed = ResearchRecordIndex::from_session(session);
    match refreshed.get(&id) {
        Some(artifact) => Ok(artifact.clone()),
        // Not "impossible": see `ClassifyError::NotReadableBack`. Roll the
        // buffer back so a failed insert leaves the document as it was
        // rather than half-written.
        None => {
            session.set_markdown(before);
            Err(ClassifyError::NotReadableBack {
                id,
                fences: session.markdown().matches("```").count(),
            })
        }
    }
}

/// Insert a rendered artifact block so the document's page-anchored blocks
/// stay **in page order** — annotations save where they belong rather than
/// piling up at the end of the file (maintainer, 2026-09-02).
///
/// The block goes immediately before the first existing artifact anchored to
/// a *later* page, so same-page blocks keep their insertion order and
/// nothing already in the document is moved. With no page anchor, or no
/// later-page block to sit in front of, it appends as before.
fn insert_block_in_page_order(session: &mut PaperSession, rendered: &str, page: Option<u32>) {
    let Some(page) = page else {
        session.append_block(rendered);
        return;
    };
    let md = session.markdown().to_string();
    let parsed = parse_document(&md);
    let successor = parsed.artifacts.iter().find(|a| {
        a.toml
            .source
            .as_ref()
            .and_then(|s| s.first_page())
            .is_some_and(|p| p > page)
    });
    match successor {
        Some(a) => {
            let at = block_span(&md, a).start;
            session.set_markdown(insert_lines_before(&md, at, rendered));
        }
        None => session.append_block(rendered),
    }
}

/// Reorder a document's page-anchored artifact blocks into page order,
/// in place, leaving everything else exactly where it is (maintainer,
/// 2026-09-02: "if the annotations are disordered, order them when opening
/// them"). Returns whether anything moved.
///
/// Only the anchored blocks' *text* is permuted between their existing
/// spans, so the paper title, `## Summary`, any prose between blocks and any
/// un-anchored artifact all keep their position. The sort is stable, so
/// same-page blocks keep the order they were written in. Bails out (doing
/// nothing) if the blocks are already ordered, or if any two spans overlap —
/// a nested artifact is not something to shuffle blindly.
pub fn sort_artifacts_by_page(session: &mut PaperSession) -> bool {
    let md = session.markdown().to_string();
    let parsed = parse_document(&md);
    let anchored: Vec<&Artifact> = parsed
        .artifacts
        .iter()
        .filter(|a| {
            a.toml
                .source
                .as_ref()
                .and_then(|s| s.first_page())
                .is_some()
        })
        .collect();
    if anchored.len() < 2 {
        return false;
    }
    let spans: Vec<std::ops::Range<usize>> = anchored.iter().map(|a| block_span(&md, a)).collect();
    let pages: Vec<u32> = anchored
        .iter()
        .map(|a| {
            a.toml
                .source
                .as_ref()
                .and_then(|s| s.first_page())
                .unwrap_or(0)
        })
        .collect();
    if pages.windows(2).all(|w| w[0] <= w[1]) {
        return false; // already in order
    }
    if spans.windows(2).any(|w| w[0].end > w[1].start) {
        return false; // nested/overlapping blocks — leave well alone
    }

    let lines: Vec<&str> = md.lines().collect();
    let blocks: Vec<String> = spans.iter().map(|r| lines[r.clone()].join("\n")).collect();
    let mut order: Vec<usize> = (0..spans.len()).collect();
    order.sort_by_key(|&i| pages[i]); // stable

    let mut out = String::new();
    let mut line = 0usize;
    let mut slot = 0usize;
    while line < lines.len() {
        if slot < spans.len() && line == spans[slot].start {
            out.push_str(&blocks[order[slot]]);
            out.push('\n');
            line = spans[slot].end;
            slot += 1;
            continue;
        }
        out.push_str(lines[line]);
        out.push('\n');
        line += 1;
    }
    session.set_markdown(out);
    true
}

/// Splice `block` into `md` immediately before 0-based line `at`, separated
/// by a blank line, keeping every other line verbatim.
fn insert_lines_before(md: &str, at: usize, block: &str) -> String {
    let lines: Vec<&str> = md.lines().collect();
    let at = at.min(lines.len());
    let mut out = String::new();
    for line in &lines[..at] {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str(block.trim_end());
    out.push_str("\n\n");
    for line in lines.iter().skip(at) {
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Replace the **body** (everything after the metadata fence) of the
/// artifact with stable id `id` in `session`'s buffer, re-rendering its
/// block so the `[kovan]` TOML stays exactly as `parse_document` expects,
/// bumping `modified`, and leaving every other line of the document
/// untouched. The mechanism the page-context panel's inline block editor
/// uses so a schema-sensitive block is never hand-edited as raw text.
///
/// # Errors
///
/// [`ClassifyError::NoUsableId`] if no artifact has that id;
/// [`ClassifyError::Render`] if re-serialising its (unchanged) metadata
/// fails.
pub fn replace_artifact_body(
    session: &mut PaperSession,
    id: &str,
    new_body: &str,
) -> Result<Artifact, ClassifyError> {
    let md = session.markdown().to_string();
    let parsed = parse_document(&md);
    let artifact = parsed
        .get(id)
        .ok_or_else(|| ClassifyError::UnknownId(id.to_string()))?;

    let mut toml = artifact.toml.clone();
    toml.kovan.modified = utc_now_iso8601();
    let rendered = render_artifact_block(artifact.level, &artifact.heading, &toml, new_body)
        .map_err(ClassifyError::Render)?;

    let span = block_span(&md, artifact);
    session.set_markdown(splice_lines(&md, span, &rendered));

    let refreshed = ResearchRecordIndex::from_session(session);
    Ok(refreshed.get(id).expect("just replaced").clone())
}

/// Save a digitised table/graph's CSV into `session`'s buffer as a
/// `[kovan]` artifact — the single path both digitiser tabs' "save into
/// notes" goes through (GH issue #35 2026-09-02: digitised blocks become
/// real fenced-TOML artifacts so the page-context panel can re-open them).
///
/// When `replace_id` names an existing artifact — a *re-digitise* of a
/// block the panel double-click re-cropped — only its body is swapped
/// ([`replace_artifact_body`]), keeping the original `[source]`/`[extraction]`
/// and not appending a duplicate. Otherwise a new artifact is inserted with
/// `kind`, `anchor`, and an `[extraction]` block
/// (`method = "manual_digitisation"`).
///
/// `csv_body` is the fenced block verbatim, e.g. ```` "```csv\nx,y\n1,2\n```\n" ````.
pub fn save_digitised_csv(
    session: &mut PaperSession,
    kind: ArtifactKind,
    heading: &str,
    anchor: Option<SourceAnchor>,
    extraction: Option<Extraction>,
    replace_id: Option<&str>,
    csv_body: &str,
) -> Result<Artifact, ClassifyError> {
    if let Some(id) = replace_id {
        if parse_document(session.markdown()).get(id).is_some() {
            return replace_artifact_body(session, id, csv_body);
        }
    }
    let index = ResearchRecordIndex::from_session(session);
    insert_artifact(
        session,
        &index,
        heading,
        kind,
        anchor,
        Classification::default(),
        extraction.or_else(|| Some(Extraction::new("manual_digitisation", None))),
        csv_body,
    )
}

/// Replace lines `range` (0-based, end-exclusive) of `md` with
/// `replacement`, keeping every other line verbatim and the document
/// newline-terminated.
///
/// `pub(crate)`, not private: [`crate::relation`]'s connection CRUD splices a
/// re-rendered artifact block back into a paper's Markdown exactly the way
/// [`replace_artifact_body`] does, and reuses this rather than a second
/// hand-rolled line-splice (the workspace's "search before building" rule).
pub(crate) fn splice_lines(md: &str, range: std::ops::Range<usize>, replacement: &str) -> String {
    let lines: Vec<&str> = md.lines().collect();
    let mut out = String::new();
    for line in &lines[..range.start.min(lines.len())] {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str(replacement.trim_end());
    out.push('\n');
    for line in lines.iter().skip(range.end.min(lines.len())) {
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Remove lines `range` (0-based, end-exclusive) from `md` outright — no
/// replacement text, and no stray blank line left in the gap (unlike
/// [`splice_lines`] with an empty `replacement`, which would still emit one
/// blank line). Used by [`delete_artifact_cascade`] to remove a whole
/// artifact block.
fn remove_lines(md: &str, range: std::ops::Range<usize>) -> String {
    let lines: Vec<&str> = md.lines().collect();
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        if i >= range.start && i < range.end {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Delete artifact `artifact_id` from paper `citekey`, **and** every
/// [`crate::relation::UserRelation`] incident to it (as either its source or
/// its target), as one operation this function owns end to end (op-30um.2).
///
/// # Design: the confirm dialog only decides whether to call this
///
/// The layer-1 prototype's requirement is that the egui "Delete
/// annotation..." confirmation ("Sure anot? [No] [Yes]") must not itself
/// walk the graph, delete edges, or leave a half-applied state — it may only
/// decide whether this function is called at all. No: nothing runs, nothing
/// changes. Yes: this function runs exactly once and owns every step.
///
/// # What "one transaction" means here, and its real limit
///
/// **Checked before anything is written:** the artifact must actually exist
/// in `citekey`'s paper, or this returns
/// [`CascadeError::ArtifactNotFound`] having touched no file at all — a
/// failed precondition can never leave a partial mutation behind.
///
/// **Ordered once writing starts:** incident relations are removed first
/// ([`crate::relation::delete_incident`]), the artifact's own block second.
/// If the process is interrupted between the two, the surviving state is
/// "artifact still present, no relations pointing at it" rather than
/// "relations dangling at a node that no longer exists" — the safer of the
/// two half-finished states, since a leftover artifact is merely undeleted
/// (re-run the operation) while a dangling relation is a silent broken
/// reference nothing else in this crate currently detects.
///
/// **This is NOT a cross-file ACID transaction.** A relation incident to
/// this artifact may be recorded inside a *different* paper's Markdown file
/// than the artifact itself (see `crate::relation`'s module docs), and plain
/// file writes with no journal cannot be rolled back automatically if the
/// process dies mid-sequence. The operation is idempotent, though: a
/// repeated call after a partial failure finds fewer (or zero) incident
/// relations left to remove and, once the artifact itself is gone, returns
/// [`CascadeError::ArtifactNotFound`] cleanly rather than corrupting
/// anything further. This limit is inherent to storing authored data in
/// plain tracked files rather than a database — it is documented here
/// rather than papered over with an unearned "atomic" claim.
///
/// # Errors
///
/// [`CascadeError::ArtifactNotFound`] if `citekey`'s paper has no artifact
/// `artifact_id`; [`CascadeError::Session`] if the paper cannot be opened or
/// saved; [`CascadeError::Relation`] if removing an incident relation fails.
///
/// # Returns
///
/// The number of incident relations removed alongside the artifact.
pub fn delete_artifact_cascade(
    root: &KovanRoot,
    index: &KnowledgeIndex,
    citekey: &str,
    artifact_id: &str,
) -> Result<usize, CascadeError> {
    // Preconditions, checked before any write: the artifact must exist, and
    // it must not be the paper's own header.
    let probe = PaperSession::open(root, citekey).map_err(CascadeError::Session)?;
    let doc = parse_document(probe.markdown());
    let artifact = doc
        .get(artifact_id)
        .ok_or_else(|| CascadeError::ArtifactNotFound {
            citekey: citekey.to_string(),
            artifact_id: artifact_id.to_string(),
        })?;
    // "however it will never delete the main title" — the paper header
    // carries the document's identity and its BibTeX record, and every
    // other artifact in the file sits under it.
    if artifact.kind() == ArtifactKind::Paper {
        return Err(CascadeError::CannotDeletePaper {
            citekey: citekey.to_string(),
        });
    }
    drop(probe);

    let node = artifact_node(citekey, artifact_id);
    let removed_relations = delete_incident(root, index, &node).map_err(CascadeError::Relation)?;

    // Re-open: `delete_incident` may just have rewritten this very paper's
    // file, if this artifact had any outgoing relation of its own.
    let mut session = PaperSession::open(root, citekey).map_err(CascadeError::Session)?;
    let md = session.markdown().to_string();
    let artifact = parse_document(&md)
        .artifacts
        .into_iter()
        .find(|a| a.id() == artifact_id)
        .ok_or_else(|| CascadeError::ArtifactNotFound {
            citekey: citekey.to_string(),
            artifact_id: artifact_id.to_string(),
        })?;
    let span = block_span(&md, &artifact);
    session.set_markdown(remove_lines(&md, span));
    session.save_document().map_err(CascadeError::Session)?;

    Ok(removed_relations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{Access, CiteKey, EntityConfig};
    use crate::root::{KovanRoot, RootConfig};

    fn open_session() -> (tempfile::TempDir, PaperSession) {
        let dir = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(dir.path(), RootConfig::new("lib", "Lib"), false).unwrap();
        let citekey = "wang2018multiphysics";
        EntityConfig::paper(CiteKey::parse(citekey).unwrap(), Access::Restricted)
            .with_topics(["htgrs"])
            .save_paper(&root.paper_dir(citekey))
            .unwrap();
        (dir, PaperSession::open(&root, citekey).unwrap())
    }

    // -------------------------------------------------------------------
    // Legacy digitiser-section migration (GH issue #35, 2026-09-08). The
    // fixture is the real shape found in the maintainer's own library —
    // `papers/shin2021geant4dna/shin2021geant4dna.md`, saved 2026-09-02 by
    // the graph digitiser's no-active-paper path — which is why its
    // digitised graph never drew a box on the PDF canvas.
    // -------------------------------------------------------------------

    const LEGACY_GRAPH: &str = "\
# shin2021geant4dna

## Summary

### Fig 1. — page 3, pixel bbox [38.6, 71.9, 1215.4, 797.4], 2026-09-02T02:31:04Z, unnamed

```csv
# kovan digitiser dataset (schema v1)
# figure: Fig 1.
x,y
0.0014512785392496895,5.142650829867108
```
";

    #[test]
    fn a_stale_session_buffer_would_resurrect_a_cascaded_artifact_unless_reloaded() {
        // The delete bug behind GH issue #35's "sure anot box cannot":
        // `delete_artifact_cascade` works on its own session read from disk,
        // so a GUI holding an open `PaperSession` keeps the deleted artifact
        // in its buffer. Saving that buffer afterwards writes it straight
        // back. `PaperSession::reload` is what closes the gap.
        let (dir, mut session) = open_session();
        let root = KovanRoot::open(dir.path()).unwrap();
        let citekey = session.citekey().to_string();

        let index = ResearchRecordIndex::from_session(&session);
        let artifact = insert_artifact(
            &mut session,
            &index,
            "Doomed note",
            ArtifactKind::Annotation,
            Some(SourceAnchor {
                page: Some(1),
                pages: None,
                region: None,
            }),
            Classification::default(),
            None,
            "body",
        )
        .unwrap();
        let id = artifact.id().to_string();
        session.save_document().unwrap();

        let k_index = KnowledgeIndex::rebuild(&root);
        assert_eq!(
            delete_artifact_cascade(&root, &k_index, &citekey, &id).unwrap(),
            0
        );

        // On disk it is gone...
        let on_disk = std::fs::read_to_string(root.paper_markdown(&citekey)).unwrap();
        assert!(parse_document(&on_disk).get(&id).is_none());

        // ...but the still-open session has not noticed, which is exactly
        // what made the canvas keep drawing the box.
        assert!(parse_document(session.markdown()).get(&id).is_some());

        session.reload().unwrap();
        assert!(parse_document(session.markdown()).get(&id).is_none());
        assert!(!session.is_dirty());
    }

    #[test]
    fn an_unbalanced_code_fence_reports_instead_of_panicking() {
        // The GUI crash found while dogfooding on 2026-09-08: the
        // maintainer's shin2021geant4dna.md had a second digitiser dataset
        // written with no heading and no opening fence, leaving a dangling
        // ``` at the end of the file. Everything appended after it lands
        // inside that unterminated code block, so `parse_document` cannot
        // see the artifact just written — which used to hit
        // `expect("just inserted")` and take the whole window down.
        let (_dir, mut session) = open_session();
        session.set_markdown("# paper\n\nsome notes\n\n```\n");
        let before = session.markdown().to_string();

        let index = ResearchRecordIndex::from_session(&session);
        let err = insert_artifact(
            &mut session,
            &index,
            "Fig 1.",
            ArtifactKind::DigitisedGraph,
            None,
            Classification::default(),
            None,
            "```csv\nx,y\n1,2\n```",
        )
        .unwrap_err();

        match err {
            ClassifyError::NotReadableBack { ref id, fences } => {
                assert_eq!(id, "fig-1");
                assert_eq!(fences % 2, 1, "an odd fence count is the whole diagnosis");
            }
            other => panic!("wrong error: {other}"),
        }
        assert!(err.to_string().contains("unterminated code block"));
        // And the failed insert left the document exactly as it was.
        assert_eq!(session.markdown(), before);
    }

    #[test]
    fn find_legacy_csv_sections_reads_the_real_world_graph_section() {
        let found = find_legacy_csv_sections(LEGACY_GRAPH);
        assert_eq!(found.len(), 1);
        let s = &found[0];
        assert_eq!(s.heading, "Fig 1.");
        assert_eq!(s.kind, ArtifactKind::DigitisedGraph);
        assert_eq!(s.page, Some(3));
        assert_eq!(s.bbox, Some([38.6, 71.9, 1215.4, 797.4]));
        assert!(s.csv.starts_with("```csv"));
        assert!(s.csv.trim_end().ends_with("```"));
    }

    #[test]
    fn find_legacy_csv_sections_reads_the_table_digitisers_fixed_heading() {
        let md = "### Digitised table — page 2, pixel bbox [1.0, 2.0, 3.0, 4.0], t, a\n\n```csv\nx,y\n1,2\n```\n";
        let found = find_legacy_csv_sections(md);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, ArtifactKind::DigitisedTable);
        assert_eq!(found[0].page, Some(2));
    }

    #[test]
    fn find_legacy_csv_sections_ignores_a_hand_written_csv_section() {
        // No provenance tail => somebody's own table, not a legacy save.
        let md = "### My own numbers\n\n```csv\nx,y\n1,2\n```\n";
        assert!(find_legacy_csv_sections(md).is_empty());
    }

    #[test]
    fn migrating_a_legacy_graph_section_gives_it_a_page_and_a_region() {
        let (_dir, mut session) = open_session();
        session.set_markdown(LEGACY_GRAPH);

        // Before: parse_document sees no artifact at all — the exact reason
        // the canvas drew nothing.
        assert!(parse_document(session.markdown()).artifacts.is_empty());

        let n = migrate_legacy_csv_sections(&mut session, [1240.0, 1754.0]).unwrap();
        assert_eq!(n, 1);

        let doc = parse_document(session.markdown());
        assert_eq!(doc.artifacts.len(), 1);
        let art = &doc.artifacts[0];
        assert_eq!(art.kind(), ArtifactKind::DigitisedGraph);
        let source = art.toml.source.as_ref().expect("page anchor");
        assert_eq!(source.page, Some(3));
        let region = source.region.expect("region normalised from the bbox");
        assert!(region.is_valid());
        // 38.6/1240 and 797.4/1754, to the resolution the heading recorded.
        assert!((region.x0 - 0.031_129).abs() < 1e-4, "x0 = {}", region.x0);
        assert!((region.y1 - 0.454_617).abs() < 1e-4, "y1 = {}", region.y1);
        // The CSV body survives verbatim.
        assert!(art.body.contains("0.0014512785392496895,5.142650829867108"));
        // And the legacy heading is gone.
        assert!(!session.markdown().contains("pixel bbox"));
    }

    #[test]
    fn migrating_a_document_with_no_legacy_sections_changes_nothing() {
        let (_dir, mut session) = open_session();
        let before = session.markdown().to_string();
        assert_eq!(migrate_legacy_csv_sections(&mut session, [100.0, 100.0]).unwrap(), 0);
        assert_eq!(session.markdown(), before);
    }

    #[test]
    fn migration_is_idempotent() {
        let (_dir, mut session) = open_session();
        session.set_markdown(LEGACY_GRAPH);
        assert_eq!(migrate_legacy_csv_sections(&mut session, [1240.0, 1754.0]).unwrap(), 1);
        let after_first = session.markdown().to_string();
        assert_eq!(migrate_legacy_csv_sections(&mut session, [1240.0, 1754.0]).unwrap(), 0);
        assert_eq!(session.markdown(), after_first);
    }

    #[test]
    fn a_legacy_section_with_an_unusable_bbox_still_migrates_with_its_page() {
        let (_dir, mut session) = open_session();
        // bbox wider than the page => cannot normalise to a valid region.
        session.set_markdown(
            "### Fig 9. — page 1, pixel bbox [0.0, 0.0, 99999.0, 99999.0], t, a\n\n```csv\nx,y\n1,2\n```\n",
        );
        assert_eq!(migrate_legacy_csv_sections(&mut session, [100.0, 100.0]).unwrap(), 1);
        let doc = parse_document(session.markdown());
        let source = doc.artifacts[0].toml.source.as_ref().unwrap();
        assert_eq!(source.page, Some(1));
        assert!(source.region.is_none());
    }

    #[test]
    fn slugify_matches_the_issues_own_examples() {
        assert_eq!(
            slugify("Coupled neutronics methodology"),
            "coupled-neutronics-methodology"
        );
        assert_eq!(
            slugify("Table 4.4 — Core component materials"),
            "table-4-4-core-component-materials"
        );
        assert_eq!(slugify("   ---   "), "");
    }

    #[test]
    fn classify_selection_inserts_a_page_range_source_reference() {
        let (_dir, mut session) = open_session();
        let index = ResearchRecordIndex::from_session(&session);

        let anchor = SourceAnchor {
            page: None,
            pages: Some([42, 48]),
            region: None,
        };
        let classification = Classification {
            topics: vec!["htgrs/neutronics".to_string()],
            projects: vec![],
        };
        let artifact = classify_selection(
            &mut session,
            &index,
            "Coupled neutronics methodology",
            ArtifactKind::SourceReference,
            anchor,
            classification,
            "",
        )
        .unwrap();

        assert_eq!(artifact.id(), "coupled-neutronics-methodology");
        assert!(session.is_dirty());

        let refreshed = ResearchRecordIndex::from_session(&session);
        // The paper header artifact plus the one just inserted.
        assert_eq!(refreshed.artifacts().len(), 2);
        assert!(refreshed.anchored_to_page(45).len() == 1);
    }

    #[test]
    fn a_second_selection_with_the_same_heading_gets_a_disambiguated_id() {
        let (_dir, mut session) = open_session();
        let anchor = || SourceAnchor {
            page: Some(3),
            pages: None,
            region: None,
        };

        let index = ResearchRecordIndex::from_session(&session);
        classify_selection(
            &mut session,
            &index,
            "Note",
            ArtifactKind::Annotation,
            anchor(),
            Classification::default(),
            "",
        )
        .unwrap();

        let index = ResearchRecordIndex::from_session(&session);
        let second = classify_selection(
            &mut session,
            &index,
            "Note",
            ArtifactKind::Annotation,
            anchor(),
            Classification::default(),
            "",
        )
        .unwrap();

        assert_eq!(second.id(), "note-2");
    }

    #[test]
    fn an_invalid_anchor_is_rejected_before_anything_is_written() {
        let (_dir, mut session) = open_session();
        let index = ResearchRecordIndex::from_session(&session);
        let original = session.markdown().to_string();

        let bad_anchor = SourceAnchor {
            page: None,
            pages: None,
            region: None,
        }; // neither page nor pages
        let err = classify_selection(
            &mut session,
            &index,
            "Bad",
            ArtifactKind::Note,
            bad_anchor,
            Classification::default(),
            "",
        )
        .unwrap_err();

        assert!(matches!(err, ClassifyError::BadAnchor(_)));
        assert_eq!(
            session.markdown(),
            original,
            "a rejected anchor must not touch the buffer"
        );
        assert!(!session.is_dirty());
    }

    #[test]
    fn insert_artifact_records_an_extraction_block_for_a_digitised_graph() {
        let (_dir, mut session) = open_session();
        let index = ResearchRecordIndex::from_session(&session);
        let anchor = SourceAnchor {
            page: Some(7),
            pages: None,
            region: Some(crate::artifact::Region::from([0.1, 0.2, 0.6, 0.7])),
        };
        let art = insert_artifact(
            &mut session,
            &index,
            "Figure 4 — decay heat",
            ArtifactKind::DigitisedGraph,
            Some(anchor),
            Classification::default(),
            Some(Extraction::new("manual_digitisation", None)),
            "```csv\nx,y\n1,2\n```",
        )
        .unwrap();

        assert_eq!(art.kind(), ArtifactKind::DigitisedGraph);
        let reparsed = ResearchRecordIndex::from_session(&session);
        let a = reparsed.get(art.id()).unwrap();
        assert_eq!(
            a.toml.extraction.as_ref().unwrap().method,
            "manual_digitisation"
        );
        assert_eq!(a.toml.source.as_ref().unwrap().region.unwrap().x1, 0.6);
        assert!(a.csv_block().is_some());
    }

    #[test]
    fn replace_artifact_body_swaps_only_the_body_and_bumps_modified() {
        let (_dir, mut session) = open_session();
        let index = ResearchRecordIndex::from_session(&session);
        let art = classify_selection(
            &mut session,
            &index,
            "Graphite temperature assumption",
            ArtifactKind::Annotation,
            SourceAnchor {
                page: Some(87),
                pages: None,
                region: None,
            },
            Classification::default(),
            "first draft of the note",
        )
        .unwrap();
        let created = art.toml.kovan.created.clone();
        let before_lines = session.markdown().lines().count();

        let updated =
            replace_artifact_body(&mut session, art.id(), "a corrected note about it").unwrap();

        assert_eq!(updated.toml.kovan.created, created, "created is stable");
        // `modified` is re-stamped with `utc_now_iso8601()` by construction;
        // its 1-second resolution makes an in-test time delta unreliable, so
        // that is not asserted here.
        let md = session.markdown();
        assert!(md.contains("a corrected note about it"));
        assert!(!md.contains("first draft of the note"));
        assert!(
            md.contains(&format!("id = \"{}\"", art.id())),
            "the [kovan] block survives"
        );
        assert_eq!(
            session.markdown().lines().count(),
            before_lines,
            "no lines added/removed"
        );
    }

    /// Maintainer, 2026-09-02: "when saving annotations, i want them auto
    /// organised by page number. Not saved one after another."
    #[test]
    fn artifacts_are_inserted_in_page_order_not_appended() {
        let (_dir, mut session) = open_session();
        let insert = |session: &mut PaperSession, name: &str, page: u32| {
            let index = ResearchRecordIndex::from_session(session);
            insert_artifact(
                session,
                &index,
                name,
                ArtifactKind::Annotation,
                Some(SourceAnchor {
                    page: Some(page),
                    pages: None,
                    region: None,
                }),
                Classification::default(),
                None,
                "body",
            )
            .unwrap();
        };
        // Saved out of order, as an operator wandering the PDF would.
        insert(&mut session, "Note E", 5);
        insert(&mut session, "Note A", 1);
        insert(&mut session, "Note C", 3);
        insert(&mut session, "Note A2", 1);

        let pages: Vec<u32> = ResearchRecordIndex::from_session(&session)
            .artifacts()
            .iter()
            .filter_map(|a| a.toml.source.as_ref().and_then(|s| s.first_page()))
            .collect();
        assert_eq!(pages, vec![1, 1, 3, 5], "document order follows page order");

        // Same-page blocks keep the order they were saved in.
        let headings: Vec<String> = ResearchRecordIndex::from_session(&session)
            .artifacts()
            .iter()
            .map(|a| a.heading.clone())
            .collect();
        // The paper header sorts first (it carries no page anchor), then
        // the page-ordered notes.
        assert_eq!(
            headings,
            vec!["wang2018multiphysics", "Note A", "Note A2", "Note C", "Note E"]
        );
    }

    /// Maintainer, 2026-09-02: "if the annotations are disordered, order
    /// them when opening them."
    #[test]
    fn sort_artifacts_by_page_tidies_a_disordered_document_and_keeps_the_rest() {
        let (_dir, mut session) = open_session();
        // Build a deliberately out-of-order document by hand, with a
        // non-artifact section in the middle that must not move.
        let md = format!(
            "# Paper\n\n{}\n## Summary\n\nprose that must stay put.\n\n{}\n{}\n",
            block("note-e", 5, "Note E"),
            block("note-a", 1, "Note A"),
            block("note-c", 3, "Note C"),
        );
        session.set_markdown(md);

        assert!(sort_artifacts_by_page(&mut session));
        let out = session.markdown().to_string();
        let pages: Vec<u32> = ResearchRecordIndex::from_session(&session)
            .artifacts()
            .iter()
            .filter_map(|a| a.toml.source.as_ref().and_then(|s| s.first_page()))
            .collect();
        assert_eq!(pages, vec![1, 3, 5], "{out}");
        assert!(out.contains("prose that must stay put."), "{out}");
        assert!(out.contains("## Summary"), "{out}");
        // Idempotent.
        assert!(!sort_artifacts_by_page(&mut session));
    }

    fn block(id: &str, page: u32, heading: &str) -> String {
        format!(
            "# {heading}\n\n```toml\n[kovan]\nid = \"{id}\"\nkind = \"annotation\"\ncreated = \"c\"\nmodified = \"m\"\n\n[source]\npage = {page}\n```\n\nbody of {heading}\n"
        )
    }

    #[test]
    fn replace_artifact_body_rejects_an_unknown_id() {
        let (_dir, mut session) = open_session();
        let err = replace_artifact_body(&mut session, "does-not-exist", "x").unwrap_err();
        assert!(matches!(err, ClassifyError::UnknownId(_)));
    }

    #[test]
    fn save_digitised_csv_inserts_then_a_re_digitise_replaces_in_place() {
        let (_dir, mut session) = open_session();
        let anchor = SourceAnchor {
            page: Some(4),
            pages: None,
            region: Some(crate::artifact::Region::from([0.1, 0.1, 0.5, 0.5])),
        };
        let first = save_digitised_csv(
            &mut session,
            ArtifactKind::DigitisedGraph,
            "Figure 2",
            Some(anchor),
            Some(Extraction::new("manual_digitisation", Some("kopitiam-ocr".into()))),
            None,
            "```csv\nx,y\n1,10\n```\n",
        )
        .unwrap();
        // The paper header artifact plus the digitised graph.
        assert_eq!(
            ResearchRecordIndex::from_session(&session)
                .artifacts()
                .len(),
            2
        );

        // Re-digitise: same id, new numbers — one artifact, updated body,
        // `[source]` region preserved.
        let again = save_digitised_csv(
            &mut session,
            ArtifactKind::DigitisedGraph,
            "ignored on replace",
            None,
            None,
            Some(first.id()),
            "```csv\nx,y\n1,11\n2,22\n```\n",
        )
        .unwrap();
        assert_eq!(again.id(), first.id());
        let idx = ResearchRecordIndex::from_session(&session);
        // Header + the one digitised artifact: replaced in place, not
        // appended as a second copy.
        assert_eq!(idx.artifacts().len(), 2, "replace, not append");
        let a = idx.get(first.id()).unwrap();
        assert!(a.body.contains("2,22"));
        assert_eq!(
            a.toml.source.as_ref().unwrap().page,
            Some(4),
            "[source] survives a re-digitise"
        );
    }

    // -----------------------------------------------------------------
    // delete_artifact_cascade (op-30um.2)
    // -----------------------------------------------------------------

    /// Two papers, each with one artifact, plus one relation between them —
    /// enough to exercise a cascade delete that must remove both an
    /// artifact and an incident relation living in an *other* paper's file.
    fn make_cascade_fixture() -> (tempfile::TempDir, KovanRoot, KnowledgeIndex) {
        let dir = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(dir.path(), RootConfig::new("lib", "Lib"), false).unwrap();
        for citekey in ["src", "dst"] {
            EntityConfig::paper(CiteKey::parse(citekey).unwrap(), Access::Open)
                .save_paper(&root.paper_dir(citekey))
                .unwrap();
        }
        for (citekey, heading, body) in [("src", "Note A", "body a"), ("dst", "Note B", "body b")] {
            let mut session = PaperSession::open(&root, citekey).unwrap();
            let index = ResearchRecordIndex::from_session(&session);
            insert_artifact(
                &mut session,
                &index,
                heading,
                ArtifactKind::Note,
                None,
                Classification::default(),
                None,
                body,
            )
            .unwrap();
            session.save_document().unwrap();
        }
        let index = KnowledgeIndex::rebuild(&root);
        (dir, root, index)
    }

    #[test]
    fn cascade_removes_the_artifact_and_its_own_outgoing_relation() {
        let (_dir, root, index) = make_cascade_fixture();
        let source = artifact_node("src", "note-a");
        let target = artifact_node("dst", "note-b");
        crate::relation::add_connection(&root, &source, &target, crate::relation::RelationKind::Supports)
            .unwrap();

        let removed = delete_artifact_cascade(&root, &index, "src", "note-a").unwrap();
        assert_eq!(removed, 1);

        let text = std::fs::read_to_string(root.paper_markdown("src")).unwrap();
        assert!(parse_document(&text).get("note-a").is_none());
        assert!(crate::relation::connections(&root, &index, &target).is_empty());
    }

    #[test]
    fn cascade_removes_an_incoming_relation_recorded_in_another_papers_file() {
        let (_dir, root, index) = make_cascade_fixture();
        let source = artifact_node("dst", "note-b");
        let target = artifact_node("src", "note-a");
        // "dst"'s note-b relates TO "src"'s note-a: the relation record
        // lives inside dst's file, but we are about to delete note-a.
        crate::relation::add_connection(&root, &source, &target, crate::relation::RelationKind::Contradicts)
            .unwrap();

        let removed = delete_artifact_cascade(&root, &index, "src", "note-a").unwrap();
        assert_eq!(removed, 1, "the incoming relation in dst's file must also go");

        assert!(crate::relation::connections(&root, &index, &source).is_empty());
        let dst_text = std::fs::read_to_string(root.paper_markdown("dst")).unwrap();
        assert!(parse_document(&dst_text).get("note-b").is_some(), "dst's own artifact survives");
    }

    #[test]
    fn cascade_on_an_unknown_artifact_errors_and_touches_nothing() {
        let (_dir, root, index) = make_cascade_fixture();
        let before = std::fs::read_to_string(root.paper_markdown("src")).unwrap();

        let err = delete_artifact_cascade(&root, &index, "src", "no-such-id").unwrap_err();
        assert!(matches!(err, CascadeError::ArtifactNotFound { .. }));

        let after = std::fs::read_to_string(root.paper_markdown("src")).unwrap();
        assert_eq!(before, after, "a failed precondition must not touch the file");
    }

    #[test]
    fn cascade_with_no_incident_relations_just_removes_the_artifact() {
        let (_dir, root, index) = make_cascade_fixture();
        let removed = delete_artifact_cascade(&root, &index, "src", "note-a").unwrap();
        assert_eq!(removed, 0);
        let text = std::fs::read_to_string(root.paper_markdown("src")).unwrap();
        assert!(parse_document(&text).get("note-a").is_none());
    }

    #[test]
    fn a_cancelled_cascade_never_happens_the_ui_just_does_not_call_it() {
        // op-30um.2's actual requirement: the confirm dialog only decides
        // whether to invoke `delete_artifact_cascade` at all. There is
        // nothing to assert about "No" beyond "the function was never
        // called" — captured here as a compile-time/documentation fact
        // rather than a runtime one, since the No path is simply the
        // absence of a call.
        let (_dir, root, _index) = make_cascade_fixture();
        let text = std::fs::read_to_string(root.paper_markdown("src")).unwrap();
        assert!(parse_document(&text).get("note-a").is_some());
    }
}

/// Give `session`'s document its paper header artifact if it has none:
/// `# <citekey>` plus a `[kovan] kind = "paper"` TOML block, and the paper's
/// BibTeX record in a ```latex fence when `bibtex` is given.
///
/// The header is the document's first artifact, and the one
/// [`delete_artifact_cascade`] refuses to remove. Papers scaffolded before
/// GH issue #35 (2026-09-08) open with a bare `# <citekey>` title and no
/// TOML, so they parse as no artifact at all — this upgrades them in place,
/// keeping every existing line below the header untouched.
///
/// Returns `true` when it added the header, `false` when one was already
/// there. Idempotent, so it is safe to call on every open. Does not save —
/// the caller decides when to write.
pub fn ensure_paper_header(
    session: &mut PaperSession,
    bitex_source: Option<&str>,
) -> Result<bool, ClassifyError> {
    let citekey = session.citekey().to_string();
    let md = session.markdown().to_string();
    if parse_document(&md)
        .artifacts
        .iter()
        .any(|a| a.kind() == ArtifactKind::Paper)
    {
        return Ok(false);
    }

    let now = utc_now_iso8601();
    let toml = ArtifactToml {
        kovan: ArtifactMeta {
            id: citekey.clone(),
            kind: ArtifactKind::Paper,
            created: now.clone(),
            modified: now,
            reviewed: None,
        },
        source: None,
        classification: Classification::default(),
        extraction: None,
        connections: Vec::new(),
        relation: None,
    };
    let body = bitex_source
        .map(|b| crate::artifact::render_latex_body(b))
        .unwrap_or_default();
    let header =
        render_artifact_block(ARTIFACT_HEADING_LEVEL, &citekey, &toml, &body)
            .map_err(ClassifyError::Render)?;

    // Drop a pre-existing bare `# <citekey>` title line: the header artifact
    // replaces it, and leaving both would give the document two level-1
    // headings for the same thing.
    let mut rest: Vec<&str> = md.lines().collect();
    if let Some(first) = rest.iter().position(|l| !l.trim().is_empty()) {
        if rest[first].trim() == format!("# {citekey}") {
            rest.drain(..=first);
        }
    }
    let tail = rest.join("\n");
    let mut next = header.trim_end().to_string();
    next.push_str("\n");
    if !tail.trim().is_empty() {
        next.push('\n');
        next.push_str(tail.trim_start_matches('\n'));
    }
    if !next.ends_with('\n') {
        next.push('\n');
    }
    session.set_markdown(next);
    Ok(true)
}

// ---------------------------------------------------------------------------
// Legacy digitiser-section migration (GH issue #35, 2026-09-08)
// ---------------------------------------------------------------------------

/// One **legacy** digitiser CSV section — the pre-artifact format the graph
/// and table digitisers wrote when no paper was active, as a plain Markdown
/// heading plus a bare ```csv fence and no `[kovan]` block at all:
///
/// ```text
/// ### Fig 1. — page 3, pixel bbox [38.6, 71.9, 1215.4, 797.4], 2026-09-02T02:31:04Z, unnamed
///
/// ```csv
/// …
/// ```
/// ```
///
/// Because such a section carries no fenced TOML, [`parse_document`] does not
/// see it as an artifact at all: it has no id, no kind and no `[source]`, so
/// the PDF canvas cannot draw a region box for it and the page-context panel
/// cannot list it. That is the whole reason a digitised graph or table saved
/// this way is invisible in the GUI while annotations show up fine.
///
/// The heading itself carries everything needed to rebuild a real artifact
/// except the page's pixel size, which the caller supplies — see
/// [`migrate_legacy_csv_sections`].
#[derive(Debug, Clone, PartialEq)]
pub struct LegacyCsvSection {
    /// The section title with the provenance tail stripped, e.g. `"Fig 1."`.
    /// Becomes the migrated artifact's heading.
    pub heading: String,
    /// [`ArtifactKind::DigitisedTable`] when the heading is the table
    /// digitiser's fixed `"Digitised table"`, otherwise
    /// [`ArtifactKind::DigitisedGraph`] — the two legacy writers are
    /// distinguishable only by that title, since neither recorded a kind.
    pub kind: ArtifactKind,
    /// 1-based source page from the heading, if it recorded one.
    pub page: Option<u32>,
    /// The crop rectangle in page pixels as `[min_x, min_y, max_x, max_y]`,
    /// if the heading recorded a `pixel bbox`.
    pub bbox: Option<[f32; 4]>,
    /// The CSV body, fence included, exactly as written.
    pub csv: String,
    /// The section's line span in the document, 0-based and end-exclusive —
    /// heading through closing fence.
    pub lines: std::ops::Range<usize>,
}

/// Every legacy digitiser CSV section in `md`, in document order.
///
/// Recognises a level-3 heading whose text either is `Digitised table` or is
/// followed by the ` — page N, pixel bbox [...], <timestamp>, <author>` tail
/// the legacy writers appended, and which is followed by a ```csv fence. A
/// heading with no CSV fence before the next heading is not a legacy section
/// and is skipped.
///
/// Pure: takes and returns owned data, touches no file, so the recogniser is
/// unit-testable without a session or a PDF.
pub fn find_legacy_csv_sections(md: &str) -> Vec<LegacyCsvSection> {
    let lines: Vec<&str> = md.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let Some(rest) = lines[i].strip_prefix("### ") else {
            i += 1;
            continue;
        };
        let (title, page, bbox) = split_legacy_heading(rest);
        // Find the CSV fence that belongs to this heading, before any
        // following heading.
        let mut j = i + 1;
        let mut fence_start = None;
        while j < lines.len() {
            let l = lines[j].trim_start();
            if l.starts_with('#') {
                break;
            }
            if l.starts_with("```csv") {
                fence_start = Some(j);
                break;
            }
            j += 1;
        }
        let Some(fence_start) = fence_start else {
            i += 1;
            continue;
        };
        // A legacy section must have carried provenance; a bare `### x` with
        // a CSV fence under it is somebody's hand-written table, not ours.
        if page.is_none() && bbox.is_none() && title != "Digitised table" {
            i = fence_start + 1;
            continue;
        }
        let mut end = fence_start + 1;
        while end < lines.len() && lines[end].trim_end() != "```" {
            end += 1;
        }
        if end >= lines.len() {
            i = fence_start + 1;
            continue;
        }
        end += 1; // include the closing fence
        let kind = if title == "Digitised table" {
            ArtifactKind::DigitisedTable
        } else {
            ArtifactKind::DigitisedGraph
        };
        out.push(LegacyCsvSection {
            heading: title,
            kind,
            page,
            bbox,
            csv: lines[fence_start..end].join("\n"),
            lines: i..end,
        });
        i = end;
    }
    out
}

/// Split a legacy `### ` heading's text into its title and the provenance
/// the legacy writers appended (`— page N, pixel bbox [x0, y0, x1, y1], …`).
/// A heading with no such tail yields the whole text as the title and no
/// page/bbox.
fn split_legacy_heading(rest: &str) -> (String, Option<u32>, Option<[f32; 4]>) {
    let Some((title, tail)) = rest.split_once(" — ") else {
        return (rest.trim().to_string(), None, None);
    };
    let page = tail
        .split_once("page ")
        .and_then(|(_, t)| t.split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|d| d.parse::<u32>().ok());
    let bbox = tail
        .split_once("pixel bbox [")
        .and_then(|(_, t)| t.split_once(']'))
        .and_then(|(inner, _)| {
            let v: Vec<f32> = inner
                .split(',')
                .filter_map(|n| n.trim().parse::<f32>().ok())
                .collect();
            (v.len() == 4).then(|| [v[0], v[1], v[2], v[3]])
        });
    (title.trim().to_string(), page, bbox)
}

/// Rewrite every legacy digitiser CSV section in `session`'s buffer as a real
/// fenced-TOML artifact, so it gains an id, a kind and a `[source]` anchor
/// and therefore draws on the PDF canvas like any other artifact.
///
/// `page_px` is the document's page size in pixels at the render DPI (what
/// `PageView::page_size_px` returns) — needed because the legacy heading
/// recorded the crop in raw pixels, and `[source] region` is normalised page
/// fractions. A section whose heading recorded no bbox, or whose bbox does
/// not normalise to a valid [`Region`], still migrates: it keeps its page
/// anchor and simply has no region, which is the honest representation of
/// what was recorded.
///
/// Returns how many sections were migrated. Idempotent: a document with no
/// legacy sections is left byte-identical and returns `Ok(0)`, so it is safe
/// to run on every paper.
///
/// Does not save the session — the caller decides when to write to disk.
pub fn migrate_legacy_csv_sections(
    session: &mut PaperSession,
    page_px: [f32; 2],
) -> Result<usize, ClassifyError> {
    let sections = find_legacy_csv_sections(session.markdown());
    if sections.is_empty() {
        return Ok(0);
    }
    // Remove the legacy blocks bottom-up so each range stays valid, then
    // re-insert as artifacts (which places them in page order).
    for section in sections.iter().rev() {
        let md = splice_lines(session.markdown(), section.lines.clone(), "");
        session.set_markdown(md);
    }
    let mut migrated = 0;
    for section in &sections {
        let anchor = section.page.map(|page| SourceAnchor {
            page: Some(page),
            pages: None,
            region: section.bbox.and_then(|b| {
                Region::from_pixels((b[0], b[1]), (b[2], b[3]), page_px[0], page_px[1])
            }),
        });
        let index = ResearchRecordIndex::from_session(session);
        insert_artifact(
            session,
            &index,
            &section.heading,
            section.kind,
            anchor,
            Classification::default(),
            Some(Extraction::new("manual_digitisation", None)),
            &section.csv,
        )?;
        migrated += 1;
    }
    Ok(migrated)
}
