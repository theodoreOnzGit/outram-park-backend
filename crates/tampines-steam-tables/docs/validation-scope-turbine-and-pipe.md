# Validation scope: digital-twin steam turbine and steam/water pipe

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

**Status of this document:** scope only. It names candidate validation
cases, their sources, and their access terms. It reports **no results** —
nothing here has been run. Per the workspace V&V rule, a case is only
"validated" once its own test/doc carries both a methodology *and* measured
numbers with uncertainty and a date.

Written 2026-08-04 from a literature search of open sources.

---

## 1. What is being validated

Two components of the offline digital twin, at **lumped / 1-D system scale**
— not blade-passage or CFD scale.

### Steam turbine

Today the turbine exists in two disconnected halves:

| Piece | File | State |
|---|---|---|
| `ThreePhaseElectricGeneratorTurbine` | `src/steam_turbine_equations/generator.rs` | Working lumped electromechanical rotor: torque balance → $\omega$, EMF, current, power. **No steam path.** |
| `tampines::components::Turbine` | `crates/tampines/src/components/turbine.rs` | Inert shell: `inlet: HemSteamCv` + `adiabatic_efficiency`; `expand_to()` returns `NotYetImplemented`. |

The maintainer's architecture direction (bead `op-dt3.18`) is to rebuild the
turbine on `TampinesSteamArray` (the HEM array) as the baseline solver, i.e.
**a control volume with sink terms** rather than a steady-state efficiency
map.

That direction is sound, with three constraints this document assumes:

1. **The sink is shaft work, not heat, and it must be coupled.** The energy
   sink is $\dot{W} = \dot{m}\, \eta_{ad}\, (h_1 - h_{2s})$, and it must
   drive the rotor inertia ODE $J \, d\omega/dt = T_{turb} - T_{gen}$ —
   otherwise load rejection and overspeed are unreachable, and those are the
   transients a twin exists for. Casing heat loss is a genuine second sink
   but is small (order 0.5–1 %).
2. **No real fluid inventory in the blading.** Steam residence time in a
   turbine is milliseconds. A CV carrying true blade-path volume introduces
   a stiff acoustic mode that shrinks the timestep and buys no physics. The
   standard system-code arrangement is a quasi-steady **junction** (flow law
   + work extraction) between two volumes, with storage held in the steam
   chest and the adjacent pipes. If the turbine CV is given a volume, it
   should represent the chest/casing, not the blading.
3. **Efficiency alone is not a turbine.** $\eta_{ad}$ pins the work but not
   the mass flow. Off-design flow needs a second constitutive law —
   **Stodola's cone (ellipse) law** — and, because the LP end runs wet,
   $\eta_{ad}$ needs a **Baumann wetness correction** (roughly 1 %
   efficiency lost per 1 % mean wetness) rather than being a constant.

HEM is the right closure for an integral work balance. It places the
expansion on the equilibrium line rather than the supersaturated
(Wilson-line) path; that distinction only bites at nozzle-resolving
fidelity, which is where §3.4 applies.

### Steam/water pipe

`TampinesSteamArray`
(`src/openfoam_algorithms/rhoPimpleFoam/mod.rs`) — the 1-D compressible
rhoPimpleFoam-derived array with a real IAPWS-IF97 $(p,h)$ two-phase flash.
Also `tampines::components::Pipe` (diameter, length, roughness,
inclination), which wraps it.

---

## 2. Current V&V status (verified by inspection, 2026-08-04)

Honest baseline — several cases already exist, in varying states of
completion:

| Case | Location | State |
|---|---|---|
| Edwards–O'Brien pipe blowdown | `tests/edwards_blowdown.rs` | **Run**, results recorded 2026-07-16. 2 tests, none ignored. Sanity-only assertions (finiteness, bounds); reported as RMSE, not a hard gate |
| Moody critical mass flux | `.../tests/moody_critical_mass_flux_homogeneous_eqm.rs` | **Active** — 14 tests, 1 ignored, and that one is `diagnose_deep_subcooled_failures`, a diagnostic. `isobar_pref_0_25` is **not** ignored (bead `op-21g.2` looks stale) |
| Zaloudek critical mass flux | `.../tests/zaloudek_critical_mass_flux_homogeneous_eqm/` | **Active** — 89 tests across 5 files, 1 ignored, and that one is `diagnose_bubble_point_artifact`, a diagnostic |
| Marviken tests 23/24 | `.../tests/marviken_tests.rs` | **Not done** — `#[ignore="skip first, Marviken is more complex"]`, assertion commented out, body ends in `todo!()` at line 222. Digitised NUREG/CR-2671 data is present |
| Bubble-point saturation | `.../tests/bubble_point_saturation_validation.rs` | 2 tests, none ignored |
| CD-nozzle subsonic / choked / perfectly-expanded | `.../tests/cd_nozzle_*.rs`, `diverging_nozzle_*.rs` | 9 tests, **2 ignored and both unfinished** — `wet_steam_test` ("temporary skip test") and `..._wet_steam` ("test not ready"). Both are on the **wet-steam** path the turbine needs |
| **Steam turbine — anything** | — | **Nothing.** No V&V case exists |
| Steady-state two-phase pipe Δp | — | **Nothing** |
| Void fraction | — | **Nothing** |

