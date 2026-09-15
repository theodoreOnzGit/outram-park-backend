//! Spatial acceleration for SCLS retained-history lookup.
//!
//! The anticipated SCLS bottleneck is not the physics but the **lookup**: every flight
//! asks "which remembered inclusion, if any, covers this point?", and a linear scan over
//! the retained set makes that O(N) per query. With the Dynamic Inclusion Sphere holding
//! a few hundred to a few thousand histories, that scan can dominate runtime.
//!
//! This module puts the query behind one abstraction so a faster backend can be swapped
//! in and measured without touching the transport code.
//!
//! # Design-doc deviations (workspace rules take precedence)
//!
//! Design doc §16 specifies `pub trait SpatialIndex` with brute-force / KD-tree / R-tree
//! backends, naming the `kiddo` and `rstar` crates. Three workspace rules reshape that:
//!
//! 1. **No trait-object dispatch** — the backend set is closed and known at compile
//!    time, so this is the enum [`SpatialIndex`], dispatched by `match`.
//! 2. **Dependency policy** — third-party versions live only in the root
//!    `[workspace.dependencies]`. Adding `kiddo`/`rstar` is a workspace-level decision,
//!    not something a leaf module does on its own.
//! 3. **Android/Termux portability** — every non-GUI library must build natively on
//!    Termux, so any new dependency must be checked there before adoption.
//!
//! So no third-party tree dependency is added. [`SpatialIndex::BruteForce`] is the
//! correctness baseline; [`SpatialIndex::KdTree`] is a **dependency-free, index-based
//! kd-tree** ([`KdTreeIndex`]) written to the workspace rules (no `Box`; tree links are
//! `u32` indices into a flat node `Vec`), so the acceleration seam is real and its
//! answers are unit-tested to match brute force exactly. [`SpatialIndex::RTree`] remains
//! the honest "declared but not built" variant — adopting `rstar` is a workspace-level
//! dependency + Termux-portability decision, not a leaf-module one. Brute force stays the
//! baseline the trees must beat in the benchmark. Tracked as bead `op-eby.5`.
//!
//! # Note on the existing grid
//!
//! [`crate::pebble_beds::sphere_packing::PackedSpheres`] already carries a uniform spatial hash
//! grid for its RSA overlap test. That grid is tuned for a *static, equal-radius*
//! packing; SCLS needs an index over a *churning* history set that is rebuilt as the
//! inclusion sphere moves. Reusing versus rebuilding it is an open question the
//! benchmark should settle rather than a decision to make up front.

use crate::geometry::position::Position;
use crate::stochastic::scls::ParticleHistory;

/// Errors from a spatial-index query.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum IndexError {
    /// The backend is declared but not built out (see the module docs).
    #[error("spatial-index backend '{0}' is declared but not implemented yet")]
    BackendNotImplemented(&'static str),
}

/// Linear-scan index — the correctness baseline.
///
/// O(N) per query and O(N) memory, with no dependency and no build step. Every faster
/// backend must reproduce its answers exactly, so it doubles as the test oracle.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BruteForceIndex {
    histories: Vec<ParticleHistory>,
}

impl BruteForceIndex {
    /// An empty index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Build an index over an existing history set.
    pub fn from_histories(histories: Vec<ParticleHistory>) -> Self {
        Self { histories }
    }

    /// Add one remembered inclusion.
    pub fn insert(&mut self, history: ParticleHistory) {
        self.histories.push(history);
    }

    /// Drop every indexed history (called when the inclusion sphere is rebuilt).
    pub fn clear(&mut self) {
        self.histories.clear();
    }

    /// How many histories are indexed.
    pub fn len(&self) -> usize {
        self.histories.len()
    }

    /// Whether the index is empty.
    pub fn is_empty(&self) -> bool {
        self.histories.is_empty()
    }

    /// The indexed histories.
    pub fn histories(&self) -> &[ParticleHistory] {
        &self.histories
    }

    /// The first indexed inclusion containing `p` \[cm\], if any.
    ///
    /// "First" is well defined only because a valid packing is non-overlapping — at most
    /// one inclusion can contain a point. If two ever match, the packing is invalid.
    pub fn find_containing(&self, p: Position) -> Option<&ParticleHistory> {
        self.histories.iter().find(|h| h.contains(p))
    }

    /// Indices of every inclusion whose *centre* lies within `radius` \[cm\] of `p`.
    ///
    /// Centre-distance, not body-overlap — this is the neighbourhood query a chord
    /// sampler uses to find candidate inclusions along a ray.
    pub fn query_within(&self, p: Position, radius: f64) -> Vec<usize> {
        let r2 = radius * radius;
        self.histories
            .iter()
            .enumerate()
            .filter(|(_, h)| {
                let dx = h.center.x - p.x;
                let dy = h.center.y - p.y;
                let dz = h.center.z - p.z;
                dx * dx + dy * dy + dz * dz < r2
            })
            .map(|(i, _)| i)
            .collect()
    }
}

