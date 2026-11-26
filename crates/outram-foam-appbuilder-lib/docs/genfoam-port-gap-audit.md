# GeN-Foam port gap audit (gh:#1217, epic gh:#1216)

**Taken 2026-11-04** against upstream GeN-Foam commit `652b3da` (the commit
every port header cites; it is the archived tip, "docs: Add archive banner"),
fetched shallow and sparse from `gitlab.com/foam-for-nuclear/GeN-Foam`. Port
side: `crates/outram-foam-appbuilder-lib/src/genfoam/` at branch
`claude/outram-foam-8ookor` merged with `develop` (merge `270705faf`).

> **Update, same day, after gh:#1218.** The tables below are the state
> *before* the two-phase port, kept as audited. `solver/two_phase/` now
> covers, for the selections `1D_boiling` makes (compared with the real binary since 2026-11-12, gh:#1361 round 3: `verification_and_validation/genfoam_vs_upstream_gh1361/README.md`): `twoPhase.{C,H}`,
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
> *partial*. Still missing in the solver: ~~MULES~~ (**CORRECTED 2026-11-26** (gh:#1361 round 7): ported, `solvePhase both`), `pEqn_2p.H` (cell-centred),
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
7. ~~**`MULES`** (`myMULES` 1,225 + `solveAlphaMULES` 540): the alpha solver
   1D_boiling, 1D_CHF, 2D_voidMotionNoPhaseChange and 2D_KNS37-L22 select.~~ **CORRECTED 2026-11-26** (gh:#1361 round 7): ported for
   `solvePhase both` (what 1D_boiling selects); the `solvePhase vapour` branch (`solveAlpha` lambda, used by
   1D_HX/twoPhase and 2D_KNS37-L22) is not.

## 1. `thermalHydraulics/solvers`

| upstream | lines | port | status | tests / V&V |
|---|---:|---|---|---|
| `thermalHydraulicsModel.{C,H}` (region base: PIMPLE, `p`/`p_rgh`, `g`, `hRef`, power coupling fields) | 745 | `solver/mod.rs` (`ThermalHydraulicsSolver` enum), fields inlined in `one_phase.rs` | partial | none of its own |
| `onePhase/` (`onePhase.{C,H}`, `UEqn_1p`, `pEqn_1p`, `pEqnf_1p`, `EEqn_1p`, 7 `pEqnRoutines`) | 1,022 | `solver/one_phase.rs` (661), `solver/porous_drag.rs` (230) | partial: `one_phase.rs` is the older constant-property driver (`he = Cp T`, isotropic `Kd`, no `p_rgh`/gravity split, no `pEqnf`, no `fvOptions`), kept as it was. ~~no `pEqnf` face-centred momentum~~ **CORRECTED 2026-11-12 (gh:#1361 round 4):** `solver/two_phase/one_phase_fc.rs` (translation of `onePhase.C`, `UEqn_1p`, `pEqnf_1p` + routines, `EEqn_1p`, `FSPair::correct`, `customPimpleControl::loop`; heRhoThermo packages, `p_rgh`/gravity, `contErr`, kinetic energy; NOT ported: other momentum modes, turbulence, heat exchangers, pumps, tortuosity, non-`zeroGradient` U, closed volumes) runs the 40-cell channel derived from 1D_boiling against the real binary to 1e-13 over 108 steps. **ADDED 2026-11-26 (round 9, issues 1773/1775):** `solver/two_phase/one_phase_cc.rs` is the cell-centred `solveFluidMechanics false` energy-only region (Boussinesq `hConst`, `fixedTemperature`, constant htc, frozen flux, `alphat` read, `turbulence->correct()` never runs in it): the 2D_MSFR coupled transient agrees with the real binary over 50 steps (T 2.5e-14); UEqn/pEqn with `momentumMode cellCentered` and `porousKEpsilon` SOLVING are still missing. **ADDED 2026-11-26 (round 18, gh:#1812):** the upstream `steadyStateTH` reference protocol (`scripts/run_msfr_th_tight.sh`, 14 steps, turbulence on/off) and committed predictions are in README round 18; the `wedge` VECTOR patch field (`fv::VectorBc::Wedge`, `wedge_velocity_bc`, `rotation_tensor`: ESI `wedgeFvPatchField`/`wedgePolyPatch`, a building block `UEqn_1p` needs on the 2D_MSFR mesh) is ported and unit-tested analytically; still missing for the stage: `UEqn_1p`/`pEqn_1p` assembly, one-phase `porousKEpsilon` solving, the `kqR`/`epsilon`/`nutk` wall-function PATCHES that act on fields (none exist in the workspace), `ReynoldsPower` `Kd` in the momentum equation, the pump `momentumSource` in `phiSbyA`. ~~one-phase `porousKEpsilon` solving and the wall-function PATCHES missing~~ **CORRECTED 2026-11-26 (round 19, gh:#1837):** `UEqn_1p`/`pEqn_1p` with `turbulence false` are ported (`one_phase_cc/flow.rs`, 3 steps to <= 1.4e-11). **ADDED 2026-11-26 (round 20, gh:#1838/#1839):** `one_phase_cc/turbulence.rs` ports single-phase `porousKEpsilon::correct()` with `kqRWallFunction`, `epsilonWallFunction` (cell weights, `G`/`epsilon` overwrite, `setValues`), `nutkWallFunction` (stepwise blender) and `nearWallDist`; `turbulence true` runs and agrees with the real binary for all nine fields in steps 1-3 (<= 6e-13), at steps 6, 9, 14 (<= 6e-13) and in every `k`/`epsilon` solve of 14 steps (residual 1e-11, iterations within 2). Refused by name: `DhStruct`, wall-function entries beyond `type`/`value`, other blenders. `EigenvalueStage::SteadyStateTh` is still refused (gh:#1841) | unit: `solver/tests.rs` (6); `tests/genfoam_one_phase_vs_upstream.rs` (vs the real binary) |
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
| `multiRegimeBoilingTRACE` | 454 | `closures/heat_transfer/boiling_trace.rs`; wired in `solver/two_phase` (`FsHtcModel::MultiRegimeBoilingTrace`, `dmdtW`) | ported (CORRECTED 2026-11-26, gh:#1222; ~~missing~~) | unit (8, shared with TRACECHF); **upstream run** on the closed `Phase_Ex1_12223` variant: steps 1-8 agree to round-off, the nucleate-boiling branch differs by 7.8e-9 (step 9) to 6e-7 (step 12), cause open (`tests/genfoam_psbt_trace_vs_upstream.rs`, README "Round 8") |
| `multiRegimeBoilingTRACECHF` | 684 | `closures/heat_transfer/boiling_trace_chf.rs`; wired in `solver/two_phase` (`FsHtcModel::MultiRegimeBoilingTraceChf`, fields `chf_bool`, `wf_transition_boiling`, `t_leidenfrost`, `dmdtW`) | ported and wired (**CORRECTED 2026-11-26**, gh:#1223/#1224/#1361 round 10; ~~pure function ported, NOT wired: its post-CHF branches need `CachardLiquid` (missing) and the Ex2/Ex3 cases `lookUpTableCHF` (deferred)~~) | unit (8, shared with TRACE); **upstream run** on closed `1D_CHF` variants (`tests/genfoam_chf_trace_vs_upstream.rs`): `constant_chf` agrees to <= 4.3e-5 over steps 1-40 (transition then film boiling), parts at a threshold event at step 41; `lookup_table_chf` to <= 8e-4 over steps 1-3, parts at step 4 (README "Round 10") |
| `multiRegimeBoilingVapourTRACE` | 304 | `closures/heat_transfer/boiling_trace_chf.rs`; wired (`FsHtcModel::MultiRegimeBoilingVapourTrace`) | ported and wired (**CORRECTED 2026-11-26**, gh:#1224; ~~not wired~~) | unit; upstream run: `htc.vapour.structure` in `lookup_table_chf` (contact partition 0.7) to 4.3e-4 over steps 1-3 |
| `NusseltWallAndHfromFMU` | 327 | — | missing (FMU co-simulation, out of scope) | — |
| CHF `constantCHF` | 202 | `closures/heat_transfer/chf.rs` | ported | unit |
| CHF `lookUpTableCHF` | 366 | `closures/heat_transfer/chf_table.rs` on `common/interpolate_table_nd.rs` | ported (**CORRECTED 2026-11-26**, gh:#1223, maintainer decision of the same day: the table is GeN-Foam's own GPL-3.0 case file, ported with citation, `tests/data/genfoam_chf_gh1223/PROVENANCE.md`; ~~missing: deferred to the maintainer, no licence statement~~). **The premise that `1D_CHF` uses it was wrong:** `1D_CHF` selects `constantCHF`; only `PhaseII_Ex2/Ex3` select the table | unit (8, incl. the shipped table: knots, midpoints, K1/K2, out-of-range); upstream run only through a `lookUpTableCHF` variant of `1D_CHF` (a modified case, README "Round 10") |
| Leidenfrost `GroeneveldStewart` | 220 | `chf.rs` | ported | unit |
| TONB `Basu` | 245 | `chf.rs` | ported | unit |
| flow enhancement Chen, COBRA-TF, RezkallahSims | 641 | `chf.rs` | ported | unit |
| suppression Chen, COBRA-TF | 434 | `chf.rs` | ported | unit |
| sub-cooled boiling fraction SahaZuber, constant | 442 | `chf.rs` | ported | unit |
| post-CHF `CachardLiquid`, `CachardVapour` | 560 | `closures/heat_transfer/cachard.rs` | ported (**CORRECTED 2026-11-26**, gh:#1224; ~~missing~~). The upstream defect was re-read and holds: `CachardLiquidFSHeatTransferCoefficient.C:124` and `CachardVapourFSHeatTransferCoefficient.C:101` bind `pi` to `p_[celli]` (pressure) and use it as a pitch in `pow(pi/max(DRi,1e-6),2)`. **Default = upstream, bit for bit** (the film is ~1e9 m thick, so `h_gamma` ~ 1e-11 and the post-CHF coefficient is radiation); the intended pitch is the named option `CachardPitch::Pitch`; the maintainer decides which is the default (gh:#1224) | unit; upstream run (both models, through TRACECHF/VapourTRACE) |
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
| regime map `twoParameters` (`regimeDomain2D`, `regimeBoundary2D`) | 1,501 | `closures/interfacial/regime_map_2d.rs` | ported (CORRECTED 2026-11-26, gh:#1225; ~~missing~~): incl. `Foam::Random`, `hashlittle` and `HashTable` walk order | unit (7; lookup3 vectors, g++ draws, PSBT map, concave polygon vs winding number); in the upstream comparison via the closed PSBT case |
| templated `byRegime` dispatch | 428 | `closures/interfacial/by_regime.rs` (`ByRegimeModel<M>`); wired for dispersion, contact partition, dispersed diameter, interfacial-area density | ported for those four (CORRECTED 2026-11-26, gh:#1225); drag, virtual-mass, multiplier and htc `byRegime` not wired | unit; upstream run (closed PSBT) |
| templated `constant` | 346 | folded into each family's `Constant` variant | ported | unit |
| `phasePairs/FFPair.{C,H}` | 1,129 | `phase/phase_pair.rs` (`ff_reynolds`, `ff_relative_velocity_magnitude`) | partial: kinematics only | unit (3) |
| `phasePairs/FSPair.{C,H}` | 738 | `phase/phase_pair.rs` (`fs_reynolds`) | partial: kinematics only | unit |
| **turbulence** porousKEpsilon, porousKEpsilon2PhaseCorrected, LaheyKEpsilon, mixtureKEpsilon | 3,663 | `closures/turbulence/*.rs` | partial: closure algebra only; k/ε transport and `correctNut` missing. **ADDED 2026-11-26 (gh:#1802, README round 16):** `solver/two_phase/turbulence.rs` solves `porousKEpsilon2PhaseCorrected` in `solver::two_phase` (`correct()`, `correctNut`, ESI `bound`, `fvMatrix::relax`; feeds `nuEff` to the momentum and `alphaEff` to the energy equation); it is NOT a derivation of `porousKEpsilon` (it derives from `eddyViscosity<RASModel>`), ~~so the single-phase `porousKEpsilon` (gh:#1775/#1812) is still missing~~ **CORRECTED 2026-11-26 (round 20):** the single-phase `porousKEpsilon` is ported in `solver/two_phase/one_phase_cc/turbulence.rs` (see the `onePhase/` row); `LaheyKEpsilon`, `mixtureKEpsilon` are still missing, as are `LaheyKEpsilon`, `mixtureKEpsilon`, plain `kEpsilon`, `nutStabilization` (`DhStruct`) ~~and wall-function patches (refused by name)~~ **CORRECTED 2026-11-26 (round 20):** wall functions exist for the single-phase region only; the two-phase reader still refuses them by name. Against the real binary on `PhaseII_Ex1` the turbulence solves of the first outer iteration agree with upstream's to the printed residuals and Ex3's one-iteration step 1 to 6e-16, but the flow differs by a baseline (`p` 8e-7 at step 1, 2e-5 at step 25) with `turbulence off` as well: not the model's | unit (12 + 4: regex, `bound`, `relax`, `G`); `tests/genfoam_psbt2_turbulence_vs_upstream.rs` (vs the real binary) |

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
| power `fixedPower` | 518 | `structure/power_model.rs` | ported; **`powerTimeProfile` ADDED 2026-11-26 (gh:#1815)** as upstream's recursion `pd(t) = prof(t) pdOld / prof(t - dt)` (the closed form `initial prof(t)` the port used before is `with_closed_form_power_profile`, deprecated); wired in the two-phase case reader (`type table` only) | unit (6, shared) |
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
| `diffusion/` | 1,218 | `neutronics/diffusion/` incl. `precursor_drift/` | ported; ~~transient: single pass, no neutron iteration~~ **CORRECTED 2026-11-25** (gh:#1361 round 6): `upstream_transient.rs` translates `solveNeutronics.H` for `eigenvalueNeutronics false`, `liquidFuel true` (neutron iteration, Gauss-Seidel over groups, `precEq.H` with `ddt`, transport and diffusion); `DiffusionNeutronics::step` is unchanged and remains the simple form. Aitken acceleration not ported (upstream performs it 0 times, see the module doc). **ADDED 2026-11-26 (gh:#1814, round 11):** `upstream_eigenvalue.rs` translates the `eigenvalueNeutronics true`, `liquidFuel true` step (`PseudoTimeError::LiquidFuel` still refuses the OLD stationary-fuel driver for circulating fuel; the new step is the one `steadyStateEN` uses); 600 steps of `2D_MSFR` `steadyStateEN` against the real binary: keff 2.2e-13, flux 1.3e-10, precStar 8e-10 (`tests/genfoam_msfr_chain_vs_upstream.rs`) | unit (9); **upstream run**: MSFR ~~+0.3 pcm~~ (a static `k_eff` against a creeping pseudo-time `keff_` resolves no better than the creep, README round 6), ESFR +60.7 pcm (`tests/genfoam_tutorial_keff.rs`); **real binary, `2D_MSFR` transient, 50 steps, gh:#1361 round 6 (2026-11-25)**: power 6.2e-14, iteration counts 50/50, precursor totals 1.3e-11 from upstream's own TH state (`tests/genfoam_msfr_transient_vs_upstream.rs`); **real binary, guided slabs, gh:#1361 (2026-11-12)**: k_eff 6e-14 and flux per cell 6e-13 from upstream at tight tolerances (`tests/genfoam_neutronics_vs_upstream.rs`) |
| `SP3/` | 1,140 | `neutronics/sp3/` | ported | unit (5); **real binary, guided slabs, gh:#1361 (2026-11-12)**: k_eff 1.7e-13, flux per cell 2.3e-12 (SP3 ~~"unit (5)" only~~ CORRECTED, it had not been compared with an upstream run before) |
| `SN/` | 1,323 | `neutronics/sn/` | partial: eigenvalue only, transient missing. ~~higher moments missing~~ **CORRECTED 2026-11-12** (gh:#1474): `sn/pseudo_time.rs` + `sn/quadrature_set.rs` port the case `quadratureSet` reader, P1..P7 scattering and upstream's per-step eigenvalue iteration (`SnNeutronics` itself is unchanged) | unit (6); `tests/genfoam_sn_vs_upstream.rs`: small S_N slab with Godiva's quadrature and P0..P3 scattering, step by step vs the real binary to 1e-14 (README of `verification_and_validation/genfoam_vs_upstream_gh1361`); full Godiva_SN, upstream ~7.5 min per step, 2.9 GB: see that README |
| `albedoSP3/` | 642 | `neutronics/albedo/` | ported | unit (5); upstream run (MSFR boundary rows, ≤ 0.6 pcm) |
| `pointKinetics/` | 2,391 | `neutronics/point_kinetics/` | ported | unit (5); inhour test |
| `XS/` + `nuclearDataOneEnergy` | 2,722 | `neutronics/xs/` | partial: discontinuity-factor adjustment deferred. gh:#1361 round 6: per-cell cross sections against GeN-Foam's own `XS` class on `2D_MSFR` (`upstream_driver/dump_xs`): `D`, `nuSigmaEff`, `sigmaPow`, `sigmaRemoval`, `sigmaFromTo`, `IV`, `BetaTot` bit-identical; **`chiPrompt`/`chiDelayed` are interpolated here but never in upstream** (`XS::init()` does not `build()` them): `CrossSectionData::with_upstream_unparametrised_chi()`, default unchanged, default question deferred to the maintainer | unit (14) |
| `adjointDiffusion/` | 849 | — | missing | — |

## 6. `thermoMechanics`, `multiRegion`, `common`, others

| upstream | lines | port | status | tests / V&V |
|---|---:|---|---|---|
| `thermoMechanics.{C,H}` + `legacyThermoMechanics/` | 1,424 | `thermo_mechanics/` (`MechanicsMeshSolver`, the older solver) and `thermo_mechanics/legacy_fvm/` (`LegacyFvmThermoMechanics`) | ~~partial: isotropic stiffness, no traction BC, no non-orthogonal correction~~ **CORRECTED 2026-11-26 (gh:#1781, round 23; read from `legacyThermoMechanics.C`, then measured):** upstream's legacy class is ITSELF isotropic (scalar `mu_`, `lambda_`, `threeK_` volFields; no stiffness tensor), and non-orthogonal correction is the `Gauss linear corrected` Laplacian scheme the case selects, not a model feature; both were ported as part of `legacy_fvm`: per-zone field-valued properties from `cellZones`, `TStructFromTH`/`isPorous` forcing with the row-scaled asymmetric `TEqn` (`PBiCG`/`DILU` ported), `compactNormalStress yes` with `leastSquares` or `Gauss linear` gradients and the explicit non-orthogonal correction, `fuelDispEqn`/`CRDispEqn` (`Gauss upwind` along `pinDirection`) and `meshDisp`. **Still missing:** `GFTractionDisplacement`/`gapContact` patches (gh:#1235), `compactNormalStress no` and transient `d2dt2` (gh:#1845), ~~`deformMesh` -> `movePoints` (gh:#1784)~~ (ported in round 26, `multi_region/mesh_deformation.rs`; not yet driven by the coupled run, gh:#1846), `GAMG` (named substitute; gh:#1843), the finite-element version (gh:#1782). | unit (22); **real class and real `GeN-Foam` executable, 3D_SmallESFR mechanics region, 23340 cells, 3 steps: every field bit-identical** (README round 23) |
| `extendedThermoMechanics/` | 455 | — | missing (also excluded from the upstream build: needs the lost `offbeat` API) | — |
| `tractionDisplacement/` (`GFTractionDisplacement`, `gapContact`) | 1,035 | `thermo_mechanics/legacy_fvm/traction.rs` (+ `DispPatch::GfTractionDisplacement`) | ~~missing~~ **CORRECTED 2026-11-26 (gh:#1235, round 24): `GFTractionDisplacement` (465 lines) ported and bit-identical to the real class (3 cases, 36/36 solves each); `gapContact` (570 lines): arithmetic ported (`GapContact`), unit-tested only, refused in cases; needs the `regionCoupled` coupling (gh:#1846)** | `verification_and_validation/genfoam_vs_upstream_gh1361/README.md`, "Round 24" |
| `multiRegion/meshHandler` | 1,194 | `multi_region/coupling_fields.rs`, `mesh_to_mesh.rs`, `rbf_mapping.rs` | partial: ~~no `movePoints` mesh deformation~~ **CORRECTED 2026-11-26 (gh:#1784, round 26; read from `meshHandler.C`, `meshHandlerTemplates.C` and the three `deformMesh()` bodies, then measured):** `meshHandler` itself has NO `movePoints` (it holds the meshes, `meshToMesh` objects and the `mapAllFields` loops); the deformation is `deformMesh()` of each region solver (`neutronics.C:363`, `thermalHydraulicsModel.C:380`, `thermoMechanics.C:353`), ported as `multi_region/mesh_deformation.rs` (`volPointInterpolation` + `fvMesh::movePoints` geometry; `meshDeformation` dictionary reader; refusals by name), compared with the real `volPointInterpolation`/`movePoints` and the real `legacyThermoMechanics::deformMesh()` (README round 26). **Still missing:** calling it from the coupled loop (regions own no `DeformableMesh`), `meshPhi`/`V0`/`V00` of a moving mesh (gh:#1846), `meshToMesh` `cellVolumeWeight` between different meshes (supermesh, gh:#651), `removeBaffles` copies, `createCouplingFields`. **ADDED 2026-11-26 (round 9, issue 1774):** `coupled_loop::IdenticalMeshMap` (`cellVolumeWeight` between two identical meshes: measured on upstream, identity with weight exactly 1.0; refuses a different mesh), `CouplingDict` (reader); the general supermesh is still missing (issue 651) | unit (15) ~~`meshToMesh` is nearest-cell + global rescale~~ **CORRECTED 2026-11-26 (gh:#651, round 28):** `cell_overlap.rs` is a line-by-line port of ESI `cellVolumeWeightMethod`/`tetOverlapVolume`/`meshToMesh::mapTgtToSrc`, bit-identical on the 3D_SmallESFR meshes (README Round 28); not yet wired into `IdenticalMeshMap` or the loop (gh:#1852). |
| `multiRegion/multiPhysicsSolver` | 460 | `multi_region/outer_iteration.rs` | partial. **ADDED 2026-11-26 (round 9):** `coupled_loop::correct_physics_loop` (literal `correctPhysics` loop), `adjust_delta_t`/`initial_delta_t` (`setDeltaT.C`), compared with the real binary on the 2D_MSFR transient | unit (3) |
| `Allrun` chain `steadyStateEN -> transient` (not a class) | n/a | `workflow/` | **ADDED 2026-11-26 (gh:#1814, round 11):** `EigenvalueStage`/`NormalisedState`/`TransientRun` (enum dispatch: diffusion and SP3 stationary fuel, circulating-fuel diffusion + energy-only `onePhase`); `ReactorState` reads/writes `uniform/reactorState`. ~~`steadyStateTH` (gh:#1812) is refused by name~~ **CORRECTED 2026-11-26 (round 21, gh:#1841):** `steadyStateTH` is ported: `workflow::steady_state_th` (`SteadyStateTh`: the `GeN-Foam.C` loop on the `Time` clock with the `adjustableRunTime` write alignment), `workflow::allrun::{RootCase, AllrunEdits}` (the `Allrun` edits as typed values; `steadyStateTH -> steadyStateEN -> transient` from `rootCase` with the 15 s fluid fields handed over in memory; `multiRegionCouplingDict_steadyStateTH` checked to map nothing); `p_rgh` `GAMG` runs as the NAMED deviation `GamgPolicy::SubstitutePcgDic` (basic-lib GAMG is not a translation, gh:#1843). SN and point kinetics are still refused by name | unit (5); **real binary, 2D_MSFR EN 600 steps then port's own transient 50 steps** (README round 11) |
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

1. ~~**twoPhase solver: MULES alpha transport**~~ (**CORRECTED 2026-11-26** (gh:#1361 round 7): done for `solvePhase both`, compared with the real binary
   on 1D_boiling as shipped) (`myMULES` + `solveAlphaMULES.H`,
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
4. ~~**`multiRegimeBoilingTRACE` / `TRACECHF` / `VapourTRACE`** (1,442 lines). Needed
   by every PSBT case. Verify against the committed PSBT upstream fields.~~ **CORRECTED 2026-11-26** (gh:#1222): all three are ported
   (`TRACE` also wired and compared with the real binary; `TRACECHF`/`VapourTRACE` as pure functions). The premise was wrong in two ways: only
   `Phase_Ex1_12223` selects `TRACE` (`PhaseII_Ex1_01_5215` selects `multiRegimeBoiling`; Ex2/Ex3 select `TRACECHF`), and no PSBT case runs end to
   ~~end in the port because every one is a through-flow case (velocity inlet, `pressureInletOutletVelocity`, `slip`, `inletOutlet`,
   `fixedFluxPressure`) and the three `PhaseII` cases use `porousKEpsilon2PhaseCorrected` turbulence. Not re-checked against the committed
   PSBT fields: those are a different (through-flow) case from the one compared.~~ **CORRECTED 2026-11-26** (gh:#1801, gh:#1361 round 12):
   the through-flow boundary conditions are ported (`fixedValue`/`slip`/`pressureInletOutletVelocity` velocity patches with `fixedValue`
   flux patches, `inletOutlet` phase fractions with implicitUpwind, `fixedFluxPressure` with `setSnGradP_rgh_2p.H`/`correctBoundaryFlux_2p.H`),
   and `Phase_Ex1_12223` runs AS SHIPPED, compared with the real binary step by step (agreement to 1e-8 through step 9, to the vapour onset at
   step 13 and on to step 50; the two runs separate at step 51, README "Round 12"). ~~The three `PhaseII` cases still need the two-phase
   `porousKEpsilon2PhaseCorrected` turbulence model (gh:#1802) and are refused by name.~~ **CORRECTED 2026-11-26 (round 16):** all three read and run (turbulence ported; plain `multiRegimeBoiling`, `symmetry` patches and `powerDensity.fixedPower` added); read in `tests/genfoam_psbt2_turbulence_vs_upstream.rs`. Every other velocity or scalar boundary-condition
   type is now refused by name too (`scalar_bcs` used to turn them into `calculated` silently).
5. ~~**`lookUpTableCHF`** (366 lines) with the 2006 Groeneveld table data
   (licence to be checked). Needed by PSBT Phase II Ex 2 and 3. **2026-11-26 (gh:#1223): licence unclear, deferred to the maintainer**; left refused.~~ **CORRECTED 2026-11-26** (gh:#1223, maintainer: "Port can use table with proper citation"): ported (`chf_table.rs`), table vendored with provenance.
6. ~~**Post-CHF `CachardLiquid`/`CachardVapour`** (560 lines): port with
   upstream's pressure-as-pitch binding reproduced and flagged, or report the
   upstream defect.~~ **CORRECTED 2026-11-26** (gh:#1224): ported with upstream's binding as the default and the pitch as a named option; reporting the defect upstream and the choice of default remain with the maintainer.
7. ~~**`regimeMapModels/twoParameters`** (1,501 lines). Needed by PSBT.~~ **CORRECTED 2026-11-26** (gh:#1225): ported.
8. ~~**`templatedModels/byRegime` dispatch** (428 lines) for diameter,
   dispersion, contact partition, area. 1D_boiling and PSBT both use it.~~ **CORRECTED 2026-11-26** (gh:#1225): ported as `ByRegimeModel<M>` for those four.
9. **`phaseChangeModel` orchestration to parity** (925 lines): ~~`dmdtW` wall
   boiling split~~ (**CORRECTED 2026-11-26**, gh:#1222: `dmdtW`, `fw` and the wall part of `c1`/`c2` are ported), `limitMassTransfer`, `latentHeatModel::adjust()`.
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
18. **thermoMechanics**: traction / gap-contact BCs (1,035 lines; gh:#1235). ~~The legacy FVM solver itself~~ **ADDED 2026-11-26 (round 23):** `legacy_fvm` is compared bit for bit with the real class on the ESFR mechanics region.
19. **`common`**: `mergeOrSplitBaffles`, `latticeMap` (923 lines), when the
    multi-region layer needs them.
20. **`openFoamImportedSolvers`**: `compressibleInterFoam`, `scalarTransportFoam`
    (1,884 lines); check first whether the workspace's own ports cover them.
