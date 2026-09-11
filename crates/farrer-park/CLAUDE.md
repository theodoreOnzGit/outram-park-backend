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

- 2026-09-11 — proposed bar: observed L2 convergence order within 0.15 of
  theory under MMS for every implemented element; patch test satisfied to 1e-12
  relative; thick-walled cylinder within 1% of the closed-form Lamé solution on
  a converged mesh. Evidence class: **analytical / MMS**.

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
