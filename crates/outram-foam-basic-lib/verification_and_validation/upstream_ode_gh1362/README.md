# ODE solvers against OpenFOAM-v2506 (ESI), gh:#1362 row 6

Upstream line: **ESI v2506** `libODE` (unmodified), the case of
`applications/test/ODE/Test-ODE.C` (`testODE`, Bessel functions `y = (J0, J1, J2, J3)`).
Foundation was not read or run.

Driver `driver/odeDump.C` is `Test-ODE.C` transcribed line for line with the
solver chosen by name (Euler, RKF45, Rosenbrock23). It prints hex bit patterns
of: the start state; for `i = 0..14` one adaptive `solve(x, y, dxNext)` from
`x = 1`, `dxNext = 0.6` at `relTol = exp(-(i+1))` (relTol, `x - xStart`, `dxNext`,
`y[0..3]`); the final `solve(1, 2, y, dxEst = 0.5)` at `relTol = 1e-4`
(`y[0..3]`, `dxEst`). `absTol` is upstream's default `SMALL = 1e-15`.
`maxSteps` is raised to 400000 in the driver because upstream's Euler cannot
reach `x = 2` at `relTol 1e-4` in the default 10000 steps (FatalError at
`x = 1.23094`, `dxDid = 2.5893e-05`); the port is checked to fail the same way.
Rebuild: `scripts/rebuild_reference.sh`. Reference: `reference/ode_esi_v2506.txt`
(51 lines). Test: `tests/openfoam_ode_code_to_code.rs`.

The test feeds the port the upstream start state and upstream `relTol` bits so
the solver is isolated from libm (`j0`, `exp`).

## Predictions (committed before the Rust side ran)

- P1: Euler bit-identical on all 15 steps and on the final solve (no
  transcendental functions; one explicit step with the error estimate
  `dx * f`).
- P2: RKF45 bit-identical if the Fehlberg coefficients and the summation
  order match upstream; otherwise ulp-level on the step and a changed `dxNext`
  at the 1e-16 level. No libm.
- P3: Rosenbrock23 bit-identical only if the LU decomposition, the Jacobian
  evaluation order and the `d`, `a`, `c` coefficients match; the first
  suspect for a difference is the LU solve ordering.
- P4: the port's Euler with the default `max_steps = 10000` returns
  `MaxStepsExceeded` as upstream's FatalError.

## Results (measured 2026-11-13, release)

| solver | 15 single steps (x advanced, dxNext, y0..y3 = 90 values) | final solve to x = 2, `integrate` (old) | final solve, `integrate_esi_v2506` |
|---|---|---|---|
| Euler | 90/90 bit-identical | 0/5 | **5/5** |
| RKF45 | 90/90 | 3/5 | **5/5** |
| Rosenbrock23 | 90/90 | 0/5 | **5/5** |

**The solvers themselves are bit-identical to upstream.** The whole difference
is the interval loop of `ODESolver::solve(xStart, xEnd, y, dxTry)`, shared by the
three solvers in the port's `integrate_interval`:

1. the step handed to the solver is `(x + dx).min(x_end) - x`, which is not `dx`
   bitwise (upstream passes `dxTry` unchanged unless `x + dxTry` passes `xEnd`);
2. after a truncated last step upstream returns the suggestion made **before**
   the truncation (`dxTry0`), the port returned the growth after the short
   step: `dxEst` differs by ~0.13 to 0.18 relative on this case.

**Option added, existing form kept:** `OdeSolver::integrate_esi_v2506` (and the
same method on `Euler`, `Rkf45`, `Rosenbrock23`), a line-for-line translation of
the upstream loop. `integrate` is unchanged because its consumer
`outram-park-fork-offbeat` (`viscochab`) was not re-measured in this round (not
run: resources); making the upstream loop the default is a maintainer decision,
recorded on the issue as deferred, with the predicted movement: ulp-level on `y`,
and a different returned `dxEst` after a truncated step.

Upstream finding (not a port difference): upstream's Euler cannot reach
`x = 2` at `relTol 1e-4` in the default 10000 steps (FatalError at
`x = 1.23094`, `dxDid = 2.5893e-05`); the port returns `MaxStepsExceeded` for the
same call (test `euler_default_max_steps_fails_like_upstream`).

### Predictions against the run

P1 (Euler bit-identical) right for the steps, wrong for the final solve. P2
(RKF45) the same. P3 (Rosenbrock23, LU as first suspect) **wrong**: the
Rosenbrock23 steps are bit-identical, so the LU, the Jacobian order and the
coefficients match; the only difference is the interval loop. P4 right.
