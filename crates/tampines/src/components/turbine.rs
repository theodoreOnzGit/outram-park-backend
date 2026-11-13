//! Steam turbine component.
//!
//! Composes [`crate::hem::HemSteamCv`] (the inlet/outlet steam state) with
//! [`outram_park_fork_dwsim_libs::expander::isentropic`]'s adiabatic/Schultz
//! polytropic turbine model.
//!
//! **For a transient pipe network, use [`super::junction::HemJunction::Turbine`]
//! (gh:#1072, RELAP-7 parity epic gh:#1056)**, which runs inside a
//! [`super::junction::HemNetwork`] with its own V&V
//! (`tampines-steam-tables/verification_and_validation/hem_network_junctions.md`).
//! This struct is the steady turbine sizing/rating wrapper and is separate from it.

use crate::hem::HemSteamCv;
use crate::TampinesError;
use uom::si::f64::{Pressure, Ratio};

/// A steam turbine: inlet state plus an efficiency specification.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Turbine {
    /// Inlet steam state.
    pub inlet: HemSteamCv,
    /// Adiabatic efficiency \[0, 1\] (this scaffold only represents the
    /// adiabatic path; the polytropic path needs a flash-dependent
    /// iteration -- see
    /// [`outram_park_fork_dwsim_libs::expander::isentropic::solve_polytropic_efficiency`]).
    pub adiabatic_efficiency: Ratio,
}

impl Turbine {
    /// Construct a new turbine with the given inlet state and adiabatic
    /// efficiency.
    pub fn new(inlet: HemSteamCv, adiabatic_efficiency: Ratio) -> Self {
        Self {
            inlet,
            adiabatic_efficiency,
        }
    }

    /// Expand this turbine's inlet steam to the given outlet pressure,
    /// returning the outlet state and generated power.
    ///
    /// Not yet implemented -- the underlying isentropic/Schultz algebra
    /// already exists and works
    /// ([`outram_park_fork_dwsim_libs::expander::isentropic`]); this
    /// wrapper's job (computing the isentropic outlet enthalpy via a real
    /// PS flash, then applying the efficiency correction) is not yet wired
    /// up.
    pub fn expand_to(&self, _outlet_pressure: Pressure) -> Result<HemSteamCv, TampinesError> {
        Err(TampinesError::NotYetImplemented {
            component: "components::turbine::Turbine::expand_to",
        })
    }
}
