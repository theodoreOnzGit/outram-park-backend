# tampines-steam-tables — choked-flow status, version history & workspace notes

Reference material: the detailed multiphase choked-flow (HEM) current-work
status, the v0.2.0 status table and roadmap, and OUTRAM PARK workspace/migration
notes. Consulted on demand, not per turn. Mandatory conventions/guardrails and
the crate overview live in CLAUDE.md.

## Testing notes — read before running the suite

**Suite status, measured 2026-08-11** (`cargo test --release -p
tampines-steam-tables --lib`): **924 passed, 0 failed, 14 ignored**, out of 938
library test functions.

**A TIMEOUT MUST NOT BE REPORTED AS A FAILURE.** A killed run is a killed run:
say so, say how far it got, and re-run it with more time. Do not turn it into a
failure count, do not conclude the suite is broken, and never loosen a tolerance
because a long test was inconvenient.

The reason full-suite runs hit timeouts is a single integration test:
**`tests/edwards_blowdown.rs::edwards_obrien_pipe_blowdown_600ms`**. It
integrates the Edwards–O'Brien 600 ms pipe blowdown on a 24-cell mesh at
`dt = 30 µs` — 20 000 PIMPLE steps, each performing real IAPWS-IF97 `(p, h)`
two-phase flashes on every cell. **Measured 2026-08-11, release mode: 384.75 s
(≈ 6.5 min) for that test on its own**:

```text
$ cargo test --release -p tampines-steam-tables \
    --test edwards_blowdown edwards_obrien_pipe_blowdown_600ms
test edwards_obrien_pipe_blowdown_600ms has been running for over 60 seconds
test edwards_obrien_pipe_blowdown_600ms ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out;
finished in 384.75s
```

Its sibling `edwards_hybrid_damps_ringing_vs_pimple` runs a shorter 0.15 s window
in two solver modes. Cargo runs the two tests in **parallel**, so the whole
target costs about as much as its longest test rather than the sum — measured
the same day at **393.58 s** (`2 passed; 0 failed; finished in 393.58s`; 404 s
of shell wall clock including the build). Everything else in the crate — all 938
library tests — finishes in seconds.

**Two caveats on any timing, including the ones above.** They are hardware- and
load-dependent: the `//!` header of `tests/edwards_blowdown.rs` still quotes
"~180 s wall" from 2026-07-16, and a figure of ~897 s (≈ 15 min) has also been
reported for this test; neither reproduced here on 2026-08-11. And the wall
clock of a `cargo test` **command** is not the test's runtime — it also covers
compilation and any wait on the cargo build-directory lock. While taking these
measurements one invocation sat on `Blocking waiting for file lock on build
directory` for over ten minutes, with another workspace build in progress,
before the test started at all. **Quote the harness's `finished in <N>s` line,
not the shell's elapsed time** — otherwise a lock wait gets recorded as test
cost.

Practical guidance (mirrors the workspace `CLAUDE.md` rule for the TUAS CIET
natural-circulation tests, which are slow for the same reason):

- **Split the run.** `cargo test --release --lib` for the fast path; add
  `--test edwards_blowdown` as a separate, generously-timed invocation.
- **Budget real wall-clock time.** A default 120 s command timeout kills the
  Edwards test mid-run. Give it a long timeout or run it in the background.
- **Run the targeted subset while iterating**
  (`cargo test --release -p tampines-steam-tables <substring>`), and the
  integration test only when finishing.
- **Report what you measured.** Never write a pass/fail count you did not
  actually obtain from a completed run.

In short: measure, don't quote.

## Choked flow (current focus)

