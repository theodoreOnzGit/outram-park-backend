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

//! HTR-10 fuel-pebble **composition**, from Li, Yu & Wei (2014) Table 2.
//!
//! The geometry lives next door ([`TrisoSpec::HTR10_LI2014`](super::TrisoSpec),
//! [`PebbleParams::htr10_li2014`](crate::dh_universe::PebbleParams)); this
//! module is the material side, kept separate because **the composition is the
//! shared input to both the Monte Carlo and the deterministic ends** of the
//! neutronics. A multigroup or diffusion solver needs exactly these atom
//! densities; if it derived its own, the two would drift and the comparison
//! would be measuring the drift.
//!
//! # Source
//!
//! Li Wanlin, Yu Ganglin & Wei Chunlin, *"Research on Benchmark Calculation and
//! Analysis of HTR-10 with RMC Code"*, 7th International Topical Meeting on High
//! Temperature Reactor Technology (HTR 2014), Weihai, China, 27-31 October 2014.
//! **Table 2**, "Characteristics of HTR-10 fuel and moderator ball".
//!
//! # Three things Table 2 does not say
//!
//! Every one is an assumption a reader must be able to overrule, so each is a
//! named constant or an enum rather than a buried literal:
//!
//! 1. **The enrichment basis.** "Fuel enrichment 17 %" gives no basis.
//!    [`ENRICHMENT_WT`] takes it as **weight** percent, the industry convention.
//!    The table's own 5 g heavy-metal figure *cannot* arbitrate — it comes out
//!    4.99991 g on a weight reading against 4.99992 g on an atom reading — but
//!    the two differ by **1.06 % in U-235 number density**, which an eigenvalue
//!    does see.
//! 2. **The fuel ball's graphite density.** Table 2 states 1.73 g/cm3 only for
//!    the *moderator* ball. [`RHO_GRAPHITE`] applies it to the matrix and shell
//!    too.
//! 3. **The "ppm" basis.** Taken as by weight, of *natural* boron — see
//!    [`BoronReading`], which exists because this one is both easy to get wrong
//!    and expensive when you do.

use crate::material::material::{Material, NuclideComponent};

/// Avogadro's number \[1/mol\].
const NA: f64 = 6.022_140_76e23;
const M_U235: f64 = 235.043_930;
const M_U238: f64 = 238.050_788;
const M_O16: f64 = 15.994_914_6;
const M_C: f64 = 12.011;
const M_SI: f64 = 28.0855;
const M_B10: f64 = 10.0129;

/// Fuel enrichment as a **weight** fraction of U-235 in uranium (Table 2: 17 %).
///
/// See the module docs for why the basis is an assumption and what it costs.
pub const ENRICHMENT_WT: f64 = 0.17;
/// UO2 kernel density \[g/cm3\] (Table 2).
pub const RHO_UO2: f64 = 10.4;
/// Buffer (porous PyC) density \[g/cm3\] (Table 2, first of `1.1/1.9/3.18/1.9`).
pub const RHO_BUFFER: f64 = 1.1;
/// IPyC and OPyC density \[g/cm3\] (Table 2).
pub const RHO_PYC: f64 = 1.9;
/// SiC density \[g/cm3\] (Table 2).
pub const RHO_SIC: f64 = 3.18;
/// Graphite density \[g/cm3\] — Table 2 gives this for the **moderator ball**;
/// applied here to the fuel ball's matrix and shell as well. See the module docs.
pub const RHO_GRAPHITE: f64 = 1.73;
/// Natural boron in the uranium \[ppm by weight\] (Table 2).
pub const B_PPM_URANIUM: f64 = 4.0;
/// Natural boron in the graphite and moderator \[ppm by weight\] (Table 2).
pub const B_PPM_GRAPHITE: f64 = 1.3;

/// B-10 **weight** fraction of natural boron (19.9 at% B-10 / 80.1 at% B-11).
///
/// B-11 is left out of the compositions below: its absorption cross section is
/// ~0.005 b against B-10's ~3840 b, and at 1.3 ppm its scattering contributes
/// nothing. Only the absorber is modelled.
pub const B10_WEIGHT_FRACTION_OF_NATURAL_B: f64 = 0.184_3;

