//! Condenser component.
//!
//! Wraps [`crate::hem::HemSteamCv`] directly, rather than any DWSIM-derived
//! model -- DWSIM has no dedicated condenser unit-op (its closest analog,
//! `Cooler.vb`, has no phase-change-specific treatment; see the workspace's
//! `op-dt3.10` bead notes). Condensation is naturally represented via
//! `HemSteamCv`'s own VLE-dome-aware quality state instead.

use crate::hem::HemSteamCv;
use crate::TampinesError;
use uom::si::f64::Pressure;

/// A condenser: an operating pressure and a target outlet quality (`0.0` =
/// saturated liquid, the typical target; a small positive value models
/// subcooling downstream instead).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Condenser {
    /// Condenser (shell-side steam) operating pressure.
    pub pressure: Pressure,
    /// Target outlet quality \[0, 1\] -- `0.0` for saturated-liquid outlet.
    pub target_outlet_quality: f64,
}

impl Condenser {
    /// Construct a new condenser at the given operating pressure and target
    /// outlet quality.
    pub fn new(pressure: Pressure, target_outlet_quality: f64) -> Self {
        Self { pressure, target_outlet_quality }
    }

    /// Condense the given inlet steam state to this condenser's target
    /// outlet quality at its operating pressure.
    ///
    /// Not yet implemented -- needs [`HemSteamCv`]'s saturation-pressure
    /// constructor plus an energy balance against the cooling-water side,
    /// which isn't threaded through yet.
    pub fn condense(&self, _inlet: HemSteamCv) -> Result<HemSteamCv, TampinesError> {
        Err(TampinesError::NotYetImplemented {
            component: "components::condenser::Condenser::condense",
        })
    }
}
