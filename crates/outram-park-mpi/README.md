# outram-park-mpi

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

An independent, **pure-Rust translation of a subset of MPICH** (the Argonne
National Laboratory reference MPI implementation), providing the MPI-3 API
surface most simulations need over a **shared-memory, threads-as-ranks**
transport.

**Name.** `outram-park` is the workspace name, OUTRAM PARK: **O**pen-source
**U**nified **TR**Ansient **M**ulti-Physics **A**dvanced **R**eactor simulation
**K**it (`docs/ecosystem-naming.md`); `mpi` is the Message Passing Interface
standard whose API it implements. **The question it answers:** what does an MPI
library actually do when ranks send, reduce and split, and in what order does
it add the numbers?

**What this crate lets you study and challenge.** MPI is usually a black box
linked from C. Here the message matching, the binomial broadcast and reduce
trees, recursive-doubling all-reduce and Rabenseifner's long-message reduce are
short Rust functions over in-memory mailboxes, named after the MPICH routines
they follow (`MPIR_Bcast_intra_binomial`, `MPIR_Reduce_intra_binomial`,
`MPIR_Comm_split_impl`). The differential record shows why reduction order is
physics-relevant: before the fix, `all_reduce` of `f64` sums differed from
MPICH in the last bits for some rank counts (n = 5 among 2..8), and the record
keeps the predictions that were wrong. It also records honestly that the MPICH
version the port was first written from cannot be determined.

## Knowledge in this crate

See the workspace [vision](../../VISION.md).

