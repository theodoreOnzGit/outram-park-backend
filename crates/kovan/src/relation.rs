//! User-authored relations between research nodes (op-30um.1).
//!
//! ## What this is, and why it exists
//!
//! The layer-1 dogfood prototype (`collaboration/kovan-issue-35-prototypes/
//! layer1-connections/`) argues for a typed relation a *user* draws between
//! two graph nodes — "this note `Supports` that table", "this model
//! `VerifiedAgainst` that benchmark" — which is a different kind of edge
//! from anything [`crate::graph::KnowledgeGraph`] computes today
//! (`Classification`/`WikiLink`/`Cites` are all *derived* from what a paper's
//! Markdown already says elsewhere). The prototype's own `kovan_relations.py`
//! is explicit that its JSON shadow store under `.kovan/` is **not** the
//! proposed final format — it exists only to settle the CRUD shape before
//! this module was written.
//!
//! ## Where a relation is persisted, and why (the deliverable decision)
//!
//! **A [`UserRelation`] is authored knowledge, not derived state.** A human
//! decided the relationship; nothing recomputes it from the surrounding
//! Markdown the way an outlink or a classification edge is recomputed.
//! op-9vo6's north star holds for authored data exactly as it holds for a
//! citekey or a topic tag: `rm -rf .kovan` must never lose it, because
//! `.kovan/` is documented, everywhere else in this crate, as a disposable
//! cache (see `graph.rs`'s and `index.rs`'s own module docs). So a relation
//! cannot live *only* under `.kovan/` — that would just be the prototype's
//! shadow store promoted to production.
//!
//! **Decision: a relation is persisted as one `[[relation]]` table inside the
//! fenced TOML block of its own *source* artifact** —
//! [`crate::artifact::ArtifactToml::relation`], beside `source` /
//! `classification` / `extraction`. Only the `id`, `target` and `kind` are
//! recorded on disk ([`RelationRecord`]); the `source` half of the full
//! [`UserRelation`] triple is left implicit, because it is always exactly
//! the artifact whose block the record lives inside — recording it again
//! would be a second, driftable copy of a fact already given by the file
//! it's found in.
//!
//! This keeps a relation exactly as reviewable and mergeable as the rest of
//! a paper's Markdown (a real `git diff` on a real tracked file), and lets
//! [`crate::graph::KnowledgeGraph::rebuild`] pick user relations up the same
//! way it already picks up wiki links and citations — by parsing the tracked
//! files, never the other way around. That wiring is deliberately **not**
//! done in this bead (see "What this does NOT do yet" below), to keep this
//! change reviewable on its own.
//!
//! ### Rejected alternatives, and why
//!
//! - **A new relation file per paper or per root** (e.g.
//!   `relations.toml` beside `kovan.toml`). Rejected: it is a second place
//!   that has to stay in sync with an artifact rename/move, which an
//!   in-block record cannot drift from — the record and the thing it
//!   describes are always edited, moved and reviewed together.
//! - **A new [`crate::graph::EdgeKind`] variant computed the same way as
//!   `Classification`/`WikiLink`/`Cites`.** Rejected: `KnowledgeGraph` is
//!   explicitly a *derived, disposable* cache under `.kovan/graph/` — folding
//!   `RelationKind` straight into `EdgeKind` would still need a tracked,
//!   human-readable store underneath it to survive `rm -rf .kovan`, which is
//!   exactly what this module already is. Adding the variant would rename the
//!   problem rather than solve it. It remains the right *read-side* shape for
//!   later: once wired, `KnowledgeGraph::rebuild` should fold each
//!   [`UserRelation`] in as an `EdgeKind::UserRelation` edge alongside the
//!   other three, so a caller that only wants "the graph" still sees one
//!   list — the cache stays derived, this module stays the source of truth.
//!
//! ### Restriction accepted for v1: only an artifact can be a relation's source
//!
//! A paper's `kovan.toml` and a topic/project's `kovan.toml`
//! ([`crate::entity::EntityConfig`]) have no fenced-TOML-block mechanism —
//! that machinery ([`crate::artifact::parse_document`],
//! [`crate::artifact::render_artifact_block`]) exists only for artifacts
//! inside a paper's research Markdown. So [`add_connection`] only accepts a
//! `source` that parses as `artifact:<citekey>#<id>`
//! ([`crate::graph::artifact_node`]'s own format). Every entry point this
//! epic wires up (the PDF canvas's annotation right-click menu, op-30um.3)
//! only ever draws a connection *from* an annotation or other page-anchored
//! artifact, so this costs nothing in practice today. A `target`, by
//! contrast, may be **any** node identity string — paper, artifact, topic or
//! project — since it is only ever read back, never used to locate a place
//! to write.
//!
//! ### What this does NOT do yet
//!
//! - **`KnowledgeGraph::rebuild` does not fold `[[relation]]` records in as
//!   edges.** [`connections`] is the read path for now; wiring it into the
//!   derived graph (so a mindmap or a backlink query sees user relations
//!   without a second, relation-specific call) is follow-up work, noted
//!   above under "Rejected alternatives".
//! - **No live [`crate::session::PaperSession`] integration.** Every function
//!   here reads a paper's Markdown fresh from disk and writes straight back
//!   with [`crate::session::PaperSession::open`]/`save_document`, exactly
//!   like [`crate::graph::KnowledgeGraph::rebuild`] and
//!   [`crate::autocomplete::library_candidates`] already do for their own
//!   cross-paper reads. A caller that *also* has a live, possibly-unsaved
//!   session open for the very paper being mutated here (the PDF canvas
//!   editing the same annotation, say) can have that session's buffer
//!   silently clobbered by [`add_connection`]/[`edit_connection`]/
//!   [`delete_connection`] writing straight to disk underneath it — §32's
//!   "the buffer is authoritative while a paper is open" rule is a UI-layer
//!   concern to route around (route the mutation through the open session
//!   instead of this module when one exists), not something this module can
//!   enforce from here. Left as a documented caveat for whoever wires up
//!   op-30um.3, rather than guessed at without a concrete caller in hand.
//!
//! ## `connections(node)` scans the whole library
//!
//! A relation only ever lives inside its *source* artifact's file, so
//! finding every relation touching a node **as a target** means scanning
//! every paper's Markdown — the same O(papers) cost class
//! [`crate::autocomplete::library_candidates`] already accepts for its own
//! cross-paper artifact search, and for the same reason: there is no
//! library-wide relation cache (yet) to search instead.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::artifact::{block_span, parse_document, render_artifact_block};
use crate::classify::splice_lines;
use crate::digitiser::dataset::utc_now_iso8601;
use crate::digitiser::raster::hex_lower;
use crate::graph::{artifact_node, NodeId};
use crate::index::KnowledgeIndex;
use crate::root::KovanRoot;
use crate::session::{PaperSession, SessionError};

