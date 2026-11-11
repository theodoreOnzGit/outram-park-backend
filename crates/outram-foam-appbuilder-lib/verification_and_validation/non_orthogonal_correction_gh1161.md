# PimpleFoam non-orthogonal correction (gh:#1161)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

**Issue:** gh:#1161 (found by stage 0 of Foam Studio, gh:#1154; gates the
stage 1 pipe V&V, gh:#1155)
**Started:** 2026-11-03, from commit `86b8c71df`

## The defect

`PimpleFoam` assembled both of its Laplacians, `fvm::laplacian(rAU, p)` and
the viscous `fvm::laplacian_vec(ν, U)`, with `1/|d|` coefficients and no
explicit correction, whatever `schemes.default_laplacian` said (its default
already read `Gauss linear corrected`). On the stage 0 O-grid pipe the
developed dp/dz was +3.16 / 3.18 / 3.19 % above Hagen–Poiseuille at
5k / 20k / 50k cells; the polygonal wall explains only 1.15 / 0.64 / 0.41 %,
and the remainder (≈ 2.0 / 2.5 / 2.8 %) does not shrink with refinement.

## What upstream does (read 2026-11-03, before any change)

Read from OpenFOAM-dev (`master`, fetched 2026-11-03):
`src/finiteVolume/finiteVolume/laplacianSchemes/gaussLaplacianScheme/gaussLaplacianSchemes.C`,
`.../snGradSchemes/correctedSnGrad/correctedSnGrad.C`,
`src/finiteVolume/interpolation/surfaceInterpolation/surfaceInterpolation/surfaceInterpolation.C`;
and from OpenFOAM-10 (dev has moved the solver into a module):
`applications/solvers/incompressible/pimpleFoam/pEqn.H` and
`src/finiteVolume/cfdTools/general/solutionControl/solutionControl/nonOrthogonalSolutionControl/nonOrthogonalSolutionControl.C`.
No OpenFOAM source is vendored in this workspace; the files were fetched to
a scratch directory and are not committed.

1. **Implicit coefficients.** `fvmLaplacianUncorrected(γ|Sf|,
   snGradScheme.deltaCoeffs(vf), vf)`. For `corrected` the delta
   coefficients are `nonOrthDeltaCoeffs`:
   `1/max(n·d, 0.05|d|)` on internal faces, and on boundary faces
   `1/max(n·δ, 0.05|δ|)` with `δ = fvPatch::delta()`, the patch-normal
   projection of `Cf − C`, i.e. `1/(n·(Cf − C))`. Only `orthogonal` uses
   `1/|d|` (which is what the port did).
2. **Explicit correction.** `fvm.source() -= V·div(γ|Sf| · k_f·(∇φ)_f)` with
   `k_f = n − d·nonOrthDeltaCoeff` (`makeNonOrthCorrectionVectors`) and
   `(∇φ)_f` the `linear` interpolate of the cell gradient from `gradSchemes`
   (`Gauss linear` in the tutorials). **The correction vectors are set to zero
   on every non-coupled boundary patch**, so no boundary face gets an
   explicit term. For a vector field (the momentum Laplacian) it is applied
   component by component.
3. **Flux.** `p` is flux-required, so the face correction is kept as
   `faceFluxCorrection` and is part of `pEqn.flux()`; `phi = phiHbyA −
   pEqn.flux()` subtracts it.
4. **Loop.** `while (pimple.correctNonOrthogonal())` assembles, solves and,
   on the final pass only, updates `phi`. `nNonOrthogonalCorrectors` is
   `lookupOrDefault(..., 0)`: **one pass by default, and the explicit
   correction is still applied in it**, evaluated from the `p` the pass
   starts from (the previous PISO corrector or time step). It converges with
   the PISO and time iteration, not inside the pass. pitzDaily and cavity
   ship `Gauss linear corrected` and `nNonOrthogonalCorrectors 0`.

