//! `ResearchRecordIndex` — the semantic index over one open paper's
//! canonical Markdown (§32, §43, `op-9vo6.13`).
//!
//! Always **derived from a [`PaperSession`]'s in-memory buffer**
//! ([`crate::artifact::parse_document`]), never from a separate disk read.
//! That is the stale-buffer guard §32 exists to enforce: if this index were
//! built from `std::fs::read_to_string`, it could silently disagree with
//! what an editor is showing — an unsaved edit invisible to it, or a
//! concurrently modified file wrongly trusted.
//!
//! This module does not re-solve artifact parsing — `crate::artifact`
//! already does that (`op-9vo6.12`). What this adds is the "always fresh
//! from the buffer, never from disk" contract, plus the lookups §31's PDF
//! synchronisation and §16's classification browsing actually need.

use crate::artifact::{parse_document, Artifact, ArtifactError};
use crate::session::PaperSession;

/// The semantic index of one open paper — every artifact its buffer
/// currently contains, plus any parse problems.
pub struct ResearchRecordIndex {
    citekey: String,
    artifacts: Vec<Artifact>,
    problems: Vec<ArtifactError>,
}

impl ResearchRecordIndex {
    /// Build fresh from `session`'s current buffer. The only constructor —
    /// there is deliberately no `from_path`/`from_disk`, so a stale index
    /// cannot be built by accident.
    pub fn from_session(session: &PaperSession) -> Self {
        let parsed = parse_document(session.markdown());
        Self {
            citekey: session.citekey().to_string(),
            artifacts: parsed.artifacts,
            problems: parsed.problems,
        }
    }

    /// Rebuild in place from `session`'s current buffer — call this after
    /// every edit that should be reflected (an artifact insertion, a
    /// manual edit once `op-9vo6.17`'s editor exists).
    pub fn refresh(&mut self, session: &PaperSession) {
        *self = Self::from_session(session);
    }

    pub fn citekey(&self) -> &str {
        &self.citekey
    }

    /// Every well-formed artifact, in document order.
    pub fn artifacts(&self) -> &[Artifact] {
        &self.artifacts
    }

    /// Artifacts whose `[kovan]` table failed to parse — reported, never
    /// fatal (see `crate::artifact`'s own "parsing is total" rule).
    pub fn problems(&self) -> &[ArtifactError] {
        &self.problems
    }

    /// Look an artifact up by its stable id.
    pub fn get(&self, id: &str) -> Option<&Artifact> {
        self.artifacts.iter().find(|a| a.id() == id)
    }

    /// Every artifact anchored to 1-based `page` — §31's "Follow" query:
    /// when the PDF reader shows page 87, these are what to highlight.
    pub fn anchored_to_page(&self, page: u32) -> Vec<&Artifact> {
        self.artifacts
            .iter()
            .filter(|a| a.toml.source.as_ref().is_some_and(|s| s.covers_page(page)))
            .collect()
    }
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
        let session = PaperSession::open(&root, citekey).unwrap();
        (dir, session)
    }

    #[test]
    fn a_fresh_paper_stub_has_no_artifacts() {
        let (_dir, session) = open_session();
        let index = ResearchRecordIndex::from_session(&session);
        assert!(index.artifacts().is_empty());
        assert!(index.problems().is_empty());
    }

    #[test]
    fn reflects_an_appended_artifact_without_touching_disk() {
        let (_dir, mut session) = open_session();
        session.append_block(
            "## Note\n\n```toml\n[kovan]\nid = \"note-1\"\nkind = \"note\"\ncreated = \"c\"\nmodified = \"m\"\n\n[source]\npage = 7\n```\n",
        );

        // Not saved to disk yet — the index must still see it, because it
        // is built from the buffer, not a re-read.
        let index = ResearchRecordIndex::from_session(&session);
        assert_eq!(index.artifacts().len(), 1);
        assert_eq!(index.get("note-1").unwrap().id(), "note-1");
        assert_eq!(index.anchored_to_page(7).len(), 1);
        assert!(index.anchored_to_page(8).is_empty());

        let on_disk = std::fs::read_to_string(session.markdown_path()).unwrap();
        assert!(
            !on_disk.contains("note-1"),
            "disk must be unaffected before save_document"
        );
    }

    #[test]
    fn refresh_picks_up_a_later_change() {
        let (_dir, mut session) = open_session();
        let mut index = ResearchRecordIndex::from_session(&session);
        assert!(index.artifacts().is_empty());

        session.append_block(
            "## Note\n\n```toml\n[kovan]\nid = \"note-1\"\nkind = \"note\"\ncreated = \"c\"\nmodified = \"m\"\n```\n",
        );
        index.refresh(&session);
        assert_eq!(index.artifacts().len(), 1);
    }
}
