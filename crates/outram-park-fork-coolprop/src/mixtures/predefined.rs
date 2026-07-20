//! Predefined mixtures — ready-made compositions so a user need not hand-build
//! the parallel `(components, mole_fractions)` arrays.
//!
//! Compositions are transcribed from CoolProp's predefined-mixture table
//! (`dev/mixtures/predefined_mixtures.json`, the source of the `*.mix` fluids),
//! MIT-licensed. Each constructor builds the arrays and routes them through
//! [`Mixture::new`], so the same composition hygiene (lengths, non-negativity,
//! `Σ x_i = 1` within [`Mixture::SUM_TOLERANCE`]) applies to the predefined
//! sets as to any user composition. The tabulated fractions all sum to 1 to
//! well within that tolerance, so these constructors are effectively infallible
//! — the fallibility is retained only so the single validation path in
//! [`Mixture::new`] stays the sole gatekeeper.
//!
//! Molar composition (mole fractions), not mass fractions.

use super::{Mixture, MixtureError};
use crate::fluid::Fluid;

impl Mixture {
    /// Standard dry air as a three-component multi-fluid mixture — Nitrogen /
    /// Oxygen / Argon.
    ///
    /// Mole fractions (CoolProp `Air.mix`, from
    /// `dev/mixtures/predefined_mixtures.json`):
    ///
    /// | Component | `x_i` \[-\] |
    /// |---|---|
    /// | Nitrogen | 0.7812 |
    /// | Oxygen   | 0.2096 |
    /// | Argon    | 0.0092 |
    ///
    /// These sum to exactly 1. This is the Lemmon et al. (2000) reference dry-air
    /// composition CoolProp uses for its pseudo-pure `Air` fluid, here evaluated
    /// through the *actual* three-component multi-fluid surface
    /// ([`Mixture::state_trho_molar`]).
    ///
    /// Returns an error only if the shared [`Mixture::new`] validation ever
    /// rejects this (fixed, valid) composition — in practice it does not.
    pub fn air() -> Result<Mixture, MixtureError> {
        Mixture::new(
            vec![Fluid::Nitrogen, Fluid::Oxygen, Fluid::Argon],
            vec![0.7812, 0.2096, 0.0092],
        )
    }

    /// R-410A refrigerant blend — a near-azeotropic binary of R-32 and R-125.
    ///
    /// Mole fractions (CoolProp `R410A.mix`, from
    /// `dev/mixtures/predefined_mixtures.json`):
    ///
    /// | Component | `x_i` \[-\] |
    /// |---|---|
    /// | R32  | 0.697614699375863 |
    /// | R125 | 0.302385300624138 |
    ///
    /// (This is the mole-fraction split CoolProp derives from R-410A's nominal
    /// 50/50 *mass* split of R-32/R-125.) Provided as a second, chemically
    /// distinct predefined blend alongside [`Mixture::air`].
    ///
    /// Returns an error only if the shared [`Mixture::new`] validation ever
    /// rejects this (fixed, valid) composition — in practice it does not.
    pub fn r410a() -> Result<Mixture, MixtureError> {
        Mixture::new(
            vec![Fluid::R32, Fluid::R125],
            vec![0.697_614_699_375_863, 0.302_385_300_624_138],
        )
    }
}
