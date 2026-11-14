# viewFactor radiation coupled into the CHT interface, vs ESI v2506 (gh:#1541, under gh:#1529 / #1540 / #1531, epic gh:#1379)

> Research, education and V&V only (`RESPONSIBLE_USE.md`). Not for plant analysis, licensing or any
> safety decision. AI-assisted output: untrusted until reviewed.

Date of every number: 2026-11-14, ESI OpenFOAM-v2506 built by `scripts/upstream/build-esi-v2506-and-genfoam.sh`
(partial dummy-Pstream build), Rust 1.99.1, release. Predictions: `PREDICTIONS.md` (commit `7e9f474d8`, before
the ESI reference was generated and before the port coupling was first run).

## What was built

* `outram_foam_appbuilder_lib::solvers::energy_view_factor::ViewFactorCoupling`, attached with
  `EnergyEquation::with_view_factor_radiation`. `viewFactor` has no volumetric source (`Ru = Rp = 0`), so the
  energy equation is unchanged; after each solve `rad.correct()` runs `viewFactor::calculate()` (first call and
  every `solverFreq`-th time index) from the stored boundary temperatures of the radiating patches and writes
  `qr` on them. `CoupledTemperature` reads `qr` from the fluid by default, as for P1 (gh:#1531):
  `has_radiative_flux` / `radiative_flux_on` in `cht_multi_region_foam/mod.rs` now also see the view-factor
  coupling. P1 and viewFactor exclude each other on one equation (ESI: one `radiationModel` per region).
* `ViewFactorCoupling::generate` builds `F` with the Rust `viewFactorsGen` port (gh:#1540) from the region's
  `PolyMesh`, with a constant emissivity and `qro` per radiating patch; `ViewFactorCoupling::new` takes a
  prebuilt `ViewFactorModel`.
* Driver: the gh:#1531 `chtRadDump.C`, with one additive guard (the `G` dump only if a `G` field exists, so a
  viewFactor fluid does not abort; the P1 output is unchanged). `scripts/run_cht_vf.sh` builds the case, runs
  upstream `viewFactorsGen -region fluid` and the driver.
* Search before building (root rule): checked `outram-foam-basic-lib::{view_factor_radiation, view_factor_generation,
  radiation}` (reused as is), `appbuilder::solvers::energy_radiation` (P1 coupling pattern copied, the `qr`
  interface path reused unchanged), the gh:#1529 / #1531 V&V folders and the `viewfactors` / `radiation` /
  `rad-cht-driver` build stages (reused). No existing viewFactor-to-energy coupling.

## Case

The gh:#1363 two-region case (fluid 20x5x1 water 0.02 x 0.01 x 0.001 m graded, solid 10x5x1, dt 0.1 s, 50
steps, left 300 K, solid wall 360 K) with ONE change: the fluid `frontAndBack` is a `wall` patch, not `empty`,
so the fluid boundary is a closed 3-D box of 250 faces (left 5, interface 5, walls 40, frontAndBack 200) and
ESI's `viewFactorsGen` takes its 3-D branch (the 2-D branch is not ported; with `empty` patches ESI would take
it). `radiationModel viewFactor` (direct LU, no smoothing, variable emissivity, no agglomeration), emissivities
left 0.9, interface 0.8, walls 0.5, frontAndBack 0.6, `qro` 0; the interface conditions carry `qr`
(fluid `qr qr; qrNbr none`, solid `qr none; qrNbr qr`). Fluid frontAndBack T is zeroGradient. Files:
`fluid/polyMesh/` (the case mesh), `reference/{F,globalFaceFaces,mapDist}.gz` (viewFactorsGen output),
`reference/cht_vf_v2506.txt.gz` (bit patterns of T and `qr` per step). Geometry:
the fluid mesh is the committed gh:#1363 mesh (20x5x1 blockMesh hex, drawn in the gh:#1529 images for the plate
analogue); not re-drawn here, so it is unreviewed by eye.

## Results

Test `tests/upstream_view_factor_cht_gh1541.rs` (6 tests pass; three of them pin a KNOWN RESIDUAL, see below).

| quantity (worst over 50 steps) | prediction | port with ESI's F | port with Rust-generated F |
|---|---|---|---|
| fluid cell T | <= 5e-11 K | **1.08e-11 K** | 1.69e-8 K |
| solid cell T | <= 2e-11 K | **3.81e-12 K** | 3.92e-8 K |
| `qr` on 4 patches, relative to the patch max | <= 1e-9 | **3.66e-12** | 2.49e-3 |
| bit-identical `qr` values | not claimed | 5 / 12,500 | 0 / 12,500 |

Max |qr| of the case: 223.8 W/m2 (hot interface/walls), so `qr` is resolved. The fluid-T floor 1.1e-11 K equals
the gh:#1363 / #1531 floor (T-form vs h-form), so the viewFactor coupling added no new error source when F is
ESI's. Bit-identity of the coupled T is not claimed or counted (as gh:#1531).

Scorecard (PREDICTIONS.md):

| # | prediction | result |
|---|---|---|
| 1 | Rust F byte-identical to ESI on this box (65 %) | **wrong**: 209 / 250 visible lists equal; upstream lists 86 pairs Rust does not (largest \|F\| 3.5e-6), Rust lists 44 pairs upstream does not (largest \|F\| **1.62e-4**); on the 41,853 common pairs F is bit-identical (worst 0). Same family as gh:#1564 (coplanar candidacy), but one Rust-only entry is not noise-sized |
| 2 | T <= 5e-11 / 2e-11 K, qr <= 1e-9, no bit-identity | **held with ESI's F**; **missed with Rust F** (1.7e-8 K, 2.5e-3): the F residual of row 1 propagates |
| 3 | ablation (no qr term) differs from ESI by 1e-5 .. 1 K | held: 5.41e-3 K fluid, 1.94e-2 K solid |
| 4 | ESI qr energy balance within 3 % | **wrong**: 0.2859. ESI's own F row sums are 0.9989 to 1.0586 and F is not reciprocal on this thin graded box; with `qro = 0` an exact F would give 0. The port reproduces that F, so this is upstream's discretisation error on thin cells, not a port defect. Pinned to the recorded value |
| 5 | off by default, existing differentials unchanged | held (see test runs in the commit message / issue comment) |
| 6 | P1 and viewFactor mutually exclusive | held, both orders refused |

**Known residual (not fixed, reported).** The Rust generator misses/adds visible pairs on this thin box; the
three pinned tests assert the measured residual (209 rows, 41,939 vs 41,897 pairs; coupled T 5e-11..1e-6 K) and
fail when gh:#1564 is fixed, at which point they must take the strict bounds of the ESI-F test. No tolerance was
relaxed to make anything pass.

## Not done

* htr10_rz cavity (RPV to RCCS) itself: needs the cavity-to-patch mapping and an axisymmetric/2-D generator.
* Agglomeration (`finalAgglom`), `smoothing`, `constantEmissivity`, `useSolarLoad`, iterative solver, parallel.
* A fluid with `empty` patches (open enclosure / ESI 2-D branch).
* Mesh images for this case (see Case).

## Reproduce

`scripts/upstream/build-esi-v2506-and-genfoam.sh viewfactors` (and `radiation`), then
`nice -n 10 verification_and_validation/upstream_view_factor_gh1541/scripts/run_cht_vf.sh 50`, then
`cargo test --release -p outram-foam-appbuilder-lib --test upstream_view_factor_cht_gh1541`.
