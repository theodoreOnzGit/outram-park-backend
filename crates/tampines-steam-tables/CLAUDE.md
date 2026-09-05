# CLAUDE.md

Guidance for Claude Code (and other AI assistants) working in this repository.

## Project overview

## Maturity: DECLARED MATURE (2026-09-05)

The API-usability rules in the root `CLAUDE.md` ("Human interface layer",
and the Haiku dogfooding hard rule) **are in force for this crate**. See the
maturity gate in that file for what this means and how the bar is revised.

- **2026-09-05 — mature.** Bar: agreement with the **IAPWS-IF97** published
  verification tables to **max_relative = 1e-8** on the forward properties;
  **5e-5** on the backward `T(p,h)` and `T(p,s)` correlations (looser because
  the backward equations are themselves fits, not inversions); and **1e-4** on
  the extreme rows (1000 bar / 0 °C), set by the published table precision
  rather than by this crate. Evidence class: **cross-code / reference-standard
  comparison** against IAPWS-IF97, supported by unit tests.

  Measured at declaration: 1001 `#[test]` markers in-crate and the suite green
  (individual per-crate pass count not separately recorded at declaration time;
  118 explicit IAPWS-IF97 references in-source). Additional evidence: re-running
  the suite regenerates `verification_and_validation/generated/*.md` with every
  number byte-identical and only the timestamp line changed — the V&V results
  reproduce exactly.

  IF97 is a released international standard with its own verification tables,
  which makes this the best-anchored bar of the six declared crates.


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
cargo test --release --lib                      # unit/verification tests (938 test fns; see the note below)
cargo test --release <name>                     # run a subset by substring match
cargo run --release --example fhr_sim_v1        # earlier FHR educational simulator (fhr_sim_v2 moved to crates/tampines/examples/)
```

**No system BLAS is needed.** This crate's `[dependencies]` are only `approx`,
`ndarray`, `thiserror` and `uom` — the `ndarray-linalg` entries that used to sit
in the three `[target.*.dependencies]` blocks were vestigial and have been
removed, and nothing in `src/` imports `ndarray_linalg`. Earlier revisions of
this file told Linux users to `sudo apt install libopenblas-dev` and told
Windows/macOS users that the static Intel MKL feature applied; neither is true
any more. `tuas_boussinesq_solver` (a dev-dependency, used only by the
FHR-simulator examples) also dropped `ndarray-linalg` at TUAS v0.1.2 in favour of
pure-Rust `peroxide`, so the transitive need is gone too.

### Testing notes — the suite is dominated by one very long test

**HARD RULE: a timeout is NOT a test failure.** Do not report a killed run as a
failing test, do not "fix" it by loosening a tolerance, and do not conclude the
suite is broken because a full-suite command was cut off.

The Edwards–O'Brien blowdown integration test
**`tests/edwards_blowdown.rs::edwards_obrien_pipe_blowdown_600ms`** integrates
the full 600 ms transient on a 24-cell mesh at `dt = 30 µs` — 20 000 PIMPLE
steps, each doing real IAPWS-IF97 `(p, h)` two-phase flashes on every cell.
**Measured 2026-08-11 in release mode: 384.75 s (≈ 6.5 min) for that test alone**
(`cargo test --release -p tampines-steam-tables --test edwards_blowdown
edwards_obrien_pipe_blowdown_600ms` → `1 passed; 0 failed; finished in 384.75s`).
Its sibling `edwards_hybrid_damps_ringing_vs_pimple` runs a shorter 0.15 s window
in two solver modes. Cargo runs the two in **parallel**, so the whole target
costs about as much as its longest test rather than the sum — **measured
2026-08-11: `cargo test --release -p tampines-steam-tables --test
edwards_blowdown` → `2 passed; 0 failed; finished in 393.58s`** (404 s of wall
clock including the build). That integration target, not the library tests, is
why a bare `cargo test --release -p tampines-steam-tables` blows through a
default command timeout.

**Two caveats on any timing you read, including the ones above.**

- *Hardware and load.* The in-file `//!` header of `tests/edwards_blowdown.rs`
  still says "~180 s wall" from 2026-07-16, and a figure of ~897 s (≈ 15 min)
  has also been reported for this test. Neither reproduced here on 2026-08-11.
  Measure on the machine in front of you rather than quoting a number.
