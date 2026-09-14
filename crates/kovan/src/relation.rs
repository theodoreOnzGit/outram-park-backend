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

use crate::artifact::{
    block_span, parse_document, render_artifact_block, ArtifactKind, ArtifactMeta, ArtifactToml,
    ARTIFACT_LEVEL,
};
use crate::entity::Classification;
use crate::classify::splice_lines;
use crate::digitiser::dataset::utc_now_iso8601;
use crate::digitiser::raster::hex_lower;
#[cfg(test)]
use crate::graph::artifact_node;
use crate::graph::NodeId;
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

impl RelationKind {
    /// The snake_case wire name, as written in a relation artifact's
    /// `[relation] kind` and shown in its heading.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RelatedTo => "related_to",
            Self::Supports => "supports",
            Self::Contradicts => "contradicts",
            Self::DerivedFrom => "derived_from",
            Self::UsesDataFrom => "uses_data_from",
            Self::Validates => "validates",
            Self::VerifiedAgainst => "verified_against",
            Self::Implements => "implements",
        }
    }
}

/// The `[relation]` table of a relation artifact.
///
/// Both endpoints are explicit: a relation is its own artifact now, not a
/// record nested inside the thing it starts from, so nothing about it is
/// implied by where it is written. The id lives in `[kovan] id`, like every
/// other artifact's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationRecord {
    /// The node this relation starts at.
    pub source: NodeId,
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
/// The paper a node belongs to, for the endpoints that live in one.
///
/// `artifact:<citekey>#<id>` and `paper:<citekey>` both resolve; a
/// `collection:` node has no Markdown file and yields `None`.
fn owning_paper(node: &str) -> Option<String> {
    if let Some((citekey, _)) = parse_artifact_node(node) {
        return Some(citekey.to_string());
    }
    node.strip_prefix("paper:").map(str::to_string)
}

/// Open the mindmap document's buffer, creating the file with its own
/// `[kovan] kind = "mindmap"` header artifact if it does not exist yet.
///
/// The mindmap is a document in the same schema as a paper, so it opens
/// with a header artifact naming itself — which is also what makes every
/// relation under it addressable as `artifact:mindmap#<id>`.
fn open_mindmap(root: &KovanRoot) -> Result<(std::path::PathBuf, String), RelationError> {
    let path = root.mindmap_markdown();
    if !path.is_file() {
        let header = ArtifactToml {
            kovan: ArtifactMeta {
                id: MINDMAP_DOC.to_string(),
                kind: ArtifactKind::Mindmap,
                created: utc_now_iso8601(),
                modified: utc_now_iso8601(),
                reviewed: None,
            },
            source: None,
            classification: Classification::default(),
            extraction: None,
            connections: Vec::new(),
            relation: None,
        };
        let rendered = render_artifact_block(ARTIFACT_LEVEL, MINDMAP_DOC, &header, "")
            .map_err(RelationError::Render)?;
        std::fs::write(&path, &rendered).map_err(|e| {
            RelationError::Render(format!("cannot create {}: {e}", path.display()))
        })?;
        return Ok((path, rendered));
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|e| RelationError::Render(format!("cannot read {}: {e}", path.display())))?;
    Ok((path, text))
}

/// The mindmap document's own id, used as the citekey half of a relation
/// artifact's node identity (`artifact:mindmap#<relation id>`).
pub const MINDMAP_DOC: &str = "mindmap";

