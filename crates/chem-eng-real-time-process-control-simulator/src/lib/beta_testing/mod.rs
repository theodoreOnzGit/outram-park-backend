// Copyright [2023] [Theodore Kay Chen Ong, Professor Per F. Peterson,
// Thermal Hydraulics Lab, Repository Contributors and
// Singapore Nuclear Research and Safety Initiative (SNRSI)]
//
// SPDX-License-Identifier: GPL-3.0-only
//
// Relicensed from Apache-2.0 to GPL-3.0-only on 2026-08-11 by the sole
// copyright holder (maintainer-directed) — see the crate NOTICE file.
// Versions of this crate published to crates.io before the relicense
// remain available under Apache-2.0.
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, version 3 of the License.
//
// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with this program.  If not, see <https://www.gnu.org/licenses/>.
pub mod prelude;
pub(crate) mod stable_transfer_functions;
pub(crate) mod controllers;
pub mod errors;
pub mod transfer_fn_wrapper_and_enums;
pub mod z_domain;

use uom::si::{Quantity, ISQ, SI};
use uom::typenum::*;
pub(crate) type TimeSquared = Quantity<ISQ<Z0, Z0, P2, Z0, Z0, Z0, Z0>, SI<f64>, f64>;

// Time squared unit for use in second order functions

#[test]
pub fn timesq_test() {
    // this just tests the time squared unit
    use uom::si::{time::second, f64::Time};

    let a = Time::new::<second>(1.0);
    let a_sq: TimeSquared = a * a;
    assert_eq!(a * a, a_sq);
}
