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

//! The group structures of the HTR-10 MGXS cases.
//!
//! Every case tallies once, on [`tally_structure`], and collapses onto each
//! [`Collapse`] afterwards. That only works because every coarse edge is an
//! edge of the tally structure; [`Collapse::structure`] is built from the
//! tally structure's own values so that holds by construction.

use crate::mgxs::GroupStructure;
use njoy_outram_park_fork::errorr::groups::neutron_group_structure;

/// Top of the tally structure \[eV\]. WIMS-69 stops at 10 MeV; a fission
/// spectrum does not, so one group from 10 to 20 MeV is added rather than
/// letting those neutrons fall outside every bin.
pub const TOP_EDGE_EV: f64 = 2.0e7;

/// NJOY's `ign` for the EPRI-CPM/WIMS 69-group structure (`groupr.f90`).
pub const WIMS69_IGN: i32 = 9;

/// The 70-group tally structure: NJOY's WIMS-69 edges (`ign = 9`, from the
/// `njoy-outram-park-fork` port of `gengpn`), ascending from 1e-5 eV, plus
/// [`TOP_EDGE_EV`].
///
/// WIMS-69 because it is a thermal-reactor structure with 45 groups below
/// 4 eV, so the graphite thermal spectrum and the low resonances are
/// resolved, and because every structure in [`Collapse`] nests in it.
///
/// # Panics
///
/// Never in practice: `ign = 9` is a built-in table of the port, checked by
/// `tally_structure_is_wims69_plus_one` below.
#[must_use]
pub fn tally_structure() -> GroupStructure {
    let mut edges =
        neutron_group_structure(WIMS69_IGN).expect("ign = 9 (WIMS-69) is a built-in NJOY table");
    edges.push(TOP_EDGE_EV);
    GroupStructure::new(edges).expect("WIMS-69 ascends and ends at 1e7 eV, below the 2e7 top")
}

/// A structure the fine library is collapsed onto.
///
/// None of these is chosen yet: #851 picks one from the measured MG-vs-CE
/// miss. Every edge is a WIMS-69 edge, so each is an exact collapse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Collapse {
    /// Thermal below 0.625 eV, fast above.
    Two,
    /// 0.625 eV, 9.118 keV, 0.821 MeV: the four-group split `dhoby-ghaut`'s
    /// workbench offers, all WIMS-69 edges.
    Four,
    /// The eight-group structure of `examples/htr10_deterministic_vs_mc.rs`
    /// (1e-5, 0.14, 0.625, 4, 29, 130, 9.12e3, 1.35e6, 2e7 eV) **snapped to
    /// the nearest WIMS-69 edges**: 29 → 27.7, 130 → 148.728,
    /// 9.12e3 → 9118, 1.35e6 → 1.353e6 eV. The example's own edges do not
    /// nest in WIMS-69, so its numbers are not on exactly this structure.
    Eight,
    /// The tally structure itself, 70 groups.
    Fine,
}

impl Collapse {
    /// Every structure a case reports, coarsest first.
    pub const ALL: [Collapse; 4] = [Collapse::Two, Collapse::Four, Collapse::Eight, Collapse::Fine];

    /// Short label for tables and file names.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Two => "2g",
            Self::Four => "4g",
            Self::Eight => "8g-wims",
            Self::Fine => "70g",
        }
    }

    /// The interior edges \[eV\] (the outer two are the tally structure's).
    fn interior_ev(self) -> &'static [f64] {
        match self {
            Self::Two => &[0.625],
            Self::Four => &[0.625, 9118.0, 8.21e5],
            Self::Eight => &[0.14, 0.625, 4.0, 27.7, 148.728, 9118.0, 1.353e6],
            Self::Fine => &[],
        }
    }

    /// The structure, with every edge taken from [`tally_structure`]'s own
    /// values (matched to a relative 1e-6), so it nests exactly.
    ///
    /// # Panics
    ///
    /// If an interior edge is not a WIMS-69 edge. The edges are constants
    /// checked by `every_collapse_nests_in_the_tally_structure`, so this is a
    /// defect in this file, not a runtime condition.
    #[must_use]
    pub fn structure(self) -> GroupStructure {
        let fine = tally_structure();
        if self == Self::Fine {
            return fine;
        }
        let f = fine.edges();
        let mut edges = vec![f[0]];
        for &e in self.interior_ev() {
            let hit = f
                .iter()
                .copied()
                .find(|&x| (x - e).abs() <= 1.0e-6 * e)
                .unwrap_or_else(|| panic!("{e} eV is not a WIMS-69 edge"));
            edges.push(hit);
        }
        edges.push(f[f.len() - 1]);
        GroupStructure::new(edges).expect("interior edges are listed ascending")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 70 groups, 1e-5 eV to 20 MeV, with WIMS-69's 0.625 eV and 1 MeV-range
    /// edges where NJOY puts them.
    #[test]
    fn tally_structure_is_wims69_plus_one() {
        let g = tally_structure();
        assert_eq!(g.n_groups(), 70);
        let e = g.edges();
        assert_eq!(e[0], 1.0e-5);
        assert_eq!(e[e.len() - 2], 1.0e7);
        assert_eq!(e[e.len() - 1], TOP_EDGE_EV);
        assert!(e.contains(&0.625) && e.contains(&9118.0) && e.contains(&1.353e6));
    }

    /// Every collapse target is accepted by the exact collapse, and has the
    /// group count its name says.
    #[test]
    fn every_collapse_nests_in_the_tally_structure() {
        let fine = tally_structure();
        for (c, n) in Collapse::ALL.iter().zip([2, 4, 8, 70]) {
            let s = c.structure();
            assert_eq!(s.n_groups(), n, "{}", c.label());
            for e in s.edges() {
                assert!(fine.edges().contains(e), "{}: {e} not a fine edge", c.label());
            }
        }
    }
}
