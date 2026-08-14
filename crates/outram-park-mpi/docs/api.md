# Crate Documentation

**Version:** 0.0.0

**Format Version:** 60

# Module `outram_park_mpi`

# outram-park-mpi

An independent, pure-Rust translation of a **subset of MPICH** — the reference
MPI implementation from Argonne National Laboratory — providing the MPI-3 API
surface most simulations need, over a **shared-memory, threads-as-ranks**
transport.

> **⚠️ SCAFFOLD / VERIFICATION-ONLY — no human V&V yet.** This is the first
> milestone: the shared-memory transport, datatypes, communicators (world),
> and point-to-point communication. Collectives, communicator
> duplication/splitting, groups, and a TCP (multi-node) transport are
> bead-tracked follow-ups (epic op-erl). No comparison against MPICH's own
> test suite has been made. Untrusted AI-generated draft until a human reviews
> it, per the workspace `RESPONSIBLE_USE.md`.
>
> **Independent translation, not MPICH.** "MPI" names only the standard whose
> API this implements; "MPICH" names the reference implementation whose
> semantics and (later) collective algorithms are translated. Nothing here is
> endorsed by or affiliated with the MPICH project or Argonne National
> Laboratory, and no MPICH C code is linked. See `NOTICE`.

## What this is for

A pure-Rust, Android-buildable message-passing layer for single-node multicore
domain decomposition — the foundation for MPI-style scale-out in the OUTRAM
PARK simulators (e.g. pflotran's `op-v6s.15.9`). Ranks are threads in one
process communicating through in-memory mailboxes; there is no C toolchain, no
system MPI, and no network stack, so the library builds and runs anywhere the
rest of the workspace does, Termux included.

## Quick start

Run a closure on `n` ranks; each rank gets its own [`Communicator`]:

```
use outram_park_mpi::{run, ANY_TAG};

// A ring: each rank sends its rank number to the next and receives from the
// previous, so every rank ends up knowing its left neighbour's id.
let neighbours = run(4, |comm| {
    let n = comm.size();
    let me = comm.rank();
    let right = (me + 1) % n;
    let left = (me - 1 + n) % n;
    comm.send(&[me], right, 0).unwrap();
    let (msg, _status) = comm.recv::<i32>(left, ANY_TAG).unwrap();
    msg[0]
})
.unwrap();

assert_eq!(neighbours, vec![3, 0, 1, 2]); // rank r received from r-1 (mod 4)
```

## Module map

| Module | MPICH analogue | Contents |
|---|---|---|
| [`error`] | error classes | [`MpiError`] / [`MpiResult`] |
| [`datatype`] | predefined datatypes | [`Datatype`] tag + [`MpiPrimitive`] codec |
| [`transport`] *(internal)* | device / ADI layer | shared-memory rank mailboxes |
| [`communicator`] | `MPID_Comm` + pt2pt | [`Communicator`], [`Request`], [`Status`] |
| [`collective`] | collectives + ops | barrier/bcast/reduce/allreduce/scatter/gather/allgather, [`ReduceOp`] |
| [`comm_mgmt`] | `MPI_Comm_dup`/`split` | [`Communicator::dup`], [`Communicator::split`] |
| [`group`] | `MPI_Group_*` | [`Group`] set ops + [`Communicator::create_from_group`] |
| [`topology`] | `MPI_Cart_*` | [`CartesianComm`] coords/rank/shift |

## Design rules (workspace mandate)

- **Enum dispatch, no trait objects.** [`Datatype`] and request/test outcomes
  are enums matched exhaustively; [`MpiPrimitive`] is a compiler-checked
  contract on each primitive, never a `dyn` object.
- **No `Box`, no lifetime parameters.** Shared state is held by `Arc`; the
  rank threads borrow the user closure through a scoped-thread scope.
- **Pure Rust, Android-safe.** `std` threads + sync only; no C/FFI, no system MPI.

## Modules

## Module `collective`

Collective communication and reduction operations.

These are the collectives a parallel solver leans on — [barrier](Communicator::barrier),
[broadcast](Communicator::broadcast), [reduce](Communicator::reduce),
[all_reduce](Communicator::all_reduce), [scatter](Communicator::scatter),
[gather](Communicator::gather), and [all_gather](Communicator::all_gather) —
built on the point-to-point layer. Every rank in the communicator must call
the same collective in the same order (the MPI collective-ordering rule); a
collective is blocking (it returns once this rank's part is complete).

Collective messages travel on a **separate communication context**
([`Communicator::coll_ctx`]) from user point-to-point traffic, so a user
`send`/`recv` can never accidentally match a collective's internal message —
the same isolation MPI provides with hidden contexts.

# Algorithms and provenance

[broadcast](Communicator::broadcast) and [reduce](Communicator::reduce) use
**binomial trees** (`O(log P)` rounds), translated from MPICH's
`MPIR_Bcast_intra_binomial` / `MPIR_Reduce_intra_binomial`.
[all_reduce](Communicator::all_reduce) is reduce-then-broadcast (`2 log P`);
[all_gather](Communicator::all_gather) is gather-then-broadcast;
[scatter](Communicator::scatter), [gather](Communicator::gather), and the
[barrier](Communicator::barrier) use linear (`O(P)`) root-centred schedules.
MPICH's fully-optimised variants — recursive-doubling all-reduce, ring
all-gather, dissemination barrier — are a documented follow-up (epic op-erl);
the versions here are correctness-first and give identical results.

# Untrusted AI draft

Verification-only, per the workspace `RESPONSIBLE_USE.md`; not checked against
MPICH's own collective test suite.

```rust
pub mod collective { /* ... */ }
```

### Types

#### Enum `ReduceOp`

A predefined reduction operation (mirrors the MPI predefined `MPI_Op`s).

Applied element-wise by [`Reducible::combine`]. Arithmetic uses the element
type's native operators (which wrap on integer overflow in release builds, as
everywhere in Rust).

```rust
pub enum ReduceOp {
    Sum,
    Product,
    Max,
    Min,
}
```

##### Variants

###### `Sum`

Sum — `MPI_SUM`.

###### `Product`

Product — `MPI_PROD`.

###### `Max`

Maximum — `MPI_MAX`.

###### `Min`

Minimum — `MPI_MIN`.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ReduceOp { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ReduceOp) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Traits

#### Trait `Reducible`

A datatype that supports the predefined [`ReduceOp`] reductions.

Implemented for the numeric primitives (all ten [`MpiPrimitive`] types). This
is a compiler-checked contract used to bound [`Communicator::reduce`] /
[`Communicator::all_reduce`]; it is never a `dyn` object.

```rust
pub trait Reducible: MpiPrimitive {
    /* Associated items */
}
```

> This trait is not object-safe and cannot be used in dynamic trait objects.

##### Required Items

###### Required Methods

- `combine`: Combine two values under `op`.

##### Implementations

This trait is implemented for the following types:

- `i8`
- `i16`
- `i32`
- `i64`
- `u8`
- `u16`
- `u32`
- `u64`
- `f32`
- `f64`

## Module `comm_mgmt`

Communicator management — duplication and splitting.

These are the collective communicator-creation calls: every rank of the parent
communicator participates, and the result is a *new* communicator with its own
isolated communication context.

- [`Communicator::dup`] mirrors `MPI_Comm_dup`: same group, fresh context (so a
  library can message internally without colliding with the caller's tags).
- [`Communicator::split`] mirrors `MPI_Comm_split`: partition the ranks by
  `color` into disjoint sub-communicators, ordered within each by `key`
  (ties broken by old rank). A rank passing [`Communicator::UNDEFINED`] as its
  color joins no group and gets `None`.

# How the group agrees on a context

A fresh context id must be process-unique *and* identical across every rank of
the new group. One rank ([`allocates`](crate::transport) via the transport's
atomic counter) and the value is distributed with a [broadcast](Communicator::broadcast)
(`dup`) or a small broadcast map keyed by color (`split`), so the whole group
ends up on the same context. Sub-communicator ranks are renumbered `0..k`, and
the handle records the local→global mailbox mapping so point-to-point and
collective addressing stay correct.

# Provenance

`MPI_Comm_dup` / `MPI_Comm_split` semantics per the MPI-3.1 standard; the
all-gather-then-local-recompute strategy for `split` mirrors MPICH's
`MPIR_Comm_split_impl`. Untrusted AI draft, verification-only.

```rust
pub mod comm_mgmt { /* ... */ }
```

## Module `communicator`

Communicators and point-to-point communication.

A [`Communicator`] is a rank's handle onto the shared [transport](crate::transport):
it knows the rank's own id, the group size, and the communicator context id
that isolates its messages from those of other communicators. This is the
MPICH `MPID_Comm` analogue, minus process groups/topologies (a later bead).

# Point-to-point

- Blocking [`Communicator::send`] / [`Communicator::recv`] mirror `MPI_Send` /
  `MPI_Recv`. The send is buffered (it copies into the receiver's mailbox and
  returns), so a matched pair never deadlocks regardless of order — the
  shared-memory analogue of an eager protocol.
- Non-blocking [`Communicator::isend`] / [`Communicator::irecv`] mirror
  `MPI_Isend` / `MPI_Irecv`, returning a [`Request`] completed with
  [`Request::wait`] (`MPI_Wait`) or polled with [`Request::test`] (`MPI_Test`).

The buffer/count/datatype triple is expressed idiomatically: sends take a
typed slice `&[T]`, receives return a typed `Vec<T>` plus a [`Status`], and the
datatype tag travels with the message so a receiver decoding the wrong type
gets an [`MpiError::TypeMismatch`] rather than silent reinterpretation.

# Wildcards

Pass [`ANY_SOURCE`] / [`ANY_TAG`] to a receive to match any sender / any tag,
mirroring `MPI_ANY_SOURCE` / `MPI_ANY_TAG`. The [`Status`] reports the actual
source and tag that matched.

```rust
pub mod communicator { /* ... */ }
```

### Types

#### Struct `Status`

Outcome of a completed receive: which message actually matched.

Mirrors `MPI_Status` (the fields a program reads: `MPI_SOURCE`, `MPI_TAG`, and
the element count from `MPI_Get_count`).

```rust
pub struct Status {
    pub source: i32,
    pub tag: i32,
    pub count: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `source` | `i32` | Rank of the sender of the matched message. |
| `tag` | `i32` | Tag of the matched message. |
| `count` | `usize` | Number of elements received. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Status { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Status) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Communicator`

A rank's handle onto the shared-memory message fabric for one communicator.

Cloning is cheap (a couple of `Arc` bumps plus a few integers) and yields
another handle onto the *same* communicator context — used internally to hand
a context to a pending [`Request`]. Construct rank handles with [`crate::run`],
not directly.

# Local vs global ranks

A communicator numbers its members `0..size` — *local* ranks. In a
sub-communicator (from [`Communicator::split`]) those differ from the global
thread ids that index the transport mailboxes, so the handle carries both:
`world_rank` (this thread's own mailbox index) and `world_ranks` (local rank →
global mailbox index) to address peers correctly. For the world communicator
the mapping is the identity.

```rust
pub struct Communicator {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn barrier(self: &Self) -> MpiResult<()> { /* ... */ }
  ```
  Block until every rank in the communicator has called `barrier` (mirrors

- ```rust
  pub fn broadcast<T: MpiPrimitive>(self: &Self, data: Option<&[T]>, root: i32) -> MpiResult<Vec<T>> { /* ... */ }
  ```
  Broadcast a buffer from `root` to every rank (mirrors `MPI_Bcast`).

- ```rust
  pub fn reduce<T: Reducible>(self: &Self, sendbuf: &[T], op: ReduceOp, root: i32) -> MpiResult<Option<Vec<T>>> { /* ... */ }
  ```
  Element-wise reduce every rank's `sendbuf` onto `root` under `op` (mirrors

- ```rust
  pub fn all_reduce<T: Reducible>(self: &Self, sendbuf: &[T], op: ReduceOp) -> MpiResult<Vec<T>> { /* ... */ }
  ```
  Element-wise reduce onto every rank (mirrors `MPI_Allreduce`).

- ```rust
  pub fn scatter<T: MpiPrimitive>(self: &Self, sendbuf: Option<&[T]>, count: usize, root: i32) -> MpiResult<Vec<T>> { /* ... */ }
  ```
  Scatter equal-sized `count`-element chunks of `root`'s buffer to each rank

- ```rust
  pub fn gather<T: MpiPrimitive>(self: &Self, sendbuf: &[T], root: i32) -> MpiResult<Option<Vec<T>>> { /* ... */ }
  ```
  Gather each rank's equal-length `sendbuf` onto `root`, concatenated in rank

- ```rust
  pub fn all_gather<T: MpiPrimitive>(self: &Self, sendbuf: &[T]) -> MpiResult<Vec<T>> { /* ... */ }
  ```
  Gather every rank's `sendbuf` onto **all** ranks, concatenated in rank order

- ```rust
  pub fn dup(self: &Self) -> MpiResult<Communicator> { /* ... */ }
  ```
  Duplicate this communicator: a new communicator over the **same group of

- ```rust
  pub fn split(self: &Self, color: i32, key: i32) -> MpiResult<Option<Communicator>> { /* ... */ }
  ```
  Split this communicator into disjoint sub-communicators by `color`, ordered

- ```rust
  pub fn rank(self: &Self) -> i32 { /* ... */ }
  ```
  This rank's id within the communicator, `0..size` (mirrors `MPI_Comm_rank`).

- ```rust
  pub fn size(self: &Self) -> i32 { /* ... */ }
  ```
  Number of ranks in the communicator (mirrors `MPI_Comm_size`).

- ```rust
  pub fn send<T: MpiPrimitive>(self: &Self, data: &[T], dest: i32, tag: i32) -> MpiResult<()> { /* ... */ }
  ```
  Blocking send of `data` to `dest` with `tag` (mirrors `MPI_Send`).

- ```rust
  pub fn recv<T: MpiPrimitive>(self: &Self, source: i32, tag: i32) -> MpiResult<(Vec<T>, Status)> { /* ... */ }
  ```
  Blocking receive of a message from `source` with `tag` (mirrors `MPI_Recv`).

- ```rust
  pub fn recv_into<T: MpiPrimitive>(self: &Self, buf: &mut [T], source: i32, tag: i32) -> MpiResult<Status> { /* ... */ }
  ```
  Blocking receive into a caller-provided buffer (mirrors `MPI_Recv` with an

- ```rust
  pub fn isend<T: MpiPrimitive>(self: &Self, data: &[T], dest: i32, tag: i32) -> MpiResult<Request<T>> { /* ... */ }
  ```
  Non-blocking send (mirrors `MPI_Isend`). Because the shared-memory transport

- ```rust
  pub fn irecv<T: MpiPrimitive>(self: &Self, source: i32, tag: i32) -> MpiResult<Request<T>> { /* ... */ }
  ```
  Non-blocking receive (mirrors `MPI_Irecv`). Returns a [`Request`] that is

- ```rust
  pub fn group(self: &Self) -> Group { /* ... */ }
  ```
  The group of this communicator: its members' world ranks in local-rank

- ```rust
  pub fn create_from_group(self: &Self, group: &Group) -> MpiResult<Option<Communicator>> { /* ... */ }
  ```
  Collectively create a new communicator over the processes in `group`

- ```rust
  pub fn cart_create(self: &Self, dims: &[i32], periods: &[bool]) -> MpiResult<Option<CartesianComm>> { /* ... */ }
  ```
  Create a Cartesian topology of shape `dims` (with per-dimension `periods`

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Communicator { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Request`

A handle to an outstanding non-blocking operation (mirrors `MPI_Request`).

Complete a send request with [`Request::wait`] (a no-op) or a receive request
with [`Request::wait`] (blocks for the message) / [`Request::test`] (polls).
The payload type `T` is fixed when the request is created.

```rust
pub struct Request<T: MpiPrimitive> {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn wait(self: Self) -> MpiResult<Option<(Vec<T>, Status)>> { /* ... */ }
  ```
  Block until the operation completes and return its result (mirrors

- ```rust
  pub fn test(self: &mut Self) -> MpiResult<TestOutcome<T>> { /* ... */ }
  ```
  Poll for completion without blocking (mirrors `MPI_Test`).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Send**
- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `TestOutcome`

Result of polling a [`Request`] with [`Request::test`].

```rust
pub enum TestOutcome<T: MpiPrimitive> {
    SendComplete,
    Pending,
    RecvComplete(Vec<T>, Status),
}
```

##### Variants

###### `SendComplete`

A send request — already complete, no data.

###### `Pending`

A receive that has not yet matched a message.

###### `RecvComplete`

A receive that completed, carrying its payload and status.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<T>` |  |
| 1 | `Status` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> TestOutcome<T> { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TestOutcome<T>) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Constants and Statics

#### Constant `ANY_SOURCE`

Wildcard source: a receive with this source matches a message from any rank
(mirrors `MPI_ANY_SOURCE`).

```rust
pub const ANY_SOURCE: i32 = -1;
```

#### Constant `ANY_TAG`

Wildcard tag: a receive with this tag matches a message with any tag (mirrors
`MPI_ANY_TAG`).

```rust
pub const ANY_TAG: i32 = -1;
```

## Module `datatype`

MPI datatypes for the primitive Rust numeric types.

The MPI standard describes every message by the triple `(buffer, count,
datatype)`. Because messages of different element types share one untyped
mailbox in the [transport](crate::transport), this crate carries each message
as a byte buffer tagged with a [`Datatype`] and an element `count`; the
receiver decodes those bytes back into a typed slice, checking the tag so a
type mismatch is an error rather than silent reinterpretation (mirroring
`MPI_ERR_TYPE`).

# Enum dispatch, not `dyn`

Per the workspace rules the set of built-in datatypes is a closed [`Datatype`]
enum, matched exhaustively. The [`MpiPrimitive`] trait is a *compiler-checked
contract* on each concrete primitive (it maps the Rust type to its `Datatype`
tag and byte codec); it is never used as a trait object.

# Provenance

The built-in datatype set corresponds to the MPI-3.1 standard named predefined
datatypes (`MPI_INT32_T`, `MPI_DOUBLE`, …). Byte encoding is native-endian,
valid because this shared-memory transport never crosses machine boundaries; a
future TCP transport would negotiate/convert endianness.

```rust
pub mod datatype { /* ... */ }
```

### Types

#### Enum `Datatype`

A built-in MPI datatype tag — one per supported primitive Rust type.

The set is closed and matched exhaustively (no `dyn`). Each variant names the
element type carried in a message's byte buffer and its fixed size in bytes.

```rust
pub enum Datatype {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
}
```

##### Variants

###### `I8`

`i8` — MPI `MPI_INT8_T`.

###### `I16`

`i16` — MPI `MPI_INT16_T`.

###### `I32`

`i32` — MPI `MPI_INT32_T`.

###### `I64`

`i64` — MPI `MPI_INT64_T`.

###### `U8`

`u8` — MPI `MPI_UINT8_T`.

###### `U16`

`u16` — MPI `MPI_UINT16_T`.

###### `U32`

`u32` — MPI `MPI_UINT32_T`.

###### `U64`

`u64` — MPI `MPI_UINT64_T`.

###### `F32`

`f32` — MPI `MPI_FLOAT`.

###### `F64`

`f64` — MPI `MPI_DOUBLE`.

##### Implementations

###### Methods

- ```rust
  pub fn size(self: Self) -> usize { /* ... */ }
  ```
  Size of one element of this datatype, in bytes.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Datatype { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Datatype) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Traits

#### Trait `MpiPrimitive`

Compiler-checked contract mapping a primitive Rust type to its [`Datatype`]
tag and a native-endian byte codec.

Implemented for the ten built-in numeric primitives. This is a bound on
generic message operations (`comm.send::<f64>(…)`), never a `dyn` object.

# Safety of the byte codec

[`MpiPrimitive::encode`]/[`MpiPrimitive::decode`] round-trip a slice through a
native-endian byte buffer. They are pure-safe Rust (`to_ne_bytes` /
`from_ne_bytes`), so there is no `unsafe`, no alignment hazard, and no
undefined behaviour on malformed input (a short trailing chunk is rejected).

```rust
pub trait MpiPrimitive: Copy + Send + Sync + ''static {
    /* Associated items */
}
```

> This trait is not object-safe and cannot be used in dynamic trait objects.

##### Required Items

###### Associated Constants

- `DATATYPE`: The [`Datatype`] tag for this Rust type.

###### Required Methods

- `push_bytes`: Append the native-endian bytes of one value to `out`.
- `from_bytes`: Read one value from exactly [`Datatype::size`] native-endian bytes.

##### Provided Methods

- ```rust
  fn encode(data: &[Self]) -> Vec<u8> { /* ... */ }
  ```
  Encode a typed slice into a native-endian byte buffer.

- ```rust
  fn decode(bytes: &[u8]) -> Vec<Self> { /* ... */ }
  ```
  Decode a native-endian byte buffer back into a typed `Vec`.

##### Implementations

This trait is implemented for the following types:

- `i8`
- `i16`
- `i32`
- `i64`
- `u8`
- `u16`
- `u32`
- `u64`
- `f32`
- `f64`

## Module `error`

Crate error type.

Every fallible operation in this crate returns [`MpiResult`], whose error
variant is [`MpiError`]. The variants mirror the failure conditions the MPI
standard reports through error classes (`MPI_ERR_RANK`, `MPI_ERR_TRUNCATE`,
`MPI_ERR_TYPE`, `MPI_ERR_ARG`), adapted to Rust's `Result` idiom rather than
MPI's integer error codes + error handlers.

```rust
pub mod error { /* ... */ }
```

### Types

#### Enum `MpiError`

An error from an MPI-subset operation.

These correspond to the MPI standard's error classes but are surfaced as a
typed `Result` error rather than an integer code:
- [`MpiError::InvalidRank`] ↔ `MPI_ERR_RANK`
- [`MpiError::Truncated`] ↔ `MPI_ERR_TRUNCATE`
- [`MpiError::TypeMismatch`] ↔ `MPI_ERR_TYPE`
- [`MpiError::InvalidArgument`] ↔ `MPI_ERR_ARG` / `MPI_ERR_COUNT`

```rust
pub enum MpiError {
    InvalidRank {
        rank: i32,
        size: i32,
    },
    Truncated {
        buffer: usize,
        message: usize,
    },
    TypeMismatch {
        expected: crate::datatype::Datatype,
        actual: crate::datatype::Datatype,
    },
    InvalidArgument(String),
    Transport(String),
}
```

##### Variants

###### `InvalidRank`

A rank argument was outside `0..size` for the communicator it was used with.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `rank` | `i32` | The offending rank value. |
| `size` | `i32` | The communicator size it was checked against. |

###### `Truncated`

A receive buffer was smaller than the message that arrived (the message
would be truncated). Mirrors `MPI_ERR_TRUNCATE`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `buffer` | `usize` | Capacity of the receive buffer, in elements. |
| `message` | `usize` | Number of elements in the incoming message. |

###### `TypeMismatch`

The datatype of a received message did not match the datatype the receiver
asked to decode it as. Mirrors `MPI_ERR_TYPE`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `expected` | `crate::datatype::Datatype` | The datatype the receiver requested. |
| `actual` | `crate::datatype::Datatype` | The datatype the sender used. |

###### `InvalidArgument`

A generic invalid argument (bad count, mismatched root, empty operation, …).
Mirrors `MPI_ERR_ARG` / `MPI_ERR_COUNT`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Transport`

The shared-memory transport failed (e.g. a rank thread panicked while a
peer was blocked waiting on it).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> MpiError { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Error**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &MpiError) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Type Alias `MpiResult`

Convenience `Result` alias for this crate's fallible operations.

```rust
pub type MpiResult<T> = core::result::Result<T, MpiError>;
```

## Module `group`

Process groups and group-based communicator creation.

An MPI [`Group`] is an **ordered set of processes** — here, an ordered list of
world ranks — decoupled from any communication context. Groups are manipulated
with purely *local* set operations (no messages): include/exclude a subset,
union/intersection/difference two groups, or translate a rank from one group's
numbering to another's. A communicator is then created from a group with the
collective [`Communicator::create_from_group`] (mirrors `MPI_Comm_create`).

This mirrors the MPI-3.1 group API: [`Communicator::group`] ↔ `MPI_Comm_group`,
[`Group::incl`]/[`Group::excl`] ↔ `MPI_Group_incl`/`_excl`,
[`Group::union`]/[`Group::intersection`]/[`Group::difference`] ↔ the set ops,
[`Group::translate_ranks`] ↔ `MPI_Group_translate_ranks`.

# Provenance

MPI-3.1 group semantics; set operations preserve MPI's ordering rules (union
keeps the first group's order then appends the second group's new members;
intersection/difference keep the first group's order). Untrusted AI draft,
verification-only.

```rust
pub mod group { /* ... */ }
```

### Types

#### Struct `Group`

An ordered set of processes, identified by their **world ranks**.

Group rank `i` (`0..size`) maps to world rank `ranks()[i]`. World ranks within
a group are distinct. A group carries no communication context — build a
communicator from it with [`Communicator::create_from_group`].

```rust
pub struct Group {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn world_ranks(self: &Self) -> &[i32] { /* ... */ }
  ```
  The world ranks of this group, in group-rank order.

- ```rust
  pub fn size(self: &Self) -> i32 { /* ... */ }
  ```
  Number of processes in the group (mirrors `MPI_Group_size`).

- ```rust
  pub fn rank_of(self: &Self, world_rank: i32) -> Option<i32> { /* ... */ }
  ```
  This group's rank of the process with the given world rank, or `None` if it

- ```rust
  pub fn incl(self: &Self, indices: &[i32]) -> Group { /* ... */ }
  ```
  Subgroup containing the members at the given **group-rank indices**, in the

- ```rust
  pub fn excl(self: &Self, indices: &[i32]) -> Group { /* ... */ }
  ```
  Subgroup with the members at the given group-rank indices **removed**,

- ```rust
  pub fn union(self: &Self, other: &Group) -> Group { /* ... */ }
  ```
  Union: this group's members, then the other's members not already present

- ```rust
  pub fn intersection(self: &Self, other: &Group) -> Group { /* ... */ }
  ```
  Intersection: members in both groups, in this group's order (mirrors

- ```rust
  pub fn difference(self: &Self, other: &Group) -> Group { /* ... */ }
  ```
  Difference: members of this group not in the other, in this group's order

- ```rust
  pub fn translate_ranks(self: &Self, ranks: &[i32], other: &Group) -> Vec<Option<i32>> { /* ... */ }
  ```
  Translate each of `ranks` (group ranks in *this* group) into the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Group { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Group) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `topology`

Cartesian process topologies.

A Cartesian topology maps the ranks of a communicator onto an N-dimensional
grid so a stencil code can find neighbours by coordinate rather than by hand.
[`Communicator::cart_create`] (mirrors `MPI_Cart_create`) builds a
[`CartesianComm`] over the first `dims.product()` ranks; on it,
[`CartesianComm::coords`] / [`CartesianComm::rank`] convert between a rank and
its grid coordinates (row-major: the last dimension varies fastest, as in MPI),
and [`CartesianComm::shift`] (mirrors `MPI_Cart_shift`) returns the
`(source, dest)` ranks for a shift along one dimension — exactly the pair a
halo exchange needs.

Dimensions may be **periodic** (wrap-around, e.g. a torus) or not; at a
non-periodic edge a shift's off-grid neighbour is `None` (MPI's
`MPI_PROC_NULL`).

# Provenance

MPI-3.1 Cartesian-topology semantics (`MPI_Cart_create`/`_coords`/`_rank`/
`_shift`), row-major coordinate ordering. Reordering for locality
(`MPI_Cart_create`'s `reorder` flag) is not performed — ranks keep their
identity. Untrusted AI draft, verification-only.

```rust
pub mod topology { /* ... */ }
```

### Types

#### Struct `CartesianComm`

A communicator with an attached N-dimensional Cartesian grid topology.

Wraps the underlying [`Communicator`] (the first `dims.product()` ranks of the
parent) plus the grid `dims` and per-dimension `periods` (wrap-around) flags.

```rust
pub struct CartesianComm {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn comm(self: &Self) -> &Communicator { /* ... */ }
  ```
  The underlying communicator (for point-to-point and collectives).

- ```rust
  pub fn dims(self: &Self) -> &[i32] { /* ... */ }
  ```
  The grid dimensions.

- ```rust
  pub fn periods(self: &Self) -> &[bool] { /* ... */ }
  ```
  The per-dimension periodicity (wrap-around) flags.

- ```rust
  pub fn my_coords(self: &Self) -> Vec<i32> { /* ... */ }
  ```
  This rank's own grid coordinates.

- ```rust
  pub fn coords(self: &Self, rank: i32) -> Vec<i32> { /* ... */ }
  ```
  Grid coordinates of `rank` (row-major: the last dimension varies fastest),

- ```rust
  pub fn rank(self: &Self, coords: &[i32]) -> Option<i32> { /* ... */ }
  ```
  Rank at the given grid `coords`, or `None` if any coordinate is out of range

- ```rust
  pub fn shift(self: &Self, direction: usize, disp: i32) -> (Option<i32>, Option<i32>) { /* ... */ }
  ```
  The `(source, dest)` ranks for a shift of `disp` cells along dimension

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> CartesianComm { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `transport`

Shared-memory transport — the threads-as-ranks message layer.

This is the MPICH *device/ADI* analogue for a single process: each rank is a
thread, and every rank owns one **mailbox** (an inbox queue guarded by a
`Mutex` + `Condvar`). A send pushes an [`Envelope`] onto the destination
rank's mailbox and wakes any thread blocked on it; a receive scans its own
mailbox for the first envelope matching the requested `(communicator, source,
tag)` and, if none is present, blocks on the condition variable until one
arrives.

# Message matching and ordering

MPI guarantees that messages sent from one rank to another **on the same
communicator with the same tag are non-overtaking**: they are received in send
order. The mailbox preserves this because it is a FIFO queue and a receive
takes the *first* matching envelope — so two messages with the same
`(comm, src, tag)` are dequeued in the order they were enqueued. A wildcard
receive (`MPI_ANY_SOURCE` / `MPI_ANY_TAG`) matches the earliest envelope
satisfying the non-wildcard fields.

# Why shared-memory first

Threads-as-ranks needs no network stack, no launcher, and no C toolchain, so
it builds and runs on Android/Termux like any other pure-Rust library, and it
directly serves single-node multicore domain decomposition. A future TCP
transport (multi-node) will implement the same enqueue/match interface behind
a Cargo feature.

```rust
pub mod transport { /* ... */ }
```

## Functions

### Function `run`

Run `f` on `n_ranks` ranks and collect their results in rank order.

This is the runtime entry point — the analogue of launching an MPI program
with `mpiexec -n <n_ranks>`, except the ranks are threads in the current
process. Each rank thread invokes `f` with its own world [`Communicator`]
(`rank() in 0..n_ranks`, `size() == n_ranks`) and the returned values are
gathered into a `Vec` indexed by rank.

The ranks run inside a [scoped thread](std::thread::scope) scope, so `f` may
borrow from the caller's stack (no `'static` bound); it must be `Sync` because
every rank shares the one closure.

# Errors
[`MpiError::InvalidArgument`] if `n_ranks <= 0`.

# Panics
Propagates a panic from any rank thread (the whole run aborts), matching MPI's
"a failed process kills the job" behaviour.

# Examples
```
use outram_park_mpi::run;
// Each rank reports its own id; results come back in rank order.
let ids = run(3, |comm| comm.rank()).unwrap();
assert_eq!(ids, vec![0, 1, 2]);
```

```rust
pub fn run<F, R>(n_ranks: i32, f: F) -> MpiResult<Vec<R>>
where
    F: Fn(&Communicator) -> R + Sync,
    R: Send { /* ... */ }
```

## Re-exports

### Re-export `Reducible`

```rust
pub use collective::Reducible;
```

### Re-export `ReduceOp`

```rust
pub use collective::ReduceOp;
```

### Re-export `Communicator`

```rust
pub use communicator::Communicator;
```

### Re-export `Request`

```rust
pub use communicator::Request;
```

### Re-export `Status`

```rust
pub use communicator::Status;
```

### Re-export `TestOutcome`

```rust
pub use communicator::TestOutcome;
```

### Re-export `ANY_SOURCE`

```rust
pub use communicator::ANY_SOURCE;
```

### Re-export `ANY_TAG`

```rust
pub use communicator::ANY_TAG;
```

### Re-export `Datatype`

```rust
pub use datatype::Datatype;
```

### Re-export `MpiPrimitive`

```rust
pub use datatype::MpiPrimitive;
```

### Re-export `MpiError`

```rust
pub use error::MpiError;
```

### Re-export `MpiResult`

```rust
pub use error::MpiResult;
```

### Re-export `Group`

```rust
pub use group::Group;
```

### Re-export `CartesianComm`

```rust
pub use topology::CartesianComm;
```

