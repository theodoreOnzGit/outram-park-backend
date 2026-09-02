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
    block_span, parse_document, render_artifact_block, Artifact, ArtifactKind, ArtifactMeta, ArtifactToml,
    Extraction, SourceAnchor,
};
use crate::digitiser::dataset::utc_now_iso8601;
use crate::entity::Classification;
use crate::research_record::ResearchRecordIndex;
use crate::session::PaperSession;

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
}

impl std::fmt::Display for ClassifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoUsableId => write!(f, "the heading has no usable characters for an id"),
            Self::UnknownId(id) => write!(f, "no artifact with id {id:?}"),
            Self::BadAnchor(msg) => write!(f, "{msg}"),
            Self::Render(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for ClassifyError {}

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
const ARTIFACT_HEADING_LEVEL: u8 = 2;

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
    insert_artifact(session, index, heading, kind, Some(anchor), classification, None, body)
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

    let toml = ArtifactToml {
        kovan: ArtifactMeta { id: id.clone(), kind, created: now.clone(), modified: now, reviewed: None },
        source: anchor,
        classification,
        extraction,
    };
    let rendered = render_artifact_block(ARTIFACT_HEADING_LEVEL, heading, &toml, body)
        .map_err(ClassifyError::Render)?;
    insert_block_in_page_order(session, &rendered, toml.source.as_ref().and_then(|s| s.first_page()));

    let refreshed = ResearchRecordIndex::from_session(session);
    Ok(refreshed.get(&id).expect("just inserted").clone())
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
    let successor = parsed
        .artifacts
        .iter()
        .find(|a| a.toml.source.as_ref().and_then(|s| s.first_page()).is_some_and(|p| p > page));
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
        .filter(|a| a.toml.source.as_ref().and_then(|s| s.first_page()).is_some())
        .collect();
    if anchored.len() < 2 {
        return false;
    }
    let spans: Vec<std::ops::Range<usize>> = anchored.iter().map(|a| block_span(&md, a)).collect();
    let pages: Vec<u32> = anchored
        .iter()
        .map(|a| a.toml.source.as_ref().and_then(|s| s.first_page()).unwrap_or(0))
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
    let artifact = parsed.get(id).ok_or_else(|| ClassifyError::UnknownId(id.to_string()))?;

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
    engine: Option<String>,
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
        Some(Extraction { method: "manual_digitisation".to_string(), engine }),
        csv_body,
    )
}

/// Replace lines `range` (0-based, end-exclusive) of `md` with
/// `replacement`, keeping every other line verbatim and the document
/// newline-terminated.
fn splice_lines(md: &str, range: std::ops::Range<usize>, replacement: &str) -> String {
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

    #[test]
    fn slugify_matches_the_issues_own_examples() {
        assert_eq!(slugify("Coupled neutronics methodology"), "coupled-neutronics-methodology");
        assert_eq!(slugify("Table 4.4 — Core component materials"), "table-4-4-core-component-materials");
        assert_eq!(slugify("   ---   "), "");
    }

    #[test]
    fn classify_selection_inserts_a_page_range_source_reference() {
        let (_dir, mut session) = open_session();
        let index = ResearchRecordIndex::from_session(&session);

        let anchor = SourceAnchor { page: None, pages: Some([42, 48]), region: None };
        let classification = Classification { topics: vec!["htgrs/neutronics".to_string()], projects: vec![] };
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
        assert_eq!(refreshed.artifacts().len(), 1);
        assert!(refreshed.anchored_to_page(45).len() == 1);
    }

    #[test]
    fn a_second_selection_with_the_same_heading_gets_a_disambiguated_id() {
        let (_dir, mut session) = open_session();
        let anchor = || SourceAnchor { page: Some(3), pages: None, region: None };

        let index = ResearchRecordIndex::from_session(&session);
        classify_selection(&mut session, &index, "Note", ArtifactKind::Annotation, anchor(), Classification::default(), "").unwrap();

        let index = ResearchRecordIndex::from_session(&session);
        let second =
            classify_selection(&mut session, &index, "Note", ArtifactKind::Annotation, anchor(), Classification::default(), "").unwrap();

        assert_eq!(second.id(), "note-2");
    }

    #[test]
    fn an_invalid_anchor_is_rejected_before_anything_is_written() {
        let (_dir, mut session) = open_session();
        let index = ResearchRecordIndex::from_session(&session);
        let original = session.markdown().to_string();

        let bad_anchor = SourceAnchor { page: None, pages: None, region: None }; // neither page nor pages
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
        assert_eq!(session.markdown(), original, "a rejected anchor must not touch the buffer");
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
            Some(Extraction { method: "manual_digitisation".into(), engine: None }),
            "```csv\nx,y\n1,2\n```",
        )
        .unwrap();

        assert_eq!(art.kind(), ArtifactKind::DigitisedGraph);
        let reparsed = ResearchRecordIndex::from_session(&session);
        let a = reparsed.get(art.id()).unwrap();
        assert_eq!(a.toml.extraction.as_ref().unwrap().method, "manual_digitisation");
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
            SourceAnchor { page: Some(87), pages: None, region: None },
            Classification::default(),
            "first draft of the note",
        )
        .unwrap();
        let created = art.toml.kovan.created.clone();
        let before_lines = session.markdown().lines().count();

        let updated = replace_artifact_body(&mut session, art.id(), "a corrected note about it").unwrap();

        assert_eq!(updated.toml.kovan.created, created, "created is stable");
        // `modified` is re-stamped with `utc_now_iso8601()` by construction;
        // its 1-second resolution makes an in-test time delta unreliable, so
        // that is not asserted here.
        let md = session.markdown();
        assert!(md.contains("a corrected note about it"));
        assert!(!md.contains("first draft of the note"));
        assert!(md.contains(&format!("id = \"{}\"", art.id())), "the [kovan] block survives");
        assert_eq!(session.markdown().lines().count(), before_lines, "no lines added/removed");
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
                Some(SourceAnchor { page: Some(page), pages: None, region: None }),
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
        let headings: Vec<String> =
            ResearchRecordIndex::from_session(&session).artifacts().iter().map(|a| a.heading.clone()).collect();
        assert_eq!(headings, vec!["Note A", "Note A2", "Note C", "Note E"]);
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
            "## {heading}\n\n```toml\n[kovan]\nid = \"{id}\"\nkind = \"annotation\"\ncreated = \"c\"\nmodified = \"m\"\n\n[source]\npage = {page}\n```\n\nbody of {heading}\n"
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
            Some("kopitiam-ocr".into()),
            None,
            "```csv\nx,y\n1,10\n```\n",
        )
        .unwrap();
        assert_eq!(ResearchRecordIndex::from_session(&session).artifacts().len(), 1);

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
        assert_eq!(idx.artifacts().len(), 1, "replace, not append");
        let a = idx.get(first.id()).unwrap();
        assert!(a.body.contains("2,22"));
        assert_eq!(a.toml.source.as_ref().unwrap().page, Some(4), "[source] survives a re-digitise");
    }
}
