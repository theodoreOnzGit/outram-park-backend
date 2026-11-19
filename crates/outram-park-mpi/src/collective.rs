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
//! ~~[all_reduce](Communicator::all_reduce) is reduce-then-broadcast (`2 log P`);~~
//! **CORRECTED 2026-11-13 (gh:#1435):** [all_reduce](Communicator::all_reduce) is
//! MPICH's recursive doubling (`MPIR_Allreduce_intra_recursive_doubling`), and
//! [reduce](Communicator::reduce) switches, as MPICH does above 2048 bytes with at least
//! `pof2` elements, to Rabenseifner's reduce-scatter + gather
//! (`MPIR_Reduce_intra_reduce_scatter_gather`). The earlier schedules associated
//! floating-point sums differently from MPICH (5 ranks and above, and every non-zero
//! root for long messages), so the last bit differed; see
//! `verification_and_validation/mpich_differential/README.md`.
//! [all_gather](Communicator::all_gather) is gather-then-broadcast;
//! [scatter](Communicator::scatter), [gather](Communicator::gather), and the
//! [barrier](Communicator::barrier) use linear (`O(P)`) root-centred schedules.
//! ~~MPICH's fully-optimised variants — recursive-doubling all-reduce, ring
//! all-gather, dissemination barrier — are a documented follow-up (epic op-erl);
//! the versions here are correctness-first and give identical results.~~ The ring
//! all-gather, dissemination barrier and the other MPICH algorithms (Brucks,
//! scatter-allgather broadcast, ...) are not translated; they move data or carry no
//! data, so they cannot change a result, only the message pattern.
//!
//! # Upstream version
//!
//! The MPICH version the original translation was written from was not recorded and
//! cannot be determined (`NOTICE`, "UPSTREAM VERSION"): the routines exist, with the
//! same algorithm, in every tag from v3.3 to v4.3.2. The algorithms translated on
//! 2026-11-13 were read from v4.2.0.
//!
//! # Untrusted AI draft
//!
//! Verification-only, per the workspace `RESPONSIBLE_USE.md`. Compared bit for bit
//! against MPICH 4.2.0 (not against MPICH's own collective test suite):
//! `tests/mpich_differential.rs`.

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
const TAG_ALLREDUCE: i32 = 5;

/// MPICH's `MPIR_CVAR_REDUCE_SHORT_MSG_SIZE` default (bytes): at or below it `reduce`
/// uses the binomial tree, above it (with at least `pof2` elements) Rabenseifner's
/// reduce-scatter + gather.
const REDUCE_SHORT_MSG_SIZE: usize = 2048;

/// The largest power of two that is `<= n` (MPICH's `MPL_pof2`), for `n >= 1`.
fn largest_power_of_two_le(n: i32) -> i32 {
    let mut p = 1;
    while p * 2 <= n {
        p *= 2;
    }
    p
}

/// `acc[i] = combine(op, acc[i], incoming[i])` for every element: the accumulator is
/// the first operand, as in MPICH's `MPIR_Reduce_local(in = incoming, inout = acc)`,
/// whose built-in ops compute `op(inout, in)`. The order is observable on ties
/// (`+0.0` against `-0.0`) and NaN for `Max`/`Min`.
///
/// # Errors
/// [`MpiError::InvalidArgument`] if the lengths differ.
fn combine_into<T: Reducible>(op: ReduceOp, acc: &mut [T], incoming: &[T]) -> MpiResult<()> {
    if incoming.len() != acc.len() {
        return Err(MpiError::InvalidArgument(format!(
            "reduce: length mismatch (have {}, got {})",
            acc.len(),
            incoming.len()
        )));
    }
    for (a, b) in acc.iter_mut().zip(incoming.iter()) {
        *a = T::combine(op, *a, *b);
    }
    Ok(())
}

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

