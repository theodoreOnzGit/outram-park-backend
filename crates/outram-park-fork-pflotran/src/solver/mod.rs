//! Newton–Krylov nonlinear solver layer (v1, KEYSTONE) — bead op-v6s.4.
//!
//! **Scaffold.** A serial, pure-Rust Newton driver (residual/Jacobian assembly
//! contract, damping/line-search, convergence control) whose linear step is
//! delegated to outram-foam-basic-lib's `krylov` asymmetric solvers
//! (BiCGStab / GMRES + ILU(0)/Jacobi). No PETSc, no MPI.
