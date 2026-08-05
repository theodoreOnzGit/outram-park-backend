# Scoping: BEDOK — porting Than Yan Ren's coupled nodal-diffusion / TH code

**Date:** 2026-08-05 · **Status:** scoping and strategy, not an approved plan.
**Nothing has been ported.**
**Domain:** BEDOK — systems-level multiphysics coupling (see
`docs/ecosystem-naming.md`).

> **Upstream author: Than Yan Ren**, fellow researcher at the Singapore Nuclear
> Research and Safety Institute (SNRSI). Source staged at
> `collaboration/BEDOKfiles/` (gitignored). 50 files, 13,012 lines of MATLAB.
>
> **Snapshot identity** — `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…`,
> 139,609 bytes, received 2026-08-05; newest source file dated 2026-07-16.
> **The code is unfinished, and Yan Ren has handed it over** — this snapshot is
> terminal, and completing it is now this project's job (see §1.0).

---

## 1. Strategy — two stages, with parity gates between them

This is the governing decision and everything below serves it.

### 1.0 The reference is unfinished, and it has been handed over

**Yan Ren's code is incomplete, but he has stopped working on it and handed
BEDOK to this project.** Updated 2026-08-05 — this supersedes an earlier
reading of the reference as a moving target. Two facts, and they pull in
different directions:

- **The snapshot is terminal.** No newer drop is expected, so there is no
  re-sync task. The snapshot named in the header *is* the upstream, finally.
- **The snapshot is not complete.** Whatever Yan Ren had not finished, nobody
  upstream will finish. Completing it is now this project's job.

**The rule that follows — and it matters more than it looks.** Translate the
gaps *as they are*, including the unfinished parts, and **record each one
explicitly** in the doc comment where it occurs and in
`docs/bedok-reference-defects.md`. Do not complete, repair or improve anything
during translation, even where the fix looks obvious.

**Corrections are stage-2 work** (decided 2026-08-05). But they are *not*
substitutions and cannot share their gate: a substitution must reproduce stage 1
within tolerance, whereas a correction **deliberately changes the answer**, so
parity cannot validate it. Each correction needs before/after numbers and a
justification that does not appeal to the reference — benchmark agreement, or a
physical argument. One at a time, never in the same change as a substitution.
See the defect register for the full rule and the current list.

The reason is not deference to Yan Ren — he is no longer maintaining it. It is
that a translation carrying silent repairs cannot be debugged: when the ported
code disagrees with the benchmark, there is no way to tell a translation error
from a well-meant "improvement". Keeping the two apart is the only thing that
makes the first disagreement diagnosable. Once translation parity is
established, completing the gaps is straightforward and safe.

### Stage 1 — faithful reference implementation

Translate Yan Ren's MATLAB to Rust **as directly as the language allows**. The
goal is *behavioural equivalence*, not elegance:

- **Do not re-architect.** Keep the solver structure, the iteration order, and
  the convergence logic as they are.
- **Do not optimise.** A faster formulation that changes floating-point
  accumulation order defeats the purpose.
- **Do not substitute** any OUTRAM PARK library — with the single decided
  exception in §3.
- **Do not "improve" the physics.** Anything questionable gets a doc comment,
  not a fix.

The output is the reference the rest of the work is measured against. The
snapshot is terminal, so it is frozen once translated — there is no re-sync
(§1.0).

### Stage 2 — substitute OUTRAM PARK libraries, gated on parity

Replace stage-1 components with the workspace's own libraries **one at a
time**, each behind a parity gate:

> **No component is accepted into the substituted path until it reproduces
> stage 1 on the benchmark suite to a stated tolerance. No improvement to any
> component before it has passed parity.**

That rule is the point of the exercise. A substitution that changes results
*and* claims to be better cannot be told apart from a substitution that is
simply wrong.

Only after a component is at parity may it be improved — and any improvement
then documents what changed relative to the reference and why.

**Consequence for layout:** stage 1 must survive as running code, not as an
archived branch. Parity tests need to call both paths in the same process.

---

## 2. What the code is

A coupled 3-D nodal-diffusion neutronics + thermal-hydraulics reactor code,
validated against published benchmarks.

| Group | Lines | Files | Content |
|---|---|---|---|
| **Nodal diffusion (SANM)** | 3,857 | 14 | Semi-analytic nodal method — transverse leakage, A1/A1234 expansions, ABEFGH coefficients, buckling, gradient/diffusion coefficients, fission-source extrapolation, plus a finite-difference fallback solver |
| **Benchmark cases + drivers** | 2,423 | 9 | IAEA-3D, NEACRP A1/A2, NEACRP D1, plus `main_exec_diff3d` and `run_neacrpd1t` |
| **Thermal hydraulics** | 1,450 | 9 | 1-D single-phase with evaporation (steady + transient), 6-equation drift flux, 1-D cylindrical fuel-rod conduction (steady + transient), W-3 CHF |
| **Coupling + XS feedback** | 1,436 | 5 | Steady and transient coupled drivers, cross-section feedback update, critical-boron search |
| **Utilities** | 485 | 12 | Index/sparse-format conversion, coordinate handling, NaN guards, plotting |
| *`IAPWS_IF97.m`* | *3,361* | *1* | *Third-party — not ported, see §3* |
| **Total to port** | **9,651** | **49** | |

