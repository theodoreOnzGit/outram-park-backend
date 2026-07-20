# op-bsz — Self-shielded MGXS (URR PENDF feeder + Bondarenko-dilution MGXS + Chi/fission matrix)

**⚠️ AI-GENERATED DRAFT — HUMAN REVIEW REQUIRED per `RESPONSIBLE_USE.md`.**

This work was produced by an AI fleet (a lead agent that scaffolded the module
structure, then per-slot Opus subagents that filled each stub). It is **untrusted
draft material** until a human has inspected it, checked licence provenance, and
verified the physics against reference cases. Nothing here is validated against a
real NJOY GROUPR GENDF golden tape (see "Top V&V asks").

- **Bead:** op-bsz (blocks op-6tz.6.3).
- **Upstream mirrored:** NJOY2016 `src/groupr.f90` @ commit `ac5adf5`
  (`/home/teddy0/Documents/research/NJOY2016/src/groupr.f90`).
- **Data:** open-source ENDF/B-VIII.0 only (U-235 MAT 9228, U-238 MAT 9237).
- **Date:** 2026-07-15.

## What existed before (op-3ut) — the base this extends

`crates/njoy-outram-park-fork/src/groupr/unresolved.rs` already ported the URR
self-shielding **math** and is green:
- `bondarenko_flux_value` / `genflx_bondarenko` — the P0 Bondarenko
  narrow-resonance weighting flux (`groupr.f90:5309-5684`, Bondarenko branch).
- `UnresolvedTable::store` / `::shield` (`getunr`, `groupr.f90:6896-6994`,
  `iovl==0` path) + `terpu` sigma-zero interpolation (`groupr.f90:6996-7036`).
- `read_urr_from_pendf()` — a no-arg `NotPorted` boundary marker (kept).

`groupr/matrix.rs` (op-3ut) ported the elastic scatter matrix; `groupr/panel.rs`
(op-cjw.15) ported the vector group-average engine.

## Scaffold layout (this change) — 5 new modules + 1 test skeleton

| Slot | New file (`src/groupr/`) | Fortran mirrored | Purpose |
|---|---|---|---|
| 1 | `urr_pendf.rs` | `stounr` 6822-6875 | PENDF MF=2/MT=152 URR-tape reader → `UnresolvedTable` |
| 2 | `slowing_down.rs` | `genflx` 5396-5620 | Integral slowing-down / heterogeneity flux branch |
| 3 | `overlap.rs` | `getunr` iovl/xtot 6958-6991 | Resolved/unresolved overlap background correction |
| 4 | `self_shielded.rs` | `panel`/`displa` nz>1, 5858-6091 | Bondarenko-dilution per-σ0 group-XS assembly |
| 5 | `fission_matrix.rs` | fission feed (MT=18) | Group Chi collapse + separable fission matrix |

Test skeleton: `tests/openmc_notebooks_data/mgxs_part_i.rs` —
`self_shielded_mgxs_dilution_limits_u238` (real U-238 ENDF/B-VIII.0 inputs;
`#[ignore]` until the assembly lands + is verified). The pre-existing
`flux_weighted_self_shielded_mgxs` `#[ignore]` test is the op-6tz.6.3 target.

Each new module carries the GPLv3/NJOY2016 provenance header (upstream project,
source file, commit, licence, "modified non-LANL" disclaimer) and `//!` module
docs mapping Rust items → Fortran line ranges.

## Per-piece status — DONE vs PARTIAL

> Filled from the subagents' verified results. `LIVE` = a real V&V test passes;
> `NotPorted` = an honest documented gap (no fabricated value).

| Slot | Status | Test(s) | Measured result |
|---|---|---|---|
| 1 `urr_pendf` | **LIVE** | `urr_reader_round_trips_synthetic_mf2_mt152`, `..._skips_to_matching_temperature`, `..._errors_when_temperature_too_low`, `lssf_flag_mapping` (4) | Synthetic MF=2/MT=152 round-trip reproduces sigma0 grid, energies, overlap flags, all 18 cross sections bit-exactly; temperature skip + too-low error paths correct. |
| 2 `slowing_down` | **PARTIAL (FULL homogeneous case)** | `slowing_down_reduces_to_bondarenko_pure_absorber`, `..._infinite_dilution_returns_weight`, `heterogeneity_terms_report_not_ported`, `invalid_inputs_are_rejected` (4) | Homogeneous single-moderator branch (`5396-5620`, `nalph=1`) reduces to the Bondarenko flux in the NR limit **bit-identically (max dev 0.0)**; infinite-dilution `|phi-1| = 1.88e-8`. Heterogeneity/multi-moderator (`beta`/`sam`/`alpha2/3`/`gamma`) honestly `NotPorted`. Added `absorber_awr` field. |
| 3 `overlap` | **LIVE** (needed integrator accessor) | `non_overlap_energy_matches_plain_shield`, `overlap_augments_partial_reaction_background`, `overlap_rejects_length_mismatch` (3) | Non-overlap energy == plain `shield`; at an overlap point Total sets `xtot = sig(1)-sinf` and partials self-shield at `sigma0+xtot` (== reference augmented `shield`). Unblocked by adding `UnresolvedTable::overlap_context` to `unresolved.rs` (integrator). |
| 4 `self_shielded` | **LIVE** | `infinite_dilution_equals_vector_average`, `group_xs_strictly_increasing_in_dilution`, `fully_shielded_floor_is_the_minimum`, `urr_table_shields_below_flux_only` (4) | Triangular resonance: inf column `[51.0, 51.0]` == vector avg (<1e-12); strictly monotone in σ0 (`51.0 → 8.83`); floor is minimum; URR table shields further (17.31 → 6.93 b). |
| 5 `fission_matrix` | **LIVE** | `watt_group_chi_normalizes_and_is_fast`, `separable_matrix_row_sums_and_proportionality`, `invalid_inputs_rejected` (3) | Group Chi sums to `1.0` (dev <1e-16), fast fraction `0.987`; separable matrix row sums == group νσ_f to ≤3e-16; rows ∝ shared Chi. |

