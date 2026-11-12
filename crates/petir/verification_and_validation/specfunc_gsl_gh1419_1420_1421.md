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

### Prediction 2, written 2026-11-12 after hypothesis H was measured and BEFORE `debye.rs` was changed

H alone (measured, below) did not move `debye_2..6`, `bessel_j0/j1/y0/y1` or
`transport_5`. Reading `debye.c` against `debye.rs` shows a second cause:
`debye.rs` collapses upstream's six functions into one table-driven body
(its doc says "nothing numerical differs"), but upstream writes different
arithmetic per order: `c - x/3.0` against `c - lin*x` with `lin = 1.0/3.0`,
`x*x/24.0` against `quad*x*x`, `(1.0 + 2.0/xk + 2.0/(xk*xk))/rk` against a
Horner loop over falling factorials, `val_infinity/(x*x)` against
`val_infinity/pow_n(x, n)`, and `(val_infinity/x)/x` against `val_infinity/x^n`.
Each is a different rounding. Prediction: transcribing the six functions
exactly (same expressions, same order) makes `debye_2..6` **100 %
bit-identical** against GSL linked to PETIR's libm (and `debye_1` stays at
100 %), and moves nothing else.

## Results

Bit-identical share of the 86 238 values GSL returns successfully, against GSL
linked to PETIR's libm (2026-11-12, Rust 1.99.1 release):

| stage | whole sweep |
|---|---|
| before this work (PETIR's libm shim, pre-synchrotron-fix README figure) | 80 820 (93.72 %) |
| after #1419/#1420 ports and the `LogRootTwoPi_` fix | 84 803 (98.34 %) |
| after hypothesis H (`cheb_eval_e` argument mapping) | 84 990 (98.55 %) |

Hypothesis H, measured: **partly confirmed.** Moved to (almost) bit-identical:
`fd_1`, `fd_2` (1022/1024), `fd_3half` (1023/1024), `fd_half`, `fd_mhalf`,
`fd_0`, `psi_1piy`, `zetam1`, `zeta`, `eta` (981/1026; the worst relative
difference of the remaining 45 rows is 0, and every one of the four printed
is an even negative integer where GSL returns `+0.0` and PETIR `-0.0`: a signed
zero, a separate small defect, not examined further and left open), and a
few of the `airy_*` rows (+4 each). **Not moved, so the prediction missed
for them:** `debye_2..6`, `bessel_j0/j1/y0/y1`, `transport_5`, and, as
predicted, `dilog`, `elljac` and `psi` at `x < -30`. The cross-check that H is a
real defect and not a fit: `expint.rs` and `gamma_inc.rs` already map the
argument exactly as `cheb_eval_e` does, and had no residual.
