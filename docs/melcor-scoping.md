# Scoping: a MELCOR-class severe-accident capability in OUTRAM PARK

**Status:** scoping only — no code, no beads filed yet.
**Date:** 2026-08-04.
**Question asked:** if we wanted to re-create MELCOR inside OUTRAM PARK, what
are we missing, and what GPLv3-compatible libraries could be ported in?

---

## 0. Intended-use guardrail (read first)

Severe-accident and source-term analysis sits uncomfortably close to several
things `RESPONSIBLE_USE.md` prohibits. This scoping is explicitly for
**education, research, capability-building, and V&V against published
benchmarks only**. It is **not** for licensing, safety-critical
decision-making, emergency response, real-time plant monitoring, or
safeguards/security-sensitive analysis, and nothing built from it may be
framed as authoritative for those purposes.

Two `DATA_POLICY.md` consequences that bite specifically here:

- **Plant-specific MELCOR input decks are usually off-limits.** Utility PSA
  decks, vendor decks, and most Fukushima-unit decks are proprietary or
  operational-facility data. Only *published benchmark specifications* may be
  used (see §5).
- **Do not obtain or read MELCOR source.** See §1.

---

## 1. Provenance: what can and cannot be ported

**MELCOR's source code cannot be ported.** It is a Sandia/US-NRC code
distributed under controlled agreement (RSICC / NRC CAMP), export-controlled,
and not open source. Obtaining it for translation would breach both its
distribution terms and `RESEARCH_INTEGRITY_AND_PROVENANCE.md`.

**MELCOR's reference manuals are public**, and that is the legitimate route.
The unlimited-release SAND reports are on the NRC ADAMS public library:

