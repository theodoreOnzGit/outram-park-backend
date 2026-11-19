# Steffen and Akima interpolation against compiled GSL 2.8 (gh:#1341)

Parent: gh:#1341, epic gh:#1336. Research, education and V&V only
(`RESPONSIBLE_USE.md`). AI-assisted work: untrusted until a human reviews it.

## Methodology

- **Port:** `src/interp_piecewise.rs`, from GSL 2.8 `interpolation/steffen.c`,
  `akima.c` and `integ_eval.h` (commit `cf180cd7`, read 2026-11-13). Additive:
  `crate::interp::Interp` is unchanged (its private `integ_eval` became
  `pub(crate)` to be shared; no body change).
- **Reference:** `reference-data/gsl/interp_piecewise_reference_driver.c`, built
  against GSL 2.8 rebuilt from `ampl/gsl@cf180cd7` (`gcc -O2`, glibc, x86-64),
  output `reference-data/gsl/interp-piecewise-gsl-2.8-reference.txt`. It calls
  `gsl_interp_eval`, `_eval_deriv`, `_eval_deriv2` and `_eval_integ` with no
  accelerator, for `gsl_interp_steffen`, `gsl_interp_akima` and
  `gsl_interp_akima_periodic`, on five tables (Runge, a non-uniform cp-shaped
  table, a flat-step-flat table, a periodic sine table, and a table with equal
  consecutive slopes that reaches Akima's `NE == 0` branch); 241 evaluation
  points plus every knot per table and 231 ordered integration limits.
- **Replayed by** `tests/gsl_interp_piecewise_code_to_code.rs`, comparing bit
  patterns.
- **Pass criterion:** every row bit-identical. No libm function is called by
  either side on these paths (only `fabs`, which is exact), so no
  glibc-versus-PETIR allowance applies and none is claimed.

## Prediction (written 2026-11-13 BEFORE the port was run against the reference)

- **P1.** 100 % of the rows bit-identical for all three methods on all five
  tables. This can fail: it fails if a grouping in `(sim1*hi + si*him1) /
  (him1 + hi)`, `/ hi / hi`, the Akima `m[-2..size]` offsets (the periodic
  wrap especially) or the integration weights is mis-transcribed.
- **P2.** Steffen never leaves the data range on the step table (monotone),
  checked by a property test independent of GSL.

## Results (2026-11-13, Rust 1.99.1 release, GSL 2.8 `cf180cd7`, `gcc -O2`)

- **P1 held, first run.** 10 812 of 10 812 rows bit-identical: Steffen
  eval/deriv/deriv2 1 252 each and integ 1 155; Akima the same; periodic Akima
  253 each and integ 231 (periodic table only). Worst difference 0.
- **P2 held** (`steffen_is_monotone_on_a_monotone_table`, 9 001 points).
- The tolerance was never in play: the criterion is bit equality.
- Not ported from gh:#1341 (still open): `cspline_periodic`, `polynomial`,
  `spline.c` convenience wrapper, and the 2-D family (`interp2d`, `bilinear`,
  `bicubic`, `spline2d`).

## Periodic cubic spline and polynomial interpolation (written 2026-11-13, prediction committed BEFORE the port is run)

- **Port:** `src/interp_cspline_poly.rs` (`PeriodicSpline`,
  `PolynomialInterp`) from GSL 2.8 `interpolation/cspline.c`
  (`cspline_init_periodic` and the shared kernels) and `poly.c`, on
  `poly/dd.c` (`gsl_poly_dd_taylor`, ported line by line here because
  `DividedDifference::to_taylor` is a different accumulation) and
  `src/linalg/tridiag_cyc.rs` (`solve_cyc_tridiag`, ported with it).
  `spline.c` is **not ported**: `gsl_spline` is `gsl_interp` plus an owned
  copy of `x`, `y`, which `Interp` and the new types already own (checked
  against `spline.c`).
- **Reference:** `reference-data/gsl/interp_cspline_poly_reference_driver.c`;
  6 periodic tables (including `per3`, the closed-form 2x2 branch, `nonper`,
  where `y[0] != y[n-1]`, and `per2`, where GSL's init fails) and 5
  polynomial tables. **Replay:** `tests/gsl_interp_cspline_poly_code_to_code.rs`.
- **P7 (periodic spline).** `gsl_interp_init` status equal on all six tables
  (nonzero only for `per2`), and every `E/D/D2/I` row bit-identical: no libm
  function is called. It fails if the cyclic solver's grouping
  (`-delta[i-1] * offdiag[i-1] / alpha[i]`, the corner sums) is wrong.
- **P8 (polynomial).** `E` rows bit-identical (`gsl_poly_dd_init`/`_eval` were
  already bit-identical in `tests/gsl_poly_code_to_code.rs`), `D`/`D2` rows
  bit-identical (line-by-line `dd_taylor`), and `I` rows bit-identical on at
  least 90 % (they call `pow(b, i+1)`, glibc in GSL against PETIR's `pow`
  here; a 1-ulp difference is accepted, counted and pinned).

### Results (2026-11-13, Rust 1.99.1 release, GSL 2.8 `cf180cd7`, `gcc -O2`, glibc)

`tests/gsl_interp_cspline_poly_code_to_code.rs`: 9 726 rows, 9 595
bit-identical (98.7 %), plus 11 `INIT` rows.

| method | op | rows | bit-identical |
|---|---|---|---|
| cspline_periodic | E, D, D2, I | 1 237 each (I: 1 155) | **all** (4 866/4 866) |
| polynomial | E, D, D2 | 1 235 each | **all** (3 705/3 705) |
| polynomial | I | 1 155 | **1 024** (88.7 %) |

- **P7 held**: construction succeeds or fails exactly as `gsl_interp_init`
  does (`per2`: status 19, `GSL_EBADLEN`, from the cyclic solver; PETIR
  `Invalid`), all four ops bit-identical on all six tables including the
  closed-form `n = 3` branch and the unchecked non-periodic data.
- **P8: the first two clauses held, the integral clause FAILED.** `E`, `D`,
  `D2` are bit-identical (the line-by-line `dd_taylor` is what makes `D`/`D2`
  so). The integral `I` is 1 024/1 155 (88.7 %), below the predicted 90 %.
  **Cause demonstrated**: GSL's driver relinked with PETIR's `pow` alone makes
  all 9 726 rows bit-identical. `polynomial_integ` calls `pow(b, i+1)` and
  glibc's and PETIR's differ in the last bit for some arguments; the
  cancellation `pow(b) - pow(a)` then spreads it. Accepted per `AGENTS.md`;
  1 024 pinned.
- `spline.c` not ported, for the reason above (checked, not skipped).
- Still open in gh:#1341: the 2-D family (`interp2d`, `bilinear`, `bicubic`,
  `spline2d`).

## 2-D interpolation: bilinear and bicubic (written 2026-11-13, prediction committed BEFORE the port is run)

- **Port:** `src/interp2d.rs` (`Interp2d`, kinds `Bilinear` and `Bicubic`) from
  GSL 2.8 `interpolation/interp2d.c`, `bilinear.c`, `bicubic.c`. The six
  bicubic polynomial sums are in `src/interp2d_bicubic.rs`, **generated** from
  `bicubic.c` by `reference-data/gsl/gen_bicubic_rs.py` (only integer literals
  became floats and `*z += e` became `z += e`; the coefficients are not
  typed). `bicubic_init`'s row/column splines use the existing
  `Interp` natural cubic spline (already bit-identical to GSL's
  `gsl_interp_cspline`). `spline2d.c` is not ported: `gsl_spline2d` is
  `gsl_interp2d` plus owned copies of the arrays (checked against the file).
- **Reference:** `reference-data/gsl/interp2d_reference_driver.c`; four grids
  (2x3, 4x4, 5x6, 8x7), both methods, value and five partials on a 17 x 17
  grid plus every node, 12 extrapolation points, 12 out-of-range refusals and
  the `alloc` outcome. **Replay:** `tests/gsl_interp2d_code_to_code.rs`.
- **P9.** `alloc` accepts or refuses each (method, grid) exactly as GSL does
  (bicubic on the 2x3 grid is refused); all out-of-range points are refused
  (`GSL_EDOM`); and **every value is bit-identical** for both methods: no
  libm function is involved (the grids' `z` values are printed by the C
  driver, not recomputed). It fails if a generated coefficient line or the
  `zx`/`zy`/`zxy` derivative grids differ, or if `/dt` versus `*dx` is
  transcribed wrongly.

### 2-D results (2026-11-13, Rust 1.99.1 release, GSL 2.8 `cf180cd7`, `gcc -O2`, glibc)

`tests/gsl_interp2d_code_to_code.rs`: **13 482 of 13 482 values
bit-identical**, 8 `ALLOC` rows (bicubic on the 2x3 grid refused, as GSL
does) and 84 out-of-range refusals (`GSL_EDOM`).

| method | E | DX | DY | DXX | DYY | DXY | extrapolated (X) |
|---|---|---|---|---|---|---|---|
| bilinear | 1 264 | 1 264 | 1 264 | 1 264 | 1 264 | 1 264 | 48 |
| bicubic | 969 | 969 | 969 | 969 | 969 | 969 | 36 |

(each cell is all-identical)

- **P9 held, first run.** Negative control run: changing one generated
  coefficient (`4.0*zxyminmin` to `5.0*zxyminmin`, first occurrence) makes the
  test fail, and regenerating from `bicubic.c` reproduces the committed file
  byte for byte.
- Not ported: `spline.c`, `spline2d.c` (owned-copy wrappers; see above).
  gh:#1341 has no port left open from the issue's list.