/// Add `relation_id` to `node`'s artifact `connections` list, in that
/// artifact's own paper — the back-pointer half of the two-way reference.
///
/// A node with no Markdown artifact of its own (a collection, or a paper
/// with no header block yet) is skipped rather than treated as an error:
/// the relation itself is already recorded in the mindmap, and a missing
/// back-pointer degrades discovery, not correctness.
fn link_back(root: &KovanRoot, node: &str, relation_id: &str, add: bool) -> Result<(), RelationError> {
    let Some(citekey) = owning_paper(node) else {
        return Ok(());
    };
    let artifact_id = match parse_artifact_node(node) {
        Some((_, id)) => id.to_string(),
        None => citekey.clone(),
    };
    let Ok(mut session) = PaperSession::open(root, &citekey) else {
        return Ok(());
    };
    let md = session.markdown().to_string();
    let parsed = parse_document(&md);
    let Some(artifact) = parsed.get(&artifact_id) else {
        return Ok(());
    };
    let mut toml = artifact.toml.clone();
    let had = toml.connections.iter().any(|c| c == relation_id);
    if add && !had {
        toml.connections.push(relation_id.to_string());
    } else if !add && had {
        toml.connections.retain(|c| c != relation_id);
    } else {
        return Ok(());
    }
    toml.kovan.modified = utc_now_iso8601();
    let rendered = render_artifact_block(artifact.level, &artifact.heading, &toml, &artifact.body)
        .map_err(RelationError::Render)?;
    let span = block_span(&md, artifact);
    session.set_markdown(splice_lines(&md, span, &rendered));
    session.save_document().map_err(RelationError::Session)?;
    Ok(())
}

/// Render one relation as its own artifact block.
///
/// Every relation is a first-class artifact in the document schema — `#`
/// heading, ```toml metadata, nothing else — exactly like an annotation or
/// a digitised graph (maintainer direction, GH issue #35, 2026-09-08: "I
/// want all connectors, relationships to use the same artifact schema").
/// It carries no body: a connector is metadata, and any prose about it
/// belongs in the artifacts it joins.
fn render_relation_artifact(rel: &UserRelation, created: &str) -> Result<String, RelationError> {
    let toml = ArtifactToml {
        kovan: ArtifactMeta {
            id: rel.id.clone(),
            kind: ArtifactKind::Relation,
            created: created.to_string(),
            modified: utc_now_iso8601(),
            reviewed: None,
        },
        source: None,
        classification: Classification::default(),
        extraction: None,
        connections: Vec::new(),
        relation: Some(RelationRecord {
            source: rel.source.clone(),
            target: rel.target.clone(),
            kind: rel.kind,
        }),
    };
    render_artifact_block(ARTIFACT_LEVEL, &relation_heading(rel), &toml, "")
        .map_err(RelationError::Render)
}

/// A relation artifact's heading: readable in a plain editor, and carrying
/// no `#` of its own (the schema reserves a line-leading `#` for artifact
/// boundaries).
fn relation_heading(rel: &UserRelation) -> String {
    format!(
        "relation: {} {} {}",
        rel.source,
        rel.kind.as_str(),
        rel.target
    )
}

/// Every relation in the library, read from the mindmap document.
fn all_relations(root: &KovanRoot, _index: &KnowledgeIndex) -> Vec<UserRelation> {
    let Ok(text) = std::fs::read_to_string(root.mindmap_markdown()) else {
        return Vec::new();
    };
    parse_document(&text)
        .artifacts
        .iter()
        .filter(|a| a.kind() == ArtifactKind::Relation)
        .filter_map(|a| {
            a.toml.relation.as_ref().map(|r| UserRelation {
                id: a.id().to_string(),
                source: r.source.clone(),
                target: r.target.clone(),
                kind: r.kind,
            })
        })
        .collect()
}

/// Relation `id`, read from the mindmap document.
fn locate_relation(
    root: &KovanRoot,
    index: &KnowledgeIndex,
    id: &str,
) -> Option<UserRelation> {
    all_relations(root, index).into_iter().find(|r| r.id == id)
}

/// Remove relation artifact `id` from the mindmap document, saving it.
fn remove_relation_artifact(root: &KovanRoot, id: &str) -> Result<bool, RelationError> {
    let (path, md) = open_mindmap(root)?;
    let parsed = parse_document(&md);
    let Some(artifact) = parsed
        .artifacts
        .iter()
        .find(|a| a.id() == id && a.kind() == ArtifactKind::Relation)
    else {
        return Ok(false);
    };
    let span = block_span(&md, artifact);
    std::fs::write(&path, splice_lines(&md, span, ""))
        .map_err(|e| RelationError::Render(format!("cannot write {}: {e}", path.display())))?;
    Ok(true)
}

