# GeN-Foam port gap audit (gh:#1217, epic gh:#1216)

**Taken 2026-11-04** against upstream GeN-Foam commit `652b3da` (the commit
every port header cites; it is the archived tip, "docs: Add archive banner"),
fetched shallow and sparse from `gitlab.com/foam-for-nuclear/GeN-Foam`. Port
side: `crates/outram-foam-appbuilder-lib/src/genfoam/` at branch
`claude/outram-foam-8ookor` merged with `develop` (merge `270705faf`).

> **REFRESHED 2026-11-27 (maintainer: "Port genfoam exhaustively", epic gh:#1216).**
> Every row below was re-checked against the code at branch tip `f2c31dc42`
> (rounds 1-31 of the V&V record) and against `652b3da` re-read in
> `/home/user/upstream-build/GeN-Foam-src`; stale rows are struck through with
> a dated `CORRECTED` note saying what was verified, and rows that were already
> true are untouched. **What was NOT done:** no `cargo` run (the status of a row
> is read from the code, the module docs, the tests' names and the V&V record,
> not re-measured), no re-run of the real binary, and the closure-level
> "reached by a real-binary run" marks are read from the *model selections* of the
> shipped cases the port already compares (`1D_boiling`, `1D_PSBT_SC`, `1D_CHF`,
> `2D_MSFR`), not from instrumenting the run. Three sections are new: **"Rows
> added 2026-11-27"**, **"Tutorial coverage"** and **"Remaining work, ordered
> (2026-11-27)"**; the re-counted summary table is first in "Summary".

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
  **Re-checked 2026-11-27:** 88,169 is the sum over every `*.C`/`*.H` under `src/` with
  the `lnInclude` symlink farms excluded (they duplicate the sources and would
  double-count: the per-area "upstream lines" below coincide with the `lnInclude`
  totals only because the farms are complete). The audit had left out the
  **application** `GeN-Foam/` (`GeN-Foam.C` 227 + `include/{setDeltaT.C,setDeltaT.H,modifyDictLegacy.H}` 630 =
  **857 lines**) and the two upstream test drivers (`tests/hydrogenThermophysicalProperties`,
  `tests/radialBasisFunctions`); they are rows A1-A3 below. `FMU4FOAM/` is an empty
  submodule in the checkout and `Tools/` is Python (Serpent/OpenMC cross-section
  converters, case formatting, plotting): neither is solver logic and neither is
  counted. Grand total with the application: **89,026 lines**.
- **Port line counts** are `wc -l` over every `*.rs` under `src/genfoam/`:
  ~~**38,791 lines** (tests included)~~ **CORRECTED 2026-11-27:** **68,855 lines**
  (`find src/genfoam -name '*.rs' | xargs cat | wc -l`, unit-test modules included;
  the integration tests under `tests/genfoam_*.rs` (33 files) are not in the figure).
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
  **Vocabulary from 2026-11-27 (five states, used by the re-count):**
  **PD** = ported + differentially tested against the real binary (the
  tolerance and the round are stated; "bit-identical" or <= 1e-9 is the goal,
  anything looser is said so); **PU** = ported + unit-only (no run of the real
  binary reaches it; a row that mixes models says "k of n models in a real-binary run");
  **PA** = partial (what is missing is named, including options refused by name);
  **M** = missing (nothing translated; "blocked" where an external dependency is
  absent); **OS** = out of scope, used ONLY where upstream itself does not build
  or reference the code, with the citation. FMU and OFFBEAT are NOT out of scope:
  the maintainer's 2026-11-26/27 direction is "port ALL of GeNFoam", and
  gh:#1786 / gh:#1783 ask to *decide and port*; they are "missing, blocked on an
  absent submodule".
- **V&V class.** *Unit*: tested against hand calculations, closed forms or
  published correlation values. *Upstream run*: compared cell by cell against
  a run of upstream GeN-Foam (`tests/genfoam_*.rs`, data in `tests/data/`).
  *Tutorial*: compared against a tutorial `Alltest` reference value.

## Summary

### Re-counted 2026-11-27 (rows / upstream lines per status)

Every upstream `.C`/`.H` is in exactly one row (101 rows, **89,026 lines**: the
88,169 under `src/classes/` plus the 857 of `GeN-Foam/`). Statuses are the five of the
vocabulary above. Areas: **A** application `GeN-Foam/`; **B** `thermalHydraulics/solvers`;
**C** `thermalHydraulics/src/physicsModels` (closures, pairs, turbulence, family bases);
**D** `thermalHydraulics/src/phaseModels` (fluid, structure, power models); **E**
`thermalHydraulics/src` other (BCs, function objects, thermo, `customPimpleControl`,
`include`); **F** `neutronics`; **G** `thermoMechanics`; **H** `multiRegion`; **I**
`common`; **J** `openFoamImportedSolvers`; **K** `offbeat`. The row list behind the counts
is the tables below plus section "Rows added 2026-11-27".

| area | PD ported + differential | PU ported, unit-only | PA partial | M missing | OS out of scope |
|---|---:|---:|---:|---:|---:|
| A application | 2 / 394 | 0 | 0 | 1 / 463 | 0 |
| B solvers | 1 / 148 | 0 | 4 / 6,187 | 0 | 1 / 985 |
| C physicsModels | 22 / 12,677 | 12 / 16,955 | 5 / 4,510 | 2 / 585 | 0 |
| D phaseModels | 4 / 2,396 | 1 / 242 | 5 / 4,949 | 7 / 7,483 | 0 |
| E `src` other | 1 / 309 | 5 / 5,374 | 1 / 615 | 3 / 2,135 | 0 |
| F neutronics | 2 / 1,469 | 0 | 5 / 8,792 | 1 / 849 | 0 |
| G thermoMechanics | 0 | 0 | 2 / 2,671 | 1 / 454 | 0 |
| H multiRegion | 0 | 0 | 3 / 2,379 | 0 | 0 |
| I common | 3 / 2,162 | 1 / 457 | 0 | 3 / 1,077 | 0 |
| J imported solvers | 0 | 0 | 0 | 2 / 1,884 | 0 |
| K offbeat | 0 | 0 | 0 | 1 / 425 | 0 |
| **total** | **35 / 19,555** | **19 / 23,028** | **25 / 30,103** | **21 / 15,355** | **1 / 985** |

How to read it, and what the numbers hide:

- **PD is 22 % of the lines, PU 26 %, PA 34 %, M 17 %, OS 1 %** (rows: 35 / 19 / 25 / 21 / 1 = 101). The PD figure counts a
  row as differential when the real binary reaches every model in it; the 12 PU closure
  rows (16,955 lines) contain models that ARE reached (e.g. 1 of 8 FS drag models,
  2 of 6 FF drag, 3 of 4 FS heat-transfer): see the per-row "k of n" notes and the two
  closure-sweep issues (gh:#1873, gh:#1874).
- **Of the 21 missing rows, 3,424 lines are blocked on an absent external dependency, not
  on effort:** FMU (`NusseltWallAndHfromFMU` 327, `fixedTemperatureFMU`/`nuclearFuelFMU` 1,890,
  `fieldIntegralToFMU` 328 = 2,545 lines; the `FMU4FOAM` submodule is empty and upstream
  builds it only with `Allwmake --fmi`; gh:#1786) and OFFBEAT (`extendedThermoMechanics`
  454 + `fuelBehaviour` 425 = 879 lines; the `src/classes/offbeat/src` submodule is not in
  the tree and both classes `#include "offbeatTime.H"`, `thermalSubSolver.H`, ...; gh:#1783).
- **The one OS row is `onePhaseLegacy` (985 lines):** upstream does not build it
  (`src/classes/thermalHydraulics/solvers/Make/files` lists only `thermalHydraulicsModel.C`,
  `onePhase/onePhase.C`, `twoPhase/twoPhase.C`) and nothing under `src/`, `GeN-Foam/` or
  `Tutorials/` references the class by name outside its own files (grep, 2026-11-27).
  The earlier draft child (gh:#1231, "and port `onePhaseLegacy`") therefore cannot be
  differentially tested against the real binary at all; the `onePhase` half of #1231 stands.
- `compressibleInterFoam` / `scalarTransportFoam` (J) ARE compiled upstream
  (`libOFSolvers`) but are referenced by nothing in GeN-Foam (same grep): "missing", the
  lowest-value rows; gh:#1237.

### As audited 2026-11-04 (superseded; kept because the struck-through rows below refer to it)

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
   ~~1D_HX/twoPhase and 2D_KNS37-L22) is not.~~ **CORRECTED 2026-11-27** (gh:#1799, round 34): the `solveAlpha` lambda is ported and compared with the real binary on documented variants of 1D_boiling (README "Round 34"); the two shipped cases still need other unported pieces.

**CORRECTED 2026-11-27 — the list above was the 2026-11-04 ranking; items 1-5 and 7 have
been delivered (twoPhase, FFPair/FSPair field assembly for the isotropic case, TRACE /
TRACECHF / VapourTRACE / lookUpTableCHF, `twoParameters` and `byRegime`, MULES for
`solvePhase both`) and are compared with the real binary. The gaps by consequence on
2026-11-27, with their issues, are:**

1. **Whole shipped tutorials that have never been read or run by the port** (the
   acceptance tests of the epic): `2D_voidMotionNoPhaseChange` (gh:#1868), `2D_KNS37-L22`
   (gh:#1869), `1D_HX` (gh:#1867), `2D_FFTF` (gh:#1870), `2D_cavityBoussinesq` (gh:#1866),
   `2D_externalSourceDiffusion` (gh:#1863), the point-kinetics cases (gh:#1864), `3D_HTR-10` /
   `3D_gFHR` / `3D_NTPfuelAssembly` (gh:#1876), `3D_SmallESFR_*` (gh:#1846), `Godiva_SN`
   beyond one step (gh:#1788); see "Tutorial coverage".
2. **Radial-conduction power models** (`heatedPin`, `nuclearFuelPin`,
   `interpolatedNuclearFuelPin`, `lumpedNuclearStructure`, `nuclearFuelPinTest`: 4,882 lines;
   gh:#1230, `nuclearFuelPinTest` gh:#1787): ~~still entirely missing~~ **CORRECTED 2026-11-27 (round 35): the four models of #1230 are ported and bit-identical to the real binary as stand-alone models; `nuclearFuelPinTest` and the wiring into the TWO-PHASE region are still missing** **CORRECTED 2026-11-27 (gh:#1879, round 37): wired into the `onePhase` cell-centred region and compared with the real binary on the `2D_MSFR` TH region with the shipped dictionaries (no shipped tutorial can host them: each needs other unported features)**, and the largest single
   blocker of reactor-scale tutorials (`2D_FFTF`, `2D_KNS37-L22`, `2D_fullCoupling`,
   `3D_SmallESFR_*`, `1D_thermalMSR`, `3D_NTPfuelAssembly`, `3D_gFHR` lumped variant).
3. **The `structure` field layer** (`structure.C` 1,181, `byZone` 711): tortuosity,
   anisotropy, local frames, heat exchangers and pumps inside the solvers (gh:#1229, #1847, #1867).
4. **`pointKinetics`** (2,391 lines) has never been compared with a real-binary run
   (gh:#1864); ~~and diffusion has no external source (gh:#1863)~~ **CORRECTED 2026-11-27 (round 32):** the diffusion external source is ported and matches the real binary (`diffusion/external_source.rs`).
5. **The moving-mesh coupled ESFR run** (gh:#1846 <- #1856 <- #1854): the mechanics,
   deformation and `meshToMesh` pieces are each bit-identical to upstream, but nothing
   calls them from a region solver.
6. **Closures never reached by a real-binary run** (gh:#1873, gh:#1874) and
   **function objects / BCs** (gh:#1871), **H2** (gh:#1872).

## 1. `thermalHydraulics/solvers`

| upstream | lines | port | status | tests / V&V |
|---|---:|---|---|---|
| `thermalHydraulicsModel.{C,H}` (region base: PIMPLE, `p`/`p_rgh`, `g`, `hRef`, power coupling fields) | 745 | `solver/mod.rs` (`ThermalHydraulicsSolver` enum), fields inlined in `one_phase.rs` | partial. **Re-checked 2026-11-27 (PA):** the base-class state (PIMPLE controls, `p`/`p_rgh`, `g`, `hRef`, power coupling fields) is re-implemented inside each of `solver/two_phase/{mod.rs,one_phase_fc.rs,one_phase_cc.rs}` with no shared type; each copy is differential-tested through its own solver. Owner: gh:#1475 (regionSolvers), consolidation gh:#1877 | none of its own; through each solver's differential tests |
| `onePhase/` (`onePhase.{C,H}`, `UEqn_1p`, `pEqn_1p`, `pEqnf_1p`, `EEqn_1p`, 7 `pEqnRoutines`) | 1,022 | `solver/one_phase.rs` (661), `solver/porous_drag.rs` (230) | partial: `one_phase.rs` is the older constant-property driver (`he = Cp T`, isotropic `Kd`, no `p_rgh`/gravity split, no `pEqnf`, no `fvOptions`), kept as it was. ~~no `pEqnf` face-centred momentum~~ **CORRECTED 2026-11-12 (gh:#1361 round 4):** `solver/two_phase/one_phase_fc.rs` (translation of `onePhase.C`, `UEqn_1p`, `pEqnf_1p` + routines, `EEqn_1p`, `FSPair::correct`, `customPimpleControl::loop`; heRhoThermo packages, `p_rgh`/gravity, `contErr`, kinetic energy; NOT ported: other momentum modes, turbulence, heat exchangers, pumps, tortuosity, ~~non-`zeroGradient` U, closed volumes~~ **CORRECTED 2026-11-27 (round 38):** `noSlip` U and closed volumes (value branch) are ported) runs the 40-cell channel derived from 1D_boiling against the real binary to 1e-13 over 108 steps. **ADDED 2026-11-26 (round 9, issues 1773/1775):** `solver/two_phase/one_phase_cc.rs` is the cell-centred `solveFluidMechanics false` energy-only region (Boussinesq `hConst`, `fixedTemperature`, constant htc, frozen flux, `alphat` read, `turbulence->correct()` never runs in it): the 2D_MSFR coupled transient agrees with the real binary over 50 steps (T 2.5e-14); UEqn/pEqn with `momentumMode cellCentered` and `porousKEpsilon` SOLVING are still missing. **ADDED 2026-11-26 (round 18, gh:#1812):** the upstream `steadyStateTH` reference protocol (`scripts/run_msfr_th_tight.sh`, 14 steps, turbulence on/off) and committed predictions are in README round 18; the `wedge` VECTOR patch field (`fv::VectorBc::Wedge`, `wedge_velocity_bc`, `rotation_tensor`: ESI `wedgeFvPatchField`/`wedgePolyPatch`, a building block `UEqn_1p` needs on the 2D_MSFR mesh) is ported and unit-tested analytically; still missing for the stage: `UEqn_1p`/`pEqn_1p` assembly, one-phase `porousKEpsilon` solving, the `kqR`/`epsilon`/`nutk` wall-function PATCHES that act on fields (none exist in the workspace), `ReynoldsPower` `Kd` in the momentum equation, the pump `momentumSource` in `phiSbyA`. ~~one-phase `porousKEpsilon` solving and the wall-function PATCHES missing~~ **CORRECTED 2026-11-26 (round 19, gh:#1837):** `UEqn_1p`/`pEqn_1p` with `turbulence false` are ported (`one_phase_cc/flow.rs`, 3 steps to <= 1.4e-11). **ADDED 2026-11-26 (round 20, gh:#1838/#1839):** `one_phase_cc/turbulence.rs` ports single-phase `porousKEpsilon::correct()` with `kqRWallFunction`, `epsilonWallFunction` (cell weights, `G`/`epsilon` overwrite, `setValues`), `nutkWallFunction` (stepwise blender) and `nearWallDist`; `turbulence true` runs and agrees with the real binary for all nine fields in steps 1-3 (<= 6e-13), at steps 6, 9, 14 (<= 6e-13) and in every `k`/`epsilon` solve of 14 steps (residual 1e-11, iterations within 2). ~~Refused by name: `DhStruct`, wall-function entries beyond `type`/`value`, other blenders.~~ **CORRECTED 2026-11-27 (round 29, gh:#1842; read from `one_phase_cc/turbulence.rs`):** `DhStruct` (`nut_stabilization`, `dh_struct`, `laminar_re_struct`), the wall-function entries and the five `nutk`/`epsilon` blenders are ported and agree with the binary (U, k, epsilon, nut at steps 1 and 3 of 3; README round 29, 15 tests). ~~`EigenvalueStage::SteadyStateTh` is still refused (gh:#1841)~~ **CORRECTED 2026-11-26 (round 21):** ported. **ADDED 2026-11-27 (rounds 29-31, gh:#1855):** `customPimpleControl` `residualControl`, the `Final` selection it causes and `relaxationFactors` `default`/`fields` for the cell-centred region: 14 variants at <= 2.6e-12 (two loose-`p_rghFinal` variants at 5.5e-7, cause explained, README round 31). **Status 2026-11-27: PA (partial), differential for what is ported.** Still refused by name (read from `one_phase_cc/flow.rs`, `one_phase_cc.rs`, `one_phase_fc.rs`): `momentumMode cellCenteredFaceReconstruction` and `porousInterfaceSharpness != 0` (gh:#1865; `2D_FFTF`), ~~`nNonOrthogonalCorrectors != 0`, closed volume, `Boussinesq` (gh:#1866; `2D_cavityBoussinesq`, `3D_HTR-10`)~~ **CORRECTED 2026-11-27 (round 33 for the cell-centred region, round 38 for the face-centred one; read from `one_phase_cc/flow.rs` and `one_phase_fc.rs`):** the `nNonOrthogonalCorrectors` loop, the closed-volume reference cell (value branch, `psi = 0`) and `Boussinesq` `hConst` are ported in BOTH regions and agree with the real binary (round 33: 6 variants at <= 2.3e-12; round 38: `2D_cavityBoussinesq` AS SHIPPED, 3169 steps to 30 s, `p` 1.6e-14, `T` 2.8e-13, `U` 2.2e-12 of `max |U|` at the last step; the loop on the channel and on the cavity at 1 and 2 passes). **Still refused by name in `one_phase_fc`:** `3D_HTR-10` (`fixedFluxExtrapolatedPressure`, `perfectGas`/`hPolynomial` 3-D, not run), a closed volume with a compressible (`psi > 0`) thermo (`getRefCellValue`, `deltaMass/compressibility`), a `corrected` Laplacian (gh:#1878); ~~heat exchangers (gh:#1847, #1867)~~ **CORRECTED 2026-11-27 (gh:#1847, README round 44; read from `one_phase_fc.rs`, `one_phase_cc.rs`, `case::read_heat_exchangers`):** heat exchangers are ported in BOTH onePhase regions (secondary zone in the same mesh; `secondaryRegion` and `interpolationMethod != 2` refused by name; the twoPhase region stays gh:#1867) and `1D_HX/onePhase` AS SHIPPED runs 2008 steps to 20 s against the real binary (`deltaT` bit-identical, 37,104 `p_rgh` solves with equal counts, `T` <= 3.2e-11, `T.HX` <= 1.9e-11; `p` misses its committed gate by 1e-7, gh:#1894), `powerOffCriterionModel` and `momentumSourceTimeProfile` (gh:#1870), tortuosity / local frames (gh:#1229), `fvOptions`, drag models other than `ReynoldsPower` and heat-transfer models other than `constant` in the cell-centred region, `fixedValue`/`slip`/`flowRateInletVelocity`/`partialSlip` velocity patches in `one_phase_fc` (gh:#1867; `fixedValue`, `slip` are accepted by the shared reader and NOT verified; ~~`noSlip` and `fixedFluxPressure`~~ **CORRECTED 2026-11-27 (round 38): `noSlip` and `fixedFluxPressure` are ported and verified on `2D_cavityBoussinesq`**), ~~the `Final` selection in `one_phase_fc` (gh:#1857)~~ **CORRECTED 2026-11-27 (round 38):** `p_rghFinal` is selected at the last pass of the last corrector in `one_phase_fc` (verified only through the solve log, README round 38 C-K2); `residualControl` and the `Final` selection of the OTHER fields stay open (gh:#1857). **ADDED 2026-11-27 (round 38, gh:#1878):** `one_phase_fc` solves `p_rgh` with the translated `PCG`/`DIC` (`GAMG` = the named substitution `GamgPolicy::SubstitutePcgDic`), the older ILU-BiCGStab is `with_krylov_for_p_rgh`; the tight protocol on the round-4 channel (5.0e-14 over 108 steps) and a 2-D open cavity (<= 9e-15 for `p`, 5.8e-12 for `U` on the `max |U|` scale at step 100); upstream's `nuEff`/`Re` use the THERMO density (a defect of the port found on the cavity) | unit: `solver/tests.rs` (6); `tests/genfoam_one_phase_vs_upstream.rs`, `genfoam_msfr_th_vs_upstream.rs`, `genfoam_msfr_th_options_vs_upstream.rs`, `genfoam_msfr_th_pimple_vs_upstream.rs`, `genfoam_msfr_allrun_vs_upstream.rs` (vs the real binary) |
| `onePhaseLegacy/` | 985 | — | ~~missing~~ **CORRECTED 2026-11-27: OS (out of scope, upstream does not build it).** Verified: `solvers/Make/files` is `thermalHydraulicsModel.C`, `onePhase/onePhase.C`, `twoPhase/twoPhase.C`; a grep for `onePhaseLegacy` over `src/`, `GeN-Foam/` and `Tutorials/` hits only the class's own files. It cannot be compared with the real binary (not in `libFFSSolvers2`). gh:#1231 mentions porting it; its `onePhase` half stands | — |
| `twoPhase/` (`twoPhase.{C,H}`, `UEqns_2p`, `pEqn_2p`, `pEqnf_2p`, `EEqns_2p`, `alphaEqns_2p`, 9 `pEqnRoutines`) | 2,655 | `solver/two_phase/{mod,case,fv,thermo,turbulence,pcg,pbicgstab}.rs` (since gh:#1218) | ~~missing~~ **CORRECTED 2026-11-27: PA, differential (rounds 3, 5, 7, 8, 10, 12, 13, 16, 17):** `1D_boiling` AS SHIPPED agrees to <= 1e-9 (p 5e-12) for 65 steps and through the onset, the single-phase run to t = 6 s to 2.5e-11; shipped `Phase_Ex1_12223` runs all 407 steps to the end at p <= 5.1e-9, `T.vapour` <= 1.5e-9, alpha <= 6.8e-9 (round 13); the three `PhaseII` PSBT cases at p <= 1.5e-9 over steps 1-25, Ex2/Ex3 steps 1-5 at <= 2.6e-10 (round 17); shipped `1D_CHF/imposedPower` to 11 s at alpha 6.4e-10 (round 13). **After the onset the flow excursion amplifies a 1e-6 difference to O(1)**, so `1D_boiling` is gated only up to and through the onset (gh:#1811, #1831, #1239). **Missing / refused by name:** `momentumMode cellCentered` (`pEqn_2p.H`, 414 lines) and the `explicit`/`none` partial-elimination modes (gh:#1220); `porousInterfaceSharpness != 0`; `fixedTemperature` + `temperatureTimeProfile` structure (`1D_CHF/imposedTemperature`, gh:#1822); heat exchangers, pumps, passive structures, tortuosity / local frames (gh:#1229, #1867); `fvOptions`; the `Final` selection and `residualControl` of the new `customPimpleControl` (gh:#1857); `oscillationLimiterFraction != 0`; `filmCondensationModel` and `heatFluxSuperposition false` (gh:#1874) | `tests/genfoam_two_phase_vs_upstream.rs`, `genfoam_two_phase_mules_vs_upstream.rs`, `genfoam_psbt_trace_vs_upstream.rs`, `genfoam_psbt_shipped_vs_upstream.rs`, `genfoam_chf_trace_vs_upstream.rs`, `genfoam_chf_shipped_vs_upstream.rs`, `genfoam_psbt2_turbulence_vs_upstream.rs`; unit `two_phase/tests.rs` |
| `twoPhase/.../solveAlphaImplicitUpwind.H` | 148 | `two_phase/mod.rs` `correct_alpha` (since gh:#1218) | ~~missing~~ **CORRECTED 2026-11-27: PD** (round 3 and every later two-phase run: `implicitUpwind` is what the PSBT and CHF cases select) | as the `twoPhase/` row |
| `twoPhase/.../solveAlphaMULES.H` + `myMULES/` | 1,765 | `two_phase/{mules,alpha_mules}.rs` (gh:#1361 round 7) | ~~missing~~ **CORRECTED 2026-11-27: PA.** `solvePhase both` is ported and agrees with the binary on `1D_boiling` as shipped to the single-phase round-off (1e-11) up to the onset (README round 7, `tests/genfoam_two_phase_mules_vs_upstream.rs`, 6 gates). ~~**Missing:** the `solvePhase` lambda path (`solveAlpha`, `limit(returnCorr = false)`, `movingAlpha - alpha` flux, the 9-argument `explicitSolve`; refused by name in `TwoPhaseCase::read`; used by `1D_HX/twoPhase` and `2D_KNS37-L22`: gh:#1799)~~ **CORRECTED 2026-11-27 (gh:#1799, round 34):** the `solveAlpha` lambda (`limit(returnCorr = false)`, `movingAlpha - alpha` flux, 9-argument `explicitSolve`, `Sp`/`Su` in the limiter, other fluid by continuity, lazy `alpha.oldTime()` of the other fluid in step 1) is ported and agrees with the binary at round-off (p <= 6.6e-13, alpha <= 2.2e-12) on 4 variants of 1D_boiling with 1-3 sub-cycles and a non-uniform start (`tests/genfoam_two_phase_mules_solve_phase_vs_upstream.rs`); the `solvePhase liquid` variant agrees to step 44 and then separates at the noise floor. **Missing:** coupled (`cyclic`) patches, now REFUSED by name (a `wedge` patch's `lambda = 0` is ported, no numerical effect), moving meshes, `rho` other than `geometricOneField`; ~~the sub-cycle path (`nSubCycles > 1`) is not compared bitwise~~ compared to round-off (not bitwise: the p_rgh GAMG solve differs); `calculated` alpha patches are read as `zeroGradient` (upstream keeps their assigned values; not exercised while no vapour reaches a boundary cell: gh follow-up) | as above |

## 2. `thermalHydraulics/src/physicsModels` — the closures

| upstream family / model | lines | port | status | tests / V&V |
|---|---:|---|---|---|
| **FS drag** `FSDragCoefficientModels/` Churchill, Colebrook, Engel, modifiedEngel, BaxiDalleDonne, NoKazimi, Rehme, ReynoldsPower (8) | 2,233 | `closures/fs_drag/mod.rs` | ported (all 8). **Re-checked 2026-11-27: PU, 1 of 8 in a real-binary run** (`ReynoldsPower`: `1D_boiling`, PSBT, 2D_MSFR); `Churchill` (selected by `2D_FFTF`), `Colebrook` (`3D_NTPfuelAssembly`), `Rehme` (`2D_KNS37-L22`, `2D_FFTF`), `Engel`, `modifiedEngel`, `BaxiDalleDonne`, `NoKazimi` (no shipped case) are unit-only, gh:#1873 | unit (6); **upstream run**: `ReynoldsPower` on PSBT, 836 cells, 6.15e-6 (`tests/genfoam_psbt_closures.rs`) |
| `FSDragFactor.{C,H}` (`Kd` tensor assembly) | 478 | `solver/porous_drag.rs` | partial: isotropic branch only; anisotropic `localDhAnisotropy`/local frames missing. **Re-checked 2026-11-27, still true (PA):** the isotropic `Kd` is what every differential run exercises (`solver/two_phase`, `one_phase_fc`, `one_phase_cc`); gh:#1221 / #1229 own the anisotropic branch | unit (1); through every solver comparison |
| **FF drag** `FFDragCoefficientModels/` Autruffe, Bestion, BestionTRACE, NoKazimi, SchillerNaumann, Wallis (6) | 1,678 | `closures/ff_drag/interfacial.rs` | ported (all 6, per-cell `Cd`). **Re-checked 2026-11-27: PU, 3 of 6 in a real-binary run** (`SchillerNaumann`: `1D_boiling`; `Bestion`: PSBT Phase_Ex1, CHF; `BestionTRACE`: `PhaseII_Ex1_01_5215`, steps 1-25); `Autruffe` (selected by `2D_KNS37-L22`), `NoKazimi`, `Wallis` (selected by no shipped case: grep) unit-only, gh:#1873 | unit (in `ff_drag/tests.rs`) |
| `FFDragFactor.{C,H}` (`Kd = 0.75 Cd ρ_c |Ur| α_d / D` assembly) | 240 | `solver/two_phase/mod.rs` (`FFPair::correct` assembly, gh:#1218) | ~~missing~~ **CORRECTED 2026-11-27: PD** (every two-phase run: `Kd12` enters the partial elimination of `pEqnf_2p`; rounds 3-17) | through `genfoam_two_phase_vs_upstream.rs` and the later two-phase tests |
| **two-phase multipliers** ChenKalish, Kaiser74, Kaiser88, KottowskiSavatteri, LockhartMartinelli, LottesFlinn, LottesFlinnNguyen, constant (8) | 2,102 | `closures/ff_drag/multipliers.rs` | ported (all 8). **Re-checked 2026-11-27: PU, 2 of 8 in a real-binary run** (`Kaiser88`: `1D_boiling`; `LockhartMartinelli`: PSBT, CHF); `ChenKalish`, `Kaiser74`, `KottowskiSavatteri`, `LottesFlinn` (`2D_voidMotionNoPhaseChange`), `LottesFlinnNguyen`, `constant` unit-only, gh:#1873 | unit (11 in `ff_drag/tests.rs`) |
| `twoPhaseDragFactor.{C,H}` (applies the multiplier to `Kd`) | 251 | `solver/two_phase/mod.rs` (gh:#1218) | ~~missing~~ **CORRECTED 2026-11-27: PD** (same runs as `FFDragFactor`) | as `FFDragFactor` |
| **FS heat transfer** Nusselt, NusseltAndWall, Shah, Gorenflo | 1,193 | `closures/heat_transfer/fs_htc.rs` | ported. **Re-checked 2026-11-27: PU, 3 of 4 in a real-binary run** (`Shah`: `1D_boiling`; `Nusselt`, `Gorenflo`: PSBT, CHF); `NusseltAndWall` unit-only, gh:#1873 | unit (34 in `heat_transfer/tests.rs`, shared) |
| `multiRegimeBoiling` | 588 | `closures/heat_transfer/boiling.rs` | ported (post-CHF branch absent, as upstream: `TCHFi = 1e69`). **2026-11-27: PD** (`PhaseII_Ex1_01_5215` selects it, round 16/17: p 1.5e-9 at step 25); `filmCondensationModel` and `heatFluxSuperposition false` are refused by name (gh:#1874) | unit; `tests/genfoam_psbt2_turbulence_vs_upstream.rs` |
| `superpositionNucleateBoiling` | 252 | `closures/heat_transfer/boiling.rs` | ported (htc combiner). **2026-11-27: PD** (`1D_boiling`) | unit; two-phase runs |
| `multiRegimeBoilingTRACE` | 454 | `closures/heat_transfer/boiling_trace.rs`; wired in `solver/two_phase` (`FsHtcModel::MultiRegimeBoilingTrace`, `dmdtW`) | ported (CORRECTED 2026-11-26, gh:#1222; ~~missing~~). **2026-11-27: PD, looser than 1e-9 and why** | unit (8, shared with TRACECHF); **upstream run** on the closed `Phase_Ex1_12223` variant: ~~steps 1-8 agree to round-off, the nucleate-boiling branch differs by 7.8e-9 (step 9) to 6e-7 (step 12), cause open~~ **CORRECTED 2026-11-27 (round 13, gh:#1803 / #1820):** the 7.8e-9 seed was four things (CODATA gas constant, converged `T` from `he`, live markers, `dmdt/dmdtIi0` flux; README round 13); now htc 1.3e-10 at step 9, <= 1.0e-8 (the noise floor of the two `p_rgh` solvers) at steps 10-13, alpha <= 3.5e-8 and p <= 1.1e-9 over all 40 reference steps, and the SHIPPED case to its end (407 steps) at p <= 5.1e-9, htc <= 4.9e-8 (`tests/genfoam_psbt_trace_vs_upstream.rs`, `genfoam_psbt_shipped_vs_upstream.rs`; README "Round 8", "Round 13"). gh:#1803 is open although the cause is found: proposed for closure |
| `multiRegimeBoilingTRACECHF` | 684 | `closures/heat_transfer/boiling_trace_chf.rs`; wired in `solver/two_phase` (`FsHtcModel::MultiRegimeBoilingTraceChf`, fields `chf_bool`, `wf_transition_boiling`, `t_leidenfrost`, `dmdtW`) | ported and wired (**CORRECTED 2026-11-26**, gh:#1223/#1224/#1361 round 10; ~~pure function ported, NOT wired: its post-CHF branches need `CachardLiquid` (missing) and the Ex2/Ex3 cases `lookUpTableCHF` (deferred)~~) | unit (8, shared with TRACE); **upstream run** on closed `1D_CHF` variants (`tests/genfoam_chf_trace_vs_upstream.rs`): ~~`constant_chf` agrees to <= 4.3e-5 over steps 1-40 (transition then film boiling), parts at a threshold event at step 41; `lookup_table_chf` to <= 8e-4 over steps 1-3, parts at step 4~~ **CORRECTED 2026-11-27 (round 13, gh:#1816):** `constant_chf` no longer parts at step 41 (that was the `dmdt/dmdtIi0` flux translation): the whole 86-step reference agrees at p 2.7e-9, alpha 3.3e-6, `T.fixedPower` 2.9e-7, dt 8.1e-7; its step-1 seed (p 1.8e-8, `U.liquid` 8.2e-7, alpha 3.5e-7) is **still unexplained** (gh:#1816, open); `lookup_table_chf` is at 6.1e-4..8.1e-4 over steps 1-3 (unchanged, unexplained) and 6.0e-4 (alpha) at step 40. **Status 2026-11-27: PD, looser than 1e-9** (README "Round 10", "Round 13"); the SHIPPED `1D_CHF/imposedPower` runs to 11 s at alpha 6.4e-10 (`tests/genfoam_chf_shipped_vs_upstream.rs`) |
| `multiRegimeBoilingVapourTRACE` | 304 | `closures/heat_transfer/boiling_trace_chf.rs`; wired (`FsHtcModel::MultiRegimeBoilingVapourTrace`) | ported and wired (**CORRECTED 2026-11-26**, gh:#1224; ~~not wired~~) | unit; upstream run: `htc.vapour.structure` in `lookup_table_chf` (contact partition 0.7) to 4.3e-4 over steps 1-3 |
| `NusseltWallAndHfromFMU` | 327 | — | missing (~~FMU co-simulation, out of scope~~ **CORRECTED 2026-11-27: M, blocked on the empty `FMU4FOAM` submodule; NOT out of scope**: the maintainer's direction is "port ALL of GeNFoam" and gh:#1786 asks to decide and port; the class is in `src/classes/thermalHydraulics/src/Make/files` but upstream builds `FMU4FOAM` only with `Allwmake --fmi`) | — |
| CHF `constantCHF` | 202 | `closures/heat_transfer/chf.rs` | ported. **2026-11-27: PD** (`1D_CHF` selects it; rounds 10, 13) | unit; `genfoam_chf_*` |
| CHF `lookUpTableCHF` | 366 | `closures/heat_transfer/chf_table.rs` on `common/interpolate_table_nd.rs` | ported (**CORRECTED 2026-11-26**, gh:#1223, maintainer decision of the same day: the table is GeN-Foam's own GPL-3.0 case file, ported with citation, `tests/data/genfoam_chf_gh1223/PROVENANCE.md`; ~~missing: deferred to the maintainer, no licence statement~~). **The premise that `1D_CHF` uses it was wrong:** `1D_CHF` selects `constantCHF`; only `PhaseII_Ex2/Ex3` select the table | unit (8, incl. the shipped table: knots, midpoints, K1/K2, out-of-range); upstream run only through a `lookUpTableCHF` variant of `1D_CHF` (a modified case, README "Round 10"). **Re-checked 2026-11-27: PD, looser** (htc 6.1e-4..8.1e-4 over steps 1-3, unexplained, gh:#1816); the shipped `PhaseII_Ex2/Ex3` use the table and are compared for steps 1-5 only (round 17: p <= 2.6e-10); whether those steps reach a CHF evaluation was not checked |
| Leidenfrost `GroeneveldStewart` | 220 | `chf.rs` | ported. **2026-11-27: PD** (`1D_CHF`, PSBT Ex2) | unit; `genfoam_chf_*` |
| TONB `Basu` | 245 | `chf.rs` | ported. **2026-11-27: PD** (PSBT, CHF) | unit; PSBT/CHF runs |
| flow enhancement Chen, COBRA-TF, RezkallahSims | 641 | `chf.rs` | ported. **2026-11-27: PD, 3 of 3** (COBRA-TF: `1D_boiling`; Chen, RezkallahSims: PSBT) | unit; two-phase runs |
| suppression Chen, COBRA-TF | 434 | `chf.rs` | ported. **2026-11-27: PD, 2 of 2** (COBRA-TF: `1D_boiling`; Chen: PSBT) | unit; two-phase runs |
| sub-cooled boiling fraction SahaZuber, constant | 442 | `chf.rs` | ported. **2026-11-27: PU, 1 of 2 in a real-binary run** (`SahaZuber`: PSBT, CHF; `constant` unit-only, gh:#1874) | unit; PSBT/CHF runs |
| post-CHF `CachardLiquid`, `CachardVapour` | 560 | `closures/heat_transfer/cachard.rs` | ported (**CORRECTED 2026-11-26**, gh:#1224; ~~missing~~). The upstream defect was re-read and holds: `CachardLiquidFSHeatTransferCoefficient.C:124` and `CachardVapourFSHeatTransferCoefficient.C:101` bind `pi` to `p_[celli]` (pressure) and use it as a pitch in `pow(pi/max(DRi,1e-6),2)`. **Default = upstream, bit for bit** (the film is ~1e9 m thick, so `h_gamma` ~ 1e-11 and the post-CHF coefficient is radiation); the intended pitch is the named option `CachardPitch::Pitch`; the maintainer decides which is the default (gh:#1224) | unit; upstream run (both models, through TRACECHF/VapourTRACE) |
| **FF heat transfer** Nusselt | 269 | `closures/heat_transfer/ff_htc.rs` | ported. **2026-11-27: PD** (`1D_boiling`, PSBT, CHF select it) | unit; two-phase runs |
| FF `NoKazimi` | 258 | — | missing (deferred in `ff_htc.rs`). **Re-checked 2026-11-27, still true** (`ff_htc.rs` says the upstream model is undocumented; gh:#1227 is open and unchanged); no shipped case selects it (grep of the tutorials' `phaseProperties`) | — |
| FF `constant` (templated) | — | `ff_htc.rs` / templated | ported as a value | unit |
| **phase change** saturation constantTemperature, water, waterTRACE, BrowningPotter | 1,448 | `closures/phase_change/saturation.rs` | ported (all 4). **Re-checked 2026-11-27: PU, 3 of 4 in a real-binary run** (`water`, `waterTRACE`: PSBT, CHF; `BrowningPotter`: `1D_boiling`); `constantTemperature` unit-only, gh:#1874 | unit (18 in `phase_change/tests.rs`, shared) |
| latent heat FinkLeibowitz, water, fromThermophysicalProperties | 997 | `closures/phase_change/latent_heat.rs` | ~~partial: values ported, `latentHeatModel::adjust()` (enthalpy-consistent `L`) missing; 1D_boiling sets `adjust true`~~ **CORRECTED 2026-11-27: ported, `adjust` included.** Verified: `TwoPhaseSolver::correct_latent_heat` (`two_phase/mod.rs`, "`latentHeatModel::correctField` + `adjust`") is the translation and runs in every `1D_boiling` comparison; the `latent_heat.rs` module doc still says `adjust` is deferred: that doc is stale (not edited here: this change touches the audit only; flagged in "Doc claims checked 2026-11-27"). **PU: 2 of 3 in a real-binary run** (`FinkLeibowitz`: `1D_boiling`; `water`: PSBT, CHF); `fromThermophysicalProperties` and the `adjust false` branch unit-only, gh:#1874 | unit |
| rate `heatDriven` (4 modes), `forcedConstant` | 579 | `closures/phase_change/rate.rs` | ported as algebra. **2026-11-27: PU, 1 of 2** (`heatDriven`: every two-phase run; `forcedConstant` unit-only, gh:#1874) | unit |
| `phaseChangeModel.{C,H}` (orchestration: interfacial temperature, `limitMassTransfer`, `dmdtW` wall split, `heSources`) | 925 | `solver/two_phase/mod.rs` (`correct_phase_change`, `correct_latent_heat`, `limitMassTransfer` block, `dmdt_w`; since gh:#1218, `dmdtW` gh:#1222) | ~~missing~~ **CORRECTED 2026-11-27: PD.** Verified in `two_phase/mod.rs`: `dmdt = dmdtI + dmdtW`, `limitMassTransfer` with `massTransferSafetyFactor`, `dmdt.relax()`, the interfacial temperature with `maxTInterfaceDdt`; compared in rounds 3, 5, 8, 13 (alpha <= 3.5e-8 and dmdt <= 7.7e-9 over the whole shipped PSBT run). The draft child gh:#1226 ("`dmdtW` and `limitMassTransfer`") is therefore delivered; proposed for closure | `two_phase/tests.rs`; the two-phase runs |
| **interfacial** area spherical, Schor, annular, NoKazimi | 1,347 | `closures/interfacial/area.rs` | ported. **2026-11-27: PU, 1 of 4** (`spherical`: `1D_boiling`, PSBT, CHF; `Schor` (selected by `2D_KNS37-L22`), `annular`, `NoKazimi` unit-only, gh:#1874) | unit (15 in `interfacial/tests.rs`, shared) |
| fluid diameter WallisFilm, isomolarBubble, isothermalBubble, pipeFilm | 1,218 | `closures/interfacial/diameter.rs` | ported. **2026-11-27: PU** (no shipped case selects any of the four: a grep of every shipped `phaseProperties` finds none of `WallisFilm`, `isomolarBubble`, `isothermalBubble`, `pipeFilm`; the dispatch and `fluidDiameterModel::correctField` are PD through the constant/byRegime selections; the four models are reachable only by a variant case, gh:#1874) | unit |
| virtual mass coefficient constant | 630 | `closures/interfacial/virtual_mass.rs` | ported. **2026-11-27: PD** (`1D_boiling` selects it; `virtualMass` with non-`zeroGradient` `U` patches is refused by name) | unit; `1D_boiling` runs |
| dispersion constant | 524 | `closures/interfacial/dispersion.rs` | ported. **2026-11-27: PD** (through `byRegime` dispersion in `1D_boiling`) | unit; `1D_boiling` runs |
| contact partition complementary, linear | 788 | `closures/interfacial/contact_partition.rs` | ported. **2026-11-27: PU** (no shipped case selects `complementary` or `linear`: grep of every shipped `phaseProperties`; the `byRegime`-dispatched `constant` partition is PD; gh:#1874) | unit |
| regime map `oneParameter` | 1,115 | `closures/interfacial/regime_map.rs` | ported. **2026-11-27: PD** (`1D_boiling`) | unit; `1D_boiling` runs |
| regime map `twoParameters` (`regimeDomain2D`, `regimeBoundary2D`) | 1,501 | `closures/interfacial/regime_map_2d.rs` | ported (CORRECTED 2026-11-26, gh:#1225; ~~missing~~): incl. `Foam::Random`, `hashlittle` and `HashTable` walk order | unit (7; lookup3 vectors, g++ draws, PSBT map, concave polygon vs winding number); in the upstream comparison via the closed PSBT case |
| templated `byRegime` dispatch | 428 | `closures/interfacial/by_regime.rs` (`ByRegimeModel<M>`); wired for dispersion, contact partition, dispersed diameter, interfacial-area density | ported for those four (CORRECTED 2026-11-26, gh:#1225); drag, virtual-mass, multiplier and htc `byRegime` not wired. **Re-checked 2026-11-27: PA** (four families PD, four not wired; gh:#1873) | unit; upstream run (closed PSBT, `1D_boiling`) |
| templated `constant` | 346 | folded into each family's `Constant` variant | ported. **2026-11-27: PD** (every run) | unit |
| `phasePairs/FFPair.{C,H}` | 1,129 | `phase/phase_pair.rs` (`ff_reynolds`, `ff_relative_velocity_magnitude`); the assembly in `solver/two_phase/mod.rs` | ~~partial: kinematics only~~ **CORRECTED 2026-11-27: PA, differential.** `phase/phase_pair.rs` is still kinematics only and unused by the solvers (gh:#1877), but `FFPair::correct` (Reynolds, relative velocity, `Kd`, `htc`, contact partition, interface temperature with `maxTInterfaceDdt`, virtual mass, dispersion, interfacial area, diameters) is translated inside `solver::two_phase` and compared in every two-phase round. **Still missing / unverified (gh:#1221):** `residualKd` (upstream reads nothing by that keyword per #1218: unconfirmed), the `minMagU` floors (not checked), anisotropic Kd | unit (3); the two-phase runs |
| `phasePairs/FSPair.{C,H}` | 738 | `phase/phase_pair.rs` (`fs_reynolds`); the assembly in `solver/two_phase/mod.rs`, `one_phase_fc.rs`, `one_phase_cc.rs` | ~~partial: kinematics only~~ **CORRECTED 2026-11-27: PA, differential for the isotropic branch** (`FSPair::correct` is in all three solvers; `htc` relaxation by `fields` agrees, README round 31 `fld_htc` is a no-op test of it). Anisotropic `Kd` / local frames missing (gh:#1221, #1229) | unit; the solver runs |
| **turbulence** porousKEpsilon, porousKEpsilon2PhaseCorrected, LaheyKEpsilon, mixtureKEpsilon | 3,663 | `closures/turbulence/*.rs` | partial: closure algebra only; k/ε transport and `correctNut` missing. **ADDED 2026-11-26 (gh:#1802, README round 16):** `solver/two_phase/turbulence.rs` solves `porousKEpsilon2PhaseCorrected` in `solver::two_phase` (`correct()`, `correctNut`, ESI `bound`, `fvMatrix::relax`; feeds `nuEff` to the momentum and `alphaEff` to the energy equation); it is NOT a derivation of `porousKEpsilon` (it derives from `eddyViscosity<RASModel>`), ~~so the single-phase `porousKEpsilon` (gh:#1775/#1812) is still missing~~ **CORRECTED 2026-11-26 (round 20):** the single-phase `porousKEpsilon` is ported in `solver/two_phase/one_phase_cc/turbulence.rs` (see the `onePhase/` row); `LaheyKEpsilon`, `mixtureKEpsilon` are still missing, as are `LaheyKEpsilon`, `mixtureKEpsilon`, plain `kEpsilon`, `nutStabilization` (`DhStruct`) ~~and wall-function patches (refused by name)~~ **CORRECTED 2026-11-26 (round 20):** wall functions exist for the single-phase region only; the two-phase reader still refuses them by name. Against the real binary on `PhaseII_Ex1` the turbulence solves of the first outer iteration agree with upstream's to the printed residuals and Ex3's one-iteration step 1 to 6e-16, ~~but the flow differs by a baseline (`p` 8e-7 at step 1, 2e-5 at step 25) with `turbulence off` as well: not the model's~~ **CORRECTED 2026-11-26 (round 17, gh:#1833):** the baseline was the `symmetry` constraint on `UEqn.H()` plus `PCG`/`DIC` for `p_rgh`, both fixed (Ex3 step 1 p 1.4e-12, Ex1 step 25 p 1.5e-9, `U.liquid` 6.7e-7; Ex2/Ex3 steps 1-5 p <= 2.6e-10). **Re-split 2026-11-27 (read from `closures/turbulence/`, `two_phase/turbulence.rs`, `one_phase_cc/turbulence.rs`, and every shipped `turbulenceProperties`): `porousKEpsilon` (966) + `porousKEpsilon2PhaseCorrected` (960) are PD (1,926 lines: `PhaseII` PSBT and the single-phase 2D_MSFR); `LaheyKEpsilon` (592) and `mixtureKEpsilon` (1,145) stay PA: closure algebra only, no `correct()`/`correctNut` and no field solve; no shipped case solves either (the cases that name `LaheyKEpsilon` or `kEpsilon` set `simulationType laminar`), so there is no tutorial to compare them on and a variant case is needed (gh:#1228).** The two-phase reader still refuses wall-function patches, `DhStruct` and `kqRWallFunction` entries by name (gh:#1834, #1835) | unit (12 + 4: regex, `bound`, `relax`, `G`); `tests/genfoam_psbt2_turbulence_vs_upstream.rs`, `genfoam_msfr_th_vs_upstream.rs` (vs the real binary) |

## 3. `thermalHydraulics/src/phaseModels`

| upstream | lines | port | status | tests / V&V |
|---|---:|---|---|---|
| `phaseBase.{C,H}` | 242 | `phase/phase_base.rs` | ported. **2026-11-27: PU** (the module is unit-only; the solvers carry their own `Phase`: gh:#1877) | via `phase/fluid/tests.rs` |
| `fluid/fluid.{C,H}` + turbulence fwd headers | 1,520 | `phase/fluid/mod.rs` | partial: field bag ported; `fluid::correct` thermo update, `initAlphaPhis`, `constructDiameterModel`, the `thermoResidualAlpha` energy switch logic not wired to a thermo package. **Re-checked 2026-11-27, PA, with a correction:** the module is still unwired and unit-only (7), BUT the logic it lacks exists in `solver::two_phase` (thermo update through `thermo.rs`; the `thermoResidualAlpha` switch, which is what separates the shipped PSBT run at step 51, agreement 4.9e-12 after round 13; diameter construction through `fluidDiameterModel::correctField`) and is compared with the real binary. Two implementations: gh:#1877 | unit (7); the two-phase runs for the solver-local copy |
| `structure.{C,H}` | 1,689 | `structure/mod.rs`, `structure/heat_source.rs` | partial: per-cell algebra; anisotropic tensors, local frames, `fvm::Sp` semi-implicit source matrix missing. **Re-checked 2026-11-27, PA, with a correction:** the semi-implicit `linearizedSemiImplicitHeatSource` IS in `solver::two_phase` (`Structure`, `structure.correct`, `fixedPower`; PD, rounds 3-13) and the `fixedTemperature` + `temperatureTimeProfile` form is in `one_phase_cc` (PD, round 22); the `structure/` module itself is unit-only and unused by the solvers (gh:#1877). **ADDED 2026-11-27 (gh:#1879):** the four radial power models of `structure/` ARE used by `one_phase_cc` (not the rest of `structure/`). **Missing:** anisotropic tensors, local frames, tortuosity, `byZone` dispatch, heat exchangers and `fixedTemperature` in the two-phase region, passive structures (gh:#1229, #1847, #1822) | unit (3+3); the solver runs |
| `byZone/byZoneStructure` | 711 | — | missing. **Re-checked 2026-11-27, still true** (gh:#1229). No shipped case selects it (a grep finds `byZone` only in `3D_SmallESFR_NewSolverVerification/newSolver/.../thermoMechanicalProperties`, a different class) | — |
| `heatExchanger` | 587 | `structure/heat_exchanger.rs` | ~~ported (per-cell two-sided solve); mesh-to-mesh mapping missing.~~ ~~**Re-checked 2026-11-27: PA, unit-only** (no solver calls it: `OnePhaseCcCase::read` and the face-centred readers refuse `heatExchangers` by name; gh:#1847, #1867; shipped consumers `1D_HX`, `2D_FFTF`)~~ **CORRECTED 2026-11-27 (gh:#1847, README round 44): PD (differential) for the onePhase regions.** `structure/heat_exchanger.rs`: `HeatExchangerZone` (zone centre-of-volume translation, the `1e-2` volume error, the translated mesh, the exact round-28 `meshToMesh`), `HeatExchangerSet` (`iAHX`, `T.HX`, the `max(THX) > 1e-69` guard, secondary `htc*T` with the CURRENT `T`), used in the energy equations of `one_phase_fc` and `one_phase_cc` as one sum with the active-structure source. Compared with the real binary: `1D_HX/onePhase` as shipped and with the secondary zone refined to 30 cells (non-conformal weights), 2008 steps each; `T.HX` bit-identical in the first outer pass, `T` <= 3.2e-11 and `T.HX` <= 1.9e-11 over 20 s, the exchanged heat to 1.8e-13 at the end; `p` differs 3e-8 at step 1 (an unexplained baseline of the face-centred region, not of the exchangers, gh:#1894). The cell-centred region is compared only with `hx` paired with itself (`T.HX` 8.8e-16). **Refused by name:** `secondaryRegion`, `interpolationMethod != 2`, the twoPhase region (gh:#1867); a two-zone cell-centred case and `secondaryRegion`: gh:#1895 | unit (5); `genfoam_heat_exchanger_vs_upstream.rs` (A, B, 3 controls, mapping, refusal), `genfoam_heat_exchanger_cc_vs_upstream.rs` |
| `pump` | 297 | `structure/pump.rs` | ported. **Re-checked 2026-11-27: PA** (`structure/pump.rs` is unit-only; the solved cell-centred flow of `one_phase_cc` takes a CONSTANT per-cell `momentumSource` (2D_MSFR, PD, rounds 19-20) and refuses `momentumSourceTimeProfile`; gh:#1870) | unit (2); the 2D_MSFR flow runs |
| `powerOffCriterionModels` timer, fieldValue | 856 | `structure/power_off.rs` | ported. **Re-checked 2026-11-27: PA.** `timer` is wired in `solver::two_phase` and PD (`1D_boiling` selects it); `fieldValue` (350 lines) is unit-only: the only shipped mention of it is a commented-out block in `1D_boiling/constant/fluidRegion/phaseProperties`, and `two_phase/case.rs` refuses any `powerOffCriterionModel` other than `timer` by name; `one_phase_cc` refuses the whole key. gh:#1229 (structure layer) | unit (4); `1D_boiling` for `timer` |
| power `fixedPower` | 518 | `structure/power_model.rs` | ported; **`powerTimeProfile` ADDED 2026-11-26 (gh:#1815)** as upstream's recursion `pd(t) = prof(t) pdOld / prof(t - dt)` (the closed form `initial prof(t)` the port used before is `with_closed_form_power_profile`, deprecated); wired in the two-phase case reader (`type table` only). **2026-11-27: PD** (`1D_boiling`, PSBT, `1D_CHF/imposedPower` as shipped; round 13) | unit (6, shared); the two-phase runs |
| power `fixedTemperature` | 394 | `structure/power_model.rs` | ported. **2026-11-27: PD in `one_phase_cc`** (`2D_MSFR`, with `temperatureTimeProfile`, rounds 14-22); absent from the two-phase region (`1D_CHF/imposedTemperature`, gh:#1822) | unit; `genfoam_msfr_*` |
| power `nuclearSteadyStatePebble` | 956 | `structure/nuclear_steady_state_pebble/` | ported (not a `PowerModel` variant). **2026-11-27: PD, looser** (upstream's class against the port on the gFHR data, 7.2e-6) | **upstream run**: gFHR, ≤ 7.2e-6 over 292,500 cells |
| power `heatedPin` | 822 | `structure/heated_pin.rs` | ~~missing~~ **CORRECTED 2026-11-27 (gh:#1230, README round 35): PD, bit-identical to the real binary** (12 steps x 1 outer and 3 x 2 outer, 1,224 + 612 values; analytic: solid pin exact to round-off, hollow pin second order). **CORRECTED 2026-11-27 (gh:#1879, README round 37): wired into the `onePhase` `cellCentered` region** (`one_phase_cc::structure_correct`, reader `one_phase_cc/radial_power.rs`, ~~moved~~ 2026-11-27 to `two_phase/radial_power.rs`); against the real binary on the `2D_MSFR` TH region with the shipped KNS37 dictionary in the `hx` zone: pin fields 3.6e-16, `T` 1e-15, flow 3.8e-12 (3 steps). ~~NOT in the two-phase region~~ **CORRECTED 2026-11-27 (gh:#1884, README round 41): wired into the `twoPhase` region** (`TwoPhaseSolver::radial_structure_correct`, `EEqns_2p.H:331` with the post-solve `T1`, `T2`; reader shared in `two_phase/radial_power.rs`) and compared with the real binary on documented variants of `1D_boiling` (shipped dictionaries in the `mid` zone; heatedPin incl. the `powerDensity.heatedPin` field file and `powerOff`, nuclearFuelPin gapH/table, lumpedNuclearStructure; through the boiling onset for heatedPin 3 mm and nuclearFuelPin 2x power): pin fields <= 6e-14, flow p <= 4e-12, 1-5e-11 for U; `2D_KNS37-L22` itself still not run | `tests/genfoam_radial_power_models_vs_upstream.rs` (vs the real binary, 0 ulp); `tests/genfoam_radial_power_wired_vs_upstream.rs` (wired, vs the real binary) |
| power `nuclearFuelPin` | 1,533 | `structure/nuclear_fuel_pin.rs` | ~~missing~~ **CORRECTED 2026-11-27 (gh:#1230, README round 35): PD, bit-identical to the real binary** (2,364 + 1,182 values; constant `gapH` and the `gapHPowerDensity` table; upstream's two ring-area slips reproduced and named). **CORRECTED 2026-11-27 (gh:#1879, README round 37): wired into the `onePhase` `cellCentered` region**; the shipped 2D_fullCoupling dictionary in the MSFR `hx` zone, `gapH` and `gapHPowerDensity` table: pin fields, `T`, `TFuelAv`/`TCladAv` 5.6e-16, flow 1.2e-11 (3 steps). ~~NOT in the two-phase region~~ **CORRECTED 2026-11-27 (gh:#1884, README round 41): wired into the `twoPhase` region** (`TwoPhaseSolver::radial_structure_correct`, `EEqns_2p.H:331` with the post-solve `T1`, `T2`; reader shared in `two_phase/radial_power.rs`) and compared with the real binary on documented variants of `1D_boiling` (shipped dictionaries in the `mid` zone; heatedPin incl. the `powerDensity.heatedPin` field file and `powerOff`, nuclearFuelPin gapH/table, lumpedNuclearStructure; through the boiling onset for heatedPin 3 mm and nuclearFuelPin 2x power): pin fields <= 6e-14, flow p <= 4e-12, 1-5e-11 for U; `2D_KNS37-L22` itself still not run | `tests/genfoam_radial_power_models_vs_upstream.rs` (vs the real binary, 0 ulp); `tests/genfoam_radial_power_wired_vs_upstream.rs` (wired, vs the real binary) |
| power `interpolatedNuclearFuelPin` | 352 | `structure/interpolated_nuclear_fuel_pin.rs` | ~~missing~~ **CORRECTED 2026-11-27 (gh:#1230, README round 35): PD, bit-identical to the real binary** (480 values; the real binary traps SIGFPE on an unzoned cell (`0/0`), the port evaluates the model cells only). **CORRECTED 2026-11-27 (gh:#1879, README round 37): read and wired into the `onePhase` region, NO differential run** (its heat flux is an external field; unit-level check only). ~~NOT in the two-phase region~~ **CORRECTED 2026-11-27 (gh:#1884, README round 41): wired into the `twoPhase` region** (`TwoPhaseSolver::radial_structure_correct`, `EEqns_2p.H:331` with the post-solve `T1`, `T2`; reader shared in `two_phase/radial_power.rs`) (read and wired, NO differential run, as in the one-phase region); `2D_KNS37-L22` itself still not run | `tests/genfoam_radial_power_models_vs_upstream.rs` (vs the real binary, 0 ulp); `tests/genfoam_radial_power_wired_vs_upstream.rs` (wired, vs the real binary) |
| power `lumpedNuclearStructure` | 759 | `structure/lumped_nuclear_structure.rs` | ~~missing~~ **CORRECTED 2026-11-27 (gh:#1230, README round 35): PD, bit-identical to the real binary** (948 + 474 values; `fvc::laplacian(Tmatrix)` is an input, fed upstream's dumped value). **CORRECTED 2026-11-27 (gh:#1879, README round 37): wired into the `onePhase` `cellCentered` region, `fvc::laplacian(Tmatrix)` now computed by the port** (`uncorrected` form, internal faces; `kappaMatrix 12`): 1 and 3 nodes, `Tmatrix`/`Tmax`/`Tsurface`/`T` 6e-16 or better, 3 steps. ~~NOT in the two-phase region~~ **CORRECTED 2026-11-27 (gh:#1884, README round 41): wired into the `twoPhase` region** (`TwoPhaseSolver::radial_structure_correct`, `EEqns_2p.H:331` with the post-solve `T1`, `T2`; reader shared in `two_phase/radial_power.rs`) and compared with the real binary on documented variants of `1D_boiling` (shipped dictionaries in the `mid` zone; heatedPin incl. the `powerDensity.heatedPin` field file and `powerOff`, nuclearFuelPin gapH/table, lumpedNuclearStructure; through the boiling onset for heatedPin 3 mm and nuclearFuelPin 2x power): pin fields <= 6e-14, flow p <= 4e-12, 1-5e-11 for U; `2D_KNS37-L22` itself still not run | `tests/genfoam_radial_power_models_vs_upstream.rs` (vs the real binary, 0 ulp); `tests/genfoam_radial_power_wired_vs_upstream.rs` (wired, vs the real binary) |
| power `nuclearFuelPinTest` | 1,416 | — | missing (upstream test variant; named in the `3D_SmallESFR_*` `phaseProperties`; gh:#1787) | — |
| power `fixedTemperatureFMU` 332, `nuclearFuelFMU` 1,558 | 1,890 | — | missing (~~FMU co-simulation, out of scope~~ **CORRECTED 2026-11-27: M, blocked on the empty `FMU4FOAM` submodule; not out of scope**; gh:#1786) | — |
| `powerModels/powerModel.{C,H}` base | 529 | `structure/power_model.rs` (enum) | ported. **2026-11-27: PD** (through `fixedPower` / `fixedTemperature`) | unit |

## 4. `thermalHydraulics/src`, other

| upstream | lines | port | status | tests / V&V |
|---|---:|---|---|---|
| `boundaryConditions/blackBodyRadiation` | 424 | `boundary_conditions/blackbody_radiation.rs` | ported. **2026-11-27: PU**, never compared with the binary (gh:#1871) | unit (4) |
| `boundaryConditions/timeFieldTable` | 434 | `boundary_conditions/time_field_table.rs` | ported. **2026-11-27: PU** (gh:#1871) | unit (6) |
| `boundaryConditions/velocityRundown` | 367 | `boundary_conditions/velocity_rundown.rs` | ported. **2026-11-27: PU** (gh:#1871) | unit (3) |
| `boundaryConditions/NusseltThermalBaffle1D` | 1,190 | `boundary_conditions/nusselt_baffle.rs` | ~~partial: scaffold, every method `unimplemented!()`~~ **CORRECTED 2026-11-27: M** (a scaffold: the two methods that carry the physics, `convective_htc` and `coupled_wall_temperatures`, are `unimplemented!()` with a `TODO(genfoam)`; verified in the file; nothing of the 1,190 lines is translated). Shipped consumer: only `fmuCases/2D_LFRpowerPlant` (`T` and `changeDictionaryDict`), i.e. tied to the FMU decision, gh:#1232 | none |
| `functionObjects/` TBulk, fieldDiffExtents, fieldIntegralToFMU, massFlow, patchScalarFieldValue, pressureDrop, stopIfMaxFieldDiff | 3,062 | `function_objects/*.rs` | ported (~~FMU export itself out of scope~~ **2026-11-27: `fieldIntegralToFMU` (328) is M, blocked on `FMU4FOAM`, gh:#1786; the other six (2,734 lines) are PU**, never compared with the binary although `TBulk` and `massFlow` appear 77 times in the tutorials' `controlDict`s; gh:#1871) | unit (16) |
| `thermophysicalProperties/H2` | 1,415 | `thermophysical/hydrogen/` | ported. **2026-11-27: PU** (9 unit tests against hand calculations; upstream's own test driver `tests/hydrogenThermophysicalProperties` has never been built or compared; binary diffusion and part of the collision-integral machinery are deliberately omitted per the module docs, whether GeN-Foam reaches them is unchecked: gh:#1872) | unit (9) |
| `customPimpleControl/` | 615 | `solver/two_phase/mod.rs` (older hard-wired form), `one_phase_cc/pimple.rs` (full), `one_phase_fc.rs` | ~~missing (`correctUntilConvergence`, `useFirstPISOInitialResidual` residual control)~~ **CORRECTED 2026-11-27: PA, differential.** Read from the code: `correctUntilConvergence` and `useFirstPISOInitialResidual` are in the twoPhase and `one_phase_fc` drivers (rounds 4-5, README "What upstream's `customPimpleControl` does"); the full class (`loop()`, `criteriaSatisfied()`, `residualControl`, the `Final` selection, `updateFirstPISOPrevPIMPLE`, `p_rghFinal` on every outer iteration) is `PimpleState` in `one_phase_cc/pimple.rs` and agrees with the binary in 14 variants (round 31, <= 2.6e-12; two loose-`p_rghFinal` variants 5.5e-7). **Missing:** the `Final` selection and `residualControl` in `twoPhase`/`one_phase_fc` (gh:#1857), `stopIfFirstResRelChangeAboveRelTol` (upstream itself traps a floating-point exception: refused by name, README round 31 finding 1). The draft child gh:#1233 is partly delivered | `genfoam_msfr_th_pimple_vs_upstream.rs` (14), `genfoam_two_phase_vs_upstream.rs` |
| `include/IOFieldField` + helpers | 617 | — | missing (infrastructure; folds into I/O). **Re-checked 2026-11-27, still true** (gh:#1787) | — |
| `include/myOps.{C,H}`, `myOpsTemplates.C`, `notes` | 309 | `one_phase_cc/pimple.rs` (`RelaxationFactors`, `store_prev_iter_fields`) | not in the 2026-11-04 audit (its 926-line row lumped these with `IOFieldField`). **2026-11-27: PD** for `relax`, `relaxationFactor`, `storePrevIterIfRelax` (round 31: `fld_h`, `fld_p`, `fld_kd` at <= 2.0e-12); `fieldInfo` is a diagnostic print and is not ported | `genfoam_msfr_th_pimple_vs_upstream.rs` |

## 5. `neutronics`

| upstream | lines | port | status | tests / V&V |
|---|---:|---|---|---|
| `neutronics.{C,H}` base + `include/` | 827 | `neutronics/mod.rs` (`NeutronicsModel` enum), `state.rs` | ported. **2026-11-27: PD** (through the MSFR chain and the guided slabs) | unit (5) |
| `diffusion/` | 1,218 | `neutronics/diffusion/` incl. `precursor_drift/` | ported (**Re-checked 2026-11-27: PA, differential for the liquid-fuel and guided-slab paths.** ~~Missing: the external neutron source (`externalSourceNeutronics`, `externalSourceFlux*`; no hit in the port; shipped consumer `2D_externalSourceDiffusion`; gh:#1863)~~ **CORRECTED 2026-11-27 (gh:#1863, round 32):** ported, `diffusion/external_source.rs` (the `liquidFuel false`, `eigenvalueNeutronics false` `solveNeutronics.H` step with `ddt` `steadyState`/`Euler`, `corrected`/`uncorrected` laplacian, the source and `pTarget`) and `external_source_case.rs` (reader); against the real binary on `2D_externalSourceDiffusion` (variants A, B, C): power 1.5e-13 to 2.4e-13 at every step, neutron iteration counts equal at every step, flux 1.8e-13 at t = 100 (`tests/genfoam_external_source_vs_upstream.rs`). ~~Still missing: a time profile read from `constant/<region>/externalSource` (refused by name; build it with `ExternalSource::with_modulation`)~~ **CORRECTED 2026-11-27 (gh:#1863, round 32b):** the time profile of `type table` (inline `table table (...)` and `values (...)`, `startTime`) is read and compared with the real binary on two Euler variants: power 2.35e-13 / 2.07e-14 worst over 795 / 678 steps, iteration counts and `deltaT` equal at every step, controls fail (`fmi` and other `Function1` types stay refused). Still missing: the external source with `liquidFuel true` (refused by name), SP3/SN (upstream ignores the flag; the port refuses), ~~the stationary-fuel `step` as a translation (gh:#1818)~~ the `liquidFuel false`, `eigenvalueNeutronics false` form is now a translation (`upstream_stationary_step`; gh:#1818 concerns SP3), the old stationary-fuel `step` as a translation (gh:#1818), `aitkenAcceleration`); ~~transient: single pass, no neutron iteration~~ **CORRECTED 2026-11-25** (gh:#1361 round 6): `upstream_transient.rs` translates `solveNeutronics.H` for `eigenvalueNeutronics false`, `liquidFuel true` (neutron iteration, Gauss-Seidel over groups, `precEq.H` with `ddt`, transport and diffusion); `DiffusionNeutronics::step` is unchanged and remains the simple form. Aitken acceleration not ported (upstream performs it 0 times, see the module doc). **ADDED 2026-11-26 (gh:#1814, round 11):** `upstream_eigenvalue.rs` translates the `eigenvalueNeutronics true`, `liquidFuel true` step (`PseudoTimeError::LiquidFuel` still refuses the OLD stationary-fuel driver for circulating fuel; the new step is the one `steadyStateEN` uses); 600 steps of `2D_MSFR` `steadyStateEN` against the real binary: keff 2.2e-13, flux 1.3e-10, precStar 8e-10 (`tests/genfoam_msfr_chain_vs_upstream.rs`) ~~Linear solves: basic-lib CG / BiCGStab, round-off differences from the binary attributed to the solvers~~ **UPDATED 2026-11-27 (issue 1890, README round 40):** every neutronics linear solve runs upstream's solver (`common/linear_solvers.rs`: `PCG`/`DIC` flux, `PBiCG`/`DILU` precursors, `PBiCGStab`/`DILU` angular flux; OpenFOAM `normFactor` residual, `tolerance`/`relTol`/`maxIter`/`minIter`, zero-iteration exit); the old form is `with_basic_lib_cg()`. External source B, C, F now bit-identical to the binary (power, flux, `pTarget`, residual); A, G 8e-16 / 3e-15. **The precursor `rel_prec_max` (5.8e-14 to 4.2e-12) did NOT move: it was not the solver (round 6's hypothesis refuted), cause open.** `GAMG`: no faithful port (issue 1843), PCG/DIC named deviation. | unit (9); **upstream run**: MSFR ~~+0.3 pcm~~ (a static `k_eff` against a creeping pseudo-time `keff_` resolves no better than the creep, README round 6), ESFR +60.7 pcm (`tests/genfoam_tutorial_keff.rs`); **real binary, `2D_MSFR` transient, 50 steps, gh:#1361 round 6 (2026-11-25)**: power 6.2e-14, iteration counts 50/50, precursor totals 1.3e-11 from upstream's own TH state (`tests/genfoam_msfr_transient_vs_upstream.rs`); **real binary, guided slabs, gh:#1361 (2026-11-12)**: k_eff 6e-14 and flux per cell 6e-13 from upstream at tight tolerances (`tests/genfoam_neutronics_vs_upstream.rs`) |
| `SP3/` | 1,140 | `neutronics/sp3/` | ported. **UPDATED 2026-11-27 (gh:#1887, round 42): the pseudo-time step divides by `discFactor` (`fluxEqSP3.H`, `createFluxMatricesSP3.H`) and calls `adjustDiscFactors(fluxStar_)`; compared with the binary on the df_slab data (given factors `dk` 6.7e-15 over 345 steps, adjustment with a skipped group and zone 1.8e-13); the unstable-adjustment variant agrees only until round-off is amplified (README).** **Re-checked 2026-11-27: PA** (eigenvalue PD on the slabs; the transient is not a translation of `solveNeutronicsSP3.H` (gh:#1818: the stationary-fuel chain is only contract-tested), no SP3 run on the ESFR geometry, `aitkenAcceleration` unported (upstream performs it 0 times); gh:#1788, #1818) **UPDATED 2026-11-27 (issue 1890, round 40):** SP3 linear solves are upstream's `PCG`/`DIC` (SP3 pseudo-time `dk` 4.5e-14 / 1.3e-13 -> 6.7e-15 / 1.6e-14). | unit (5); **real binary, guided slabs, gh:#1361 (2026-11-12)**: k_eff 1.7e-13, flux per cell 2.3e-12 (SP3 ~~"unit (5)" only~~ CORRECTED, it had not been compared with an upstream run before) |
| `SN/` | 1,323 | `neutronics/sn/` | partial: eigenvalue only, transient missing. **Re-checked 2026-11-27: PA, differential for the eigenvalue step** (small S_N slab to 1e-14; `Godiva_SN` one step in `tests/genfoam_godiva_sn_step.rs`, `#[ignore]` because it needs the 78 MB case and runs for minutes; no SN transient: gh:#1234, #1788; `aitkenAcceleration` unported). ~~higher moments missing~~ **CORRECTED 2026-11-12** (gh:#1474): `sn/pseudo_time.rs` + `sn/quadrature_set.rs` port the case `quadratureSet` reader, P1..P7 scattering and upstream's per-step eigenvalue iteration (`SnNeutronics` itself is unchanged). **Re-checked 2026-11-27 (gh:#1474 item c): anisotropic scattering is not open** (`Godiva_SN` declares `legendreMoments 3`; the differential is the slab to 1e-15 per step and the Godiva step to 3.1e-14, README round 2; round 39 adds nothing to `sn/`) **UPDATED 2026-11-27 (issue 1890, round 40):** the angular-flux solves are upstream's `PBiCGStab`/`DILU` (new `pbicgstab_dilu`); the slab history is unchanged to the printed digits (not solver-limited). | unit (6); `tests/genfoam_sn_vs_upstream.rs`: small S_N slab with Godiva's quadrature and P0..P3 scattering, step by step vs the real binary to 1e-14 (README of `verification_and_validation/genfoam_vs_upstream_gh1361`); full Godiva_SN, upstream ~7.5 min per step, 2.9 GB: see that README |
| `albedoSP3/` | 642 | `neutronics/albedo/` | ported. **2026-11-27: PD, looser** | unit (5); upstream run (MSFR boundary rows, ≤ 0.6 pcm) |
| `pointKinetics/` | 2,391 | `neutronics/point_kinetics/`, `multi_region/reactivity_feedback.rs` | ~~ported~~ **CORRECTED 2026-11-27: PA, unit-only.** The module doc itself says it ports "the 0-D reactivity-driven ODE core only" (631 lines; `rho` is an input) and the feedback assembly is `multi_region/reactivity_feedback.rs`. A grep of the port finds none of `GEMReactivityMap`, `TDriveLineRef`, `externalSourceMode`, `subcriticalIndex`, `beamEnergy`, `ksrc`, `powerModulationDelay`, `modulationFactor`, `decayPowerTimeProfile`; the workflow refuses point kinetics by name (no eigenvalue stage). **No run of the real binary has been compared with it** (gh:#1864; shipped cases `1D_MSR_pointKinetics`, `1D_thermalMSR_pointKinetics`, `2D_onePhaseAndPointKinetics*`, `2D_fullCoupling`, `testing/2D_MSFR_ULOF_diffusion_and_pk`, `2D_FFTF`) **UPDATED 2026-11-27 (gh:#1864, README round 36): solid-fuel class PD, differential (bit-identical).** `neutronics/point_kinetics/model.rs` (`PointKineticNeutronics`: constructor, flux-squared weighted feedback averages, the twelve reactivity terms, driveline map, external source `transient` and `powerMonitoring`, `subcriticalIndex`/`ksrc`, beam parameters, `decayPowerTimeProfile`, external/boron profiles, `reactorState` `precursorPowers` restart, `maxDeltaT`) and upstream's `solve()` as the default of `PointKineticsState::step` (the Crout LU stays as `LinearSolveForm::CroutLu`). **Every quantity of 10 case variants x 120 steps is bit-identical to the real `correctPhysics()`** in a free run. Found: upstream ignores `coolantFeedbackZones` (reads `coolFeedbackZones`), so the shipped MSR/FFTF cases average coolant over all cells. **Still missing (refused by name or not run):** the liquid-fuel path `solvePointKineticsLiquidFuel.H` (and with it the shipped `1D_MSR_pointKinetics`, `1D_thermalMSR_pointKinetics`, which are liquid-fuel cases), `GEMReactivityMap`, FMU inputs, time-profile types other than `table`, `reactorState` references and write-out, the case reader and mesh wrapper, the coupled loop and the `workflow` stage, the shipped cases (none run) | unit (+3 model, +1 solver form); `genfoam_point_kinetics_vs_upstream.rs` (5: free run of 10 variants bit-identical, 3 controls, closed form); inhour test (analytic) |
| `XS/` + `nuclearDataOneEnergy` | 2,722 | `neutronics/xs/` | partial: ~~discontinuity-factor adjustment deferred~~ **UPDATED 2026-11-27 (gh:#1785, #1474, README round 39): PA, differential.** Ported and bit-identical to the real `XS` class (`dump_xs_df`, derived from it): `adjustDiscFactors.H` (`xs/disc_factors.rs`), the per-cell `discFactor` fill of `correct()` (the dictionary list is used inside a run even with `adjustDiscFactors false`: 2D_FFTF's 24-group lists were previously ignored), and the control-rod driveline motion of `CRmove` (`xs/driveline.rs`: `CRposition`, follower/rod zone swap; `IV`, `Beta`, `BetaTot`, `lambda` stay with the mesh zone), both acting through `diffusion::XsEvolution` on the diffusion pseudo-time eigenvalue driver (8 slab variants vs the real binary: 6 meet T1-T7 at 1e-13 or better; the two with adjustment AND a rod drive the rod zone's factor to 1e-14 and upstream's own solve then stops unconverged, a recorded miss). **Still missing, refused by name:** ~~SP3 and SN with a factor other than 1, the static/transient/liquid-fuel diffusion steps with a factor other than 1~~ **CORRECTED 2026-11-27 (gh:#1887, README round 42): SP3 pseudo-time (given factors and adjustment), the stationary-fuel step and the static eigenvalue solve now divide by the factors and are compared with the real binary; SN applies no factor upstream (`setXSFieldsSN.H:18` only declares it; binary bit-identical with and without); still refused by name: the liquid-fuel `upstream_eigenvalue_step` / `upstream_transient_step`, the port's own single-pass `step`, the stationary step with an `XsEvolution`**, time-varying feedback fields inside `XsEvolution`, `disp`-driven `radExp`/`axExp` (`axialOrientation`), `isLowMemory`, `secondaryPowerVolumeFraction`. **Earlier text, re-checked 2026-11-27, PA, differential for the MSFR data** (the keys below were missing before round 39): `adjustDiscFactors` / `dfAdjust` / `useGivenDiscFactors`, `TDriveLineRef`, `followerName`, `initialDistanceFromMeshCR`, `axialOrientation`, `secondaryPowerVolumeFraction`, `isLowMemory`. gh:#1361 round 6: per-cell cross sections against GeN-Foam's own `XS` class on `2D_MSFR` (`upstream_driver/dump_xs`): `D`, `nuSigmaEff`, `sigmaPow`, `sigmaRemoval`, `sigmaFromTo`, `IV`, `BetaTot` bit-identical; **`chiPrompt`/`chiDelayed` are interpolated here but never in upstream** (`XS::init()` does not `build()` them): `CrossSectionData::with_upstream_unparametrised_chi()`, default unchanged, default question deferred to the maintainer | unit (14 + 5); `genfoam_df_driveline_vs_upstream.rs` (20) |
| `adjointDiffusion/` | 849 | — | missing. **Re-checked 2026-11-27, still true** (gh:#1234); no shipped runnable case selects it (a grep finds `adjoint` only under `toBeUpdated/2D_RegressionESFR`, which upstream marks as needing an update, and in `2D_FFTF`'s `fvSolution`) | — |

## 6. `thermoMechanics`, `multiRegion`, `common`, others

| upstream | lines | port | status | tests / V&V |
|---|---:|---|---|---|
| `thermoMechanics.{C,H}` + `legacyThermoMechanics/` | ~~1,424~~ **1,636 (CORRECTED 2026-11-27: `legacyThermoMechanics/include/` holds 212 lines more; thermoMechanics 636 + legacy 788 + include 212; the section total 3,125 was right, the row was short)** | `thermo_mechanics/` (`MechanicsMeshSolver`, the older solver) and `thermo_mechanics/legacy_fvm/` (`LegacyFvmThermoMechanics`) | ~~partial: isotropic stiffness, no traction BC, no non-orthogonal correction~~ **CORRECTED 2026-11-26 (gh:#1781, round 23; read from `legacyThermoMechanics.C`, then measured):** upstream's legacy class is ITSELF isotropic (scalar `mu_`, `lambda_`, `threeK_` volFields; no stiffness tensor), and non-orthogonal correction is the `Gauss linear corrected` Laplacian scheme the case selects, not a model feature; both were ported as part of `legacy_fvm`: per-zone field-valued properties from `cellZones`, `TStructFromTH`/`isPorous` forcing with the row-scaled asymmetric `TEqn` (`PBiCG`/`DILU` ported), `compactNormalStress yes` with `leastSquares` or `Gauss linear` gradients and the explicit non-orthogonal correction, `fuelDispEqn`/`CRDispEqn` (`Gauss upwind` along `pinDirection`) and `meshDisp`. ~~**Still missing:** `GFTractionDisplacement`/`gapContact` patches (gh:#1235), `compactNormalStress no` and transient `d2dt2` (gh:#1845),~~ **CORRECTED 2026-11-27:** `GFTractionDisplacement` is ported (round 24, row below), `compactNormalStress no` and Euler `d2dt2` are ported and bit-identical (round 27, gh:#1845, `tests/genfoam_legacy_tm_flux_d2dt2_vs_upstream.rs`, 7 tests; ESI v2506 has no other transient `d2dt2` scheme, so `backward` is refused by name); **status 2026-11-27: PA, differential (bit-identical)**; still missing: `gapContact` in a case (gh:#1848 <- #1846), `d2dt2` on a moving mesh and non-zero initial displacement (gh:#1853), the coupled-run wiring (gh:#1846). **Still missing, as originally listed:** ~~`deformMesh` -> `movePoints` (gh:#1784)~~ (ported in round 26, `multi_region/mesh_deformation.rs`; not yet driven by the coupled run, gh:#1846), `GAMG` (named substitute; gh:#1843), the finite-element version (gh:#1782). | unit (22); **real class and real `GeN-Foam` executable, 3D_SmallESFR mechanics region, 23340 cells, 3 steps: every field bit-identical** (README round 23) |
| `extendedThermoMechanics/` | 455 (405 + `include/` 49: 454) | — | missing (~~also excluded from the upstream build: needs the lost `offbeat` API~~ **CORRECTED 2026-11-27: it is listed in `thermoMechanics/Make/files`, so upstream does NOT exclude it; it cannot be built here because it `#include`s `offbeatTime.H`, `thermalSubSolver.H`, `mechanicsSubSolver.H`, `heatSource.H`, `timeHandling.H`, `sliceMapper.H` from the `src/classes/offbeat/src` submodule, which is absent from the checkout.** M, blocked; NOT out of scope: gh:#1783. Shipped consumers: `3D_SmallESFR_NewSolverVerification/newSolver`, `EMPTY`) | — |
| `tractionDisplacement/` (`GFTractionDisplacement`, `gapContact`) | 1,035 | `thermo_mechanics/legacy_fvm/traction.rs` (+ `DispPatch::GfTractionDisplacement`) | ~~missing~~ **CORRECTED 2026-11-26 (gh:#1235, round 24): `GFTractionDisplacement` (465 lines) ported and bit-identical to the real class (3 cases, 36/36 solves each); `gapContact` (570 lines): arithmetic ported (`GapContact`), unit-tested only, refused in cases; needs the `regionCoupled` coupling (gh:#1846)** | `verification_and_validation/genfoam_vs_upstream_gh1361/README.md`, "Round 24" |
| `multiRegion/meshHandler` | 1,194 | `multi_region/coupling_fields.rs`, `mesh_to_mesh.rs`, `rbf_mapping.rs` | partial: ~~no `movePoints` mesh deformation~~ **CORRECTED 2026-11-26 (gh:#1784, round 26; read from `meshHandler.C`, `meshHandlerTemplates.C` and the three `deformMesh()` bodies, then measured):** `meshHandler` itself has NO `movePoints` (it holds the meshes, `meshToMesh` objects and the `mapAllFields` loops); the deformation is `deformMesh()` of each region solver (`neutronics.C:363`, `thermalHydraulicsModel.C:380`, `thermoMechanics.C:353`), ported as `multi_region/mesh_deformation.rs` (`volPointInterpolation` + `fvMesh::movePoints` geometry; `meshDeformation` dictionary reader; refusals by name), compared with the real `volPointInterpolation`/`movePoints` and the real `legacyThermoMechanics::deformMesh()` (README round 26). **Still missing:** calling it from the coupled loop (regions own no `DeformableMesh`), `meshPhi`/`V0`/`V00` of a moving mesh (gh:#1846), `meshToMesh` `cellVolumeWeight` between different meshes (supermesh, gh:#651), `removeBaffles` copies, `createCouplingFields`. **ADDED 2026-11-26 (round 9, issue 1774):** `coupled_loop::IdenticalMeshMap` (`cellVolumeWeight` between two identical meshes: measured on upstream, identity with weight exactly 1.0; refuses a different mesh), `CouplingDict` (reader); the general supermesh is still missing (issue 651) | unit (15) ~~`meshToMesh` is nearest-cell + global rescale~~ **CORRECTED 2026-11-26 (gh:#651, round 28):** `cell_overlap.rs` is a line-by-line port of ESI `cellVolumeWeightMethod`/`tetOverlapVolume`/`meshToMesh::mapTgtToSrc`, bit-identical on the 3D_SmallESFR meshes (README Round 28); not yet wired into `IdenticalMeshMap` or the loop (gh:#1852). **ADDED 2026-11-27 (round 30, gh:#1852, #1854; README round 30):** the moving-mesh state of `fvMesh::movePoints` (`V0`/`V00`/`meshPhi`, keyed by time index as `deformMesh()` runs every Picard iteration) and the exact `meshToMesh` in the coupled sequence (`coupled_loop` variant, `DeformableMesh::euler_fvm_ddt`) are ported and agree with the real code (`tests/genfoam_coupled_deformation_vs_upstream.rs`, 4 tests, the 3D_SmallESFR meshes). **Status 2026-11-27: PA, differential (bit-identical in rounds 26, 28; round 30 per the README).** Still missing: the Rust region solvers do not yet take `V`/`V0`/`meshPhi` from the moving mesh (gh:#1856), `cell_overlap` is not yet wired into `IdenticalMeshMap`/`MeshHandler` for non-identical meshes and `mapSrcToTgt` of vectors / boundary mapping (gh:#1854), `removeBaffles` / `mergeOrSplitBaffles` (gh:#1236), `createCouplingFields`, point constraints of `volPointInterpolation` (`symmetryPlane`, `wedge`, `empty`, `cyclic`, `processor`: gh:#1851), the octree `findInside` seed (named deviation) |
| `multiRegion/multiPhysicsSolver` | 460 | `multi_region/outer_iteration.rs` | partial. **ADDED 2026-11-26 (round 9):** `coupled_loop::correct_physics_loop` (literal `correctPhysics` loop), `adjust_delta_t`/`initial_delta_t` (`setDeltaT.C`), compared with the real binary on the 2D_MSFR transient. **Re-checked 2026-11-27: PA, differential** (2D_MSFR only: the full transient to 175 s with a multi-iteration loop, README round 22, and the unconverged-loop iterations 2+, round 15; gh:#1832 is open but round 15 is its measurement). Missing: the three-region 3D_SmallESFR run with `legacyThermoMechanics` (gh:#1846), the deformation step driven from a region (gh:#1856) | unit (3); `genfoam_msfr_coupled_vs_upstream.rs` |
| `Allrun` chain `steadyStateEN -> transient` (not a class) | n/a | `workflow/` | **ADDED 2026-11-26 (gh:#1814, round 11):** `EigenvalueStage`/`NormalisedState`/`TransientRun` (enum dispatch: diffusion and SP3 stationary fuel, circulating-fuel diffusion + energy-only `onePhase`); `ReactorState` reads/writes `uniform/reactorState`. ~~`steadyStateTH` (gh:#1812) is refused by name~~ **CORRECTED 2026-11-26 (round 21, gh:#1841):** `steadyStateTH` is ported: `workflow::steady_state_th` (`SteadyStateTh`: the `GeN-Foam.C` loop on the `Time` clock with the `adjustableRunTime` write alignment), `workflow::allrun::{RootCase, AllrunEdits}` (the `Allrun` edits as typed values; `steadyStateTH -> steadyStateEN -> transient` from `rootCase` with the 15 s fluid fields handed over in memory; `multiRegionCouplingDict_steadyStateTH` checked to map nothing); `p_rgh` `GAMG` runs as the NAMED deviation `GamgPolicy::SubstitutePcgDic` (basic-lib GAMG is not a translation, gh:#1843). SN and point kinetics are still refused by name | unit (5); **real binary, 2D_MSFR EN 600 steps then port's own transient 50 steps** (README round 11) |
| `multiRegion/regionSolvers` | 726 | `multi_region/outer_iteration.rs`, `mesh_region.rs` (`RegionKernel`) | partial: the TH region kernel is a lumped stand-in (`MeshThermalHydraulics`), not `onePhase`/`twoPhase`. **Re-checked 2026-11-27, still true for `multi_region::mesh_region.rs` (PA)**; the real one-phase kernel is driven only through `workflow/` (`OnePhaseCcSolver` inside `TransientRun`), not through a `RegionKernel`; there is no region kernel for `twoPhase` or `one_phase_fc` (gh:#1475, #1856) | unit (6) |
| `common/radialBasisFunctionInterpolation` | 1,090 | `common/rbf.rs`, `multi_region/rbf_mapping.rs` | ported | unit; upstream run (MSFR XS parametrisation, 14 pcm); **libCommon driver, gh:#1361 (2026-11-12)**: inverse, weights and values bit-identical to GeN-Foam `libCommon` on 34 cases (weights were NOT before: the solve is now `inv(A) * v` as upstream) |
| `common/timeProfile` | 397 | `common/time_profile.rs` | ported (`commDataLayer` out of scope) | unit (8); **libCommon driver, gh:#1361 (2026-11-12)**: table values bit-identical to upstream on 712 points (ESI `Function1::Table` form; the Foundation form is kept as `value_foundation` and differed in 242 of 667 interior points, up to 24 ulp) |
| `common/InterpolateTable` | 675 | `common/interpolate_table.rs` | ported (1-D); ~~`FieldField` 2-D/3-D instantiations deferred~~ **CORRECTED 2026-11-27:** the `Field` / `FieldField` 2-D and 3-D instantiations are ported in `common/interpolate_table_nd.rs` (gh:#1223; `lookUpTableCHF` is the only consumer, `common/mod.rs` still lists them as deferred: stale doc, flagged below); **PD for the 1-D table (bit-identical), PU-by-use for the N-D one** (reached through the `lookUpTableCHF` variant runs at 6e-4..8e-4, unexplained, gh:#1816) | unit (9); **libCommon driver, gh:#1361 (2026-11-12)**: 618 values, 90 integrals and 504 `getNextPoint` bit-identical to upstream |
| `common/solver` | 457 | enum dispatch in each module | partial (by design). **Re-checked 2026-11-27: PU (by design):** the run-time-selectable region-solver base is `NeutronicsModel` + `ThermalHydraulicsSolver` + `workflow` enums; its `deltaTFactor`/`maxDeltaT` handling is in `coupled_loop::adjust_delta_t` (PD, round 9) | — |
| `common/mergeOrSplitBaffles` | 608 | — | missing | — |
| `common/latticeMap` | 315 | — | missing | — |
| `common/listOperation` | 154 | — | missing (dictionary I/O helper) | — |
| `openFoamImportedSolvers/compressibleInterFoam` | 1,511 (575 + `include/equations` 936) | — | missing. **Re-checked 2026-11-27:** compiled into `libOFSolvers` but referenced by nothing in `src/`, `GeN-Foam/` or `Tutorials/` (grep): the lowest-value row; gh:#1237 asks to check the workspace's own `outram-foam-multiphase` port first | — |
| `openFoamImportedSolvers/scalarTransportFoam` | 373 | — | missing. **Re-checked 2026-11-27:** as above (unreferenced); gh:#1237 | — |
| `offbeat/solvers/fuelBehaviour` | 425 | — | missing (~~couples to the external OFFBEAT library; out of scope~~ **CORRECTED 2026-11-27: M, blocked, NOT out of scope:** `Allwmake` fetches the `src/classes/offbeat` submodule and builds `SCIANTIX`, `offbeatLib` and these solvers; the submodule is absent from the checkout, so the API this class calls cannot be read; gh:#1783 asks to decide the coupling against `outram-park-fork-offbeat`) | — |

## Rows added 2026-11-27 (upstream files the 2026-11-04 audit had no row for)

Checked by listing every `.C`/`.H` under `GeN-Foam-src/{src,GeN-Foam,tests}` (88,169 lines
under `src/classes` plus the application) and matching it to a row.

| upstream | lines | port | status | tests / V&V |
|---|---:|---|---|---|
| `GeN-Foam/GeN-Foam.C` (the application main loop; the `-legacy` option is at lines 102-133) | 227 | `workflow/{mod,steady_state_th,energy_neutro,allrun,reactor_state}.rs`, `multi_region/coupled_loop.rs` | **PD.** `steadyStateTH -> steadyStateEN -> transient` from the root case (round 21), the `Time` clock with `adjustableRunTime` write alignment, the stage chain (round 11), the full 2D_MSFR transient (round 22). The `-legacy` branch (lines 102-133) is not ported (next row). Stationary-fuel diffusion/SP3 stages and SN / point kinetics are refused by name (gh:#1818, #1817, #1844) | `genfoam_msfr_allrun_vs_upstream.rs`, `genfoam_msfr_chain_vs_upstream.rs`, `genfoam_msfr_coupled_vs_upstream.rs` |
| `GeN-Foam/include/setDeltaT.{C,H}` | 167 | `multi_region/coupled_loop.rs` (`adjust_delta_t`, `initial_delta_t`) | **PD** (round 9, `maxDeltaT`/`deltaTFactor` against the binary on the 2D_MSFR transient) | `genfoam_msfr_coupled_vs_upstream.rs` |
| `GeN-Foam/include/modifyDictLegacy.H` (the `-legacy` V1-to-V2 dictionary converter) | 463 | — | **M** (no hit in the port, docs or V&V record); shipped consumer `3D_SmallESFR_old/Allrun_parallel`; gh:#1875 | — |
| `thermalHydraulics/src/include/myOps.{C,H}`, `myOpsTemplates.C` | 309 | `one_phase_cc/pimple.rs` | **PD** for the relaxation helpers (see section 4; the old 926-line `IOFieldField + helpers` row is split) | `genfoam_msfr_th_pimple_vs_upstream.rs` |
| Family bases, run-time-selection factories and macro headers of `physicsModels/` (`*Model.{C,H}`, `new*.C`, `*Models.C`, `runTimeSelectablePairModelMacros.H`, `turbulenceModels/multiphaseCompressibleTurbulenceModels.C`) | 2,930 | folded into the Rust enums of each closure family (`closures/*`) | **PU** (every compared run goes through the enum selections, but the bases carry no unit test of their own; the `new*.C` factories' refusal behaviour for an unknown model name is the reader's refusal by name). The per-family sums are in the rows above | via the families |
| `tests/hydrogenThermophysicalProperties/Test-hydrogenThermophysicalProperties.C`, `tests/radialBasisFunctions/Test-radialBasisFunctions.C` (upstream's own test drivers, built by `Allwmake`) | n/a | `upstream_driver/` of the V&V record uses the RBF driver; the hydrogen driver is not used | RBF driver: **PD** (round 1, `libCommon`, 34 cases bit-identical); hydrogen driver: **unused**, gh:#1872 | `genfoam_common_vs_upstream.rs` |
| `FMU4FOAM/` | empty | — | the submodule is empty in the checkout (`--fmi` builds it); gh:#1786 | — |
| `Tools/` (`openmcToFoam`, `serpentToFoam`, `caseFormatting`, `meshGenerationWithGmsh`, `chebichevLegendreQuadratureSets`, `residualsPlot`: Python) | n/a | — | **OS, not solver logic** (cross-section / quadrature / mesh pre-processing; the workspace has its own `outram-mc-libs` and MGXS path, gh:#1188). Not counted in the 89,026 lines | — |

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

**Checked 2026-11-27 (this refresh).** Claims in `src/` doc comments that the code
contradicts. Not edited in this change (it is the audit only, and the parent task forbids
touching Rust); each is listed so the next change that touches the file corrects it
in place, struck through, per the workspace rule.

| file | stale claim | what the code shows |
|---|---|---|
| `src/genfoam/common/mod.rs` ("Deferred / folded-into-caller") | "The 2-D/3-D `FieldField` `InterpolateTableGF` instantiations ... **Deferred** to `genfoam::neutronics::xs`" | ported in `common/interpolate_table_nd.rs` (gh:#1223); the file is `pub mod`'d on the same page. Also: `common/latticeMap`, `mergeOrSplitBaffles` "Deferred to `genfoam::multi_region` (tracked under the appbuilder epic `op-p6p`)": `op-*` ids are retired, they are gh:#1236 |
| `src/genfoam/thermal_hydraulics/closures/phase_change/latent_heat.rs` (line 24) | "**Deferred:** upstream's `latentHeatModel::adjust()`" | `TwoPhaseSolver::correct_latent_heat` (`solver/two_phase/mod.rs`) translates `correctField` + `adjust` and is in every `1D_boiling` comparison |
| `src/genfoam/thermal_hydraulics/phase/phase_pair.rs` ("Scope: kinematics only ... out of scope here") | true of the module, but a reader will take it as the state of `FFPair`/`FSPair` | the full `FFPair::correct` / `FSPair::correct` assembly exists in `solver::two_phase` (gh:#1877 reconciles) |
| `src/genfoam/thermal_hydraulics/structure/mod.rs` ("Design boundary — what belongs to the porous-solver bead") | the semi-implicit enthalpy source and the assembly into `EEqn` are "deferred to the porous one-phase solver bead (op-p6p.7.11)" | `linearizedSemiImplicitHeatSource` is in `solver::two_phase` and `one_phase_cc`; `op-*` ids are retired |
| `src/genfoam/neutronics/point_kinetics/mod.rs` ("Scope of this port") | already carries a 2026-11-01 correction: "deferred no longer holds for all of it" | re-checked, true; what remains missing is listed in the `pointKinetics/` row (gh:#1864) |
| `docs/genfoam-port-gap-audit.md` itself, the `onePhase/` row of 2026-11-04 | "no `pEqnf` face-centred momentum" etc. | already struck and corrected by rounds 4, 18-20 (above) |

Re-verified and found TRUE (not changed): `nusselt_baffle.rs` scaffold with
`unimplemented!()` (two methods); `ff_htc.rs` deferral of `NoKazimi`; `power_model.rs`
2026-11-03 correction for `nuclearSteadyStatePebble`; `solver/two_phase/mod.rs`
refusals by name quoted in the `twoPhase/` row; `legacy_fvm/mod.rs` "Deliberate omissions"
(round 27/23 text matches the code).

**Not re-checked:** whether `thermalHydraulics/solvers/Make/files` lists exactly the
three files above in every upstream branch (read once, at `652b3da`); the exact
`FSPair`/`FFPair` `minMagU` floors and `residualKd` (gh:#1221 asks to confirm); the
per-model "real-binary run" marks of the closure rows are read from the shipped cases'
model selections, not from instrumenting the run; no test was run (task rule: no `cargo`).

## Draft child issues (for the maintainer to file under epic gh:#1216)

> **2026-11-27:** all twenty drafts have been filed (gh:#1218-#1237 under the epic, plus
> the later children #1474-#1475, #1781-#1788 and the round issues #1801-#1857) and the
> gaps that had no issue are filed as gh:#1863-#1877. The mapping of every remaining
> gap to its issue, with the order of work, is "Remaining work, ordered (2026-11-27)"
> below; the drafts here are kept as written on 2026-11-04 with their corrections.

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
   boiling split~~ (**CORRECTED 2026-11-26**, gh:#1222: `dmdtW`, `fw` and the wall part of `c1`/`c2` are ported), ~~`limitMassTransfer`, `latentHeatModel::adjust()`~~ **CORRECTED 2026-11-27:** both are ported and compared (rows `phaseChangeModel`, latent heat above); gh:#1226 is delivered and proposed for closure.
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

## Tutorial coverage (2026-11-27)

Every shipped case under `Tutorials/` (the acceptance tests of an exhaustive port), with
what a port run against the real binary has reached. "Shipped" means the case files as
upstream ships them. Source of each state: README rounds cited; the blocker issues are the ones
in "Remaining work, ordered".

| case | selects (read 2026-11-27) | state of the port | blocked by |
|---|---|---|---|
| `featureCases/1D_boiling` | twoPhase, MULES `both`, `fixedPower`, `timer`, COBRA-TF, Shah, Kaiser88, byRegime | **shipped, PD** to the onset and through it (p 5e-12, alpha 3e-6; rounds 3, 5, 7); **after the onset** the excursion amplifies 1e-6 to O(1) | #1831, #1811, #1239 (post-onset), #1830 |
| `featureCases/1D_PSBT_SC/Phase_Ex1_12223` | twoPhase, `multiRegimeBoilingTRACE`, `twoParameters`, through-flow BCs | **shipped, PD, all 407 steps** (p <= 5.1e-9, round 13) | none |
| `featureCases/1D_PSBT_SC/PhaseII_Ex1_01_5215` / `Ex2_04_6770` / `Ex3_11-0312` | `multiRegimeBoiling` / `TRACECHF` + `lookUpTableCHF` + Cachard / same, RAS `porousKEpsilon2PhaseCorrected` | **shipped, PD for the first steps** (Ex1 steps 1-25 p 1.5e-9; Ex2, Ex3 steps 1-5 <= 2.6e-10; rounds 16-17); longer runs not compared | #1835, #1834 (vapour-phase k-eps from outer iteration 2, `dgdt.liquid`) |
| `featureCases/1D_CHF/imposedPower` | twoPhase, `constantCHF`, TRACECHF, VapourTRACE, Cachard, `fixedPower` + `powerTimeProfile` | **shipped, PD to 11 s** (alpha 6.4e-10; round 13); the step-1 seed (1.4e-7) is unexplained | #1816, #1830 |
| `featureCases/1D_CHF/imposedTemperature` | `fixedTemperature` + `temperatureTimeProfile` in the two-phase region | **not run** | #1822 |
| `featureCases/1D_HX/onePhase` | onePhase `faceCentered`, heat exchangers, `fixedValue` U inlet, `slip`, `fixedFluxPressure` | ~~**not run**~~ **CORRECTED 2026-11-27 (gh:#1847, README round 44): RUN AS SHIPPED** to its `endTime 20` (2008 steps) against the real binary at the tight protocol (the named `GAMG -> PCG/DIC` deviation): `deltaT` bit-identical at every step, 37,104 `p_rgh` solves with equal counts and iterations, `T` 3.2e-11, `T.HX` 1.9e-11, exchanged heat 1.8e-13 at the end; `p`/`Uz` miss their committed early-step gates (3e-8 / 1e-9, gh:#1894). Status PD with that miss | #1867 (twoPhase half) |
| `featureCases/1D_HX/twoPhase` | twoPhase, MULES `solvePhase vapour`, heat exchangers | **not run** (its `solvePhase vapour` lambda is now ported and compared on 1D_boiling variants, round 34; ~~heat exchangers missing~~ ported in the onePhase regions only, round 44: the twoPhase region still needs them) | #1867 |
| `featureCases/2D_KNS37-L22` | twoPhase, MULES `solvePhase vapour`, `heatedPin`, Rehme, Autruffe, Schor, annular, KottowskiSavatteri | **not run** (its `solvePhase vapour` lambda is now ported, round 34) **2026-11-27 (gh:#1884, round 41): `heatedPin` is wired into the twoPhase region and its `powerDensity.heatedPin` field is read; left: the five closures above, `localTortuosity`, start time -2** | #1869 <- #1230 ; **2026-11-27 (gh:#1879, round 37): `heatedPin` is wired in the one-phase region only; this case needs the two-phase wiring (follow-up filed)** |
| `featureCases/2D_voidMotionNoPhaseChange` | twoPhase, MULES `both`, LottesFlinn, SchillerNaumann, `empty`/`slip` | **never read by the port** | #1868 (none known) |
| `featureCases/2D_cavityBoussinesq` | onePhase, Boussinesq, closed cavity | ~~**not run** (**2026-11-27, round 33: needs `faceCentered` + closed-volume reference cell + Boussinesq thermo in `one_phase_fc`, plus `noSlip`/`empty` patches; none ported there yet, remainder filed**)~~ **CORRECTED 2026-11-27 (round 38, gh:#1878): RUN AS SHIPPED, differential.** The real binary on the shipped files (only `p_rgh` `GAMG -> PCG/DIC`, the named deviation, and the tight protocol) against `OnePhaseFcSolver`: all 3169 steps to 30 s, `deltaT` bit-identical for 17 steps and within 2.2e-12 after, last step `p` 1.6e-14, `p_rgh` 1.6e-14, `T` 2.8e-13, `U` 2.2e-12 / 3.8e-12 of `max |U|` | #1866, #1878 |
| `featureCases/2D_externalSourceDiffusion` | diffusion with an external source | ~~**not run** (no source term in the port)~~ **PD (round 32, 2026-11-27):** variants A (shipped), B (`Alltest`), C (Euler) run in both codes: power 2.4e-13 worst, iterations equal 183/183, 180/180, 549/549 | #1863 (done; ~~time profile and~~ `liquidFuel true` + source remain refused; time profile of `type table` ported in round 32b, power 2.35e-13) |
| `featureCases/2D_fullCoupling` | onePhase + diffusion + `legacyThermoMechanics` (+ point-kinetics transient), `nuclearFuelPin` | **not run** | #1230, #1846, #1864 ; **2026-11-27 (gh:#1879): `nuclearFuelPin` is wired and run in an MSFR variant; this case still needs `hPolynomial`/`icoPolynomial` thermo, `NusseltReynoldsPrandtlPower`, `passiveProperties`, GAMG, legacy thermomechanics** |
| `featureCases/2D_onePhaseAndPointKineticsCoupling`, `..AndSubcriticalPointKineticsCoupling` | onePhase, point kinetics (subcritical: external source), `nuclearFuelPin` | **not run** | #1864, #1230, #1865 (subcritical uses `cellCenteredFaceReconstruction`) |
| `guidedCases/1_*`, `2_*` | diffusion, 1-D slabs | **PD** (k_eff 6e-14, flux 6e-13; round 1) | none |
| `reactorCases/2D_MSFR` | diffusion + onePhase `cellCentered` + `porousKEpsilon`, `fixedTemperature` | **shipped, PD: `steadyStateTH -> steadyStateEN -> transient` from the root case, the whole transient (1347 steps to 175 s), multi-iteration coupling** (rounds 9, 11, 14, 15, 19-22) | #1844, #1832, #1813 (residual items) |
| `reactorCases/3D_SmallESFR_old` | onePhase, diffusion, `legacyThermoMechanics`, `nuclearFuelPin`, `nuclearFuelPinTest`, `-legacy` | mechanics region alone **PD bit-identical** (round 23); coupled run **not run** | #1846 <- #1856, #1230, #1787, #1474; #1875 |
| `reactorCases/3D_SmallESFR_NewSolverVerification/{legacySolver,newSolver}` | as above; `newSolver` selects `extendedThermoMechanics` | `legacySolver` as above; `newSolver` **blocked** | #1846; #1783 |
| `reactorCases/3D_HTR-10`, `3D_gFHR`, `3D_NTPfuelAssembly` | onePhase, `nuclearSteadyStatePebble` / `lumpedNuclearStructure`, H2 | pebble power model **PD** (gFHR, 7.2e-6); whole cases **not run** | #1876 <- #1866, #1854, #1230, #1872 |
| `reactorCases/2D_FFTF` | `nuclearFuelPin`, heat exchanger, `cellCenteredFaceReconstruction`, `momentumSourceTimeProfile`, decay power, GEM | **never read by the port** | #1870 <- #1230, #1847, #1865, #1864 ; **2026-11-27 (gh:#1879): `nuclearFuelPin` is wired; still needs the heat exchanger, `cellCenteredFaceReconstruction`, `momentumSourceTimeProfile`** ~~the heat exchanger~~ **2026-11-27 (round 44): the same-region heat exchanger is ported in the cell-centred region (its `heatExchangers` block pairs two zones of the same mesh; a grep of its `phaseProperties` finds no `secondaryRegion`, checked 2026-11-27)** |
| `reactorCases/1D_MSR_pointKinetics`, `1D_thermalMSR_pointKinetics` | point kinetics, `fixedTemperature` / `lumpedNuclearStructure` | **not run**; point kinetics never compared with the binary | #1864, #1230 ; **2026-11-27 (gh:#1879): `lumpedNuclearStructure` is wired in the `cellCentered` region (incl. `fvc::laplacian`); the case is `faceCentered` with `momentumSourceTimeProfile` and point kinetics** |
| `reactorCases/Godiva_SN` | SN, 23 groups, P3 | one step **PD** (`#[ignore]`d, many minutes); more steps not run | #1788, #1234 (SN transient) |
| `testing/2D_MSFR_ULOF_diffusion_and_pk` | diffusion + point kinetics, `cellCentered`, `nNonOrthogonalCorrectors 1`, `constantTemperature` saturation | **not run**; its TH `PIMPLE` block (`nNonOrthogonalCorrectors 1`, one outer iteration) **PD against the real binary at 2.3e-12 (round 33, variant `no_ulof`)** | #1864, #1866 |
| `toBeUpdated/2D_RegressionESFR` | an old ESFR case upstream keeps under `toBeUpdated` | not run; **upstream itself marks it as needing an update** (directory name): not a target until upstream fixes it | n/a |
| `fmuCases/*` (`2D_LFRpowerPlant`, `2D_PKCoupleFMI`, `powerTemperatureMomentumControl`) | FMU power models, `fieldIntegralToFMU`, `NusseltThermalBaffle1D`, `NusseltAndWall` | **not run**; need `FMU4FOAM` | #1786, #1232 |
| `EMPTY` (template case) | `extendedThermoMechanics` | n/a | #1783 |

## Remaining work, ordered (2026-11-27)

Every PA / M row of the tables above with its issue, ordered by **(a) dependency** (what
blocks what), **(b) whether a shipped upstream tutorial exercises it** (the case is the
test), **(c) size** (upstream lines, smaller first within a tier). "NEW" marks an issue filed in
this refresh. No issue is closed by this change; "proposed for closure" means the
cited round delivers what the issue asked for and the maintainer decides.

**Proposed for closure (delivered, issue still open):** gh:#1219 (MULES, `solvePhase both`,
round 7; the `solvePhase` remainder is #1799), #1222 (TRACE family), #1223 (`lookUpTableCHF`),
#1224 (Cachard), #1225 (`twoParameters`, `byRegime`), #1226 (`dmdtW`, `limitMassTransfer`:
verified in `two_phase/mod.rs`), #1803 (cause found in round 13), #1820 and #1821 (round 13:
shipped PSBT to its end, shipped CHF to 11 s), #1837, #1838, #1839 (rounds 19-20), #1842
(round 29), #1845 (round 27), #1852 (round 30), #1855 (round 31; its remainder is #1857). Also delivered in
substance, with their remainders re-filed as the issues named: #1231 `onePhase` to parity (rounds 4, 18-22, 29,
31; remainders #1865, #1866, #1867; its `onePhaseLegacy` half is out of scope), #1233
`customPimpleControl` (rounds 4-5, 31; remainder #1857).

### Tier 1: no dependency, and a shipped tutorial is blocked on it (start now)

| # | issue | row(s) | upstream lines | tutorial | notes |
|---|---|---|---:|---|---|
| 1 | ~~**#1863 NEW** diffusion external neutron source~~ **DONE 2026-11-27 (round 32)** | `diffusion/` ~~PA~~ PD for the stationary-fuel form | ~940 read | `2D_externalSourceDiffusion` | smallest, self-contained; `neutronics/diffusion/` only |
| 2 | **#1866 NEW** onePhase `nNonOrthogonalCorrectors`, closed volume, Boussinesq (**2026-11-27, round 33: the cell-centred `nNonOrthogonalCorrectors` branch is ported and PD; the closed-volume reference cell and Boussinesq were already in `one_phase_cc` (value branch); ~~the `one_phase_fc` remainder is filed~~ **CORRECTED 2026-11-27 (round 38, gh:#1878): the `one_phase_fc` remainder is ported (loop, reference cell, Boussinesq, `noSlip`/`empty`) and PD on `2D_cavityBoussinesq`; `3D_HTR-10` (`fixedFluxExtrapolatedPressure`, `perfectGas`/`hPolynomial`, 3-D) is not run**) | `onePhase/` PA | part of 1,022 | `2D_cavityBoussinesq`, `3D_HTR-10`, ULOF | `one_phase_cc/flow.rs`, `one_phase_fc.rs` |
| 3 | **#1868 NEW** `2D_voidMotionNoPhaseChange` as shipped | `twoPhase/`, MULES | verification | itself | find every refusal it hits first; cheapest test of MULES `both` + `LottesFlinn` |
| 4 | ~~#1799 MULES `solvePhase` (`solveAlpha`, 9-argument `explicitSolve`)~~ **done 2026-11-27, round 34 (proposed for closure)** | `MULES` PA | part of 1,765 | `1D_HX/twoPhase`, `2D_KNS37-L22` | `two_phase/{mules,alpha_mules}.rs` + `case.rs` |
| 5 | #1822 twoPhase `fixedTemperature` + `temperatureTimeProfile` | `twoPhase/` PA | 394 (`fixedTemperature`) + parts of `structure.C` | `1D_CHF/imposedTemperature` | `two_phase/case.rs` Structure; the `one_phase_cc` form exists (round 22): port, do not rewrite |
| 6 | #1865 **NEW** `cellCenteredFaceReconstruction`, `porousInterfaceSharpness` | `onePhase/` PA | `pEqn_1p.H` 90 + `reconstructU_1p.H` | `2D_FFTF` | after #1866 (same file `one_phase_cc/flow.rs`) |
| 7 | #1857 `Final` selection, `residualControl`, `relaxationFactors default` for `twoPhase` / `one_phase_fc` | `customPimpleControl` PA | part of 615 | none directly (shipped cases select `Final` entries; check) | `two_phase/mod.rs`, `one_phase_fc.rs`; the cell-centred form (`pimple.rs`) is the template |
| 8 | #1230 radial-conduction power models: `heatedPin` 822 -> `nuclearFuelPin` 1,533 -> `lumpedNuclearStructure` 759 -> `interpolatedNuclearFuelPin` 352 | `power` M x4 | 3,466 | `2D_KNS37-L22`; `2D_FFTF`, `2D_fullCoupling`, `3D_SmallESFR_*`; `1D_thermalMSR`, NTP, gFHR lumped; (none) | largest blocker of tutorials (10 cases); new files under `structure/`, each with its own case variant |
| 9 | #1787 `nuclearFuelPinTest` 1,416 (+ `fluid::correct`, `listOperation`, `IOFieldField`) | `power` M, `include` M | 1,416 + 771 | `3D_SmallESFR_*` | after the `nuclearFuelPin` of #1230 (it is a test variant of it) |
| 10 | #1864 **NEW** `pointKinetics` to parity | `pointKinetics` PA | 2,391 | 7 cases | the 0-D core exists; the external-source option needs #1863 |
| 11 | #1785 `dfAdjust` / `adjustDiscFactors`; #1474 driveline motion (`TDriveLineRef`, `followerName`) | `XS` ~~PA~~ **PA, differential for the diffusion eigenvalue driver (round 39)** **and, round 42 (gh:#1887), for SP3 pseudo-time, the stationary-fuel step and the static solve** | part of 2,721 | `3D_SmallESFR_old`, `2D_FFTF` | `neutronics/xs/` |

### Tier 2: needs a Tier 1 item, a shipped tutorial exercises it

| # | issue | needs | upstream lines | tutorial |
|---|---|---|---:|---|
| 12 | #1854 wire `cell_overlap` into `IdenticalMeshMap` / `MeshHandler`, vector `mapSrcToTgt`, boundary mapping | round 28 (done) | part of 1,194 | gates #1847, #1856 |
| 13 | ~~#1847 heat exchangers in the cell-centred region~~ **done 2026-11-27, round 44 (proposed for closure; remaining work gh:#1894, #1895)** | #1854 (zone-to-zone `meshToMesh`), `heatExchanger` 587 | 587 + `structure.C` parts | `2D_FFTF`, `1D_HX` |
| 14 | #1867 **NEW** `1D_HX` onePhase then twoPhase as shipped | #1847; twoPhase half needs #1799 | verification + `one_phase_fc` BCs | `1D_HX` |
| 15 | #1856 region solvers take `V`/`V0`/`meshPhi` from the moving mesh; drive `deform_region_mesh` from the coupled loop | #1854 | ~3 files | `3D_SmallESFR_*` |
| 16 | #1846 `legacyThermoMechanics` inside the coupled ESFR run | #1856, #1854, #1230 (`nuclearFuelPin`), #1787, #1474, the 3-D onePhase/diffusion reaching the ESFR (read from #1846's body) | n/a | `3D_SmallESFR_old`, `2D_fullCoupling` |
| 17 | #1869 **NEW** `2D_KNS37-L22` as shipped | #1230 (`heatedPin`), #1799 | verification | itself |
| 18 | #1870 **NEW** `2D_FFTF` as shipped (+ `momentumSourceTimeProfile`) | #1230, #1847, #1865, #1864 | pump 297 + acceptance | itself |
| 19 | #1876 **NEW** `3D_HTR-10`, `3D_gFHR`, `3D_NTPfuelAssembly` as shipped | #1866, #1854, #1230, #1872 | verification | the three cases |
| 20 | #1848 / #1235 `gapContact` differential, `GFTractionDisplacement` with the flux form | #1846 (`regionCoupled`), #1853 | `gapContact` 570 | `3D_SmallESFR_*` |

### Tier 3: no tutorial is blocked (verification of ported code, and numerical investigations), any time

| # | issue | upstream lines | what |
|---|---|---:|---|
| 21 | #1873 **NEW** drag / heat-transfer closure sweep, `byRegime` for the rest | ~9,900 (closures) | unit-only models get a real-binary variant each (`Churchill`, `Colebrook`, `Rehme`, `Autruffe`, `KottowskiSavatteri`, ...) |
| 22 | #1874 **NEW** phase-change / interfacial closure sweep, `filmCondensationModel` | ~9,500 | same method |
| 23 | #1871 **NEW** function objects and BCs vs the binary | 3,062 + 1,225 | `postProcessing/` numbers, minimal BC cases |
| 24 | #1872 **NEW** H2 properties vs `Test-hydrogenThermophysicalProperties` | 1,415 | gates #1876 |
| 25 | #1877 **NEW** reconcile unit-only modules with the solver-local implementations | n/a | one implementation per class (kovan-stamp rule applies) |
| 26 | #1840 onePhase turbulence-off steps 4-14 and the zero-iteration `convergedPISO` branch | n/a | verification only |
| 27 | #1834, #1835: two-phase turbulence wall functions, vapour-phase k-epsilon from outer iteration 2, `dgdt.liquid`; `DhStruct` in the two-phase reader | part of 3,663 | `two_phase/turbulence.rs` only |
| 28 | #1228 `LaheyKEpsilon` (592) and `mixtureKEpsilon` (1,145) solving | 1,737 | no shipped case solves them; needs variants |
| 29 | #1221 FFPair/FSPair remainder (anisotropic `Kd`, `minMagU`, `residualKd` confirmation) | part of 1,867 | after #1229 for the anisotropic part |
| 30 | #1229 `structure` field layer: tortuosity, local frames, `byZone`, passive structures | 1,689 + 711 | touches `two_phase/mod.rs`, `one_phase_cc.rs`: serial with the solver streams |
| 31 | #1830, #1831, #1811, #1239, #1816: seeds and the post-onset stall of the boiling runs | n/a | open investigations; keep upstream's form as the default |
| 32 | #1220 `pEqn_2p.H` cell-centred and `explicit`/`none` partial elimination | ~500 (`pEqn_2p.H` 414) | no shipped case selects them; port last of the twoPhase items |
| 33 | #1788, #1818, #1817, #1844, #1832, #1813: SN/SP3 transients and the case-driven chain | n/a | `workflow/`, `neutronics/{sn,sp3}` |
| 34 | #1234 `adjointDiffusion` (849) and the SN transient | 849 | no runnable shipped case selects it |
| 35 | #1850, #1849, #1851, #1853, #1843: FEM remainder, `zeroGradient` traction investigation, `volPointInterpolation` constraints, moving-mesh `d2dt2`, GAMG | n/a | `thermo_mechanics/fem`, `multi_region/mesh_deformation.rs` |
| 36 | #1475 `RegionKernel`s for `onePhase` / `twoPhase` (replace the lumped stand-in) | 725 | after #1856 |

### Tier 4: infrastructure and blocked (decide, then port)

#1236 `mergeOrSplitBaffles` 608 + `latticeMap` 315 (needed by `removeBaffles`, meshHandler);
#1787 `listOperation` 154, `IOFieldField` 617; #1232 `NusseltThermalBaffle1D` 1,190 (shipped
consumer only `fmuCases/2D_LFRpowerPlant`); #1875 **NEW** `-legacy` / `modifyDictLegacy.H` 463
(`3D_SmallESFR_old`); #1237 `compressibleInterFoam` 1,511 / `scalarTransportFoam` 373
(referenced by nothing); #1227 FF `NoKazimi` htc 258 (no shipped case); **blocked on an absent
submodule:** #1786 FMU (2,545 lines), #1783 OFFBEAT / `extendedThermoMechanics` 454 +
`fuelBehaviour` 425. **Out of scope:** `onePhaseLegacy` 985 (#1231's second half; upstream does
not build it).

### Parallel streams (disjoint files, so they can run at the same time)

| stream | items (in order) | files touched |
|---|---|---|
| **A, neutronics** | #1863 -> #1864; #1785 and #1474 in parallel with them | `neutronics/diffusion/`, `neutronics/point_kinetics/`, `neutronics/xs/` (three disjoint directories: all three can run together) |
| **B, one-phase flow** | #1866 -> #1865 -> #1847 -> #1867 | `solver/two_phase/one_phase_cc/**`, `one_phase_cc.rs`, `one_phase_fc.rs` (serial: same files) |
| **C, two-phase solver** | #1799 -> #1822 -> #1857 -> #1220 | `solver/two_phase/{mod,case,mules,alpha_mules}.rs` (serial: `case.rs`, `mod.rs`) |
| **D, power models** | #1230 (heatedPin first) -> #1787 | new files under `structure/`; the reader hooks in `case.rs` / `one_phase_cc.rs` are the only overlap with B and C (a few lines: coordinate) |
| **E, mechanics and multi-region** | #1854 -> #1856 -> #1846 -> #1848; #1853, #1850, #1849, #1851, #1843 alongside | `multi_region/**`, `thermo_mechanics/**` |
| **F, verification only** | #1868, #1871, #1872, #1873, #1874, #1840 | `tests/`, the V&V refs submodule, case variants; library files only if a defect is found |

**Suggested parallel pairs (no shared file):** (#1863, #1866); (#1799, #1230); (#1864, #1854);
(#1785, #1822); (#1866, #1799); (#1872, #1871); (#1873, #1856). **Not parallel:** #1866 with
#1865 or #1847 or #1867 (same `flow.rs`); #1799 with #1822 or #1857 (same `case.rs` /
`mod.rs`); #1229 with any of B or C.

**Dependency chain to remember (the longest):** #1854 -> #1856 -> #1846 -> #1848 (mechanics,
`3D_SmallESFR_*`), with #1230 (`nuclearFuelPin`) and #1787 as side inputs of #1846;
#1799 + #1230 -> #1869 (`2D_KNS37-L22`); #1847 (after #1854) + #1799 -> #1867 (`1D_HX`);
#1230 + #1847 + #1865 + #1864 -> #1870 (`2D_FFTF`).