So the gaps this scope addresses are: the **entire turbine**, and the pipe's
**steady-state** closures (friction multiplier, void fraction, post-dryout).
The pipe's *transient* behaviour is the one thing already exercised, via
Edwards–O'Brien.

**Counting caveat.** `#[ignore]` here often carries a reason string
(`#[ignore="..."]`), so the obvious `grep '#\[ignore\]'` **undercounts real
skips and overcounts comment mentions**. Count with
`grep -rnE '^\s*#\[ignore' src/`. An earlier pass of this document reported
Moody as "8 ignored" on the bad pattern; the true figure is 1, and it is a
diagnostic.

---

## 3. Candidate cases — steam turbine

### 3.1 Spencer, Cotton & Cannon (1963, rev. 1974)

- **Gates:** `adiabatic_efficiency` itself.
- **What it is:** the standard method for predicting the performance of
  steam turbine-generators 16,500 kW and larger — efficiency correlations
  for the governing stage and the HP, IP and LP sections as functions of
  volumetric flow, pressure ratio, and wetness.
- **Citation:** Spencer, R. C., Cotton, K. C., Cannon, C. N., *A Method for
  Predicting the Performance of Steam Turbine-Generators … 16,500 kW and
  Larger*, ASME Paper 62-WA-209; *J. Eng. Power* **85** (1963) 249–298;
  revised July 1974.
- **Access:** ASME, paywalled. The published *Discussion* of the paper is
  indexed openly, and EBSILON's "Component 122: Steam Turbine (SCC)"
  documentation describes which figures (2–18) carry which correlation —
  useful for scoping, **not** a substitute for the primary source.
- **Provenance caution:** the primary document was **not** obtained during
  this search; its content is described here from secondary sources. Obtain
  it before implementing.

### 3.2 AP1000 rated turbine-cycle heat balance

- **Gates:** the whole secondary loop end to end — turbine + pipes +
  moisture separator reheater + condenser + feedwater, as one consistent
  state-point set.
- **What it is:** Figure 10.1-1, "rated heat balance for the turbine cycle",
  giving throttle pressure/temperature/flow, extraction points, MSR
  conditions, condenser pressure, and gross generator output.
- **Citation:** *AP1000 Design Control Document*, Rev. 19, Chapter 10
  ("Steam and Power Conversion System"), Westinghouse. NRC ADAMS accession
  **ML11171A341**.
- **Access:** public regulatory document via NRC ADAMS.
- **Provenance caution:** the NRC server returned HTTP 403 to automated
  fetching on 2026-08-04. The document and figure were confirmed from search
  metadata only — **verify the contents on manual download** before relying
  on any specific number.

### 3.3 Stodola cone-law off-design comparisons

- **Gates:** the mass-flow-vs-pressure law (constraint 3 in §1).
- **What it is:** published comparisons of Stodola's ellipse law against the
  Schegliáiev model for sliding-pressure, throttle-valve and nozzle-valve
  control modes.
- **Sources:** "Application of steam turbines simulation models in power
  generation systems", *Revista de Engenharia Térmica* (open access);
  Springer, "Steam Turbine Modeling" (book chapter).
- **Access:** the journal article is open access.

### 3.4 Wet-steam condensing nozzle benchmarks

- **Gates:** the existing `converging_diverging_nozzles` module, and
  specifically **how much the HEM assumption costs** versus non-equilibrium
  condensation.
- **What it is:** the four standard benchmark supersonic nozzles — **Moore
  B**, **Barschdorff**, IWSEP, and SUT de Laval — with measured axial static
  pressure distributions and fog-droplet sizes.
- **Citations:** Moore, M. J., Walters, P. T., Crane, R. I., Davidson, B. J.,
  "Predicting the Fog-Drop Size in Wet-Steam Turbines", IMechE Conf. on Heat
  and Fluid Flow in Steam and Gas Turbine Plant, Coventry, April 1973,
  C37/73, 101–109; Barschdorff, D. (1971); Moses & Stein; Gyarmathy.