/// A static 3-D kd-tree over inclusion centres — an accelerated alternative to the
/// linear scan for the retained-history lookup.
///
/// The tree is stored as a flat `Vec` of nodes referencing their children by index
/// (`u32::MAX` = none), not with `Box` pointers — the workspace forbids `Box` and models
/// tree links as indices (the `CellId(usize)` pattern). It is **static**: built once from
/// a history snapshot with [`Self::build`], and rebuilt (not incrementally updated) when
/// the inclusion sphere churns the retained set. Whether periodic rebuilds beat the
/// brute-force scan for the few-hundred-history working set is exactly what the benchmark
/// (`op-eby.7`) can now measure, since both backends are real and return identical
/// answers.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KdTreeIndex {
    histories: Vec<ParticleHistory>,
    nodes: Vec<KdNode>,
    root: u32,
    /// Largest inclusion radius in the set — the search radius [`Self::find_containing`]
    /// must widen the centre query by, so a sphere whose centre is up to `R` away can
    /// still be found containing the point.
    max_radius: f64,
}

/// One kd-tree node: a splitting inclusion plus its two child sub-trees (by index).
#[derive(Debug, Clone, Copy, PartialEq)]
struct KdNode {
    /// Index into [`KdTreeIndex::histories`] of the inclusion at this node.
    hist: u32,
    /// Split axis: 0 = x, 1 = y, 2 = z.
    axis: u8,
    /// Left child node index (`u32::MAX` = none).
    left: u32,
    /// Right child node index (`u32::MAX` = none).
    right: u32,
}

const NIL: u32 = u32::MAX;

impl KdTreeIndex {
    /// Build a balanced kd-tree over `histories`. O(N log N) via median partitioning on
    /// cycling axes.
    pub fn build(histories: Vec<ParticleHistory>) -> Self {
        let max_radius = histories.iter().map(|h| h.radius).fold(0.0_f64, f64::max);
        let mut idx: Vec<u32> = (0..histories.len() as u32).collect();
        let mut nodes: Vec<KdNode> = Vec::with_capacity(histories.len());
        let root = build_subtree(&histories, &mut idx, &mut nodes, 0);
        Self {
            histories,
            nodes,
            root,
            max_radius,
        }
    }

    /// How many inclusions are indexed.
    pub fn len(&self) -> usize {
        self.histories.len()
    }

    /// Whether the tree is empty.
    pub fn is_empty(&self) -> bool {
        self.histories.is_empty()
    }

    fn coord(p: Position, axis: u8) -> f64 {
        match axis {
            0 => p.x,
            1 => p.y,
            _ => p.z,
        }
    }

    /// The inclusion containing `p`, if any — a range search over centres within
    /// `max_radius` of `p`, then an exact containment test.
    pub fn find_containing(&self, p: Position) -> Option<&ParticleHistory> {
        let mut found: Option<usize> = None;
        self.range_visit(self.root, p, self.max_radius, &mut |h_idx| {
            if self.histories[h_idx].contains(p) {
                found = Some(h_idx);
                true // stop: a valid packing has at most one container
            } else {
                false
            }
        });
        found.map(|i| &self.histories[i])
    }

    /// Indices (into the original history order) of every inclusion whose *centre* lies
    /// within `radius` of `p`.
    pub fn query_within(&self, p: Position, radius: f64) -> Vec<usize> {
        let r2 = radius * radius;
        let mut out = Vec::new();
        self.range_visit(self.root, p, radius, &mut |h_idx| {
            let c = self.histories[h_idx].center;
            let dx = c.x - p.x;
            let dy = c.y - p.y;
            let dz = c.z - p.z;
            if dx * dx + dy * dy + dz * dz < r2 {
                out.push(h_idx);
            }
            false
        });
        out
    }

    /// Visit every node whose centre could lie within `radius` of `p`, pruning subtrees
    /// on the wrong side of a split plane by more than `radius`. `visit` returns `true`
    /// to stop the search early.
    fn range_visit(
        &self,
        node: u32,
        p: Position,
        radius: f64,
        visit: &mut impl FnMut(usize) -> bool,
    ) -> bool {
        if node == NIL {
            return false;
        }
        let n = self.nodes[node as usize];
        let center = self.histories[n.hist as usize].center;
        if visit(n.hist as usize) {
            return true;
        }
        let delta = Self::coord(p, n.axis) - Self::coord(center, n.axis);
        // Near side first, then the far side only if the plane is within `radius`.
        let (near, far) = if delta < 0.0 {
            (n.left, n.right)
        } else {
            (n.right, n.left)
        };
        if self.range_visit(near, p, radius, visit) {
            return true;
        }
        if delta.abs() <= radius && self.range_visit(far, p, radius, visit) {
            return true;
        }
        false
    }
}

