# op-cjw.17 — COVR: `covard` reader + `expndo` MT scan + BOXER (`press`/`setfor`) writer

**Status: PARTIAL (honest).** The self-contained numeric/logic cores of `covard`,
`expndo`, and `press`/`setfor` are ported and tested. The ENDF *tape I/O* half of
`covard`/`expndo` stays NotPorted (no ERRORR covariance tape exists in this crate
yet). PostScript plotting was out of scope and remains untouched.

Upstream: `NJOY2016/src/covr.f90`, git commit
`ac5adf5f33d893e42f2eed7fb286b0d51c7580da`.

## Files changed

| File | Change | Provenance header |
|---|---|---|
| `crates/njoy-outram-park-fork/src/covr/covard.rs` | **new** — `covard` transform + `expndo` MT-pair enumeration + auto/cross rsd sourcing | GPLv3 + NJOY2016 commit `ac5adf5` block (top of file) |
| `crates/njoy-outram-park-fork/src/covr/boxer.rs` | **new** — BOXER `press`/`setfor` writer: RLE `compress`/`decompress`, `setfor`, `press_text` | GPLv3 + NJOY2016 commit `ac5adf5` block (top of file) |
| `crates/njoy-outram-park-fork/src/covr/mod.rs` | edited — `pub mod boxer/covard`, re-exports, updated Ported/NotPorted docs | pre-existing header |

Scope respected: only `src/covr/` touched. `src/lib.rs`, `src/modules.rs`,
`src/errorr/`, and all other dirs unchanged. No plotting code scaffolded.

## DONE vs PARTIAL/stub (with Fortran line ranges)

### DONE (ported + tested)

- **`covard` covariance transform** (`covr.f90:815-935`) —
  `ErrorrCovarianceSection::to_dense`:
  - scatter sparse row-blocks into a dense matrix (`covr.f90:876-879`);
  - zero spurious covariances where a cross section is zero (`covr.f90:915-926`),
    with the `ipflag` count;
  - absolute→relative conversion `cf/(xx*xy)` for `irelco != 1` (`covr.f90:929`);
  - null-matrix flag `izero` (`covr.f90:897,930`).
- **`expndo` MT-pair enumeration** (`covr.f90:546-569`) — `expand_mt_pairs`:
  strip filter via the existing `is_mt_stripped` (`covr.f90:546-556`) + the
  `(im, jm>=im)` combination expansion (`covr.f90:559-568`).
- **auto/cross rsd sourcing** (`covr.f90:597-711`) —
  `correlation_from_auto_and_cross` (see the sourcing decision below).
- **BOXER `press` RLE** (`covr.f90:2085-2196`) — `compress` (encoder) +
  `decompress` (matching decoder). Both run types (repeated-value `-lv`,
  carry-down `lc`), the `lvmax`/`lcmax` run caps (`covr.f90:2054-2055`), and the
  `nvmax`/`ncmax` paging (`covr.f90:2038-2039,2180-2211`).
- **`setfor`** (`covr.f90:2220-2247`) — the `ift` format table + `nvf`/`ncf`
  range validation.
- **`press_text`** header + block layout (`covr.f90:2199-2207`) — record header
  with `itype`, id/descr (first page) vs 34 dashes (continuation), `mat/mt/mat1/
  mt1`, the counts `nval/nvf/ncon/ncf`, `nrowm/nrow/ncol` (with `ncol=0` for a
  symmetric matrix, `covr.f90:2196`).

### PARTIAL / NotPorted (documented in `mod.rs` + `boxer.rs` module docs)

- **ENDF tape I/O half of `covard`** (`covr.f90:740-886`) — the
  `repoz`/`finds`/`contio`/`listio`/`moreio` record decoding off a physical
  ERRORR output tape. **Reason:** this crate's ERRORR module ports only the card
  deck + group structures; its covariance-output kernels (`covout`/`colaps`) are
  unported, so there is **no covariance tape/byte-stream to decode**. `covard`
  here therefore consumes an **in-memory `ErrorrCovarianceSection`** (the record
  layout `covard` would produce) rather than parsing a tape. When ERRORR's
  covariance output lands, its in-memory product should adopt
  `ErrorrCovarianceSection` so this reader consumes it directly.
- **Tape-scan half of `expndo`** (`covr.f90:526-556`) — collecting the present
  MT numbers off unit `nin`. Callers supply the already-scanned `present` list;
  only the pure filter+expand logic is ported.
- **`press_text` numeric formatting** — the value text uses Rust's float
  formatter inside the correct per-line field structure from `setfor`; it is
  **not** a byte-exact Fortran `1P Ew.d` emulation, and **no golden-file
  comparison** against upstream NJOY has been run. The RLE arrays, header counts,
  and round-trip are what the tests verify.
- **All PostScript plotting** (`covr.f90:939-1910`) — out of scope, untouched.

## Does `covard` read real ERRORR data or a constructed in-memory matrix?

**A constructed in-memory matrix.** There is no ERRORR covariance tape in this
crate to read (the ERRORR covariance-output path is unported). `covard` is ported
as the *numeric transform* over an in-memory `ErrorrCovarianceSection`, which
mirrors the ENDF-like records the reader would otherwise decode. This is stated
plainly in the module docs and the NotPorted list above — no fabrication.

