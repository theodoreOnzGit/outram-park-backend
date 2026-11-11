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
| Cf, triangles | 58/90, 1 ulp | none | port `(a+b+c)/3` against upstream `(1/3)*(a+b+c)` (proof) |
| Cf, other faces | 1737/3159, 3 ulp | 2933/5364, 23 ulp | port divides each triangle centre by 3 before the area weighting; upstream `(1/3)*sumAc/sumA` (proof) |
| V | 42/316, 47 ulp | 70/470, 60 ulp | port interleaves faces and uses `/3`; upstream sums owner faces then neighbour faces and uses `*(1/3)` (proof) |
| C | 342/948, 4 ulp | 496/1410, 6 ulp | summation order, as V (proof) |
| linear weights | all identical | all identical | |
| deltaCoeffs | **789/1083, rel 0.66** | **1236/1788, rel 0.88** | **DEFECT D1**, below (proof) |
| nonOrthDeltaCoeffs | 772/1083, 6 ulp | 1193/1788, 4 ulp | `sf*(1/area)` against upstream `Sf/magSf` (proof) |
| interpolate linear | 888/1083, 12 ulp | 1469/1788, 16 ulp | port `w*a+(1-w)*b`; upstream `w*(a-b)+b` (proof) |
| grad, Gauss linear | 506/948, 62 ulp | 696/1410, 103 ulp | interpolation form, plus port `g*(1/V)` against upstream `g/V` (proof: `g/V` on upstream face values is 948/948; `g*(1/V)` is 728/948) |
| snGrad, orthogonal | **533/1083, rel 0.66** | **890/1788, rel 0.88** | boundary deltaCoeffs (D1), plus `/d` against `deltaCoeffs*` (proof) |
| snGrad, uncorrected (port: via `DeltaCoeff::NonOrthogonal`) | 627/1083, 3 ulp | 947/1788, 3 ulp | `nonOrthDeltaCoeffs` form, `/delta` against `*dC` |
| div(phi) | all identical | all identical | both divide by V |
| div(phi,T), Gauss linear | 196/316, 34 ulp | 297/470, 49 ulp | interpolation form |
| reconstruct upwind (+/-) | 1546/1578, 32 ulp | 2407/2472, 8 ulp | upstream `upwind` weights are 1 or 0 applied as `1*(P-N)+N`, which is not `P` in floating point; the port returns `P` (proof) |
| reconstruct linear / vanLeer / Minmod | 1199, 813, 1010 of 1578; 16, 192, 32 ulp | 1858, 1322, 1537 of 2472; 16, 1024, 80 ulp | different structure from upstream's `limitedScheme` (`P + lambda*(lin-P)` against `w*(P-N)+N` with `w = lambda*cdWeight + (1-lambda)*upwind`), and `r` amplifies gradient ulps; not replayed |
| laplacian upper / diag / source (uncorrected) | 466/789, 224/316, 197/316; 3 ulp | 740/1236, 312/470, 311/470; 8 ulp | `gamma*magSf/delta` against `dC*(gamma*magSf)` (proof for upper) |
| limiters vanLeer, Minmod, SuperBee, MUSCL, UMIST, limitedLinear(1, 0.5) | all identical (314) | same | |
| limiter vanAlbada | r>=0 232/232; **r<0 1/82** | same | **D3**: upstream has no `max(0,r)` clamp (proof) |
| limiter OSPRE | r>=0 151/232, 2 ulp; **r<0 1/82** | same | operation order, and the same clamp (proof) |

## Findings
Nothing below was changed in solver code: each fix would move solver results,
so each is a maintainer decision.
- **D1. Boundary `deltaCoeffs` in `DeltaCoeff::Orthogonal` mode (the default of `fvm::laplacian` and `fvc::sn_grad`).** Upstream `fvPatch::delta()` returns the patch-NORMAL component `n (n . d)` (ESI v2506 `fvPatch.C:156`), so `deltaCoeffs = 1/|n . d|` on boundary faces. The port uses `1/|Cf - C|`, so on a non-orthogonal boundary face the coefficient is too small by `cos(theta)`; here by up to 66 % (blockMesh) and 88 % (polyDual). The `NonOrthogonal` mode already does it. Predicted movement of the fix: every non-orthogonal boundary face of every case that uses the default mode; zero on orthogonal meshes. Not fixed: the pinned appbuilder results would need re-measuring.
- **D2. Arithmetic order (ulp level), all proven above:** cell volume and centre and face centre order, `*(1/x)` in place of `/x` (grad, `nonOrthDeltaCoeffs`), linear interpolation form, upwind. Bit-identity needs each changed to upstream's form; each change moves every downstream result by ulps, so none was made. Predicted movement: relative 1e-15 on fields, up to 1e-13 where a limiter ratio `r` amplifies it.
- **D3. vanAlbada and OSPRE clamp `r` at 0; upstream does not.** Their doc comments claimed the clamp was upstream's; corrected in `src/limiters.rs` (documentation only).
- **Which line.** The source read and run is ESI v2506. The Foundation (dev) text of `makeFaceCentresAndAreas`, `fvPatch::delta` and `surfaceInterpolationScheme::interpolate` was not run or read here, so D2's orderings are stated for ESI only and may differ in dev.

## Not covered (open items on gh:#1362)
Thermophysics and turbulence (need `libthermophysicalModels` and `libmomentumTransportModels`, not in the partial build; not attempted); `linearUpwind` and `limitedLinear` as full schemes (the port has neither; only the limiter functions are compared); `leastSquares` and `cellLimited` gradients; `snGrad corrected` and limited; an AMI pair; linear solvers (case 3); ODE (case 6).

## gh:#1488 (D1 fix): prediction, written 2026-11-11 BEFORE the code change
The baseline (all pinned and V&V numbers, current code) was recorded first; this
prediction was committed before the fix.
- **Differential test.** After the fix, boundary `deltaCoeffs` should be bit-identical on both meshes (1083/1083 and 1788/1788): `1/|nHat (nHat . d)|` with `nHat = Sf/magSf` is the upstream arithmetic. `snGrad (orthogonal)` still differs on interior faces by the `/d` against `deltaCoeffs*` ordering, so its identical count should rise from 533 and 890 to about the interior-face level, with a worst error of a few ulp instead of 66 % / 88 % relative.
- **Solver numbers.** Only boundary faces whose normal is not parallel to `Cf - C` move, and only in `DeltaCoeff::Orthogonal` mode (the default). Cases with orthogonal wall cells (Cartesian blocks, the 1-D and RZ structured pipes) should not move beyond an ulp (`|n . d|` against `|d|`). Skewed or polyhedral boundary cells should move: the wall conductance `Gamma |Sf| / |n . d|` gets larger (`|n . d| <= |d|`), so wall flux and wall shear grow in magnitude for the same wall value. Foam Studio's unstructured (polyDual) pipe, pinned at 0.9394 / 1.0062, is the likeliest to move, by up to a few percent; the sign relative to the analytic Hagen-Poiseuille value is not known in advance. GeN-Foam V&V and the CHT double pipe (structured, near-orthogonal) should move by under 1e-6 relative.
