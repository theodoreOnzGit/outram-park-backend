# FLEXPART code-to-code verification

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

## Methodology

Every reference value is produced by **compiling and running the upstream
FLEXPART Fortran**, not by re-deriving the physics and not by transcribing
numbers from a paper.

```
dev/flexpart_reference.f90   driver; links upstream routines VERBATIM
        + dev/build_reference.sh
            -> tests/data/flexpart_reference_real4.csv   as FLEXPART ships
            -> tests/data/flexpart_reference_real8.csv   -fdefault-real-8
                    -> tests/flexpart_code_to_code.rs    replays both
```

The driver links these upstream files unmodified: `par_mod.f90`, `ew.f90`,
`dynamic_viscosity.f90`, `psim.f90`, `psih.f90`, `scalev.f90`, `raerod.f90`,
`obukhov.f90`, `erf.f90`, `part0.f90`. The only local Fortran is
`dev/class_gribfile_shim.f90`, which supplies the single integer **parameter**
`obukhov.f90` takes from `class_gribfile` — the real module additionally
`USE grib_api`, dragging in ecCodes for no benefit here. Because that symbol is
a compile-time parameter with the identical upstream value
(`GRIBFILE_CENTRE_ECMWF = 2`), `obukhov.f90` compiles to the same code either
way; the routine under test is upstream's, untouched.

Upstream: `github.com/flexpart/flexpart`, **v10.4 (2019-11-12), commit
`3d7eebf`**, GPL-3.0-or-later. The clone lives at
`upstream_source/FLEXPART`, is gitignored, and is never compiled into the crate.

```bash
./dev/build_reference.sh                       # regenerate both fixtures
cargo test --release -p changi --test flexpart_code_to_code -- --nocapture
```

## Why two references: FLEXPART ships in single precision

**FLEXPART's makefile passes no `-fdefault-real-8`.** Its default `real` is
therefore `real(4)`. `par_mod.f90` writes `pi = 3.14159265` and stores
`3.14159274…`.

A `f64` port cannot agree with a stock FLEXPART build to better than about
`1e-7`, no matter how perfect the translation. Comparing against only the
shipped build would conflate two entirely different things — a translation
error, and upstream's storage format — so the driver is built **twice** and the
test checks both:

- **against `real8`** — agreement to near machine precision proves the
  **translation**, because the only remaining difference is floating-point
  operation order;
- **against `real4`** — the residual then **measures FLEXPART's own precision**,
  reported rather than assumed.

This split is what makes the result interpretable. Without it, a `3e-6`
disagreement would be indistinguishable from a bug.

## Results

Taken **2026-09-15**, upstream `3d7eebf`, **1 756 cases per fixture, 13 tests,
all passing**.

| Function group | Cases | vs `real8` | vs `real4` |
|---|---:|---:|---:|
| `ew` (saturation vapour pressure) | 33 | **0 (bit-exact)** | 1.74e-6 |
| `viscosity` (Sutherland) | 31 | **0 (bit-exact)** | 1.68e-7 |
| `psim` (MO momentum) | 72 | **0 (bit-exact)** | 6.49e-8 |
| `psih` (MO heat) | 72 | **0 (bit-exact)** | 5.00e-8 |
| `scalev` (friction velocity) | 108 | **0 (bit-exact)** | 1.25e-7 |
| `obukhov` NCEP path | 28 | **0 (bit-exact)** | 1.63e-7 |
| `obukhov` ECMWF path | 28 | 1.86e-16 (1 ulp) | 1.42e-7 |
| `raerod` (aerodynamic resistance) | 160 | **0 (bit-exact)** | 1.44e-6 |
| `part0.fract` (mass fractions) | 396 | **0 (bit-exact)** | 3.36e-6 |
| `part0.schmi` (Schmidt factor) | 396 | **0 (bit-exact)** | 5.51e-7 |
| `part0.vsh` (settling velocity) | 396 | **0 (bit-exact)** | 7.13e-7 |
| `part0.cun_scalar` (upstream scalar `cun`) | 36 | **0 (bit-exact)** | 6.44e-8 |

**Interpretation.** Against the double-precision build of the same Fortran,
**11 of 12 groups are bit-exact** and the twelfth differs by a single ulp, in
the ECMWF branch of `obukhov` where the hybrid-coefficient average introduces
one extra rounding. The translation is therefore not merely close but, on this
input grid, identical. Against FLEXPART as it actually ships, the spread of
5.0e-8 to 3.4e-6 is the `real(4)` precision band and nothing else.

`part0.fract` being bit-exact against `real8` is worth noting: that quantity is
computed from `erf`, and the port uses **`petir`'s GSL-derived `erf`** rather
than porting FLEXPART's own Numerical-Recipes-style `erf.f90`. The two
implementations agree to the last bit on all 396 cases, so the substitution
demanded by the workspace "reuse before porting" rule costs nothing here. That
was measured, not assumed — the test's tolerance for this group was deliberately
left looser than the others in case it did not hold.

### Two groups need an absolute floor, and why

`psim` and `psih` cancel to near zero near neutral stratification. The test's
`real4` check therefore passes on **either** a relative bound **or** an absolute
floor, with the floor derived from the measured term scale — not chosen to make
a test pass.

