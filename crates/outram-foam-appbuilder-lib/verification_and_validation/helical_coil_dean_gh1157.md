# Laminar Dean flow in a helical coil: f_coil/f_straight against Mishra & Gupta (1979) (gh:#1157)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

**Issue:** gh:#1157 (the CFD coil section of the helical steam-generator demo,
epic gh:#1153). Builds on gh:#1155 (`laminar_pipe_gh1155.md`) and gh:#1161
(`non_orthogonal_correction_gh1161.md`). **Date:** 2026-11-03.

## Question

On a pipe O-grid swept along a helix, does OUTRAM PARK's `PimpleFoam` produce
the secondary (Dean) flow of a laminar coil (core flowing outward, the axial
maximum pushed to the outer wall), and does its friction factor rise above the
straight-pipe 64/Re as the laminar coil correlation of Mishra & Gupta (1979)
says, as a function of the Dean number?

## Methodology (fixed 2026-11-03, before any run)

**Mesh:** `outram_foam_mesh::block_primitives::HelicalPipe`: the
`PipeOGrid` of the tube's developed length, every point mapped onto the helix
on a rotation-minimising frame (no twist between layers). Images:
`helical_coil_dean_gh1157/` (see "Mesh images").

**Coil:** tube d = 10 mm, coil diameter D_w = 100 mm, pitch h = 20 mm (2 d),
3 turns. Curvature diameter `D_c = 2/κ = D_w [1 + (h/(π D_w))²]` = 100.405 mm,
d/D_c = 0.09960. Developed length L = 3 √((π D_w)² + h²) = 0.9444 m = 94.4 d;
one turn L_t = 31.5 d.

**Flow:** `cases::laminar_pipe::PipeFlow::build_solver` unchanged (uniform
inlet, p = 0 outlet, no-slip wall, from rest), bulk U = 5 mm/s, Re set
through ν. Re = 50, 100, 200, 400, 800, i.e. **De = Re √(d/D_c) = 15.8, 31.6,
63.1, 126, 252**, all far below Schmidt's (1967) critical Re (9 300 at this
d/D_c, `tampines` `critical_reynolds_schmidt`).

**Developing, not periodic, and why.** The developed coil flow is periodic in
s, but `PimpleFoam` has no mean-pressure-gradient source (OpenFOAM's
`meanVelocityForce`), and one turn maps onto the next by a pure translation
only on the Frenet frame, whose cross-sections shear the cells; on the
untwisted frame the O-grid comes back rotated by `τ L_t` (23° per turn
here), so the cyclic faces would not match. So: three turns from a uniform
inlet, and the developed quantities are measured on turn 3.