/// Every relation in the library, read from the mindmap document.
///
/// The whole-library read the mindmap needs; [`connections`] filters this
/// to one node.
pub fn connections_all(root: &KovanRoot) -> Vec<UserRelation> {
    all_relations(root, &KnowledgeIndex::default())
}

/// Every relation with `node` at either end.
///
/// Scans every paper in `index`; see the module docs on why that is
/// acceptable at library scale and what would replace it if it stops being.
pub fn connections(root: &KovanRoot, index: &KnowledgeIndex, node: &str) -> Vec<UserRelation> {
    all_relations(root, index)
        .into_iter()
        .filter(|r| r.source == node || r.target == node)
        .collect()
}

/// Record a new typed relation from `source` to `target`, as a relation
/// artifact appended to `source`'s own paper.
///
/// # Errors
///
/// [`RelationError::SourceNotArtifact`] when `source` is not an artifact or
/// paper node (a collection cannot own a relation, having no file of its
/// own), and [`RelationError::Session`]/[`RelationError::Render`] on a
/// failure to read, render or write the paper.
pub fn add_connection(
    root: &KovanRoot,
    source: &str,
    target: &str,
    kind: RelationKind,
) -> Result<UserRelation, RelationError> {
    if source == target {
        return Err(RelationError::SelfRelation(source.to_string()));
    }
    // A source must belong to a paper, so the back-pointer has somewhere to
    // live; the relation itself always goes in the mindmap document.
    owning_paper(source).ok_or_else(|| RelationError::SourceNotArtifact(source.to_string()))?;

    let rel = UserRelation {
        id: generate_relation_id(),
        source: source.to_string(),
        target: target.to_string(),
        kind,
    };
    let (path, md) = open_mindmap(root)?;
    let rendered = render_relation_artifact(&rel, &utc_now_iso8601())?;
    let mut next = md.trim_end().to_string();
    next.push_str("\n\n");
    next.push_str(rendered.trim_end());
    next.push('\n');
    std::fs::write(&path, next)
        .map_err(|e| RelationError::Render(format!("cannot write {}: {e}", path.display())))?;

    // Two-way reference: each endpoint's own artifact records the relation
    // id, so a reader holding the paper can find the connector without
    // scanning, and the connector names the paper and artifact it came from.
    link_back(root, source, &rel.id, true)?;
    link_back(root, target, &rel.id, true)?;
    Ok(rel)
}

/// Change relation `id`'s target, its kind, or both. `None` leaves that
/// field as it was.
///
/// # Errors
///
/// [`RelationError::NotFound`] when no relation artifact has that id.
pub fn edit_connection(
    root: &KovanRoot,
    index: &KnowledgeIndex,
    id: &str,
    target: Option<&str>,
    kind: Option<RelationKind>,
) -> Result<UserRelation, RelationError> {
    let mut rel = locate_relation(root, index, id)
        .ok_or_else(|| RelationError::RelationNotFound(id.to_string()))?;
    let old_target = rel.target.clone();
    if let Some(t) = target {
        rel.target = t.to_string();
    }
    if let Some(k) = kind {
        rel.kind = k;
    }

    let (path, md) = open_mindmap(root)?;
    let parsed = parse_document(&md);
    let artifact = parsed
        .artifacts
        .iter()
        .find(|a| a.id() == id && a.kind() == ArtifactKind::Relation)
        .ok_or_else(|| RelationError::RelationNotFound(id.to_string()))?;
    let created = artifact.toml.kovan.created.clone();
    let rendered = render_relation_artifact(&rel, &created)?;
    let span = block_span(&md, artifact);
    std::fs::write(&path, splice_lines(&md, span, &rendered))
        .map_err(|e| RelationError::Render(format!("cannot write {}: {e}", path.display())))?;

    // Retarget: the old endpoint stops pointing at this relation, the new
    // one starts.
    if old_target != rel.target {
        link_back(root, &old_target, id, false)?;
        link_back(root, &rel.target, id, true)?;
    }
    Ok(rel)
}

