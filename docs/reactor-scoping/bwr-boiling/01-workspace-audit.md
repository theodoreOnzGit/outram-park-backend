# Workspace audit — boiling & two-phase heat transfer (epic #895)

Audit date 2026-10-16. Read-only; nothing in the repo was edited. All paths are
relative to `/home/user/outram-park-backend`. "VERIFIED PRESENT" = I opened the
file. "NOT FOUND" = grep over `crates/**/*.rs` (excluding the literature
submodule) returned nothing, search terms listed.

## 0. Bottom line

The workspace has a lot of **closures** (CHF, RPI wall boiling, Chen-type
superposition, two-phase multipliers, drift-flux slip, interfacial drag/area,
regime map) spread across **four independent lineages**, and **no heated
boiling-channel solver that uses them**. Nothing boiling-related is validated
against boiling experiments. The only experimental two-phase validation is
critical flow (Marviken, Moody/Zaloudek) and blowdown (Edwards).

Four lineages, which do not share code with each other:
1. `outram-foam-multiphase`: OpenFOAM `multiphaseEuler` / `incompressibleDriftFlux` + literature CHF.
2. `outram-foam-appbuilder-lib/src/genfoam`: GeN-Foam porous TH closures (TRACE-style).
3. `bedok`: Than Yan Ren's MATLAB (HEM + Zuber-Findlay void, W-3 DNBR).
4. `outram-park-fork-dwsim-libs/src/pipe`: DWSIM pipe correlations (Lockhart-Martinelli, Beggs-Brill).
Plus `tampines/src/multiphase_1d`, which consumes lineage 1 through its public API.

## 1. Code inventory

### 1a. `crates/outram-foam-multiphase` (port of OpenFOAM-dev + literature correlations)
Status, from `crates/outram-foam-multiphase/README.md:30-50`: every stage is a "foundation", meaning "unit-tested translations, not benchmark-validated". The crate CLAUDE.md has only the geometry-drawing rule and no status notes.

| Model | Where | Upstream | V&V |
|---|---|---|---|
| CHF: Biasi (1967), W-3/Tong (1967), Bowring (1972), Groeneveld LUT framework; `ChfCorrelation` enum | `src/chf.rs:183` (enum), `:302` Biasi, `:389` W3, `:564` Bowring, `:694` GroeneveldLut | literature correlations | Verification only: hand-computed points (`chf.rs:73-84` states "No benchmark validation against experimental CHF databases"). **The Groeneveld LUT holds no real table data**: synthetic sample grid plus a CSV loader (`chf.rs:666-680`). |
| RPI wall boiling (Kurul & Podowski 1991) with Lemmert-Chawla site density, Tolubinski-Kostanchuk departure diameter, Cole frequency, Del Valle-Kenning area | `src/wall_boiling.rs:280` `RpiPartitioning`; sub-closure table `:60-82` | OpenFOAM `fvModels/wallBoiling/wallBoiling.C` and its sub-model classes (cited per item) | Unit tests `:692-830`; no experiment |
| Wall-boiling regime arms CHF / subcooled CHF / dryout / film boiling | `src/wall_boiling.rs:517,538,559,581` | — | **Return `NotImplemented`**. The CHF correlations in `chf.rs` are not wired into them. |
| Dryout onset (critical-void worked closure) + post-dryout (Dougall-Rohsenow film boiling) | `src/dryout.rs:250` `DryoutOnsetModel`, `:359` `PostDryoutModel` | literature | `CriticalHeatFlux`, `LiquidFilmDepletion`, `FilmBoilingMechanistic`, `TransitionBoiling` and `ChfRecovery` return `NotImplemented` (`:305,:311,:476-486`) |
| Drift-flux mixture, algebraic slip (Terminal velocity, **Zuber-Findlay** `C0 j + Vgj`), α transport | `src/drift_flux.rs` | OpenFOAM `incompressibleDriftFlux` (file:line table at `:28-40`) | Unit tests, plus the Dahl settling case `crates/outram-foam-appbuilder-lib/tests/foam_vv_cases/main.rs:294-340` (closure exactness + runs/settles, **not a boiling case**) |
| Euler-Euler two-fluid (Schiller-Naumann, Wen-Yu drag), PIMPLE loops | `src/two_fluid.rs`, `src/two_fluid_pimple.rs`, `src/pimple.rs` | OpenFOAM `multiphaseEuler/phaseSystem` | Unit tests only. Incompressible, constant density, **no energy equation, no phase change** |
| Interfacial heat transfer K (Ranz-Marshall etc.) | `src/heat_transfer.rs` | OpenFOAM `multiphaseEuler` | Unit tests |

