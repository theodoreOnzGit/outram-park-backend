# Review manifest — op-6tz TRISO finish (random packing + delta-tracking k∞)

**⚠️ AI-GENERATED DRAFT — HUMAN REVIEW REQUIRED per `RESPONSIBLE_USE.md`.**

This change is untrusted AI-authored draft material until a human reviews it. It
implements random TRISO packing (op-6tz.25) and assembles the full random-packed
doubly-heterogeneous k∞ driven by delta (Woodcock) tracking (op-6tz.16), and
documents delta tracking in the triso example. Scope: **only**
`crates/outram-mc-libs/`.

Date: 2026-07-15. Data tier: embedded LOW (`Nuclide::from_core`, WMP ENDF/B-VII.1
+ fast MGXS). Branch: `develop` (isolated worktree).

## Files changed

| File | Change |
|---|---|
| `src/pebble_beds/stochastic_media.rs` | **Implemented** `pack_spheres` (RSA) + `PackedSpheres` (spatial-hash membership). Was a stub returning `NotImplemented`. |
| `src/pebble_beds/delta_tracking.rs` | **Added** `Majorant::bounding` — a bin-maximum majorant that provably bounds Σ_t across resonances. Existing `Majorant`/`track_to_collision` behaviour unchanged. |
| `src/pebble_beds/keff_delta.rs` | **New module** — `run_keff_delta`: fission-source power iteration over a reflective cube, every history streamed by delta tracking. Reflective-cube ray reflection + analog collision partition mirrored from `keff.rs`. |
| `src/pebble_beds/mod.rs` | Added `keff_delta` module; updated the module map doc. |
| `src/prelude.rs` | Re-exported `pack_spheres`, `PackedSpheres`, `PackingConfig`, `PackingMethod`, `Majorant`, `track_to_collision`, `DeltaEvent`, `DeltaFlight`, `run_keff_delta`. |
| `tests/openmc_notebooks/triso.rs` | Rewrote to random packing + delta-tracking k∞; added the delta-tracking `//!` explanation and V&V methodology/results. 5 LIVE tests. |
| `tests/openmc_notebooks.rs` | Marked `triso` LIVE in the module map. |
| `examples/triso_delta_tracking.rs` | **New** runnable, documented walkthrough of delta tracking. |
| `verification_and_validation/openmc_notebook_comparisons/triso.csv` | **New** comparison CSV (GITIGNORED — not committed). |

## Provenance

- **Notebook**: `triso.ipynb`, openmc-notebooks @ `cf1e5db2cd77d53a4fa76ffd9af7ab638f468713` (MIT).
  Fetched and analyzed this session. **The notebook runs `run_mode='plot'` only —
  it computes no k-eigenvalue and prints no reference k-eff.** Target packing
  fraction 0.30, 1 cm³ reflective box, `pack_spheres(radius=42.5e-4, pf=0.30, seed=124848351)`.
- **RSA packing**: ported from OpenMC (MIT) `openmc/model/triso.py` —
  `_random_sequential_pack` (line 882), `_RectangularPrism` container (line 253),
  `pack_spheres` (line 1210), `MAX_PF_RSP=0.38` (line 20). Cited in the source doc
  comments.
- **Delta tracking**: standard method (Woodcock, ANL-7050, 1965); collision
  partition mirrors OpenMC `src/physics.cpp`. The reflective-cube k∞ assembly and
  the bin-max majorant are new pebble-bed work built on the existing primitives.

## Assumptions / simplifications (honest)

1. **No notebook reference k-eff exists.** The doubly-het k∞ is therefore
   **correctness-asserted, not validated** against a benchmark. Do not cite it as a
   validated result.
2. **Single fuel-kernel sphere** — buffer/IPyC/SiC/OPyC shells collapsed into the
   matrix (fidelity simplification, not a transport limitation).
3. **Fast/epithermal LOW-tier data**, no graphite S(α,β) — a fast HEU/H
   demonstrator, not a thermal pebble-bed benchmark.
4. **Kernel radius 0.04 cm**, a tractable stand-in for the notebook's 42.5 µm
   particle (whose 0.30 packing needs ~9.3×10⁵ spheres). Packing-fraction
   correctness is scale-independent; k∞ (infinite medium) depends on volume
   fractions, not absolute size.
5. **RSA jams above ~0.30–0.31** in a finite box (below the 0.38 asymptote) — a
   known RSA property. Higher pebble-bed packing fractions need CRP (Jodrey–Tory)
   or the RSA–DEM/ODR–DEM methods, which are **not** ported (follow-up bead).
