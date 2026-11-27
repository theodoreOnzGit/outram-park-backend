# CHANGI

**C**onsequence and **H**azard **A**nalysis for **N**uclear **G**round-level and
atmospheric **I**mpacts.

Atmospheric consequences for the OUTRAM PARK suite — *"what happens after
release?"*

CHANGI makes **atmospheric dispersion studyable and challengeable** at two
scales: FLEXPART's Lagrangian particle physics (Monin–Obukhov similarity,
Hanna turbulence, the skewed convective boundary layer, dry and wet
deposition) translated routine by routine from the Fortran and checked
against the compiled upstream, and the R `puff` Gaussian puff model, small
enough to read top to bottom, checked against the R it came from. A reader
can see exactly where the port departs from upstream (the puff mass-doubling
default) and how large that departure is, measured rather than asserted.

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

## Knowledge in this crate

See the workspace [vision](../../VISION.md).

| Knowledge artifact | In this crate |
|---|---|
| Scientific literature | Liu & Cao (2002), the HTR-10 source term, whose tables are embedded in `src/activity/` (catalogue entry in `crates/kovan-literature/CATALOGUE.md`; provenance of every table in `docs/References.md`); Singapore climatology for `puff::climatology` (`docs/References.md`). Upstream code: FLEXPART v10.4, commit `3d7eebf` (GPL-3.0-or-later; `LICENSE.flexpart`, `NOTICE.flexpart`); R [`puff`](https://github.com/Hammerling-Research-Group/puff) `0.1.1`, commit `5213d58` (MIT; `LICENSE.puff`, `NOTICE.puff`). |
| Mathematical equations | Module docs of each translated routine under `src/flexpart/` (named after its `.f90`, tabulated below), the Pasquill–Gifford puff kernel (`src/puff/`), and the dilution factor, time-integrated air concentration and dry deposition (`src/activity/chi_over_q.rs`, `src/activity/deposition.rs`). |
| Source code | `src/flexpart/` (surface layer, turbulence, CBL, deposition, particle advance, met interpolation, convection, concentration gridding and output), `src/puff/` (stability, coefficients, kernel, sensor and grid modes), `src/activity/` (not a port: inventory, airborne and accident release, primary helium, fuel release, source, survey, deposition). |
| Interactive simulations | None in this crate. CLI examples: `examples/puff_site_survey.rs` (start here), `examples/chi_over_q_survey.rs`, `examples/site_activity_survey.rs`, `examples/field_timing.rs`. `crates/outram-park-digital-twin-engine/examples/htgr_sim_v1` drives `changi::puff` and `changi::activity`. |
| Visualisations | None yet; upstream `puff`'s `R/plots.R` is deliberately not ported. A lesson book sits in `docs/lessons/`. |
| Verification and validation evidence | `docs/flexpart-code-to-code.md`: all 14 function groups bit-exact against a `-fdefault-real-8` build of FLEXPART (corrected 2026-10-02), 5e-8 to 3.4e-6 against single-precision FLEXPART as shipped. `docs/puff-code-to-code.md`: 10 182 reference cases from the executed R, bit-exact to 2.7e-15 relative, 19 of 19 mutations killed. `docs/activity-consistency-checks.md` for the activity layer, which has no upstream. Fixture builders in `dev/`. Verification only; no comparison with measured dispersion. |
| Human reviews and corrections | 0 functions carry kovan human-review stamps. No entry in `docs/human-corrections-to-ai-work.md` names this crate. An earlier naming draft claiming emergency-response capability was corrected to match `RESPONSIBLE_USE.md` (below); the dose tables were moved to `buangkok` at the maintainer's direction (2026-09-28). |

**Status:** the FLEXPART kernels and the puff model are verified code to code
against their executed upstreams; the `activity` layer has only consistency
checks, and nothing here is validated against measured dispersion.

## `no_std` + `alloc` (gh:#1587)

The default `std` feature can be turned off: the library is then `no_std` +
`alloc` and builds for bare metal
(`cargo check --release -p changi --no-default-features --target x86_64-unknown-none`,
gated by `scripts/check-nostd.sh`). `std` carries the Gaussian puff field's `rayon` thread pool (`puff::wgsl::field_pooled`); without it `field_pooled` is the serial `field_serial`, bit for bit the same field (pinned by `the_pooled_field_is_bit_identical_to_the_serial_one`). `gpu` implies `std`. With `std` on (the default)
nothing changes.

## Scope — now

Research and educational use only:

- Atmospheric dispersion
- Plume transport
- Radionuclide deposition
- Ground contamination

**Input:** source terms produced by SEMBAWANG. ~~`sembawang` exists in the
workspace as an explicit placeholder (created 2026-09-18) with no
implementation, so until it has a real source-term calculation, a
release-rate time series has to be supplied by hand —
FLEXPART needs nothing more than that from the accident side.~~
**CORRECTED 2026-09-28** — `sembawang` is implemented (`crates/sembawang/src/`:
`scenario.rs`, `inventory.rs`, `chain.rs`); it drives `boon-lay`'s TRISO-ATOPS
release and hands the result to this crate through
`sembawang::chain::pad_for_dispersion` (`crates/sembawang/examples/npmhtgr_chain.rs`).
A release-rate time series can still be supplied by hand.

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

(The unverified banner moved to the top of this README on 2026-11-27.)

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
| SEMBAWANG | What gets released? | `sembawang` — ~~placeholder crate, nothing implemented~~ **CORRECTED 2026-09-28**: implemented TRISO release orchestration over `boon-lay` (`crates/sembawang/src/`) |
| **CHANGI** | **What happens after release?** | **this crate** |
| REDHILL | What happens after deposition? | `redhill` — placeholder crate, nothing implemented; will build on `outram-park-fork-pflotran` |

## What exists today

Two independent, separately verified ports. **CORRECTED 2026-09-28** (this line previously listed only the two ports) —
plus a third module that is **not** a port: `activity` (`src/activity/`, dilution
factors, time-integrated air concentration, dry deposition, and the published
HTR-10 core inventory in `activity::inventory`, plus, added 2026-09-28, the
published HTR-10 annual airborne release in `activity::airborne_release`, the
published HTR-10 end-of-life primary-helium activity in
`activity::primary_helium`, and
~~the published HTR-10 normal-operation and accident dose-versus-distance
tables (Tables 7 and 9)~~ (**MOVED 2026-09-28** to the dose crate,
`buangkok::published`; changi holds no dose data).
~~Nothing consumes ~~either~~ any of these tables yet~~ **CORRECTED
2026-11-27**: the inventory table is consumed by `sembawang::htr10`
(`crates/sembawang/src/htr10.rs`, `htr10_equilibrium_core`) and by
`htgr_sim_v1`, below). It has no upstream and no
code-to-code verification (see its module doc). `htgr_sim_v1` uses it
(`changi::activity::{chi_over_q, inventory, source, survey, deposition}`) together
with `changi::puff`, in
`crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/atmospheric_dispersion.rs`.

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
| Hanna turbulence, wind alignment | `hanna.f90`, `hanna1.f90`, `hanna_short.f90`, `windalign.f90` | yes (2026-10-02) |
| Skewed convective boundary layer | `cbl.f90` | yes (2026-10-02) |
| Dry deposition: `r_b`, Wesely `r_c`, particles, cell assembly, settling | `getrb.f90`, `getrc.f90`, `partdep.f90`, `getvdep.f90`, `get_settling.f90` | yes (2026-10-02) |
| Profile fluxes, Richardson mixing height, saturation humidity | `pbl_profile.f90`, `richardson.f90`, `qvsat.f90` | yes (2026-10-02) |
| Solar zenith angle, O(¹D) photolysis, great-circle distance | `zenithangle.f90`, `photo_O1D.f90`, `distance*.f90` | yes (2026-10-02) |
| Calendar | `juldate.f90`, `caldate.f90` (day count re-derived, not translated: NR provenance) | yes (2026-10-02) |
| Met interpolation (+ nests, polar grids) | `interpol_*.f90` | yes (2026-10-02) |
| The particle step: Langevin, CBL, reflection, Petterssen, mesoscale | `advance.f90`, `initialize.f90`, `initialize_cbl_vel.f90`, `re_initialize_particle.f90`, `get_vdep_prob.f90` | yes (2026-10-02); also checked statistically against FLEXPART's own random numbers |
| Map projections, release coordinates | `cmapf_mod.f90`, `coordtrafo.f90` | yes (2026-10-02) |
| Surface/PBL parameters, potential vorticity | `calcpar*.f90`, `calcpv*.f90` | yes (2026-10-02) |
| Wet scavenging and deposition | `interpol_rain*.f90`, `get_wetscav.f90`, `wetdepo.f90`, `wetdepokernel*.f90` | yes (2026-10-02) |
| OH chemistry, landuse | `gethourlyOH.f90`, `ohreaction.f90`, `assignland.f90` | yes (2026-10-02) |
| Concentration/deposition gridding, plume statistics | `conccalc.f90`, `drydepokernel*.f90`, `centerofmass.f90`, `clustering.f90`, `plumetraj.f90`, `mean_mod.f90`, `partpos_average.f90` | yes (2026-10-02) |
| Emanuel convection and convective mixing | `convect43c.f90`, `calcmatrix.f90`, `redist.f90`, `convmix.f90` | yes (2026-10-02) |

**Not ported**, which is still most of the *model*: ~~the Hanna turbulence
parameterisation, the convective boundary-layer scheme, the Richardson
mixing-height diagnostic~~ (**CORRECTED 2026-10-02**: those are ported and
verified, above). ~~Not yet ported: the particle advection loop (`advance.f90`),
the meteorological interpolation, wet scavenging, the GRIB/NetCDF readers, the
output grids and the OH reaction.~~ **CORRECTED 2026-10-02**: all of those
except the readers are now ported and verified, above. Not yet ported: the
GRIB/NetCDF readers and the file writers (I/O, not numerics). The rest was
**CORRECTED 2026-10-02**: `verttransform_*`, particle release and domain
filling, the output-grid set-up, the `concoutput*` conversion and
`timemanager` are ported and verified (gh:#410). This is a verified set of kernels, not "FLEXPART in Rust".

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

Every condition in it is an illustrative **Singapore** one, from
`changi::puff::climatology` — CHANGI is named for Changi, and the figures have
their provenance recorded once, in [`docs/References.md`](docs/References.md),
rather than as round numbers repeated per example.

Three things in it are worth knowing before using the crate for anything:

- **Stability, not wind speed, is the dominant control.** At the *same* 1.0 m/s
  inter-monsoon wind, a clear night reads ~77x a sunny afternoon, because the
  afternoon's convection dilutes the plume far faster than any horizontal wind
  does.
- **Singapore's mean wind sits exactly on a Pasquill band edge.** The
  stability table switches at 2 m/s and the mean surface wind *is* about 2 m/s,
  so the daytime class flips between the ambiguous pair `A/B` and a single `B`
  across a measurement uncertainty of a few cm/s. At night, every condition
  below a monsoon surge is ambiguous.
- **Upstream's mass doubling is not a factor of two you can divide out — or
  even bound.** The mass doubles exactly, but the two duplicated puffs carry
  *different* stability classes, so the concentration ratio is
  geometry-dependent: measured from **1.04 to 3.97** across the Singapore
  conditions, on both sides of 2.

Because those ambiguous night-time conditions are Singapore's normal state
rather than a corner case, this port's mass-conserving default is doing real
work here, not guarding a hypothetical.

## Verification

Every reference value comes from **compiling and running the upstream FLEXPART
Fortran itself**, at two precisions. Full methodology, results and analysis:
[`docs/flexpart-code-to-code.md`](docs/flexpart-code-to-code.md).

Headline: against a `-fdefault-real-8` build of the same routines, ~~**13 of 14
function groups agree bit-exactly** (the fourteenth by one ulp)~~ **all 14
function groups agree bit-exactly** (**CORRECTED 2026-10-02**: the one-ulp
group was an operator-association defect in the port's `obukhov`, now fixed). Against
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
  crate ~~carries no nuclide data of its own~~ **CORRECTED 2026-09-28** — carries
  no half-life or decay-constant data of its own, but it **does** embed
  ~~one nuclide table~~ ~~two~~ three published nuclide tables of activities: the 22-entry HTR-10
  equilibrium-core inventory (`reference/htr10_equilibrium_core_inventory.csv`,
  compiled in by `src/activity/inventory.rs`), and, added 2026-09-28, the
  22-entry HTR-10 annual normal-operation airborne release
  (`reference/htr10_normal_operation_annual_airborne_release.csv`, compiled in
  by `src/activity/airborne_release.rs`), and, also added 2026-09-28, the
  20-entry HTR-10 end-of-life primary-helium activity
  (`reference/htr10_primary_helium_activity_end_of_life.csv`, compiled in by
  `src/activity/primary_helium.rs`). ~~Provenance for both~~ Provenance for all
  three (**CORRECTED 2026-09-28**) is in `docs/References.md`.
  **CORRECTED 2026-11-27** — there are now **five** embedded tables:
  `reference/` also holds `htr10_accident_airborne_release.csv` (compiled in
  by `src/activity/accident_airborne_release.rs`) and
  `htr10_fuel_element_release_rate.csv` (`src/activity/fuel_release.rs`),
  checked by their `include_str!` lines; `docs/References.md` has a
  provenance section for each (checked by grep).
- **`outram-mc-libs`**' LCG will supply the pseudo-random numbers the Langevin
  turbulence scheme needs. Not yet wired in ~~— no stochastic code has
  landed~~. **CORRECTED 2026-11-27** — the stochastic particle step has landed
  (`src/flexpart/advance.rs`, 2026-10-02, table above); it takes its random
  deviates as input, after FLEXPART's `random_mod.f90`, and `outram-mc-libs`
  is still not a dependency (checked in `Cargo.toml`: only `uom` and `petir`).
- **`outram-foam-basic-lib`** will supply the gridded field and interpolation
  layer when the concentration-grid phase arrives. Deliberately **not** yet a
  dependency: the scalar kernels need nothing from it, and taking a
  finite-volume CFD dependency before there is a field to put on a mesh would be
  premature. **CORRECTED 2026-11-27** — the concentration gridding and met
  interpolation have since been ported (`conccalc.f90`, `interpol_*.f90`,
  2026-10-02) without it; it is still not a dependency.

## Licence

~~GPL-3.0~~ **AGPL-3.0-only, as a combined work** (relicensed 2026-10-20, gh:#973): the FLEXPART port keeps GPL-3.0 and `LICENSE.GPL-3.0` ships beside `LICENSE` (see `docs/relicensing-agpl-2026-10-20.md`). It contains two ports:

- **FLEXPART** (GPL-3.0-or-later, © FLEXPART 1998-2019) —
  [`LICENSE.flexpart`](LICENSE.flexpart), [`NOTICE.flexpart`](NOTICE.flexpart).
  Independent fork; not affiliated with or endorsed by NILU or the FLEXPART
  developers.
- **`puff`** (MIT, © 2025 Hammerling Research Group) —
  [`LICENSE.puff`](LICENSE.puff), [`NOTICE.puff`](NOTICE.puff). MIT into
  ~~GPL-3.0~~ **AGPL-3.0-only** is compatible and **one-way**: nothing in `src/puff/` may be copied
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
