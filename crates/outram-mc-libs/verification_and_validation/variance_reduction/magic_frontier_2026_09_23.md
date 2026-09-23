# MAGIC does not penetrate the shield — a negative result

GitHub **#258**, acceptance item 3 ("a figure of merit improvement reported for
the shielding case"). Measured 2026-09-23 by
`tests/openmc_notebooks/shielded_room_weight_window.rs`.

**Acceptance item 3 is NOT met.** This records why, with the measurements, rather
than leaving the item to look outstanding for want of effort.

## What was fixed first

Two real defects in the variance-reduction code were found and fixed getting
this far; both are in the shipped library, not the test.

1. **Split daughters did not inherit their parent's window state.** Upstream
   copies `wgt_born`, `wgt_ww_born` and `n_split` onto every bank site
   (`particle.cpp:140-142` and `:114-116`, restored at `:201-202`). The port
   dropped all three, so `MAX_HISTORY_SPLITS` never bit and each daughter
   re-anchored the window to its own reduced weight, immediately re-qualifying
   to split. **The test ran 5.6 hours against a 64-second budget**; it now
   completes. Pinned by
   `the_split_cascade_terminates_because_daughters_inherit_their_parent`, which
   was verified to fail on the old behaviour before being kept.
2. **`max_split` was `f64` where upstream has `int`**, admitting a fractional
   cap that made `round(n_split)` disagree with `weight / n_split` — at
   `max_split = 2.5`, three copies of `w/2.5` each, i.e. **+20 % weight created**
   by the one routine whose justification is being weight-neutral. No recorded
   result was affected: every construction path used the integral default of 10.

## Methodology

Analog arm: 50 000 particles. Window arm: six MAGIC iterations (each running
with the *previous* iteration's windows, so the frontier bootstraps), then a
scoring run, the whole thing charged against the analog arm's wall-clock. Deep
region: 68 mesh cells beyond 200 cm of concrete.

A single MAGIC pass was tried first and is recorded here because its failure is
informative: MAGIC places a window only where its generating run scored, the
generating run is analog, and analog reaches zero deep cells — so the deep
region had no windows, nothing steered, and the window arm reproduced the
analog arm exactly (4140 particles, 0/68 deep cells). **One pass of an
iterative method is not the method.**

## Results

```
ANALOG   : 128.3 s, 0/68 deep cells scored, total flux 8.562e7
MAGIC it0: 558/1178 cells carry a window, generating run reached 0/68 deep
MAGIC it1: 641/1178                                            0/68
MAGIC it2: 631/1178                                            0/68
MAGIC it3: 657/1178                                            1/68
MAGIC it4: 656/1178                                            0/68
                                                  (timed out at 1 h)
```

| quantity | measured |
|---|---|
| window-arm cost | **0.83 s/particle** |
| analog cost | **2.6 ms/particle** (128.3 s / 50 000) |
| cost ratio | **~320x** |
| window frontier | saturates at **~650 of 1178 cells** |
| deep cells reached | **0 of 68**, both arms |
| FOM ratio | **NOT MEASURABLE** — undefined with no cell resolved in either arm |

## Why the frontier saturates, established from the code rather than guessed

`update_magic` sets `lower[i] = -1` when `sum[i] <= 0.0 || rel_err[i] >
threshold`. With `threshold = 1.0`, a frontier cell reached by one or two of
800 particles has a relative error of order 1 and is **refused a window**. That
is MAGIC working correctly — it declines to invent a window it cannot resolve —
and it means the frontier can only advance where the generating run already has
usable statistics. At 800 particles per iteration it never does, so the
frontier oscillates (558, 641, 631, 657, 656) instead of advancing.

The method's own requirement is therefore more particles per iteration, not
more iterations. At 0.83 s/particle that is unaffordable here: resolving each
successive shell across 200 cm of concrete needs iteration sizes that put the
test into hours.

## What this does and does not say

**Does not say** the weight-window implementation is wrong. Its unit invariants
hold (13/13), the cascade terminates, weight is conserved exactly across a full
cascade, and the roulette is unbiased. On the eigenvalue problem the paired
ablation gives `+14 ± 22 pcm` with the bias bounded at 57 pcm (2 σ).

**Does say** that this port's weight windows have **not** been shown to buy
anything on a deep-penetration problem, which is the case they exist for. The
notebook's claim is not reproduced.

**The open question is the 320x cost**, which is the thing to investigate next:
paying 320x per particle while the frontier does not advance suggests the
windows are driving repeated splitting *within* the already-resolved region
rather than pushing outward. The leading suspect is the `ratio = 5.0`
upper/lower spread being too tight against this problem's flux gradient, so a
particle re-splits on every cell crossing. That is a hypothesis with a stated
mechanism, not a measurement, and it is the next thing to test.

## The gate was not weakened

The assertion `ww_deep > analog_deep` is unchanged and still fails honestly.
The test is marked `#[ignore]` because it cannot complete in a tractable time,
with the reason naming this document — not because the criterion was moved.
