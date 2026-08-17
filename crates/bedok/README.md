# BEDOK

3-D nodal-diffusion neutronics coupled to thermal hydraulics — the fidelity
band **above 1-D neutronics and below CFD**.

A Rust translation of Than Yan Ren's (SNRSI) MATLAB implementation, ported from
the `main_exec_diff3d_standalone` snapshot with the author's permission.

> **Status: rewrite in progress.** 27 of 48 MATLAB files are translated. The
> nodal-diffusion layer is complete and runs end to end; the thermal-hydraulics
> layer has started. The crate builds clean under clippy and rustdoc and its 135
> unit tests pass. **No benchmark comparison has been made** — the case files
> that would supply one are not translated yet. See
> [Porting status](#porting-status).

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

The reasoning is in `docs/bedok-port-scoping.md` §1.0: a translation carrying
well-meant fixes cannot be debugged against a benchmark, because a disagreement
can no longer be attributed to either the translation or the original.

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
| Thermal hydraulics | 9 | 1 — `w3chf` (W-3 critical heat flux) |
| Coupling and cross-section feedback | 5 | 0 |
| Benchmark cases and drivers | 7 | 0 |
| `IAPWS_IF97.m` (3361 lines, 107 subfunctions) | 1 | regions 1, 2 and 4, plus the enthalpy entry points `h1_pT` / `h2_pT` / `hL_p` / `hV_p` |

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
published `k_eff` is involved, because the case files are not translated yet.
Nothing in this crate should be described as validated.

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

**Status: builds clean under clippy, 100 of 100 unit tests pass** — rustc 1.97.1,
release profile, measured 2026-08-13.

On Windows the host toolchain must be `stable-x86_64-pc-windows-gnu` unless
Visual Studio Build Tools with the C++ workload are installed; the MSVC target
fails at link time with `linker 'link.exe' not found`. Prefix with
`+stable-x86_64-pc-windows-gnu` or set it as the default.

**What passing tests do and do not mean.** They cover the translated utility
layer and pin the known reference defects; the IAPWS region-1 functions agree
with the published verification values to ~3e-9 relative. Nothing has been run
against a reactor benchmark, because the solvers are not translated yet. Per
`RESPONSIBLE_USE.md` this is AI-assisted draft material pending human review.

## Provenance

| | |
|---|---|
| Original author | Than Yan Ren, Singapore Nuclear Research and Safety Institute (SNRSI) |
| Source | `main_exec_diff3d_standalone` snapshot |
| Permission | given by the author for open-source release under OUTRAM PARK |
| Institutional approval | given by the project lead, for the open-source repository |
| Licence | GPL-3.0-only |

Recorded in full in `docs/bedok-port-scoping.md` §6.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.