/// Shared reduction body for every numeric primitive (same arithmetic the
/// per-type impls below delegate to, so overflow behaviour is unchanged).
fn combine_ordered<T>(op: ReduceOp, a: T, b: T) -> T
where
    T: PartialOrd + core::ops::Add<Output = T> + core::ops::Mul<Output = T>,
{
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

impl Reducible for i8 {
    fn combine(op: ReduceOp, a: Self, b: Self) -> Self {
        combine_ordered(op, a, b)
    }
}

impl Reducible for i16 {
    fn combine(op: ReduceOp, a: Self, b: Self) -> Self {
        combine_ordered(op, a, b)
    }
}

impl Reducible for i32 {
    fn combine(op: ReduceOp, a: Self, b: Self) -> Self {
        combine_ordered(op, a, b)
    }
}

impl Reducible for i64 {
    fn combine(op: ReduceOp, a: Self, b: Self) -> Self {
        combine_ordered(op, a, b)
    }
}

impl Reducible for u8 {
    fn combine(op: ReduceOp, a: Self, b: Self) -> Self {
        combine_ordered(op, a, b)
    }
}

impl Reducible for u16 {
    fn combine(op: ReduceOp, a: Self, b: Self) -> Self {
        combine_ordered(op, a, b)
    }
}

impl Reducible for u32 {
    fn combine(op: ReduceOp, a: Self, b: Self) -> Self {
        combine_ordered(op, a, b)
    }
}

impl Reducible for u64 {
    fn combine(op: ReduceOp, a: Self, b: Self) -> Self {
        combine_ordered(op, a, b)
    }
}

impl Reducible for f32 {
    fn combine(op: ReduceOp, a: Self, b: Self) -> Self {
        combine_ordered(op, a, b)
    }
}

impl Reducible for f64 {
    fn combine(op: ReduceOp, a: Self, b: Self) -> Self {
        combine_ordered(op, a, b)
    }
}

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
        // MPICH's selection (`MPIR_Reduce_intra_auto`, the 2 KiB `REDUCE_SHORT_MSG_SIZE`
        // default): long messages with at least `pof2` elements go to Rabenseifner's
        // reduce-scatter + gather, everything else to the binomial tree. Measured on
        // MPICH 4.2.0: 256 doubles take the binomial, 257 the reduce-scatter-gather.
        let pof2 = largest_power_of_two_le(self.size()) as usize;
        let bytes = sendbuf.len() * T::DATATYPE.size();
        if bytes > REDUCE_SHORT_MSG_SIZE && sendbuf.len() >= pof2 {
            return self.reduce_scatter_gather(sendbuf, op, root);
        }
        self.reduce_binomial(sendbuf, op, root)
    }

    /// Binomial-tree reduce onto `root`: a translation of MPICH's
    /// `MPIR_Reduce_intra_binomial` for a commutative op (see the module header).
    fn reduce_binomial<T: Reducible>(
        &self,
        sendbuf: &[T],
        op: ReduceOp,
        root: i32,
    ) -> MpiResult<Option<Vec<T>>> {
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

    /// Rabenseifner's reduce (recursive-halving reduce-scatter, then a binomial gather
    /// to `root`): a translation of MPICH's `MPIR_Reduce_intra_reduce_scatter_gather`
    /// (`src/mpi/coll/reduce/reduce_intra_reduce_scatter_gather.c`, v4.2.0) for a
    /// predefined (commutative) op. Requires `sendbuf.len() >= pof2`.
    ///
    /// The association order of every element is the fold-then-butterfly of the
    /// reduce-scatter and does **not** depend on `root`, which is why floating-point
    /// sums of long messages differ from the rooted binomial tree.
    fn reduce_scatter_gather<T: Reducible>(
        &self,
        sendbuf: &[T],
        op: ReduceOp,
        root: i32,
    ) -> MpiResult<Option<Vec<T>>> {
        let ctx = self.coll_ctx();
        let size = self.size();
        let rank = self.rank();
        let count = sendbuf.len();
        let mut recvbuf = sendbuf.to_vec();

        let pof2 = largest_power_of_two_le(size);
        let rem = size - pof2;
        let p = pof2 as usize;

        // Fold the odd ranks below 2*rem onto their left neighbours.
        let mut newrank: i32;
        if rank < 2 * rem {
            if rank % 2 != 0 {
                self.send_ctx(&recvbuf, rank - 1, TAG_REDUCE, ctx)?;
                newrank = -1;
            } else {
                let (tmp, _status) = self.recv_ctx::<T>(rank + 1, TAG_REDUCE, ctx)?;
                combine_into(op, &mut recvbuf, &tmp)?;
                newrank = rank / 2;
            }
        } else {
            newrank = rank - rem;
        }

        // Segment sizes and displacements (computed on every rank: an excluded root
        // needs them later, as in upstream).
        let mut cnts = vec![count / p; p];
        for c in cnts.iter_mut().take(count % p) {
            *c += 1;
        }
        let mut disps = vec![0usize; p];
        for i in 1..p {
            disps[i] = disps[i - 1] + cnts[i - 1];
        }
        let seg_len = |lo: usize, hi: usize| -> usize { cnts[lo..hi].iter().sum() };

        let mut send_idx = 0usize;
        let mut recv_idx = 0usize;
        let mut last_idx = 0usize;

        if newrank != -1 {
            let mut mask: i32 = 1;
            last_idx = p;
            while mask < pof2 {
                let newdst = newrank ^ mask;
                let dst = if newdst < rem {
                    newdst * 2
                } else {
                    newdst + rem
                };
                let half = p / (mask as usize * 2);
                let (send_cnt, recv_cnt);
                if newrank < newdst {
                    send_idx = recv_idx + half;
                    send_cnt = seg_len(send_idx, last_idx);
                    recv_cnt = seg_len(recv_idx, send_idx);
                } else {
                    recv_idx = send_idx + half;
                    send_cnt = seg_len(send_idx, recv_idx);
                    recv_cnt = seg_len(recv_idx, last_idx);
                }
                let s0 = disps[send_idx];
                self.send_ctx(&recvbuf[s0..s0 + send_cnt], dst, TAG_REDUCE, ctx)?;
                let (tmp, _status) = self.recv_ctx::<T>(dst, TAG_REDUCE, ctx)?;
                let r0 = disps[recv_idx];
                combine_into(op, &mut recvbuf[r0..r0 + recv_cnt], &tmp)?;
                send_idx = recv_idx;
                mask <<= 1;
                if mask < pof2 {
                    last_idx = recv_idx + p / mask as usize;
                }
            }
        }

        // Gather to the root. If the root was folded away (odd and below 2*rem),
        // rank 0 hands its segment to the root, which then plays newrank 0.
        let newroot: i32;
        if root < 2 * rem {
            if root % 2 != 0 {
                if rank == root {
                    let (seg, _status) = self.recv_ctx::<T>(0, TAG_REDUCE, ctx)?;
                    recvbuf[..seg.len()].copy_from_slice(&seg);
                    newrank = 0;
                    send_idx = 0;
                    last_idx = 2;
                } else if newrank == 0 {
                    self.send_ctx(&recvbuf[..cnts[0]], root, TAG_REDUCE, ctx)?;
                    newrank = -1;
                }
                newroot = 0;
            } else {
                newroot = root / 2;
            }
        } else {
            newroot = root - rem;
        }

        if newrank != -1 {
            let mut j: i32 = 0;
            let mut mask: i32 = 1;
            while mask < pof2 {
                mask <<= 1;
                j += 1;
            }
            mask >>= 1;
            j -= 1;
            while mask > 0 {
                let newdst = newrank ^ mask;
                let mut dst = if newdst < rem {
                    newdst * 2
                } else {
                    newdst + rem
                };
                if newdst == 0 && root < 2 * rem && root % 2 != 0 {
                    dst = root;
                }
                let newdst_tree_root = (newdst >> j) << j;
                let newroot_tree_root = (newroot >> j) << j;
                let half = p / (mask as usize * 2);
                let (send_cnt, recv_cnt);
                if newrank < newdst {
                    if mask != pof2 / 2 {
                        last_idx += half;
                    }
                    recv_idx = send_idx + half;
                    send_cnt = seg_len(send_idx, recv_idx);
                    recv_cnt = seg_len(recv_idx, last_idx);
                } else {
                    recv_idx = send_idx - half;
                    send_cnt = seg_len(send_idx, last_idx);
                    recv_cnt = seg_len(recv_idx, send_idx);
                }
                if newdst_tree_root == newroot_tree_root {
                    let s0 = disps[send_idx];
                    self.send_ctx(&recvbuf[s0..s0 + send_cnt], dst, TAG_REDUCE, ctx)?;
                    break;
                }
                let (seg, _status) = self.recv_ctx::<T>(dst, TAG_REDUCE, ctx)?;
                let r0 = disps[recv_idx];
                recvbuf[r0..r0 + recv_cnt].copy_from_slice(&seg);
                if newrank > newdst {
                    send_idx = recv_idx;
                }
                mask >>= 1;
                j -= 1;
            }
        }

        if rank == root {
            Ok(Some(recvbuf))
        } else {
            Ok(None)
        }
    }

    /// Element-wise reduce onto every rank (mirrors `MPI_Allreduce`).
    ///
    /// Recursive doubling, a translation of MPICH's
    /// `MPIR_Allreduce_intra_recursive_doubling`
    /// (`src/mpi/coll/allreduce/allreduce_intra_recursive_doubling.c`, identical
    /// algorithm in v3.3 through v4.3.2): a non-power-of-two communicator first folds
    /// its even ranks below `2 * rem` onto their odd neighbours, the remaining
    /// power-of-two ranks exchange and combine with partners at distance 1, 2, 4, ...,
    /// and the folded ranks receive the result at the end. Every rank returns the
    /// result.
    ///
    /// ~~Reduce onto rank 0 followed by a broadcast.~~ **CORRECTED 2026-11-13
    /// (gh:#1435):** that tree associates floating-point sums differently from MPICH
    /// at 5 ranks (and, by the same fold, at larger non-power-of-two sizes), so the
    /// sums differed in the last bit; measured against MPICH 4.2.0 in
    /// `tests/mpich_differential.rs`.
    ///
    /// MPICH's default for long messages (`reduce_scatter_allgather`) has the same
    /// per-element association as recursive doubling (it halves with `mask = 1`
    /// first), so the bits agree with MPICH for every size measured (n = 2..=8);
    /// only the communication volume differs.
    ///
    /// # Errors
    /// [`MpiError::InvalidArgument`] if a partner's buffer length disagrees with this
    /// rank's.
    pub fn all_reduce<T: Reducible>(&self, sendbuf: &[T], op: ReduceOp) -> MpiResult<Vec<T>> {
        let ctx = self.coll_ctx();
        let size = self.size();
        let rank = self.rank();
        let mut acc = sendbuf.to_vec();

        let pof2 = largest_power_of_two_le(size);
        let rem = size - pof2;

        let newrank: i32;
        if rank < 2 * rem {
            if rank % 2 == 0 {
                self.send_ctx(&acc, rank + 1, TAG_ALLREDUCE, ctx)?;
                newrank = -1;
            } else {
                let (tmp, _status) = self.recv_ctx::<T>(rank - 1, TAG_ALLREDUCE, ctx)?;
                combine_into(op, &mut acc, &tmp)?;
                newrank = rank / 2;
            }
        } else {
            newrank = rank - rem;
        }

        if newrank != -1 {
            let mut mask = 1;
            while mask < pof2 {
                let newdst = newrank ^ mask;
                let dst = if newdst < rem {
                    newdst * 2 + 1
                } else {
                    newdst + rem
                };
                self.send_ctx(&acc, dst, TAG_ALLREDUCE, ctx)?;
                let (tmp, _status) = self.recv_ctx::<T>(dst, TAG_ALLREDUCE, ctx)?;
                combine_into(op, &mut acc, &tmp)?;
                mask <<= 1;
            }
        }

        if rank < 2 * rem {
            if rank % 2 != 0 {
                self.send_ctx(&acc, rank - 1, TAG_ALLREDUCE, ctx)?;
            } else {
                let (res, _status) = self.recv_ctx::<T>(rank + 1, TAG_ALLREDUCE, ctx)?;
                acc = res;
            }
        }
        Ok(acc)
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
