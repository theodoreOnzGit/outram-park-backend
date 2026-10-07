# Power iteration and the statistics behind the error bar

## The problem

`k_eff` is the ratio of neutrons in one generation to the generation before.
Monte Carlo estimates it by **running generations**. Each generation's fission
sites become the next generation's source, and the eigenvalue is read off the
ratio. Two things make this harder than it sounds: the first generations start
from a guessed source, and every generation's source depends on the one before.

## The loop

[`run_keff_csg_seq`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L537-L642)
is the whole algorithm:

1. transport every source neutron, collecting fission sites into `next_bank`
   and the total fission production;
2. the generation estimate is `k_gen = production / n_particles`;
3. after the first `n_inactive` generations, record `k_gen`;
4. resample `next_bank` back to `n_particles` sites and repeat.

The reported `k` is the mean of the active-generation estimates. Its error bar is
the standard error of that mean
([`raffles::estimators::mean_and_stderr`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L643)).
That function used to exist as four identical private copies, and now lives in one
place (GitHub #500).

## Why the inactive generations exist, and how to tell they were enough

The first source is a guess (a box rejection-sampled into fuel). The estimates
from those early generations describe *that guess*, not the converged shape, so
they are discarded. How many to discard is a question the crate answers with
**Shannon entropy** of the fission source on a mesh: when the entropy stops
drifting, the spatial shape has settled. It is computed from the bank **before**
resampling, ported from OpenMC [(Romano et al., 2015)](#ref-romano2015openmc) `src/eigenvalue.cpp:587`
([`transport_csg.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L607-L626)).

`k` alone is a poor convergence test. It is an integral quantity and can settle
long before the spatial source does. That is why entropy is the standard
diagnostic.

## Population control: comb, don't draw (GitHub #460)

Step 4 keeps the bank size fixed. Until 2026-09-30 it drew `n` sites independently
**with replacement**. That is unbiased for any one generation, but the extra
multinomial noise makes a known effect worse: power iteration with a finite bank
carries a small **negative** bias of order `1/N`. It now uses OpenMC's **uniform
comb** (`synchronize_bank`): `n` evenly spaced teeth with one random offset, so
every site is taken either ⌊n/total⌋ or ⌈n/total⌉ times
([`fission.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/fission.rs#L58-L89)).

```rust,ignore
{{#include ../../../src/physics/fission.rs:76:89}}
```

The test below it checks that property directly. Sampling with replacement would
fail it, because some site is almost surely taken zero or three or more times.

## What the error bar does and does not mean

The quoted standard error treats the active generations as **independent**. They
are not: each generation's source is built from the last one's. The correlation
usually makes the naive error bar **too small**. The `stats` module
(epic #493) computes batch means and an integrated autocorrelation time **beside**
the run's own `k_std`, which it never replaces. By the maintainer's 2026-10-03
decision every gate there is still **NOT YET MEASURED**
([`CLAUDE.md`, "Statistics on top of RAFFLES"](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/CLAUDE.md#L847-L893)).

The other honest move is the one the V&V examples make: run **many independent
seeds** and pool them. A single run's `± σ` describes that run's noise; the spread
across seeds shows whether it is right.

### An honesty lesson from the RNG

[`docs/validation.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/docs/validation.md#L1-L16) opens
with a banner. On 2026-08-06 the RNG output function changed (OpenMC's PCG
permutation was ported), and **every `k` in that document moved**. The ENDF
Godiva result it records (+94 pcm) could not be re-run on that machine, so the
document marks it superseded and says plainly that **no replacement was measured
or invented**. A number that was true for an old generator is history, not
evidence.

## Variance reduction: estimators, not physics

Survival biasing, Russian roulette and weight windows are all **off by default**
([`KeffSettings::variance_reduction`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/keff.rs#L170-L180)).
(Uniform fission site weighting is ported in `physics::ufs`, but no driver
calls it yet.) Leaving them off looks like it contradicts the workspace rule that correct physics is on by
default (next chapter). It does not. These change how a quantity is
**estimated**, never what it is
([`variance_reduction.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/variance_reduction.rs#L1-L41)).
The obligation that replaces "default on" is harder: each scheme must be **shown
unbiased** by a paired comparison against the analog arm, and leaving it off must
change nothing (`tests/variance_reduction_is_bit_identical_when_analog.rs`).

The same module doc records a porting decision: upstream only applies roulette
when survival biasing is on, because in analog transport every weight is exactly 1
and roulette has nothing to act on. The port follows upstream rather than the
issue that listed them separately.

### Measured: survival biasing

[`verification_and_validation/variance_reduction/ablation_2026_09_22.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/verification_and_validation/variance_reduction/ablation_2026_09_22.md)
(Godiva-radius HEU sphere, paired seeds, predictions written **before** the runs):

| N | seeds | survival − analog |
|---|---|---|
| 2000 | 96 | +122 ± 62 pcm (1.97σ) |
| 8000 | 32 | +83 ± 64 pcm (1.30σ) |
| 32000 | 48 | **+14 ± 22 pcm (0.61σ)** |

The apparent bias at small `N` shrinks as the bank grows, consistent with the
`1/N` population-control bias above rather than a defect. The report bounds the
asymptotic bias at |b| ≤ 57 pcm (2σ), and the figure-of-merit ratio ranges from 1.38× to 1.55× across the rows.

### Measured: weight windows, an honest miss

[`fom_2026_09_24.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/verification_and_validation/variance_reduction/fom_2026_09_24.md)
reports, in its own words, that weight windows do **not** improve the figure of
merit on the shielded-room case where the analog arm can also resolve, except in
the far field. Their real contribution is 43 mesh cells the analog arm cannot
resolve at all. "That is the honestly-derived answer, and it is a miss against the
hoped-for one." The same report also records a defect that had to be fixed before
any FOM could be trusted: a relative standard deviation that was `√n` too large.

<!-- references:begin -->
## References

<p class="csl-entry" id="ref-romano2015openmc" style="padding-left: 2em; text-indent: -2em;">Romano, P. K., Horelik, N. E., Herman, B. R., Nelson, A. G., Forget, B., & Smith, K. (2015). OpenMC: A state-of-the-art Monte Carlo code for research and development. <span style="font-style: italic;">Annals of Nuclear Energy</span>, <span style="font-style: italic;">82</span>, 90–97.</p>

<!-- references:end -->