- *Command time is not test time.* Wall-clock for the `cargo test` **command**
  also includes compilation and, importantly, **waiting on the cargo file
  lock** when another process is building the workspace. During this
  measurement one such invocation sat on `Blocking waiting for file lock on
  build directory` for well over ten minutes before the test even started, which
  is an easy way to attribute a lock wait to the test. Read the harness's own
  `finished in <N>s` line, not the shell's elapsed time.

Practical consequences for an agent or a CI step:

- **Budget real wall-clock time.** A default 120 s command timeout kills the
  Edwards test mid-run. Give it a generous timeout or run it in the background.
- **Split the run.** `cargo test --release --lib` is the fast path — the 938
  library test functions finish in seconds. Add `--test edwards_blowdown` as a
  separate, long-timeout invocation when you actually need it.
- **Run the targeted subset while iterating**
  (`cargo test --release -p tampines-steam-tables <substring>`), and the
  integration test only when finishing.
- **Report what you measured.** If a run was killed, say it was killed and how
  long it got — never convert that into a pass or a fail count.

**Current suite status (measured 2026-08-11, `cargo test --release --lib`,
after the Marviken work of bead `op-21g.16` landed):** **940 passed, 0 failed,
13 ignored** (953 test functions). List the ignored ones
with `cargo test --release -p tampines-steam-tables --lib -- --ignored --list`
rather than trusting any count written down in a document.

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
validated against Moody (1975) and Zaloudek HEM reference curves. **Marviken is
now gated, with a split result — validated on test 23, NOT validated on
test 24** — see below. The three split solvers (in-dome /
subcooled / superheated-vapour) cover all stagnation buckets relative to the
p-h VLE dome.

All three stagnation buckets (in-dome, subcooled, superheated) are validated.
The known discrepancy is `isobar_pref_0_25` (p₀ = 1.72 bar) in the Moody chart
tests — its sole deeply-subcooled data point deviates by |Δ log10 G| = 0.170
because the IAPWS-IF97 isentrope diverges from the incompressible-Bernoulli
limit at extreme pressure ratios (p_bubble/p₀ ≈ 0.02). **That test is no longer
`#[ignore]`d** — it is active and passes, because the Moody tests are now
region-filtered and assert only their in-dome (Region 4) points, skipping the
single-phase ones (see the region-filtering note below, and bead `op-21g.2`,
which is a candidate for closure on those grounds). All Moody isobars
(0.25 – 30.0 × p_ref) are active.

**Ignored tests — the complete list, 13 of them** (regenerated 2026-08-11 from
`cargo test --release -p tampines-steam-tables --lib -- --ignored --list`, which
is authoritative; `grep -rnE '^\s*#\[ignore' src/` returns 15 hits because one of
them sits inside a `/* … */` block comment and is not compiled at all):

| Test | Kind |
|---|---|
| `zaloudek_*::outside_dome_stagnation_subcooled::diagnose_bubble_point_artifact` | Diagnostic — per-point sweep, asserts nothing |
| `cd_nozzle_choked_flow_overexpanded::wet_steam_test` | **Unfinished** — "temporary skip test" |
| `diverging_nozzle_perfectly_expanded_supersonic::..._wet_steam` | **Unfinished** — "test not ready" |
| `ph_flash_steam_table::single_phase_table_1000_bar::single_phase_table_2_to_750_degc` | Known gap — "at 1000 bar, ph flashing goes out of bounds, yet to debug" |
| `hs_flash_steam_table::single_phase_table_1000_bar::single_phase_table_2_to_750_degc` | Same 1000-bar gap, `(h,s)` path |
| `hs_flash_steam_table::single_phase_table_0_006112127_bar::single_phase_table_0_to_240_degc_except_triple_pt` | Known gap — "hs flash cannot do triple point pressure yet" |
| `hs_flash_steam_table::single_phase_table_0_006112127_bar::single_phase_table_250_to_800_degc` | Same triple-point gap |
| `ps_flash_steam_table::single_phase_table_240_bar_to_1000_bar::single_phase_table_2_to_750_degc_1000_bar` | Deferred — "to implement in next major version" |
| `pt_flash_steam_table::single_phase_table_240_bar_to_1000_bar::single_phase_table_2_to_750_degc_1000_bar` | Deferred — "to implement in next major version" |
| `openfoam_source::thermophysics::eos::peng_robinson::tests::co2_nist_density_400k_10mpa` | **Known error** — PR EOS 17 % off at Pr > 1 |
| `openfoam_source::thermophysics::eos::peng_robinson::tests::n2_nist_density_300k_10mpa` | **Known error** — PR EOS 7 % off vs NIST at Pr = 2.94 |
| `openfoam_source::thermophysics::eos::peng_robinson::tests::n2_nist_density_200k_5mpa` | **Known error** — PR EOS 26 % off vs NIST at 200 K / 5 MPa |
| `openfoam_source::thermophysics::thermo::janaf::tests::newton_converges_from_bad_initial_guess` | **Known error** — Newton stalls at ~1152 K; JANAF discontinuity at `Tcommon` |

