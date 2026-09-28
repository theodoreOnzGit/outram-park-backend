// SPDX-License-Identifier: GPL-3.0-only
//! The dose newtype of the pyDOSEIA port, and the dilution-factor type it
//! shares with `changi`.
//!
//! # Why dose is a newtype and not a `uom` quantity
//!
//! `uom` 0.38 has no sievert, gray or equivalent-dose quantity (checked
//! 2026-09-28; there is no `absorbed_dose` or `dose_equivalent` module in
//! `uom::si`). `AvailableEnergy` (J/kg) has the right *dimension* and was
//! rejected on purpose: a sievert is a J/kg only after radiation and tissue
//! weighting, and a type that lets a dose be added to a specific energy would
//! hide that. [`EffectiveDose`] therefore wraps an `f64` and names its unit in
//! every accessor.
//!
//! **It stores millisieverts, not sieverts.** Upstream computes every dose in
//! mSv (it multiplies by `1000` inside each pathway), and the code-to-code test
//! compares the port against upstream's mSv values. Storing the mSv value as
//! computed keeps that comparison bit-exact; converting to Sv and back would
//! add a rounding step upstream does not have.
//!
//! # The dilution factor is `changi`'s
//!
//! `chi/Q` (s/m^3) is [`DilutionFactor`], re-exported from
//! `changi::activity::units`, the workspace's existing type for it. The port
//! therefore hands its dilution factors straight to anything in `changi`
//! and takes `changi`'s. `buangkok -> changi` is the dependency direction: dose
//! sits downstream of dispersion.

pub use changi::activity::units::DilutionFactor;

/// An effective dose (or, for a long-term release, the dose accrued over one
/// year of discharge), as computed by the pyDOSEIA port.
///
/// Stored in **millisieverts** (see the module docs). Research-grade only:
/// never a dose to a real person (`RESPONSIBLE_USE.md`).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct EffectiveDose(f64);

impl EffectiveDose {
    /// From a value in millisieverts.
    #[must_use]
    pub const fn from_millisieverts(msv: f64) -> Self {
        Self(msv)
    }

    /// From a value in sieverts.
    #[must_use]
    pub fn from_sieverts(sv: f64) -> Self {
        Self(sv * 1000.0)
    }

    /// The dose in millisieverts (exactly the value upstream reports).
    #[must_use]
    pub const fn millisieverts(self) -> f64 {
        self.0
    }

    /// The dose in sieverts.
    #[must_use]
    pub fn sieverts(self) -> f64 {
        self.0 / 1000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dose_unit_conversions_round_trip() {
        let d = EffectiveDose::from_sieverts(2.5e-3);
        assert_eq!(d.millisieverts(), 2.5);
        assert_eq!(d.sieverts(), 2.5e-3);
    }
}
