//! Collective communication and reduction operations.
//!
//! These are the collectives a parallel solver leans on — [barrier](Communicator::barrier),
//! [broadcast](Communicator::broadcast), [reduce](Communicator::reduce),
//! [all_reduce](Communicator::all_reduce), [scatter](Communicator::scatter),
//! [gather](Communicator::gather), and [all_gather](Communicator::all_gather) —
//! built on the point-to-point layer. Every rank in the communicator must call
//! the same collective in the same order (the MPI collective-ordering rule); a
//! collective is blocking (it returns once this rank's part is complete).
//!
//! Collective messages travel on a **separate communication context**
//! ([`Communicator::coll_ctx`]) from user point-to-point traffic, so a user
//! `send`/`recv` can never accidentally match a collective's internal message —
//! the same isolation MPI provides with hidden contexts.
//!
//! # Algorithms and provenance
//!
//! [broadcast](Communicator::broadcast) and [reduce](Communicator::reduce) use
//! **binomial trees** (`O(log P)` rounds), translated from MPICH's
//! `MPIR_Bcast_intra_binomial` / `MPIR_Reduce_intra_binomial`.
//! [all_reduce](Communicator::all_reduce) is reduce-then-broadcast (`2 log P`);
//! [all_gather](Communicator::all_gather) is gather-then-broadcast;
//! [scatter](Communicator::scatter), [gather](Communicator::gather), and the
//! [barrier](Communicator::barrier) use linear (`O(P)`) root-centred schedules.
//! MPICH's fully-optimised variants — recursive-doubling all-reduce, ring
//! all-gather, dissemination barrier — are a documented follow-up (epic op-erl);
//! the versions here are correctness-first and give identical results.
//!
//! # Untrusted AI draft
//!
//! Verification-only, per the workspace `RESPONSIBLE_USE.md`; not checked against
//! MPICH's own collective test suite.

use crate::communicator::Communicator;
use crate::datatype::MpiPrimitive;
use crate::error::{MpiError, MpiResult};

/// Internal tags for each collective phase, on the collective context. Distinct
/// per operation so a mis-ordered program fails loudly rather than cross-matching.
const TAG_BARRIER: i32 = 0;
const TAG_BCAST: i32 = 1;
const TAG_REDUCE: i32 = 2;
const TAG_SCATTER: i32 = 3;
const TAG_GATHER: i32 = 4;

/// A predefined reduction operation (mirrors the MPI predefined `MPI_Op`s).
///
/// Applied element-wise by [`Reducible::combine`]. Arithmetic uses the element
/// type's native operators (which wrap on integer overflow in release builds, as
/// everywhere in Rust).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReduceOp {
    /// Sum — `MPI_SUM`.
    Sum,
    /// Product — `MPI_PROD`.
    Product,
    /// Maximum — `MPI_MAX`.
    Max,
    /// Minimum — `MPI_MIN`.
    Min,
}

/// A datatype that supports the predefined [`ReduceOp`] reductions.
///
/// Implemented for the numeric primitives (all ten [`MpiPrimitive`] types). This
/// is a compiler-checked contract used to bound [`Communicator::reduce`] /
/// [`Communicator::all_reduce`]; it is never a `dyn` object.
pub trait Reducible: MpiPrimitive {
    /// Combine two values under `op`.
    fn combine(op: ReduceOp, a: Self, b: Self) -> Self;
}

macro_rules! impl_reducible {
    ($ty:ty) => {
        impl Reducible for $ty {
            fn combine(op: ReduceOp, a: Self, b: Self) -> Self {
                match op {
                    ReduceOp::Sum => a + b,
                    ReduceOp::Product => a * b,
                    ReduceOp::Max => {
                        if a > b {
                            a
                        } else {
                            b
                        }
                    }
                    ReduceOp::Min => {
                        if a < b {
                            a
                        } else {
                            b
                        }
                    }
                }
            }
        }
    };
}

impl_reducible!(i8);
impl_reducible!(i16);
impl_reducible!(i32);
impl_reducible!(i64);
impl_reducible!(u8);
impl_reducible!(u16);
impl_reducible!(u32);
impl_reducible!(u64);
impl_reducible!(f32);
impl_reducible!(f64);

impl Communicator {
    /// Validate a `root` rank against the communicator size.
    fn check_root(&self, root: i32) -> MpiResult<()> {
        if root < 0 || root >= self.size() {
            return Err(MpiError::InvalidRank {
                rank: root,
                size: self.size(),
            });
        }
        Ok(())
    }

