# Opt-in P1 radiation in the appbuilder energy equation, vs ESI v2506 (gh:#1531, epic gh:#1379 / #1376 / #1336)

> Research, education and V&V only (`RESPONSIBLE_USE.md`). Not for plant analysis, licensing or any
> safety decision. AI-assisted output: untrusted until reviewed.

## What was done

`outram_foam_appbuilder_lib::solvers::energy_radiation::RadiationCoupling`, attached with
`EnergyEquation::with_radiation(P1Model, g0)` (so also to the fluid regions of `ChtMultiRegionFoam`
and to `PimpleFoam::energy`). Translation of ESI v2506 `chtMultiRegionFoam/fluid/EEqn.H`:

| upstream | here |
|---|---|
| `== ... + rad.Sh(thermo, he)` (lagged `G`, current `T`/`he`; implicit in `he`) | `RadiationCoupling::energy_source`, added to the T-matrix before the solve, assembled once per solve from the entry temperature |
| `EEqn.solve; thermo.correct(); rad.correct();` | `EnergyEquation::solve` calls `RadiationCoupling::correct` after each solve (once per outer pass) |
| `radiationModel::correct`: `firstIter_ \|\| timeIndex % solverFreq == 0` | `solver_freq`, `begin_time_step()` (called by `PimpleFoam::step` and `ChtMultiRegionFoam::step`) |
| `P1::calculate` solves from the current `G_` | `P1Model::solve_from` (new; `solve` is unchanged and delegates with a cold start) |
| `0/G` read before the first `correct()` | `P1Model::initial_solution(t, g0)` (new) |
| `he = Cp (T - Tref)`, `Tref = Tstd = 298.15` (`hConst`) | `he -> T` conversion in `energy_source`: `diag_T = diag_h/rho`, `source_T = source_h/(rho cp) + (diag_h/rho) Tref`. `Tref` cancels analytically; kept at ESI's value for round-off |

Defaults: `EnergyEquation::radiation` is `None`; nothing is added to the off path. Pinned by
`radiation_is_off_by_default_and_a_bad_model_is_refused` and by the unchanged gh:#1363 / #1156 /
energy tests (full appbuilder suite, below).

Workspace search before building (root rule): the P1 model exists in `outram-foam-basic-lib::radiation`
(gh:#1379, reused as is, plus two additive methods); no other energy-equation radiation coupling existed
(`htr10_rz` radiates through effective conductivities and boundary conditions, see below). Checked and
rejected: `tuas_boussinesq_solver` view factors and `outram-park-fork-liggghts` radiation (not field models).

## Method (differential)

`driver/chtRadDump.C` is gh:#1363's `chtDump.C` (`chtMultiRegionFoam.C` + `EEqn.H` + `solveSolid.H`
transcribed on the unmodified ESI libraries, real `heRhoThermo`, real `radiationModel::New` -> `P1`, real
`turbulentTemperatureRadCoupledMixed`) with an added dump of `G`, `qr` and `G` on the patches. Substitutions
are gh:#1363's (frozen flow, fluid at rest, `rhoThermo` for `rhoReactionThermo`, no coupled-energy assembly,
`nOuterCorrectors 1`; see `../cht_multi_region_foam_vs_openfoam.md`). Built by the new sub-stage
`rad-cht-driver` of the `radiation` stage of `scripts/upstream/build-esi-v2506-and-genfoam.sh`
(~15 s); `scripts/run_cht_rad.sh` writes the cases and the references.

