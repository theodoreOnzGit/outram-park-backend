# REVIEW MANIFEST — op-6tz.11 hexagonal-lattice notebook LIVE

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

**⚠️ AI-GENERATED DRAFT — HUMAN REVIEW REQUIRED per RESPONSIBLE_USE.md**

This change was produced by an AI assistant (Claude, Opus 4.8). Under the
workspace responsible-use policy it is **untrusted draft material** until a human
has inspected it, checked licence provenance, re-run the tests, and verified the
physics/geometry against the reference. Treat every claim below as *to be
verified*, not *verified*.

- **Bead:** op-6tz.11 (make the `hexagonal-lattice` notebook test LIVE)
- **Date:** 2026-07-15
- **Crate touched (only):** `crates/outram-mc-libs/`
- **Reference C++ mirrored:** OpenMC `HexLattice`,
  `/home/teddy0/Documents/research/openmc/src/lattice.cpp` (MIT).
- **Reference notebook:** `hexagonal-lattice.ipynb`, openmc-notebooks
  @ `cf1e5db2cd77d53a4fa76ffd9af7ab638f468713` (MIT).

---

## Headline outcome

The `hexagonal-lattice` notebook is a **geometry demonstration** — it builds a
`HexLattice` and *plots* it, but its own final markdown cell says that to
simulate one *would* create `openmc.Settings` and run; **it never runs a
k-eigenvalue**, and there is **no reference k in its cell outputs**. Per the
outram-mc V&V rule ("reference values come from the .ipynb itself; do not invent
one"), the test is made LIVE via **geometry-correctness assertions**, plus a
clearly-labelled non-referenced k **smoke** run.

| Test | Status | What it asserts |
|---|---|---|
| `hexagonal_lattice_geometry` | **LIVE (pass)** | All 37 tiles of the 4-ring lattice locate to the notebook's material; outer region = water; beyond r=5 = lost |
| `hexagonal_lattice_keff_smoke` | **LIVE (pass)** | k finite/positive/stationary over the hex lattice — **no reference compared** |
| `hex_tests::*` (5 unit tests in `src/geometry/lattice.rs`) | **LIVE (pass)** | Tile count, index round-trip, ring fill, face-distance geometry, outer resolution |

Nothing left `#[ignore]`d for this notebook. (The `HexagonalPrism` bounding
surface and the `orientation='x'` *plot* demo from the notebook's later cells are
not needed for the geometry/transport verification; the `'x'` code path itself
*is* ported and unit-testable via `HexLattice`.)

---

## Files changed

| File | Change |
|---|---|
| `src/geometry/lattice.rs` | Replaced the `HexLattice` data-only stub with a full port: `get_indices`, `are_valid_indices`, `flat_index`, `universe_at`, `get_local_position`, `center_offset`, `distance`, `from_rings` (+ `fill_lattice_y/x`), `HexOrientation`, `HEX_NONE`. Added the `Lattice` **enum** (`Rect`/`Hex`) for dispatch. Added `hex_tests` unit module (5 tests). |
| `src/geometry/geometry.rs` | `Geometry.lattices` changed `Vec<RectLattice>` → `Vec<Lattice>`; `locate`/`distance_to_boundary` now dispatch through the enum (hex distance also receives the tile `lattice_index`). |
| `src/prelude.rs` | Export `HexLattice`, `HexOrientation`, `Lattice` alongside `RectLattice`. |
| `tests/openmc_notebooks/hexagonal_lattice.rs` | Replaced the `unimplemented!()` ignored stub with the LIVE geometry test + k smoke test. |
| `tests/openmc_notebooks/triso.rs` | Wrap the existing `RectLattice` in `Lattice::Rect(...)` for the new enum field (no behaviour change). |
| `verification_and_validation/openmc_notebook_comparisons/hexagonal_lattice.csv` | **(gitignored)** comparison CSV — reference columns marked N/A (geometry-only notebook), smoke k recorded. |

---

## Provenance / port fidelity (VERIFY THIS)

Every hex method cites its OpenMC source line in its doc comment. Key mappings:

- `HexLattice::get_indices` ← `src/lattice.cpp:877` (skewed-basis floor + 2×2
  Voronoi nearest-centre refinement + on-boundary direction tie-break).
- `HexLattice::distance` ← `src/lattice.cpp:736` (beta/gamma/delta face
  directions; computed relative to *neighbour* tile centres for finite-precision
  robustness — hence it needs the current tile index).
- `HexLattice::get_local_position` ← `src/lattice.cpp:981`.
- `HexLattice::are_valid_indices` ← `src/lattice.cpp:725`.
- `HexLattice::flat_index` ← `HexLattice::get_flat_index`, `src/lattice.cpp:973`.
- `fill_lattice_y` ← `src/lattice.cpp:598`; `fill_lattice_x` ← `src/lattice.cpp:546`.
- `coincident`, `FP_COINCIDENT`, `FP_PRECISION` ← `include/openmc/geometry.h:33`,
  `include/openmc/constants.h:53,55`.

**Adaptation to VERIFY:** OpenMC calls hex `distance(r, u, i_xyz)` with `r` in the
*lattice* frame (reconstructed from the parent coord level in
`src/geometry.cpp:390`). This crate's `Geometry` descent stores the *tile-local*
position in `Coord.r`, so the port passes `r_local` and reconstructs the
lattice-frame `r` via `r_local + center_offset(i_xyz)` — an exact algebraic
inverse of `get_local_position`. A reviewer should confirm this reconstruction is
equivalent (it is a pure translation; no rotation is involved in this crate).

**Design-rule compliance to VERIFY:** enum dispatch (no `dyn`); no `Box`; no
lifetimes; pure `f64` inner loop; every new public item has a `///`/`//!` doc;
Android-safe (no new deps, pure Rust `f64`).

---

## Assumptions & limitations (VERIFY / KNOWN GAPS)

1. **No k-eff reference** — the notebook has none; the smoke k is NOT a
   validation number. Do not cite it as agreement with OpenMC.
2. **2-D only in `from_rings`** — the ring-based constructor builds a single
   axial level (`n_axial = 1`), matching the notebook. The 3-D axial-stack
   ring-input path (OpenMC's `m` loop) is not exposed by `from_rings`, though the
   `distance`/`get_indices` code carries the `is_3d` axial branches (ported,
   currently unit-tested only implicitly). Follow-up bead suggested.
3. **`orientation = 'x'`** is ported (both `fill_lattice_x` and the X branches of
   every method) but the LIVE notebook test only exercises the default `'y'`
   orientation. An `'x'` round-trip unit test is a suggested follow-up.
4. **Free-gas thermal** — the smoke run attaches no S(α,β) to H-1, so water
   moderation is free-gas; combined with the vacuum r=5 boundary this makes the
   system far subcritical. Expected, not a defect.
5. **`get_indices` at an exact tile centre** divides by `sqrt(d)=0` for the
   self-tile (matching OpenMC's own `r_t /= sqrt(d)`); the resulting NaN dot
   product is harmless because the zero-distance tile wins the `d < d_min` test
   before any NaN comparison matters. Verified by the round-trip unit test over
   all 37 tiles, but worth a human eye.

---

## Actual build / test output (2026-07-15)

```
cargo test -p outram-mc-libs --lib --release
  test result: ok. 44 passed; 0 failed; 0 ignored   (incl. 5 new hex_tests)

cargo test -p outram-mc-libs --release --test openmc_notebooks hexagonal_lattice_geometry
  test hexagonal_lattice::hexagonal_lattice_geometry ... ok

cargo test -p outram-mc-libs --release --test openmc_notebooks hexagonal_lattice_keff_smoke -- --nocapture
  [hex lattice] k = 0.28395 ± 0.00797  (npart=300, 15+25 gen, 20.1s)  [NO notebook reference — smoke only]
  test hexagonal_lattice::hexagonal_lattice_keff_smoke ... ok

cargo test -p outram-mc-libs --release --test openmc_notebooks triso   (enum-touched)
  test result: ok. 3 passed; 0 failed
```

- **Geometry property result:** all 37 tiles located correctly (4 big-pin→U238,
  33 pin→U235), outer→water, beyond boundary→lost. **PASS.**
- **Smoke k:** `k = 0.28395 ± 0.00797` (finite, positive, σ small). **Recorded,
  not compared.**

CSV: `verification_and_validation/openmc_notebook_comparisons/hexagonal_lattice.csv`
(gitignored, reproducible).

---

## nuclear-data (njoy) status

**No additional njoy data needed.** All four nuclides (U235, U238, H1, O16) are
in the embedded **CORE WMP** library via `Nuclide::from_core`. The geometry test
uses no nuclear data at all. njoy was **not** modified.

*(Optional future fidelity, not a gap for this task: attaching H-in-H₂O S(α,β) —
already available for the pincell — would let a hex smoke run thermalize
properly, but the notebook still has no reference to compare against.)*

---

## Human-verify checklist

- [ ] Diff `HexLattice::distance` / `get_indices` against `src/lattice.cpp:736,877`.
- [ ] Confirm the `r_local + center_offset` lattice-frame reconstruction in
      `HexLattice::distance` is exactly equivalent to OpenMC's parent-frame `r`.
- [ ] Re-run the tests locally in `--release` and confirm the geometry test and
      the 5 unit tests pass; sanity-check the smoke k is finite/positive.
- [ ] Confirm the enum change to `Geometry.lattices` didn't regress triso.
- [ ] Confirm CSV stays gitignored (not committed).
- [ ] Decide whether to file follow-up beads for: 3-D ring input in `from_rings`,
      an `orientation='x'` round-trip test, and an optional thermal hex smoke.
