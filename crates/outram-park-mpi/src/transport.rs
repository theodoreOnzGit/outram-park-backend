//! Shared-memory transport — the threads-as-ranks message layer.
//!
//! This is the MPICH *device/ADI* analogue for a single process: each rank is a
//! thread, and every rank owns one **mailbox** (an inbox queue guarded by a
//! `Mutex` + `Condvar`). A send pushes an [`Envelope`] onto the destination
//! rank's mailbox and wakes any thread blocked on it; a receive scans its own
//! mailbox for the first envelope matching the requested `(communicator, source,
//! tag)` and, if none is present, blocks on the condition variable until one
//! arrives.
//!
//! # Message matching and ordering
//!
//! MPI guarantees that messages sent from one rank to another **on the same
//! communicator with the same tag are non-overtaking**: they are received in send
//! order. The mailbox preserves this because it is a FIFO queue and a receive
//! takes the *first* matching envelope — so two messages with the same
//! `(comm, src, tag)` are dequeued in the order they were enqueued. A wildcard
//! receive (`MPI_ANY_SOURCE` / `MPI_ANY_TAG`) matches the earliest envelope
//! satisfying the non-wildcard fields.
//!
//! # Why shared-memory first
//!
//! Threads-as-ranks needs no network stack, no launcher, and no C toolchain, so
//! it builds and runs on Android/Termux like any other pure-Rust library, and it
//! directly serves single-node multicore domain decomposition. A future TCP
//! transport (multi-node) will implement the same enqueue/match interface behind
//! a Cargo feature.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use crate::datatype::Datatype;

/// One posted message in transit to a rank's mailbox.
///
/// Fields are the MPI *message envelope* (`source`, `tag`, communicator context)
/// plus the type-erased payload (`datatype` tag, element `count`, native-endian
/// `bytes`) the receiver decodes.
#[derive(Debug, Clone)]
pub(crate) struct Envelope {
    /// Rank of the sender within the communicator.
    pub src: i32,
    /// Message tag chosen by the sender.
    pub tag: i32,
    /// Communicator context id — messages never match across communicators.
    pub comm_id: usize,
    /// Datatype tag of the payload elements.
    pub datatype: Datatype,
    /// Number of elements in the payload.
    pub count: usize,
    /// Native-endian payload bytes (`count * datatype.size()` long).
    pub bytes: Vec<u8>,
}

/// A single rank's inbox: a FIFO envelope queue plus a condition variable other
/// ranks signal when they deliver into it.
struct Mailbox {
    queue: Mutex<VecDeque<Envelope>>,
    signal: Condvar,
}

impl Mailbox {
    fn new() -> Self {
        Mailbox {
            queue: Mutex::new(VecDeque::new()),
            signal: Condvar::new(),
        }
    }
}

/// The process-wide message fabric: one [`Mailbox`] per rank, shared (via
/// `Arc`) by every rank thread.
pub(crate) struct Transport {
    mailboxes: Vec<Mailbox>,
    /// Monotonic allocator for fresh communicator context ids. World is context
    /// 0, so allocation starts at 1; `comm_dup`/`comm_split` draw new point-to-
    /// point context ids here (their collective contexts are these plus
    /// [`crate::communicator::COLL_CONTEXT_OFFSET`], which never collide because
    /// allocated ids stay far below that offset).
    next_comm_id: AtomicUsize,
}

impl Transport {
    /// Build a transport for `size` ranks (one empty mailbox each).
    pub(crate) fn new(size: usize) -> Arc<Self> {
        let mailboxes = (0..size).map(|_| Mailbox::new()).collect();
        Arc::new(Transport {
            mailboxes,
            next_comm_id: AtomicUsize::new(1),
        })
    }

