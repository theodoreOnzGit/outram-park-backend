# Predictions, written BEFORE the recorded comparison runs (gh:#1435, 2026-11-13)

Upstream: MPICH 4.2.0 (Ubuntu 24.04 apt `mpich 4.2.0-5build3`, device `ch4:ucx`),
driver `upstream_driver/mpich_driver.c`, unmodified library.

**What was read before predicting (source, not a run of the port).** This crate's
`collective.rs` (binomial `reduce`/`broadcast` on relative rank
`rel = (rank - root) mod size`; `all_reduce` = `reduce` onto rank 0 then
`broadcast`; `combine(op, acc, incoming)`), `group.rs`, `comm_mgmt.rs`,
`topology.rs`; MPICH tags `v3.3`..`v4.3.2` file lists under `src/mpi/coll/`
(`reduce_intra_binomial.c`, `allreduce_intra_recursive_doubling.c`,
`allreduce_intra_reduce_scatter_allgather.c` exist in all of them). Two
exploratory MPICH-only launches (n = 4 and n = 5, with a first version of the
test values that did not round, so it said nothing about order) were made while
getting the launcher to work; no port output had been compared with anything.

**Design.** One C driver, one Rust test with the same cases. Each rank prints a line;
floating-point values as the hex of their IEEE bit pattern. n = 2..8 ranks.
Test values `mk(r, j)` have exponents spread over 2^-78..2^78 so every sum rounds and
its association order shows in the bits.

| # | claim | predicted outcome |
|---|---|---|
| P1 | pt2pt: ring with `ANY_SOURCE`/`ANY_TAG` status (source, tag, count); non-overtaking and selective matching (tags 7, 8, 7, 8) | **100 % identical** (standard-mandated semantics) |
| P2 | `MPI_Reduce` MPI_SUM f64, roots 0, n-1, n/2, counts 1 and 3, MPICH **default** algorithm | **100 % bit-identical** at every n: MPICH picks `reduce_intra_binomial` for messages under 2 KiB, the same relative-rank tree; the sum of two operands is commutative in IEEE so `tmp op acc` vs `acc op tmp` gives the same bits |
| P3 | `MPI_Allreduce` MPI_SUM f64 (counts 1, 3) and f32 | the port (reduce onto 0 + bcast, a binomial tree) differs from MPICH's recursive doubling **only for non power-of-two n** (3, 5, 6, 7): for n = 2, 4, 8 the two trees associate identically (`(x0+x1)+(x2+x3)`). For n in {3,5,6,7} I expect **a majority of the f64/f32 cases to differ** in the last bits, at most a few ulp of the result's largest operand; for n = 5 MPICH computes `((x0+x1)+x2)+(x3+x4)`, the port `((x0+x1)+(x2+x3))+x4` |
| P4 | integer `MAX`/`MIN`/`PROD`, f64 `MAX`/`MIN` | 100 % identical |
| P5 | bcast, scatter, gather, allgather of i32; barrier completes | 100 % identical (data movement only) |
| P6 | `MPI_Group_incl/excl/union/intersection/difference/translate_ranks` on the world group, with an out-of-order include list | 100 % identical, with `MPI_UNDEFINED` equal to the port's `None` |
| P7 | `MPI_Comm_create` from a group; `MPI_Comm_split` with `MPI_UNDEFINED` colour and tied keys; `MPI_Comm_dup` | 100 % identical (new ranks ordered by key then old rank) |
| P8 | `MPI_Cart_create` 2-D (2x3 periodic/open, 3x2 open/periodic, 2x2 periodic), `coords`, `shift` including `disp = -2` across a periodic edge, `MPI_Cart_rank` on in-range or periodic coordinates; surplus ranks get `MPI_COMM_NULL` | 100 % identical |

**If P3 comes true the cause will be proven, not assumed:** by forcing MPICH's
algorithm with `MPIR_CVAR_ALLREDUCE_INTRA_ALGORITHM=recursive_doubling` and
`...=reduce_scatter_allgather` and `MPIR_CVAR_REDUCE_INTRA_ALGORITHM=binomial` and
showing which one the port's bits equal, and by a pure-Rust model of recursive doubling
that is checked against MPICH. The fix is then to translate
`MPIR_Allreduce_intra_recursive_doubling` into `all_reduce` (for the message sizes where
MPICH selects it).

