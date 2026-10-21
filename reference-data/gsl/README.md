# GSL reference output for PETIR's V&V

Numerical output produced by **running the GNU Scientific Library itself**,
used as the code-to-code oracle for the `petir` port. Like `../endf/` and
`../leapr/`, this lives at the repository root, outside `crates/`, so it is
git-tracked but never part of a published crate tarball.

## Provenance

- **Library:** GSL 2.8, commit `cf180cd7fbd06039a577f9c9ff0b428784765ac1`
  (2026-05-19), read from the `ampl/gsl` mirror — see
  `crates/petir/upstream_source/README.md` for the licence verification and the
  mirror caveat.
- **Built:** 2026-09-14, `gcc -O2`, glibc on x86_64-unknown-linux-gnu, from
  `cheb/{init,eval,deriv,integ}.c` + `err/{error,stream}.c` against GSL's own
  headers.
- **Driver:** `cheb_reference_driver.c` in this directory, committed so the file
  can be regenerated rather than trusted.

## `numerics-gsl-2.8-reference.txt` — roots, min, deriv, interp, integration, ode

- **Built:** 2026-09-15, same static GSL as the others.
- **Driver:** `numerics_reference_driver.c` in this directory.
- **Why:** these six petir modules were ported from GSL and tested against
  GSL's own assertions and analytical results, but had no compiled reference
  when petir was declared mature. This makes the maturity bar mean one thing
  across the crate.

### Iterate sequences, not converged answers

For the iterative routines the driver emits **every iterate**. Any correct
bisection converges to the same root and so does any correct Brent, so
comparing final answers would pass against a port of a different algorithm.
The trajectory identifies what is actually implemented. The ODE cases compare
the per-step error estimate alongside the state for the same reason.

### What the comparison found (2026-09-15)

| surface | values | bit-identical | worst rel. diff |
|---|---|---|---|
| roots, bracketing | 270 | 270 (100.0%) | 0 |
| roots, polishing | 33 | 33 (100.0%) | 0 |
| min | 150 | 150 (100.0%) | 0 |
| interp | 244 | 244 (100.0%) | 0 |
| ode (RKF45) | 120 | 120 (100.0%) | 0 |
| deriv | 78 | 76 (97.4%) | 3.27e-16 |
| integration | 28 | 27 (96.4%) | 1.82e-16 |

Two deviations surfaced here rather than by reading: the secant solver returns
`ZeroDivide` where GSL divides by zero and repeats the iterate, and GSL's
public RK4 stepper **step-doubles** so it is not comparable to petir's single
classical step (measured gap 1.27e-7 at h = 0.05, the expected O(h^5)). Both
are documented in the replaying test and in the crate source.

Replayed by `crates/petir/tests/gsl_numerics_code_to_code.rs`.

## `interp-integ-gsl-2.8-reference.txt` — `gsl_interp_eval_integ`

- **Built:** 2026-10-20, `gcc -O2`, glibc on x86_64-unknown-linux-gnu, from
  the vendored tree's `interpolation/{interp,linear,cspline,accel}.c`,
  `linalg/tridiag.c`, `vector/{init,view}.c`, `block/init.c` and
  `err/{error,stream}.c` against GSL's own headers (recipe in the driver
  header; that object list is the one that linked).
