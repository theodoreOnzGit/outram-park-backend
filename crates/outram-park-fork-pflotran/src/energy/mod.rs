//! TH energy transport (v1) — bead op-v6s.10.
//!
//! **Scaffold.** Heat transport for the thermal-hydraulic (TH) flow mode:
//! conservation of energy for temperature `T`, advected by the Darcy flow field
//! (from a RICHARDS solve) and conducted through the fluid+rock. Implicit Euler,
//! cell-centred FV, first-order upwind advection + two-point conduction, solved
//! with the foam-basic-lib Krylov backend. v1 is one-way (weakly) coupled: the
//! flow field is frozen; buoyancy/viscosity feedback on flow is deferred.