**Workspace search first.** `outram-foam-basic-lib` already has
`fvm::laplacian_with_delta(.., DeltaCoeff::NonOrthogonal)` (exactly item 1,
boundary projection included) and `fvm::laplacian_corrected` /
`non_ortho_geometry` (items 1–2 for a scalar). `laplacian_corrected` was
checked and **not used** in the solver: it adds an explicit term on
Dirichlet boundary faces (upstream: zero), it averages the two cell
gradients with equal weights (upstream: `linear`), it does not return the
face correction `pEqn.flux()` needs, and it has no vector form.
`laplacian_with_delta` and `non_ortho_geometry` are reused.

## Prediction (written 2026-11-03, before the corrected solver was built or run)

**Mechanism.** The O-grid's non-orthogonality (max 20–30°, mean 2–3°) is on
the cross-section faces near the corners of the central square and in the
first ring; it does not fall with refinement because the block topology is
the same at every resolution. In a developed pipe flow the pressure is
almost uniform over a cross-section, so the pressure Laplacian's
cross-section error matters little; the **viscous Laplacian of u_z** over the
cross-section is what sets the wall shear and so dp/dz. An uncorrected
Laplacian on a fixed-angle non-orthogonal mesh is inconsistent (its error is
O(1) in h for a quadratic field), which is a mesh-independent offset of the
kind measured.

**Predicted sign and magnitude.**

- dp/dz / HP **falls** on all three O-grids. The non-polygon residual
  (≈ 2.0 / 2.5 / 2.8 %) drops below **0.7 %** and **shrinks with
  refinement**, so dp/dz / HP lands at about **1.012–1.018 (5k), 1.007–1.012
  (20k), 1.004–1.008 (50k)**, i.e. within ~0.5 % above the polygon-area
  values 1.0115 / 1.0064 / 1.0041.
- u_c / 2U at z = 0.15 m moves by less than 0.5 % (it is set mainly by
  development length and resolution).
- Mass conservation is unchanged (outlet/inlet flux = 1 to 1e-9), because
  the same face correction goes into the matrix and into `phi`.
- **Cartesian cases move by round-off only.** The cavity and the plane
  channel have orthogonal faces: `k_f` is zero up to the rounding of
  `n = Sf/|Sf|`, and `1/(n·d)` equals `1/|d|` to the last bit or one ulp. So
  the cavity Ghia numbers, the icoFoam comparison, the BDF2 study and the
  open-channel test change below 1e-10 relative, and every pinned value
  holds. If any of them moves by more, that is a defect in this change.
- With `nNonOrthogonalCorrectors = 1` instead of 0, the steady pipe result
  changes by less than 1e-4 relative (the lag converges with the time
  iteration).

## Methodology (as run)

**What changed.** `PimpleFoam` now honours `schemes.default_laplacian`
(default `Gauss linear corrected`) for both Laplacians, through the new
`src/solvers/non_orthogonal.rs`: `fvm::laplacian_with_delta(..,
NonOrthogonal)` for the pressure, a re-stamp of the viscous matrix to
`1/(n·d)`, the explicit `k·∇φ` face correction (linear-weighted Gauss
gradient, internal faces only) on both, the same face correction subtracted
from `phi`, and the `nNonOrthogonalCorrectors` loop. The ablation is
`PimpleFoam::without_non_orthogonal_correction()` (`Gauss linear
orthogonal`, bit-for-bit the old solver); a new `LaplacianScheme` variant,
`GaussLinearOrthogonal`, names it.