/// What kind of relationship a [`UserRelation`] records between two nodes
/// (the layer-1 prototype's `RelationKind`, ported verbatim).
///
/// Deliberately not exhaustive of every scientific-argument shape a user
/// might want — it is the fixed vocabulary the prototype dogfooded and
/// agreed on; widening it is a future decision, not something this module
/// pre-empts by adding a catch-all variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind {
    /// A generic, otherwise-unclassified relationship.
    RelatedTo,
    /// The source's argument or data supports the target's.
    Supports,
    /// The source's argument or data contradicts the target's.
    Contradicts,
    /// The source was derived from the target (e.g. a fit derived from a
    /// digitised dataset).
    DerivedFrom,
    /// The source uses data owned by the target (e.g. a model that consumes
    /// a digitised graph's CSV payload).
    UsesDataFrom,
    /// The source validates the target against reality/experiment.
    Validates,
    /// The source was checked against the target as a verification
    /// reference (numerics/implementation correctness, not physical
    /// validity — see `VERIFICATION_AND_VALIDATION.md`'s verification vs.
    /// validation distinction).
    VerifiedAgainst,
    /// The source implements a method/model the target describes.
    Implements,
}

impl RelationKind {
    /// Every variant, in the fixed order the layer-1 prototype declared
    /// them. The one canonical ordering a UI cycles or lists through (e.g.
    /// the PDF canvas connection picker, op-30um.3) — never re-derived
    /// per call site.
    pub const ALL: [RelationKind; 8] = [
        RelationKind::RelatedTo,
        RelationKind::Supports,
        RelationKind::Contradicts,
        RelationKind::DerivedFrom,
        RelationKind::UsesDataFrom,
        RelationKind::Validates,
        RelationKind::VerifiedAgainst,
        RelationKind::Implements,
    ];

