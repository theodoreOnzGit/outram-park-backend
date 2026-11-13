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
