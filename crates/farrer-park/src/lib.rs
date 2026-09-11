//! # FARRER PARK — FEM structural mechanics
//!
//! **F**inite-element **A**nalysis for **R**eactor **R**eliability,
//! **E**ngineering **R**esponse, **P**lasticity **A**nd **R**isk.
//!
//! Small-strain solid mechanics on unstructured meshes: linear elasticity and
//! J2 (von Mises) plasticity, solved with a Newton iteration over Krylov
//! solves supplied by [`outram_foam_basic_lib`].
//!
//! ## Two modelling choices, both explicit enums, both defaulting to the
//! conservative option
//!
//! Carried on [`assembly::System`] through [`assembly::SystemOptions`]:
//!
//! - [`assembly::Formulation`] — `FullIntegration` (default) or `BBar`. B-bar
//!   (mean dilatation) is the cure for **volumetric locking**, the over-stiff
//!   response of a low-order element whose material is nearly incompressible,
//!   elastically as `nu -> 0.5` or plastically because J2 flow preserves
//!   volume. It does nothing for **shear locking**, which is a different
//!   mechanism and is still an open defect here — both are measured in
//!   `docs/verification.md`, cases 6 to 8.
//! - [`material::PlaneCondition`] — `PlaneStrain` (default, and the only valid
//!   setting on a three-dimensional mesh, where it is a no-op) or
//!   `PlaneStress`, which condenses `eps_zz` out of the constitutive law.
//!
//! Neither default changes behaviour that existed before they were added, which
//! is deliberate: a silently changed default is worse than the defect it fixes.
//!
//! ## What belongs in this crate
//!
//! Everything specific to the **finite-element method**: reference elements and
//! shape functions, quadrature rules, degree-of-freedom numbering, weak-form
//! assembly, boundary conditions, constitutive integration at quadrature
//! points, tangent assembly, and nonlinear solution control.
//!
//! ## What does NOT belong here
//!
//! - **Krylov solvers, preconditioners and multigrid.** Those live in
//!   `outram-foam-basic-lib` and are shared with the finite-volume side. This
//!   crate contributes a matrix type that implements the same operator
//!   contract; it does not reimplement the solvers.
//! - **Finite-volume machinery.** `FvMesh`, `fvPatchField`, face-addressed
//!   operators. Farrer Park is genuinely FEM and must stay that way — see the
//!   note below.
//! - **Cross-section or neutronics data.** That is `njoy-outram-park-fork`.
//!
//! ## FEM stays FEM
//!
//! Farrer Park depends on `outram-foam-basic-lib` for *shared numerical
//! infrastructure*, never for its spatial discretisation. Structural mechanics
//! is **not** reformulated as finite volume merely because the linear algebra
//! is shared (GitHub issue #175).
//!
//! The two discretisations meet at exactly one contract,
//! [`operator::FemLinearOperator`]:
//!
//! - `LduMatrix` in `outram-foam-basic-lib` remains the face-addressed,
//!   FVM-optimised representation.
//! - [`sparse::CsrMatrix`] here is the FEM-appropriate row-compressed
//!   representation, built from the DoF connectivity graph.
//!
//! Both implement the contract, so both reuse one Krylov layer while keeping
//! the matrix layout each discretisation actually needs.
//!
//! ## Units
//!
//! **Every public item's doc comment states its units in words**, without
//! exception, because a human reading a signature should not have to infer
//! them. Internally, assembly, the constitutive integration and the linear
//! solve all work in bare `f64` **SI base units** — metres, pascals, newtons —
//! since a Krylov vector and a fourth-order tangent have no single `uom` type
//! between them.
//!
//! `uom` typing is offered at the **material boundary**, which is where a user
//! actually types a physical number and where a factor of `1e6` is most easily
//! lost: [`material::LinearElastic::from_quantities`] and
//! [`material::J2LinearHardening::from_quantities`] take unit-checked
//! quantities, and the named aliases [`material::YoungsModulus`],
//! [`material::ShearModulus`], [`material::BulkModulus`],
//! [`material::YieldStress`], [`material::HardeningModulus`] and
//! [`material::PoissonRatio`] keep a hover showing a name rather than a raw
//! `Quantity<ISQ<...>, SI<f64>, f64>`.
//!
//! Mesh coordinates, displacements and nodal forces are **not** `uom`-typed.
//! They live in long `Vec<f64>` buffers that are indexed by degree of freedom
//! and handed straight to the linear solver; wrapping each entry would cost the
//! flat layout the solver needs and buy nothing a doc comment does not already
//! give. That is a deliberate line, stated here rather than left to be
//! discovered.
//!
//! ## Status
//!
//! AI-assisted draft. Verified against analytical and manufactured solutions
//! only — see `docs/verification.md` for methodology *and* measured results.
//! There is **no human V&V sign-off**, no benchmark validation, and no crystal
//! plasticity or fatigue layer yet. Not for RPV or piping life assessment.
//!
//! ## Design rules this crate follows
//!
//! Per the workspace `CLAUDE.md`: no trait objects for dispatch (closed sets
//! are enums, traits are compiler-enforced contracts used as generic bounds),
//! no `Box<T>`, no lifetime parameters in structs (own by value or share with
//! `Arc<T>`; topology is referenced by index newtypes such as
//! [`mesh::NodeId`]).

#![forbid(unsafe_code)]

pub mod assembly;
pub mod bc;
pub mod dof;
pub mod element;
pub mod error;
pub mod material;
pub mod mesh;
pub mod operator;
pub mod quadrature;
pub mod solver;
pub mod sparse;
pub mod tensor;

pub mod prelude;
