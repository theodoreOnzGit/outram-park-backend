//! Communicators and point-to-point communication.
//!
//! A [`Communicator`] is a rank's handle onto the shared [transport](crate::transport):
//! it knows the rank's own id, the group size, and the communicator context id
//! that isolates its messages from those of other communicators. This is the
//! MPICH `MPID_Comm` analogue, minus process groups/topologies (a later bead).
//!
//! # Point-to-point
//!
//! - Blocking [`Communicator::send`] / [`Communicator::recv`] mirror `MPI_Send` /
//!   `MPI_Recv`. The send is buffered (it copies into the receiver's mailbox and
//!   returns), so a matched pair never deadlocks regardless of order — the
//!   shared-memory analogue of an eager protocol.
//! - Non-blocking [`Communicator::isend`] / [`Communicator::irecv`] mirror
//!   `MPI_Isend` / `MPI_Irecv`, returning a [`Request`] completed with
//!   [`Request::wait`] (`MPI_Wait`) or polled with [`Request::test`] (`MPI_Test`).
//!
//! The buffer/count/datatype triple is expressed idiomatically: sends take a
//! typed slice `&[T]`, receives return a typed `Vec<T>` plus a [`Status`], and the
//! datatype tag travels with the message so a receiver decoding the wrong type
//! gets an [`MpiError::TypeMismatch`] rather than silent reinterpretation.
//!
//! # Wildcards
//!
//! Pass [`ANY_SOURCE`] / [`ANY_TAG`] to a receive to match any sender / any tag,
//! mirroring `MPI_ANY_SOURCE` / `MPI_ANY_TAG`. The [`Status`] reports the actual
//! source and tag that matched.

use std::sync::Arc;

use crate::datatype::MpiPrimitive;
use crate::error::{MpiError, MpiResult};
use crate::transport::{Envelope, Transport};

/// Wildcard source: a receive with this source matches a message from any rank
/// (mirrors `MPI_ANY_SOURCE`).
pub const ANY_SOURCE: i32 = -1;

/// Wildcard tag: a receive with this tag matches a message with any tag (mirrors
/// `MPI_ANY_TAG`).
pub const ANY_TAG: i32 = -1;

/// The context id of the world communicator every rank starts with.
pub(crate) const WORLD_COMM_ID: usize = 0;

/// Offset added to a communicator's point-to-point context id to form its
/// **collective** context id. MPI isolates collective messages from user
/// point-to-point messages with a hidden communication context; this crate does
/// the same by posting collective traffic on `comm_id + COLL_CONTEXT_OFFSET`, so
/// a user `send`/`recv` on a tag a collective happens to reuse can never match a
/// collective's internal message. The offset is far above any real allocated
/// comm id (which count up from 0).
pub(crate) const COLL_CONTEXT_OFFSET: usize = 1 << 40;

/// Outcome of a completed receive: which message actually matched.
///
/// Mirrors `MPI_Status` (the fields a program reads: `MPI_SOURCE`, `MPI_TAG`, and
/// the element count from `MPI_Get_count`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Status {
    /// Rank of the sender of the matched message.
    pub source: i32,
    /// Tag of the matched message.
    pub tag: i32,
    /// Number of elements received.
    pub count: usize,
}

/// A rank's handle onto the shared-memory message fabric for one communicator.
///
/// Cloning is cheap (an `Arc` bump plus three integers) and yields another handle
/// onto the *same* communicator context — used internally to hand a context to a
/// pending [`Request`]. Construct rank handles with [`crate::run`], not directly.
#[derive(Clone)]
pub struct Communicator {
    transport: Arc<Transport>,
    rank: i32,
    size: i32,
    comm_id: usize,
}

impl Communicator {
    /// Internal constructor used by the runtime and collective/duplication paths.
    pub(crate) fn new(transport: Arc<Transport>, rank: i32, size: i32, comm_id: usize) -> Self {
        Communicator {
            transport,
            rank,
            size,
            comm_id,
        }
    }

    /// This rank's id within the communicator, `0..size` (mirrors `MPI_Comm_rank`).
    pub fn rank(&self) -> i32 {
        self.rank
    }

    /// Number of ranks in the communicator (mirrors `MPI_Comm_size`).
    pub fn size(&self) -> i32 {
        self.size
    }

    /// The communicator context id (internal; distinguishes duplicated communicators).
    pub(crate) fn comm_id(&self) -> usize {
        self.comm_id
    }

    /// Shared transport handle (internal, for the collective layer).
    pub(crate) fn transport(&self) -> &Arc<Transport> {
        &self.transport
    }

    /// This communicator's collective context id (isolates collective messages
    /// from user point-to-point traffic).
    pub(crate) fn coll_ctx(&self) -> usize {
        self.comm_id + COLL_CONTEXT_OFFSET
    }

