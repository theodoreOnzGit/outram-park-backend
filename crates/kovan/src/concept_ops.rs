//! Full CRUD on a user's concepts — rename, move and delete — as a single
//! **transactional** rewrite of every reference to the paths involved.
//!
//! # Why this is not three small functions
//!
//! A concept's *path* is its identity. Renaming `htgrs` to `htgr` does not
//! touch one directory: it changes the path of every descendant, and every
//! place that names any of those paths has to change with it, or the library
//! is left pointing at concepts that no longer exist. Epic #241 listed
//! Rename / Move / Merge / Delete as out of scope for exactly this reason —
//! "they need the transactional rewrite of classifications and `[[...]]`
//! links". This module is that rewrite.
//!
//! # What refers to a concept path
//!
//! Four things, and all four are handled here:
//!
//! 1. Its **directory**, under `topics/` or `projects/`, with its subtree.
//! 2. Each paper's **`kovan.toml`** `[classification]`.
//! 3. Each **artifact block** in any markdown: its own `[classification]`,
//!    and a `[relation]` endpoint naming `collection:<path>`.
//! 4. The **mindmap document**, whose relation artifacts are blocks of the
//!    same shape.
//!
//! Wiki links are not in the list: `[[target]]` names a citekey, never a
//! collection, so a concept rename cannot invalidate one.
//!
//! # Plan, then apply
//!
//! [`plan`] reads and computes; [`apply`] writes. Nothing is written while
//! anything can still fail to *compute*, so an operation that turns out to
//! be impossible leaves the library untouched rather than half-rewritten.
//! A caller can also show the plan first — how many papers and artifacts a
//! rename will touch is exactly the thing a user wants to know before
//! agreeing to it.
//!
//! Writing itself is ordered so the worst interruption is recoverable: every
//! **file** edit lands first, and the **directory move** last. Interrupted in
//! between, the references point at a path the directory has not reached yet,
//! which `plan` can be run again to finish — the opposite order would leave
//! references pointing at a directory that is already gone.

use std::path::PathBuf;

use crate::classify::{retarget_document, PathChange};
use crate::entity::{EntityConfig, EntityKind};
use crate::index::KnowledgeIndex;
use crate::root::KovanRoot;

/// What to do to a concept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConceptOp {
    /// Give it a new slug, keeping its parent.
    Rename { new_name: String },
    /// Move it under `new_parent` (`""` for the top of its tree), keeping
    /// its slug.
    Move { new_parent: String },
    /// Delete it and its subtree, dropping every reference to any of them.
    Delete,
}

/// Why an operation cannot be planned or applied.
#[derive(Debug)]
pub enum ConceptOpError {
    /// No such concept in the index.
    Unknown(String),
    /// The new name has no usable characters for a slug.
    EmptyName,
    /// A concept already exists at the destination path.
    Occupied(String),
    /// A move would put a concept inside its own subtree.
    IntoItself(String),
    /// Rewriting a document's blocks failed.
    Document { path: PathBuf, message: String },
    Io { path: PathBuf, message: String },
}

impl std::fmt::Display for ConceptOpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown(p) => write!(f, "no concept at {p:?}"),
            Self::EmptyName => write!(f, "name has no usable characters"),
            Self::Occupied(p) => write!(f, "something already exists at {p:?}"),
            Self::IntoItself(p) => write!(f, "cannot move {p:?} inside itself"),
            Self::Document { path, message } => {
                write!(f, "{}: {message}", path.display())
            }
            Self::Io { path, message } => write!(f, "{}: {message}", path.display()),
        }
    }
}

impl std::error::Error for ConceptOpError {}

/// A computed, not-yet-applied change set.
#[derive(Debug, Clone)]
pub struct Plan {
    /// The concept this is about, and its kind.
    pub path: String,
    pub kind: EntityKind,
    /// `old path -> new path`, for the concept and every descendant.
    /// Empty `new` means the path is being removed.
    pub moves: Vec<(String, String)>,
    /// The directory to move, and where to — `None` when deleting.
    pub dir: (PathBuf, Option<PathBuf>),
    /// Whole file contents to write, computed from what is on disk now.
    pub edits: Vec<(PathBuf, String)>,
    /// How many papers' own classifications change.
    pub papers_touched: usize,
}

impl Plan {
    /// Whether this plan deletes rather than relocates.
    pub fn is_delete(&self) -> bool {
        self.dir.1.is_none()
    }

