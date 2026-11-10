# AGENTS.md

Guidance for Claude Code (and other AI assistants) working in this crate.

> This crate is a member of the **OUTRAM PARK** workspace (`crates/teh-o-prke`).
> See the workspace root `CLAUDE.md` for the shared dependency policy and the
> full consolidation/migration history. Dependencies are inherited from the root
> `[workspace.dependencies]` — do not pin versions in this crate's `Cargo.toml`.

## Maturity: DECLARED MATURE (2026-09-05)

The API-usability rules in the root `CLAUDE.md` ("Human interface layer",
and the Haiku dogfooding hard rule) **are in force for this crate**. See the
maturity gate in that file for what this means and how the bar is revised.

- **2026-09-05 — mature.** Bar: published delayed-neutron data reproduced
  exactly (U-235 five-group total β = 0.0065; U-233 and Pu-239 group sets
  likewise), PRKE limiting cases exact (zero reactivity holds power constant,
  cold state produces no decay heat, precursor update is O(1) per step), and
  the LU solver verified against hand-computed 2×2 and 3×3 systems including
  pivoting and singular detection. Evidence class: **unit tests and internal
  consistency**, with literature constants as the reference. **29 tests pass**.

  **Note 2026-11-10 (gh:#1319):** "U-233 and Pu-239 group sets likewise"
  holds for the delayed fractions `beta_i` only. The decay constants
  `lambda_i` are one shared U-235 set for all three nuclides
  (`six_group_constants.rs`), and the source of that set and of the
  `beta_i` is not identified. Open, awaiting a source decision on the issue.

  **This is the thinnest declaration in the workspace, and is deliberately
  recorded as such.** ~~There is no analytical transient validation yet — no
  step-reactivity insertion checked against the exact in-hour solution, and no
  Nordheim-Fuchs excursion checked against its closed form.~~ **CORRECTED
  2026-11-01 (gh:#1082):** both now exist, in
  `zero_power_prke::n_group_prke` (in-hour, six groups and one group: 3.8e-6
  to 7.0e-5 relative in `omega`) and `point_reactor` (prompt-only
  Nordheim-Fuchs: peak 1.7e-4, energy 8.3e-5), recorded in
  `verification_and_validation/prke_analytic_checks.md`. The maturity bar has
  not been restated by the maintainer; that is theirs to do.

### Known API defect, found on declaration day

The time-stepping method is named:

```
solve_next_timestep_precursor_concentration_and_neutron_pop_vector_implicit
```

That is **76 characters**, and it is the single most important call in the
crate — advancing the PRKE state is the whole point of the library. The
explicit variant is the same name ending `_explicit`.

This is exactly the class of defect the Haiku dogfooding rule exists to catch,
and it was found by writing the prelude's own doc example: the call does not
fit on a line, so the example wraps it awkwardly, which is the visible symptom.

~~**Not fixed yet, because renaming is a breaking change**~~ **CORRECTED
2026-11-01:** half done. `step_implicit` / `step_explicit` exist as short
forwarders (`six_group_precursor_prke/mod.rs`, gh #58), and the N-group
`PointKinetics` uses only the short names. The long names are not yet
`#[deprecated]`, because this crate is
consumed by the `outram-park` Python bindings and their generated stubs. The
remaining fix: mark the long names `#[deprecated]` for one release
and regenerate the bindings. Recorded here rather than silently left, so the
next person does not have to rediscover it.


## Note: `pki/` is a dummy key, not a secret (do not flag as a security issue)

The `pki/` directory (`pki/own/`, `pki/private/`) is a **throwaway dummy key**
left over from early experimentation with the tooling — it is **not** a real
credential and is **not** a security concern. It is **untracked by git** (so it
is never committed or pushed) and is also `exclude`d from the packaged crate
(`exclude = ["pki", "docs"]`). Confirmed by the maintainer (2026-07-16).

Automated audits / secret scanners may surface it — **do not** treat it as a
leaked key, do not "rotate" it, and do not open a security issue for it. If tidying
is ever wanted, it can simply be deleted (it is untracked local scratch).

## What this is

**teh-o-prke** — the Point Reactor Kinetics Equations (PRKE) module for **Teh-O**
(the **T**ransport, **E**igenvalue and **H**ybrid **O**pen-source solver; named
after "teh-O", the Southeast-Asian tea). It models reactor point kinetics with
delayed-neutron precursor groups, reactivity feedback, and decay heat, using
`uom` dimensioned quantities throughout. License: ~~GPL-3.0~~ **AGPL-3.0-only** combined work (relicensed 2026-10-20, gh:#973; ported files keep GPL-3.0 and `LICENSE.GPL-3.0` ships beside `LICENSE`; see `docs/relicensing-agpl-2026-10-20.md`).

## Code layout (`src/`)

- `zero_power_prke/` — point kinetics core.
  - `n_group_prke.rs` — **the one PRKE core** (gh:#1082): `PointKinetics`
    with any number of delayed groups, backward Euler and RKF45, in-hour
    solver. `SixGroupPRKE` delegates to it and is pinned bit-identical to its
    pre-refactor self (`six_group_bit_identity_pin.rs`).
  - `circulating.rs` — `CirculatingPointKinetics` (gh:#1131, MSRE T-C1):
    precursors drift out with the fuel (core transit τ_c) and return after a
    loop transit τ_L. Verified against its analytic circulation loss and
    circulating in-hour equation, and against `PointKinetics` as τ_c → ∞.
  - `six_group_precursor_prke/` — six delayed-neutron precursor groups.
    - `six_group_constants.rs` — the `FissioningNuclideType` enum and the
      `new_u233_/u235_/pu239_delayed_neutron_fraction_array()` constructors
      (per-nuclide delayed fractions). **These items live in this submodule**,
      not at the `six_group_precursor_prke` level (see migration note below).
- `feedback_mechanisms/` — incl. `fission_product_poisons/` (Xe/Sm).
  - `separable.rs` — RELAP5 separable feedback (RELAP-7 Eq 443, gh:#1083).
  - `fast_reactor.rs` — fast-reactor feedback beside it (gh:#1326): fast
    Doppler `-K_D ln(T/T_ref)`, axial fuel, clad, coolant T and density,
    radial structures, driveline expansion via a rod-worth map. Port of
    GeN-Foam's point-kinetics feedback sum; every coefficient a caller input.
- `fuel_temperature_feedback`, `control_rod_feedback`, `decay_heat`.
- `ans_decay_heat` — ANS standards enum, isotope mix incl. Pu-241, `F_gamma`,
  user-supplied coefficients, `G(t)` (gh:#1080). Only the 1978 draft set is
  built in; the 1979/1994/2005 tables are copyrighted and user supplied (see
  the module docs).
- `point_reactor` — `PointReactor`: kinetics + decay heat + actinide slot,
  `n -> psi -> P` conversion, total power Eq 429 (gh:#1082). Actinides are
  gh:#1081, not implemented.
- `time_stepping/` — integrators (incl. OpenFOAM-derived source files).
- `teh_o_prke_error` — crate error enum (thiserror).

## Build, test, run

**Rule: always use `--release` for builds and tests.** Never run in debug mode.

```bash
cargo test -p teh-o-prke --release                        # unit tests
cargo run -p teh-o-prke --example fhr_sim_v1 --release   # FHR educational GUI
```

~~Requires system OpenBLAS (see root CLAUDE.md).~~ **CORRECTED 2026-11-01:**
no BLAS is needed; `Cargo.toml` has no `ndarray-linalg` and the solve is the
pure-Rust `matrix::SquareMatrix`.

## Conventions

- Public APIs take/return `uom` dimensioned quantities — no bare `f64` SI values
  at API boundaries.

## Notes (read on demand)

The plan to drop the `ndarray-linalg` runtime dep (one 7×7 solve) and the
2026-06 migration log live in **`docs/notes.md`**.

## egui simulators need a headless mode

This crate's egui/eframe examples fall under the workspace headless-mode
rule: a deterministic `--headless` path with a regression test. The full rule
lives in
[`crates/outram-park-digital-twin-engine/CLAUDE.md`](../outram-park-digital-twin-engine/CLAUDE.md).

## Avoid writing macros (workspace rule, 2026-11-05)

Maintainer direction: macros make code harder to read and review. Do not
write new `macro_rules!` or proc macros in this crate; use functions,
generics, enums and plain `match` arms. Using std and dependency macros
(`vec!`, `format!`, derives) is fine. Full rule: root `CLAUDE.md`, "Avoid
writing macros".
