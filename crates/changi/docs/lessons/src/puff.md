# One puff: stability, sigmas and the kernel

## The problem

Instead of a steady stream, release a single parcel of mass `Q` at height `H`
and let the wind carry it. After it has travelled a distance `s`, what
concentration does it give at a receptor?

The answer needs three things, and `changi::puff` has one module for each:

| Step | Upstream R (`helpers.R`) | Rust |
|---|---|---|
| How turbulent is the air? | `is_day`, `get_stab_class` | [`puff::stability`](../../api/changi/puff/stability/index.html) |
| How wide has the puff grown? | `compute_sigma_vals` | [`puff::dispersion`](../../api/changi/puff/dispersion/index.html) |
| What concentration does it give? | `gpuff` | [`puff::concentration`](../../api/changi/puff/concentration/index.html) |

([`puff/mod.rs`, lines 44–52](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/puff/mod.rs#L44-L52))

## 1. Stability: a table with two answers

Pasquill sorted the atmosphere into six classes, from `A` (a hot, calm, sunny
afternoon: strong convection, fast vertical mixing) to `F` (a clear, calm
night: a stable layer that keeps the puff thin). Upstream picks the class from
the wind speed and whether it is day, where day is the fixed clock window
07:00–18:00 inclusive and not a solar calculation
([`stability.rs`, lines 154–174](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/puff/stability.rs#L154-L174)).

```rust,ignore
{{#include ../../../src/puff/stability.rs:206:248}}
```

The original Pasquill table gives a **range** of classes, and upstream keeps
the range: six of its ten regimes return two classes. The port makes that
impossible to ignore by putting it in the type. `StabilitySet` is either
`One(class)` or `Two(class, class)`, never an empty or open-ended list
([lines 87–99](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/puff/stability.rs#L87-L99)).
[The puff-train page](./puff-train.md) shows why that mattered: upstream itself mishandles the second
class, in two different ways.

## 2. Sigmas: empirical fits, used in kilometres

`sigma_y` and `sigma_z` are the standard deviations of the puff crosswind and
vertically. The fits are Martin's (1976) algebraic forms, as used in the US
EPA's ISC models, fitted to the Prairie Grass and related tracer campaigns
([`dispersion.rs`, lines 7–28](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/puff/dispersion.rs#L7-L29)):

```text
sigma_z = a x^b                      (x in km, capped at 5000 m)
sigma_y = 465.11628 x tan(theta),    theta = 0.017453293 (c - d ln x)
```

`(a, b)` come from a per-class table binned by distance, and `(c, d)` are
constant per class. Class `C` alone is a single unbinned power law
([lines 97–180](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/puff/dispersion.rs#L100-L183)).
The evaluation is short:

```rust,ignore
{{#include ../../../src/puff/dispersion.rs:228:262}}
```

Things to notice:

- **The distance is a `uom::Length`.** The fits are in kilometres, and the
  conversion happens inside the function, so a caller cannot pass metres by
  mistake. Upstream takes a bare number and converts it with a
  `total_dist / 1000` inside `gpuff`.
- **A puff that has not moved has no sigma.** Upstream returns `NA`. The port
  returns `None`, which the caller is forced to handle.
- **`0.017453293` is not `PI / 180`.** It differs in the ninth significant
  figure, a relative `2.8e-8`, which is visible in a code-to-code comparison.
  The literal stays, and a mutation test checks that replacing it would be
  caught
  ([lines 60–69](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/puff/dispersion.rs#L63-L72)).
- **The fits are only meaningful over roughly 0.1–10 km.** Upstream applies
  them at any distance and so does the port. `sigma_z` saturates at the
  5000 m cap. `sigma_y` has no cap, and for class D its angle `theta` only
  reaches zero at about 9.6e4 km, beyond any real use. Neither regime is
  clamped, because a range check upstream does not have would break the
  comparison. A caller who needs a bound applies it at the call site
  ([lines 202–223](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/puff/dispersion.rs#L205-L226)).

## 3. The kernel: a 3-D Gaussian with a mirror image

The puff is a three-dimensional Gaussian of total mass `Q`, centred at
`(x_p, y_p, H)`, plus an image at `-H` so nothing crosses the ground
([`concentration.rs`, lines 36–79](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/puff/concentration.rs#L36-L79)):

```text
C = Q / ((2 pi)^{3/2} sigma_y^2 sigma_z)
    * exp(-((x_r - x_p)^2 + (y_r - y_p)^2) / (2 sigma_y^2))
    * [ exp(-(z_r - H)^2 / (2 sigma_z^2)) + exp(-(z_r + H)^2 / (2 sigma_z^2)) ]
```

```rust,ignore
{{#include ../../../src/puff/concentration.rs:89:116}}
```

Compare it with [the plume](./plume.md):

- **There is no wind speed in it.** A puff is a snapshot, so the wind enters
  only through where the puff is (`x_p, y_p`) and how far it has travelled
  (which sets the sigmas). Upstream's `gpuff` does declare a `U` argument, but
  never reads it. The fixture confirms this: sweeping `U` over seven values,
  including zero and a negative speed, gives bit-identical answers. The port
  does not take `U` at all.
- **The horizontal spread is circular.** `sigma_y` is used for both `x` and
  `y`. That is upstream's modelling choice, not an oversight. A puff model
  represents spreading along the wind by the *spacing* of successive puffs,
  so giving each puff its own along-wind sigma as well would count it twice.
- **`travel_distance` is not the puff-to-receptor distance.** It is how far
  the puff has come from the source. That distinction is what [the puff-train page](./puff-train.md)'s
  wind-turning fix depends on.
- **An unmoved puff reports zero.** The sigmas are undefined at zero
  distance, and upstream maps the resulting `NA` to `0`. The doc comment says
  plainly that this is a modelling artefact, not physics. A freshly emitted
  puff has a very high concentration at its own centre, and this model reports
  none.

## Mass, not ppm

Upstream's `gpuff` returns parts per million **of methane**, using a factor
`1e6 × 1.524` that encodes methane's molar mass. For a radionuclide that is
wrong twice over: the molar mass is different, and an activity concentration
belongs in Bq/m³, not ppm. So
[`gaussian_puff_concentration`](../../api/changi/puff/concentration/fn.gaussian_puff_concentration.html)
returns a `MassDensity`, and the ppm version exists only so the port can be
compared with upstream
([lines 17–34](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/puff/concentration.rs#L17-L34)).

The ppm helper also teaches a `uom` lesson. Returning ppm as a `uom::Ratio`
divides by `1e6` to store it in base units, and puff concentrations far below
`1e-300` then fall into the subnormal range and lose digits. One fixture case
lost five significant figures with no warning, so the ppm helper returns a
bare `f64`
([lines 126–148](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/puff/concentration.rs#L126-L148)).

## Try it

1. Integrate the kernel over all space (keeping both the real Gaussian and
   its image in `z > 0`). Show that the total is `Q`, so the image exactly
   returns the mass that would have leaked below ground.
2. Read upstream's two documented examples, which the port reproduces in its
   unit tests: `compute_sigma_vals("A", 0.7)` gives `sigma_y = 152.3098 m`,
   `sigma_z = 213.3275 m`
   ([`dispersion.rs`, lines 269–278](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/puff/dispersion.rs#L272-L281)).
   Work out which `sigma_z` distance bin class A uses at 0.7 km.
