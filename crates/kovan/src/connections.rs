//! The user's own node-to-node connections, `<root>/mindmap/connections.toml`
//! (GitHub issue #285).
//!
//! # Why this exists beside [`crate::relation`]
//!
//! [`crate::relation`] records a typed relation **owned by an artifact**: the
//! relation artifact is appended to `<root>/mindmap.md` and each endpoint's
//! own artifact records the relation id. `add_connection` therefore refuses a
//! collection outright — `RelationError::SourceNotArtifact`, "a collection
//! cannot own a relation, having no file of its own".
//!
//! A mind-map hyperlink is exactly that excluded case: *this concept links to
//! that concept*, where neither end owns a document the link could live in.
//! The maintainer's decision of 2026-09-22 is where it goes instead — "user
//! connections go in the user root's `mindmap/connections.toml`, persisted
//! once, reverse edges derived" — and this module is that file.
//!
//! **Two stores, for now.** Relations owned by an artifact stay in
//! `mindmap.md`; this file holds the links that have no owner. Folding the
//! first into the second is the migration that decision implies, and is not
//! done here.
//!
//! # Persisted once, reverse derived
//!
//! A link is written in the direction the user made it, once. Both ends see
//! it: [`for_node`] returns a link whether the node is its source or its
//! target, so "what does this concept link to" and "what links here" come
//! from the same single entry rather than from two rows that can disagree.
//!
//! # Totality
//!
//! A missing file is "no connections yet", not an error — a fresh library has
//! none, and the mind map asks on every frame. An entry whose node id will
//! not parse is skipped rather than failing the whole file, the same rule
//! [`crate::index`] applies to a malformed entity: one bad row must not make
//! the rest unreadable.

use serde::{Deserialize, Serialize};

use crate::node_id::NodeId;
use crate::root::KovanRoot;

/// One user-made link between two nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connection {
    /// The node the link was made *from* — the concept whose star shows it
    /// as an outgoing hyperlink.
    pub source: NodeId,
    /// The node it points at.
    pub target: NodeId,
}

/// What went wrong writing the file. Reading never fails (see the module
/// doc's "Totality").
#[derive(Debug)]
pub enum ConnectionsError {
    /// Source and target are the same node.
    SelfLink(String),
    /// The link is already recorded, in one direction or the other.
    Duplicate(String, String),
    Io(std::io::Error),
    /// The file could not be rendered as TOML. Not reachable through this
    /// module's own types; kept so a future field cannot panic here.
    Render(String),
}