    /// A short, lower-case, human-readable label, e.g. `"supports"` — reads
    /// naturally inline ("this note supports that table").
    pub fn label(self) -> &'static str {
        match self {
            Self::RelatedTo => "related to",
            Self::Supports => "supports",
            Self::Contradicts => "contradicts",
            Self::DerivedFrom => "derived from",
            Self::UsesDataFrom => "uses data from",
            Self::Validates => "validates",
            Self::VerifiedAgainst => "verified against",
            Self::Implements => "implements",
        }
    }

    /// The next variant in [`Self::ALL`]'s fixed order, wrapping back to the
    /// first after the last — the whole implementation of a UI's "cycle
    /// kind" button, so no call site hand-rolls its own wraparound.
    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|k| *k == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

/// One user-authored relation between two graph nodes, in memory.
///
/// This is the full triple a caller reasons about; only [`RelationRecord`]
/// (the `target`/`kind` half, with `source` implicit) is ever written to
/// disk — see the module docs for why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserRelation {
    /// Stable id, unique within the library. Generated once by
    /// [`add_connection`] and never recomputed.
    pub id: String,
    /// The node this relation originates from. Always an artifact node
    /// identity (`artifact:<citekey>#<id>`) in the current implementation —
    /// see the module docs' "Restriction accepted for v1".
    pub source: NodeId,
    /// The node this relation points at. Any node identity string —
    /// `paper:`, `artifact:` or `collection:`.
    pub target: NodeId,
    /// What kind of relationship this is.
    pub kind: RelationKind,
}

/// The on-disk record of one [`UserRelation`], as stored inside its source
/// artifact's fenced TOML (`ArtifactToml::relation`). `source` is implicit —
/// it is always the artifact this record is found inside.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationRecord {
    /// Stable id, unique within the library.
    pub id: String,
    /// The node this relation points at.
    pub target: NodeId,
    /// What kind of relationship this is.
    pub kind: RelationKind,
}

/// Errors from the connection CRUD operations.
#[derive(Debug)]
pub enum RelationError {
    /// `source` did not parse as `artifact:<citekey>#<id>` — only an
    /// artifact node may be a relation's source (see the module docs).
    SourceNotArtifact(String),
    /// `add_connection` was asked to relate a node to itself.
    SelfRelation(String),
    /// Opening, reading or saving the artifact's owning paper failed.
    Session(SessionError),
    /// The owning paper exists but has no artifact with this id (or the
    /// artifact named by a `source` node identity does not exist).
    ArtifactNotFound { citekey: String, artifact_id: String },
    /// No relation anywhere in the scanned library has this id.
    RelationNotFound(String),
    /// Re-serialising the artifact's TOML failed (mirrors
    /// `crate::classify::ClassifyError::Render`; cannot happen for
    /// `ArtifactToml`'s current field types, but the caller still gets a
    /// `Result` rather than a `panic!`).
    Render(String),
}

