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
//! - [`coherent`] — **done**: coherent-elastic (Bragg) cross section σ(E)=S(E)/E
//!   and the discrete reflection cosines/weights.
//! - [`incoherent_elastic`] — **done**: incoherent-elastic cross section
//!   σ(E,T) = (σ_b/2N)·(1−e^{−4EW'})/(2EW') and its equally-probable cosines.
//! - [`inelastic`] — **done**: incoherent-inelastic double-differential
//!   `d²σ/dE'dμ` from S(α,β), and the integrated `σ(E→E')` / `σ_inel(E)`.
//! - Secondary energy-angle *distributions* for the thermal ACE ITXE block, and
//!   the `aceth.f90` writer — still to come.
//!
//! See `docs/porting-plan.md` (Phase 3 THERMR, Phase 4f thermal ACE).

pub mod coherent;
pub mod incoherent_elastic;
pub mod inelastic;
pub mod mf7;
