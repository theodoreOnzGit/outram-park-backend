//! # outram-park-mpi
//!
//! An independent, pure-Rust translation of a **subset of MPICH** — the reference
//! MPI implementation from Argonne National Laboratory — providing the MPI-3 API
//! surface most simulations need, over a **shared-memory, threads-as-ranks**
//! transport.
//!
//! > **⚠️ SCAFFOLD / VERIFICATION-ONLY — no human V&V yet.** This is the first
//! > milestone: the shared-memory transport, datatypes, communicators (world),
//! > and point-to-point communication. Collectives, communicator
//! > duplication/splitting, groups, and a TCP (multi-node) transport are
//! > bead-tracked follow-ups (epic op-erl). No comparison against MPICH's own
//! > test suite has been made. Untrusted AI-generated draft until a human reviews
//! > it, per the workspace `RESPONSIBLE_USE.md`.
//! >
//! > **Independent translation, not MPICH.** "MPI" names only the standard whose
//! > API this implements; "MPICH" names the reference implementation whose
//! > semantics and (later) collective algorithms are translated. Nothing here is
//! > endorsed by or affiliated with the MPICH project or Argonne National
//! > Laboratory, and no MPICH C code is linked. See `NOTICE`.
//!
//! ## What this is for
//!
//! A pure-Rust, Android-buildable message-passing layer for single-node multicore
//! domain decomposition — the foundation for MPI-style scale-out in the OUTRAM
//! PARK simulators (e.g. pflotran's `op-v6s.15.9`). Ranks are threads in one
//! process communicating through in-memory mailboxes; there is no C toolchain, no
//! system MPI, and no network stack, so the library builds and runs anywhere the
//! rest of the workspace does, Termux included.
//!
//! ## Quick start
//!
//! Run a closure on `n` ranks; each rank gets its own [`Communicator`]:
//!
//! ```
//! use outram_park_mpi::{run, ANY_TAG};
//!
//! // A ring: each rank sends its rank number to the next and receives from the
//! // previous, so every rank ends up knowing its left neighbour's id.
//! let neighbours = run(4, |comm| {
//!     let n = comm.size();
//!     let me = comm.rank();
//!     let right = (me + 1) % n;
//!     let left = (me - 1 + n) % n;
//!     comm.send(&[me], right, 0).unwrap();
//!     let (msg, _status) = comm.recv::<i32>(left, ANY_TAG).unwrap();
//!     msg[0]
//! })
//! .unwrap();
//!
//! assert_eq!(neighbours, vec![3, 0, 1, 2]); // rank r received from r-1 (mod 4)
//! ```
//!
//! ## Module map
//!
//! | Module | MPICH analogue | Contents |
//! |---|---|---|
//! | [`error`] | error classes | [`MpiError`] / [`MpiResult`] |
//! | [`datatype`] | predefined datatypes | [`Datatype`] tag + [`MpiPrimitive`] codec |
//! | [`transport`] *(internal)* | device / ADI layer | shared-memory rank mailboxes |
//! | [`communicator`] | `MPID_Comm` + pt2pt | [`Communicator`], [`Request`], [`Status`] |
//!
//! ## Design rules (workspace mandate)
//!
//! - **Enum dispatch, no trait objects.** [`Datatype`] and request/test outcomes
//!   are enums matched exhaustively; [`MpiPrimitive`] is a compiler-checked
//!   contract on each primitive, never a `dyn` object.
//! - **No `Box`, no lifetime parameters.** Shared state is held by `Arc`; the
//!   rank threads borrow the user closure through a scoped-thread scope.
//! - **Pure Rust, Android-safe.** `std` threads + sync only; no C/FFI, no system MPI.

pub mod collective;
pub mod communicator;
pub mod datatype;
pub mod error;
pub mod transport;

pub use collective::{Reducible, ReduceOp};
pub use communicator::{Communicator, Request, Status, TestOutcome, ANY_SOURCE, ANY_TAG};
pub use datatype::{Datatype, MpiPrimitive};
pub use error::{MpiError, MpiResult};

use communicator::WORLD_COMM_ID;
use transport::Transport;

