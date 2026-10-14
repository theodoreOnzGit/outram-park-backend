//! **Code reviews as artefacts of the mindmap** (GitHub #844; maintainer,
//! 2026-10-12: "If i am adding the code repo, i want to be able to add code
//! reviews as well (these are artefacts too)").
//!
//! A Kovan folder can mount Rust code repositories as its code corpus
//! (`[[repos.code]]`, #840). This module lists, for every mounted
//! repository, the functions, types and constants a reviewer has tied to a
//! standard corpus concept, so the mindmap can show them on that concept's
//! right-click menu beside its literature artefacts and open Code Review
//! at the entry.
//!
//! ```text
//!  mounted code repository
//!    docs/mindmap/connections.toml   "Add connection…" (#839)  ─┐
//!    */review.md  [[relation]] kind = "implements"             ─┼─> CodeReviewArtifact
//!    stamp states (the staleness engine, stamping::stamp_states)┘     (concept, entry,
//!                                                                      rung, state, reviewer)
//! ```
//!
//! **How one is added.** In Code Review, not here: by "Add connection…" on a
//! reviewed entry, or by the concept link chosen while stamping. Both are
//! written into the code repository; nothing is written into the Kovan
//! folder, so the list cannot drift from the code repository it describes.
//!
//! **Only standard corpus concepts.** A connection can name nothing else
//! ([`crate::code_connections`]); an `implements` link to a code
//! repository's own collection is left out, since the Kovan folder's
//! mindmap has no such node.
//!
//! **Cost.** Judging the reviews loads the repository's workspace
//! ([`crate::stamping::load_workspace`]), so [`collect`] is for a worker
//! thread; the mindmap draws from the last result ([`for_concept`]).

use std::collections::BTreeMap;
use std::path::PathBuf;

use kovan_common::artifact::relation::RelationKind;
use kovan_common::review::state::StateKind;

use crate::corpus_tiers::CodeRepo;
use crate::node_id::{Namespace, NodeId};

/// How an entry was tied to its concept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Via {
    /// `docs/mindmap/connections.toml` ("Add connection…").
    Connection,
    /// The review's own `implements` link, chosen while stamping.
    Implements,
}

/// One reviewed entry of a mounted code repository, under one concept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeReviewArtifact {
    /// The code repository's name in `kovan_root.toml`.
    pub repo: String,
    /// Its folder on disk: the workspace Code Review opens.
    pub repo_dir: PathBuf,
    /// The entry, `path/to/file.rs::qual` within the repository (the id
    /// Code Review opens).
    pub entry: String,
    /// The standard corpus concept it is listed under.
    pub concept: NodeId,
    pub via: Via,
    /// The review's state now; `None` when the repository is not indexed or
    /// the entry has no review entry at all.
    pub state: Option<StateKind>,
    /// The rung of the review (0 when there is none).
    pub rung: u8,
    pub reviewer: String,
}

impl CodeReviewArtifact {
    /// What the mindmap's menu shows: the entry without its folders, the
    /// repository, then the review in words.
    pub fn label(&self) -> String {
        let short = self.entry.rsplit('/').next().unwrap_or(&self.entry);
        let review = match self.state {
            Some(s) if s.counts() => format!("rung {}, {}", self.rung, s.label()),
            Some(s) => s.label().to_string(),
            None => "no review found".to_string(),
        };
        format!("{short} ({}): {review}", self.repo)
    }

    /// The fuller line for a hover: the whole path, the reviewer and how it
    /// was tied to the concept.
    pub fn detail(&self) -> String {
        let how = match self.via {
            Via::Connection => "connected with Add connection",
            Via::Implements => "linked while stamping",
        };
        let by = if self.reviewer.is_empty() { String::new() } else { format!(", reviewed by {}", self.reviewer) };
        format!("{}{by}; {how}. Opens Code Review at this entry.", self.entry)
    }
}