- **`psim`** at `z = 0.01 m`, `L = -10000 m`: terms of O(1.57)
  (`ln(a1 a2)`, `2 atan(x)`, `pi/2`) sum to `3.75e-6`. Cancellation ratio
  **4.19e5**, so the `real(4)` relative noise floor is
  `eps_f32 x 4.19e5 = 5.0e-2`. FLEXPART's answer there is exactly `2^-18` —
  fully quantised, no significant digits left. Floor: `1e-6` absolute.
- **`psih`** at `z = 0.01 m`, `L = 1 m`: the stable branch carries the fixed
  constant `b c / d = 0.667 x 5 / 0.35 = 9.5286`, and the four terms sum to
  `-4.996e-2`. Cancellation ratio **191**, absolute noise floor
  `eps_f32 x 9.5286 = 1.14e-6`; the measured disagreement is `1.03e-6`, inside
  it. Floor: `5e-6` absolute, a small safety factor over the bound.

Because the intermediate terms are O(1) in both cases, an absolute floor of
`1e-6`/`5e-6` is still a *relative* `1e-6`/`5e-6` on any quantity of normal
magnitude, so it does not weaken the test where the arithmetic is well
conditioned. The `real8` bounds stay tight and pass.

### The suite is not vacuous

Demonstrated by mutation on 2026-09-15. Four mutations, and the attribution came
out exactly right:

| Mutation | Groups that failed |
|---|---|
| `PI` `3.14159265` -> `3.14159266` | `psim`, `part0.schmi` |
| `psim` stable coefficient `-4.7` -> `-4.701` | `psim` |
| `ew` scale `101324.6` -> `101324.7` | `ew`, `scalev`, `obukhov.ncep`, `obukhov.ecmwf` |
| `part0` settling divisor `18.0` -> `18.01` | `part0.vsh` |

Seven groups failed and six stayed green, each correctly: `viscosity`, `psih`
and `raerod` use none of the mutated constants; `part0.fract` depends only on
`erf`; `part0.cun_scalar` uses neither `PI` nor the settling divisor. The `ew`
mutation propagating into `scalev` and both `obukhov` paths is the expected
dependency chain, since both call `ew` for the vapour pressure. All mutations
were reverted and the tree re-verified.

## Upstream findings

Reading upstream first — the workspace rule — turned up two things worth
recording.

### `part0` returns a Cunningham factor for only the largest bin

`part0.f90` declares `cun` as a **scalar** output but assigns it inside the
per-class loop, so on return it holds only the **last (largest)** class's value.
Its caller, `readreleases.f90:338`, then computes
`cunningham = sum_j(cun · fract_j)` — mass-weighting a value that is not
per-class, which collapses to `cun_last · sum(fract)`.

The port returns the genuine per-class array, which is what the physics needs,
and exposes upstream's scalar separately as
`AerosolBins::upstream_scalar_cunningham()` so code-to-code comparison stays
like-for-like. That accessor is pinned by its own test, which is what makes this
finding falsifiable rather than an assertion.

### `psih` mutates its `Obukhov length` argument

`psih.f90` nudges a near-zero `l` to `±1e-20` **in place**. Fortran arguments
are by reference, so an upstream caller passing a variable with `|L| < 1e-20`
finds it modified on return. The Rust port takes `L` by value and nudges its
local copy: identical arithmetic, no caller-visible side effect. No in-tree
caller relies on the mutation (`raerod.f90` passes its own `l` straight
through), so the port drops it — recorded here rather than silently. The Fortran
driver passes a copy for the same reason, so the fixture records the argument
the caller actually supplied.

## What this does NOT establish

**Verification, not validation.** It establishes that the Rust computes what the
Fortran computes. It says nothing about whether FLEXPART's parameterisations
reproduce measured atmospheric dispersion — that needs field data, and is not
claimed anywhere in this crate.

Per the crate's binding scope limit, nothing here supports emergency planning,
emergency response, dose assessment for real populations, or operational Level 3
PSA. The `Bookkeeping status` block in the README records that human review of
both the V&V and the interface is still outstanding.

Not covered, and tracked in beads:

- the particle advection loop (`advance.f90`) and the Hanna turbulence
  parameterisation — the heart of the Lagrangian model, and the point at which
  the RNG and `boon-lay`'s prior art become relevant;
- the convective boundary-layer scheme (`cbl.f90`);
- wet scavenging (`wetdepo.f90`, `get_wetscav.f90`) and the dry-deposition
  velocity assembly (`getvdep.f90`);
- the Richardson-number mixing-height diagnostic (`richardson.f90`);
- `get_settling.f90`'s ambient rescaling of the reference-state settling
  velocities `part0` returns;
- the GRIB/NetCDF meteorological readers, the output grids, and the OH chemistry.

Radioactive decay is ported but has **no Fortran fixture**: upstream computes it
inline in `readreleases.f90:317` and `timemanager.f90:275` rather than in a
callable routine, so there is nothing to link against. It is checked against
those upstream *expressions* instead, which the test states plainly rather than
presenting as code-to-code.