One diagnostic is ignored by design. Three unfinished ones are real gaps and all
three sit on the **wet-steam** path — the same path the turbine work depends on.
The four `peng_robinson` / `janaf` entries are **recorded numerical errors, not
skipped scaffolding**: they document a suspected root-selection or formula bug in
the vendored OpenFOAM thermophysics, so do not describe that layer as verified.

**Correction (2026-08-11):** an earlier version of this table listed
`moody_*::diagnose_deep_subcooled_failures` as an ignored test and claimed the
list was complete at five entries. Both were wrong.
`diagnose_deep_subcooled_failures` was commented out wholesale on 2026-06-30
(it lives inside a `/* … */` block in
`moody_critical_mass_flux_homogeneous_eqm.rs`), so it is not a test at all —
that file has **13 tests and zero `#[ignore]`s**. The nine flash-table, Peng-
Robinson and JANAF entries were simply missed.

Verification tests are under `.../tests/`, validated against:

- Moody (1975), maximum discharge rate of liquid-vapour mixtures — `moody_*`.
  These tests are **region-filtered**: each isobar asserts only its in-dome
  (Region 4) points and skips the single-phase points. The subcooled (Region 1)
  branch is a documented HEM limitation — HEM equilibrium under-predicts subcooled
  critical flow, and no local discriminator separates Moody's deep-subcooling
  (Bernoulli) reference from Zaloudek's near-saturation (sonic) reference (see
  README v0.2.1). Like Zaloudek, Moody's data is graph-read, so G tolerances are
  loose (0.06 in log10). `isobar_pref_4_00` formerly needed a 0.25 tolerance from a
  bad digitisation (~0.13 log10 high); it was re-digitised (README v0.2.1, 2026-06-30
  update) and now passes at the standard 0.06 like every other isobar.
- Zaloudek critical mass flux — `zaloudek_*`. NOTE: these reference values are
  graph-read (digitised) HEM curves, not raw experimental data, so keep mass-flux
  (G) tolerances loose.

**Marviken — gated 2026-08-11, split outcome (bead `op-21g.16`).**
`marviken_tests.rs` is now a real V&V case: 6 active tests, none ignored,
~1.4 s, comparing this crate's HEM critical-flow dispatcher against the
digitised NUREG/CR-2671 (MXC-301) Fig. 8:24 envelopes for the 500 mm /
`L/D` = 0.3 nozzle.

- **Test 23 (3 K nominal subcooling) — VALIDATED.** Mean deviation 12.6 %,
  worst 23.1 %, inside a justified ±25 % per-point / ±15 % mean band whose
  dominant term is the ±12.9 % measured experimental scatter.
- **Test 24 (33 K nominal subcooling) — NOT VALIDATED.** Mean −48.5 %, worst
  −70.2 %, 31 of 40 points outside the band. Kept as an honest characterisation
  test, not a validation. **Do not describe the choked-flow work as
  Marviken-validated for subcooled stagnation states, and do not cite it as such
  in a paper.**