- **Access:** the primary papers are old conference proceedings. Digitised
  geometry and pressure distributions for all four nozzles appear in
  "Numerical analysis of non-equilibrium steam condensing flows in various
  Laval nozzles and cascades", *Engineering Applications of Computational
  Fluid Mechanics* — **open access**, and the practical route in.
- **Note:** this is a *diagnostic* case, not a pass/fail gate. HEM is
  expected to miss the condensation shock; the deliverable is the quantified
  gap.

---

## 4. Candidate cases — steam/water pipe

### 4.0 Master catalogue: CSNI SET validation matrix

Not a case — the **index**. NEA/CSNI/R(93)14, *Separate Effects Test Matrix
for Thermal-Hydraulic Code Validation*, Vol. 1 (phenomena, facility and test
selection) and Vol. 2 (facility and experiment characteristics). 67
phenomena cross-referenced against roughly 2094 openly available tests.
**Free PDF** from oecd-nea.org. Use it to select further pipe cases
systematically rather than ad hoc.

### 4.1 Two-phase frictional pressure drop — Thom / Martinelli–Nelson

- **Gates:** `Pipe`'s friction closure over the two-phase range.
- **Citations:** Thom, J. R. S. (1964) — two-phase multipliers $r_2$, $r_3$,
  $r_4$ for boiling water/steam, tabulated by pressure; Martinelli, R. C.,
  Nelson, D. B. (1948) — separated-flow multiplier from 0.1 MPa to the
  critical pressure.
- **Why these two:** both are steam-water-specific and recommended at the
  high pressures a secondary loop runs at, unlike the air-water-rooted
  Lockhart–Martinelli (1949).
- **Access:** tabulated in standard two-phase texts and reproduced in many
  open papers.

### 4.2 Subcooled boiling void fraction — Bartolomei & Chanturiya

- **Gates:** the void/quality relation in the heated pipe.
- **What it is:** cross-sectionally averaged void fraction at axial stations
  in a vertical heated tube, upward flow; pressure to 15 MPa, mass flux to
  2000 kg m⁻² s⁻¹, heat flux to 2.2 MW m⁻². Wall and subcooled liquid
  temperature available for one condition.
- **Citation:** Bartolomej, G. G., Chanturiya, V. M., "Experimental study of
  true void fraction when boiling subcooled water in vertical tubes",
  *Thermal Engineering* **14** (1967) 123–128.
- **Access:** the original Russian-journal paper is hard to obtain. The data
  is digitised in numerous open-access validation papers — e.g.
  "Implementation and validation of two-phase boiling flow models in
  OpenFOAM", arXiv:1709.01783. **Digitisation provenance must be recorded**
  (which figure, which paper, which reader tool), the same way
  `marviken_tests.rs` records its graphreader points.

### 4.3 Post-dryout wall temperature — Bennett et al.

- **Gates:** post-CHF / dispersed-flow heat transfer (relevant to beads
  `op-dt3.15`, `op-dt3.16`).
- **What it is:** 224 steady cases, vertical Nimonic-80 tube of 12.62 mm
  bore, heated lengths 3.66 m and 5.56 m, uniform heat flux, 27 axial
  thermocouples; mass flux 393–5235 kg m⁻² s⁻¹, inlet subcooling
  42–181 kJ kg⁻¹.
- **Citation:** Bennett, A. W., et al. (1967), UKAEA report **AERE-R5373**,
  Harwell High Pressure Two-Phase Heat Transfer Loop.
- **Access:** UKAEA report; widely reproduced in the post-dryout literature.
- **Caveat:** measures wall temperature but **not** non-equilibrium vapour
  temperature or actual quality, so it constrains an equilibrium
  formulation only.

### 4.4 Already covered

Edwards–O'Brien (transient blowdown, §2) and the Marviken / Moody / Zaloudek
critical-flow family. Marviken is unfinished and is the nearest open
pipe-side task.

---

## 5. Property layer — the prerequisite

**IAPWS R7-97(2012)**, *Revised Release on the IAPWS Industrial Formulation
1997 for the Thermodynamic Properties of Water and Steam*, carries
verification tables for each region. **Free PDF** from iapws.org.

A turbine efficiency validated on a wrong $h(p,s)$ proves nothing, so this
gate logically precedes everything in §3 and §4. Check whether the crate
already covers it before filing new work — `bubble_point_saturation_validation.rs`
covers the saturation line but not the region-by-region verification tables.

---

## 6. Open-source libraries that already model this

