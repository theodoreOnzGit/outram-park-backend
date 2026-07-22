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

### D4 — `krylov` module in `outram-foam-basic-lib` (fleet-built)

- **GMRES uses RIGHT preconditioning** (solves `A M^-1 u = b`, `x = M^-1 u`) so
  the Givens residual estimate equals the true residual; it also recomputes the
  true residual each restart as the authoritative stopping test.
- **ILU(0) is a genuine incomplete-LU** (IKJ elimination on a CSR view built
  from the LDU coefficients, restricted to A's sparsity), exact for tridiagonal
  matrices — *not* a Jacobi fallback. Only safeguard is a small-pivot floor
  (`1e-300`, sign-preserving).
- Verification only: solvers cross-checked against dense `SquareMatrix` LU
  (rel err < 1e-6) and analytic tridiagonal LU. No physical validation.
- **REVIEW:** the crate's own `CLAUDE.md` porting workflow asks for a README
  "Ported items" row and a `cargo test --doc` run — done in the bookkeeping
  step; confirm the `krylov` module belongs at Layer 1b of that crate.

### D5 — Grid conventions

- Cell ordering **x-fastest**: `index = i + nx*(j + ny*k)`. Connection/LDU face
  order is the same traversal (each cell emits +x, +y, +z faces), so LDU face
  `f` ↔ `connections()[f]`, and `owner < neighbour` holds by construction.
- `cell_index`/`cell_ijk` use `debug_assert!` bounds (no release-mode cost) —
  out-of-range in release is UB-adjacent (panics downstream), a deliberate
  hot-path choice. **REVIEW:** confirm acceptable.

### D6 — Property models and their known singularities

- **Model pairing:** van Genuchten retention ↔ **Mualem** relative permeability;
  Brooks–Corey retention ↔ **Burdine** relative permeability. These are the
  classical self-consistent pairings and match PFLOTRAN defaults. VG+Burdine is
  not offered in v1.
- **EOS:** slightly-compressible exponential `rho(p) = rho_ref*exp(c*(p-p_ref))`
  (strictly positive, derivative exactly `c*rho` for a consistent Jacobian);
  constant viscosity.
- **Known non-smoothness a Newton solver must handle:** VG–Mualem
  `dkr/dSe -> +inf` as `Se -> 1` (returned as `f64::INFINITY`, intrinsic to the
  model); Brooks–Corey `dSe/dpc` is discontinuous at the air-entry pressure
  `pc = 1/alpha`. **REVIEW:** these are physical properties of the models, but
  the RICHARDS Jacobian/line-search must be robust to them (see D8).

### D7 — Input-deck format is an AI-designed subset (NOT real PFLOTRAN syntax)

- The `io` card grammar (`GRID`/`MATERIAL`/`CHARACTERISTIC_CURVES`/
  `BOUNDARY_CONDITION`/`TIME` blocks, `#`/`!` comments, `END`/`/` terminators)
  is an AI-invented minimal subset. It is **not** compatible with genuine
  PFLOTRAN input decks and makes no fidelity claim.
- **REVIEW (high priority):** a human must decide whether to (a) keep this
  lite format for v1 verification and document it as non-PFLOTRAN, or (b)
  replace it with a real PFLOTRAN input-deck parser. Flagged in `io/mod.rs`.

### D8 — Newton solver strategy

- Backtracking **Armijo** line search (`||F(x+λdx)|| <= (1 - 1e-4 λ)||F(x)||`,
  λ halved from 1 up to `max_backtracks`); if none pass, take the least-residual
  trial λ to guarantee progress.
- **Inexact Newton:** a non-converged inner Krylov solve is tolerated (its
  `converged` flag is ignored and the direction still used); only a non-finite
  `dx` is fatal. **REVIEW:** whether to surface inner-solver non-convergence in
  `NewtonReport` (currently not a field) is a maintainer decision.
