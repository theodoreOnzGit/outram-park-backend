# P1 radiation: port of ESI v2506 and its V&V record (gh:#1379, epic gh:#1376 / #1336)

> Research, education and V&V only (`RESPONSIBLE_USE.md`). Not for plant analysis,
> licensing or any safety decision. AI-assisted output: untrusted until reviewed.

## What was done

`outram_foam_basic_lib::radiation` ports OpenFOAM **ESI v2506** (`src/thermophysicalModels/radiation`):
the P1 `G` equation (`P1::calculate`), `MarshakRadiation`, `MarshakRadiationFixedTemperature`,
`constantAbsorptionEmission`, `noScatter` (as `sigma_eff = 0`), `boundaryRadiationProperties`
`lookup` (constant emissivity per patch), `Ru`/`Rp`, and `radiationModel::ST(T)` /
`radiationModel::Sh(thermo, he)` as matrices. It is additive and opt-in: no solver calls it,
no default of any existing solver changed.
One supporting change: `fvm::laplacian_with_delta_diag_and_source` (the existing
`laplacian_with_delta_and_source` now delegates to it with `None`; behaviour unchanged,
re-verified by the full lib suite) so that the `-Sp(a, G)` diagonal enters between the
interior faces and the boundary coefficients, upstream's order.

Workspace search before building (root rule): no radiation transport model existed.
Related code that is NOT a field model and was left alone: surface-to-surface conductances
in `tuas_boussinesq_solver` (`view_factors/cocentric_cylinders.rs`), `outram-park-fork-liggghts`
(`thermal_radiation.rs`), and the lumped grey RPV-to-RCCS radiation of
`outram-foam-appbuilder-lib/src/htr10_rz` (`model.rs`, with `RadiateTo50C`). A later step can
use `P1Model` to test that cavity's effective treatment (see "Remaining").

## Upstream build

`libradiationModels.so` is built by the existing `closure` stage of
`scripts/upstream/build-esi-v2506-and-genfoam.sh`. A new stage **`radiation`** (run under
`nice -n 10`, `WM_NCOMPPROCS=2`) re-asserts that library (stamped, a no-op) and builds the
differential driver `driver/radDump.C` to `/home/user/upstream-build/gh1379/bin/radDump`
(11 s). `scripts/run.sh` rebuilds the four references.

## Method (differential)

`radDump` calls the unmodified upstream `radiationModel::New(T)` -> `P1::correct()` and prints
IEEE-754 hex of geometry, T, sigma, G (cells, boundary), qr, Marshak valueFraction/refValue,
`Ru`, `Rp`, and the matrices of `ST(T)` and of `Sh` (the three lines of `radiationModel::Sh`
with a polynomial `Cpv` and `he` in place of a `basicThermo`; stated in the driver). Cases:

