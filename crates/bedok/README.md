# BEDOK

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

---

**Systems-level multiphysics coupling** — three-dimensional nodal-diffusion
neutronics coupled to thermal hydraulics, at the fidelity band **above
one-dimensional neutronics and below CFD**.

Neighbouring tiers: GeN-Foam in `outram-foam-appbuilder-lib` owns CFD-fidelity
multiphysics coupling; `nee_soon` owns neutronics-and-nuclear-data integration.
See `docs/ecosystem-naming.md`.

## Provenance

A Rust translation of a MATLAB implementation by **Than Yan Ren**, fellow
researcher at the Singapore Nuclear Research and Safety Institute (SNRSI).

He approached the maintainer about the translation, gave permission by email
with the open-source destination stated up front, and **SiCong** approved
sharing in this open-source repository. Every ported file carries his
attribution and names the source `.m` file and the snapshot it came from
(sha256 `e45cd6f57be2087c…`).

**The original was handed over unfinished, and he has stopped work on it.** The
snapshot is therefore terminal — there is no upstream to re-sync with, and
completing the gaps is this project's job.

## Two stages

| | |
|---|---|
| [`reference`](src/reference) | **Stage 1** — the faithful translation. Structure, iteration order and convergence logic follow the MATLAB line for line. Deliberately not idiomatic and not optimised. |
| [`substituted`](src/substituted) | **Stage 2** — the same physics rebuilt on OUTRAM PARK libraries. Not started. |

Both paths coexist so parity tests can call them in the same process.

**The rule stage 2 rests on:** no component is accepted until it reproduces
stage 1 on the benchmark suite within tolerance, and **no component is improved
before it has passed parity**. A substitution that changes results *and* claims
to be better cannot be told apart from one that is simply wrong.

The single substitution allowed inside stage 1 is IAPWS-IF97, which comes from
`tampines-steam-tables` rather than porting the third-party `IAPWS_IF97.m`
(Copyright © 2013 Mark Mifofski).

## Defects in the reference — read this before changing anything

**[`docs/bedok-reference-defects.md`](../../docs/bedok-reference-defects.md)**
records **57 defects** found in Yan Ren's MATLAB while translating it. **None
of them is fixed**, by design.

Translating the gaps as they are — rather than repairing them in passing — is
what makes a later disagreement diagnosable: when the port first differs from
the benchmark, a translation error can be told apart from a well-meant
"improvement". Repairs made during translation destroy that distinction
permanently.

**Corrections are stage-2 work, and they are not substitutions.** A
substitution must *not* change results and is validated by parity; a correction
*deliberately* changes them and therefore cannot be. Each correction needs
before/after numbers and a justification that does not appeal to the reference,
done one at a time and never in the same change as a substitution. The register
explains the full rule.

Some of the more consequential entries: fuel node volumes identically zero in
all four NEACRP cases; an uninitialised control-rod level exactly when a bank is
fully withdrawn — the end state of every rod-ejection transient; a
hottest-channel search confined to the diagonal by an `ix`/`iy` slip; two
solvers exiting silently on their iteration cap; and a nodal-update interval
whose own default formula destabilises small meshes.

## Benchmarks

| Case | Source |
|---|---|
| IAEA-3D (problem 11-A1) | Argonne Code Center, *Benchmark Problem Book*, ANL-7416 (Suppl. 2), 1977 |
| NEACRP-L-335 A1, A2 | PWR rod-ejection transients |
| NEACRP D1 | BWR cold-water injection transient |

## Verification status — stated precisely

- The committed fixtures under `tests/fixtures/` record **what Yan Ren's
  implementation produces**, captured by running it under GNU Octave with
  compatibility shims (none of which changes a computed result).
- His implementation gives `k_eff = 1.0290842762` on IAEA-3D against the
  ANL-7416 extrapolated reference of `1.02903` — about **+5.4 pcm**.
- **The Rust translation has not yet been run against those fixtures.** No
  parity claim is made. The parity tests exist and are `#[ignore]`d pending the
  solver wiring.
- 270 tests pass in release mode. They are internal-consistency, correlation
  and index-convention checks — **not** agreement with the reference or with
  the published benchmarks.

Agreement with the fixtures would show the translation is *faithful*, which is
not the same as being *correct*. Benchmark comparison is a separate check.

## Design notes

The state-vector index convention is pinned once, in
[`reference::grid`](src/reference/grid.rs), and taken from
`main_exec_diff3d.m:176` — group-major, then `ix`, then `iy`, with `iz` varying
fastest, 1-based in the MATLAB. Getting it wrong does not crash anything: it
silently permutes the reactor and converges happily to a wrong answer. It is
tested by round-trip over all 10,982 entries.

Only the reduced, benchmark-shaped fixtures are committed (about 20 kB). The
full node-level fields are ~1.4 MB, matter only when a parity failure has to be
pinned to a specific node, and regenerate in about 77 seconds — see
`tests/fixtures/iaea3d/PROVENANCE.md`.

## Further reading

- [`docs/bedok-port-scoping.md`](../../docs/bedok-port-scoping.md) — the
  governing plan: strategy, module inventory, substitution map, conventions
- [`docs/bedok-reference-defects.md`](../../docs/bedok-reference-defects.md) —
  the defect register and the correction rule
- [`docs/ecosystem-naming.md`](../../docs/ecosystem-naming.md) — where BEDOK
  sits among the OUTRAM PARK domains
