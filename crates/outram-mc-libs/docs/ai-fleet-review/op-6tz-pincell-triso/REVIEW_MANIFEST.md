# Review manifest — pincell + triso notebook tests (op-6tz.7/.8/.9/.10/.16)

<!-- op-jis-historical-note -->
> ⚠️ **HISTORICAL RECORD — the statistics below predate `op-jis` (noted 2026-08-06).**
> Every measured number in this manifest was produced **before** bead `op-jis`
> added OpenMC's PCG-RXS-M-XS output permutation to `rng::lcg::prn` on
> 2026-08-06. The LCG **state recurrence was not changed**, so integer-state
> facts still hold, but every statistic derived from the sampled **uniform
> values** — k values and their σ, tallies, fractions, σ-distances — **no longer
> reflects the current generator**. This is a dated review record, so its numbers
> are deliberately **left exactly as they were measured** and are *not* rewritten
> here. Do not cite them as current; current values live in the crate's V&V docs
> and test doc comments.

**⚠️ AI-GENERATED DRAFT — HUMAN REVIEW REQUIRED per `RESPONSIBLE_USE.md`.**

This change was produced by an AI coding agent (Claude Opus). It is **untrusted
draft material** until a human reviews the transport correctness, the ported
geometry logic against the OpenMC C++ source, and the V&V claims. Nothing here
is validated for any operational or safety-relevant purpose (see
`RESPONSIBLE_USE.md` intended-use scope).

- **Date:** 2026-07-15 (Asia/Singapore working hours)
- **Crate touched:** `crates/outram-mc-libs/**` only. No other crate modified;
  `njoy-outram-park-fork` was read for the data assessment but **not** changed.
- **Beads:** epic `op-6tz`; children `op-6tz.7` (geometry nav), `.8` (transport
  loop), `.9` (tally scoring), `.10` (rect-lattice/reflective k-eff), `.16`
  (TRISO), `.12` (S(α,β) — still open).

---

## 1. What was wired

The bare-sphere `run_keff` was the only end-to-end path before this change; the
CSG geometry (`Cell::contains`, `Universe::find_cell`,
`RectLattice::get_indices`, `Geometry::locate`/`distance_to_boundary`) were all
`todo!()` stubs. This change implements them and builds a general
surface-tracking k-eigenvalue + tally on top, then makes the `pincell` and
`triso` notebook tests LIVE against it.

### New capability (op-6tz.7 — geometry navigation, foundation)
- `ZCylinder::distance` — ported from OpenMC `axis_aligned_cylinder_distance`
  (`src/surface.cpp:401`).
- `SurfaceKind` enum — dispatch over the concrete surfaces **without trait
  objects** (workspace design rule); the `Surface` trait stays as the
  compiler-enforced contract.
- `Cell::contains` — RPN boolean region evaluator (generalised from
  `Region::contains_simple`, `src/cell.cpp:987`); `Cell::distance_to_boundary`
  (from `Region::distance`, `src/cell.cpp:947`).
- `Universe::find_cell` — first containing cell (`src/universe.cpp:40`).
- `RectLattice::{get_indices, are_valid_indices, universe_at,
  get_local_position, distance}` — ported from `src/lattice.cpp:236–355`,
  including the coincidence-by-direction index rule.
- `Geometry::{locate, distance_to_boundary, cross_surface, sigma_t_at}` —
  nested universe/lattice descent (`find_cell_inner`, `src/geometry.cpp:102`)
  and multi-level nearest-boundary selection (`distance_to_boundary`,
  `src/geometry.cpp:361`), plus vacuum/reflective BC handling.

### New capability (op-6tz.8/.10 — transport loop)
- `physics/transport_csg.rs` `run_keff_csg` — surface-tracking analog
  k-eigenvalue power iteration over any `Geometry`, with per-surface reflective /
  vacuum boundary conditions. Structure ported from
  `transport_history_based` + `distance_to_boundary` (`src/physics.cpp`,
  `src/geometry.cpp`); the collision reaction partition is the same as
  `keff.rs`. A `MAX_EVENTS` safety cap (100k events/history) prevents a
  pathological reflective-medium history from hanging (see §5 — human-verify).

