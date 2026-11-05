# Conjugate heat transfer: the energy equation, chtMultiRegionFoam's coupled interface, and a counterflow double-pipe exchanger (gh:#1156)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

**Issue:** gh:#1156 (Foam Studio stage 2, epic gh:#1153). **Date:** 2026-11-03.

The predictions section below was written and saved **before** any of the
runs it predicts; results were added after, below it, without editing the
predictions.

## Question

1. Does the incompressible energy equation added to `PimpleFoam`
   (`solvers::energy`) reproduce the fully developed laminar Nusselt numbers
   of a circular pipe: 48/11 = 4.364 at constant wall heat flux, 3.657 at
   constant wall temperature (the Graetz limit)?
2. Does the port of `compressible::turbulentTemperatureCoupledBaffleMixed`
   with chtMultiRegionFoam's region loop (`solvers::cht_multi_region_foam`)
   reproduce the analytic composite slab, including a contact layer?
3. On the counterflow double-pipe exchanger, does the solved effectiveness
   agree with ε-NTU at the solved UA, and does energy close?

## What upstream does (read 2026-11-03, before writing the port)

OpenFOAM-8 (Foundation), fetched from `github.com/OpenFOAM/OpenFOAM-8`
(`master`) to a scratch directory; not vendored, not committed:

- `src/ThermophysicalTransportModels/derivedFvPatchFields/turbulentTemperatureCoupledBaffleMixed/turbulentTemperatureCoupledBaffleMixedFvPatchScalarField.{H,C}`:
  a mixed condition. `updateCoeffs()` sets `refValue` = the neighbour's
  patch-internal field, `refGrad = 0`, `valueFraction = nbrKDelta/(nbrKDelta +
  myKDelta)` with `KDelta = kappa·deltaCoeffs`. With `thicknessLayers` /
  `kappaLayers`, `contactRes_ = 1/Σ(t/k)` replaces `nbrKDelta` and the
  neighbour's **face** values replace its internal field.
- `.../temperatureCoupledBase/temperatureCoupledBase.C`: `kappa()` is the
  fluid's `kappaEff` (laminar: k) or the solid's `kappa`.
- `applications/solvers/heatTransfer/chtMultiRegionFoam/chtMultiRegionFoam.C`:
  inside `while (pimples.loop())`, every fluid region (`solveFluid.H`: UEqn,
  EEqn, PISO; with `frozenFlow`, EEqn only), then every solid region
  (`solveSolid.H`: the energy equation inside `correctNonOrthogonal()`).
- `fluid/EEqn.H`: the compressible energy equation (kinetic energy, pressure
  work, buoyancy work, radiation, reactions). For constant-property
  incompressible forced convection these terms are absent or of Eckert-number
  order (~1e-9 here) and the port does not carry them (stated in
  `solvers/energy.rs`).

Where the port differs, and why, is in the `cht_multi_region_foam` module
docs (flow solved once per step rather than in every outer pass, valid only
without buoyancy and with constant properties; the fluid T solved after the
PISO loop with the corrected flux).

## Methodology

### V1 — composite slab (`tests/cht_composite_slab.rs`)

Two **solid** regions on separate 1-D meshes (`cht_multi_region_foam::line_mesh`),
A on x ∈ [0, 10 mm] (k = 0.5 W/(m K), 10 cells), B on x ∈ [10, 40 mm]
(k = 16 W/(m K), 15 cells), joined at x = 10 mm by the coupled condition;
T = 400 K at x = 0, 300 K at x = 40 mm, every other face adiabatic.
Steady (`steadyState` ddt), iterated by the region loop to a pass change of
1e-11 K. Then the same with a contact layer (t = 0.5 mm, k = 0.1 W/(m K),
i.e. 200 W/(m² K)), and the same with region A replaced by a **fluid** at
rest (a `PimpleFoam` with `frozenFlow` and an energy equation, k = 0.6), so
the fluid side's α-form equation and the solid's are coupled through k.

Reference (exact): series resistance, `q = ΔT / (L_A/k_A + 1/h_c + L_B/k_B)`,
piecewise-linear T.

### V2 — Graetz / developed Nusselt number (`tests/energy_graetz_nusselt.rs`)