    /// Send `data` to `dest` with `tag` on an explicit context id. The public
    /// [`Communicator::send`] uses the point-to-point context; the collective
    /// layer uses [`Communicator::coll_ctx`].
    pub(crate) fn send_ctx<T: MpiPrimitive>(
        &self,
        data: &[T],
        dest: i32,
        tag: i32,
        ctx: usize,
    ) -> MpiResult<()> {
        self.check_rank(dest)?;
        let env = Envelope {
            src: self.rank,
            tag,
            comm_id: ctx,
            datatype: T::DATATYPE,
            count: data.len(),
            bytes: T::encode(data),
        };
        self.transport.post(dest as usize, env);
        Ok(())
    }

    /// Blocking receive from `source`/`tag` on an explicit context id.
    pub(crate) fn recv_ctx<T: MpiPrimitive>(
        &self,
        source: i32,
        tag: i32,
        ctx: usize,
    ) -> MpiResult<(Vec<T>, Status)> {
        let src = self.match_source(source)?;
        let tagf = match_tag(tag);
        let env = self
            .transport
            .recv_blocking(self.rank as usize, ctx, src, tagf);
        self.decode_envelope(env)
    }

    /// Validate a destination/source rank against the communicator size.
    fn check_rank(&self, rank: i32) -> MpiResult<()> {
        if rank < 0 || rank >= self.size {
            return Err(MpiError::InvalidRank {
                rank,
                size: self.size,
            });
        }
        Ok(())
    }

    /// Blocking send of `data` to `dest` with `tag` (mirrors `MPI_Send`).
    ///
    /// The message is copied into `dest`'s mailbox and this call returns
    /// immediately (buffered semantics), so a correctly-matched send/recv pair
    /// cannot deadlock on ordering.
    ///
    /// # Errors
    /// [`MpiError::InvalidRank`] if `dest` is outside `0..size`.
    pub fn send<T: MpiPrimitive>(&self, data: &[T], dest: i32, tag: i32) -> MpiResult<()> {
        self.send_ctx(data, dest, tag, self.comm_id)
    }

    /// Decode a received [`Envelope`] into `(Vec<T>, Status)`, checking the type.
    fn decode_envelope<T: MpiPrimitive>(&self, env: Envelope) -> MpiResult<(Vec<T>, Status)> {
        if env.datatype != T::DATATYPE {
            return Err(MpiError::TypeMismatch {
                expected: T::DATATYPE,
                actual: env.datatype,
            });
        }
        let status = Status {
            source: env.src,
            tag: env.tag,
            count: env.count,
        };
        Ok((T::decode(&env.bytes), status))
    }

    /// Blocking receive of a message from `source` with `tag` (mirrors `MPI_Recv`).
    ///
    /// Returns the decoded payload and a [`Status`]. Use [`ANY_SOURCE`] / [`ANY_TAG`]
    /// to match any sender / tag. Blocks until a matching message arrives.
    ///
    /// # Errors
    /// [`MpiError::InvalidRank`] for a non-wildcard `source` outside `0..size`;
    /// [`MpiError::TypeMismatch`] if the sender used a different datatype.
    pub fn recv<T: MpiPrimitive>(&self, source: i32, tag: i32) -> MpiResult<(Vec<T>, Status)> {
        self.recv_ctx(source, tag, self.comm_id)
    }

    /// Blocking receive into a caller-provided buffer (mirrors `MPI_Recv` with an
    /// explicit `count`). Returns the [`Status`]; errors with
    /// [`MpiError::Truncated`] if the message has more elements than `buf` holds.
    ///
    /// Elements actually received (`status.count`) are written to the front of
    /// `buf`; any remaining slots are left unchanged.
    ///
    /// # Errors
    /// [`MpiError::InvalidRank`], [`MpiError::TypeMismatch`], or
    /// [`MpiError::Truncated`].
    pub fn recv_into<T: MpiPrimitive>(
        &self,
        buf: &mut [T],
        source: i32,
        tag: i32,
    ) -> MpiResult<Status> {
        let (data, status) = self.recv::<T>(source, tag)?;
        if data.len() > buf.len() {
            return Err(MpiError::Truncated {
                buffer: buf.len(),
                message: data.len(),
            });
        }
        buf[..data.len()].copy_from_slice(&data);
        Ok(status)
    }

    /// Non-blocking send (mirrors `MPI_Isend`). Because the shared-memory transport
    /// buffers eagerly, the message is already delivered when this returns; the
    /// [`Request`] it yields is complete and its [`Request::wait`] is a no-op that
    /// returns no data.
    ///
    /// # Errors
    /// [`MpiError::InvalidRank`] if `dest` is outside `0..size`.
    pub fn isend<T: MpiPrimitive>(&self, data: &[T], dest: i32, tag: i32) -> MpiResult<Request<T>> {
        self.send(data, dest, tag)?;
        Ok(Request {
            kind: RequestKind::SendComplete,
        })
    }

