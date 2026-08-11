# tampines-steam-tables — choked-flow status, version history & workspace notes

Reference material: the detailed multiphase choked-flow (HEM) current-work
status, the v0.2.0 status table and roadmap, and OUTRAM PARK workspace/migration
notes. Consulted on demand, not per turn. Mandatory conventions/guardrails and
the crate overview live in CLAUDE.md.

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
> and pass** (only `isobar_pref_0_25` remains `#[ignore]`d, for a documented
> isentrope-vs-Bernoulli divergence at p_bubble/p₀ ≈ 0.02). The solvers are now
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
  mass-flux (G) tolerances loose; the bubble-point edge (x_t ≈ 0) is a known
  HEM physics limitation, not a validation target.
- Marviken critical flow tests — `marviken_tests.rs`. **NOT validated**:
  the digitised NUREG/CR-2671 data is present, but the test is `#[ignore]`d,
  its assertion is commented out, and it ends in `todo!()` (see the Roadmap
  section below and bead `op-21g.16`).

### Current effort: near-bubble-point HEM artifact

We are trying to solve the **near-bubble-point HEM artifact** that breaks the
Zaloudek VLE critical-pressure / mass-flux tests.

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

The **active failing test** is
`outside_dome_stagnation_subcooled::quality_bubble_point_subcooled`
(x_t = 1e-4, throats essentially on the saturated-liquid line). The `#[ignore]`
has been removed — this is the current work item. The detailed three-failure-mode
writeup lives in the comment block directly above that test; in short, HEM cannot
reproduce the x≈0 choking line in both mass flux and pressure (mass-flux artifact
at 5/10 psia, 11–21% choke-pressure error at 15–200 psia, in both solver
branches) and a non-equilibrium / relaxation model is required.

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
- **HRM is required at the dome boundaries.** HEM assumes instantaneous phase
  equilibrium, which breaks down where nucleation or droplet-formation lags behind
  the local pressure drop:
  - x_t ≈ 0 (bubble point / saturated liquid): flashing lags → HEM overpredicts G
    and underpredicts choke pressure in the 5–200 psia range (see failing canary).
  - x_t ≈ 1 (dew point / saturated vapour): droplet condensation lags → HEM is
    similarly unreliable near the right-hand dome boundary.
  For interior qualities (0.05 ≤ x_t ≤ 0.95) the equilibrium assumption holds
  well and HEM is sufficient.

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
| `get_critical_pressure_and_mass_flux_subcooled_liquid_ph` | ✅ Validated for interior curves — 20 genuinely-subcooled Zaloudek curves (x_t = 0.05 … 1.00) pass; x_t ≈ 0 bubble-point is the one failing fringe case |
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

**Near-bubble-point HEM artifact (x_t ≈ 0):**
The test `outside_dome_stagnation_subcooled::quality_bubble_point_subcooled`
has its `#[ignore]` removed and is actively failing. Root cause is fundamental:
HEM assumes instantaneous equilibrium flashing at the bubble point, which
overpredicts mass flux by 3–7× at 5–10 psia and places the choke point 11–21%
below the measured throat at 15–200 psia. An HRM (Homogeneous Relaxation Model)
is required to reproduce the x ≈ 0 Zaloudek curve. See the long comment block
above that test for the full three-failure-mode analysis.

**Actively failing tests:**
- `outside_dome_stagnation_subcooled::quality_bubble_point_subcooled` — x_t ≈ 0 bubble-point; HEM fundamental limitation at the saturated-liquid-line boundary

**Ignored tests:**
- `moody_critical_mass_flux_homogeneous_eqm::isobar_pref_*` — moody isobar
  tests (pre-existing `#[ignore]`)
- `generic_multiphase_stagnation::quality_*` — old combined-canary suite,
  superseded by the split in-dome / subcooled test files

### Roadmap

**v0.3.0 (planned):**
- **Marviken integration tests** — `marviken_tests.rs` is an `#[ignore]`d stub
  with data loaded but assertions missing. The next step is to write the assertion
  block (comparing HEM mass flux to measured Marviken CFT-23/24 curves) and
  un-ignore the test.
- **HRM at the saturated-liquid line** — HEM is validated for the interior of the
  two-phase dome (x_t = 0.05 … 0.95), the high-pressure subcooled tail, and the
  superheated-vapour / supercritical region, but breaks down near the saturated-
  liquid line (x_t ≈ 0) where thermal non-equilibrium governs the choke. An
  HRM-style relaxation model (e.g. Feburie, or Henry–Fauske) is required for that
  boundary curve. `quality_bubble_point_subcooled` is the primary canary. (The
  mirror x_t ≈ 1 dew-point edge turned out to be better behaved — the x_t = 1.00
  vapour curve passes with the energy-balance max-G solver — so only the
  near-critical-point corner of the vapour side still wants a relaxation term.)

**Nice-to-have:** WASM build of the egui GUI for browser demos; full two-phase
property surface (currently only saturation + quality interpolation).

---

## OUTRAM PARK workspace notes

> This crate is now a member of the **OUTRAM PARK** workspace
> (`crates/tampines-steam-tables`). See the workspace root `CLAUDE.md` for the
> shared dependency policy and full migration history. Dependencies are inherited
> from the root `[workspace.dependencies]` — **do not** pin versions here
> (`uom.workspace = true`, etc.).

### Planned: remove vestigial `ndarray-linalg` dep

`ndarray-linalg` is listed in all three `[target.*.dependencies]` blocks in
`Cargo.toml` but is **never imported anywhere in the source tree**. The
`.solve()` calls under `src/openfoam_algorithms/` are all commented out. This
is a vestigial entry — no code changes are needed to remove it.

**To clean up:** delete the three `[target.*.dependencies]` `ndarray-linalg`
lines from `Cargo.toml` (`cfg(windows)`, `cfg(macos)`, `cfg(unix)`). Run
`cargo check -p tampines-steam-tables --lib` to confirm nothing breaks.

Note: `tuas_boussinesq_solver` (a runtime dep of tampines) used to pull in
`ndarray-linalg` transitively, which is likely why this was listed explicitly.
Since TUAS v0.1.2 no longer uses `ndarray-linalg`, the transitive need is also
gone.

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