6. **Reflective (infinite-medium) geometry** — the eigenvalue is k∞, leakage-free.
7. **Existing MAX_EVENTS band-aid** reused unchanged in the new driver (per the
   instruction not to add more silently). The nested-lattice *surface* tracker is
   known to lose histories in this geometry (see below); the new delta path does
   not rely on it.

## What is now DONE vs still partial

**DONE**
- op-6tz.25 random TRISO packing: `pack_spheres` (RSA) + `PackedSpheres`, with
  overlap-free / contained / target-fraction / reproducibility tests. Verified.
- op-6tz.16 full doubly-het k∞ by delta tracking: assembled, LIVE test asserts a
  real k∞ ± σ. Delta tracking proven unbiased vs surface tracking on identical
  geometry.
- Delta-tracking documented in the example (`examples/triso_delta_tracking.rs`),
  the test module `//!`, and the `delta_tracking`/`keff_delta` module `//!` docs.

**STILL PARTIAL / follow-ups** (beads filed)
- High-packing-fraction methods (CRP / RSA–DEM / ODR–DEM) for pf > ~0.31.
- Full TRISO shell stack (buffer/IPyC/SiC/OPyC) as nested layers.
- Thermal data (graphite S(α,β)) for a thermal pebble-bed benchmark.
- The nested-lattice *surface* tracker under-counts histories in the reflective
  TRISO lattice (root cause of the ~0.90 vs ~1.93 k gap on that geometry) — a
  pre-existing issue that motivates delta tracking; worth a dedicated bead.

## Build / test output (release)

```
cargo test -p outram-mc-libs --lib --tests --release
  lib:   test result: ok. 50 passed; 0 failed; 0 ignored     (44 pre-existing + 6 new)
  tests: test result: ok. 12 passed; 0 failed; 17 ignored     (5 triso LIVE + others)
```

triso live output:
```
test triso::triso_nested_lattice_geometry_navigation ... ok
test triso::triso_random_packing_is_valid ... ok
test triso::triso_delta_flight_reaches_collision_in_packed_medium ... ok
[unbiasedness] surface k = 2.22081 ± 0.00454 | delta k = 2.21925 ± 0.00320
test triso::triso_delta_tracking_unbiased_vs_surface_tracking ... ok
[triso random-packed doubly-heterogeneous] k∞ = 1.87105 ± 0.00615 over 45 generations
test triso::triso_random_packed_doubly_heterogeneous_keff ... ok
```

## Measured numbers

| Quantity | Value | Note |
|---|---|---|
| Random packing (target pf 0.30) | realized **0.3000**, N=1119, 0 overlaps, reproducible | op-6tz.25 |
| Delta-tracking unbiasedness (homogeneous U235 reflective cube) | surface **2.22081 ± 0.00454** vs delta **2.21925 ± 0.00320** → **0.28σ** | identical geometry; proves unbiasedness |
| Doubly-het k∞ (random pf-0.30 HEU/H) | **1.87105 ± 0.00615** (45 gens) | correctness-asserted, **no reference** |
| Example (100 gens) | k∞ **1.87853 ± 0.00233** | consistency cross-check |

## CSV comparison

`verification_and_validation/openmc_notebook_comparisons/triso.csv` (gitignored).
Because the notebook has **no reference k-eff**, the CSV records: (1) the packing
fraction vs the notebook target 0.30 (realized 0.3000); (2) the delta-vs-surface
unbiasedness cross-check (0.28σ, internal — not a notebook reference); (3) the
headline k∞ marked `no_reference_correctness_asserted`.

## Top human-verify asks

1. **Packing-fraction correctness** — confirm the RSA port (mesh overlap search,
   `MAX_PF_RSA` gate, contained-in-domain limits) matches OpenMC and that
   "no overlap + realized pf = target" is the right correctness bar.
2. **Delta-tracking unbiasedness** — confirm the surface-vs-delta agreement on
   identical geometry (0.28σ) is an acceptable unbiasedness proof, and that
   `Majorant::bounding` (bin-max, verified 0 under-bounds over 2×10⁶ probes) is a
   sound way to guarantee Σ_maj ≥ Σ_t across resonances.
3. **k∞ interpretation** — confirm reporting 1.87 as correctness-asserted (not
   validated) is the honest framing given the notebook has no reference.
4. **Reflective-cube ray reflection** in `keff_delta::advance_reflective` — check
   the per-axis wall reflection and clamping are correct and path-length-conserving.
