# MELCOR parity — scoping and audit

**Status:** scoping and code audit only. **No code was written or modified.**
**Date:** 2026-08-07. **Audit method:** function bodies read directly, plus
targeted greps across all 36 member crates. Doc comments and READMEs were
*not* trusted where they disagreed with code; §6 lists every disagreement found.

**Supersedes in part:** `docs/melcor-scoping.md` (2026-08-04, one commit, never
revised). That document's *strategy* — clean-room reimplementation from public
manuals, lumped-parameter architecture, phasing — holds up well and is not
repeated here. What it got wrong was the **inventory**: it credited several
scaffolds as substrates and missed several real assets. §6 is the correction
list. Read the two together; this one is the factual layer.

> **Untrusted AI-assisted draft.** Per `RESPONSIBLE_USE.md`, this document is
> draft material until a human reviews it. The `file:line` citations are the
> part to spot-check first — they are the load-bearing claims.

---

## 0. Intended-use guardrail (read first)

Severe-accident and source-term analysis sits close to several things
`RESPONSIBLE_USE.md` prohibits outright. Anything built from this scoping is for
**education, research, capability building, and V&V against published
benchmarks only**. It is **not** for licensing, safety-critical
decision-making, emergency response, real-time plant monitoring, probabilistic
safety assessment of a real facility, or safeguards/security-sensitive
analysis, and must never be framed as authoritative for those purposes.

Two `DATA_POLICY.md` consequences bite specifically here:

- **Plant-specific input decks are off-limits.** Utility PSA decks, vendor
  decks, and most Fukushima-unit decks are proprietary or operational-facility
  data. Only *published benchmark specifications* may be used.
- **Do not obtain, read, or accept MELCOR source** — see §1, which is stronger
  than the previous document stated.

---

## 1. Provenance and licensing — the binding constraint

### 1.1 MELCOR is under NDA, not merely export-controlled

