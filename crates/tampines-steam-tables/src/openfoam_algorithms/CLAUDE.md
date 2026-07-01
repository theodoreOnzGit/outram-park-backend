# CLAUDE.md — openfoam_algorithms module

## Purpose

This module houses a 1D HEM-closed rhoPimpleFoam solver (Rust port) plus
copied OpenFOAM basic-lib primitives it depends on. The sole public export
from this module is `TampinesSteamArray`; everything else is `pub(crate)`
and never touches the public API of `tampines-steam-tables`.

## One-time fork discipline

The OpenFOAM primitives inside this module are a one-time fork of
`openfoam-basic-lib`. No upstream sync obligation. See `PROVENANCE.md` in
this directory for the fork event, date, and scope.

## Mandatory architecture rules

- **No `Box<dyn Trait>`** — use enums for dispatch, exhaustiveness-checked
- **No `Box<T>`** except recursive structures (which don't exist here)
- **No lifetime parameters** on structs, traits, or impls — own by value
  or use `Arc<T>`
- **`Arc<RwLock<T>>` over channels** for shared simulation state; no
  `mpsc` / `crossbeam` in timestep loops
- Every public API must be rust-analyzer-navigable
- All public APIs need mandatory doc comments answering: what physical
  quantity, valid ranges, units. Use named `uom` type aliases.

## Copied OpenFOAM code

Every copied file gets:
- GPLv3 SPDX header
- Provenance comment naming the source file in `openfoam-basic-lib`
  (path + copy date)
- `pub(crate)` visibility only

## Public surface

Only export: `TampinesSteamArray` and its associated snapshot types.

## `TampinesSteamArray` contract

Construction:
- Generic constructor: `TampinesSteamArray::new(cells, break_bc)`
- Preset: `TampinesSteamArray::edwards_obrien_24cv()` — T&A modified
  nodalisation, Hendrie (1973) axial enthalpy IC

Time stepping:
- `array.step(dt: Time)` — single timestep, caller drives loop
- `array.run(t_final: Time, dt: Time, sample_every: Time) -> Vec<ArraySnapshot>`

Observables:
- `array.snapshot() -> ArraySnapshot` returning full field state —
  `Vec<Pressure>`, `Vec<Ratio>` for void, `MassRate` break flow, plus any
  extras chosen by the physics lead

## Test contract (Edwards pipe test)

Location: `tampines-steam-tables/src/steam_turbine_equations/converging_diverging_nozzle/choked_flow/tests/edwards_pipe/`

Output (printed to stdout via `println!`, viewable with
`cargo test -- --nocapture`):

1. **SI time-series CSV block**
   Header: `t_s, p_gs1_pa, p_gs2_pa, p_gs3_pa, p_gs4_pa, p_gs5_pa, p_gs6_pa, p_gs7_pa, alpha_gs5, mdot_break_kg_s`

2. **T&A-native time-series CSV block**
   Header: `t_s, p_gs1_psia, p_gs2_psia, p_gs3_psia, p_gs4_psia, p_gs5_psia, p_gs6_psia, p_gs7_psia, alpha_gs5, mdot_break_lbm_s`

3. **Error summary CSV block**
   Header: `station, rms_error, linf_error`
   One row per (gauge station, variable) tuple, computed vs. digitised
   T&A curves *and* vs. Edwards experimental data (two subsections).

Reference data: hard-coded inline `const` arrays with provenance comments
naming the source figure and digitisation tool (graphreader.com).

Comparison timestamps: at points where the digitised reference data
actually has values. Interpolate simulation output onto reference
timestamps, not vice versa.

Assertions: sanity only (no NaN, all values finite). Do not hard-assert
tolerance — HEM is WIP; the point is to report error metrics for the
paper's V&V section.

## Judgment calls delegated to Opus

- Break BC path selection from TAMPINES three-solver dispatcher (in-dome,
  subcooled, superheated) at each timestep given local (p, h)
- Ingestion strategy for Hendrie (1973) axial enthalpy IC into the CV list
- Any HEM closure design decisions (ψ handling across saturation dome,
  h-primary vs. T-primary EEqn — h-primary is required, but implementation
  is your call)
- Rupture disc opening ramp (T&A stipulates 1.0 ms; how to model is your call)

## Sonnet-appropriate work

- `TampinesSteamArray` struct scaffolding
- `ArraySnapshot` struct definition
- CSV `println!` formatting
- Provenance comment headers
- Test bootstrap boilerplate
- Copy-paste of digitised reference arrays
- Downgrading `pub use` → `pub(crate) use` on primitives you touch

## Physics assumptions

- Adiabatic pipe interior (T&A removed heat structures for short transient)
- Break area 87% of pipe cross-section (T&A stipulation)
- Break location at end of Volume 24 (T&A nomenclature)
- Semi-implicit time integration
- dt_max ≤ 0.1 ms (T&A stipulation)

## Test always passes unless

- NaN or non-finite value appears in any snapshot field
- Test panics from unwrap or assertion in solver internals

## Known port debt — `pub use` → `pub(crate) use`

The initial verbatim copy of openfoam-basic-lib into this crate uses
`pub use` re-exports in several places. This was expedient during the
port but is incorrect for the intended module contract: none of the
copied OpenFOAM primitives should be part of `tampines-steam-tables`'s
public surface.

As part of your work in this module:

- Downgrade `pub use` → `pub(crate) use` on any copied openfoam-basic-lib
  re-export you touch or depend on
- Do not add any new `pub use` re-exports from the copied primitives
- If `cargo build` breaks because an external consumer was relying on a
  `pub use` re-export, stop and ask — that's a signal the primitive was
  leaking into the public API and needs a proper wrapper, not a
  visibility downgrade
- The only symbols that should be `pub` from this module are
  `TampinesSteamArray` and its associated snapshot types

This is incremental cleanup — fix what you touch, don't do a
crate-wide sweep. The intent is to prevent the debt from compounding,
not to block Edwards work on housekeeping.

