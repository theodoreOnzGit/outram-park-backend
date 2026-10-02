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

**Radioactive decay is the one exception, and it is handled explicitly rather
than skipped.** FLEXPART computes decay inline in caller code —
`readreleases.f90:317` (`decay(i)=0.693147/decay(i)`) and
`timemanager.f90:275` (`exp(-1.*outstep*decay(ks))`) — not in a standalone
subroutine, so there is no routine to link against verbatim. `emit_decay` in
the driver copies each line **byte-for-byte** out of the cited upstream source
line into its own small subroutine, compiled and run exactly like every other
group. What is under test is still upstream's literal expression, not a
Rust-side re-derivation of it.

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

Taken **2026-09-15**, upstream `3d7eebf`, **1 812 cases per fixture, 15 tests,
all passing**.

| Function group | Cases | vs `real8` | vs `real4` |
|---|---:|---:|---:|
| `ew` (saturation vapour pressure) | 33 | **0 (bit-exact)** | 1.74e-6 |
| `viscosity` (Sutherland) | 31 | **0 (bit-exact)** | 1.68e-7 |
| `psim` (MO momentum) | 72 | **0 (bit-exact)** | 6.49e-8 |
| `psih` (MO heat) | 72 | **0 (bit-exact)** | 5.00e-8 |
| `scalev` (friction velocity) | 108 | **0 (bit-exact)** | 1.25e-7 |
| `obukhov` NCEP path | 28 | **0 (bit-exact)** | 1.63e-7 |
| `obukhov` ECMWF path | 28 | ~~1.86e-16 (1 ulp)~~ **0 (bit-exact), CORRECTED 2026-10-02** | 1.42e-7 |
| `raerod` (aerodynamic resistance) | 160 | **0 (bit-exact)** | 1.44e-6 |
| `part0.fract` (mass fractions) | 396 | **0 (bit-exact)** | 3.36e-6 |
| `part0.schmi` (Schmidt factor) | 396 | **0 (bit-exact)** | 5.51e-7 |
| `part0.vsh` (settling velocity) | 396 | **0 (bit-exact)** | 7.13e-7 |
| `part0.cun_scalar` (upstream scalar `cun`) | 36 | **0 (bit-exact)** | 6.44e-8 |
| `decay.constant` (half-life → λ, `readreleases.f90:317`) | 7 | **0 (bit-exact)** | 2.69e-8 |
| `decay.surviving` (`exp(-lambda dt)`, `timemanager.f90:275`) | 49 | **0 (bit-exact)** | 9.90e-7 |

**Interpretation.** Against the double-precision build of the same Fortran,
~~**13 of 14 groups are bit-exact** and the fourteenth differs by a single ulp,
in the ECMWF branch of `obukhov` where the hybrid-coefficient average
introduces one extra rounding.~~ **CORRECTED 2026-10-02: all 14 groups are
bit-exact.** The one-ulp residual was a translation defect, not upstream's
rounding. The port evaluated `theta*ustar**2` as `(theta*ustar)*ustar`, but in
Fortran the power binds first, giving `theta*(ustar*ustar)`. The stage-3
`calcpar` verification exposed it: 52 of 978 outputs were off by 1 ulp. With
the association fixed, `obukhov.ecmwf` and `calcpar` are both bit-exact. The
"hybrid-coefficient average" explanation had never been tested, and it was
wrong. The translation is therefore not merely close but, on this input grid,
identical. Against FLEXPART as it actually ships, the
spread of 5.0e-8 to 3.4e-6 is the `real(4)` precision band and nothing else.

The `decay.surviving` grid deliberately includes the region where
`exp(-lambda dt)` underflows to exactly `0.0` in both languages (e.g.
`lambda = 0.693147 s^-1`, `dt = 3600 s`, `lambda dt ≈ 2495`) — the port
reproduces the underflow rather than clamping or NaN-ing, which is the correct
IEEE-754 behaviour and matches upstream exactly.

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


## Stage 1 (2026-10-02): turbulence, CBL, dry deposition, PBL, solar, calendar

### Methodology

A second driver, `dev/flexpart_reference_physics.f90`, links **27 upstream
files verbatim**: `par_mod`, `com_mod`, `hanna_mod`, `ew`,
`dynamic_viscosity`, `psim`, `psih`, `raerod`, `hanna`, `hanna1`,
`hanna_short`, `cbl`, `getrb`, `getrc`, `partdep`, `caldate`, `juldate`,
`getvdep`, `get_settling`, `pbl_profile`, `qvsat`, `richardson`, `windalign`,
`zenithangle`, `photo_O1D`, `distance`, `distance2`.