- *MELCOR Computer Code Manuals Vol. 2: Reference Manual*, SAND2017-0876 O —
  [ML17040A420](https://www.nrc.gov/docs/ML1704/ML17040A420.pdf)
- *Vol. 1: Primer and Users' Guide*, SAND2017-0455 O —
  [ML17040A429](https://www.nrc.gov/docs/ML1704/ML17040A429.pdf)
- Later revisions (SAND2021-0726 O, ML21042B319/ML21042B324) likewise public.

The same holds for the codes MELCOR absorbed: **CORCON-Mod3** (MCCI),
**VANESA** (MCCI aerosol release), **CONTAIN 2.0**, **SPARC-90** (pool
scrubbing), **MAEROS** (aerosol dynamics), **CORSOR/CORSOR-Booth** (FP
release). Documentation public, code controlled.

**So the stance is: clean-room re-implementation from published reference
manuals and open literature, with every model citing its NUREG/SAND source in
the doc comment.** That is a legitimate, defensible provenance story, and it
is the same one already used for the NJOY and OFFBEAT ports — except that here
there is no upstream source file to attribute, so the citation is to the
*document*, not to a file and commit.

Codes that are **not** portable for licence reasons, so do not plan around
them: ASTEC (IRSN, restricted), MAAP (EPRI/Fauske, commercial), RELAP5 /
RELAP5-3D / TRACE (NRC/INL controlled), SCDAP/RELAP5, BISON (export-controlled
despite MOOSE's LGPL base), FRAPCON / FRAPTRAN (export-controlled), CATHARE
(CEA, commercial), GOTHIC (EPRI/Zachry, commercial).

---

## 2. Package-by-package gap analysis

MELCOR is an assembly of ~20 packages advanced by a common executive. Mapping
each onto what this workspace already has:

| MELCOR package | What it does | OUTRAM PARK today | Gap |
|---|---|---|---|
| **CVH** — control volume hydrodynamics | Lumped volumes with separate pool/atmosphere, two-phase non-equilibrium, water + non-condensable gases | `tuas_boussinesq_solver` (`single_control_vol`, `array_control_vol_and_fluid_component_collections`) — but **single-phase, incompressible, Boussinesq**; `tampines/hem` homogeneous-equilibrium | **Major.** Architecture right, physics one-phase. |
| **FL** — flow paths | Junction momentum, valves, pumps, critical flow, CCFL, bubble rise, pool/atmosphere phase separation at junctions | `tuas` `fluid_mechanics_correlations`; `tampines/critical_flow`; steam-table converging–diverging nozzle + choked flow; `dwsim` IEC-60534 valves | **Moderate.** Good pieces, no network momentum solver over a two-phase CV set; no CCFL. |
| **HS** — heat structures | 1-D conduction, convective/radiative BCs, film condensation with NCG degradation | `tuas` `one_d_solid_structure` + `heat_transfer_correlations`; `outram-foam` for 3-D | **Small.** Need NCG-degraded condensation (Uchida / Dehbi / diffusion-layer) and enclosure radiation. |
| **COR** — core degradation | Rod heatup, Zr–steam oxidation, candling, relocation, rubble/debris, molten pool, eutectics, support failure | `outram-park-fork-offbeat` — intact-rod mechanics, gap conductance, `corrosion/kinetics` oxide growth, creep/plasticity rheology, burnup/FGR | **The single biggest gap.** Everything OFFBEAT does stops at the intact rod. Nothing on candling, relocation, debris beds, molten pools, eutectic dissolution. **No open code exists to port** — clean-room only. |
| **B-H** — bottom head | Penetration failure, creep rupture (Larson–Miller), vessel breach | OFFBEAT `rheology` (creep, plasticity, yield stress) | **Moderate.** Constitutive substrate exists; the vessel-scale failure model does not. |
| **RN** — radionuclides | FP release (CORSOR-M / Booth), 17 RN classes, aerosol dynamics (MAEROS), vapour condensation, chemisorption, pool scrubbing (SPARC), iodine chemistry | `boon-lay` (Lagrangian decay + transmutation, TRISO-ATOPS release), `outram-park-fork-onix` (depletion / FP inventory), `outram-mc-libs` depletion | **Major on the transport side.** Inventory and release have real substrates. **No sectional aerosol dynamics, no pool scrubbing, no iodine chemistry at all.** |
| **DCH** — decay heat | ANS-5.1 standard curve + isotopic decay power | `teh-o-prke/src/decay_heat.rs` — 7-group precursor model, and its own doc comment flags it as buggy ("precursors are energy units, not power") | **Small–moderate.** Fix the existing model, add ANS-5.1, wire ONIX inventory for isotopic decay power. |
| **CAV** — MCCI | CORCON-Mod3 concrete ablation, crust, gas release; VANESA aerosol release from the melt | `outram-park-fork-thermochimica` (CALPHAD Gibbs minimiser — currently MSR fluoride salts); `outram-park-fork-pflotran` (porous/reactive transport); `outram-park-fork-liggghts` (DEM, debris beds) | **Major**, but the substrates are unusually good — Thermochimica extended to corium oxide/metal systems *is* the VANESA chemistry engine. |
| **BUR** — combustion | H2 / CO deflagration, ignition criteria, flame speed, DDT | nothing | **Major**, but well-covered by portable GPL/BSD code (§3). |
| **SPR** — containment sprays | Droplet heat/mass transfer, aerosol washout | nothing; `boon-lay` and `liggghts` provide Lagrangian particle infrastructure | **Moderate.** |
| **FDI / FCI** | Fuel–coolant interaction, debris quench, steam explosion energetics | `outram-foam-multiphase` (drift flux, CHF, dryout — scaffold, no V&V) | **Major.** |
| **CF / TF / EDF** | Control functions, trips, tabular functions, external data | `chem-eng-real-time-process-control-simulator` (PID, transfer functions) | **Small.** Closest thing to a solved problem here. |
| **NCG / H2O / MP** | Equation of state, material properties | `tampines-steam-tables` (IAPWS-IF97), `outram-park-fork-coolprop` (Helmholtz EOS, 137 fluids, humid air, mixtures), OFFBEAT (~70 material correlations) | **Ahead of MELCOR.** This is genuinely a strength — MELCOR's water EOS is a table fit; yours is the real IF97 + Helmholtz. |
| **PAR / ESF** | Passive autocatalytic recombiners, engineered safety features | nothing | **Small.** |
| **MELGEN / restart / plot** | Input deck language, restart files, plot output | `outram-blender` (geometry authoring), KOVAN (FSAR/literature extraction) | **Moderate.** No deck language, and — importantly — **no restart/checkpoint anywhere in the workspace.** |
| **Non-LWR** (Na, MSR, HTGR) | Sodium fires and properties, molten salt, graphite oxidation, dust | `boon-lay` TRISO, `thermochimica` + `outram-park-fork-moltres` MSR | **Partly ahead of MELCOR** on MSR and TRISO. |

---

## 3. The four structural gaps (these matter more than any single package)

The table above undersells the problem, because the hardest gaps are
architectural rather than physical.

### 3.1 There is no two-phase, multi-component control-volume EOS

This is **the single highest-value missing brick**, and almost everything else
hangs off it. MELCOR's inner loop asks: given a volume holding
`(P, U_pool, U_atm, M_H2O, M_NCG[i])`, find temperatures, void fraction,
partial pressures, and the pool/atmosphere split — then invert that to advance
pressure. Every one of CVH, FL, HS, RN condensation, BUR and SPR calls it.

You have world-class *forward* property evaluation (IF97, Helmholtz) and no
*inverse, multi-component, two-phase, pool-plus-atmosphere* solve. The
`(p,h)`-flash work in `tampines-steam-tables` is the right starting point (and
the workspace already prefers `(p,h)` flashing for exactly this reason), but it
handles water, not water + N2 + H2 + O2 + CO + CO2 + He with a separated pool.

**Nothing else should start before this.**

### 3.2 There is no integrated executive

MELCOR advances ~20 packages over a few thousand control volumes for days of
simulated time in hours of wall clock, with adaptive timestep control, package
ordering, convergence-failure retry with step halving, and restart. OUTRAM
PARK has a CFD stack, an MC stack, and a single-phase loop code —
`nee_soon`, the designated coupling layer, is ~1000 lines of scaffold.

An integrated executive is a real piece of engineering, not glue. It is also
where the "fast-running" property lives, and fast-running *is* MELCOR's reason
to exist.

### 3.3 There is no degraded-geometry representation

COR's whole point is that geometry **changes**: intact rod → oxidised → candled
→ rubble → molten pool → relocated to lower head → ex-vessel. Every mesh in
this workspace is fixed at construction (that is exactly why OFFBEAT's
topology-changer did not port — see `corrosion/model.rs`, which says so
outright). What is needed is a **component/material state machine over arrays**,
not a mesh: a cell-by-cell inventory of intact fuel / cladding / oxide /
conglomerate / particulate debris / molten pool, with mass and energy conserved
across transitions.

This is the piece with no upstream to port from, and it is the piece that makes
a severe-accident code a severe-accident code.

### 3.4 There is no restart/checkpoint or plot-file infrastructure

Severe-accident runs cover days of simulated time and routinely need to be
restarted from an arbitrary point with modified boundary conditions. That is
not an optional convenience; the analysis workflow assumes it. Nothing in the
workspace serialises solver state today.

---

## 4. Portable libraries, by licence tier

### Tier A — GPLv3 confirmed, port-worthy

| Library | Licence | What it buys you | Notes |
|---|---|---|---|
| **containmentFOAM** (FZ Jülich) | GPL-3.0 (verified) | Containment atmosphere mixing, **wall condensation with non-condensables**, H2/CO mixing and mitigation (PAR), gas radiation, aerosol transport, conjugate heat transfer | **Top pick.** OpenFOAM-based, so it drops straight into `outram-foam-appbuilder-lib`'s existing porting pipeline. Covers most of the CONTAIN/containment side and is *validated against* THAI/PANDA. |
| **AeroSolved** (Philip Morris Intl R&D) | GPL-3.0 (verified) | Multispecies aerosol: nucleation, condensation/evaporation, coagulation, deposition; sectional **and** moment methods | **Top pick.** This is your MAEROS substitute, with an open provenance MAEROS does not have. Written for CFD fields — needs adapting to lumped control volumes for the MELCOR-analog path. |
| **FLEXPART** | GPL-3.0 since v8.2 (verified) | Offsite atmospheric dispersion: transport, turbulent diffusion, dry/wet deposition, **radioactive decay**, point/area/volume sources | Replaces the MACCS side of a source-term study. Standalone crate; no coupling required beyond a release-rate time series. Fortran → Rust, well-documented (GMD papers). |
| **OpenFOAM combustion + Lagrangian solvers** (`reactingFoam`, `XiFoam`, `PDRFoam`, `sprayFoam`, `reactingParcelFoam`) | GPL-3.0 | H2/CO deflagration in congested geometry (BUR); containment spray droplets and aerosol washout (SPR) | Already in scope for `outram-foam-appbuilder-lib` — this is an extension of existing work, not a new fork. `PDRFoam` is literally a gas-explosion solver. |
| **Code_Aster** (EDF) | GPL (verify v2-vs-v3 before porting) | Creep, damage, fracture, contact — mature vessel-integrity constitutive laws for lower-head creep rupture | 1.5 MLOC; **port the constitutive laws only, never the framework.** OFFBEAT already gives you the rheology scaffolding to hang them on. |
| **PartMC** | GPL-2.0-**or-later** (verified) → upgradeable to GPLv3 | Particle-resolved stochastic aerosol dynamics | Best used as a **reference model to verify** the sectional aerosol implementation against, not as the production path. The "or later" is what makes it usable — confirm it survives in the file headers you port. |

### Tier B — permissive, GPL-compatible (absorbable into a GPLv3 work)

| Library | Licence | What it buys you |
|---|---|---|
| **Cantera** | BSD-3-Clause | Gas-phase kinetics, thermodynamics, transport. The single best dependency for BUR (H2/CO combustion) and for RN gas-phase chemistry. Mature, well-tested, widely cited. |
| **Thermochimica** (ORNL) | BSD-3-Clause | **Already forked here** (`outram-park-fork-thermochimica`, currently MSR fluorides). Extending its CALPHAD Gibbs minimiser to corium oxide/metal systems gives you the MCCI and VANESA chemistry engine. Highest leverage-per-effort item in the whole list. |
| **OpenCalphad** (Sundman et al.) | GNU licence — **verify v2 vs v3** | Mature, independent CALPHAD implementation with a broad model set; a cross-check and/or complement to Thermochimica for corium phase equilibria. |
| **PHREEQC** (USGS) | Public domain | Aqueous geochemistry — the natural engine for sump/pool **iodine chemistry**, which MELCOR handles only crudely. |
| **GEMS3K** | LGPL-3.0 | Gibbs-energy-minimisation for aqueous/solid systems; pairs naturally with the existing PFLOTRAN fork for basemat and sump chemistry. |
| **PyNE** | BSD-3-Clause | Decay data, ENDF handling, activation — an independent cross-check for DCH decay heat and RN inventory. |
| **FDS** (NIST) | Public domain | Compartment fire and (for non-LWR scope) sodium-fire modelling. Only relevant if the SFR path is wanted. |
| **MOOSE** | LGPL-2.1 (→ GPLv3-compatible) | Listed for completeness. **Recommend against** — `outram-foam-basic-lib` already covers the FEM/FV framework role, and MOOSE would duplicate it. |

### Tier C — document-only sources (implement from, do not port)

MELCOR itself, CORCON-Mod3, VANESA, CONTAIN 2.0, SPARC-90, MAEROS,
CORSOR/CORSOR-Booth, and the Zircaloy oxidation correlations (Cathcart–Pawel,
Baker–Just, Urbanic–Heidrick, Prater–Courtright). All are described in
publicly released NUREG/SAND reports and open literature; all have controlled
or unavailable source. Cite the report in the doc comment; write the model
from the equations.

---

## 5. Public validation targets

The V&V rule in `CLAUDE.md` requires methodology *and* results. These are the
public benchmark families a MELCOR-class capability is judged against, all with
published specifications:

- **In-vessel degradation:** CORA, QUENCH (KIT — extensively published),
  TMI-2 Standard Problem.
- **Integral FP behaviour:** PHÉBUS-FP (FPT-0…FPT-4), ISP-46 (FPT-1).
- **MCCI:** OECD/NEA MCCI and CCI tests, ACE, SURC, BETA.
- **Aerosol:** ABCOVE, LACE, VANAM M3, THAI aerosol tests, ARTIST.
- **Hydrogen:** THAI HD/HM, ENACCEF, PANDA (ISP-42), SETH.
- **Pool scrubbing:** POSEIDON, ACE scrubbing tests, the OECD IPRESCA project.
- **Iodine chemistry:** ISP-41, THAI iodine tests.
- **Full-plant:** OECD/NEA BSAF (Fukushima) — **use the published OECD report
  only**; the underlying plant data is restricted and out of scope per
  `DATA_POLICY.md`.

---

## 6. Suggested phasing

Deliberately ordered so that each phase unblocks the next, and so the
highest-uncertainty item (COR) is attempted only once its foundations are real.

| Phase | Content | Depends on | Notes |
|---|---|---|---|
| **0** | Architecture decision: a new lumped-parameter crate (working name `outram-park-severe-accident`), deliberately **not** mesh-based, alongside `tampines` | — | Consumes `tampines-steam-tables` + `coolprop` for EOS, `tuas` for CV/FL primitives, `teh-o-prke` for kinetics. |
| **1** | **CVH / FL / HS core** — two-phase multi-component CV EOS, flow-path network momentum solver, heat structures with NCG-degraded condensation | 0 | **The big one.** Roughly TUAS-sized on its own. Everything downstream is blocked on §3.1. |
| **2** | **DCH + RN inventory & release** — fix `decay_heat.rs`, add ANS-5.1, wire ONIX inventory, CORSOR-Booth release from public correlations | 1 | Cheapest real capability; `boon-lay` already carries Lagrangian FP transport. |
| **3** | **Aerosol + pool scrubbing** — port AeroSolved's sectional/moment methods, adapt to lumped CVs; SPARC-90 from documentation; iodine chemistry via PHREEQC-style aqueous model | 2 | Verify against PartMC. |
| **4** | **COR — core degradation** | 1 | Clean-room, no upstream. Degraded-geometry state machine + Zr oxidation + candling + debris + molten pool. Highest risk in the whole programme. |
| **5** | **Ex-vessel** — MCCI on an extended Thermochimica; lower-head creep failure on OFFBEAT rheology + Code_Aster constitutive laws | 4 | Thermochimica extension is the leverage point. |
| **6** | **Containment** — port containmentFOAM for the CFD-fidelity path; BUR via Cantera + XiFoam/PDRFoam | 1 | Largely independent of COR — could run in parallel with phase 4. |
| **7** | **Offsite** — port FLEXPART | 2 | Fully independent; could be done any time after a release-rate time series exists. |
| **cross-cutting** | Restart/checkpoint + plot-file infrastructure (§3.4) | 1 | Should land *with* phase 1, not after. |

---

## 7. Honest effort estimate

MELCOR is roughly 400 kLOC of Fortran accumulated over ~40 years by a national
laboratory. A **credible subset** — LWR, in-vessel plus containment plus a
basic source term, validated against two or three of the benchmark families in
§5 — is plausibly **150–250 kLOC of Rust**. For calibration, that is comparable
to `tuas_boussinesq_solver` (164 kLOC) plus `njoy-outram-park-fork` (52 kLOC)
combined, i.e. the two largest things in this workspace put together.

Phase 1 alone is a substantial project. Phase 4 (COR) is the one where a
schedule estimate would be dishonest — there is no code to port, the physics is
empirical and geometry-dependent, and it is where severe-accident codes
historically diverge from each other most.

**What is genuinely favourable:** the equation-of-state layer, the material
properties, the nuclear data, the depletion/inventory chain, and the fuel
mechanics are already better founded here than MELCOR's own. The gap is
concentrated in degraded-core geometry, two-phase lumped hydrodynamics, and
aerosol physics — three well-bounded targets, two of which have GPLv3 code
available to port.

## 8. What not to do

- **Do not build it on a mesh.** MELCOR's value is that a whole plant runs in
  hours. A CFD-fidelity severe-accident code already exists in spirit
  (containmentFOAM) and is a *different* tool, worth having separately.
- **Do not fold it into `outram-foam-*`.** Different architecture, different
  performance contract.
- **Do not reproduce MELCOR's input-deck language.** It exists for backward
  compatibility with decks this project cannot legally use anyway.
- **Do not touch MELCOR, ASTEC, MAAP, RELAP5, or FRAPCON source** under any
  circumstances, including "just to check an equation."
