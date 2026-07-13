//! Steam generator component (primary-to-secondary heat exchanger with
//! boiling on the secondary side).
//!
//! DWSIM has no dedicated steam-generator/boiler unit-op; the closest analog
//! is its generic `HeatExchanger.vb` (see the workspace's `op-dt3.10` bead
//! notes). This component treats a steam generator as a specialized
//! two-phase heat exchanger: [`crate::components::heat_exchanger::HeatExchanger`]
//! for the primary-to-secondary coupling, [`crate::hem::HemSteamCv`] for the
//! boiling secondary-side steam/water state.

use crate::components::heat_exchanger::HeatExchanger;
use crate::hem::HemSteamCv;
use crate::TampinesError;
use uom::si::f64::Power;

/// A steam generator: the primary-to-secondary heat exchanger plus the
/// current secondary-side (steam/water) state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SteamGenerator {
    /// Primary-to-secondary heat-exchanger geometry/coefficient.
    pub heat_exchanger: HeatExchanger,
    /// Current secondary-side steam/water state.
    pub secondary_side: HemSteamCv,
}

impl SteamGenerator {
    /// Construct a new steam generator around the given heat exchanger and
    /// initial secondary-side state.
    pub fn new(heat_exchanger: HeatExchanger, secondary_side: HemSteamCv) -> Self {
        Self { heat_exchanger, secondary_side }
    }

    /// Advance the secondary-side state given a primary-side heat duty.
    ///
    /// Not yet implemented -- needs [`HemSteamCv::advance_timestep`] fed by
    /// a real mass/enthalpy balance against the primary side, which isn't
    /// threaded through yet.
    pub fn step(&mut self, _primary_duty: Power) -> Result<(), TampinesError> {
        Err(TampinesError::NotYetImplemented {
            component: "components::steam_generator::SteamGenerator::step",
        })
    }
}
