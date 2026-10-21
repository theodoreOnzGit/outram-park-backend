# Rung 2: how wide? Stability classes and sigma curves

> **Research, education and V&V only.** Nothing on this page is for emergency
> planning or response, dose assessment for real people, licensing or any
> safety decision (`RESPONSIBLE_USE.md`).

> **Review status:** AI-assisted first draft, 2026-10-04, not yet reviewed by
> a human. Built from commit
> [`@@COMMIT_SHORT@@`](https://github.com/theodoreOnzGit/outram-park-backend/commit/@@COMMIT@@).
> [It doesn't tally](https://github.com/theodoreOnzGit/outram-park-backend/issues/new?title=Dispersion%20rung%202%20doesn%27t%20tally).

> **Demo:** [open this rung in the dispersion demo](../../../demos/dispersion/?rung=sigmas): both sigma sets on log-log axes for the class you pick: changi's Martin/ISC fits and buangkok's BARC/AERB fits. Every calculation runs in a background worker, so the page stays live; the demo's "What's happening here?" link opens this page.

## The problem

Rung 1 ended with every number riding on two curves, `sigma_y(x)` and
`sigma_z(x)`. The same stack, the same release and the same wind give a
ground-level concentration that differs by **orders of magnitude** between a
sunny afternoon and a clear night (rung 1's table: at 400 m, class C gives
`6.0e-5` s/m³ and class F `7.7e-8`). **Where do the curves come from, and
which ones does this workspace use?**

---

## Step 1. Why does the time of day matter?

**Short answer.** The width of a plume is set by turbulence, and turbulence
comes from two sources: the **wind shearing past the ground** (mechanical)
and **the sun heating the ground** (convective). On a sunny afternoon warm
air rises in plumes of its own and stirs the release up and down fast: the
air is **unstable**. On a clear night the ground cools, a layer of cold air
sits under warm air, and vertical motion is damped: the air is **stable**. A
strong wind mixes mechanically and pushes both towards **neutral**.

Pasquill sorted this into six classes, **A** (very unstable) to **F**
(moderately stable), read from wind speed and time of day (and, in the full
scheme, cloud cover and sun angle).

**The formula.** There is none: it is a lookup table. `changi`'s puff port
reproduces upstream R `puff`'s version, where day is a fixed clock window,
07:00 to 18:00 inclusive:

| wind speed `U` (m/s) | day | night |
|---|---|---|
| `U < 2` | A, B | E, F |
| `2 <= U < 3` | B | E, F |
| `3 <= U < 5` | B, C | D, E |
| `5 <= U < 6` | C, D | D |
| `U >= 6` | D | D |

Six of the ten cells give **two** classes, because Pasquill's table gives a
range. The port puts that in the type: a
[`StabilitySet`](../../../api/changi/puff/stability/enum.StabilitySet.html)
is either `One(class)` or `Two(class, class)`. Rung 4 shows why that mattered:
upstream mishandles the second class in two different ways.

**Animation.** [In the demo](../../../demos/dispersion/?rung=sigmas&step=day-night): the same plume at 14:00 and at 02:00, same wind,
side by side, each class picked by `changi`'s `stability_class` and each map
computed with `buangkok`'s plume.

**In the code.** From the puff example's first part down to the lookup:

> **On the [Code Review map](../../../code-review/):** start at [`main`](../../../code-review/#crates/changi/examples/puff_site_survey.rs::main) and follow its callees (▸) to [`stability_class`](../../../code-review/#crates/changi/src/puff/stability.rs::stability_class). Each card opens the function's source and shows its review stamp.

The lookup itself:

```rust,ignore
{{#include ../../../../src/puff/stability.rs:206:248}}
```

**The forks.** `None` for the wind (a missing value) returns neutral D, as
upstream does; a negative wind speed **panics** in the port, where upstream
would silently classify it as calm (a plausible-looking wrong answer). Day or
night is [`is_day`](../../../api/changi/puff/stability/fn.is_day.html), a
fixed clock window rather than a solar-angle calculation: right for upstream's
near-field use, wrong at high latitude or out of season, and stated so in its
doc comment.

`buangkok`'s plume does **not** derive the class: it takes it from the caller
or, for long-term releases, from a year of hourly records that already carry a
stability code ([met processing](../ext/met-processing.md)).

**Predict.** For the same distance, which class gives the widest plume
vertically, A or F? And which gives the higher ground-level concentration
from a 30 m stack at 200 m?

---

## Step 2. How wide: the sigma curves

**Short answer.** For each class, `sigma_y` and `sigma_z` are **empirical
fits** to tracer releases, power laws in distance. They are not derived from
the advection–diffusion equation: rung 1's `sigma ~ sqrt(x)` holds only for
constant diffusivity, which the atmosphere does not have.

**The formula.** The workspace carries **two published sets**, one per port,
and they are deliberately kept separate:

```text
changi::puff (Martin 1976 forms, as in the US EPA ISC models; x in km)

  sigma_z = min( a x^b , 5000 m )         (a, b) from a per-class table binned by x
  sigma_y = 465.11628 x tan(theta)          theta = 0.017453293 (c - d ln x)

buangkok::pydoseia (BARC/AERB power laws; x in m)

  sigma_y = A_y x^0.9031                    one A_y per class
  sigma_z = A_z x^q + r                     three bands: x < 100 m, 100-1000 m, > 1000 m
```

Two numbers in the `changi` set deserve a remark. `465.11628` is
`1000 / 2.15`: kilometres to metres, divided by the 2.15 that turns the
Pasquill–Gifford nomograms' half-width into a standard deviation. And
`0.017453293` is upstream's degrees-to-radians, which differs from `pi / 180`
in the ninth significant figure (relative `2.8e-8`); the port keeps upstream's
literal because the code-to-code comparison sees the difference.

**Animation.** [In the demo](../../../demos/dispersion/?rung=sigmas): the `sigma_y` and `sigma_z` curves on log-log axes, both sets, for the class you pick. Class C
of `changi`'s set is one straight line (it is unbinned), the others show their
bin breakpoints as slope changes.

**In the code.** `changi`'s sigma evaluation, from the same example:

> **On the [Code Review map](../../../code-review/):** start at [`main`](../../../code-review/#crates/changi/examples/puff_site_survey.rs::main) and follow its callees (▸) to [`pasquill_gifford_sigmas`](../../../code-review/#crates/changi/src/puff/dispersion.rs::pasquill_gifford_sigmas). Each card opens the function's source and shows its review stamp.

and `buangkok`'s, from rung 1's example:

> **On the [Code Review map](../../../code-review/):** start at [`main`](../../../code-review/#crates/buangkok/examples/plume_chi_over_q.rs::main) and follow its callees (▸) to [`sigma_z`](../../../code-review/#crates/buangkok/src/pydoseia/dispersion.rs::sigma_z). Each card opens the function's source and shows its review stamp.

The `changi` evaluation:

```rust,ignore
{{#include ../../../../src/puff/dispersion.rs:228:262}}
```

What to notice:

- **The distance is a `uom::Length`.** The fits are in kilometres, and the
  conversion happens inside the function; upstream takes a bare number and
  divides by 1000 inside its kernel.
- **A puff that has not moved has no sigma.** Upstream returns `NA`; the port
  returns `None`, which the caller must handle.
- **Neither set is clamped to its fitted range.** Both are fitted over roughly
  0.1–10 km and both are applied at any distance. `changi` caps `sigma_z` at
  5000 m (upstream's stand-in for the mixing height) and does not cap
  `sigma_y`.

**Predict.** Both sets claim to describe the same six classes. At 1 km in
class D, how close do you expect them to be: 1 %, 10 %, a factor of two?

---

## Step 3. Two sets, one class: how far apart are they?

**Short answer.** Mostly within 10 % in the plume they produce, and in a few
corners by a factor of 2.5 to 700. Choosing a sigma set is a modelling
decision of that size.

**The measurement.** `crates/buangkok/tests/changi_puff_train_vs_plume.rs`
(gh:#380), its fourth test, **reported and not gated**: 6 classes × 7
distances, ground-level centreline, a 40 m stack. Recorded 2026-09-29 in the
test's module doc, and **re-run for this lesson** (see below):

- `sigma_y` within **4–15 %**, `sigma_z` mostly within **4 %**, so the two
  plumes mostly agree within **10 %** (up to about 30 % near the stack) over
  0.1–10 km;
- **class A beyond about 5 km**: the plumes differ **2.5–8×**, because
  `changi` caps `sigma_z` at 5000 m and pyDOSEIA does not;
- **stable classes near an elevated stack**: class F at 120 m differs about
  **700×**, because a small difference in `sigma_z` is exponentiated by
  `exp(-H^2 / 2 sigma_z^2)` deep in the plume's vertical tail.

**Re-run for this lesson** (`develop` `d4428668be`, one core (core 12, `-j 1`), 2026-10-04; the four tests of the file took 16.1 s):
every number above reproduced. Selected rows of the printed table, the ratio
of the two plumes (pyDOSEIA over `changi`) at a 40 m stack, ground level:

| Class | 120 m | 1.5 km | 5 km | 8.5 km |
|---|---|---|---|---|
| A | 1.00 | 1.11 | 0.40 | 0.13 |
| D | 0.80 | 0.91 | 0.90 | 0.91 |
| E | 1.29 | 0.98 | 0.95 | 0.96 |
| F | 1.4e-3 | 1.06 | 0.93 | 0.92 |

Class F at 120 m is the 700× case (`1 / 1.4e-3`); class A at 5 and 8.5 km is
`changi`'s 5000 m `sigma_z` cap.

**Why it matters.** The near-stack, stable case is exactly where a ground
release or a low stack puts the highest concentrations (rung 1's peak table:
class F peaks at about 1.6 km for a 30 m stack). A consequence number quoted
without naming its sigma set carries an unstated uncertainty of this size.

---

## Step 4. The check

Both sigma sets are checked **code-to-code against their own upstream**,
which is verification of the translation, not of the fits.

- **`changi::puff` against the upstream R** (recorded 2026-09-21,
  [`docs/puff-code-to-code.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/docs/puff-code-to-code.md)):
  stability decisions, 1 539 cases, exact; `compute_sigma_vals`, 234 cases for
  each sigma, **bit-exact**. Upstream's own documented example,
  `compute_sigma_vals("A", 0.7)`, gives `sigma_y = 152.3098 m`,
  `sigma_z = 213.3275 m`, and the port's unit test reproduces it. A mutation
  that replaces `0.017453293` by `pi / 180` is caught.
- **`buangkok` against pyDOSEIA** (recorded 2026-09-28): `sigmay` and
  `sigmaz`, 66 cases each, exact (rung 1's table).

**Re-run for this lesson** (`develop` `d4428668be`, one core (core 12, `-j 1`), 2026-10-04): `cargo test --release -p changi --test
puff_code_to_code`, **18 of 18 pass** in 0.69 s, so the 2026-09-21 record
still holds at this commit.

**Validation:** none. Neither set has been compared, in this workspace, with
the tracer data it was fitted to.

**Predict (into rung 3).** A real stack emits warm gas at several metres per
second. Does the plume's centreline stay at the stack height `H`?

---

## Deliberate liberties

| Liberty | Whose | What it may cost |
|---|---|---|
| Day = 07:00–18:00 fixed window | upstream `puff` | Wrong at high latitude or out of season; not measured here |
| Two classes kept as a pair; the port disperses with the more unstable one by default | the port (rung 4) | Upstream's handling differs by 1.04–3.97× (rung 4) |
| Fits applied outside 0.1–10 km, unclamped | both upstreams | Extrapolation; `changi`'s 5000 m `sigma_z` cap changes class A by 2.5–8× past 5 km |
| Sampling-time correction to `sigma_y` not applied | pyDOSEIA | Not measured here |

## History

> **[HISTORY: needs a source]** Pasquill's 1961 classification, Gifford's
> curves and the tracer campaigns behind them (Prairie Grass, 1956) are the
> hook for this rung. No source for them is in the open corpus yet; the
> maintainer supplies literature, so this stays a placeholder.

## Call-tree appendix

The whole of `changi`'s puff example, as an architecture map (generated):

> **On the [Code Review map](../../../code-review/):** start at [`main`](../../../code-review/#crates/changi/examples/puff_site_survey.rs::main). Each card opens the function's source and shows its review stamp.

**Next:** [Rung 3: how high? Plume rise, ground reflection, building wake](./03-rise-wake.md).