impl std::fmt::Display for RelationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SourceNotArtifact(node) => {
                write!(f, "{node:?} is not an artifact node (artifact:<citekey>#<id>) — only an artifact may be a relation's source")
            }
            Self::SelfRelation(node) => write!(f, "cannot relate {node:?} to itself"),
            Self::Session(e) => write!(f, "{e}"),
            Self::ArtifactNotFound {
                citekey,
                artifact_id,
            } => write!(f, "paper {citekey:?} has no artifact with id {artifact_id:?}"),
            Self::RelationNotFound(id) => write!(f, "no relation with id {id:?}"),
            Self::Render(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for RelationError {}

/// Parse `artifact:<citekey>#<id>` into `(citekey, id)`, or `None` if `node`
/// is not that shape. The exact inverse of [`crate::graph::artifact_node`].
fn parse_artifact_node(node: &str) -> Option<(&str, &str)> {
    node.strip_prefix("artifact:")?.split_once('#')
}

/// A short, unique-enough id for a new relation: a `sha2` digest of a
/// nanosecond timestamp plus a process-local atomic counter, truncated to 12
/// hex characters — the same length as the prototype's
/// `uuid.uuid4().hex[:12]`, without adding a `uuid` dependency this crate
/// does not otherwise need. Not cryptographically random: uniqueness relies
/// on the timestamp+counter pair never repeating within one process, which
/// holds for every caller in this crate (all CRUD here runs synchronously
/// against one open root).
fn generate_relation_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut hasher = Sha256::new();
    hasher.update(nanos.to_le_bytes());
    hasher.update(seq.to_le_bytes());
    hex_lower(&hasher.finalize())[..12].to_string()
}

/// Apply `f` to the relation list of artifact `artifact_id` in `session`'s
/// buffer, re-rendering its block exactly like
/// [`crate::classify::replace_artifact_body`] does for a body edit — same
/// [`splice_lines`] mechanism, same "every other line untouched" guarantee —
/// and bumping `modified`. `f` returning `Err` leaves the buffer exactly as
/// it was (the re-render/splice only happens after `f` succeeds).
fn mutate_relations<T>(
    session: &mut PaperSession,
    artifact_id: &str,
    f: impl FnOnce(&mut Vec<RelationRecord>) -> Result<T, RelationError>,
) -> Result<T, RelationError> {
    let md = session.markdown().to_string();
    let parsed = parse_document(&md);
    let artifact = parsed
        .get(artifact_id)
        .ok_or_else(|| RelationError::ArtifactNotFound {
            citekey: session.citekey().to_string(),
            artifact_id: artifact_id.to_string(),
        })?;

    let mut toml = artifact.toml.clone();
    let result = f(&mut toml.relation)?;
    toml.kovan.modified = utc_now_iso8601();
    let rendered = render_artifact_block(artifact.level, &artifact.heading, &toml, &artifact.body)
        .map_err(RelationError::Render)?;

    let span = block_span(&md, artifact);
    session.set_markdown(splice_lines(&md, span, &rendered));
    Ok(result)
}

/// Every relation record in `root`, paired with the artifact node it
/// belongs to. Reads every paper in `index` from disk — see the module
/// docs' "`connections(node)` scans the whole library".
fn all_relations(root: &KovanRoot, index: &KnowledgeIndex) -> Vec<UserRelation> {
    let mut out = Vec::new();
    for paper in &index.papers {
        let Ok(text) = std::fs::read_to_string(root.paper_markdown(&paper.citekey)) else {
            continue;
        };
        let parsed = parse_document(&text);
        for artifact in &parsed.artifacts {
            if artifact.toml.relation.is_empty() {
                continue;
            }
            let source = artifact_node(&paper.citekey, artifact.id());
            for rec in &artifact.toml.relation {
                out.push(UserRelation {
                    id: rec.id.clone(),
                    source: source.clone(),
                    target: rec.target.clone(),
                    kind: rec.kind,
                });
            }
        }
    }
    out
}

