//! Thermal neutron scattering — the **THERMR** domain (S(α,β) processing).
//!
//! This module is the *input* side of the thermal pipeline: it reads the ENDF
//! **MF=7** thermal scattering-law evaluations (the `tsl-*` sublibrary) and, in
//! future, computes the bound-atom thermal cross sections and secondary
//! distributions that the thermal ACE writer ([`crate::ace::thermal`]) consumes.
//!
//! ```text
//!   MF=7 (S(α,β))  →  thermal::mf7 (read)  →  THERMR (compute σ, dists)  →  ace::thermal (write)
//!        this module ^^^^^^^^^^^^^^^^^^^^      (Phase 3, future)            (Phase 4f, scaffold)
//! ```
//!
//! ## Status
//!
//! - [`mf7`] — **done**: parse MF=7 MT=2 (coherent/incoherent elastic) and MT=4
//!   (incoherent inelastic S(α,β)) into typed data.
//! - THERMR cross-section/distribution computation — not yet ported (`thermr.f90`).
//!
//! See `docs/porting-plan.md` (Phase 3 THERMR, Phase 4f thermal ACE).

pub mod mf7;
