# BEDOK

3-D nodal-diffusion neutronics coupled to thermal hydraulics — the fidelity
band **above 1-D neutronics and below CFD**.

A Rust translation of Than Yan Ren's (SNRSI) MATLAB implementation, ported from
the `main_exec_diff3d_standalone` snapshot with the author's permission.

> **Status: translation complete.** All 50 MATLAB files are accounted for —
> every solver, both coupling drivers, the critical-boron search, and all six
> benchmark cases. The IAEA-3D eigenvalue matches the published value to
> **-1.1 pcm**.
>
> **Both steady NEACRP cases now reproduce the MATLAB exactly.** Running the
> reference under MATLAB R2026a and comparing: A2 (PWR, 15.5 MPa, graded mesh,
> five feedback channels) gives `k_eff = 1.0139476080` in both codes, and D1
> (BWR, 6.7 MPa, boiling coolant, uniform mesh) gives `0.9752848326` in both —
> with fuel temperature, coolant temperature, heat flux and power identical to
> every printed digit.
>
> **The transient path now reproduces the MATLAB too**, on all three cases: the
> D1 cold-water injection (C1 power to 1.6e-11), the A2 rod ejection (rod
> position exact at every step), and the A1 super-prompt HZP ejection — the last
> agreeing to **2.1e-7 through a 67-fold power excursion**. D1t is also verified
> over its **full specified 20 s window** (261 steps, 2.9e-11), including the
> non-monotonic power peak and the fuel melting clamp.
>
> **IAEA-3D matches the MATLAB too** (`k_eff = 1.0290842762`, -0.0000 pcm), and
> the critical-boron search **fails where the reference fails** — both abort at
> the same boron concentration on a destabilised eigensolve.
>
> No transient has been compared to a *published* curve; the NEACRP
> specification is not in the literature archive. See
> `docs/bedok-reference-defects.md`, "Verified against the running MATLAB", for
> the full table and for what remains uncompared.
>
> The two top-level
> scripts land as `examples/` rather than modules, and `plotreactor3dcolour`'s
> figure emission is deliberately not reproduced. See [Porting status](#porting-status).
>
> **Two things to know before running a NEACRP case.**
>
> 1. **Case A2 must not be run at the default nodal-update interval.** The
>    default for its mesh is `ceil((17+17+18)/10) = 6`, and at 6 the inner
>    eigensolve is unstable — a cold solve diverges in **both** codes (3661 here,
>    837 in the MATLAB), so the coupled answer is meaningless. Use
>    `nodalupd >= 20`. Case D1 is stable at its own default. This is defect N1
>    with a real-case consequence.
> 2. **The PWR cases use a graded axial mesh**, and the diffusion operator is
>    only a consistent discretisation on a uniform one — defect G1, misstating
>    the face coupling by up to **+137.5%** at A2's worst axial joint. Pinned by
>    test, not repaired; both codes carry it identically.

## What this crate is

A coupled reactor code: semi-analytic nodal diffusion (SANM) for the
neutronics, 1-D channel thermal hydraulics with a drift-flux option for the
coolant, 1-D cylindrical conduction for the fuel rod, and a cross-section
feedback loop closing the two. It carries the IAEA-3D, NEACRP A1/A2 and
NEACRP D1 benchmark cases.

The diffusion coefficient, for orientation, is the usual

$$ D = \frac{n}{(2n + 1) \Sigma_{tot}} $$

with $n = 1$ giving $D = 1 / (3 \Sigma_{tot})$.

## How it is laid out

**One module per `.m` file, flat, named after the original.** `th_solverxyz.rs`
is `th_solverxyz.m`. There is no `nodal/`, `th/` or `coupling/` grouping,
because the MATLAB has none and the point of the translation is that the two
can be read side by side.

Module names are `snake_case` even where the original was not — `makegradDxyz.m`
becomes `makegrad_dxyz` — and the original filename is always named in the
module's doc comment.

Three modules have **no** `.m` counterpart and say so at the top:

| Module | Role |
|---|---|
| `matlab` | 1-based column-major containers, sparse triplets, `find`, `nnz` |
| `types` | the `params` / `geometry` structs the reference passes everywhere |
| `error` | the conditions the reference raises with `error(...)` |

## Indexing

**The port is 0-based.** The reference's 1-based arithmetic is converted, so

$$ idx = (g-1) e_s + (ix-1) \, maxiy \cdot maxiz + (iy-1) maxiz + iz $$

becomes

$$ idx = g \, e_s + ix \cdot maxiy \cdot maxiz + iy \cdot maxiz + iz $$

Storage stays **column-major**, because the reference does linear indexing into
multi-dimensional arrays and the layout is observable.

Two places where the conversion was **not** mechanical — check these first if an
index bug appears:

| Where | What changed |
|---|---|
| `convert_grid3d` | used `0` as a "no material" sentinel, which only worked because MATLAB indices start at 1. The map is now `Option<usize>`, `None` for absent |
| `convertindexc2d` | keeps 1-based arithmetic **internally** and converts at the boundary, because it maps between two index spaces whose definition (the `(2n+1)` half-index grid) is stated in 1-based terms where the offsets are load-bearing |

Separately, `whichsigma` stores **1-based material numbers with `0` meaning
void**, straight from the benchmark composition CSVs — so a node holding
material `m` reads row `m - 1` of the cross-section table.

## Translation policy — no silent repairs

The MATLAB is unfinished and the snapshot is terminal. Defects are translated
**as they are**, described in the doc comment of the item that carries them,
and pinned by a test that asserts the wrong behaviour — so a later fix is a
visible, deliberate change rather than a drift.

The reasoning: a translation carrying well-meant fixes cannot be debugged
against a benchmark, because a disagreement can no longer be attributed to
either the translation or the original. Keeping the two apart is the only thing
that makes the first disagreement diagnosable.

Two facts about the reference set this policy, and they pull in different
directions. **The snapshot is terminal** — Yan Ren has stopped working on the
code and handed BEDOK to this project, so there is no re-sync task and the
snapshot named in each module header *is* the upstream, finally. But **the
snapshot is not complete**, and whatever he had not finished, nobody upstream
will. Completing it is now this project's job.

So: translate the gaps *as they are*, including the unfinished parts, and record
each one in the doc comment where it occurs and in
`docs/bedok-reference-defects.md`. Do not complete, repair or improve anything
during translation, even where the fix looks obvious.

**Corrections are a separate stage**, and they are *not* substitutions and
cannot share their gate: a substitution must reproduce the faithful translation
within tolerance, whereas a correction **deliberately changes the answer**, so
parity cannot validate it. Each correction needs before/after numbers and a
justification that does not appeal to the reference — benchmark agreement, or a
physical argument. One at a time, never in the same change as a substitution.

Defects found so far:

| Module | Defect |
|---|---|
| `convertindexc2d` | mode 1 → mode 2 → mode 1 is **not** the identity; the forward row calculation is off by one at the start of each row, and the reverse does not make the matching error. Found by running the tests |
| `handle3dcoords` | generic branch assigns `params.maxix` where `params.maxi3` was meant |
| `convert_grid3d` | precursor indices collide for `Nc > 1`; latent at `Nc = 1` |
| `geometry_ends3d` | only the first contiguous run per grid line is found |
| `convertsparsekey3d` | diagnostic decode hard-coded to a 17x17x19 grid |
| `calc_bucklingxyz` | the cache fingerprint is three sums and three non-zero counts, which cannot separate every distinct cross-section set. A collision silently reuses the wrong cached coefficients — a real risk in a T-H feedback loop, where the cross sections move by small amounts each pass |
| `calc_ABEFGHxyz` | `abefgh` loses precision to cancellation as `alpha` goes to zero (optically thin nodes: large `D`, small `Sigma_r`, or a fine mesh), with no series fallback |
| `makesigmadfxyz` | in half-index mode the `iz` loop bound is `maxiz` where the other two axes use `m*max...`, so the upper half of the core silently gets no cross sections. Latent — every call site passes mode 1 |
| `makegradDxyz` | a **fuelled** node outside its line's `[low, high]` bounds is skipped by the `z` pass, keeps the pre-filled identity `1`, then has `y` and `x` accumulated on top — a spurious `+1` on its diagonal. Reachable via `geometry_ends3d`'s first-contiguous-run limitation. Confirmed by test |
| `calc_sanodalxyz` | same root cause, opposite symptom: the `y`/`x` passes accumulate into a diagonal slot the `z` pass was supposed to create, so a node `z` missed **aborts** instead of computing something wrong. Confirmed by test |
| `sanodaldiffusion_solverxyz` | a nodal-update interval of **1 does not converge** — it runs to the 5000-iteration cap and diverges at every mesh size tried. The built-in default `ceil((nx+ny+nz)/10)` **is** 1 whenever the extents sum to 10 or less, so a small mesh hits it without the caller doing anything unusual. Confirmed by test |
| `diffusion_solverxyz` | the empty-grid compaction is dead code — both it and its inverse are guarded by `keychange == 1` where `keychange` is the literal `0` four lines above. Four `writematrix` CSV dumps also run unconditionally on every call |
| both flux solvers | a bailed-out iteration returns the **previous** pass's `k_eff` and residuals and gives no indication it bailed. The translation adds a `Termination` value rather than reproducing the silence |
| `w3chf` | **the upwind enthalpy is halved** — `(0.5*h_i + 0.5*h_{i-1})/2` is a two-point average with a stray extra `/2`. It raises `Kfour` and so **overpredicts** the critical heat flux by a measured **22.8%** at PWR conditions: a large, systematic, *non-conservative* error in the one quantity meant to bound a safety margin. Confirmed by test |
| `w3chf` | uses a per-node local enthalpy where published W-3 uses the constant **inlet** enthalpy. Possibly a deliberate variant, possibly the same unfinished edit as the halving — the snapshot does not say |
| `makeheatlaplacian_1dcylnd` | **dead code**: its only call site is a commented-out line in `th_solverxyz.m`. The live path, `fuelrodheat_1dcylnd`, assembles the same operator inline with a *different* interface-conductivity formula, so the snapshot ships two divergent discretisations and the unreachable one is the more readable. It also writes outside its declared sparse shape at the last interior node |

## Porting status

| Layer | Files | Done |
|---|---|---|
| Utilities and indexing | 12 | 12 |
| Nodal diffusion (SANM) | 14 | **14 — complete.** `makegradDxyz`, `calc_sanodalxyz`, `sigmavalupd3d`, `calc_ABEFGHxyz`, `calc_bucklingxyz`, `makesigmadfxyz`, `fiss_src_extrapolatexyz`, the full leakage trio `calc_transleakagexyz` / `calc_1sttransleakagexyz` / `calc_2ndtransleakagexyz`, `calc_a1_expansionxyz` and its driver `calc_a1234_expansionxyz`, and the two flux solvers `diffusion_solverxyz` and `sanodaldiffusion_solverxyz` |
| Thermal hydraulics | 9 | **9 — complete.** `w3chf`, `w3chfhottest`, `fuelrodheat_1dcylnd`, `fuelrodheattime_1dcylnd`, `singleflow1devap`, `singleflow1devaptime`, `makeheatlaplacian_1dcylnd` (dead in the reference), and the `th_solverxyz` / `th_solvertimexyz` drivers |
| Coupling and cross-section feedback | 6 | **6 — complete.** Plus `criticalboron_xyz` (the critical-boron search). `sigmavalupd3d`, `sigmavalupd3d_handler`, `driftflux6_solverstatic3d`, and both drivers: `thdiffusion_solverxyz` (steady) and `thdiffusion_solvertimexyz` (transient) |
| Benchmark cases and drivers | 10 | **10 — complete.** Six cases (`iaea3ds`, `neacrpd1`, `neacrpd1t`, `neacrpa2`, `neacrpa2t`, `neacrpa1t`), the legacy 2-D `geom2dxycase1` (no solver can run it), `plotreactor3dcolour`'s data half, and the two scripts as `examples/` |
| `IAPWS_IF97.m` (3361 lines, 107 subfunctions) | 1 | regions 1, 2 and 4 with the backward `T(p,h)` and transport entry points. **Region 3 is not translated**, which caps everything at 16.5292 MPa |

### One file the snapshot does not contain

`driftflux6_solverstatic1d.m` — the 1-D kernel the default two-fluid
thermal-hydraulic path calls — is **absent from the snapshot**. The reference
wraps the call in `try`/`catch`, so MATLAB's "Undefined function" is swallowed
and every powered channel silently fails and keeps its previous state. That
behaviour is reproduced faithfully and surfaced as
`ChannelOutcome::SolverMissing` rather than hidden.

Its practical effect is visible in the NEACRP results above: on the default
path the coolant never leaves its inlet temperature. The `hem` model, which
routes to `singleflow1devap` instead, is the working path.

### The two flux solvers

`sanodaldiffusion_solverxyz` is the one the benchmark drivers call;
`diffusion_solverxyz` is the plain finite-difference baseline it is judged
against. They share the `gradD` operator and almost nothing else — the operator
split, the normalisation, the acceleration and the iteration caps all differ —
so they are translated as two modules and deliberately not factored together.

| | `diffusion_solverxyz` | `sanodaldiffusion_solverxyz` |
|---|---|---|
| Left-hand side | `gradD + tot - sd` | `gradD + nodal + tot - s` |
| Right-hand side | `fs/k + (s - sd) phi` | `fs/k` |
| Scattering | within-group implicit, rest lagged | fully implicit |
| Iteration cap | 10000 | 5000 |
| Acceleration | none | fission-source extrapolation every `fsexp` |
| Flux state | one vector | five-generation history |
| Warm start | no | yes, `varargin{2}` |
| Refactorisation | once | every `nodalupd` iterations |

#### What was measured — verification, not validation

On a uniform leaking cube (one group, 10 cm nodes, `Sigma_tot = 0.5`,
`Sigma_s = 0.4`, `Sigma_f = 0.1`, `nu = 2.5`, vacuum on all six faces, so
`k_inf = 2.5`), measured 2026-08-13 in release mode:

| Mesh | Finite difference | SANM (`nodalupd` 3) | Difference |
|---|---|---|---|
| 3x3x3 | 2.13823592 | 2.12831001 | -464 pcm |
| 4x4x4 | 2.26638105 | 2.25960501 | -299 pcm |
| 5x5x5 | 2.33888182 | 2.33508992 | -162 pcm |

Both converge to a positive, centre-peaked fundamental mode below `k_inf`, and
the nodal correction lowers the eigenvalue in the same direction and of the same
order as the "-103 pcm" the defect register records for a 3-cube.

**This is a self-consistency check on a hand-made problem, not a benchmark.** No
published `k_eff` is involved. The benchmark comparison is the next section.

#### What was measured — validation, IAEA-3D

The first published-benchmark comparison in this crate. `crate::iaea3ds` builds
the IAEA 3-D PWR benchmark — 17x17x19 quarter core, two groups, five materials,
10 cm radial / 20 cm axial mesh, reflective on the low `x` and `y` faces — and
hands it to the SANM nodal solver with no thermal-hydraulic feedback and no
coupling. Measured 2026-08-18 in release mode:

| | `k_eff` | difference |
|---|---|---|
| **this port, SANM nodal** | **1.029084** | — |
| PARCS | 1.029096 | **-1.1 pcm** |
| ADPRES | 1.029082 | **+0.2 pcm** |

Converged in 256 source iterations over 42 nodal rebuilds; fission-source
residual 9.611e-7, `k_eff` residual 9.272e-10. The converged flux has zero
negative entries in 10 982 and peaks at node `(2, 3, 8)` — inside the fuelled
region, below mid-height, which is the direction the rods in levels 15-18 push
it.

The agreement is closer than the two reference codes are to each other (they
differ by 1.4 pcm). Read `src/data/PROVENANCE.md` before citing it: both
reference values are quoted from `iaea3ds.m`'s own header, not from a primary
publication checked in this repository.

**What this establishes, and what it does not.** It validates the
nodal-diffusion stack — cross-section expansion, diffusion coefficients, the
gradient operator, the SANM correction, transverse leakage, and the eigenvalue
iteration — against a published reactor. It says nothing about the
thermal-hydraulics, the coupling, or the transient path, none of which this case
exercises, and it does not compare the benchmark's published assembly powers.
The coupled driver in particular is **not** shown to converge; see
`crate::thdiffusion_solverxyz`'s "Verification status".

#### Two deliberate departures from the reference

Everything else in the crate reproduces the MATLAB exactly. These two do not,
and both are recorded in `docs/bedok-reference-defects.md`:

1. **The diagnostic CSV dumps are returned, not written.** The reference calls
   `writematrix` fourteen times across the two solvers — four of them
   unconditionally, on every call. The same quantities are computed and handed
   back in a `Diagnostics` struct instead. A library that writes files as a side
   effect cannot be called concurrently or tested cleanly, and the physics is
   identical either way.
2. **The `gmres`/`ilu` branch is not translated.** It is selected at
   `philenf >= 50000000` — fifty million unknowns — which is unreachable for any
   runnable problem, so an ILU and a restarted GMRES written for it could never
   be verified against the reference. Both solvers return
   `BedokError::IterativeSolveNotTranslated`, which names the threshold and
   cannot be mistaken for the direct path having run.

A third difference is an addition: both return a `Termination` value saying why
the iteration stopped, which the reference has no equivalent of. A bailed-out
run in the MATLAB is indistinguishable from a converged one.

### Verified against the IAPWS-IF97 standard

Both translated regions are checked against the published verification values
(Table 5 for region 1, Table 15 for region 2), measured 2026-08-12:

| Region | States | Worst relative deviation |
|---|---|---|
| 1 — compressed liquid | 3 MPa/300 K, 80 MPa/300 K, 3 MPa/500 K | 2.810e-9 |
| 2 — superheated vapour | 0.0035 MPa/300 K, 0.0035 MPa/700 K, 30 MPa/700 K | 1.841e-9 |
| 4 — saturation pressure (Table 35) | 300 K, 500 K, 600 K | 1.752e-9 |
| 4 — saturation temperature (Table 36) | 0.1 MPa, 1 MPa, 10 MPa | 1.043e-9 |

Region 4's two directions are also checked against each other:
`Tsat_p(psat_T(T))` returns to within **4.263e-15** relative across the whole
saturation line — machine precision, and five orders tighter than the agreement
with the printed tables, which localises that ~1e-9 residual to the tables' own
rounding rather than to either expression.

The saturated-enthalpy chain `hL_p -> Tsat_p -> h1_pT` puts the normal boiling
point at **373.1243 K (99.974 °C)** and the latent heat of vaporisation at
**2256.54 kJ/kg** — both textbook, and a check on a state none of the published
tables above touches.

**One gap is load-bearing.** `hL_p` and `hV_p` cover the saturation line only
*below* the region 1/3 boundary at 16.5292 MPa; above it the saturated liquid is
a region-3 state and region 3 is not translated, so they return `NaN` rather
than a wrong number. Both BEDOK operating points — a PWR at 15.5 MPa and a BWR
at 7 MPa — sit below the boundary.

The published tables carry 9 significant figures, so this is agreement at the
reference's own precision. Region 2's mixed derivative is additionally
cross-checked against a finite difference of its `pi` derivative, which catches
a mistyped coefficient in either of the two independently transcribed 43-term
sums.

### IAPWS_IF97.m — licence cleared, port started

This is the one module that is **not** Than Yan Ren's code. It translates a
third-party MATLAB implementation the snapshot vendored in:

| | |
|---|---|
| Upstream | <https://github.com/mikofski/IAPWS_IF97> |
| Copyright | Copyright (c) 2013, Mark Mikofski |
| Licence | BSD-2-Clause — **GPL-3.0-compatible** |
| Terms | reproduced in full in the crate `NOTICE`, as that licence requires |

The vendored copy misspells the author as "Mifofski"; the upstream repository
and its `license.txt` both give **Mikofski**, used here as correct.

Region 1 is translated. Regions 2-4, the backward equations, the region
boundaries, viscosity/conductivity and the basic property functions are not.
The BEDOK thermal hydraulics reaches this almost entirely through the `_ph`
entry points — `T_ph`, `v_ph`, `cp_ph`, `x_ph` — so those and their dependency
chains are the critical path.

## Build and test

Per the workspace rule, release mode:

```bash
cargo test --release -p bedok --lib
```

**Status: builds clean under clippy and rustdoc; 247 unit tests pass, 4
ignored** — release profile, measured 2026-08-18. The ignored tests are
`thdiffusion_solverxyz`'s three, which depend on a synthetic fixture that is not
a well-posed coupled problem (see that module's "Verification status"), and
`criticalboron_xyz`'s case-A1 search, which is a ten-minute diagnostic for the
open X1 discrepancy rather than a gate.

The suite takes about 21 minutes. Most of that is three benchmark cases that
each run a full coupled solve, so prefer a filter while iterating.

On Windows use the **MSVC** toolchain:

```bash
rustup default stable-x86_64-pc-windows-msvc
```

It needs Visual Studio Build Tools with **both** the C++ workload *and* the
Windows SDK — the SDK is a separate component, and its absence shows up
misleadingly as `linker 'link.exe' not found`, which reads like a missing
compiler rather than a missing SDK.

The GNU toolchain (`stable-x86_64-pc-windows-gnu`) is **not** a workaround: it
cannot build `faer`, which needs `dlltool` from MinGW binutils.

**What passing tests do and do not mean.** They cover the translated utility
layer and pin the known reference defects; the IAPWS region-1 functions agree
with the published verification values to ~3e-9 relative. Nothing has been run
against a reactor benchmark, because the solvers are not translated yet. Per
`RESPONSIBLE_USE.md` this is AI-assisted draft material pending human review.

## Provenance

| | |
|---|---|
### Permission and attribution

**Permission to translate has been given.** Recorded here because
`RESEARCH_INTEGRITY_AND_PROVENANCE.md` requires the provenance of ported work to
be documented rather than assumed. This section is the canonical record; every
ported module's doc comment points at it.

| | |
|---|---|
| **Author** | Than Yan Ren, fellow researcher, SNRSI |
| **How it arose** | Yan Ren approached the maintainer; the translation is something he wanted |
| **Permission to translate** | given by Yan Ren directly, by email |
| **Scope of that permission** | **explicitly open-source, under OUTRAM PARK** — stated up front, not inferred |
| **Institutional approval** | given by SiCong (project lead), explicitly for sharing in the open-source repository |
| **Source** | the `main_exec_diff3d_standalone` snapshot |
| **Licence** | GPL-3.0-only |
| **Recorded** | 2026-08-05 |

The scope question that matters for a copyleft repository — translation for
internal use versus publication as open source — was settled before the fact:
the open-source destination was stated explicitly when permission was sought,
and approved at both the author and project-lead level. **No further clearance
is outstanding.** Retain the email as the record.

**Attribution.** Every ported file carries a header naming **Than Yan Ren
(SNRSI)** as the original author, the original `.m` filename, and the snapshot
the translation was taken from.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.