### 1b. `crates/outram-foam-appbuilder-lib/src/genfoam/thermal_hydraulics/closures` (port of GeN-Foam)
Module status (`closures/mod.rs`): "All six families are implemented … **unit-tested, not system-validated** — none has been exercised inside a converged multiphysics run."
**The two-phase GeN-Foam solver driver is not ported.** `genfoam/thermal_hydraulics/solver/mod.rs:64-82`: `ThermalHydraulicsSolver` has only `OnePhase`, with a TODO for bead `op-p6p.7.12` (twoPhase MULES + pEqn).

| Model | Where | Notes |
|---|---|---|
| Multi-regime boiling dispatcher (TRACE philosophy: condensation / FC / ONB / Chen flux- or htc-superposition) and `superpositionNucleateBoiling` | `closures/heat_transfer/boiling.rs` (269 lines) | Two upstream quirks reproduced verbatim and flagged (`boiling.rs` header). The `dmdtW` subcooled-boiling mass-source side effect is returned as values, not written |
| CHF (**`Constant` only**), Leidenfrost (Groeneveld-Stewart), ONB temperature (**Basu**), flow-enhancement F (**Chen**, Rezkallah-Sims, COBRA-TF), suppression S (**Chen**, COBRA-TF), subcooled-boiling fraction (constant, **Saha-Zuber**) | `closures/heat_transfer/chf.rs:111,129,172,251,339,420` | **Deferred, not ported:** `lookUpTableCHF`, the `multiRegimeBoilingTRACE*` variants, the Cachard post-CHF models (upstream is dimensionally inconsistent and uom refuses it) (`heat_transfer/mod.rs` "What is deferred"). A Saha-Zuber `k`-division deviation from upstream is documented (`heat_transfer/tests.rs` header) |
| Pool boiling: Shah (liquid metal), Gorenflo; Nusselt FC | `closures/heat_transfer/fs_htc.rs` | Shah here is the liquid-metal pool-boiling model, **not** Shah flow boiling |
| Two-phase friction multipliers: LockhartMartinelli (Chisholm C), ChenKalish, Kaiser74/88, KottowskiSavatteri, LottesFlinn, LottesFlinnNguyen | `closures/ff_drag/multipliers.rs` | Algebra-only port of `twoPhaseDragMultiplierModels` |
| Interfacial drag: Wallis, Schiller-Naumann, **Bestion / BestionTRACE**, Autruffe, NoKazimi | `closures/ff_drag/` (see `ff_drag/mod.rs`) | Bestion is effectively the drift-flux-derived drag used by PSBT |
| Phase-change rate (heat-driven, 4 modes), saturation, latent heat | `closures/phase_change/{rate,saturation,latent_heat}.rs` | Wall-boiling `dmdtW` split deferred |
| Interfacial area, diameter, virtual mass, contact partition, one-parameter regime map | `closures/interfacial/*.rs` | `regime_map.rs` reproduces upstream's measure-zero junction quirk |
| Bubble-induced turbulence (Lahey) | `closures/turbulence/` | algebra only; k-ε transport deferred |
| HRMFoam (Downar-Zapolski relaxation, flashing) | `src/solvers/hrm_foam/mod.rs` | 3-D flashing, not wall boiling |
| reactingTwoPhaseEulerFoam with interfacial mass transfer | `src/solvers/reacting_two_phase_euler_foam/mod.rs:45,273` | interfacial phase change; no wall-boiling model found in it |

**PSBT V&V that exists** (`crates/outram-foam-appbuilder-lib/tests/genfoam_psbt_closures.rs`, data `tests/data/genfoam_psbt_fs_drag.csv`; doc `crates/outram-foam-appbuilder-lib/docs/genfoam-tutorial-verification.md:57,66,170-190`):
- What it is: code-to-code verification of the **single-phase `ReynoldsPower` wall friction only**, against a run of upstream GeN-Foam `652b3da` / OpenFOAM v2506 on `1D_PSBT_SC` case `PhaseII_Ex1_01_5215`.
- The upstream run reproduced the tutorial's `alpha.vapour = 0.123835`, T = 620.178.
- Result: 836 single-phase cells, worst deviation 6.15e-6. The 334 two-phase cells are a negative control (0.53).
- **The port cannot compute PSBT void**, because it has no two-phase driver.
- **The boiling, multiplier and Bestion closures that PSBT names are not checked against that run.**