**Solver settings:** PISO, 1 outer and 2 correctors, Euler pseudo-transient
at Co = 4 on the axial cell length, the time step capped at a viscous number
`ν Δt / h² ≤ 0.5` (h the mean cell size, gh:#1162), `Gauss linear corrected`
Laplacians with one non-orthogonal corrector (gh:#1161, gh:#1167), run to the
stage 0 steady criterion (max |ΔU|/U per flow-through time L/U < 1e-3).

**Convection: `Gauss linear` (deferred correction), chosen before running and
kept.** The Dean vortices are a balance of in-plane convection and viscosity.
Upwind's numerical viscosity `|u| h / 2` on the in-plane secondary velocity
(~0.2 U) at h ≈ d/12 is about `Re/120` times ν, i.e. 3 ν at Re = 400: it would
compete with the physics being measured. `Gauss linear` is second order; it is
assembled as a deferred correction on the upwind matrix
(`solvers::schemes::div_vec_scheme`). The upwind default is run once as an
ablation (De = 126, m = 3) and reported whatever it shows.

> **CORRECTED 2026-11-12 (gh:#1363), not re-run.** Since dfb334c42
> `DivScheme::GaussLinear` is upstream's fully implicit `Gauss linear`
> (differential vs OpenFOAM-dev `icoFoam` cavity, ~1e-13). Every number in this
> record was measured with the **deferred-correction** form, which is not an
> OpenFOAM scheme; it survives as `DivScheme::GaussLinearDeferred`, and Foam
> Studio's coil runs now name it explicitly. With upstream's implicit
> `Gauss linear` the coarse Foam Studio coil at Re 200 **diverges**
> (`headless_coil_shows_dean_flow`, `a_coarse_coil_shows_dean_flow`). Read
> "Gauss linear" below as "Gauss linear, deferred correction". Whether this
> study should be re-run with the upstream scheme is open (gh:#1363).

**Grids:** the similar family `n = nr = 2m` (cross-section `20 m²` cells, as
`PipeOGrid::level`), `round(m L/d)` layers (Δs = d/m):
m = 2: 80 × 189 = 15 120 cells; **m = 3: 180 × 283 = 50 940 (the De sweep)**;
m = 4: 320 × 378 = 120 960.

**Instruments (fixed before the runs):**

- **Friction factor:** `−dp/ds` from a volume-weighted p-against-s fit over
  every cell with s on turn 3, `[2 L_t, 3 L_t − 2 d]` (2 d kept off the
  outlet), `cases::helical_coil::arc_pressure_gradient`, then the Darcy
  `f = (−dp/ds) d / (½ U²)`. **Quoted as f·Re/64 = f_coil / f_straight**,
  against Mishra & Gupta's `f_c/f_s = 1 + 0.033 (log₁₀ De)⁴` (Ind. Eng. Chem.
  Process Des. Dev. 18(1), 130–137, 1979, doi:10.1021/i260069a017), **called
  from `tampines::components::helical_coil_otsg::correlations::darcy_friction_laminar_mishra_gupta`**
  (its provenance is in that file: a secondary source, the primary paper not
  read). Range of the correlation: 1 < De < 3000.
- **Same-grid ratio:** f·Re/64 divided by the straight-limit value on the same
  cross-section (below), which takes the O-grid's polygon offset (+1 % at
  m = 3, `laminar_pipe_gh1155.md`) out. Reported beside, not instead of, the
  absolute one.
- **Development:** the same fit on turn 2, `[L_t, 2 L_t]`; the ratio
  turn 2 / turn 3. If it differs from 1 by more than 1 %, turn 3 is not
  called developed and that row is flagged.
- **Dean section** (`cases::helical_coil::dean_section`, the cells within
  half a layer of s = 2.5 L_t): `n/R` of the axial maximum (negative is
  outward), the largest secondary speed `√(u_n² + u_b²)/U`, the mean `u_n/U`
  of the core (r < R/3; negative is outward), and the vortex-pair sign
  (negative for the classic pair whose return flows meet at the inner wall;
  **corrected 2026-11-03 before any V&V run**: the doc comment first said
  positive, a sign slip in the derivation, found when the regression test's
  coarse coil gave a negative value and the circulation was re-derived).
- **Straight-pipe limit:** the same code with D_w = 10 m (1000 d), 0.01 turns
  (L = 31.4 d), pitch 0, at Re = 100 (De = 3.2, Mishra & Gupta +0.2 %), m = 3
  cross-section and Δs = d/3, gradient fit over s ∈ [15 d, 29 d] (past the
  Durst 5.8 d development length); against 64/Re. (As run, the window is the
  code's general rule `[L − w, L − 2d]` with w = min(one turn, L/2), i.e.
  [15.7 d, 29.4 d]; stated here because it differs from the line above by
  0.7 d at the start.)
- **Grid convergence:** m = 2, 3, 4 at Re = 200 (De = 63); observed order
  from the three, as in `laminar_pipe_gh1155.md`.
- **Dean (1928) small-De asymptote:** not done. It was not read in this
  session, and the task's rule is to cite only a source actually read.

## Predictions (written 2026-11-03, before any flow run on the coil)

| # | quantity | predicted sign and magnitude |
|---|---|---|
| P1 | straight limit, f·Re/64 at m = 3 | 1.008 to 1.013 (the m = 3 straight O-grid of gh:#1155 gave 1.0108; De = 3.2 adds 0.2 %) |
| P2 | sweep at m = 3, f·Re/64 against Mishra & Gupta | De ≤ 63: CFD above MG by 0 to +2 % (the polygon offset); De = 126: within ±5 %; De = 252: CFD **below** MG by 3 to 10 % (the secondary boundary layers, ~d De^-1/2, are only 2–3 cells thick) |
| P3 | development, turn 2 / turn 3 gradient | within 1 % for Re ≤ 400; within 3 % at Re = 800 |
| P4 | Dean section | core u_n < 0 (outward) and axial maximum at n < 0 (outer side) at every De; |n_peak|/R from ~0.1 at De 16 to 0.4–0.6 at De 252; max secondary 0.1–0.4 U for De ≥ 63 |
| P5 | grid study at De 63 | monotone; observed order between 1 and 2; m = 3 within 2 % of the extrapolated f·Re |
| P6 | upwind ablation, De 126 | f_coil/f_straight **lower** than Gauss linear's by 3 to 15 % (damped secondary flow) |
| P7 | cost | m = 3, Re = 200: about 400 steps, about 5 min on this 4-CPU host |

## Results (2026-11-03)

Runs: `cargo run --release -p dhoby-ghaut --example foam_studio_web -- --coil-run <m> <Re> [linear|upwind] [straight] [co=<Co>] [tol=<tol>] [svg=<file>]`
(`examples/foam_studio_web/coil.rs`, `run_to_steady`), each printing one
`ROW` line; wall times are on the shared 4-CPU host with two to four runs at
once, so they are indicative only. Every run's inlet/outlet flux balance is
the `laminar_pipe` boundary set-up's (the demo's headless coil regression
asserts out/in = 1 to 1e-8). Repeating Re = 100 gave the same `ROW` to every
printed digit (determinism).

### Straight-pipe limit (P1)

| case | cells | steps | f·Re/64 | predicted |
|---|---|---|---|---|
| D_w = 1000 d, m = 3, Re 100 (De 3.2) | 16 920 | 42 (14 s) | **1.01064** | 1.008 to 1.013 ✓ |

The swept mesh reduces to the straight O-grid's answer (gh:#1155's m = 3
straight grid gave 1.0108; Mishra & Gupta add 0.2 % at De 3.2). This is the
denominator of the same-grid ratio below.

### Dean-number sweep, m = 3 (50 940 cells), Gauss linear (P2, P3, P4)

| Re | De | Co | steps (wall) | f·Re/64, turn 3 | turn 2 / turn 3 | Mishra & Gupta | CFD / MG | same-grid ratio / MG | predicted CFD / MG |
|---|---|---|---|---|---|---|---|---|---|
| 50 | 15.8 | 4 | 49 (108 s) | 1.04338 | 1.00001 | 1.06799 | **0.977** | 0.967 | 1.00 to 1.02 ✗ |
| 100 | 31.6 | 4 | 73 (144 s) | 1.16616 | 0.99973 | 1.16667 | **1.000** | 0.989 | 1.00 to 1.02 ✓ (at the edge) |
| 200 | 63.1 | 4 | 150 (208 s) | 1.38521 | 0.99907 | 1.34654 | **1.029** | 1.018 | 1.00 to 1.02 ✗ |
| 400 | 126 | 1 | not steady, see below | 1.68964 | 0.9976 to 0.9948 | 1.64323 | **1.028** | 1.017 | 0.95 to 1.05 ✓ |
| 800 | 252 | 1 | not steady, see below | 2.158 to 2.216 | 0.97 to 1.01 | 2.09890 | **1.03 to 1.06** | 1.02 to 1.04 | 0.90 to 0.97 ✗ (wrong sign) |

**Quote:** the CFD / MG column for De ≤ 126, with De 400's caveat; the
same-grid column removes the O-grid's +1.06 % offset (P1) and is the better
estimate of the coil effect alone. De = 252 has no converged value.

**Developed (P3):** turn 2 and turn 3 agree to 0.1 % at De ≤ 63 ✓. At
De = 126 turn 3 is stationary but turn 2 drifts (below); at De = 252 neither is.

**Dean section at s = 2.5 L_t (P4)** (cells of one layer; n/R is quantised
by the cell size, about 0.15 R at m = 3):

| De | axial max n/R | axial max / U | max secondary / U | core u_n / U | vortex pair |
|---|---|---|---|---|---|
| 15.8 | −0.12 | 1.963 | 0.128 | −0.097 | −0.053 |
| 31.6 | −0.38 | 1.887 | 0.181 | −0.141 | −0.081 |
| 63.1 | −0.53 | 1.766 | 0.199 | −0.134 | −0.089 |
| 126 (upwind ablation; the linear run was stopped before a report) | −0.53 | 1.573 | 0.189 | −0.107 | −0.078 |
| 252 (not converged, step 300) | −0.82 | 1.646 | 0.210 | −0.037 | −0.043 |

Every De: the core flows outward (u_n < 0), the axial maximum is on the outer
side and moves outward with De, the axial maximum falls below the
Hagen–Poiseuille 2U, and the return flows meet at the inner wall (pair < 0):
the Dean vortices. P4 ✓ for the signs and for |n_peak| ≈ 0.1 at De 16; at
De 252 |n_peak| = 0.82 is above the predicted 0.4–0.6, but that run is not
converged. Pictures from the solver's cells: `dean_m3_re100.png`,
`dean_m3_re200.png` (and `.svg`), see "Images".

### Re ≥ 400 does not reach a steady state (P7 and the protocol)

- **At Co 4 neither converged.** Re 400: the steady metric fell to 3.6e-2 by
  step 150, then grew (7.5e-2 at 250); Re 800: 1.7 at step 200, 9.5 at 300.
  Both stopped. (Re ≤ 200 converged at Co 4 in 49–150 steps.)
- **At Co 1, Re 400:** the metric fell to 0.13 at step 250, then grew
  steadily (2.5 at step 800, ×20 in 550 steps). The largest one-step change
  sat in turns 1 and 2 (s = 0.22–0.67 m, r/R 0.3–0.8). Turn 3's f·Re/64 was
  1.69200, 1.68978, 1.68971, 1.68964 at steps 150, 250, 300, 500 and 1.68964
  to step 800, while turn 2's drifted from 1.68773 to 1.68084 as the
  disturbance spread. **The quoted 1.68964 is a stationary turn-3 value, not
  a converged solution** by the record's criterion.
- **At Co 1, Re 800:** no stationary value; over steps 100–500 turn 3's
  f·Re/64 moved within 2.158–2.216 and turn 2's within 2.098–2.224; the
  largest change wandered over turns 2 and 3 (r/R 0.5–0.8).
- **Not diagnosed.** Laminar coil flow at these De (Re far below Schmidt's
  9 300) is expected to be steady, so a numerical cause is the first
  hypothesis: `Gauss linear` as a deferred correction at axial cell Péclet
  numbers `U Δs/ν` = Re/3 ≈ 130–270. Upwind at Re 400, Co 1 converged in
  182 steps. Filed as gh:#1172. Nothing was tuned
  to make these rows pass; they are reported as they came.

### Grid convergence at De = 63 (P5)

| m | cells | steps | f·Re/64 |
|---|---|---|---|
| 2 | 15 120 | 292 (98 s) | 1.42231 |
| 3 | 50 940 | 150 (208 s) | 1.38521 |
| 4 | 120 960 | 478 (62 min) | 1.37396 |
| extrapolated (p from the three) | | | **1.3626** |

Monotone; **observed order p = 2.40** (predicted 1 to 2 ✗, a little above);
m = 3 is 1.66 % above the extrapolated value (predicted within 2 % ✓). The
extrapolated f_coil/f_straight, 1.3626, is **+1.2 % above Mishra & Gupta**
(1.3465); dividing by the straight O-grid's grid-limit 64.09/64 (gh:#1155)
gives 1.3607, +1.1 %. That is the best single number of this record.

### Steady criterion (protocol check)

Re = 200, m = 3 run on to the metric < 1e-4 (292 steps): f·Re/64 = 1.38521,
the same five digits as at 1e-3 (150 steps). The criterion does not move the
answer at the precision quoted.

### Convection ablation at De = 126 (P6)

| scheme | Co | steps | f·Re/64 | CFD / MG |
|---|---|---|---|---|
| Gauss linear (the record's) | 1 | not steady | 1.68964 | 1.028 |
| Gauss upwind | 1 | 182, steady | **1.85225** | 1.127 |

Upwind's f is **9.6 % higher**, not lower: **P6 ✗, wrong sign.** The reasoning
in P6 (upwind damps the secondary flow, so less enhancement) was wrong in
what it left out: the numerical viscosity also carries axial momentum across
the section with the secondary flow, steepening the wall gradient, and that
wins here. The secondary speed did fall (0.189 U against 0.199 U at De 63),
as P6 assumed. The choice of `Gauss linear` stands for the reason given before
the runs, and this row shows what the default would have cost: a 13 % miss
against the correlation instead of 3 %.

### Cost (P7)

m = 3 at Re 200: 150 steps, 208 s with three runs sharing the host (P7: about
400 steps and 5 min; fewer steps than predicted). Re ≥ 400 never finished
(above).

### What misses, in one place

- De 15.8 is **2.3 % below** Mishra & Gupta (3.3 % on the same-grid ratio),
  outside P2's 0 to +2 %; De 63 is 2.9 % above (1.8 % same-grid; +1.2 % in the
  grid limit). Mishra & Gupta is itself a fit to data; its scatter was not
  read in this session, so whether a ±3 % difference is inside it is open.
- Re ≥ 400 is not steady on m = 3 with `Gauss linear` (undiagnosed); De 252
  has no converged friction factor, and its band lies above Mishra & Gupta
  where P2 predicted below.
- P6 had the wrong sign; P5's order is above the predicted range.

## The demo coil (not the V&V grid)

Foam Studio's "Helical coil section" (`examples/foam_studio_web`,
`engine::Design::coil`, `coil::CoilSpec::for_budget`): the V&V tube and coil
with **2 turns** at the 20k default, cross-section `(n, n − 2)` with as many
layers as fit up to 3 per diameter and none longer than d: **8 × 6 × 78
layers, 19 968 cells** (layers 0.81 d long, against d/3 on the V&V grid; its
volume is 0.43 % short of A·L from the long chords, measured, against 0.074 %
for m = 3). Native headless, Re 200: 215 steps, f·Re/64 = **1.3819** on turn
2 (1.3924 on turn 1, which holds the entrance), Mishra & Gupta 1.3465, the
axial maximum at n/R = −0.48, core u_n −0.136 U. That agrees with the V&V
m = 3 value (1.3852) to 0.2 %, so the demo's number is a fair one at De 63.
**Why 2 turns:** 3 turns at 20k leave layers of 0.9 d (`for_budget` picks
7 × 5 × 105); with 2 the turn-1/turn-2 difference is 0.8 %, and the m = 3 V&V
runs show turn 2 developed to 0.1 % at De ≤ 63. Above Re 250 the demo marches
at Co 1 (`engine::COIL_CO4_MAX_RE`), because Co 4 did not converge there on
the V&V grid; whether Co 1 converges on the demo grid at Re 400–800 was not
measured, and on the V&V grid it does not (above).