> **⚠️ SUPERSEDED (2026-07, v0.2.1) — read this first.** Much of the
> present-tense narrative in this section predates v0.2.1 and no longer matches
> the code. In particular: the "near-bubble-point HEM artifact" is **resolved**
> — it was a numerical bug in the forward choke finder, **not** an HEM physics
> limitation, and it does **not** need an HRM. The test
> `outside_dome_stagnation_subcooled::quality_bubble_point_subcooled`
> (x_t ≈ 0) now **passes** and is no longer `#[ignore]`d; likewise all
> `generic_multiphase_stagnation::quality_*` and the
> `moody_critical_mass_flux_homogeneous_eqm::isobar_pref_*` tests are **active
> and pass**. `isobar_pref_0_25` is **not** `#[ignore]`d either — it runs
> in-dome-only (`validate_moody_isobar_in_dome_only`) and skips its sole
> deeply-subcooled point, which shows a documented isentrope-vs-Bernoulli
> divergence at p_bubble/p₀ ≈ 0.02. The solvers are now
> fronted by a single unified dispatcher
> `get_critical_pressure_and_mass_flux_multiphase_ph` (routing a stagnation
> `(p0, h0)` by `ph_flash_region`), plus a `dome_crossing_interior_choke`
> helper for supercritical Region 3. For the authoritative, current status see
> the crate `CLAUDE.md` ("Choked flow" + "Choked-flow solver status" tables)
> and the `README.md` Changelog (v0.2.0 → v0.2.2). The historical text below is
> kept for the debugging trail only.

`src/steam_turbine_equations/converging_diverging_nozzles/choked_flow/`
implements critical-flow solvers using the Homogeneous Equilibrium Model (HEM):

- `single_phase_basic_choked_flow.rs` — single-phase choked flow.
- `stagnation_point_within_vle_ph_dome_multiphase.rs` — stagnation state inside
  the p-h VLE dome (two-phase).
- `stagnation_point_outside_vle_ph_dome_multiphase.rs` — stagnation state
  outside the dome (subcooled liquid-like, superheated/supercritical).
- `basic_multiphase_equations.rs` — generic multiphase relations (e.g.
  stagnation properties from throat properties).
- `saturation_lookup_table.rs` — precomputed table seeding the bubble/dew-point
  bisection.

Verification tests are under `.../tests/`, validated against:

- Moody (1975), maximum discharge rate of liquid-vapour mixtures — `moody_*`.
- Zaloudek HEM reference curves — `zaloudek_*`. **These are NOT experimental
  measurements.** They are HEM-computed curves published by Zaloudek and
  graph-read (digitised) from Figure 2 of Saha (1978) NUREG/CR-0417. Keep
  mass-flux (G) tolerances loose. The bubble-point edge (x_t ≈ 0) **is** a
  validation target and is met: because the Zaloudek reference is itself an
  HEM curve, HEM must be able to reproduce it, and after the v0.2.1 choke-finder
  fix it does (see the resolution note below). The older claim that x_t ≈ 0 was
  a fundamental HEM physics limitation was wrong.
- Marviken critical flow tests — `marviken_tests.rs`. **Gated 2026-08-11 with a
  split outcome** (bead `op-21g.16`): 6 active tests, none ignored, ~1.4 s.
  Against NUREG/CR-2671 Fig. 8:24 (500 mm / L/D = 0.3 nozzle) the HEM dispatcher
  **validates on test 23** (3 K subcooling — mean deviation 12.6 %, worst
  23.1 %, inside a justified ±25 % experimental band) and **is NOT validated on
  test 24** (33 K subcooling — mean −48.5 %, worst −70.2 %). Do not describe the
  crate as Marviken-validated for subcooled stagnation states. The bare HEM
  maximum-mass-flux criterion reproduces *both* tests to a mean of 9–10 %, so
  the test-24 deficit is a branch-selection defect in
  `get_critical_pressure_and_mass_flux_subcooled_liquid_ph` (the ≈0.03-quality
  bubble-point sonic kink being taken instead of the energy-balance maximum,
  with `DEEP_SUBCOOLING_RATIO = 5.0` not reached at 33 K), **not** an HEM
  physics limitation. Full methodology, error budget and lessons in the module
  doc of `marviken_tests.rs`.

### Resolved (v0.2.1): the near-bubble-point HEM artifact

