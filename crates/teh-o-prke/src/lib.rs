// `no_std` + `alloc` without the default `std` feature (gh:#1600, epic
// gh:#1585). Unit tests always build with `std`.
#![cfg_attr(not(any(feature = "std", test)), no_std)]
#![cfg_attr(not(feature = "std"), allow(dead_code, unused_imports))]

extern crate alloc;
extern crate ndarray_nostd as ndarray;
extern crate thiserror_nostd as thiserror;

/// What the `std` prelude gives every module and a `no_std` build has to
/// import: the `alloc` types and macros, and `petir::real::Real` for the
/// float methods `core` lacks (`sqrt`, `floor`, ...). Glob-imported by each
/// module in `no_std` builds only, so a `std` build resolves every name, and
/// computes every number, exactly as before.
#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
pub(crate) mod nostd_prelude {
    pub(crate) use alloc::borrow::ToOwned;
    pub(crate) use alloc::boxed::Box;
    pub(crate) use alloc::format;
    pub(crate) use alloc::string::{String, ToString};
    pub(crate) use alloc::vec;
    pub(crate) use alloc::vec::Vec;
    pub(crate) use petir::real::Real;
}
#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use nostd_prelude::*;

#[warn(missing_docs)]
/// contains structs for the zero power point reactor kinetics equations
///
/// the SixGroupPRKE struct contains the code which performs solution of
/// the PRKE matrix with six precursor groups
///
/// but you must supply reactivity (or keff equivalently) as an input.
///
/// for real-time calculations, only thermal reactors are okay
/// because the neutron generation time is on the order of 10E-4 s
/// for fast reactors, neutron generation time is on the order of 10E-8
/// but home computers calculate on the order of 1E-5s per calculation or
/// 1E-6s at best
///
/// Will probably need some other kind of method to calculate feedback
///
/// Convenience re-exports: `use teh_o_prke::prelude::*;`. This is the
/// intended entry point — see the module docs for the completeness rule.
pub mod prelude;

pub mod zero_power_prke;

/// Pure-Rust dense LU solver (`SquareMatrix`), inlined from `outram-foam-basic-lib`
/// so this crate has no `outram-foam-basic-lib` dependency — see the module doc.
pub mod matrix;

/// contains functions and structs for fuel temperature feedback
///
/// this is the simplest feedback mechanism
/// where rudimentary thermal hydraulics model is added.
pub mod fuel_temperature_feedback;

/// contains functions and structs for control rod feedback
pub mod control_rod_feedback;

/// error type for the crate
pub mod teh_o_prke_error;

/// contains code for various feedback mechanisms
/// this uses the six factor formula rather than simply adjusting reactivity
pub mod feedback_mechanisms;

/// contains code for decay heat simulation
/// the user can have up to seven groups
///
pub mod decay_heat;

/// ANS-5.1 decay-heat machinery (gh:#1080): standards enum, isotope mix
/// including Pu-241, conservatism factor F_gamma, user-supplied coefficients,
/// and the capture correction G(t).
pub mod ans_decay_heat;

/// `PointReactor` (gh:#1082): N-group kinetics, decay heat and an actinide
/// slot in one state, with the density -> fission rate -> power conversion.
pub mod point_reactor;

/// contains code for time stepping for prke
/// some algorithms copied from OpenFOAM
///
pub mod time_stepping;

/// analytical (closed-form) Nordheim-Fuchs exact timestepper for prompt
/// reactivity excursions with adiabatic fuel-temperature feedback -- a
/// real-time-friendly "Prompt Excursion Layer", distinct from (and much
/// cheaper than) the six-group precursor PRKE in [`zero_power_prke`].
pub mod nordheim_fuchs;

/// reusable "Delayed Neutron Layer" -- a reduced point-kinetics precursor
/// bank modelled as five first-order lags, one per delayed-neutron group.
/// Sits between the prompt-only [`nordheim_fuchs`] layer and a thermal-
/// hydraulics layer, restoring the delayed-neutron source that damps a
/// reactivity-power-temperature feedback loop.
pub mod delayed_neutron_layer;
