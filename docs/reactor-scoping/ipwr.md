# iPWR — integral pressurised water SMR

Scoping document for an offline digital-twin simulator of a natural-circulation
integral PWR (NuScale-type or Holtec SMR-300-type), built in
`crates/outram-park-digital-twin-engine` with its coupled secondary steam loop.

> **Intended use.** Education, research, capability building, and V&V only. This
> is an offline demonstration with no connection to any operational system. See
> `RESPONSIBLE_USE.md`.
>
> **Status of this document.** Capability findings come from a codebase audit
> performed 2026-08-06. Validation source identifiers are **deliberately
> unverified** — see [Open validation data](#open-validation-data).

Corresponds to the existing bead `op-wqk.10` (`ipwr_sim_v1`).

## 1. Plant configuration

| Loop | Fluid | Purpose |
|---|---|---|
| Primary | Pressurised water, approx. 15.5 MPa | Natural circulation through core and an internal helical-coil steam generator; no primary pumps |
| Secondary | Water / steam, approx. 6–7 MPa | Once-through SG produces steam directly; turbine, condenser, feedwater |

Distinguishing features against a loop PWR: everything inside one vessel, natural
circulation instead of forced flow, a **helical-coil once-through** steam
generator rather than a U-tube recirculating one, and an integral pressuriser in
the vessel head.

## 2. Capability audit

Audited 2026-08-06 against the workspace at commit `ebbde1b`.

### HAVE

| Capability | Where | Notes |
|---|---|---|
| **IAPWS-IF97 water and steam** | `crates/tampines-steam-tables` | The strongest asset in the workspace. 937 tests. Regions 1–5, backward flashes, saturation line, metastable region, transport properties |
| **Tested at PWR pressures specifically** | `.../ph_flash_steam_table/single_phase_table_{100..220}_bar.rs` | The 15.5 MPa primary and 6–7 MPa secondary sit inside the *tested* envelope, not merely the valid one |
| Rich steam control volume | `.../object_oriented_programming/{setter_methods,mass_and_energy_balance}.rs` | Isentropic expansion, isobaric heat addition, mass addition and removal, timestep advance |
| 1-D compressible steam pipe | `crates/tampines-steam-tables/src/openfoam_algorithms/rhoPimpleFoam/` | Finite volume closed by IF97 (p,h); PISO/SIMPLE/PIMPLE |
| **Validated choked flow** | `.../converging_diverging_nozzles/tests/` | Homogeneous equilibrium critical flow tested against Marviken, Moody and Zaloudek. Directly reusable for steam-line-break and tube-rupture flow |
| Real generator with torque balance | `.../steam_turbine_equations/generator.rs:21` | Explicit angular-velocity integration, per-phase power. This is what makes the turbine widget actually spin |
| One-dimensional loop hydraulics | `crates/tuas_boussinesq_solver` | Fluid arrays with lateral conduction; series and parallel hydraulic solvers |
| **Experimentally validated natural circulation** | `crates/tuas_boussinesq_solver/verification_and_validation/coupled_natural_circulation_SAM_vs_TUAS_vs_experiment.md` | 25 steady states, TUAS against experiment and against SAM. Caveat: oil at near-atmospheric pressure |
| Point kinetics, Doppler, xenon, rod worth | `crates/teh-o-prke` | Six-group PRKE, delayed neutron layer, closed-form prompt excursion, iodine/xenon dynamics, S-curve rod worth |
| Working closed-cycle secondary reference | `crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/secondary_loop.rs` | Real feedwater pump work, isentropic expansion de-rated by efficiency, condenser energy balance, lagged proportional feedwater control. Six tests pass |
| PID primitives | `crates/chem-eng-real-time-process-control-simulator` | Proportional, integral, filtered-derivative blocks and transfer functions |
| Engine widget and threading framework | `crates/outram-park-digital-twin-engine/src/{app_scaffold,animation,components}` | Shared state, physics-thread crash detection with restart modal, flow tracers, Crameri colour grading |

### SCAFFOLD — do not count as working

- **The entire `tampines` component layer is unimplemented.** `SteamGenerator::step`,
  `Turbine::expand_to`, `Condenser::condense`, `HeatExchanger::calculate`, plus
  pump, valve and cooling tower, all return a not-yet-implemented error
  (`crates/tampines/src/components/*.rs`). Only `Pipe::step` is real. The crate's
  own `lib.rs:37` says "Scaffold only."
- **Engine widgets inherit that emptiness.** Condenser and heat exchanger widgets
  paint a fixed colour because their physics types carry no fluid state — honestly
  documented, but display-only.
- **The steam-turbine module is nozzles plus a generator**, not turbine stages.
  There is no blade-row model, no expansion line, no moisture separator reheater,
  no feedwater heater train.
- **`htgr_sim_v1`'s live steam pressure is fixed.** No SG mass and energy
  inventory, so no sliding pressure — the secondary cannot respond to load. This
  is the single most important thing to fix when adapting it.
- **Two thousand lines of PWR-relevant two-phase closures are orphaned.**
  `crates/outram-foam-multiphase/src/chf.rs` (Biasi, W-3, Bowring, Groeneveld
  lookup table) and `src/wall_boiling.rs` (RPI heat-flux partitioning, nucleation
  site density, departure diameter and frequency) have 67 tests and are wired to
  no thermal-hydraulic consumer.
- `crates/teh-o-prke/verification_and_validation/` contains only a README — no
  V&V case exists for the kinetics crate.

### MISSING

#### The primary-side gap, and why it is shallower than it looks

**TUAS is architecturally pressure-blind.** Every property call accepts a pressure
and discards it — see the `_pressure` parameters at
`crates/tuas_boussinesq_solver/.../density.rs:49`, and identically in heat
capacity, conductivity and viscosity. That is the Boussinesq design premise. There
is also no water in `LiquidMaterial`; `crates/tampines/src/single_phase/mod.rs:6`
states outright that TUAS "does not yet back water, air, or helium."

But at a **fixed** 15.5 MPa, IF97 Region 1 properties are functions of temperature
alone. So a `CustomLiquid` backed by IF97 at fixed pressure is a **small** job and
unlocks the whole TUAS loop stack for a subcooled primary.

The price, which must be stated in the README: density no longer responds to
pressure, so a pressuriser that moves system pressure cannot feed back into
primary density and natural-circulation head. Acceptable for a v1 educational
twin; a real limitation.

#### Sized gaps

| Gap | Size | Notes |
|---|---|---|
| Fixed-pressure water via `CustomLiquid` backed by IF97 | Small | Unblocks the TUAS loop stack |
| **Bounds-checked IF97 façade returning `Result`** | Small–Medium | See the safety note below |
| **Pressuriser** | Medium | Two-region saturated bubble, spray, heaters, surge line, relief valve. Zero prior art — a workspace-wide grep finds no pressuriser code at all |
| **Helical-coil once-through steam generator** | Large | Nothing helical exists anywhere. Needs coil-curvature Nusselt and friction, boiling-length or three-zone tracking, counter-current coupling. The LMTD and NTU algebra already exists in `crates/outram-park-fork-dwsim-libs` |
| SG mass and energy inventory giving sliding steam pressure | Medium | Closes `htgr_sim_v1`'s biggest declared gap |
| **Boron** — reactivity worth, boration and dilution balance, CVCS | Medium | No soluble-poison model exists in `teh-o-prke` |
| PWR reactivity roll-up: linear Doppler + moderator coefficient + rod-bank worth + boron | Medium | The existing six-factor feedback is multiplicative and closure-driven; a PWR-shaped additive aggregator is cleaner than bending it |
| Turbine stages, moisture separator reheater, feedwater heaters | Medium–Large | Only nozzles and a generator exist |
| Control loops: average-temperature program, pressuriser pressure and level, three-element SG level, turbine governor, steam dump | Medium | Primitives exist; anti-windup, output saturation and cascade structures do not |
| Wire the orphaned CHF and wall-boiling closures into a consumer | Medium | W-3 and Groeneveld are exactly the PWR correlations wanted |
| Fill the flat-rectangle widgets | Medium | Already tracked under `op-wqk.14.*` |

#### Safety note — out-of-range behaviour

The IF97 implementation **panics** rather than returning an error on out-of-range
input, at roughly forty sites. A transient that overshoots will kill the physics
thread. This is partially mitigated by the engine's crash-detection and restart
modal, but an iPWR simulator doing depressurisation or a steam-line break **will**
hit these. A bounds-checked façade returning `Result` should be built before the
transient work, not after.

Related design constraint: the `(T,p)` flash **deliberately panics** in the
two-phase region, because a `(T,p)` saturation point is under-determined. Drive
everything with `(p,h)`. This is consistent with existing workspace guidance.

## 3. Open validation data

**Access tier: mixed, and the vendor tier is the weak one.**

> **No report identifiers, benchmark numbers, or measured values are asserted
> here.** They must be obtained from the actual documents.

### Confident, open, and directly usable

| Source | Why it fits |
|---|---|
| **OECD/NEA main-steam-line-break benchmark** (based on a real loop PWR) | The canonical open coupled three-dimensional-neutronics and system-thermal-hydraulics benchmark. Its entire point is primary–secondary coupling: asymmetric SG cooldown drives moderator and Doppler reactivity into a return to power. Exactly the exercise wanted, though the plant is a large loop PWR, not an iPWR |
| **PWR subchannel and bundle void/DNB tests** | Void distribution and departure from nucleate boiling at PWR pressure. **Already present in this repo** as a vendored GeN-Foam tutorial under `crates/outram-foam-appbuilder-lib/upstream_source/` |
| **VVER-1000 coolant transient benchmarks** | Coolant mixing and steam-line-break-type coupled transients, open specification |
| **Marviken, Moody and Zaloudek critical flow** | **Already used in-repo** as a validated test suite. Covers break and tube-rupture flow |
| **IAEA educational PWR simulator manual** | Nominal plant parameters; **already cited in-repo**. Good for heat-balance sanity, not pointwise validation |
| **IAEA public SMR design descriptions** | Top-level public parameters for NuScale, SMR-300, SMART, CAREM and others. The policy-clean source for illustrative nominal values |
| **Open integral-PWR design literature** — IRIS in particular, plus SMART and CAREM | IRIS is an integral PWR with helical-coil once-through SGs and has a substantial open journal literature on both the integral layout and the helical SG thermal-hydraulics. This is the best open basis for the helical SG model |
| **Helical-coil tube heat transfer and pressure drop correlations** | A healthy open correlation literature exists for single-phase and flow-boiling in helically coiled tubes — enough to build a defensible closure without vendor data |

### Exists, but access is the question

- **A scaled natural-circulation integral-PWR facility with a helical-coil SG**
  was the subject of an IAEA international collaborative standard problem, and a
  describing publication exists. This is the closest open analogue to a
  NuScale-type iPWR. Confidence that the exercise and publication exist is high;
  confidence that the raw time-history data package is freely downloadable is
  **low**.
- **Classic integral-effects PWR facilities** — ROSA/LSTF, PKL, BETHSY, LOBI,
  Semiscale, LOFT. Facility descriptions and many result summaries are openly
  published; complete data packages generally sit behind project membership or
  data-bank terms.

### Largely proprietary — plan around it

- **NuScale.** The public regulatory docket contains a **redacted** safety
  analysis report with nominal design and transient-analysis chapters. The
  supporting topical reports — system-code models, helical-coil SG heat transfer
  and stability correlations, and the integral test facility data — are
  substantially proprietary. Treat NuScale as a source of *qualitative
  architecture and rough nominal parameters*, **not** a validation dataset.
- **Holtec SMR-300.** Much earlier in licensing; public material is largely design
  description. Do not expect usable validation data.

### Recommended validation ladder

Each rung has open data already in, or reachable from, this repository.

1. IF97 property tables at 100–220 bar — **in-repo, already done**
2. Critical-flow tests — **in-repo, already done**
3. Subchannel void and DNB at PWR pressure — specification public, tutorial case
   already vendored in-repo
4. CIET natural circulation as a loop-solver regression, acknowledging the oil and
   low-pressure caveat — **in-repo, already done**
5. A coupled main-steam-line-break benchmark for primary, secondary and kinetics
   together
6. Public SMR design parameters for the plant-scale steady state

## 4. Recommended approach

**Build on `htgr_sim_v1`'s structure, not `fhr_sim_v2`'s.** The HTGR example is
newer, cleaner, and each physics module opens with an explicit "what is real /
what is illustrative" contract. Its secondary loop is already a working closed
Rankine cycle with real IF97 flashes and passing tests — the nearest thing in the
workspace to what an iPWR secondary needs.

Note that `fhr_sim_v2` carries a known-broken parallel-branch flow solver, marked
in-file as not working, and a module whose filename openly flags its own
provenance. Prefer the HTGR lineage.

## 5. Proposed work breakdown

| Bead | Work | Depends on |
|---|---|---|
| `ipwr_sim_v1` | Parent — the existing `op-wqk.10` | — |
| Bounds-checked IF97 façade returning `Result` | Prevents transient overshoot killing the physics thread | — |
| Fixed-pressure water in `LiquidMaterial` | Via `CustomLiquid` backed by IF97 Region 1 | — |
| Natural-circulation primary loop | On TUAS fluid arrays | Fixed-pressure water |
| Pressuriser | Two-region bubble, spray, heaters, relief | IF97 façade |
| Helical-coil once-through SG | Coil correlations plus zone tracking | Fixed-pressure water; CHF/wall-boiling wiring |
| Wire CHF and wall-boiling closures to a consumer | Currently orphaned | — |
| SG inventory giving sliding steam pressure | Replaces the fixed-pressure assumption | Helical SG |
| Secondary cycle | Adapt the HTGR secondary loop | SG inventory |
| Boron reactivity and CVCS | New | — |
| PWR reactivity aggregator | Additive Doppler + moderator + rods + boron | Boron |
| Control loop set | Average-temperature program, pressuriser, SG level, governor | Aggregator, SG inventory |
| Widget fills | Condenser, SG, heat exchanger, pump | — |
| V&V ladder rungs 3, 5 and 6 | Methodology and measured results per the workspace V&V rule | All of the above |

## 6. Open questions for the maintainer

1. **Which design to draw** — NuScale, SMR-300, or a generic unbranded iPWR? Given
   that neither vendor's data is usable for validation, a generic integral PWR
   using public IAEA nominal parameters may be the more honest framing, and avoids
   implying vendor endorsement.
2. **Is the helical-coil SG in scope for v1?** It is the largest single item in
   this document. A straight-tube once-through SG would get a working twin much
   sooner, at the cost of geometric fidelity.
3. **Should the IF97 panic-to-`Result` façade be its own bead** covering the whole
   workspace? Every reactor in this slate that touches the steam side inherits the
   same hazard.