    /// Block until every rank in the communicator has called `barrier` (mirrors
    /// `MPI_Barrier`).
    ///
    /// Linear schedule: every non-root rank reports to rank 0, which then releases
    /// them once all have arrived.
    pub fn barrier(&self) -> MpiResult<()> {
        let ctx = self.coll_ctx();
        let size = self.size();
        if self.rank() == 0 {
            for r in 1..size {
                let _ = self.recv_ctx::<u8>(r, TAG_BARRIER, ctx)?;
            }
            for r in 1..size {
                self.send_ctx(&[0u8], r, TAG_BARRIER, ctx)?;
            }
        } else {
            self.send_ctx(&[0u8], 0, TAG_BARRIER, ctx)?;
            let _ = self.recv_ctx::<u8>(0, TAG_BARRIER, ctx)?;
        }
        Ok(())
    }

    /// Broadcast a buffer from `root` to every rank (mirrors `MPI_Bcast`).
    ///
    /// `root` passes `Some(data)`; every other rank passes `None`. All ranks
    /// return the broadcast buffer. Binomial-tree schedule.
    ///
    /// # Errors
    /// [`MpiError::InvalidRank`] for a bad `root`; [`MpiError::InvalidArgument`]
    /// if `root` passed `None` or a non-root passed `Some`.
    pub fn broadcast<T: MpiPrimitive>(&self, data: Option<&[T]>, root: i32) -> MpiResult<Vec<T>> {
        self.check_root(root)?;
        let ctx = self.coll_ctx();
        let size = self.size();
        let rel = (self.rank() - root).rem_euclid(size);

        let mut buf: Vec<T> = if self.rank() == root {
            match data {
                Some(d) => d.to_vec(),
                None => {
                    return Err(MpiError::InvalidArgument(
                        "broadcast: root must pass Some(data)".into(),
                    ))
                }
            }
        } else {
            if data.is_some() {
                return Err(MpiError::InvalidArgument(
                    "broadcast: non-root ranks must pass None".into(),
                ));
            }
            Vec::new()
        };

        // Receive from parent (the round whose bit is the lowest set bit of rel).
        let mut mask = 1;
        while mask < size {
            if rel & mask != 0 {
                let src = (rel - mask + root).rem_euclid(size);
                let (recvd, _status) = self.recv_ctx::<T>(src, TAG_BCAST, ctx)?;
                buf = recvd;
                break;
            }
            mask <<= 1;
        }
        // Forward to children on the lower bits.
        mask >>= 1;
        while mask > 0 {
            let child_rel = rel + mask;
            if child_rel < size {
                let child = (child_rel + root).rem_euclid(size);
                self.send_ctx(&buf, child, TAG_BCAST, ctx)?;
            }
            mask >>= 1;
        }
        Ok(buf)
    }

    /// Element-wise reduce every rank's `sendbuf` onto `root` under `op` (mirrors
    /// `MPI_Reduce`).
    ///
    /// Every rank passes a `sendbuf` of the same length. `root` returns
    /// `Some(result)` (the element-wise combination across all ranks); every other
    /// rank returns `None`. Binomial-tree schedule.
    ///
    /// # Errors
    /// [`MpiError::InvalidRank`] for a bad `root`; [`MpiError::InvalidArgument`]
    /// if a contributing buffer length disagrees with this rank's.
    pub fn reduce<T: Reducible>(
        &self,
        sendbuf: &[T],
        op: ReduceOp,
        root: i32,
    ) -> MpiResult<Option<Vec<T>>> {
        self.check_root(root)?;
        let ctx = self.coll_ctx();
        let size = self.size();
        let rel = (self.rank() - root).rem_euclid(size);
        let mut acc = sendbuf.to_vec();

        let mut mask = 1;
        while mask < size {
            if rel & mask == 0 {
                // Receiver: absorb a child if one exists at this bit.
                let child_rel = rel + mask;
                if child_rel < size {
                    let child = (child_rel + root).rem_euclid(size);
                    let (incoming, _status) = self.recv_ctx::<T>(child, TAG_REDUCE, ctx)?;
                    if incoming.len() != acc.len() {
                        return Err(MpiError::InvalidArgument(format!(
                            "reduce: length mismatch (have {}, got {})",
                            acc.len(),
                            incoming.len()
                        )));
                    }
                    for (a, b) in acc.iter_mut().zip(incoming) {
                        *a = T::combine(op, *a, b);
                    }
                }
                mask <<= 1;
            } else {
                // Sender: hand the partial up to the parent and stop.
                let parent = (rel - mask + root).rem_euclid(size);
                self.send_ctx(&acc, parent, TAG_REDUCE, ctx)?;
                return Ok(None);
            }
        }
        Ok(Some(acc))
    }

