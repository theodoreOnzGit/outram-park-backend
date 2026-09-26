# Writing the ACE UNR block — and the PURR grid defect it exposed

GitHub **#325**. Implemented and measured 2026-09-26.

## The gap

Since GitHub #307 `Nuclide::from_ace` **reads** the unresolved-range
probability tables (the UNR block, `JXS(23)`) and applies them by default. But
this crate's ACE **writer** emitted no UNR block at all — there was no `JXS(23)`
anywhere in `acer::build` — so a table written here could not carry URR
self-shielding even though both ends of the round trip could now handle it.
`outram-mc-libs`' `lct008_ace_roundtrip` had to ablate URR off its ENDF arm to
get a symmetric comparison, because it had no way to add URR to its ACE arm.

## What was implemented

| piece | where | ported from |
|---|---|---|
| the block writer | `acer::unr::unr_words` | `acefc.f90:5958-5990`, Type-1 typing `:13447-13458` |
| a constructor that takes the tables | `AceTable::from_reconr_full_with_urr` | — |
| the `…+PURR+ACER` deck | `acer::build_full_with_purr` | the reference library's own deck |
| competition flags `ILF`/`IOA` | `purr::tables::competition_flags` | `purr.f90:1110-1192` |
| heating column carried per band | `UrrPoint::heating` | `purr.f90:524-560` (the `ihave = 0` case) |
| **the unresolved energy grid** | `purr::tables::urr_energy_grid`, rewritten | `unresr.f90:426-748` (`rdunf2`) |

`build_full` is **unchanged** and still writes no UNR block: it is the
`RECONR+BROADR+ACER` deck, and several byte-parity gates compare it against NJOY
tables made without PURR. Its doc now says so, and points at
`build_full_with_purr` for any table meant for transport.

Two decisions:

- **The block goes immediately after DLW.** Measured on NJOY's own U-238 table:
  `DLW -> LUNR -> DNU -> BDD -> DNEDL -> DNED -> GPD -> MTRP`.
- **Heating is zero, deliberately and as upstream.** PURR fills the heating
  column from HEATR's MT=301 on the PENDF it reads; an ENDF evaluation carries no
  MT=301, which is upstream's `ihave = 0` case ("ur heating set to zero"). The
  reference deck has no HEATR either, and NJOY's own U-238 heating column is all
  zeros.

## Methodology

Three gates, `tests/unr_block_write_vs_njoy2016.rs`, each against NJOY2016:

1. **The writer, with PURR's randomness removed.** PURR samples resonance
   ladders, so generated tables cannot match NJOY's band for band. Instead:
   read NJOY's own UNR block from the reference U-234/235/238 tables, give those
   tables to `unr_words`, and require **every word** — bit pattern and integer
   typing — to equal NJOY's.
2. **The flags and the grid, which do not depend on the ladders.** Generate
   tables from the evaluation at minimal statistics and require NJOY's `ILF`,
   `IOA`, point count and every energy.
3. **The pipeline.** `build_full_with_purr` on U-234, serialised to Type-1
   **text**, parsed back by the production reader: the block must sit
   immediately after DLW, decode to the generated tables, and `build_full` must
   still write none.

## Results

| nuclide | block words | header `(N, M, INT, ILF, IOA, IFF)` | value / typing mismatches |
|---|---|---|---|
| U-234 | 3 152 | `(26, 20, 2, 51, −1, 0)` | **0 / 0** |
| U-235 | 2 305 | `(19, 20, 2, 4, 0, 1)` | **0 / 0** |
| U-238 | 10 049 | `(83, 20, 2, 51, 0, 1)` | **0 / 0** |

Bit-exact on all three, compared with `f64::to_bits`. The three headers happen
to exercise every branch of the flag rule — one inelastic competitor (`51`),
several (`4`), one absorption competitor… none (`−1`), several (`0`) — and both
values of `IFF`.