    /// Allocate a fresh, process-unique communicator context id.
    ///
    /// Called on **one** rank during a collective communicator-creation call
    /// (`comm_dup`/`comm_split`); that rank then broadcasts the id so the whole
    /// new group agrees on it.
    pub(crate) fn alloc_comm_id(&self) -> usize {
        self.next_comm_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Number of ranks (mailboxes) in this transport. (Used by the collective
    /// layer, a follow-up milestone; the communicator carries its own size for
    /// point-to-point.)
    #[allow(dead_code)]
    pub(crate) fn size(&self) -> usize {
        self.mailboxes.len()
    }

    /// Deliver `env` into `dest`'s mailbox and wake a thread blocked on it.
    ///
    /// Delivery is buffered and returns immediately — the shared-memory analogue
    /// of MPI's buffered/eager send. Panics only if `dest` is out of range, which
    /// callers (the communicator layer) prevent by validating ranks first.
    pub(crate) fn post(&self, dest: usize, env: Envelope) {
        let mb = &self.mailboxes[dest];
        let mut q = mb.queue.lock().expect("mailbox mutex poisoned");
        q.push_back(env);
        // A single waiter can consume this message; notify_one is sufficient and
        // avoids a thundering herd. Any waiter that wakes and finds no match
        // re-waits (the scan below is inside the condvar loop).
        mb.signal.notify_one();
    }

    /// Index of the first envelope in `q` matching the selection criteria, or
    /// `None`. `src`/`tag` of `None` are wildcards (`MPI_ANY_SOURCE`/`MPI_ANY_TAG`).
    fn find_match(
        q: &VecDeque<Envelope>,
        comm_id: usize,
        src: Option<i32>,
        tag: Option<i32>,
    ) -> Option<usize> {
        q.iter().position(|e| {
            e.comm_id == comm_id
                && src.map_or(true, |s| e.src == s)
                && tag.map_or(true, |t| e.tag == t)
        })
    }

    /// Block until an envelope matching `(comm_id, src, tag)` is in `me`'s mailbox,
    /// then remove and return it. `src`/`tag` of `None` are wildcards.
    pub(crate) fn recv_blocking(
        &self,
        me: usize,
        comm_id: usize,
        src: Option<i32>,
        tag: Option<i32>,
    ) -> Envelope {
        let mb = &self.mailboxes[me];
        let mut q = mb.queue.lock().expect("mailbox mutex poisoned");
        loop {
            if let Some(pos) = Self::find_match(&q, comm_id, src, tag) {
                return q.remove(pos).expect("matched index is in range");
            }
            q = mb.signal.wait(q).expect("mailbox condvar poisoned");
        }
    }

    /// Non-blocking probe-and-take: return a matching envelope if one is already
    /// queued, else `None`. Used by the `test`/`try_*` completion path.
    pub(crate) fn recv_try(
        &self,
        me: usize,
        comm_id: usize,
        src: Option<i32>,
        tag: Option<i32>,
    ) -> Option<Envelope> {
        let mb = &self.mailboxes[me];
        let mut q = mb.queue.lock().expect("mailbox mutex poisoned");
        Self::find_match(&q, comm_id, src, tag)
            .map(|pos| q.remove(pos).expect("matched index is in range"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(src: i32, tag: i32, comm_id: usize) -> Envelope {
        Envelope {
            src,
            tag,
            comm_id,
            datatype: Datatype::I32,
            count: 0,
            bytes: Vec::new(),
        }
    }

    #[test]
    fn post_then_recv_try_matches_by_src_tag_comm() {
        let t = Transport::new(1);
        t.post(0, env(3, 7, 0));
        assert!(t.recv_try(0, 0, Some(3), Some(7)).is_some());
        // consumed
        assert!(t.recv_try(0, 0, Some(3), Some(7)).is_none());
    }

    #[test]
    fn wildcards_match_any_src_and_tag() {
        let t = Transport::new(1);
        t.post(0, env(5, 42, 0));
        assert!(t.recv_try(0, 0, None, None).is_some());
    }

    #[test]
    fn different_comm_id_does_not_match() {
        let t = Transport::new(1);
        t.post(0, env(1, 1, 9));
        assert!(t.recv_try(0, 0, Some(1), Some(1)).is_none());
        assert!(t.recv_try(0, 9, Some(1), Some(1)).is_some());
    }

    #[test]
    fn same_src_tag_are_fifo_non_overtaking() {
        let t = Transport::new(1);
        let mut a = env(2, 1, 0);
        a.count = 100;
        let mut b = env(2, 1, 0);
        b.count = 200;
        t.post(0, a);
        t.post(0, b);
        // First matching taken first -> send order preserved.
        assert_eq!(t.recv_try(0, 0, Some(2), Some(1)).unwrap().count, 100);
        assert_eq!(t.recv_try(0, 0, Some(2), Some(1)).unwrap().count, 200);
    }
}