### 1c. `crates/tampines` — 1-D system-code two-phase
- `src/multiphase_1d/drift_flux.rs` (1597 lines): `DriftFlux1d` is a 4-eq compressible semi-implicit marcher (IF97 flash).
  - Slip comes from `outram_foam_multiphase::drift_flux::SlipModel` (Zuber-Findlay).
  - Wall friction is single-phase Darcy on mixture properties (`:1474-1486`). **No two-phase multiplier.**
- `src/multiphase_1d/two_fluid.rs` (2488 lines): `TwoFluid1d`, 6-eq, with prescribed bubble diameter. Regularisation is discussed in `crates/tampines/docs/six-equation-regularisation.md`.
- Honest-scope list, `src/multiphase_1d/mod.rs:80-120`: **no wall heat transfer (adiabatic), no regime map, no interfacial area transport, no CCFL**. Break flow is HEM.
- V&V:
  - Edwards-O'Brien GS-1 pressure: `edwards_tests.rs:847`, RMSE 29.0 psia.
  - Marviken 23/24 at drift-flux fidelity: `marviken_tests.rs:1-60`, test 23 VALIDATED with 8.9 % mean deviation, test 24 not validated.
  - Both are depressurisation, **not boiling heat transfer**.
- `src/hem/mod.rs` and `src/critical_flow/mod.rs`: thin re-exports of `tampines-steam-tables` HEM / choked flow.
- `src/components/steam_generator.rs:1-40`: HEM secondary. `advance` is not implemented.
- `examples/fhr_sim_v2/.../secondary_loop/pool_boiling.rs`, plus copies in `outram-park-digital-twin-engine/examples/fhr_sim_v2/...` and `tampines-steam-tables/examples/fhr_sim_v1/...`: a **"dummy DNB type model"**, a hand-shaped UA curve and not a correlation (file header lines 1-30).

### 1d. `crates/bedok` (Rust translation of SNRSI MATLAB, author's permission)
- `src/singleflow1devap.rs`: steady HEM channel, enthalpy march + equilibrium quality.
  - Void comes from Zuber-Findlay with `C0 = 1.2` and `Vgj = √2 (Δρ g σ/ρ_l²)^¼` (`:366-395`).
  - Constant pressure, no subcooled boiling, no density-wave (`:30-42`).
- `src/w3chf.rs`, `src/w3chfhottest.rs`: W-3 (Tong) CHF and DNBR on the hot channel. Unit-folding checked term by term (`w3chf.rs:1-40`).
- `src/driftflux6_solverstatic1d.rs`: the six-equation per-channel kernel **is absent from the MATLAB snapshot**. The wrapper returns `ChannelOutcome::SolverMissing` (`:1-50`).
- V&V: NEACRP D1 (BWR, 6.7 MPa, boiling coolant). k_eff matches the MATLAB to every digit (`crates/bedok/README.md`), and D1t transient matches the MATLAB to 2.9e-11.
  - This is **code-to-code against MATLAB, not against a published result** (README: "No transient has been compared to a published curve").
  - NEACRP-L-335 is catalogued **proprietary** (`crates/kovan-literature/CATALOGUE.md:624-660`).

### 1e. `crates/outram-park-fork-dwsim-libs/src/pipe` (port of DWSIM VB.NET)
- `lockhart_martinelli.rs`: Lockhart-Martinelli (1949) separated-flow multiplier pressure drop, ported from `FluidFlowCorrelations/LockhartMartinelli.vb`.
- `beggs_brill.rs`: Beggs & Brill (1973).
- `petalas_aziz.rs:63`: stub returning `!`, because upstream is an unshipped DLL.
- These are oil/gas pipe correlations. There are 14 `#[test]` in `pipe/`, and no tests/ directory hits.

