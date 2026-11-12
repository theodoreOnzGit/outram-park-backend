# GeN-Foam port gap audit (gh:#1217, epic gh:#1216)

**Taken 2026-11-04** against upstream GeN-Foam commit `652b3da` (the commit
every port header cites; it is the archived tip, "docs: Add archive banner"),
fetched shallow and sparse from `gitlab.com/foam-for-nuclear/GeN-Foam`. Port
side: `crates/outram-foam-appbuilder-lib/src/genfoam/` at branch
`claude/outram-foam-8ookor` merged with `develop` (merge `270705faf`).

> **Update, same day, after gh:#1218.** The tables below are the state
> *before* the two-phase port, kept as audited. `solver/two_phase/` now
> covers, for the selections `1D_boiling` makes: `twoPhase.{C,H}`,
> `alphaEqns_2p.H` + `solveAlphaImplicitUpwind.H`, `UEqns_2p.H`, `pEqnf_2p.H`
> with the implicit partial-elimination branch and its `pEqnRoutines`,
> `EEqns_2p.H` (both stabilisation modes), the field orchestration of
> `FFPair::correct`, `FSPair::correct` (isotropic), `FFDragFactor`,
> `twoPhaseDragFactor`, `virtualMass`, `phaseChangeModel::correct` with
> `heatDriven` (no `dmdtW` source), `latentHeatModel::adjust`,
> `fluidDiameterModel::correctField`, the `byRegime` dispatch for dispersion
> and contact partition, `structure::correct`/`linearizedSemiImplicitHeatSource`
> for `fixedPower`, and `customPimpleControl`'s residual control and
> `correctUntilConvergence`. Each of those rows moves from *missing* to
> *partial*. Still missing in the solver: MULES, `pEqn_2p.H` (cell-centred),
> the `none`/`explicit` partial-elimination branches, turbulence, tortuosity.

## How this was measured

- **Upstream line counts** are `wc -l` over every `*.C`/`*.H` file under
  `src/classes/`, summed per directory: **88,169 lines** in total.
- **Port line counts** are `wc -l` over every `*.rs` under `src/genfoam/`:
  **38,791 lines** (tests included).
- **Mapping.** For every upstream directory, the port was searched for the
  directory's leaf name (the model or class name) as a whole word; each hit was
  then read to see whether it is a port, a citation, or a "not ported" note.
  The status of every row below was set from the port module's own doc comment
  and code, not from the search hit alone. Where the module doc and the code
  disagree, the code wins and the doc is flagged.
- **Status.** *Ported*: the upstream logic is translated and reached by a test.
  *Partial*: part of the upstream logic is translated (some variants, or the
  per-cell algebra without the field-level orchestration upstream does).
  *Missing*: nothing translated. A closure that exists but is incomplete is
  *partial*, and counts as a gap.
- **V&V class.** *Unit*: tested against hand calculations, closed forms or
  published correlation values. *Upstream run*: compared cell by cell against
  a run of upstream GeN-Foam (`tests/genfoam_*.rs`, data in `tests/data/`).
  *Tutorial*: compared against a tutorial `Alltest` reference value.

## Summary

| area | upstream lines | ported | partial | missing |
|---|---:|---:|---:|---:|
| `thermalHydraulics/solvers` | 7,320 | 0 | 1 (`onePhase`) | 2 (`twoPhase`, `onePhaseLegacy`) + base class partial |
| `thermalHydraulics/src/physicsModels` (closures) | 34,727 | 46 models | 9 families | 12 models |
| `thermalHydraulics/src/phaseModels` | 15,070 | 6 | 3 | 8 |
| `thermalHydraulics/src` other (BCs, FOs, thermo, PIMPLE, include) | 8,433 | 10 | 1 | 3 |
| `neutronics` | 11,110 | 6 | 2 | 1 |
| `thermoMechanics` | 3,125 | 1 | 1 | 2 |
| `multiRegion` | 2,379 | 0 | 3 | 0 |
| `common` | 3,696 | 3 | 1 | 3 |
| `openFoamImportedSolvers` | 1,884 | 0 | 0 | 2 |
| `offbeat` | 425 | 0 | 0 | 1 (external coupling, out of scope) |

Counts are of rows in the tables below (one row per upstream model or class).

