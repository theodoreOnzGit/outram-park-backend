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

Prediction 2, measured: **confirmed.** `debye_1..6` are 419/419, 418/418,
417/417, 417/417, 417/417 bit-identical (100 %) against GSL linked to PETIR's
libm, from 94.5 / 98.6 / 90.2 / 92.6 / 91.8 % (stock-GSL figures before; the
libm-linked figures before were 396/419 ... 385/417). Whole sweep 98.55 % ->
98.70 %.

Three more causes found while looking for the rest, each by reading the C
against the Rust, none predicted in advance (so none of the below is a
prediction that held):

| cause | where | effect, libm-linked GSL |
|---|---|---|
| `transport` multiplied the Chebyshev value by a running product `((x*x)*x)*x`; GSL writes `x2*x`, `x2*x2` (transport.c:343, :420) | `transport.rs` | `transport_5` 379/416 -> 416/416 |
| `dilog_series_2` returned `s + 1.0 + t`; GSL does `val += 1.0 + t`, i.e. `s + (1.0 + t)` (dilog.c:119) | `dilog.rs` | `dilog` 935/1053 -> 1053/1053 |
| the diagnostic shim lacked `sincos`: `gcc -O2` fuses `sin(y)`, `cos(y)` into one glibc `sincos` call (17 references in `libgsl.a`) that bypassed the shim | `reference-data/gsl/petir_libm_shim.rs` | `bessel_j0/j1/y0/y1`, `elljac`, `psi` (x < -30), `psi_1piy`: all 100 % once the shim exports it. These were NOT port defects. |

Final, 2026-11-12, against GSL linked to PETIR's libm (shim with `sincos`):
**85 360 of 86 238 (98.98 %)** bit-identical, from 80 820 (93.72 %).
The remaining 878 values:

- `airy_ai/_scaled`, `airy_bi/_scaled` (98 + 98 + 89 + 89 rows) and `ci` (44):
  GSL's own `gsl_sf_cos_e`/`sin_e` (trig.c, Cody-Waite reduction plus a Chebyshev
  series), where PETIR calls libm's `cos`/`sin`. `reference-data/gsl/README.md`
  records this as a deliberate, more accurate substitution for huge arguments;
  porting `trig.c`'s sin/cos would close it at the cost of that accuracy.
  Not changed here; needs a maintainer decision.
- `erf`/`erfc` (154 + 249): not ports (fdlibm substitution), documented.
- `eta` (45) and `transport_4(-0.0)` (1): a signed zero.
- `fd_2`, `fd_3half`: 1 + 2 rows at x = 1e15, 1e50 (an operation order at huge x
  not yet traced).
- `ln_gamma`, `ln_gamma_sgn` (4 each): `+-1e-310`, `5e-324`, where GSL returns
  `+inf` for a finite answer and PETIR keeps the finite value (the documented
  DEVIATION in `lngamma_sgn_0`).

Against STOCK GSL (glibc libm, the committed reference): 79 642 (92.35 %) ->
**83 717 (97.08 %)**. The libm noise is what remains between those two figures.

Gamma/beta, gh:#1419/#1420 (stock GSL): see the doc comments of
`near_pole_rows_meet_the_gh1419_prediction` and
`fact_choose_beta_rows_meet_the_gh1420_prediction`; every prediction written
before the port held (worst near-pole difference 2.3e-16 against a predicted
5e-15; 99.1 % bit-identical against a predicted 60 %).

One thing in `lngamma_sgn_sing` is upstream's, not the port's: its series is
truncated and at the edge of its window (`|eps|` near 0.02) it differs from
fdlibm and glibc `lgamma` by up to 1.35e-13 absolute (3.8e-14 at -4.9801), while
the port reproduces GSL's value. The unit test
`ln_gamma_and_sign_near_every_pole_agree_with_fdlibm` bounds it and says so.