/// Run `f` on `n_ranks` ranks and collect their results in rank order.
///
/// This is the runtime entry point — the analogue of launching an MPI program
/// with `mpiexec -n <n_ranks>`, except the ranks are threads in the current
/// process. Each rank thread invokes `f` with its own world [`Communicator`]
/// (`rank() in 0..n_ranks`, `size() == n_ranks`) and the returned values are
/// gathered into a `Vec` indexed by rank.
///
/// The ranks run inside a [scoped thread](std::thread::scope) scope, so `f` may
/// borrow from the caller's stack (no `'static` bound); it must be `Sync` because
/// every rank shares the one closure.
///
/// # Errors
/// [`MpiError::InvalidArgument`] if `n_ranks <= 0`.
///
/// # Panics
/// Propagates a panic from any rank thread (the whole run aborts), matching MPI's
/// "a failed process kills the job" behaviour.
///
/// # Examples
/// ```
/// use outram_park_mpi::run;
/// // Each rank reports its own id; results come back in rank order.
/// let ids = run(3, |comm| comm.rank()).unwrap();
/// assert_eq!(ids, vec![0, 1, 2]);
/// ```
pub fn run<F, R>(n_ranks: i32, f: F) -> MpiResult<Vec<R>>
where
    F: Fn(&Communicator) -> R + Sync,
    R: Send,
{
    if n_ranks <= 0 {
        return Err(MpiError::InvalidArgument(format!(
            "n_ranks must be > 0, got {n_ranks}"
        )));
    }
    let transport = Transport::new(n_ranks as usize);
    let f_ref = &f;
    let results = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..n_ranks)
            .map(|rank| {
                let t = std::sync::Arc::clone(&transport);
                scope.spawn(move || {
                    let comm = Communicator::new(t, rank, n_ranks, WORLD_COMM_ID);
                    f_ref(&comm)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("rank thread panicked"))
            .collect::<Vec<R>>()
    });
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_rejects_nonpositive_ranks() {
        assert!(run(0, |_| ()).is_err());
        assert!(run(-2, |_| ()).is_err());
    }

    #[test]
    fn each_rank_sees_its_rank_and_size() {
        let out = run(5, |c| (c.rank(), c.size())).unwrap();
        assert_eq!(
            out,
            vec![(0, 5), (1, 5), (2, 5), (3, 5), (4, 5)]
        );
    }

    #[test]
    fn point_to_point_send_recv_between_two_ranks() {
        let out = run(2, |c| {
            if c.rank() == 0 {
                c.send(&[10.0_f64, 20.0, 30.0], 1, 7).unwrap();
                Vec::new()
            } else {
                let (data, status) = c.recv::<f64>(0, 7).unwrap();
                assert_eq!(status.source, 0);
                assert_eq!(status.tag, 7);
                assert_eq!(status.count, 3);
                data
            }
        })
        .unwrap();
        assert_eq!(out[1], vec![10.0, 20.0, 30.0]);
    }

    #[test]
    fn wildcard_recv_reports_actual_source_and_tag() {
        let out = run(3, |c| {
            if c.rank() == 0 {
                // Receive one message from anyone/any tag.
                let (data, status) = c.recv::<i32>(ANY_SOURCE, ANY_TAG).unwrap();
                (data[0], status.source, status.tag)
            } else {
                c.send(&[100 + c.rank()], 0, 5).unwrap();
                (0, -1, -1)
            }
        })
        .unwrap();
        let (val, src, tag) = out[0];
        assert!(src == 1 || src == 2);
        assert_eq!(tag, 5);
        assert_eq!(val, 100 + src);
    }

    #[test]
    fn nonblocking_isend_irecv_complete_with_wait() {
        let out = run(2, |c| {
            if c.rank() == 0 {
                let req = c.isend(&[42_u64], 1, 1).unwrap();
                assert!(req.wait().unwrap().is_none()); // send carries no data
                0
            } else {
                let req = c.irecv::<u64>(0, 1).unwrap();
                let (data, _status) = req.wait().unwrap().expect("recv yields data");
                data[0]
            }
        })
        .unwrap();
        assert_eq!(out[1], 42);
    }

    #[test]
    fn recv_into_truncation_is_an_error() {
        let out = run(2, |c| {
            if c.rank() == 0 {
                c.send(&[1.0_f32, 2.0, 3.0], 1, 0).unwrap();
                Ok(())
            } else {
                let mut buf = [0.0_f32; 2]; // too small for 3 elements
                match c.recv_into(&mut buf, 0, 0) {
                    Err(MpiError::Truncated { buffer, message }) => {
                        assert_eq!(buffer, 2);
                        assert_eq!(message, 3);
                        Ok(())
                    }
                    other => Err(format!("expected truncation, got {other:?}")),
                }
            }
        })
        .unwrap();
        assert!(out[1].is_ok());
    }

    #[test]
    fn barrier_completes_on_all_ranks() {
        // If the barrier deadlocked or mis-routed, run() would hang; reaching the
        // assert means every rank passed it.
        let out = run(6, |c| {
            c.barrier().unwrap();
            c.rank()
        })
        .unwrap();
        assert_eq!(out, vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn broadcast_delivers_root_data_to_all() {
        for root in 0..4 {
            let out = run(4, move |c| {
                let data = if c.rank() == root {
                    Some(vec![7.0_f64, 8.0, 9.0])
                } else {
                    None
                };
                c.broadcast(data.as_deref(), root).unwrap()
            })
            .unwrap();
            for got in &out {
                assert_eq!(got, &vec![7.0, 8.0, 9.0]);
            }
        }
    }

    #[test]
    fn reduce_sum_and_max_land_on_root() {
        // Each rank r contributes [r, 10+r]; sum over 5 ranks = [10, 60].
        let out = run(5, |c| {
            let r = c.rank();
            let sum = c.reduce(&[r, 10 + r], ReduceOp::Sum, 0).unwrap();
            let max = c.reduce(&[r, 10 + r], ReduceOp::Max, 0).unwrap();
            (sum, max)
        })
        .unwrap();
        assert_eq!(out[0].0, Some(vec![0 + 1 + 2 + 3 + 4, 10 + 11 + 12 + 13 + 14]));
        assert_eq!(out[0].1, Some(vec![4, 14]));
        // Non-root ranks get None.
        assert_eq!(out[3].0, None);
    }

    #[test]
    fn all_reduce_sum_lands_on_everyone() {
        let out = run(4, |c| {
            let r = c.rank();
            c.all_reduce(&[r + 1], ReduceOp::Sum).unwrap()
        })
        .unwrap();
        // sum of (1+2+3+4) = 10 on every rank.
        for got in &out {
            assert_eq!(got, &vec![10]);
        }
    }

    #[test]
    fn scatter_then_gather_roundtrips() {
        let out = run(3, |c| {
            // Root 0 scatters [0,1, 2,3, 4,5] as 2-element chunks.
            let send = if c.rank() == 0 {
                Some(vec![0_i32, 1, 2, 3, 4, 5])
            } else {
                None
            };
            let chunk = c.scatter(send.as_deref(), 2, 0).unwrap();
            // Each rank doubles its chunk, then gathers back to root 0.
            let doubled: Vec<i32> = chunk.iter().map(|x| x * 2).collect();
            let gathered = c.gather(&doubled, 0).unwrap();
            (chunk, gathered)
        })
        .unwrap();
        assert_eq!(out[0].0, vec![0, 1]);
        assert_eq!(out[1].0, vec![2, 3]);
        assert_eq!(out[2].0, vec![4, 5]);
        assert_eq!(out[0].1, Some(vec![0, 2, 4, 6, 8, 10]));
        assert_eq!(out[1].1, None);
    }

    #[test]
    fn all_gather_concatenates_on_every_rank() {
        let out = run(4, |c| c.all_gather(&[c.rank()]).unwrap()).unwrap();
        for got in &out {
            assert_eq!(got, &vec![0, 1, 2, 3]);
        }
    }

    #[test]
    fn collectives_do_not_cross_match_user_pt2p() {
        // A user send on the same tag a collective uses must not be consumed by
        // the collective (separate context), and vice-versa.
        let out = run(2, |c| {
            if c.rank() == 0 {
                c.send(&[999_i32], 1, TAG_LIKE_BCAST).unwrap();
                let bc = c.broadcast(Some(&[1_i32, 2]), 0).unwrap();
                bc
            } else {
                let bc = c.broadcast(None, 0).unwrap();
                let (user, _s) = c.recv::<i32>(0, TAG_LIKE_BCAST).unwrap();
                assert_eq!(user, vec![999]);
                bc
            }
        })
        .unwrap();
        assert_eq!(out[1], vec![1, 2]);
    }

    // A user tag numerically equal to the internal broadcast tag (1) — proves
    // isolation is by context, not tag.
    const TAG_LIKE_BCAST: i32 = 1;

    #[test]
    fn type_mismatch_is_detected() {
        let out = run(2, |c| {
            if c.rank() == 0 {
                c.send(&[1_i32, 2], 1, 0).unwrap();
                true
            } else {
                // Sender used i32; decode as f64 must fail.
                matches!(c.recv::<f64>(0, 0), Err(MpiError::TypeMismatch { .. }))
            }
        })
        .unwrap();
        assert!(out[1]);
    }
}
