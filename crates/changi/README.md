# CHANGI

**C**onsequence and **H**azard **A**nalysis for **N**uclear **G**round-level and
atmospheric **I**mpacts.

Atmospheric consequences for the OUTRAM PARK suite — *"what happens after
release?"*

## Scope — now

Research and educational use only:

- Atmospheric dispersion
- Plume transport
- Radionuclide deposition
- Ground contamination

**Input:** source terms produced by SEMBAWANG. `sembawang` exists in the
workspace as an explicit placeholder (created 2026-09-18) with no
implementation, so until it has a real source-term calculation, a
release-rate time series has to be supplied by hand —
FLEXPART needs nothing more than that from the accident side.

## Scope — future, not current

Recorded so the direction is not lost, and **explicitly not what this crate is
for today** (maintainer direction, 2026-09-15):

- Radiological consequence assessment
- Dose assessment
- Emergency planning support
- Level 3 PSA support

None of these is implemented, and none should be described as available. They
are a later phase, and reaching them needs more than code: `RESPONSIBLE_USE.md`
sets the intended-use limits, so moving any of them from this list to the one
above is a maintainer decision made deliberately there, never a side effect of
adding a feature.

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

Named for Changi, following the workspace's Singapore-place-name convention.

## Intended use — a binding limit, not a disclaimer

**CHANGI is for research, education and verification/validation only.**