Several of these read meteorology and tables from `com_mod` rather than from
their arguments. The driver writes **synthetic fields straight into
`com_mod`**: Wesely tables, landuse fractions, roughness lengths, level
heights, `tt`/`rho` columns, `bdate`, `ldirect` and so on. No GRIB or NetCDF
file is involved. Each such input is echoed into the fixture as a setup row
(`particle_bins`, `wesely`, `richardson.column`), so the Rust test feeds the
port the same values.

The routines that carry state between calls (`hanna*`, through `hanna_mod`)
are given fixed sentinel priors before every call, and those priors are
recorded. Two upstream behaviours that depend on prior state are therefore
*verified* rather than merely tolerated:

- `hanna_short` applies `max(10, tlu)` to a `tlu` it never computes;
- `hanna1` leaves `sigma_w` unassigned for unstable `zeta >= 1`.

The build adds two flags to the original harness:

- `-mcmodel=medium`, because `com_mod`'s static arrays exceed 2 GB at real(8);
- `-fdefault-double-8` for the real(8) build. Without it, the driver's
  `double precision` Julian dates are promoted to real(16) and no longer match
  upstream's `real(kind=dp)`. That was found because the first real(8) run
  printed `****` for every Julian date.

The real4 fixture prints 9 significant digits, and the test parses them as
**`f32` then widens**, so the port is fed exactly the values the Fortran was.

```bash
./dev/build_reference.sh
cargo test --release -p changi --test flexpart_physics_code_to_code -- --nocapture
```

### Results

Taken **2026-10-02**, upstream `3d7eebf`, **5 521 rows per fixture, 20 tests,
all passing**. "max rel dev" is the largest relative deviation over every
output of every row.

| Group | Rows | vs `real8` | vs `real4` |
|---|---:|---:|---:|
| `hanna` | 640 | **0 (bit-exact)** | 7.66e-7 (floor `1e-10`, see below) |
| `hanna1` | 640 | **0 (bit-exact)** | 1.06e-6 (floor `1e-10`) |
| `hanna_short` | 640 | **0 (bit-exact)** | 6.05e-7 (floor `1e-10`) |
| `cbl` | 1 200 | 2.43e-15 (`erf` substitution) | spread rule, 248 outputs |
| `getrb` | 32 | **0 (bit-exact)** | 1.18e-7 |
| `getrc` | 481 | **0 (bit-exact)** | 1.45e-7 |
| `partdep` | 90 | **0 (bit-exact)** | 2.16e-7 |
| `getvdep` | 1 008 | **0 (bit-exact)** | 4.59e-6 |
| `get_settling` | 70 | **0 (bit-exact)** | 3.44e-7 |
| `pbl_profile` | 56 | **0 (bit-exact)** | 2.49e-6 |
| `qvsat` | 24 | **0 (bit-exact)** | 8.39e-7 (21 rows, see below) |
| `richardson` | 36 | **0 (bit-exact)** | 1.08e-4 (spread rule, 3 outputs) |
| `windalign` | 36 | **0 (bit-exact)** | 1.57e-7 |
| `zenithangle` | 168 | **0 (bit-exact)** | 5.63e-7 |
| `photo_O1D` | 99 | **0 (bit-exact)** | 3.29e-5 (spread rule, 2 outputs) |
| `distance` | 8 | **0 (bit-exact)** | 6.32e-8 |
| `distance2` | 9 | **0 (bit-exact)** | 6.15e-8 |
| `juldate` | 84 | **0 (bit-exact)** | 0 |
| `caldate` | 126 | **0 (bit-exact)** | 0 (117 rows, see below) |

**Interpretation.** Against real(8), **18 of 19 groups are bit-exact on every
output of every row**. The nineteenth, `cbl`, differs in 26 of its 6 000
outputs by at most `2.4e-15`. That residual was **attributed by experiment,
not by argument**: rebuilt in a scratch crate with glibc's `erf` (which is
what gfortran's intrinsic calls) in place of `petir::specfunc::erf`, `cbl`
is bit-exact on all 6 000 outputs. The port keeps `petir`'s `erf` per the
crate's reuse rule. (`cbl` uses the **intrinsic**: its `real :: erf`
declaration has no `external`, so the intrinsic wins over `erf.f90`.)

