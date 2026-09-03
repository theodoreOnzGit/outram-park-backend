//! Communicator management — duplication and splitting.
//!
//! These are the collective communicator-creation calls: every rank of the parent
//! communicator participates, and the result is a *new* communicator with its own
//! isolated communication context.
//!
//! - [`Communicator::dup`] mirrors `MPI_Comm_dup`: same group, fresh context (so a
//!   library can message internally without colliding with the caller's tags).
//! - [`Communicator::split`] mirrors `MPI_Comm_split`: partition the ranks by
//!   `color` into disjoint sub-communicators, ordered within each by `key`
//!   (ties broken by old rank). A rank passing [`Communicator::UNDEFINED`] as its
//!   color joins no group and gets `None`.
//!
//! # How the group agrees on a context
//!
//! A fresh context id must be process-unique *and* identical across every rank of
//! the new group. One rank ([`allocates`](crate::transport) via the transport's
//! atomic counter) and the value is distributed with a [broadcast](Communicator::broadcast)
//! (`dup`) or a small broadcast map keyed by color (`split`), so the whole group
//! ends up on the same context. Sub-communicator ranks are renumbered `0..k`, and
//! the handle records the local→global mailbox mapping so point-to-point and
//! collective addressing stay correct.
//!
//! # Provenance
//!
//! `MPI_Comm_dup` / `MPI_Comm_split` semantics per the MPI-3.1 standard; the
//! all-gather-then-local-recompute strategy for `split` mirrors MPICH's
//! `MPIR_Comm_split_impl`. Untrusted AI draft, verification-only.

use std::sync::Arc;

use crate::communicator::Communicator;
use crate::error::{MpiError, MpiResult};

impl Communicator {
    /// Sentinel color for [`Communicator::split`] meaning "this rank joins no
    /// sub-communicator" (mirrors `MPI_UNDEFINED`); such a rank receives `None`.
    pub const UNDEFINED: i32 = i32::MIN + 1;

    /// Duplicate this communicator: a new communicator over the **same group of
    /// ranks** with a fresh, isolated communication context (mirrors
    /// `MPI_Comm_dup`).
    ///
    /// Collective: every rank must call it. Messages on the duplicate never match
    /// messages on the original, even with identical tags.
    ///
    /// # Errors
    /// Propagates any transport/collective error from the internal broadcast.
    pub fn dup(&self) -> MpiResult<Communicator> {
        // One rank allocates the fresh context; broadcast so all agree.
        let ctx = self.agree_new_context()?;
        Ok(Communicator::new(
            Arc::clone(self.transport()),
            self.rank(),
            self.size(),
            ctx,
            self.world_rank(),
            self.world_ranks_arc(),
        ))
    }

    /// Split this communicator into disjoint sub-communicators by `color`, ordered
    /// within each group by `key` (ties broken by ascending original rank),
    /// mirroring `MPI_Comm_split`.
    ///
    /// Every rank passes a `color` and a `key`. Ranks sharing a `color` form one
    /// new communicator; a rank passing [`Communicator::UNDEFINED`] joins none and
    /// gets `Ok(None)`. Collective: every rank must call it.
    ///
    /// # Errors
    /// Propagates any transport/collective error from the internal all-gather /
    /// broadcast.
    pub fn split(&self, color: i32, key: i32) -> MpiResult<Option<Communicator>> {
        let size = self.size();

        // 1) Everyone learns everyone's (color, key, original rank).
        let mine = [color, key, self.rank()];
        let all = self.all_gather(&mine)?; // 3 * size, in rank order

        // 2) Rank 0 assigns a fresh context id to each distinct non-UNDEFINED
        //    color (sorted) and broadcasts the [n, color0, ctx0, color1, ctx1, …]
        //    map so every rank resolves its color to the same context.
        let map: Vec<i64> = if self.rank() == 0 {
            let mut colors: Vec<i32> = (0..size)
                .map(|r| all[3 * r as usize])
                .filter(|&c| c != Self::UNDEFINED)
                .collect();
            colors.sort_unstable();
            colors.dedup();
            let mut buf = vec![colors.len() as i64];
            for c in colors {
                buf.push(c as i64);
                buf.push(self.alloc_context() as i64);
            }
            self.broadcast(Some(&buf), 0)?
        } else {
            self.broadcast(None, 0)?
        };

        // 3) A rank with no color joins no communicator.
        if color == Self::UNDEFINED {
            return Ok(None);
        }

        // 4) Resolve my color to its context id.
        let n = map[0] as usize;
        let mut ctx = None;
        for i in 0..n {
            if map[1 + 2 * i] as i32 == color {
                ctx = Some(map[2 + 2 * i] as u64);
                break;
            }
        }
        let ctx = ctx.ok_or_else(|| {
            MpiError::Transport("split: color missing from the broadcast context map".into())
        })?;

        // 5) Build my new group: all ranks of my color, ordered by (key, old rank).
        //    That ordering fixes both my new rank and the local→global mapping.
        let mut members: Vec<(i32, i32)> = (0..size)
            .filter(|&r| all[3 * r as usize] == color)
            .map(|r| (all[3 * r as usize + 1], all[3 * r as usize + 2]))
            .collect();
        members.sort_unstable(); // by key, then original rank
        let world_ranks: Vec<i32> = members.iter().map(|&(_, r)| r).collect();
        let new_size = world_ranks.len() as i32;
        let new_rank = world_ranks
            .iter()
            .position(|&r| r == self.rank())
            .expect("this rank is a member of its own color group") as i32;

        Ok(Some(Communicator::new(
            Arc::clone(self.transport()),
            new_rank,
            new_size,
            ctx,
            self.world_rank(),
            Arc::new(world_ranks),
        )))
    }

    /// Allocate a fresh context and broadcast it from rank 0 so the whole group
    /// agrees (used by [`dup`](Communicator::dup) and
    /// [`create_from_group`](Communicator::create_from_group)).
    pub(crate) fn agree_new_context(&self) -> MpiResult<u64> {
        let agreed = if self.rank() == 0 {
            let id = self.alloc_context() as i64;
            self.broadcast(Some(&[id]), 0)?
        } else {
            self.broadcast(None, 0)?
        };
        Ok(agreed[0] as u64)
    }
}
