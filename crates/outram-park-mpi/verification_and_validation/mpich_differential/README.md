# outram-park-mpi vs MPICH: provenance, differential results and gap survey

gh:#1435 (epic #1336). Run 2026-11-13. Research, education and V&V only
(`RESPONSIBLE_USE.md`); AI-generated, untrusted until a human reviews it.

## 1. Provenance: which MPICH?

**Not determinable. Best-supported statement: MPICH 3.3 or later.**

The original crate (commit `74377bf28`, 2026-07-23) records no version, commit or
source file. It names `MPIR_Bcast_intra_binomial`, `MPIR_Reduce_intra_binomial` and
`MPIR_Comm_split_impl` in doc comments. Evidence, from a clone of
https://github.com/pmodels/mpich (tags listed with `git ls-tree`, files read with
`git show`):

| check | result |
|---|---|
| `bcast/bcast_intra_binomial.c`, `reduce/reduce_intra_binomial.c` exist | at `v3.3`, `v3.3.2`, `v3.4`, `v3.4.3`, `v4.0`, `v4.2.0`, `v4.3.2`; **absent at `v3.0`, `v3.2.1`** |
| `diff -w` of `reduce_intra_binomial.c` v3.3.2 vs v4.3.2 | only `errflag`/`MPI_Aint count`/`MPIR_THREADCOMM_RANK_SIZE` plumbing; the tree is identical |
| same for `bcast_intra_binomial.c`, `allreduce_intra_recursive_doubling.c`, `reduce_intra_reduce_scatter_gather.c` | plumbing only |
| `MPIR_Comm_split_impl` (all-gather of (color, key), stable sort by key) | present in v3.3.2 and v4.2.0 |
| a version string, tag, commit hash or file path anywhere in the crate | none |

So the port was written against **3.3 or later**; nothing separates 3.3 from 4.3.2,
because the algorithms it follows did not change. The "likely 4.x" in `NOTICE` is a
guess from the creation date and is labelled as one. The headers were corrected
(`NOTICE` "UPSTREAM VERSION", `src/collective.rs`, `src/lib.rs`). MPICH's `COPYRIGHT`
was read at v3.3.2 and v4.2.0 (it is Argonne's own permissive grant, not verbatim BSD)
and `NOTICE`'s earlier BSD-style claim was struck through and replaced.

## 2. Differential

**Upstream used:** MPICH **4.2.0** (Ubuntu 24.04 `mpich 4.2.0-5build3`, `ch4:ucx`),
unmodified. It is the version installable here, not the version the port came from.

**Launcher oddity (recorded so it is not rediscovered):** Debian's build links the PMIx
client, and hydra's `mpiexec` does not serve PMIx, so every rank came up as a
singleton world of size 1 (`MPI_Comm_size` = 1 under `mpiexec -n 4`). The MPICH
binaries are launched with Open MPI's `mpirun`, whose embedded PMIx server they join
(`ldd` shows only `libmpich.so.12`; `MPI_Get_library_version` prints MPICH 4.2.0).

**Method.** `upstream_driver/mpich_driver.c` and `tests/mpich_differential.rs` run the
same cases; each rank prints a line, floating-point values as IEEE bit patterns (hex),
long vectors as an FNV-1a hash of their bit patterns. 2 to 12 ranks. Output committed
in `refs/` (`scripts/run_mpich.sh` regenerates it). The test compares **line for line, no
tolerance**. Two test-value sets: `wide` (exponents 2^-78..2^78) and `narrow`
(exponents within +-3). `PREDICTIONS.md` was committed before the runs it covers
(commits `874a9c17e`, `3a9c33371`, `544f1ba32`).

**Reduction order matters and was proved, not assumed.** `refs/` also holds MPICH runs
with the algorithm forced through its CVARs (`MPIR_CVAR_ALLREDUCE_INTRA_ALGORITHM`,
`MPIR_CVAR_REDUCE_INTRA_ALGORITHM`); `forced_mpich_algorithms_identify_the_default_ones`
asserts what they show: MPICH's default `allreduce` is recursive doubling (whole run
identical to the forced one, also for 8 KiB); its default `reduce` is the binomial tree up
to 2048 bytes and reduce-scatter-gather above (256 doubles binomial, 257 and 1024
reduce-scatter-gather). A pure-arithmetic Rust model of recursive doubling reproduces
MPICH's bits for every rank (`recursive_doubling_model_*`).

### Results (n = 2..12, lines identical / lines compared)

Before = the crate as found (`all_reduce` = binomial reduce onto rank 0 + broadcast;
`reduce` always binomial). After = this change.

