//! Pump component.
//!
//! Composes [`outram_park_fork_dwsim_libs::pump::modes`]'s already-working
//! calculation-mode algebra (`PumpSpecification`/`PumpInlet`/`evaluate`)
//! behind a TAMPINES-native struct.

use crate::TampinesError;
use outram_park_fork_dwsim_libs::pump::modes::{PumpInlet, PumpResult, PumpSpecification};
use uom::si::f64::Ratio;

/// A pump: one [`PumpSpecification`] (which quantity fixes its operating
/// point) plus an efficiency.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pump {
    /// Which quantity specifies this pump's operating point.
    pub specification: PumpSpecification,
    /// Pump efficiency \[0, 1\].
    pub efficiency: Ratio,
}

impl Pump {
    /// Construct a new pump with the given operating-point specification and
    /// efficiency.
    pub fn new(specification: PumpSpecification, efficiency: Ratio) -> Self {
        Self {
            specification,
            efficiency,
        }
    }

    /// Evaluate this pump's outlet state for the given inlet.
    ///
    /// Not yet implemented as a TAMPINES component method -- the underlying
    /// algebra already exists and works
    /// ([`outram_park_fork_dwsim_libs::pump::modes::evaluate`]); this
    /// wrapper's job (dispatching to it, and in future threading through a
    /// real flash for outlet temperature) is not yet wired up.
    pub fn evaluate(&self, _inlet: PumpInlet) -> Result<PumpResult, TampinesError> {
        Err(TampinesError::NotYetImplemented {
            component: "components::pump::Pump::evaluate",
        })
    }
}