- **The test-24 deficit is a solver defect, not HEM physics.** The bare HEM
  maximum-mass-flux criterion (`max_p ρ√(2Δh)` along the isentrope, built from
  the public `(p,s)` flashes in the same test file) reproduces *both* Marviken
  tests to a mean of 9–10 %, with test 24 essentially unbiased at −1.8 %. The
  48.6 % gap comes from `get_critical_pressure_and_mass_flux_subcooled_liquid_ph`
  taking its bubble-point sonic-kink branch (`ρ_f·c_2φ`) because the quality at
  the energy maximum is < 0.03 while `DEEP_SUBCOOLING_RATIO = 5.0` is not
  reached — i.e. the Marviken points land in the "overlap zone" that solver's
  own comment block flags as unresolved for want of experimental data. **This is
  the second time an apparent HEM limitation here turned out to be a
  choke-finder branch problem**; see the near-bubble-point artifact below.
  Retuning the threshold was deliberately *not* done as part of the validation —
  that would be fitting the model to its own validation data.

Full methodology, data provenance, error budget, per-point results and the
lessons note for higher-fidelity (drift-flux / two-fluid) reruns live in the
module `//!` doc of `marviken_tests.rs`.

### Resolved: near-bubble-point HEM artifact (x ≈ 0)

The **near-bubble-point HEM artifact** that used to break the saturated-liquid-
line Zaloudek tests is **fixed**. It was a numerical issue in the forward choke
finder, not an HEM physics limitation — `mass_flux_ps_eqm_throat` evaluated at
the throat reproduces the x ≈ 0 reference to ±0.04 in log10 G at every point (the
Zaloudek reference is itself HEM). The energy-balance maximum of `G(p)` is blind
to the sound-speed discontinuity at the bubble point, so on the saturated-liquid
line it overshoots (5,10,300,500 psia) or walks off to a deeper non-physical
stationary point (15–200 psia, 11–21 % low). `get_critical_pressure_and_mass_flux_subcooled_liquid_ph`
now detects this regime by the two-phase quality at the energy-max choke
(< 0.03 ⇒ throat ≈ saturated liquid) and takes the bubble-point kink choke with
ρ_f·c_2φ read from a precomputed sonic map along the saturated-liquid line
(`saturation_line_sonic_mass_flux`). Neither stagnation subcooling nor pressure
separates the artifact from genuine interior choking — the quality at the choke
is the only clean discriminator. All x_t = 0.0–1.00 curves now pass.

The original combined canary
`zaloudek_*::generic_multiphase_stagnation::quality_0_05_stagnation` is now
`#[ignore]`d. The strategy is **two separate solvers, one per stagnation region**,
with the test files partitioning each Zaloudek throat by where its backward-mapped
stagnation `(p0, h0)` lands relative to the VLE dome (`ph_flash_region`):

- `outside_dome_stagnation_subcooled.rs` — stagnation OUTSIDE the dome (left
  side, Region 1 subcooled liquid). Keeps only `ph_flash_region == Region1`,
  runs `get_critical_pressure_and_mass_flux_subcooled_liquid_ph`. The 20
  genuinely-subcooled curves (x_t = 0.05 … 1.00) pass.
- `in_dome_stagnation.rs` — stagnation INSIDE the dome (two-phase, Region 4).
  Keeps only `ph_flash_region == Region4`, runs
  `get_critical_pressure_and_mass_flux_ph_vle_dome`. All 21 quality curves
  (x_t = 0.0 … 1.00) pass.

Both files run the full quality sweep over the same data; the region filter
routes each point and `continue`-skips the rest (so a green test may have
silently skipped most points — check the `skip p=…` stderr lines). The two
buckets are complementary: for a given quality, low-pressure throats keep a
two-phase stagnation (in-dome runs them, subcooled skips), while the high-
pressure tail recompresses out of the dome to Region 1 / Region 3 (subcooled
runs them, in-dome skips).

Diagnostic — the dome routing is what fixed the old +25% artifact. Worked
example, x_t = 0.05 in-dome: 13 points (5–750 psia) stay in the dome and pass
(worst pressure error +0.86% at 100 psia — the *same* point the old combined
canary missed by +25%); the 4 high-pressure points skip out (1000/1500/2000 psia
→ Region 1, 3000 psia → Region 3). Note `quality_0_05_in_dome` loosens its
pressure tolerance to 0.01 (bubble-point edge of the dome, ~0.7% round-trip);
all other in-dome curves use 0.005.

The x = 0.0 bubble-point curve is the curve of primary interest going forward
(`quality_bubble_point_in_dome`, x_t = 0.0, and its subcooled counterpart at
x_t = 1e-4).