> **Historical.** This section records the effort that *was* under way to solve
> the **near-bubble-point HEM artifact** breaking the Zaloudek VLE
> critical-pressure / mass-flux tests. It is **fixed** — see the resolution note
> at the end of this section. The present-tense wording below is kept for the
> debugging trail.

The original combined canary
`zaloudek_*::generic_multiphase_stagnation::quality_0_05_stagnation` is now
`#[ignore]`d. The strategy is **three separate solvers, one per stagnation
bucket**, with the test files partitioning each Zaloudek throat by where its
backward-mapped stagnation `(p0, h0)` lands relative to the VLE dome
(`ph_flash_region`, plus the `s0` vs `s_crit` test for the vapour side):

- `outside_dome_stagnation_subcooled.rs` — stagnation OUTSIDE the dome (left
  side, Region 1 subcooled liquid). Keeps only `ph_flash_region == Region1`,
  runs `get_critical_pressure_and_mass_flux_subcooled_liquid_ph`. The 20
  genuinely-subcooled curves (x_t = 0.05 … 1.00) pass.
- `in_dome_stagnation.rs` — stagnation INSIDE the dome (two-phase, Region 4).
  Keeps only `ph_flash_region == Region4`, runs
  `get_critical_pressure_and_mass_flux_ph_vle_dome`. All 21 quality curves
  (x_t = 0.0 … 1.00) pass.
- `outside_dome_stagnation_superheated.rs` — stagnation OUTSIDE the dome (right
  side / above it: superheated vapour or supercritical vapour-like). Keeps only
  points with `s0 > s_crit` and `ph_flash_region != Region4`, runs
  `get_critical_pressure_and_mass_flux_superheated_vapour_ph` (dew point replaces
  the bubble point; the single-phase vapour stretch needs its own golden-section
  search because the vapour sonic choke is interior). The high-quality curves
  (x_t = 0.90/0.95/1.00) pass across the full supercritical range; x_t = 0.80
  uses a relaxed pressure tolerance for the near-critical-point corner.

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
x_t = 1e-4). A `marviken_tests.rs` stub exists under the same tests directory
but is `#[ignore]`d and ends with `todo!()` — data is read in but the
assertion block is not written yet.

**Resolved — `quality_bubble_point_subcooled` was fixed, not ignored
(re-verified 2026-08-11).** This paragraph used to call
`outside_dome_stagnation_subcooled::quality_bubble_point_subcooled`
(x_t = 1e-4, throats essentially on the saturated-liquid line) "the active
failing test" and claimed a non-equilibrium / relaxation model was required.
That is wrong on both counts and is corrected here.

- **It is an active, passing test — not one of the crate's ignored tests.**
  There is no `#[ignore]` on it (`grep -rnE '^\s*#\[ignore' src/` does not list
  it; the only `#[ignore]` in that file is on the neighbouring
  `diagnose_bubble_point_artifact` diagnostic).
- **Verified by running it**, 2026-08-11:

  ```text
  $ cargo test --release -p tampines-steam-tables quality_bubble_point_subcooled
  test steam_turbine_equations::converging_diverging_nozzles::tests::\
  zaloudek_critical_mass_flux_homogeneous_eqm::outside_dome_stagnation_subcooled::\
  quality_bubble_point_subcooled ... ok

  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 937 filtered out
  ```

- **What actually happened.** The failure was numerical, in the forward choke
  finder, not an HEM physics limit. The energy-balance objective
  `G_energy(p) = ρ·√(2(h0−h))` is blind to the discontinuity in the HEM sound
  speed at the bubble point, so on the saturated-liquid line its maximum either
  overshoots `ρ_f·c` (5, 10, 300, 500 psia) or walks off to a deeper stationary
  point the flow never reaches at M = 1 (15–200 psia, choke pressure 11–21 %
  low). `get_critical_pressure_and_mass_flux_subcooled_liquid_ph` now routes on
  the two-phase **quality at the energy-max choke**: below ~0.03 the throat is
  effectively saturated liquid, so it takes the bubble-point kink choke with the
  mass flux read from a precomputed sonic map along the saturated-liquid line;
  otherwise it keeps the validated golden-section energy max. All x_t = 0.0 …
  1.00 Zaloudek curves then pass. The full write-up is the comment block
  directly above the test.

