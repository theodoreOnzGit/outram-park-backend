# FARRER PARK

**F**inite-element **A**nalysis for **R**eactor **R**eliability, **E**ngineering
**R**esponse, **P**lasticity **A**nd **R**isk.

FEM structural mechanics for the OUTRAM PARK suite: small-strain elasticity and
J2 plasticity on unstructured meshes, with the linear algebra supplied by
`outram-foam-basic-lib`.

> **Not a validated life-assessment tool.** This is an evolving FEM/materials
> research capability. It is verified against analytical and manufactured
> solutions only, has no human V&V sign-off, and must not be used for reactor
> pressure vessel or piping life assessment, licensing, or safety-related
> decisions. See `RESPONSIBLE_USE.md` at the workspace root.

## Why this is FEM and not finite volume

Farrer Park depends on `outram-foam-basic-lib` for **shared numerical
infrastructure only** — Krylov solvers, preconditioners, multigrid. It does
**not** borrow its spatial discretisation. Structural mechanics stays a genuine
finite-element formulation: nodal degrees of freedom, shape functions,
quadrature and weak-form assembly.

The two discretisations meet at one contract, `LinearOperator`:

- `LduMatrix` (in `outram-foam-basic-lib`) stays the face-addressed,
  FVM-optimised representation.
- `CsrMatrix` (here) is the FEM-appropriate sparse representation.

Both implement `LinearOperator`, so both reuse the same Krylov layer without
either being forced into the other's matrix layout.

## What is implemented

| Area | Status |
|---|---|
| Lagrange elements — Tri3, Tri6, Quad4, Tet4, Hex8 | implemented |
| Gauss quadrature, exactness order recorded per rule | implemented |
| DoF numbering and CSR sparsity from the DoF graph | implemented |
| Weak-form assembly, linear elasticity | implemented |
| Dirichlet (strong + penalty) and Neumann/traction BCs | implemented |
| J2 plasticity, radial return, consistent tangent | implemented |
| Newton solution control with load stepping | implemented |
| Crystal plasticity (PRISMS-Plasticity) | **not started** |
| Microstructure-sensitive fatigue, FIPs (PRISMS-Fatigue) | **not started** |
| PRISMS-Fatigue published case-study parity | **not started** |

## Verification

Methodology and measured results are recorded in
[`docs/verification.md`](docs/verification.md) and in the doc comment of each
test. Summary of what is checked:

- **Patch test** — constant strain reproduced to machine precision.
- **MMS convergence** — observed order of accuracy matches theory per element.
- **Thick-walled cylinder** — against the closed-form Lamé solution.
- **Cantilever beam** — tip deflection against Euler-Bernoulli/Timoshenko.
- **Uniaxial J2 plasticity** — against the closed-form elastic-plastic response.

These are **verification** ("is it implemented correctly?"), not validation
("does it represent physical reality well enough?"). No benchmark validation is
claimed.

## Licensing

GPL-3.0-only. Ported from MOOSE, PRISMS-Plasticity and PRISMS-Fatigue, all
LGPL-2.1, relicensed under LGPL-2.1 §3. The flow is one-way — see
[`NOTICE`](NOTICE).

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.