- **Driver:** `interp_integ_reference_driver.c` in this directory.
- **Why:** `petir::interp::Interp::eval_integ` was ported on 2026-10-20 so
  that `tuas_boussinesq_solver` could drop `peroxide`, whose
  `CubicSpline::integrate` it used for enthalpy (gh:#989).

Linear and natural cubic spline on two tables: the 9-knot Runge table of the
`interp` case above over all 231 ordered pairs `a <= b` of a 21-point grid,
and a non-uniform 11-knot cp-shaped table on `[200, 1200]` integrated from
273.15 and from its first knot. Plus 10 `ERR` rows (`a > b`, a limit outside
the table) where GSL, with its error handler off, returns `NaN`.

### What the comparison found (2026-10-20)

566 values, **566 bit-identical (100.0 %)**; all 10 `ERR` rows refused as
`PetirError::Domain`. Replayed by
`crates/petir/tests/gsl_interp_integ_code_to_code.rs`.
## `cholesky-gsl-2.8-reference.txt` — Cholesky factor, solve, inverse (gh:#993)

- **Built:** 2026-10-20, `gcc -O2`, glibc on x86_64-unknown-linux-gnu,
  against the full static GSL built from the vendored tree (recipe in the QR
  section below).
- **Driver:** `cholesky_reference_driver.c` in this directory. It writes the
  inputs (`A`, `B`) as well as the outputs, so the replaying test rebuilds
  them from the file; `crates/petir/tests/linalg_gsl_common/` reads every
  linear-algebra reference in this shape.
- **Routines:** `gsl_linalg_cholesky_decomp1`, `_solve`, `_invert`.
- **Cases:** 3x3 with an exact integer factor, Hilbert 6x6, pseudo-random
  Gram matrices 8x8, 24x24 and 30x30, and a 2x2 indefinite matrix (refused
  with `GSL_EDOM`).

### `cholesky-gsl-2.8-level2-reference.txt` — the same driver, Level 3 off

GSL runs Level-2 code for `n <= 24` and a recursive Level-3 blocking above
(`CROSSOVER` in `linalg/recurse.h`). PETIR ports only the Level-2 code. This
second file is the same driver linked against `linalg/cholesky.c`, `invtri.c`
and `trimult.c` recompiled with `CROSSOVER` raised to 100000, made by
`level2_rebuild.sh` in this directory:

```sh
reference-data/gsl/level2_rebuild.sh reference-data/gsl/cholesky_reference_driver.c \
    reference-data/gsl/cholesky-gsl-2.8-level2-reference.txt \
    linalg/cholesky.c linalg/invtri.c linalg/trimult.c
```

### What the comparison found (2026-10-20)

| comparison | values | bit-identical | worst relative difference |
|---|---|---|---|
| stock GSL, `n <= 24` | 1411 | 1411 (100 %) | 0 |
| stock GSL, 30x30 | 1830 | 741 (40.5 %) | 1.39e-12 per entry; normwise 1.1e-16 factor, 3.8e-16 inverse, 2.9e-16 solution |
| GSL at Level 2, all six cases | 3241 | 3241 (100 %) | 0 |

So the whole 30x30 difference is GSL's blocking. Replayed by
`crates/petir/tests/gsl_cholesky_code_to_code.rs`.

## `eigen-symm-gsl-2.8-reference.txt` — symmetric tridiagonal form and eigensolver (gh:#993)

- **Built:** 2026-10-20, same static GSL; `build_driver.sh` in this directory
  builds and runs any driver here.
- **Driver:** `eigen_symm_reference_driver.c`.
- **Routines:** `gsl_linalg_symmtd_decomp`, `_unpack`, `gsl_eigen_symm`,
  `gsl_eigen_symmv`, `gsl_eigen_symmv_sort(VAL_ASC)`.
- **Cases:** 1x1, 2x2, 4x4, Hilbert 8x8, Wilkinson W21+, block-diagonal 6x6,
  `5 I + 1 1^T` (four-fold eigenvalue), random symmetric 10x10, Gram 30x30.

## `svd-gsl-2.8-reference.txt` — Golub–Reinsch SVD and solve (gh:#993)

- **Built:** 2026-10-20, same static GSL, `build_driver.sh`.
- **Driver:** `svd_reference_driver.c`.
- **Routines:** `gsl_linalg_SV_decomp`, `gsl_linalg_SV_solve`.
- **Cases:** twelve, chosen to reach every branch of `linalg/svdstep.c`
  (the `N == 1` column, `svd2`, both zero-chasing routines, no sweep at all,
  the block rescaling for `1e-160` and `1e160` entries) plus Hilbert 6x6 and
  random 4x3, 8x5 and 30x20.
- **Result (2026-10-20):** 1428/1428 bit-identical against the
  `libm`-`hypot` run below; against stock GSL singular values agree to
  1.12e-15 normwise. Replayed by `crates/petir/tests/gsl_svd_code_to_code.rs`.

## `eigen-nonsymm-gsl-2.8-reference.txt` — Hessenberg, balancing, Francis QR (gh:#993)

- **Built:** 2026-10-20, same static GSL, `build_driver.sh`.
- **Driver:** `eigen_nonsymm_reference_driver.c`.
- **Routines:** `gsl_linalg_hessenberg_decomp`, `gsl_linalg_balance_matrix`,
  `gsl_eigen_nonsymm` (eigenvalues only) unbalanced and balanced.
- **Cases:** thirteen — 1x1, rotation, real 2x2, 3- and 4-cycles (exceptional
  shifts), a companion matrix with a complex pair, upper triangular, Frank 8,
  a badly scaled 3x3, block-diagonal 6x6 (mid-matrix split), random 6, 12, 25.
- **Result (2026-10-20):** 2419/2419 bit-identical against the
  `libm`-`hypot` run; against stock GSL the eigenvalues agree as a multiset
  to 4.54e-13 of `max|lambda|`. Replayed by
  `crates/petir/tests/gsl_eigen_nonsymm_code_to_code.rs`.

## Isolating `hypot`: `*-libm-hypot-reference.txt`

GSL calls the platform `hypot` (glibc here); PETIR ~~calls the `libm` crate's~~
(**since 2026-10-20, gh:#997, its own port of CORE-MATH's correctly rounded
`cr_hypot`**), through `crate::real::Real`, for `no_std` and cross-platform
reproducibility. The two differ in the last ulp on some inputs:
`hypot(1, sqrt(3))` is `2.0` from glibc (the correctly rounded value) and
`2 - 2^-52` from `libm`. (~~glibc's is the correctly rounded one~~
**CORRECTED 2026-10-20, gh:#997**: glibc 2.44's `hypot` is not correctly
rounded either; against MPFR it misrounds 23 per million uniform inputs and
6884 of CORE-MATH's 33174 worst cases — `reference-data/core-math/`.)
`libm_shim.rs` exports the `libm` crate's `hypot` under the C name; linking it
ahead of `-lm` gives GSL PETIR's `hypot` and nothing else:

```sh
rustc --edition 2021 --crate-type staticlib -C opt-level=3 -C panic=abort \
    --extern libm=target/release/deps/liblibm-<hash>.rlib -L target/release/deps \
    reference-data/gsl/libm_shim.rs -o /tmp/libm_shim.a     # libm 0.2.16
reference-data/gsl/build_driver.sh reference-data/gsl/eigen_symm_reference_driver.c \
    reference-data/gsl/eigen-symm-gsl-2.8-libm-hypot-reference.txt /tmp/libm_shim.a
reference-data/gsl/build_driver.sh reference-data/gsl/qr_reference_driver.c \
    reference-data/gsl/qr-gsl-2.8-libm-hypot-reference.txt /tmp/libm_shim.a
```

### What the comparison found (2026-10-20)

| surface | vs stock GSL | vs GSL + `libm` `hypot` |
|---|---|---|
| `symmtd` + `eigen_symm[v]` + sort, 6091 values | decomposition 62.2 %, eigenvalues 42.9 %, vectors 31.3 % bit-identical; structural difference on `5 I + 1 1^T` | **6091/6091 bit-identical** |
| `linalg::qr` (existing port), 85 values + solves | 81/85 (95.3 %) | **85/85, solutions and residuals bit-identical** |
| `linalg::svd`, 1428 values | singular values 27/57; 1.12e-15 normwise | **1428/1428 bit-identical** |
| `linalg::eigen_nonsymm` + Hessenberg + balancing, 2419 values | eigenvalues 104/320 in order; 4.54e-13 as a multiset | **2419/2419 bit-identical** |

So both ports are exact, and every difference from stock GSL is `hypot`.

### Re-measured with PETIR's CORE-MATH `hypot` (2026-10-20, gh:#997)

`petir_hypot_shim.rs` exports PETIR's `crate::hypot::hypot` under the C name
(a `std` staticlib, since petir links `alloc`):

```sh
cargo build --release -p petir
rustc --edition 2021 --crate-type staticlib -C opt-level=3 -C panic=abort \
    --extern petir=target/release/deps/libpetir-<hash>.rlib -L target/release/deps \
    reference-data/gsl/petir_hypot_shim.rs -o /tmp/petir_hypot_shim.a
reference-data/gsl/build_driver.sh reference-data/gsl/eigen_nonsymm_reference_driver.c \
    reference-data/gsl/eigen-nonsymm-gsl-2.8-petir-hypot-reference.txt /tmp/petir_hypot_shim.a
```

With that shim, the QR, SVD and symmetric-eigen drivers write files
**byte-identical to stock GSL's** (glibc 2.44 happens to round every `hypot`
those drivers call correctly), so their bit-for-bit tests now read the stock
files and no `*-petir-hypot-*` file is kept for them. The nonsymmetric driver
differs from stock on `rand25` only, and its file is kept:

| surface | PETIR vs stock GSL, before -> after | PETIR vs GSL + PETIR `hypot` |
|---|---|---|
| `symmtd` + `eigen_symm[v]` + sort | 62.2 / 42.9 / 31.3 % -> **100 %** (6091/6091) | 6091/6091 |
| `linalg::qr` | 81/85 -> **85/85**, solves and residuals bit-identical | 85/85 |
| `linalg::svd` | singular values 27/57 -> **57/57**; all 1428 values bit-identical | 1428/1428 |
| `linalg::eigen_nonsymm` + Hessenberg + balancing | eigenvalues 104/320 -> **236/320** in order; multiset 4.54e-13 -> **3.73e-15**; 12 of 13 cases bit-identical | 2419/2419 |

The `rand25` residue is glibc's: logging every `hypot` call the driver makes
(glibc, PETIR and MPFR side by side) finds five distinct inputs, each called
twice, where glibc returns one ulp above the correctly rounded value and
PETIR returns MPFR's, e.g. `hypot(-0x1.569095968f0f5p-2, 0x1.e62aaa381022ep-2)`
= `0x1.295df708e098bp-1` (glibc `...8cp-1`). The `*-libm-hypot-*` files and
`libm_shim.rs` are kept as the record of the gh:#993 isolation; no test reads
them any more.
Against stock GSL the eigensolver agrees as an eigen-decomposition: sorted
eigenvalues to 1.73e-15 normwise, eigenvectors within 0.55 of their
gap-scaled perturbation bound. Replayed by
`crates/petir/tests/gsl_eigen_symm_code_to_code.rs` and
`gsl_qr_code_to_code.rs`.

## `qags-gsl-2.8-reference.txt` — `gsl_integration_qags`

- **Built:** 2026-09-28, `gcc -O2`, glibc on x86_64-unknown-linux-gnu, from
  the vendored tree's `integration/{qags,qk21,qk15,qk,workspace}.c` and
  `err/{error,stream}.c` against GSL's own headers (recipe in the driver
  header).