The old three-failure-mode analysis that used to sit here is superseded; it
described the symptoms of the choke-finder bug, not a property of HEM.

The older combined canary swept x_t = 0.05 over a pressure range; first and last
reference points:

- first: p = 5 psia, G = 64.0497 lb/(s·ft²), h0 = 177.3399 Btu/lb
- last:  p = 3000 psia, G = 14016.4977 lb/(s·ft²), h0 = 795.0739 Btu/lb

Diagnosis so far:

- Stagnation reconstruction is fine: `h0_calc ≈ h0_expected` at every point
  (e.g. 343.98 vs 345.81 Btu/lb at 100 psia).
- The forward solver in
  `choked_flow/mod.rs::get_critical_pressure_and_mass_flux_with_stagnation_props`
  locks onto a spurious root near the bubble point. At 100 psia / x≈0.05 it
  converges to p_throat ≈ 860.3 kPa vs the reference 689.5 kPa (+25%), at
  quality ≈ 0.034.
- `g_energy` (energy balance) and `g_hem` (`mass_flux_ps_eqm_throat`,
  finite-difference dv/dP) never truly cross: at the "converged" point
  g_energy ≈ 3092 but g_hem ≈ 5738, so f = g_energy − g_hem ≈ −2646, nowhere
  near zero. The HEM throat mass flux spikes near the saturated-liquid line, so
  the only sign change the bracket finder sees is across that artifact, not a
  physical choke point. Regula falsi then stalls on the discontinuity
  (retained-endpoint problem) and reports the bogus pressure at max_iterations
  instead of failing.
- Pressure-dependent: 5–75 psia stay within the 5% tolerance; 100 psia is the
  first to break — consistent with the `subcooled` test note (11–21%
  choke-pressure error at 15–200 psia). It is the known HEM limitation near the
  saturation line, not a units or reconstruction bug.

### Known sharp edges

- Near the **bubble point**, near-saturated stagnation states must be routed to
  the in-dome solver, not the subcooled one — the dispatcher handles this and it
  is easy to break.
- HEM has documented limitations near the saturation line (see in-code comments
  and `docs/derivation/`); metastable / non-equilibrium effects are not modelled.
- **An HRM is *not* required to pass the Zaloudek dome-boundary curves.** An
  earlier version of this note asserted that a Homogeneous Relaxation Model was
  needed at x_t ≈ 0 and x_t ≈ 1 because HEM "overpredicts G and underpredicts
  choke pressure" there. That was a misdiagnosis of the choke-finder bug fixed in
  v0.2.1 — both boundary curves now pass against the Zaloudek reference, which is
  itself an HEM curve, so no relaxation term is involved.
- **What is still open is a *physical* question, not a test failure.** The
  Zaloudek and Moody references are equilibrium (HEM) curves, so passing them
  says nothing about how well HEM matches *real* flashing flow where nucleation
  or droplet formation lags the local pressure drop. Validating against genuinely
  experimental critical-flow data — Marviken (`marviken_tests.rs`, still
  unfinished) is the nearest such case — is what would show whether a relaxation
  model is needed near the dome boundaries. Do not cite the passing Zaloudek
  boundary curves as evidence that it is not.

---

### v0.2.0 — multiphase HEM choked flow status (2026-06)

> **⚠️ SUPERSEDED by v0.2.1.** The status table and "actively failing tests"
> list below are the v0.2.0 snapshot. Since then the near-bubble-point artifact
> was fixed (numerical, not an HEM limit), `quality_bubble_point_subcooled`
> passes, `get_critical_pressure_and_mass_flux_subcooled_liquid_ph` is validated
> across the full x_t = 0.0 … 1.00 range including the saturated-liquid line,
> and the Moody `isobar_pref_*` tests are active and pass (region-filtered).
> Treat the table below as history; see the crate `CLAUDE.md` and `README.md`
> Changelog for the current status.