**The biggest gaps, by consequence:**

1. **The `twoPhase` solver** (`solvers/twoPhase`, 4,568 lines including
   `myMULES`): no two-fluid driver existed, so no closure in the
   two-phase families had ever run inside a coupled solve. This audit's
   step 2 (gh:#1218) adds the first slice; see
   `verification_and_validation/genfoam_two_phase/README.md` for what it
   covers.
2. **`FFPair`/`FSPair` field assembly** (1,867 lines): only the Reynolds
   numbers and relative velocity are ported. The `Kd`, `htc`, contact
   partition, interfacial temperature and virtual-mass assembly that the
   two-phase equations consume are not, outside the new two-phase slice.
3. **`multiRegimeBoilingTRACE`/`TRACECHF`/`VapourTRACE`** (1,442 lines) and
   **`lookUpTableCHF`** (366): every PSBT tutorial selects
   `multiRegimeBoilingTRACE`; Exercises 2 and 3 select `lookUpTableCHF`. Without
   them no PSBT case can run.
4. **`regimeMapModels/twoParameters`** (1,501) and the **`byRegime`
   templated dispatch** (428): PSBT's regime map is `twoParameters`, and
   every two-phase tutorial (1D_boiling included) dispatches dispersion,
   contact partition or diameter `byRegime`.
5. **The structure field layer** (`structure.C` 1,181, `byZone` 711): the
   anisotropic `Kd`/tortuosity tensors, local frames, the semi-implicit
   enthalpy source and `byZone` zone dispatch.
6. **Radial-conduction power models** (`heatedPin` 824, `nuclearFuelPin`
   1,534, `interpolatedNuclearFuelPin` 353, `lumpedNuclearStructure` 759):
   every reactor-scale tutorial that is not a pebble bed uses one.
7. **`MULES`** (`myMULES` 1,225 + `solveAlphaMULES` 540): the alpha solver
   1D_boiling, 1D_CHF, 2D_voidMotionNoPhaseChange and 2D_KNS37-L22 select.

## 1. `thermalHydraulics/solvers`

| upstream | lines | port | status | tests / V&V |
|---|---:|---|---|---|
| `thermalHydraulicsModel.{C,H}` (region base: PIMPLE, `p`/`p_rgh`, `g`, `hRef`, power coupling fields) | 745 | `solver/mod.rs` (`ThermalHydraulicsSolver` enum), fields inlined in `one_phase.rs` | partial | none of its own |
| `onePhase/` (`onePhase.{C,H}`, `UEqn_1p`, `pEqn_1p`, `pEqnf_1p`, `EEqn_1p`, 7 `pEqnRoutines`) | 1,022 | `solver/one_phase.rs` (661), `solver/porous_drag.rs` (230) | partial: `one_phase.rs` is the older constant-property driver (`he = Cp T`, isotropic `Kd`, no `p_rgh`/gravity split, no `pEqnf`, no `fvOptions`), kept as it was. ~~no `pEqnf` face-centred momentum~~ **CORRECTED 2026-11-12 (gh:#1361 round 4):** `solver/two_phase/one_phase_fc.rs` (translation of `onePhase.C`, `UEqn_1p`, `pEqnf_1p` + routines, `EEqn_1p`, `FSPair::correct`, `customPimpleControl::loop`; heRhoThermo packages, `p_rgh`/gravity, `contErr`, kinetic energy; NOT ported: other momentum modes, turbulence, heat exchangers, pumps, tortuosity, non-`zeroGradient` U, closed volumes) runs the 40-cell channel derived from 1D_boiling against the real binary to 1e-13 over 108 steps | unit: `solver/tests.rs` (6); `tests/genfoam_one_phase_vs_upstream.rs` (vs the real binary) |
| `onePhaseLegacy/` | 985 | — | missing | — |
| `twoPhase/` (`twoPhase.{C,H}`, `UEqns_2p`, `pEqn_2p`, `pEqnf_2p`, `EEqns_2p`, `alphaEqns_2p`, 9 `pEqnRoutines`) | 2,654 | none before gh:#1218 | missing | — |
| `twoPhase/.../solveAlphaImplicitUpwind.H` | 148 | none before gh:#1218 | missing | — |
| `twoPhase/.../solveAlphaMULES.H` + `myMULES/` | 1,765 | — (the workspace's other two-fluid solvers, `outram-foam-multiphase::two_fluid`, state they are MULES-free) | missing | — |

## 2. `thermalHydraulics/src/physicsModels` — the closures

| upstream family / model | lines | port | status | tests / V&V |
|---|---:|---|---|---|
| **FS drag** `FSDragCoefficientModels/` Churchill, Colebrook, Engel, modifiedEngel, BaxiDalleDonne, NoKazimi, Rehme, ReynoldsPower (8) | 2,233 | `closures/fs_drag/mod.rs` | ported (all 8) | unit (6); **upstream run**: `ReynoldsPower` on PSBT, 836 cells, 6.15e-6 (`tests/genfoam_psbt_closures.rs`) |
| `FSDragFactor.{C,H}` (`Kd` tensor assembly) | 478 | `solver/porous_drag.rs` | partial: isotropic branch only; anisotropic `localDhAnisotropy`/local frames missing | unit (1) |
| **FF drag** `FFDragCoefficientModels/` Autruffe, Bestion, BestionTRACE, NoKazimi, SchillerNaumann, Wallis (6) | 1,678 | `closures/ff_drag/interfacial.rs` | ported (all 6, per-cell `Cd`) | unit (in `ff_drag/tests.rs`) |
| `FFDragFactor.{C,H}` (`Kd = 0.75 Cd ρ_c |Ur| α_d / D` assembly) | 240 | none before gh:#1218 | missing | — |
| **two-phase multipliers** ChenKalish, Kaiser74, Kaiser88, KottowskiSavatteri, LockhartMartinelli, LottesFlinn, LottesFlinnNguyen, constant (8) | 2,102 | `closures/ff_drag/multipliers.rs` | ported (all 8) | unit (11 in `ff_drag/tests.rs`) |
| `twoPhaseDragFactor.{C,H}` (applies the multiplier to `Kd`) | 251 | none before gh:#1218 | missing | — |
| **FS heat transfer** Nusselt, NusseltAndWall, Shah, Gorenflo | 1,193 | `closures/heat_transfer/fs_htc.rs` | ported | unit (34 in `heat_transfer/tests.rs`, shared) |
| `multiRegimeBoiling` | 588 | `closures/heat_transfer/boiling.rs` | ported (post-CHF branch absent, as upstream: `TCHFi = 1e69`) | unit |
| `superpositionNucleateBoiling` | 252 | `closures/heat_transfer/boiling.rs` | ported (htc combiner) | unit |
| `multiRegimeBoilingTRACE` | 454 | — | missing (deferred in `heat_transfer/mod.rs`) | — |
| `multiRegimeBoilingTRACECHF` | 684 | — | missing | — |
| `multiRegimeBoilingVapourTRACE` | 304 | — | missing | — |
| `NusseltWallAndHfromFMU` | 327 | — | missing (FMU co-simulation, out of scope) | — |
| CHF `constantCHF` | 202 | `closures/heat_transfer/chf.rs` | ported | unit |
| CHF `lookUpTableCHF` | 366 | — | missing | — |
| Leidenfrost `GroeneveldStewart` | 220 | `chf.rs` | ported | unit |
| TONB `Basu` | 245 | `chf.rs` | ported | unit |
| flow enhancement Chen, COBRA-TF, RezkallahSims | 641 | `chf.rs` | ported | unit |
| suppression Chen, COBRA-TF | 434 | `chf.rs` | ported | unit |
| sub-cooled boiling fraction SahaZuber, constant | 442 | `chf.rs` | ported | unit |
| post-CHF `CachardLiquid`, `CachardVapour` | 560 | — | missing. The port's stated reason was checked against upstream and holds: `CachardLiquidFSHeatTransferCoefficient.C:124` binds `pi` to `p_[celli]` (pressure) and uses it as a pitch in `pow(pi/max(DRi,1e-6),2)` (line 136) | — |
| **FF heat transfer** Nusselt | 269 | `closures/heat_transfer/ff_htc.rs` | ported | unit |
| FF `NoKazimi` | 258 | — | missing (deferred in `ff_htc.rs`) | — |
| FF `constant` (templated) | — | `ff_htc.rs` / templated | ported as a value | unit |
| **phase change** saturation constantTemperature, water, waterTRACE, BrowningPotter | 1,448 | `closures/phase_change/saturation.rs` | ported (all 4) | unit (18 in `phase_change/tests.rs`, shared) |
| latent heat FinkLeibowitz, water, fromThermophysicalProperties | 997 | `closures/phase_change/latent_heat.rs` | partial: values ported, `latentHeatModel::adjust()` (enthalpy-consistent `L`) missing; 1D_boiling sets `adjust true` | unit |
| rate `heatDriven` (4 modes), `forcedConstant` | 579 | `closures/phase_change/rate.rs` | ported as algebra | unit |
| `phaseChangeModel.{C,H}` (orchestration: interfacial temperature, `limitMassTransfer`, `dmdtW` wall split, `heSources`) | 925 | none before gh:#1218 | missing | — |
| **interfacial** area spherical, Schor, annular, NoKazimi | 1,347 | `closures/interfacial/area.rs` | ported | unit (15 in `interfacial/tests.rs`, shared) |
| fluid diameter WallisFilm, isomolarBubble, isothermalBubble, pipeFilm | 1,218 | `closures/interfacial/diameter.rs` | ported | unit |
| virtual mass coefficient constant | 630 | `closures/interfacial/virtual_mass.rs` | ported | unit |
| dispersion constant | 524 | `closures/interfacial/dispersion.rs` | ported | unit |
| contact partition complementary, linear | 788 | `closures/interfacial/contact_partition.rs` | ported | unit |
| regime map `oneParameter` | 1,115 | `closures/interfacial/regime_map.rs` | ported | unit |
| regime map `twoParameters` (`regimeDomain2D`, `regimeBoundary2D`) | 1,501 | — | missing (deferred in `regime_map.rs`) | — |
| templated `byRegime` dispatch | 428 | — | missing (deferred in `regime_map.rs`) | — |
| templated `constant` | 346 | folded into each family's `Constant` variant | ported | unit |
| `phasePairs/FFPair.{C,H}` | 1,129 | `phase/phase_pair.rs` (`ff_reynolds`, `ff_relative_velocity_magnitude`) | partial: kinematics only | unit (3) |
| `phasePairs/FSPair.{C,H}` | 738 | `phase/phase_pair.rs` (`fs_reynolds`) | partial: kinematics only | unit |
| **turbulence** porousKEpsilon, porousKEpsilon2PhaseCorrected, LaheyKEpsilon, mixtureKEpsilon | 3,663 | `closures/turbulence/*.rs` | partial: closure algebra only; k/ε transport and `correctNut` missing | unit (12) |

## 3. `thermalHydraulics/src/phaseModels`

| upstream | lines | port | status | tests / V&V |
|---|---:|---|---|---|
| `phaseBase.{C,H}` | 242 | `phase/phase_base.rs` | ported | via `phase/fluid/tests.rs` |
| `fluid/fluid.{C,H}` + turbulence fwd headers | 1,520 | `phase/fluid/mod.rs` | partial: field bag ported; `fluid::correct` thermo update, `initAlphaPhis`, `constructDiameterModel`, the `thermoResidualAlpha` energy switch logic not wired to a thermo package | unit (7) |
| `structure.{C,H}` | 1,689 | `structure/mod.rs`, `structure/heat_source.rs` | partial: per-cell algebra; anisotropic tensors, local frames, `fvm::Sp` semi-implicit source matrix missing | unit (3+3) |
| `byZone/byZoneStructure` | 711 | — | missing | — |
| `heatExchanger` | 587 | `structure/heat_exchanger.rs` | ported (per-cell two-sided solve); mesh-to-mesh mapping missing | unit (3) |
| `pump` | 297 | `structure/pump.rs` | ported | unit (2) |
| `powerOffCriterionModels` timer, fieldValue | 856 | `structure/power_off.rs` | ported | unit (4) |
| power `fixedPower` | 518 | `structure/power_model.rs` | ported | unit (5, shared) |
| power `fixedTemperature` | 395 | `structure/power_model.rs` | ported | unit |
| power `nuclearSteadyStatePebble` | 956 | `structure/nuclear_steady_state_pebble/` | ported (not a `PowerModel` variant) | **upstream run**: gFHR, ≤ 7.2e-6 over 292,500 cells |
| power `heatedPin` | 824 | — | missing | — |
| power `nuclearFuelPin` | 1,534 | — | missing | — |
| power `interpolatedNuclearFuelPin` | 353 | — | missing | — |
| power `lumpedNuclearStructure` | 759 | — | missing | — |
| power `nuclearFuelPinTest` | 1,418 | — | missing (upstream test variant) | — |
| power `fixedTemperatureFMU`, `nuclearFuelFMU` | 1,892 | — | missing (FMU co-simulation, out of scope) | — |
| `powerModels/powerModel.{C,H}` base | 529 | `structure/power_model.rs` (enum) | ported | unit |

## 4. `thermalHydraulics/src`, other

| upstream | lines | port | status | tests / V&V |
|---|---:|---|---|---|
| `boundaryConditions/blackBodyRadiation` | 424 | `boundary_conditions/blackbody_radiation.rs` | ported | unit (4) |
| `boundaryConditions/timeFieldTable` | 434 | `boundary_conditions/time_field_table.rs` | ported | unit (6) |
| `boundaryConditions/velocityRundown` | 367 | `boundary_conditions/velocity_rundown.rs` | ported | unit (3) |
| `boundaryConditions/NusseltThermalBaffle1D` | 1,190 | `boundary_conditions/nusselt_baffle.rs` | partial: scaffold, every method `unimplemented!()` | none |
| `functionObjects/` TBulk, fieldDiffExtents, fieldIntegralToFMU, massFlow, patchScalarFieldValue, pressureDrop, stopIfMaxFieldDiff | 3,062 | `function_objects/*.rs` | ported (FMU export itself out of scope) | unit (16) |
| `thermophysicalProperties/H2` | 1,415 | `thermophysical/hydrogen/` | ported | unit (9) |
| `customPimpleControl/` | 615 | — | missing (`correctUntilConvergence`, `useFirstPISOInitialResidual` residual control) | — |
| `include/IOFieldField` + helpers | 926 | — | missing (infrastructure; folds into I/O) | — |

## 5. `neutronics`

| upstream | lines | port | status | tests / V&V |
|---|---:|---|---|---|
| `neutronics.{C,H}` base + `include/` | 827 | `neutronics/mod.rs` (`NeutronicsModel` enum), `state.rs` | ported | unit (5) |
| `diffusion/` | 1,218 | `neutronics/diffusion/` incl. `precursor_drift/` | ported | unit (9); **upstream run**: MSFR +0.3 pcm, ESFR +60.7 pcm (`tests/genfoam_tutorial_keff.rs`); **real binary, guided slabs, gh:#1361 (2026-11-12)**: k_eff 6e-14 and flux per cell 6e-13 from upstream at tight tolerances (`tests/genfoam_neutronics_vs_upstream.rs`) |
| `SP3/` | 1,140 | `neutronics/sp3/` | ported | unit (5); **real binary, guided slabs, gh:#1361 (2026-11-12)**: k_eff 1.7e-13, flux per cell 2.3e-12 (SP3 ~~"unit (5)" only~~ CORRECTED, it had not been compared with an upstream run before) |
| `SN/` | 1,323 | `neutronics/sn/` | partial: eigenvalue only, transient missing. ~~higher moments missing~~ **CORRECTED 2026-11-12** (gh:#1474): `sn/pseudo_time.rs` + `sn/quadrature_set.rs` port the case `quadratureSet` reader, P1..P7 scattering and upstream's per-step eigenvalue iteration (`SnNeutronics` itself is unchanged) | unit (6); `tests/genfoam_sn_vs_upstream.rs`: small S_N slab with Godiva's quadrature and P0..P3 scattering, step by step vs the real binary to 1e-14 (README of `verification_and_validation/genfoam_vs_upstream_gh1361`); full Godiva_SN, upstream ~7.5 min per step, 2.9 GB: see that README |
| `albedoSP3/` | 642 | `neutronics/albedo/` | ported | unit (5); upstream run (MSFR boundary rows, ≤ 0.6 pcm) |
| `pointKinetics/` | 2,391 | `neutronics/point_kinetics/` | ported | unit (5); inhour test |
| `XS/` + `nuclearDataOneEnergy` | 2,722 | `neutronics/xs/` | partial: discontinuity-factor adjustment deferred | unit (13) |
| `adjointDiffusion/` | 849 | — | missing | — |

## 6. `thermoMechanics`, `multiRegion`, `common`, others

| upstream | lines | port | status | tests / V&V |
|---|---:|---|---|---|
| `thermoMechanics.{C,H}` + `legacyThermoMechanics/` | 1,424 | `thermo_mechanics/` | partial: isotropic stiffness, no traction BC, no non-orthogonal correction | unit (16) |
| `extendedThermoMechanics/` | 455 | — | missing (also excluded from the upstream build: needs the lost `offbeat` API) | — |
| `tractionDisplacement/` (`GFTractionDisplacement`, `gapContact`) | 1,035 | — | missing | — |
| `multiRegion/meshHandler` | 1,194 | `multi_region/coupling_fields.rs`, `mesh_to_mesh.rs`, `rbf_mapping.rs` | partial: no `movePoints` mesh deformation | unit (15) |
| `multiRegion/multiPhysicsSolver` | 460 | `multi_region/outer_iteration.rs` | partial | unit (3) |
| `multiRegion/regionSolvers` | 726 | `multi_region/outer_iteration.rs`, `mesh_region.rs` (`RegionKernel`) | partial: the TH region kernel is a lumped stand-in (`MeshThermalHydraulics`), not `onePhase`/`twoPhase` | unit (6) |
| `common/radialBasisFunctionInterpolation` | 1,090 | `common/rbf.rs`, `multi_region/rbf_mapping.rs` | ported | unit; upstream run (MSFR XS parametrisation, 14 pcm); **libCommon driver, gh:#1361 (2026-11-12)**: inverse, weights and values bit-identical to GeN-Foam `libCommon` on 34 cases (weights were NOT before: the solve is now `inv(A) * v` as upstream) |
| `common/timeProfile` | 397 | `common/time_profile.rs` | ported (`commDataLayer` out of scope) | unit (8); **libCommon driver, gh:#1361 (2026-11-12)**: table values bit-identical to upstream on 712 points (ESI `Function1::Table` form; the Foundation form is kept as `value_foundation` and differed in 242 of 667 interior points, up to 24 ulp) |
| `common/InterpolateTable` | 675 | `common/interpolate_table.rs` | ported (1-D); `FieldField` 2-D/3-D instantiations deferred | unit (9); **libCommon driver, gh:#1361 (2026-11-12)**: 618 values, 90 integrals and 504 `getNextPoint` bit-identical to upstream |
| `common/solver` | 457 | enum dispatch in each module | partial (by design) | — |
| `common/mergeOrSplitBaffles` | 608 | — | missing | — |
| `common/latticeMap` | 315 | — | missing | — |
| `common/listOperation` | 154 | — | missing (dictionary I/O helper) | — |
| `openFoamImportedSolvers/compressibleInterFoam` | 1,512 | — | missing | — |
| `openFoamImportedSolvers/scalarTransportFoam` | 374 | — | missing | — |
| `offbeat/solvers/fuelBehaviour` | 425 | — | missing (couples to the external OFFBEAT library; out of scope) | — |

## The thermophysical layer the two-phase tutorials select

Checked because gh:#1218 asked for IF97 *or* upstream's own layer, whichever
the tutorials select. **They select OpenFOAM's `heRhoThermo`, not IF97:** every
two-phase feature case (1D_boiling, 1D_PSBT_SC, 1D_CHF, 2D_KNS37-L22,
2D_voidMotionNoPhaseChange) uses `icoPolynomial` + `hPolynomial` + `polynomial`
transport for the liquid and `perfectGas` + `hConst` + `const` transport for the
vapour, including the water cases (PSBT liquid: `rhoCoeffs<8>`, `CpCoeffs<8>`;
vapour: `perfectGas`, `Cp 10000`). All six of those are already ported in
`outram-foam-basic-lib::thermophysics` (`eos::{IcoPolynomial, PerfectGas}`,
`thermo::{HPolynomialThermo, HConstThermo}`, `transport::{polynomial,
const_transport}`), so the two-phase port reuses them and does **not** depend on
`tampines-steam-tables`. IF97 enters upstream only through the `water` /
`waterTRACE` saturation and latent-heat closures, which are already ported as
fits in `closures/phase_change`.

## Doc claims checked during the audit

- `heat_transfer/mod.rs`, Cachard deferral reason: **verified** against
  upstream (see the post-CHF row).
- `docs/genfoam-port-plan.md`, "`thermalHydraulics` ~65k LOC in 351 files":
  re-measured as 65,550 lines (`src/` 58,230 + `solvers/` 7,320). Consistent.
- `power_model.rs` already carries a 2026-11-03 correction for
  `nuclearSteadyStatePebble`; re-checked, still true.
- `solver/mod.rs` and `thermal_hydraulics/mod.rs` said the two-phase solver is
  not implemented; corrected in the gh:#1218 change that adds it.

## Draft child issues (for the maintainer to file under epic gh:#1216)

Each is one deliverable. Sizes are upstream lines to port.

1. **twoPhase solver: MULES alpha transport** (`myMULES` + `solveAlphaMULES.H`,
   1,765 lines). Port GeN-Foam's `myMULES` limiter and `solveAlphaMULES`
   (sub-cycling, `alphaMaxCo`, structure-aware bounds) as a second
   `AlphaSolver` variant beside `implicitUpwind`. Verify on 1D_boiling and
   2D_voidMotionNoPhaseChange against an upstream run. Blocked by gh:#1218.
2. **twoPhase solver: cell-centred momentum (`pEqn_2p.H`) and the
   `explicit`/`none` partial-elimination modes** (~500 lines). gh:#1218 ports
   the face-centred path only.
3. **FFPair/FSPair field assembly to parity** (1,867 lines): interface
   temperature relaxation (`maxTInterfaceDdt`), virtual mass, turbulent
   dispersion, `residualKd`, the `minMagU` floors, anisotropic FS `Kd`.
4. **`multiRegimeBoilingTRACE` / `TRACECHF` / `VapourTRACE`** (1,442 lines). Needed
   by every PSBT case. Verify against the committed PSBT upstream fields.
5. **`lookUpTableCHF`** (366 lines) with the 2006 Groeneveld table data
   (licence to be checked). Needed by PSBT Phase II Ex 2 and 3.
6. **Post-CHF `CachardLiquid`/`CachardVapour`** (560 lines): port with
   upstream's pressure-as-pitch binding reproduced and flagged, or report the
   upstream defect.
7. **`regimeMapModels/twoParameters`** (1,501 lines). Needed by PSBT.
8. **`templatedModels/byRegime` dispatch** (428 lines) for diameter,
   dispersion, contact partition, area. 1D_boiling and PSBT both use it.
9. **`phaseChangeModel` orchestration to parity** (925 lines): `dmdtW` wall
   boiling split, `limitMassTransfer`, `latentHeatModel::adjust()`.
10. **FF heat transfer `NoKazimi`** (258 lines).
11. **Turbulence transport**: k/ε equations and `correctNut` for the four
    porous/two-phase models (closure algebra exists; ~2,000 lines of the 3,663
    are field orchestration).
12. **`structure` field layer and `byZone`** (1,892 lines): anisotropic
    tensors, local frames, semi-implicit source, zone dispatch.
13. **Radial-conduction power models**: `heatedPin`, `nuclearFuelPin`,
    `interpolatedNuclearFuelPin`, `lumpedNuclearStructure` (3,470 lines).
14. **`onePhase` to parity**: variable properties through basic-lib thermo,
    `p_rgh`, `pEqnf_1p` face-centred momentum; and port `onePhaseLegacy`
    (985 lines).
15. **`NusseltThermalBaffle1D`** (1,190 lines; currently `unimplemented!()`).
16. **`customPimpleControl`** (615 lines): `correctUntilConvergence` and
    residual control, shared by `onePhase` and `twoPhase`.
17. **`adjointDiffusion`** (849 lines) and the SN transient.
18. **thermoMechanics**: traction / gap-contact BCs (1,035 lines).
19. **`common`**: `mergeOrSplitBaffles`, `latticeMap` (923 lines), when the
    multi-region layer needs them.
20. **`openFoamImportedSolvers`**: `compressibleInterFoam`, `scalarTransportFoam`
    (1,884 lines); check first whether the workspace's own ports cover them.
