//! Process groups and group-based communicator creation.
//!
//! An MPI [`Group`] is an **ordered set of processes** — here, an ordered list of
//! world ranks — decoupled from any communication context. Groups are manipulated
//! with purely *local* set operations (no messages): include/exclude a subset,
//! union/intersection/difference two groups, or translate a rank from one group's
//! numbering to another's. A communicator is then created from a group with the
//! collective [`Communicator::create_from_group`] (mirrors `MPI_Comm_create`).
//!
//! This mirrors the MPI-3.1 group API: [`Communicator::group`] ↔ `MPI_Comm_group`,
//! [`Group::incl`]/[`Group::excl`] ↔ `MPI_Group_incl`/`_excl`,
//! [`Group::union`]/[`Group::intersection`]/[`Group::difference`] ↔ the set ops,
//! [`Group::translate_ranks`] ↔ `MPI_Group_translate_ranks`.
//!
//! # Provenance
//!
//! MPI-3.1 group semantics; set operations preserve MPI's ordering rules (union
//! keeps the first group's order then appends the second group's new members;
//! intersection/difference keep the first group's order). Untrusted AI draft,
//! verification-only.

use std::sync::Arc;

use crate::communicator::Communicator;
use crate::error::MpiResult;

/// An ordered set of processes, identified by their **world ranks**.
///
/// Group rank `i` (`0..size`) maps to world rank `ranks()[i]`. World ranks within
/// a group are distinct. A group carries no communication context — build a
/// communicator from it with [`Communicator::create_from_group`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    ranks: Vec<i32>,
}

impl Group {
    /// Build a group directly from an ordered list of world ranks (deduplicated,
    /// order preserved). Used internally and for tests.
    pub(crate) fn from_world_ranks(ranks: Vec<i32>) -> Self {
        let mut seen = Vec::new();
        for r in ranks {
            if !seen.contains(&r) {
                seen.push(r);
            }
        }
        Group { ranks: seen }
    }

    /// The world ranks of this group, in group-rank order.
    pub fn world_ranks(&self) -> &[i32] {
        &self.ranks
    }

    /// Number of processes in the group (mirrors `MPI_Group_size`).
    pub fn size(&self) -> i32 {
        self.ranks.len() as i32
    }

    /// This group's rank of the process with the given world rank, or `None` if it
    /// is not a member (mirrors `MPI_Group_rank` for the caller's own rank).
    pub fn rank_of(&self, world_rank: i32) -> Option<i32> {
        self.ranks
            .iter()
            .position(|&r| r == world_rank)
            .map(|i| i as i32)
    }

    /// Subgroup containing the members at the given **group-rank indices**, in the
    /// order listed (mirrors `MPI_Group_incl`). Out-of-range indices are skipped.
    pub fn incl(&self, indices: &[i32]) -> Group {
        let ranks = indices
            .iter()
            .filter_map(|&i| self.ranks.get(i as usize).copied())
            .collect();
        Group::from_world_ranks(ranks)
    }

    /// Subgroup with the members at the given group-rank indices **removed**,
    /// keeping the original order (mirrors `MPI_Group_excl`).
    pub fn excl(&self, indices: &[i32]) -> Group {
        let ranks = (0..self.ranks.len())
            .filter(|i| !indices.contains(&(*i as i32)))
            .map(|i| self.ranks[i])
            .collect();
        Group::from_world_ranks(ranks)
    }

    /// Union: this group's members, then the other's members not already present
    /// (mirrors `MPI_Group_union`).
    pub fn union(&self, other: &Group) -> Group {
        let mut ranks = self.ranks.clone();
        for &r in &other.ranks {
            if !ranks.contains(&r) {
                ranks.push(r);
            }
        }
        Group::from_world_ranks(ranks)
    }

    /// Intersection: members in both groups, in this group's order (mirrors
    /// `MPI_Group_intersection`).
    pub fn intersection(&self, other: &Group) -> Group {
        let ranks = self
            .ranks
            .iter()
            .filter(|r| other.ranks.contains(r))
            .copied()
            .collect();
        Group::from_world_ranks(ranks)
    }

    /// Difference: members of this group not in the other, in this group's order
    /// (mirrors `MPI_Group_difference`).
    pub fn difference(&self, other: &Group) -> Group {
        let ranks = self
            .ranks
            .iter()
            .filter(|r| !other.ranks.contains(r))
            .copied()
            .collect();
        Group::from_world_ranks(ranks)
    }

    /// Translate each of `ranks` (group ranks in *this* group) into the
    /// corresponding rank in `other`, or `None` where the process is not in
    /// `other` (mirrors `MPI_Group_translate_ranks`).
    pub fn translate_ranks(&self, ranks: &[i32], other: &Group) -> Vec<Option<i32>> {
        ranks
            .iter()
            .map(|&i| {
                self.ranks
                    .get(i as usize)
                    .and_then(|&world| other.rank_of(world))
            })
            .collect()
    }
}

impl Communicator {
    /// The group of this communicator: its members' world ranks in local-rank
    /// order (mirrors `MPI_Comm_group`).
    pub fn group(&self) -> Group {
        Group::from_world_ranks(self.world_ranks_vec())
    }

    /// Collectively create a new communicator over the processes in `group`
    /// (mirrors `MPI_Comm_create`). Every rank of this communicator must call it
    /// with the *same* group; ranks that are members receive the new communicator
    /// (renumbered to the group's order), and non-members receive `Ok(None)`.
    ///
    /// # Errors
    /// Propagates any transport/collective error from the internal context
    /// agreement broadcast.
    pub fn create_from_group(&self, group: &Group) -> MpiResult<Option<Communicator>> {
        // Agree a fresh context across the whole parent communicator (rank 0
        // allocates, broadcasts) so all members land on the same context.
        let ctx = self.agree_new_context()?;

        match group.rank_of(self.world_rank()) {
            Some(new_rank) => Ok(Some(Communicator::new(
                Arc::clone(self.transport()),
                new_rank,
                group.size(),
                ctx,
                self.world_rank(),
                Arc::new(group.world_ranks().to_vec()),
            ))),
            None => Ok(None),
        }
    }
}