/// Recursively build a balanced kd-subtree from the index slice `idx`, splitting on the
/// median of the cycling `depth % 3` axis. Returns the new node's index, or `NIL` for an
/// empty slice.
fn build_subtree(
    histories: &[ParticleHistory],
    idx: &mut [u32],
    nodes: &mut Vec<KdNode>,
    depth: usize,
) -> u32 {
    if idx.is_empty() {
        return NIL;
    }
    let axis = (depth % 3) as u8;
    idx.sort_by(|&a, &b| {
        let ca = KdTreeIndex::coord(histories[a as usize].center, axis);
        let cb = KdTreeIndex::coord(histories[b as usize].center, axis);
        ca.partial_cmp(&cb).unwrap_or(std::cmp::Ordering::Equal)
    });
    let mid = idx.len() / 2;
    let hist = idx[mid];
    // Reserve this node's slot before recursing so child indices are stable.
    let node_index = nodes.len() as u32;
    nodes.push(KdNode {
        hist,
        axis,
        left: NIL,
        right: NIL,
    });
    let (left_idx, right_slice) = idx.split_at_mut(mid);
    let right_idx = &mut right_slice[1..]; // skip the median
    let left = build_subtree(histories, left_idx, nodes, depth + 1);
    let right = build_subtree(histories, right_idx, nodes, depth + 1);
    nodes[node_index as usize].left = left;
    nodes[node_index as usize].right = right;
    node_index
}

/// A spatial-index backend, dispatched by `match` (no trait objects — see module docs).
#[derive(Debug, Clone, PartialEq)]
pub enum SpatialIndex {
    /// Linear scan. Implemented; the correctness baseline.
    BruteForce(BruteForceIndex),
    /// Static kd-tree over inclusion centres (dependency-free, index-based). Implemented;
    /// returns answers identical to [`Self::BruteForce`].
    KdTree(KdTreeIndex),
    /// R-tree (design doc suggests `rstar`). **Not implemented** — adding the crate is a
    /// workspace-level dependency + Termux-portability decision, kept as the honest
    /// "declared but not built" seam.
    RTree,
}

impl SpatialIndex {
    /// A brute-force index over `histories`.
    pub fn brute_force(histories: Vec<ParticleHistory>) -> Self {
        Self::BruteForce(BruteForceIndex::from_histories(histories))
    }

    /// A kd-tree index over `histories` (dependency-free, built in O(N log N)).
    pub fn kd_tree(histories: Vec<ParticleHistory>) -> Self {
        Self::KdTree(KdTreeIndex::build(histories))
    }