    /// A one-line summary for a confirmation prompt.
    pub fn summary(&self) -> String {
        let concepts = self.moves.len();
        let files = self.edits.len();
        if self.is_delete() {
            format!(
                "delete {concepts} concept(s), updating {files} file(s) and {} paper classification(s)",
                self.papers_touched
            )
        } else {
            format!(
                "move {concepts} concept(s), updating {files} file(s) and {} paper classification(s)",
                self.papers_touched
            )
        }
    }
}

/// The tree a concept of `kind` lives in.
fn tree_root(root: &KovanRoot, kind: EntityKind) -> PathBuf {
    match kind {
        EntityKind::Project => root.projects_dir(),
        _ => root.topics_dir(),
    }
}

/// Whether `path` is `ancestor` itself or inside it.
fn within(ancestor: &str, path: &str) -> bool {
    path == ancestor || path.starts_with(&format!("{ancestor}/"))
}

/// Compute the change set for `op` on the concept at `path`.
///
/// Reads the whole library; writes nothing.
pub fn plan(
    root: &KovanRoot,
    index: &KnowledgeIndex,
    path: &str,
    kind: EntityKind,
    op: &ConceptOp,
) -> Result<Plan, ConceptOpError> {
    let entry = index
        .collections
        .iter()
        .find(|c| c.path == path && c.kind == kind)
        .ok_or_else(|| ConceptOpError::Unknown(path.to_string()))?;
    let _ = entry;

    // Where the concept ends up.
    let parent = path.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
    let slug = path.rsplit('/').next().unwrap_or(path);
    let new_path = match op {
        ConceptOp::Delete => None,
        ConceptOp::Rename { new_name } => {
            let new_slug = crate::classify::slugify(new_name);
            if new_slug.is_empty() {
                return Err(ConceptOpError::EmptyName);
            }
            Some(if parent.is_empty() {
                new_slug
            } else {
                format!("{parent}/{new_slug}")
            })
        }
        ConceptOp::Move { new_parent } => {
            if within(path, new_parent) {
                return Err(ConceptOpError::IntoItself(path.to_string()));
            }
            Some(if new_parent.is_empty() {
                slug.to_string()
            } else {
                format!("{new_parent}/{slug}")
            })
        }
    };

    if let Some(dest) = &new_path {
        if dest != path
            && index
                .collections
                .iter()
                .any(|c| c.kind == kind && c.path == *dest)
        {
            return Err(ConceptOpError::Occupied(dest.clone()));
        }
    }

    // Every affected path: the concept and its descendants.
    let mut moves: Vec<(String, String)> = index
        .collections
        .iter()
        .filter(|c| c.kind == kind && within(path, &c.path))
        .map(|c| {
            let dest = match &new_path {
                None => String::new(),
                Some(dest) => format!("{dest}{}", &c.path[path.len()..]),
            };
            (c.path.clone(), dest)
        })
        .collect();
    moves.sort();

    let change = |p: &str| -> PathChange {
        match moves.iter().find(|(old, _)| old == p) {
            None => PathChange::Keep,
            Some((_, dest)) if dest.is_empty() => PathChange::Remove,
            Some((_, dest)) => PathChange::To(dest.clone()),
        }
    };

    let mut edits: Vec<(PathBuf, String)> = Vec::new();
    let mut papers_touched = 0;

    // 2. Each paper's own classification.
    for paper in &index.papers {
        let dir = root.paper_dir(&paper.citekey);
        let Ok(mut config) = EntityConfig::load(&dir) else {
            continue;
        };
        let mut touched = false;
        for list in [
            &mut config.classification.topics,
            &mut config.classification.projects,
        ] {
            let mut out: Vec<String> = Vec::with_capacity(list.len());
            for p in list.iter() {
                match change(p) {
                    PathChange::Keep => out.push(p.clone()),
                    PathChange::To(q) => {
                        touched = true;
                        if !out.contains(&q) {
                            out.push(q);
                        }
                    }
                    PathChange::Remove => touched = true,
                }
            }
            if touched {
                *list = out;
            }
        }
        if touched {
            // A paper left with nothing is back in the inbox, not in limbo:
            // `EntityConfig::validate` rejects an empty classification, and
            // an unclassified paper is exactly what `unsorted` is for.
            if config.classification.topics.is_empty()
                && config.classification.projects.is_empty()
            {
                config.classification = crate::entity::Classification::unsorted();
            }
            // Serialised here rather than saved, so a failure is found
            // while the plan is still only a plan.
            config.validate().map_err(|e| ConceptOpError::Document {
                path: dir.join(crate::entity::ENTITY_MARKER),
                message: e.to_string(),
            })?;
            let toml = config.to_toml().map_err(|message| ConceptOpError::Document {
                path: dir.join(crate::entity::ENTITY_MARKER),
                message,
            })?;
            edits.push((dir.join(crate::entity::ENTITY_MARKER), toml));
            papers_touched += 1;
        }

        // 3. The paper's artifact blocks.
        let md_path = root.paper_markdown(&paper.citekey);
        if let Ok(md) = std::fs::read_to_string(&md_path) {
            match retarget_document(&md, &change) {
                Ok(Some(new_md)) => edits.push((md_path, new_md)),
                Ok(None) => {}
                Err(e) => {
                    return Err(ConceptOpError::Document {
                        path: md_path,
                        message: e.to_string(),
                    })
                }
            }
        }
    }

    // 4. The mindmap document's relation artifacts.
    let map_path = root.mindmap_markdown();
    if let Ok(md) = std::fs::read_to_string(&map_path) {
        match retarget_document(&md, &change) {
            Ok(Some(new_md)) => edits.push((map_path, new_md)),
            Ok(None) => {}
            Err(e) => {
                return Err(ConceptOpError::Document {
                    path: map_path,
                    message: e.to_string(),
                })
            }
        }
    }

    let tree = tree_root(root, kind);
    let dir = (
        tree.join(path),
        new_path.as_ref().map(|dest| tree.join(dest)),
    );

    Ok(Plan {
        path: path.to_string(),
        kind,
        moves,
        dir,
        edits,
        papers_touched,
    })
}

