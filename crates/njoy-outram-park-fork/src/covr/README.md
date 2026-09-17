# COVR — covariance post-processing and correlation reporting

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §COVR); upstream Fortran: `covr.f90` (2250 lines).

## Provenance

- **Upstream:** `NJOY2016/src/covr.f90`, git commit
  `ac5adf5f33d893e42f2eed7fb286b0d51c7580da`.
- **Licence:** NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence,
  GPL-compatible; these derivative files are distributed under **GPL-3.0-only**.
  This is a modified, non-LANL version, not endorsed by LANL/DOE. See the crate
  root `LICENSE.njoy` + `NOTICE`. Every ported `.rs` file carries the provenance
  header block.

## Theory

COVR is an **editing** module that stands to ERRORR as MATXSR/DTFR stand to
GROUPR: it consumes ERRORR's multigroup covariance output and performs two
largely independent functions (`covr.f90:49-66`):

1. **Report / library** — per-group relative standard deviations, the
   correlation matrix, and a condensed BOXER-format covariance library.
2. **Plotting** — VIEWR PostScript figures of the correlation matrix (shaded
   contour) and the standard-deviation vectors.

### The numeric heart (`subroutine corr`, `covr.f90:578-718`)

For a multigroup covariance matrix `cov(i,j)` over energy groups `i, j`:

- **Relative standard deviation** per group (`covr.f90:636-641`):

$$\mathrm{rsd}(i) = \sqrt{\mathrm{cov}(i,i)}$$

  with a non-positive diagonal element mapped to 0.

- **Correlation coefficient** (`covr.f90:679-680`):

$$\mathrm{corr}(i,j) = \frac{\mathrm{cov}(i,j)}{\mathrm{rsd}_x(i)\,\mathrm{rsd}_y(j)}$$

  For an **auto-covariance** (a reaction against itself) $\mathrm{rsd}_x = \mathrm{rsd}_y = \mathrm{rsd}$,
  giving the familiar $\mathrm{corr}(i,j) = \mathrm{cov}(i,j)/\sqrt{\mathrm{cov}(i,i)\,\mathrm{cov}(j,j)}$,
  with unit diagonal. For a **cross-covariance** between two reactions,
  $\mathrm{rsd}_x$ is the standard-deviation vector of the row reaction (MAT/MT)
  and $\mathrm{rsd}_y$ that of the column reaction (MAT1/MT1). Where either the
  numerator or the denominator is exactly zero, the correlation is set to 0.

COVR does **not** clamp correlations during the transform; the plotting stage
clamps to $[-1, 1]$ (`covr.f90:1371-1372`), reproduced by
`CorrelationMatrix::clamped`.

### Units

- Correlations are **dimensionless**, in $[-1, 1]$ for a physically consistent
  (positive-semidefinite) covariance.