Not compared, and why (repeated in the gap survey): derived datatypes (none exist in the
port), non-blocking collectives, `MPI_Reduce_scatter`, `MPI_Alltoall`, `MPI_Scan`,
`MPI_Dims_create`, error-class numbers, large messages (over 2 KiB, where MPICH switches
algorithm), real multi-node transports.

Any number outside these is reported as observed with its localisation. None of the
bounds above will be edited after the run.

---

# Addendum, written after the first port run, BEFORE the runs it refers to (2026-11-13)

**What the first run showed (recorded, not edited out).** With the `wide` value set
(exponents over 2^-78..2^78) 100 % of lines were identical, including every
`all_reduce`: that set is **insensitive** to association order (each sum is dominated by
its largest term), so P3 could not have failed on it. A `narrow` set (exponents within
+-3) was added as the sensitive one; **both are kept and both are reported**. On the
narrow set the port differs from MPICH in `allreduce_sum_f64` for 5 of 70 lines. P3
predicted "a majority of the non power-of-two cases"; observed: only n = 5. Working the
two trees by hand explains why, and it is a *prediction for n = 2..8 made before any
further run*: the binomial reduce+bcast and recursive doubling associate identically for
n = 2, 3, 4, 6, 7, 8 and differ only at n = 5 (`((x0+x1)+x2)+(x3+x4)` against
`((x0+x1)+(x2+x3))+x4`). n = 9 and above would add more.

| # | claim (new runs) | predicted outcome |
|---|---|---|
| P9 | add 40-lane `reduce`/`allreduce` sums (320 bytes, still under MPICH's 2 KiB short-message limit), compared by FNV-1a hash | `reduce` identical at all n; `allreduce` identical except n = 5 before the fix, identical everywhere after translating recursive doubling |
| P10 | `MPI_MAX`/`MPI_MIN` on f64 with `+0.0`/`-0.0` and with a NaN, through `reduce` and `allreduce` | MPICH's built-in is `inoutvec = MAX(invec, inoutvec)` with `MAX(a,b) = a > b ? a : b`, so on ties and NaN the **accumulator** (`inout`) wins. This port calls `combine(op, acc, incoming)`, i.e. the **incoming** value wins. **Predicted: `reduce` differs at n = 2 for the zeros case (sign of zero) and for the NaN case**; fixing the operand order to `combine(op, incoming, acc)` removes it |
| P12 | added after the second run (it showed P11 false, see below): `reduce` of 256, 257 and 1024 doubles at roots n-1 and n/2 | 256 doubles (2048 bytes, "short"): port identical (binomial rooted at `root`). 257 and 1024 doubles ("long", MPICH `reduce_scatter_gather`, always rooted at rank 0 then moved to `root`): port **differs for every n >= 3 where the rotated tree associates differently from the unrotated fold+butterfly**, i.e. root n-1 and n/2 at most n, and root 0 at n = 5; after the fix **all identical** |
| P11 (**FALSE, see the second-run note under this table**) | messages over 2 KiB (1024 doubles): MPICH's default switches from recursive doubling to `reduce_scatter_allgather`, whose per-element association is the *halving* butterfly, so even for n = 4 it differs from recursive doubling | recorded as a known remaining difference of the fix (not closed here), and filed as a child issue |

**Second-run note (written before P12's run).** The second MPICH run (with P9-P11's cases
and forced algorithms) showed: (a) MPICH's default `allreduce` at 8 KiB gives the same bits
as forced recursive doubling **and** as forced `reduce_scatter_allgather` at every n = 2..8,
so P11's reasoning (halving distance n/2 first) was wrong: MPICH's reduce-scatter halves
with `mask = 1` first, the same per-element association as recursive doubling. (b) MPICH's
default `reduce` at 8 KiB, n = 5, root 0 gave the same bits as its `allreduce`, not the
binomial's: the long-message `reduce` is `reduce_scatter_gather`. That is what P12
tests.