/// Every artefact of the mounted code repositories `repos` (module doc),
/// sorted by concept, repository and entry; an entry tied to one concept
/// both ways is listed once, as a connection. A repository that is not
/// checked out, or not indexed, contributes what can still be read (its
/// connections, without a state).
pub fn collect(repos: &[CodeRepo]) -> Vec<CodeReviewArtifact> {
    let mut found: BTreeMap<(String, String, String), CodeReviewArtifact> = BTreeMap::new();
    for repo in repos {
        if !repo.dir.is_dir() {
            continue;
        }
        let states: BTreeMap<String, (StateKind, u8, String)> = crate::stamping::stamp_states(&repo.dir)
            .unwrap_or_default()
            .into_iter()
            .map(|s| (s.function.clone(), (s.kind(), s.rung, s.reviewer)))
            .collect();
        let mut add = |entry: String, concept: NodeId, via: Via| {
            let (state, rung, reviewer) = match states.get(&entry) {
                Some((k, r, by)) => (Some(*k), *r, by.clone()),
                None => (None, 0, String::new()),
            };
            found
                .entry((concept.to_string(), repo.name.clone(), entry.clone()))
                .or_insert(CodeReviewArtifact {
                    repo: repo.name.clone(),
                    repo_dir: repo.dir.clone(),
                    entry,
                    concept,
                    via,
                    state,
                    rung,
                    reviewer,
                });
        };
        // Connections first: an entry tied both ways is listed as one.
        for c in crate::code_connections::read(&repo.dir).connections() {
            add(c.path, c.target, Via::Connection);
        }
        if let Ok(ws) = crate::stamping::load_workspace(&repo.dir) {
            for m in ws.reviews.values() {
                for r in m.doc.reviews() {
                    let Some(entry) = r.path() else { continue };
                    for rel in r.relations.iter().filter(|rel| rel.kind == RelationKind::Implements) {
                        let Some(path) = rel.target.strip_prefix(crate::stamping::concepts::CONCEPT) else {
                            continue;
                        };
                        let path = path.trim_matches('/');
                        if crate::corpus::topic_at(path).is_some() {
                            add(entry.clone(), NodeId::concept(Namespace::Corpus, path), Via::Implements);
                        }
                    }
                }
            }
        }
    }
    found.into_values().collect()
}