| Knowledge artifact | In this crate |
|---|---|
| Scientific literature | Upstream code: [MPICH](https://github.com/pmodels/mpich) (Argonne's permissive licence, read at v3.3.2 and v4.2.0; see [`NOTICE`](NOTICE)). The original port's MPICH version is not determinable (best-supported: 3.3 or later); the collective algorithms added in gh:#1435 are translated from MPICH 4.2.0. The MPI-3 standard (MPI Forum) defines the API. No `crates/kovan-literature/CATALOGUE.md` entry is tied to this crate. |
| Mathematical equations | None as such. The algorithms are described in the module docs: binomial trees (`collective`), recursive doubling, reduce-scatter + gather above 2048 bytes, the stable sort by key in `split` (`comm_mgmt`), Cartesian coordinate and shift arithmetic (`topology`). |
| Source code | `transport` (mailboxes), `communicator`, `datatype`, `collective`, `comm_mgmt`, `group`, `topology`, `error`. |
| Interactive simulations | None yet. |
| Visualisations | None yet. |
| Verification and validation evidence | [`verification_and_validation/mpich_differential/`](verification_and_validation/mpich_differential/README.md) (run 2026-11-13, against unmodified MPICH 4.2.0, 2 to 12 ranks, line for line, no tolerance): 1487/1487 lines identical in both value sets after the fix (before: narrow 1257/1487, wide 1317/1487); predictions committed before the runs in `PREDICTIONS.md`; MPICH's own `test/mpi` suite not run. Test: `tests/mpich_differential.rs`. |
| Human reviews and corrections | 0 functions carry a kovan human-review stamp (no `kind = "review"` block in any `review.md` here). No entry in `docs/human-corrections-to-ai-work.md` names this crate. Struck-through corrections in this README, `src/lib.rs` and `NOTICE` record the collectives, communicator management and the MPICH licence reading (gh:#1435). |

**Status:** point-to-point, the seven collectives, groups, communicators and
Cartesian topologies have AI-run bit-for-bit evidence against MPICH 4.2.0 on
the cases recorded; ten MPICH collectives, probes and persistent requests are
not ported, and there is no multi-node transport. Nothing here has been
reviewed by a human.

> **⚠️ Scaffold / verification-only — no human V&V yet.** This is the first
> milestone: the shared-memory transport, datatypes, the world communicator, and
> point-to-point communication. ~~Collectives, communicator duplication/splitting,
> groups, and a TCP (multi-node) transport are follow-ups.~~ **CORRECTED 2026-11-13
> (gh:#1435):** collectives, duplication/splitting, groups and Cartesian topologies
> exist (see "API surface" below); only a TCP (multi-node) transport is a follow-up.
> ~~No comparison against MPICH's own test suite has been made.~~ **CORRECTED
> 2026-11-13 (gh:#1435):** the crate is compared bit for bit against **MPICH 4.2.0**
> for 2 to 12 ranks (results: `verification_and_validation/mpich_differential/`);
> MPICH's own test suite (`test/mpi`) has not been run. **The MPICH version the crate
> was translated from was never recorded and cannot be determined** (`NOTICE`:
> MPICH 3.3 or later). Untrusted AI-generated draft until a human reviews it, per the
> workspace `RESPONSIBLE_USE.md`.

> **Research, education and V&V only** (`RESPONSIBLE_USE.md`): not for nuclear
> facility operation, reactor control, licensing, safety-critical or operational
> use.
>
> **Independent translation, not MPICH.** "MPI" names only the standard whose API
> this implements; "MPICH" names the reference implementation whose semantics and
> (later) collective algorithms are translated. No MPICH C code is linked, and
> nothing here is endorsed by or affiliated with the MPICH project or Argonne
> National Laboratory.

## Why

A pure-Rust, Android-buildable message-passing layer for single-node multicore
domain decomposition — the foundation for MPI-style scale-out in the OUTRAM PARK
simulators (e.g. pflotran's `op-v6s.15.9`). Ranks are threads in one process
communicating through in-memory mailboxes: no C toolchain, no system MPI, no
network stack, so it builds and runs anywhere the rest of the workspace does,
Termux included.

## Example — a rank ring

```rust
use outram_park_mpi::{run, ANY_TAG};

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

## API surface (this milestone)

- `run(n_ranks, |comm| ...)` — launch `n_ranks` rank-threads, each with a world
  `Communicator`; collect results in rank order (the `mpiexec -n` analogue).
- `Communicator`: `rank()`, `size()`; blocking `send` / `recv` / `recv_into`;
  non-blocking `isend` / `irecv` returning a `Request` completed with `wait` /
  polled with `test`. `ANY_SOURCE` / `ANY_TAG` wildcards; `Status` reports the
  matched source, tag, and count.
- `Datatype` + `MpiPrimitive`: the ten built-in numeric primitives
  (`i8`…`i64`, `u8`…`u64`, `f32`, `f64`), native-endian byte codec, type-tag
  checking (a mismatched decode is an error, not silent reinterpretation).
- Collectives: `barrier`, `broadcast`, `reduce`, `all_reduce`, `scatter`,
  `gather`, `all_gather`, with the predefined `ReduceOp`s (`Sum`, `Product`,
  `Max`, `Min`) over any `Reducible` primitive. Broadcast/reduce use binomial
  trees; collective traffic runs on a separate communication context, so it can
  never cross-match user point-to-point messages. ~~`all_reduce` is reduce then
  broadcast.~~ **CORRECTED 2026-11-13 (gh:#1435):** `all_reduce` is recursive doubling
  and `reduce` of more than 2048 bytes is reduce-scatter + gather, as in MPICH.

- Communicators: `dup` (same group, fresh context) and `split` (partition by
  `color`, order by `key`); sub-communicators renumber ranks `0..k` and keep a
  local→global mailbox map so point-to-point and collectives address correctly.

- Groups: `Group` set operations (`incl`/`excl`/`union`/`intersection`/
  `difference`/`translate_ranks`) + `Communicator::create_from_group`
  (`MPI_Comm_create`).
- Cartesian topologies: `cart_create` → `CartesianComm` with `coords`/`rank`/
  `shift` — the last gives the `(source, dest)` neighbour pair a halo exchange
  needs, with per-dimension periodicity.

## Roadmap (epic `op-erl`)

- ~~Optimised collectives (recursive-doubling all-reduce, ring all-gather,
  dissemination barrier).~~ **CORRECTED 2026-11-13 (gh:#1435):** recursive-doubling
  `all_reduce` and Rabenseifner's long-message `reduce` are translated from MPICH 4.2.0
  and match its bits (`tests/mpich_differential.rs`); ring all-gather and the
  dissemination barrier move or carry no data and cannot change a result, so they stay
  untranslated. The gap survey is in `verification_and_validation/mpich_differential/README.md`.
- TCP (multi-node) transport behind a Cargo feature.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

## License

**AGPL-3.0-only** (`license.workspace = true` in `Cargo.toml`; relicensed
2026-10-20, gh:#973; see [`NOTICE`](NOTICE) for the MPICH attribution).
