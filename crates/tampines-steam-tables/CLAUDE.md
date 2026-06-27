# CLAUDE.md

Guidance for Claude Code (and other AI assistants) working in this repository.

## Project overview

TAMPINES Steam Tables is an in-house Rust implementation of the IAPWS-IF97
steam/water property formulation for the **T**hermo-hydraulic **A**rtificial
intelligence **M**ulti-**P**hase **IN**tegrated **E**mulator **S**ystem
(TAMPINES) solver. Unlike the upstream [rust-steam](https://github.com/marciorvneto/rusteam)
library it draws from, this crate uses **dimensioned units** throughout via the
`uom` crate, and incorporates verification tests against the International Steam
Tables (Kretzschmar & Wagner, 2019).

It also provides steam-turbine and converging-diverging nozzle equations,
including choked (critical) two-phase flow, and powers the secondary loop of an
FHR (Fluoride salt-cooled High-temperature Reactor) educational simulator.

License: GPL-3.0 (OpenFOAM-derived algorithms are included; see README).

## Build, test, run

**Rule: always use `--release` for builds and tests.** Never run in debug mode.

```bash
cargo build --release                           # build the library
cargo test --release                            # run all unit/verification tests (~144 test fns)
cargo test --release <name>                     # run a subset by substring match
cargo run --release --example fhr_sim_v2       # FHR educational simulator
```

On Linux, `ndarray-linalg` uses the system OpenBLAS, so you need:

```bash
sudo apt install libopenblas-dev
```

Windows/macOS targets use the static Intel MKL feature instead (see `Cargo.toml`).

## Code layout

Properties are organised by IAPWS-IF97 region under `src/`:

- `region_1_subcooled_liquid/` — region 1 (subcooled liquid)
- `region_2_vapour/` — region 2 (vapour, incl. metastable subregion)
- `region_3_single_phase_plus_supercritical_steam/` — region 3 + supercritical
- `region_4_vap_liq_equilibrium/` — region 4 (saturation line / VLE)
- `region_5_steam_at_800_plus_degc/` — region 5 (ultra-high-temp steam)

Forward equations are `(p,T)` / `(v,T)` flashes. Backward (inverse) equations
live in `backward_eqn_ph_*`, `backward_eqn_ps_*`, `backward_eqn_hs_*`.

Transport and misc properties: `dynamic_viscosity/`, `thermal_conductivity/`,
`surface_tension/`, `dielectric_constant/`.

User-facing entry points are in `interfaces/` — both a functional-programming
API (`(p,T)`, `(p,h)`, `(p,s)`, `(h,s)` flashes) and an object-oriented
`TampinesSteamTableCV` control-volume wrapper. The region-dispatch logic mostly
lives here.

`steam_turbine_equations/` holds nozzle and turbine equations, including the
choked-flow work (see below). `openfoam_algorithms/` contains reference
OpenFOAM solver ports (rhoPimpleFoam, driftFluxFoam, etc.) intended for future
transient two-phase coupling.

## Choked flow (current focus)

Multiphase critical-flow solvers (Homogeneous Equilibrium Model) live in
`src/steam_turbine_equations/converging_diverging_nozzles/choked_flow/`,
validated against Moody (1975), Zaloudek HEM reference curves, and Marviken.
The three split solvers (in-dome / subcooled / superheated-vapour) cover all
stagnation buckets relative to the p-h VLE dome.

The one unresolved case is the **near-bubble-point HEM artifact** (x_t ≈ 0):
the active failing test `outside_dome_stagnation_subcooled::quality_bubble_point_subcooled`
reflects a fundamental HEM limitation at the saturated-liquid line — an HRM
relaxation model is required. **Zaloudek curves are HEM-computed, not
experimental** (digitised from Saha 1978).

Full status, the three-failure-mode analysis, per-solver validation detail,
and the v0.2.0 roadmap are in **`docs/notes.md`**.

## Conventions

- All public property functions take and return `uom` dimensioned quantities —
  do not introduce bare `f64` SI values at API boundaries.
- Match the existing per-region module structure when adding equations
  (`dimensionless_*`, `gamma_*` / `phi_*` derivatives, `intensive_properties.rs`).
- Add a verification test against steam-table or published reference data for any
  new property or flash path; existing tests document expected accuracy bounds.
- The README `# Changelog` is the project's running history — add an entry there
  when bumping the version in `Cargo.toml`.
- Run `cargo fmt` and `cargo clippy -- -D warnings` clean before merge.

### Guardrails — do not violate without explicit human sign-off

- **Never strip `uom`** from public signatures for "simplicity". The type-level
  unit checking is the project's main safety net.
- **Never loosen tolerances** in verification tests to make a test pass. If a
  test fails, the equation or the boundary detection is wrong, not the tolerance.
- **Never paper over `NonConvergent`** with a default value. Propagate the error.
- **Respect region boundaries.** Don't call R2 equations on R1 inputs — the
  polynomial extrapolations diverge fast.
- **Prefer adding a new module** over editing `region_*/` files; the forward
  equations are line-for-line traceable to IAPWS tables and diffs against them
  must stay reviewable.
- **When in doubt, write the verification test first.** The IAPWS reference
  tables are the spec.

## Known accuracy pitfalls

- **Critical point (T ≈ 647.096 K, p ≈ 22.064 MPa):** Region 3 backward
  equations lose digits within ~0.5 K of Tc; expect deviations larger than IF97
  stated tolerances near Tc. Prefer `(ρ,T)` forward calls here.
- **`(h,s)` flash:** valid only in the IF97-defined hs envelopes. Outside those,
  the iterative fallback can stall near the two-phase dome; check
  `Result::Err(NonConvergent)`.
- **Low pressure (p < 611.657 Pa, triple-point pressure):** R1/R2 equations are
  extrapolated and not validated below the triple point.
- **R5 boundary:** results above 2273 K are extrapolations, not IF97. The
  library returns `OutOfRange` by default.
- **Transport near saturation:** the IAPWS R12-08 / R15-11 critical-enhancement
  terms for μ and λ are intentionally omitted in the fast path; enable them
  when accuracy very close to Tc matters.

## Workspace notes (read on demand)

Member of the **OUTRAM PARK** workspace (`crates/tampines-steam-tables`).
Dependencies are inherited from the root `[workspace.dependencies]` — **do not
pin versions here** (`uom.workspace = true`, etc.). `tuas_boussinesq_solver`
resolves to the in-tree path crate.

See **`docs/notes.md`** for the 2026-06 migration log, the planned removal of
the vestigial `ndarray-linalg` dep, and the known `fhr_sim_v2` UI
state-update bug.
