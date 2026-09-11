//! # FARRER PARK — FEM structural mechanics
//!
//! **F**inite-element **A**nalysis for **R**eactor **R**eliability,
//! **E**ngineering **R**esponse, **P**lasticity **A**nd **R**isk.
//!
//! Small-strain solid mechanics on unstructured meshes: linear elasticity and
//! J2 (von Mises) plasticity, solved with a Newton iteration over Krylov
//! solves supplied by [`outram_foam_basic_lib`].
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
//! [`operator::LinearOperator`]:
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
//! The public API boundary is `uom`-typed. Internally, assembly and the linear
//! solve work in bare `f64` **SI base units** for speed — metres, pascals,
//! newtons, kelvin. Every conversion happens at the boundary, and each public
//! item's doc comment states its units in words even where `uom` already
//! enforces them, because a human reading the signature should not have to
//! infer them.
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