### New capability (op-6tz.9 — tally scoring)
- `tally/scoring.rs` `score_collision` — collision-estimator scoring through the
  existing `Filter` conjunction (Cell/Material/Energy/Universe). Flux `w/Σ_t`,
  reaction rates `w·Σ_x/Σ_t`, total `w`. Ported from the collision branch of
  `score_general` (`src/tallies/tally_scoring.cpp`).

### Files changed
| File | Change |
|---|---|
| `src/geometry/surface.rs` | +`ZCylinder::distance`, +`SurfaceKind` enum dispatch |
| `src/geometry/cell.rs` | RPN `contains`, `distance_to_boundary`, `translation`, ctors |
| `src/geometry/universe.rs` | `find_cell` |
| `src/geometry/lattice.rs` | rect-lattice indexing/local-pos/distance; hex left as stub |
| `src/geometry/geometry.rs` | `Geometry`, `locate`, `distance_to_boundary`, `cross_surface`, unit tests |
| `src/physics/transport_csg.rs` | **new** — `run_keff_csg`, `SourceBox` |
| `src/physics/mod.rs` | register `transport_csg` |
| `src/tally/scoring.rs` | `score_collision` + unit test |
| `src/prelude.rs` | export the new geometry/transport surface |
| `tests/openmc_notebooks/pincell.rs` | 4 LIVE + 1 honest-ignore |
| `tests/openmc_notebooks/triso.rs` | 3 LIVE |

---

## 2. LIVE vs IGNORED (honest status)

### pincell — 4 LIVE, 1 IGNORED
| Test | Status | What it verifies |
|---|---|---|
| `pincell_criticality_eigenvalue_via_godiva_bare_sphere` | LIVE | bare-sphere k-eff (unchanged) |
| `pincell_leakage_reduces_reactivity` | LIVE | leakage sign (unchanged) |
| `pincell_reflective_cell_suppresses_leakage` | **LIVE (new)** | CSG reflective infinite medium: k_inf ≫ leaky sphere |
| `pincell_heterogeneous_csg_with_cell_flux_tally` | **LIVE (new)** | heterogeneous CSG transport + cell-flux tally |
| `pincell_lwr_thermal_pin_benchmark` | IGNORED | true thermal LWR pin — blocked on S(α,β) (op-6tz.12) |

### triso — 3 LIVE (was fully ignored)
| Test | Status | What it verifies |
|---|---|---|
| `triso_nested_lattice_geometry_navigation` | **LIVE (new)** | root→lattice-tile→TRISO-universe descent, material at kernel/matrix |
| `triso_delta_tracking_through_packed_medium` | **LIVE (new)** | Woodcock tracking over the nested-lattice `sigma_t_at` lookup |
| `triso_doubly_heterogeneous_keff` | **LIVE (new)** | assembled doubly-heterogeneous k-eff over the TRISO lattice |

**Why the thermal pincell stays IGNORED, not faked:** the notebook's pin is a
*thermal* LWR lattice whose physics depends on H-in-H₂O S(α,β). Without it the
water moderator cannot thermalize correctly, so a benchmark-accurate thermal
k_eff cannot be produced honestly. It remains `#[ignore]`d with an
`unimplemented!()` body (fails loudly if the ignore is removed before op-6tz.12).

---

## 3. Measured results (2026-07-15, this harness)

```
cargo test -p outram-mc-libs --lib --release           → 37 passed, 0 failed
cargo test -p outram-mc-libs --test openmc_notebooks --release
                                                       → 7 passed, 19 ignored, 0 failed (13.5 s)
```

k-eff numbers (embedded LOW-tier data, analog transport):

| Model | k ± σ | Interpretation |
|---|---|---|
| pincell reflective infinite medium (homogeneous HEU) | **k_inf = 2.20421 ± 0.00316** | HEU fast infinite medium; physically sensible (~2.2) |
| …vs same-material bare sphere (r = 1 cm) | k = 0.11925 ± 0.00247 | tiny sphere, leakage-dominated |
| triso doubly-heterogeneous (3³ HEU-in-H lattice, ~38 % packing) | **k = 0.89504 ± 0.00745** (45 gens) | converged, stationary, subcritical |
| Godiva bare sphere (regression) | k ≈ 1.01 ± 0.002 | unchanged from `keff.rs` |