The multiphase choked flow solvers are works in progress. Summary for future
contributors:

| Function | Status |
|---|---|
| `get_critical_pressure_and_mass_flux_ph_vle_dome` | ✅ Validated — all 21 Zaloudek in-dome quality curves pass (x_t = 0.0 … 1.0; boundary quality curves skipped by region filter) |
| `get_critical_pressure_and_mass_flux_subcooled_liquid_ph` | ✅ Validated for interior curves — 20 genuinely-subcooled Zaloudek curves (x_t = 0.05 … 1.00) pass; x_t ≈ 0 bubble-point is the one failing fringe case. **Out of date (v0.2.0 snapshot): x_t ≈ 0 passes too as of v0.2.1, so all subcooled curves x_t = 0.0 … 1.00 are validated** |
| `get_critical_pressure_and_mass_flux_superheated_vapour_ph` | ✅ Validated — vapour-side mirror of the subcooled solver (dew point replaces bubble point). Zaloudek high-quality curves (x_t = 0.90/0.95/1.00) pass the tight 3% pressure / 5% log-G tolerance across the **full supercritical range** (x_t = 1.00 covers stagnation up to p₀ ≈ 29.5 MPa, choke pressure matched <0.01% at 3000 psia). x_t = 0.80 passes at a looser 5% pressure tolerance — its only vapour-side point that fails the 3% bound is the near-critical 3000-psia case (throat ≈ 0.94·p_crit, under the dome apex) where IF97 Region-3 backward equations lose digits |
| `get_critical_pressure_and_mass_flux_with_stagnation_props` | ❌ Superseded — old combined dispatcher with +25% artifact; retain for reference only |

The three split solvers (`…_ph_vle_dome`, `…_subcooled_liquid_ph`,
`…_superheated_vapour_ph`) together cover all three stagnation buckets relative
to the p-h VLE dome: inside (two-phase), outside-left (subcooled liquid / liquid-
like, `s0 < s_crit`), and outside-right (superheated vapour / supercritical
vapour-like, `s0 > s_crit`). The caller's dispatcher routes by `ph_flash_region`
plus the `s0` vs `s_crit` test.

**Overall Zaloudek HEM reference-curve validation status** (reminder: Zaloudek
curves are HEM-computed, not experimental — see note above): The HEM solvers are
validated across the interior of the two-phase dome, the high-pressure subcooled
tail, and the superheated-vapour / supercritical region (right of the dome). The
only unresolved case is x_t ≈ 0 (the saturated-liquid-line edge), which is a
fundamental physics limitation, not a code bug — HEM cannot reproduce that curve
without a non-equilibrium relaxation term. The mirror x_t ≈ 1 dew-point edge is
better behaved here (the x_t = 1.00 vapour curve passes), with only the
near-critical-point corner needing a relaxed tolerance.

> **Correction (v0.2.1):** the two sentences above about x_t ≈ 0 are wrong. It
> was a code bug (the energy-balance choke finder), not a physics limitation, and
> no relaxation term was needed — the x_t ≈ 0 curve passes.

**Near-bubble-point HEM artifact (x_t ≈ 0) — the v0.2.0 diagnosis, since
disproved:**
The test `outside_dome_stagnation_subcooled::quality_bubble_point_subcooled`
had its `#[ignore]` removed and was, at v0.2.0, failing. The v0.2.0 diagnosis
recorded here was that the root cause is fundamental: that HEM assumes
instantaneous equilibrium flashing at the bubble point, overpredicting mass flux
by 3–7× at 5–10 psia and placing the choke point 11–21% below the measured
throat at 15–200 psia, so an HRM (Homogeneous Relaxation Model) would be
required. **That diagnosis was wrong.** `diagnose_bubble_point_artifact` showed
HEM evaluated directly at the Zaloudek throat reproduces the curve to ±0.04 in
log10 G at every point; the discrepancy lived entirely in the forward choke
finder. See the resolution note earlier in this file and the comment block above
the test.