| case | before, wide | before, narrow | after (both) |
|---|---|---|---|
| p2p ring (ANY_SOURCE/ANY_TAG status), non-overtaking, selective matching | 77/77, 11/11 | 77/77, 11/11 | 77/77, 11/11 |
| `reduce` sum f64, roots 0, n-1, n/2, 1 and 3 lanes (66 lines); 40 lanes (11); max/min i32, prod i64 (33) | all identical | all identical | all identical |
| `reduce` +-0.0 and NaN max/min (operand order) | 11/11 | 11/11 | 11/11 |
| `all_reduce` max/min f64, sum f32 | 77/77 | 77/77 | 77/77 |
| **`all_reduce` sum f64, 1 and 3 lanes** | 154/154 | **99/154** | 154/154 |
| **`all_reduce` sum f64, 40 lanes (hash)** | 42/77 | **42/77** | 77/77 |
| **`all_reduce` sum f64, 1024 lanes (hash)** | 42/77 | **42/77** | 77/77 |
| **`all_reduce` +-0.0 and NaN max/min** | 4/77 | **4/77** | 77/77 |
| **`reduce` sum f64, 1024 lanes, root 0** | 7/11 | **7/11** | 11/11 |
| **`reduce` sum f64, 256/257/1024 lanes, roots n-1 and n/2** | 21/44 | **16/44** | 44/44 |
| bcast, scatter, gather, allgather (i32) | all identical | all identical | all identical |
| `Group` incl/excl/union/intersection/difference/translate_ranks (n >= 4) | 9/9, 9/9 | same | same |
| `create_from_group`, `split` (UNDEFINED colour, tied keys), `dup` | all identical | all identical | all identical |
| 2-D `cart_create`/coords/shift (periodic wrap, `disp = -2`)/rank, surplus ranks | 198/198 | 198/198 | 198/198 |
| **total lines per set** | **1317/1487** | **1257/1487** | **1487/1487** |

The `wide` set was insensitive to association order (each sum is dominated by its largest
term), so it passed even before the fix for the 1-and-3-lane sums; that is why the
`narrow` set was added after the first run. Both are kept and both reported. The 40- and
1024-lane `wide` hashes (42/77) differ before the fix because many lanes there do round.

### Cause of the differences (before)

* `all_reduce`: binomial reduce+bcast against MPICH's recursive doubling. For n = 2, 3, 4,
  6, 7, 8 and 12 the two trees associate every element identically; they differ at
  n = 5 (`((x0+x1)+x2)+(x3+x4)` against `((x0+x1)+(x2+x3))+x4`), 9, 10 and 11. Count of
  (n, lane) sums whose bits differ between the two orders, 40 lanes per n:
  n=5: 20, n=9: 29, n=10: 15, n=11: 23, all others 0 (87 of 440).
* `all_reduce` NaN/+-0.0: MPICH's own ranks disagree with each other (each combines
  `op(own, partner)`); the old schedule gave every rank the same value. Matching
  MPICH here means matching a rank-dependent result, which the translation does.
* `reduce` above 2048 bytes: MPICH uses Rabenseifner's reduce-scatter + gather, whose
  per-element order is the fold-then-butterfly rooted at rank 0 whatever `root` is; the
  binomial tree is rotated to `root`.

### Fixes (translating upstream)

* `Communicator::all_reduce` = `MPIR_Allreduce_intra_recursive_doubling` (v4.2.0).
* `Communicator::reduce` for more than 2048 bytes and at least `pof2` elements =
  `MPIR_Reduce_intra_reduce_scatter_gather` (v4.2.0), else the binomial tree as before.
* The operand order `op(accumulator, incoming)` (MPICH's `MPIR_Reduce_local(in, inout)`
  computes `op(inout, in)` for the built-ins, measured with +-0.0/NaN) is now one helper,
  `combine_into`, used by every path. No change in value for `reduce` as found (its order
  already agreed: 11/11).

### Predictions scored (`PREDICTIONS.md`)

| # | verdict |
|---|---|
| P1 pt2pt identical | **true** |
| P2 `reduce` sum identical at every n | **true** (n = 2..12, roots 0, n-1, n/2, up to 2048 bytes) |
| P3 `all_reduce` differs for a *majority* of non power-of-two cases; identical for powers of two | **half true**: powers of two identical; but of n = 3, 5, 6, 7 only n = 5 differed (3, 6, 7 associate like the binomial). The majority claim was wrong; the addendum (written after the first run, before the next) predicted "only n = 5" for n = 2..8, and that held |
| P4, P5, P6, P7, P8 identical | **true** |
| P9 `reduce` identical, `all_reduce` differs only at n = 5 (2..8) | **true** |
| P10 `reduce` max/min operand order differs from MPICH at n = 2 | **false**: the port's order already matched; my assumption about MPICH's macro argument order was wrong |
| P11 above 2 KiB MPICH's `allreduce` differs from recursive doubling at n = 4 | **false**: it is bit-identical to recursive doubling (the reduce-scatter halves with `mask = 1` first) |
| P12 long `reduce` differs, becomes identical after the fix | **true** (before: 16/44 narrow, 21/44 wide; after 44/44) |

### Moved numbers

* `all_reduce` (narrow, n = 2..12): 99/154 -> 154/154 (3 lanes or fewer), 42/77 ->
  77/77 (40 and 1024 lanes), 4/77 -> 77/77 (+-0/NaN max/min).
