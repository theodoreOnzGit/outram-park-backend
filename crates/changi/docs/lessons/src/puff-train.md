# A train of puffs, and a wind that turns

## The problem

A release lasts twenty minutes, and halfway through the wind swings from east
to north. The steady plume of [the plume page](./plume.md) cannot represent either fact. A puff
model can: emit a puff every `puff_dt`, carry each one on the wind, and add up
their contributions at the receptor
([`simulate.rs`, lines 7–20](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/puff/simulate.rs#L7-L20)).

That loop is
[`simulate_sensor_mode`](../../api/changi/puff/simulate/fn.simulate_sensor_mode.html)
and
[`simulate_grid_mode`](../../api/changi/puff/simulate/fn.simulate_grid_mode.html),
ported from upstream's two R drivers. This chapter is about the two places
where **the port's default deliberately differs from upstream**, and why.

## Defect 5: a puff that cannot turn

### What upstream does

Upstream records the wind **at the moment a puff is emitted**. At every later
step it recomputes the puff's position from its age:

```r
x_p <- x_0 + puff$wind_u * (current_elapsed - puff$time_emitted)
y_p <- y_0 + puff$wind_v * (current_elapsed - puff$time_emitted)
total_dist <- sqrt((x_p - x_0)^2 + (y_p - y_0)^2)
```

So a puff flies a straight ray on the wind of its birth for its whole
1200-second life, and **never responds to the wind again**. Two things go
wrong when the wind veers
([`docs/puff-code-to-code.md`, lines 136–193](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/docs/puff-code-to-code.md#L136-L194)):

1. The plume cannot bend. What should become a dog-leg stays a straight line
   on the old bearing.
2. The dispersion distance is the **chord**, not the **path**. A puff blown out
   and partly back has been spreading for its whole journey, but upstream sizes
   it by how far it now sits from the source.

### Why the fixture did not catch it

The code-to-code comparison agrees with upstream to machine precision, and it
still missed this. **Every fixture case uses a constant wind**, and on a
constant wind the defect is invisible. It was found from a user report. That is
the general lesson: a comparison against upstream can only find what differs
on the inputs it was given.

### The fix: integrate the trajectory

The port's default,
[`AdvectionPolicy::LagrangianTrajectory`](../../api/changi/puff/simulate/enum.AdvectionPolicy.html),
treats each puff as a parcel with its own state. Its position is the integral
of the wind it has actually met, and its dispersion distance is the **path
length** it has accumulated
([`simulate.rs`, lines 34–108](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/puff/simulate.rs#L34-L108)):

```rust,ignore
{{#include ../../../src/puff/simulate.rs:263:290}}
```

Upstream's behaviour is kept as `AdvectionPolicy::UpstreamFrozenWind`, and the
code-to-code fixture asks for it by name.

### The measurement

One puff, with the wind due east at 5 m/s for 100 s and then due north at
5 m/s for 100 s
([test, lines 817 onwards](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/puff/simulate.rs#L818-L891)):

| Quantity | Lagrangian (default) | Frozen wind (upstream) |
|---|---|---|
| `dx` | **500.0 m** | 1000.0 m |
| `dy` | **500.0 m** | 0.0 m |
| dispersion distance | **1000.0 m** (path) | 1000.0 m (chord) |
| net displacement | 707.1 m | 1000.0 m |

The two puffs end up 707 m apart (`hypot(500, 500)`; the repository's own write-up says 500 m, which is the `dx` difference alone). Notice that the **dispersion distance agrees**,
because the two legs happen to be equal. A test that checked only the distance
would have passed against the defect.

### Does the fix change any steady-wind number?

No. Marching `dx += u dt` for `n` steps is `u n dt`, which is `u × age`, so
the two policies agree algebraically on a constant wind. They are not
*bit*-identical, because many additions do not round like one multiplication.
Over a full 1200 s life at 10 s steps the measured difference in distance is
2.8e-15 relative
([test, line 927](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/puff/simulate.rs#L928)).
That rounding difference is exactly why the fixture names the upstream policy
rather than relying on the agreement.

## Defects 2 and 3: the second stability class

[The puff page](./puff.md) showed that six of the ten Pasquill regimes return two classes.
Upstream mishandles that pair twice, in opposite directions
([`docs/puff-code-to-code.md`, lines 211–293](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/docs/puff-code-to-code.md#L212-L294)):

- **In the kernel, the second class is silently dropped.** `gpuff` reads a
  column-major R matrix as `sigma.vec[1]`, `sigma.vec[2]`, which are both from
  the *first* class.
- **In the drivers, the second class doubles the mass.** The puff record is an
  R `data.frame` whose length-1 columns are recycled against a length-2
  `stab_class` column. Each emission therefore becomes **two rows, each with
  the full puff mass**.

The doubling is the common case, not an edge case. And the effect on the
reported concentration is **not** a clean factor of two, because the two
recycled puffs disperse with different classes. Measured across the four
Singapore monsoon conditions in `puff::climatology`, at 14:00 and 02:00, 50 m
downwind, the ratio of upstream's peak to the mass-conserving one runs from
**1.04 to 3.97**. A published `puff` result therefore cannot be corrected by
dividing by two.

The port's default,
[`EmissionPolicy::OnePuffPerEmission`](../../api/changi/puff/simulate/enum.EmissionPolicy.html),
emits one puff carrying the whole mass and disperses it with the primary (more
unstable) class. That matches what upstream's own `gpuff` does with an
ambiguous class. Upstream's doubling is kept as
`EmissionPolicy::UpstreamRecycleStabilityClasses`
([`simulate.rs`, lines 111–150](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/puff/simulate.rs#L111-L150)).

## The pattern: correct by default, upstream by name

Both fixes follow one workspace rule. **Physics the model is meant to
represent is on by default, and reproducing upstream's behaviour is an
explicit, visible choice.** The port therefore has two jobs, and keeps them
separate:

- **Verification** asks "does this compute what upstream computes?" It runs
  with both upstream policies named, and agrees with the R to machine
  precision.
- **Use** asks "is this the right physics?" It runs with the corrected
  defaults, and each default is pinned by its own test so it cannot drift
  back.

## Two smaller things worth knowing

- **Grid mode never writes its last output row.** Upstream's index arithmetic
  stops one short, so the final row stays zero, which a reader could take as
  "the plume has passed". The port reproduces it, and pins it with a test that
  fails loudly if upstream ever fixes it.
- **The two modes report different quantities.** Sensor mode returns the
  **mean** over each output interval. Grid mode returns the **instantaneous**
  value at the end of it. They cannot be compared at the same place and time.
  ([`docs/puff-code-to-code.md`, lines 294–321](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/docs/puff-code-to-code.md#L295-L322))

## Try it

1. A puff travels 1 km east and then 1 km back west. Under each policy, what
   are its net displacement and its dispersion distance? Which `sigma` does
   each policy give it, and which is physically right?
2. Why does `RunConfig` insist that `puff_dt` is an integer multiple of
   `sim_dt`? (Hint: [the activity page](./activity.md) bins puffs by age.)
