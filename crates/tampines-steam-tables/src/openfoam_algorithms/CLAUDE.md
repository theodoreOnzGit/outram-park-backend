# CLAUDE.md — openfoam_algorithms module

## Purpose

This module houses a 1D HEM-closed rhoPimpleFoam solver (Rust port).
The sole public export from this module is `TampinesSteamArray`;
everything else is `pub(crate)` and never touches the public API of
`tampines-steam-tables`.

**Stability primer (read this if `step()` blows up):** the pressure–velocity
coupling failure modes for this solver — the pressure-source clobbering bug,
the boundary flux write-back, stiff-liquid water-hammer, pressure bounding,
and BC well-posedness — are explained step-by-step in
`crates/outram-foam-appbuilder-lib/src/solvers/rho_pimple_foam/docs/stability_a_students_guide.md`.
`TampinesSteamArray` shares this solver's design, so that guide applies here
directly. See also the V&V log
`verification_and_validation/pressure_bounding_vs_openfoam_pressurecontrol.md`
and bead `op-21g.12`.

## Numerical primitives already in-tree

The outram-foam-basic-lib primitives (matrix, PCG, DIC, GAMG, MUSCL, FV
operators, etc.) are already committed to `tampines-steam-tables` as
Rust source under `openfoam_algorithms/openfoam_source/`. Import from
the existing internal modules — do not attempt to fetch, copy, or
reorganise them. If a primitive is missing for the task, ask before
adding.

## Known port debt — `pub use` → `pub(crate) use`

The initial verbatim copy of outram-foam-basic-lib into this crate uses
`pub use` re-exports in several places. This was expedient during the
port but is incorrect for the intended module contract: none of the
copied OpenFOAM primitives should be part of `tampines-steam-tables`'s
public surface.

As part of your work in this module:

- Downgrade `pub use` → `pub(crate) use` on any copied outram-foam-basic-lib
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

## Mandatory architecture rules (outram-park-backend directives)

- No `Box<dyn Trait>` — use enums for dispatch, exhaustiveness-checked
- No `Box<T>` except recursive structures (which don't exist here)
- No lifetime parameters on structs, traits, or impls — own by value
  or use `Arc<T>`
- `Arc<RwLock<T>>` over channels for shared simulation state; no
  `mpsc` / `crossbeam` in timestep loops
- Every public API must be rust-analyzer-navigable
- All public APIs need mandatory doc comments answering: what physical
  quantity, valid ranges, units. Use named `uom` type aliases.
- All code must pass `cargo test`

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
  `Vec<Pressure>`, `Vec<Ratio>` for void fraction, `MassRate` break flow,
  plus any extras chosen by the physics lead

## Test contract (Edwards pipe test)

> **Location corrected 2026-08-11.** This section was written as a plan and
> named a location that was never used. There is no `edwards_pipe/` directory
> anywhere under `src/`, and nothing is registered in the
> `converging_diverging_nozzles/tests/` `mod.rs` for it.

**Actual location: `tampines-steam-tables/tests/edwards_blowdown.rs`** — an
integration-test target at the crate root, not an in-`src` unit-test module. It
holds two tests, `edwards_obrien_pipe_blowdown_600ms` and
`edwards_hybrid_damps_ringing_vs_pimple`, neither `#[ignore]`d.

**It is very slow.** Measured 2026-08-11: `edwards_obrien_pipe_blowdown_600ms`
alone **384.75 s**, and the whole target (both tests, run in parallel by cargo)
**393.58 s**. Run it as its own generously-timed invocation:

```bash
cargo test --release -p tampines-steam-tables --test edwards_blowdown
```

**A timeout is not a failure** — if the run is killed, report that it was killed,
not that a test failed. See the crate `CLAUDE.md` "Testing notes".

The original planned location, kept for the record, was
`.../converging_diverging_nozzles/tests/edwards_pipe/`, alongside
`marviken_tests.rs`, `zaloudek_*` and `moody_*`.

Output (printed to stdout via `println!`, viewable with
`cargo test -- --nocapture`):

1. SI time-series CSV block
   Header:
   `t_s, p_gs1_pa, p_gs2_pa, p_gs3_pa, p_gs4_pa, p_gs5_pa, p_gs6_pa, p_gs7_pa, alpha_gs5, mdot_break_kg_s`

2. T&A-native time-series CSV block
   Header:
   `t_s, p_gs1_psia, p_gs2_psia, p_gs3_psia, p_gs4_psia, p_gs5_psia, p_gs6_psia, p_gs7_psia, alpha_gs5, mdot_break_lbm_s`

3. Error summary CSV block
   Header: `station, rms_error, linf_error`
   One row per (gauge station, variable) tuple, computed vs. digitised
   T&A curves *and* vs. Edwards experimental data (two subsections,
   each with its own header row).

Reference data: hard-coded inline `const` arrays with provenance comments
naming the source figure and digitisation tool (graphreader.com).

Comparison timestamps: at points where the digitised reference data
actually has values. Interpolate simulation output onto reference
timestamps, not vice versa.

Assertions: sanity only (no NaN, all values finite). Do not hard-assert
tolerance — HEM is WIP; the point is to report error metrics for the
paper's V&V section.

## Judgment calls — use Opus

- Break BC path selection from TAMPINES three-solver dispatcher (in-dome,
  subcooled, superheated) at each timestep given local (p, h)
- Ingestion strategy for Hendrie (1973) axial enthalpy IC into the CV list
- HEM closure design decisions (ψ = ∂ρ/∂p handling across saturation dome,
  h-primary EEqn implementation)
- Rupture disc opening ramp (T&A stipulates 1.0 ms; modelling choice)
- Wall friction closure (defer to physics lead if unsure)

## Sonnet-appropriate work

- `TampinesSteamArray` struct scaffolding
- `ArraySnapshot` struct definition
- CSV `println!` formatting
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

## Primary references

- Tomlinson & Aumiller (1999), B-T-3271 — RELAP5-3D assessment
- Edwards & O'Brien (1970), J. British Nuclear Energy Society, 125–135
- Hendrie (1973), USAEC letter — axial enthalpy IC source
- Schmidt, Gopalakrishnan & Jasak (2010), Int J Multiphase Flow 36(4),
  284–292 — HRMFoam algorithmic template
- De Lorenzo et al. (2017a), Int J Multiphase Flow 95, 199–219 — HEM +
  tabulated IAPWS-IF97, closest architectural precedent for TAMPINES-as-thermo
