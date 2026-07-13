# OUTRAM PARK Digital Twin Plan (2026-07-13)

Approved plan summary for: extending `OPCPFluidArray` (coolprop), scaffolding
the new `tampines` crate, and scaffolding the new `outram-park-digital-twin-gui`
crate. Full planning-tool record: `/home/teddy0/.claude/plans/glimmering-pondering-platypus.md`.

**Beads tracking** (converted 2026-07-13, per the new standing rule in the
root `CLAUDE.md`): Part 1 → `op-kbc.18` (child of the `outram-park-fork-coolprop`
epic `op-kbc`); Part 2 → epic `op-dt3` (`tampines`); Part 3 → epic `op-wqk`
(`outram-park-digital-twin-gui`). Run `bd show op-dt3` / `bd show op-wqk` /
`bd dep tree op-dt3` for live status — this file is a point-in-time summary,
beads is the source of truth for progress.

## Context

OUTRAM PARK is organized around reusable platforms, with reactor simulators
as example applications built on top of them, not foundational libraries
themselves. `outram-park-backend` (this repo) holds reusable frameworks only
and is meant to be a git submodule inside a separate, not-yet-existing parent
repo `outram-park` that will hold the actual simulator *applications*,
lessons, papers, etc.

- **TAMPINES** (new crate `tampines`, distinct from `tampines-steam-tables`)
  — the central thermal-hydraulic framework. Owns all fluid flow, TH,
  thermophysical properties, heat transfer, balance of plant, humid-air
  psychrometrics, multiphase TH. Depends on `tuas_boussinesq_solver`,
  `outram-park-fork-coolprop`, `tampines-steam-tables`, `outram-foam-basic-lib`,
  `chem-eng-real-time-process-control-simulator`.
- **Outram Park Digital Twin GUI** (new crate `outram-park-digital-twin-gui`)
  — the reusable visualization framework. Depends on `tampines`, `nee_soon`
  (reactor-vessel/instrumentation), `chem-eng-real-time-process-control-simulator`.
  Provides visual process objects (Pipe, Pump, Valve, HeatExchanger,
  SteamGenerator, Turbine, Condenser, CoolingTower, ReactorVessel,
  Instrumentation) whose rendering derives directly from physics state (cell
  count -> displayed cells, temperature -> cell color, mass flow -> tracer
  direction, residence time -> tracer travel time). Philosophy: "avoid
  separating physics and rendering unnecessarily" — components bundle physics
  state + visual representation + animation logic together.

**Development-time exception, confirmed by the user**: the example
applications `htgr_sim_v1`/`fhr_sim_v2` — architecturally destined for the
outer `outram-park` repo's `simulators/` directory — will for now live inside
`outram-park-backend`, in `tampines/examples/`, since the outer repo doesn't
exist yet. `fhr_sim_v2` = move + adapt the existing working GUI example out of
`tampines-steam-tables`. `htgr_sim_v1` = a thin stub (no HTGR simulator exists
yet anywhere, and its `NEE_SOON` dependency isn't ready).

## Part 1 — Extend `OPCPFluidArray` (bead `op-kbc.18`)

`OPCPFluidArray` (`crates/outram-park-fork-coolprop/src/openfoam_algorithms/rhoPimpleFoam/mod.rs`)
gains: lateral thermal coupling (`Vec<Vec<ThermodynamicTemperature>>` /
`Vec<Vec<ThermalConductance>>`, caller supplies conductance directly — no
`NusseltCorrelation` port), a volumetric heat source (`q_vector`/
`q_fraction_vector`), pipe geometry (`wetted_perimeter`, `incline_angle`), and
flow bookkeeping (`mass_flowrate`, `pressure_loss`, `internal_pressure_source`,
plain independent get/set — no `DimensionlessDarcyLossCorrelations` port).
New error type `OPCPFluidArrayError::LengthMismatch`, re-exported from
`src/lib.rs`. New file `rhoPimpleFoam/lateral_coupling.rs` holds the error
type, the new `impl OPCPFluidArray` block (13 methods), and 7 new tests.

Status: struct fields added; `new()` initializer and `lateral_coupling.rs`
still outstanding (see `bd show op-kbc.18`).

## Part 2 — `tampines` crate (epic `op-dt3`)

New crate, workspace member, headless library (no GUI deps except as
`examples/`-only dev-deps, per the Android-portability rule). Module
structure: `fluids/`, `single_phase/` (wraps TUAS `FluidArray`),
`compressible/` (wraps `OPCPFluidArray`, depends on Part 1), `hem/` +
`critical_flow/` (thin wrappers over `tampines-steam-tables`), `heat_transfer/`,
`humid_air/`, `components/` (8 BOP component types — `Pipe` enum-dispatches
`PipeBackend::Lumped(tuas FluidArray)` / `Compressible(coolprop OPCPFluidArray)`,
real structs + stub methods returning `TampinesError::NotYetImplemented`),
`balance_of_plant/`, `cooling_tower/`, and `hem/future_multiphase/` (CHF,
dryout, drift flux, two-fluid, six-equation TH — trait/doc stubs only, no
implementation). `examples/fhr_sim_v2` (moved from `tampines-steam-tables`)
and `examples/htgr_sim_v1` (thin stub) land last, after Part 3 exists.

Bead children: `op-dt3.1`–`op-dt3.9` (`bd show op-dt3` for the live list).

## Part 3 — `outram-park-digital-twin-gui` crate (epic `op-wqk`)

New crate, workspace member, GUI deps are real dependencies (presentation
layer, no Android-portability claim). `color_maps/` (real, ported hot/cold +
quality colour functions), `components/` (10 visual process object wrappers,
each composing a `tampines::components::X` + visual-only fields),
`animation/` (tracer/travel-time — genuinely new design, trait stubs only),
`app_scaffold/` (real, ported `Arc<Mutex<_>>` physics-thread + panel-dispatch
pattern from the existing GUI examples).

Bead children: `op-wqk.1`–`op-wqk.6` (`bd show op-wqk` for the live list).

## Overall execution order

1. Part 1 (`OPCPFluidArray`) — fully independent, no new crates, do first.
2. Part 2's library modules (`fluids`/`single_phase`/.../`components`) —
   independent of Part 3.
3. Part 3 (`outram-park-digital-twin-gui`) fully.
4. Part 2's `examples/` (the `fhr_sim_v2` move + `htgr_sim_v1` stub) last, so
   they can use Part 3's `app_scaffold`/components rather than being moved
   twice — enforced in beads via `op-dt3.7`/`op-dt3.8` depending on `op-wqk.6`.
5. Full workspace build/test after each part, not just at the end.

Per the user's standing collaboration preference, check in between parts
rather than proceeding through all three without confirmation.
