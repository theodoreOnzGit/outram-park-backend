# Oxidation kinetics (corrosion) vs OFFBEAT 80e84450 (gh:#1393, round 2)

Measured 2026-11-13; predictions committed first (`offbeat_differential_gh1393/PREDICTIONS_round2.md` G).
Route and caveats as in `conductivity_vs_offbeat.md` (standalone C++ transcription, `dump_corrosion.cpp`; fixture
`reference_corrosion.txt`, 8 400 rows; test `tests/offbeat_differential_corrosion.rs`; **not a run of the compiled
library**).  `check_verbatim.sh`: every UPSTREAM line found.  Compared: `EPRI_KWU_CE`, `CathcartPawel` and the low/high
combination at 673 K.  Not compared: the interface-temperature and metal-thickness updates of `zircaloyOuterCorrosion`
(mesh patch fields), hydrogen pick-up, thermal/exotherm (not OFFBEAT models).

## Results (G1 `8 eps x 4 x value`)

| model | compared (vs mode 2 / stateless) | bit-identical | worst relative | worst G1 ratio | failures |
|---|---:|---:|---:|---:|---|
| CathcartPawel (incl. the broken Schanz window 1800-1900 K) | 1 680 | 1 680 | 0 | 0 | 0 |
| EPRI_KWU_CE | 1 680 | 764 | 6.3e-15 | 0.888 | 0 |
| EPRI + Cathcart low/high | 1 680 | 1 406 | 6.3e-15 | 0.888 | 0 |

Passes, but the EPRI margin is thin (0.888 of the limit) and the cause is **not** the one predicted.  Localisation
(`localise_corr.cpp`, `localise_corr_output.txt`): upstream takes the cube root as `pow(x, 1.0/3.0)`, the port as `cbrt(x)`.
`1.0/3.0` differs from 1/3 by 1.85e-17, so the two cube roots differ on all 80 sampled arguments by up to 1.2e-15
(~5 eps, `|1.0/3.0 - 1/3| |ln x|` with `x ~ 1e-17`), and on rows that cross the transition the difference is amplified
to 8.3e-15 in `dt_pre = (S_trans - S0)/(S_pre - S0) dt` (cancellation when the step is short against the thickness).
Observed worst 6.3e-15, inside that estimate.  The prediction (only libm differences enter, scale `4 x value`) was
too optimistic about the cause; the gate was not changed and still holds.  The port's `cbrt` is the more accurate
operation; the upstream rounding is the reference.

## Upstream behaviour: the kinetics depend on the entry value of the output variable

`EPRI_KWU_CE::correctOxideThickness(S, S0, ...)` branches on `S` (`if (S <= S_trans)`), the in/out variable, not on `S0`.
`S` holds `Sold` on the first nonlinear iteration of a step and the previous call's result afterwards.  A step that crosses
2 um therefore returns the pure cubic overshoot `S_pre` on the first call and the crossing result on later calls; a step
starting exactly at `S0 = S_trans` returns the cubic value on the first call and the linear one afterwards.  The port is
stateless and equals the later-iteration value (mode 2) bit-for-bit within the gate.  Against the first-call value (mode 1)
the port differs on exactly the 870 rows that cross (600) or start at the transition (270); on the 2 490 other
EPRI-bearing rows the two coincide.  Prediction 2 (differs exactly on crossing rows) was incomplete: I had missed the
`S0 = S_trans` equality rows (270), found when the first run failed my own assertion; the rule was extended and the
assertion re-run, with the equality case documented here.

Prediction scorecard: 1 held (the gate; the cause was misjudged), 2 held after the extension above, 3 held (Schanz window
reproduced bit-for-bit, 1 680/1 680), 4 held.

Provenance: OFFBEAT 80e84450 (GPL-3.0, gitlab.com/foam-for-nuclear/offbeat), accessed 2026-11-13.