**Case.** The stage 0 pipe (D = 20 mm, L = 200 mm, U = 5 mm/s, ν = 1e-6
m²/s, Re = 100, uniform inlet, p = 0 outlet, no-slip wall), from rest, PISO 1
outer / 2 correctors, Euler, upwind. Meshes: the stage 0 O-grids 6/4/40,
8/6/80, 10/8/120 (5 280 / 20 480 / 50 400 cells), now built by
`outram_foam_mesh::block_primitives::PipeOGrid` (the same blockMeshDict, now
straight through the blockMesh port's `PolyMesh::to_fv_mesh`). Δt for an
axial Courant number of 4 (stage 0's steady recommendation) unless stated.
Run to stage 0's steady criterion (max cell |ΔU|/U per flow-through time
< 1e-3) or a step cap.

**Instruments** (`src/cases/laminar_pipe.rs`, the stage 0 fits): `−dp/dz`
from a volume-weighted p-against-z fit over z ∈ [0.13, 0.17] m, against
Hagen–Poiseuille `32νU/D²`; "poly" divides that by `πR²/A_poly`, the
Hagen–Poiseuille excess of an equal-area duct at the same bulk velocity
(1.0115 / 1.0064 / 1.0041); u_c from the intercept of a `u = a + b r²` fit
over r < R/2. Test: `tests/laminar_pipe_hagen_poiseuille.rs`
(`pipe_5k_non_orthogonal_correction` runs by default; `measure_gh1161_*`
print the tables, `--ignored`).

Host: Intel Xeon 2.10 GHz, 4 logical CPUs, shared; wall times were taken
with other builds running and are indicative only (stage 0 measured 25–50 %
noise on an idle host).

## Results (2026-11-03)

### 1. The pipe, Re = 100 (z ∈ [0.13, 0.17] m, still developing)

| grid | Laplacian | steps to steady | u_c/2U at 0.15 m | (−dp/dz)/HP | ÷ polygon | out/in flux |
|---|---|---|---|---|---|---|
| 5k | orthogonal (ablation) | 19 | 0.9902 | **1.03160** | 1.01986 | 1.0000000000 |
| 5k | corrected, 0 correctors | 22 | 0.9902 | **1.01749** | 1.00590 | 1.0000000000 |
| 20k | orthogonal | 34 | 0.9940 | **1.03174** | 1.02513 | 0.9999999999 |
| 20k | corrected, 0 correctors | 301 | 0.9940 | **1.01513** | 1.00862 | 1.0000000000 |
| 20k | corrected, 1 corrector | 34 | 0.9940 | 1.01513 | 1.00862 | 0.9999999999 |
| 20k | corrected, 0 correctors, Co ≈ 1 | 122 | 0.9940 | 1.01513 | 1.00862 | 1.0000000000 |
| 50k | orthogonal | 69 | 0.9957 | **1.03188** | 1.02764 | 0.9999999999 |
| 50k | corrected, 0 correctors | **diverged** (NaN by step 36) | — | — | — | — |
| 50k | corrected, 1 corrector | 76 | 0.9954 | **1.01363** | 1.00947 | 0.9999999996 |

The ablation reproduces stage 0's numbers (1.0316 / 1.0318 / 1.0319 at
Co ≈ 1) to 3e-4, at Co ≈ 4, on the same geometry built by the new mesher.
Worst cell continuity error ≤ 5e-12 of the inlet flow in every converged row.

### 2. The pipe, Re = 25 (developed in the slab; Co ≈ 1)

At Co ≈ 4 and Re = 25 the viscous number νΔt/Δr² is about 7, and neither
solver settles in 1000 steps (orthogonal: (−dp/dz)/HP = 0.797 when stopped):
the large-viscous-number limit stage 0 already recorded. These rows are
therefore at Co ≈ 1, as stage 0's Re = 25 run was.

| grid | Laplacian | steps | u_c/2U | (−dp/dz)/HP | ÷ polygon |
|---|---|---|---|---|---|
| 20k | orthogonal | 47 | 0.9983 | **1.02386** | 1.01730 |
| 20k | corrected, 0 correctors | 368 | 0.9983 | **1.00724** | **1.00078** |

### 3. Cartesian cases (round-off only)

`tests/pimple_foam_open_channel.rs::cartesian_channel_moves_by_round_off_only`
(the plane channel, 0.2 s): corrected against orthogonal, max |ΔU|/U =
2.5e-15, max |Δp|/max p = 5.5e-14; **not bit-identical**. Cause: `n =
Sf/|Sf|` and `1/(n·d)` round differently from `1/|d|` (one ulp), and `k_f`
is ~1e-16 rather than 0. The open-channel test's own numbers are unchanged to
every printed digit (mass 2.34e-11, u_max/1.5U 0.99751, dp/dx/HP 0.99502).
The cavity, BDF2, scheme-selection and turbulence tests pass unchanged with
their pins (suite run recorded in the hand-off of this change).

### 4. Predictions against results

