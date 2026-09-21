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
provenance files and verification harness:

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
force, and on this crate it has already paid: every one of the four upstream
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

**Half-lives come from `boon-lay`.** The decay module holds no nuclide data of
its own, deliberately, so the two cannot drift. Do not add a nuclide table here.

**`puff`'s unit conversion is methane-specific and must not be generalised by
assumption.** Upstream is an oil-and-gas leak-detection package; its
`1e6 * 1.524` converts kg/m^3 to ppm *of methane*. CHANGI's subject is
radionuclides, for which that factor is wrong twice over — wrong molar mass,
and ppm is the wrong unit for an activity concentration, which belongs in
Bq/m^3. `gaussian_puff_concentration` returns a `MassDensity` for exactly this
reason; the ppm helper exists for the code-to-code comparison.

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