### 1f. `crates/tampines-steam-tables`
- HEM critical flow, using Saha 1978 NUREG/CR-0417 eq. 10 (`choked_flow/mod.rs:549`). Note this is *Saha's critical-flow review*, not Saha-Zuber.
- Validated against Moody and Zaloudek (graph-read HEM curves). Marviken test 23 is validated and test 24 is not (`converging_diverging_nozzles/tests/marviken_tests.rs:139-142`).
- Edwards blowdown: `tests/edwards_blowdown.rs`.
- KNP/rhoPimpleFoam steam array with boiling SG tube: `src/openfoam_algorithms/rhoPimpleFoam/lateral_coupling.rs`, test `tests/heated_tube_never_undercuts_its_feedwater.rs`. This is the subject of sibling epic **#896**.
- `src/openfoam_algorithms/driftFluxFoam/mod.rs`: commented-out C++ reference only.

### 1g. Others checked
| Crate | Finding |
|---|---|
| `outram-park-fork-coolprop` | KNP duplicate (#896); hits were VLE/"drift" wording only |
| `tuas_boussinesq_solver` | Single-phase; the only hit is a copper-property citation of a pool-boiling paper (`solid_database/copper.rs:28`) |
| `outram-park-digital-twin-engine` | `src/components/reactor_archetype.rs:65` `Bwr` is a **drawing plus illustrative kinetics** (`:200`), not physics. Its scoping doc is `docs/reactor-scoping/bwr.md` |
| `outram-mc-libs`, `farrer-park`, `raffles`, `liggghts`, `offbeat`, `pflotran`, `njoy` | False positives ("void" cells, porosity, "Armand"-like strings) |

### 1h. NOT FOUND (grep over `crates/**/*.rs`, excluding the literature submodule)
- **Boiling, void and CHF correlations:** Jens-Lottes, Thom (`\bThom\b`), Chexal(-Lellouche), Bankoff, Armand, Friedel, CISE, Hench-Levy, Levy, Rouhani, Ishii, Hibiki, Kocamustafaogullari, Ünal, Martinelli-Nelson, Woldesemayat/Ghajar.
- **Datasets:** Bartolomei, Christensen, DEBORA, BFBT. No CSV or test files with these or `*psbt*` / `*bfbt*` names exist, except the PSBT drag fixture above.
- **Density-wave / stability:** "density wave" appears once, only as a stated *absence* (`bedok/src/singleflow1devap.rs:33`).
- **Not built anywhere:** a heated boiling channel, a separator or dryer model, or a real Groeneveld 2006 LUT dataset.
- **Repo-level:** `reference-data/` holds no boiling data. `reference-data/virtual_test_bed/lwr/` holds only `hcf` and `rpv_fracture`.

## 2. Literature held

- **`reactor-literature` submodule: EMPTY.** `git submodule status` shows `-6a28920e…` (not initialised), and `ls` shows 0 entries. I did not fetch it.
- **`crates/kovan-literature/CATALOGUE.md`** (761 lines; organised as an Open tier, a proprietary tier and a librarian history; a `KovanDocument` JSON per entry). The boiling-relevant entries are:
  - `nrc1982marviken`, NUREG/CR-2671 (open; `CATALOGUE.md:238-260`; JSON+MD in `open/`/`generated/`). Critical flow only.
  - `1991finnemann-neacrp-l-335`, NEACRP BWR D1 spec (**proprietary**; `:624-660`).
  - `gao2002htr10th` and others are HTGR TH (RELAP5 mention at `:513` is HTR-10 accident analysis), not boiling.
- **Kovan hardcoded corpus** (`crates/kovan/src/corpus.rs`, 131 ids). There is no boiling-specific source.
  - Most relevant: DOE-HDBK-1012/1-3-92 *Thermodynamics, Heat Transfer and Fluid Flow* (`corpus.rs:1541,1562,1583`). This is a general-level text that covers boiling regimes, DNB and two-phase flow.
  - Also present: NUREG-0800 §4.2 (fuel, not §4.4 TH), NUREG-1537 part 1 (§4.6 TH design), NUREG/CR-7041 (SCALE LWR lattice).
- **Concept tree:** `crates/kovan-literature/src/concept_proposals.toml:497-560` proposes TH nodes, including "Fuel heat-removal limits (ONB, DNB, flow instability)" at `:522` and TH stability at `:530`. They are sourced to NUREG-1537 and ORNL/TM-2018-976, not to boiling literature.
- **NOT catalogued** (searched `CATALOGUE.md` and `corpus.rs`): Collier & Thome, Todreas & Kazimi, Lahey & Moody, RELAP5 or TRACE manuals, PSBT or BFBT specs, Bartolomei, Christensen, DEBORA, Groeneveld 2006 LUT, Kurul-Podowski, Zuber-Findlay 1965, Saha-Zuber 1974.
- Per #895, Carey's *Liquid-Vapor Phase-Change Phenomena* is held by the maintainer, outside the repo.

## 3. Issue and doc context

- **#895** (this epic) lists as related: #33 "thermal hydraulics v&v" (open), and #406 and #342 (Edwards / 273.15 K, open).
- **#896** is the sibling epic: HEM/KNP stabilisation on petir.
- No other open or closed issue title (893 issues scanned via REST) mentions boiling, BWR, PSBT, BFBT, CHF or drift-flux.
- Old bead ids, not in gh:
  - `op-wqk.11` BWR sim (P4);
  - `op-2kk.1–.5` multiphase stages;
  - `op-p6p.7.12` GeN-Foam two-phase driver;
  - `op-dt3.12/.13` 1-D trace-back;
  - `op-21g.12` steam array;
  - `op-2df1` PSBT closure.
- Docs:
  - `docs/reactor-scoping/bwr.md`, a 2026-08-06 audit with a HAVE/SCAFFOLD/MISSING list and a work breakdown;
  - `docs/real-time-multifidelity-scoping.md:153,283-290,982-1002,1080-1089`;
  - `crates/outram-foam-appbuilder-lib/docs/genfoam-tutorial-verification.md:170-190` (PSBT);
  - `crates/outram-foam-appbuilder-lib/docs/genfoam-port-plan.md`.

### Doc claims the code contradicts (found during the audit; NOT fixed, because this slice is read-only)

**Status 2026-10-16:** items 1, 2 and 4–7 were verified and corrected in place,
with strike-through, in the change that committed this report. Item 7 was
confirmed by running `steam_generator_tube_boils_feedwater`, which passes.
Item 3 (bedok) was corrected from `crates/bedok/CLAUDE.md` without re-running
bedok, and the `seam.rs` rows are marked NOT RE-CHECKED.

The caller should fix these under the root CLAUDE.md "doc claim contradicted by code" rule.
1. **`docs/reactor-scoping/bwr.md`, MISSING item 9:** "No two-phase multiplier exists anywhere".
   - Contradicted by: `genfoam/.../ff_drag/multipliers.rs` (8 variants) and `dwsim-libs/src/pipe/lockhart_martinelli.rs`.
2. **`bwr.md`, SCAFFOLD list:** "`tampines/src/multiphase_1d/two_fluid.rs` is a documented scaffold computing nothing — 139 lines".
   - Contradicted by: the file is 2488 lines and has a 1979-line test file.
3. **`bwr.md`, SCAFFOLD list + item 10:** bedok "has never been run against its benchmark … zero evidence it executes".
   - Contradicted by: `crates/bedok/README.md` (D1 parity with MATLAB) and the `neacrpd1::tests` in `crates/bedok/CLAUDE.md:57-67`.
4. **`docs/real-time-multifidelity-scoping.md:984,1089`:** "`DriftFlux1d::step()` … never executed by any test".
   - Contradicted by: `tampines/src/multiphase_1d/edwards_tests.rs:550`, `marviken_tests.rs:521` and `tests.rs:498`.
   - Same doc, `:982,1080`, cites `crates/bedok/src/reference/coupling/seam.rs`, which does not exist now.
5. **`tampines/src/multiphase_1d/mod.rs:107`:** "One validation case, and only one" (Edwards).
   - Contradicted by: `marviken_tests.rs` in the same module, which reports test 23 VALIDATED.
6. **`tampines/src/critical_flow/mod.rs` header:** "Marviken is NOT validated … `#[ignore]` … ends in `todo!()`".
   - Contradicted by: `tampines-steam-tables/.../tests/marviken_tests.rs:139-142` reports the test-23 verdict, and there is no `ignore` or `todo!` in that file.
7. **`tampines-steam-tables/src/openfoam_algorithms/rhoPimpleFoam/lateral_coupling.rs:517-523`** and the `bwr.md` "load-bearing sharp edge" both say the (T,p) Region-4 `todo!()` is still reachable.
   - Likely stale: the same file at `:785` says the case "used to `todo!()`-panic before the two-phase λ fix", and `thermal_conductivity/mod.rs:226` now quality-weights in region 4.
   - Strength of evidence: inferred from comments; I did not run it.
