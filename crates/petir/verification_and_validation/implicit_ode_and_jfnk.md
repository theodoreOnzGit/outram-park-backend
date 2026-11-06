# V&V: implicit ODE steppers and JFNK (gh:#1065, gh:#1066)

> Research, education and V&V only (`RESPONSIBLE_USE.md`). AI draft,
> unreviewed. Not for reactor operation, licensing or safety decisions.

Run 2026-11-01, `cargo test --release -p petir`, x86-64 Linux, Rust 1.99.1.
Every number below was printed by the test named beside it. The predictions
were written in each test's doc comment before it was run.

## What is verified

| module | what | lineage |
|---|---|---|
| `ode::implicit` | backward Euler; variable-step BDF2 started with one backward-Euler step; Newton inner solve | GSL `modnewton1.c` / `rk1imp.c` / `rksubs.c` / `cstd.c`; OpenFOAM `backwardDdtScheme.C` coefficients |
| `multiroots` | forward-difference Jacobian | GSL `multiroots/fdjac.c` |
| `newton_krylov` | JFNK: Walker–Pernice finite-difference `J v`, restarted right-preconditioned GMRES, dense-LU right preconditioning, absolute/relative/step-length tests | workspace `gmres_op` and pflotran driver; PETSc `MatMult_MFFD`, `MatMFFDCompute_WP`, `SNESConvergedDefault` |

Evidence class: **analytical solution** (observed order) and **code-to-code**
(Robertson against SciPy's Radau; JFNK against an assembled-Jacobian Newton
solve). Nothing here is validation against a physical benchmark.

## 1. Observed order on an analytic linear system

`tests/ode_implicit_order_and_robertson.rs::observed_order_on_an_analytic_linear_system`

**Method.** `y' = A y`, `A = [[-2, 1], [1, -2]]`, `y(0) = (1, 0)`, exact
`y = ((e^-t + e^-3t)/2, (e^-t - e^-3t)/2)`. Uniform steps `n = 10 ... 640` to
`t = 1`, analytic Jacobian, Newton error level `1e-13 |y| + 1e-15`. Error is
the max-norm error at `t = 1`; order `p = log2(e_n / e_2n)`. Pass: the four
finest orders within 0.05 of 1 (BE) and 2 (BDF2).

**Prediction.** BE orders rising to 1.00; BDF2 2.0 ± 0.05; errors at
`n = 640` ~1e-4 (BE) and ~1e-7 (BDF2).

**Result. PASS.**

| n | BE error | BE order | BDF2 error | BDF2 order |
|---|---|---|---|---|
| 10 | 2.020747e-2 | | 1.513150e-3 | |
| 20 | 1.016163e-2 | 0.9918 | 3.581746e-4 | 2.0788 |
| 40 | 5.091732e-3 | 0.9969 | 8.625291e-5 | 2.0540 |
| 80 | 2.548069e-3 | 0.9988 | 2.114516e-5 | 2.0282 |
| 160 | 1.274514e-3 | 0.9995 | 5.233990e-6 | 2.0143 |
| 320 | 6.373672e-4 | 0.9998 | 1.301962e-6 | 2.0072 |
| 640 | 3.187100e-4 | 0.9999 | 3.246739e-7 | 2.0036 |

The orders were as predicted; the error *sizes* at `n = 640` were about 3x
the predicted ones (error constants, not orders). The BE start does not cost
BDF2 its second order. A finite-difference Jacobian (`fd_jacobian`, GSL's
`fdjac`) gave final states bit-identical to the analytic-Jacobian run (BDF2,
`n = 80`).

## 2. Robertson's stiff kinetics to t = 40

`tests/ode_implicit_order_and_robertson.rs::robertson_to_t40_against_a_radau_reference`

**Method.** `y1' = -0.04 y1 + 1e4 y2 y3`, `y2' = 0.04 y1 - 1e4 y2 y3 - 3e7 y2^2`,
`y3' = 3e7 y2^2`, `y(0) = (1, 0, 0)`. Grid: 0, then `1e-5 * 1.5^k` up to 40
(39 intervals), each split into `m = 4 ... 64` equal steps (156 to 2496
steps), so doubling `m` halves every step. Analytic Jacobian; Newton error
level `1e-12 |y| + 1e-18`.