| prediction | result | verdict |
|---|---|---|
| dp/dz falls on all three grids | 1.0316→1.0175, 1.0317→1.0151, 1.0319→1.0136 | **held** |
| 5k lands at 1.012–1.018 | 1.0175 | held |
| 20k at 1.007–1.012, 50k at 1.004–1.008 | 1.0151, 1.0136 | **missed** (above) |
| the excess over the polygon value shrinks with refinement, below 0.7 % | 0.59 / 0.86 / 0.95 % at Re = 100 | **missed** at Re = 100; **held** at Re = 25, where the slab is developed: **0.08 %** |
| u_c moves by < 0.5 % | ≤ 0.03 % | held |
| mass unchanged to 1e-9 | ≤ 4e-10 | held |
| Cartesian cases move by round-off only (< 1e-10) | 2.5e-15 / 5.5e-14 | held |
| 1 corrector changes the steady answer by < 1e-4 | identical to 5 digits | held |

**What the misses mean.** The non-orthogonal part of the error is gone: with
the slab developed (Re = 25) the corrected dp/dz is 0.08 % from the equal-area
polygon value, against 1.7 % before. At Re = 100 the slab z ∈ [0.13, 0.17] m
(6.5–8.5 D) is only just past the development length (5.8 D, Durst et al.
2005), and the residual 0.6–0.9 % there is the still-developing flow, which
the finer grids resolve with less numerical diffusion (their L_99 is shorter:
5.51 / 5.40 / 5.36 D) — so the slab measures development, not the
Laplacian. I predicted from the stage 0 slab without checking that it was
developed; the stage 1 V&V (`laminar_pipe_gh1155.md`) therefore uses
L/D = 20 and a slab at 12.5–17.5 D.

### 5. A finding not predicted: the lagged correction slows or breaks the pseudo-transient solve

With upstream's default `nNonOrthogonalCorrectors = 0` the explicit
correction is lagged by one PISO corrector, and at Co ≈ 4 on this mesh that
lag makes the march to steady state **9× slower at 20k (301 steps against
34) and divergent at 50k**; at Re = 25, Co ≈ 1, 8× slower (368 against 47).
One non-orthogonal corrector restores the step count (34 at 20k, 76 at 50k)
and gives the same answer. The structure is upstream's, so OpenFOAM would be
expected to behave alike, but **no OpenFOAM run was made to confirm it**.
The default stays upstream's (0); `cases::laminar_pipe` users and the Foam
Studio demo set 1 corrector, and say so. Cost of the corrector and the
correction: at 20k, 34 steps took 7.9 s against 5.5 s for the ablation
(+43 %, noisy host).

### 6. Numbers this moved

- `tests/laminar_pipe_hagen_poiseuille.rs` (new) pins the 5k row: ablation
  within 1.5e-3 of 1.0316, corrected at least 5e-3 lower and within 1 % of
  the polygon value.
- The stage 0 README's "dp/dz +3.2 % above Hagen–Poiseuille" describes the
  pre-gh:#1161 solver; it is corrected in place there.
- Every Cartesian V&V number (cavity Ghia, icoFoam comparison, BDF2 order,
  open channel) moves by round-off only (section 3), so no pin moved.

## Update 2026-11-11 (gh:#1488): the orthogonal ablation after the boundary-`deltaCoeffs` fix

The orthogonal mode's boundary distance was `|Cf - C|`; upstream `fvPatch::delta()` is patch-normal (`|n . d|`, ESI v2506 `fvPatch.C:156`). With the fix, the 5k ablation row of table 1 is ~~dpdz/HP **1.03160**, ÷ polygon 1.01986, u_c/2U 0.9902~~ **1.03221, 1.02046, 0.9904** (re-run in release, `pipe_5k_non_orthogonal_correction --nocapture`; steps 19, flux 0.9999999999, cont 1.3e-11 against 1.4e-11 before). It moves 6e-4 AWAY from Hagen-Poiseuille. The corrected row is unchanged to the printed digits (1.01768, 1.00610; `cont` 5.7e-12), because the corrected path's boundary term already used the patch-normal distance. The other table-1 and Re = 25 ablation rows (20k, 50k) were not re-run; expect a similar shift (Not re-checked).