The architectural spine is `thdiffusion_solvertimexyz` (transient coupled
solve), calling `sanodaldiffusion_solverxyz` for neutronics and
`th_solvertimexyz` for thermal hydraulics, with `sigmavalupd3d` closing the
cross-section feedback loop.

> **Correction, 2026-08-05.** An earlier draft of this document described the
> transient driver as "JFNK-preconditioned". **It is not.** `params.jfnkprecon`,
> `jfnkrel` and `jfnkverb` are set in `main_exec_diff3d.m` and `run_neacrpd1t.m`,
> but no file in the snapshot reads them — their owner,
> `driftflux_solverstatic1d.m`, is one of the files missing from the handover
> (along with `driftflux_eqnstatic1d5.m`, `enthmix_forward.m`,
> `enthmix_invert.m`, `bwrchfhottest.m`). The transient solver as shipped is a
> **linear implicit-Euler scheme with exponential transform**: one direct sparse
> solve per flux update, Picard passes for the feedback. No Newton, no Krylov,
> no preconditioner. The claim came from reading parameter names in the driver
> rather than the solver body.

**Why this is the right BEDOK payload:** semi-analytic nodal diffusion is
assembly-level and fast — above 1-D, below CFD, exactly the fidelity band BEDOK
was defined for. Nothing in the workspace does coarse-mesh nodal diffusion
today; GeN-Foam's SP3 is the tier above.

---

## 3. The one substitution allowed in stage 1 — and its parity gate

**`IAPWS_IF97.m` is not ported.** Decided 2026-08-05. It is third-party
(`Copyright (c) 2013 Mark Mifofski`), 3,361 lines, and `tampines-steam-tables`
already implements IAPWS-IF97 in Rust. The MATLAB calls it almost entirely
through `_ph` entry points (`T_ph`, `v_ph`, `cp_ph`, `x_ph`), which is exactly
the `(p,h)` flash interface `tampines-steam-tables` is built around.

**But this substitution is itself a parity risk, and it happens *inside* the
reference implementation.** If the two IF97 implementations disagree anywhere
the benchmarks exercise, every downstream comparison silently inherits that
discrepancy — and stage 1 stops being a clean oracle.

> **Therefore the first gate comes before any porting.** Since the MATLAB will
> not be run (§4), the check is not implementation-against-implementation but
> **both against the standard**: confirm `tampines-steam-tables` reproduces the
> **published IAPWS-IF97 verification values** over the pressure, enthalpy and
> temperature ranges the four benchmark cases actually exercise, to a
> documented tolerance.
>
> This is the stronger check anyway — IF97 is a published standard with
> official verification tables, so agreement with the standard implies
> agreement with any correct implementation, including Mifofski's. If
> `tampines-steam-tables` already carries such a test, the work is to confirm
> its range covers the benchmarks' operating envelope (PWR ~15.5 MPa, BWR
> ~7 MPa, plus the two-phase region the BWR case enters), not to write it
> again.

---

## 4. Verification — one oracle, and what that costs

**Decided 2026-08-05: the MATLAB will not be run.** Verification is against the
**published benchmark reference values** — IAEA-3D, NEACRP-L-335 (PWR rod
ejection, cases A1 and A2), and NEACRP D1 (BWR cold-water injection). All are
public and citable, and clean under `DATA_POLICY.md`. The bar is **rough parity
with the V&V cases**, at the accuracy a nodal-diffusion code can be expected to
reach.

**No golden fixtures ship with the snapshot.** The ten CSVs in `BEDOKfiles/`
were checked and are all *inputs* — 17×17 assembly composition maps and control
rod-bank patterns — not reference outputs. Neither MATLAB nor GNU Octave is
installed on this machine.

**State the consequence plainly, because it changes what may be claimed.**
Without running the reference, the port is verified **against the benchmark**,
not verified **as a faithful translation**. Those are different claims:

- If the port matches the benchmark, that is a real V&V result and may be
  reported as one.
- If the port *disagrees* with the benchmark, the cause is ambiguous — a
  translation error, or Yan Ren's code also disagreeing (entirely possible,
  since it is unfinished). Nothing available distinguishes them.

So **"reproduces Yan Ren's results" must not be claimed anywhere** unless the
reference is actually run. The honest formulation is "translated from Yan Ren's
implementation; verified against the published benchmarks".

Per the workspace V&V rule, each gate records **methodology and measured
results with uncertainty**, not merely "ported".

