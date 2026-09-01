//! `PaperSession` — the object that owns one open paper (§31, §43,
//! `op-9vo6.10`).
//!
//! Replaces the current ad-hoc per-panel state on `DigitiseApp`: today
//! `pdf_reader`, `markdown_editor`, `bibliography` and `table_digitiser`
//! are four independent sibling fields with no shared notion of "the paper
//! currently open" — each can point at a different file with nothing
//! keeping them in sync. `PaperSession` is the one thing a paper-centric
//! Research workspace (§25) opens: it owns the paper's identity and its
//! canonical Markdown buffer — the single authoritative in-memory copy
//! §32 requires.
//!
//! # Staged rollout — read before extending this struct
//!
//! Two things the Research workspace will eventually route through this
//! struct are deliberately absent rather than stubbed:
//!
//! - **A `kopitiam-pdf` reader instance** (§24, `op-9vo6.11`) — blocked on
//!   a `kopitiam-pdf` 0.3.2+ publish (kopitiam#96). Until then, opening a
//!   paper's PDF still goes through the existing
//!   `app::pdf_reader::PdfReaderState`.
//! - **A `kopitiam-neovim` buffer** (§26, `op-9vo6.17`) — `markdown` here
//!   is a plain `String` for now. §32's "the buffer is authoritative" rule
//!   already holds for that form; a later step may change the storage
//!   type without changing the rule.
//!
//! Wiring those two together live is `op-9vo6.18`'s job (`SyncController`),
//! not this one's.

use std::path::{Path, PathBuf};

use crate::root::KovanRoot;

/// Errors opening or saving a paper session.
#[derive(Debug)]
pub enum SessionError {
    /// No paper with this citekey exists in the library.
    NotFound { citekey: String },
    Io { path: PathBuf, source: std::io::Error },
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { citekey } => write!(f, "no paper with citekey {citekey:?} in this library"),
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
        }
    }
}

impl std::error::Error for SessionError {}

/// One open paper: its identity and its canonical research Markdown (§12),
/// held in memory as the single authoritative copy while it's open.
#[derive(Debug)]
pub struct PaperSession {
    citekey: String,
    markdown_path: PathBuf,
    /// §32: while a paper is open, this buffer is authoritative. Artifact
    /// insertions (`op-9vo6.14`) go through [`Self::append_block`], never
    /// straight to disk behind it.
    markdown: String,
    /// `true` once `markdown` has diverged from what [`Self::save_document`]
    /// last wrote — the "Document" half of §37's split (`op-9vo6.19`).
    dirty: bool,
}

impl PaperSession {
    /// Open the paper `citekey` from `root`, reading its canonical
    /// Markdown once into memory. Nothing else touches disk again until
    /// [`Self::save_document`].
    pub fn open(root: &KovanRoot, citekey: &str) -> Result<Self, SessionError> {
        let markdown_path = root.paper_markdown(citekey);
        if !markdown_path.is_file() {
            return Err(SessionError::NotFound { citekey: citekey.to_string() });
        }
        let markdown = std::fs::read_to_string(&markdown_path)
            .map_err(|source| SessionError::Io { path: markdown_path.clone(), source })?;
        Ok(Self { citekey: citekey.to_string(), markdown_path, markdown, dirty: false })
    }

    pub fn citekey(&self) -> &str {
        &self.citekey
    }

    pub fn markdown_path(&self) -> &Path {
        &self.markdown_path
    }

    /// The current buffer text — the authoritative copy per §32.
    pub fn markdown(&self) -> &str {
        &self.markdown
    }

    /// Whether the buffer has unsaved changes.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Replace the buffer text (what an editor calls on every edit, once
    /// `op-9vo6.17`'s adapter exists). Marks dirty only on an actual change.
    pub fn set_markdown(&mut self, text: impl Into<String>) {
        let text = text.into();
        if text != self.markdown {
            self.markdown = text;
            self.dirty = true;
        }
    }

    /// Append a fenced-TOML artifact block (a rendered `Artifact` — see
    /// `crate::artifact::render_artifact_block`) to the end of the buffer.
    /// §32's "insertions go through the buffer" guard: this is the one
    /// path `op-9vo6.14`'s classification flow writes through.
    pub fn append_block(&mut self, heading_and_block: &str) {
        let trimmed_len = self.markdown.trim_end().len();
        self.markdown.truncate(trimmed_len);
        if !self.markdown.is_empty() {
            self.markdown.push_str("\n\n");
        }
        self.markdown.push_str(heading_and_block.trim_end());
        self.markdown.push('\n');
        self.dirty = true;
    }

    /// §37's "Save Document": write the buffer to disk. Does not stage or
    /// commit anything — that is `op-9vo6.19`'s separate "Save Repository".
    pub fn save_document(&mut self) -> Result<(), SessionError> {
        std::fs::write(&self.markdown_path, &self.markdown)
            .map_err(|source| SessionError::Io { path: self.markdown_path.clone(), source })?;
        self.dirty = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{Access, CiteKey, EntityConfig};
    use crate::root::RootConfig;

    fn make_paper() -> (tempfile::TempDir, KovanRoot, String) {
        let dir = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(dir.path(), RootConfig::new("lib", "Lib"), false).unwrap();
        let citekey = "wang2018multiphysics";
        EntityConfig::paper(CiteKey::parse(citekey).unwrap(), Access::Restricted)
            .with_topics(["htgrs"])
            .save_paper(&root.paper_dir(citekey))
            .unwrap();
        (dir, root, citekey.to_string())
    }

    #[test]
    fn open_reads_the_existing_markdown_stub() {
        let (_dir, root, citekey) = make_paper();
        let session = PaperSession::open(&root, &citekey).unwrap();
        assert!(session.markdown().contains("## Summary"));
        assert!(!session.is_dirty());
    }

    #[test]
    fn open_on_an_unknown_citekey_is_not_found() {
        let (_dir, root, _) = make_paper();
        let err = PaperSession::open(&root, "nonexistent2020nobody").unwrap_err();
        assert!(matches!(err, SessionError::NotFound { .. }));
    }

    #[test]
    fn set_markdown_marks_dirty_only_on_an_actual_change() {
        let (_dir, root, citekey) = make_paper();
        let mut session = PaperSession::open(&root, &citekey).unwrap();
        let original = session.markdown().to_string();

        session.set_markdown(original.clone());
        assert!(!session.is_dirty(), "identical text must not mark dirty");

        session.set_markdown(format!("{original}\nA new line."));
        assert!(session.is_dirty());
    }

    #[test]
    fn save_document_persists_and_clears_dirty() {
        let (_dir, root, citekey) = make_paper();
        let mut session = PaperSession::open(&root, &citekey).unwrap();
        session.set_markdown("# Rewritten\n\n## Summary\n\nNew content.\n");
        session.save_document().unwrap();
        assert!(!session.is_dirty());

        let on_disk = std::fs::read_to_string(root.paper_markdown(&citekey)).unwrap();
        assert_eq!(on_disk, session.markdown());
    }

    #[test]
    fn append_block_adds_a_blank_line_separator() {
        let (_dir, root, citekey) = make_paper();
        let mut session = PaperSession::open(&root, &citekey).unwrap();
        session.append_block("## A note\n\n```toml\n[kovan]\nid = \"a-note\"\n```\n");
        assert!(session.markdown().contains("## Summary\n\n## A note"));
        assert!(session.is_dirty());
    }
}
