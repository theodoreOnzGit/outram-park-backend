# AI-directed decisions — `outram-park-fork-pflotran`

> **Status: awaiting human review.** This document records engineering
> decisions made **autonomously by an AI assistant** (Claude) while building the
> v1 vertical slice of the PFLOTRAN Rust fork, under an explicit
> "make-the-best-decisions-you-can and flag them" directive from the maintainer
> (2026-07-22). Every entry here is an AI judgement call that a human has **not
> yet** reviewed or approved. Per the workspace `RESPONSIBLE_USE.md`, all of
> this is **untrusted draft material** until a maintainer signs off (see the
> crate README "Bookkeeping status" block — both axes remain ❌).
>
> Nothing here has been validated against published PFLOTRAN reference results.
> Verification tests (MMS, analytical) check *internal correctness only*.

## How to use this document

Each decision has: **what** was decided, **why**, **alternatives considered**,
and an **open question / review ask** for the human. Reviewers: search for
`REVIEW:` to find the explicit asks.

---

## D1 — Reuse `outram-foam-basic-lib` linear algebra; add a pure-Rust `krylov` module rather than translating OpenBLAS

- **Decision.** The Newton–Krylov linear backbone reuses
  `outram-foam-basic-lib`'s face-addressed sparse `LduMatrix` (its `multiply`
  is the SpMV) and dense `SquareMatrix` LU. The pieces foam-basic-lib lacks for
  an **asymmetric** Richards Jacobian — BiCGStab, GMRES(m), ILU(0) and Jacobi
  preconditioners, and BLAS-1 vector helpers — are added as a **new pure-Rust
  `krylov` module inside foam-basic-lib**, then imported here.
- **Why.** The maintainer directed "use outram-foam basic libs for matrix
  algebra … otherwise translate OpenBLAS … only if needed", and later "upgrade
  outram-foam basic libs some capabilities from OpenBLAS, as a separate module,
  then import them in pflotran". foam-basic-lib already ships SPD solvers
  (DIC-PCG, GAMG) but only Gauss–Seidel for asymmetric systems, which is too
  weak as a Newton inner solve. A first-principles pure-Rust Krylov module is
  the minimal capability gap, and keeps everything Android-clean (no system
  BLAS, satisfying the workspace Android rule).
- **Alternatives.** (a) Translate OpenBLAS/LAPACK wholesale — rejected as
  vastly out of scope and Android-hostile. (b) Use only Gauss–Seidel —
  rejected, poor convergence for advection-dominated / anisotropic systems.
- **REVIEW:** confirm the new capability belongs in `outram-foam-basic-lib`
  (making it a solver dependency of pflotran) rather than living inside the
  pflotran crate. Confirm the `krylov` public API shape.

## D2 — wgpu GPU kernels deferred (CPU pure-Rust is the trusted baseline)

- **Decision.** No wgpu dependency is added in this pass. The CPU pure-Rust
  Krylov/SpMV path is implemented as the trusted, deterministic baseline; a
  documented seam is left so a future Android-gated wgpu SpMV/precond kernel can
  slot in without an API change.
- **Why.** The maintainer said "use wgpu kernels with android compatibility …
  only if needed". A scaffold with no working solve yet has no profiled hot
  kernel to accelerate; adding a GPU backend before the CPU path is verified
  would invert the "CPU is the trusted path, GPU is acceleration only" rule the
  workspace already applies in `outram-blender`.
- **REVIEW:** approve deferring wgpu to a dedicated follow-up (a bead should be
  filed) once the CPU RICHARDS solve is verified and profiled.

## D3 — v1 scope (unchanged from bead op-v6s.2, restated for the build)

- RICHARDS (variably-saturated single-phase) · structured Cartesian FV ·
  two-point flux · serial pure-Rust Newton–Krylov · minimal card-deck I/O +
  CSV/VTK. **Out of v1:** unstructured grids, MPI, HDF5, multiphase (GENERAL),
  energy (TH), solute transport, geochemistry (GIRT).

---

## Verification & validation posture (important — read before trusting any number)

- **Verification (implemented correctly?)** is done with the **Method of
  Manufactured Solutions (MMS)** and closed-form analytical cases (steady 1D
  saturated flow). These check that the discretisation converges at its design
  order and reproduces exact solutions — internal correctness only.
- **Validation (matches physical reality / published reference?)** is **NOT**
  done. The transient Richards cases (e.g. a Celia-1990-style infiltration) are
  currently **regression** tests: they record the *current* output as a frozen
  baseline so future changes are caught, but that baseline has **not** been
  compared to published PFLOTRAN gold-files or experimental data. Do not cite
  any of these numbers as validated (bead op-v6s.9).
- No result in this crate's tests was hand-written; every recorded number is
  produced by actually running the code. Where a reference value is used, its
  source is cited in the test doc comment.

---

## Decision log (appended as work proceeds)

<!-- New decisions are appended below with the next Dn index. -->