The reflective jump **0.119 → 2.204** is the headline verification: the CSG
reflective-BC path removes all leakage exactly as expected. These are asserted
at run time (broad plausibility bands), not hard-coded literals.

### V&V methodology (per the mandatory V&V doc rule)
- **pincell reflective:** homogeneous-HEU square cell, reflective x/y planes,
  infinite in z ⇒ zero leakage ⇒ k_inf. Pass = stationary **and** strictly above
  the finite bare sphere of the same material. Reference concept: an infinite
  medium has no leakage term, so k_inf > k_eff(finite) always.
- **triso k-eff:** 3³ reflective lattice of HEU kernels (r 0.18 cm, pitch 0.4 cm)
  in H-1 matrix, surface-tracked through the nested universe/lattice. Pass =
  converged, positive, broad plausibility band; stationarity checked when all
  generations ran. No public benchmark asserted (regular packing + single-sphere
  kernel + fast data are simplifications — see §4).

---

## 4. Assumptions & known limitations (do not present as benchmark-grade)

1. **Fast/epithermal data only** (`Nuclide::from_core`: WMP + Watt-collapsed fast
   MGXS). No S(α,β) thermal scattering ⇒ thermal-spectrum systems are not
   benchmark-accurate. This is *the* reason the thermal pincell is ignored.
2. **triso uses a regular lattice, not random packing.** `pack_spheres`
   (`pebble_beds::stochastic_media`) is still a stub, so kernels sit on a regular
   3-D grid (one per tile). Honest `create_triso_lattice`-style arrangement.
3. **triso kernel is a single sphere** — buffer/IPyC/SiC/OPyC shells collapsed
   into the matrix. A fidelity simplification.
4. **Collision estimator, not track-length.** Flux = `w/Σ_t` per collision.
   Absorption score approximated as `Σ_t − Σ_s` over `Σ_t` because `MacroXs`
   carries no explicit Σ_a column yet.
5. **`MAX_EVENTS` cap (100k/history).** A guard, not a physics model — a history
   that would otherwise spin (see §5) is leaked. Needs root-causing.
6. **Nested frames are pure translations, no rotation.** `cross_surface`
   reflects in the global frame, valid because reflective surfaces are at the
   root level with identity transform in these models.
7. **White/Periodic BCs are approximated as reflective** in `cross_surface`
   (documented in-code); only Vacuum/Reflective/Transmissive are exercised.

---

## 5. Human-verify list (top ask: transport correctness)

1. **Transport correctness (highest priority).** Independently confirm
   `run_keff_csg` is unbiased: the reflective-infinite-medium k_inf should equal
   the k_inf of a homogeneous infinite medium of the same material computed by an
   independent method; and the triso k should be reproducible / mesh-independent.
   Check the collision/boundary ordering and the reflective reflection sign.
2. **`MAX_EVENTS` root cause.** Before the guard, the reflective run hung (a
   single history looped ~9 CPU-min). Find the geometry/nudge corner case that
   stalls (suspect: a history stuck near a reflective plane/corner or a
   coincident-surface re-cross) and fix it properly rather than relying on the
   cap. The cap currently leaks such histories, a small bias.
3. **Ported geometry vs OpenMC.** Diff `Geometry::locate` /
   `distance_to_boundary` / `RectLattice::*` against the cited `src/*.cpp` lines,
   especially the lattice coincidence-index rule and the multi-level FP tie-break.
4. **Tally normalisation.** The collision-estimator flux is un-normalised
   (per source particle, not per unit volume/source). Confirm this matches the
   intended tally semantics before using absolute numbers.
5. **Absorption score.** Replace the `Σ_t − Σ_s` approximation with a real Σ_a
   once `MacroXs` carries an absorption column.

---

## 6. njoy data wiring needed (REQUIRED assessment — for the user)

**Question:** what nuclear data do pincell + triso need from
`njoy-outram-park-fork`, what exists, and what is missing?

