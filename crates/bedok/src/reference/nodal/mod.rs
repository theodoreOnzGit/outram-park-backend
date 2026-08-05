//! Semi-analytic nodal method (SANM) — the neutronics core.
//!
//! # Provenance
//!
//! Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
//! Institute (SNRSI). Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`.
//! Translated under the permission recorded in `docs/bedok-port-scoping.md` §6.
//!
//! Fourteen MATLAB files, ~3,857 lines, are translated here. Each Rust module
//! names its `.m` source in its own header; the map is:
//!
//! | MATLAB file | Rust module |
//! |---|---|
//! | `sanodaldiffusion_solverxyz.m` | [`sanm_solver`] |
//! | `diffusion_solverxyz.m` | [`finite_difference_solver`] |
//! | `calc_sanodalxyz.m` | [`nodal_correction`] |
//! | `calc_a1234_expansionxyz.m` | [`expansion`] |
//! | `calc_a1_expansionxyz.m` | [`first_moment`] |
//! | `calc_ABEFGHxyz.m` | [`nodal_coefficients`] |
//! | `calc_bucklingxyz.m` | [`buckling`] |
//! | `calc_transleakagexyz.m` | [`transverse_leakage`] |
//! | `calc_1sttransleakagexyz.m`, `calc_2ndtransleakagexyz.m` | [`leakage_moments`] |
//! | `makegradDxyz.m` | [`gradient_diffusion`] |
//! | `makesigmadfxyz.m`, `calcdiffvalues3d.m` | [`cross_sections`] |
//! | `fiss_src_extrapolatexyz.m` | [`fission_source`] |
//! | *(the `geometry`/`params` structs, `handle3dcoords.m`)* | [`geometry`] |
//! | *(MATLAB sparse built-ins, `\`, `decomposition`)* | [`sparse`] |
//! | *(the `scalar_flux` history matrix)* | [`flux_history`] |
//!
//! # What the method is
//!
//! Coarse-mesh nodal diffusion. The core is divided into assembly-sized nodes
//! — 20 cm across in the benchmarks, far too coarse for finite difference to be
//! accurate. Rather than refine the mesh, the semi-analytic nodal method solves
//! the one-dimensional diffusion equation *analytically* within each node
//! along each axis, treating leakage in the other two directions as a known
//! transverse source fitted by a parabola. The resulting surface currents are
//! folded back into a finite-difference-shaped operator as a **correction** to
//! the coupling coefficients, so the global solve stays a sparse linear system
//! of the same size and sparsity as plain finite difference.
//!
//! The data flow of one nodal update, which is also the module dependency
//! order:
//!
//! ```text
//! cross_sections ---> gradient_diffusion ---+
//!        |                                  |
//!        +---> nodal_coefficients ----------+
//!                                           v
//!            transverse_leakage ---> leakage_moments ---> buckling
//!                                           |                 |
//!                                           +--> first_moment<+
//!                                                    |
//!                                              expansion (A1..A4)
//!                                                    |
//!                                            nodal_correction
//!                                                    |
//!                                               sanm_solver
//! ```
//!
//! # Entry points
//!
//! - [`sanm_solver::solve`] — the nodal `k`-eigenvalue solve.
//! - [`finite_difference_solver::solve`] — the same problem without the nodal
//!   correction, as a cross-check.
//!
//! Everything else exists so those two can be read, tested and debugged a
//! stage at a time.
//!
//! # Faithfulness and its consequences
//!
//! This is a stage-1 translation: structure, iteration order and convergence
//! logic follow the MATLAB, and nothing that looked wrong was repaired. The
//! places where the reference is unfinished, fragile or self-inconsistent are
//! recorded in the doc comment of the item where they occur, under a heading
//! that says so. The ones a reader should know about before trusting a result:
//!
//! - **`Nc > 0` does not work**, in the MATLAB or here — see
//!   [`geometry::NodalParams::n_precursor_groups`].
//! - **A direction with a single node is not a supported mesh**: the boundary
//!   blocks index a neighbour outside the grid. Two nodes per direction is the
//!   minimum.
//! - **The near-zero-flux guard in [`nodal_correction`] silently falls back to
//!   finite difference**, on a threshold relative to the global flux maximum.
//! - **A reflective outer face over a node with no material** makes an
//!   outer-face system exactly singular; the reference produces `Inf`/`NaN`
//!   there, and so does this port — see [`first_moment::assemble`].
//! - **The `1e6` substitution for a zero diffusion coefficient** in
//!   [`expansion::assemble`] is a magic guard, not a physical value.
//!
//! # Verification status
//!
//! **Unverified against the benchmarks.** The unit tests here check formulas,
//! index maps and a handful of analytically-known limits (an infinite-medium
//! `k_inf`, conservation on a reflective block). They do not establish that the
//! method reproduces IAEA-3D or NEACRP, and no comparison against Yan Ren's own
//! results has been made — see `docs/bedok-port-scoping.md` §4 for why
//! "reproduces Yan Ren's results" must not be claimed.

pub mod buckling;
pub mod cross_sections;
pub mod expansion;
pub mod finite_difference_solver;
pub mod first_moment;
pub mod fission_source;
pub mod flux_history;
pub mod geometry;
pub mod gradient_diffusion;
pub mod leakage_moments;
pub mod nodal_coefficients;
pub mod nodal_correction;
pub mod sanm_solver;
pub mod sparse;
pub mod transverse_leakage;

pub use finite_difference_solver::FiniteDifferenceSolution;
pub use flux_history::FluxHistory;
pub use geometry::{
    ActiveRange, Axis, BoundaryCondition, BoundaryConditions, Face, FaceTerms, NodalGeometry,
    NodalParams,
};
pub use sanm_solver::{DiffusionSolution, Termination};
