# CLAUDE.md — `changi`

Crate-specific guidance. The workspace `CLAUDE.md` governs everything else and
is not repeated here.

## What this crate is

Atmospheric dispersion, plume transport, deposition and ground contamination —
the middle link of the offsite chain (SEMBAWANG → **CHANGI** → REDHILL — both
now exist as crates; ~~both placeholders, no implementation~~ **CORRECTED
2026-09-21** — `sembawang` implements the TRISO release half and feeds
`changi::activity`; `redhill` is still a placeholder).

It hosts **two independent ports**, each with its own upstream, licence,
provenance files and verification harness (**CORRECTED 2026-09-28** — plus a
third, non-ported module, `activity` (`src/activity/`), which consumes both ports
and has no upstream and no code-to-code harness; see its module doc. **Since
2026-09-29 (gh:#380)** its `chi_over_q` has an independent check against a
hand-written age sum and buangkok's pyDOSEIA plume, in
`crates/buangkok/tests/changi_puff_train_vs_plume.rs` — it lives in buangkok
because buangkok depends on changi, not the reverse):

| Module | Upstream | Licence | Harness |
|---|---|---|---|
| `flexpart` | FLEXPART v10.4, commit `3d7eebf` | GPL-3.0-or-later | Fortran, `dev/flexpart_reference.f90` |
| `puff` | Hammerling-Research-Group/puff `0.1.1`, commit `5213d58` | MIT | R, `dev/gen_puff_reference.R` |

**Keep them separate.** They model different things (Lagrangian particles on
gridded meteorology vs. analytic puffs on a single wind series), they carry
different licences, and their reference harnesses are in different languages.
Do not merge their shared-looking pieces — a "common stability class" or a
"common dispersion coefficient" abstraction spanning both would make each one's
code-to-code comparison against *its own* upstream harder to read and easier to
break.

## Scope limit — binding, do not soften

**CHANGI is research, education and V&V only.** Never describe it, in code,
docs, commit messages or chat, as supporting emergency planning, emergency
response, dose assessment for real populations, or Level 3 PSA.

Radiological consequence assessment, dose assessment, emergency-planning support
and Level 3 PSA support appear in the crate's **future** scope list. They are
recorded so the direction is not lost; they are **not** what this crate is for
today (maintainer direction, 2026-09-15), none is implemented, and none may be
advertised as available.

This is not boilerplate. `docs/ecosystem-naming.md` decision 3 (2026-08-05)
records that the original naming draft claimed exactly that capability and was
**corrected** because it contradicted `RESPONSIBLE_USE.md`. An agent that
reintroduces the framing is undoing a deliberate decision. Promoting an item
from the future list to the current one is a maintainer decision taken in
`RESPONSIBLE_USE.md`, never something an agent infers from having implemented
the physics.

## Maturity

**Not declared mature.** No maturity bar is claimed, so the workspace API
dogfooding rule ("if it is too complex for Haiku…") does not yet apply. Do not
propose maturity without the evidence the workspace gate requires, and never
flip the README's `Bookkeeping status` axes — those record *human* review.

## Porting rules specific to this crate

**Upstream is the specification.** FLEXPART is at `upstream_source/FLEXPART`
and `puff` at `upstream_source/puff` (both gitignored, reference-only, never
compiled into the crate). Read the upstream source before proposing any change
to ported code — the workspace "read upstream first" rule applies with full
force, and on this crate it has already paid: every one of the ~~four~~ five (CORRECTED 2026-10-03) upstream
`puff` defects in `docs/puff-code-to-code.md` came from reading or running
upstream, not from reasoning about the physics.

**Keep the provenance header** on every ported file: upstream project, version,
commit, the exact upstream source file, copyright and licence. It is what lets a
reviewer open the two side by side.

**Reuse before porting.** `petir` supplies the numerics; FLEXPART's `erf.f90` is
deliberately not ported because `petir::specfunc::erf` already covers it with
its own test suite, and the two were measured to agree bit-for-bit. Before
porting any further FLEXPART numerical utility, check `petir` first, then
`outram-foam-basic-lib`.

**Half-lives come from `boon-lay`; published nuclide tables live here.**
The decay module holds no half-life or decay-constant data of its own,
deliberately, so it cannot drift from `boon-lay`. ~~Do not add a nuclide table
here.~~ **CHANGED 2026-09-28** (maintainer: "update changi to include nuclide
tables"): changi **is** the home for published nuclide tables of activities
and releases, since the dispersion chain is their consumer. Each one sits in
`reference/` as a CSV, is compiled in by its own loader under
`src/activity/` (the two-column ones sharing `inventory::parse_nuclide_bq_csv`;
Table 8's two-case CSV has its own three-column parser), and has a full
provenance section in `docs/References.md`. The rule that still stands: **no
half-life or decay-constant tables** here. Those come from `boon-lay`.

Tables held today, all from Liu and Cao (2002), NED 218, 81–90:

| Table | What | CSV | Loader |
|---|---|---|---|
| 1 | Equilibrium-core inventory, 22 nuclides (Bq) | `htr10_equilibrium_core_inventory.csv` | `activity/inventory.rs` |
| 2 | Release rate from the fuel elements, equilibrium core, normal operation, 22 nuclides (Bq h^-1 MWt^-1). **Row ADDED 2026-10-04:** the table has been held since 2026-09-29 (`f464c76f33`), and this list omitted it | `htr10_fuel_element_release_rate.csv` | `activity/fuel_release.rs` |
| 3 | Primary-helium activity at end of a 20-year life, 20 nuclides (Bq) | `htr10_primary_helium_activity_end_of_life.csv` | `activity/primary_helium.rs` |
| 5 | Annual normal-operation airborne release, 22 nuclides (Bq/a) | `htr10_normal_operation_annual_airborne_release.csv` | `activity/airborne_release.rs` |
| ~~7~~ | ~~Individual effective dose vs distance, normal operation (mSv/a)~~ — **MOVED to `buangkok::published` 2026-09-28** | — | — |
| 8 | Airborne release for two design-basis accidents (depressurization, water ingress), 18 nuclides (Bq per accident); the paper's "C-4" stored as C-14. Added 2026-09-28 | `htr10_accident_airborne_release.csv` | `activity/accident_airborne_release.rs` |
| ~~9~~ | ~~Individual thyroid and whole-body dose vs distance, two design-basis accidents (mSv)~~ — **MOVED to `buangkok::published` 2026-09-28** | — | — |

~~Nothing in the workspace consumes these tables yet.~~ **CORRECTED
2026-09-29** (checked by grep): Table 1 is consumed by `htgr_sim_v1`'s
`physics::fission_product_release` (`changi::activity::inventory`), which
drives the absolute arm of its dispersion map and, since 2026-09-29, its
indicative dose rate; Table 8 is referenced by `buangkok::published`
(`AccidentCase`). ~~Tables 3 and 5 still have no consumer.~~ **CORRECTED 2026-10-04**
(checked by grep over `crates/`): Table 3 is read by `htgr_sim_v1`
(`physics::primary_loop`, `physics::fission_product_release`), by
`sembawang::lwr_comparison` and by three `sembawang` examples
(`htr10_air_ingress_kora_bound`, its human-audited copy, `htr10_dlofc_dose`);
Table 5 by `sembawang::lwr_comparison` and `buangkok`'s
`tests/liu_cao_external_dose_cross_check.rs`; Table 2 by `htgr_sim_v1`'s
`physics::fission_product_release` (its uncalibrated release check, gh:#399).

~~**The Table 7 and Table 9 dose tables are parked
here, not settled here** (**CORRECTED 2026-09-28**: Table 9, the accident
doses, a published STOERNEU result, was added on the same terms). Each is stored,
cited data (Table 7 a published AIRDOS-EPA result). Neither is computed, and changi
still computes no dose. The maintainer (2026-09-28) may move dose data to
another crate: dose is a biological quantity, and keeping it apart from the
dispersion physics avoids it being mistaken for a health-assessment
capability. This workspace uses it for safety analysis in the research
sense only (see `RESPONSIBLE_USE.md`). The maintainer named **`buangkok`** as the home for dose (placeholder crate,
2026-09-28), but has not asked for ~~this table~~ either table to move. **Leave them where they are
until the maintainer decides.** Do not build on them, move them, or add dose computation
around them unasked.~~ **SETTLED 2026-09-28** (maintainer: "move table 7 and 9
to buangkok"): both dose tables moved to the dose crate (`buangkok::published`,
provenance in `crates/buangkok/docs/References.md`). changi holds **no dose
data** and computes no dose. The accident releases (Table 8) stay here, and
`AccidentCase` is defined in `activity::accident_airborne_release` (buangkok
re-exports it).

**`puff`'s unit conversion is methane-specific and must not be generalised by
assumption.** Upstream is an oil-and-gas leak-detection package; its
`1e6 * 1.524` converts kg/m^3 to ppm *of methane*. CHANGI's subject is
radionuclides, for which that factor is wrong twice over — wrong molar mass,
and ppm is the wrong unit for an activity concentration, which belongs in
Bq/m^3. `gaussian_puff_concentration` returns a `MassDensity` for exactly this
reason; the ppm helper exists for the code-to-code comparison.

**External numbers go in `docs/References.md`, with their retrieval status.**
The Singapore wind climatology the examples run at is recorded there with its
source, the date, which figures are MSS's and which are this project's own
sector choices, and the fact that `weather.gov.sg` was egress-blocked so the
page was never read directly. Do not add a climatological or material figure
to this crate without a row in that file, and do not upgrade a
`Not re-checked` row without actually re-checking it.

**Watch `uom` round trips on very small quantities.** Returning ppm as a
`uom::Ratio` was measured to lose five significant digits, because `Ratio`
stores its base unit and the `/1e6` pushed puff concentrations subnormal. `uom`
is still the right default everywhere else in this crate — but a quantity whose
natural magnitude is far from its base unit needs the round trip checked, not
assumed. Recorded in `docs/puff-code-to-code.md`.

## Verification: always build BOTH Fortran references

**FLEXPART ships in single precision** — its makefile passes no
`-fdefault-real-8`, so `real` is `real(4)` and `pi` stores as `3.14159274`.

`dev/build_reference.sh` therefore builds the driver twice, and
`tests/flexpart_code_to_code.rs` checks both. **Do not drop the `real8`
reference**: it is the only thing that separates a translation error from
upstream's storage format. A `3e-6` disagreement against the shipped build is
expected and means nothing on its own; a `3e-6` disagreement against the `real8`
build is a bug.

When adding a ported routine, extend `dev/flexpart_reference.f90` to call the
upstream routine over a branch-covering grid, regenerate both fixtures, and add
the group to the test. Never hand-write an expected value.

**Since 2026-10-02 (gh:#410) each stage has its own driver**
(`dev/flexpart_reference_<stage>.f90`), build script
(`dev/build_reference_<stage>.sh`, all run by `dev/build_reference.sh`),
fixtures and test, and they share one harness, `tests/common/mod.rs`. Five
rules carry over from what those stages found:

- **Real(4) misses need a diagnosis, not a tolerance.** Use
  `Real4Rule::PrecisionSpreadOn(columns)` only on columns shown to be
  ill-conditioned in `f32`, or take rows out of real(4) scope on a criterion
  derived outside the comparison, and document which. Feeding both builds
  inputs that are exact in `f32` (dyadic literals, or double values rounded
  once) makes the real(4)-vs-real(8) spread a clean measure of upstream's
  arithmetic.
- **Only configuration may be edited.** `par_mod.f90` copies (nest
  dimensions, output switches) are FLEXPART's user configuration; each build
  checks the diff. Never edit another upstream file.
- **No Numerical Recipes code.** `random_mod.f90` is replaced in the
  deterministic drivers by shims returning driver-chosen draws. The port takes
  draws as inputs. Only the stochastic driver compiles the real `random_mod`,
  as a reference.
- **Fortran precedence is part of the translation.** `a*b**2` is `a*(b*b)`. A
  one-ulp residual against real(8) is a translation defect until shown
  otherwise. `obukhov`'s was, and it had been written off as upstream
  rounding.
- **The stochastic test** (`tests/flexpart_stochastic_advance.rs`) compares
  against FLEXPART's own generators in replicates. Its seed is fixed: never
  change it to make a comparison pass.

Results and analysis: [`docs/flexpart-code-to-code.md`](docs/flexpart-code-to-code.md).

## Verification: `puff` runs the upstream R

`dev/gen_puff_reference.R` sources upstream's `R/helpers.R`,
`R/simulate_sensor_mode.R` and `R/simulate_grid_mode.R` and sweeps them over a
branch-covering grid. Requires R (>= 4.1) and `dplyr` — upstream's `simulate_*`
call `dplyr::bind_rows`, and nothing else on the physics path needs a package.

Unlike FLEXPART there is **no dual-precision reference**, and none is needed:
upstream R is double precision, so a single fixture isolates the translation
with no storage-format floor in the way. Tolerances sit near machine epsilon
accordingly — do not loosen them to an engineering bound.

Two rules when extending it:

- **Sweep the same inputs the caller uses.** The sigma grid initially missed
  `0.001 km`, the distance `gpuff` was actually being called at, which left a
  real residual unexplained and briefly pointed at the wrong cause. The grid now
  derives its distances from the `gpuff` sweep.
- **Do not "improve" a constant.** Upstream writes degrees-to-radians as
  `0.017453293`, which differs from `PI/180` in the 9th significant figure —
  `2.8e-8` relative, four orders of magnitude above the tolerance. The literal
  stays, and a mutation substituting `PI/180` is in the non-vacuity table.

Results and analysis: [`docs/puff-code-to-code.md`](docs/puff-code-to-code.md).

## Android / wasm

Non-GUI library code, no OS threads, no filesystem access at run time (the test
fixtures are `include_str!`-baked). It must keep compiling for
`aarch64-linux-android` and `wasm32-unknown-unknown`; `changi` is in
`scripts/check-wasm.sh`'s **in-scope** set, not its exclusion list.

`dev/` holds Fortran, a shell script and an R script. None is built by cargo, so
none affects either target.

## Tracking

Epic `op-k9em`, children `op-k9em.1` … `op-k9em.4`.
