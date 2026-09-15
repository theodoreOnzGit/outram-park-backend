//! Real-time educational simulator prototypes for the CIET three-branch
//! plus DRACS loop.
//!
//! Each `version_*` submodule is a successive iteration of the loop
//! solver used as a playable educational simulator (not a validated
//! model):
//!
//! - [`version_1`] — mass flowrates solved serially; no CTAH PID
//!   control yet.
//! - [`version_2`] — adds CTAH (and TCHX) PID control, still single
//!   threaded.
//! - [`version_3`] — adds thread-based parallelism over version 2 to
//!   run the loop faster.
//!
//! [`regression_tests`] guards that these changes still reproduce the
//! expected physics (e.g. the natural-circulation loop). Time is in
//! seconds, temperatures in degC, mass flow in kg/s and power in W in
//! these prototype drivers.

/// this ensures that despite all the changes, the three branch
/// ciet should still reproduce results
///
/// eg. natural circulation loop
///
#[cfg(test)]
pub mod regression_tests;

/// this function runs ciet ver 1 test,
/// mass flowrates are calculated serially
/// for simplicity
///
/// it works well enough, but the CTAH branch controller
/// is not configured yet
#[cfg(test)]
pub mod version_1;

#[cfg(test)]
pub use version_1::*;

/// version 2 adds CTAH control capabilities
/// to the simulator
///
/// It runs in a single threaded manner.
///
#[cfg(test)]
pub mod version_2;

/// version 3 adds parallelism to version 2,
/// hopefully to get it faster
pub mod version_3;