## Images

In `helical_coil_dean_gh1157/`.

**Mesh** (`cargo run --release -p dhoby-ghaut --example foam_studio_web -- --render-coil`,
from the `PolyMesh` the solver's `FvMesh` is built from, one colour per cell,
through `outram_blender`'s mesh plotter):

| file | what |
|---|---|
| `vv_m3_xz_axis.png`, `demo_20k_xz_axis.png` | the whole coil, cut by the vertical plane through its axis (y = 0): every half-turn crosses it |
| `vv_m3_xy_plan.png`, `demo_20k_xy_plan.png` | plan view, horizontal cut at mid first turn |
| `vv_m3_xz_section_{1,4,6}pi.png`, `demo_20k_xz_section_{1,3,4}pi.png` | zoom onto single tube sections on y = 0 at φ = π … (tilted from the true cross-section by the 3.6° pitch angle) |

**Checked by eye (2026-11-03):** the axial cut shows 3 (2) turns as tube
sections at x = ±5 cm, rising 1 cm per half turn, the inlet and outlet cut in
half at z = 0 and z = 6 (4) cm; each section is a closed 24-gon (32-gon) on
the 5 mm circle with a 6 × 6 (8 × 8) core, no gaps, overlaps or folded cells;
the core square's orientation turns from section to section by −τ s about the
tube axis (−45.7° at s = 0.630 m, 4π, visible in `vv_m3_xz_section_4pi.png`),
which is the untwisted frame turning against the Frenet frame, as designed:
the cells are not twisted between layers, the frame is. The plan view is the
expected crescent of half a turn either side of φ = π. The printed volumes
match A·L to −7.4e-4 (V&V) and −4.3e-3 (demo), the measured second-order chord
shortfall `−0.166 (κΔs)²` (`block_primitives` test). **Not checked:** the
faces between layers away from the cut planes, the face warping (not
plotted), and the patch assignment (not coloured; the inlet/outlet flux
balance and the inlet's normal being −T, asserted in
`helical_mesh_has_the_swept_volume_and_locates`, cover it).

**Flow:** `dean_m3_re100.{svg,png}`, `dean_m3_re200.{svg,png}` (written by
`--coil-run … svg=`): the solver's cells of one layer at s = 0.786 m, in the
Frenet plane, coil axis left, outer wall right, fill u_t/U, arrows the
secondary velocity. Both show the two Dean cells: the core moving outward,
the return flow along the upper and lower walls, the axial maximum on the
outer side.

**Browser** (`browser_phone_390_*.png`, `browser_desktop_1280_*.png`): see
the next section.

## The browser demo (checks, not V&V)

Full wasm build (`web/foam_studio/build.sh`, 12 MB), served locally on a
free port, driven by `helical_coil_dean_gh1157/browser_check.js` (gh:#1155's
script with the coil's URL, `?autostart&shape=coil&cells=20000`) in headless
Chromium (Playwright's build, SwiftShader software GL) on the shared 4-CPU
host, with one native V&V run (m = 4) still going. While the worker solves,
the script taps "+" and drags on the view six times and records every
animation frame. Report: `helical_coil_dean_gh1157/browser_report_20000.json`.

| viewport | to steady (wall) | steps · last ms/step (worker) | frame interval while solving: median / p95 / max | after: median / max | input round trips |
|---|---|---|---|---|---|
| phone 390×844 (touch, DPR 2) | **229 s** | 215 · 777 ms | 83 / 133 / 400 ms | 16.7 / 67 ms | 0.3–1.5 s |
| desktop 1280×800 | **219 s** | 215 · 725 ms | 16.7 / 16.8 / 517 ms | 16.7 / 117 ms | 0.3–0.7 s |

- **It runs, and converges in the browser to the native answer:** 215 steps,
  f/f_s 1.382 on the last turn, 1.392 the turn before, Mishra & Gupta 1.347,
  the same numbers as the native headless run (title, status line and the
  dock's Dean panel show them).
- **Phone layout:** the panel opens folded ("Controls »"); + / − / Reset and
  the orbit / pan toggle sit top right; the bottom dock holds the residual
  history and the Dean cross-section (coil axis left, outer wall right, colour
  u_t/U, white arrows the secondary flow) with its numbers beside it.
  Screenshots: `browser_phone_390_solving.png` (mid-solve, after the zoom
  taps, while f is still negative from the start-up transient),
  `browser_phone_390_steady.png`, `browser_desktop_1280_steady.png`.
- **Responsiveness:** on the desktop the median frame stays at 16.7 ms
  through the solve. On the phone emulation it is 83 ms while solving and
  16.7 ms after: the page repaints every 100 ms while a run is going, and at
  DPR 2 SwiftShader rasterises four times the pixels of the desktop (the coil's
  ~5 000 wireframe edges plus the Dean panel's ~800 arrow strokes) on CPUs
  that the worker and a native solve also use. The zoom taps and drags took
  effect mid-solve. It was not measured on a hardware GPU. The first run of
  this check, with three native solves on the host, gave 133 ms (phone) and
  100 ms (desktop) medians and 287–307 s to steady.
- **Found and fixed during the check:** the font has no ← / → glyphs (the
  Dean panel's side labels showed boxes), the panel's numbers ran off a phone
  screen, and the colour bar's label overlapped its end values; all three
  changed before the second run, which the table reports.
- **Against the 20k interactive default:** 3.7 min to steady is far from the
  pipe's 13 s; the coil needs about six times the steps (two turns are 63 d
  of tube, L/U of 126 s at Co 4) at about three times the cost per step
  (Gauss linear, the coil report each chunk). Fewer cells (the ~10k button)
  roughly halves it; the 20k default stays because it is what gives the
  0.2 % agreement with the V&V grid above.

## Re-run with upstream's implicit `Gauss linear` (gh:#1500, 2026-11-12)

Maintainer direction, 2026-11-12: "Deprecate the old scheme, replace helical
coil with new one." `DivScheme::GaussLinearDeferred` is deprecated; Foam
Studio and this record now use `DivScheme::GaussLinear`, upstream's fully
implicit form. This section is written in the order it was done: diagnosis
hypotheses first, then results (added below, after the runs).

### Diagnosis of the coarse-coil divergence: hypotheses (written BEFORE any test run)

**Observation (2026-11-12, `--coil-run 2 200 linear trace`, m = 2, 15 120
cells, Co 4, dt = 2.0 s):** step 1 completes (max |U| = 1.77 U, change
1.7e2), step 2 returns non-finite values. The solver has no under-relaxation
and no momentum-predictor switch (`PimpleFoam::step_flow`: Euler, one outer
corrector, 2 pressure correctors, one non-orthogonal corrector); the knobs are
the Courant number, the time step, the mesh and the number of outer/pressure
correctors.

**Upstream read first (OpenFOAM-dev 852527377c7e and v2506 tutorials, read
2026-11-12).** The one laminar PISO tutorial that uses an unbounded
`div(phi,U) Gauss linear` is `legacy/incompressible/icoFoam/cavity`: dt =
0.005, dx = 0.1/20 = 0.005, lid U = 1, i.e. **a cell Courant number of 1**
(the same case as the port's 1e-13 differential). Every laminar or
unstructured tutorial that runs above Co 1 uses a bounded scheme
(`icoFoam/elbow`, `pimpleFoam/laminar/*` and `pipeCyclic`: `limitedLinearV 1`;
`ductSecondaryFlow`, `planarPoiseuille`: `linearUpwind`), and the steady ones
(`moodyChart`, `ductSecondaryFlow`) use `relaxationFactors` with large
`nOuterCorrectors`. Nothing upstream runs an unbounded implicit central
scheme at Co 4 with PISO and no relaxation.

**H1 (Courant number; the leading hypothesis).** The implicit central
convection matrix has off-diagonals `+-phi/2` and diagonal `V/dt`
(+ viscous); it is diagonally dominant only while the face-flux Courant
number `phi dt / V` is at most about 1 (viscosity is small at Re 200: cell
Peclet `U ds / nu` = Re/2 = 100). The run's `Co = 4` (on 2U, `pseudo_time_step`)
is four times that, and the impulsive start from rest adds a large pressure
transient. The deferred form hid this because its implicit part is upwind,
which is always dominant. *Prediction:* the coarse coil diverges at Co 4 and
at Co 2, and is stable at Co <= 1 (the cavity's value) and reaches the
steady criterion; its steady f*Re/64 then lies within a few percent of the
deferred form's (same order of accuracy, different operator), not equal.

**H2 (a defect at boundaries on the swept O-grid, e.g. the implicit central
term at the inlet fixed-value / outlet zero-gradient faces or the
non-orthogonal correction).** *Prediction:* divergence persists at small Co
(0.1 or less) and first appears next to the inlet/outlet or the wall. If H1's
Co <= 1 run is stable, H2 is not needed.

**H3 (nonlinear outer iterations).** More outer correctors would stabilise
Co 4: *prediction:* nOuter = 3 at Co 4 does not cure it (no relaxation to damp
the extra iterations), so H3 is the weakest.

**Protocol rule, fixed now:** whatever the cause, the fix changes the
Courant number / time step (the protocol), never a gate; the tests' physics
assertions (steady criterion 1e-3, mass balance 1e-8, Dean signs) are not
touched; any pinned number that moves is re-recorded old -> new.

### Diagnosis, part 2: after Co 1 the steady criterion is still not reached (hypotheses written BEFORE the test)

**Results of the H1 test (2026-11-12, m = 2, Re 200, `--coil-run 2 200 linear
<max_steps> co=<Co> trace`; the upstream `CoNum` of `CourantNo.H` is printed
by `engine::courant_numbers`):** Co 4 (CoNum 5.9 at step 1) diverges at step
2; Co 2 (CoNum 2.6) at step 6; Co 1.5 (CoNum 1.8) at step 10; Co 1 (CoNum 1.07
at step 1, 1.5 at step 40) and Co 0.5 run on. **H1 holds** (stable below, and
only below, a Courant number of about 1 to 1.5, as predicted; and H2 is not
needed: no defect shows at Co <= 1). H3 (outer correctors) was not tested; the
Co result made it unnecessary.

**New observation (m = 2, Re 200, Co 1, 3000 steps, 445 s):** the run does
**not** reach the steady criterion. f·Re/64 is 1.42097 from step 250 on (the
same five digits to step 3000), but the steady metric sits at **4.680e-3**,
constant to four digits for 2 500 steps, with the largest one-step change
|dU| = 6.19e-8 m/s always in the same cell (turn 1, s = 0.267 m, r/R 0.19):
a state that is fixed, or a period-2 cycle of fixed amplitude, not a slow
drift (the friction factor does not move). The criterion is 1e-3 and is not
touched.

**H4 (the linear-solver floor).** The momentum equation is solved by
warm-started Gauss-Seidel to a normalised-residual tolerance of 1e-7
(`SolverSettings::default`), and the implicit central matrix is far less
diagonally dominant than the upwind-based one, so the sweeps stop with a
residual just under 1e-7 and the state wanders by ~1e-7 m/s (U = 5e-3 m/s:
1e-5 of U) from step to step: a floor set by the solver, not by the physics.
*Prediction:* tightening the momentum tolerance to 1e-10 and 1e-12 lowers the
plateau by roughly the same factor (it falls below 1e-3, i.e. the run stops
on the unchanged criterion), and f·Re/64 does not change in its first four
digits.

**H5 (a real two-grid / odd-even oscillation of the central scheme at cell
Peclet 100).** *Prediction:* the plateau amplitude does not depend on the
linear tolerance; the change alternates sign each step.

**Result of the H4/H5 test (2026-11-12, m = 2, Re 200, Co 1, same binary,
`utol=` set on the momentum solve):**

| momentum tolerance | steps to the unchanged 1e-3 criterion | steady metric at step 300 | f·Re/64 |
|---|---|---|---|
| 1e-7 (default) | **not reached in 3 000** (plateau 4.680e-3) | 4.680e-3 | 1.42097 |
| 1e-10 | **351** | 3.295e-3 | 1.42097 |
| 1e-12 | reached (stopped at 400 by the harness cap) | 3.295e-3 | 1.42097 |

**H4 holds, H5 does not**: the plateau is the linear solver's floor; at 1e-10
and 1e-12 the trajectories agree to four digits and the answer is the same to
five. The coil runs therefore set the momentum tolerance to 1e-10
(`coil::U_TOLERANCE`; the point where the metric stops depending on the
tolerance, chosen from this table, not from a target value). The 1e-3
criterion, the mass-balance gate and the Dean-sign assertions are unchanged.

**The protocol that follows (gh:#1500): `Gauss linear` (upstream's), Co 1,
momentum tolerance 1e-10.** What the earlier deferred-form record ran was
`GaussLinearDeferred`, Co 4 (1 above Re 250), tolerance 1e-7.

## Reproduce

```bash
cargo test --release -p outram-foam-mesh --lib block_primitives
cargo test --release -p outram-foam-appbuilder-lib --lib helical_coil
cargo test --release -p dhoby-ghaut --example foam_studio_web coil
cargo run  --release -p dhoby-ghaut --example foam_studio_web -- --coil-run 3 200 linear
cargo run  --release -p dhoby-ghaut --example foam_studio_web -- --coil-run 3 100 linear straight
cargo run  --release -p dhoby-ghaut --example foam_studio_web -- --render-coil
```