It must **not** be presented as, or used for, emergency planning, emergency
response, dose assessment for real populations, Level 3 PSA support, nuclear
facility operation, or any safety-critical or licensing decision. This limit
comes from the workspace `RESPONSIBLE_USE.md` (§ intended use, and line 43:
outputs "must not be treated as authoritative for safety, licensing,
operational, regulatory, or emergency-response purposes") and from
`docs/ecosystem-naming.md` decision 3 (2026-08-05), reaffirmed by the maintainer
on 2026-09-15.

An earlier naming draft claimed emergency-response capability and was corrected
precisely because it contradicted that policy. Do not reintroduce that framing.

## Where it sits

CHANGI is the middle link of the offsite chain:

| | Question | Crate |
|---|---|---|
| SEMBAWANG | What gets released? | `sembawang` — placeholder crate, nothing implemented |
| **CHANGI** | **What happens after release?** | **this crate** |
| REDHILL | What happens after deposition? | `redhill` — placeholder crate, nothing implemented; will build on `outram-park-fork-pflotran` |

## What exists today

Two independent, separately verified ports.

### 1. `flexpart` — FLEXPART's surface-layer and deposition scalar kernels

The pure functions that turn meteorological surface fields into turbulence
scales and aerosol deposition properties.

| Area | Upstream files | Ported |
|---|---|---|
| Physical constants | `par_mod.f90` | yes |
| Saturation vapour pressure, dynamic viscosity | `ew.f90`, `dynamic_viscosity.f90` | yes |
| Monin–Obukhov similarity, friction velocity, aerodynamic resistance | `psim.f90`, `psih.f90`, `scalev.f90`, `obukhov.f90`, `raerod.f90` | yes |
| Lognormal aerosol bins: settling, Cunningham, Schmidt | `part0.f90` | yes |
| Radioactive decay | `readreleases.f90`, `timemanager.f90` | yes |

**Not ported**, which is most of FLEXPART: the particle advection loop
(`advance.f90`), the Hanna turbulence parameterisation, the convective
boundary-layer scheme, wet scavenging, the Richardson mixing-height diagnostic,
the GRIB/NetCDF readers, the output grids and the OH chemistry. This is the
first verified slice of a port, not "FLEXPART in Rust".

### 2. `puff` — the Gaussian puff forward model

A complete Rust port of the physics of the R package
[`puff`](https://github.com/Hammerling-Research-Group/puff) (MIT, `0.1.1`,
commit `5213d58`), from the Hammerling Research Group at the Colorado School of
Mines.

A Gaussian *puff* model discretises a continuous release into a train of
discrete puffs, each advected by the wind sampled when it was emitted and
spread by a Pasquill–Gifford coefficient that grows with the distance it has
travelled. It complements the FLEXPART port rather than duplicating it:
FLEXPART is a Lagrangian *particle* model on gridded meteorology, built for
synoptic scales; this is analytic, needs no meteorological files, and is cheap
enough to run interactively over a site-sized domain.

| Area | Upstream file | Ported |
|---|---|---|
| Pasquill stability classification | `R/helpers.R` | yes |
| Pasquill–Gifford `sigma_y`, `sigma_z` | `R/helpers.R` | yes |
| Met-convention wind handling, resampling | `R/helpers.R` | yes |
| The Gaussian puff kernel | `R/helpers.R` | yes |
| Sensor mode | `R/simulate_sensor_mode.R` | yes |
| Grid mode | `R/simulate_grid_mode.R` | yes |
| Plotting | `R/plots.R` | **no** — see below |

**Not ported:** `R/plots.R`, which is 1 077 of upstream's 2 296 lines — 47 % of
the package. It is `ggplot2`/`plotly` chart construction with no physics.
`changi` is a headless library that must build for Android and `wasm32`, and
does not own its caller's plotting. Everything else in upstream is physics, and
all of it is here.

**One default deliberately differs from upstream.** Upstream's puff record is
built with an R `data.frame` whose length-1 columns get recycled against a
length-2 stability class, so an emission event with an ambiguous class produces
**two puffs each carrying the full mass** — doubling the emitted mass across six
of the stability table's ten regimes. This port defaults to the mass-conserving
reading and offers upstream's behaviour as
`EmissionPolicy::UpstreamRecycleStabilityClasses`. Three further upstream
defects are reproduced faithfully and documented. See
[`docs/puff-code-to-code.md`](docs/puff-code-to-code.md).

**The unit conversion is methane-specific.** Upstream's oil-and-gas application
converts to ppm of methane with a factor encoding methane's molar mass. For a
radionuclide that factor is wrong twice over — wrong molar mass, and ppm is the
wrong unit for an activity concentration. Use the mass-density return.

## Start here

```bash
cargo run --release -p changi --example puff_site_survey
```

[`examples/puff_site_survey.rs`](examples/puff_site_survey.rs) is the entry
point for the puff model and is written to be read top to bottom — one source
leaking over a small site with a ring of sensors, what the stability class does
to the answer, and the one place this port deliberately disagrees with its
upstream, with the size of that disagreement measured rather than asserted.

Two things in it are worth knowing before using the crate for anything:

- **Stability, not wind speed, is the dominant control.** At the *same* 1.0 m/s,
  a calm clear night reads ~77x a calm sunny afternoon, because the afternoon's
  convection dilutes the plume far faster than any horizontal wind does.
- **Upstream's mass doubling is not a factor of two you can divide out.** The
  mass doubles exactly, but the two duplicated puffs carry *different* stability
  classes, so the concentration ratio is geometry-dependent — 1.78 in the
  example's configuration.

## Verification

Every reference value comes from **compiling and running the upstream FLEXPART
Fortran itself**, at two precisions. Full methodology, results and analysis:
[`docs/flexpart-code-to-code.md`](docs/flexpart-code-to-code.md).

Headline: against a `-fdefault-real-8` build of the same routines, **13 of 14
function groups agree bit-exactly** (the fourteenth by one ulp). Against
FLEXPART as it actually ships — which is single precision, since its makefile
passes no `-fdefault-real-8` — agreement is 5e-8 to 3.4e-6, which is upstream's
own precision rather than any translation error. Every currently-ported
function, including radioactive decay, is checked against a compiled Fortran
fixture — none against a hand-derived expected value.

That is **verification, not validation**: it shows the Rust computes what the
Fortran computes, and says nothing about whether FLEXPART's parameterisations
reproduce measured dispersion.

The `puff` port is verified the same way, by **executing the upstream R
itself** — 10 182 reference cases, 18 tests. Full methodology and results:
[`docs/puff-code-to-code.md`](docs/puff-code-to-code.md).

Headline: agreement from **bit-exact to 2.7e-15** relative. Both sides are
double precision, so unlike the FLEXPART comparison there is no precision floor
to account for. The Pasquill–Gifford coefficients agree **bit-exactly** across
all 234 cases. Nineteen mutations of the port were run to check the suite is not
vacuous; **all nineteen were killed**. The pass also found three defects in the
port itself, including a `uom::Ratio` round trip that was silently truncating
small concentrations — all three are recorded in the doc.

```bash
cargo test --release -p changi
./dev/build_reference.sh                # FLEXPART fixtures (needs gfortran + the upstream clone)
Rscript dev/gen_puff_reference.R        # puff fixture (needs R + dplyr + the upstream clone)
Rscript dev/gen_puff_reference.R --check  # verify the committed fixture is current
```

## Reuse

Per the workspace "reuse before porting, port before writing" rule:

- **`petir`** supplies `erf` for the aerosol size distribution; FLEXPART's own
  `erf.f90` is deliberately **not** ported.
- **`boon-lay`**'s nuclide database is the intended source of half-lives; this
  crate carries no nuclide data of its own.
- **`outram-mc-libs`**' LCG will supply the pseudo-random numbers the Langevin
  turbulence scheme needs. Not yet wired in — no stochastic code has landed.
- **`outram-foam-basic-lib`** will supply the gridded field and interpolation
  layer when the concentration-grid phase arrives. Deliberately **not** yet a
  dependency: the scalar kernels need nothing from it, and taking a
  finite-volume CFD dependency before there is a field to put on a mesh would be
  premature.

## Licence

GPL-3.0, containing two ports:

- **FLEXPART** (GPL-3.0-or-later, © FLEXPART 1998-2019) —
  [`LICENSE.flexpart`](LICENSE.flexpart), [`NOTICE.flexpart`](NOTICE.flexpart).
  Independent fork; not affiliated with or endorsed by NILU or the FLEXPART
  developers.
- **`puff`** (MIT, © 2025 Hammerling Research Group) —
  [`LICENSE.puff`](LICENSE.puff), [`NOTICE.puff`](NOTICE.puff). MIT into
  GPL-3.0 is compatible and **one-way**: nothing in `src/puff/` may be copied
  back into an MIT project on the strength of its origin here. Independent
  fork; not affiliated with or endorsed by the Hammerling Research Group or the
  Colorado School of Mines.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.
