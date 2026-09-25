# CLAUDE.md — `farrer-park`

> **HARD RULE — draw reactor geometry for a human (2026-09-25).** Whenever you
> build or change a complex reactor geometry or mesh in this crate, render the
> ASSEMBLED geometry as images (PNG/JPG/SVG) — an axial slice, radial slices at
> the heights that matter, and zoomed slices at every nested level — commit
> them with the change and point the human at them before reporting results as
> more than tentative. Full rule: the workspace
> [`CLAUDE.md`](../../CLAUDE.md), "Reactor geometry is DRAWN for a human to
> check before it is trusted".

Crate-specific guidance. The workspace-root `CLAUDE.md` still binds; this file
adds to it and never relaxes it.

## What this crate is

FEM structural mechanics: small-strain linear elasticity, J2 plasticity and
rate-dependent crystal plasticity on unstructured meshes, plus a partial
microstructure-sensitive fatigue layer. Ported from MOOSE, PRISMS-Plasticity
and PRISMS-Fatigue (all LGPL-2.1 or LGPL-2.1-or-later — see `NOTICE` and
`docs/upstream-provenance.md`). GitHub issue #175, beads epic `op-vrtt`.

## The one rule that defines this crate

**Farrer Park is a genuine finite-element code and must stay one.** It depends
on `outram-foam-basic-lib` for *shared numerical infrastructure only* — the
Krylov solvers and preconditioners reached through
`outram_foam_basic_lib::linear_operator` — and never for its spatial
discretisation.

Algebraic multigrid (`gamg`) is **not** on that contract and is not shared:
coarsening needs the face addressing of an `LduMatrix`, not just a
matrix-vector product. This crate preconditions with ILU(0) on its own CSR
pattern instead. Generalising GAMG would mean adding a coarsening contract,
which is a design decision, not a mechanical port.

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
- **State units in words in every public doc comment, without exception.**
  Assembly, constitutive integration and the linear solve run on bare `f64` in
  **SI base units**, because a Krylov vector and a fourth-order tangent share
  no single `uom` type. `uom` typing is offered where a user actually types a
  physical number — the material constructors
  (`LinearElastic::from_quantities`, `J2LinearHardening::from_quantities`) and
  the named aliases `YoungsModulus`, `ShearModulus`, `BulkModulus`,
  `YieldStress`, `HardeningModulus`, `PoissonRatio`. Mesh coordinates,
  displacements and nodal forces are deliberately **not** `uom`-typed: they
  live in flat `Vec<f64>` buffers indexed by degree of freedom and handed
  straight to the solver, and wrapping each entry would cost that layout and
  buy nothing a doc comment does not give. If that line moves, move it
  explicitly here rather than by drift.
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

**DECLARED MATURE 2026-09-13 by the maintainer.** The dogfooding/API-usability
rule now applies to this crate — see "What maturity turns on" below, because it
is not a formality.

- 2026-09-13 — **mature.** Bar, as corrected on 2026-09-11 and met with margin:
  observed L2 convergence order within 0.15 of theory under MMS for every
  implemented element; patch test to 1e-12 relative; thick-walled cylinder
  **displacement** within 1% of the closed-form Lamé solution on a converged
  mesh, with **stress** judged on observed convergence order (1 ± 0.2 linear,
  2 ± 0.2 quadratic) rather than an absolute tolerance. Evidence class:
  **analytical / MMS**.

  Measured at declaration, on rustc 1.98.1, release, 101 tests green:

    MMS L2 / H1 order   Tri3 1.997/0.998, Quad4 1.998/1.000, Tri6 2.978/1.989,
                        Tet4 1.957/0.969, Hex8 1.983/1.008 (theory p+1 / p)
    patch test          9.0e-17 displacement, 1.3e-14 stress, every element
    Lamé                u_r 2.07e-4 at order 1.998; sigma_r order 0.959
    uniaxial J2         exact to 1.95e-14 through load, unload and reverse yield
    Newton order        2.004, preserved under B-bar
    crystal plasticity  analytic Schmid 1.5e-15, frame indifference 7.6e-16,
                        Richardson-extrapolated tangent 3.6e-9

  **What this bar does NOT claim.** It is verification against analytical and
  manufactured solutions only. No published benchmark, no physical measurement,
  and no human V&V sign-off — the `README.md` bookkeeping axes remain ❌ and are
  the maintainer's to clear separately. Maturity here means the numerics are
  demonstrably right, not that the physics has been shown to represent reality.
  Do not cite this entry as validation.

  Known gaps live alongside it and are not waived by the declaration: shear
  locking is measured and uncured (`op-uqqg`), ILU(0) stagnates on nearly
  incompressible systems (`op-ldaz`), no curved elements, small strain only so
  no lattice reorientation or backstress (`op-tau7`), and fatigue is partial
  (`op-q1zn`).

### What maturity turns on

Declaring maturity is not cosmetic. Two workspace rules begin to bind here:

