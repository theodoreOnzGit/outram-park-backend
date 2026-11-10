# EBR-II — Experimental Breeder Reactor II (sodium-cooled fast reactor)

Scoping document for an offline digital-twin simulator of EBR-II, built in
`crates/outram-park-digital-twin-engine` with its coupled intermediate loop and
steam plant.

> **Intended use.** Education, research, capability building, and V&V only. This
> is an offline demonstration with no connection to any operational system. See
> `RESPONSIBLE_USE.md`.
>
> **Status of this document.** Capability findings come from a codebase audit
> performed 2026-08-06. Validation source identifiers are **deliberately
> unverified** — see [Open validation data](#open-validation-data).

## 1. Why this reactor, and why not EBR-I

EBR-II is the liquid-metal case worth building. It is a pool-type sodium fast
reactor that operated for three decades, and in 1986 it was deliberately
subjected to an **unprotected loss-of-flow at full power with the scram system
disabled** — the reactor shut itself down on inherent reactivity feedback alone.
That test, and its protected counterpart, were later used as an international
benchmark exercise. It is a rare case where the headline physics and the
published validation data are the same thing.

**EBR-I is not recommended as a validation case.** It is historically
significant, but no modern benchmark-quality instrumented transient dataset
comparable to the EBR-II tests appears to exist. Its one quantitatively
documented event — the 1955 Mark-II partial meltdown, traced to a positive
prompt power coefficient caused by fuel-rod bowing — rests on 1950s-era, sparsely
instrumented, reconstructed data. It is an interesting qualitative target for a
bowing feedback model and nothing more.

Note also that **EBR-I used NaK, not sodium**. If it is modelled at all, it needs
the NaK property set, not the sodium one.

## 2. Plant configuration

| Loop | Fluid | Purpose |
|---|---|---|
| Primary | Liquid sodium | Large pool containing core, pumps and IHX; free surface, thermally stratified |
| Intermediate | Liquid sodium | Carries heat from the IHX to the steam plant; isolates radioactive primary sodium from water |
| Tertiary | Water / steam | Steam generator, turbine, condenser, feedwater |

The pool is the defining feature. Its thermal inertia and stratification set the
timescale of every loss-of-flow and loss-of-heat-sink transient, and it couples
the core outlet, the IHX inlet and the shutdown coolers.

Core is a **wire-wrapped hexagonal bundle** of **metallic** fuel ~~(U-Zr and
U-Pu-Zr)~~ **CORRECTED 2026-11-10:** the MARK-II driver fuel of the SHRT-17 and
SHRT-45R cores is **U-5Fs** (uranium with 5 wt % fissium), 67 wt % U-235
(ANL-ARC-228 Table 3.1.1 p.17 and Table 3.2.2 p.21, transcribed in
`reference-data/ebr2/`). The 1963 wet-critical core was also U-5Fs, at 48.4 %
enrichment (ANL-6864 p.11). The gap is **sodium-bonded**. All three of those
properties matter and none is currently modelled.

## 3. Capability audit

Audited 2026-08-06 against the workspace at commit `ebbde1b`.

### HAVE

| Capability | Where | Notes |
|---|---|---|
| **Liquid sodium properties** | `crates/outram-park-fork-coolprop/src/incompressibles/fluids/liqna.rs:10` | CoolProp `LiqNa`, valid 400–2500 K; polynomial density, heat capacity, conductivity, exponential viscosity and saturation pressure |
| **NaK eutectic properties** | `crates/outram-park-fork-coolprop/src/incompressibles/fluids/nak.rs` | Relevant to EBR-I |
| Shell-and-tube heat exchanger | `crates/tuas_boussinesq_solver/src/lib/pre_built_components/shell_and_tube_heat_exchanger/` | ~6000 lines, discretised counter/parallel flow, calibratable Nusselt both sides, has a validation case. The IHX is essentially there |
| Multi-branch loop networks with natural circulation | `.../pre_built_components/ciet_three_branch_plus_dracs/`, `.../gfhr_pipe_tests/multi_branch/` | Structurally the closest analogue to pool plus intermediate loop plus shutdown-heat-removal path |
| **SFR expansion feedback physics** | `crates/outram-foam-appbuilder-lib/src/genfoam/thermo_mechanics/feedback.rs:96,125,161` | Free axial expansion, cumulative axial expansion profile, mesh displacement assembly. GeN-Foam port, documented as fast-reactor stabilising feedback |
| Fast-spectrum nuclear data | `crates/njoy-outram-park-fork/src/data/wmp_core.wmpl` | Embedded ENDF/B-VII.1 windowed multipole set already includes Na-23, U-235/238, Pu-239/240, Zr, Mo, and the Cr/Fe/Ni/Mn/Si needed for HT9 and stainless |
| Fast-reactor weighting spectrum | `crates/njoy-outram-park-fork/src/groupr/weights.rs:123` | VITAMIN-E |
| Monte Carlo transport, spectrum-agnostic | `crates/outram-mc-libs` | k-eigenvalue, fixed source, Woodcock tracking, Watt/Maxwellian sources. Thermal scattering is an optional below-cutoff layer a fast core simply does not activate |
| Closed-form prompt excursion timestepper | `crates/teh-o-prke/src/nordheim_fuchs.rs:145` | Takes generation time, delayed fraction and feedback coefficient as arguments, so it is spectrum-agnostic. Important: it has no timestep-versus-generation-time restriction, which matters when generation time is of order microseconds |
| IAPWS-IF97 water and steam | `crates/tampines-steam-tables` | Strongest asset in the workspace; full region coverage plus turbine equations. Tests pass |

**Verification actually run:** the CoolProp fork's full suite passes in release,
including a smoke test that evaluates all 126 incompressible fluids — sodium and
NaK among them — for finite positive properties and an enthalpy/temperature
round-trip.

### SCAFFOLD — do not count as working

- ~~**Sodium is unreachable from the thermal-hydraulics stack.**~~ **CORRECTED
  2026-11-10 (checked in code, gh:#1331):** TUAS now has
  `liquid_database::liquid_sodium_coolprop::liquid_sodium()` (CoolProp `LiqNa`,
  400-1000 K, gh:#1320), used by `outram_park_digital_twin_engine::ebr2::model`.
  The original finding follows. `LiquidMaterial`
  (`crates/tuas_boussinesq_solver/.../boussinesq_thermophysical_properties/mod.rs:126`)
  has no sodium arm, and `TampinesFluid`
  (`crates/tampines/src/fluids/mod.rs:23`) wraps only the equation-of-state
  fluids, not the incompressible set. The data exists; nothing that needs it can
  see it.
- **The expansion feedback loop is not closed.** The genfoam thermo-mechanics
  code produces geometry, but the neutronics side is pinned to a single static
  feedback state — `crates/outram-foam-appbuilder-lib/.../neutronics/diffusion/fields.rs:45`
  states per-cell feedback driven by live thermal-hydraulics is deferred. So
  geometry to cross-sections to reactivity does not currently connect.
- **Precursor decay constants are identical across nuclides.**
  `crates/teh-o-prke/.../six_group_constants.rs:24` returns the same decay
  constant array for U-233, U-235 and Pu-239, and says so in its own comment. The
  half-life set at `:66` carries a comment doubting its provenance. This is a
  latent correctness trap that should be fixed before any effective delayed
  fraction is trusted.
- **Fuel temperature feedback is explicitly thermal-spectrum.** Functions at
  `crates/teh-o-prke/src/fuel_temperature_feedback.rs:213,243` are named for
  thermal spectrum ~~and have no fast-spectrum sibling~~. **CORRECTED
  2026-11-10 (gh:#1326):** a fast-spectrum sibling now exists,
  `teh_o_prke::feedback_mechanisms::fast_reactor::FastDopplerTerm`
  (`-K_D ln(T/T_ref)`, GeN-Foam's `fastNeutrons` branch).
- **The steam side is unwired.** `crates/tampines/src/components/turbine.rs:39`,
  `condenser.rs:37` and `heat_exchanger.rs:40` all return a not-yet-implemented
  error. Their own doc comments note the underlying algebra already exists in
  `crates/outram-park-fork-dwsim-libs` — this is wiring, not derivation.
- **Engine heat-exchanger widget is wired to a stub.**
  ~~`crates/outram-park-digital-twin-engine/src/components/heat_exchanger.rs:29`~~ `crates/bugis/src/components/heat_exchanger.rs:29` (moved 2026-11-02, gh:#1112; line numbers not re-checked)
  draws a fixed-colour rectangle and its doc comment says it has no fluid state
  to colour by.
- The low-Peclet axial-conduction correction that sodium actually depends on is
  self-flagged as buggy at
  `crates/tuas_boussinesq_solver/.../fluid_array_lateral_coupling/calculation.rs:636`.

### MISSING — ranked

#### 1. SFR reactivity feedback set — Medium–Large. **The blocker.**

EBR-II's famous behaviour *is* the expansion feedbacks. `teh-o-prke` currently
offers a six-factor-formula feedback set, which is a thermal-reactor
parameterisation and the wrong shape for a fast core. Four feedbacks dominate and
~~none is available as a reactivity coefficient a point-kinetics model can sum~~
**CORRECTED 2026-11-10 (gh:#1326): all four are now available as coefficient
terms a point-kinetics model can sum**, in
`crates/teh-o-prke/src/feedback_mechanisms/fast_reactor.rs` (a port of GeN-Foam's
point-kinetics feedback sum, beside the separable model). What remains open is
the coefficients themselves (caller inputs, none built in) and the
thermal-hydraulic side that supplies the temperatures:

- axial fuel expansion — portable from the orphaned genfoam code **(done: `axial_fuel_expansion`)**
- radial / grid-plate expansion — ~~new~~ **done: `radial_expansion`, one term per structure**
- control-rod-drive-line expansion — ~~new~~ **done: `driveline` (expansion x rod-worth map, as GeN-Foam; no thermal lag, as GeN-Foam)**
- sodium density and void worth — ~~new~~ **done: `coolant_density`**

Without these the simulator will not reproduce the self-shutdown at all. It will
behave like a generic reactor with a fuel-temperature coefficient.

#### 2. Pool-type primary hydraulics — Medium

No pool, plenum or free-surface control volume exists anywhere in the workspace;
every control volume in TUAS is a one-dimensional pipe. Needs a multi-node
stratified pool with buoyancy-driven exchange between nodes and natural
circulation closure once the primary pumps coast down.

#### 3. Sodium properties and low-Prandtl heat transfer reaching TUAS — Small–Medium, but gates everything

Ordinary Dittus-Boelter is invalid for liquid metals ~~and no low-Prandtl
correlation exists in the workspace~~. **CORRECTED 2026-11-10 (checked in code,
gh:#1331):** TUAS has named liquid-metal correlations (Lyon-Martinelli,
Seban-Shimazaki, Kazimi-Carelli, Schad-modified, Mikityuk) in
`heat_transfer_correlations::nusselt_number_correlations::liquid_metal`
(gh:#1321). Path, as originally written:

1. Add a first-class sodium arm to `LiquidMaterial` delegating to the CoolProp
   incompressible, or use the `CustomLiquid` escape hatch for a same-day
   prototype. The hatch is fully threaded through every property path, and
   Prandtl and diffusivity derive from the four supplied functions.
2. Add an incompressible variant to `TampinesFluid`.
3. Express Lyon-Martinelli through the **existing** correlation form
   `Nu = a + b\,Re^{c} Pr^{d}` at
   `crates/tuas_boussinesq_solver/.../nusselt_number_correlations/input_structs.rs:16`
   — this is configuration, not new code.
4. Write genuine wire-wrapped-bundle sodium correlations as a new variant. These
   are geometry-dependent and do **not** fit the existing form.
5. Fix or bound the acknowledged-buggy low-Peclet axial-conduction correction.

#### 4. Metallic fuel performance — Large

`crates/outram-park-fork-offbeat` is oxide-and-Zircaloy only. Its conductivity
and expansion models cover UO2 and MOX; its corrosion module is Zircaloy
waterside oxidation, which is physically meaningless in sodium. Missing for
EBR-II specifically: U-Zr and U-Pu-Zr properties, the large anisotropic free
swelling of metallic fuel, fuel-cladding chemical interaction, and the
**sodium-bonded gap** — the existing gap model assumes helium, a fundamentally
different conductance regime.

Not required for a first thermal-hydraulic twin.

#### 5. Fast-spectrum kinetics constants — Small as data entry

Generation time for EBR-II is of order a tenth of a microsecond, roughly four
orders below the value the HTGR example uses. Nordheim-Fuchs is the right choice
precisely because it is closed-form and carries no stiffness restriction.

#### 6. Steam side wiring — Medium

## 4. Open validation data

**Access tier: mixed — verify before relying.**

> **No report identifiers, benchmark numbers, or measured values are asserted
> here.** They must be obtained from the actual documents.

**High confidence.** The IAEA ran a coordinated research project on EBR-II
shutdown-heat-removal tests. Two exercises were used: a **protected**
loss-of-flow, in which the pumps trip and the reactor scrams, and an
**unprotected** loss-of-flow at full power with the scram system deliberately
disabled. Argonne National Laboratory was the data provider and specification
author. The project followed the usual blind-calculation-then-data-release
structure, and a substantial peer-reviewed comparison literature exists.

**Lower confidence — the access question.** The detailed benchmark specification
may have been distributed to project participants rather than published for open
download, and may require a request through the IAEA or ANL. The *comparison
results* are in the open literature regardless. **Confirm access terms before
committing to this as the validation case.**

Measured quantities the benchmark is expected to cover, qualitatively: core inlet
and outlet sodium temperatures, instrumented-subassembly temperatures and flows,
primary pump coastdown histories, intermediate heat exchanger conditions on both
sides, pool temperatures, and reactor power. The unprotected test exercises
exactly the feedbacks listed as gap 1 — which is why that gap ranks first.

**Also worth chasing.** ANL plant description and design reporting for EBR-II is
extensive and openly published. The Mark-II and Mark-III metallic fuel
irradiation performance database underpins essentially all open-literature U-Zr
and U-Pu-Zr correlations, and is the source for gap 4.

**EBR-I.** Some EBR-I criticality configurations may appear in the international
criticality safety benchmark handbook. Medium-low confidence. If present, treat
any such entry as a **criticality** benchmark only — useful for validating cross
sections, useless for transient thermal-hydraulics.

**Added 2026-11-10 (gh:#1331):** a lumped, uncalibrated SHRT-17 / SHRT-45R
model is now in the engine library as `outram_park_digital_twin_engine::ebr2`.
It has:

- cited data;
- Argonne's kinetics and feedback coefficients;
- a four-node fuel / core / IHX / pool model;
- the NATDEMO pump curve.

It is NOT VALIDATED: see
`crates/outram-park-digital-twin-engine/docs/ebr2_library.md`. It does not
close gap 2 (a stratified pool control volume in TUAS) or gap 4.

## 5. Recommended scope for a first twin

Build the **thermal-hydraulic and kinetics twin** — pool, intermediate loop, IHX,
steam plant — with the SFR expansion feedbacks, and target the **protected**
loss-of-flow first. Defer metallic fuel performance entirely.

The unprotected test is the prize, but it is the harder target: it depends on all
four expansion feedbacks being right, and getting it wrong produces a
plausible-looking simulation that is physically meaningless. Reaching it after
the protected case is validated is the honest ordering.

## 6. Proposed work breakdown

| Bead | Work | Depends on |
|---|---|---|
| `sfr_sim_v1` | Parent task | — |
| Sodium reachable from TUAS | `LiquidMaterial` arm plus `TampinesFluid` incompressible variant | — |
| Lyon-Martinelli via existing correlation form | Configuration only | Sodium reachable |
| Wire-wrapped bundle sodium Nusselt | New correlation variant | Sodium reachable |
| Low-Peclet axial conduction fix | Bound or repair the flagged-buggy correction | — |
| Stratified pool control volume | Multi-node, free surface, buoyancy exchange | — |
| SFR feedback module in `teh-o-prke` | Axial (port from genfoam), radial/grid-plate, CRDL, sodium density. **Built 2026-11-10 (gh:#1326), coefficients are caller inputs** | — |
| Fast-spectrum kinetics constants | Data entry, sourced and cited | Fix the identical-decay-constant defect first |
| Fix identical precursor decay constants | Correctness defect, affects all reactors | — |
| Intermediate loop plus IHX | Sodium-to-sodium | Sodium reachable, pool CV |
| Steam plant wiring | Connect the dwsim algebra to the tampines stubs | — |
| SFR widget art | Pool vessel, IHX, intermediate loop | — |
| V&V against the protected loss-of-flow test | Methodology and measured results per the workspace V&V rule | All of the above |

## 7. Open questions for the maintainer

1. **Is the benchmark specification actually obtainable?** This determines
   whether EBR-II is a validation case or only a demonstration. Worth resolving
   before the work is scheduled.
2. **Port or re-derive the axial expansion feedback?** The genfoam code is a
   GeN-Foam port sitting in the OpenFOAM app-builder crate. Moving it into
   `teh-o-prke` crosses a crate boundary and duplicates it.
3. **Does the identical-decay-constant defect warrant its own bead now?** It is
   not EBR-II specific — it affects every reactor in this slate that uses the
   six-group model.
