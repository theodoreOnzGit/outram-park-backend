// Copyright [2023] [Theodore Kay Chen Ong, Professor Per F. Peterson,
// University of California, Berkeley
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
pub mod alpha_nightly;
pub mod beta_testing;
pub mod stable;

/// Convenience re-exports: `use chem_eng_real_time_process_control_simulator::prelude::*;`
///
/// The tiered preludes (`alpha_nightly::prelude`, `beta_testing::prelude`,
/// `stable::prelude`) remain available for callers who want to pin one. This
/// module exists because the *plain* path is what a caller writes first, and
/// without it that path did not resolve at all — the crate's own examples all
/// reach for `alpha_nightly::prelude::*`, which is not a name anyone guesses.
///
/// It forwards to `alpha_nightly`, which is currently the only populated tier.
/// When `beta_testing` or `stable` is filled in, re-point this at the most
/// stable populated tier rather than adding a fourth thing to choose between.
///
/// ```
/// use chem_eng_real_time_process_control_simulator::prelude::*;
///
/// // Transfer-function types resolve from the prelude alone.
/// fn takes_tf(_: &TransferFn, _: &TransferFnFirstOrder, _: &TransferFnSecondOrder) {}
/// ```
pub mod prelude {
    pub use crate::alpha_nightly::prelude::*;
}