impl std::fmt::Display for ConnectionsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SelfLink(n) => write!(f, "{n} cannot link to itself"),
            Self::Duplicate(a, b) => write!(f, "{a} and {b} are already linked"),
            Self::Io(e) => write!(f, "{e}"),
            Self::Render(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ConnectionsError {}

/// The on-disk shape. A table array, so the file reads as a list of links
/// and a hand edit is obvious:
///
/// ```toml
/// [[connection]]
/// source = "library:concept/njoy"
/// target = "corpus:concept/nuclear-data/njoy"
/// ```
#[derive(Debug, Default, Serialize, Deserialize)]
struct ConnectionsFile {
    #[serde(default, rename = "connection")]
    connections: Vec<Entry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Entry {
    source: String,
    target: String,
}

/// Every connection in `root`, in the order they were made.
///
/// A missing or unreadable file gives an empty list, and an entry with an
/// unparseable node id is skipped — see the module doc.
pub fn load(root: &KovanRoot) -> Vec<Connection> {
    let Ok(text) = std::fs::read_to_string(root.mindmap_connections()) else {
        return Vec::new();
    };
    let parsed: ConnectionsFile = toml::from_str(&text).unwrap_or_default();
    parsed
        .connections
        .into_iter()
        .filter_map(|e| {
            Some(Connection {
                source: NodeId::parse(&e.source).ok()?,
                target: NodeId::parse(&e.target).ok()?,
            })
        })
        .collect()
}

/// Write `connections` to `root`, replacing whatever was there, creating
/// `mindmap/` if this is the first one.
pub fn save(root: &KovanRoot, connections: &[Connection]) -> Result<(), ConnectionsError> {
    let file = ConnectionsFile {
        connections: connections
            .iter()
            .map(|c| Entry {
                source: c.source.to_string(),
                target: c.target.to_string(),
            })
            .collect(),
    };
    let body =
        toml::to_string_pretty(&file).map_err(|e| ConnectionsError::Render(e.to_string()))?;
    let path = root.mindmap_connections();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(ConnectionsError::Io)?;
    }
    let header = "# Kovan mind-map connections.\n\
                  #\n\
                  # One entry per link the user made between two nodes, written in the\n\
                  # direction it was made. The reverse is derived when the map is drawn,\n\
                  # so a link is never recorded twice.\n\n";
    std::fs::write(&path, format!("{header}{body}")).map_err(ConnectionsError::Io)
}

/// Record a link from `source` to `target`.
///
/// # Errors
///
/// [`ConnectionsError::SelfLink`] when the two are the same node, and
/// [`ConnectionsError::Duplicate`] when they are already linked **in either
/// direction** — the reverse is derived, so recording it again would only
/// draw the same link twice.
pub fn add(root: &KovanRoot, source: &NodeId, target: &NodeId) -> Result<(), ConnectionsError> {
    if source == target {
        return Err(ConnectionsError::SelfLink(source.to_string()));
    }
    let mut all = load(root);
    if all.iter().any(|c| joins(c, source, target)) {
        return Err(ConnectionsError::Duplicate(
            source.to_string(),
            target.to_string(),
        ));
    }
    all.push(Connection {
        source: source.clone(),
        target: target.clone(),
    });
    save(root, &all)
}

/// Remove the link between `a` and `b`, whichever direction it was made in.
/// `false` means there was none.
pub fn remove(root: &KovanRoot, a: &NodeId, b: &NodeId) -> Result<bool, ConnectionsError> {
    let all = load(root);
    let kept: Vec<Connection> = all.iter().filter(|c| !joins(c, a, b)).cloned().collect();
    if kept.len() == all.len() {
        return Ok(false);
    }
    save(root, &kept)?;
    Ok(true)
}

/// Every connection with `node` at either end, and the node at the *other*
/// end of each — which is what a star actually wants to draw.
///
/// The direction the link was made in is deliberately not reported: a
/// hyperlink between two concepts means the same thing from both sides, and
/// the mind map draws it on both.
pub fn for_node<'a>(connections: &'a [Connection], node: &NodeId) -> Vec<&'a NodeId> {
    connections
        .iter()
        .filter_map(|c| {
            if &c.source == node {
                Some(&c.target)
            } else if &c.target == node {
                Some(&c.source)
            } else {
                None
            }
        })
        .collect()
}

