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
  of the core (r < R/3; negative is outward), and the vortex-pair sign.
- **Straight-pipe limit:** the same code with D_w = 10 m (1000 d), 0.01 turns
  (L = 31.4 d), pitch 0, at Re = 100 (De = 3.2, Mishra & Gupta +0.2 %), m = 3
  cross-section and Δs = d/3, gradient fit over s ∈ [15 d, 29 d] (past the
  Durst 5.8 d development length); against 64/Re.
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

## Results

(Not yet run.)