`docs/melcor-scoping.md` §1 says MELCOR is "export-controlled, and not open
source." That is true but understates it. The actual distribution route, per
Sandia's public [code-distribution
page](https://www.sandia.gov/MELCOR/code-distribution/), is:

1. Sign a **non-disclosure agreement** and email it to `safetycodes@nrc.gov`.
2. NRC approval, then Sandia approval.
3. Download from a Sandia transfer server, then request a **per-machine licence
   key** from `MLR@sandia.gov`.

CAMP and CSARP members follow the same route. RSICC's separate
[export-control terms](https://rsicc.ornl.gov/documents/export_control.pdf)
add US export-law compliance and specific country prohibitions.

**The operational consequence is sharper than "don't port it".** An NDA binds
the *person*, not just the artefact. So:

- Nobody who holds MELCOR under an NDA may contribute source, pseudocode,
  algorithm descriptions, coefficient values, or "I remember how it does X" to
  this project — even paraphrased, even in an issue comment, even in an AI
  prompt. That is a disclosure.
- The clean-room boundary is therefore a **people** boundary as well as a code
  boundary. If the project ever gains a MELCOR licensee, the parity work needs
  a documented separation, or that person must stay out of it.
- Every model must cite the **public document** it was written from, in the
  doc comment, with the ADAMS/SAND identifier. A model with no citable public
  source is not admissible.

The same holds for ASTEC (IRSN/ASNR, restricted), MAAP (EPRI, commercial),
RELAP5/RELAP5-3D/TRACE, SCDAP/RELAP5, CATHARE, GOTHIC, FRAPCON/FRAPTRAN, and
the MOOSE nuclear applications (see §1.4).

### 1.2 What *is* public, and is the legitimate source

The MELCOR manuals are **unlimited release** and on the NRC ADAMS public
library. These are the citable sources:

| Document | Identifier | ADAMS |
|---|---|---|
| MELCOR Computer Code Manuals Vol. 2: Reference Manual | SAND2017-0876 O | [ML17040A420](https://www.nrc.gov/docs/ML1704/ML17040A420.pdf) |
| Vol. 1: Primer and Users' Guide | SAND2017-0455 O | [ML17040A429](https://www.nrc.gov/docs/ML1704/ML17040A429.pdf) |
| Vol. 1 (later revision) | SAND2021-0726 O | [ML21042B319](https://www.nrc.gov/docs/ML2104/ML21042B319.pdf) |
| MELCOR 1.8.5 Manuals Vol. 2 Rev. 2 | NUREG/CR-6119 / SAND2000-2417 | [ML010190117](https://www.nrc.gov/docs/ML0101/ML010190117.pdf) |
| Core (COR) Package Reference Manual | (within the above) | [ML010190222](https://www.nrc.gov/docs/ML0101/ML010190222.pdf) |

The absorbed codes are the same story — documentation public, source
controlled: **CORCON-Mod3** (MCCI), **VANESA** (melt aerosol release),
**CONTAIN 2.0**, **SPARC-90** (pool scrubbing), **MAEROS** (aerosol dynamics),
**CORSOR / CORSOR-M / CORSOR-Booth** (FP release), and the Zircaloy oxidation
correlations (Cathcart–Pawel, Baker–Just, Urbanic–Heidrick,
Prater–Courtright).

**Ingest these through `kovan`, not by hand.** `kovan lit import <pdf>
--json-out … --markdown-out …` produces a `KovanDocument` with provenance
intact, which is exactly the record `RESEARCH_INTEGRITY_AND_PROVENANCE.md`
asks for. The manuals are openly published, so they belong under
`crates/kovan-literature/open/`.

### 1.3 There is no open integral severe-accident code

This was checked, and the answer is unambiguous: the three integral codes
(MELCOR, ASTEC, MAAP) are all closed. **There is no upstream to fork for the
integral code itself.** That is the single most important structural fact in
this document, and it means MELCOR parity is *categorically* different from
the NJOY, OFFBEAT, cfMesh, CoolProp or GeN-Foam ports, every one of which had
a real open upstream to translate.

What can be legitimately translated are *component* codes covering individual
packages:

| Source | Licence | Covers | Verified |
|---|---|---|---|
| **containmentFOAM** (FZ Jülich, `iffgit.fz-juelich.de`, **not** GitHub) | GPL-3.0-or-later by OpenFOAM inheritance | Containment atmosphere mixing, wall condensation with non-condensables, H2/CO mitigation, gas radiation, aerosol transport | Hosting and OpenFOAM-11 base confirmed; the repo's own LICENSE should be read before porting |
| **AeroSolved** (PMI R&D, `github.com/philipmorrisintl/aerosolved`) | **GPL-3.0** | Multispecies aerosol: nucleation, condensation/evaporation, coagulation, deposition; sectional *and* moment methods | Confirmed |
| **FLEXPART** (`gitlab.phaidra.org/flexpart/flexpart`) | **GPL-3.0 since v8.2 (2010)** | Offsite atmospheric dispersion, dry/wet deposition, radioactive decay | Confirmed |
| **PartMC** (`github.com/compdyn/partmc`) | **GPL-2.0-or-later** → usable as GPLv3 | Particle-resolved stochastic aerosol dynamics | Confirmed |
| **CFAST** (NIST, `github.com/firemodels/cfast`) | **Public domain** (17 USC §105) | Two-zone compartment model: lumped volumes with an upper/lower layer split, vent flows, HVAC, wall conduction | Confirmed. **New — see §1.5** |
| **FDS** (NIST, same org) | Public domain | Compartment fire; sodium fire if the SFR path is wanted | Confirmed |
| **Cantera** | BSD-3-Clause | Gas-phase kinetics/thermo/transport — the substrate for BUR and RN gas chemistry | |
| **Thermochimica** (ORNL) | BSD-3-Clause | CALPHAD Gibbs minimisation — but see §1.6, the database is the problem | |
| **OpenCalphad** (`github.com/sundmanbo/opencalphad`) | "GNU GPL", **version not stated in the README** | Independent CALPHAD implementation | Read the repo LICENSE before any port |
| **PHREEQC** (USGS) | Public domain | Aqueous geochemistry — sump/pool iodine chemistry | |
| **GEMS3K** | LGPL-3.0 | Gibbs minimisation for aqueous/solid systems | |
| **code_aster** (EDF) | **GPL-3.0-or-later** | Creep, damage, fracture — vessel-integrity constitutive laws | Already partially ported here; see §2.2 and `docs/code-aster-port-scoping.md` |
| **OpenFOAM combustion/Lagrangian solvers** | GPL-3.0 | `XiFoam`/`PDRFoam` for deflagration; `sprayFoam`/`reactingParcelFoam` for sprays | |

### 1.4 A correction worth stating plainly: MOOSE ≠ its nuclear applications

MOOSE itself is LGPL-2.1 and open. **BISON, Griffin, SAM, Pronghorn and
Sockeye are not.** They are distributed through INL's NCRC under controlled
licence agreements. Search results routinely conflate the two. None of those
five is a legitimate translation source.

### 1.5 CFAST is the closest open architectural analogue to CVH — and it was missed

MELCOR's CVH package is a network of lumped control volumes, each with a
separated **pool** and **atmosphere**, connected by flow paths, exchanging heat
with 1-D wall structures. CFAST is a network of lumped compartments, each with
a separated **lower** and **upper layer**, connected by vents, exchanging heat
with 1-D wall structures. The physics differs (no two-phase water, no
non-condensable steam condensation) but the **numerical architecture is the
same problem**: a stiff DAE over volume pressures, layer energies and species
masses, with a vent-flow momentum closure and adaptive stepping.

It is public domain, actively maintained by NIST on GitHub, and there is no
licence friction whatsoever. **For the executive, the volume/layer state
vector, the vent-flow solve and the DAE time integration, CFAST is a
legitimate and unusually clean translation source.** `docs/melcor-scoping.md`
does not mention it. This is the most useful single addition this audit makes
on the licensing side.

### 1.6 The MCCI blocker is a **database**, not code — and the databases are closed

`docs/melcor-scoping.md:80,166` calls extending `outram-park-fork-thermochimica`
to corium "the highest leverage-per-effort item in the whole list." That
assessment does not survive contact with two facts.

First, the crate is far smaller than implied — see §3.

Second, and decisively: a CALPHAD Gibbs minimiser is useless without a
thermodynamic database, and the corium databases are **not open**:

- **TAF-ID** (OECD/NEA, launched 2013) — a working version is accessible
  **only to project signatories**.
- **NUCLEA** (IRSN) — the European reference corium database, restricted.
- **MSTDB-TC** (US DOE / ORNL, molten salts) — controlled distribution, and
  scoped to fluoride/chloride salts rather than corium oxide/metal systems.

So the MCCI/VANESA path is blocked not by the minimiser (which exists in
skeleton form) but by data this project may not legally hold. The honest
options are: assess phase equilibria from **published** CALPHAD assessments
one system at a time (slow, citable, legitimate), or treat MCCI as
out of scope. **Do not plan around acquiring TAF-ID or NUCLEA.**

### 1.7 Also relevant: ANS-5.1 is a purchased standard

MELCOR's DCH package implements the ANSI/ANS-5.1 decay-heat standard. The
standard document itself is copyrighted and sold by ANS. Its coefficients are
reproduced in open literature and in other codes' public documentation, and
numerical tables are facts rather than expression — but the safe route is to
cite an **open secondary source** for the coefficients, or to compute decay
power from open ENDF decay data instead. Flag this for the maintainer to
decide; do not transcribe from a purchased PDF without checking.

### 1.8 Provenance gaps found in-tree

Three fork crates claim upstream provenance in their README but carry
**neither a `LICENSE` nor a `NOTICE` file**, contrary to the `CLAUDE.md`
attribution rule:

- `crates/outram-park-fork-thermochimica/` — claims BSD-3 ORNL Thermochimica.
- `crates/outram-park-fork-onix/` — claims MIT ONIX.
- `crates/outram-park-fork-moltres/` — claims LGPL-2.1 Moltres.

(Their `upstream_source/` directories are correctly gitignored with an
explanatory README, so the clone hygiene is fine — it is the attribution files
that are missing.) This is a pre-existing defect, not caused by this work, and
is reported rather than fixed here.

---

## 2. MELCOR package inventory → workspace mapping

The package list below is taken from the **public** MELCOR manuals (§1.2), not
from memory: BUR, CAV, CND, COR, CVH/FL, CVT, DCH, FCL, FDI, HS, MP, NCG/H2O,
PAR, RN, SPR, plus the utility packages CF, TF, EDF, ESF, TP and the
MELGEN/executive layer. MELCOR 1.8.x additionally had BH (bottom head) and SC
(sensitivity coefficients); 2.x folds BH into COR.

Tier key, used throughout:

- **A — exists and is tested.** Real function bodies with assertions that
  compare against a correlation value, a closed form, published data, or an
  upstream reference deck.
- **B — scaffold or stub.** Types, enums, module structure, or algorithm
  skeletons without the data or wiring needed to use them; or real code whose
  tests assert nothing numerical.
- **C — nothing at all.**

### 2.1 The table

| MELCOR package | What it does | Nearest workspace home | Tier | Fraction |
|---|---|---|---|---|
| **CVH / CVT** | Lumped volumes, pool + atmosphere separated, two-phase non-equilibrium, water + N non-condensables, pressure as the solved unknown | `tuas_boussinesq_solver::single_control_vol` | **C** for the actual object | ~0 % |
| **FL** | Junction momentum over a CV network, valves, pumps, critical flow, CCFL, bubble rise, phase separation at junctions | `tampines-steam-tables` choked flow; `tuas` `fluid_mechanics_correlations`; `dwsim` valves | **A** for choked flow, **C** for the network solve | ~15 % |
| **HS** | 1-D conduction, convective/radiative BCs, film condensation degraded by non-condensables | `tuas` `one_d_solid_array_*`, `heat_transfer_correlations` | **A** for conduction/convection, **C** for NCG condensation | ~40 % |
| **COR** | Rod heatup, Zr–steam oxidation + H2 + exotherm, candling, relocation, rubble, molten pool, eutectics, support failure | `outram-park-fork-offbeat` | **A** for two oxidation branches only, **C** for everything degraded | ~5 % |
| **BH** (1.8.x) / lower head | Penetration failure, creep rupture, vessel breach | `offbeat::rheology::aster` | **B** — laws exist, unwired, no vessel steels | ~15 % |
| **RN** | FP release (CORSOR/Booth), 17 RN classes, MAEROS aerosol dynamics, vapour condensation, chemisorption, pool scrubbing, iodine chemistry | `boon-lay` (TRISO Booth release), `outram-park-fork-onix` (CRAM) | **A** for Booth release + CRAM solver, **C** for all transport | ~10 % |
| **DCH** | ANS-5.1 curve + isotopic decay power | `teh-o-prke/src/decay_heat.rs` | **B** — present and self-flagged buggy | ~10 % |
| **CAV** | CORCON concrete ablation, crust, gas release; VANESA melt aerosol release | `outram-park-fork-thermochimica` (minimiser skeleton) | **C** | ~2 % |
| **BUR** | H2/CO deflagration, ignition criteria, flame speed, burn propagation between volumes, DDT | one global Arrhenius source in `reacting_two_phase_euler_foam` | **C** | ~1 % |
| **FDI** | Fuel dispersal, debris quench, FCI energetics | `outram-foam-multiphase` (CHF, wall boiling, dryout, drift flux, two-fluid — 7 kLOC), `genfoam` two-phase closures | **B** | ~15 % |
| **SPR** | Spray droplet heat/mass transfer, aerosol washout | nothing; DEM solids exist | **C** | 0 % |
| **PAR / FCL / CND / ESF** | Recombiners, fan coolers, condensers, ice condensers, filtered venting, accumulators, ECCS | nothing | **C** | 0 % |
| **CF / TF / EDF** | Control functions, trips, tabular functions, external data | `genfoam::common::{time_profile, interpolate_table}`, `genfoam …::power_off` | **A** for trips + time tables, **C** for a CF network | ~25 % |
| **MP / NCG / H2O** | Material properties and equations of state | `tampines-steam-tables` (IF97), `outram-park-fork-coolprop`, OFFBEAT (61 correlations) | **A** | **ahead of MELCOR** |
| **MELGEN / restart / plot** | Input deck, restart records, plot files | nothing | **C** | 0 % |
| **TP / executive** | Package ordering, adaptive dt, step-halving retry | `genfoam::multi_region::outer_iteration` (Picard, no dt control) | **B** | ~10 % |

The "fraction" column is a judgement, not a measurement. It is there to stop
the table reading as binary; treat it as ±half its own value.

### 2.2 Package notes, with evidence

**CVH — the gap is bigger than `docs/melcor-scoping.md` says.** That document
(line 73) rates CVH "Architecture right, physics one-phase." The architecture
is not right; it is a different object. `SingleCVNode`
(`crates/tuas_boussinesq_solver/src/lib/single_control_vol/mod.rs:66`) carries
one specific enthalpy, one `Material`, a **fixed** `mass_control_volume`, a
**fixed** `volume`, and a `pressure_control_volume` that is initialised to one
atmosphere and never solved for. Heat transfer pushes `Power` onto
`rate_enthalpy_change_vector` and `advance_timestep` marches an explicit Euler
step. There is no mass equation, no void fraction, no species vector, and no
pool/atmosphere split. It is a **thermal node**, not a hydrodynamic volume.

The material system makes this even sharper than the field list does.
`enum Material` (`.../boussinesq_thermophysical_properties/mod.rs:14`) has
exactly two variants, `Solid` and `Liquid`, which are never mixed.
`LiquidMaterial` (`:126`) is TherminolVP1 / DowthermA / HITEC / YD325 / FLiBe /
FLiNaK / Custom. **There is no steam, no water, no air, and no gas of any kind
in TUAS's material database.** A whole-crate grep for
`void_fraction|two_phase|quality|noncondens|partial_pressure|steam|boiling|condens`
returns zero physics hits — the only "steam generator" is a *name* for a
single-phase shell-side component with a user-supplied UA
(`.../gfhr_pipe_tests/components.rs:1062`), and "pool boiling" appears only
inside a literature citation for surface roughness.
MELCOR's CVH volume solves `(P, U_pool, U_atm, M_H2O, M_NCG[i])` with pressure
as the unknown. The distance between those two objects is the whole package.

**FL — real choked flow, no network.** The critical-flow capability is genuine
and is the best-founded severe-accident-relevant asset in the workspace:
`get_critical_pressure_and_mass_flux_multiphase_ph` in
`crates/tampines-steam-tables/src/steam_turbine_equations/converging_diverging_nozzles/`,
dispatching by IF97 flash region across subcooled, two-phase and superheated
stagnation states, and exercised end-to-end by
`crates/tampines-steam-tables/tests/edwards_blowdown.rs` — a 600 ms
Edwards–O'Brien pipe-blowdown transient, 24 cells, choked HEM break boundary,
gauge pressures compared against the digitised experimental curve as an RMSE.
Note the test's own honesty: assertions are **sanity-only** (finiteness,
bounds), not a tolerance gate, because HEM is a work in progress. That is
correct practice and should not be read as a passing validation.

**I ran this subset:** `cargo test -p tampines-steam-tables --lib
converging_diverging_nozzles` gives **113 passed, 0 failed, 4 ignored**. These
are genuine V&V with documented log-scale tolerances
(`tests/moody_critical_mass_flux_homogeneous_eqm.rs:16` `MOODY_LOG10_TOL =
0.06`), not smoke tests.

**But the V&V is narrower than the crate's own doc comment advertises, and I
repeated that overclaim in my first draft.** `crates/tampines/src/critical_flow/mod.rs:3-6`
says the solvers are "validated against Moody, Zaloudek, and Marviken reference
data." In fact:

- **The Marviken validation does not run.** `tests/marviken_tests.rs:116-117`
  is `#[ignore]`d and its body is `todo!()`; same at `:222`.
- **Nine Moody isobars are commented out as failing** —
  `tests/moody_critical_mass_flux_homogeneous_eqm.rs:42,48,54,66,72,78,84` and
  `:1351`.
- Two CD-nozzle cases are `#[ignore]`d
  (`tests/diverging_nozzle_perfectly_expanded_supersonic.rs:229`,
  `tests/cd_nozzle_choked_flow_overexpanded.rs:422`).

Zaloudek and the passing Moody cases are real. Marviken is not validated at
all. Treat "validated against Moody, Zaloudek and Marviken" as a claim to fix,
not a result to cite.

Model class is **HEM only** — no Henry-Fauske, no Moody slip model, no
non-equilibrium relaxation critical flow, and no discharge-coefficient orifice
model of the MELCOR FL type.

Two attribution corrections: `crates/tampines/src/hem/mod.rs` (18 lines) and
`crates/tampines/src/critical_flow/mod.rs` (28 lines) are **re-export shims**.
`docs/melcor-scoping.md:74` credits `tampines` for capability that lives in
`tampines-steam-tables`.

What is missing on the FL side is the thing that makes it a package: a
momentum equation over a *network* of junctions solved simultaneously with the
volume pressures. CCFL, bubble rise and junction phase separation are all
absent (zero grep hits for `CCFL` workspace-wide).

**A note on TUAS's `todo!()` count, because the raw number misleads.** A grep
gives 99 in `src/`, but only **63 are real `todo!()` expressions** and only
about **12 are genuine physics or data gaps**: the tungsten heating-element
material database (10 sites, `solid_database/generic_heating_element.rs:22,31,50,63,92,183,218,341,348,354`
— every getter panics, and several run a range check against *Pyrogel HPS* data
first), FeCrAl surface roughness (`fecral.rs:34`), the Ergun packed-bed
correlation and the bare-Darcy dispatch for every non-`Pipe` loss variant
(`.../fluid_component_calculation/mod.rs:162,219`), and two-phase thermal
conductivity dispatch (`thermal_conductivity.rs:175`). Roughly 45 are
`HeatTransferEntity::ControlVolume(x) => x, _ => todo!()` type-narrowing arms
in test and GUI code — ergonomic warts, not unfinished physics.

**Two are real defects worth fixing**, both being `todo!()` where an `Err`
belongs: `.../super_collection_series_and_parallel_functions.rs:1347`
— `todo!("debugging: all root finding methods used not successful")` — means
**the parallel-branch flow solver panics the process on non-convergence**,
and `.../heat_transfer_entities/preprocessing/mod.rs:119` likewise.

**HS — conduction yes, condensation no.** 1-D solid arrays with lateral
coupling exist and are used throughout TUAS's CIET validation work; I ran
`cargo test -p tuas_boussinesq_solver --lib semi_infinite` and both
semi-infinite analytical-solution checks pass. The convective correlation set
is rich but **entirely single-phase liquid** — thirteen `NusseltCorrelation`
variants (`nusselt_number_correlations/enums.rs:35-213`) covering
Dittus-Boelter, Sieder-Tate, Gnielinski and Wakao, but **no natural-convection
correlation (Churchill-Chu, McAdams), no pool or flow boiling (Chen, Rohsenow,
Thom), no minimum-film-boiling or Bromley, no quench front.** The boundary
condition set is three variants wide: `enum BCType`
(`boundary_conditions/mod.rs:15`) is `UserSpecifiedTemperature`,
`UserSpecifiedHeatFlux`, `UserSpecifiedHeatAddition`.

Radiation exists only as a **two-body gray** interaction —
`HeatTransferInteractionType::SimpleRadiation(Area)`
(`heat_transfer_interactions/heat_transfer_interaction_enums.rs:333`,
conductance body at `conductance.rs:403-450`, correctly linearising
`sigma*(T_h^4 - T_c^4)`) — where the "area" is a **user-supplied effective
area** into which emissivity and view factor must be folded by hand. It is
**not wired into the solid-array axial boundary path**:
`.../one_d_solid_array_with_lateral_coupling/axial_connection/interaction_with_bc.rs:114,252`
match `SimpleRadiation(_)` and reject it. View factors exist for exactly one
geometry, as analytical concentric cylinders
(`.../heat_transfer_correlations/view_factors/cocentric_cylinders.rs:18,89,174`,
with a view-factor-sums-to-one algebra check at `:253`). There is no radiosity
or Gebhart enclosure network, and `src/lib/lib.rs:84` says so outright:
"radiation heat transfer is NOT included in this one (yet)".

Condensation degraded by non-condensable gas — Uchida, Dehbi, or a
Colburn–Hougen diffusion-layer model — is **absent**: zero grep hits anywhere
in the six thermal-hydraulic crates. The only film-condensation mention in the
whole repository is a **hook, not a model**:
`crates/outram-foam-appbuilder-lib/src/genfoam/thermal_hydraulics/closures/heat_transfer/boiling.rs:107`
exposes `htc_film_condensation: Option<HeatTransferCoefficient>` — the caller
must supply the coefficient, and the code only blends it in above
`alpha_vapour > 0.8` (`:159-182`). This matters more than its size suggests,
because NCG-degraded condensation is what sets containment pressure.

**COR — the single biggest gap, but the previous doc mis-stated where the line
falls.** `docs/melcor-scoping.md:76` says "Everything OFFBEAT does stops at the
intact rod." That is right about geometry and wrong about oxidation.
`crates/outram-park-fork-offbeat/src/corrosion/kinetics.rs` contains genuine
high-temperature parabolic laws:

```
kinetics.rs:134   LEISTIKOW_A            = 7.82e-6   m^2/s   (T < 1800 K)
kinetics.rs:137   LEISTIKOW_Q_OVER_R     = 20214.0   K
kinetics.rs:141   PRATER_COURTRIGHT_A    = 2.98e-3   m^2/s   (T >= 1900 K)
kinetics.rs:144   PRATER_COURTRIGHT_Q_OVER_R = 28420.0 K
```

with the rate law `S = sqrt(S0^2 + A*exp(-Q/RT)*dt)` at `:662-679` and branch
selection at `:684-703`. Three caveats, all material:

1. **The enum variant is misnamed.** `OxidationKinetics::CathcartPawel`
   (`kinetics.rs:306`) does not contain Cathcart–Pawel's constants; the port
   says so at `:260-262`. Cathcart–Pawel proper, Baker–Just and
   Urbanic–Heidrick are all **absent** (zero grep hits).
2. **The 1800–1900 K interpolation window is arithmetically broken**, faithfully
   reproducing an upstream OFFBEAT defect. The port's own test at
   `kinetics.rs:1101` records the measured values: `Q/R = -692375.60 K`
   (negative — the rate *falls* with temperature) against a correct
   `+75756.40`. The checked path refuses the window (`kinetics.rs:544-556`);
   the unchecked path returns nonsense. **That window is inside the MELCOR COR
   range.**
3. **The two things COR actually needs from oxidation are absent.** The
   correlation returns ZrO2 *layer thickness* only. There is **no H2 source
   term** — `hydrogen_liberated` (`corrosion/hydrogen.rs:189`) returns wt-ppm
   hydrogen *in the cladding wall* for hydride embrittlement, and
   `CorrosionStep` (`corrosion/state.rs:150-177`) has only
   `oxide_thickness`, `oxide_growth`, `metal_loss`, `hydrogen_pickup`. And
   there is **no oxidation exotherm** (zero hits for
   `exotherm|heat of reaction|enthalpy of formation`). Zr–steam oxidation
   releases ~6.5 MJ/kg-Zr and is *the* mechanism that turns a loss of cooling
   into a severe accident. Modelling the oxide layer without the heat and the
   hydrogen is modelling the bookkeeping and not the accident.

Everything degraded is absent workspace-wide. A grep for
`candl|debris|rubble|corium|slump` returns **zero code hits**. `molten` hits
molten-*salt* coolant properties in TUAS and the `melt_foam` CFD phase change.
`eutectic` hits a `eutectic_fraction = 0` parameter of the CFD solidification
model (`crates/outram-foam-appbuilder-lib/src/solvers/melt_foam/mod.rs:297`)
and salt compositions — not UO2–Zr eutectic dissolution.

**Beware one false friend.**
`crates/outram-park-fork-offbeat/src/materials/behavioral/relocation.rs` is
**not** core relocation. It is the FRAPCON cracked-fuel-fragment radial
relocation model for normal operation; its own doc at `:64-71` says
"Relocation is NOT a volumetric strain … a radial displacement of a cracked,
essentially constant-volume body." Anyone grepping `relocat` will get 119 hits
and none of them are severe-accident relocation.

**Lower head — the constitutive laws are already ported and nobody wired
them.** This is the most surprising finding of the audit.
`crates/outram-park-fork-offbeat/src/rheology/aster/` contains a real
code_aster port: `NORTON` secondary creep (`viscoplastic.rs:191`),
`LEMAITRE`/`LEMAITRE_IRRA` (`viscoplastic.rs:180`), **`VENDOCHAB`
Lemaitre–Chaboche creep damage with a tertiary-runaway stage**
(`damage.rs:612`, params `:385`), Rousselier `ROUSS_PR`/`ROUSS_VISC`
(`damage.rs:1348`), GTN (`damage.rs:2183`), and an explicit rupture criterion
**`CRIT_RUPT`** (`damage.rs:2573`, state `:2588`, latching `broken` flag at
`:2602-2605`). It is verified against code_aster's own reference decks —
`crates/outram-park-fork-offbeat/tests/astest_ssnv101a.rs:324` asserts against
upstream `VALE_CALC` to 1e-3 relative, and `tests/astest_ssnv126a.rs` does the
same for `VENDOCHAB`. That is genuine cross-code verification, the strongest
form of evidence in the crate.

**But it is unreachable.** `MechanicsSolver::set_rheology` takes
`Rheology::ByMaterial` (`src/rheology/by_material.rs:170`) which dispatches to
`ConstitutiveLaw` (`src/rheology/law.rs:80`) — and `ConstitutiveLaw` has no
`aster` variant. The `aster` subtree is a parallel library with no bridge.
Three further blockers for vessel work: **no vessel steels** (grep for
`SA508|SA533|A508|A533|16MND5` returns nothing; the steels present are
15-15Ti and D9, fast-reactor *cladding*), **no Larson–Miller** (zero hits), and
**small-strain kinematics only** (`src/mechanics/mod.rs:96-100`) when a
creeping lower head undergoes large strain.

**RN — inventory and release are real; transport is entirely absent.**
`boon-lay`'s TRISO-ATOPS fork implements Booth diffusion release properly:
`booth_longlived` and `booth_shortlived_fast_diffuse`
(`crates/boon-lay/src/triso_atops_fork/release_models/steady_state.rs`),
`booth_transient` (`transient.rs:125`), Daynes–Barrer breakthrough
(`steady_state.rs:112`), graphite attenuation (`steady_state.rs:28`), and the
Booth equivalent-sphere radius at `release_models/mod.rs:53-58`, with series
truncations matching upstream.

This one is **verified, not merely tested**, and deserves saying plainly —
`crates/boon-lay/tests/triso_atops_fork_verification.rs` (240 lines, 9 tests)
checks against independent references rather than against itself: the Booth
early-time asymptote `6*sqrt(D't/pi) - 3D't = 0.033550` at `D't = 1e-4` to 1e-3
relative (`:145`), the long-lived limit `D't -> infinity = 1.0` (`:115-131`),
iodine kernel diffusivity at 1000 degC recomputed from the NP-MHTGR Arrhenius
correlation (`:82`), silver-in-SiC at 1200 degC (`:93`), the Cs-137 decay
constant from the decay database to 1e-9 relative (`:104`), and the Bq/Ci/lambda-N
activity unit chain to 1e-12 (`:186-192`). The header at `:52-58` further records
checks against values produced by the upstream TRISO-ATOPS Python (commit
`de374c8`) on identical inputs. That is the same class of evidence as OFFBEAT's
code_aster `astest` cases, and it is the strongest V&V anywhere in the
severe-accident-adjacent code. The file is candid that it is verification and
not validation (`:47-50`).

The caveat is scope: the verified path is **kernel / buffer / PyC / SiC only**.
Transport through matrix or structural graphite still panics (see below). `outram-park-fork-onix` has a real order-16
CRAM Bateman solver (`src/cram.rs`, 292 lines) with an analytic-Bateman V&V
test (`tests/vv_bateman.rs`, 272 lines).

Both are **solvers without data**, and the inventory path is blocked at three
separate points:

- **ONIX is data-free by design** — `src/lib.rs:59-62` states outright that
  ONIX's data libraries are not ported and "the caller supplies precomputed
  data instead." `DecayData`, `ReactionRates` and `FissionYields`
  (`src/chain.rs:40,92,145`) are caller-supplied containers. Every test uses
  fictitious nuclides such as `Nuclide::new(50,100,0)`. The CRAM engine itself
  is correct — Pusa CRAM-16 poles and residues at `src/cram.rs:55-87`, solve at
  `:184`, matrix assembly at `src/driver.rs:175`, and real decay-mode →
  daughter transforms at `src/reactions.rs:193-202` — but it has nothing to
  chew on.
- **`outram-mc-libs`'s depletion has real data but only nine nuclides, and no
  public way to add more.** `src/depletion/chain.rs:203` `simple()` is OpenMC's
  `chain_simple.xml` — I135, Xe135, Xe136, Cs135, Gd157, Gd156, U234, U235,
  U238 — with genuine half-lives and thermal fission yields, and
  `simple_from_data()` (`:366`) cross-checks two of them against
  `openmc-endf-8-depletion-lib-b`. But the general constructor
  `from_nuclides` at `src/depletion/chain.rs:208` is **private** (`fn`, not
  `pub fn`). There is no public API to assemble an arbitrary several-hundred-
  nuclide FP inventory. The crate documents why at `chain.rs:45-63`: the
  upstream registry crates do not expose the reaction targets or U-235 yields
  publicly.
- **`njoy-outram-park-fork` carries no decay sublibrary and no fission
  yields.** There is no MF=8 decoder — the ENDF reader is generic on
  (MAT, MF, MT) (`src/endf/mod.rs:50-51`, `src/endf/parse.rs:305`) but only
  MF=1, 2, 3, 6 and 12 are actually handled. The only decay constants present
  are **delayed-neutron precursor** lambdas from MF=1/MT=455
  (`src/nuclear_data/delayed.rs:54-56,88,182`) — that is neutron kinetics, not
  the radioactive-decay sublibrary.

So there is no path today from "a reactor operated for N days" to "here is the
FP inventory." Note also that the per-decay energy data needed for isotopic
decay power *does* exist —
`crates/boon-lay/src/nuclide_reaction_and_decay_data/get_decay_info/mod.rs:37`
`get_decay_energy()` — and has **zero call sites** workspace-wide. Nothing
anywhere forms the sum over `lambda_i * N_i * E_i`.

Everything downstream of release is **absent**, and this is the cleanest
"tier C" result in the audit. Grep across all crates returns **zero** for
`coagul`, `thermophor`, `diffusiophor`, `MAEROS`, `sectional method`,
`deposition velocity`, `Cunningham`, `Stokes number`, `CORSOR`, `SPARC`,
`decontamination factor`, `CsI`, and aqueous radiolysis. No sectional or moment
aerosol method, no pool scrubbing, no iodine chemistry, no chemisorption, no RN
class structure.

Four false friends will mislead anyone who greps carelessly:

- `sectional` hits only "cross-sectional area"; `agglomerat` hits GAMG
  algebraic multigrid; `nucleat` hits nucleate boiling; `settling` hits the
  drift-flux mixture model.
- `Brownian` hits a first-passage random walk **inside a TRISO kernel**
  (`crates/boon-lay/src/lagrangian_decay_simulator/lagrangian_diffusion/first_passage/sphere_fpt.rs:3,38`)
  — atomic diffusion in a solid, not aerosol transport.
- **Iodine hits are all neutronics.**
  `crates/teh-o-prke/src/feedback_mechanisms/fission_product_poisons/mod.rs:26-92`
  is a real, working I-135/Xe-135 poisoning model with fission yields and a
  semi-implicit ODE — but it is *reactivity feedback*. There is no volatile or
  organic iodine speciation, no I2/CH3I partitioning, no pH-dependent iodine
  chemistry. `scrub` appears exactly once, in a comment about a helium
  purification system.
- `crates/boon-lay/src/lagrangian_transmutation_and_fission_simulator/mod.rs`
  is a **28-line re-export shim**; its doc at `:12-14` concedes the neutron
  field is "currently a single explicit `(n,gamma)` channel", so there is no
  fission-yield-driven FP production in the Lagrangian path either.

`boon-lay` also has a live defect that blocks its own use for FP transport:
31 of its 52 `todo!()`s are in
`src/lagrangian_decay_simulator/lagrangian_diffusion/temperature_dependent_collisions/diffusion_coeffs/mod.rs`,
covering **every** `MatrixGraphite` and `StructuralGraphite` arm. Any transport
of a fission product out of a pebble and into the coolant panics. Two more are
self-flagged: `get_decay_info/mod.rs:140,230` — `todo!("code is buggy!")`.

**DCH — present, self-flagged buggy, confirmed verbatim.**
`crates/teh-o-prke/src/decay_heat.rs:12-14`:

```
/// i think this is slightly buggy, need to change code
///
/// the precursors are energy units, not power...
```

288 lines, a 7-group precursor model, **zero tests** (the crate has 21
elsewhere). ANS-5.1 is not implemented anywhere, and no isotopic
(nuclide-by-nuclide) decay power exists.

**But the doc comment misdiagnoses its own bug, and so does
`docs/melcor-scoping.md:79` in repeating it.** Storing the precursor as an
energy `E_i` [J] is a perfectly legitimate formulation:
`add_decay_heat_precursorN` accumulates `E_i += f_i*P_fis*dt`
(`decay_heat.rs:38`) and the physical release rate is `lambda_i*E_i`, which at
saturation gives `lambda_i*E_i = f_i*P_fis` — the correct limit. "Energy units,
not power" is not the defect.

The **actual defect is a sign error**. At `decay_heat.rs:98` the function
returns `(e_decay_t_plus_delta_t - e_decay_t)/timestep`, and with
`E_new = E_old/(1 + lambda*dt)` (`:94`) that evaluates to `-lambda*E_new` —
**negative power for a positive inventory**. Every caller in the workspace
papers over it with `.abs()`:
`crates/teh-o-prke/examples/fhr_sim_v1/app/prke_backend/mod.rs:334-336`,
`crates/tampines-steam-tables/examples/fhr_sim_v1/app/prke_backend/mod.rs:334-336`,
`crates/tampines/examples/fhr_sim_v2/app/prke_backend/mod.rs:720-722`,
`crates/outram-park-digital-twin-engine/examples/fhr_sim_v2/app/prke_backend/mod.rs:720-722`.
With `.abs()` the magnitude is correct, so the shipped GUI examples get the
right number **by accident**; any new caller that omits it gets negative decay
heat.

Three further defects the comment does not mention:

- `calc_decay_heat_power_N` **mutates** the precursor as a side effect of
  reading it (`decay_heat.rs:96`). Calling it twice in one timestep
  double-decays. A silent API foot-gun.
- Group fractions `f_i` are not stored on the struct at all — the caller must
  hard-code them. The `Default` impl (`decay_heat.rs:266-286`) creates groups
  4–7 at 1 yr, 30 yr, 1000 yr and 1 s with no documented fractions, and no
  in-repo caller ever feeds them. Dead state.
- The half-life set is **ad hoc** and corresponds to no published fit — neither
  ANS-5.1's 23-exponential-group-per-fissionable-nuclide form nor the classic
  three-group Untermyer–Weills fit. The documented example fractions sum to
  10 % of fission power at saturation against a physical ~6.5–7 % at shutdown.

So this row is cheaper to *replace* than to repair, and "fix `decay_heat.rs`"
in §5.4 phase 2 should be read as "write ANS-5.1 and delete this."

**CAV — see §1.6.** `crates/outram-park-fork-thermochimica` is 1,479 lines in
two files with 6 tests (all passing). The minimiser is **real and its algorithm
is identifiable**: element-potential / Lagrange-multiplier Gibbs minimisation
with a `(M+P)x(M+P)` saddle-point assembly (`gem.rs:849-896`), multiplicative
log-space update (`gem.rs:930-957`), **backtracking line search** on an
L1-penalised merit function (`gem.rs:949-967`), rank-deficiency and
charge-constraint handling (`gem.rs:647-705`), and a correct multicomponent
Redlich-Kister partial-molar derivative (`gem.rs:301-341`). That is honest work
and should not be dismissed.

What it is **not** is a chemistry engine:

- **No thermodynamic database of any kind.** Zero SGTE-style
  `G(T) = a + bT + cT lnT + …` Gibbs functions, zero coefficients. No ChemSage
  `.dat` parser — explicitly out of scope at `gem.rs:166-169`. Every `g0` is
  passed in per solve as a raw `f64` (`gem.rs:736`), and every test uses
  **admittedly fabricated numbers**: `gem.rs:1196-1197` calls its UF4 value
  "an arbitrary reference value", and the FLiBe test at `gem.rs:1361-1362` says
  its inputs are "illustrative reference values, not measured FactSage data —
  so this is a mass/charge-balance *identity* check … not a validated
  equilibrium."
- **Binary Redlich-Kister only** (`gem.rs:260-274`). SUBG/SUBI/SUBL/SUBM
  modified-quasichemical — the models molten fluoride salts actually require —
  plus QKTO, magnetic terms and ternary interpolation are all not ported
  (`gem.rs:170-173`).
- **A hard architectural ceiling**, documented at `gem.rs:183-190`: the number
  of simultaneously active phases must not exceed the number of independent
  element rows, or the solve returns `SingularSystem` rather than repairing the
  assemblage. There is no phase-assemblage management, no miscibility-gap
  detection, no leveling (`gem.rs:174-181`). The largest case ever exercised is
  **2 phases, 2 species each, 3 elements** (`gem.rs:1235-1242`); the FLiBe case
  is 1 phase, 2 species.

So it cannot compute fission-product speciation, UF3/UF4 redox, or solubility
in a fluoride melt today, let alone corium oxide/metal equilibria. Extending it
is not leverage on an existing asset — it is building the data layer, the
parser, the sublattice models and the assemblage manager from nothing, on top
of a database that cannot legally be obtained (§1.6).

Concrete ablation, crust formation and melt-gas release are all absent
(`ablation`, `MCCI` — zero hits).

**BUR — effectively zero.** The only combustion anywhere is a single global
irreversible Arrhenius reaction inside the two-fluid solver:
`crates/outram-foam-appbuilder-lib/src/solvers/reacting_two_phase_euler_foam/mod.rs:459`
(`ReactionMechanism`), `:472` (`Arrhenius`), rate and heat release at
`:755-775`, test at `:1494`. The module's own scope note at `:96-101` is
honest about the limits. Workspace-wide grep for
`deflagration|flame|ignition|burning velocity` returns **zero code hits**. No
`XiFoam`/`PDRFoam` analogue, no Shapiro–Moffette flammability limits, no burn
propagation between volumes, no DDT, no CO combustion, no chemical mechanism
reader. Note the false friend:
`genfoam/thermal_hydraulics/thermophysical/hydrogen/` is hydrogen *coolant
properties*, not combustion.

**SPR — absent, and the distinction matters.** There is no Lagrangian droplet
model: no parcel type, no drag law on a droplet, no evaporation or
condensation submodel, no cloud. What exists is (a) **DEM solid particles**
in `outram-park-fork-liggghts` — Hooke and Hertz–Mindlin contacts, walls,
bonds, rolling friction, cell-list neighbour search
(`src/simulation.rs:338-381`), ~9.7 kLOC and 72 tests — and (b) a **Monte Carlo
radionuclide random walk** in `boon-lay`
(`lagrangian_diffusion/single_particle_simulator/mod.rs:14`) that tracks a
diffusing *atom* with no mass, drag or thermal state. Neither is a spray. The
CFD-DEM seam is explicitly empty:
`crates/outram-park-fork-liggghts/src/coupling.rs:25-37` — "RESERVED
ARCHITECTURE ONLY — NO PHYSICS IS IMPLEMENTED HERE … there is no drag law, no
interpolation, no volume averaging."

**ESF / PAR / FCL / CND — all absent.** Targeted grep for
`recombiner|passive autocatalytic|ECCS|safety injection|containment spray|ice
condenser|fan cooler|filtered vent|core catcher` returns **zero hits**.
(`accumulator` returns ~30 hits, every one a numerical running sum.) The only
adjacent items are a SCRAM/power-off criterion
(`genfoam/thermal_hydraulics/structure/power_off.rs:60`, real and tested), an
IEC-60534 control valve (`crates/tampines/src/components/valve.rs:13`, sizing
only), and pump coast-down (`genfoam …/structure/pump.rs:55`).

**CF / TF — the previous doc pointed at the wrong crate.**
`docs/melcor-scoping.md:84` calls control functions "the closest thing to a
solved problem here" and credits
`chem-eng-real-time-process-control-simulator`. That crate is 8.8 kLOC with
**4 `#[test]` attributes, of which 2 are duplicates across a near-duplicate
`alpha_nightly`/`beta_testing` tree, and all 4 assert nothing numerical**
(one asserts `a*a == a*a`). Its `src/examples/*` are compiled into a binary,
not into `cargo test`, and their assertions are placeholders such as
`assert_abs_diff_eq!(1.0, 1.01, epsilon = 0.1)`. Worse, several `todo!()`s sit
on live paths inside `Result`-returning constructors —
`generic_first_order.rs:193,196` panic on `tau_p <= 0` instead of returning
`Err`; `generic_second_order.rs:229` panics on `zeta < 0`; `:43` makes
`Default` panic. The PID and transfer-function numerics are used in production
(the CIET simulator, ~20 TUAS regression tests) and have **zero numerical test
coverage**.

The real CF/TF substrate is elsewhere and is genuinely good:
`genfoam::common::time_profile` (`TimeProfile`, 7 tests),
`genfoam::common::interpolate_table` (Linear/Step, bounds policy, 9 tests),
`genfoam …/structure/power_off.rs:60` (`PowerOffCriterion` — `Timer` and
`FieldValue` threshold with a latch, 4 tests), and time-varying boundary
conditions (`…/boundary_conditions/time_field_table.rs`). What is missing is a
**general, user-composable control-function graph** — CFs feeding CFs feeding
boundary conditions — which MELCOR's input language provides and which no crate
here has.

**MP / NCG / H2O — genuinely ahead of MELCOR.** IF97 (`tampines-steam-tables`,
93 kLOC, 937 tests), Helmholtz EOS for 137 fluids
(`outram-park-fork-coolprop`, 43 kLOC), and 61 named material correlations
across 7 property families in OFFBEAT (conductivity 14, heat capacity 8,
density 5, emissivity 3, Young's modulus 12, Poisson 6, thermal expansion 13;
UO2, MOX, MA-MOX, Zircaloy/M5/ZIRLO, Mo, Hastelloy-N, 15-15Ti, D9, SiC, PyC,
TRISO buffer). MELCOR's water EOS is a table fit; this is the real thing. This
is the one row where parity is already exceeded.

**Executive / restart / plot — the cross-cutting hole.** Confirmed absent, all
three:

- **No adaptive timestep with step-halving retry at PDE level.** All seven
  appbuilder solvers run identical fixed-dt boilerplate
  (`solvers/pimple_foam/mod.rs:499`, `rho_central_foam/mod.rs:433`,
  `rho_pimple_foam/mod.rs:322`, `melt_foam/mod.rs:575`, `hrm_foam/mod.rs:504`,
  `sonic_foam/mod.rs:266`, `reacting_two_phase_euler_foam/mod.rs:1107`).
  `ControlDict` declares `adjust_time_step`, `max_co`, `max_delta_t`
  (`src/io/control_dict/mod.rs:43-45`) and **none of those fields is read
  anywhere.** Real step rejection exists only in the ODE integrators
  (`tampines-steam-tables/src/openfoam_algorithms/openfoam_source/ode/mod.rs:149`),
  used by no PDE solver.
- **No restart.** Workspace-wide, `#[derive(Serialize/Deserialize)]` appears at
  19 sites, every one either kovan literature types or an `eframe` GUI
  persistence struct (slider positions). Zero serde on solver state. Zero
  `checkpoint`/`save_state`/`write_restart` in any solver. The nearest miss is
  `outram-park-fork-pflotran/src/hdf5_io/mod.rs:144,218` — a round-trip-tested
  grid+fields HDF5 snapshot that no solver calls, and which stores no solver
  internals.
- **No plot files.** The appbuilder output writers are stubs:
  `src/io/output/mod.rs:49,61,74` are all `todo!()`. The only real field writer
  (`outram-foam-basic-lib/src/io/field.rs:114,130`) is used for a
  final-state dump. No PDE solver writes a time series.

The closest thing to an executive is genuinely good but is not one:
`genfoam::multi_region::outer_iteration.rs:617` `MultiPhysicsSolver::solve(dt)`
— a real Picard loop over a closed enum of regions with field exchange,
max-residual convergence and a cold-start guard, 24 tests. But it takes `dt`
from the caller, has no time loop, and on non-convergence returns an error
rather than halving the step.

---

## 3. Honest gap analysis, in three tiers

### Tier A — exists and is tested (usable as a foundation today)

| Capability | Where | Note |
|---|---|---|
| IAPWS-IF97 water/steam properties, all regions, backward equations | `tampines-steam-tables` (93 kLOC, 937 tests) | Ahead of MELCOR |
| Helmholtz EOS, 137 fluids, incompressibles, humid air | `outram-park-fork-coolprop` (43 kLOC, 333 tests) | |
| HEM critical/choked flow across all flash regions | `tampines-steam-tables …/converging_diverging_nozzles/choked_flow` | Edwards–O'Brien blowdown exercised; **sanity-only assertions** |
| 1-D solid conduction with lateral coupling, convective correlations | `tuas_boussinesq_solver` | Extensively used in CIET validation |
| Zircaloy HT oxidation — Leistikow and Prater–Courtright branches | `offbeat/src/corrosion/kinetics.rs:134-144` | Two branches of four; 1800–1900 K window broken |
| 61 material property correlations, 7 families, 11 materials | `offbeat/src/materials/properties/` | Verification-level tests, never validated against irradiation data |
| Small-strain mechanics with eigenstrain, plasticity, creep | `offbeat/src/mechanics/`, `src/rheology/` | Closed-form verification tests |
| code_aster constitutive laws incl. creep damage and rupture criterion | `offbeat/src/rheology/aster/` | Verified vs upstream `astest` decks; **not wired to the solver** |
| Booth / breakthrough / attenuation FP release from TRISO | `boon-lay/src/triso_atops_fork/release_models/`, verified by `boon-lay/tests/triso_atops_fork_verification.rs` | **Verified, not merely tested** — see below |
| Order-16 CRAM Bateman depletion | `outram-park-fork-onix/src/cram.rs` | Data-free; no nuclide library exists to feed it |
| Deterministic neutronics: diffusion, SP3, S_N, eigenvalue + transient | `outram-foam-appbuilder-lib/src/genfoam/neutronics/` | 262 genfoam tests; analytic slab V&V |
| Porous-medium single-phase TH with fluid-structure drag | `genfoam/thermal_hydraulics/solver/one_phase.rs` | Two-phase driver not ported |
| Two-phase closures: CHF, boiling, phase change, interfacial, turbulence | `genfoam/thermal_hydraulics/closures/` (8.2 kLOC) | Closures without their solver |
| Enthalpy-porosity melting/solidification | `outram-foam-basic-lib/src/fv_options/solidification_melting.rs`, `solvers/melt_foam/` | 965-LOC V&V suite |
| HRM (Downar-Zapolski) flashing **with an NCG mass-fraction equation** | `solvers/hrm_foam/mod.rs:64,120` | CFD-mesh, not lumped |
| Euler-Euler two-fluid with interfacial mass transfer and latent heat | `solvers/reacting_two_phase_euler_foam/mod.rs` | |
| DEM: Hooke/Hertz-Mindlin, walls, bonds, rolling friction | `outram-park-fork-liggghts` | Thermal DEM exists as functions but is **not in the time loop** (`src/simulation.rs:338-381`) |
| Trips, time tables, time-varying BCs | `genfoam/common/`, `genfoam …/power_off.rs` | |
| Aqueous speciation, Debye-Hückel/Davies/Pitzer activities, kinetic mineral dissolution, sorption, pH-dependent surface complexation, **radioactive decay chains with daughter ingrowth**, RICHARDS flow, reactive transport | `outram-park-fork-pflotran` (26.4 kLOC, **284 tests run and passing**) | The most substantial crate in this audit. Candidate substrate for sump/basemat chemistry — but see the three blockers below |
| **Multicomponent VLE / VLLE flash** — Rachford-Rice, nested-loops TP flash, three-phase flash, inside-out, SLE/SVLLE, cubic EOS with `ln_phi` and enthalpy departure, PR1978/PRSV2/Lee-Kesler, UNIFAC | `outram-park-fork-dwsim-libs/src/thermo/` (26.5 kLOC, 329 tests, one `todo!()`) | **The most consequential find of this audit** — see §4.1. Steam + N2 + CO2 flash is computable today; nothing calls it, and only five components are built in |
| IEC 60534 valve sizing (liquid, gas, two-phase) and pump modes with NPSH | `dwsim-libs/src/valve/iec_60534.rs:100,191,266`, `src/pump/modes.rs:73,131` | Real bodies incl. F_F, choked clamping, expansion factor Y, closed-form and bisection P2 back-solves. The `tampines` wrappers around both are stubs |
| UQ: distributions, samplers, Sobol/correlation sensitivity | `raffles` (5.7 kLOC, 34 tests) | README still says "nothing is implemented" — see §6 |

**The pflotran caveat, because it matters for a future sump-chemistry path.**
The building blocks are unusually well matched to sump water chemistry and
basemat leaching, but three things block use today, and all three are the same
shape — *modules that exist standalone and are not wired into the solve*:

1. `src/geochemistry/network.rs:23-28` — the speciation core uses **ideal
   activities (gamma = 1)**, has **no mineral phases**, and has **no
   electroneutrality constraint**. The `activity` and `pitzer` modules are not
   called by `speciate()`.
2. `src/reactive_transport/mod.rs:44-49` — "all species are aqueous and
   mobile". Sorbed and mineral phases are never immobilised, so the `sorption`
   and `surface_complexation` modules are likewise uncoupled from transport.
3. No concrete/cement chemistry, no high-temperature aqueous data, no
   boric-acid/lithium chemistry.

The Pitzer model is also narrower than its presence suggests: binary 1:1 and
2:1 salts at 25 degC only, with no theta/psi mixing terms
(`src/pitzer/mod.rs:1-33`).

### Tier B — scaffold, stub, or real-but-unwired (do NOT count as coverage)

| Item | Evidence |
|---|---|
| `nee_soon`, the designated coupling layer | `NeeSoon` is a **zero-field struct** (`src/lib.rs:95`); all four workflow stages return `Err(NotYetImplemented)` (`mgxs.rs:97`, `mesh_mc.rs:109`, `sp3_multiphysics.rs:135`, `validation.rs:96`); 5 tests |
| `outram-park-fork-thermochimica` | 1,479 LOC / 2 files / 6 tests; a minimiser with **no database** (`src/gem.rs:734`) |
| `tampines` two-fluid and steam generator | `src/multiphase_1d/two_fluid.rs:128` and `src/components/steam_generator.rs:38` — `step()` returns `Err(NotYetImplemented)`. 4 `#[test]` in the whole `src/` tree |
| `chem-eng` PID / transfer functions | Real code, **zero numerical test coverage**; `todo!()` panics on live `Result` paths |
| `bedok` benchmark gates | 15 gates, all `#[ignore]`d — 7 in `tests/benchmark/main.rs`, 8 in `tests/parity/main.rs`; `tests/support/mod.rs:572-576` returns `None` so a run **passes as a skip**. **Runtime-confirmed:** `cargo test --release` reports `tests/benchmark/main.rs … 0 passed; 0 failed; 7 ignored` — not one benchmark gate executes. The 7 that pass in `parity/main.rs` are comparator self-checks (`parity/main.rs:14-19`). No test runs `solve_coupled_steady` end to end |
| OFFBEAT fission gas release | `src/fgr/mod.rs:826-895` — only `Disabled`, `TransientVenting` (vents a caller-supplied inventory), and `Sciantix` which returns `NotImplemented` at `:1013`. **Nothing generates gas** |
| OFFBEAT failure criteria, phase transition | `materials/behavioral/failure.rs:12` and `phase_transition.rs:12` — 12-line placeholders |
| OFFBEAT thermal sub-solver | Absent; the crate's own README says the correlations exist and the solve does not |
| OFFBEAT gap contact | Real pure functions (`gap/contact.rs:129,239`) but `crate::gap` is not imported anywhere in `mechanics/` — no closed-loop gap-closure solve |
| `outram-foam-multiphase` | **Not a scaffold in the code sense** — 6,953 LOC, **67/67 tests run and pass**, zero `todo!()`. Real physics: Biasi (`chf.rs:302`), Westinghouse W-3 (`:389`), Bowring (`:564`), RPI three-way wall-boiling partition (`wall_boiling.rs:441`), Zuber-Findlay drift (`drift_flux.rs:383`), Rhie-Chow drift-flux PIMPLE (`pimple.rs:234`), Euler-Euler shared-pressure PISO (`two_fluid_pimple.rs:279`). Roughly 15 tests are hand-computed formula checks and 16 are physical-limit invariants. **Three specific holes:** the Groeneveld LUT machinery is real but **no real Groeneveld data ships** — `chf.rs:1023` is a synthetic sample explicitly labelled "do not use its numbers for any physical purpose"; the post-CHF wall regimes return typed `NotImplemented` (`wall_boiling.rs:517,538,559,581`); and there is **no interphase mass transfer in the field equations at all** — `drift_flux.rs:501` and `two_fluid.rs:664` are pure advection, so the CHF and boiling closures are standalone algebra never coupled back into the void-fraction equation. Tier B on *evidence*, not effort |
| `tampines` component wrappers | Nearly all stubs returning `NotYetImplemented` — valve `components/valve.rs:51`, pump `pump.rs:36`, condenser `:38`, heat exchanger `:45`, cooling tower `:49` and `cooling_tower/mod.rs:31`, steam generator `:39`, turbine `:40` — **while the algebra underneath them works** (see the dwsim row in Tier A). This is a wiring gap, not a physics gap |
| `tampines::multiphase_1d::DriftFlux1d` | Substantial and sophisticated (`drift_flux.rs:569` `step()` — explicit momentum predictor, implicit pressure equation with secant re-linearisation across the saturation kink, choked-outlet BC) but **entirely untested**. `cargo test -p tampines --lib` runs 4 tests (Thomas solver x2, IF97 compressibility, pipe geometry) and **not one constructs or steps a `DriftFlux1d`**. There is no `tests/` directory in the crate |
| liggghts CFD-DEM coupling | `src/coupling.rs:25-37` — self-declared "NO PHYSICS IS IMPLEMENTED HERE" |
| appbuilder I/O | `src/io/output/mod.rs:49,61,74` and the three dict readers — all `todo!()` |
| genfoam two-phase solver driver | `…/solver/mod.rs:35-45` — only `one_phase` implemented; `twoPhase` tracked in beads |
| `genfoam …/boundary_conditions/nusselt_baffle.rs:157,176` | every method `unimplemented!()` |

### Tier C — nothing at all

Confirmed by targeted grep across all 36 crates, zero code hits each:

Two-phase pool/atmosphere control volume · flow-path network momentum solve ·
CCFL · bubble rise · junction phase separation · NCG-degraded condensation
(Uchida/Dehbi/Colburn–Hougen) · enclosure radiation network · candling ·
core relocation · debris/rubble beds · molten pools · corium · eutectic
dissolution · UO2–Zr interaction · support-structure failure · H2 generation
source term · Zr oxidation exotherm · Cathcart–Pawel proper · Baker–Just ·
Urbanic–Heidrick · Larson–Miller · vessel steels · lower-head breach model ·
aerosol dynamics of any kind (coagulation, nucleation, condensation onto
particles, sectional or moment methods, gravitational settling,
thermophoresis, diffusiophoresis) · pool scrubbing / decontamination factors ·
iodine chemistry · aqueous radiolysis · chemisorption · RN class structure ·
CORSOR family · fission-yield and decay-data libraries · ANS-5.1 · isotopic
decay power · concrete ablation · MCCI · melt-gas release · deflagration ·
flame speed · ignition criteria · flammability limits · DDT · CO combustion ·
chemical mechanism integration · Lagrangian droplets/sprays · aerosol washout ·
PARs/recombiners · fan coolers · ice condensers · filtered venting · ECCS
accumulators · safety injection · core catchers · restart/checkpoint of a
running transient · adaptive dt with step-halving · plot-file time series from
any solver · general control-function network · input-deck language ·
atmospheric dispersion.

---

## 4. The hard parts

Five things are genuinely hard *in this architecture*, as distinct from merely
large. Ranked by how much they block everything else.

### 4.1 The two-phase, multi-component control volume (blocks almost everything)

`docs/melcor-scoping.md` §3.1 identified this correctly and it remains the
right answer. The inner loop MELCOR needs is: given a volume holding
`(P, U_pool, U_atm, M_H2O, M_NCG[i])`, return temperatures, void fraction,
partial pressures and the pool/atmosphere split, then invert to advance
pressure. CVH, FL, HS condensation, RN condensation, BUR and SPR all call it.

Why it is hard here specifically:

- The workspace has world-class **forward** property evaluation. IF97 `(p,h)`
  flash handles water, including the two-phase Region 4 with genuine
  quality-weighted mixing and the hard Region-3/4 boundary above 623.15 K
  (`crates/tampines-steam-tables/src/interfaces/functional_programming/ph_flash_eqm/mod.rs:56,631,751`).

- **Correction to the previous framing, and it is a real one: the
  multicomponent flash algebra is NOT missing.**
  `crates/outram-park-fork-dwsim-libs/src/thermo/` is 26,518 lines with a
  complete flash suite — Rachford-Rice (`flash.rs:206,273`), a TP VLE nested-
  loops flash (`flash.rs:424`), a **three-phase VLLE flash**
  (`flash_vlle.rs:478`), inside-out variants, SLE/SVLLE, cubic EOS with
  `ln_phi` and enthalpy departure (`cubic_eos.rs:385,471`), PR1978, PRSV2,
  Lee-Kesler, and UNIFAC activity models. 329 tests, one `todo!()` in the whole
  crate. Water, nitrogen and carbon dioxide are among the five hard-coded
  components (`src/thermo/component.rs:167,218,235`).

  **A steam + N2 + CO2 isothermal-isobaric flash is computable in this
  workspace today.** What does not exist is (a) a control volume that could
  hold the result, and (b) any call site — nothing in `tuas`, `tampines` or
  `tampines-steam-tables` references `dwsim-libs`. So §4.1 is better stated as
  *a missing state object and a missing wiring job*, not a missing solver. That
  is a materially cheaper problem than the previous scoping implied, and it
  should change where phase 1 starts: **evaluate `dwsim-libs`'s flash as the
  CVH thermodynamic kernel before writing a new one.**

  Two caveats before banking it: only five components are built in, everything
  else must be caller-supplied; and it is a *chemical-process* flash, so the
  pool/atmosphere non-equilibrium split MELCOR needs (two separate energy
  equations, not one equilibrium state) still has to be built around it.

- `outram-park-fork-coolprop` is **not** the answer here despite appearances.
  Its mixture support is real GERG-2008-style `(T, rho, x)` evaluation
  (`src/mixtures/mod.rs:191`) but `mod.rs:9-12` states plainly that **no
  flash/VLE is implemented** and that it "has not been validated against
  GERG-2008 reference values yet" — 3 tests. Pure-fluid flashes are explicitly
  single-phase-only and return `NonConvergent` inside the dome
  (`src/flash.rs:14-22`).
- The **`uom` typing rule cuts against it.** A CVH state vector is a
  heterogeneous bag (one pressure, two energies, `1 + N` masses) that gets
  handed to a Newton solve. `uom` types are `Copy` and dimension-checked, which
  is exactly what you want at the interface and exactly what you must
  temporarily leave behind inside a residual vector. The clean pattern —
  strongly typed struct in, dimensionless scaled residual vector inside, typed
  struct out — needs to be designed once, deliberately, and then followed.
  Getting this wrong will either destroy the type safety or make the solver
  unwritable.
- **No trait objects and no lifetimes** is not a real obstacle here; a closed
  enum over EOS backends is the right shape anyway.

The partial coverage is real but does not substitute. `hrm_foam` proves the
workspace can carry a non-condensable fraction alongside a flashing two-phase
mixture — but on a CFD mesh, with a single lumped NCG mass fraction rather
than a species vector, and with no pool/atmosphere separation. `coolprop`'s
mixture machinery is the most likely reusable piece.

### 4.2 Degraded-geometry representation (no upstream, and the mesh fights it)

COR's whole point is that the geometry *changes*: intact rod → oxidised →
candled → rubble → molten pool → relocated to the lower head → ex-vessel. This
was verified to be structurally impossible today:
`crates/outram-foam-basic-lib/src/mesh/fv_mesh.rs` has **exactly one
`&mut self` method in the entire file** — the private `ensure_face_areas` at
`:562`. There is no `move_points`, no topology change, no public mutator. The
mesh is immutable after `FvMeshBuilder::build`. OFFBEAT's own modules say the
same: `src/corrosion/mod.rs:107-133` deliberately does not port the layer
addition/removal topology changer; `src/gap/mod.rs:103` and
`src/gap/free_volume.rs:66` call it "irreducibly a mesh-topology algorithm.
Deferred."

The right answer is the one the previous doc gave: **do not fight the mesh —
do not use one.** COR wants a component/material state machine over arrays: a
cell-by-cell inventory of intact fuel / cladding / oxide / conglomerate /
particulate debris / molten pool, with mass and energy conserved across every
transition. That is a fixed array with a varying *interpretation*, which suits
Rust enums extremely well and suits the workspace's no-`dyn` rule perfectly.

It remains the hardest item because **there is no upstream to port** (§1.3),
the physics is empirical and geometry-dependent, and it is precisely where
real severe-accident codes disagree with each other most. Any schedule
estimate for this is a guess and should be labelled as one.

### 4.3 Aerosol dynamics (large, but tractable — and open code exists)

MAEROS is a sectional aerosol model: a size-discretised population balance
with coagulation kernels (Brownian, gravitational, turbulent), condensational
growth, and removal by settling, thermophoresis and diffusiophoresis. It is
mathematically well-posed and the workspace has **nothing** — this is the
single largest tier-C block with a clean open substitute.

Both **AeroSolved** (GPL-3.0) and **PartMC** (GPL-2.0-or-later) are
legitimately portable. The adaptation cost is real: both are written for
resolved fields, and the MELCOR-analogue path needs them over **lumped control
volumes**. PartMC is best used as a particle-resolved *reference model* to
verify a sectional implementation against, not as the production path.

Pool scrubbing (SPARC-90) and iodine chemistry have no open code and must come
from documentation and literature; PHREEQC (public domain) is the natural
engine for the aqueous iodine side.

### 4.4 MCCI (blocked on data, not physics — see §1.6)

The chemistry engine skeleton exists; the corium CALPHAD database does not and
cannot legitimately be obtained. Concrete ablation, crust mechanics and
melt-gas release are all additionally absent. **This should be treated as out
of scope for the foreseeable future**, or approached one published CALPHAD
assessment at a time as a long-horizon research track. Recommending otherwise
would be dishonest about what is reachable.

### 4.5 The executive, restart, and output (unglamorous, and the actual product)

MELCOR's reason to exist is that a whole plant runs for days of simulated time
in hours of wall clock, restartably. That property lives entirely in the
executive: package ordering, adaptive stepping, convergence-failure retry with
step halving, restart records, and plot files. All four are absent (§2.2).

This is not glue. It is the difference between a library of correlations and a
code. It is also the piece most likely to be under-scoped, because it produces
no physics results of its own and therefore never feels urgent.

**CFAST (§1.5) is the right thing to read for this**, and is public domain.

---

## 5. Effort estimate and phasing

### 5.1 Calibration

Measured against this workspace, not guessed. Current Rust LOC (all `.rs`,
including tests and doc comments, which run roughly half of total here):

| Crate | kLOC | `#[test]` |
|---|--:|--:|
| `tuas_boussinesq_solver` | 175 | 371 |
| `tampines-steam-tables` | 93 | 937 |
| `outram-park-fork-offbeat` | 61 | 550 |
| `njoy-outram-park-fork` | 57 | 538 |
| `outram-park-fork-coolprop` | 43 | 333 |
| `outram-foam-appbuilder-lib` | 43 | 303 |
| `outram-mc-libs` | 36 | 255 |
| `outram-foam-basic-lib` | 32 | 355 |
| `bedok` | 25 | 285 |
| `boon-lay` | 21 | 184 |

MELCOR itself is roughly 400 kLOC of Fortran accumulated over ~40 years.

**Generation rate is not the constraint.** `docs/historian/historian_220726_to_230726.md`
records 48.4 kLOC of Rust added in a two-day window. At that rate the entire
estimate below is weeks of *writing*. The constraint is that none of it is
trusted until a human reviews it: of the 36 crates, 26 carry the
`## Bookkeeping status` block and **all 26 still show at least one axis "Not
yet manually checked."** Zero crates in this workspace are maintainer-signed-off
on both V&V and human-interface. Any schedule that counts kLOC and not
human-review-hours is measuring the wrong thing.

### 5.2 MELCOR-lite — the minimal useful subset

**Recommended definition:** the smallest thing that produces a *physically
meaningful containment pressure, temperature and hydrogen history* for a
station-blackout-class LWR transient. Deliberately **no source term** — no
aerosols, no fission-product transport, no MCCI.

That is a real, citable capability (it is what most containment-response
studies actually need), it is validatable against public benchmarks
(QUENCH for oxidation, THAI HD/HM for hydrogen distribution, ISP-42/PANDA for
containment mixing), and it stops short of every item in §4 except 4.1 and
4.5.

| Component | Rust kLOC | Notes |
|---|--:|---|
| CVH/CVT: two-phase pool+atmosphere volume with N non-condensables | 12–22 | §4.1. Nothing starts before this. **Revised down** — `dwsim-libs` already has the multicomponent flash; the missing pieces are the state object, the pool/atmosphere non-equilibrium split, and the wiring |
| FL: junction momentum network, valves, pumps; reuse existing choked flow | 10–15 | Choked flow already exists |
| HS: reuse TUAS conduction; add NCG-degraded condensation and an enclosure radiation network | 8–12 | |
| Executive: package ordering, adaptive dt with step-halving, restart records, plot-file time series | 10–15 | §4.5. **Must land with phase 1, not after** |
| DCH: fix `decay_heat.rs`, add ANS-5.1 (§1.7) | 2–3 | Cheapest real capability in the programme |
| COR-lite: rod heatup, Zr oxidation **with H2 source and exotherm**, no relocation | 10–15 | Fix the 1800–1900 K window; add Cathcart–Pawel and Baker–Just |
| BUR-lite: flammability limits, complete-combustion deflagration, burn propagation between volumes | 5–8 | Cantera-backed |
| ESF-lite: fan cooler, spray as a lumped heat/mass sink, PAR | 4–6 | |
| **MELCOR-lite total** | **70–105** | |

For scale: that is between `njoy-outram-park-fork` and `tampines-steam-tables`
in size — one large crate, not a workspace.

### 5.3 Full parity — the additional cost

| Component | Additional kLOC | Risk |
|---|--:|---|
| COR full: degraded-geometry state machine, candling, relocation, debris, molten pool, eutectics, support failure | 30–50 | **Highest — no upstream, §4.2** |
| RN full: 17 classes, CORSOR/Booth release, MAEROS-class sectional aerosols, vapour condensation, chemisorption, pool scrubbing, iodine chemistry | 35–50 | Moderate — AeroSolved is portable |
| Fission-yield and decay-data libraries (to feed ONIX and DCH) | 5–10 | Low — open ENDF data |
| Lower head: wire the `aster` laws to `MechanicsSolver`, add vessel steels, large-strain kinematics, breach model | 10–15 | Moderate — laws already ported |
| CAV/MCCI | 15–25 | **Blocked on data, §1.6 — recommend deferring** |
| FDI/FCI, debris quench | 8–12 | |
| SPR full: Lagrangian droplets with evaporation and aerosol washout | 6–10 | |
| Offsite dispersion (FLEXPART port) | 15–25 | Low — GPL-3.0, fully independent |
| MELGEN-analogue input layer | 8–15 | |
| **Full-parity total (incl. MELCOR-lite)** | **~200–320** | |

This revises `docs/melcor-scoping.md` §7's "150–250 kLOC" upward, mainly
because that estimate did not carry the executive, restart, plot files, the
data libraries, or the input layer as separate line items.

### 5.4 Dependency-ordered phasing

Each phase unblocks the next. Phases 6 and 7 are independent and can run in
parallel with anything.

| Phase | Content | Blocks on | Note |
|---|---|---|---|
| **0** | Decide the crate. A new lumped-parameter crate — deliberately **not** mesh-based — consuming `tampines-steam-tables` + `coolprop` for EOS and `teh-o-prke` for kinetics. Ingest the public manuals via `kovan`. File the epic and child issues. | — | Also: settle the `uom`-at-the-boundary / scaled-residual-inside pattern (§4.1) *before* writing the solver |
| **1** | **CVH/CVT/FL/HS core** + **the executive, restart and plot files** | 0 | The big one. Read CFAST (§1.5). **First task: evaluate `dwsim-libs`'s flash as the CVH thermodynamic kernel** rather than writing one (§4.1). Restart is not a phase-2 item |
| **2** | **DCH** — fix `decay_heat.rs`, add ANS-5.1, add open decay/yield data, wire ONIX | 1 | Cheapest real capability |
| **3** | **COR-lite** — heatup, oxidation with H2 and exotherm, no relocation. Fix `kinetics.rs:684-703` | 1 | Delivers MELCOR-lite together with phase 4 |
| **4** | **BUR-lite + ESF-lite** | 1 | Cantera-backed. **MELCOR-lite is complete here** |
| **5** | **RN** — release correlations, classes, then port AeroSolved's sectional method onto lumped CVs; verify against PartMC | 2 | The largest tractable block |
| **6** | **COR full** — the degraded-geometry state machine | 3 | Highest risk in the programme. No schedule estimate is honest here |
| **7** | **Lower head** — wire `rheology::aster` to `MechanicsSolver`, add vessel steels | 3 | Cheaper than it looks; the laws are already ported and verified |
| **8** | **Containment CFD path** — port containmentFOAM into `outram-foam-appbuilder-lib` | 1 | Independent of COR; a *different tool*, worth having separately |
| **9** | **Offsite** — FLEXPART port | 5 | Fully independent once a release-rate time series exists |
| **deferred** | **CAV/MCCI** | — | Blocked on closed databases (§1.6) |

### 5.5 What not to do

Carried forward from `docs/melcor-scoping.md` §8, still correct, plus two
additions:

- **Do not build it on a mesh.** The value is that a whole plant runs in hours.
- **Do not fold it into `outram-foam-*`.** Different architecture, different
  performance contract.
- **Do not reproduce MELCOR's input-deck language.** It exists for backward
  compatibility with decks this project cannot legally use anyway.
- **Do not touch MELCOR, ASTEC, MAAP, RELAP5, FRAPCON, BISON, Griffin, SAM or
  Pronghorn source** under any circumstances, including "just to check an
  equation" — and see §1.1 on the *people* boundary.
- **(new) Do not plan around acquiring TAF-ID or NUCLEA** (§1.6).
- **(new) Do not start any physics package before the executive exists.**
  Every previous coupling attempt in this workspace — `nee_soon`, `tampines`'s
  two-fluid path — stalled at exactly the point where an executive was needed
  and absent.

---

## 6. Staleness findings

Every item below is a documented claim that the code contradicts. Reported,
not fixed — several are in files outside this document's scope, and beads are
the maintainer's to close.

### 6.1 In `docs/melcor-scoping.md`

| Line | Claim | Reality |
|---|---|---|
| 73 | CVH: "Architecture right, physics one-phase" | `SingleCVNode` is a thermal node with fixed mass and unsolved pressure — a different object, not a one-phase version of the right one (§2.2) |
| 74 | Credits `tampines/critical_flow` and `tampines/hem` | 28- and 18-line re-export shims; the capability is in `tampines-steam-tables` |
| 76 | COR: "Everything OFFBEAT does stops at the intact rod" | Understates it in one direction (Leistikow + Prater–Courtright HT oxidation exist) and overstates in another (no H2 source, no exotherm, 1800–1900 K window broken) |
| 80, 166 | Thermochimica extension is "the highest leverage-per-effort item in the whole list" / "the VANESA chemistry engine" | 1,479 LOC, 2 files, 6 tests, a minimiser with **no database** — and the corium databases are closed (§1.6). This is the single largest overclaim in the document |
| 84 | CF/TF: "Small. Closest thing to a solved problem here" | `chem-eng` has 4 trivial tests, zero numerical coverage, and `todo!()` panics on live `Result` paths. The real substrate is in `genfoam` |
| 158 | Lists code_aster as a Tier-A library **to port** | Already partially ported into `offbeat/src/rheology/aster/` and verified against upstream `astest` decks — it just is not wired to the solver |
| 78 | RN: "Inventory and release have real substrates" | Release yes (Booth). Inventory no — ONIX is data-free and njoy carries no decay data or fission yields |
| — | Does not mention **CFAST** | Public-domain, the closest open architectural analogue to CVH (§1.5) |
| — | Does not mention `hrm_foam`, `melt_foam`, `reacting_two_phase_euler_foam`, the genfoam two-phase closures, the Edwards blowdown case, or `raffles` | All real, all severe-accident-adjacent |
| 32–35 | MELCOR is "export-controlled, and not open source" | True but understated: distribution is under a **signed NDA**, which binds people as well as artefacts (§1.1) |

### 6.2 Elsewhere in the repository

| Location | Claim | Reality |
|---|---|---|
| `docs/architecture.md:99,104,114` | GenFOAM is *(planned)*, "on hold until the MC + nuclear-data path is further along" | `crates/outram-foam-appbuilder-lib/src/genfoam/` is **32,256 LOC with 262 tests**: diffusion, SP3 and S_N eigenvalue *and* transient solvers, cross-section feedback, porous TH, thermo-mechanics, multi-region Picard coupling, and an analytic slab V&V case. Badly stale |
| `CLAUDE.md:859` | Same "on hold" claim in the planned-crates table | Same |
| `CLAUDE.md:830` and `crates/raffles/README.md:11-14` | raffles is "Scaffold only, nothing implemented … No distribution, no sampler, no sensitivity estimator" | 5,705 LOC: `distributions.rs` 2,795, `samplers.rs` 1,460, `sensitivity.rs` 1,241, 34 tests, **zero** `todo!()`. Only `surrogate.rs` (54 lines) is genuinely a stub |
| `CLAUDE.md:810` | bedok "carries a committed NEACRP BWR transient case; the benchmark gates are `#[ignore]`d" | Gates are indeed ignored — runtime-confirmed at `0 passed; 0 failed; 7 ignored` for the benchmark suite — but **there are no NEACRP gates at all**; all 15 ignored gates are IAEA-3D. There are also **two** NEACRP cases in `src/reference/cases/` (PWR A1/A2 and BWR D1), not one |
| `crates/outram-foam-appbuilder-lib/src/genfoam/multi_region/outer_iteration.rs:60-72` | Mesh-based models "cannot yet dispatch" | `mesh_region.rs` (804 LOC) implements `MeshNeutronics` with Doppler feedback |
| `crates/outram-park-fork-thermochimica/`, `outram-park-fork-onix/`, `outram-park-fork-moltres/` | READMEs claim BSD-3 / MIT / LGPL-2.1 upstreams | None of the three carries a `LICENSE` or `NOTICE` file (§1.8) |
| `crates/outram-foam-appbuilder-lib/src/io/control_dict/mod.rs:43-45` | `ControlDict` exposes `adjust_time_step`, `max_co`, `max_delta_t` | None of the three is read anywhere. Adaptive stepping is declared, not implemented |
| `crates/outram-park-fork-offbeat/src/corrosion/kinetics.rs:306` | Enum variant named `CathcartPawel` | Contains Leistikow and Prater–Courtright constants; the port says so at `:260-262`. A reader trusting the name would mis-cite the model |
| `Cargo.toml:25-27` | pflotran is a "SCAFFOLD: units/error/flow-mode shape only; RICHARDS solve, Newton-Krylov solver, grid, and I/O are planned" | Badly stale — all of those are implemented, and **284 tests pass**. The crate's own `src/lib.rs:10-32` is accurate ("VERIFICATION-ONLY — no validation, no human V&V yet"); it is the workspace manifest comment that is wrong |
| `crates/outram-park-fork-thermochimica/src/gem.rs:143` | Cites "(V&V `collapse_*` test)" as evidence for the vanishing-phase mechanism | **No such test exists.** The six tests are `pure_component_trivial_limit`, `ideal_binary_nernst_partition`, `redlich_kister_activity_coefficient_identity`, `regular_solution_partition_in_solver`, `molten_fluoride_flibe_ideal_mixing`, `linear_solver_and_error_paths`. Phase collapse is never tested |
| `crates/teh-o-prke/src/decay_heat.rs:12-14` | "the precursors are energy units, not power" | Misdiagnoses its own bug. The energy formulation is legitimate; the real defect is a **sign error** at `:98`. See §2.2 — and `docs/melcor-scoping.md:79` repeats the misdiagnosis |
| `crates/tampines/src/critical_flow/mod.rs:3-6` | Choked-flow solvers are "validated against Moody, Zaloudek, and **Marviken** reference data" | **The Marviken tests do not run** — `tampines-steam-tables/tests/marviken_tests.rs:116-117` is `#[ignore]`d with a `todo!()` body. Nine Moody isobars are commented out as failing (`moody_critical_mass_flux_homogeneous_eqm.rs:42-84`). Zaloudek and the passing Moody cases are genuine; the Marviken claim is not |
| `docs/melcor-scoping.md:97-111` (§3.1) | "There is no two-phase, multi-component control-volume EOS … **Nothing else should start before this**" | Half right. The *control volume* is indeed absent, but the multicomponent VLE/VLLE **flash exists and is tested** in `outram-park-fork-dwsim-libs/src/thermo/` (329 tests). The gap is a state object and a call site, not a solver |

---

## 7. Public validation targets

Unchanged from `docs/melcor-scoping.md` §5 and still correct. Recorded here so
this document stands alone. Per the `CLAUDE.md` V&V rule, any test written
against these must document **both** the methodology and the measured results
with uncertainty.

- **In-vessel degradation:** CORA, QUENCH (KIT), TMI-2 Standard Problem.
- **Integral FP behaviour:** PHÉBUS-FP (FPT-0…FPT-4), ISP-46 (FPT-1).
- **MCCI:** OECD/NEA MCCI and CCI, ACE, SURC, BETA.
- **Aerosol:** ABCOVE, LACE, VANAM M3, THAI aerosol, ARTIST.
- **Hydrogen:** THAI HD/HM, ENACCEF, PANDA (ISP-42), SETH.
- **Pool scrubbing:** POSEIDON, ACE scrubbing, OECD IPRESCA.
- **Iodine chemistry:** ISP-41, THAI iodine.
- **Blowdown / critical flow:** Edwards–O'Brien (already exercised), Marviken,
  Moody, Zaloudek.
- **Full-plant:** OECD/NEA BSAF (Fukushima) — **the published OECD report
  only**; the underlying plant data is restricted per `DATA_POLICY.md`.

---

## 8. What could not be determined

Stated explicitly rather than glossed:

- **Whether the test counts quoted are passing counts — resolved for four
  crates, still static for the rest.** Most counts in this document are
  `#[test]` *attribute* counts obtained by static reading. Four crates were
  additionally run under `cargo test --release` and are therefore passing
  counts:

  | Crate | Measured |
  |---|---|
  | `outram-park-fork-offbeat` | 545 lib + 71 doc + 8 integration = **553 passed, 0 failed, 0 ignored** |
  | `boon-lay` | 175 lib + 9 `triso_atops_fork_verification` = **184 passed** |
  | `outram-park-fork-liggghts` | **72 passed, 0 ignored** |
  | `bedok` | 251 lib + 12 fixture + 7 parity self-checks passed; **15 gates ignored** (7 benchmark, 8 parity) |
  | `outram-park-fork-pflotran` | **284 passed, 0 failed** (268 lib + 16 integration, 1 ignored) |
  | `outram-park-fork-onix` | **25 passed** (17 lib + 7 integration + 1 doctest) |
  | `outram-park-fork-thermochimica` | **6 passed** |
  | `outram-foam-multiphase` | **67 passed, 0 failed, 0 ignored** |
  | `tampines-steam-tables` (nozzle subset only) | **113 passed, 0 failed, 4 ignored** — the other 821 lib tests were **not** run |
  | `tuas_boussinesq_solver` (conduction subset only) | 3 passed, including two semi-infinite analytical checks — the other ~368 were **not** run |

  OFFBEAT's `README.md:65-68` count is **exactly accurate**, which is worth
  noting given how much else in this repository's documentation is not.
  Everything else quoted here (TUAS, `tampines-steam-tables`, njoy, coolprop,
  appbuilder, `outram-foam-multiphase`, `raffles`, …) remains a static
  attribute count and was **not** run.

  A green suite does not move any ABSENT verdict in §3, and the reason is worth
  stating: passing tests measure the code that exists. Nothing exercises
  Baker–Just, Urbanic–Heidrick, H2 generation, the oxidation exotherm,
  candling, debris beds, molten pools, Larson–Miller, vessel steels or a
  mutable mesh, because none of those exist to test.
- **Numerical correctness of any solver** beyond what its own V&V documentation
  asserts. Assertions were verified to exist and their references identified;
  the references were not re-derived.
- **The exact licence version of OpenCalphad** — the README does not state it
  and the repository `LICENSE` was not fetched.
- **containmentFOAM's own LICENSE file** — GPL-3.0-or-later is inferred from
  its OpenFOAM base and is very likely right, but was not read directly from
  the FZ Jülich GitLab.
- **Whether MSTDB-TC has a genuinely open subset** usable under
  `DATA_POLICY.md`. The TAF-ID and NUCLEA restrictions were confirmed; MSTDB-TC
  was not resolved.
- **The `bn` issue tracker was not updated.** Per `CLAUDE.md`, kopi-beans
  cannot read this repository's store (`unsupported meta format_version 1`,
  [kopitiam#16](https://github.com/theodoreOnzGit/kopitiam/issues/16)), so no
  epic or child issues were filed for the phasing in §5.4. That breakdown still
  needs filing once the blocker clears.
