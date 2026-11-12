# fix addforce / viscous / freeze vs LIGGGHTS-PUBLIC 3d5c00f2 (gh:#1418)

Source: `src/force_fixes.rs` (translation of `fix_addforce.cpp`, `fix_viscous.cpp`,
`fix_freeze.cpp` `post_force`). Deck: `reference-data/liggghts/in.forcefix` ->
`forcefix.csv` (upstream run with `run_deck.sh`, 2026-11-11). Test:
`tests/force_fixes_cross_code.rs`.

Prediction (before the run): all values bit-identical.

| Comparison | Result |
|---|---|
| frames 0..700 (8 x 354 x 9) | 25 488 / 25 488 bit-identical |
| full deck, frames 0..2000 (21 frames) | 66 699 / 66 906 bit-identical; ignored test |

First run failed (38 841/66 906): the builder attached the fixes after the
construction-time set-up force evaluation, so the step-0 forces (first
half-kick) lacked them; upstream's `setup` calls every `post_force`. Fixed by
re-running the set-up evaluation in `with_force_fixes`.

Open: from frame 800 one pebble (index 188, `addforce` group) differs (v 0.3 %,
omega 40 %) at the onset of a sustained contact (overlap 1e-6) with frozen
pebble 24, begun between steps 700 and 800. Not explained; not loosened.
Not ported: addforce `region`/atom-style variables/`energy`.

## `fix wall/region` (gh:#1416, added 2026-11-11)

Translated from `src/fix_wall_region.cpp` (LIGGGHTS-PUBLIC `3d5c00f2`):
`ForceFix::WallRegion`, with `InsertRegion::surface_interior` from
`region_block.cpp` / `region_cylinder.cpp`. Interior `block` and `cylinder`
regions only. Deck `reference-data/liggghts/in.wallregion`
(`make_wallregion_deck.sh`): settled 354-pebble bed, no `fix wall/gran`, four
`fix wall/region` (harmonic, lj93, lj126, colloid), 2000 steps.
Test: `tests/wall_region_cross_code.rs`.

**Prediction (before the run):** all bit-identical (same operation order; glibc
`pow`/`log` on both sides).

**Result (2026-11-11, Rust 1.99.1):** trajectory **66 906 / 66 906**; thermo
(energy and the three wall-force components of each of the 4 fixes, 21 rows)
**336 / 336**, worst relative difference 0.
First run gave thermo 113/336 (worst 1.1e-14) with the trajectory already
identical: the cause was summation order. Upstream sums `ewall` over its local
atom order, which `Atom::sort` permutes into bin order at setup; the port
summed in tag order. Fixed by passing `GranularSystem::local_order_indices()`
into `ForceFix::apply`. Not ported: exterior regions, other region styles,
moving regions.

**Re-run 2026-11-12 on PETIR maths (epic gh:#1336):** the port's `pow`/`log`
moved from `std` (glibc) to `petir::mathf::RealMath` (`r_powf`, `r_ln`). Both
tests still pass, every trajectory and thermo value bit-identical.