/// How the two "ppm" rows of Table 2 are read.
///
/// This is an **ablation knob**, not a modelling preference. Table 2 says
/// "natural boron content", and taking that as *elemental B-10* instead
/// over-absorbs by `1/0.1843` = 5.43x — a mistake the table's wording does
/// nothing to prevent. The arms exist so the cost is measured rather than
/// asserted; `examples/htr10_pebble_delta_tracking.rs` runs all four and
/// `tests/htr10_boron_ablation_control.rs` gates that they actually differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoronReading {
    /// Table 2 read as written: ppm by weight of **natural** boron, so only
    /// [`B10_WEIGHT_FRACTION_OF_NATURAL_B`] of it absorbs. The correct reading.
    Natural,
    /// Both impurity rows dropped — the "does boron matter at all" arm.
    None,
    /// Graphite's 1.3 ppm kept, the uranium's 4 ppm dropped.
    ///
    /// Isolates which row carries the worth. A 2200 m/s hand estimate says the
    /// kernel's is ~0.06 % of local absorption and the graphite's ~24 %, so this
    /// arm is predicted to be **indistinguishable from [`Self::Natural`]** —
    /// stated before measuring, so the measurement can falsify it.
    GraphiteOnly,
    /// The mistake: ppm read as **elemental B-10**, over-absorbing 5.43x.
    AsElementalB10,
}

impl BoronReading {
    /// A short label for tables and logs.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Natural => "natural B (as specified)",
            Self::None => "no boron at all",
            Self::GraphiteOnly => "graphite boron only",
            Self::AsElementalB10 => "ppm read as elemental B-10",
        }
    }

    /// The B-10 weight fraction applied to the stated ppm under this reading.
    #[must_use]
    pub fn b10_fraction(self) -> f64 {
        match self {
            Self::Natural | Self::GraphiteOnly => B10_WEIGHT_FRACTION_OF_NATURAL_B,
            Self::None => 0.0,
            Self::AsElementalB10 => 1.0,
        }
    }

    /// Natural-boron ppm applied to the **kernel** under this reading.
    #[must_use]
    pub fn kernel_ppm(self) -> f64 {
        match self {
            Self::None | Self::GraphiteOnly => 0.0,
            _ => B_PPM_URANIUM,
        }
    }

    /// Natural-boron ppm applied to the **graphite** under this reading.
    #[must_use]
    pub fn graphite_ppm(self) -> f64 {
        match self {
            Self::None => 0.0,
            _ => B_PPM_GRAPHITE,
        }
    }

    /// Every arm, for iterating an ablation study.
    #[must_use]
    pub fn all() -> [Self; 4] {
        [
            Self::Natural,
            Self::None,
            Self::GraphiteOnly,
            Self::AsElementalB10,
        ]
    }
}

/// Where each nuclide sits in the slice handed to the transport driver.
///
/// Indices, not names, because that is the convention
/// [`Material`] already uses — see
/// [`DhUniverse::material_at`](crate::dh_universe::DhUniverse::material_at).
#[derive(Debug, Clone, Copy)]
pub struct Htr10Nuclides {
    /// U-235.
    pub u235: usize,
    /// U-238.
    pub u238: usize,
    /// O-16.
    pub o16: usize,
    /// Free-gas carbon — the **ablation arm only**.
    ///
    /// Was the SiC layer's carbon until 2026-09-23. It is not that any more:
    /// SiC has its own bound thermal law and [`Self::c_sic`] carries it. This
    /// slot survives so `OUTRAM_HTR10_NO_SAB` can still strip every S(alpha,
    /// beta) and measure what they are worth.
    pub c_free: usize,
    /// Graphite-bound carbon (with S(alpha,beta)) — buffer, PyC, matrix, shell.
    ///
    /// Using free-gas carbon here would misrepresent the thermal spectrum a
    /// graphite-moderated pebble lives in. The distinction is not cosmetic.
    pub c_graphite: usize,
    /// Si-28, bound in SiC (with S(alpha,beta)).
    pub si28: usize,
    /// B-10 — the impurity absorber.
    pub b10: usize,
    /// Carbon bound in **SiC**, with the C-in-SiC S(alpha,beta).
    ///
    /// Added 2026-09-23. The SiC coating's carbon had been free-gas, which
    /// is the same error the doc on [`Self::c_graphite`] warns about one
    /// layer out: SiC is a crystal, its carbon is bound, and ENDF/B-VIII.0
    /// ships `tsl-CinSiC` (MAT 44) precisely so it need not be approximated.
    pub c_sic: usize,
    /// Si-29, bound in SiC.
    ///
    /// Added 2026-09-23. Natural silicon is 92.223 % Si-28, **4.685 % Si-29
    /// and 3.092 % Si-30**, and the model carried all of it as Si-28. The
    /// atom density was always computed from silicon's NATURAL molar mass,
    /// so the total was right and only the isotopic split was missing.
    pub si29: usize,
    /// Si-30, bound in SiC. See [`Self::si29`].
    pub si30: usize,
}

