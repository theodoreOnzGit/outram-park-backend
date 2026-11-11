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
