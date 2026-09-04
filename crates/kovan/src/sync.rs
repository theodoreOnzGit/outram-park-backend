//! `SyncController` — PDF ↔ Markdown synchronisation (§31, `op-9vo6.18`).
//!
//! §31: synchronisation belongs entirely to Kovan, not to `kopitiam-pdf` or
//! `kopitiam-neovim` — neither knows the other exists. This module is the
//! seam between them, and is deliberately **PDF-reader-agnostic**: it
//! speaks only in page numbers and [`Artifact`]s, never in any concrete
//! reader's own types.
//!
//! # Why this is reader-agnostic, and what that defers
//!
//! `op-9vo6.10`'s `PaperSession` does not yet own a `kopitiam-pdf` reader
//! instance — that integration is `op-9vo6.11`, still blocked on a
//! `kopitiam-pdf` 0.3.2+ publish (kopitiam#96). Keeping `SyncController`'s
//! contract to plain `u32` page numbers and [`Artifact`] references (never
//! a `PdfReaderState`/`kopitiam_pdf::mupdf::PdfDocument` type) means it can
//! be wired to *either* the legacy `app::pdf_reader`
//! (usable today) or the eventual reusable reader with no change to this
//! module — only to whatever glue code reads its outputs. **Live GUI
//! wiring of a real reader+editor pair through this controller is left for
//! that later step**, once `PaperSession` actually owns both sides; what
//! ships here is the synchronisation *logic*, tested directly against
//! [`ResearchRecordIndex`]/[`Artifact`].
//!
//! # The three policies
//!
//! - **Follow** ([`SyncController::follow_page`]): soft, contextual. When
//!   the PDF reader shows page N, find the artifacts anchored there.
//! - **Edit** ([`SyncController::set_editing_active`]/
//!   [`SyncController::allow_follow`]): while the user is actively typing,
//!   Follow must not yank anything out from under them. The caller
//!   (`kvim_editor`, once wired) sets this from `Mode::Insert`/recent
//!   keystrokes; `allow_follow` gates every Follow call on it.
//! - **Explicit jump** ([`SyncController::jump_to_source`]/
//!   [`SyncController::artifacts_at_editor_line`]): a deliberate
//!   click/invoke always synchronises immediately, regardless of the Edit
//!   guard — §46's "Bidirectional navigation" acceptance scenario.

use crate::artifact::{Artifact, Region};
use crate::research_record::ResearchRecordIndex;

/// Where an explicit jump from an artifact should take the PDF reader.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PdfJumpTarget {
    /// 1-based page to show.
    pub page: u32,
    /// A rectangle on that page to highlight/scroll to, if the anchor
    /// named one (§15's normalised page coordinates).
    pub region: Option<Region>,
}

/// The PDF↔Markdown synchronisation seam for one open paper.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SyncController {
    /// §31's Edit policy: `true` while the user is actively typing in the
    /// editor. The caller updates this every frame/keystroke; this
    /// controller never infers it on its own.
    editing_active: bool,
}

impl SyncController {
    pub fn new() -> Self {
        Self::default()
    }

    /// The caller's per-frame report of whether the editor is actively
    /// receiving keystrokes right now (§31's Edit policy).
    pub fn set_editing_active(&mut self, active: bool) {
        self.editing_active = active;
    }

    /// Whether a Follow sync (a PDF page change updating the editor/
    /// highlight) is currently allowed. `false` while the user is
    /// actively typing — an explicit jump ignores this entirely, by
    /// design: a deliberate click always wins.
    pub fn allow_follow(&self) -> bool {
        !self.editing_active
    }