### Where the shipped real(4) build cannot be held to a relative bound

Each case below shows the port agreeing exactly with real(8) and the shipped
build departing from its own real(8) self. These are properties of FLEXPART
as distributed.

- **`cbl`: the drift term in the PDF tails.** When the particle velocity lies
  far in the tail of both Gaussian modes, `ptot` falls to `1e-12 .. 1e-7`.
  `Phi` is then a sum of `O(1e-4)` terms cancelling to `O(ptot)`, and the
  drift `a = (...)/ptot` divides that noise by a tiny number. Upstream's
  real(4) build differs from its own real(8) build by up to **1.1e3 relative
  in `Phi` and 5.1e3 in `a`**, and the drift's sign flips in some rows (70 of
  the original 900 rows). Any particle in that regime gets an arbitrary drift
  in the shipped model; how often that happens in a real run is not measured
  here.
- **`hanna*`: gradient underflow.** In a stable layer with `u* = 1e-6`, the
  gradient `d sigma_w / dz` is `~1e-48`, below `f32`'s smallest subnormal. The
  shipped build flushes it to 0 and then applies upstream's own
  `0 -> 1e-10` substitute. The real(4) check carries an absolute floor of
  exactly that substitute, `1e-10` as `f32` stores it.
- **`qvsat`: a threshold decided by rounding.** The `f32` image of the input
  253.15 K *is* the `f32` threshold `253.15`. The shipped build therefore
  takes the liquid branch, while the port given that `f32` value takes the
  ice branch; they differ by 18 %. Those three rows (three pressures) are out
  of the real(4) scope. Real(8) covers both sides of the switch exactly.
- **`caldate`: the 1600 century leap day.** The shipped build returns
  `16010231`, which is not a date, for JD 2305507 (1600-02-29), because
  `((julday-1867216)-0.25)/36524.25` is evaluated in default `real`. Real(8)
  and the port return `16000229`. The defect is pinned by its own test so a
  change upstream is noticed. Dates before the Gregorian switch (1582-10-15)
  are outside the port's documented range and are skipped.
- **`photo_O1D` near the horizon** (87–88°, `exp(-0.4/cos)` amplifying `cos`
  error) and **`richardson`'s `hmixplus`** (a Brunt–Väisälä frequency from a
  `theta` difference across a twentieth of a layer) sit at `3e-5` and
  `1.1e-4`.

The **precision-spread rule** used for `cbl`, `photo_O1D` and `richardson` is
opt-in per group. It accepts a real(4) output when the port lies within 4x
upstream's own real(4)-vs-real(8) distance for that output, and every output
accepted this way is counted in the test's printout. It is the statement
"this residual is FLEXPART's single precision". The real(8) check is what
proves the translation.

### The suite is not vacuous

Mutation run on 2026-10-02. **19 mutations, 19 killed**:

| Mutation | Killed by |
|---|---|
| `hanna` exponent `0.66666 -> 0.66667` | `hanna`, `hanna_short` |
| `hanna` `sigma_w` offset `1e-2 -> 1e-3` | `hanna`, `hanna_short` |
| `cbl` closure `0.66667 -> 0.6667` | `cbl` |
| `cbl` cube-root exponent `0.333333333 -> 1/3` | `cbl` |
| `cbl` taper switch `-h/L < 15 -> < 14` | `cbl` |
| `getrb` Prandtl `0.72 -> 0.71` | `getrb`, `getvdep` |
| `getrc` `r_c >= 10` floor removed | `getrc` |
| `getvdep` southern shift `365/2 -> 182.5` | `getvdep` |
| `getvdep` tropical season `mmdd 600 -> 1000` | `getvdep` |
| `partdep` `alpha <= log10(eps) -> <= -4` | `partdep` |
| `get_settling` drag switch `Re < 500 -> < 400` | `get_settling` |
| `pbl_profile` `r1 0.74 -> 0.75` | `pbl_profile` |
| `richardson` excess `bs 8.5 -> 8.0` | `richardson` |
| `richardson` drop the `k = k-1` | `richardson` |
| `qvsat` switch `t >= 253.15 -> t > 253.16` | `qvsat` |
| `zenithangle`/`photo_O1D` local pi `3.1415927 -> PI` | `zenithangle`, `photo_O1D` |
| `zenithangle` leap-day `+1` removed | `zenithangle` |
| `distance` radius `6.3712e6 -> 6.371e6` | `distance`, `distance2` |
| `caldate` `ss == 60` rollover | `caldate`, `zenithangle` |