/// Whether `c` joins `a` and `b`, in either direction.
fn joins(c: &Connection, a: &NodeId, b: &NodeId) -> bool {
    (&c.source == a && &c.target == b) || (&c.source == b && &c.target == a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_id::Namespace;
    use crate::root::RootConfig;

    fn make_root() -> (tempfile::TempDir, KovanRoot) {
        let dir = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(dir.path(), RootConfig::new("lib", "Lib"), false).unwrap();
        (dir, root)
    }

    fn mine(path: &str) -> NodeId {
        NodeId::concept(Namespace::Library, path)
    }

    fn corpus(path: &str) -> NodeId {
        NodeId::concept(Namespace::Corpus, path)
    }

    /// A library with no connections file yet reads as "none", not as an
    /// error — the mind map asks on every frame.
    #[test]
    fn a_library_with_no_file_has_no_connections() {
        let (_dir, root) = make_root();
        assert!(!root.mindmap_connections().exists());
        assert!(load(&root).is_empty());
    }

    /// The maintainer's own example: a user topic `njoy` linked to the NJOY
    /// entry in the built-in corpus, surviving a round trip through the file.
    #[test]
    fn a_link_round_trips_and_is_visible_from_both_ends() {
        let (_dir, root) = make_root();
        let (from, to) = (mine("njoy"), corpus("nuclear-data/njoy"));
        add(&root, &from, &to).unwrap();

        let all = load(&root);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].source, from);
        assert_eq!(all[0].target, to);

        // Written once, seen from both ends.
        assert_eq!(for_node(&all, &from), vec![&to]);
        assert_eq!(for_node(&all, &to), vec![&from]);
        assert!(for_node(&all, &mine("somewhere-else")).is_empty());
    }

    /// The file is the tracked, readable TOML the decision asked for, not an
    /// opaque cache: check the shape a human would edit.
    #[test]
    fn the_file_is_readable_toml_with_one_entry_per_link() {
        let (_dir, root) = make_root();
        add(&root, &mine("njoy"), &corpus("nuclear-data/njoy")).unwrap();
        let text = std::fs::read_to_string(root.mindmap_connections()).unwrap();
        assert!(text.starts_with("# Kovan mind-map connections."), "{text}");
        assert!(text.contains("[[connection]]"), "{text}");
        assert!(text.contains("source = \"library:concept/njoy\""), "{text}");
        assert!(
            text.contains("target = \"corpus:concept/nuclear-data/njoy\""),
            "{text}"
        );
    }

    /// A link is recorded once. Asking for it again — in either direction —
    /// is refused rather than drawing the same line twice.
    #[test]
    fn a_link_is_never_recorded_twice_or_to_itself() {
        let (_dir, root) = make_root();
        let (a, b) = (mine("njoy"), corpus("nuclear-data/njoy"));
        add(&root, &a, &b).unwrap();

        assert!(matches!(
            add(&root, &a, &b),
            Err(ConnectionsError::Duplicate(..))
        ));
        assert!(
            matches!(add(&root, &b, &a), Err(ConnectionsError::Duplicate(..))),
            "the reverse is derived, so it is the same link"
        );
        assert!(matches!(
            add(&root, &a, &a),
            Err(ConnectionsError::SelfLink(_))
        ));
        assert_eq!(load(&root).len(), 1);
    }

    #[test]
    fn removing_a_link_works_from_either_direction_and_says_when_there_was_none() {
        let (_dir, root) = make_root();
        let (a, b) = (mine("njoy"), corpus("nuclear-data/njoy"));
        add(&root, &a, &b).unwrap();
        add(&root, &a, &mine("other")).unwrap();

        // Named backwards from how it was made, and still found.
        assert!(remove(&root, &b, &a).unwrap());
        assert_eq!(load(&root).len(), 1);
        assert!(!remove(&root, &b, &a).unwrap());
    }

    /// One unparseable row must not hide the rest — the same rule
    /// `index::rebuild` applies to a malformed entity.
    #[test]
    fn a_malformed_entry_is_skipped_and_the_rest_still_load() {
        let (_dir, root) = make_root();
        std::fs::create_dir_all(root.mindmap_dir()).unwrap();
        std::fs::write(
            root.mindmap_connections(),
            "[[connection]]\nsource = \"not a node id\"\ntarget = \"library:concept/njoy\"\n\n\
             [[connection]]\nsource = \"library:concept/njoy\"\ntarget = \"corpus:concept/nuclear-data/njoy\"\n",
        )
        .unwrap();
        let all = load(&root);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].source, mine("njoy"));
    }

    /// A file that is not TOML at all is "no connections", not a panic.
    #[test]
    fn an_unreadable_file_is_no_connections_rather_than_a_failure() {
        let (_dir, root) = make_root();
        std::fs::create_dir_all(root.mindmap_dir()).unwrap();
        std::fs::write(root.mindmap_connections(), "this is not toml {{{").unwrap();
        assert!(load(&root).is_empty());
    }
}
