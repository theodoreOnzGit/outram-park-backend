// SPDX-License-Identifier: GPL-3.0
//
// NOT a port. boon-lay's own extensions of the TRISO-ATOPS model, written
// 2026-10-05 (gh:#583). They are built ON the ported code in
// `crate::triso_atops_fork` (a fork of INL's TRISO-ATOPS, MIT, commit de374c8)
// and call it unchanged; nothing in `triso_atops_fork` is modified by them.
// Upstream has no counterpart to compare them with code to code, so each one
// is verified by reducing EXACTLY to the ported model when given upstream's
// inputs (see each module's tests).

//! # TRISO-ATOPS extensions (not a port)
//!
//! [`crate::triso_atops_fork`] is a faithful port of INL's TRISO-ATOPS, and it
//! stays faithful: code-to-code agreement with upstream is its contract. Some
//! questions need more than upstream's model can express. This module holds
//! boon-lay's answers to them, **kept apart from the port** so that:
//!
//! - nobody mistakes an extension for upstream behaviour, or the reverse;
//! - the port can be re-verified against upstream without the extensions in the
//!   way;
//! - each extension states what it adds, and proves it reduces to the port when
//!   that addition is switched off.
//!
//! ## The extensions
//!
//! | Module | What it adds | Reduces to the port when |
//! |---|---|---|
//! | [`removal_rates`] | Plate-out and purification rates **per transport group, or per element** ([`RemovalRates`]), instead of one `k_plate` and one `k_clean` per reactor; conversions from per-cycle fractions and purification efficiencies; Liu & Cao's (2002) cited HTR-10 per-cycle data | [`RemovalRates::upstream`] |
//! | [`normal_operation`] | [`normal_operation_node_with_rates`]: the ported per-node normal-operation chain with the rate routing taken from a [`RemovalRates`] | given [`RemovalRates::upstream`], bit for bit for every one of the 84 supported nuclides |
//!
//! The leak sink and the live stepping of the pools from any state are
//! another extension that predates this module and lives beside the port:
//! [`crate::triso_atops_fork::activities::live_pools`] (gh:#399).
//! [`RemovalRates::pool_rates`] connects the two.
//!
//! ## Why the rates needed extending (gh:#583)
//!
//! Upstream reads one `k_plate` and one `k_clean` per reactor
//! (`run_functions.py`, lines 164 and 192). The element only switches each one
//! on or off: noble gases do not plate out, and the purification system
//! takes noble gases and halogens only (`trisoatops.py`, lines 118–124).
//! Iodine, caesium, strontium and silver therefore plate out at one rate. HTR-10's
//! own source gives element-specific values instead: Liu & Cao (2002), §2.4.1,
//! with plate-out per cycle of 20 % for I, 30 % for Rb and Sr, 50 % for Ag and
//! Cs, and purification efficiencies of 99 % for I, Kr, Xe, C and H-3 and
//! 90 % for Sr, Ag, Cs and Rb ([`removal_rates::liu_cao_2002_htr10`]).
//!
//! **What this does not supply.** Turning those per-cycle figures into rate
//! constants needs the helium's circuit cycle time and the fraction of the flow
//! sent through the purification system. Neither is in the workspace's record
//! of the paper, so both are **required arguments**, never defaults
//! ([`RemovalRates::from_per_cycle`]).
//!
//! Research, education and V&V only (`RESPONSIBLE_USE.md`): not for reactor
//! operation, licensing, safety decisions or emergency response.

pub mod normal_operation;
pub mod removal_rates;

pub use normal_operation::normal_operation_node_with_rates;
pub use removal_rates::{
    liu_cao_2002_htr10, purification_rate, rate_from_fraction_per_cycle, GroupRates,
    PerCycleRemoval, RemovalRates,
};