| case | mesh | medium | patches |
|---|---|---|---|
| hexA | 316-cell skewed hex+prism blockMesh (gh:#1362), 294 boundary faces | a = e = 2 /m, E = 0 | inlet Marshak eps 0.9; outlet MarshakFixedTemperature eps 0.6, Trad 900 K; walls Marshak eps 0.8 |
| hexB | same | a = 1.5, e = 1.2 /m, E = 500 W/m3 | same |
| slabThick | 40-cell slab, 1 m | a = e = 2 /m, T = 1000 K | xmin eps 0.8 at 1500 K; xmax eps 0.5 at 500 K; sides zeroGradient |
| slabThin | same | a = e = 0.5 /m | same |

T is a polynomial of position (no libm); `laplacianSchemes Gauss linear uncorrected`; PCG/DIC,
tolerance 1e-10, relTol 0; sigma = ESI's value from the truncated `etc/controlDict` constants.
`tests/radiation_vs_openfoam.rs` builds the port mesh from the upstream geometry and T and runs
`P1Model::solve` for exactly ESI's iteration count with tolerance 0, so arithmetic is separated
from the residual stop.

## Results: differential (release, 2026-11-13), PREDICTIONS.md items 1-4

Every compared quantity is **bit-identical** (+0.0/-0.0 counted equal): 100 % in every row.

| quantity | hexA | hexB | slabThick | slabThin |
|---|---|---|---|---|
| sigma | 1/1 | 1/1 | 1/1 | 1/1 |
| G (cells) | 316/316 | 316/316 | 40/40 | 40/40 |
| G (boundary values) | 294/294 | 294/294 | 162/162 | 162/162 |
| qr (boundary) | 294/294 | 294/294 | 162/162 | 162/162 |
| Marshak valueFraction | 294/294 | 294/294 | 162/162 | 162/162 |
| Marshak refValue | 294/294 | 294/294 | 162/162 | 162/162 |
| Ru | 316/316 | 316/316 | 40/40 | 40/40 |
| Rp | 316/316 | 316/316 | 40/40 | 40/40 |
| ST diag / source | 316/316 each | 316/316 each | 40/40 each | 40/40 each |
| Sh diag / source | 316/316 each | 316/316 each | 40/40 each | 40/40 each |
| ESI PCG iterations | 15 | 17 | 1 | 1 |
| port PCG iterations at tolerance 1e-10 | 15 | 17 | 1 | 1 |

Max ulp difference and max relative difference: 0 everywhere. No libm is involved (the
polynomial inputs, `pow4` as `sqr(sqr)`), so no 1-ulp PETIR allowance was needed.

Prediction scorecard (PREDICTIONS.md, committed before any run): items 1, 2, 3 held (100 %).
Item 4 MISSED: I expected the port's differently-normalised residual to stop up to 3 iterations
away from ESI; it stopped at the same count in all four cases (15, 17, 1, 1). The hex
cases are small (316 cells), so this says little about large meshes. The slab solves converge in
one iteration (a tridiagonal system: DIC is an exact factorisation), so the hex cases carry the
weight of the PCG comparison. **Not measured:** an ablation of the diagonal order (adding
`V a` after the boundary coefficients); the prediction that this would cost an ulp in boundary
cells was never tested, so `laplacian_with_delta_diag_and_source` is justified by upstream's
order, not by a measured difference.

## Results: analytic (PREDICTIONS.md items 5-7)

`tests/radiation_p1_analytic.rs`: uniform-T slab, closed form (`cosh`/`sinh`, constants from
the Marshak conditions), reference Modest, *Radiative Heat Transfer* 3rd ed. (2013), ch. 16.
Max cell-centre error of G relative to max|G|, and wall-flux `qr` error, by cell count:

| N | a = 2 /m: err G | order | err qr | order | a = 0.5 /m: err G | order | err qr | order |
|---|---|---|---|---|---|---|---|---|
| 10 | 2.579e-3 | - | 5.341e-3 | - | 2.322e-4 | - | 3.368e-4 | - |
| 20 | 7.362e-4 | 1.81 | 1.348e-3 | 1.99 | 5.930e-5 | 1.97 | 8.426e-5 | 2.00 |
| 40 | 1.956e-4 | 1.91 | 3.379e-4 | 2.00 | 1.498e-5 | 1.99 | 2.107e-5 | 2.00 |
| 80 | 5.035e-5 | 1.96 | 8.452e-5 | 2.00 | 3.764e-6 | 1.99 | 5.268e-6 | 2.00 |
| 160 | 1.277e-5 | 1.98 | 2.113e-5 | 2.00 | 9.435e-7 | 2.00 | 1.317e-6 | 2.00 |

Prediction 5 held (order 1.8-2.1 for the last pair: 1.98 and 2.00; N = 80 error < 1e-4: 5.0e-5 and
3.8e-6). This verifies that the discrete system solves the P1 equation with the Marshak
condition correctly (convergence to the exact solution of that equation). It does NOT
validate P1 against exact radiative transfer; P1 is itself an approximation (poor for optically
thin media and near walls), and that model error was not measured here.
Items 6-7 are gated by `src/radiation.rs` unit tests: isothermal equilibrium G = 4 sigma T^4
to 1e-10, and `sum V (Ru - Rp T^4) + sum qr |Sf|` below 1e-8 of the emitted power.

## Survey (not ported): what the rest of the radiation library would take

Sizes are ESI v2506 lines (`.C` + `.H`) under `src/thermophysicalModels/radiation`.

| piece | lines | what it needs | verdict |
|---|---|---|---|
| `viewFactor` | 1,470 (+ `viewFactorsGen`, `createViewFactors` ray-shooting/Hottel/2AI/2LI generators) | coarse-face agglomeration (`singleCellFvMesh`, `faceAgglomerate`), dense LU solve of the radiosity system, a precomputed view-factor matrix (file or generator), `greyDiffusiveViewFactor` BC, serial path only needed here (the parallel gather code is skipped) | medium-large: child issue |
| `fvDOM` | 2,668 | `radiativeIntensityRay` per direction (nPhi x nTheta), per-ray transport equation with `fvm::div` upwind and a `GaussSeidel` ray-coupling loop, `blackBodyEmission`, `greyDiffusiveRadiation` BC, wide-band `absorptionCoeffs` | large: child issue |
| `solarLoad`, `solarCalculator` | 2,903 | ray-casting onto surfaces (searchableSurface), sky models | out of scope for HTGR |
| `opaqueSolid` | 238 | trivial wrapper | with viewFactor |
| absorption models | 3,150 | `greyMean*`, `multiBand`, `wideBand` need thermo/species data | constant is done; the rest follow when a species thermo is wired |
| scatter / soot | 590 / 911 | `constantScatter`, `sootModel` | small, not needed for gray P1 |
| `boundaryRadiationProperties` | 2,737 | per-face / mixed / wavelength-dependent wall properties | `lookup` constant is done |

Remaining for HTGR relevance: `viewFactor` (surface cavities such as RPV to RCCS) is the
better test of the htr10_rz effective-conductivity assumption than P1, which models a
participating medium. Children are filed on the issue (see its comments).

## Files

- `driver/radDump.C`, `driver/Make/`: the upstream driver; `inputs/`: case files; `scripts/run.sh`;
  `reference/ref_*.txt.gz`: the four dumps (`gzip -n`, read by `include_bytes!`).
- `PREDICTIONS.md`: written and committed before any run (commit e614a3ce, 2026-11-13).
- Tests: `tests/radiation_vs_openfoam.rs`, `tests/radiation_p1_analytic.rs`, `src/radiation.rs` unit tests.

## Remaining / deferred

- ~~Coupling of `P1Solution::st_matrix`/`sh_matrix` into `outram-foam-appbuilder-lib` (`solvers::energy`,
  `cht_multi_region_foam`, `htr10_rz`) is NOT done: wiring touches solver loops and needs its own
  V&V against a radiation tutorial (`buoyantSimpleFoam/hotRadiationRoom`). Deferred, recorded on the issue.~~ **DONE 2026-11-13 (gh:#1531):** `solvers::energy` and the fluid regions of `cht_multi_region_foam` carry opt-in P1 (`EnergyEquation::with_radiation`), compared with ESI `chtMultiRegionFoam` + P1 on the gh:#1363 two-region case (a reduced coupled case, not `hotRadiationRoom`): `outram-foam-appbuilder-lib/verification_and_validation/upstream_radiation_gh1531/README.md`. `htr10_rz` is not coupled (same record, section "htr10_rz").
- Coupled (cyclic/AMI/processor) and wedge patches are not exercised: upstream skips `qr` on `coupled()` patches; the port's `P1Model` has no coupled-patch variant and treats every non-Marshak patch via `ZeroGradient` only when asked.
- Non-orthogonal correction (`corrected`) of the G Laplacian is not used; `uncorrected` was the
  scheme compared.