/// Find which paper+artifact holds relation `id`, scanning `index`'s papers
/// from disk. `None` if no relation anywhere has this id.
fn locate_relation(root: &KovanRoot, index: &KnowledgeIndex, id: &str) -> Option<(String, String)> {
    for paper in &index.papers {
        let Ok(text) = std::fs::read_to_string(root.paper_markdown(&paper.citekey)) else {
            continue;
        };
        let parsed = parse_document(&text);
        for artifact in &parsed.artifacts {
            if artifact.toml.relation.iter().any(|r| r.id == id) {
                return Some((paper.citekey.clone(), artifact.id().to_string()));
            }
        }
    }
    None
}

/// Every relation touching `node`, as either its source or its target —
/// matching the prototype's `RelationStore.incident`. Empty if `node` has no
/// connections, never an error (same total-function contract as
/// `crate::autocomplete::library_candidates`).
pub fn connections(root: &KovanRoot, index: &KnowledgeIndex, node: &str) -> Vec<UserRelation> {
    all_relations(root, index)
        .into_iter()
        .filter(|r| r.source == node || r.target == node)
        .collect()
}

/// Create a new [`UserRelation`] from `source` to `target` and persist it
/// into `source`'s own artifact block.
///
/// # Errors
///
/// [`RelationError::SourceNotArtifact`] if `source` is not
/// `artifact:<citekey>#<id>`; [`RelationError::SelfRelation`] if
/// `source == target`; [`RelationError::Session`] if the owning paper
/// cannot be opened or saved; [`RelationError::ArtifactNotFound`] if that
/// paper has no artifact with the named id.
pub fn add_connection(
    root: &KovanRoot,
    source: &str,
    target: &str,
    kind: RelationKind,
) -> Result<UserRelation, RelationError> {
    if source == target {
        return Err(RelationError::SelfRelation(source.to_string()));
    }
    let (citekey, artifact_id) =
        parse_artifact_node(source).ok_or_else(|| RelationError::SourceNotArtifact(source.to_string()))?;

    let mut session = PaperSession::open(root, citekey).map_err(RelationError::Session)?;
    let id = generate_relation_id();
    mutate_relations(&mut session, artifact_id, |relations| {
        relations.push(RelationRecord {
            id: id.clone(),
            target: target.to_string(),
            kind,
        });
        Ok(())
    })?;
    session.save_document().map_err(RelationError::Session)?;

    Ok(UserRelation {
        id,
        source: source.to_string(),
        target: target.to_string(),
        kind,
    })
}

/// Change relation `id`'s `target` and/or `kind`, leaving whichever is
/// `None` unchanged. Locates the owning paper+artifact by scanning `index`
/// (see the module docs).
///
/// # Errors
///
/// [`RelationError::RelationNotFound`] if no relation anywhere has this id;
/// [`RelationError::Session`] if the owning paper cannot be opened or saved.
pub fn edit_connection(
    root: &KovanRoot,
    index: &KnowledgeIndex,
    id: &str,
    target: Option<&str>,
    kind: Option<RelationKind>,
) -> Result<UserRelation, RelationError> {
    let (citekey, artifact_id) =
        locate_relation(root, index, id).ok_or_else(|| RelationError::RelationNotFound(id.to_string()))?;

    let mut session = PaperSession::open(root, &citekey).map_err(RelationError::Session)?;
    let mut updated: Option<RelationRecord> = None;
    mutate_relations(&mut session, &artifact_id, |relations| {
        let rec = relations
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or_else(|| RelationError::RelationNotFound(id.to_string()))?;
        if let Some(t) = target {
            rec.target = t.to_string();
        }
        if let Some(k) = kind {
            rec.kind = k;
        }
        updated = Some(rec.clone());
        Ok(())
    })?;
    session.save_document().map_err(RelationError::Session)?;

    let rec = updated.expect("mutate_relations only returns Ok after finding and updating the record");
    Ok(UserRelation {
        id: rec.id,
        source: artifact_node(&citekey, &artifact_id),
        target: rec.target,
        kind: rec.kind,
    })
}