**Not `hotRadiationRoom`.** That tutorial is `buoyantFoam`: compressible, buoyant, k-epsilon with wall
functions, `fvDOM` in some versions. The port's fluid is incompressible and has no buoyant pressure
equation, so the tutorial cannot be represented (the same reason gh:#1363 did not run `multiRegionHeater`).
The reduced coupled case is the gh:#1363 two-region case (meshes reused from
`../upstream_openfoam_gh1363/cht`): water 20x5x1 cells + solid 10x5x1, fixed 300 K left wall, 360 K solid
wall, dt 0.1 s, 50 steps, with P1 in the fluid: a = e = 50 /m, E = 0, `G` starting at 0, wall emissivities
0.9 (left) / 0.8 (interface) / 0.5 (walls), `laplacian(gammaRad,G) Gauss linear uncorrected` (the only G
Laplacian the port has), G solved by PCG/DIC to 1e-15 by ESI; the port replays ESI's per-step PCG iteration
count with tolerance 0 (arithmetic compared, not the stopping rule). Energy solves: ESI 1e-15, port 1e-16.
A second reference re-runs ESI with `solverFreq 3`. ~~The interface condition has `qr none` on both sides
(the port's `CoupledTemperature` has no radiative flux term): radiation heats and cools the fluid, and the wall
flux `qr` is computed and compared, but does not enter the interface balance.~~ **CORRECTED 2026-11-14:** that
describes the three references above (`cht_rad`, `cht_rad_f3`), which are `qr none` on both sides and are compared
with the port's explicit ablation `without_radiative_flux_coupling`. Since 2026-11-14 `CoupledTemperature` carries
the flux term and the port applies it by default; the fourth reference `cht_rad_qr` has `qr qr; qrNbr none` (fluid)
and `qr none; qrNbr qr` (solid). See "qr coupling" below.

`tests/upstream_radiation_cht_gh1531.rs` compares per step: fluid and solid cell T, fluid cell `G`, `qr` and `G`
on the three patches.

## Predictions and scorecard (PREDICTIONS.md, committed in ac7a2b7a0 before any run)

| # | prediction | result | verdict |
|---|---|---|---|
| 1 | mean step-1 fluid dT = -2.197e-3 K (+-2 %), derived from `Rp T^4 dt/(rho cp)` | ESI -2.1891e-3 K (-0.4 %); port the same (test pins it) | held |
| 2 | no-radiation port differs from ESI-with-radiation by >= 1e-3 K and <= 0.12 K | worst 1.1045e-2 K (50 steps) | held |
| 3 | port vs ESI, fluid cell T <= 5e-11 K, solid <= 2e-11 K | 1.07e-11 K, 3.6e-12 K (after the fix below) | held |
| 4 | G relative <= 1e-9, qr <= 1e-8 of max | G 1.5e-13, patch G 1.6e-13, qr 2.8e-10 | held |
| 5 | first divergence at the interface Marshak `refValue`, via ESI's one-step-lagged stored boundary T, **at step 2, up to ~10 W/m2** | cause right; **step wrong (step 1) and size wrong** (first G error 6.4e-8 relative = 1.2e-4 W/m2) | cause held, step and size missed |
| 6 | defaults unchanged | new field `None`; full suite below | held |
| 7 | `solverFreq 3` rule reproduced | ESI solves G on steps 1,3,6,9,...; port matches (below) | held |

## Results (release, 2026-11-13)

First run, before the fix (port used the post-solve interface face temperature for the Marshak wall):

| quantity (worst over 50 steps) | first run | after the fix |
|---|---|---|
| fluid cell T | 1.28e-6 K (grew from step 2) | **1.07e-11 K** |
| solid cell T | 7.2e-8 K | **3.6e-12 K** |
| G cells, relative to max G | 1.6e-5 (6.4e-8 already at step 1) | **1.5e-13** |
| G on patches, relative | 1.8e-5 | **1.6e-13** |
| qr on patches, relative to max |qr| | 8.0e-3 (step 1) | **2.8e-10** |

With `solverFreq 3`: fluid T 1.01e-11 K, solid 3.7e-12 K, G 1.5e-13, patch G 1.6e-13, qr 2.8e-10.
The fluid-T floor 1.07e-11 K equals gh:#1363's no-radiation floor (1.05e-11 K, T-form vs h-form
representation), so radiation added no new error source. Bit-identity is not claimed or counted.

**Localised first divergence and the fix (translation of upstream behaviour).** Step 1, `G` of the cells
at the interface, `Gb_interface`: the Marshak `refValue = 4 sigma Tw^4` reads the stored `T` boundary value
of the interface. ESI's stored value on a mixed (coupled) patch is the one the condition gave at assembly,
from this step's coefficients and the OLD cell value (ESI step 1: 300.0 exactly; the value implied by the new
cell is 299.9999838), as already found in gh:#1363 ("Tb" vs "Tf"). The port re-evaluated the boundary after the
solve. `EnergyEquation::solve` now hands `rad.correct` the pre-solve boundary values on `Mixed`/`MixedField`
patches and the post-solve values elsewhere (`fixedValue`, `zeroGradient` agree either way). Measured, not
derived from upstream source: **the `Tb` semantics was read from the dump, not from `heThermo` /
`mixedEnergy` source; I did not trace why ESI behaves so.** Not tested: a non-mixed coupled patch type.

Other checks: `radiative_equilibrium_leaves_t_unchanged` (|dT| < 1e-9 K), the linearised first-step cooling
formula (1e-9), `solver_freq_follows_the_time_index`, `warm_start_and_initial_solution`, and, for the HTR-10
discussion below, `transparent_gap_recovers_the_grey_two_plate_exchange` in `outram-foam-basic-lib`.

## htr10_rz and the HTR-10 effective-conductivity assumption (nothing changed; no HTR-10 number moved)

