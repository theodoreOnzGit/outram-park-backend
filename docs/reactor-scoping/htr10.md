# HTR-10 — 10 MWth pebble-bed high-temperature gas-cooled reactor

Scoping document for an offline digital-twin simulator of the HTR-10, built in
`crates/outram-park-digital-twin-engine` with its coupled steam-generator
secondary loop.

> **Intended use.** Education, research, capability building, and V&V only. This
> is an offline demonstration with no connection to any operational system. See
> `RESPONSIBLE_USE.md`.
>
> **Status of this document.** Capability findings come from a codebase audit
> performed 2026-08-06. Validation source identifiers are **deliberately
> unverified** — see [Open validation data](#open-validation-data).

Relates to the existing bead `op-wqk.9` (`htgr_sim_v1`) and its children.

## 1. Framing correction — read this first

**The existing `htgr_sim_v1` example is a prismatic-block HTGR, not a pebble
bed.** It says so at `crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/primary_loop.rs:3`.
There is no pebble bed anywhere in it: no packed-bed pressure drop, no bed
conductivity, no pebble conduction, no graphite properties.

**For HTR-10 the core model is a rewrite, not a retune.** The secondary loop,
the app shell and the widget layer are reusable almost as-is — which is a
substantial head start, but the core is new work.

## 2. Plant configuration

| Loop | Fluid | Purpose |
|---|---|---|
| Primary | Helium, approx. 7 MPa | Flows through a graphite-moderated pebble bed; multi-pass pebble recirculation |
| Secondary | Water / steam | Helical-coil once-through modular steam generator with helium on the shell side |

Passive safety case rests on the reflector, core barrel and reactor-cavity
cooling path — the decay-heat route that has no model in this workspace today.

## 3. Capability audit

Audited 2026-08-06 against the workspace at commit `ebbde1b`.
Verified this session: `htgr_sim_v1` tests 12/12 pass, `fhr_sim_v2` 3/3 pass.

### HAVE

| Capability | Where | Notes |
|---|---|---|
| **Helium equation of state and transport** | `crates/outram-park-fork-coolprop/src/fluids/helium.rs:13`, `src/transport.rs:710,732` | Full Helmholtz EOS plus real NIST-lineage viscosity and conductivity correlations. Tested against tabulated reference states including the critical point |
| Helium consumed live by the existing sim | `examples/htgr_sim_v1/physics/primary_loop.rs:314` | With a test documenting heat capacity against the ideal-gas limit |
| **Wakao packed-bed particle-to-fluid Nusselt** | `crates/tuas_boussinesq_solver/.../nusselt_number_correlations/input_structs.rs:152` | Reynolds and Nusselt on pebble diameter — directly the right form |
| Three-array porous-media component | `crates/tuas_boussinesq_solver/src/lib/pre_built_components/non_insulated_porous_media_fluid_components/` | Fluid array, shell, interior solid matrix with real radial nodal conductances. The correct *structural* template for a pebble bed |
| **Doubly heterogeneous Monte Carlo transport** | `crates/outram-mc-libs/src/pebble_beds/` | Woodcock delta tracking, k-eigenvalue over packed kernels, random sequential adsorption sphere packing. The crate's deliberate specialisation, and its strongest pebble-bed asset |
| **TRISO fission-product diffusion and release** | `crates/boon-lay` | Four-layer TRISO CSG, Lagrangian Monte Carlo with walk-on-spheres first passage, temperature-dependent per-layer diffusion, closed-form Booth solution. Includes a port of INL's TRISO release code |
| Granular DEM with thermal contact | `crates/outram-park-fork-liggghts` | Hooke and Hertz-Mindlin contact, rolling resistance, contact conduction, grey-body radiation with near-field gas-gap conduction |
| Point kinetics, Doppler, rod worth, xenon | `crates/teh-o-prke` | Six-group PRKE, delayed neutron layer, closed-form prompt excursion, S-curve rod worth, iodine/xenon dynamics |
| **Working Rankine secondary cycle** | `examples/htgr_sim_v1/physics/secondary_loop.rs` | Real feedwater pump work, isentropic expansion, condenser energy balance, lagged feedwater control. Six tests pass with methodology and results documented |
| **Genuinely two-way primary/secondary coupling** | `examples/htgr_sim_v1/physics/mod.rs:119` | Secondary saturation temperature is read before the primary step and used as the IHX pinch, so core inlet is a computed variable. Covered by a test |
| Engine widget, animation and threading framework | `crates/outram-park-digital-twin-engine/src/` | Plus a working OPC-UA telemetry path if the twin ever needs one |

### SCAFFOLD — do not count as working

- **Ergun is a `todo!()`.** The packed-bed pressure-drop variant is declared with
  its citation at
  `crates/tuas_boussinesq_solver/.../fluid_component_calculation/mod.rs:48` and
  unimplemented at `:161`. The gFHR pebble-bed components carry the comment
  "not putting in ergun equation yet" **seventeen times**, using pipe friction on
  a pebble-derived hydraulic diameter instead.
- The TUAS porous-media component states its own gap: pressure-drop correlations
  are not properly implemented, so it behaves like a pipe.
- `fhr_sim_v2`'s pebble-bed thermal hydraulics is a **single lumped enthalpy
  control volume** for a UO2 pebble with externally supplied heat transfer
  coefficient and area, defaulting to a constant-heat-capacity heuristic. It is
  UO2, not graphite matrix, so it does not transfer to HTR-10.
- **CFD-DEM coupling is an explicit no-physics stub** —
  `crates/outram-park-fork-liggghts/src/coupling.rs:25` states that every
  behavioural method returns not-implemented, with no drag law, no interpolation,
  no volume averaging and no fluid solve.
- **Decay heat is self-flagged as suspect.** `crates/teh-o-prke/src/decay_heat.rs:12`
  carries the author's own comment doubting its correctness on unit grounds. It is
  not currently wired into `htgr_sim_v1`.
- `htgr_sim_v1`'s live steam pressure is hard-fixed, so there is no
  sliding-pressure or drum dynamics.
- The whole `crates/tampines` component layer returns not-implemented; only
  `Pipe::step` is real.

### Defect worth its own bead

**A TRISO "verification" test does not verify anything.**
`crates/boon-lay/.../release_fraction_crp_6_case_1a_1b.rs:50` wraps its assertion
against the published reference range in `catch_unwind` and **discards the
result**, then asserts the code's own output against a value that lies **outside**
that reference range. The test passes while the model disagrees with the
reference it names. This should be treated as unvalidated and fixed.

### MISSING

| Gap | Size | Notes |
|---|---|---|
| Ergun or KTA-form packed-bed pressure drop | Small to write, Medium to wire and validate | Nothing exists |
| **Pebble-bed effective radial conductivity** | Medium | Solid, gas, contact and radiation contributions plus wall-region correction. Literally zero code in the workspace |
| **Graphite properties** — matrix and reflector grades, conductivity as a function of temperature and fast-neutron dose | Medium | The solid database holds only copper, stainless, fibreglass, aerogel, FeCrAl and a generic heating element. No graphite anywhere |
| Gas arm in the TUAS material enum | Small–Medium | `Material` is solid-or-liquid only, so the whole TUAS prebuilt component library cannot be used for a helium loop |
| Radial pebble conduction, fuel zone to surface | Medium | GeN-Foam's pebble routine was deliberately not ported; the app-builder crate says so explicitly |
| **Reflector, barrel and cavity-cooling decay-heat path** | Medium–Large | This is the HTR-10 passive safety case. No model |
| Multi-pass pebble flow and recirculation | Medium | DEM primitives exist but are unvalidated |
| Helical-coil once-through SG, three-zone moving boundary | Medium | The LMTD and NTU algebra exists in `crates/outram-park-fork-dwsim-libs`; the zone tracking and helical correlations do not |
| SG inventory giving sliding steam pressure | Medium | The real remaining content of bead `op-wqk.9.3` |
| **Graphite/moderator temperature feedback as a separate channel** | Small–Medium | Central to HTR-10 loss-of-flow behaviour. Only lumped fuel feedback exists |
| Trustworthy decay heat | Small–Medium | See the flagged defect above |
| Wall-friction or porous-drag source in the compressible pipe solver | Medium | The CoolProp compressible solver has no friction term at all; pressure drop comes only from the resolved momentum equation |
| Nuclear data for a from-first-principles criticality calculation | Large | `reference-data/endf/` holds only a README |

### Bead accuracy — three of four children are stale

| Bead | Recorded state | Reality |
|---|---|---|
| `op-wqk.9.1` helium TH are scaffold placeholders | open | **Half stale.** Helium properties are real EOS and Darcy-Weisbach friction is implemented. Still true: single lumped node, and a hardcoded constant viscosity despite the real correlation existing one crate away |
| `op-wqk.9.2` wire kinetics to the delayed neutron layer | closed | **Correct** |
| `op-wqk.9.3` secondary is scaffold | open | **Mostly stale.** The listed defects — simplified IHX duty, fixed secondary mass flow, no real turbine expansion or condenser balance — are all now implemented and tested. Only fixed steam pressure remains true |
| `op-wqk.9.4` schematic omits the pipe widget | open | **Stale** — the schematic uses it for every connector |

> **See also [vtb-findings.md](vtb-findings.md)** — the vendored NRIC/INL
> Virtual Test Bed carries material that closes several gaps recorded below.
> For HTR-10 specifically it supplies confirmed report identifiers and reference
> values that were deliberately left unasserted here.

## 4. Open validation data

**Access tier: openly published, and unusually good for this reactor.**

> **No report identifiers, benchmark numbers, or measured values are asserted
> here.** They must be obtained from the actual documents.

| Source | Confidence | Relevance |
|---|---|---|
| **IAEA coordinated research programme benchmark on HTGR performance**, pairing HTR-10 with Japan's HTTR for initial testing | High | The canonical HTR-10 neutronics validation target. Cases include first criticality (critical loading height), temperature reactivity coefficient and control-rod worth. Widely reproduced in the Monte Carlo literature |
| **Follow-on IAEA programme** extending to steady-state operation and transients | High | Coupled neutronics and thermal-hydraulics code comparison |
| **HTR-10 safety demonstration tests** — loss of forced cooling without scram, and control-rod withdrawal without scram | High | The best available transient validation targets. Expect digitised plots rather than tabulated data |
| **The HTR-10 design description paper** by Wu, Lin and Zhong in *Nuclear Engineering and Design* | High on authors, title and journal; volume and pages unverified | Standard open source for core geometry, power, helium conditions and SG arrangement |
| **OECD/NEA PBMR-400 coupled benchmark** | High | Not HTR-10, but the most completely specified open **pebble-bed** benchmark, covering steady state plus depressurised and pressurised loss-of-cooling and rod withdrawal. The right *first* target before attempting HTR-10 |
| **IAEA programme on HTGR fuel technology** — TRISO fission-product release cases | High | `crates/boon-lay` already names one in a filename, though that test is the flagged self-referential one |
| **German AVR and THTR-300 operating data**, and packed-bed afterheat-removal experiments | Medium | Directly relevant for validating a pebble-bed effective conductivity model |
| HTR-10 as an evaluated criticality-handbook case | **Low — verify independently** | Would not rely on this |

**Nothing HTR-10-specific is currently in this repository.** `reference-data/`
contains only a README.

## 5. Recommended sequencing

1. **Target the PBMR-400 benchmark before HTR-10.** It is the most completely
   specified open pebble-bed case, and it exercises exactly the coupling that
   must be built. Reaching HTR-10's measured transients with an unvalidated bed
   model would be guessing.
2. Build the bed closures first — pressure drop, effective conductivity, graphite
   properties — since every downstream result depends on them.
3. Add the graphite feedback channel before attempting any loss-of-flow transient.
4. The reflector and cavity-cooling path is what makes HTR-10 interesting. Budget
   for it rather than deferring it indefinitely.

## 6. Proposed work breakdown

| Bead | Work | Depends on |
|---|---|---|
| `htr10_sim_v1` | Parent; distinct from the prismatic `htgr_sim_v1` | — |
| Refresh the stale `op-wqk.9.*` children | Bookkeeping against current reality | — |
| Graphite properties in the solid database | Matrix and reflector grades, conductivity versus temperature and dose | — |
| Ergun / KTA packed-bed pressure drop | Implement and wire into the friction path | — |
| Pebble-bed effective radial conductivity | Solid, gas, contact and radiation contributions | Graphite properties |
| Gas arm in the TUAS material enum | Unlocks the prebuilt component library for helium | — |
| Radial pebble conduction | Fuel zone to pebble surface | Graphite properties |
| Reflector, barrel and cavity cooling path | The passive decay-heat route | Bed conductivity |
| Graphite temperature feedback channel | Separate from fuel feedback | — |
| Fix or replace decay heat | Currently self-flagged as suspect | — |
| Fix the self-referential TRISO release test | Correctness defect in `crates/boon-lay` | — |
| Helical-coil once-through SG | Three-zone moving boundary | — |
| SG inventory giving sliding steam pressure | Closes the real `op-wqk.9.3` gap | Helical SG |
| V&V against the PBMR-400 benchmark | Methodology and measured results per the workspace V&V rule | Bed closures, feedback channel |
| V&V against HTR-10 initial criticality and safety demonstration tests | Same | PBMR-400 first |

## 7. Open questions for the maintainer

1. **Separate simulator, or generalise the existing one?** `htgr_sim_v1` is
   prismatic. A pebble-bed HTR-10 could be a sibling example or a mode of the
   same one. Given the core model is a rewrite either way, a sibling seems
   cleaner — but it duplicates the app shell.
2. **Should the stale `op-wqk.9.*` beads be refreshed now?** Three of four
   describe defects that have since been fixed. I have not touched them; closing
   or editing beads is your call.
3. **Does the self-referential TRISO test warrant a `crates/boon-lay` bead
   immediately?** It is a correctness issue independent of this reactor slate.