    /// Non-blocking receive (mirrors `MPI_Irecv`). Returns a [`Request`] that is
    /// completed with [`Request::wait`] (blocking) or polled with [`Request::test`]
    /// (non-blocking); completion yields the payload and [`Status`].
    ///
    /// # Errors
    /// [`MpiError::InvalidRank`] for a non-wildcard `source` outside `0..size`.
    pub fn irecv<T: MpiPrimitive>(&self, source: i32, tag: i32) -> MpiResult<Request<T>> {
        // Validate eagerly so a bad rank is reported at post time, like MPICH.
        self.match_source(source)?;
        Ok(Request {
            kind: RequestKind::Recv {
                comm: self.clone(),
                source,
                tag,
                _marker: core::marker::PhantomData,
            },
        })
    }

    /// Translate a possibly-wildcard source into the transport's `Option<i32>`,
    /// validating a concrete rank.
    fn match_source(&self, source: i32) -> MpiResult<Option<i32>> {
        if source == ANY_SOURCE {
            Ok(None)
        } else {
            self.check_rank(source)?;
            Ok(Some(source))
        }
    }
}

/// Translate a possibly-wildcard tag into the transport's `Option<i32>`.
fn match_tag(tag: i32) -> Option<i32> {
    if tag == ANY_TAG {
        None
    } else {
        Some(tag)
    }
}

/// The internal state of a [`Request`].
enum RequestKind<T: MpiPrimitive> {
    /// A completed non-blocking send — nothing to wait for.
    SendComplete,
    /// A pending non-blocking receive holding the context to complete it.
    Recv {
        comm: Communicator,
        source: i32,
        tag: i32,
        _marker: core::marker::PhantomData<T>,
    },
}

/// A handle to an outstanding non-blocking operation (mirrors `MPI_Request`).
///
/// Complete a send request with [`Request::wait`] (a no-op) or a receive request
/// with [`Request::wait`] (blocks for the message) / [`Request::test`] (polls).
/// The payload type `T` is fixed when the request is created.
pub struct Request<T: MpiPrimitive> {
    kind: RequestKind<T>,
}

impl<T: MpiPrimitive> Request<T> {
    /// Block until the operation completes and return its result (mirrors
    /// `MPI_Wait`).
    ///
    /// For a send request the result is `None` (a send carries no received data).
    /// For a receive request it is `Some((payload, status))`.
    ///
    /// # Errors
    /// [`MpiError::TypeMismatch`] if a received message's datatype differs from `T`.
    pub fn wait(self) -> MpiResult<Option<(Vec<T>, Status)>> {
        match self.kind {
            RequestKind::SendComplete => Ok(None),
            RequestKind::Recv {
                comm, source, tag, ..
            } => {
                let (data, status) = comm.recv::<T>(source, tag)?;
                Ok(Some((data, status)))
            }
        }
    }

    /// Poll for completion without blocking (mirrors `MPI_Test`).
    ///
    /// Returns `Ok(None)` if the operation is not yet complete (a receive whose
    /// message has not arrived). A send request is always complete. On completion
    /// of a receive, returns `Ok(Some((payload, status)))` and the request is
    /// consumed; if incomplete, ownership is returned in the `Err`-free `Left`
    /// arm via re-construction is avoided by taking `&mut self`.
    ///
    /// # Errors
    /// [`MpiError::TypeMismatch`] if a received message's datatype differs from `T`.
    pub fn test(&mut self) -> MpiResult<TestOutcome<T>> {
        match &self.kind {
            RequestKind::SendComplete => Ok(TestOutcome::SendComplete),
            RequestKind::Recv {
                comm, source, tag, ..
            } => {
                let src = comm.match_source(*source)?;
                let tagf = match_tag(*tag);
                match comm
                    .transport()
                    .recv_try(comm.rank() as usize, comm.comm_id(), src, tagf)
                {
                    Some(env) => {
                        let (data, status) = comm.decode_envelope::<T>(env)?;
                        Ok(TestOutcome::RecvComplete(data, status))
                    }
                    None => Ok(TestOutcome::Pending),
                }
            }
        }
    }
}

/// Result of polling a [`Request`] with [`Request::test`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestOutcome<T: MpiPrimitive> {
    /// A send request — already complete, no data.
    SendComplete,
    /// A receive that has not yet matched a message.
    Pending,
    /// A receive that completed, carrying its payload and status.
    RecvComplete(Vec<T>, Status),
}
