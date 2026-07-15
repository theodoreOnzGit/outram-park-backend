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
| Driver skeleton (documents pipeline) | `covr.f90:49-506` | `run_with_deck` (returns `NotPorted`) |

### NOT ported (honest gap list)

| Piece | Upstream | Reason |
|---|---|---|
| **ERRORR tape reader** (`covard`) | `covr.f90:720-937` | Reads the ENDF-like covariance records off the ERRORR output tape (`contio`/`listio`/`moreio`/`finds`). The in-memory record layout is not independently verifiable here, so per the port brief the transform is validated against a documented `CovarianceMatrix` and the reader is a gap. |
| **MT-list tape scan** (`expndo`) | `covr.f90:508-576` | Depends on ENDF tape I/O. Only the pure strip *logic* (`is_mt_stripped`) is ported. |
| **BOXER-format library writer** (`press`, `setfor`) | `covr.f90:1991-2247` | Compressed covariance-library output formatting. |
| **All PostScript plotting** (`plotit`, `matshd`, `patlev`, `smilab`, `matmes`, `elem`, `mtno`, `truncg`, `copyst`) | `covr.f90:939-1599,1649-1910` | VIEWR figure generation — graphics for a target OUTRAM PARK does not support. Only the self-contained numeric pieces used *by* the plot path (`level`, the shade array) are ported. |

The registry entry point `covr::run()` (no-arg) returns `NotPorted("covr")`;
the deck-driven skeleton `run_with_deck(&CovrInput)` validates the deck and
returns `NotPorted("covr::run")`.

## Testing — status and results

Inline `#[cfg(test)]` unit tests (no `tests/` dir), run via
`crates/njoy-outram-park-fork/scripts/test.sh covr` (12 GB cap).

**Result (2026-07-15): 17 passed, 0 failed.** `cargo check -p
njoy-outram-park-fork --lib` is clean.

Key V&V checks (methodology + numbers):

- **cov → corr, 2×2 SPD** — `cov = [[4, 2], [2, 9]]` (det 32 > 0), `rsd = [2, 3]`.
  Hand result `corr = [[1, 1/3], [1/3, 1]]`; test confirms unit diagonal,
  off-diagonal = 1/3, symmetry, `|corr| ≤ 1` (tol 1e-12).
- **cov → corr, 3×3 SPD** — `cov = [[4,2,0],[2,9,-3],[0,-3,16]]` (leading minors
  4, 32, det 476 all > 0), `rsd = [2,3,4]`. Hand results `corr(0,1)=1/3`,
  `corr(1,2)=-0.25`, `corr(0,2)=0`; unit diagonal, symmetry, `|corr| ≤ 1`.
- **rsd = sqrt(diag)** — `diag(4,9,16,-1,0) → rsd = [2,3,4,0,0]` (non-positive
  diagonal → 0).
- **zero-variance group** — `cov = [[4,1],[1,0]]` gives `corr = [[1,0],[0,0]]`
  with no NaN (guards the `rsd*rsd ≠ 0` branch).
- **cross-reaction** — `cov_xy = [[6,0],[0,12]]`, `rsd_x = [2,3]`, `rsd_y=[3,4]`
  → unit diagonal; length mismatch errors ("group structures do not agree").
- **shade-level expansion** — default `tlev` with ndiv=1 → `[0.001, 0.1, 0.2,
  0.3, 0.6, 1.001]`; ndiv=2 subdivides each interval; non-increasing `tlev`
  rejected.
- **MT-strip** — worked examples `-4`, `-62`, `-102` from `covr.f90:137-143`.
- **`level` indexing** — signed 1-based level against the default shade array.

## What a human must verify (untrusted AI draft)

1. **Correlation transform semantics vs upstream** — confirm the auto- vs
   cross-covariance split (`rsd_x`, `rsd_y` sourcing in `subroutine corr`,
   `covr.f90:597-670`) matches how a real ERRORR tape feeds `covard`; the port
   models the *math* of `corr` but not its tape-fed data flow.
2. **The unported tape reader** (`covard`) — the record layout, the
   absolute→relative conversion (`covr.f90:929`), and the zero-xsec spurious
   covariance zeroing (`covr.f90:915-926`) are **not** implemented; a full COVR
   run needs them and an ENDF I/O layer.
3. **Card defaults / coercions** — spot-check `matype≠4→3`, `irelco≠1→relative`,
   `ndiv≤0→1`, and the `ncase` limits against `covr.f90:229,247,929`.
4. **End-to-end** — no golden-file comparison against upstream NJOY has been
   run; the tests are analytic hand checks only.

## References

- NJOY2016 manual §COVR (LA-UR-17-20093)
- `covr.f90` (NJOY2016, commit `ac5adf5f33d893e42f2eed7fb286b0d51c7580da`)
