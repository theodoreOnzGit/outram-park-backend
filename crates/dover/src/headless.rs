// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! Headless runner — deck in, CSV out, no window and no event loop.
//!
//! DOVER has no GUI, so unlike the `dhoby-ghaut` studios there is no headless
//! *mode* to switch into: this is the only way it runs. The workspace's
//! requirements for a headless path are met all the same, because they are
//! what make a result checkable rather than merely producible —
//! **deterministic** (no clock, no RNG, no I/O in the loop), **machine
//! readable** with a stable header, and **fixed precision** so a committed
//! fixture diffs cleanly.
//!
//! An unconverged case does not abort the run: its row carries the error in
//! the `status` column and the sweep continues. A sweep that stops at its
//! first failure tells you less than one that shows you where the failures
//! are.

use crate::deck::Deck;
use crate::smr::{SmrCase, SmrReaction};
use crate::species::Species;

/// The CSV header. Keep this and [`run_case`]'s row in lockstep — a fixture
/// diffs against both.
pub const CSV_HEADER: &str = "case,sweep_value,T_K,P_Pa,S_C,V_m3,tau_s,\
X_CH4,y_CH4,y_H2O,y_CO,y_CO2,y_H2,H2_per_CH4,duty_W,K_reform,K_wgs,status\n";

/// Run one operating point and format its CSV row.
///
/// `sweep_value` is the swept variable's value, or `f64::NAN` for a single
/// point (written as an empty field).
#[must_use]
pub fn run_case(name: &str, sweep_value: f64, case: &SmrCase) -> String {
    let sweep = if sweep_value.is_nan() {
        String::new()
    } else {
        format!("{sweep_value:.6}")
    };
    let k_ref = SmrReaction::Reforming.equilibrium_constant(case.temperature);
    let k_wgs = SmrReaction::WaterGasShift.equilibrium_constant(case.temperature);

    match case.solve() {
        Ok(o) => format!(
            "{name},{sweep},{:.4},{:.1},{:.4},{:.6},{:.6},{:.6},\
{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.4},{:.6e},{:.6e},ok\n",
            case.temperature,
            case.pressure,
            case.steam_to_carbon,
            case.volume,
            o.residence_time,
            o.methane_conversion,
            o.mole_fraction(Species::Methane),
            o.mole_fraction(Species::Steam),
            o.mole_fraction(Species::CarbonMonoxide),
            o.mole_fraction(Species::CarbonDioxide),
            o.mole_fraction(Species::Hydrogen),
            o.hydrogen_yield(case.methane_feed),
            o.heat_duty,
            k_ref,
            k_wgs,
        ),
        Err(e) => format!(
            "{name},{sweep},{:.4},{:.1},{:.4},{:.6},{:.6},,,,,,,,,{:.6e},{:.6e},ERROR:{}\n",
            case.temperature,
            case.pressure,
            case.steam_to_carbon,
            case.volume,
            case.residence_time(),
            k_ref,
            k_wgs,
            e.to_string().replace(',', ";"),
        ),
    }
}

/// Run every case a deck describes and return the whole CSV.
#[must_use]
pub fn run_deck(deck: &Deck) -> String {
    let mut out = String::from(CSV_HEADER);
    for (v, case) in deck.cases() {
        out.push_str(&run_case(&deck.case.name, v, &case));
    }
    out
}

/// Parse a deck from TOML text and run it.
///
/// # Errors
///
/// Propagates [`crate::deck::DeckError`] from parsing or validation — a bad
/// deck fails before any solving happens, rather than producing rows nobody
/// should trust.
pub fn run_toml(text: &str) -> Result<String, crate::deck::DeckError> {
    Ok(run_deck(&Deck::from_toml(text)?))
}