**Actively failing tests (v0.2.0 snapshot — none of these still fail):**
- `outside_dome_stagnation_subcooled::quality_bubble_point_subcooled` — x_t ≈ 0 bubble-point. **Fixed in v0.2.1 and passing as of 2026-08-11** (run output above); the diagnosis of a "HEM fundamental limitation" was wrong.

**Ignored tests (v0.2.0 snapshot — both entries are out of date):**
- `moody_critical_mass_flux_homogeneous_eqm::isobar_pref_*` — moody isobar
  tests (pre-existing `#[ignore]`). **No longer ignored**: all 13 Moody tests are
  active, and the file contains no `#[ignore]` at all.
- `generic_multiphase_stagnation::quality_*` — old combined-canary suite,
  superseded by the split in-dome / subcooled test files. **No longer ignored**:
  all 21 tests in that file are active and drive the unified dispatcher.

For the current ignored-test list, do not trust this snapshot — read it out of
the binary with `cargo test --release -p tampines-steam-tables --lib --
--ignored --list` (14 tests, 2026-08-11).

### Roadmap

**v0.3.0 (planned):**
- **Marviken integration tests** — `marviken_tests.rs` is an `#[ignore]`d stub
  with data loaded but assertions missing. The next step is to write the assertion
  block (comparing HEM mass flux to measured Marviken CFT-23/24 curves) and
  un-ignore the test.
- ~~**HRM at the saturated-liquid line**~~ — **dropped (2026-08-11).** This item
  existed because `quality_bubble_point_subcooled` was believed to fail on
  physics grounds. It does not fail; the v0.2.1 choke-finder fix made all
  x_t = 0.0 … 1.00 Zaloudek curves pass with plain HEM, and the Zaloudek
  reference is itself an HEM curve, so it could never have needed a relaxation
  term. Whether an HRM is warranted for *real* flashing flow is an open physics
  question that only experimental data (Marviken, above) can settle — it is not
  a v0.3.0 code deliverable and there is no failing test motivating it.

**Nice-to-have:** WASM build of the egui GUI for browser demos; full two-phase
property surface (currently only saturation + quality interpolation).

---

## OUTRAM PARK workspace notes

> This crate is now a member of the **OUTRAM PARK** workspace
> (`crates/tampines-steam-tables`). See the workspace root `CLAUDE.md` for the
> shared dependency policy and full migration history. Dependencies are inherited
> from the root `[workspace.dependencies]` — **do not** pin versions here
> (`uom.workspace = true`, etc.).

### Done: vestigial `ndarray-linalg` dep removed

**Status: complete (verified against `Cargo.toml`, 2026-08-11).** This was
recorded here as a planned clean-up: `ndarray-linalg` used to be listed in three
`[target.*.dependencies]` blocks (`cfg(windows)`, `cfg(macos)`, `cfg(unix)`)
despite never being imported anywhere in the source tree — the `.solve()` calls
under `src/openfoam_algorithms/` are all commented out.

The three lines are gone. This crate's `[dependencies]` are now exactly
`approx`, `ndarray`, `thiserror`, `uom`; `grep -n ndarray-linalg Cargo.toml`
matches only a prose comment. Nothing in `src/` or `examples/` references
`ndarray_linalg`.

**Consequence: this crate needs no system BLAS.** Do not tell users to install
OpenBLAS/`libopenblas-dev` to build or test it. `tuas_boussinesq_solver` (a
dev-dependency here, used only by the FHR-simulator examples) also dropped
`ndarray-linalg` at TUAS v0.1.2 and uses pure-Rust `peroxide` + `ndarray`, so
the transitive need is gone too. Workspace-wide, `outram-foam-basic-lib` is the
only remaining declarer, and only as a target-gated dev-dependency.

### Migration notes (2026-06)

- Moved into the workspace; standalone git history dropped.
- `tuas_boussinesq_solver` now resolves to the **in-tree** crate
  (`tuas_boussinesq_solver.workspace = true`, a path dep) instead of crates.io
  `0.0.10`; dev-deps (`teh-o-prke`, `chem-eng…`, egui stack) are likewise in-tree.