Asked directly: **yes for the model structure, no for the validation data.**
No open library ships a ready-made digitised validation-data set for these
cases; what exists is reference *implementations* and reference *papers*.

| Library | What it has | Licence | Position for us |
|---|---|---|---|
| **ClaRa** (`xrg-simulation/ClaRa-official`, mirror `ClaRaLibrary/ClaRa`) | `SteamTurbineVLE_L1.mo` — "A steam turbine model based on STODOLA's law": mass + energy conservation, isentropic efficiency, and an optional mechanical port with moment of inertia `J`. Structurally **exactly** the "HemSteamCv with sink terms" design in §1, including the rotor coupling | **BSD-3-Clause**, verified at three levels — see §6.1 | GPLv3-compatible one-way: may be incorporated into this GPLv3 workspace **with copyright notice, licence conditions and disclaimer retained**. The best structural reference |
| **ThermoPower** (`casella/ThermoPower`, Politecnico di Milano) | `TurbineStodola` and friends; applied to steam generators, combined cycles, Gen-III/IV nuclear plants | **Modelica License 2** | **Do not port.** ML2 is not clearly GPL-compatible and does not cleanly permit forking. Read for ideas only; no code, no structure-copying |
| **MOOSE Thermal Hydraulics Module** (INL) | Component-network system TH — pipes, junctions, valves; regression + MMS verification | LGPL 2.1 | Reference for component-network architecture. **Note:** an Edwards–O'Brien case could *not* be confirmed in its public docs during this search — do not assume one exists |
| **OpenFOAM** `wallBoiling` sub-models + published validations | RPI wall-partitioning validated against Bartolomei and DEBORA | GPL | Directly relevant to §4.2; the arXiv:1709.01783 paper is the usable write-up |
| **CoolProp / IAPWS Python** | IF97 implementations with verification suites | — | We already fork CoolProp (`outram-park-fork-coolprop`); relevant to §5 |

**Licence rule reminder:** any code ported from these must keep its
attribution header block (upstream project, source file, version/commit,
copyright, licence) per the workspace `RESEARCH_INTEGRITY_AND_PROVENANCE.md`.
The ThermoPower row is a hard "no", not a caution.

### 6.1 ClaRa licence — verified 2026-08-04

Checked directly rather than taken from a search summary, because a wrong
call here contaminates the crate. Three independent levels agree on
**BSD-3-Clause**:

1. **Repo `LICENSE`** (`xrg-simulation/ClaRa-official`) — verbatim standard
   BSD 3-Clause text. `Copyright (c) Copyright 2013-2023, ClaRa development
   team`; the team is TLK-Thermo GmbH (Braunschweig) and XRG Simulation GmbH
   (Hamburg).
2. **Per-file header** on `ClaRa/Components/TurboMachines/Turbines/`
   `SteamTurbineVLE_L1.mo` (ClaRa v1.9.0) — "Licensed by the ClaRa
   development team under the 3-clause BSD License. Copyright 2013-2024."
   Identical header in both the `xrg-simulation` and `ClaRaLibrary` copies.
3. **Per-model documentation annotation** in that same file — "This
   component was developed by ClaRa development team under the 3-clause BSD
   License." Original contribution: DYNCAP/DYNSTART development team,
   copyright 2011–2024, funded by the German Federal Ministry for Economic
   Affairs and Energy (FKZ 03ET2009, FKZ 03ET7060).

Four things to know before relying on this:

- **GitHub's licence detector reports `NOASSERTION` ("Other")** for the
  repo. That is a false alarm caused by the malformed copyright line
  (`Copyright (c) Copyright 2013-2023,`), which defeats the strict matcher.
  The licence body is unmodified BSD-3. Expect any automated licence scan we
  run to flag it; this note is the answer.
- **Prefer `xrg-simulation/ClaRa-official`.** The `ClaRaLibrary/ClaRa` mirror
  has **no `LICENSE` file at its root** (GitHub API returns 404) — its
  per-file headers still say BSD-3, but a repo-level licence file is the
  stronger record.
- **Per-model provenance can differ from the blanket licence.** Every ClaRa
  file carries the warning "Contents published in ClaRa have been
  contributed by different authors and institutions. Please see model
  documentation for detailed information on original authorship and
  copyrights." So check the specific model's documentation annotation before
  porting it — as was done above for `SteamTurbineVLE_L1.mo`, which is
  clean. Do not generalise that result to other ClaRa models.
- **ClaRa+ and the older demo distribution are Modelica License 2**, not
  BSD. Do not treat "ClaRa" as one thing; only the official BSD-3 library
  above is portable.