/// The artefacts listed under `concept` (a corpus node, or the library
/// mirror of one: both name the same concept on the map).
pub fn for_concept<'a>(all: &'a [CodeReviewArtifact], concept: &NodeId) -> Vec<&'a CodeReviewArtifact> {
    let here = crate::runtime_graph::canonical_concept(concept);
    all.iter().filter(|a| a.concept == here).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stamping::tests::{clean, founder_key, Repo, BY, LIB};
    use crate::stamping::{draft_stamp, write_review, StampRequest};

    /// Two concepts of the built-in standard corpus (any two).
    fn two_concepts() -> (NodeId, NodeId) {
        let mut t = crate::corpus::topics().iter().filter(|t| t.concept.is_some()).map(|t| t.id());
        (t.next().unwrap(), t.next().unwrap())
    }

    /// Methodology: the stamping fixture stands for a mounted code
    /// repository. `leaf` is stamped and connected to concept A with Add
    /// connection; `other` is stamped with an `implements` link to concept
    /// B and also connected to B (tied both ways); `twice` is stamped with
    /// an `implements` link to a collection of the code repository's own.
    /// Then `leaf` is edited.
    ///
    /// Pass: two artefacts, `leaf` under A (a connection) and `other` under
    /// B (listed once, as the connection); each carries the review's rung,
    /// state and reviewer and the folder Code Review opens; the link to the
    /// repository's own collection is left out; the library mirror of a
    /// concept finds the same list; after the edit `leaf` reads "changed
    /// since review" and its label says so; a repository folder that does
    /// not exist contributes nothing.
    ///
    /// Result (2026-10-13): passes.
    #[test]
    fn reviews_of_a_mounted_repository_are_listed_under_their_concepts() {
        let r = Repo::new();
        let store = tempfile::tempdir().unwrap();
        let key = founder_key(&r, store.path());
        let (a, b) = two_concepts();
        let stamp_linked = |qual: &str, target: &str| {
            let req = StampRequest {
                function: format!("{LIB}::{qual}"),
                by: BY.into(),
                checklist: clean(),
                relations: vec![crate::stamping::concepts::implements(target)],
                ..StampRequest::default()
            };
            let mut d = draft_stamp(r.path(), &req).unwrap();
            key.sign_review(&mut d.entry).unwrap();
            write_review(r.path(), &d.entry, "").unwrap();
            r.commit(&format!("stamp {qual}"));
        };
        r.stamp(&key, "leaf", "");
        crate::code_connections::add_for_reviewed(r.path(), &format!("{LIB}::leaf"), &a).unwrap();
        stamp_linked("other", &format!("{}{}", crate::stamping::concepts::CONCEPT, b.path));
        crate::code_connections::add_for_reviewed(r.path(), &format!("{LIB}::other"), &b).unwrap();
        // (This stamp's commit also takes the connections file in.)
        stamp_linked("twice", "collection:physics/own");

        let repo = CodeRepo {
            name: "demo".into(),
            rel: "code/demo".into(),
            dir: r.path().to_path_buf(),
            remote: None,
            branch: None,
        };
        let gone = CodeRepo { name: "gone".into(), dir: r.path().join("no-such-folder"), ..repo.clone() };
        let all = collect(&[repo.clone(), gone]);
        let rows: Vec<(String, String, Via)> = all.iter().map(|x| (x.concept.to_string(), x.entry.clone(), x.via)).collect();
        let mut want = vec![
            (a.to_string(), format!("{LIB}::leaf"), Via::Connection),
            (b.to_string(), format!("{LIB}::other"), Via::Connection),
        ];
        want.sort();
        assert_eq!(rows, want);
        let leaf = for_concept(&all, &a)[0];
        assert_eq!((leaf.state, leaf.rung, leaf.reviewer.as_str()), (Some(StateKind::Valid), 3, BY));
        assert_eq!(leaf.repo_dir, r.path());
        assert_eq!(leaf.label(), "lib.rs::leaf (demo): rung 3, valid");
        assert!(leaf.detail().contains(LIB) && leaf.detail().contains(BY));
        let mirror = NodeId::concept(Namespace::Library, &a.path);
        assert_eq!(for_concept(&all, &mirror).len(), usize::from(crate::runtime_graph::canonical_concept(&mirror) == a));
        assert_eq!(for_concept(&all, &b).len(), 1);

        r.edit("x * 2.0", "x * 3.0");
        r.commit("edit leaf");
        r.reindex();
        let all = collect(&[repo]);
        let leaf = for_concept(&all, &a)[0];
        assert_eq!(leaf.state, Some(StateKind::DirectlyStale));
        assert_eq!(leaf.label(), "lib.rs::leaf (demo): changed since review");
    }

    /// Methodology: the whole path the mindmap takes. A Kovan folder in a
    /// temporary directory; the stamping fixture, given a `Cargo.toml` and
    /// one stamped, connected function, is added to it as a code repository
    /// (`repo_management::add_repository`, a submodule under `code/`). A
    /// fresh `MindmapState` is polled as the app polls it, never waiting on
    /// a frame.
    ///
    /// Pass: the first poll starts the collection and the map has nothing
    /// yet; once the worker finishes, the map holds the one review under
    /// its concept, valid at rung 3, pointing at the mounted folder; a poll
    /// with nothing changed starts no new collection; `invalidate` starts
    /// one; with no folder open the list is empty; and the map draws
    /// headless with the review in its state.
    ///
    /// Result (2026-10-13): passes.
    #[cfg(all(feature = "gui", not(target_os = "android")))]
    #[test]
    fn the_mindmap_collects_a_mounted_repositorys_reviews_in_the_background() {
        use crate::repo_management::{add_repository, NewRepository, RepoKind};
        use crate::root::{KovanRoot, RootConfig};
        let r = Repo::new();
        let store = tempfile::tempdir().unwrap();
        let key = founder_key(&r, store.path());
        let (a, _) = two_concepts();
        r.write("Cargo.toml", "[workspace]\n");
        r.commit("a Cargo.toml at the root");
        r.stamp(&key, "leaf", "");
        crate::code_connections::add_for_reviewed(r.path(), &format!("{LIB}::leaf"), &a).unwrap();
        r.commit("connect leaf");

        let base = tempfile::tempdir().unwrap();
        let folder = base.path().join("kovan");
        std::fs::create_dir_all(&folder).unwrap();
        KovanRoot::create(&folder, RootConfig::new("t", "t"), true).unwrap();
        add_repository(
            &folder,
            &NewRepository {
                kind: RepoKind::Code,
                name: "demo".into(),
                remote: r.path().to_string_lossy().into_owned(),
                path: None,
                branch: None,
            },
        )
        .unwrap();
        let root = KovanRoot::open(&folder).unwrap();
        assert_eq!(root.code_repos().len(), 1);

        let mut map = crate::mindmap::MindmapState::default();
        assert!(map.poll_code_reviews(Some(&root)), "the collection starts");
        assert!(map.code_reviews().is_empty());
        let wait = |map: &mut crate::mindmap::MindmapState| {
            let start = std::time::Instant::now();
            while map.poll_code_reviews(Some(&root)) {
                assert!(start.elapsed() < std::time::Duration::from_secs(120));
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        };
        wait(&mut map);
        let found = map.code_reviews().to_vec();
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].concept, a);
        assert_eq!((found[0].repo.as_str(), found[0].state, found[0].rung), ("demo", Some(StateKind::Valid), 3));
        assert_eq!(found[0].repo_dir, folder.join("code/demo"));
        assert!(!map.poll_code_reviews(Some(&root)), "nothing changed: no new collection");
        map.invalidate_code_reviews();
        assert!(map.poll_code_reviews(Some(&root)));
        wait(&mut map);
        assert_eq!(map.code_reviews().len(), 1);

        let ctx = egui::Context::default();
        let _ = ctx.run_ui(Default::default(), |ui| {
            let _ = map.ui(ui, Some(&root), None, None, None);
        });
        assert_eq!(map.code_reviews().len(), 1);
        assert!(!map.poll_code_reviews(None));
        assert!(map.code_reviews().is_empty());
    }
}
