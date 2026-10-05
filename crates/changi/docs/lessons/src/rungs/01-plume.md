# Rung 1: a steady stack, and the Gaussian plume

> **Research, education and V&V only.** Nothing on this page is for emergency
> planning or response, dose assessment for real people, licensing or any
> safety decision (`RESPONSIBLE_USE.md`). The numbers are per unit release
> and describe a model, not a site.

> **Review status:** AI-assisted first draft, 2026-10-04, not yet reviewed by
> a human. Built from commit
> [`@@COMMIT_SHORT@@`](https://github.com/theodoreOnzGit/outram-park-backend/commit/@@COMMIT@@).
> A claim the code contradicts is a defect:
> [it doesn't tally](https://github.com/theodoreOnzGit/outram-park-backend/issues/new?title=Dispersion%20rung%201%20doesn%27t%20tally).

> **Demo:** [open this rung in the dispersion demo](../../../demos/dispersion/?rung=plume): the steady plume on a map, with stability class, wind speed, release height and wind direction in the panel; the colours are buangkok's ground-level `chi/Q`, computed in a background worker as you move the sliders. Every calculation runs in a background worker, so the page stays live; the demo's "What's happening here?" link opens this page.

## The problem

A 30 m stack releases a gas at a steady rate `Q` into a steady 2 m/s wind.
**How much of it is in the air at ground level 1 km downwind?**

This rung answers with the oldest model in the field, the steady Gaussian
plume, and shows where the workspace computes it. Every later rung adds one
thing this rung leaves out: rung 2 asks where the plume's width comes from,
rung 3 asks how high it really flies, rung 4 lets the release start, stop and
turn.

---

## Step 1. What does the wind do to a steady release?

**Short answer.** Two things. The mean wind carries the gas downwind
(**advection**), and the turbulent eddies in the wind spread it sideways and
up and down (**turbulent diffusion**).

**The formula.** Write the concentration as `C(x, y, z)` with `x` downwind.
In steady state, with the eddies modelled as a diffusivity `K`:

```text
u dC/dx  =  K_y d2C/dy2  +  K_z d2C/dz2
```

One term has been dropped: diffusion **along** the wind, `K_x d2C/dx2`. In a
wind of a few metres per second, carrying beats along-wind spreading by a wide
margin, so the plume is *slender*. That is the **slender-plume
approximation**, and it is the reason `u` appears only once.

**Animation.** *Not yet built* ([gh:#548](https://github.com/theodoreOnzGit/outram-park-backend/issues/548)): tracer particles leaving a stack,
drifting with the wind and jittering across it. *Predict first:* if the wind
doubles, does the concentration 1 km downwind go up, down, or stay the same?

**The code walk.** *There is no function for this step.* Nothing in the
workspace solves this partial differential equation for the plume. The code
evaluates its **solution**, which is the next step. Saying so matters: the
plume model's physics lives in a formula, not a solver, and every assumption
in the next step is baked into it.

**Predict.** If you release a puff into air with constant `K` and follow it
downwind for a time `t = x / u`, how does its width grow with `x`?

---

## Step 2. Solve it: why the answer is a Gaussian

**Short answer.** Substitute `t = x / u`. The equation becomes the ordinary
diffusion equation in `y` and `z`, with downwind distance playing the part of
time. A point source of a diffusing substance spreads into a Gaussian whose
variance grows linearly with time.

**The formula.**

```text
d C / d t  =  K_y d2C/dy2  +  K_z d2C/dz2,      t = x / u

C(x, y, z) = Q / (2 pi sigma_y sigma_z u)  exp(-y^2 / 2 sigma_y^2)  exp(-(z - H)^2 / 2 sigma_z^2)

sigma_y^2 = 2 K_y x / u,      sigma_z^2 = 2 K_z x / u
```

The prefactor is fixed by **conservation**: every vertical plane across the
wind must carry the whole release, `u * (integral of C dy dz) = Q`. Each
Gaussian integrates to `sqrt(2 pi) sigma`, and their product cancels the
`2 pi sigma_y sigma_z u`.

With constant `K`, `sigma` would grow as `sqrt(x)`. Real plumes do not do
that: near the source the eddies that matter are smaller than the plume, and
they grow with it. So in practice `sigma_y(x)` and `sigma_z(x)` are not
derived from `K`. They are **fitted to tracer experiments**, per weather
class. That is rung 2.

**Animation.** *Not yet built* ([gh:#548](https://github.com/theodoreOnzGit/outram-park-backend/issues/548)): a crosswind slice of the plume at increasing `x`,
the Gaussian widening and flattening while its area (the flux) stays fixed.
*Predict first:* when `sigma_z` doubles at the same `sigma_y`, what happens to
the peak?

**The code walk.** No function yet: this is the derivation. The code that
evaluates the result is in step 3.

**Predict.** The ground is at `z = 0`. Nothing in the formula above stops the
Gaussian from extending below it. What happens to the material that would
have gone underground?

---

## Step 3. The ground: an image source

**Short answer.** A gas does not go into the ground. Mathematically, the
ground is a wall the plume cannot cross, and the standard way to enforce that
is a **mirror source** at `-H`. The mirror's plume reflects exactly the part
of the real plume that would have crossed `z = 0`.

**The formula.** The plume with ground reflection, divided by `Q`, is the
**dilution factor** `chi/Q`, in s/m³:

```text
chi/Q = 1 / (2 pi sigma_y sigma_z u)
        * exp(-y^2 / 2 sigma_y^2)
        * [ exp(-(z - H)^2 / 2 sigma_z^2) + exp(-(z + H)^2 / 2 sigma_z^2) ]
```

`chi/Q` is what consequence codes carry. It depends only on geometry and
weather, never on what was released: multiply it by the becquerels released
and you have the time-integrated air concentration, Bq·s/m³.

At ground level on the centreline (`y = z = 0`) the two exponentials are equal
and the formula collapses to

```text
chi/Q (ground, centreline) = exp(-H^2 / 2 sigma_z^2) / (pi sigma_y sigma_z u)
```

**Animation.** *Not yet built* ([gh:#548](https://github.com/theodoreOnzGit/outram-park-backend/issues/548)): the plume and its mirror image, with the part below
ground folded back up.

**The code walk.** The workspace's steady plume is `buangkok`'s port of
[pyDOSEIA](https://github.com/BiswajitSadhu/pyDOSEIA) (MIT, commit
`dca4cdc3`), which cites Hukkoo and Bapat's BARC manual, eq. 2.5, for this
formula. The example written for this rung asks for the ground-level
centreline value at each distance; the walk below goes from its `main` down to
the line that evaluates the two exponentials.

<!-- code-walk: from=crates/buangkok/examples/plume_chi_over_q.rs::main to=crates/buangkok/src/pydoseia/dispersion.rs::master_equation_single_plume -->
<!-- generated by `kovan-cli code-walk-check --update`; edit the comment above, not this -->

Call chain from `plume_chi_over_q.rs::main` to `dispersion.rs::master_equation_single_plume`: 3 hops, 1 shortest chain.

- [`plume_chi_over_q.rs::main`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_chi_over_q.rs#L102) `fn main()`
  - [`plume_chi_over_q.rs::chi_over_q`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_chi_over_q.rs#L97) `fn chi_over_q(x_m: f64) -> [f64; 6]` · called at [L113](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_chi_over_q.rs#L113)
    - [`dispersion.rs::dilution_single_plume_no_met`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L293) `pub fn dilution_single_plume_no_met(x: Length, geometry: PlumeGeometry, scaling: MeanSpeedScaling) -> [DilutionFactor; 6]` — Dilution factor for an **instantaneous (single-plume) release without met data**, one value per stability class A-F, s/m^3 (time-integrated concentration per Bq released, at unit wind speed times the height correction). · called at [L98](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_chi_over_q.rs#L98)
      - [`dispersion.rs::master_equation_single_plume`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L197) `pub fn master_equation_single_plume(sigma_y: Length, sigma_z: Length, speed_factor: f64, release_height: Length, receptor: Receptor) -> MasterEquationTerms` — Single (instantaneous / short-term) Gaussian plume, Hukkoo-Bapat eq. · called at [L302](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L302)
<!-- /code-walk -->

The function at the bottom, as it is in the source at this commit:

```rust,ignore
{{#include ../../../../../buangkok/src/pydoseia/dispersion.rs:192:214}}
```

What to notice:

- **The answer comes back in two pieces**, `pre_expo` and `expo`, exactly as
  upstream splits it. The port keeps upstream's operation order to the last
  multiplication so that it can be compared with pyDOSEIA bit for bit.
- **`speed_factor` is the wind at release height.** pyDOSEIA measures the wind
  at a reference height `H_m` (10 m here) and scales it by `(H / H_m)^p`, with
  `p` set by the stability class; a release below 10 m is raised to 10 m
  first. That is
  [`height_correction_factor`](../../../api/buangkok/pydoseia/dispersion/fn.height_correction_factor.html).
- **`Receptor::GroundLevelCentreline`** sets `y = z = 0`; `Receptor::Offset`
  takes any `(y, z)`.

**The branch you did not take.** `dilution_single_plume_no_met` is one of
four release modes. The others share the sigmas and the height correction but
use a different master equation or weighting:

| Mode | Function | Master equation | Use |
|---|---|---|---|
| single plume, no met data | `dilution_single_plume_no_met` | eq. 2.5 (this step) | a short release, one class at a time |
| long term, no met data | `dilution_long_term_no_met` | eq. 2.32, sector-averaged | a year of release, wind smeared over a 22.5° sector |
| long term, with met data | `dilution_long_term_with_met` | eq. 2.32, weighted by the joint frequency table | a year of release on a site's wind statistics ([met processing](../ext/met-processing.md)) |
| single plume, with met data | `dilution_single_plume_with_met_speeds` | eq. 2.5, divided by each class's mean speed | **cannot run upstream** (defect D3); the port's corrected version is a labelled divergence |

The sector-averaged equation replaces the crosswind Gaussian by a uniform
spread over the sector, `1 / (sqrt(2 pi) x theta sigma_z)`. Upstream writes
the sector width as `0.39275` rad where 22.5° is `0.392699…`, and the port
keeps upstream's constant (relative difference 1.3e-4), because a port that
"improves" upstream can no longer be checked against it.

**Predict.** Integrate `u * chi/Q` over a whole crosswind plane above the
ground. What number must come out, at every distance and in every class?

---

## Step 4. The check: does every plane carry the whole release?

**Short answer.** Yes, to rounding. And the port reproduces pyDOSEIA exactly.
Both are **verification**: the code against its own formula, and against the
code it was ported from. Neither is validation.

**Check 1: conservation, a test that can fail** (new for this lesson,
`crates/buangkok/tests/plume_mass_flux_conservation.rs`). For every class A–F
at 0.2, 1 and 5 km (one distance in each `sigma_z` band), the test integrates
`u * chi/Q` over `y` and over `z >= 0` with the trapezoid rule on a grid of
`sigma / 4`, which for a Gaussian is accurate to far below rounding. The
criterion, fixed before the run, is `|u * integral - 1| < 1e-12`. A second test
integrates over the **whole** line in `z` and must get **2**, because the
kernel carries the real plume and its image; a kernel that had lost its image
would get 1 there. The prediction written before the first run was "about
1e-15 everywhere".

**Result** (run for this lesson, 2026-10-04, on `develop` `d4428668be` plus
the new test, `cargo test --release -p buangkok --test
plume_mass_flux_conservation`, one core, under 0.01 s): **both tests pass**.
Over the 18 half-space cases `u * integral - 1` lies between `-3.9e-15` and
`+1.6e-15`, **worst 3.89e-15** (class F, 200 m), with no trend in class or
band; the low release integrated over the whole line gives
2.000000000000001. The prediction held. The record, with methodology, is the
test's module doc.

**Check 2: code-to-code against pyDOSEIA.** The upstream Python is executed
over a grid of inputs and the port must reproduce every number. Recorded
2026-09-28 (upstream `dca4cdc3`, Python 3.14.7, numpy 2.5.3, scipy 1.18.1),
[`crates/buangkok/docs/pydoseia-code-to-code.md`, "Results"](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/docs/pydoseia-code-to-code.md#results):

| Group | Cases | Max relative deviation |
|---|---:|---:|
| `sigmay` | 66 | 0 (exact) |
| `sigmaz` | 66 | 0 (exact) |
| `height_factor` | 60 | 0 (exact) |
| `master_single` | 10 | 0 (exact) |
| `master_sector` | 6 | 0 (exact) |
| `dilution_no_met` | 120 | 0 (exact) |

**Re-run for this lesson** (`develop` `d4428668be`, one core, 2026-10-04): `cargo test --release -p buangkok --test pydoseia_code_to_code`, **15 of 15 tests pass** in 100 s, so the 2026-09-28 record holds at this commit.

**What neither check can tell you** is whether the formula describes a real
plume. No part of this workspace's plume has been compared with a measured
tracer release. That is a validation gap, recorded on
[the V&V page](../vv-and-limits.md).

**Predict.** For a release at height `H`, where along the ground is the
concentration highest? Close to the stack, far away, or somewhere between?

---

## Step 5. Where is the ground-level maximum?

**Short answer.** Somewhere between. At the stack foot the plume has not yet
reached the ground (`exp(-H^2 / 2 sigma_z^2)` is tiny); far away it has spread
thin (`1 / sigma_y sigma_z` is small). In between there is a peak.

**The formula.** If `sigma_y` were proportional to `sigma_z`, setting the
derivative of the ground-level centreline `chi/Q` with respect to `sigma_z` to
zero gives

```text
sigma_z (at the peak) = H / sqrt(2)
```

so a taller stack moves the peak further downwind and lowers it. That is
the textbook rule. It is only a guide here, because pyDOSEIA's `sigma_y` and
`sigma_z` grow at different rates.

**Use, modify, create.**

- **Use.** Run the example written for this rung:

  ```bash
  cargo run --release -p buangkok --example plume_chi_over_q
  ```

  It prints `chi/Q` against distance for every class at a 30 m release in a
  2 m/s wind (at 10 m), and then where each class's ground-level peak falls,
  with `sigma_z` at the peak beside `H / sqrt(2)`.

  **Result** (run 2026-10-04, `develop` `d4428668be` plus the new example,
  one core; the record is the example's module doc):

  | class | peak at | peak `chi/Q` (s/m³) | `sigma_z` there | `H / sqrt 2` |
  |---|---|---|---|---|
  | A | 164 m | 7.01e-5 | 22.4 m | 21.2 m |
  | B | 216 m | 7.04e-5 | 21.6 m | 21.2 m |
  | C | 314 m | 6.52e-5 | 21.3 m | 21.2 m |
  | D | 571 m | 4.99e-5 | 20.4 m | 21.2 m |
  | E | 902 m | 3.67e-5 | 20.0 m | 21.2 m |
  | F | 1568 m | 3.07e-5 | 19.3 m | 21.2 m |

  The textbook rule holds to within 6 % (A) to 9 % (F). One surprise, kept
  because the run contradicted the first draft: for this elevated release the
  stable classes peak further out **and lower** than the unstable ones. At
  100 m, class F's ground-level `chi/Q` is `2e-41` s/m³: the plume has not yet
  reached the ground.

- **Modify.** Change `RELEASE_HEIGHT_M` to 60 and run it again. Before you
  run it, write down which way each class's peak moves and by roughly how
  much.
- **Create.** Add a column for `dilution_long_term_no_met` (the
  sector-averaged equation) beside the single-plume one. Which is larger at
  1 km for class D, and why must it be, given that the sector spreads the
  plume over a 22.5° arc?

**Predict (into rung 2).** The whole answer, including the position of the
peak, rides on `sigma_y(x)` and `sigma_z(x)`. Where do those curves come from,
and why does the time of day change them?

---

## Deliberate liberties in this rung's model

Each is pyDOSEIA's, kept by the port so that it can be compared with upstream:

| Liberty | Why it is taken | What it may cost |
|---|---|---|
| Steady state | The formula has no time in it | Cannot describe a release that starts, stops, or meets a wind that turns (rung 4) |
| Slender plume (no along-wind diffusion) | Advection dominates at a few m/s | Fails in calm air; pyDOSEIA treats calms separately ([met processing](../ext/met-processing.md)) |
| Power-law wind profile, release raised to 10 m | Upstream's height correction | Not measured here |
| Sampling-time correction to `sigma_y` computed but not applied | Upstream comments the multiplication out | Not measured here |
| `sigma_z` bands meet only approximately (jumps up to 0.84 %, class E at 1000 m) | Upstream's fit coefficients | A visible step at 100 m and 1 km |
| Sector width `0.39275` rad | Upstream's constant | Relative 1.3e-4 on the long-term mode |
| No plume rise, no building wake | Upstream's dose path uses the effective height as given | Rung 3 |

## History

> **[HISTORY: needs a source]** The Gaussian plume's development (Sutton's
> diffusion theory, Pasquill's 1961 stability classes, Gifford's curves, the
> Prairie Grass tracer campaign) is the natural hook for this rung. The open
> corpus does not yet hold a source for it, so no history is told here until
> the maintainer supplies one.

## Call-tree appendix

Everything the rung's example reaches inside the workspace, as an
architecture map (generated; calls into `std` and dependencies are filtered
out):

<!-- code-walk: from=crates/buangkok/examples/plume_chi_over_q.rs::main depth=4 -->
<!-- generated by `kovan-cli code-walk-check --update`; edit the comment above, not this -->

Everything `crates/buangkok/examples/plume_chi_over_q.rs::main` reaches in the workspace, to 4 hops: 15 functions, 1 unresolved call. A function is expanded once; later calls to it say *(expanded elsewhere in this walk)*.

- [`plume_chi_over_q.rs::main`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_chi_over_q.rs#L102) `fn main()`
  - [`plume_chi_over_q.rs::chi_over_q`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_chi_over_q.rs#L97) `fn chi_over_q(x_m: f64) -> [f64; 6]` · called at [L113](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_chi_over_q.rs#L113)
    - [`dispersion.rs::dilution_single_plume_no_met`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L293) `pub fn dilution_single_plume_no_met(x: Length, geometry: PlumeGeometry, scaling: MeanSpeedScaling) -> [DilutionFactor; 6]` — Dilution factor for an **instantaneous (single-plume) release without met data**, one value per stability class A-F, s/m^3 (time-integrated concentration per Bq released, at unit wind speed times the height correction). · called at [L98](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_chi_over_q.rs#L98)
      - [`dispersion.rs::height_correction_factor`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L137) `pub fn height_correction_factor(stability: StabilityClass, release_height: Length, measurement_height: Length) -> f64` — Wind-speed correction from measurement height to release height, `(H / H_m)^p` with `p = n / (2 - n)`, `n = 0.2` (A-C), `0.25` (D), `0.5` (E-F). · called at [L301](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L301)
      - [`dispersion.rs::master_equation_single_plume`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L197) `pub fn master_equation_single_plume(sigma_y: Length, sigma_z: Length, speed_factor: f64, release_height: Length, receptor: Receptor) -> MasterEquationTerms` — Single (instantaneous / short-term) Gaussian plume, Hukkoo-Bapat eq. · called at [L302](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L302)
        - [`dispersion.rs::Receptor::yz`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L174) `fn yz(self) -> (f64, f64)` · called at [L204](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L204) · *(calls below the depth limit not shown)*
      - [`dispersion.rs::sigma_y`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L94) `pub fn sigma_y(stability: StabilityClass, x: Length) -> Length` — Lateral plume spread `sigma_y = A_y x^0.9031`, m, for downwind distance `x`. · called at [L303](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L303)
        - `dispersion.rs::StabilityClass::index` *(expanded elsewhere in this walk)* · called at [L97](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L97)
      - `dispersion.rs::sigma_z` *(expanded elsewhere in this walk)* · called at [L304](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L304)
      - `dispersion.rs::StabilityClass::index` *(expanded elsewhere in this walk)* · called at [L309](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L309)
      - [`dispersion.rs::kqij`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L268) `fn kqij(terms: MasterEquationTerms, sumnu: f64, hours_denominator: f64) -> f64` — Frequency-weighted accumulation used by all three modes, in upstream's operation order: `KQIJ = pre * expo * SUMNU; KQIJ = (KQIJ * 3600) / (hours * 3600)`. · called at [L309](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L309)
      - [`dispersion.rs::apply_scaling`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L273) `fn apply_scaling(values: [f64; 6], scaling: MeanSpeedScaling) -> [DilutionFactor; 6]` · called at [L311](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L311)
        - UNRESOLVED(other): `default` at [L274](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L274) (→ [`crates/changi/src/activity/units.rs:41`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/activity/units.rs#L41)) — resolves to `#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]`, not a function body
        - [`units.rs::DilutionFactor::new`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/activity/units.rs#L47) `pub const fn new(seconds_per_cubic_meter: f64) -> Self` — From a bare value in seconds per cubic metre. · called at [L276](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L276) · *(calls below the depth limit not shown)*
    - [`plume_chi_over_q.rs::geometry`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_chi_over_q.rs#L85) `fn geometry() -> PlumeGeometry` · called at [L98](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_chi_over_q.rs#L98)
    - [`plume_chi_over_q.rs::scaling`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_chi_over_q.rs#L93) `fn scaling() -> MeanSpeedScaling` · called at [L98](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_chi_over_q.rs#L98)
    - [`units.rs::DilutionFactor::seconds_per_cubic_meter`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/activity/units.rs#L53) `pub const fn seconds_per_cubic_meter(self) -> f64` — The value in seconds per cubic metre. · called at [L99](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_chi_over_q.rs#L99)
  - [`dispersion.rs::StabilityClass::index`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L69) `pub const fn index(self) -> usize` — Zero-based index, 0 (A) to 5 (F). · called at [L133](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_chi_over_q.rs#L133)
  - [`dispersion.rs::sigma_z`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L107) `pub fn sigma_z(stability: StabilityClass, x: Length) -> Length` — Vertical plume spread `sigma_z = A_z x^q + r`, m, with three distance bands (`x < 100 m`, `100 <= x <= 1000 m`, `x > 1000 m`) exactly as upstream. · called at [L139](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_chi_over_q.rs#L139)
    - `dispersion.rs::StabilityClass::index` *(expanded elsewhere in this walk)* · called at [L109](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L109)
<!-- /code-walk -->

**Next:** [Rung 2: how wide? Stability classes and sigma curves](./02-sigmas.md).
