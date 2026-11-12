# specfunc against compiled GSL 2.8: gh:#1419, gh:#1420, gh:#1421

Parent: gh:#1338 (sweep), gh:#1336. Harness:
`crates/petir/tests/gsl_specfunc_code_to_code.rs`; references:
`reference-data/gsl/specfunc-gsl-2.8-reference.txt` (stock GSL, glibc libm) and,
as a diagnostic that is regenerated and not committed (7 MB shim), the same
driver linked against `reference-data/gsl/petir_libm_shim.rs`.

Research, education and V&V only (`RESPONSIBLE_USE.md`). AI-assisted work:
untrusted until a human reviews it.

## Methodology

1. GSL 2.8 (commit `cf180cd7`, `crates/petir/upstream_source/GSL`, shallow
   clone of `ampl/gsl`, `./configure --disable-shared --enable-static && make`)
   was rebuilt on 2026-11-12; rebuilding the stock reference with
   `reference-data/gsl/build_driver.sh` reproduced the committed
   `specfunc-gsl-2.8-reference.txt` byte for byte (`cmp`).
2. The same driver was linked against a shim exporting PETIR's own
   `exp/log/pow/sin/cos/...` (`petir_libm_shim.rs`, built with
   `rustc --crate-type staticlib --extern petir=...libpetir.rlib`). Against
   that reference the difference between GSL and PETIR is the port alone, with
   libm removed.
3. Every routine ported here was translated from the vendored C, with file and
   line in the doc comment. Independent references in the unit tests: `libm`'s
   fdlibm `lgamma_r`/`tgamma`, exact 128-bit integers, exact recurrences and
   closed forms (`B(1,b) = 1/b`).

## Predictions (written BEFORE the code, committed ahead of it)

- gh:#1419 and gh:#1420: in the doc comments of
  `gamma_near_negative_integer_poles_match_gsl` and `beta_family_matches_gsl`
  (commit `184d8bd0e`).
- gh:#1421, **written 2026-11-12 before any Chebyshev call site was changed**:
  hypothesis H: GSL's `cheb_eval_e` (specfunc/cheb_eval.c) maps its argument
  `y = (2.0*x - a - b)/(b - a)`. Every `cheb_series` in `specfunc/*.c` has
  `a = -1, b = 1` (checked by script: 121 of 121), so `y = ((2x + 1) - 1)/2`,
  which is NOT `x` when `2x + 1` rounds. PETIR's `eval_gsl` takes `x` as the
  already-mapped argument and the specfunc modules pass their `t` straight in
  (`airy.rs` even says "no argument rescaling is needed"), so they differ from
  GSL by that rounding. Prediction: applying GSL's mapping at every `cheb_eval_e`
  call site moves the Chebyshev-based families toward bit-identity with
  libm-linked GSL: `debye_2..6`, `bessel_j0/j1/y0/y1`, `fd_*`, `transport_5`,
  `psi_1piy`, `zeta/eta/zetam1` each to **at least 99 %** bit-identical; the
  whole sweep from 98.34 % to **at least 99.0 %**. Families with no Chebyshev
  call do NOT move: `dilog`, `elljac`, `psi` at `x < -30` stay as they are,
  which would show they have a different cause. The prediction can fail.

## Results

Filled in below as each fix lands (numbers measured, dated).