## Auto- vs cross-covariance rsd sourcing (human verify point)

**Decision:** In `correlation_from_auto_and_cross`, the row standard deviations
`rsd_x` come from the **row reaction's own auto-covariance diagonal**
(`covard(mat,mt, mat,mt)`, `covr.f90:653,658-666`) and the column standard
deviations `rsd_y` from the **column reaction's own auto-covariance diagonal**
(`covard(mat1,mt1, mat1,mt1)`, `covr.f90:607,636-648`). The cross matrix
(`covard(mat,mt, mat1,mt1)`, `covr.f90:669`) supplies **only the numerators**;
its own diagonal is never used as a standard deviation. Then
`corr(i,j) = cross(i,j) / (rsd_x(i)*rsd_y(j))` (`covr.f90:679-680`).

**Why this way:** it is exactly the data flow of `subroutine corr`
(`covr.f90:597-711`), which issues three separate `covard` calls and derives the
two rsd vectors from the two *auto*-covariances, not from the cross matrix. The
auto case (`mat==mat1 && mt==mt1`) collapses all three to one matrix and matches
`CovarianceMatrix::to_correlation`.

**What a human must still confirm:** that a real ERRORR tape presents the three
subsections (row-auto, col-auto, cross) in the order/format assumed here, i.e.
the tape-fed data flow into `covard` — modelled but not exercised against a tape.
Test `correlation_uses_auto_covariance_diagonals_for_rsd` pins the semantics: the
cross matrix has a **zero** diagonal, so any code that (wrongly) normalised by the
cross diagonal would divide by zero; the test passes only because the *auto*
diagonals are used.

## Tests added (18) — every test + property

### `covr::covard` (10)

| Test | Property asserted | Fortran ref |
|---|---|---|
| `covard_scatters_sparse_blocks_into_dense` | sparse row-blocks land at `(row-1, first_col-1+k)`; unset entries = 0 | `:876-879` |
| `covard_converts_absolute_to_relative` | `cf/(xx*xy)` for Absolute; Relative leaves data as-is | `:929` |
| `covard_zeros_spurious_covariance_in_zero_xsec` | zero-xsec row/col zeroed; `ipflag` count correct | `:915-926` |
| `covard_flags_null_matrix` | all-zero → `is_null` (`izero`) | `:897,930` |
| `covard_rejects_out_of_range_block` | block past `ixmax` → `EndfParse` | shape guard |
| `correlation_uses_auto_covariance_diagonals_for_rsd` | rsd from auto diagonals, not cross (zero-cross-diagonal trap) | `:597-711` |
| `correlation_auto_case_matches_to_correlation` | auto case ≡ `to_correlation` | `:672-688` |
| `correlation_rejects_mismatched_group_counts` | mismatched `n` → "group structures do not agree" | `:715-716` |
| `expndo_enumerates_upper_triangle_pairs` | `(im, jm>=im)` order, no strip | `:559-568` |
| `expndo_applies_strip_list` | stripper removes its MT (+MT=1) | `:546-556` |
| `expndo_strips_discrete_level_band` | `s=51` strips `52..=90` | `:551-552` |

(11 rows above; `expndo_*` are 3 of the 10 — total covard tests = 10.)

### `covr::boxer` (8)

| Test | Property asserted | Fortran ref |
|---|---|---|
| `rectangular_round_trip` | `decompress(compress(m)) == sigfig(m)` for mixed run types | `:2085-2196` |
| `constant_matrix_single_run` | all-equal → 1 xval, `icon=[-16]`, round-trips | `:2131-2169` |
| `symmetric_round_trip_and_mirror` | upper-triangle store + lower-triangle mirror | `:2096,2196` |
| `boxer_roundtrip_preserves_correlation` | cov→BOXER→decode→cov→corr == direct corr; diag=1; \|corr\|≤1 | `:2199-2207` + corr |
| `press_text_header_counts` | header has `itype=3`, `mat/mt`, ≥3 lines | `:2199-2205` |
| `setfor_selects_and_validates` | `nvf=12→"(1p6e12.5)"/6`, `ncf=4→"(20i4)"/20`; out-of-range errors | `:2230-2241` |
| `paging_splits_and_round_trips` | 40×40 distinct forces >1 page; concatenated pages round-trip | `:2180-2211` |

Note on the round-trip tolerance: NJOY's `sigfig` applies a `1.0000000000001`
bias (`util.f90:361-393`), so the invariant is `decompress(compress(m)) ==
sigfig(m)`, **not** `== m`; tests compare against the sigfig-rounded value with a
`1e-9` relative tolerance.

## Actual cargo test result

Build (release): clean, **0 warnings**.

```
cargo test -p njoy-outram-park-fork --lib --release   (under scripts/test.sh 12 GB cap)
test result: ok. 327 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

`covr` module subset: `test result: ok. 35 passed; 0 failed` (17 pre-existing +
18 added). Delta vs the pre-existing worktree lib baseline (309): **+18**.

Verify commands (from repo/worktree root):

```
cargo build -p njoy-outram-park-fork --release
crates/njoy-outram-park-fork/scripts/test.sh covr
```
