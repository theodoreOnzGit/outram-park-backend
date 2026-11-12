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
  ~~Not changed here; needs a maintainer decision.~~ **CLOSED 2026-11-12**:
  approved, ported as the opt-in `TrigKernel::Gsl` (last section below).
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


## gh:#1421, `trig.c` option (maintainer approval 2026-11-12: "yes")

The maintainer approved porting GSL's `specfunc/trig.c` `sin`/`cos` into PETIR
**as an option**, so that Airy and `Ci` (and anything else built on
`gsl_sf_sin_e`/`gsl_sf_cos_e`) can match GSL bit for bit. The default path stays
PETIR's own `sin`/`cos`.

### Reading `trig.c` first (2026-11-12, GSL 2.8 `cf180cd7`)

What the decision text assumed and the source says differ in one place that
matters. The earlier note said GSL's `sin_e`/`cos_e` use "the platform
`sin`/`cos` for the reduced argument". **They do not.** `gsl_sf_sin_e`
(`trig.c:162`) and `gsl_sf_cos_e` (`trig.c:235`) call no libm function at all:
octant reduction with a three-piece `pi/4`, then `cheb_eval_e` on the `sin_cs`
(12 coefficients) or `cos_cs` (11) series, then `z*(1 + z*z*s)` or
`1 - 0.5*z*z*(1 - z*z*c)`. Only the `*_err_e` wrappers (`trig.c:620`,
`:629`) call libm `cos`/`sin`, and only to widen `err`, which PETIR does not
carry. `airy.c` (668, 709, 747, 804) and `sinint.c` (`Ci`, 374-375) reach
`sin_e`/`cos_e` this way. So the translation is deterministic and
platform-independent, and a bit-identity claim does not need the "1 ulp from
glibc" allowance for these two functions: the reference is **stock** GSL, with
no libm shim.

### Predictions (written BEFORE the port was run against anything)

- **P1.** `sin_gsl`/`cos_gsl` against `gsl_sf_sin_e`/`gsl_sf_cos_e` over the
  grid of `reference-data/gsl/trig_sincos_reference_driver.c` (dense, octant
  boundaries and their neighbours, 18 000 pseudo-random arguments to 1e14, huge
  and tiny arguments): **100 % bit-identical on every finite row**. This can
  fail: it fails if my reading of the sign bookkeeping, the order of operations
  (`0.5*z*z*(...)` is `((0.5*z)*z)*(...)`) or a coefficient is wrong.
- **P2.** `airy_ai/_scaled`, `airy_bi/_scaled` with `TrigKernel::Gsl` against
  the committed **stock** reference (`specfunc-gsl-2.8-reference.txt`): **100 %
  bit-identical** (the only libm calls on the `x < -1` branch are `sqrt`,
  which is exact); `ci` with `TrigKernel::Gsl`: **100 %**. Hence, against GSL
  linked to PETIR's libm, the whole sweep moves from 85 360 to **85 778 of
  86 238 (99.47 %)** (+418 = 98 + 98 + 89 + 89 + 44), the remaining 460 being
  `erf`/`erfc` 403, `eta` 45, `ln_gamma*` 8, `fd_*` 3, `transport_4` 1. If an
  airy row still differs, there is a second cause under the trig one, to be
  traced to its first divergent operation, and the prediction has failed.
- **P3.** Nothing on the default path moves: against stock GSL the whole sweep
  stays **83 717 of 86 238 (97.08 %)**, per-routine counts unchanged, and
  `tests/std_shadowing_digest.rs` keeps its pinned digests.

### The mechanism chosen

`specfunc::trig` gains `sin_gsl(x)` / `cos_gsl(x)` (translations of
`gsl_sf_sin_e`/`gsl_sf_cos_e`, `trig.c:162-308`, with `SIN_DATA`/`COS_DATA`
audited bit for bit by `tests/gsl_tables_audit.rs`, now 2436 literals) and a
plain enum `TrigKernel { Petir (Default), Gsl }` with `sin`/`cos` methods.
`airy_ai/_scaled`, `airy_bi/_scaled` and `ci` keep their signatures and call
`*_with(x, TrigKernel::Petir)`; `airy_*_with` and `ci_with` take the kernel.
No feature flag, no change of default. `si` is not routed: GSL's `gsl_sf_Si_e`
calls libm `sin`/`cos` itself (`sinint.c:345`). Other `trig.c` users in GSL
(spherical Bessel, Legendre conical/`H3d`) are not ported in PETIR; `sinc` is
not ported.

### Results (2026-11-12, Rust 1.99.1 release, GSL 2.8 `cf180cd7`, `gcc -O2`)

The GSL tree was re-cloned from `ampl/gsl` (ftp.gnu.org returned 403) and
rebuilt; the stock `specfunc` reference regenerated **byte-identical** to the
committed one (`cmp`).

- **P1 held.** `sin_gsl`/`cos_gsl` vs `gsl_sf_sin_e`/`cos_e`: **39 412 of
  39 412 (100 %)** bit-identical (`tests/gsl_trig_sincos_code_to_code.rs`,
  19 706 arguments including 1e300 and `DBL_MAX`, where GSL's reduction has no
  digits left and the port reproduces the same garbage). For information,
  PETIR's own `Real::sin/cos` vs GSL on the same rows: 28 772 of 39 412
  (73 %); the gap is 1 ulp on moderate arguments and unbounded where GSL's
  reduction has run out of digits.
- **P2 held** (measured against stock GSL and against GSL linked to PETIR's
  libm). Rows whose value goes through `sin_e`/`cos_e` (`x < -1` for Airy,
  `x > 4` for `Ci`), default kernel -> `Gsl` kernel, bit-identical:
  `airy_ai` 370 -> 468 of 468 (stock) / 370 -> 468 (libm-linked);
  `airy_ai_scaled` 370 -> 468; `airy_bi` 378 -> 468 (stock), 379 -> 468
  (libm-linked); `airy_bi_scaled` likewise; `ci` 195 -> 239 of 239. **Every
  row, 100 %.** The remaining rows of those routines (other regions, kernel
  independent) are libm noise against stock GSL (e.g. `airy_ai` 531 of 548)
  and 100 % against the libm-linked GSL (548 of 548, 579 of 579, 176 of
  176).
- Whole sweep against GSL linked to PETIR's libm (shim rebuilt with
  `sincos`): **85 360 -> 85 778 of 86 238 (98.98 % -> 99.47 %)** with the
  `Gsl` kernel on the five routines (+98, +98, +89, +89, +44 = +418, exactly the
  predicted figure). 460 remain: `erf`/`erfc` 403, `eta` 45, `ln_gamma*` 8,
  `fd_*` 3, `transport_4` 1, as before.
- **P3 held.** Default path, stock GSL: whole sweep **83 717 of 86 238
  (97.08 %)**, unchanged; per-routine airy/ci counts pinned in
  `default_trig_kernel_numbers_did_not_move`; against the libm-linked GSL the
  default path still gives 85 360. `tests/std_shadowing_digest.rs` is not
  changed and passes.
- **What bit-identity costs.** `TrigKernel::Gsl` inherits GSL's accuracy
  limit: past about `|x| = 1e8` for the Airy phase, `1e10` for `Ci`, its
  Cody-Waite reduction loses digits (`trig-vs-libm-probe-gsl-2.8.txt`:
  4.6e-7 absolute at 1e10, 3.4e-2 at 1e15), which is why it is an option and
  not the default. Choose it to match GSL; leave the default for accuracy.
- Not 1-ulp libm noise: these two functions involve no libm in GSL, so no
  glibc-versus-PETIR ulp allowance was needed anywhere in the comparison.
