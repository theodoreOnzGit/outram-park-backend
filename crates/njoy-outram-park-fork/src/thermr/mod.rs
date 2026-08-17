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
//! - [`mf7`] — **done, all tabulated temperatures**: parse MF=7 MT=2
//!   (coherent/incoherent elastic) and MT=4 (incoherent inelastic S(α,β)) into
//!   typed data. The coherent-elastic `S(E)` is retained at *every* tabulated
//!   temperature with the ENDF `LI` codes (the pre-2026-08-11 base-T₀-only
//!   defect was bead `op-1y4y`), and the S(α,β) is resolved at a requested
//!   temperature by the shared policy: NJOY-tolerance match → `LI`-law
//!   interpolation → refuse with
//!   [`TemperatureOutOfRange`](crate::NjoyError::TemperatureOutOfRange) —
//!   never a silent snap.
//! - [`coherent`] — **done**: coherent-elastic (Bragg) cross section
//!   σ(E,T)=S(E,T)/E and the discrete reflection cosines/weights, temperature-
//!   resolved across the evaluation's full tabulated range (296–2000 K for the
//!   ENDF/B-VIII.0 graphites).
//! - [`incoherent_elastic`] — **done**: incoherent-elastic cross section
//!   σ(E,T) = (σ_b/2N)·(1−e^{−4EW'})/(2EW') and its equally-probable cosines
//!   (closed-form CDF inversion of the exponential angular law).
//! - [`inelastic`] — **done**: incoherent-inelastic double-differential
//!   `d²σ/dE'dμ` from S(α,β), including the **short-collision-time (SCT) tail**
//!   beyond the tabulated `(α,β)` grid so `σ(E)` reaches the free-atom limit at
//!   high `E`; the integrated `σ(E→E')` / `σ_inel(E)`; and the `nieb`×`nang`
//!   equiprobable emission table (`equiprobable_emission`) the ACE ITXE block
//!   needs, via numerical CDF inversion.
//! - [`scattering`] — **done, all three channels**: the consumer surface
//!   `outram-mc-libs` calls — [`scattering::IncoherentInelasticScattering`]
//!   (σ_inel + emission bins), [`scattering::CoherentElasticScattering`]
//!   (graphite's dominant channel: Bragg σ + discrete cosines + `sample`), and
//!   [`scattering::IncoherentElasticScattering`] (ZrH: σ + equiprobable
//!   cosines), each `uom`-typed, per principal atom, and temperature-resolved
//!   at construction.
//! - [`temperature_thinning`] — **study tool, not a production path**: measures
//!   what a *thinned* tabulated-temperature grid costs in accuracy, by
//!   withholding tabulated temperatures, interpolating to them from the ones
//!   kept, and comparing against the evaluation's own values. Also does
//!   leave-one-out characterisation of the existing production interpolation.
//! - The `aceth.f90` writer ([`crate::acer::thermal`]) is **done** for the
//!   standard IFENG=0 (equiprobable) case, both coherent- and
//!   incoherent-elastic, with the coherent `S(E)` resolved at the requested
//!   temperature. Not ported: IFENG=1/2 (skewed/continuous inelastic forms)
//!   and multi-scatterer mixing (`nmix` > 1) — see that module's docs.
//!
//! See `docs/porting-plan.md` (Phase 3 THERMR, Phase 4f thermal ACE).

pub mod coherent;
pub mod incoherent_elastic;
pub mod inelastic;
pub mod mf7;
pub mod scattering;
pub mod temperature_thinning;

pub use scattering::{
    CoherentElasticScattering, IncoherentElasticScattering, IncoherentInelasticScattering,
    ThermalEmissionBin,
};

/// Run the THERMR card-input driver (NJOY module entry point).
///
/// **Status:** this module's processing physics is ported (see its `README.md`
/// and the typed API above); the NJOY *card-input driver* itself is not yet
/// ported, so this returns [`crate::NjoyError::NotPorted`]. Use the module's
/// typed API directly rather than this driver.
pub fn run() -> Result<(), crate::NjoyError> {
    Err(crate::NjoyError::NotPorted(
        "thermr driver (physics ported — use the module API)",
    ))
}
