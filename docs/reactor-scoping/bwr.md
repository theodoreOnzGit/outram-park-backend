# BWR — natural-circulation boiling water SMR (BWRX-300 architecture)

Scoping document for an offline digital-twin simulator of a natural-circulation
BWR, built in `crates/outram-park-digital-twin-engine` with its direct-cycle
secondary side.

> **Intended use.** Education, research, capability building, and V&V only. This
> is an offline demonstration with no connection to any operational system. See
> `RESPONSIBLE_USE.md`.
>
> **Status of this document.** Capability findings come from a codebase audit
> performed 2026-08-06. Validation source identifiers are **deliberately
> unverified** — see [Open validation data](#open-validation-data).

Corresponds to the existing bead `op-wqk.11`, currently P4 and described as
future work.

## 1. Verdict up front

A credible BWR twin is **not** blocked on missing two-phase physics. The
workspace has an unusually rich set of two-phase *closures* — drift-flux slip,
RPI wall boiling, four CHF correlations, boiling regime maps, interfacial area,
phase-change rate, homogeneous-equilibrium steam tables — plus a real
one-dimensional drift-flux marcher with gravity and friction, and a real
three-dimensional nodal neutronics code carrying a **committed BWR benchmark
case**.

What does not exist is the **integration**: a heated boiling channel, a steam
separator, loop closure for natural circulation, and void reactivity wired into
the kinetics path.

This reframes the bead. The two-phase beads marked in progress delivered real,
tested closures — 67 passing tests — not empty frameworks. The gap is
differently shaped than the bead titles suggest.

## 2. Plant configuration

| Loop | Fluid | Purpose |
|---|---|---|
| Primary | Boiling water, approx. 7 MPa | Natural circulation: heated core, chimney, steam separator, downcomer. No recirculation pumps |
| Secondary | Steam / water | **Direct cycle** — steam goes straight from the vessel to the turbine, then condenser and feedwater |

The chimney and downcomer topology is what generates the natural-circulation
driving head. Carryunder — void carried down into the downcomer by an imperfect
separator — directly reduces that head, and is the term that makes the loop
close correctly.

## 3. Capability audit

Audited 2026-08-06 against the workspace at commit `ebbde1b`.
Verified this session: `outram-foam-multiphase` 67 tests pass.

### HAVE

| Capability | Where | Notes |
|---|---|---|
| **Drift-flux closures** — mixture properties, Zuber-Findlay algebraic slip, void transport | `crates/outram-foam-multiphase/src/drift_flux.rs` | Real, 10 tests |
| **Euler-Euler two-fluid** with drag models and saturation constraint | `crates/outram-foam-multiphase/src/two_fluid.rs` | Real, 15 tests |
| Mixture and two-fluid pressure-correction solvers | `.../pimple.rs`, `.../two_fluid_pimple.rs` | Real, 10 tests between them |
| **RPI wall boiling** — heat-flux partitioning into convection, quenching and evaporation, with site density, departure diameter and frequency | `crates/outram-foam-multiphase/src/wall_boiling.rs` | Real, 7 tests |
| **Four CHF correlations** — Biasi, W-3, Bowring, plus a lookup-table framework | `crates/outram-foam-multiphase/src/chf.rs` | Real, 15 tests, unit-typed with validity ranges |
| Dryout onset and post-dryout film boiling | `crates/outram-foam-multiphase/src/dryout.rs` | Two worked closures, 6 tests |
| **1-D four-equation drift-flux marcher** with axial gravity, wall friction, drift momentum flux and vapour relaxation | `crates/tampines/src/multiphase_1d/drift_flux.rs` | **The closest thing to a BWR channel solver in the repo.** Missing only wall heat and loop closure |
| **3-D two-group nodal diffusion coupled to channel TH** | `crates/bedok` | Boiling channel with void closure, coolant-density cross-section feedback, transient solver with six delayed groups, W-3 departure-from-nucleate-boiling ratio, fuel-rod conduction |
| **A committed BWR benchmark case with data on disk** | `crates/bedok/src/reference/cases/neacrp_d1/` | Three-dimensional LWR core transient benchmark, BWR cold-water injection. Geometry and composition data committed. Its own doc states the void feedback is the physics of interest |
| **Real void reactivity term** | `crates/outram-foam-appbuilder-lib/.../multi_region/reactivity_feedback.rs:71` | Density feedback, importance-weighted over mesh cells, documented as coolant density / void feedback |
| Genfoam two-phase closures — phase change, boiling regime map, CHF, interfacial area and drag, bubble-induced turbulence | `crates/outram-foam-appbuilder-lib/src/genfoam/thermal_hydraulics/closures/` | Substantial and tested. **The module docs understate this badly** — see below |
| Three-dimensional two-phase solvers with energy and phase change | `.../solvers/reacting_two_phase_euler_foam/`, `.../solvers/hrm_foam/` | Real, verification-tested |
| **Validated two-phase choked flow** | `crates/tampines-steam-tables/.../converging_diverging_nozzles/` | Against Moody, Zaloudek and Marviken. The only genuinely *validated* two-phase physics in the workspace. Directly reusable for main-steam-isolation and relief-valve flow |
| **Working direct-cycle secondary, roughly 70% written** | `examples/htgr_sim_v1/physics/secondary_loop.rs` | Real isentropic expansion, exhaust quality flash, condenser balance, feed pump work, lagged feedwater control. For a BWR you delete the steam generator and feed core steam straight in |
| Turbine widget with real rotation, and generator torque balance | `crates/outram-park-digital-twin-engine/src/components/turbine.rs`, `crates/tampines-steam-tables/.../generator.rs` | Rotor spins at real angular velocity |
| Pipe widget with **phase shading** | `crates/outram-park-digital-twin-engine/src/components/pipe.rs:127` | Already distinguishes liquid, two-phase and gas |

### SCAFFOLD — do not count as working

- **The genfoam two-phase driver has zero lines written.** The thermal-hydraulics
  solver enum has exactly one variant, single-phase, and
  `crates/outram-foam-appbuilder-lib/.../solver/mod.rs:80` carries the to-do
  naming bead `op-p6p.7.12`. The closures it would wire **are** written; the
  wiring is absent. That bead is marked in progress and assigned.
- **`crates/tampines/src/multiphase_1d/two_fluid.rs` is a documented scaffold**
  computing nothing — 139 lines, returns not-implemented. Its own docs correctly
  flag the reason it is hard: the naive six-equation system is ill-posed and the
  regularisation choice changes the answer.
- In `outram-foam-multiphase`, the CHF, subcooled-CHF, dryout and film-boiling
  **regime arms of wall boiling** return not-implemented. Only nucleate boiling
  works — and the CHF correlations in the same crate are **not wired into** them.
- Void bounding is a plain clamp, not a conservative limiter. No energy equation,
  no wall-heat coupling, no phase-change source in that crate; densities constant.
- **Doc/reality drift, both directions.** The genfoam closures module claims only
  drag is real when most of it is implemented; anyone auditing by reading module
  docs will materially under-count what exists. Conversely the two-phase solver
  table overstates.
- **`crates/bedok` has never been run against its benchmark.** Every benchmark
  gate is ignored, no parity run has been done, and its README makes no parity
  claim. Its six-equation channel kernel is missing from the upstream snapshot
  and was never written. Fifty-seven known upstream defects are deliberately
  preserved unrepaired.
- `crates/tampines` component wrappers — turbine and condenser — return
  not-implemented, so the turbine widget renders a **stationary** rotor in that
  mode and the condenser draws a fixed colour.

### The load-bearing sharp edge

`crates/tampines-steam-tables/.../rhoPimpleFoam/lateral_coupling.rs:414` documents
it precisely: near saturation, the `(p,h)` region classification and the
conductivity path's internal `(T,p)` re-classification can disagree, reaching a
`todo!()` in the two-phase region. Its own comment says a real boiling steam
generator tube passes through that boundary, so it must be addressed first.

**A BWR core channel crosses that boundary on every cell, every step.** This is
the crux of "how mature is the two-phase path": the `(p,h)` flash carries phase
data fine; the `(p,T)` path panics in the dome; and the compressible steam array
mixes both.

Separately, that solver's inclination angle is **stored but never read** — it is
bookkeeping only. There is no gravity or buoyancy term in it at all.

### MISSING — sized

**Small**

1. **Void reactivity in the kinetics path.** The prompt excursion timestepper
   carries exactly one feedback, fuel temperature, and its constructor *requires*
   it to be negative. There is no moderator-density term in the layer both engine
   examples are built on. The six-factor feedback hook has the right shape but is
   an empty function pointer with no built-in coefficient.
2. Finish the turbine and condenser component wrappers — the algebra already
   exists in the HTGR example. Unblocks the rotor and the condenser colouring.
3. Wire the existing CHF correlations into the wall-boiling regime arms. Both
   halves live in the same crate and do not talk.
4. Direct-cycle plumbing: replace the steam-generator node with the core steam
   dome; add isolation and stop valves using the validated choked-flow path.
5. Resolve the two-phase `(p,T)` reachability, or forbid that path in two-phase
   cells.

**Medium — the critical path**

6. **Heated boiling channel.** Add wall heat with regime-selected heat transfer
   to the one-dimensional drift-flux marcher. Every closure needed already
   exists across `outram-foam-multiphase` and the genfoam closures. This is
   assembly, not invention.
7. **Loop closure for natural circulation.** Chain heated core, chimney,
   separator and downcomer into a closed circuit and solve for circulation flow
   from driving head against total loss. The CIET loop-closure and
   parallel-branch machinery is the structural precedent; the two-phase
   pressure-drop side is new.
8. **Steam separator, dryer and steam-dome inventory.** Nothing exists — a
   workspace-wide grep for separator or dryer returns only GUI calls. Even a
   lumped model needs carryover, **carryunder**, separator pressure drop, and a
   vessel inventory that lets dome pressure slide.
9. **Two-phase pressure drop.** No two-phase multiplier exists anywhere; the
   drift-flux marcher uses single-phase friction on mixture properties.
10. Run `crates/bedok`'s benchmark case end to end. There is currently zero
    evidence it executes.

**Large**

11. Coupled three-dimensional neutronics and boiling TH with axially shaped void
    feedback. A core-average void coefficient is a demo, not a twin.
12. Stability and density-wave capability — BWR-specific and the hardest thing to
    get right. Do not promise it until the two-fluid solver exists.
13. Plant model and balance-of-plant transients: turbine trip, isolation-valve
    closure, feedwater loss, passive cooling.

## 4. Open validation data

> **No report identifiers, benchmark numbers, or measured values are asserted
> here.** They must be obtained from the actual documents.

### Already in this repository — highest confidence, it is on disk

- **The three-dimensional LWR core transient benchmark, BWR cold-water injection
  case.** Geometry and composition data are committed in `crates/bedok`. Two
  group, with fuel-temperature and coolant-density feedback; the transient is a
  step in inlet subcooling. **This is the single best immediately actionable
  coupled BWR void-feedback validation case available.** It exercises void
  reactivity but not the secondary side.
- Critical-flow data, already reproduced as passing tests.
- A blowdown pipe case, currently verification-only.

### Coupled primary-and-secondary BWR benchmarks — high confidence they exist

- **A turbine-trip benchmark based on a real BWR**, published through OECD/NEA
  with the US regulator and a university partner, structured as three exercises:
  plant system thermal-hydraulics with fixed power, three-dimensional neutronics
  with fixed thermal-hydraulics, and the full coupled problem. **This is exactly
  the primary-plus-secondary coupled transient wanted** — stop-valve closure
  sends a pressure wave up the steam line, void collapses, power spikes, then
  scram and void feedback. Confidence high that it exists and the specification
  is publicly available; some cross-section libraries are distributed under
  registration rather than fully open.
- **A BWR stability benchmark** with in-phase and out-of-phase power oscillation
  measurements. High confidence; the classic open stability case.

### Void-fraction and dryout data — what the closures actually need

- **A full-size BWR bundle test programme** with X-ray-measured void
  distributions, critical power and pressure drop, steady and transient. High
  confidence this is *the* reference dataset for BWR void and dryout models.
  **Caveat: distributed via a data bank with registration conditions — treat as
  publicly documented but conditionally distributed, not open download.**
- **Classic open subcooled void profile datasets** — the standard validation
  targets for RPI wall-boiling models. Modest range but genuinely open.
- A published **CHF lookup table** — the framework in the CHF module already
  expects it.

### BWRX-300 specifically — lowest confidence, handle carefully

Public design information is real but thin and mostly qualitative. Licensing
topical reports filed with the US and Canadian regulators are publicly
retrievable, and a public SMR design-description entry likely carries top-level
parameters.

**What you will almost certainly not find publicly:** channel geometry, void
coefficient against exposure, axial power shapes, chimney and separator
dimensions, or a validated plant transient dataset.

**Best public proxy: the ESBWR.** Its design control document is public and
describes a natural-circulation BWR with a chimney — the same architectural
family and the direct ancestor of this flow path. For a driving-head model this
is the most useful open document class.

### Recommendation

Follow the precedent this workspace already set in `htgr_sim_v1`: build on
**round, illustrative, order-of-magnitude parameters explicitly declared as not
representing any specific licensed design**, validate the *physics* against the
open benchmarks, and treat BWRX-300 as the **architectural** target — natural
circulation, chimney, no recirculation pumps, direct cycle — rather than a
numeric one.

## 5. Proposed work breakdown

Suggested sequencing to unpark `op-wqk.11`:

| Order | Work | Size |
|---|---|---|
| 1 | Void coefficient in the kinetics path | Small |
| 2 | Wall heat into the drift-flux marcher using the existing closures | Medium |
| 3 | Lumped separator with carryunder plus steam-dome inventory | Medium — must be written from scratch |
| 4 | Loop closure for natural circulation | Medium |
| 5 | Direct-cycle secondary lifted from the HTGR example | Small |
| 6 | Two-phase pressure-drop multiplier | Small–Medium |
| 7 | Wire CHF correlations into the wall-boiling regime arms | Small |
| 8 | V&V against the committed benchmark case already on disk | — |
| 9 | V&V against bundle void distributions | — |
| 10 | V&V against the turbine-trip benchmark for the coupled claim | — |

## 6. Open questions for the maintainer

1. **Is `crates/bedok` the intended neutronics path for this sim?** It carries
   the committed BWR benchmark and real void feedback, but has never been run
   against it and preserves fifty-seven upstream defects deliberately. Using it
   means confronting that first.
2. **Should the stale genfoam closure module docs be corrected?** They
   under-report what exists, which actively misleads audits.
3. **Does `op-p6p.7.12` need re-scoping?** It is marked in progress and assigned,
   but zero lines of the two-phase driver are written.