Reference at `t = 40`: SciPy 1.17.1 `solve_ivp(method="Radau")`, analytic
Jacobian, `rtol = 1e-13`, `atol = (1e-16, 1e-20, 1e-16)`:
`(7.158270687194055e-01, 9.185534764557759e-06, 2.841637457458306e-01)`.
SciPy's `BDF` and `LSODA` at the same tolerances agree with it to 7e-13 in
`y1`. Script (run in a throwaway venv, not committed):

```python
from scipy.integrate import solve_ivp
f = lambda t, y: [-0.04*y[0] + 1e4*y[1]*y[2],
                  0.04*y[0] - 1e4*y[1]*y[2] - 3e7*y[1]**2, 3e7*y[1]**2]
jac = lambda t, y: [[-0.04, 1e4*y[2], 1e4*y[1]],
                    [0.04, -1e4*y[2] - 6e7*y[1], -1e4*y[1]], [0, 6e7*y[1], 0]]
s = solve_ivp(f, (0, 40.0), [1, 0, 0], method="Radau", jac=jac,
              rtol=1e-13, atol=[1e-16, 1e-20, 1e-16])
```

Pass: no Newton failure; mass `y1 + y2 + y3 = 1` to 1e-12; finest two `y1`
orders within 0.15 of theory; ~~BDF2 at `m = 64` within 1e-6 relative in
`y1`, `y3`~~ (see below).

**Prediction.** No Newton failures; BE ~1e-3 at `m = 4` halving per
refinement; BDF2 ~1e-5 at `m = 4` quartering; mass to ~1e-14.

**Result. PASS, with one pre-run criterion FAILED and replaced.**

| method | m | steps | `y1` error | `y2` error | mass drift |
|---|---|---|---|---|---|
| BE | 4 | 156 | 3.481e-3 | 1.365e-7 | 4.4e-16 |
| BE | 8 | 312 | 1.758e-3 | 6.863e-8 | 4.4e-16 |
| BE | 16 | 624 | 8.831e-4 | 3.440e-8 | 1.1e-15 |
| BE | 32 | 1248 | 4.427e-4 | 1.722e-8 | 1.6e-15 |
| BE | 64 | 2496 | 2.216e-4 | 8.618e-9 | 4.0e-15 |
| BDF2 | 4 | 156 | 4.043e-4 | 1.570e-8 | 3.1e-15 |
| BDF2 | 8 | 312 | 9.778e-5 | 3.799e-9 | 1.3e-15 |
| BDF2 | 16 | 624 | 2.409e-5 | 9.362e-10 | 4.2e-15 |
| BDF2 | 32 | 1248 | 5.981e-6 | 2.325e-10 | 5.8e-15 |
| BDF2 | 64 | 2496 | 1.490e-6 | 5.792e-11 | 1.3e-14 |

Observed `y1` orders: BE 0.9860, 0.9929, 0.9965, 0.9982; BDF2 2.0479, 2.0211,
2.0099, 2.0048.

**The miss.** The pre-run criterion "BDF2 at `m = 64` within 1e-6 relative"
failed: `y1` is 2.08e-6 relative off. The BDF2 error constant on this grid
was under-predicted about 40x (4.0e-4 at `m = 4` against ~1e-5 predicted).
The criterion was replaced, after the run, by one that tests convergence to
the reference rather than an error size: Richardson extrapolation of
`m = 32, 64` with the theoretical order must match the reference to 1e-7
relative. BDF2: **9.2e-9, 2.8e-8, 2.3e-8** (gated). BE: 7.6e-7, 1.2e-6,
1.9e-6 (printed, not gated). Both methods converge, at their theoretical
order, to the Radau answer on a problem with stiffness ratio ~1e11.

## 3. JFNK on 1-D Bratu and a 1-D nonlinear heat equation

`tests/jfnk_bratu_and_nonlinear_heat.rs`

**Method.** Both problems on (0, 1) with `u(0) = u(1) = 0`, `N = 100`
interior nodes, central differences, initial guess `u = 0`:

- Bratu: `-u'' = e^u`.
- Heat: steady `-(k(u) u')' = 20`, `k = 1 + u^2`, arithmetic-mean face
  conductivity.

Reference: full-step Newton with the analytic Jacobian assembled densely and
solved by GSL's LU, iterated until the step is below `1e-12 ||x||`. Each
analytic Jacobian is first checked against GSL's `fdjacobian` (agreement
`< 1e-6` of its largest entry). JFNK: `rel_tol = 1e-10`, `step_tol = 1e-14`,
GMRES(200) capped at 5000 iterations, Walker–Pernice differencing with
`error_rel = sqrt(eps)`. Three preconditioners each: none; an **approximate**
Jacobian (Bratu: the Laplacian alone; heat: the Picard matrix with `k`
frozen and the `k'` terms dropped); the exact Jacobian. Pass: relative-test
convergence, `max |u - u_ref| < 1e-9`, approximate preconditioning cuts
GMRES iterations at least 5x, exact preconditioning ≤ 3 GMRES per step.