* `reduce` long: 7/11 -> 11/11 (1024 lanes, root 0); 16/44 -> 44/44 (roots n-1, n/2).
* Total lines identical: narrow 1257/1487 -> 1487/1487, wide 1317/1487 -> 1487/1487.
* Nothing else moved; no tolerance or gate was changed.

## 3. Gap survey (MPICH 4.2.0 `src/mpi/`: attr coll comm datatype debugger errhan group info init misc pt2pt request rma romio session spawn stream threadcomm topo)

| MPICH area | status in `outram-park-mpi` |
|---|---|
| `pt2pt` (send/recv, isend/irecv, wildcards, status) | ported, **bit-identical on the cases above**. Missing: `MPI_Probe`/`Iprobe`, `Sendrecv`, `Ssend`/`Bsend`/`Rsend`, `Waitall/any/some`, `Test*`, persistent requests, `Cancel` |
| `coll`, blocking: 17 operations (allgather, allgatherv, allreduce, alltoall, alltoallv, alltoallw, barrier, bcast, exscan, gather, gatherv, reduce, reduce_scatter, reduce_scatter_block, scan, scatter, scatterv) | **7 ported**; `reduce` (binomial, Rabenseifner) and `all_reduce` (recursive doubling) are now bit-identical, `bcast` is the binomial tree. **Not ported: allgatherv, alltoall, alltoallv, alltoallw, exscan, gatherv, reduce_scatter, reduce_scatter_block, scan, scatterv** (child issue) |
| `coll`, non-blocking (19 `i*` operations) and neighbour collectives (10) | not ported |
| `coll` algorithm variety: ring/Brucks allgather, scatter-allgather bcast, dissemination barrier, SMP-aware, tree/recexch | not translated (carry data or none; cannot change a result). Rabenseifner's `allreduce` has the same association as recursive doubling (measured), so it is not needed for bit-identity |
| `coll/op` (predefined ops) | `Sum`, `Product`, `Max`, `Min` over 10 numeric types. Missing: `Land/Lor/Lxor`, `Band/Bor/Bxor`, `Maxloc/Minloc`, `Replace`, complex and `bool` types, user-defined ops |
| `comm` | `dup`, `split` and `create_from_group` ported and identical. Missing: `Comm_create_group`, `split_type`, `compare`, `free`, names, inter-communicators, `idup`, `dup_with_info` |
| `group` | `incl/excl/union/intersection/difference/translate_ranks` identical. Missing: `range_incl/excl`, `compare`, `Group_free` (n/a), Rust `Group::incl` silently deduplicates where MPICH errors (error behaviour not compared) |
| `topo` | `cart_create/coords/rank/shift` identical for the shapes above. Missing: `Dims_create` (a real gap for the halo-exchange consumer), `Cart_sub`, `Cart_get`, `Cart_map`, `reorder = true`, graph and dist-graph topologies |
| `datatype` | only the 10 predefined numeric types (`typerep`, `type_create*`, pack/unpack, `Type_vector/indexed/struct/subarray/darray`, `get_elements`, `commit/free` are **absent**; there is nothing to compare for "datatype packing" beyond the native-endian codec) |
| `errhan`, error classes | `MpiError` enum, not the MPICH error classes/strings; abort-on-error behaviour (`MPI_ERRORS_ARE_FATAL`) not modelled; not compared |
| `attr`, `info`, `session`, `stream`, `threadcomm`, `spawn`, `rma`, `romio` (MPI-IO), `MPI_T` | not ported (no consumer) |
| `init` (`Init`, `Finalize`, `Wtime`, `Get_processor_name`, `Initialized`) | `run()` replaces `mpiexec`+`MPI_Init`; no `Wtime`, no thread-level query |
| network transports (`ch3`/`ch4`: TCP, OFI, UCX), shared-memory (`posix`/`xpmem`) | not ported by design: threads-as-ranks only |
| MPICH's own test suite (`test/mpi`, 817 C files) | not run. The tests that use only ported calls could be ported as Rust programs; no consumer yet |

Child issues filed for the large pieces: see the comment on #1435.

## 4. What this does not show

* One MPICH build (4.2.0, ucx) and one set of CVARs; other versions and other
  algorithm selections (e.g. the intra-node `release_gather` collectives MPICH can use in
  `ch4:ofi`/`posix` builds, node-aware `smp` algorithms on multi-node jobs) are untested.
* 2 to 12 ranks only, sums of 1 to 1024 doubles, `float`/`double`/`int`/`long long`.
  Counts below `pof2` elements (which make MPICH fall back from reduce-scatter) were not
  exercised at long sizes.
* The timing/ordering of nondeterministic receives (`ANY_SOURCE` arrival order) is not a
  semantic of MPI and was not compared.
* The MPICH source text was read for the algorithms translated (v4.2.0); a function-level
  line-by-line audit of the other ported routines (`bcast`, `split`, topologies) was not
  done: they were compared by behaviour only.
