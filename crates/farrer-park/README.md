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
infrastructure only** — Krylov solvers and preconditioners. It does **not**
borrow its spatial discretisation. Structural mechanics stays a genuine
finite-element formulation: nodal degrees of freedom, shape functions,
quadrature and weak-form assembly.

The two discretisations meet at one contract, `LinearOperator`:

- `LduMatrix` (in `outram-foam-basic-lib`) stays the face-addressed,
  FVM-optimised representation.
- `CsrMatrix` (here) is the FEM-appropriate sparse representation.

Both implement `LinearOperator`, so both reuse the same Krylov layer without
either being forced into the other's matrix layout. The generic drivers are
`cg_op`, `gmres_op` and `bicgstab_op` in
`outram_foam_basic_lib::linear_operator`; they are **new** functions and the
existing finite-volume entry points are untouched.

Algebraic multigrid (`gamg`) is **not** part of the shared contract: it needs
the coarsening structure of an `LduMatrix` rather than only a matrix-vector
product, so it remains finite-volume specific. Farrer Park preconditions with
ILU(0) on its own CSR pattern instead.

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
| B-bar (mean dilatation) for volumetric locking, Quad4/Hex8 | implemented (opt-in; full integration stays the default) |
| Plane stress, elastic and J2 (condensed `eps_zz`) | implemented |
| Shear-locking treatment (incompatible modes / enhanced strain) | **not started** — measured at 11-67 % too stiff, see below |
| Selective reduced integration, mixed u-p, F-bar | **not started** |
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
- **Volumetric locking** — nearly incompressible MMS at `nu = 0.499` on Quad4
  and Hex8, full integration against B-bar.
- **Fully plastic limit load** — thick cylinder collapse against
  `p_L = (2/sqrt(3)) sigma_y ln(b/a)`.
- **Shear locking** — Quad4 cantilever, quantified, and shown **not** to be
  cured by B-bar.
- **Plane stress** — thin plate in tension, the exact plane-stress/plane-strain
  equivalence, and J2 in uniaxial and equibiaxial tension.

Headline numbers, measured 2026-09-11:

| Check | Result |
|---|---|
| Patch test, all five elements | machine precision (worst 1.3e-14 relative in stress) |
| MMS observed order, Quad4 / Tri3 | L2 1.998 / 1.997, H1 1.000 / 0.998 (theory 2, 1) |
| MMS observed order, Tri6 | L2 2.978, H1 1.989 (theory 3, 2) |
| MMS observed order, Hex8 / Tet4 | L2 1.983 / 1.957, H1 1.008 / 0.969 (theory 2, 1) |
| Thick-walled cylinder | 2.45 % max stress error (first order), 0.021 % in u_r (second order) |
| Cantilever, excess over Euler-Bernoulli | 3.677 / 0.928 / 0.233 % at L/H = 4 / 8 / 16, against a 3.750 / 0.938 / 0.234 % shear-deformation prediction |
| Uniaxial J2 vs closed form | exact to round-off (1.95e-14), including elastic unloading and reverse yield |
| Consistent tangent vs central difference | 1.055e-7 relative, worst entry |
| Newton convergence order, partially plastic step | **2.004** |
| Volumetric locking, Quad4 MMS at `nu = 0.499` | full integration order 0.69-1.24; **B-bar 2.004**; 16.96x smaller error |
| Volumetric locking, Hex8 MMS at `nu = 0.499` | full integration order **0.634**; **B-bar 2.050**; 6.36x smaller error |
| Plastic collapse vs closed-form limit load | full integration +23.9 % to +0.42 %; **B-bar +1.06 % to +0.017 %** |
| Shear locking, Quad4 cantilever | 11.25 % too stiff at 2 elements through the depth, 66.7 % at aspect ratio 4; **B-bar does not cure it** |
| Plane stress, thin plate and the exact plane-strain equivalence | round-off (4.3e-16 and 2.9e-16) |
| Plane-stress J2, uniaxial / equibiaxial vs closed form | exact to round-off (3.58e-16 / 6.77e-16) |

These are **verification** ("is it implemented correctly?"), not validation
("does it represent physical reality well enough?"). No benchmark validation is
claimed.

## Portability

Pure Rust with no BLAS, no C or Fortran toolchain and no GUI. The library
compiles for `wasm32-unknown-unknown` and for `aarch64-linux-android`
(`cargo check -p farrer-park --all-targets --target aarch64-linux-android`, and
`scripts/check-wasm.sh`). As the workspace `CLAUDE.md` warns, **compiling is not
running**: neither target has been executed, and no claim is made that it works
in a browser or on a device.

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
