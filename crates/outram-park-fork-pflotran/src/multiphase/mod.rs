//! GENERAL multiphase flow (v1) — bead op-v6s.13.
//!
//! **Scaffold.** Two-phase immiscible (air–water) isothermal flow on the block
//! multi-DOF Newton-Krylov solver ([`crate::solver::block`]): two primary
//! unknowns per cell (liquid pressure + liquid saturation) with capillary
//! closure `P_g - P_l = P_c(S_l)`. The full air–water–energy GENERAL mode
//! (adding temperature as a third unknown) is a follow-up.