A single fluid region on `PipeOGrid::level(R = 10 mm, L = 200 mm, m)`,
m = 2, 3, 4 (16-, 24- and 32-gon walls). **Hydrodynamically developed flow
is prescribed**, not solved: `phi` on every axial face is the Hagen–Poiseuille
`2U(1 − r²/R²)` at the face centre times the face area, zero on every other
face. On this extruded mesh that flux is exactly divergence-free (every
layer's axial faces are identical), and it is what the Graetz problem
assumes. `frozenFlow` is set, so only the energy equation is solved, steady.
Re = U D/ν = 50, Pr = 1 (Pe = 50); inlet T = 300 K fixed, outlet
zeroGradient. Two wall conditions: constant flux (fixedGradient, q = 100
W/m², k = 0.6) and constant temperature (fixedValue 350 K).

Instrument (fixed before the runs): per axial layer, the wall heat flux
`q_w` = the matrix's wall conduction over the wall area of the layer, the
wall temperature (fixed value, or `T_P + g (n·d)` per face for the flux
case, area-weighted), the bulk temperature as the flux-weighted cell mean
of the layer (the upwind face mixing-cup), and
`Nu = q_w D_h / (k (T_w − T_b))` with D_h = 4A/P of the polygon (also
reported on D = 2R). The developed value is the mean over the layers with
z/D ∈ [6, 9].

References, with provenance:

- constant flux: Nu = 48/11 = 4.3636, the closed-form solution for
  Hagen–Poiseuille flow (derived, not looked up; it is the standard result
  quoted e.g. on Wikipedia's "Nusselt number" page as 4.36).
- constant temperature: the Graetz eigenvalue problem
  `f'' + f'/r + β Pe_R û(r) f = 0`, `f'(0) = 0`, `f(1) = 0`,
  `û = 2(1 − r²)`, solved in the test by RK4 shooting, Nu = −2 f'(1)/f_b.
  Without axial conduction this must give 3.657 (the same Wikipedia page
  quotes 3.66); that pins the shooting code.
- constant temperature **with axial diffusion**: the same problem with the
  extra term `β² (1 + Pe_R û Δz/(2R)) f`, i.e. the physical axial conduction
  (Pe = 50 is not large) plus first-order upwind's numerical axial
  diffusivity `u Δz/2` from the modified equation. This is the prediction for
  what the discrete scheme should approach as the radial grid is refined at
  fixed Δz/D.

### V3 — counterflow double-pipe exchanger (`cases::double_pipe`, test `tests/cht_double_pipe_exchanger.rs`)

Three regions on three conformal meshes (`block_primitives::DoublePipe`): the
inner fluid (O-grid, r < r₁), the tube wall (solid annulus r₁ < r < r₂), and
the annulus fluid (r₂ < r < r₃, outer wall adiabatic). Hot fluid in the
tube flows +z, cold fluid in the annulus flows −z. Both fluids have the same
properties and the same volumetric flow rate, so C_hot = C_cold (C_r = 1).
Flow solved to steady first (pseudo-transient PISO), then frozen, then the
three energy equations iterated to steady.

Instruments:

- ε_CFD = Q_hot / (C_min ΔT_max) from the mixing-cup inlet and outlet
  temperatures.
- **UA_local** = Σ over axial layers of (heat through the inner interface
  in the layer) / (T_b,hot − T_b,cold in the layer). For C_r = 1 the
  stream-to-stream temperature difference is constant along an ideal
  exchanger **whatever U(z) does**, and ε = NTU/(1 + NTU) holds exactly
  with NTU = UA/C for any U(z), so this is the UA ε-NTU should be given.
- ε-NTU from `outram_park_fork_dwsim_libs::heat_exchanger::ntu_effectiveness::evaluate`
  (counter-current) at UA_local, and also at UA_LMTD = Q/LMTD. The second is
  near-tautological (for constant C, ε-NTU at Q/LMTD returns ε exactly when
  Q_hot = Q_cold), so it measures only the energy imbalance; it is reported,
  not counted.
- Energy balance: Q_hot (enthalpy lost by the tube fluid), Q_cold (gained by
  the annulus fluid), the heat through each of the two interfaces.

## Predictions (written before the runs)

| case | quantity | prediction | why |
|---|---|---|---|
| V1 | max \|T − T_exact\| over 100 K | < 1e-8 K | the scheme is exact for piecewise-linear T (two-point flux, half-cell boundary distance, harmonic interface weight) |
| V1 | heat through the interface, A side vs B side | equal and opposite to < 1e-10 relative, equal to the analytic q | the mixed condition's two sides share one face flux |
| V1 | region passes to 1e-11 K | ≈ 60 | block Gauss–Seidel contraction per pass ≈ G_i/(G_i + G_A) · G_i/(G_i + G_B) = 0.947 · 0.630 = 0.60 for these conductances; ln(1e13)/ln(1/0.60) ≈ 58 |
| V1 | with contact layer, and fluid-at-rest side | same exactness (< 1e-8 K) | same argument; the fluid's α-form equation carries k through ρc_p consistently |
| V2 | constant-flux Nu (D_h) on m = 2, 3, 4 | within 3 % of 4.364 at m = 4, converging toward it; sign not predicted | radial discretisation only: upwind is exact for the developed linear axial rise |
| V2 | constant-T Nu, no-axial shooting | 3.657 ± 0.001 | the shooting code against the known Graetz limit |
| V2 | constant-T Nu, CFD | **above** 3.657; within the radial error (~3 %) of the shooting with physical + upwind axial diffusion at the run's Δz | Pe = 50 axial conduction and upwind numerical diffusion both raise the constant-T Nu |
| V3 | ε_CFD against ε-NTU(UA_local) | within 1 %, ε_CFD the lower | C_r = 1 makes ε-NTU exact for any U(z); the gap is axial conduction (wall: k_w A_w/(L C) ≈ 3e-3; fluid: smaller; upwind: zero where T is linear in z), all of which lower counterflow ε |
| V3 | energy: Q_hot vs Q_cold vs interface heats | agree to < 1e-6 relative once the region loop has converged | steady, conservative discretisation, solve tolerance 1e-12 |

## Results (2026-11-03)

### V1 — composite slab

| variant | passes | max \|T − T_exact\| | q_A / q − 1 | q_B / q − 1 |
|---|---|---|---|---|
| two solids | 46 | 5.8e-9 K | −6.3e-11 | −6.3e-11 |
| contact layer (h_c = 200 W/(m² K)) | 18 | 7.8e-9 K | +7.4e-11 | +7.4e-11 |
| fluid at rest + solid | 51 | 7.7e-9 K | −8.5e-11 | −8.5e-11 |
| *diagnostic: same, solve tolerance 1e-15* | 57 / 22 / 64 | 1.2 / 1.3 / 1.4e-11 K | ≤ 1e-12 | ≤ 1e-12 |

**Within the predictions.** The exactness bound (< 1e-8 K) holds, but with
less margin than expected. The diagnostic row shows why: at the default
1e-12 solve tolerance the residual T error is the linear solve's; at 1e-15
it falls to the 1e-11 K level the pass-stopping criterion predicts, and the
passes become 57 for the solid pair against the **58 predicted** from the
block Gauss–Seidel contraction. The interface condition is therefore exact
for the piecewise-linear solution, with and without the contact layer, and
when one side is a fluid region in the α-form.

### V2 — developed Nusselt numbers (prescribed developed flow, Re = 50, Pr = 1)

| wall | m | cells | Δz/D | Nu (D_h) | Nu (D = 2R) | reference | Nu(D_h)/ref − 1 | energy imbalance |
|---|---|---|---|---|---|---|---|---|
| flux | 3 | 8 640 | 0.208 | 4.4435 | 4.4818 | 4.3636 | +1.83 % | −8e-14 |
| flux | 4 | 20 480 | 0.156 | 4.4191 | 4.4405 | 4.3636 | +1.27 % | 3e-13 |
| flux | 5 | 40 000 | 0.125 | 4.4059 | 4.4195 | 4.3636 | +0.97 % | 1e-13 |
| flux | 6 | 69 120 | 0.104 | 4.3978 | 4.4073 | 4.3636 | +0.78 % | −3e-13 |
| T | 3 | 8 640 | 0.208 | 3.6953 | 3.7272 | 3.6585 | +1.01 % | 1e-11 |
| T | 4 | 20 480 | 0.156 | 3.6859 | 3.7038 | 3.6585 | +0.75 % | −2e-10 |
| T | 5 | 40 000 | 0.125 | 3.6804 | 3.6918 | 3.6585 | +0.60 % | −6e-16 |
| T | 6 | 69 120 | 0.104 | 3.6768 | 3.6847 | 3.6585 | +0.50 % | 3e-14 |

Reference-free three-grid extrapolation (observed order from the solutions
alone, then Richardson): constant flux, m = 4/5/6, **p = 1.41, Nu → 4.3704
(+0.15 %)**; constant T, **p = 1.11, Nu → 3.6607 (+0.06 %)** (m = 3/4/5 give
4.3700 and 3.6599). The shooting code gives **3.65679** without axial
diffusion (the Graetz limit, prediction 3.657 ± 0.001: met), **3.65858** with
the physical axial conduction at Pe = 50, and 3.65848–3.65852 with upwind's
numerical diffusion added at these Δz. Flux divergence ≤ 1e-17 of the
inlet flow on every grid. Wall time 0.3–7.7 s per run.

**Against the predictions.**

- Constant flux within 3 % at m = 4 (+1.27 %) and converging: **met**. The
  observed order, 1.4 on the solutions (1.2 measured against the
  reference), is below 2; the same sub-second-order behaviour as stage 1's
  f·Re (gh:#1168), on the same O-grid family.
- Constant T above 3.657 and within ~3 % of the shooting prediction: **met**
  (+0.50 to +1.01 %). **But the stated reason was wrong.** I predicted the
  excess would come from axial diffusion (physical at Pe = 50, plus upwind's
  numerical u Δz/2). The shooting model says those together move Nu by only
  +0.05 %. The excess is the radial discretisation, falling at first order,
  and it extrapolates to within 0.06 % of the shooting value. Upwind's axial
  diffusion barely matters here because the decay rate β is small (β² ≪ β Pe).
- D_h against D: the polygon's D_h (4A/P) brings the coarse grids closer
  to the circle's reference than D = 2R does, and both converge to it.
- **A defect found and fixed during these runs** (not a tuning): the first
  energy balance read 1.6e-8 of the wall heat, then 3.7e-8 after a first
  fix attempt. The cause is in `outram-foam-basic-lib`'s BiCGStab. Its
  breakdown test is absolute (`|ρ| < 1e-30`, ρ ~ ‖r‖²), so with this
  equation's ‖b‖ ≈ 2e-5 it stopped at a relative residual of about 4e-11,
  above the requested 1e-12, and returned `converged = false`. The residual
  summed over the cells (all one sign) was the imbalance. Two wrong
  hypotheses (the tolerance itself, then the recursive-residual gap) were
  tested and rejected first. The energy solve now refines against the true
  residual with a normalised right-hand side (`solvers/energy.rs`,
  `refine`), and the balance is 1e-16 to 2e-10. Filed for basic-lib as gh:#1170.

### V3 — counterflow double-pipe exchanger

Geometry r = 5 / 6 / 10 mm, L = 200 mm; `DoublePipeExchanger::demo()`
(Re = 30 in the tube, C_hot = C_cold = 0.942478 W/K, 350 K / 300 K).

| grid (n, nr, nz) | cells (tube fluid, wall, annulus) | flow steps | tube u_c/2U at 0.75 L | energy passes | ε_CFD | UA_local [W/K] | NTU | ε-NTU(UA_local) | ε_CFD/ε-NTU − 1 | Q imbalance (hot−cold, hot−wall, wall in−out) | wall time |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 3, (2,2,3), 30 (test) | 990, 720, 1 080 | 8 | 0.980 | 191 | 0.55389 | 1.22535 | 1.300 | 0.56524 | **−2.01 %** | −4e-10, −2e-10, 2e-13 | 1.1 s |
| 4, (3,2,4), 50 | 3 200, 1 600, 3 200 | 12 | 0.990 | 265 | 0.55656 | 1.21667 | 1.291 | 0.56350 | **−1.23 %** | −6e-10, −4e-10, −7e-14 | 4.7 s |
| 6, (4,2,5), 80 | 10 560, 3 840, 9 600 | 22 | 0.995 | 337 | 0.55818 | 1.21166 | 1.286 | 0.56248 | **−0.76 %** | −8e-10, −5e-10, −6e-14 | 25 s |

Q_hot = Q_cold = Q_wall = 26.10 / 26.23 / 26.30 W. UA_LMTD = 1.168 /
1.180 / 1.188 W/K, and ε-NTU at UA_LMTD reproduces ε_CFD to 0.07 %. As
predicted this is near-identity: the residual is the axial conduction
through the inlet faces, which Q_hot includes and C ΔT does not.

**Against the predictions.**

- Energy closes to < 1e-9 relative (prediction < 1e-6): **met**.
- ε_CFD below ε-NTU(UA_local): **met for the sign**. "Within 1 %" is **met
  on the finest grid (−0.76 %) and missed on nz = 50 (−1.23 %)**. The
  reason I gave (axial conduction) was **wrong** again. The gap times nz is
  −0.60, −0.62, −0.61: it falls as 1/nz (the cross-section was refined
  alongside, so this does not separate the two), so it is the first-order
  axial discretisation, and it extrapolates to |gap| < 0.1 % as nz → ∞.
- *Post-hoc, so stated:* after seeing the 1/nz scaling I derived the
  effectiveness of a counterflow of N well-mixed layers per stream with
  C_r = 1, which is what first-order upwind is: ε = NTU/(1 + NTU + NTU/N).
  At the measured NTU it gives 0.55478 / 0.55721 / 0.55855. ε_CFD is
  −0.16 / −0.12 / −0.07 % from it, and that remainder also shrinks with the
  grid. It is the expected size for the wall's axial conduction
  (k_w A_w/(L C) ≈ 3e-3) and the entrance profiles, but it was not
  separated from them.
- *An instrument defect found and fixed before these numbers were taken*
  (not a tuning): my first UA_local omitted the last axial layer of each
  stream. That layer's downstream face is the outlet patch, which the
  internal-face loop skipped. The omission biased UA low by about 2/nz and
  put ε_CFD 2.7 % **above** ε-NTU on the coarse grid. Including the outflow
  boundary faces fixed it.

**Which numbers to quote.** The finest grid's ε_CFD/ε-NTU = −0.76 %, with
the 1/nz trend that takes it to ≈ 0. The coarse test grid's −2 % is a
regression pin, not a result. UA_LMTD is a consistency check, not evidence.

### The demo (checks, not V&V)

Foam Studio's double-pipe exchanger at the default budget: the demo grid
`engine::exchanger_grid` is n = 5, nr = (4, 2, 5), nz = 75, 18 375 cells
(7 875 / 3 000 / 7 500). The flows are marched at Co 4 with the viscous
number capped at 0.5, then frozen; the energy runs 10 passes per step,
stopping when a pass changes no cell by more than 1e-7 · ΔT_in.

| run | to steady | steps (flow + energy) | result |
|---|---|---|---|
| native `--headless-exchanger 20000 30` | **12.2 s** | 19 + 20 (200 passes) | T_hot,out 322.17 K, T_cold,out 327.83 K, ε 0.5578, ε-NTU 0.5624 (−0.82 %), Q_hot/Q_cold − 1 = −2.6e-6 (the demo's looser stop) |
| browser, phone 390×844 (touch, DPR 2), run 1 | 21.8 s | 39 | frames while solving: median 33 / p95 167 / max 1017 ms; after: median 16.7, max 67 ms |
| browser, phone, run 2 (after a label fix) | 22.7 s | 39 | while solving 16.7 / 167 / 567 ms; after 16.7 / 16.7 / 67 ms |
| browser, desktop 1280×800, run 1 / run 2 | 20.3 / 21.7 s | 39 | while solving 16.7 / 100 / 400 and 16.7 / 100 / 467 ms; after max 17 / 50 ms |

Headless Chromium (Playwright's build, SwiftShader software GL) on the
shared 4-CPU host, driven by `cht_double_pipe_gh1156/browser_check.js`
(`?autostart&shape=exchanger&cells=20000`). It taps "+" and drags on the
view six times while the worker solves.

- **It runs in the browser and converges to the native answer**: the same
  step count, and the same ε and temperatures in the dock.
- **Phone layout:** the controls open folded, + / − / Reset and the drag
  toggle sit top right, the convergence dock shows the flow then the energy
  residual, and the numbers panel shows the temperatures, heats, UA and ε
  against ε-NTU (`browser_phone_390_solving.png` mid-flow,
  `browser_phone_390_steady.png`; desktop `browser_desktop_1280_steady.png`).
- **Responsiveness:** zoom taps and drags took effect mid-solve. The median
  frame while solving was 16.7 ms in three runs and 33 ms in the first phone
  run. Single slow frames of 0.4–1.0 s occur during the solve only. Each
  energy step is one worker request of about 0.6 s. Software GL and the
  worker share four CPUs here, and that contention was not separated from
  main-thread work. **Not measured:** a hardware GPU, and the 50k batch run
  in the browser.
- **Found and fixed during the check:** "→" rendered as ☐ (missing glyph),
  in the new numbers panel and in stage 1's existing probe-slider label; and
  the numbers panel overran its rect on a phone. Both fixed before run 2.

### Mesh images (the geometry-drawing rule)

In `cht_double_pipe_gh1156/`, written by
`cargo run --release -p dhoby-ghaut --example foam_studio_web -- --render-exchanger`.
They are drawn from the three region `PolyMesh`es the solver's meshes are
built from, concatenated, with one zone per region:

| file | what |
|---|---|
| `exch_demo_20k_xy_regions.png` | the demo grid's cross-section at mid-length, coloured by region |
| `exch_demo_20k_xy_wall_zoom.png` | the same, zoomed onto the tube wall (r 0.5–0.6 cm) and both interfaces |
| `exch_demo_20k_xz_regions.png` | x-z slice of the first 4.4 cm (inlet end), coloured by region |
| `exch_demo_20k_xy_cells.png` | the cross-section, one colour per cell |
| `exch_vv_8k_*` | the same four for the V&V grid n = 4, nz = 50 |

**Checked:**

- three nested regions in the right order;
- 20 (demo) and 16 (V&V) cells around every region;
- the radial grid lines continue straight across both interfaces in the
  zoom (conformal; the face pairing is also asserted < 1e-12 m in the tests);
- the inner interface at x = 0.494 cm on the y = 0 axis, the 20-gon edge
  midpoint 0.5·cos(π/20);
- the wall two cells thick;
- uniform 2.67 mm axial cells;
- the concatenated volume equal to the shell polygon prism to the digits
  printed (6.180340e-5 / 6.122935e-5 m³).

Follow-ups (50k browser run, higher-order T convection, buoyancy, non-conformal interfaces): gh:#1171.

**Added 2026-11-04 (gh:#1178, the design grid of rung 1 of gh:#1177):** run
with unequal flow rates (`annulus_flow_rate`) over Re_hot 10–90,
Re_cold 5–45, k_wall 1–100 and L 0.1–0.3 m, ε_CFD against ε-NTU(UA_local)
ranges from −3.9 % to +2.0 %, not only the −0.8 % measured here at
Re_hot = 30. Two effects this record's grid did not reach: axial wall
conduction at k_wall = 100 and L = 0.1 m (λ ≈ 0.07–0.11) lowers ε by up to
4 %; and at Re_hot = 10 (Pe = 67) 0.9–2.5 % of `report`'s `q_hot` enters by
conduction through the fixed-temperature hot inlet, so the ε defined here
(Q_hot including conduction, over C_min ΔT) can exceed the ideal value and
even 1 (1.0136 at the worst point). The definition is unchanged; see
`crates/outram-park-digital-twin-engine/verification_and_validation/hf_lf_double_pipe_gh1178.md`.

**Not checked by eye:** any slice other than mid-length and the inlet end
(the outlet end and every other layer are covered only by the volume and
the pairing assertions), and the 50k batch grid.
