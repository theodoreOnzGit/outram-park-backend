# PETIR verification summary

> **Status: verification, not validation.** Everything below establishes that
> PETIR computes what its upstreams compute. None of it compares PETIR against
> a published physical benchmark. Per
> [`VERIFICATION_AND_VALIDATION.md`](../../../VERIFICATION_AND_VALIDATION.md),
> verification asks "implemented correctly?" and validation asks "represents
> physical reality well enough?" — only the first is answered here.
>
> Generated for the record on 2026-09-15, the day the crate was declared
> mature. Numbers are measured, not estimated; each row names the test that
> produces it, and every test is runnable with `cargo test -p petir --release`.

## Method

Where a comparison says "code-to-code", it means the upstream library was
**compiled and executed**, and its output committed under
[`reference-data/gsl/`](../../../reference-data/gsl/) together with the C
driver that produced it, so the comparison can be regenerated rather than
trusted. GSL 2.8 was built from the vendored tree at commit
`cf180cd7fbd06039a577f9c9ff0b428784765ac1` with
`./configure --disable-shared --enable-static && make`, gcc `-O2`, glibc on
`x86_64-unknown-linux-gnu`.

"Bit-identical" counts values where the IEEE-754 bit patterns match exactly.
Where they do not, the worst *relative* difference is reported.

### Why iterate sequences rather than converged answers

For the iterative routines the reference records **every iterate**, not the
final result. Any correct bisection converges to the same root, and so does any
correct Brent — comparing final answers would pass against a port of a
different algorithm, or against one that had silently substituted a better
method. The trajectory is what identifies the algorithm actually implemented.

The ODE comparison follows the same principle by checking the **per-step error
estimate** alongside the state: two integrators can agree on `y` and disagree
entirely on `yerr`, and `yerr` is what a step controller acts on.

## Results against GSL 2.8, compiled and run

| surface | values | bit-identical | worst relative difference | test |
|---|---|---|---|---|
| `roots`, bracketing (bisection, false position, Brent) | 270 | 270 (100.0%) | 0 | `gsl_numerics_code_to_code.rs` |
| `roots`, polishing (Newton, secant, Steffenson) | 33 | 33 (100.0%) | 0 | `gsl_numerics_code_to_code.rs` |
| `min` (golden section, Brent) | 150 | 150 (100.0%) | 0 | `gsl_numerics_code_to_code.rs` |
| `interp` (linear, natural cubic spline) | 244 | 244 (100.0%) | 0 | `gsl_numerics_code_to_code.rs` |
| `ode` (RKF45, state and error estimate) | 120 | 120 (100.0%) | 0 | `gsl_numerics_code_to_code.rs` |
| `deriv` (central, forward, backward) | 78 | 76 (97.4%) | 3.27e-16 | `gsl_numerics_code_to_code.rs` |
| `integration` (6 Kronrod rules + QAG) | 28 | 27 (96.4%) | 1.82e-16 | `gsl_numerics_code_to_code.rs` |
| `linalg::qr` (factorisation, `tau`, least squares) | 85 | 81 (95.3%) | 4.27e-16 | `gsl_qr_code_to_code.rs` |
| `cheb::eval_mode`, reduced order | 369 | 357 (96.7%) | — | `gsl_cheb_mode_code_to_code.rs` |
| `cheb::eval_mode`, default order | 615 | 461 (75.0%) | 1.06e-15 results | `gsl_cheb_mode_code_to_code.rs` |
| `cheb` pipeline (`init`/`eval`/`deriv`/`integ`) | 6114 | 4707 (77.0%) | — | `gsl_code_to_code.rs` |

Where agreement is not exact it is at the last ulp. PETIR reaches its BLAS-1
kernels through `petir::linalg::blas1` rather than GSL's CBLAS; the two were
checked to be the same recurrences with the same association, but they are not
the same object code, and that is enough to move a last bit.

**One entry needs its own note.** `cheb::eval_mode`'s error *estimate* differs
by up to 1.05e-2 where its result differs by 1.06e-15. Upstream's estimate is
`|c[order]| + sum|c[i]| * DBL_EPSILON`, dominated by the coefficient whose true
value is nearest zero and therefore the most exposed to rounding in the
transform that produced it. The two implementations disagree by ~1% about a
quantity that is itself 1.7e-15. That is a property of the estimator, not of
the evaluation.

## Results against other upstreams

| surface | upstream | agreement | test |
|---|---|---|---|
| `fast_exp`, `fast_log`, `fast_pow` | ARM optimized-routines | **100% bit-identical** | `fast_*_vs_arm_optimized_routines.rs` |
| the 19 verbatim lifts | `outram-foam-basic-lib` (OpenFOAM), `chem-eng` (GNU Octave) | byte-identical after documented `std`→`core` substitutions, 0 allowed deviations on 17 of 19 | `verbatim_provenance.rs` |
| `specfunc::inc_gamma` vs `gamma_inc` | two independent lineages within PETIR | agree to 4.82e-10 | `incomplete_gamma_cross_check.rs` |

## Deviations from upstream, deliberate and pinned

These were found by the comparisons above rather than by reading, which is
the argument for doing them.

1. **The secant solver refuses to divide by zero.** Once an iterate lands on
   the root exactly, the update's denominator vanishes; GSL divides anyway and
   repeats the iterate indefinitely, while PETIR returns
   `PetirError::ZeroDivide`. Pinned by asserting that every iterate PETIR
   produces matches GSL's *and* that it stops only after reaching the root.

2. **`ode::rk4_step` is not `gsl_odeiv2_step_rk4`.** GSL's public stepper
   step-doubles — one full step, two half-steps, their difference as an error
   estimate. PETIR ports the private `rk4_step` helper underneath it, a single
   classical step. Measured difference 1.27e-7 over 20 steps at `h = 0.05`,
   which is the `O(h^5)` the step-doubling explains. The test asserts the gap
   is *present* and of that size, so the explanation is measured rather than
   asserted.

3. **`linalg::tridiag` guards a 1x1 system.** GSL reads one past the end of an
   empty `offdiag` array — undefined behaviour in C that happens to be
   harmless because the value is never used. PETIR returns directly instead.

4. **Errors are returned, never fatal.** GSL's default error handler calls
   `abort()`. PETIR has no handler and no abort; every fallible operation
   returns `petir::Result`.

## What is NOT covered

- **No published-benchmark comparison.** Nothing here is validation.
- **Six modules are ported from GSL but exercise only part of its surface** —
  QAGS and the infinite-range and weighted quadrature variants, implicit and
  stiff ODE methods, Akima/Steffen/periodic splines, general-degree complex
  polynomial roots, and rank-deficient (column-pivoted) least squares are all
  absent. Each is tracked as an issue rather than stubbed.
- **Adaptive Chebyshev degree selection** is absent because GSL does not have
  it to port: `gsl_cheb_alloc` sets `order_sp = order` and upstream's own
  header says the reduced order is "specific to the approximated function" and
  leaves it to the caller.
- **Both bookkeeping axes in the README remain unsigned** by the maintainer.
- **Allocation failure and `usize` overflow in debug builds** are not covered
  by the no-panic gates; the gates' own documentation says so.

## Reproducing

```sh
cargo test -p petir --release                       # 346 tests, 26 suites
cargo build -p petir --target thumbv7em-none-eabihf
cargo build -p petir --target wasm32-unknown-unknown
cargo check -p petir --all-targets --target aarch64-linux-android
cargo build -p petir --no-default-features
```

Regenerating the GSL references needs the vendored clone and a C compiler; each
file's recipe is in [`reference-data/gsl/README.md`](../../../reference-data/gsl/README.md).