- Relative standard deviations are **dimensionless** fractions.
- Covariance values are dimensionless when relative (COVR's normal case), or
  units of the observable squared when absolute.
- Energies (`epmin`, group boundaries) are in **eV**.

## Ported vs NOT ported

### Ported (`src/covr/`)

| Piece | Upstream | Rust |
|---|---|---|
| Input card deck (cards 1, 2, 2', 2a, 3a, 2b, 3b, 3c, 4) | `covr.f90:69-284` | `input.rs` — `CovrInput`, `CovrMode`, selector enums |
| Shade-level array expansion (`xlev`) | `covr.f90:289-305` | `PlotOptions::shade_levels` |
| MT-strip predicate | `covr.f90:546-553` | `is_mt_stripped` |
| `epmin` read-time scaling (`rdn`) | `covr.f90:164,222` | `PlotOptions::epmin_scaled` |
| Relative standard deviation (`rsd = sqrt(diag)`) | `covr.f90:636-641` | `CovarianceMatrix::relative_std_dev` |
| Covariance → correlation (auto) | `covr.f90:672-688` | `CovarianceMatrix::to_correlation` |
| Covariance → correlation (cross-reaction) | `covr.f90:679-680` | `correlation_cross` |
| Null-matrix test (`izero`) | `covr.f90:930` | `CovarianceMatrix::is_null` |
| Plot-stage clamp to [-1,1] | `covr.f90:1371-1372` | `CorrelationMatrix::clamped` |
| Plottability test (`ismall`) | `covr.f90:683` | `CorrelationMatrix::has_plottable_correlation` |
| Shade-level index (`level`) | `covr.f90:1601-1619` | `shade_level` |
| **Library-option driver** (`nout > 0`: tape classification, case loop, `corr`, the `press` call sequence) | `covr.f90:314-474, 578-718` | `library::run_library`; `run_with_deck` reads `tape<nin>` / writes `tape<nout>` |
| **ERRORR-tape reader** (the `finds`/`contio`/`listio` half of `covard`: group structure, MF=3 vectors, MF=33/34/35/40 subsection search) | `covr.f90:740-886` | `tape::{ErrorrTapeKind, read_group_structure, read_vector, read_covariance_rows, read_covariance_section}` |
| **`expndo` tape scan** (MTs present in `MF=mf35`) | `covr.f90:526-556` | `tape::present_mts` |
| **`covard` covariance transform** (scatter + zero-xsec zeroing + abs→rel) | `covr.f90:815-935` | `covard::ErrorrCovarianceSection::to_dense` |
| **`expndo` MT-pair enumeration** | `covr.f90:546-569` | `covard::expand_mt_pairs` |
| Auto/cross rsd sourcing (`subroutine corr` data flow) | `covr.f90:597-711` | `covard::correlation_from_auto_and_cross` |
| **BOXER `press` RLE** (encode + decode) | `covr.f90:2085-2196` | `boxer::compress` / `boxer::decompress` |
| **BOXER `setfor`** format selection | `covr.f90:2220-2247` | `boxer::setfor` |
| **BOXER `press` text layout** (byte-exact `1pEw.d`/`Fw.d`/`Iw` records) | `covr.f90:2199-2207` | `boxer_text::press_text` (+ `parse_boxer_text`, a reader for V&V) |

### NOT ported (out of scope)

| Piece | Upstream | Reason |
|---|---|---|
| **All PostScript plotting** (`plotit`, `matshd`, `patlev`, `smilab`, `matmes`, `elem`, `mtno`, `truncg`, `copyst`) and the plot-option `epmin` reset in `corr` | `covr.f90:628-643, 939-1599, 1649-1910` | VIEWR figure generation — graphics for a target OUTRAM PARK does not support. Only the self-contained numeric pieces used *by* the plot path (`level`, the shade array) are ported. `run_with_deck` returns `NotPorted("covr::plot")` for `nout <= 0`. |
| Binary (`nin < 0`) input tapes | `covr.f90:307` | The crate's ENDF reader is ASCII-only. |

Two upstream quirks are deliberately not reproduced: `covard` returns from
a lumped-component placeholder (`nmt = 0`, `covr.f90:820`) without updating
`izero`, and the library path takes `sqrt` of a diagonal without a sign guard
(`covr.f90:645-648`). The port treats a placeholder as a null matrix and a
negative diagonal as `rsd = 0`; neither case occurs on an ERRORR tape.

## Testing — status and results

- Unit tests: 42 in the modules' `#[cfg(test)]` blocks (`scripts/test.sh
  covr`) — the card deck, `covard`'s scatter/zeroing/abs→rel, the auto/cross
  rsd sourcing, `expndo` stripping, the BOXER codec round-trip and paging,
  the tape reader on a hand-built two-group tape, the Fortran field layout.
- **Oracle: `tests/covr_boxer_golden.rs`** against NJOY2016 `ac5adf5`
  `covr` run in the library option (`matype = 3` and `4`) on the nine
  ERRORR tapes of `reference-data/errorr/`; decks and outputs in
  `reference-data/covr/`.

**Measured (2026-09-10).** Tier 1 (NJOY's ERRORR tape in): the BOXER
library is **byte-identical** to NJOY's for all 18 runs — 9 materials
(H-2 relative and absolute, Be-9, Li-6, C-12, F-19, Si-30, U-234, U-238)
× 2 matrix types, 101 to 3818 lines each, including U-238's 66 pairs with
lumped `MT=851/852` and multi-page records, and Si-30's 122 suppressed null
pairs. Tier 2 (the crate's ERRORR on NJOY's PENDF → `to_tape` →
`Tape::write` → `Tape::read` → COVR): also **byte-identical** for all 18
(U-234/U-238 need `OUTRAM_PARK_NJOY_U23{4,8}_PENDF`). Tier 2 first came out
at 805/3818 identical lines for U-238: the crate's ERRORR `covout` zeroed
every output element below `eps = 1e-20`, whereas `errorr.f90:7563-7568`
uses `eps` only to bracket a row and writes interior sub-`eps` elements
verbatim (NJOY's U-238 tape carries `e-49`…`e-69` values), and `press`
encodes each such value as a distinct run. Removing the zeroing made the
tapes identical — an ERRORR-writer defect that only a downstream consumer
exposed.

## What a human must verify

1. **Scope** — the plot option is intentionally absent; confirm no OUTRAM
   PARK workflow needs the VIEWR figures.
2. **Multi-material cases** (`mat1 != mat`) — the oracle runs use one
   material per tape; the `mat1` path is exercised only by the unit tests.

## References

- NJOY2016 manual §COVR (LA-UR-17-20093)
- `covr.f90` (NJOY2016, commit `ac5adf5f33d893e42f2eed7fb286b0d51c7580da`)