- **Driver:** `qags_reference_driver.c` in this directory.
- **Why:** `petir::integration::qags` was ported on 2026-09-28 so that
  `buangkok`'s plume shine could run on petir instead of a second QUADPACK
  port; this is its compiled reference.

21 cases chosen to reach every exit of `qags`: the first-rule return, plain
convergence, convergence by extrapolation (endpoint singularities), the
iteration limit, `GSL_EDIVERGE`, `GSL_EROUND`, `GSL_ESING` (a pole: result
`inf`, error `NaN`) and `GSL_EBADTOL` (`GSL_EFAILED` was not reached by any
integrand probed), plus a plume-shine-like point kernel. Each row: status,
result, error estimate, sub-intervals, evaluations.

### What the comparison found (2026-09-28)

Statuses, sub-interval counts and evaluation counts identical in 21/21;
results 21/21 bit-identical; error estimates 19/21 bit-identical, the other
two (one integrand, two limits) 1.43e-6 relative. Cause, tested: the final
error is QUADPACK's cancelling running sum `errsum + error12 - e_i`, and
petir's `rescale_error` evaluates `pow` with the `libm` crate where GSL uses
glibc. Rebuilding GSL with a one-ulp `nextafter` on that `pow` moves GSL's
own error estimates by 4e-10 to 5.9e-4 (1.4e-6 on the same case) while every
result stays put. GSL's `integration/test.c` itself checks QAGS error
estimates only to 1e-6 and 1e-3. Replayed by
`crates/petir/tests/gsl_qags_code_to_code.rs`.

