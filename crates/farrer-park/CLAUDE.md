# CLAUDE.md — `farrer-park`

Crate-specific guidance. The workspace-root `CLAUDE.md` still binds; this file
adds to it and never relaxes it.

## What this crate is

FEM structural mechanics: small-strain linear elasticity and J2 plasticity on
unstructured meshes. Ported from MOOSE, PRISMS-Plasticity and PRISMS-Fatigue
(all LGPL-2.1 — see `NOTICE`). GitHub issue #175, beads epic `op-vrtt`.

## The one rule that defines this crate

**Farrer Park is a genuine finite-element code and must stay one.** It depends
on `outram-foam-basic-lib` for *shared numerical infrastructure only* — Krylov
solvers, preconditioners, multigrid — and never for its spatial discretisation.

Do not "simplify" by reusing the finite-volume discretisation:

- `FvMesh`, `fvPatchField`, `fvc`/`fvm` operators and face-addressed assembly
  are **not** to appear in this crate.
- `LduMatrix` stays on the FV side. This crate's `CsrMatrix` is the FEM
  representation.
- The single point of contact is the `LinearOperator` contract. If you find
  yourself widening that contract with something face-addressed, stop — the
  abstraction is leaking and the answer is a different contract, not an FV
  concept in a FEM crate.

Reformulating structural mechanics as finite volume because the linear algebra
happens to be shared is the specific failure mode issue #175 exists to prevent.

## Additions to the workspace design rules

- Element types, constitutive laws and boundary-condition kinds are **closed
  sets** — model them as enums with exhaustive `match`, never `Box<dyn>`.
  `LinearOperator` is a generic bound, never a trait object.
- Topology is referenced by index newtypes (`NodeId`, `ElemId`, `DofId`), not
  by borrowed references — this is how the no-lifetime-parameters rule is
  satisfied here.
- The public API is `uom`-typed; assembly and the linear solve run on bare
  `f64` in **SI base units** for speed. Convert at the boundary, and state
  units in words in every doc comment even where `uom` enforces them.
- Any file porting upstream logic carries the attribution header block
  (project, source file, version/commit, copyright, licence). Do not strip it
  in refactors. A module implementing a documented method from the literature
  rather than from upstream source should cite the reference instead.

## Debugging a port

The workspace "read upstream first" hard rule applies with full force. When a
result is wrong, read the MOOSE or PRISMS routine that owns the behaviour
*before* reasoning from first principles or from this crate's own comments. In
a translation, the overwhelmingly likely cause is that upstream does something
the port does not — most often a bound, a guard, or a branch, not a wrong
formula. Record what you find in the bead, including when upstream turns out to
do exactly what we do.

## Maturity

**NOT DECLARED MATURE.** The dogfooding/API-usability rule does not yet apply
to this crate, and must not be enforced on it.

Proposed bar, for the maintainer to accept, amend or reject — an agent may
propose maturity with cited runs and numbers but must **not** declare it:

- 2026-09-11 (morning) — proposed bar: observed L2 convergence order within
  0.15 of theory under MMS for every implemented element; patch test satisfied
  to 1e-12 relative; thick-walled cylinder within 1% of the closed-form Lamé
  solution on a converged mesh. Evidence class: **analytical / MMS**.
  **SUPERSEDED THE SAME DAY — the cylinder clause was wrong.** Kept here
  because the rule says to keep superseded bars, and because the mistake is
  instructive.

- 2026-09-11 (after first measurement) — proposed bar, corrected. The "within
  1% of Lamé" clause above silently assumed displacement and stress converge
  alike. They do not: stress is a first derivative of the primary unknown, so a
  bilinear element gives stress one order lower than displacement. Measured on
  the quarter-annulus, refining 4x8 → 32x64:

    max rel. error in u_r        1.28e-2 → 2.07e-4, observed order 1.998
    max rel. error in sigma_r    1.63e-1 → 2.45e-2, observed order 0.959

  The displacement result beats the 1% clause by a factor of 50. The stress
  result misses it by 2.5x — not because anything is wrong, but because 1% was
  not an achievable number for Q4 stress at that refinement, and a bar nobody
  can meet gets quietly ignored rather than met.

  Corrected bar: **displacement** within 1% of Lamé on a converged mesh, and
  **stress** judged on observed convergence order (1 ± 0.2 for linear elements,
  2 ± 0.2 for quadratic) rather than on an absolute tolerance. Asserting the
  order theory predicts is the sharper test anyway — it fails if the physics is
  wrong even when the absolute error happens to look small. MMS and patch-test
  clauses unchanged; both are met with margin. Evidence class:
  **analytical / MMS**.

Keep superseded entries when the bar moves, so a later reader can tell that it
moved rather than misreading old results as failures against a standard that
did not exist when they were produced.

## V&V

Verification results live in `docs/verification.md` and in each test's `///`
doc comment, and must record **methodology and measured results** — a test
documenting only what it does is incomplete.

Nothing here is validated. There is no human V&V sign-off and no reproduction
of the PRISMS-Fatigue published case studies. Do not describe any output of
this crate as validated, and do not frame it as authoritative for reactor
pressure vessel or piping life assessment, licensing, or any safety-related
decision.

## Testing

Release mode only: `cargo test --release -p farrer-park`. Keep the suite fast
enough to run on every change — if a verification case needs a large mesh, gate
the expensive refinement level behind `#[ignore]` and say so in its doc comment,
rather than letting the default suite become too slow to run.
