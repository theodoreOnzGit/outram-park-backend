# `changi::puff` — code-to-code verification against the upstream R

**Upstream:** [Hammerling-Research-Group/puff](https://github.com/Hammerling-Research-Group/puff)
`0.1.1`, commit `5213d58`, MIT.
**Date:** 2026-09-21. **Fixture:** 10 182 cases. **Tests:** 18, all passing.

## What this is

A Gaussian *puff* forward dispersion model, ported from R into `src/puff/` and
verified by **executing the upstream R itself** and replaying its output through
the port. No number in the fixture was transcribed from the paper, and none was
re-derived from the physics — the whole point of a code-to-code comparison is
that upstream is the specification.

The harness is `dev/gen_puff_reference.R`. It sources `R/helpers.R`,
`R/simulate_sensor_mode.R` and `R/simulate_grid_mode.R` from the gitignored
clone at `upstream_source/puff`, sweeps each function over a grid chosen to
straddle every branch, band edge and bin cutoff, and writes
`tests/data/puff_reference.csv` at `%.17g` round-trip precision.
`tests/puff_code_to_code.rs` replays it.

This complements the FLEXPART verification in `flexpart-code-to-code.md` rather
than replacing it. FLEXPART is a Lagrangian *particle* model on gridded
meteorology; this is an analytic *puff* model on a single wind series, cheap
enough to run over a site-sized domain with no meteorological files.

## Why an R harness

The same reasoning as `crates/boon-lay/dev/gen_triso_atops_reference.py` and
`crates/changi/dev/flexpart_reference.f90`: the harness must *run the upstream
reference implementation*, and upstream is written in R. Rewriting the harness
in another language would mean re-deriving the reference instead of measuring
it. The workspace's "no Python for documentation or accounting" rule scopes
itself to documentation generation and repository accounting and does not reach
V&V harnesses in any language.

Upstream's `simulate_*` functions call `dplyr::bind_rows`, so `dplyr` is
required; nothing else on the physics path needs a package. `ggplot2` and
`plotly`, upstream's other `Imports`, are used only by `R/plots.R`, which is
deliberately not ported.

## Results

Agreement runs from **bit-exact to 2.7e-15** relative. Upstream is
double-precision R and the port is `f64`, so — unlike the FLEXPART comparison,
where upstream's single-precision storage sets a `~1e-7` floor — there is no
precision gap to account for, and the tolerances sit near machine epsilon.

| Group | Cases | max_rel_dev | Tolerance |
|---|---:|---:|---:|
| `is_day` | 24 | **exact (decision)** | — |
| `get_stab_class.count` | 505 | **exact (decision)** | — |
| `get_stab_class.class1` | 505 | **exact (decision)** | — |
| `get_stab_class.class2` | 505 | **exact (decision)** | — |
| `compute_sigma_vals.sigma_y` | 234 | **0 (bit-exact)** | 1e-13 |
| `compute_sigma_vals.sigma_z` | 234 | **0 (bit-exact)** | 1e-13 |
| `wind_vector_convert.u` | 248 | 7.11e-16 | 1e-12 |
| `wind_vector_convert.v` | 248 | 2.06e-15 | 1e-12 |
| `interpolate_wind_data.u` | 30 | 2.66e-15 | 1e-12 |
| `interpolate_wind_data.v` | 30 | 2.42e-16 | 1e-12 |
| `gpuff` | 6 720 | 6.28e-16 | 1e-12 |
| `simulate_sensor_mode` | 130 | 4.23e-16 | 1e-11 |
| `simulate_sensor_mode.two_sources` | 25 | 2.74e-16 | 1e-11 |
| `simulate_grid_mode` | 720 | 3.26e-16 | 1e-11 |

The four classification groups are compared **exactly**, not to a tolerance:
the quantity under test is a decision, and a tolerance on a decision is
meaningless.

`compute_sigma_vals` being bit-exact over 234 cases spanning all six classes,
every distance-bin edge, the 5 000 m cap and the non-positive-distance `NA`
path is the strongest single result here — and it is what let the one initially
unexplained `gpuff` residual be diagnosed rather than papered over. See "Three
defects this found in the port itself", below.

## Branch coverage

* **`is_day`** — all 24 hours, so both inclusive bounds are on the grid.
* **`get_stab_class`** — 21 wind speeds × 24 hours, with speeds placed on both
  sides of and exactly on every band edge (2, 3, 5, 6 m/s), plus the
  missing-wind path. All three outputs compared: class count, first class,
  second class.
* **`compute_sigma_vals`** — all six classes over 39 distances placed on and
  around every cutoff in every class's table, into the saturated regime, at the
  non-positive distances upstream maps to `NA`, and at **every distance the
  `gpuff` sweep actually uses**.
* **`wind_vector_convert`** — 8 speeds × 31 directions including every cardinal
  and the 0°/360° wrap.
* **`gpuff`** — 6 classes × 4 masses × 7 travel distances × 4 release heights ×
  10 receptor positions, the receptors including the ground, the release height,
  well above it, and the puff's own centre.
* **Both simulate modes** — four scenarios spanning day and night, ambiguous and
  unambiguous stability, `puff_dt == sim_dt` and `puff_dt > sim_dt`, and a puff
  lifetime short enough to force expiry mid-run.

## The suite is not vacuous

Nineteen mutations, 2026-09-21. Each edits the port, runs the named test, and is
reverted. **All nineteen were killed.**

| Mutation | Test | Result |
|---|---|---|
| `is_day` upper bound made exclusive | `is_day_matches_upstream` | killed |
| stability band edge 3 → 3.5 m/s | `stability_primary_class_matches_upstream` | killed |
| night 2–3 m/s collapsed to one class | `stability_class_count_matches_upstream` | killed |
| `sigma_y` prefactor `465.11628` → `465.116` | `sigma_y_matches_upstream` | killed |
| `0.017453293` replaced by `PI/180` | `sigma_y_matches_upstream` | killed |
| `sigma_z` cap `5000` → `50000` | `sigma_z_matches_upstream` | killed |
| distance-bin edge `<=` → `<` | `sigma_z_matches_upstream` | killed |
| wind convention `270 -` → `90 -` | `wind_u_component_matches_upstream` | killed |
| R's `approx` node short-circuit removed | `interpolated_wind_u_matches_upstream` | killed |
| ground-reflection image term dropped | `gpuff_matches_upstream` | killed |
| methane factor `1.524` → `1.525` | `gpuff_matches_upstream` | killed |
| `(2 pi)^{3/2}` → `(2 pi)^{1/2}` | `gpuff_matches_upstream` | killed |
| unmoved puff returns `NaN` not `0` | `gpuff_matches_upstream` | killed |
| default emission policy flipped | `the_default_policy_emits_one_puff_per_event` | killed |
| bug-compatible policy stops doubling | `simulate_sensor_mode_matches_upstream` | killed |
| puff expiry `<=` → `<` | `simulate_sensor_mode_matches_upstream` | killed |
| sensor interval right-closed not left | `simulate_sensor_mode_matches_upstream` | killed |
| grid flatten order `x`/`z` swapped | `simulate_grid_mode_matches_upstream` | killed |
| grid mode fills its last output row | `simulate_grid_mode_last_row_is_never_written` | killed |

The `PI/180` mutation is worth singling out. Upstream writes the
degrees-to-radians factor as the 9-digit literal `0.017453293`, which differs
from `PI/180` in the 9th significant figure — a relative difference of `2.8e-8`,
four orders of magnitude above the tolerance. Substituting the "more accurate"
constant is exactly the kind of silent improvement a port should not make, and
the suite catches it.

## Upstream defects found

Four, all still present at `5213d58`. None has been reported upstream; the
workspace rule is that the deliverable for a third-party defect is a report, not
a patch, and the report has not been made yet.

### 1. `gpuff`'s `U` parameter is never read

`gpuff(Q, stab_class, x_p, y_p, x_r_vec, y_r_vec, z_r_vec, total_dist, H, U)`
declares `U`, documents it as *"Wind speed in m/s"*, and never mentions it in the
body. Wind speed reaches the calculation only through `stab_class`, which the
caller has already derived.

Demonstrated rather than asserted from inspection: the fixture sweeps `U` over
`{0, 1e-9, 1, 5, 100, 1e6, -7}` holding everything else fixed, and upstream's
answer is **bit-identical** at all seven, including at zero and at a negative
speed. Pinned by `gpuff_wind_speed_argument_is_inert`.

The port does not take `U` at all. This is the same shape as the inert pressure
argument recorded for TRISO-ATOPS in `crates/boon-lay`: a parameter that looks
like a physical control, carries a physical default, and cannot affect its own
answer.

### 2. An ambiguous stability class silently loses its second member

`compute_sigma_vals` returns a `2 x n` matrix — `rbind(sigma_y, sigma_z)` — and
`gpuff` reads it as

```r
sigma.y <- sigma.vec[1]
sigma.z <- sigma.vec[2]
```

R stores matrices **column-major**, so those two elements are `sigma_y` and
`sigma_z` **of the first class**. When `get_stab_class` returns two classes —
which it does in six of its ten regimes — the second is computed and discarded.

The port's `StabilitySet::primary()` names this rather than leaving it implicit,
and `gpuff_ambiguous_class_uses_only_the_first` verifies it against upstream for
every ambiguous pair the table can produce, so the accessor is measured rather
than assumed.

### 3. An ambiguous stability class doubles the emitted mass

The more consequential half of the same ambiguity. Both simulate drivers build
their puff record as

```r
new_puff <- data.frame(
  time_emitted = current_elapsed,            # length 1
  wind_u = wind_u_t, wind_v = wind_v_t,      # length 1
  wind_speed = wind_speed,                   # length 1
  stab_class = as.character(stab_class),     # length 1 OR 2
  mass = q_per_puff,                         # length 1
  stringsAsFactors = FALSE
)
```

R recycles the length-1 columns against a length-2 `stab_class`, so the frame
gains **two rows, each carrying the full `q_per_puff`**. Measured directly at
`5213d58`:

```
rows for a 2-class emission: 2
  time_emitted wind_u wind_v wind_speed stab_class mass
1            0      1      1        1.4          A 0.01
2            0      1      1        1.4          B 0.01
```

`q_per_puff <- (emission_rate / 3600) * puff_dt` plainly intends one puff's
worth of mass per emission interval, so the emitted mass is **doubled** wherever
the stability class is ambiguous. That is six of the ten (wind speed × day/night)
regimes — `U < 2` at any hour, `2 <= U < 3` at night, `3 <= U < 5` at any hour,
and `5 <= U < 6` by day — so it is the common case, not an edge case.

**This is the one place the port's default differs from upstream.** Mass
conservation is not a style preference, so `EmissionPolicy::OnePuffPerEmission`
is `Default` and `UpstreamRecycleStabilityClasses` must be named to get
upstream's behaviour. The code-to-code fixture is replayed with the
bug-compatible policy, because the fixture's job is to establish what upstream
computes; the corrected default is pinned separately by
`simulate::tests::the_default_policy_emits_one_puff_per_event`, and
`the_policies_agree_where_stability_is_unambiguous` bounds the divergence to
exactly the ambiguous regimes.

### 4. Grid mode never writes its last output row

`simulate_grid_mode` allocates `length(seq(start, end, by = output_dt))` rows
but writes only at

```r
if (t_idx %% (output_dt / sim_dt) == 0) {
  output_idx <- t_idx / (output_dt / sim_dt)
  concentrations[output_idx, ] <- grid_conc_step
}
```

with `t_idx` running `1..n_steps`, so the largest index reached is
`floor(n_steps / stride)` — one short. The final row keeps its initialised zero.
Reproduced and pinned by `simulate_grid_mode_last_row_is_never_written`, which
also checks upstream's own fixture rows for that row are all zero, so the claim
fails loudly if a future upstream version fixes it.

A reader meeting a row of zeros would reasonably conclude the plume had passed.

### Not a defect, but worth knowing: the two modes report different quantities

Sensor mode returns the **mean** concentration over each output interval
(`aggregate(..., FUN = mean)` over `cut(sim_timestamps, breaks =
output_timestamps)`); grid mode returns the **instantaneous** value at the step
that closes the interval. The two are not comparable at the same place and time,
and nothing in upstream's documentation says so.

## Three defects this found in the port itself

Recorded because the point of a verification pass is that it can fail, and this
one did — three times, each caught by a fixture row rather than by inspection.

1. **`uom`'s `Ratio` was silently truncating small concentrations.** Returning
   ppm as a `uom::Ratio` stores the magnitude in the base unit, so constructing
   one divides by `1e6` and reading it back multiplies by `1e6`. Puff
   concentrations go far enough below `1e-300` that the divided value lands in
   the **subnormal** range and loses mantissa bits the multiply cannot restore.
   Measured at `class A, Q = 1e-6 kg, travel = 1 m, receptor (-10, -10, 2)`:

   | | value |
   |---|---|
   | upstream R | `7.4821302396969955e-307` |
   | through `Ratio` | `7.48213023968e-307` |

   Five significant digits, gone silently. The port now returns ppm as a bare
   `f64` with the reason documented at the function, and keeps `MassDensity` —
   which does not round-trip — as the dimensioned return.

2. **Degrees-to-radians went through `uom` instead of upstream's arithmetic.**
   Upstream writes `theta <- (270 - wd) * pi / 180`: multiply by `pi`, *then*
   divide by 180. `uom`'s degree-to-radian conversion multiplies by a single
   pre-rounded `pi/180`, and the two round differently — measured at up to
   `1.3e-12` relative, about 4 000x machine epsilon. The port now does the
   arithmetic in upstream's order.

3. **R's `approx` short-circuits exact node hits, and the port did not.**
   `approx1` in R's `src/library/stats/src/approx.c` returns `y[i]` outright
   when `x[i] == v`, before evaluating the linear form. That is not an
   optimisation: `a + (b - a) * frac` at `frac == 1` cancels catastrophically
   when `|b| << |a|`. Measured with observations of 2 m/s from the east then
   3 m/s from the south, `b.u = 1.8369701987210297e-16` and the linear form
   returns exactly `0`. The port now reproduces the short-circuit.

The first of the three is the one worth generalising: **a units library can lose
precision, and it does so exactly where the value is least likely to be
checked.** `uom` is mandatory in this workspace and remains the right default —
but "dimensioned" is not the same as "lossless", and a quantity whose natural
magnitude is far from its base unit needs the round trip thought about.

A fourth near-miss is worth recording too. The `gpuff` residual in (1) was at
first unexplained, because the `compute_sigma_vals` comparison was bit-exact and
so appeared to rule out the dispersion coefficients. It did not: the sigma sweep
did not include `0.001 km`, which is the distance `gpuff` was being called at.
The grid now derives its distances from the `gpuff` sweep, so the two cannot
drift apart again. **A comparison that does not cover the inputs the caller
actually uses is not covering the caller.**

## What this does NOT establish

This is **verification, not validation**. It shows the Rust port computes what
the upstream R computes. It says nothing about whether *either* reproduces
measured atmospheric dispersion — that requires a tracer-release benchmark and
is not claimed here.

Per `RESPONSIBLE_USE.md`, AI-assisted output remains untrusted draft material
until a human reviews it, and that review is outstanding for this module. The
crate's scope limits are binding: CHANGI is for research, education and V&V
only, and must not be presented as, or used for, emergency planning, emergency
response, dose assessment for real populations, Level 3 PSA support, or any
safety-critical or licensing decision.

Two further limits specific to this module:

* **The Pasquill–Gifford fits are empirical over roughly 0.1–10 km.** The port
  applies them at any positive distance, as upstream does, and the result
  outside that range is defined but not meaningful.
* **The unit conversion is methane-specific.** Upstream's `1e6 * 1.524` encodes
  methane's molar mass. It is wrong for a radionuclide twice over — wrong molar
  mass, and ppm is the wrong unit for an activity concentration, which belongs
  in Bq/m^3. Use the mass-density return and convert with the species' own
  factor.