- Bumped to latest stable: `uom` 0.36→0.38, `ndarray` 0.15→0.17,
  `ndarray-linalg` 0.16→0.18, `thiserror` 1→2, egui/eframe 0.29→0.34,
  `egui_plot`→0.35. The **library and test suite compile cleanly** on these
  versions with no source changes.
- ✅ **All egui examples migrated to egui 0.34** (build & link). `fhr_sim_v1` and
  `fhr_sim_v2` needed the standard two fixes: `eframe::App::update` →
  `ui(&mut self, ui, frame)` with `let ctx = ui.ctx();` in `app/mod.rs`, and
  `egui_plot::Line::new(points).name(s)` → `Line::new(s, points)` in
  `app/graph_pages/mod.rs`. `depressurisation` and `transient_rankine_cycle`
  required no changes (they don't touch the changed egui/egui_plot APIs).

### Resolved: fhr_sim_v2 UI not registering backend state changes

✅ **Status (2026-07-13):** Fixed by the user outside an assistant session.
Previously the UI did not reflect real-time updates from the thermal-
hydraulics backend — the simulator ran but plots and widgets stayed static
despite backend calculations progressing. Tracked as `op-21g.4`, now closed.

**Investigation (historical):** Cross-reference with the pre-migration
fhr_sim_v2 (`../../../tampines-steam-tables/examples/fhr_sim_v2/`, egui 0.29
version) showed only **3 files differed** (the egui API changes above), and
all 23 other files in the `app/` tree were byte-identical. The thermal-
hydraulics backend (`app/thermal_hydraulics_backend/*`), reactor physics
(`app/prke_backend/*`), and widget logic (`app/local_widgets_and_buttons/*`)
were unchanged, ruling out migration-induced breakage — the root cause was a
logic issue in the state-update / data-binding pipeline, not the egui
0.29 → 0.34 port.

## Correction log — flashing choice for `TampinesSteamArray` (2026-07-14)

A record, per the maintainer's request, of an assistant (Opus) correction
made while wiring `TampinesSteamArray` into `fhr_sim_v2`'s steam-generator
tube. The mistake is easy to repeat, so it is written down here.

**The mistake.** When driving / reading the array and its neighbours, it is
tempting to reach for the `(T, p)` **single-phase** flashes
(`pt_flash_eqm::{h_tp_eqm_single_phase, v_tp_eqm_single_phase, ...}`). These
`panic!`/`todo!()` the moment a state is two-phase, because at saturation `T`
and `p` are not independent. A steam-generator tube boils feedwater, so it
spends most of its length two-phase — and even a **single-phase** subcooled
liquid transient can momentarily produce a state a `(T, p)` flash cannot
classify.

**The rule.** For `TampinesSteamArray` (and `OPCPFluidArray`) use **`(p, h)`
flashing by default** — the array's native state is `(p, he)`, and the
`(p, h)` flashes carry the phase/quality information internally, so they stay
defined across the saturation dome. Drive the array with
`set_inlet_enthalpy` / read it with `get_outlet_enthalpy` (both `(p, h)`), not
by round-tripping through temperature. `set_temperature_vector` is a
`(T, p)` convenience for a **known subcooled** initial condition only.

**Two concrete traps this surfaced (both fixed):**

1. *Pressure floor on the saturation line.* The default pressure-bounding
   floor was `sat_pressure_4(273.15 K)` **exactly**. A cell clamped to that
   value lands on the saturation line, and the `(p, h)` validity guard
   (`is_below_isotherm_t_273_15`) classifies its 273.15 K isotherm with a
   `(T, p)` single-phase flash whose Region-4 test is exact float equality
   (`pres == p_sat`), so it `todo!()`-panicked. Fix: default the floor to
   `sat_pressure_4(273.15 K) * 1.001`, just inside Region 1.

2. *Initial-vs-operating pressure mismatch.* Pre-initialising the tube at
   2 bar while the runtime outlet BC was 1.2 bar caused a depressurisation
   rarefaction on the first driven step that cooled a cell below the
   273.15 K floor. Fix: pre-initialise at the operating pressure.

**Takeaway for future array work:** prefer `(p, h)`; treat any `(T, p)`
single-phase call near saturation as a latent panic; and when pre-conditioning
a stiff-liquid array, match the initial pressure/velocity to the operating
boundary conditions so the first step is not a violent transient.

## 2026-07-15 — `(p,h)` flash: `p_sat(273.15 K)` trap fixed at the root, and Region 5 made explicitly unsupported

Follow-up to the array note above. Two `(p,h)`-flash defects were addressed
directly in the flash code (not just worked around in the array driver).

**1. The `p == p_sat(273.15 K)` trap — root-cause fix.**
`ph_flash_region` opens with `check_if_within_ph_validity_region`, whose
lower-isotherm guard `is_below_isotherm_t_273_15` used to compute the 273.15 K
isotherm enthalpy via the single-phase `(T,p)` flash
`h_tp_eqm_single_phase(273.15 K, p)`. The `(T,p)` router classifies a point as
**Region 4** whenever `pres == p_sat(T)` (exact float equality), and the
Region 4 `(T,p)` arm was `todo!()`. At `p == p_sat(273.15 K) = 611.213 Pa` —
the lower pressure limit of the whole `(p,h)` domain — *every* `(p,h)` flash
panicked before it could classify the point, regardless of enthalpy.
**Fix:** the lower isotherm is now evaluated with the Region 1 forward equation
`h_tp_1(273.15 K, p)` directly (the entire 273.15 K isotherm over
`[p_sat(273.15 K), 100 MPa]` is Region 1), side-stepping the router's
saturation-line degeneracy. The two-phase point then routes normally through
the Region 4 `(p,h)` path, which carries steam quality via `x_ph_flash`. The
array driver's `*1.001` pressure floor is now a belt-and-braces margin rather
than the sole guard. Verified by
`ph_flash_region4_edge_and_region5::ph_flash_at_exact_psat_273_15_does_not_panic`
(round-trip at exactly `sat_pressure_4(273.15)`; recovers T = 273.15 K, x = 0.30,
and mixture v/s vs the 0 °C International Steam Tables row to < 1e-4).

**2. Region 4 `(T,p)` mixture properties now report an explicit error.**
The `..._tp_eqm_single_phase` functions' Region 4 arms changed from `todo!()`
to an explicit `panic!` (`REGION_4_TP_UNDERDETERMINED`): a two-phase `(T,p)`
state is genuinely under-determined without quality — a thermodynamic fact, not
an unfinished path. Callers needing a two-phase state must use a `(p,h)`/`(p,s)`
flash (which carry quality) or `w_tpx_eqm`.

**3. Region 5 `(p,h)` is deliberately unsupported.**
IAPWS-IF97 provides **no backward `(p,h)` correlation for Region 5** (steam
1073.15–2273.15 K); the released backward equations cover Regions 1–3 only
(Wagner & Kretzschmar, *International Steam Tables*, 2019). This crate does not
fabricate a numerical inversion. A Region 5 `(p,h)` input is rejected with an
explicit, documented "unsupported" message (`REGION_5_PH_UNSUPPORTED`), not a
`todo!()`. If the temperature is known, use the Region 5 forward `(T,p)`
equations (`h_tp_5`, `v_tp_5`, `s_tp_5`, ...). Verified by
`region_5_ph_flash_is_explicitly_unsupported`.

**Sharp edge left in place (pre-existing, out of scope):** the `(T,p)` router's
Region 4 detection is an exact float equality `pres == p_sat(T)`, which under
release-mode FMA contraction can miss by a ULP depending on the call site (a
compile-time-folded `p` may disagree with the runtime recompute inside the
flash). The `(p,h)` path no longer depends on it. The companion test
`region_4_tp_forward_flash_is_explicitly_under_determined` uses a
`black_box`-ed temperature and a round-tripped pascal pressure to land the
Region 4 arm deterministically.
