# CLAUDE.md — `changi`

Crate-specific guidance. The workspace `CLAUDE.md` governs everything else and
is not repeated here.

## What this crate is

Atmospheric dispersion, plume transport, deposition and ground contamination —
the middle link of the offsite chain (SEMBAWANG → **CHANGI** → REDHILL). It
hosts a Rust port of **FLEXPART v10.4** (GPL-3.0-or-later, commit `3d7eebf`).

## Scope limit — capability vs framing, and do not blur them

CHANGI's scope **does** include radiological consequence assessment, dose
assessment, emergency-planning support and Level 3 PSA support. Those are on the
maintainer's scope list and are legitimate things to implement here.

What is forbidden is presenting the **outputs** as authoritative for operational
emergency response, for dose assessment of real populations, or for regulatory
or licensing decisions. `docs/ecosystem-naming.md` decision 3 (2026-08-05) draws
the line in one sentence: *"The capability is in scope; that framing is not."*
`RESPONSIBLE_USE.md` line 43 is the same rule from the other side — outputs
"must not be treated as authoritative for safety, licensing, operational,
regulatory, or emergency-response purposes".

So: implement the methods, document them as research/V&V, and never write a
sentence that offers the results for operational use. An earlier naming draft
blurred exactly this and was corrected; an agent that re-blurs it is undoing a
deliberate decision.

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