### Integration — the op-6tz.6.3 target test is now **LIVE**

`tests/openmc_notebooks_data/mgxs_part_i.rs`:

- **`flux_weighted_self_shielded_mgxs`** — was `#[ignore]` (panic stub), now LIVE
  on U-235 ENDF/B-VIII.0. Self-shielded total `σ_t(σ0)`: `inf → [1071.15, 35.19]`,
  `1e4 → [995.37, 34.42]`, `1e2 → [504.78, 23.36]`, `1e0 → [429.52, 14.44]` b
  (monotone; inf ≈ vector average). Group Chi `[2.2e-10, ~1.0]` sums to 1.
  Separable fission matrix row sums `[2172.6, 39.12]` b == group νσ_f exactly.
- **`self_shielded_mgxs_dilution_limits_u238`** — new, LIVE on U-238 (strong URR):
  group-2 σ_t self-shields `68.6 → 10.3 b` from σ0 `inf → 1` (the U-238 resonance
  signature); all four dilution-limit properties hold.

## Assumptions (to verify)

- **MF=2/MT=152 `unr(*)` layout** was reconstructed from `getunr`'s array
  offsets (`groupr.f90:6919-6924,6944-6956`), not from a real UNRESR-produced
  tape. The reaction-column order is assumed `(Total, Elastic, Fission, Capture,
  Current)` = the first `nx` columns.
- **No real URR table on hand.** This crate does not yet run UNRESR/PURR, so the
  MF=2/MT=152 record is exercised only with a synthetic round-trip. The
  self-shielded MGXS assembly is exercised in the honest **tape-free** mode
  (Bondarenko-flux weighting, URR table = `None`).
- **Separable fission matrix** assumes `chi(E'|E) ≈ chi(E')` (incident-energy-
  independent χ). The full incident-dependent matrix needs the MF=5 incident
  axis and is flagged as a remaining gap.
- U-238 `sigma_pot ≈ 11.3 barn` in the test skeleton is an approximate elastic
  asymptote, not read from the evaluation — refine before asserting numbers.

## Integrator changes (lead agent, beyond the 5 slots)

- `src/groupr/unresolved.rs` — added `pub struct OverlapContext` +
  `UnresolvedTable::overlap_context(reaction, e)` (the `iovl` flag + absolute
  `sinf`, `groupr.f90:6959-6979`) to unblock slot 3. This is the only edit to a
  pre-existing source file; it is additive (no behaviour change to `shield`).
- `src/groupr/overlap.rs` — completed `shield_with_overlap` (was the subagent's
  honest `NotPorted` blocked on the above accessor).
- `tests/openmc_notebooks_data/mgxs_part_i.rs` — wired the two self-shielded
  integration tests to LIVE.
- `src/groupr/mod.rs` — module wiring + re-exports for all new items.

## Build / test output (release, `ulimit -v 12 GiB`, 2026-07-15)

- Baseline (before this change): **472** lib+integration tests (357 lib).
- After: **lib 375** (357 + 18 new: urr_pendf 4, slowing_down 4, overlap 3,
  self_shielded 4, fission_matrix 3), **482 total passing, 9 ignored**, `0 failed`
  across all test binaries. No regression to the pre-existing suite; the only
  status change is `flux_weighted_self_shielded_mgxs` moving ignored → passing,
  plus one new U-238 integration test.
- Full `cargo test -p njoy-outram-park-fork --lib --tests --release`: all
  `test result: ok`, `0 failed` in every binary.

## Top V&V asks for the human reviewer

1. **URR PENDF reader correctness** — validate the MF=2/MT=152 `unr(*)` layout
   and reaction-column order against a real UNRESR/PURR-produced tape (the
   synthetic round-trip only proves self-consistency).
2. **Bondarenko-dilution MGXS correctness** — confirm the σ0→∞ / σ0→0 limits and
   monotonicity match a trusted lattice-physics reference, not just internal
   consistency.
3. **Golden-file gap (top ask, tie to op-ini)** — none of this is validated
   against a real NJOY GROUPR GENDF golden tape. Producing one (and an
   openmc-notebooks `mgxs` `.ipynb` reference where applicable, openmc-notebooks
   @ `cf1e5db`) is the outstanding validation step.
