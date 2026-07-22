//! Reactive transport (v1) — operator-split coupling of solute [`transport`](crate::transport)
//! and equilibrium [`geochemistry`](crate::geochemistry). Bead op-v6s.12.
//!
//! **Scaffold.** Each timestep transports the total aqueous component
//! concentrations (advection–dispersion, reusing the transport operator), then
//! re-speciates each cell to equilibrium. Sequential non-iterative (SNIA)
//! operator splitting. Minerals/kinetics deferred.
