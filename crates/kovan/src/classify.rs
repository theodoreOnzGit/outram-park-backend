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
//! # What this pass does and does not build
//!
//! §14's own trigger is interactive: "From the PDF: select region/text/
//! pages -> right-click -> 'Classify selection...'". That trigger needs a
//! working PDF-region-selection surface, which does not exist yet — the
//! reusable `kopitiam-pdf` reader is `op-9vo6.11`, still blocked on a
//! 0.3.2+ publish (kopitiam#96), and building a throwaway selection UI
//! against the legacy `pdf_reader.rs` (already marked DELETE AFTER PARITY
//! in the migration map) would mean writing UI that gets thrown away
//! almost immediately.
//!
//! What *is* buildable now, and is what this module provides, is
//! everything downstream of "the user picked a region/page and a
//! classification": constructing a valid artifact, generating its id,
//! rendering it to the exact §13 Markdown shape, and appending it through
//! the session. Wiring an interactive trigger to it is `op-9vo6.11`'s
//! (PDF selection) and `op-9vo6.25`'s (Research workspace) job.

use crate::artifact::{render_artifact_block, Artifact, ArtifactKind, ArtifactMeta, ArtifactToml, SourceAnchor};
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
fn slugify(text: &str) -> String {
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

/// Build and insert a fine-grained classification artifact into `session`'s
/// buffer (never straight to disk — call `session.save_document()`
/// afterwards to persist it, per §37's Save Document/Save Repository
/// split).
///
/// `heading` becomes both the artifact's display heading and, slugified,
/// its stable id (disambiguated against `index` if it collides). `anchor`
/// is validated against §15's invariants before anything is written.
#[allow(clippy::too_many_arguments)]
pub fn classify_selection(
    session: &mut PaperSession,
    index: &ResearchRecordIndex,
    heading: &str,
    kind: ArtifactKind,
    anchor: SourceAnchor,
    classification: Classification,
    body: &str,
) -> Result<Artifact, ClassifyError> {
    anchor.validate().map_err(ClassifyError::BadAnchor)?;
    let id = unique_id(heading, index)?;
    let now = utc_now_iso8601();

    let toml = ArtifactToml {
        kovan: ArtifactMeta { id: id.clone(), kind, created: now.clone(), modified: now, reviewed: None },
        source: Some(anchor),
        classification,
        extraction: None,
    };
    let heading_level = 2; // one level under the paper's own `#` title — see the §13 examples.
    let rendered =
        render_artifact_block(heading_level, heading, &toml, body).map_err(ClassifyError::Render)?;
    session.append_block(&rendered);

    let refreshed = ResearchRecordIndex::from_session(session);
    Ok(refreshed.get(&id).expect("just inserted").clone())
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
}