- **The small-model API dogfood** ("if it is too complex for Haiku, it is a bad
  API"). Farrer Park has no Python wheel, so the literal wheel-only form does
  not apply as written; the Rust-caller half does. Its traits should carry
  `#[diagnostic::on_unimplemented]` naming the concrete types that implement
  them (workspace audit `op-wiep`), and a fresh-context reader should be able to
  assemble a case from the public API and its doc comments alone.
- **The bar is now a standing commitment**, not an aspiration. A change that
  degrades any number above is a regression to be reported, not a new baseline.

Superseded proposals, kept because the rule says to keep them and because the
mistake below is instructive:

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

## Element formulation and the two-dimensional idealisation

Both are **explicit enums carried on `System` through `SystemOptions`**, never a
bool and never a hidden global, and both default to the conservative option:

- `assembly::Formulation` — `FullIntegration` (default) or `BBar`.
- `material::PlaneCondition` — `PlaneStrain` (default) or `PlaneStress`.

Rules for anyone changing this area:

- **Do not change either default.** Full integration and plane strain are what
  every verification case before 2026-09-11 was run with, and a silently changed
  default is worse than the defect it fixes.
- **Do not add plain reduced integration.** It needs hourglass stabilisation,
  which must be verified in its own right; `quadrature.rs` documents that
  refusal deliberately and it stays.
- **B-bar with plane stress is rejected at construction, on purpose.** Plane
  stress has no volumetric constraint to relax — the out-of-plane strain is free
  — so B-bar has nothing to cure there, and the combination is unverified. Do
  not "enable" it without a verification case.
- **B-bar is a no-op on Tri3 and Tet4** (constant gradients), asserted by a
  test. It does not help them; the fix for a locking simplex is a different
  element.
- **Shear locking is NOT cured and must not be described as cured.** It is
  measured in verification case 8 (11.25 % too stiff at two square elements
  through the depth, 66.7 % at element aspect ratio 4) and B-bar recovers only
  the volumetric share of it.

## Crystal plasticity: what was reduced from upstream, and why (read before
extending it)

`src/crystal/` is ported from PRISMS-Plasticity's **rate-dependent** model
(`MaterialModels/RateDependentModel/calculatePlasticity.cc`, commit
`ffdf4eb6`). Upstream is a **finite-deformation** code; this is a **small
strain** one, because that is this crate's scope. The reduction is deliberate
and must not be quietly undone one piece at a time.

**Taken from upstream, and where:**

| Piece | Upstream location |
|---|---|
| slip tables and their **ordering** | `applications/crystalPlasticity/{fcc,bcc}/*/slip{Normals,Directions}.txt` |
| latent-hardening `q` values | `.../LatentHardeningRatio.txt` (1.0 coplanar, 1.4 latent) |
| `R S R^T` into sample axes | `RateDependentModel/calculatePlasticity.cc:216-220` |
| Rodrigues to rotation matrix | `rotationOperations.cc`, `odfpoint` |
| power-law slip increment | `.../calculatePlasticity.cc:693` |
| saturating hardening, `h_b = h0 (1 - s_b/s_sat)^A`, indexed by the **slipping** system | `.../calculatePlasticity.cc:645-657, 698` |
| **the clamp `s <- min(s, s_sat)`** | `.../calculatePlasticity.cc:700-708` |
| previously converged stress as the local Newton's starting guess | the file's own header comment |
| a line search on the local residual | `lnsrch`, called at `.../calculatePlasticity.cc:602` |

**The clamp is the one to know about.** Without it `1 - s/s_sat` goes negative
and `pow(negative, 2.25)` — upstream's own FCC exponent — is **NaN**, which
then propagates into the stress silently. It is a correctness guard, not a
cosmetic bound. `crystal::flow`'s unit test asserts both halves: that the
clamped path stays finite, and that the unclamped expression really is NaN.

**NOT taken, and the consequences:**

- **Finite-deformation kinematics** (`Fe`/`Fp`, exponential update of `Fp`,
  second Piola-Kirchhoff stress). Hence **no lattice reorientation and no
  texture evolution** — there is no plastic spin in a small-strain
  formulation to rotate the lattice with.
- **Kinematic hardening / the Ohno-Wang backstress.** Hence no Bauschinger
  effect. This is the **largest gap for the fatigue work** and it is in this
  layer, not in `src/fatigue.rs`.
- **Deformation-increment sub-stepping** (`numberOfCuts`). The total-strain
  interface of `Material::update` does not carry the start-of-step strain, so
  it cannot sub-step. Instead the local Newton's *starting guess* has its
  deviator scaled back until it implies no more than 1 % slip
  (`GUESS_SLIP_CAP`), which bounds the initial residual. A large enough step
  still fails with `ConstitutiveNotConverged` rather than recovering.
- **Implicit hardening.** Upstream iterates an outer loop until `s` stops
  moving; here `s` is frozen through the local solve and advanced once
  afterwards. That is what makes the algorithmic tangent the *exact*
  derivative of the update as implemented, which verification case 17 measures
  and case 18 depends on. Making it fully implicit means differentiating an
  `18 x 18` system and is a real piece of work, not a tidy-up.

**Rules for anyone extending it:**

- **Do not add a second convention.** `Orientation` stores the
  **crystal-to-sample** matrix, matching upstream's `rotmat`. Bunge Euler
  angles define the *inverse*, and `from_bunge_euler_radians` transposes for
  you. Verification cases 13 and 20 pin this from both sides — a tensor
  rotation and a vector rotation — and either would fail instantly if the
  convention drifted.
- **`MAX_SLIP_SYSTEMS` is 12 and the state arrays are fixed-size** so
  `MaterialState` stays `Copy`. Adding BCC `{112}`/`{123}` means raising it,
  which costs memory at *every* quadrature point of *every* analysis,
  including elastic ones. Weigh that rather than just bumping the constant.
- **`MaterialState::crystal` is present for every material.** An elastic or J2
  analysis leaves it at `CrystalState::default()` — zero slip resistances,
  which is the "not a crystal" marker. `Material::initial_state` is what
  picks the right virgin state; `System::build` calls it. A caller that
  hand-builds a `MaterialState` for a crystal and forgets this gets a
  divide-by-zero in the flow rule, which `update` defends against by
  substituting `s_0`.
- **Never describe an output of `src/fatigue.rs` as a fatigue life.** It is an
  ordering of candidate initiation sites. See that module's own documentation.

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