| nuclide | flags ours / NJOY | grid points ours / NJOY | worst relative energy difference |
|---|---|---|---|
| U-234 | `(51, −1)` / `(51, −1)` | **26 / 26** | 9.99e-14 |
| U-235 | `(4, 0)` / `(4, 0)` | **19 / 19** | 1.00e-13 |
| U-238 | `(51, 0)` / `(51, 0)` | **83 / 83** | 1.00e-13 |

The 1e-13 residual is NJOY's own `sigfig` bias factor (`1.0000000000001`), which
lives in upstream's in-memory grid too and does not survive the 12-digit Type-1
print — i.e. exact at the file's precision.

Pipeline: U-234 through `build_full_with_purr`, text round trip, UNR at the
right locator, 26 energies × 15 bands, worst sampled-band difference after the
round trip **4.5e-7** (the writer's 7-figure rounding); `build_full` writes
`JXS(23) = 0`.

## The defect this found: our PURR's energy grid was not NJOY's

Gate 2 failed on first run, and not because of the writer. Our PURR built
U-234's tables on **10** energies where NJOY's have **26**, and U-235's on
**14** against **19**. U-238 matched in count, and only at 4 printed figures.

Reading `rdunf2` (`unresr.f90:426-748`) showed four differences, three of them
control flow — the class of defect this workspace's CLAUDE.md warns is more
common in a port than a wrong formula:

1. **A missing refinement pass** (`:698-722`). Wherever two adjacent nodes are
   `>= 1.26×` apart, NJOY inserts points from a fixed 78-entry ladder of "round"
   energies (`egridu`: 1.0, 1.25, 1.5, 1.7, 2.0, 2.5, 3.0, 3.5, 4.0, 5.0, 6.0,
   7.2, 8.5 per decade). Never ported. U-238's evaluation grid is fine enough
   that the pass inserts nothing, which is why the earlier U-238 comparison
   passed and the omission went unseen.
2. **Case C takes nodes from the first `(l, j)` state only**, minus its first
   and last points (`:676-679`) — not, as our code and its comment had it, the
   union of every state.
3. **Case B has a node table** — the fission-width grid (`:585-594`) — which our
   code ignored in favour of 40 log-spaced points; **Case A** is refined across
   its whole range (`indep = 1`) rather than given those 40 points.
4. **The endpoints are shaded** one unit in the 7th figure inward
   (`sigfig(el,7,+1)`, `sigfig(eh,7,-1)`, `:506-513`). U-238 runs
   `20000.01 .. 149008.6` eV, not `20000 .. 149008.7`.

**Predicted before re-measuring:** 26, 19 and 83 points, shaded endpoints, all
three at the 7th figure. **Measured:** exactly that (table above).

The earlier claims — "the union of every J-state's energies", "reproduces point
for point (83 points on U-238)", "Cases A and B carry no per-energy table" — are
struck through in `purr/tables.rs` rather than deleted, with what was wrong about
each.

## What this changes downstream, and what is not yet re-measured

The ENDF route has applied these tables **by default** since 2026-09-20, so the
grid fix changes the physics of every nuclide whose unresolved grid was coarse:
here, **U-235 and U-234**. Every `k` measured with URR on since then was taken on
the old grid:

- Godiva `−10 ± 9 pcm` (400 seeds, 2026-09-20);
- Jemima `−395 pcm` (URR on);
- LCT-008 `+139 pcm` (URR + DBRC).

These are flagged in `outram-mc-libs/verification_and_validation/icsbep/README.md`
as taken on the pre-fix grid. A Godiva re-measurement at the same 400 seeds is the
next step and its result will be recorded here; until then no claim is made about
how far any of them moved.

## What this does not claim

- **Band shapes are not compared against NJOY.** PURR's bands come from random
  ladders; `tests/purr_u238_ptables_vs_njoy.rs` is the statistical comparison of
  those, and it still passes.
- **Multi-range and resolved-overlap evaluations** are not handled: upstream
  merges every LRU=2 range of every isotope into one grid and marks overlap
  nodes negative; this crate builds from the first range only. No held
  evaluation needs either, and both are stated in the function's docs.