What `htr10_rz` does (read, `materials.rs`, `model.rs`): a solid-only R-Z conduction model
(`genfoam::thermal_hydraulics::solid_conduction`, **not** `solvers::energy`). Helium cavities are homogenised
media with `lambda = lambda_He + 4 sigma T^3 delta/(2/eps - 1)` (parallel-plate exchange written as a
conductivity); the RPV side wall radiates to a 50 degC RCCS with the concentric-cylinder `eps_eff = 0.7626`
(Kugeler 2017) in parallel with a natural-convection correlation; the heads radiate as two parallel plates
(`eps_eff = 0.7391`); the pebble bed uses a Breitbach/ZBS effective conductivity that already includes
radiation inside the bed.

What coupling P1 there would mean, and why it is not done:
1. **Structure.** P1 couples to a `T` energy equation through `Sh`; `htr10_rz` has its own solid solver and
   no `EnergyEquation`. Coupling needs a P1 source term in that solver and a G mesh of the cavities.
2. **Participating medium vs surface exchange.** The cavities are transparent helium: a -> 0 and
   `gamma = 1/(3a)` is unbounded (the model keeps it finite only through ROOT_VSMALL). In the transparent
   limit with Marshak walls, P1 gives, in planar geometry, exactly the grey two-plate exchange
   `sigma(T1^4-T2^4)/(1/e1+1/e2-1)` (hand derivation, verified numerically to 1e-3 by the basic-lib test above).
   So for the planar head cavities P1 would reproduce what `htr10_rz` already assumes: it would not test it.
3. **Curved cavities.** By the same derivation for concentric cylinders (flux x radius constant, G uniform
   as a -> 0) P1 gives `sigma(T1^4-T2^4)/(1/e1 + (r1/r2)/e2 - (1+r1/r2)/2)` against the exact
   `1/e1 + (r1/r2)(1/e2-1)`. At HTR-10's RPV-to-RCCS gap (r1/r2 = 218/285.4, eps = 0.85) the denominators
   are 1.193 vs 1.311: **P1 would overstate the exchange by about 10 %**. This is a hand derivation, **not
   run**; P1 is the wrong model for a transparent curved cavity.
4. **Bed radiation is already inside lambda_eff.** Putting P1 in the bed would double count unless
   `lambda_eff` is split into conductive and radiative parts, which the Breitbach/ZBS closures do not provide.