/// Apply a [`Plan`]: every file edit first, the directory move last.
///
/// See the module docs for why that order is the recoverable one.
pub fn apply(plan: &Plan) -> Result<(), ConceptOpError> {
    for (path, contents) in &plan.edits {
        std::fs::write(path, contents).map_err(|e| ConceptOpError::Io {
            path: path.clone(),
            message: e.to_string(),
        })?;
    }
    let (from, to) = &plan.dir;
    match to {
        Some(dest) => {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).map_err(|e| ConceptOpError::Io {
                    path: parent.to_path_buf(),
                    message: e.to_string(),
                })?;
            }
            std::fs::rename(from, dest).map_err(|e| ConceptOpError::Io {
                path: from.clone(),
                message: e.to_string(),
            })?;
        }
        None => {
            std::fs::remove_dir_all(from).map_err(|e| ConceptOpError::Io {
                path: from.clone(),
                message: e.to_string(),
            })?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{Access, CiteKey};

    /// A library with a nested topic tree, a paper filed in the deepest
    /// topic, and an artifact in that paper connected to a sibling topic.
    fn library() -> (tempfile::TempDir, KovanRoot) {
        let dir = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(
            dir.path(),
            crate::root::RootConfig::new("lib", "Lib"),
            false,
        )
        .unwrap();
        for (path, name) in [
            ("htgrs", "HTGRs"),
            ("htgrs/fuel", "Fuel"),
            ("htgrs/fuel/triso", "TRISO"),
            ("htgrs/thermal", "Thermal"),
        ] {
            EntityConfig::topic(path.rsplit('/').next().unwrap(), name)
                .save(&root.topics_dir().join(path))
                .unwrap();
        }
        let paper_dir = root.paper_dir("ong2026fuel");
        let mut config = EntityConfig::paper(CiteKey::parse("ong2026fuel").unwrap(), Access::Open);
        config.classification = crate::entity::Classification {
            topics: vec!["htgrs/fuel/triso".to_string()],
            projects: Vec::new(),
        };
        config.save_paper(&paper_dir).unwrap();

        // An artifact classified under one topic and connected to another.
        let md = root.paper_markdown("ong2026fuel");
        let body = std::fs::read_to_string(&md).unwrap();
        let block = "\n# Figure 3\n\n```toml\n[kovan]\nid = \"fig-3\"\nkind = \"digitised_graph\"\ncreated = \"2026-01-01T00:00:00Z\"\nmodified = \"2026-01-01T00:00:00Z\"\n\n[classification]\ntopics = [\"htgrs/fuel/triso\"]\n\n[relation]\nsource = \"artifact:ong2026fuel#fig-3\"\ntarget = \"collection:htgrs/thermal\"\nkind = \"related_to\"\n```\n\n";
        std::fs::write(&md, format!("{body}{block}")).unwrap();
        (dir, root)
    }

    /// Renaming a concept rewrites its whole subtree, every paper filed
    /// anywhere inside it, and every artifact block that named one of those
    /// paths — classification and relation endpoint alike.
    #[test]
    fn renaming_rewrites_the_subtree_and_every_reference() {
        let (_d, root) = library();
        let index = KnowledgeIndex::rebuild(&root);
        let op = ConceptOp::Rename {
            new_name: "Gas Reactors".to_string(),
        };
        let plan = plan(&root, &index, "htgrs", EntityKind::Topic, &op).unwrap();

        // The concept and all three descendants move.
        assert_eq!(plan.moves.len(), 4, "{:?}", plan.moves);
        assert!(plan
            .moves
            .contains(&("htgrs/fuel/triso".to_string(), "gas-reactors/fuel/triso".to_string())));
        assert_eq!(plan.papers_touched, 1);
        assert!(!plan.is_delete());

        apply(&plan).unwrap();
        let index = KnowledgeIndex::rebuild(&root);

        assert!(index.collections.iter().all(|c| !c.path.starts_with("htgrs")));
        assert_eq!(
            index.papers[0].topics,
            vec!["gas-reactors/fuel/triso".to_string()],
            "the paper followed its topic"
        );
        let md = std::fs::read_to_string(root.paper_markdown("ong2026fuel")).unwrap();
        assert!(
            md.contains("gas-reactors/fuel/triso"),
            "the artifact's classification followed too"
        );
        assert!(
            md.contains("collection:gas-reactors/thermal"),
            "so did its relation endpoint: {md}"
        );
        assert!(!md.contains("htgrs/"), "nothing still points at the old path");
    }

    /// Moving a concept to a new parent takes its subtree with it, and a
    /// move into its own subtree is refused rather than losing the tree.
    #[test]
    fn moving_reparents_the_subtree_and_refuses_a_cycle() {
        let (_d, root) = library();
        let index = KnowledgeIndex::rebuild(&root);

        let into_itself = ConceptOp::Move {
            new_parent: "htgrs/fuel".to_string(),
        };
        assert!(matches!(
            plan(&root, &index, "htgrs", EntityKind::Topic, &into_itself),
            Err(ConceptOpError::IntoItself(_))
        ));

        let op = ConceptOp::Move {
            new_parent: "htgrs/thermal".to_string(),
        };
        let plan = plan(&root, &index, "htgrs/fuel", EntityKind::Topic, &op).unwrap();
        apply(&plan).unwrap();

        let index = KnowledgeIndex::rebuild(&root);
        assert!(index
            .collections
            .iter()
            .any(|c| c.path == "htgrs/thermal/fuel/triso"));
        assert_eq!(
            index.papers[0].topics,
            vec!["htgrs/thermal/fuel/triso".to_string()]
        );
    }

    /// Deleting drops the subtree and every reference to it; a paper left
    /// with no classification goes back to the Unsorted inbox rather than
    /// becoming invalid.
    #[test]
    fn deleting_drops_references_and_returns_the_paper_to_unsorted() {
        let (_d, root) = library();
        let index = KnowledgeIndex::rebuild(&root);
        let plan = plan(&root, &index, "htgrs/fuel", EntityKind::Topic, &ConceptOp::Delete).unwrap();
        assert!(plan.is_delete());
        assert_eq!(plan.moves.len(), 2, "the concept and its one descendant");

        apply(&plan).unwrap();
        let index = KnowledgeIndex::rebuild(&root);

        assert!(!root.topics_dir().join("htgrs/fuel").exists());
        assert!(index.collections.iter().any(|c| c.path == "htgrs"));
        assert_eq!(
            index.papers[0].topics,
            vec![crate::entity::UNSORTED.to_string()],
            "a paper with nothing left is in the inbox, not invalid"
        );
        let md = std::fs::read_to_string(root.paper_markdown("ong2026fuel")).unwrap();
        assert!(
            !md.contains("htgrs/fuel"),
            "the artifact's classification lost the deleted path: {md}"
        );
    }

    /// Planning writes nothing: a plan that is computed and then dropped
    /// leaves the library exactly as it was.
    #[test]
    fn planning_alone_changes_nothing() {
        let (_d, root) = library();
        let index = KnowledgeIndex::rebuild(&root);
        let before = std::fs::read_to_string(root.paper_markdown("ong2026fuel")).unwrap();

        let _plan = plan(
            &root,
            &index,
            "htgrs",
            EntityKind::Topic,
            &ConceptOp::Rename {
                new_name: "Something Else".to_string(),
            },
        )
        .unwrap();

        assert!(root.topics_dir().join("htgrs").exists());
        assert_eq!(
            std::fs::read_to_string(root.paper_markdown("ong2026fuel")).unwrap(),
            before
        );
    }
}