## `cheb-mode-gsl-2.8-reference.txt` — `gsl_cheb_eval_mode`

- **Built:** 2026-09-15, same static GSL as the QR reference below.
- **Driver:** `cheb_mode_reference_driver.c` in this directory.
- **Routines:** `gsl_cheb_eval_mode` and `gsl_cheb_eval_mode_e` — the two
  entry points of `gsl_chebyshev.h` that petir did not cover until then.

### The point of the second half of each case

`gsl_cheb_alloc` sets `order_sp = order` and nothing in GSL ever changes it,
so on any series GSL itself builds, `GSL_PREC_SINGLE` is indistinguishable
from `GSL_PREC_DOUBLE`. A reference recording only that would be satisfied by
a port that ignored `order_sp` entirely.

So the driver **sets `cs->order_sp` by hand** — which the GSL header
explicitly invites ("Users can use it if they like, but only they know how to
calculate it, since it is specific to the approximated function") — and
records both paths. Three cases: `exp` at order 24 reduced to 6, the Runge
function at order 30 reduced to 11, and `exp` on `[-2, 3]` at order 5 reduced
to 0, i.e. the constant term alone.

### What the comparison found (2026-09-15)

Default path: 615 values, 461 bit-identical (75.0 %); worst relative
difference 1.06e-15 on results and 1.05e-2 on error estimates. The estimate is
looser because it is dominated by `|c[order]|`, the coefficient whose true
value is nearest zero and so the most exposed to rounding in the transform
that produced it — the same effect `cheb-gsl-2.8-reference.txt` already
records as 77 % pipeline bit-identity.

Reduced path: 369 values, 357 bit-identical (96.7 %), largest gap between the
truncated and full answers 14.6 — which is the evidence that the reduced
branch was actually taken.

Replayed by `crates/petir/tests/gsl_cheb_mode_code_to_code.rs`.

## `qr-gsl-2.8-reference.txt` — Householder QR and least squares

- **Built:** 2026-09-15, `gcc -O2`, glibc on x86_64-unknown-linux-gnu, against
  a full static GSL built from the vendored tree (`./configure
  --disable-shared --enable-static && make`).
- **Driver:** `qr_reference_driver.c` in this directory.
- **Routines:** `gsl_linalg_QR_decomp_old` (the CLASSICAL Householder sweep,
  which is what `petir::linalg::qr` ports) and `gsl_linalg_QR_lssolve`. Note
  it is deliberately **not** `gsl_linalg_QR_decomp`, the blocked Level-3
  variant — that computes an equally valid but numerically different
  factorisation, and comparing against it would fail for a reason that says
  nothing about the port.

### Regenerating

```sh
G=crates/petir/upstream_source/GSL          # the vendored clone
(cd $G && ./configure --disable-shared --enable-static && make -j"$(nproc)")
gcc -O2 -I"$G" -o qrdriver reference-data/gsl/qr_reference_driver.c \
    "$G/.libs/libgsl.a" "$G/cblas/.libs/libgslcblas.a" -lm
./qrdriver > reference-data/gsl/qr-gsl-2.8-reference.txt
```

### What is in it

Five cases: a 2x2 square system, a general 4x3, an overdetermined 5x3
Vandermonde (the shape a polynomial fit makes), an 8x4 Chebyshev design matrix
at non-node points (the shape `ChebSeries::fit` makes), and a 4x2 whose first
column is scaled by 1e8 against the second. For each: every entry of the
packed QR factor, every `tau`, and — where `m >= n` — the least-squares
solution and residual.

### What the comparison found (2026-09-15)

85 values compared, **81 bit-identical (95.3 %)**; worst relative difference
4.27e-16 on the factorisation, 4.94e-16 on the solution, 1.37e-14 on the
residual. Replayed by `crates/petir/tests/gsl_qr_code_to_code.rs`.

## `lu-gsl-2.8-reference.txt` — LU with partial pivoting and its solve

- **Built:** 2026-10-20 (gh:#992), `gcc -O2`, glibc on x86_64-unknown-linux-gnu,
  against a full static GSL built from the vendored tree
  (`./configure --disable-shared --enable-static && make`), linked with GSL's
  own `libgslcblas.a`.
- **Driver:** `lu_reference_driver.c` in this directory.
- **Routines:** `gsl_linalg_LU_decomp` and `gsl_linalg_LU_solve`.
- **Not the system libgsl.** Arch's `gsl 2.8-1` links netlib
  `libcblas`/`libblas`, whose `DTRSV` walks back-substitution in the opposite
  order; the same driver linked that way differs on 44 lines of this file.

```sh
G=crates/petir/upstream_source/GSL          # the vendored clone (build a copy)
(cd $G && ./configure --disable-shared --enable-static && make -j"$(nproc)")
gcc -O2 -I"$G" -o ludriver reference-data/gsl/lu_reference_driver.c \
    "$G/.libs/libgsl.a" "$G/cblas/.libs/libgslcblas.a" -lm
./ludriver > reference-data/gsl/lu-gsl-2.8-reference.txt
```

Nine cases, inputs included in the file: 2x2, a zero leading pivot, a tie in
the pivot column, rows six decades apart, a pivot below `DBL_MIN`, an exactly
singular 3x3, and unstructured 6x6, 24x24 and 30x30 (the last through GSL's
Level-3 path). **Result (2026-10-20): 1637/1637 values bit-identical**,
including the 30x30. Replayed by `crates/petir/tests/gsl_lu_code_to_code.rs`.

## Regenerating

```sh
G=crates/petir/upstream_source/GSL          # the vendored clone
mkdir -p /tmp/gslcheb/gsl && cd /tmp/gslcheb
for h in $G/*.h $G/cheb/*.h $G/err/*.h $G/sys/*.h; do ln -sf "$h" gsl/$(basename $h); done
printf '#define HAVE_INLINE 1\n#define RETURN_IF_NULL(x) if (!x) { return ; }\n' > config.h
gcc -O2 -I. -I./gsl -o gsldriver <this dir>/cheb_reference_driver.c \
    $G/cheb/{init,eval,deriv,integ}.c $G/err/{error,stream}.c -lm
./gsldriver > cheb-gsl-2.8-reference.txt
```

## What is in it

Five cases — `sin` on `[-π, π]` at order 40, `exp` on `[-1, 2]` at order 12,
the Runge function on `[-1, 1]` at order 24, and `sin` on `[-5, 5]` at orders 1
and 2 (which exercise the `n == 1` / `n == 2` special branches of
`gsl_cheb_calc_integ`). For each: every coefficient, and at 201 points across
the interval, `gsl_cheb_eval`, `gsl_cheb_eval_err` (result and error estimate),
`gsl_cheb_eval_n`, and evaluation of the derivative and integral series.

## What the comparison found (2026-09-14)

Full pipeline — our coefficients, our evaluation — against GSL's:

```
  6114 values compared, 4707 bit-identical (77.0 %)

  coefficients     worst |delta| = 3.79e-17
  eval             worst |delta| = 8.88e-16   (exp12, where values reach e^2)
  eval_err result  worst |delta| = 8.88e-16
  eval_err abserr  worst |delta| = 5.42e-18
  eval_n(7)        worst |delta| = 2.22e-16
  integ eval       worst |delta| = 3.33e-16
  deriv eval       worst |delta| = 1.65e-14
```

The derivative's larger figure is not a defect: differentiating a Chebyshev
series amplifies coefficient error by `n^2`, and `1600 * 1e-17` is `1.6e-14`.

**Where the difference comes from — isolated, not guessed.** Feeding GSL's
*own* coefficients through *our* `eval` / `eval_err` / `deriv` / `integ`:

```
  5025 / 5025 bit-identical (100.00 %), worst |delta| = 0.000e0
```

So the evaluation arithmetic is a bit-exact transcription, and the entire
residual is in the coefficients — that is, in `cos()`. PETIR uses `libm` (a
pure-Rust port of musl's) because it is `no_std`; GSL as built here uses glibc.
The two differ in the last ulp. That is the deliberate trade recorded in the
workspace manifest: one fixed implementation buys bit-identical results across
platforms, at the cost of not matching any particular system libm exactly.