The former canary
`outside_dome_stagnation_subcooled::quality_bubble_point_subcooled`
(x_t = 1e-4, throats essentially on the saturated-liquid line) now **passes** and
is no longer `#[ignore]`d. The `diagnose_bubble_point_artifact` test in the same
file prints the per-point breakdown that drove the fix (its `thr_dGlg` column is
the HEM-at-throat reproduction within ±0.04). The comment block above the canary
documents the root cause and the quality-based routing.

### Known sharp edges

- Near the **bubble point**, near-saturated stagnation states must be routed to
  the in-dome solver, not the subcooled one — the dispatcher handles this and it
  is easy to break.
- HEM has documented limitations near the saturation line (see in-code comments
  and `docs/derivation/`); metastable / non-equilibrium effects are not modelled.

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

## Choked-flow solver status

| Function | Status |
|---|---|
| `get_critical_pressure_and_mass_flux_multiphase_ph` | ✅ Unified dispatcher — routes `(p0,h0)` by `ph_flash_region` to the solvers below; powers `TampinesSteamTableCV::get_crit_pressure_and_massflux`. All 13 `generic_multiphase_stagnation` tests pass |
| `get_critical_pressure_and_mass_flux_ph_vle_dome` | ✅ Validated — all 21 Zaloudek in-dome quality curves pass |
| `get_critical_pressure_and_mass_flux_subcooled_liquid_ph` | ✅ Validated — all Zaloudek subcooled curves incl. the x_t ≈ 0 saturated-liquid line pass |
| `get_critical_pressure_and_mass_flux_superheated_vapour_ph` | ✅ Validated — Zaloudek superheated-vapour / supercritical curves (x_t = 0.80–1.00) pass |
| `dome_crossing_interior_choke` (private) | ✅ Near-critical Region 3 helper — finds the interior two-phase choke when a supercritical isentrope crosses the dome apex, skipping the spurious phase-boundary kink (see README v0.2.1) |
| `get_critical_pressure_and_mass_flux_with_stagnation_props` | ❌ Superseded — old combined dispatcher with +25% artifact; retain for reference only, no longer wired into the OOP API |

**Near-bubble-point HEM artifact (x_t ≈ 0) — fixed:**
`outside_dome_stagnation_subcooled::quality_bubble_point_subcooled` now passes.
The old failure was numerical (the energy-balance choke finder is blind to the
sound-speed discontinuity at the bubble point), not an HEM physics limitation —
HEM at the throat reproduces the x ≈ 0 reference to ±0.04 in log10 G. The solver
routes near-saturation throats (two-phase quality at the energy-max choke < 0.03)
to the bubble-point kink choke, mass flux from a saturated-liquid-line sonic map.
See the comment block above that test and `diagnose_bubble_point_artifact`.

**`generic_multiphase_stagnation::quality_*`** — now active (no longer
`#[ignore]`d). These drive the unified dispatcher
`get_critical_pressure_and_mass_flux_multiphase_ph` end-to-end and assert
per-point tolerances matching the dedicated region tests (Region 4 → 0.005/0.01,
Region 1 → 0.03, Region 2/3 → 0.05). See README v0.2.1 for the debugging trail on
the near-critical Region 3 (3000 psia) points.

**Moody isobar tests** — `moody_critical_mass_flux_homogeneous_eqm::isobar_pref_*`
are now active (no longer `#[ignore]`d) and pass via region-filtering to the
in-dome (Region 4) points; the subcooled branch is a documented HEM limitation
(see above and README v0.2.1).

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
resolves to the in-tree path crate, but it is now a **dev-dependency only** —
solely the FHR simulator examples use it. The library is TUAS-free (the former
`UEqn.rs` pipe-friction call into `tuas` was removed along with the dead
`rhoPimpleFoam` equation scaffolds, whose logic already lives inline in
`rhoPimpleFoam/mod.rs`).

See **`docs/notes.md`** for the 2026-06 migration log. The vestigial
`ndarray-linalg` dep is **removed** (done, not planned — see "Build, test, run"
above), and the former `fhr_sim_v2` UI state-update bug is resolved (both
recorded in `docs/notes.md`).
