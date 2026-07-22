//! Conservative solute transport (v1) — bead op-v6s.11.
//!
//! **Scaffold.** Advection–diffusion–dispersion of a passive (non-reactive)
//! solute on the structured Cartesian grid, given a flow field (Darcy face
//! fluxes + water content) from a RICHARDS solve. Implicit Euler in time,
//! upwind advection + two-point dispersive flux, solved with the foam-basic-lib
//! Krylov backend. Reactions/geochemistry are deferred (bead op-v6s.12).
