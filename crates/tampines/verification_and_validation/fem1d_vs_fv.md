# 1-D continuous-Galerkin FEM against the finite-volume HEM pipe (gh:#1069)

> Research, education and V&V only. AI draft, unreviewed.

Dated 2026-11-01. Part of the RELAP-7 parity epic (gh:#1056). The RELAP-7
Theory Manual (INL/EXT-14-31366) discretises flow with continuous Galerkin
finite elements (§5.1, Eqs 235–245) stabilised by SUPG (§5.4.1, Eqs 275–298),
entropy viscosity (§5.4.2, Eqs 299–339) or Lapidus (named in §5.4, no
equations given). The manual is cited by section and equation only, not
quoted. **Maintainer decision: flow stays finite volume, OpenFOAM style, by
default.** The FEM was built so the two can be compared on the same cases;
it is not the new default.

## What was built

`tampines::fem1d` (`crates/tampines/src/fem1d/`):

- `FemPipe<E: HemEquationOfState>`: linear Lagrange elements, nodal unknowns
  `V = [ρA, ρuA, ρEA]`, Galerkin weak form of the variable-area Euler
  equations (Eqs 239–242), 2-point Gauss–Legendre per element
  (`farrer_park::quadrature::facet_rule(FacetType::Line2)`, reused), the
  equation of state closed at each quadrature point and node, lumped mass
  matrix by default (consistent is also offered), explicit SSPRK3
  (`petir::ssprk`).
- `Stabilisation::{Galerkin, Supg, EntropyViscosity, Lapidus}`.
- `Discretisation::{FiniteVolume (default), FiniteElement(Stabilisation)}` and
  `Pipe1d<E>`, an enum over `HemPipe<E>` and `FemPipe<E>`, so one case runs on
  both.
- Boundary conditions: every `HemBoundary` (walls, transmissive, reservoir,
  fixed pressure, choked break, prescribed flux). The face states are built by
  the same rules as `HemPipe`'s.

Searched first: all 1-D flow in the workspace is finite volume. The only FEM
is `farrer-park` (structural); its Gauss rule is reused directly (new edge
`tampines → farrer-park`; farrer-park's own dependencies were already in
tampines's tree, so no new crate is added and there is no cycle). The flux
Jacobian's eigenvalues are checked against `petir::linalg::eigen_nonsymm`.
The exact Riemann solver, the HEM water double-rarefaction reference and
the Edwards data are copies of `tampines-steam-tables/tests/common/` (headers
say so). `HemPipe` is the FV reference, unchanged.

### Choices, and why

| Choice | Why |
|---|---|
| Explicit SSPRK3, not SSPRK2 | The Galerkin operator is a central difference with eigenvalues on the imaginary axis. SSPRK2's stability region holds none of that axis but the origin; SSPRK3's holds `\|Im\| ≤ √3`. |
| Explicit, not implicit (RELAP-7 uses BDF + JFNK) | The FV reference is explicit at the same Courant number. An implicit FEM would compare time integrators, not spatial discretisations. |
| Classical Courant 0.4 | `HemPipe`'s default central Courant 0.2 is 0.4 in classical terms. |
| Lumped mass | Diagonal, so no solve per stage. Global totals `Σ m_j V_j = ∫V dx` are conserved either way. |
| SUPG `τ` through `A\|A\|⁻¹ = P sgn(D) P⁻¹` | Algebraically the same as Eqs 286 and 288, and bounded where an eigenvalue is zero. |
| Robust eigenvectors, not Eq 293's normalisation | Eq 294 divides by `p,0 + λ1 p,1 = c² − ΓH`, which is `−(γ−1)u²/2` for an ideal gas: **zero at rest**, so it is singular at every stagnant node. The robust set spans the same eigenspaces. The unit test agrees with Eqs 293–298 to 1e-9 at a moving state. |
| `p,i` from the sound speed and the Grüneisen coefficient | `p,0 = c² + Γ(u² − H)`, `p,1 = −Γu`, `p,2 = Γ`. Then Eq 292 gives `u, u ± c`, and `p̂ = (1+Γ)p − ρc²` (Eq 285). `Γ` comes from a forward difference of the EOS. |
| SUPG `V_t` in `R̃` | The Galerkin-only nodal rate. An explicit scheme cannot put the Petrov–Galerkin time term in the mass matrix without a solve per stage. |
| Entropy viscosity frozen per step | `∂p/∂t` and `∂ρ/∂t` in the entropy residual (Eq 335) are backward differences over the previous step. The first step uses `κ_max`. Single phase: `κ = µ`, `l = β = 0`. |
| Lapidus `ν = C_L h²\|u_x\|` on `∂V/∂x`, `C_L = 1` | The manual gives no equations. Form and coefficient are from memory (Lapidus 1967; Löhner, Morgan & Zienkiewicz 1985). **Neither is in the kovan corpus, and neither is verified.** `C_L` was fixed before any run and not tuned. |

Same physics on both: inviscid, adiabatic, horizontal. `HemPipe`'s default IF97
wall friction is ablated in the water and Edwards cases, because the FEM
carries none.

## Predictions (written before the first run)

Recorded verbatim in the module doc of `tests/fem1d_vs_fv.rs`: P1–P10, plus
the rest-state prediction on `fem_rest_state_and_wall_conservation_in_the_nozzle`.
Each one is judged below.

## Methodology

All in `crates/tampines/tests/fem1d_vs_fv.rs`, release build, one thread per
test, on 2026-11-01. FEM errors are measured at the nodes, FV errors at the
cell centres, and each against the reference evaluated at its own points.
The element length equals the cell width. L1 norms use each method's own
quadrature: cell widths for FV, trapezoid weights for FEM nodes.
Conservation is `|M(t) + outflow − M(0)|/M(0)`, with the outflow
integrated by the same RK stages; energy likewise. Wall time is the solver
run only. "Run time at equal accuracy" interpolates each method's
`(L1, wall time)` curve log-log to FV's 400-cell L1. Over- and undershoot
measure the excursion beyond the exact solution's range within ±3h of each
point. Shock width counts the points between the 10 % and 90 % levels of
the jump.

| Case | Setup | Reference |
|---|---|---|
| Sod | air γ = 1.4, `[0,10]` m, 1e5/1e4 Pa, 1/0.125 kg/m³, transmissive, t = 6.32 ms, 100–1600 cells | exact Riemann solution (Toro 2009 ch. 4) |
| IF97 water | 5 MPa, 450 K, ∓5 m/s, 1 m, 100 cells, 20 ms (the existing `HemPipe` IF97 Riemann test, a double rarefaction that flashes; no water *shock* tube with a reference exists in the HEM V&V) | semi-analytical (Riemann invariant along the isentrope) |
| Nozzle, subsonic | gh:#1058 nozzle (A 1.5 → 1 → 1.5 m², 1 m), 200 cells, reservoir 1e5 Pa / 300 K, exit 0.95e5 Pa, 0.2 s | isentropic area–Mach relation |
| Nozzle, shocked | as above, exit 0.75e5 Pa, 0.1 s | normal-shock analysis (shock at 0.8604 m, M1 = 1.612) |
| Edwards | 4.096 m, 24 cells, 7 MPa, Hendrie T, choked break 0 → 0.87 over 1 ms to 0.1 MPa, IF97, to 0.30 s (the last digitised GS-1 point) | Edwards–O'Brien GS-1 data |

~~PCICE (gh:#1068) is being built separately. Its column is reserved:
`methods()` and `pcice_hook()` in the test file mark where it goes, and
every table below reads "pending" for it.~~ **UPDATED 2026-11-01:** PCICE
landed and was run on **Sod**, with the same `L1(ρ)` instrument (cell-width
weights) and PCICE's own numerics (`pcice::cases::sod_ideal_gas`: the same
tube, states and end time, walls instead of transmissive ends, which the
waves do not reach). It is **not** run on the other cases:
- the nozzles, because PCICE's variable area is verified only at rest;
- the IF97 double rarefaction and Edwards, because PCICE on flashing water is
  untested (gh:#1087).

Those cells read "not run". No prediction was written for PCICE here before
its run, so its row is a measurement, not a test of a prediction.

## Results (2026-11-01, release, `cargo test --release -p tampines --test fem1d_vs_fv -- --nocapture`)

### Summary: who wins what

| Measure | FV (HemPipe KNP) | FEM SUPG | FEM entropy viscosity | FEM Lapidus | PCICE (gh:#1068) |
|---|---|---|---|---|---|
| Sod L1(ρ), every mesh | **best** | 2nd from 200 cells | 2nd at 100 cells, 3rd after | worst | within 0.4–10 % of FV (6.94e-2 → 6.42e-3) |
| Sod observed L1(ρ) order | 0.84–0.94 | 0.81–0.86 | 0.72–0.76 | 0.68–0.73 | 0.81–0.91 |
| Sod shock width (points) | 2–3 | 1–3 | 3–5 | 3–5 | not measured |
| Sod overshoot ρ / u (% of jump / of u*) | ≤ 2.9 / ≤ 0.8 | ≤ 3.7 / **≤ 14.8** | ≤ 3.3 / ≤ 10.9 | ≤ 4.0 / ≤ 12.1 | u: 5.9 % at 100 cells (pcice.md) |
| Sod wall time at FV-400 accuracy | 0.045 s (0.042 s on the PCICE rerun) | 0.49 s (11×) | 0.32 s (7×) | 0.62 s (14×) | **0.036 s** (same rerun) |
| Subsonic nozzle, worst p error | 0.31 % | **0.19 %** | **0.19 %** | **0.19 %** | not run |
| Subsonic nozzle, mass-flux spread | 0.74 % | **0.0009 %** | 0.0085 % | 0.025 % | not run |
| Shocked nozzle, shock position | **+0.92 cells** | +1.42 | +1.42 | +1.42 | not run |
| Shocked nozzle, p overshoot (% of jump) | 1.42 | 7.45 | 1.14 | 1.08 | not run |
| IF97 water double rarefaction | **passes all four gates** | fails (EOS, step 1) | fails (EOS, 4.3 ms) | fails (EOS, 0.42 ms) | not run |
| Edwards to 0.30 s, default Courant 0.4 | **completes, RMSE 44.2 psia** | fails 0.3 ms | fails 52 ms | fails 37 ms | not run |
| Edwards, diagnostic Courant 0.1 | (not rerun) | fails 0.4 ms | completes, RMSE 48.1 psia, 93 s | completes, RMSE 46.3 psia, 88 s | not run |
| Global mass and energy | round-off | round-off | round-off | round-off | round-off (pcice.md) |

**Finding.** FV wins on every discontinuous case: accuracy, oscillation,
cost and, on IF97, robustness. The FEM wins only on the smooth subsonic
nozzle. There its pressure error is 0.6× FV's, and its mass flux is uniform
almost to round-off (the limiter's clipping at smooth extrema costs FV
0.7 %). On real-fluid water the FEM, as built, is not usable at the
registered settings. That is recorded here as the result, not tuned away.
It supports the maintainer's decision to keep FV as the default.

### Sod

| method | cells | L1(ρ) kg/m² | order | shock pts | contact pts | ρ over % | ρ under % | u over % | u under % | mass err | energy err | steps | wall s |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| FV | 100 | 6.913e-2 | – | 2 | 6 | 0.08 | 1.96 | 0.78 | 0.33 | 6.3e-16 | 1.7e-16 | 109 | 0.005 |
| FV | 200 | 3.592e-2 | 0.94 | 2 | 6 | 0.49 | 1.81 | 0.46 | 0.25 | 1.6e-16 | 5.1e-16 | 219 | 0.012 |
| FV | 400 | 1.945e-2 | 0.89 | 2 | 7 | 1.31 | 2.36 | 0.26 | 0.13 | 6.3e-16 | 1.7e-16 | 438 | 0.045 |
| FV | 800 | 1.083e-2 | 0.84 | 3 | 9 | 2.68 | 2.88 | 0.14 | 0.11 | 1.3e-15 | 1.7e-16 | 877 | 0.212 |
| FV | 1600 | 5.846e-3 | 0.89 | 2 | 10 | 2.94 | 4.27 | 0.09 | 0.28 | 6.3e-16 | 1.7e-15 | 1754 | 0.975 |
| SUPG | 100 | 1.047e-1 | – | 3 | 5 | 3.38 | 5.36 | 14.81 | 9.07 | 6.8e-15 | 5.6e-15 | 115 | 0.015 |
| SUPG | 200 | 5.858e-2 | 0.84 | 2 | 6 | 3.03 | 4.24 | 12.97 | 7.07 | 1.7e-15 | 9.3e-15 | 232 | 0.036 |
| SUPG | 400 | 3.233e-2 | 0.86 | 1 | 7 | 3.15 | 4.15 | 13.14 | 7.41 | 8.5e-15 | 1.4e-14 | 467 | 0.145 |
| SUPG | 800 | 1.788e-2 | 0.85 | 2 | 9 | 3.39 | 5.38 | 14.11 | 7.12 | 4.7e-14 | 1.9e-14 | 937 | 0.604 |
| SUPG | 1600 | 1.017e-2 | 0.81 | 3 | 11 | 3.67 | 6.19 | 14.58 | 7.43 | 5.2e-14 | 4.0e-14 | 1876 | 2.699 |
| EV | 100 | 9.773e-2 | – | 5 | 6 | 2.29 | 1.97 | 10.31 | 8.81 | 6.5e-15 | 5.4e-15 | 111 | 0.007 |
| EV | 200 | 5.911e-2 | 0.73 | 3 | 8 | 2.25 | 4.58 | 9.80 | 8.22 | 1.9e-15 | 8.8e-15 | 227 | 0.025 |
| EV | 400 | 3.496e-2 | 0.76 | 3 | 8 | 2.38 | 5.77 | 10.35 | 8.70 | 1.1e-14 | 1.7e-14 | 458 | 0.070 |
| EV | 800 | 2.067e-2 | 0.76 | 3 | 10 | 2.92 | 6.66 | 10.92 | 8.80 | 5.6e-14 | 2.9e-14 | 919 | 0.268 |
| EV | 1600 | 1.252e-2 | 0.72 | 3 | 13 | 3.32 | 7.20 | 10.67 | 8.02 | 7.6e-14 | 6.0e-14 | 1842 | 1.052 |
| Lapidus | 100 | 1.176e-1 | – | 5 | 6 | 2.76 | 2.29 | 12.14 | 10.47 | 6.2e-15 | 5.1e-15 | 112 | 0.003 |
| Lapidus | 200 | 7.225e-2 | 0.70 | 5 | 6 | 2.63 | 4.27 | 11.36 | 8.85 | 4.1e-15 | 8.5e-15 | 228 | 0.013 |
| Lapidus | 400 | 4.358e-2 | 0.73 | 5 | 10 | 3.09 | 5.70 | 10.85 | 8.44 | 1.4e-14 | 1.6e-14 | 459 | 0.055 |
| Lapidus | 800 | 2.679e-2 | 0.70 | 3 | 13 | 3.58 | 6.75 | 9.86 | 9.29 | 5.7e-14 | 3.1e-14 | 921 | 0.206 |
| Lapidus | 1600 | 1.677e-2 | 0.68 | 4 | 16 | 3.96 | 7.37 | 11.13 | 9.89 | 7.6e-14 | 5.6e-14 | 1846 | 1.041 |

Wall time at FV's 400-cell L1(ρ) = 1.945e-2 (log-log interpolation,
nothing extrapolated): FV 0.045 s, SUPG 0.493 s, EV 0.316 s, Lapidus 0.623 s,
PCICE 0.036 s on a rerun in which FV took 0.042 s (PCICE L1(ρ) 6.941e-2, 3.695e-2, 2.025e-2, 1.127e-2, 6.424e-3 at 100–1600 cells). Wall times are single runs on a shared machine, so read
them to about ±30 %.

### Quasi-1-D nozzle (200 cells)

| Subsonic, p_e/p0 = 0.95, 0.2 s | worst p err % | mean \|p err\| % | mass-flux spread % | mass err | energy err | steps | wall s |
|---|---|---|---|---|---|---|---|
| FV | 0.3124 | 0.0917 | 0.7365 | 1.2e-13 | 4.4e-13 | 47613 | 3.27 |
| SUPG | 0.1861 | 0.1009 | 0.0009 | 3.3e-12 | 3.4e-12 | 47637 | 7.56 |
| EV | 0.1870 | 0.1009 | 0.0085 | 2.5e-12 | 3.4e-12 | 47655 | 3.61 |
| Lapidus | 0.1902 | 0.0997 | 0.0248 | 3.1e-12 | 3.3e-12 | 47643 | 3.32 |
| PCICE | not run (variable area verified only at rest) | | | | | | |

| Shocked, p_e/p0 = 0.75, 0.1 s (analytic shock 0.8604 m) | offset cells | post-shock p err % | L1 \|p err\| % | shock pts | p over % jump | p under % jump | mass err | energy err | steps | wall s |
|---|---|---|---|---|---|---|---|---|---|---|
| FV | +0.92 | −0.13 | 0.795 | 2 | 1.42 | 0.00 | 4.0e-11 | 3.3e-11 | 42466 | 3.15 |
| SUPG | +1.42 | +0.03 | 0.829 | 1 | 7.45 | 7.84 | 1.5e-12 | 2.1e-12 | 36245 | 5.69 |
| EV | +1.42 | +0.08 | 0.829 | 2 | 1.14 | 1.85 | 3.6e-12 | 1.1e-12 | 35627 | 2.68 |
| Lapidus | +1.42 | −1.23 | 0.951 | 3 | 1.08 | 1.42 | 1.1e-12 | 1.3e-12 | 35538 | 2.63 |
| PCICE | not run (variable area verified only at rest) | | | | | | | | | |

FEM positions are nodes `jh` and FV positions cell centres `(i+½)h`, so a
half-cell difference in the reported offset is expected. The conservation
errors here (1e-12 to 4e-11) are the accumulated round-off of the
inflow/outflow bookkeeping over ~4e4 steps with open ends.

**Rest state in the nozzle (FEM).** A uniform-pressure rest state does not
stay at rest: `max|u|` after 2 ms is 2.4e-3 / 6.0e-4 / 1.5e-4 m/s at 200 /
400 / 800 cells (Galerkin; SUPG 1.4e-3 / 3.4e-4 / 8.5e-5; EV and Lapidus as
Galerkin), **observed order 2.0**. FV gives 4e-13 m/s (its cell pressure
force balances exactly). The cause: the nodal unknown is `ρA` (Eqs 243–245),
so at a quadrature point `ρ = V^h/A` is not uniform when `A` is quadratic.
Walls in the nozzle conserve mass and energy to ~4e-14 on every
stabilisation. Galerkin alone fails the walls Riemann problem.

### IF97 water double rarefaction (100 cells, 20 ms)

| method | plateau p err % | plateau \|u\| err % | centre p − p* (gate ±3000 Pa) | centre ρ err % | L1(p) Pa·m | mass err | steps | wall s |
|---|---|---|---|---|---|---|---|---|
| FV | −0.003 | −0.05 | −4 Pa | −0.02 | 429.8 | 1.2e-16 | 2355 | 26.9 |
| SUPG | fails at t = 1e-5 s: EOS cannot close node 49 (ρ = 939, e = 745.5 kJ/kg) | | | | | | | 0.2 |
| EV | fails at t = 4.34 ms: node 42 (ρ = 1180 kg/m³) | | | | | | | 72.7 |
| Lapidus | fails at t = 0.42 ms: node 0, the transmissive end | | | | | | | 5.8 |
| PCICE | not run (flashing water untested, gh:#1087) | | | | | | | |

Diagnostic rerun at FEM Courant 0.1 (`FEM_CFL=0.1`): SUPG still fails at
1e-5 s, and Lapidus still at 0.42 ms at node 0. EV now fails at 0.57 ms, also
at node 0. Making the step 4× smaller does not change where the runs fail.

### Edwards–O'Brien (24 cells, frictionless on both, to 0.30 s)

| method | t reached | GS-1 plateau 0.02–0.06 s psia | RMSE vs data psia (pts) | mass err incl. break | steps | wall s |
|---|---|---|---|---|---|---|
| FV | 0.30 s | 368.0 | 44.2 (16) | 2.4e-15 | 2203 | 8.1 |
| SUPG | fails at 0.3 ms, node 23 | – | – | 3.7e-16 | 6 | 0.1 |
| EV | fails at 52 ms, node 1 | 371.6 | 20.8 (3 pts, not comparable) | 4.4e-14 | 672 | 10.5 |
| Lapidus | fails at 37 ms, node 24 (the break) | 360.8 | 22.2 (2 pts, not comparable) | 4.6e-14 | 712 | 10.6 |
| *diagnostic, FEM Courant 0.1:* EV | 0.30 s | 372.2 | 48.1 (16) | 3.0e-13 | 4842 | 92.7 |
| *diagnostic, FEM Courant 0.1:* Lapidus | 0.30 s | 372.7 | 46.3 (16) | 3.6e-13 | 5743 | 87.8 |
| *diagnostic, FEM Courant 0.1:* SUPG | fails at 0.4 ms | | | | | |
| PCICE | not run (flashing water untested, gh:#1087) | | | | | | |

The FV numbers reproduce the recorded `HemPipe` Edwards record (368.0 psia,
44.2 psia; `tampines-steam-tables/tests/hem_pipe_edwards.rs`). GS-1 traces in
psia, at 0.02 / 0.06 / 0.10 / 0.20 / 0.30 s: data 351 / 367 / 312 / 289 / 190;
FV 337 / 388 / 375 / 332 / 259; EV (Courant 0.1) 365 / 385 / 385 / 337 / 261;
Lapidus (Courant 0.1) 359 / 387 / 371 / 335 / 261. Where the FEM completes,
it carries the same equilibrium-model error as FV: too slow a
depressurisation after 0.08 s. That supports the reading that the error is
in the physics (HEM), not in the discretisation.

### Predictions, judged

| # | Prediction | Outcome |
|---|---|---|
| P1 | FV best L1(ρ) at every mesh; EV 2nd within 1.5×; SUPG 3rd; Lapidus last | **Partly held.** FV is best everywhere and Lapidus last. EV is 2nd only at 100 cells; from 200 cells SUPG beats it. EV/FV is 1.4–2.1×, not ≤ 1.5×. |
| P2 | orders FV 0.7–0.9, EV 0.6–0.9, SUPG 0.5–0.8, Lapidus ≤ 0.6 | FV 0.84–0.94 (just above), EV held, **SUPG missed high** (0.81–0.86), **Lapidus missed high** (0.68–0.73). |
| P3 | shock width FV 2–3, EV 3–5, SUPG 2–4, Lapidus 2–4 | FV, EV and SUPG held (SUPG 1–3). Lapidus 3–5, one point wider than predicted. |
| P4 | ρ overshoot FV ≤ 1 %, EV ≤ 3 %, SUPG 5–15 %, Lapidus 5–20 % | **FV missed** (up to 2.9 % at 800–1600 cells; the excursion grows as the exact local range narrows on fine meshes). EV held to 800 cells (3.3 % at 1600). **SUPG and Lapidus missed low** in ρ (≤ 4 %). Their oscillation shows in **u** instead: SUPG 13–15 %, EV 10–11 %, Lapidus 10–12 %, FV < 1 %. The prediction picked the wrong variable. |
| P5 | global conservation < 1e-12 for all | Held (≤ 8e-14 on Sod). |
| P6 | FEM ~4–5× FV per step; FV fastest at equal L1 by ≥ 5× | Held: 7–14× at equal accuracy. SUPG ~2.5× EV per step, as predicted. |
| P7 | water: all plateaus within 1 %; FV and EV meet the centre gate; SUPG and Lapidus may fail | FV held, at −0.003 % and −4 Pa. **EV missed: it fails**, as do SUPG and Lapidus. |
| P8 | subsonic nozzle: all within 1 %; SUPG and EV beat FV; FEM mass-flux spread larger | Accuracy held: FEM worst error 0.19 % against FV's 0.31 %. **Spread missed, the other way**: the FEM is far more uniform (≤ 0.025 % against 0.74 %). |
| P9 | shocked nozzle: within 3 cells and 3 %; SUPG oscillates | Held. SUPG overshoots 7.5 % of the jump; EV and Lapidus ~1 %. |
| P10 | Edwards: completing FEM within ±15 psia plateau and ±10 psia RMSE of FV; EV completes; mass 1e-12; FEM 3–6× slower | **EV completion missed** at the registered Courant 0.4. At the diagnostic 0.1, EV and Lapidus complete within +4.7 psia plateau and +4.1 / +2.1 psia RMSE of FV, mass ≤ 4e-13, and 11× slower (outside the 3–6× predicted). SUPG never completes. |
| rest | FEM rest state in the nozzle `\|u\| < 1e-9` | **Missed**: 2.4e-3 m/s, falling at second order. The assumption that `p` is uniform at the quadrature points was false (see above). |

### What the failures are, and what is not yet known

Root causes are **not established**. What was checked:

- **A defect of this implementation, found and fixed by the IF97 run.**
  `inverse3` judged singularity against the largest entry cubed. For two-phase
  water (`c ≈ 8 m/s`, `H ≈ 1e6 J/kg`) it refused a regular eigenvector matrix,
  and SUPG reported "unphysical sound speed". A column-norm test refused it
  too: P's rows differ in scale by `1 : c : H`. Now only an exactly zero or
  non-finite determinant is refused, and `sign_matrix` requires `c > 0`, which
  guarantees a regular P. A unit test pins the two-phase state. After the fix
  SUPG still fails, now at the EOS. The IF97 rows above were re-measured on
  the fixed code, with identical results.
- **Time step.** At 4× smaller steps, the Edwards EV and Lapidus runs
  complete. So at Courant 0.4 the stiff-liquid case is past the explicit
  limit of these stabilised schemes, which Sod and the nozzle are not. The
  water failures do not move with the step.
- **Boundaries.** Water Lapidus (both step sizes) and EV (Courant 0.1) fail
  at node 0, the transmissive end, at about the time the liquid fan reaches it
  (~0.35 ms per the reference). The Galerkin natural boundary flux `F(node)`
  is not upwinded, unlike FV's interior KNP flux next to the same face state.
  This is the leading hypothesis, **untested**. The next step is to read how
  RELAP-7 / MOOSE weakly imposes open boundaries, and try a characteristic or
  Rusanov boundary flux.
- **SUPG on liquid water** fails within the first steps at the diaphragm node,
  whatever the step. Candidates, **untested**: the Galerkin-only `V_t` inside
  `R̃`; the forward-difference Grüneisen coefficient against an IF97
  equilibrium sound speed that may not be its exact thermodynamic partner;
  and the size of `p̂ ≈ −ρc² ≈ −1e9 Pa`. Ideal-gas SUPG (Sod, nozzle) runs.

## Limits

- One mesh for the nozzles, the water case and Edwards. Sod has five.
- The FEM has no wall friction, gravity or wall heat. The two-phase
  (7-equation) entropy-viscosity terms (`l`, `β`) are not implemented: single
  phase only, as specified.
- Wall times are single runs on a shared machine.
- Lapidus's form and coefficient are unverified against a reference.
- PCICE (gh:#1068) is compared on Sod only; the other cases are marked not run, with reasons.
