# Predictions, written before any CYCLUS run (gh:#1431)

Written and committed 2026-11-13 **before** any simulation in this directory was
run against CYCLUS. Nothing below has been compared with a CYCLUS output; the
only things done before this file was committed were (a) obtaining the binary
and (b) diffing the upstream sources (see `README.md`, "Route").

Time grid: 24 steps (t = 0..23), start Jan 2000, default Greedy solver,
default `allow_exclusive_orders`. Archetypes `:cycamore:Source` and
`:cycamore:Sink`, one commodity `u`, no recipe on either side.

| id | scenario | prediction (quantities in kg) |
|---|---|---|
| C1a | Source throughput 100, inventory 1000 -> Sink capacity 80 | t=0..11 trade 80 (12 trades), t=12 trade 40, none after; 13 trades, total 1000. All 13 **bit-identical** (80 and 1000-960=40 are exact in binary64). |
| C1b | Source throughput 33.3, unlimited -> Sink capacity 100, max_inv_size 500 | t=0..14 trade 33.3 (15 trades), t=15 the remaining space `500 - 15*33.3` (~0.5), none after. Predicted bit-identical if both sides accumulate the inventory in the same order; bound on any residual: a few ulp of 500, so at most ~1e-12 relative on the t=15 trade only (cancellation 500 -> 0.5), none on the others. |
| C2a | one Source (throughput 100), Sink A capacity 60, Sink B capacity 70, equal preference | total 100 per step. Exactly one sink is filled to its request and the other takes the remainder (40 or 30). Predicted **identical**, with the same sink first on both sides; a difference is localised to the greedy request ordering (`GreedySolver` / `GreedyPreconditioner`). |
| C2b | as C2a but preferences A = 1, B = 2 (port needs `Sink::with_preferences`) | B is served first: B 70, A 30 every step. Both sides identical. |
| C2c | Source P (throughput 60) and Source Q (throughput 70), Sink capacity 100, equal preference | total 100 per step; the first-considered supplier is filled, the other gives the remainder. Identical on both sides, tie order named if not. |
| C3 | `KahanSum`, `comp_math` normalise/add/sub, `Decayer` vs `decay` on a 5-nuclide composition | KahanSum and comp_math **bit-identical**. Decay: not bit-identical unless the same matrix-exponential method and operation order; upstream uses Uniform Taylor (`uniform_taylor.cc`), the port reuses PETIR's. Predicted worst relative difference < 1e-10. |
| C4 | `enrichment.h` SWU, feed, tails for 10 product assays (feed 0.711 %, tails 0.25 %) | closed form in `log`; predicted <= 1e-14 relative, not necessarily 0 because PETIR's `ln` may differ from glibc's by an ulp. |
| C5 | Reactor refuelling cycle | **cannot be run**: `cycamore:Reactor` is not ported (gap survey). No prediction; reported as a gap and filed as a child issue. |