/// Delete relation `id`.
///
/// # Errors
///
/// [`RelationError::NotFound`] when no relation artifact has that id.
pub fn delete_connection(
    root: &KovanRoot,
    index: &KnowledgeIndex,
    id: &str,
) -> Result<(), RelationError> {
    let rel = locate_relation(root, index, id)
        .ok_or_else(|| RelationError::RelationNotFound(id.to_string()))?;
    remove_relation_artifact(root, id)?;
    // Leave no dangling back-pointers behind.
    link_back(root, &rel.source, id, false)?;
    link_back(root, &rel.target, id, false)?;
    Ok(())
}

/// Delete every relation with `node` at either end, returning how many went.
///
/// This is what makes [`crate::classify::delete_artifact_cascade`] a
/// cascade: an *incoming* relation lives in the other paper's file, so
/// removing an artifact has to reach beyond its own document.
pub fn delete_incident(
    root: &KovanRoot,
    index: &KnowledgeIndex,
    node: &str,
) -> Result<usize, RelationError> {
    let doomed: Vec<UserRelation> = connections(root, index, node);
    let mut removed = 0;
    for rel in doomed {
        if remove_relation_artifact(root, &rel.id)? {
            link_back(root, &rel.source, &rel.id, false)?;
            link_back(root, &rel.target, &rel.id, false)?;
            removed += 1;
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
    fn add_connection_writes_the_mindmap_artifact_and_both_back_pointers() {
        let (_dir, root, _index) = make_library();
        let source = artifact_node("src", "note-a");
        let target = artifact_node("dst", "note-b");

        let rel = add_connection(&root, &source, &target, RelationKind::Supports).unwrap();
        assert_eq!(rel.source, source);
        assert_eq!(rel.target, target);
        assert_eq!(rel.kind, RelationKind::Supports);

        // The relation is its own artifact in the MINDMAP document — same
        // `#` heading + ```toml schema as every other artifact, never under
        // `.kovan/`.
        let mindmap = std::fs::read_to_string(root.mindmap_markdown()).unwrap();
        let map_doc = parse_document(&mindmap);
        let a = map_doc.get(&rel.id).expect("relation artifact in mindmap.md");
        assert_eq!(a.kind(), ArtifactKind::Relation);
        let record = a.toml.relation.as_ref().expect("[relation] table");
        assert_eq!(record.source, source, "names the artifact it came from");
        assert_eq!(record.target, target);
        assert_eq!(record.kind, RelationKind::Supports);
        // A connector is metadata: no body, and its heading is readable.
        assert!(a.body.trim().is_empty());
        assert!(a.heading.starts_with("relation: "));
        // The mindmap document opens with its own header artifact.
        assert_eq!(
            map_doc.get(MINDMAP_DOC).map(|m| m.kind()),
            Some(ArtifactKind::Mindmap)
        );

        // ...and BOTH endpoints point back at it, so the reference is
        // two-way from either file.
        for (citekey, artifact_id) in [("src", "note-a"), ("dst", "note-b")] {
            let text = std::fs::read_to_string(root.paper_markdown(citekey)).unwrap();
            let endpoint = parse_document(&text);
            let art = endpoint.get(artifact_id).unwrap();
            assert_eq!(
                art.toml.connections,
                vec![rel.id.clone()],
                "{citekey}#{artifact_id} should point back at the relation"
            );
            // The endpoint stays an ordinary artifact — the relation itself
            // is not nested inside it.
            assert!(art.toml.relation.is_none());
        }
    }

    #[test]
    fn add_connection_rejects_a_source_with_no_file_of_its_own() {
        // A paper node IS a valid source now — the paper's header block is
        // an artifact like any other (GH issue #35, 2026-09-08). What still
        // cannot own a relation is a collection: topics and projects have
        // no Markdown file for the relation artifact to live in.
        let (_dir, root, _index) = make_library();
        let err = add_connection(
            &root,
            "collection:htgrs",
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
        assert!(after_a.toml.relation.is_none());
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