5. **Which model tests the assumption.** The load-bearing assumption (cavity and RCCS exchange through an
   effective conductivity / one effective emissivity) is a surface-to-surface problem: `viewFactor` (gh:#1529),
   as the maintainer hinted, is the model that would resolve the cavity surfaces with their view factors and
   test the `eps_eff` and the parallel-plate-as-conductivity treatments. P1 is relevant to HTR-10 only for a
   participating gas (not helium) or a bed treated as a medium (point 4).

## Defects found

- In the **port** (this work): the interface-boundary lag above (fixed by translation).
- In the **docs**: `energy.rs` module doc said radiation was absent, `README.md` (appbuilder) said it was not
  carried, `basic-lib` README / gh:#1379 record said no solver called P1 and the coupling was "NOT done": all
  struck through and corrected in place (grep'd, 4 places).
- In the **earlier P1 record**: none contradicted. Observation: `P1Model::solve` started cold, upstream warm-starts;
  immaterial for the single-solve gh:#1379 cases (G starts at 0 there) but it would have shown as 1e-12-level
  differences over a transient. `solve_from` adds the warm start; `solve` is bit-unchanged (test).

## Not covered / remaining

- Flowing fluid, buoyancy, the compressible solve, `hotRadiationRoom` itself; `nOuterCorrectors > 1` with
  radiation (the port calls `correct` once per energy pass as `EEqn.H` does, but ESI is only run with one pass).
- ~~Radiative flux into the interface condition (`qr`/`qrNbr` of `turbulentTemperatureRadCoupledMixed`): needs a
  `CoupledTemperature` extension. Filed as a follow-up on the issue.~~ **DONE 2026-11-14**, see "qr coupling".
  Still open: `thicknessLayers`/`kappaLayers` together with `qr` (the port's layered mode is `baffleMixed`'s, which
  differs from `RadCoupledMixed`'s, not compared), `thermalInertia`, `useImplicit`.
- Non-orthogonal (`corrected`) G Laplacian; per-patch emissivity models; variable properties (Cp(T)): the
  `he -> T` conversion assumes constant Cp.
- `docs/outram-foam-appbuilder-lib-api.md` mirror not regenerated (`kovan-cli api-docs`).

## qr coupling (2026-11-14, predictions Q1-Q6 in the PREDICTIONS.md addendum, committed in b8c30833e before any run)

**Upstream, read (ESI v2506 `turbulentTemperatureRadCoupledMixedFvPatchScalarField.C`, `updateCoeffs`).** `qr` is looked up
on this patch (`qrName_`, default `none` = 0), `qrNbr` on the neighbour patch (`qrNbrName_`, default `none`), paired
by `distribute`. Without `thermalInertia`: `valueFraction = KDeltaNbr/(KDeltaNbr + alpha)`, `refValue = TcNbr`,
`refGrad = (qr + qrNbr)/kappaTp`, with `alpha = kappaTp (1 + KDeltaNbr/KDeltaC) deltaCoeffs` and `KDeltaC = GREAT` without
layers. There is no `qrRelaxation` in this condition in v2506 (the name occurs only in `thermalBaffle1D`); the field is
used as it stands. `qr` is the radiation model's field, wall values `-gamma snGrad(G)` (`P1.C:244`), so it is the value
of the last `rad.correct()`; the fluid sees the previous pass's, the solid (solved after the fluid in the pass) the new one.

**Port.** `CoupledTemperature::update_coeffs_with_flux` (new; `update_coeffs` calls it with `None, None`) adds the
`refGrad` term, leaving `valueFraction` and `refValue` untouched (unit test `radiative_flux_enters_ref_grad_only`).
`InterfaceRadiation::Auto` is the default: a fluid with `with_radiation` supplies `qr` to its own side and `qrNbr` to the
neighbour, the ESI tutorial convention. `with_radiative_flux` states the terms per side (a flux the region does not have is
refused at `ChtMultiRegionFoam::new`, as upstream's failed lookup); `without_radiative_flux` /
`ChtMultiRegionFoam::without_radiative_flux_coupling` is the ablation. No kovan review stamp covers
`cht_multi_region_foam` (checked: no `review.md` there), so the existing functions were edited, not wrapped.

**Method.** ESI v2506 run as above with only the two interface `T` entries changed (`scripts/run_cht_rad.sh 50 50 1
cht_rad_qr 1`, reference `reference/cht_rad_qr_v2506.txt.gz`), compared per step by `tests/upstream_radiation_cht_gh1531.rs`
`default_qr_coupling_matches_upstream_rad_coupled_mixed` (default port, nothing asked for).

**Results, 2026-11-14, 50 steps, max over steps:**

| quantity | port (default, qr on) vs ESI with qr | prediction | held |
|---|---|---|---|
| fluid cell T | 1.04e-11 K | <= 5e-11 K | yes |
| solid cell T | 3.8e-12 K | <= 2e-11 K | yes |
| G (cells), relative | 1.49e-13 | <= 1e-9 | yes |
| G (patches), relative | 1.60e-13 | <= 1e-9 | yes |
| qr on patches, relative to max | 2.8e-10 | <= 1e-8 | yes |

Not bit-identical: the floor is the same T-form (port) versus h-form (ESI) round-off as the gh:#1363 record (1.05e-11 K) and as
the qr-less run above (1.07e-11 K); the qr term did not raise it.
The ablated port reproduces the qr-less ESI reference at 1.07e-11 K, so the old record stands as an explicit
ablation. **Q2's named risk (ESI re-evaluating the condition after `rad.correct()`) did not occur**: the lag the port uses
agrees with ESI's to this floor.

Does the term matter (Q3, Q4)? Yes, measurably: ESI with `qr` versus ESI without, fluid T, step 50: **8.47e-3 K**
(prediction 1e-3 .. 1e-2 K, held), the interface-adjacent cells lower with `qr` (asserted for every interface cell, held).
`qr` on the interface is -350 .. -352 W/m2 at step 50 (negative as predicted; bound 3.8e2 W/m2 held). The port with `qr`
ablated differs from ESI-with-qr by 8.4674e-3 K, i.e. by the whole ESI shift, so the case discriminates. Q5 (default, and
nothing changes without radiation: `explicit_terms_equal_auto_and_a_missing_flux_is_refused`) held. Q6 (layers with qr) was
not tested, as stated beforehand.

**Interpretation.** The port reproduces ESI's `qr`/`qrNbr` handling in `turbulentTemperatureRadCoupledMixed` on a
frozen-flow fluid at rest with P1; it says nothing about flowing, buoyant or compressible CHT, layers, or the
`viewFactor`/`fvDOM` models. Research and V&V only.

**Doc claims struck through in this change (code contradicted them):** the "does not enter the interface balance" paragraph above;
the "Not covered" item on `qr`; `energy_radiation.rs` module docs ("Not here"); the crate README rows for `energy` and
`cht_multi_region_foam`.

## Files
`driver/chtRadDump.C`, `driver/Make/options`, `inputs/`, `scripts/run_cht_rad.sh`,
`reference/cht_rad_v2506.txt.gz`, `reference/cht_rad_f3_v2506.txt.gz`, `reference/cht_rad_qr_v2506.txt.gz`, `PREDICTIONS.md`;
tests `tests/upstream_radiation_cht_gh1531.rs` and the unit tests in `src/solvers/energy_radiation.rs`.
