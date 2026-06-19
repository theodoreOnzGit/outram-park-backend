# CLAUDE.md

Guidance for Claude Code (and other AI assistants) working in this crate.

> This crate is a member of the **OUTRAM PARK** workspace (`crates/teh-o-prke`).
> See the workspace root `CLAUDE.md` for the shared dependency policy and the
> full consolidation/migration history. Dependencies are inherited from the root
> `[workspace.dependencies]` — do not pin versions in this crate's `Cargo.toml`.

## What this is

**teh-o-prke** — the Point Reactor Kinetics Equations (PRKE) module for **Teh-O**
(the **T**ransport, **E**igenvalue and **H**ybrid **O**pen-source solver; named
after "teh-O", the Southeast-Asian tea). It models reactor point kinetics with
delayed-neutron precursor groups, reactivity feedback, and decay heat, using
`uom` dimensioned quantities throughout. License: GPL-3.0.

## Code layout (`src/`)

- `zero_power_prke/` — point kinetics core.
  - `six_group_precursor_prke/` — six delayed-neutron precursor groups.
    - `six_group_constants.rs` — the `FissioningNuclideType` enum and the
      `new_u233_/u235_/pu239_delayed_neutron_fraction_array()` constructors
      (per-nuclide delayed fractions). **These items live in this submodule**,
      not at the `six_group_precursor_prke` level (see migration note below).
- `feedback_mechanisms/` — incl. `fission_product_poisons/` (Xe/Sm).
- `fuel_temperature_feedback`, `control_rod_feedback`, `decay_heat`.
- `time_stepping/` — integrators (incl. OpenFOAM-derived source files).
- `teh_o_prke_error` — crate error enum (thiserror).

## Build, test, run

```bash
cargo test -p teh-o-prke                          # unit tests
cargo run -p teh-o-prke --example fhr_sim_v1 --release   # FHR educational GUI
```

Requires system OpenBLAS (see root CLAUDE.md).

## Conventions

- Public APIs take/return `uom` dimensioned quantities — no bare `f64` SI values
  at API boundaries.

## Migration notes (OUTRAM PARK consolidation, 2026-06)

Done while moving this crate into the workspace and bumping to latest deps:

- **`zero_power_prke/tests.rs` import fix** (pre-existing bug, unrelated to the
  dep bump): the tests imported `new_u235_delayed_neutron_fraction_array`,
  `new_u233_…`, `new_pu239_…`, and `FissioningNuclideType` from
  `six_group_precursor_prke`, but those items are defined one level down in
  `six_group_precursor_prke::six_group_constants` (the `use` in `mod.rs` is
  private, not `pub use`). Imports were repointed at the correct submodule.

- **`examples/fhr_sim_v1` egui 0.29 → 0.34 migration** (now compiles & links):
  - `app/mod.rs`: `impl eframe::App` — renamed `update(&mut self, ctx, frame)` to
    the now-required `ui(&mut self, ui, frame)`, deriving `let ctx = ui.ctx();`
    so the existing panel code is unchanged. (Panel `.show(ctx, …)` is now
    deprecated → `.show_inside(ui, …)`, but still compiles; left as-is.)
  - `app/graph_pages/mod.rs`: `egui_plot::Line::new(PlotPoints::from(v)).name(s)`
    → `Line::new(s, PlotPoints::from(v))` (name is now the first constructor arg).

Remaining warnings in the example (deprecated panel `.show`, dead-code fields,
lifetime-syntax lints) are non-blocking and were left untouched.
