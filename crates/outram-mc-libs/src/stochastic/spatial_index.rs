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
//! So no tree dependency is added yet. [`SpatialIndex::BruteForce`] is implemented and
//! correct; [`SpatialIndex::KdTree`] and [`SpatialIndex::RTree`] are declared but return
//! [`IndexError::BackendNotImplemented`]. That keeps the seam real — and the eventual
//! benchmark honest, since brute force is the baseline the trees must beat — without
//! committing the workspace to a dependency nobody has profiled. Tracked as bead
//! `op-eby.5`.
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

/// A spatial-index backend, dispatched by `match` (no trait objects — see module docs).
#[derive(Debug, Clone, PartialEq)]
pub enum SpatialIndex {
    /// Linear scan. Implemented; the correctness baseline.
    BruteForce(BruteForceIndex),
    /// KD-tree (design doc suggests `kiddo`). **Not implemented** — no dependency added
    /// yet, pending a workspace-level dependency + Termux-portability decision.
    KdTree,
    /// R-tree (design doc suggests `rstar`). **Not implemented** — same reason.
    RTree,
}

impl SpatialIndex {
    /// A brute-force index over `histories`.
    pub fn brute_force(histories: Vec<ParticleHistory>) -> Self {
        Self::BruteForce(BruteForceIndex::from_histories(histories))
    }

    /// Short backend name, for benchmark tables and error messages.
    pub fn name(&self) -> &'static str {
        match self {
            Self::BruteForce(_) => "brute-force",
            Self::KdTree => "kd-tree",
            Self::RTree => "r-tree",
        }
    }

    /// The inclusion containing `p` \[cm\], if any.
    ///
    /// # Errors
    /// [`IndexError::BackendNotImplemented`] for the tree backends.
    pub fn find_containing(&self, p: Position) -> Result<Option<&ParticleHistory>, IndexError> {
        match self {
            Self::BruteForce(idx) => Ok(idx.find_containing(p)),
            Self::KdTree => Err(IndexError::BackendNotImplemented("kd-tree")),
            Self::RTree => Err(IndexError::BackendNotImplemented("r-tree")),
        }
    }

    /// Indices of inclusions whose centre is within `radius` \[cm\] of `p`.
    ///
    /// # Errors
    /// [`IndexError::BackendNotImplemented`] for the tree backends.
    pub fn query_within(&self, p: Position, radius: f64) -> Result<Vec<usize>, IndexError> {
        match self {
            Self::BruteForce(idx) => Ok(idx.query_within(p, radius)),
            Self::KdTree => Err(IndexError::BackendNotImplemented("kd-tree")),
            Self::RTree => Err(IndexError::BackendNotImplemented("r-tree")),
        }
    }

    /// How many histories are indexed (0 for unimplemented backends).
    pub fn len(&self) -> usize {
        match self {
            Self::BruteForce(idx) => idx.len(),
            Self::KdTree | Self::RTree => 0,
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

    /// The enum delegates to brute force and reports the tree backends honestly rather
    /// than silently returning "nothing found".
    #[test]
    fn unimplemented_backends_error_instead_of_returning_empty() {
        let p = Position::new(0.0, 0.0, 0.0);

        let bf = SpatialIndex::brute_force(vec![hist(0.0, 0.1)]);
        assert_eq!(bf.name(), "brute-force");
        assert_eq!(bf.len(), 1);
        assert!(bf.find_containing(p).expect("brute force works").is_some());

        for backend in [SpatialIndex::KdTree, SpatialIndex::RTree] {
            assert!(matches!(
                backend.find_containing(p),
                Err(IndexError::BackendNotImplemented(_))
            ));
            assert!(matches!(
                backend.query_within(p, 1.0),
                Err(IndexError::BackendNotImplemented(_))
            ));
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
