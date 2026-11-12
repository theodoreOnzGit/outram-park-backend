# outram-foam-basic-lib vs upstream OpenFOAM, operator level (gh:#1362)

**Research, education and V&V only.** Not for reactor operation, licensing or
safety decisions (`RESPONSIBLE_USE.md`). AI-run; not yet reviewed by a human.

**Upstream:** OpenFOAM-v2506 (ESI), the partial build described in
`crates/outram-park-fork-cfmesh/verification_and_validation/upstream_cfmesh_gh1385/README.md`
(tag `OpenFOAM-v2506`, `615aae61d7e95e110ca842ca105c799d10f59178`,
GPL-3.0-or-later; nothing of it is committed here). The port follows the
Foundation line (OpenFOAM-dev); only ESI is runnable here, so a difference is
judged against both sources by reading, see "Findings".

## Method
`driver/opsDump.C` (our own file; it only calls upstream) reads a mesh made by
upstream `blockMesh` (3 blocks, skewed, graded, one collapsed block giving prisms:
316 cells, 33 deg max non-orthogonality) or by upstream `polyDualMesh 45` of it
(470 cells, 210 polyhedra, 60.5 deg max non-orthogonality), and prints hex bit
patterns of: points and faces; face centres, areas, cell centres, volumes;
weights, deltaCoeffs, nonOrthDeltaCoeffs; the inputs T (polynomial in x,y,z,
fixedValue on every patch), phi, gamma; and `fvc::grad` (Gauss linear), the
interpolation schemes linear / upwind / vanLeer / Minmod for flux +1 and -1,
`fvc::snGrad` orthogonal / uncorrected, `fvc::div(phi)`, `fvc::div(phi,T)`
(Gauss linear), `fvm::laplacian(gamma,T)` (Gauss linear uncorrected) and 9
limiter functions on a grid of r. Inputs are polynomials (no libm), and the
Rust test reads the inputs and upstream geometry back from the dump, so geometry
and operator differences are separable. `scripts/run.sh` regenerates the dumps.

## Predictions (written 2026-11-11 before the Rust side was run)
Read from upstream source (`primitiveMeshTools.C`, `surfaceInterpolation.C`,
`surfaceInterpolationScheme.C`, `gaussGrad.C`, `surfaceIntegrate.C`) and the port:
- G1 face areas: bit-identical (same `0.5*sumN`). Triangle face centres identical.
  Non-triangle face centres differ by last ulp on most faces: upstream is
  `(1/3)*sumAc/sumA` with `c = p + p_next + fCentre`; the port divides each
  triangle centre by 3 before the area weighting.
- G2 cell volumes and centres: ulp differences on most cells: upstream sums owner
  faces then neighbour faces and uses `*(1/3)`; the port interleaves by face and uses `/3`.
- W weights: bit-identical given identical geometry.
- D deltaCoeffs: bit-identical (1/|d|); nonOrthDeltaCoeffs: ulp differences on a
  fraction of faces (`sf*(1/area)` against `Sf/magSf`).
- I linear interpolation: ulp differences on a fraction of faces (`w*a+(1-w)*b`
  against upstream `w*(a-b)+b`).
- GR grad: ulp differences in most cells (consequence of I, and `g*(1/V)` against `g/V`).
- DV `div(phi)`: bit-identical (both divide by V). `div(phi,T)`: follows I.
- SN snGrad: ulp differences (`(b-a)/|d|` against `deltaCoeffs*(b-a)`).
- L laplacian: ulp differences (`gamma*magSf/delta` against `gamma*magSf*deltaCoeff`).
- LIM limiter functions: bit-identical (same +,-,*,/,abs,min,max expressions).

## Results (measured 2026-11-11, release; test `tests/operators_vs_openfoam.rs`)
Counts are bit-identical / total, worst ulp distance, on the blockMesh case
(316 cells) then the polyDualMesh case (470 cells); every number is pinned by
the test. Operators are run on the UPSTREAM geometry and inputs, so a row
isolates one operator. "proof" = the upstream arithmetic replayed in the test
from upstream inputs reproduces the upstream dump bit for bit, which proves the
stated cause.

