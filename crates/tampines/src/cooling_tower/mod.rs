//! Cooling-tower thermal-hydraulics.
//!
//! **Status: scaffold only.** This is the intended home for the actual
//! cooling-tower calculation engine (counter-flow air-water heat and mass
//! transfer -- the Merkel method / its effectiveness-NTU reformulation).
//! [`crate::components::CoolingTower`] is the user-facing struct/interface;
//! [`crate::humid_air`] is the psychrometrics module this engine will build
//! on. Neither the physics nor the wiring between them exists yet.
//!
//! DWSIM has no cooling-tower unit-op to port (checked directly against the
//! DWSIM source -- only a saved FOSSEE flowsheet built from generic blocks,
//! not a unit-op class; see the workspace's `op-dt3.10` bead notes). So this
//! module's eventual physics will need to be implemented from published
//! theory rather than translated, e.g.:
//!
//! - Merkel, F. (1925). *Verdunstungskühlung*. VDI-Zeitschrift.
//! - Braun, J. E., Klein, S. A., & Mitchell, J. W. (1989). Effectiveness
//!   models for cooling towers and cooling coils. *ASHRAE Transactions*,
//!   95(2), 164-174 (the effectiveness-NTU reformulation of Merkel's method).
//!
//! Implementing and verifying that engine (against a published worked
//! example, per this workspace's V&V philosophy) is tracked as follow-up
//! work, not done in this pass.

use crate::TampinesError;

/// Placeholder for the future cooling-tower calculation engine's entry
/// point. Not yet implemented -- see this module's doc for what it will
/// need to cover and why nothing exists here yet.
pub fn merkel_ntu_effectiveness() -> Result<(), TampinesError> {
    Err(TampinesError::NotYetImplemented {
        component: "cooling_tower::merkel_ntu_effectiveness",
    })
}
