# CLAUDE.md — `changi`

Crate-specific guidance. The workspace `CLAUDE.md` governs everything else and
is not repeated here.

## What this crate is

Atmospheric dispersion, plume transport, deposition and ground contamination —
the middle link of the offsite chain (SEMBAWANG → **CHANGI** → REDHILL — both
now exist as `sembawang`/`redhill` placeholder crates, no implementation). It
hosts a Rust port of **FLEXPART v10.4** (GPL-3.0-or-later, commit `3d7eebf`).

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
(gitignored, reference-only, never compiled into the crate). Read the Fortran
before proposing any change to ported code — the workspace "read upstream first"
rule applies with full force.

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

## Android / wasm

Non-GUI library code, no OS threads, no filesystem access at run time (the test
fixtures are `include_str!`-baked). It must keep compiling for
`aarch64-linux-android` and `wasm32-unknown-unknown`; `changi` is in
`scripts/check-wasm.sh`'s **in-scope** set, not its exclusion list.

`dev/` holds Fortran and a shell script. Neither is built by cargo, so neither
affects either target.

## Tracking

Epic `op-k9em`, children `op-k9em.1` … `op-k9em.4`.