**Recovering the tight oracle, if wanted later.** Installing GNU Octave is
cheap and would restore the ability to diff against the reference directly.
Compatibility is not guaranteed — `gmres`, sparse handling, `-v7.3` `.mat`
saving and the `varargin` patterns are the likely friction — but it is worth
keeping in reserve for whichever module proves hardest to get to parity.

---

## 5. Stage-2 substitution map

Candidate replacements, each gated on parity:

| Stage-1 component | Substitute | Notes |
|---|---|---|
| `singleflow1devap` / `…time` | `tuas_boussinesq_solver`, `tampines` | Single-phase channel TH |
| `driftflux6_solverstatic3d` | `outram-foam-multiphase::drift_flux` | Exists; fidelity match unverified |
| `w3chf` / `w3chfhottest` | `outram-foam-multiphase::chf`, appbuilder `closures::heat_transfer::chf` | Both exist — check which implements W-3 |
| `fuelrodheat_1dcylnd` / `…time` | `outram-park-fork-offbeat`, TUAS `one_d_solid_structure` | OFFBEAT is the richer model |
| Delayed-neutron kinetics in the transient path | `teh-o-prke` | |
| Cross-section data and feedback | `njoy-outram-park-fork` | Benchmarks supply their own two-group sets; this is a later step |
| Sparse linear algebra / GMRES | `outram-foam-basic-lib` `ldu_matrix`, `krylov` | Iteration-order differences will move results — expect tolerance work |

The last row is the one most likely to break bit-level agreement while being
perfectly correct. Parity tolerances must be set physically, not to machine
epsilon.

---

## 6. Permission and attribution

**Permission to translate has been given.** Recorded here because
`RESEARCH_INTEGRITY_AND_PROVENANCE.md` requires the provenance of ported work
to be documented rather than assumed:

| | |
|---|---|
| **Author** | Than Yan Ren, fellow researcher, SNRSI |
| **How it arose** | Yan Ren approached the maintainer; the translation is something he wanted |
| **Permission to translate** | Given by Yan Ren directly, by email |
| **Scope of that permission** | **Explicitly open-source, under OUTRAM PARK** — stated up front, not inferred |
| **Institutional approval** | Given by SiCong (project lead), explicitly for sharing in the open-source repository |
| **Recorded** | 2026-08-05 |

The scope question that matters for a copyleft repository — translation for
internal use versus publication as open source — was settled before the fact:
the open-source destination was stated explicitly when permission was sought,
and approved at both the author and project-lead level. **No further clearance
is outstanding.** Retain the email as the record.

**Attribution.** Every ported file carries a header naming **Than Yan Ren
(SNRSI)** as the original author, the original `.m` filename, and the snapshot
the translation was taken from.

---

## 7. Conventions

- **Naming.** Same three-name rule as the code_aster port
  (`docs/code-aster-port-scoping.md` §4): a descriptive Rust name for
  rust-analyzer, and the original MATLAB function and file name in the doc
  comment for traceability. `calc_a1234_expansionxyz` is provenance, not an
  API.
- **Indexing.** MATLAB is 1-based and column-major; the port is 0-based. Index
  arithmetic in `convertindexc2d`, `convertsparsekey3d`, `handle3dcoords` and
  the `calc_*xyz` family is where translation errors will concentrate. These
  want unit tests against MATLAB-generated fixtures before the solvers are
  trusted.
- **Workspace Rust rules** apply unchanged: enum dispatch, no trait objects, no
  `Box<T>`, no lifetime parameters, `uom` where a public quantity is exposed.
- **Do not commit the MATLAB.** `collaboration/` is gitignored and stays so.

---

## 8. Open questions

1. **Parity tolerances.** What counts as "reached parity" — per-quantity
   relative tolerance on the C1–C4 histories, on peak power and its timing, on
   `k_eff`? Should be fixed before stage 2 starts, not negotiated per
   substitution.

---

## 9. Proposed crate layout (needs confirmation)

A new `bedok` crate — justified as a new domain with no existing home, and
independently publishable. **Stage 1 and stage 2 live side by side** so parity
tests can call both in one process:

```
crates/bedok/
  src/
    reference/          <- stage 1: faithful translation of the terminal
                        <-          snapshot; frozen once translated
      nodal/            <- SANM: expansions, transverse leakage, buckling
      th/               <- channel flow, fuel rod, CHF, drift flux
      coupling/         <- steady + transient drivers, XS feedback, boron search
      grid/             <- indexing, sparse conversion, coordinates
    substituted/        <- stage 2: OUTRAM PARK libraries swapped in
    cases/              <- IAEA-3D, NEACRP A1/A2/D1 geometry + data
  tests/
    parity/             <- reference vs substituted, per component
    benchmark/          <- both paths vs published reference values
```

Alternative considered and not recommended: two crates, `bedok-reference` and
`bedok`. It makes the frozen-oracle status clearer but complicates the parity
tests and doubles the release surface for no physics gain.