    /// Element-wise reduce onto every rank (mirrors `MPI_Allreduce`).
    ///
    /// Implemented as [`reduce`](Communicator::reduce) onto rank 0 followed by a
    /// [`broadcast`](Communicator::broadcast); every rank returns the result.
    ///
    /// # Errors
    /// As [`Communicator::reduce`].
    pub fn all_reduce<T: Reducible>(&self, sendbuf: &[T], op: ReduceOp) -> MpiResult<Vec<T>> {
        let reduced = self.reduce(sendbuf, op, 0)?;
        match reduced {
            Some(r) => self.broadcast(Some(&r), 0),
            None => self.broadcast(None, 0),
        }
    }

    /// Scatter equal-sized `count`-element chunks of `root`'s buffer to each rank
    /// (mirrors `MPI_Scatter`).
    ///
    /// `root` passes `Some(sendbuf)` of length `count * size`; every other rank
    /// passes `None`. Each rank returns its `count`-element chunk (rank `i` gets
    /// `sendbuf[i*count .. (i+1)*count]`). Linear schedule.
    ///
    /// # Errors
    /// [`MpiError::InvalidRank`] for a bad `root`; [`MpiError::InvalidArgument`]
    /// if `root`'s buffer is not exactly `count * size` long or root passed `None`.
    pub fn scatter<T: MpiPrimitive>(
        &self,
        sendbuf: Option<&[T]>,
        count: usize,
        root: i32,
    ) -> MpiResult<Vec<T>> {
        self.check_root(root)?;
        let ctx = self.coll_ctx();
        let size = self.size();
        if self.rank() == root {
            let all = sendbuf.ok_or_else(|| {
                MpiError::InvalidArgument("scatter: root must pass Some(sendbuf)".into())
            })?;
            if all.len() != count * size as usize {
                return Err(MpiError::InvalidArgument(format!(
                    "scatter: root buffer is {} elements, expected count*size = {}",
                    all.len(),
                    count * size as usize
                )));
            }
            let mut mine = Vec::new();
            for r in 0..size {
                let chunk = &all[r as usize * count..(r as usize + 1) * count];
                if r == root {
                    mine = chunk.to_vec();
                } else {
                    self.send_ctx(chunk, r, TAG_SCATTER, ctx)?;
                }
            }
            Ok(mine)
        } else {
            let (chunk, _status) = self.recv_ctx::<T>(root, TAG_SCATTER, ctx)?;
            Ok(chunk)
        }
    }

    /// Gather each rank's equal-length `sendbuf` onto `root`, concatenated in rank
    /// order (mirrors `MPI_Gather`).
    ///
    /// `root` returns `Some(concatenation)`; every other rank returns `None`.
    /// Linear schedule; assumes every rank contributes the same number of elements.
    ///
    /// # Errors
    /// [`MpiError::InvalidRank`] for a bad `root`; [`MpiError::InvalidArgument`]
    /// if a contributed chunk length differs from root's own.
    pub fn gather<T: MpiPrimitive>(&self, sendbuf: &[T], root: i32) -> MpiResult<Option<Vec<T>>> {
        self.check_root(root)?;
        let ctx = self.coll_ctx();
        let size = self.size();
        if self.rank() == root {
            let count = sendbuf.len();
            let mut out: Vec<T> = Vec::with_capacity(count * size as usize);
            for r in 0..size {
                if r == root {
                    out.extend_from_slice(sendbuf);
                } else {
                    let (chunk, _status) = self.recv_ctx::<T>(r, TAG_GATHER, ctx)?;
                    if chunk.len() != count {
                        return Err(MpiError::InvalidArgument(format!(
                            "gather: rank {r} sent {} elements, expected {count}",
                            chunk.len()
                        )));
                    }
                    out.extend_from_slice(&chunk);
                }
            }
            Ok(Some(out))
        } else {
            self.send_ctx(sendbuf, root, TAG_GATHER, ctx)?;
            Ok(None)
        }
    }

    /// Gather every rank's `sendbuf` onto **all** ranks, concatenated in rank order
    /// (mirrors `MPI_Allgather`).
    ///
    /// Implemented as [`gather`](Communicator::gather) onto rank 0 followed by a
    /// [`broadcast`](Communicator::broadcast); every rank returns the full
    /// concatenation.
    ///
    /// # Errors
    /// As [`Communicator::gather`].
    pub fn all_gather<T: MpiPrimitive>(&self, sendbuf: &[T]) -> MpiResult<Vec<T>> {
        let gathered = self.gather(sendbuf, 0)?;
        match gathered {
            Some(g) => self.broadcast(Some(&g), 0),
            None => self.broadcast(None, 0),
        }
    }
}