    /// Short backend name, for benchmark tables and error messages.
    pub fn name(&self) -> &'static str {
        match self {
            Self::BruteForce(_) => "brute-force",
            Self::KdTree(_) => "kd-tree",
            Self::RTree => "r-tree",
        }
    }

    /// The inclusion containing `p` \[cm\], if any.
    ///
    /// # Errors
    /// [`IndexError::BackendNotImplemented`] for the not-yet-built [`Self::RTree`] backend.
    pub fn find_containing(&self, p: Position) -> Result<Option<&ParticleHistory>, IndexError> {
        match self {
            Self::BruteForce(idx) => Ok(idx.find_containing(p)),
            Self::KdTree(idx) => Ok(idx.find_containing(p)),
            Self::RTree => Err(IndexError::BackendNotImplemented("r-tree")),
        }
    }

    /// Indices of inclusions whose centre is within `radius` \[cm\] of `p`.
    ///
    /// # Errors
    /// [`IndexError::BackendNotImplemented`] for the not-yet-built [`Self::RTree`] backend.
    pub fn query_within(&self, p: Position, radius: f64) -> Result<Vec<usize>, IndexError> {
        match self {
            Self::BruteForce(idx) => Ok(idx.query_within(p, radius)),
            Self::KdTree(idx) => Ok(idx.query_within(p, radius)),
            Self::RTree => Err(IndexError::BackendNotImplemented("r-tree")),
        }
    }

    /// How many histories are indexed (0 for the unimplemented [`Self::RTree`] backend).
    pub fn len(&self) -> usize {
        match self {
            Self::BruteForce(idx) => idx.len(),
            Self::KdTree(idx) => idx.len(),
            Self::RTree => 0,
        }
    }

    /// Whether the index holds no histories.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stochastic::medium::MaterialId;

    fn hist(x: f64, r: f64) -> ParticleHistory {
        ParticleHistory::new(Position::new(x, 0.0, 0.0), r, MaterialId(1))
    }

    /// Containment finds the inclusion covering a point, and nothing for a gap.
    #[test]
    fn brute_force_finds_containing_inclusion() {
        let idx = BruteForceIndex::from_histories(vec![hist(0.0, 0.1), hist(1.0, 0.1)]);

        let found = idx.find_containing(Position::new(1.02, 0.0, 0.0));
        assert_eq!(found.map(|h| h.center.x), Some(1.0));

        // A point in the matrix gap between the two inclusions.
        assert!(idx.find_containing(Position::new(0.5, 0.0, 0.0)).is_none());
    }

    /// The neighbourhood query returns exactly the centres inside the radius.
    #[test]
    fn brute_force_neighbourhood_query_is_exclusive_of_the_radius() {
        let idx =
            BruteForceIndex::from_histories(vec![hist(0.0, 0.1), hist(1.0, 0.1), hist(5.0, 0.1)]);

        let near = idx.query_within(Position::new(0.0, 0.0, 0.0), 2.0);
        assert_eq!(near, vec![0, 1]);

        // Radius is a strict bound: a centre exactly on it is excluded.
        let exact = idx.query_within(Position::new(0.0, 0.0, 0.0), 1.0);
        assert_eq!(exact, vec![0]);

        assert!(idx
            .query_within(Position::new(100.0, 0.0, 0.0), 1.0)
            .is_empty());
    }

    /// The implemented backends answer; the not-yet-built R-tree reports its absence
    /// honestly rather than silently returning "nothing found".
    #[test]
    fn implemented_backends_answer_and_rtree_errors() {
        let p = Position::new(0.0, 0.0, 0.0);

        for backend in [
            SpatialIndex::brute_force(vec![hist(0.0, 0.1)]),
            SpatialIndex::kd_tree(vec![hist(0.0, 0.1)]),
        ] {
            assert_eq!(backend.len(), 1);
            assert!(backend.find_containing(p).expect("implemented").is_some());
        }
        assert_eq!(SpatialIndex::brute_force(vec![]).name(), "brute-force");
        assert_eq!(SpatialIndex::kd_tree(vec![]).name(), "kd-tree");

        let rtree = SpatialIndex::RTree;
        assert!(matches!(
            rtree.find_containing(p),
            Err(IndexError::BackendNotImplemented(_))
        ));
        assert!(matches!(
            rtree.query_within(p, 1.0),
            Err(IndexError::BackendNotImplemented(_))
        ));
    }

    /// The kd-tree returns exactly the same answers as the brute-force baseline — the
    /// tree is an acceleration, not a different result. Checks containment across a grid
    /// of query points and neighbourhood queries against a spread of inclusions.
    #[test]
    fn kd_tree_matches_brute_force() {
        // A spread of inclusions on a jittered lattice.
        let mut hs = Vec::new();
        for i in 0..6 {
            for j in 0..6 {
                let x = i as f64 * 0.3 + (j as f64) * 0.01;
                let y = j as f64 * 0.3 - (i as f64) * 0.01;
                hs.push(ParticleHistory::new(
                    Position::new(x, y, 0.0),
                    0.1,
                    MaterialId(1),
                ));
            }
        }
        let bf = BruteForceIndex::from_histories(hs.clone());
        let kd = KdTreeIndex::build(hs.clone());
        assert_eq!(bf.len(), kd.len());

        for gx in 0..20 {
            for gy in 0..20 {
                let p = Position::new(gx as f64 * 0.09, gy as f64 * 0.09, 0.0);
                // Containment agrees (compare the found centre, if any).
                let a = bf.find_containing(p).map(|h| h.center);
                let b = kd.find_containing(p).map(|h| h.center);
                assert_eq!(a, b, "find_containing disagreement at {p:?}");
                // Neighbourhood membership agrees (as a set).
                let mut na = bf.query_within(p, 0.4);
                let mut nb = kd.query_within(p, 0.4);
                na.sort_unstable();
                nb.sort_unstable();
                assert_eq!(na, nb, "query_within disagreement at {p:?}");
            }
        }
    }

    /// Insert/clear maintain the indexed set.
    #[test]
    fn insert_and_clear_maintain_the_set() {
        let mut idx = BruteForceIndex::new();
        assert!(idx.is_empty());
        idx.insert(hist(0.0, 0.1));
        idx.insert(hist(1.0, 0.1));
        assert_eq!(idx.len(), 2);
        idx.clear();
        assert!(idx.is_empty());
    }
}
