# What has been checked, and what has not

> **Review status:** AI-assisted first draft (2026-10-03; section 5 and the
> open items updated 2026-10-04), not yet reviewed by a human.

Two words, used strictly:

- **Verification**: does the code compute what its specification says? For a
  port, the specification is upstream.
- **Validation**: does the model represent the real atmosphere well enough for
  its purpose? That needs measured tracer-release data.

**Everything on this page is verification. No part of `changi` or `buangkok`'s
plume has been validated against measured dispersion.**

## 1. `puff` against the upstream R

**Method.** `dev/gen_puff_reference.R` sources upstream's own R files from the
`puff` package at commit `5213d58`, sweeps each function over a grid placed on
and around every branch, band edge and bin cutoff, and writes the results at
round-trip precision. `tests/puff_code_to_code.rs` replays all of them through
the port. No expected value was written by hand.

**Results** (recorded 2026-09-21, 10 182 cases, 18 tests;
[`docs/puff-code-to-code.md`, lines 42–75](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/docs/puff-code-to-code.md#L42-L75)):

| Group | Cases | Worst relative deviation |
|---|---:|---:|
| stability decisions (`is_day`, `get_stab_class`) | 1 539 | exact |
| `compute_sigma_vals` (`sigma_y`, `sigma_z`) | 234 each | **0 (bit-exact)** |
| `gpuff` | 6 720 | 6.28e-16 |
| `simulate_sensor_mode` | 130 | 4.23e-16 |
| `simulate_grid_mode` | 720 | 3.26e-16 |

Upstream R is double precision, like the port, so there is no precision floor
and the tolerances sit near machine epsilon.

**Is the suite capable of failing?** Nineteen deliberate mutations of the
port, each run against its test, **all nineteen killed**. They include
replacing `0.017453293` with `PI/180`, dropping the ground-reflection image,
moving one stability band edge, and flipping the default emission policy
([lines 96–129](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/docs/puff-code-to-code.md#L96-L129)).

**What it found.** Five upstream defects ([puff train](./puff-train.md)), and three defects in the
port itself, each caught by a fixture row rather than by inspection.

**What it cannot find.** The fixture's wind is constant throughout. Defect 5,
the puff that never turns, is invisible on a constant wind and was found from
a user report.

## 2. FLEXPART kernels against the compiled Fortran, built twice

**Method.** Each stage has a Fortran driver that links upstream FLEXPART
routines **verbatim** and calls them over a branch-covering grid. FLEXPART
ships in **single precision**, so every driver is built twice: as shipped
(`real(4)`) and with `-fdefault-real-8`. Agreement with the `real(8)` build
checks the **translation**. The residual against `real(4)` measures
**FLEXPART's own precision**. Without the split, a `3e-6` disagreement would
be indistinguishable from a bug
([`docs/flexpart-code-to-code.md`, lines 49–68](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/docs/flexpart-code-to-code.md#L49-L68)).

**Results.**

- **First kernels** (2026-09-15, 1 812 cases, 14 function groups): **all 14
  bit-exact against `real(8)`**. Against the shipped `real(4)` build, the
  spread is 5.0e-8 to 3.4e-6, which is the single-precision band and nothing
  else
  ([lines 70–110](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/docs/flexpart-code-to-code.md#L70-L110)).
- **Stages 2–7** (2026-10-02): interpolation, met fields, the particle step,
  map projections, output, convection, release, vertical transform and output
  gridding. All bit-exact against `real(8)` except 5 of 7 320 particle-step
  outputs, which differ by at most 3.0e-15 because `petir`'s `erf` replaces
  FLEXPART's
  ([lines 389–402](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/docs/flexpart-code-to-code.md#L389-L402)).

**A translation defect the method caught.** The ECMWF branch of `obukhov` was
once off by one ulp, and that had been written off as upstream rounding. A
later stage showed it was a translation defect: Fortran's `theta*ustar**2` is
`theta*(ustar*ustar)`, and the port had written `(theta*ustar)*ustar`. Fixed,
it is bit-exact. **A one-ulp residual against `real(8)` is a defect until
shown otherwise.**

**The stochastic comparison** (stage 7). Random walks cannot be compared value
by value, so the port and upstream each release 20 000 particles in five
turbulence regimes, 16 replicates each. The criterion, fixed before running,
is that the port's mean and variance of displacement lie within 1σ of
FLEXPART's run-to-run spread. **Result: 30 of 30 within 1σ, χ² = 19.7 on 30
degrees of freedom against `real(4)`.** Getting there changed the port.
FLEXPART's `gasdev1` clips every normal deviate to [−3, 3], which narrows
every turbulent velocity distribution by 0.5 %. The deterministic comparison
could not see that, and the port now applies the same clip
([lines 490–537](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/docs/flexpart-code-to-code.md#L490-L537)).

## 3. `buangkok`'s plume against pyDOSEIA

The plume of [the plume page](./plume.md) is checked the same way, against upstream pyDOSEIA
executed on synthetic inputs (2026-09-28). The `sigmay`, `sigmaz`, height
factor, both master equations and the no-met dilution factors are **exact**,
within a dataset of 1 899 cases and 43 groups, all passing
([`buangkok/docs/pydoseia-code-to-code.md`, lines 205–232](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/docs/pydoseia-code-to-code.md#L205-L232)).

## 4. `activity`: no upstream, so an independent check

`changi::activity` was written here, so there is nothing to compare it with
code-to-code. Its first evidence was internal consistency: linearity,
conservation under re-segmentation, exact decay weights, and agreement with
`simulate_sensor_mode` to better than 1e-12
([`docs/activity-consistency-checks.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/docs/activity-consistency-checks.md)).

The stronger check (gh:#380) lives in
`crates/buangkok/tests/changi_puff_train_vs_plume.rs`, because `buangkok`
depends on `changi` and not the reverse. Its gates were fixed before it was
run
([lines 7–166](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/tests/changi_puff_train_vs_plume.rs#L7-L166)):

- **Gate 1: the puff train against an independent sum.** With a constant wind
  and a fixed class, every puff has the same history, so `chi/Q` is a plain
  sum over ages of the closed-form kernel. That sum can be written without any
  of `dilution_factors`' code. Bound `1e-11`, derived from the number of terms
  times machine epsilon. **Worst: 3.9e-16 over 84 cases.**
- **Gate 2: the puff train against the plume.** A puff train is *not* exactly
  a plume, because a puff at `x_p` carries `sigma(x_p)`, not `sigma(x_r)`. A
  second-order expansion predicts the residual `c2`, and the gate asks that
  the measured residual track it. **All 21 points in the expansion's validity
  domain pass**: for example, class D at 5 km measured −6.878e-4 against a
  predicted −6.876e-4.

**A gate that failed, recorded honestly.** The first gate compared the sum
with a quadrature and expected agreement near 1e-11. It **failed**, worst
2.1e-5. The derivation was wrong, not the code: `changi`'s binned `sigma_z`
has slope changes at its breakpoints, so the integrand is only piecewise
smooth. The gate was replaced before any result was recorded, and the
difference is kept as a measurement of the time-step error.

**The two sigma sets, compared** (reported, not gated). `changi`'s
Martin/ISC fits and pyDOSEIA's BARC/AERB fits give plumes that mostly agree to
within 10 % (up to about 30 % near the stack) over 0.1–10 km. The exceptions are class A beyond about 5 km
(2.5–8×, from `changi`'s 5000 m cap) and stable classes close to an elevated
stack (700× for class F at 120 m). **Choosing a sigma set is a modelling
decision of that size.**

## 5. Conservation checks added for the rung ladder (2026-10-04)

Two analytic checks that could have failed, written for the core lessons,
with methodology and results in each test's module doc:

- **The plume carries the whole release through every crosswind plane**
  (`crates/buangkok/tests/plume_mass_flux_conservation.rs`): `u` times the
  integral of `chi/Q` over `y` and `z >= 0` must be 1. Classes A–F at 0.2, 1
  and 5 km, criterion `1e-12` fixed before the run: **worst 3.9e-15**. Over
  the whole line in `z` the kernel carries 2 (the image is present).
- **One puff holds its mass** (`crates/changi/tests/puff_mass_conservation.rs`):
  the kernel integrated over `z >= 0` must be `Q`. Same grid of cases,
  criterion `1e-12`: **worst 2.4e-13**, larger than the predicted 1e-14; a
  compensated-summation probe brought it to 2.2e-16, so it is summation
  rounding, and the criterion was not moved.

## What remains open

- **No validation.** Nothing here has been compared with a measured tracer
  release.
- **`puff`'s fits are empirical over roughly 0.1–10 km** and are applied
  outside that range without a warning.
- **The `activity` layer's dry deposition is diagnostic and not depleting**,
  and it applies no wet scavenging. The deposition velocities are uncited
  placeholders.
- **No plume rise or building wake in any pathway**; the ported formulas are
  unchecked against their cited sources, with an open downwash-term question
  (gh:#542). **No daughter ingrowth beyond two steps, no iodine speciation.**
- **No plume depletion and no wet deposition** in the chain (gh:#543).
- **FLEXPART cannot yet read meteorology.** The GRIB/NetCDF readers and the
  file writers are not ported, so the verified kernels cannot be driven by
  real weather data.
- **Varying wind, wind-derived stability, multiple sources and deposition**
  are outside gh:#380's independent check, and rest on internal consistency
  only.
- **No human has reviewed this V&V yet.** The crate's bookkeeping status
  records that both axes are still unchecked.

And, once more: this is research and education software. None of it may be
used or presented as a tool for emergency planning, emergency response, or
dose assessment for real people.
