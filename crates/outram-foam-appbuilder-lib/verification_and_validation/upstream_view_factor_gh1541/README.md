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
| 1 | Rust F byte-identical to ESI on this box (65 %) | **wrong first time** (209 / 250 visible lists equal; upstream-only 86 pairs, Rust-only 44, largest \|F\| 1.62e-4; cause: brute-force ray search, gh:#1564), **~~wrong~~ CORRECTED 2026-11-14 held after the fix**: 250 / 250 rows, 41,939 = 41,939 pairs, all bit-identical (worst 0) |
| 2 | T <= 5e-11 / 2e-11 K, qr <= 1e-9, no bit-identity | **held with ESI's F**; ~~**missed with Rust F** (1.7e-8 K, 2.5e-3): the F residual of row 1 propagates~~ **CORRECTED 2026-11-14 (gh:#1564)**: held with Rust F after the generator fix, identical to the ESI-F numbers: fluid T 1.08e-11 K, solid 3.81e-12 K, qr 3.66e-12 relative (re-measured) |
| 3 | ablation (no qr term) differs from ESI by 1e-5 .. 1 K | held: 5.41e-3 K fluid, 1.94e-2 K solid |
| 4 | ESI qr energy balance within 3 % | **wrong**: 0.2859. ESI's own F row sums are 0.9989 to 1.0586 and F is not reciprocal on this thin graded box; with `qro = 0` an exact F would give 0. The port reproduces that F, so this is upstream's discretisation error on thin cells, not a port defect. Pinned to the recorded value |
| 5 | off by default, existing differentials unchanged | held (see test runs in the commit message / issue comment) |
| 6 | P1 and viewFactor mutually exclusive | held, both orders refused |

~~**Known residual (not fixed, reported).** The Rust generator misses/adds visible pairs on this thin box; the
three pinned tests assert the measured residual (209 rows, 41,939 vs 41,897 pairs; coupled T 5e-11..1e-6 K) and
fail when gh:#1564 is fixed, at which point they must take the strict bounds of the ESI-F test.~~ **FIXED
2026-11-14 (gh:#1564).** The residual came from the brute-force ray search; the Rust `viewFactorsGen` now
runs its rays through the port of upstream's `indexedOctree::findLine` (`outram_foam_basic_lib::esi_octree`),
see the basic-lib record `upstream_view_factor_gh1529/README.md`, "gh:#1564". Second finding on this box: upstream
lists self-hit-continued rays AFTER the rays that missed on the first `findLine`, so a row is not ascending
(1 row of 250 differed in order only). Re-measured 2026-11-14 (Rust 1.99.1, release):

| quantity | before | after |
|---|---|---|
| visible lists equal | 209 / 250 | **250 / 250** |
| pairs upstream / Rust / common / bit-identical | 41,939 / 41,897 / 41,853 / 41,853 | **41,939 / 41,939 / 41,939 / 41,939**, worst 0 |
| coupled step, Rust F: fluid T / solid T / qr (rel.) vs ESI | 1.7e-8 K / 3.9e-8 K / 2.5e-3 | **1.08e-11 K / 3.81e-12 K / 3.66e-12**, equal to the ESI-F run |

The three formerly pinned tests (`generated_f_for_the_fluid_box_matches_viewfactorsgen`,
`coupled_view_factor_with_rust_f_...`) now carry the strict bounds. No tolerance was relaxed or tuned.

## Not done

* htr10_rz cavity (RPV to RCCS) itself: needs the cavity-to-patch mapping and an axisymmetric/2-D generator.
* Agglomeration (`finalAgglom`), `smoothing`, `constantEmissivity`, `useSolarLoad`, iterative solver, parallel.
* A fluid with `empty` patches (open enclosure / ESI 2-D branch).
* Mesh images for this case (see Case).

## Reproduce

`scripts/upstream/build-esi-v2506-and-genfoam.sh viewfactors` (and `radiation`), then
`nice -n 10 verification_and_validation/upstream_view_factor_gh1541/scripts/run_cht_vf.sh 50`, then
`cargo test --release -p outram-foam-appbuilder-lib --test upstream_view_factor_cht_gh1541`.
