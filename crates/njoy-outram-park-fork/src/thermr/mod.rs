//! Thermal neutron scattering — the **THERMR** domain (S(α,β) processing).
//!
//! This module is the *input* side of the thermal pipeline: it reads the ENDF
//! **MF=7** thermal scattering-law evaluations (the `tsl-*` sublibrary) and
//! computes the bound-atom thermal cross sections and secondary distributions
//! that the thermal ACE writer ([`crate::acer::thermal`]) consumes.
//!
//! ```text
//!   MF=7 (S(α,β))  →  thermr::mf7 (read)  →  THERMR (compute σ, dists)  →  acer::thermal (write)
//!        this module ^^^^^^^^^^^^^^^^^^^^      done                          done (IFENG=0)
//! ```
//!
//! ## Status
//!
//! - [`mf7`] — **done**: parse MF=7 MT=2 (coherent/incoherent elastic) and MT=4
//!   (incoherent inelastic S(α,β)) into typed data.
//! - [`coherent`] — **done**: coherent-elastic (Bragg) cross section σ(E)=S(E)/E
//!   and the discrete reflection cosines/weights.
//! - [`incoherent_elastic`] — **done**: incoherent-elastic cross section
//!   σ(E,T) = (σ_b/2N)·(1−e^{−4EW'})/(2EW') and its equally-probable cosines
//!   (closed-form CDF inversion of the exponential angular law).
//! - [`inelastic`] — **done**: incoherent-inelastic double-differential
//!   `d²σ/dE'dμ` from S(α,β), including the **short-collision-time (SCT) tail**
//!   beyond the tabulated `(α,β)` grid so `σ(E)` reaches the free-atom limit at
//!   high `E`; the integrated `σ(E→E')` / `σ_inel(E)`; and the `nieb`×`nang`
//!   equiprobable emission table (`equiprobable_emission`) the ACE ITXE block
//!   needs, via numerical CDF inversion.
//! - [`scattering`] — **done**: the [`scattering::IncoherentInelasticScattering`]
//!   consumer surface `outram-mc-libs` calls for a thermal pincell —
//!   `uom`-typed `σ_inel(E)` per principal atom and the emission table, built at
//!   a chosen temperature (selecting the nearest tabulated S(α,β) tables).
//! - The `aceth.f90` writer ([`crate::acer::thermal`]) is **done** for the
//!   standard IFENG=0 (equiprobable) case, both coherent- and
//!   incoherent-elastic. Not ported: IFENG=1/2 (skewed/continuous inelastic
//!   forms) and multi-scatterer mixing (`nmix` > 1) — see that module's docs.
//!
//! See `docs/porting-plan.md` (Phase 3 THERMR, Phase 4f thermal ACE).

pub mod coherent;
pub mod incoherent_elastic;
pub mod inelastic;
pub mod mf7;
pub mod scattering;

pub use scattering::{IncoherentInelasticScattering, ThermalEmissionBin};

/// Run the THERMR card-input driver (NJOY module entry point).
///
/// **Status:** this module's processing physics is ported (see its `README.md`
/// and the typed API above); the NJOY *card-input driver* itself is not yet
/// ported, so this returns [`crate::NjoyError::NotPorted`]. Use the module's
/// typed API directly rather than this driver.
pub fn run() -> Result<(), crate::NjoyError> {
    Err(crate::NjoyError::NotPorted("thermr driver (physics ported — use the module API)"))
}
