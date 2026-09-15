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