### What the tests use today (works, offline)
- **CE/group cross sections** for U-234/235/238 and H-1 via
  `Nuclide::from_core` → `WmpLibrary::core()` (embedded 125-nuclide CORE WMP
  blob) + `MgxsLibrary::core()` fast MGXS. This is the **LOW tier** and is fully
  wired and sufficient for the fast/epithermal LIVE tests. HEU (Godiva
  densities) and an H matrix both resolve.

### What is MISSING for the true thermal pincell (op-6tz.12)
- **H-in-H₂O S(α,β) thermal scattering.** This is the single blocker for a
  benchmark-accurate thermal LWR pin-cell.
- **njoy status — the physics IS ported, the consumer path is NOT:**
  - `njoy_outram_park_fork::thermr` — MF=7 read (`mf7`), coherent /
    incoherent-elastic / incoherent-inelastic S(α,β) processing are all marked
    **done** (typed module API), producing σ(E,T), σ(E→E′) and the equiprobable
    emission table.
  - `njoy_outram_park_fork::acer::thermal::thermal_from_mf7` — the thermal ACE
    writer is **done** for IFENG=0 (equiprobable), coherent + incoherent elastic.
  - `njoy_outram_park_fork::thermr::run()` (the card-input driver) returns
    `NjoyError::NotPorted` — but the module's typed API is usable directly.
  - **Gaps in njoy:** IFENG=1/2 (skewed/continuous inelastic) and multi-scatterer
    mixing (`nmix > 1`) are not ported; there is no `tsl-*` (MF=7) acquisition
    path analogous to the CE `acquire` module.
- **outram-mc side status — no S(α,β) consumer at all:**
  - `src/material/thermal.rs` `ThermalScattering` is an empty stub (name only —
    no energy grid, no tables).
  - `Nuclide` has no field or method to hold/sample S(α,β); the transport loop
    (`keff.rs`, `transport_csg.rs`) has no thermal branch. The embedded LOW tier
    carries no thermal data.

### The specific njoy → outram-mc work required (for op-6tz.12)
1. **Acquire** an MF=7 `tsl-HinH2O` evaluation (open ENDF/B thermal sublibrary)
   and run `njoy_outram_park_fork::thermr` (`mf7` + `inelastic` +
   `incoherent_elastic`) to produce σ(E,T), the σ(E→E′) transfer, and the
   equiprobable emission table — using the typed API (the driver is `NotPorted`).
2. **Populate** outram-mc's `ThermalScattering` from those products (energy grid,
   inelastic emission CDF, elastic cosines/weights), attached to the moderator
   `Nuclide` (H-1 bound in H₂O).
3. **Branch the transport loop** below ~4 eV to sample the S(α,β) emission
   instead of free-gas elastic (mirror OpenMC `sample_secondary` thermal branch,
   `src/thermal.cpp`), in both `keff.rs` and `transport_csg.rs`.
4. Only then remove the `#[ignore]` on `pincell_lwr_thermal_pin_benchmark` and
   assert against a published LWR pin-cell k∞ (e.g. a BEAVRS/CASL or OpenMC
   regression value).

**Note:** all of steps 1–3 are `njoy` **and** `outram-mc` work; this session
touched neither the njoy crate nor the thermal path — it only assessed them, as
scoped. Random TRISO packing (`pack_spheres`) is a separate, non-data gap in
`pebble_beds::stochastic_media`.

---

## 7. Provenance

- **Notebooks:** `pincell.ipynb`, `triso.ipynb` from
  `github.com/openmc-dev/openmc-notebooks` @ `cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`
  (MIT). Cited in each test module header.
- **OpenMC C++ mirrored:** `src/surface.cpp`, `src/cell.cpp`, `src/universe.cpp`,
  `src/lattice.cpp`, `src/geometry.cpp`, `src/physics.cpp`,
  `src/tallies/tally_scoring.cpp` (OpenMC, MIT). Reference `file:line` cited in
  the Rust doc comments.
- **Data:** embedded CORE WMP + fast MGXS (open ENDF-derived), per the crate's
  `NUCLEAR_DATA.md`. No restricted/proprietary data used.