/// Delete relation `id`. Locates the owning paper+artifact by scanning
/// `index` (see the module docs).
///
/// # Errors
///
/// [`RelationError::RelationNotFound`] if no relation anywhere has this id;
/// [`RelationError::Session`] if the owning paper cannot be opened or saved.
pub fn delete_connection(root: &KovanRoot, index: &KnowledgeIndex, id: &str) -> Result<(), RelationError> {
    let (citekey, artifact_id) =
        locate_relation(root, index, id).ok_or_else(|| RelationError::RelationNotFound(id.to_string()))?;

    let mut session = PaperSession::open(root, &citekey).map_err(RelationError::Session)?;
    mutate_relations(&mut session, &artifact_id, |relations| {
        let before = relations.len();
        relations.retain(|r| r.id != id);
        if relations.len() == before {
            return Err(RelationError::RelationNotFound(id.to_string()));
        }
        Ok(())
    })?;
    session.save_document().map_err(RelationError::Session)
}

/// Delete every relation incident to `node` (source or target), without
/// touching anything else. The building block
/// [`crate::app::delete_artifact_cascade`] (op-30um.2) composes with an
/// artifact deletion to make one atomic user-facing operation; this function
/// on its own makes no claim about atomicity with any other mutation.
///
/// Returns the number of relations removed. Never errors on "nothing to
/// remove" — an artifact with zero incident connections is the common case,
/// not a failure.
pub fn delete_incident(root: &KovanRoot, index: &KnowledgeIndex, node: &str) -> Result<usize, RelationError> {
    let incident = connections(root, index, node);
    let mut removed = 0usize;
    for rel in incident {
        // A relation may already be gone if two incident relations shared an
        // owning artifact and an earlier iteration's mutate_relations call
        // already dropped it as a side effect of some other edit; treat that
        // as already-removed rather than an error.
        match delete_connection(root, index, &rel.id) {
            Ok(()) => removed += 1,
            Err(RelationError::RelationNotFound(_)) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::insert_artifact;
    use crate::entity::{Access, CiteKey, Classification, EntityConfig};
    use crate::root::RootConfig;

    /// A disposable library with one paper (`"src"`) carrying one artifact
    /// (`"note-a"`) and a second paper (`"dst"`) carrying one artifact
    /// (`"note-b"`) — enough nodes to relate to each other without a real
    /// archive.
    fn make_library() -> (tempfile::TempDir, KovanRoot, KnowledgeIndex) {
        let dir = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(dir.path(), RootConfig::new("lib", "Lib"), false).unwrap();
        for citekey in ["src", "dst"] {
            EntityConfig::paper(CiteKey::parse(citekey).unwrap(), Access::Open)
                .save_paper(&root.paper_dir(citekey))
                .unwrap();
        }

        let mut src_session = PaperSession::open(&root, "src").unwrap();
        let src_index = crate::research_record::ResearchRecordIndex::from_session(&src_session);
        insert_artifact(
            &mut src_session,
            &src_index,
            "Note A",
            crate::artifact::ArtifactKind::Note,
            None,
            Classification::default(),
            None,
            "body a",
        )
        .unwrap();
        src_session.save_document().unwrap();

        let mut dst_session = PaperSession::open(&root, "dst").unwrap();
        let dst_index = crate::research_record::ResearchRecordIndex::from_session(&dst_session);
        insert_artifact(
            &mut dst_session,
            &dst_index,
            "Note B",
            crate::artifact::ArtifactKind::Note,
            None,
            Classification::default(),
            None,
            "body b",
        )
        .unwrap();
        dst_session.save_document().unwrap();

        let index = KnowledgeIndex::rebuild(&root);
        (dir, root, index)
    }

    #[test]
    fn add_connection_persists_into_the_source_artifacts_toml_block() {
        let (_dir, root, _index) = make_library();
        let source = artifact_node("src", "note-a");
        let target = artifact_node("dst", "note-b");

        let rel = add_connection(&root, &source, &target, RelationKind::Supports).unwrap();
        assert_eq!(rel.source, source);
        assert_eq!(rel.target, target);
        assert_eq!(rel.kind, RelationKind::Supports);

        // Persisted where the module docs say: inside the source artifact's
        // own fenced TOML, not under `.kovan/`.
        let text = std::fs::read_to_string(root.paper_markdown("src")).unwrap();
        let parsed = parse_document(&text);
        let a = parsed.get("note-a").unwrap();
        assert_eq!(a.toml.relation.len(), 1);
        assert_eq!(a.toml.relation[0].id, rel.id);
        assert_eq!(a.toml.relation[0].target, target);
        assert_eq!(a.toml.relation[0].kind, RelationKind::Supports);
    }

    #[test]
    fn add_connection_rejects_a_non_artifact_source() {
        let (_dir, root, _index) = make_library();
        let err = add_connection(
            &root,
            "paper:src",
            &artifact_node("dst", "note-b"),
            RelationKind::RelatedTo,
        )
        .unwrap_err();
        assert!(matches!(err, RelationError::SourceNotArtifact(_)));
    }

    #[test]
    fn add_connection_rejects_a_self_relation() {
        let (_dir, root, _index) = make_library();
        let node = artifact_node("src", "note-a");
        let err = add_connection(&root, &node, &node, RelationKind::RelatedTo).unwrap_err();
        assert!(matches!(err, RelationError::SelfRelation(_)));
    }

    #[test]
    fn connections_finds_a_relation_from_both_ends() {
        let (_dir, root, index) = make_library();
        let source = artifact_node("src", "note-a");
        let target = artifact_node("dst", "note-b");
        add_connection(&root, &source, &target, RelationKind::UsesDataFrom).unwrap();

        assert_eq!(connections(&root, &index, &source).len(), 1);
        assert_eq!(connections(&root, &index, &target).len(), 1);
        assert_eq!(
            connections(&root, &index, &artifact_node("dst", "nonexistent")).len(),
            0
        );
    }

    #[test]
    fn edit_connection_updates_target_and_kind_in_place() {
        let (_dir, root, index) = make_library();
        let source = artifact_node("src", "note-a");
        let original_target = artifact_node("dst", "note-b");
        let rel = add_connection(&root, &source, &original_target, RelationKind::RelatedTo).unwrap();

        let new_target = "collection:htgrs".to_string();
        let updated = edit_connection(
            &root,
            &index,
            &rel.id,
            Some(&new_target),
            Some(RelationKind::Validates),
        )
        .unwrap();

        assert_eq!(updated.id, rel.id);
        assert_eq!(updated.source, source);
        assert_eq!(updated.target, new_target);
        assert_eq!(updated.kind, RelationKind::Validates);

        // Only one relation exists — edit_connection did not duplicate it.
        assert_eq!(connections(&root, &index, &source).len(), 1);
    }

    #[test]
    fn edit_connection_leaving_a_field_none_keeps_it_unchanged() {
        let (_dir, root, index) = make_library();
        let source = artifact_node("src", "note-a");
        let target = artifact_node("dst", "note-b");
        let rel = add_connection(&root, &source, &target, RelationKind::Supports).unwrap();

        let updated = edit_connection(&root, &index, &rel.id, None, Some(RelationKind::Contradicts)).unwrap();
        assert_eq!(updated.target, target, "target must survive a kind-only edit");
        assert_eq!(updated.kind, RelationKind::Contradicts);
    }

    #[test]
    fn edit_connection_of_an_unknown_id_errors() {
        let (_dir, root, index) = make_library();
        let err = edit_connection(&root, &index, "no-such-id", None, Some(RelationKind::RelatedTo)).unwrap_err();
        assert!(matches!(err, RelationError::RelationNotFound(_)));
    }

    #[test]
    fn delete_connection_removes_it_and_a_second_delete_errors() {
        let (_dir, root, index) = make_library();
        let source = artifact_node("src", "note-a");
        let target = artifact_node("dst", "note-b");
        let rel = add_connection(&root, &source, &target, RelationKind::Implements).unwrap();

        delete_connection(&root, &index, &rel.id).unwrap();
        assert_eq!(connections(&root, &index, &source).len(), 0);

        let err = delete_connection(&root, &index, &rel.id).unwrap_err();
        assert!(matches!(err, RelationError::RelationNotFound(_)));
    }

    #[test]
    fn deleting_a_relation_leaves_the_rest_of_the_markdown_byte_identical() {
        let (_dir, root, index) = make_library();
        let source = artifact_node("src", "note-a");
        let target = artifact_node("dst", "note-b");

        let before = std::fs::read_to_string(root.paper_markdown("src")).unwrap();
        let rel = add_connection(&root, &source, &target, RelationKind::RelatedTo).unwrap();
        delete_connection(&root, &index, &rel.id).unwrap();
        let after = std::fs::read_to_string(root.paper_markdown("src")).unwrap();

        // add + delete round-trips to the same relation list (empty), but
        // `modified` is intentionally bumped by both writes, so compare
        // everything else: same artifact count, same body, same heading.
        let before_doc = parse_document(&before);
        let after_doc = parse_document(&after);
        assert_eq!(before_doc.artifacts.len(), after_doc.artifacts.len());
        let before_a = before_doc.get("note-a").unwrap();
        let after_a = after_doc.get("note-a").unwrap();
        assert_eq!(before_a.heading, after_a.heading);
        assert_eq!(before_a.body, after_a.body);
        assert!(after_a.toml.relation.is_empty());
    }

    #[test]
    fn deleted_relations_stay_deleted_across_a_graph_rebuild() {
        let (_dir, root, index) = make_library();
        let source = artifact_node("src", "note-a");
        let target = artifact_node("dst", "note-b");
        let rel = add_connection(&root, &source, &target, RelationKind::DerivedFrom).unwrap();
        delete_connection(&root, &index, &rel.id).unwrap();

        // Rebuilding the index/graph from scratch must not resurrect it —
        // there is nowhere else it could be cached.
        let rebuilt_index = KnowledgeIndex::rebuild(&root);
        assert_eq!(connections(&root, &rebuilt_index, &source).len(), 0);
        assert_eq!(connections(&root, &rebuilt_index, &target).len(), 0);
    }

    #[test]
    fn delete_incident_removes_every_relation_touching_a_node_from_either_end() {
        let (_dir, root, index) = make_library();
        let a = artifact_node("src", "note-a");
        let b = artifact_node("dst", "note-b");
        add_connection(&root, &a, &b, RelationKind::Supports).unwrap();
        add_connection(&root, &a, "collection:htgrs", RelationKind::RelatedTo).unwrap();

        let removed = delete_incident(&root, &index, &a).unwrap();
        assert_eq!(removed, 2);
        assert_eq!(connections(&root, &index, &a).len(), 0);
        assert_eq!(connections(&root, &index, &b).len(), 0);
    }

    #[test]
    fn delete_incident_on_a_node_with_no_connections_removes_nothing() {
        let (_dir, root, index) = make_library();
        let removed = delete_incident(&root, &index, &artifact_node("src", "note-a")).unwrap();
        assert_eq!(removed, 0);
    }

    #[test]
    fn relation_kind_all_has_eight_unique_variants() {
        let mut all: Vec<_> = RelationKind::ALL.to_vec();
        all.sort_by_key(|k| k.label());
        all.dedup();
        assert_eq!(RelationKind::ALL.len(), 8);
        assert_eq!(all.len(), 8, "ALL must not repeat a variant");
    }

    #[test]
    fn relation_kind_next_cycles_and_wraps() {
        let mut k = RelationKind::RelatedTo;
        for _ in 0..RelationKind::ALL.len() {
            k = k.next();
        }
        assert_eq!(k, RelationKind::RelatedTo, "a full cycle returns to the start");
    }

    #[test]
    fn relation_kind_label_is_non_empty_for_every_variant() {
        for k in RelationKind::ALL {
            assert!(!k.label().is_empty());
        }
    }
}
