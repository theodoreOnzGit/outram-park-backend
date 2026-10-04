# The steady Gaussian plume

## The problem

A stack at height `H` releases material at a steady rate `Q` into a wind of
steady speed `u`. Far enough downwind, what is the concentration at a point
`(x, y, z)`?

## The model

Turbulence spreads the plume sideways and vertically. If you assume the spread
in each direction is Gaussian, with standard deviations `sigma_y(x)` and
`sigma_z(x)` that grow with distance downwind, and that the ground reflects
rather than absorbs, the answer is the textbook Gaussian plume:

```text
            Q                    y^2          (z - H)^2           (z + H)^2
C = ------------------- exp( - ---------- ) [ exp(- ---------) + exp(- ---------) ]
    2 pi sigma_y sigma_z u      2 sigma_y^2      2 sigma_z^2         2 sigma_z^2
```

Three assumptions are carried by that formula:

- **Steady state.** Constant release and constant wind. Nothing here can
  describe a release that starts, stops, or meets a wind that turns.
- **Advection dominates along the wind.** Spreading in `x` is neglected next to
  transport by `u`, which is why `u` appears only in the denominator.
- **Image source.** The second exponential is a mirror source at `-H`, so
  nothing passes through the ground. Deposition, if wanted, has to be added
  separately.

Dividing `C` by `Q` gives the **dilution factor** `chi/Q`, in s/m³. It is the
quantity consequence codes carry, because it depends only on geometry and
weather, never on what was released.

## The code: `buangkok`'s port of pyDOSEIA

This workspace's steady plume is in `buangkok`, ported from
[pyDOSEIA](https://github.com/BiswajitSadhu/pyDOSEIA)'s `metfunc.py`. Upstream
cites Hukkoo and Bapat's BARC manual for its master equations. The single-plume
form is eq. 2.5
([`buangkok/src/pydoseia/dispersion.rs`, lines 192–214](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L192-L214)):

```rust,ignore
{{#include ../../../../buangkok/src/pydoseia/dispersion.rs:192:214}}
```

Two things are worth noticing:

- The result is split into a **pre-exponential** term and an **exponential**
  term, exactly as upstream splits it. The port keeps upstream's operation
  order so it can be compared bit for bit
  ([lines 19–20](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L19-L20)).
- `speed_factor` is the wind speed **at release height**. pyDOSEIA measures
  wind at a reference height `H_m` and scales it by `(H / H_m)^p`, a power law
  whose exponent depends on stability, with a release below 10 m raised to
  10 m first
  ([`height_correction_factor`, lines 130–155](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L130-L155)).

### The sigmas pyDOSEIA uses

- `sigma_y = A_y x^0.9031`, one coefficient per class
  ([lines 88–98](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L88-L98)).
  Upstream computes a sampling-time correction and then never applies it, and
  the port follows upstream.
- `sigma_z = A_z x^q + r` in three distance bands (`x < 100 m`,
  `100–1000 m`, `> 1000 m`). The bands meet only approximately, with jumps of
  up to 0.84 % (class E at 1000 m), and the port keeps the jumps
  ([lines 100–128](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L100-L128)).

These are **not** the sigmas `changi`'s puff model uses ([the next page](./puff.md)). The
two sets are compared on [the V&V page](./vv-and-limits.md) and in [rung 2](./rungs/02-sigmas.md).

### Short-term and long-term

pyDOSEIA has a second master equation, the **sector-averaged** plume
(eq. 2.32), for long-term releases. It smears the crosswind Gaussian uniformly
over a 22.5° wind sector. Upstream writes that sector as `0.39275` rad, and the
exact value is `0.392699…`. The port keeps upstream's constant, a relative
difference of 1.3e-4
([lines 216–240](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L216-L240)).
Keeping a known-imprecise constant is deliberate: a port that "improves"
upstream can no longer be checked against it.

API: [`master_equation_single_plume`](../../api/buangkok/pydoseia/dispersion/fn.master_equation_single_plume.html),
[`sigma_y`](../../api/buangkok/pydoseia/dispersion/fn.sigma_y.html),
[`sigma_z`](../../api/buangkok/pydoseia/dispersion/fn.sigma_z.html).

## Why a plume is not enough

The plume formula has no time in it. A real release has a start and an end,
and the wind changes during it. That is what the puff model in the next two
chapters is for. The plume earns its place as a **reference**: in the steady
limit a train of puffs should reproduce it, and [the V&V page](./vv-and-limits.md) shows that it does,
with a residual whose size was predicted before the comparison was run.

## Try it

1. At ground level on the centreline (`y = z = 0`), the plume concentration
   is `Q / (pi sigma_y sigma_z u) exp(-H^2 / 2 sigma_z^2)`. Show this from the
   formula above.
2. Suppose `sigma_y` is proportional to `sigma_z` as both grow with `x`.
   Which `sigma_z` maximises the ground-level concentration? (Answer:
   `sigma_z = H / sqrt(2)`.) Use it to explain why a taller stack moves the
   ground-level peak further downwind.