| quantity | hex/prism | polyDual | cause (proven by) |
|---|---|---|---|
| Sf, magSf | all identical | all identical | same arithmetic |
| Cf, triangles | ~~58/90, 1 ulp~~ **90/90, 0 ulp** (gh:#1489, FIXED) | none | port `(a+b+c)/3` against upstream `(1/3)*(a+b+c)` (proof) |
| Cf, other faces | ~~1737/3159, 3 ulp~~ **3159/3159, 0 ulp** (gh:#1489, FIXED) | ~~2933/5364, 23 ulp~~ **5364/5364, 0 ulp** (gh:#1489, FIXED) | port divides each triangle centre by 3 before the area weighting; upstream `(1/3)*sumAc/sumA` (proof) |
| V | ~~42/316, 47 ulp~~ **316/316, 0 ulp** (gh:#1489, FIXED) | ~~70/470, 60 ulp~~ **470/470, 0 ulp** (gh:#1489, FIXED) | port interleaves faces and uses `/3`; upstream sums owner faces then neighbour faces and uses `*(1/3)` (proof) |
| C | ~~342/948, 4 ulp~~ **948/948, 0 ulp** (gh:#1489, FIXED) | ~~496/1410, 6 ulp~~ **1410/1410, 0 ulp** (gh:#1489, FIXED) | summation order, as V (proof) |
| linear weights | all identical | all identical | |
| deltaCoeffs | ~~**789/1083, rel 0.66**~~ **1083/1083, 0 ulp** (gh:#1488) | ~~**1236/1788, rel 0.88**~~ **1788/1788, 0 ulp** (gh:#1488) | **DEFECT D1**, below, FIXED 2026-11-11 (proof) |
| nonOrthDeltaCoeffs | ~~772/1083, 6 ulp~~ **1083/1083, 0 ulp** (gh:#1489, FIXED) | ~~1193/1788, 4 ulp~~ **1788/1788, 0 ulp** (gh:#1489, FIXED) | `sf*(1/area)` against upstream `Sf/magSf` (proof) |
| interpolate linear | ~~888/1083, 12 ulp~~ **1083/1083, 0 ulp** (gh:#1489, FIXED) | ~~1469/1788, 16 ulp~~ **1788/1788, 0 ulp** (gh:#1489, FIXED) | port `w*a+(1-w)*b`; upstream `w*(a-b)+b` (proof) |
| grad, Gauss linear | ~~506/948, 62 ulp~~ **948/948, 0 ulp** (gh:#1489, FIXED) | ~~696/1410, 103 ulp~~ **1410/1410, 0 ulp** (gh:#1489, FIXED) | interpolation form, plus port `g*(1/V)` against upstream `g/V` (proof: `g/V` on upstream face values is 948/948; `g*(1/V)` is 728/948) |
| snGrad, orthogonal | ~~**533/1083, rel 0.66**~~ ~~737/1083, 1 ulp~~ **1083/1083, 0 ulp** (gh:#1489, FIXED) | ~~**890/1788, rel 0.88**~~ ~~1285/1788, 1 ulp~~ **1788/1788, 0 ulp** (gh:#1489, FIXED) | boundary deltaCoeffs (D1, fixed), plus `/d` against `deltaCoeffs*` (proof; the remaining 1-ulp differences, gh:#1489) |
| snGrad, uncorrected (port: via `DeltaCoeff::NonOrthogonal`) | ~~627/1083, 3 ulp~~ **1083/1083, 0 ulp** (gh:#1489, FIXED) | ~~947/1788, 3 ulp~~ **1788/1788, 0 ulp** (gh:#1489, FIXED) | `nonOrthDeltaCoeffs` form, `/delta` against `*dC`. ~~replayed with `NonOrthogonal` on boundary faces~~ **CORRECTED gh:#1489**: upstream's non-coupled boundary `snGrad` is `pvf.snGrad()` with the PATCH's `deltaCoeffs` (`snGradScheme::snGrad`), so the replay uses the orthogonal boundary coefficient |
| div(phi) | all identical | all identical | both divide by V |
| div(phi,T), Gauss linear | ~~196/316, 34 ulp~~ **316/316, 0 ulp** (gh:#1489, FIXED) | ~~297/470, 49 ulp~~ **470/470, 0 ulp** (gh:#1489, FIXED) | interpolation form |
| reconstruct upwind (+/-) | ~~1546/1578, 32 ulp~~ **1578/1578, 0 ulp** (gh:#1489, FIXED) | ~~2407/2472, 8 ulp~~ **2472/2472, 0 ulp** (gh:#1489, FIXED) | upstream `upwind` weights are 1 or 0 applied as `1*(P-N)+N`, which is not `P` in floating point; the port returns `P` (proof) |
| reconstruct linear / vanLeer / Minmod | ~~1199, 813, 1010 of 1578; 16, 192, 32 ulp~~ **1578 of 1578, 0 ulp, all three** (gh:#1489, FIXED) | ~~1858, 1322, 1537 of 2472; 16, 1024, 80 ulp~~ **2472 of 2472, 0 ulp, all three** (gh:#1489, FIXED) | different structure from upstream's `limitedScheme` (`P + lambda*(lin-P)` against `w*(P-N)+N` with `w = lambda*cdWeight + (1-lambda)*upwind`), and `r` amplifies gradient ulps; ~~not replayed~~ the port now follows `LimitedScheme`/`NVDTVD::r`/`limitedSurfaceInterpolationScheme::weights` operation for operation (incl. the `1000` clamp), which made them identical |
| laplacian upper / diag / source (uncorrected) | ~~466/789, 224/316, 197/316; 3 ulp~~ **789/789, 316/316, 316/316; 0 ulp** (gh:#1489, FIXED) | ~~740/1236, 312/470, 311/470; 8 ulp~~ **1236/1236, 470/470, 470/470; 0 ulp** (gh:#1489, FIXED) | `gamma*magSf/delta` against `dC*(gamma*magSf)` (proof for upper); a fixedValue source is `gm*(dC*v)`; the non-coupled boundary uses the patch `deltaCoeffs`, not the scheme's (`fvmLaplacianUncorrected`) |
| limiters vanLeer, Minmod, SuperBee, MUSCL, UMIST, limitedLinear(1, 0.5) | all identical (314) | same | |
| limiter vanAlbada | r>=0 232/232; ~~**r<0 1/82**~~ **r<0 82/82, 0 ulp** (gh:#1362, FIXED) | same | **D3**: upstream has no `max(0,r)` clamp (proof); clamp removed |
| limiter OSPRE | r>=0 ~~151/232, 2 ulp~~ **232/232, 0 ulp** (gh:#1489, `rrp1 = r(r+1)`); ~~**r<0 1/82**~~ **r<0 82/82, 0 ulp** (gh:#1362, FIXED) | same | operation order, and the same clamp (proof); clamp removed |

## Findings
Nothing below was changed in solver code: each fix would move solver results,
so each is a maintainer decision.
- **D1. Boundary `deltaCoeffs` in `DeltaCoeff::Orthogonal` mode (the default of `fvm::laplacian` and `fvc::sn_grad`).** Upstream `fvPatch::delta()` returns the patch-NORMAL component `n (n . d)` (ESI v2506 `fvPatch.C:156`), so `deltaCoeffs = 1/|n . d|` on boundary faces. The port uses `1/|Cf - C|`, so on a non-orthogonal boundary face the coefficient is too small by `cos(theta)`; here by up to 66 % (blockMesh) and 88 % (polyDual). The `NonOrthogonal` mode already does it. Predicted movement of the fix: every non-orthogonal boundary face of every case that uses the default mode; zero on orthogonal meshes. ~~Not fixed: the pinned appbuilder results would need re-measuring.~~ **FIXED 2026-11-11 (gh:#1488)**: `fvm::DeltaCoeff::Orthogonal.boundary_delta` now computes `|nHat (nHat . d)|` with upstream's operation order (`patch_normal_delta` in `src/fv_operators/fvm/laplacian.rs`); the boundary `fixedGradient` / `mixed` face reconstruction (`fvc::interpolate`), `fvc::sn_grad`, `fvm::laplacian_vec` and the boundary part of `laplacian_corrected` use it too. Boundary `deltaCoeffs` is 1083/1083 and 1788/1788 bit-identical, `snGrad (orthogonal)` is within 1 ulp (2.2e-16 relative). Not touched: coupled (cyclic / AMI) seams, which have their own `delta()` upstream.
- **D2. Arithmetic order (ulp level), all proven above:** cell volume and centre and face centre order, `*(1/x)` in place of `/x` (grad, `nonOrthDeltaCoeffs`), linear interpolation form, upwind. Bit-identity needs each changed to upstream's form; each change moves every downstream result by ulps, so none was made. Predicted movement: relative 1e-15 on fields, up to 1e-13 where a limiter ratio `r` amplifies it. ~~Bit-identity needs each changed to upstream's form; each change moves every downstream result by ulps, so none was made.~~ **FIXED 2026-11-11 (gh:#1489)**: every ordering above was changed to upstream's form; see the gh:#1489 results section below.
- **D3. vanAlbada and OSPRE clamp `r` at 0; upstream does not.** Their doc comments claimed the clamp was upstream's; corrected in `src/limiters.rs` (documentation only). ~~Not fixed~~ **FIXED 2026-11-11 (gh:#1362)**: both clamps and the trailing `max(0)` of `FluxLimiter::psi` removed; 82/82 identical for r<0 on both meshes, as predicted (section "Task 1"). No workspace consumer selects these two variants, so no solver number moved.
- **Which line.** The source read and run is ESI v2506. The Foundation (dev) text of `makeFaceCentresAndAreas`, `fvPatch::delta` and `surfaceInterpolationScheme::interpolate` was not run or read here, so D2's orderings are stated for ESI only and may differ in dev.

## Not covered (open items on gh:#1362)
Thermophysics and turbulence (need `libthermophysicalModels` and `libmomentumTransportModels`, not in the partial build; not attempted); `linearUpwind` and `limitedLinear` as full schemes (the port has neither; only the limiter functions are compared); `leastSquares` and `cellLimited` gradients; `snGrad corrected` and limited; an AMI pair; linear solvers (case 3); ODE (case 6).

## gh:#1488 (D1 fix): prediction, written 2026-11-11 BEFORE the code change
The baseline (all pinned and V&V numbers, current code) was recorded first; this
prediction was committed before the fix.
- **Differential test.** After the fix, boundary `deltaCoeffs` should be bit-identical on both meshes (1083/1083 and 1788/1788): `1/|nHat (nHat . d)|` with `nHat = Sf/magSf` is the upstream arithmetic. `snGrad (orthogonal)` still differs on interior faces by the `/d` against `deltaCoeffs*` ordering, so its identical count should rise from 533 and 890 to about the interior-face level, with a worst error of a few ulp instead of 66 % / 88 % relative.
- **Solver numbers.** Only boundary faces whose normal is not parallel to `Cf - C` move, and only in `DeltaCoeff::Orthogonal` mode (the default). Cases with orthogonal wall cells (Cartesian blocks, the 1-D and RZ structured pipes) should not move beyond an ulp (`|n . d|` against `|d|`). Skewed or polyhedral boundary cells should move: the wall conductance `Gamma |Sf| / |n . d|` gets larger (`|n . d| <= |d|`), so wall flux and wall shear grow in magnitude for the same wall value. Foam Studio's unstructured (polyDual) pipe, pinned at 0.9394 / 1.0062, is the likeliest to move, by up to a few percent; the sign relative to the analytic Hagen-Poiseuille value is not known in advance. GeN-Foam V&V and the CHT double pipe (structured, near-orthogonal) should move by under 1e-6 relative.

### gh:#1488 results (2026-11-11, release) against the prediction above
- **Differential: as predicted.** Boundary `deltaCoeffs`: hex ~~789/1083, rel 0.66~~ **1083/1083 bit-identical**; polyDual ~~1236/1788, rel 0.88~~ **1788/1788**. `snGrad (orthogonal)`: ~~533/1083~~ **737/1083, worst 1 ulp** (hex), ~~890/1788~~ **1285/1788, 1 ulp** (poly), remainder `/d` against `deltaCoeffs*` (gh:#1489). Every other row unchanged.
- **Solver numbers: the prediction was too pessimistic.** Whole suites re-run against the recorded baseline (basic-lib 566 tests, appbuilder 477, cli 29, Foam Studio 19, digital twin `double_pipe` 22, all pass). Only ONE printed number moved beyond round-off: the laminar pipe's orthogonal ablation, ~~dpdz/HP 1.03160~~ **1.03221** (see `crates/outram-foam-appbuilder-lib/verification_and_validation/non_orthogonal_correction_gh1161.md`), 6e-4 further from Hagen-Poiseuille. Foam Studio's unstructured default (0.9393 / 1.0063 printed, pins 0.9394 / 1.0062 within 2e-3), `headless_pipe_reaches_steady_hagen_poiseuille` (0.9753 / 1.0177), the GeN-Foam V&V tests and the CHT / Graetz-Nusselt / pimple / energy tests print identical numbers (round-off changes only, e.g. an energy-balance residual 1.81e-13 to 1.76e-13). I predicted Foam Studio's unstructured pipe would move; it does not, because its default solver uses the `NonOrthogonal` mode, whose boundary distance was already patch-normal, so the fix cannot reach it (the 0.9394 / 1.0062 pins did not move; not re-pinned).

## gh:#1489 (D2 fix): prediction, written 2026-11-11 BEFORE the code change
Upstream read (ESI v2506): `primitiveMeshTools::makeFaceCentresAndAreas` /
`makeCellCentresAndVols`, `basicFvGeometryScheme::nonOrthDeltaCoeffs`,
`surfaceInterpolationScheme::interpolate`, `gaussGrad::gradf`,
`gaussLaplacianScheme::fvmLaplacianUncorrected`, `fixedValueFvPatchField::
gradientBoundaryCoeffs`, `LimitedScheme::calcLimiter`, `NVDTVD::r`,
`limitedSurfaceInterpolationScheme::weights`. The "proof" rows already replay each
one bit for bit from upstream inputs, so the plan is to make the port's arithmetic
the replayed arithmetic. Foundation (dev) text not re-checked (as above).

**Changes planned (each follows its proof row):** polygon face centre `(1/3)*sumAc/sumA`,
triangle `(1/3)*(a+b+c)`, zero-area test `sumA < ROOTVSMALL` (1e-150) in place of
`EPSILON`; cell centres/volumes summed owner faces then neighbour faces, `*(1/3)`,
`VSMALL` (1e-300) test; `nonOrthDeltaCoeffs` unit normal `Sf/magSf`, boundary floor
`0.05*mag(delta)`; linear interpolation `w*(P-N)+N`; Gauss gradient `g/V`;
`snGrad` `deltaCoeffs*(N-P)`; Laplacian `dC*(gamma*magSf)` and, for a fixedValue
face, source `gm*(dC*v)`; limited-scheme reconstruction as upstream's
`w = lim*cdWeight + (1-lim)*pos0(flux)` then `w*(P-N)+N` (upwind is `1*(P-N)+N`),
with `NVDTVD::r` including its `1000` clamp (a missing LIMIT in the port, which has no
clamp and a `|dphi| < 1e-30` shortcut instead); OSPRE `rrp1 = r(r+1); 1.5 rrp1/(rrp1+1)`.

**Predicted differential result, both meshes** (n/n bit-identical, 0 ulp):
Cf (triangles and others), V, C, nonOrthDeltaCoeffs, interpolate linear, grad, snGrad
(orthogonal), snGrad (uncorrected, replayed with the stored coefficient),
div(phi,T), recon upwind / linear / vanLeer / Minmod, laplacian upper / diag / source,
limiter OSPRE [r>=0]. Already identical rows stay identical. **Remaining, and why:**
limiter vanAlbada [r<0] and OSPRE [r<0] (the port clamps `r` at 0, upstream does not;
that is a semantic difference, not operation order, outside gh:#1489) and the
boundary `Mixed` / `FixedGradient` Laplacian arms, which the differential does not
reach (fixedValue only). The recon rows are the least certain: they depend on the
gradient (now identical) and on `r` through a division, so one surviving ulp
there would mean a second ordering difference I have not found.

**Predicted solver movement:** ulp-level (relative 1e-15, up to 1e-13 where a
limiter ratio amplifies) on every field; printed digits of V&V numbers should not
move; any test that pins a bit pattern or an exact value to more than 12 digits
will move. The `1000` clamp can change a limited-scheme face value only where
`|d.grad| >= 1000 |dphi|`, so reconstruct-based solvers (rhoCentralFoam, HEM,
pcice) move beyond round-off only on near-discontinuous data; I expect none of
the pinned tolerances to be crossed.

### gh:#1489 results (2026-11-11, release) against the prediction above
- **Differential: as predicted, every row.** All 34 listed rows (18 hex, 17 poly, counting
  OSPRE r>=0 on both) moved to n/n bit-identical, 0 ulp; the table above has old
  struck through. Remaining differences: limiter vanAlbada [r<0] and OSPRE [r<0] (the
  `max(0, r)` clamp, not operation order, kept as documented), and the `proof ... g*(1/V)
  (port form)` row, which is the retired port arithmetic kept as a control.
- **One thing the prediction missed, found by the differential.** The first run left
  `laplacian diag` / `source` at 286/316 and 245/316 (hex). The cause was not
  an ordering: upstream's non-coupled branch of `fvmLaplacianUncorrected` calls
  `pvf.gradientInternalCoeffs()` with no argument, so a boundary face uses the patch's own
  `deltaCoeffs` (orthogonal, `1/mag(delta)`), not the scheme's `nonOrthDeltaCoeffs`; the same
  holds for `snGradScheme::snGrad`, whose non-coupled boundary is `pvf.snGrad()`. The port
  used the scheme's coefficient on boundary faces (an ulp apart), and my replay of
  "snGrad (uncorrected)" copied that. Both now use the patch coefficient
  (`laplacian_with_delta`, the replay in the test).
- **Missing limit found, as the porting rule predicts.** `NVDTVD::r` clamps
  `|gradcf| >= 1000|gradf|`; the port had no clamp and a `|dphi| < 1e-30` shortcut. Now
  upstream's. It changes a face value only where the slope ratio exceeds 1000.
- **Solver numbers: nothing moved beyond round-off.** Re-run, release, all pass:
  basic-lib lib+tests (546 + integration) and doc tests (119), turbulence 16, multiphase 67,
  outram-foam-mesh, appbuilder, cli, moltres, offbeat, farrer-park, tampines (222 + others),
  dhoby-ghaut, Foam Studio example (19), digital twin `double_pipe` (22). Printed V&V numbers
  equal their gh:#1488 values to the digits printed: 5k ablation dpdz/HP 1.03221 (poly 1.02046),
  uc15 0.9904, uc17 0.9926, uc_dev 0.9902; Foam Studio 0.9393 / 1.0063 and 0.9753 / 1.0177.
  Only the `operators_vs_openfoam` pins moved (old struck through in `PINS`).
- **Not changed (no upstream replay row, so not claimed):** `fvc::div_vec` /
  `div_tensor` still multiply by `1/V` (upstream `surfaceIntegrate` divides); boundary `Mixed`
  and `MixedField` arms of the Laplacian and `snGrad` keep their algebra; coupled
  (cyclic / AMI) seams; the `fvc::interpolate` boundary `Mixed` / `FixedGradient`
  reconstructions. `outram-foam-mesh`'s own geometry code (blockMesh / dual) was not touched.
  Foundation (dev) arithmetic not re-read.

## Task 1 (gh:#1362 D3): vanAlbada / OSPRE for r < 0, prediction written 2026-11-11 BEFORE the code change
Upstream read (v2506): `vanAlbada.H` returns `r*(r + 1)/(sqr(r) + 1)`, `OSPRE.H` returns
`rrp1 = r*(r + 1); 1.5*rrp1/(rrp1 + 1)`; neither clamps `r`, and `LimitedScheme::calcLimiter` /
`NVDTVD.H` apply no clip afterwards. The port's `FluxLimiter::psi` clamps `r` at 0 inside these two
arms and ends with `val.max(0.0)`. Plan: delete both clamps (the final `max` is redundant for the other
limiters, whose formulas are already `>= 0`). **Predicted:** `limiter vanAlbada [r<0]` and
`limiter OSPRE [r<0]` go from 1/82 to 82/82, 0 ulp, on both meshes; every other row unchanged. Only
consumers of `FluxLimiter::{VanAlbada, Ospre}` move; a grep of the workspace finds none outside
`limiters.rs` and this test (pflotran and tampines use other variants), so no solver number should move.

## Task 2/3 (gh:#1362 / gh:#1489 follow-up): new rows, prediction written 2026-11-11 BEFORE the Rust side was run
**Harness added** (`driver/opsDump.C`, `inputs/fvSchemes`; the old dump keys are byte-identical after
the rebuild, 0 lines removed from either reference file): `linearUpwind grad(T)`,
`limitedLinear 1` and `0.5` through `surfaceInterpolationScheme::New` (flux +1 / -1); `limitedLinear`
with k = 1e-16 on a grid of tiny r; `fvc::grad(T, "grad(Tls)")` with `leastSquares`; `mesh.nonOrthCorrectionVectors()`;
`snGradScheme` `corrected`, `limited corrected 0.33`, `limited corrected 0.5` (full `snGrad` and
`correction`); `fvm::laplacian` with `Gauss linear corrected` and `Gauss linear limited corrected 0.33`
(diag, upper, source, patch coefficients, `faceFluxCorrection`); `fvc::div(phi, U)` (vector) and `fvc::div(S)`
(tensor, `Sf & S_f`) with polynomial fixedValue inputs; a second scalar field `Tm` with an inlet
`fixedGradient` (uniform 0.7), an outlet `mixed` (per-face refValue/refGrad/valueFraction) and walls `mixed`
(uniform), dumping its boundary values, Gauss gradient, linear face values, uncorrected `snGrad` and the
uncorrected Laplacian matrix. Read upstream (ESI v2506): `leastSquaresVectors.C`, `leastSquaresGrad.C`,
`SymmTensorI.H` (`safeInv`), `correctedSnGrad.C`, `limitedSnGrad.C`, `basicFvGeometryScheme.C`
(`nonOrthCorrectionVectors`), `linearUpwind.C`, `limitedLinear.H`, `mixedFvPatchField.C`,
`gaussLaplacianScheme.C`, `gaussDivScheme.C`, `surfaceIntegrate.C`.

**Predicted agreement, hex and polyDual alike, port as it stands:**
- `linearUpwind`: the port has no such scheme (it is a new function), nothing to compare. With the
  function written in upstream order (`(1*(P-N)+N) + (Cf - C_upwind) & grad[C_upwind]`, gradient the
  Gauss-linear `fvc::grad`) I predict **bit-identical**, every face, both signs.
- `limitedLinear 1`, `0.5` reconstruction: absent as `Limiter` variants. With a `LimitedLinear(k)` variant
  evaluating `max(min(twoByk*r, 1), 0)`, `twoByk = 2/max(k, small)`, through the existing `NVDTVD::r`
  path: **bit-identical**. `limiter limitedLinear k=1e-16`: the port floors `k` at `1e-30`, upstream at
  `small` = 1e-15 (v2506 `doubleScalarSmall`); on the tiny-r grid I predict **5/7 identical, 2 differ**
  (r = 2.2e-16, 4.4e-16: upstream 0.44 and 0.89, port saturates at 1), then 7/7 after the floor is `1e-15`.
- `leastSquares` gradient: the port solves the per-cell normal equations by Gaussian elimination with weight
  `1/|d|^2` and `|Cf - C|` at boundaries; upstream uses `magSf/magSqr(d)` weights split `(1-w)`, `w`, the
  patch-normal `delta` at boundaries, `safeInv` (adjunct/det) and per-face vectors. Same linear-exact
  answer, different arithmetic: I predict **most cells differ, relative 1e-16..1e-13** (not bit-identical);
  after translating upstream exactly, **bit-identical**.
- `nonOrthCorrectionVectors`: the port's `non_ortho_geometry` forms `n = sf*(1/magSf)`, upstream `Sf/magSf`:
  **differ on a fraction of internal faces by an ulp or two**; boundary vectors are zero upstream and
  not computed by the port's `fvc` (it applies a boundary correction instead, below).
- `snGrad corrected`: no `fvc` function in the port (a new function). Written in upstream order:
  `nonOrthDeltaCoeffs*(N-P)` + `nonOrthCorrectionVectors & (w*(g_P-g_N)+g_N)`, boundary = patch
  `snGrad` and zero correction: **bit-identical**. `limited corrected 0.33/0.5`: the port's
  `NonOrthoScheme::Limited(w)` multiplies the correction by `w`; upstream's factor is
  `min(w*|snGrad|/((1-w)*|corr| + SMALL), 1)`. These are different schemes: predicted **all faces with a
  non-zero correction differ at O(1) relative** (not an ulp), until the upstream limiter is implemented.
- Laplacian `corrected` / `limited`: the port's `laplacian_corrected` (a) takes the face gradient as the
  arithmetic mean of the two cell gradients (upstream: `w*(P-N)+N`, weights not 0.5), (b) adds the
  explicit term to `source` per face, not as `V*div(faceFluxCorrection)`, (c) switches the implicit
  coefficient to `gamma*magSf*deltaCoeff` as `delta = coeff_corr - coeff_orth` increments, (d) corrects
  non-coupled Dirichlet boundary faces, where upstream's boundary correction vector is zero. Predicted:
  **upper differs on every non-orthogonal face (ulps), diag and source on nearly every cell; the explicit
  part differs at O(1e-2..1e-1) relative where the gradient varies across a face (the 0.5 mean against the
  true weight), and the boundary cells differ by (d)**.
- `fvc::div(phi, U)` (vector) and `fvc::div(S)` (tensor): the port multiplies by `1/V`, upstream divides:
  **about a quarter to a third of the components differ by 1 ulp**; scalar `div` rows (already `/V`) stay
  identical. Face interpolation of vectors/tensors uses the generic `w*(P-N)+N` and should add nothing.
- `Tm` boundary values: fixedGradient `P + g*delta` (port) against `P + g/deltaCoeffs` (upstream): **ulp
  differences on a fraction of faces**; mixed `ref*w + (P + g*delta)*(1-w)` against
  `w*ref + (1-w)*(P + g/deltaCoeffs)`: **ulp differences on a fraction**. `snGrad` mixed: port
  `w*(dc*(ref-P)) + (1-w)*g` against upstream `((w*(ref-P))*dC) + (1-w)*g`: **ulp differences**. Laplacian
  mixed: port `w*coeff`, `w*coeff*ref + (1-w)*coeff*d*g` against upstream `gm*((-w)*dC)` and
  `-(gm*(((w*dC)*ref) + (1-w)*g))`: **ulp differences**. fixedGradient Laplacian `gm*g`: **identical**.

### Task 1-3 results (2026-11-11, release) against the predictions above
Counts are bit-identical / total, worst ulp, hex then polyDual; "before" is the port as it stood
(measured by the new rows before any fix), "after" is pinned in `PINS`. 58 new pinned rows, every
one n/n bit-identical, 0 ulp, on both meshes; nothing existing moved.

| row | before | predicted | after |
|---|---|---|---|
| limiter vanAlbada / OSPRE r<0 | 1/82 | 82/82 | 82/82 |
| `limitedLinear` 1, 0.5 reconstruction (+/- flux) | absent | identical | 1578/1578, 2472/2472 |
| limiter limitedLinear k=1e-16, tiny r | 5/7 | 5/7 | 7/7 (floor `1e-30` -> `small` = 1e-15) |
| `linearUpwind` +/- flux | absent | identical | 789/789, 1236/1236 |
| `grad leastSquares` | 0/948, 0/1410 (rel up to 15) | most differ | 948/948, 1410/1410 |
| `nonOrthCorrVec` (internal) | 1699/2367, 2639/3708 | fraction differ | 2367/2367, 3708/3708 |
| `snGrad corrected`, `limited 0.33`, `limited 0.5` (+ correction) | absent | identical | all n/n |
| `laplacian corrected` / `limited 0.33`: upper, diag, source | 462/789, 148/316, 0/316 (source rel up to 10) | differ | all n/n; `faceFluxCorrection` n/n |
| `fvc::div(phi, U)` vector | 737/948, 1076/1410 | 1/4-1/3 differ | all n/n |
| `fvc::div(S)` tensor | 730/948, 1055/1410 | same | all n/n |
| `Tm` interpolate (fixedGradient + mixed) | 1079/1083, 1782/1788 | differ | all n/n |
| `Tm` Gauss gradient | 942/948, 1405/1410 (14 ulp) | follows interpolate | all n/n |
| `Tm` laplacian mixed diag / source | 309/316, 208/316; 440/470, 329/470 | differ | all n/n; upper unchanged |
| `Tm` snGrad (boundary faces) | 294/294, 552/552 | **I predicted a difference** | unchanged: the port already had v2506's `lerp` form |

**What the prediction got wrong.** (1) `snGrad (mixed)`: predicted to differ, was already identical
(the port's `w*(dC*(ref-P))` equals v2506's `lerp(g, (ref-P)*dC, w)` term). (2) The leastSquares
gradient is **not exact for a linear field at non-orthogonal boundary cells in upstream**: its
boundary stencil uses the patch-normal `fvPatch::delta()`, not `Cf - C`. On the `amp = 0.5` wavy mesh
the max error is **3.067e-1** (~~1.823e-14~~ for the previous port-original operator, which is kept as
`grad_least_squares_face_centre`). Likewise upstream's corrected Laplacian does not correct
non-coupled boundary faces; at `amp = 0.3` (10.5 deg) it reaches 1.264e-3 where the port-original
operator, which does, reaches ~7e-12 (`tests/non_orthogonal_laplacian.rs`, still green, driven by the
`_face_centre` pair).

**Source forms (ESI v2506, differ from Foundation dev).** `mixedFvPatchField` in v2506 writes
`snGrad` and `gradientBoundaryCoeffs` with `lerp(a, b, t) = (1-t)*a + t*b`; Foundation dev writes
`w*(ref-P)*dC + (1-w)*g` and `w*dC*ref + (1-w)*g`, a different rounding. The port now follows v2506
(Laplacian mixed arms); dev was read for these functions only.

**Changes:** `fvc/div.rs`, `fvc/div_tensor.rs` (`/V`), `fvc/interpolate.rs` (fixedGradient/mixed `g/dC`;
generic bound gained `Div<f64>`), `fvm/laplacian.rs` (mixed arms; `laplacian_with_delta_and_source`,
`patch_delta`), `fvc/grad_least_squares.rs` (upstream translation; old operator renamed
`..._face_centre`), `fvm/laplacian_corrected.rs` (upstream translation; old renamed `..._face_centre`;
`non_ortho_geometry` is UNCHANGED, see below), `fvc/sn_grad.rs` (`sn_grad_with_delta`), new
`fvc/sn_grad_corrected.rs`, `fvc/linear_upwind.rs`, `limiters.rs` (`small`), `fvc/muscl.rs`
(`Limiter::LimitedLinear`; `Eq` derive dropped).

**Not covered, stated:** `limitedLinearV` (no vector reconstruction in the port); AMI and cyclic
(coupled) seams for every row; Mixed arms of `laplacian_vec` and `snGrad` for
`FixedFluxPressure`/`TotalPressure`; `cellLimited` gradients; `skewCorrected` snGrad;
turbulence, thermo and linear-solver rows (separate work).

**A change that was made and reverted (gh:#1362).** `fvm::non_ortho_geometry` (used by
`outram-foam-appbuilder-lib`'s own non-orthogonal solver and by `laplacian_corrected_face_centre`)
first took upstream's `Sf/magSf` unit normal. Re-running the downstream suites, Foam Studio's
`headless_coil_shows_dean_flow` then failed: the steady coil's out/in flux ratio moved from
~~0.999999996~~ to **0.999999967** (iteration 81 both times, residual 4.1e-10 -> 4.7e-10), past that
test's 1e-8 gate. Cause: an ulp-level change in the correction vector, amplified by a stopping rule
that is residual-based; confirmed by reverting only that line (0.999999996 again). The gate was not
relaxed and nothing was retuned: `non_ortho_geometry` keeps `Sf*(1/magSf)` (documented in its source),
the differential row `nonOrthCorrVec (port-original non_ortho_geometry)` pins the 668 / 1069 faces
(hex / polyDual) that differ from upstream by an ulp or two, and the upstream-form vectors are
`fvc::non_orth_correction_vectors` (2367/2367, 3708/3708 identical). Left for the maintainer: whether
appbuilder's non-orthogonal solver should move to the upstream form and the Foam Studio coil gate be
re-baselined.

## Maintainer decision 2026-11-12 (gh:#1336): keep both operator pairs, and which is which

The maintainer decided to **keep both** pairs for a later study. Neither is removed; the choice
between them in each pair is **left open**. The `///` docs of all four functions now carry the
statement below (none of the four has a `kind = "review"` stamp: `fvc/review.md` and `fvm/review.md`
hold only `kind = "note"` entries, and no `src` file under the crate holds a review block, checked 2026-11-12).

| Pair | Upstream translation (the default, bit-identical to OpenFOAM) | Port-original (more accurate at non-orthogonal boundary cells) |
|---|---|---|
| (a) cell gradient | `fvc::grad_least_squares` (ESI v2506 `leastSquaresGrad`, row `grad leastSquares`) | `fvc::grad_least_squares_face_centre` |
| (b) corrected Laplacian | `fvm::laplacian_corrected` (ESI v2506 `gaussLaplacianScheme` `corrected` / `limited corrected`) | `fvm::laplacian_corrected_face_centre` |

**Why the port-original forms are more accurate.** They are exact for a linear field at
non-orthogonal boundary cells. Upstream's are not, for two reasons that are faithful OpenFOAM
behaviour and not port defects: (a) upstream's leastSquares boundary stencil uses the patch-normal
`fvPatch::delta()` rather than `Cf - C_P`; (b) upstream's corrected Laplacian carries no correction
on non-coupled boundary faces (its correction vectors there are zero).

| Quantity | Upstream form | Port-original form | Source |
|---|---|---|---|
| (a) max gradient error, linear field, `amp = 0.5` wavy mesh (17.50 deg) | **3.067e-1** | **1.823e-14** | this README ("What the prediction got wrong"); `tests/non_orthogonal_laplacian.rs`, `least_squares_gradient_is_exact_...` |
| (b) rel. L-inf solution error, `amp = 0.3` (10.55 deg) | **1.264e-3** | **6.981e-12** (recorded table; "~7e-12") | this README; `tests/non_orthogonal_laplacian.rs` table |

Re-read 2026-11-12: the 3.067e-1 and 1.264e-3 figures are the ones recorded in "What the prediction
got wrong" above (measured 2026-11-11); the 1.823e-14 and 6.981e-12 figures are the recorded table
in the doc comment of `tests/non_orthogonal_laplacian.rs` (measured 2026-08-07, before the gh:#1362
arithmetic changes; the test is still green, driven by the `_face_centre` pair). Not re-run in this
change: the doc-only edit does not move them.

`fvm::solve_laplacian_non_orthogonal` and `tests/non_orthogonal_laplacian.rs` stay on the
`_face_centre` pair. Two doc claims the code contradicted were corrected in the same change:
`grad_least_squares`'s "exact for any linear field on any mesh" (it described the port-original
operator) and `laplacian_corrected`'s "arithmetic mean of the two cell gradients ... owner's gradient
on a boundary face" (it described the face-centre operator).

## gh:#1362 `non_ortho_geometry` -> upstream `Sf/magSf` (maintainer approval 2026-11-12): prediction, written BEFORE the code change

Change: `fvm::non_ortho_geometry` takes upstream's unit normal `Sf/magSf` (componentwise divide) in
place of `Sf*(1/magSf)`; the previous form stays callable as `fvm::non_ortho_geometry_port_original`.

Predictions:

1. **Differential row.** `nonOrthCorrVec` computed through `fvm::non_ortho_geometry` (pinned at
   hex 668 / 2367 and polyDual 1069 / 3708 faces differing from upstream, by an ulp or two) becomes
   **bit-identical: hex 2367 / 2367, polyDual 3708 / 3708, 0 ulp**. Reason: the remaining arithmetic
   (`n.d`, `1/max(n.d, 0.05|d|)`, `n - d*delta_coeff`) is already upstream's operation order, and
   `fvc::non_orth_correction_vectors`, which already matches, differs only by the unit-normal form.
   The `non_ortho_geometry_port_original` row keeps the old pins (1699 / 2367 and 2639 / 3708 identical).
2. **Foam Studio `headless_coil_shows_dean_flow`.** out/in moves from 0.999999996 to about 0.999999967
   (the value the reverted attempt in commit 65994af06 measured; iteration 81, residual 4.1e-10 -> 4.7e-10)
   and fails its 1e-8 gate, which the maintainer approved re-baselining.
3. **Everything else** that depends on `non_ortho_geometry` (appbuilder's non-orthogonal solver,
   `laplacian_corrected_face_centre`, `max_non_orthogonality_deg`) moves at the ulp level only. I expect
   no other test to cross a gate; any that does is reported, not loosened.