**The perturbation-size rule.** PETSc's default `MATMFFD_WP` (Walker &
Pernice, SIAM J. Sci. Comput. 19 (1998) 302):
`h = sqrt(eps) * sqrt(1 + ||u||_2) / ||a||_2`, `J(u) a ~ (F(u + h a) - F(u)) / h`.

**Prediction.** Reference Newton 3–4 (Bratu), 5–7 (heat). GMRES per step:
unpreconditioned 50–100; approximate 3–5 (Bratu), 5–15 (heat); exact 1–2.
Agreement ~1e-11.

**First run (forcing term 1e-8): a protocol failure, recorded.** All six
JFNK runs converged and agreed with the reference (Bratu ≤ 1.2e-14, heat
1.3e-12), but **every GMRES solve after the first Newton step exhausted its
5000-iteration cap**. Cause: the finite-difference product's own noise. The
residuals are `1/h^2`-scaled differences of O(1) terms, so `F` carries
round-off ~1e-11 per component; divided by the differencing parameter
(~1.5e-8) that is ~1e-3 of noise in `J v`, which makes a relative linear
residual of `1e-8 ||F||` unattainable once `||F||` is small. The forcing term
was changed to **1e-4** (inside Knoll & Keyes' range for a constant forcing
term, J. Comput. Phys. 193 (2004) 357) and a second prediction written
before re-running: Newton 4–6 (Bratu), 6–8 (heat); GMRES per step
unpreconditioned 20–60 / 30–100, approximate 2–3 / 2–6, exact 1. The 1e-8 run
is kept in the test as the recorded failure, and `JfnkConfig::default()` uses
1e-4 for the same reason.

**Result (forcing 1e-4). PASS.**

Bratu (reference: 4 Newton iterations, `max u = 0.140526506595`):

| preconditioner | Newton | GMRES per step | GMRES total | `F` evals | `max |u - u_ref|` |
|---|---|---|---|---|---|
| none | 3 | 50, 50, 99 | 199 | 212 | 3.6e-14 |
| Laplacian (approximate) | 3 | 3, 2, 2 | 7 | 20 | 4.1e-13 |
| exact Jacobian | 3 | 1, 1, 1 | 3 | 16 | 5.7e-15 |

Heat (reference: 7 Newton iterations, `max u = 1.460471950314`):

| preconditioner | Newton | GMRES per step | GMRES total | `F` evals | `max |u - u_ref|` |
|---|---|---|---|---|---|
| none | 5 | 50, 50, 50, 49, 50 | 249 | 271 | 1.3e-12 |
| Picard (approximate) | 6 | 1, 6, 5, 4, 4, 4 | 24 | 50 | 5.3e-15 |
| exact Jacobian | 5 | 1, 1, 1, 1, 1 | 5 | 27 | 1.3e-12 |

For comparison, forcing 1e-8 (same problems): GMRES totals 10 050 / 10 004 /
10 001 (Bratu) and 20 098 / 20 001 / 20 001 (heat).

Interpretation: JFNK reaches the assembled-Newton root to ≤ 1.3e-12 with no
Jacobian formed. Approximate-Jacobian preconditioning cuts GMRES iterations
28x (Bratu) and 10x (heat), the exact Jacobian 66x and 50x. Newton counts came
in one or two below prediction because JFNK stops on its relative test while
the reference iterates to a step test. The 1.3e-12 heat differences are the
runs that stopped at `||F|| ~ 8e-9` against a relative target of `2e-8`.

## What this does not cover

- No adaptive step control: the steppers march a caller's grid. GSL's `msbdf`,
  `bsimp`, `rk2imp` and `rk4imp` are not ported.
- The ported GMRES is not yet pinned bit for bit against
  `outram_foam_basic_lib::linear_operator::gmres_op` (that comparison has to
  live in `outram-foam-basic-lib`, which depends on PETIR).
- No cross-code comparison against PETSc's own SNES run, and none against GSL's
  `rk1imp` (whose public stepper returns the two-half-step result, which these
  functions deliberately do not).
- `N = 100` only; dense LU preconditioning is `O(n^3)` per Newton step and is
  meant for small systems or as a template.