**Dependency note:** the ClaRa turbine model imports TILMedia functions. The
`TILMediaClaRa` submodule (`TLK-Thermo/TILMediaClaRa`) is itself
BSD-3-Clause — cleanly detected, no ambiguity. It is *not* the commercial
TILMedia product of the same family. In any case we do not need it: our
property layer is IF97 via `TampinesSteamTableCV`, so a port takes the
Stodola/efficiency/rotor algebra and drops the media calls.

**What porting obliges us to do:** retain the copyright notice, the list of
conditions, and the disclaimer; and not use the names of the copyright
holder or contributors (ClaRa, TLK-Thermo, XRG Simulation) to endorse or
promote our derived work. That maps directly onto the workspace attribution
header block required by `RESEARCH_INTEGRITY_AND_PROVENANCE.md`.

### 6.2 Where ported code goes (maintainer direction, 2026-08-04)

**Ports arising from this scope land as modules inside the existing
`tampines-steam-tables` and `tampines` crates — not as new workspace member
crates.** So, concretely:

- A ClaRa-derived Stodola flow law and turbine work-extraction closure
  belongs under `tampines-steam-tables/src/steam_turbine_equations/`
  (alongside `converging_diverging_nozzles/` and `generator.rs`), with the
  component-level wiring in `tampines/src/components/turbine.rs`.
- Two-phase friction multipliers (Thom, Martinelli–Nelson) and void-fraction
  closures belong under the existing `tampines/src/hem/` and
  `tampines/src/single_phase/` module trees, or beside `TampinesSteamArray`
  in `openfoam_algorithms/rhoPimpleFoam/`.
- A new crate is justified only if something outgrows this — publishable on
  its own, with its own dependency set. Default to a module.

This keeps the attribution-header requirement above at file granularity: a
ported module carries its own upstream header block even when it lives
inside one of our crates.

---

## 7. Recommended ordering

Property layer (§5) → finish Marviken (§4.4) → pipe Δp (§4.1) → void
fraction (§4.2) → turbine `expand_to()` + Baumann → turbine efficiency vs
SCC (§3.1) → Stodola off-design (§3.3) → full heat balance (§3.2). Nozzle
diagnostics (§3.4) and post-dryout (§4.3) are independent and can run any
time after the property layer.

---

## 8. Data-provenance requirement

Every case adopted from this list needs a `References.md` beside its test
recording: source, author/organisation, publication or dataset title,
licence/access terms, URL/DOI, date accessed, and every digitisation step
and assumption. `marviken_tests.rs` is the pattern to follow — it names
NUREG/CR-2671, the figure, the page, and the graphreader points.

All sources listed here are open literature or public regulatory documents.
Nothing in this scope requires proprietary, partner-confidential, or
operational-facility data, and none may be introduced to satisfy it.

---

## 9. Beads

Filed 2026-08-04. Run `bd show <id>` for per-case detail, `bd ready` for
what is unblocked.

**Pipe / property / nozzle — under `op-21g` (tampines-steam-tables):**

| Bead | Case | Section | Depends on |
|---|---|---|---|
| `op-21g.16` | Finish Marviken test 24 | §4.4 | — (ready) |
| `op-21g.17` | IAPWS-IF97 R7-97 region verification tables | §5 | — (ready) |
| `op-21g.18` | Thom / Martinelli–Nelson two-phase Δp | §4.1 | `op-21g.17` |
| `op-21g.19` | Bartolomei & Chanturiya void fraction | §4.2 | `op-21g.17` |
| `op-21g.20` | Bennett AERE-R5373 post-dryout | §4.3 | `op-dt3.16` |
| `op-21g.21` | Moore B / Barschdorff nozzle diagnostic | §3.4 | `op-21g.17` |

**Turbine — under `op-dt3` (tampines):**

| Bead | Case | Section | Depends on |
|---|---|---|---|
| `op-dt3.21` | `expand_to()` — (p,s) flash + Baumann wetness | §1, §3.1 | `op-dt3.18` |
| `op-dt3.22` | Stodola cone law (off-design flow) | §3.3, §6 | `op-dt3.18` |
| `op-dt3.23` | Shaft-work sink → rotor inertia + generator | §1 | `op-dt3.18`, `op-dt3.21` |
| `op-dt3.24` | V&V vs Spencer-Cotton-Cannon | §3.1 | `op-dt3.21` |
| `op-dt3.25` | V&V vs AP1000 heat balance | §3.2 | `op-dt3.21`, `op-dt3.22` |

The turbine chain all sits behind `op-dt3.18` (the HEM-baseline turbine
direction), so the two immediately actionable items are `op-21g.16`
(Marviken) and `op-21g.17` (property gate).