/// Natural silicon isotopic abundances, atom fractions (IUPAC).
///
/// The SiC atom density is built from silicon's natural molar mass, so these
/// split a total that is already correct rather than changing it.
pub const SI28_ATOM_FRACTION: f64 = 0.922_23;
/// See [`SI28_ATOM_FRACTION`].
pub const SI29_ATOM_FRACTION: f64 = 0.046_85;
/// See [`SI28_ATOM_FRACTION`].
pub const SI30_ATOM_FRACTION: f64 = 0.030_92;

/// Atom density \[atoms/b-cm\] from a mass density \[g/cm3\] and a molar mass.
#[must_use]
fn atom_density(rho: f64, molar: f64) -> f64 {
    rho * NA / molar * 1.0e-24
}

/// U-235 **atom** fraction implied by [`ENRICHMENT_WT`].
#[must_use]
pub fn u235_atom_fraction() -> f64 {
    let w = ENRICHMENT_WT;
    (w / M_U235) / ((w / M_U235) + ((1.0 - w) / M_U238))
}

/// B-10 atom density \[atoms/b-cm\] for `ppm` by weight of natural boron in a
/// host of density `rho` \[g/cm3\], under the given reading.
#[must_use]
pub fn b10_atom_density(rho_host: f64, ppm: f64, reading: BoronReading) -> f64 {
    atom_density(rho_host * ppm * 1.0e-6 * reading.b10_fraction(), M_B10)
}

/// The seven-material table for an HTR-10 fuel pebble, in the order
/// [`DhUniverse::pebble`](crate::dh_universe::DhUniverse::pebble) requires:
/// the five TRISO shells outward from the centre, then the fuel-zone matrix,
/// then the fuel-free outer shell.
///
/// `temperature_k` is stamped on every material; the paper runs at 27 C
/// (300.15 K).
#[must_use]
pub fn fuel_pebble_materials(
    n: Htr10Nuclides,
    boron: BoronReading,
    temperature_k: f64,
) -> Vec<Material> {
    let x5 = u235_atom_fraction();
    let m_u = x5 * M_U235 + (1.0 - x5) * M_U238;
    let m_uo2 = m_u + 2.0 * M_O16;
    let n_uo2 = atom_density(RHO_UO2, m_uo2);

    // Table 2 quotes the kernel's boron "of uranium", so it rides on the URANIUM
    // mass density inside the kernel, not the UO2 density.
    let rho_u = RHO_UO2 * m_u / m_uo2;
    let n_b10_kernel = b10_atom_density(rho_u, boron.kernel_ppm(), boron);
    let gr_b10 = |rho: f64| b10_atom_density(rho, boron.graphite_ppm(), boron);

    let mat = |id: i32, name: &str, comps: &[(usize, f64)]| Material {
        id,
        name: name.into(),
        temperature: temperature_k,
        components: comps
            .iter()
            .filter(|&&(_, density)| density > 0.0)
            .map(|&(nuclide_idx, atom_density)| NuclideComponent {
                nuclide_idx,
                atom_density,
            })
            .collect(),
    };

    let n_sic = atom_density(RHO_SIC, M_SI + M_C);
    let graphite = |id, name, rho: f64| {
        mat(
            id,
            name,
            &[(n.c_graphite, atom_density(rho, M_C)), (n.b10, gr_b10(rho))],
        )
    };

    vec![
        mat(
            0,
            "UO2 kernel",
            &[
                (n.u235, x5 * n_uo2),
                (n.u238, (1.0 - x5) * n_uo2),
                (n.o16, 2.0 * n_uo2),
                (n.b10, n_b10_kernel),
            ],
        ),
        graphite(1, "buffer PyC", RHO_BUFFER),
        graphite(2, "IPyC", RHO_PYC),
        // SiC: silicon split over its three natural isotopes, and the carbon
        // taken as BOUND in SiC rather than free gas. Both were wrong before
        // 2026-09-23 -- see `Htr10Nuclides::c_sic` and `::si29`.
        mat(
            3,
            "SiC",
            &[
                (n.si28, SI28_ATOM_FRACTION * n_sic),
                (n.si29, SI29_ATOM_FRACTION * n_sic),
                (n.si30, SI30_ATOM_FRACTION * n_sic),
                (n.c_sic, n_sic),
            ],
        ),
        graphite(4, "OPyC", RHO_PYC),
        graphite(5, "matrix graphite", RHO_GRAPHITE),
        graphite(6, "shell graphite", RHO_GRAPHITE),
    ]
}

#[cfg(test)]
mod tests;