The first pass left **four survivors**, and they were treated as findings
about the sweep, not the mutations:

- `cbl`: no case had `-h/L` between 14 and 15;
- `getvdep`: no southern date sat half a day from a season boundary;
- `get_settling`: the Reynolds number never entered 400–500;
- the `mmdd 600 -> 700` mutation was equivalent (both are summer), so it was
  replaced by `600 -> 1000`.

The driver gained `L = -75.5 m`, a 2 May date (Oct 31 after the +182-day
shift) and a 900 µm particle (Re 455 → 436). The suite was rerun and all 19
mutations were killed.

### Provenance decisions for Numerical Recipes code

`caldate.f90` and `juldate.f90` derive from *Numerical Recipes*. Their
licence is not GPL-compatible, so they are **not translated**. The port
computes the day count with Howard Hinnant's public-domain
`days_from_civil`/`civil_from_days`, already used in
`crates/kovan-metrics/src/date.rs`, and ports only FLEXPART's own
time-of-day arithmetic. The fixture shows the two agree exactly from
1582-10-15 onwards. `random_mod.f90` (NR `ran3`/`gasdev`) is not ported and
not reimplemented (maintainer decision, #410).

## Stages 2–7 (2026-10-02): interpolation, met fields, the particle step, output, convection, and the stochastic comparison

Each stage has its own driver, fixtures and test, built by `dev/build_reference.sh`
(which calls `dev/build_reference_<stage>.sh`) and replayed through the shared
harness `tests/common/mod.rs`. The method is the one above: upstream compiled
verbatim at real(4) and at `-fdefault-real-8 -fdefault-double-8`, synthetic
fields written into `com_mod`, every input echoed as setup rows.

Local stand-ins, none of which changes a routine under test:

- **`par_mod.f90` copies.** It is FLEXPART's user-edited configuration file.
  Nests need `maxnests=1, nxmaxn=12, nymaxn=12` instead of the shipped zeros
  (stages 2, 3, 4, 6). The output stage also needs `maxspec`, `maxageclass`,
  `nclassunc` and the kernel / particle-count switches. Each build checks the
  diff line count of its copy.
- **`random_mod.f90` shims** (`dev/random_mod_shim*.f90`). They return the
  draws the driver chooses, so both codes consume the same numbers. The
  Numerical Recipes generators are neither ported nor re-implemented (gh:#410).
- **A generated copy of `cmapf_mod`** with its `private` statement removed, so
  the driver can reach the 13 routines the module hides. The build checks that
  this is the whole diff, and every public routine is called through both
  copies.

### Results against the real(8) build (the translation check)

| Stage | Routines | Rows | vs real(8) |
|---|---|---:|---|
| 2 interpolation | `interpol_all`, `_misslev`, `_wind`, `_wind_short`, `_vdep` (each + `_nests`) | 1 944 | **bit-exact**, 20 304 outputs |
| 3 met | `calcpar`(+nests), `calcpv`(+nests), `interpol_rain`(+nests), `get_wetscav`, `wetdepo`, `wetdepokernel`(+nest), `gethourlyOH`, `ohreaction`, `assignland` | 2 144 | **bit-exact** |
| 4 particle step | `advance`, `initialize`, `initialize_cbl_vel`, `re_initialize_particle`, `get_vdep_prob` | 751 | 7 315 of 7 320 bit-exact; 5 at ≤ 3.0e-15 (`erf` substitution in `cbl`) |
| `cmapf` | all 18 routines of `cmapf_mod`, `coordtrafo` | 10 491 | **bit-exact** |
| 5 output | `conccalc`, `drydepokernel`(+nest), `centerofmass`, `clustering`, `mean`, `plumetraj`, `partpos_average` | 3 063 | **bit-exact**; `plumetraj` only to its print format |
| 6 convection | `convect43c` (CONVECT, TLIFT), `calcmatrix`, `redist`, `convmix` | 2 234 | **bit-exact**, 22 788 outputs |

The one translation defect these stages found was in an **earlier** stage. It
was `obukhov`'s `theta*ustar**2`, worked out above under Results, and stage 3's
`calcpar` exposed it.

### Real(4): what the shipped build cannot carry

Where the shipped build departs from its own real(8) self, each case was
diagnosed before any rule was applied. The tests either attribute it with
`Real4Rule::PrecisionSpreadOn(columns)`, limited to the measured columns, or
take it out of real(4) scope on a criterion derived outside the comparison.
Real(8) covers every excluded row bit for bit.

- **Sub-grid wind sigmas (stage 2).** The one-pass formula
  `sum x^2 - (sum x)^2/n` cancels in `f32`. For a uniform `ww = 0.01 m/s`, the
  shipped build reports `wsig = 5.0e-6` instead of 0.
- **Polar steps poleward of 89° (stage 4, `cmapf`).** `cxy2ll`/`cg2cxy` cannot
  be carried in `f32` there; 89° is upstream's own polar switch, and longitude
  errors reach 180°. Particle steps on the polar grid inherit up to `5e-4` in
  `xt`.
- **The Petterssen half-differences `(u_new - u_old)/2` (stage 4).** These are
  `1e-6` m/s out of `1`.
- **Convective mass fluxes (stage 6).** They scale with `0.0025*DTMA`, a 0.1 K
  difference of 300 K temperatures. For a near-zero cloud-base flux
  (`4.5e-10`), the shipped build clamps to 0 and reports a different cloud top.
- **Stages 3 and 5** have the same kinds of cases: cancellation in `calcpv`,
  near-horizon photolysis in `gethourlyOH`, the `f32`-stored output-cell
  coordinates in `conccalc`, and the near-pole longitude in `centerofmass`.
  The stage reports list them row by row.

### Upstream defects and quirks found (selected; every one is documented in the module that ports it)

- **`get_vdep_prob` interpolates with stale weights.** It sets the cell
  indices but never the bilinear or time weights, so it uses the previous
  particle's weights.
- **`getvdep_nests` and the season.** It computes the latitude for season
  selection as `jy*dy + ylat0`, with the nest's row index and the mother
  grid's spacing. A nest at 30–32°N gets southern-hemisphere seasons.
- **The pole crossing in `advance`.** It applies `mod(xt + 180, 360)` to a grid
  coordinate.
- **`nrand` after the vertical sub-step loop.** `advance` adds the loop
  variable's exit value, `ifine + 1`, to `nrand`.
- **`initialize` reuses its draws.** It feeds the same draws to the turbulent
  velocities and to the mesoscale ones.
- **The polar Petterssen step.** It writes the module winds back divided by the
  grid size.
- **`interpol_mod` relies on zeroed static storage.** Levels above `nmixz` keep
  whatever profile `interpol_all` left, possibly from another particle.
- **`calcmatrix` never lets a column's cloud-base flux relax to 0.** It restores
  the old flux whenever `convect` reports no convection.
- **Count mode with the kernel mixes units in `conccalc`.** Particle counts and
  masses land in the same grid.
- **`plumetraj` writes cluster values it never assigned.** This happens when
  there are fewer particles than clusters.
- **`calcpar_nests` has no NCEP branch.** It reads a never-assigned level
  pressure.
- **`ohreaction` indexes the wrong `tt`.** It uses `n` rather than
  `memind(n)`.
- **The shipped `real(4)` `caldate` and the 1600 leap day.** It returns
  `16010231` for 1600-02-29.

### Stage 7: the stochastic comparison

**Methodology.** `dev/flexpart_stochastic_advance.f90` compiles upstream
**including its own `random_mod.f90`**. It fills `rannumb` as `FLEXPART.f90`
does (`gasdev1`, `idummy = -320`), and each call draws its starting index from
`ran3`. There are five scenarios:

- Gaussian turbulence;
- the well-mixed scheme;
- the skewed CBL;
- the free troposphere;
- the stratosphere.

Each releases 20 000 particles and advances them 2 h, and the whole run is
repeated in 16 replicates with different `gasdev1` seeds. The port runs the
same scenarios in 16 replicates. Its `rannumb` comes from RAFFLES'
`sample_normal` on `petir`'s LCG, sub-streams `2^40` apart, and its starting
indices come from `petir`'s `prn`. The seed was fixed before the first run.

**Criterion** (gh:#410): for the mean and variance of the displacement in x, y
and z, the port's replicate mean must lie within **1σ of FLEXPART's**. Here σ
is FLEXPART's run-to-run standard deviation of that statistic: the
uncertainty of one FLEXPART result. The strict z of the difference of the two
replicate means and its χ² are reported too.

**Results.** Against real(4), **30/30** statistics are within 1σ, with χ² = 19.7
on 30 degrees of freedom. Against real(8), also **30/30**, with χ² = 20.6. Each
test runs in 5 s.

**How it got there, recorded because it changed the port.**

1. The first run used one replicate and the 1/√N standard error. It had
   22/30 within 1σ, and the mean-y deviations shared a sign. Diagnosis: every
   particle in a run shares one finite `rannumb` array, whose own sample mean
   shifts the whole ensemble coherently. FLEXPART's array had mean `+1.11e-3`,
   the port's `-1.63e-3`. The 1/√N error ignores that, so it was the wrong
   σ. Replicates measure it.
2. With replicates, all 30 were within 1σ, but χ² was 47.8 on 30 degrees of
   freedom, and the port's horizontal variances were 0.5–1 % high in four
   scenarios. Diagnosis: FLEXPART's `rannumb` has a mean square of `0.99534`
   (16 seeds), not 1. **`gasdev1` clips every deviate to [-3, 3]**, which gives
   `0.99501` for a normal. That clip is FLEXPART's own code, not the Numerical
   Recipes generator around it, and it narrows every turbulent velocity
   distribution by 0.5 %. The port now applies it (`advance::limit_rannumb`),
   and χ² fell to 19.7.

The stochastic comparison was capable of failing, and it found a real piece of
FLEXPART's model that the deterministic comparison could not see.

## What this does NOT establish

**Verification, not validation.** It establishes that the Rust computes what the
Fortran computes. It says nothing about whether FLEXPART's parameterisations
reproduce measured atmospheric dispersion — that needs field data, and is not
claimed anywhere in this crate.

Per the crate's binding scope limit, nothing here supports emergency planning,
emergency response, dose assessment for real populations, or operational Level 3
PSA. The `Bookkeeping status` block in the README records that human review of
both the V&V and the interface is still outstanding.

Not covered, and tracked in GitHub issue #410:

- ~~the particle advection loop (`advance.f90`)~~ **CORRECTED 2026-10-02** —
  ported, verified deterministically (stage 4) and statistically (stage 7);
- ~~the Hanna turbulence parameterisation~~ **CORRECTED 2026-10-02** — ported
  and verified, see "Stage 1" below;
- ~~the convective boundary-layer scheme (`cbl.f90`)~~ **CORRECTED 2026-10-02**
  — ported and verified;
- ~~wet scavenging (`wetdepo.f90`, `get_wetscav.f90`)~~ **CORRECTED
  2026-10-02** — ported and verified (stage 3); ~~and the dry-deposition
  velocity assembly (`getvdep.f90`)~~ **CORRECTED 2026-10-02** — `getvdep`
  is ported and verified;
- ~~the Richardson-number mixing-height diagnostic (`richardson.f90`)~~
  **CORRECTED 2026-10-02** — ported and verified;
- ~~`get_settling.f90`'s ambient rescaling of the reference-state settling
  velocities `part0` returns~~ **CORRECTED 2026-10-02** — ported and verified;
- ~~the meteorological interpolation, the output grids, and the OH chemistry~~
  **CORRECTED 2026-10-02** — ported and verified (stages 2, 3, 5);
- still not ported: the GRIB/NetCDF readers and file writers (I/O, not
  numerics), `verttransform_*`, release and domain filling
  (`releaseparticles`, `init_domainfill`, `boundcond_domainfill`),
  `outgrid_init*`, `initial_cond_calc`, `calcfluxes`/`fluxoutput`, the
  `concoutput*` unit conversion and `timemanager`'s per-step bookkeeping
  (gh:#410 wave 2).

**Radioactive decay's fixture gap is closed (2026-09-15).** It was the one
ported module checked only against hand-copied upstream *expressions* rather
than a compiled Fortran fixture, because upstream computes it inline
(`readreleases.f90:317`, `timemanager.f90:275`) rather than in a callable
routine. `dev/flexpart_reference.f90::emit_decay` now extracts both lines
verbatim into driver subroutines, compiled and run like every other group —
see the `decay.constant` / `decay.surviving` rows in the Results table above.
Every currently-ported `changi::flexpart` function now has a genuine
code-to-code fixture; none is checked against a hand-derived expected value.