    /// §31's Follow: the artifacts anchored to `page`, softly highlighted
    /// rather than forcibly jumped to. Returns nothing if
    /// [`Self::allow_follow`] is `false` — call sites should still call
    /// this every page change and let it self-gate, rather than checking
    /// `allow_follow` separately, so the guard cannot be forgotten at a
    /// call site.
    pub fn follow_page<'a>(&self, index: &'a ResearchRecordIndex, page: u32) -> Vec<&'a Artifact> {
        if !self.allow_follow() {
            return Vec::new();
        }
        index.anchored_to_page(page)
    }

    /// §31's Explicit jump, artifact → PDF: where to send the reader.
    /// `None` for a free-standing note with no `[source]` anchor.
    pub fn jump_to_source(artifact: &Artifact) -> Option<PdfJumpTarget> {
        let anchor = artifact.toml.source.as_ref()?;
        let page = anchor.first_page()?;
        Some(PdfJumpTarget {
            page,
            region: anchor.region,
        })
    }

    /// §31's Explicit jump, PDF → editor: every artifact whose heading is
    /// at or before `editor_line` and whose *next* artifact (if any)
    /// starts after it — i.e. "which artifact is the editor cursor
    /// currently inside", the counterpart query to clicking a PDF region
    /// and being taken to its artifact in the Markdown.
    pub fn artifact_at_editor_line<'a>(
        index: &'a ResearchRecordIndex,
        editor_line: usize,
    ) -> Option<&'a Artifact> {
        index
            .artifacts()
            .iter()
            .filter(|a| a.line <= editor_line + 1)
            .max_by_key(|a| a.line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{Access, CiteKey, EntityConfig};
    use crate::root::{KovanRoot, RootConfig};
    use crate::session::PaperSession;

    fn open_session_with_two_artifacts() -> (tempfile::TempDir, PaperSession) {
        let dir = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(dir.path(), RootConfig::new("lib", "Lib"), false).unwrap();
        let citekey = "wang2018multiphysics";
        EntityConfig::paper(CiteKey::parse(citekey).unwrap(), Access::Restricted)
            .with_topics(["htgrs"])
            .save_paper(&root.paper_dir(citekey))
            .unwrap();
        let mut session = PaperSession::open(&root, citekey).unwrap();
        session.append_block(
            "## First\n\n```toml\n[kovan]\nid = \"first\"\nkind = \"note\"\ncreated = \"c\"\nmodified = \"m\"\n\n\
             [source]\npage = 5\n```\n",
        );
        session.append_block(
            "## Second\n\n```toml\n[kovan]\nid = \"second\"\nkind = \"note\"\ncreated = \"c\"\nmodified = \"m\"\n\n\
             [source]\npage = 5\nregion = [0.1, 0.2, 0.3, 0.4]\n```\n",
        );
        (dir, session)
    }

    #[test]
    fn follow_page_returns_artifacts_anchored_there_when_allowed() {
        let (_dir, session) = open_session_with_two_artifacts();
        let index = ResearchRecordIndex::from_session(&session);
        let ctrl = SyncController::new();

        let found = ctrl.follow_page(&index, 5);
        assert_eq!(found.len(), 2);
        assert!(ctrl.follow_page(&index, 99).is_empty());
    }

    #[test]
    fn follow_page_is_suppressed_while_editing_is_active() {
        let (_dir, session) = open_session_with_two_artifacts();
        let index = ResearchRecordIndex::from_session(&session);
        let mut ctrl = SyncController::new();
        ctrl.set_editing_active(true);

        assert!(!ctrl.allow_follow());
        assert!(
            ctrl.follow_page(&index, 5).is_empty(),
            "Follow must not yank the editor while typing"
        );
    }

    #[test]
    fn jump_to_source_reads_page_and_region_from_the_anchor() {
        let (_dir, session) = open_session_with_two_artifacts();
        let index = ResearchRecordIndex::from_session(&session);

        let first = index.get("first").unwrap();
        let target = SyncController::jump_to_source(first).unwrap();
        assert_eq!(target.page, 5);
        assert!(target.region.is_none());

        let second = index.get("second").unwrap();
        let target = SyncController::jump_to_source(second).unwrap();
        assert_eq!(target.page, 5);
        assert!(target.region.is_some());
    }

    #[test]
    fn jump_to_source_is_none_for_a_note_with_no_anchor() {
        let (_dir, session) = {
            let dir = tempfile::tempdir().unwrap();
            let root = KovanRoot::create(dir.path(), RootConfig::new("lib", "Lib"), false).unwrap();
            EntityConfig::paper(
                CiteKey::parse("wang2018multiphysics").unwrap(),
                Access::Restricted,
            )
            .with_topics(["htgrs"])
            .save_paper(&root.paper_dir("wang2018multiphysics"))
            .unwrap();
            let mut s = PaperSession::open(&root, "wang2018multiphysics").unwrap();
            s.append_block("## Free note\n\n```toml\n[kovan]\nid = \"free\"\nkind = \"note\"\ncreated = \"c\"\nmodified = \"m\"\n```\n");
            (dir, s)
        };
        let index = ResearchRecordIndex::from_session(&session);
        assert!(SyncController::jump_to_source(index.get("free").unwrap()).is_none());
    }

    #[test]
    fn artifact_at_editor_line_finds_the_enclosing_artifact() {
        let (_dir, session) = open_session_with_two_artifacts();
        let index = ResearchRecordIndex::from_session(&session);

        let first_line = index.get("first").unwrap().line;
        let second_line = index.get("second").unwrap().line;
        assert!(second_line > first_line);

        // A cursor sitting inside the first artifact's body (between the
        // two headings) resolves to "first", not "second".
        let cursor_inside_first = first_line + 2;
        let found = SyncController::artifact_at_editor_line(&index, cursor_inside_first).unwrap();
        assert_eq!(found.id(), "first");

        let cursor_inside_second = second_line + 2;
        let found = SyncController::artifact_at_editor_line(&index, cursor_inside_second).unwrap();
        assert_eq!(found.id(), "second");
    }
}
