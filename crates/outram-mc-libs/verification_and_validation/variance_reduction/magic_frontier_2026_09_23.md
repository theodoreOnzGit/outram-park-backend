# MAGIC and the shield — a negative result, and its CORRECTED diagnosis

> **CORRECTION, same day.** The first version of this document (commit
> `796d25c72`) blamed the frontier stall on the window arm's 320x per-particle
> cost making the needed iterations unaffordable. **That attribution was
> wrong.** MAGIC iterations cost ~10 s each, not hours, because the windows are
> sparse early on. The actual cause was that MAGIC was being **seeded from a
> starved 800-particle pass while a 50 000-particle flux tally on the same mesh
> sat unused in the analog arm**. Since `update_magic` refuses a window
> wherever `rel_err > threshold`, the starved seed refused the whole frontier by
> construction. With the analog tally reused (free -- it is already paid for),
> the frontier advances monotonically instead of oscillating:
> `689 -> 621 -> 656 -> 675 -> 699 -> 724` of 1178 cells.
>
> The cost measurements below stand. The *conclusion drawn from them* did not,
> and is struck through where it appears.

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

~~The method's own requirement is therefore more particles per iteration, not
more iterations. At 0.83 s/particle that is unaffordable here.~~
**CORRECTED 2026-09-23** — the generating iterations do *not* run at
0.83 s/particle; measured, they run at ~25 ms/particle (50.7 s for five
400-particle iterations), because the window set is sparse while the frontier
is still near the source. The requirement is **both** better statistics for the
seed (supplied free by the analog arm's own tally) **and** enough iterations to
walk the frontier across the shield, and both are affordable. The 0.83 s/particle
figure belongs to the **scoring** arm, which runs against a fully-populated
window set, and it was wrongly generalised to the generating iterations.

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

---

# The figure of merit, measured (#258 acceptance item 3)

Measured 2026-09-23 on the run that completes, after MAGIC was seeded from the
analog tally and the chunk growth was bounded. **Acceptance item 3 is now
answered, and the answer is negative.**

## Conditions

Analog arm 50 000 particles in 138.4 s. Window arm: six MAGIC iterations
(117.6 s, leaving 751/1178 cells carrying a window) plus a matched-cost scoring
run, 172.9 s total. Deep region: 68 cells beyond 200 cm of concrete.

## Results

| depth band | FOM ratio (windows / analog) | cells | windows-only |
|---|---|---|---|
| 0–400 cm (source side) | **0.011x** (median 0.012, range 0.003–0.032) | 27 | 0 |
| 400–900 cm (mid-field) | **0.007x** (median 0.006, range 0.000–0.044) | 101 | 0 |
| 900–1450 cm (far field) | **0.011x** (median 0.009, range 0.001–0.434) | 58 | 0 |

**Weight windows are roughly 100x WORSE than analog by figure of merit on this
configuration, in every band.** `windows-only 0` in all three bands means they
never resolved a cell the analog arm did not.

**Unbiasedness holds**: flux per source particle 1712.43 analog against 1878.06
windowed, **+9.67 %**, within the 25 % gate and consistent with the window
arm's 240-particle statistics. So this is a cost result, not a correctness one —
the windows do not move the answer, they just pay far too much for it.

## The mechanism, from the numbers

At matched cost the window arm bought **240 particles against analog's 50 000**
(0.72 s/particle against 2.8 ms). It reaches no deeper, so it simply has worse
statistics everywhere, and the FOM ratio is close to the particle-count ratio.

The standing explanation for the 260x per-particle cost is the **window mesh
being far too coarse for the window ratio**: cells are 1550/31 = **50 cm**
against a fast-neutron mean free path in concrete of order 5–10 cm, so each
cell is 5–10 mfp and the flux falls by 10^2–10^4 across it, against a window
ratio of 5. Every cell crossing therefore splits at the `max_split = 10` cap,
paying for splits that buy no penetration because the *next* cell has no window
yet. Standard guidance is that the flux should change by less than the window
ratio across one mesh cell; this violates it by two to three orders of
magnitude.

**That remains a hypothesis with a stated mechanism, not a measurement.** The
discriminating test is a mesh refined until adjacent-cell flux ratios are below
5 (roughly 10–16 cm cells, so ~100x120 rather than 31x38), which needs
proportionally more statistics to populate and was not run here.

## What #258 can conclude

- **Unbiasedness: established.** Both on the eigenvalue problem
  (`+14 ± 22 pcm`, bias bounded ≤57 pcm at 2 σ) and here (+9.67 % on flux per
  source particle, within statistics).
- **Worth on a deep-penetration problem: measured, and negative.** ~100x worse
  by FOM at matched cost. The technique is implemented and unbiased; it has
  **not** been shown to pay on the case it exists for.
- **The notebook's deep-penetration claim is NOT reproduced.** The frontier was
  still advancing at +23 cells/iteration when the run was cut off; reaching the
  deep region needs ~20 more iterations at a rising cost, about 2 hours.

Quoting this as "variance reduction works" would be wrong. The correct
statement is that the estimator is right and the configuration is wrong, with
the mesh resolution named as the next thing to test.
