// SPDX-License-Identifier: GPL-3.0-only

//! **Uranium and graphite at 296 K: the compositions of tutorial rungs 2 and 3**
//! (GitHub #524, #525).
//!
//! One home for the numbers so that `examples/ugraphite_four_factor.rs`,
//! `examples/lumped_ugraphite_kinf.rs` and the `ugraphite` / `lumped` rungs of
//! the `dhoby-ghaut` Monte Carlo web demo run **the same** mixtures; nothing is
//! retyped (the pattern of [`super::godiva`]). Pure arithmetic, no I/O.
//!
//! - **Main case of rung 2** ([`htr10_pebble_mix`]): the uranium and carbon of
//!   one HTR-10 fuel pebble, smeared over the 3 cm ball, from the published
//!   specification in [`crate::pebble_beds::htr10`] (Li, Yu & Wei 2014
//!   Table 2; IAEA-TECDOC-1382 Table 4-2 for the 17 wt% basis and the
//!   1.73 g/cm3 graphite) and [`TrisoSpec::HTR10_LI2014`]. O, Si and B are left
//!   out on purpose (the rung is uranium and graphite). `N_C/N_U` = 767.2.
//! - **Step 7 sweep and rung 3** ([`natural_mix`], [`uranium_metal`]):
//!   natural uranium in graphite at 1.73 g/cm3, and natural uranium metal at
//!   [`RHO_U_METAL`] for the lumps.
//!
//! The recorded results these compositions produced are in the two examples'
//! module docs and in `verification_and_validation/tutorial_rung2/README.md`
//! and `tutorial_rung3/README.md`.

use crate::material::material::{Material, NuclideComponent};
use crate::pebble_beds::fhr_pebble::TrisoSpec;
use crate::pebble_beds::htr10::{
    u235_atom_fraction, C12_ATOM_FRACTION_OF_NATURAL_C, C13_ATOM_FRACTION_OF_NATURAL_C,
    RHO_BUFFER, RHO_GRAPHITE, RHO_PYC, RHO_SIC, RHO_UO2,
};

/// Temperature of every material and nuclide in rungs 2 and 3 \[K\]: the
/// lowest tabulated temperature of the crystalline-graphite S(alpha,beta)
/// law, so no thermal-law interpolation enters.
pub const TEMPERATURE_K: f64 = 296.0;

/// Natural uranium, atom fractions U-234, U-235, U-238: the IUPAC
/// representative isotopic composition (0.0054 %, 0.7204 %, 99.2742 %).
/// Recalled, not page-checked in the change that added it.
pub const NAT_U: [f64; 3] = [0.000_054, 0.007_204, 0.992_742];

/// Natural uranium metal density \[g/cm3\], the lumps of rung 3. A handbook
/// value, not page-checked.
pub const RHO_U_METAL: f64 = 19.05;

/// The ENDF/B-VIII.0 tapes, `(nuclide name, file in reference-data/endf/)`, in
/// the order of [`Mix::densities`], followed by the graphite thermal law.
pub const TAPES: [(&str, &str); 5] = [
    ("U234", "n-092_U_234-ENDF8.0.endf"),
    ("U235", "n-092_U_235-ENDF8.0.endf"),
    ("U238", "n-092_U_238.endf"),
    ("C12", "n-006_C_012-ENDF8.0.endf"),
    ("C13", "n-006_C_013-ENDF8.0.endf"),
];
/// Crystalline graphite S(alpha,beta), MAT 30 (OpenMC's `c_Graphite`),
/// attached to C-12 and C-13.
pub const GRAPHITE_TSL: (&str, i32) = ("tsl-crystalline-graphite.endf", 30);

/// Avogadro's number times 1e-24 \[atoms cm2 / (mol barn)\].
pub const NA_B: f64 = 0.602_214_076;
const M_U234: f64 = 234.040_952;
const M_U235: f64 = 235.043_930;
const M_U238: f64 = 238.050_788;
const M_O16: f64 = 15.994_914_6;
const M_C: f64 = 12.011;
const M_SI: f64 = 28.0855;

/// A U + C composition \[atoms/barn-cm\]. Either part may be zero (a pure
/// uranium lump, a pure graphite moderator).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mix {
    /// U-234, U-235, U-238.
    pub u: [f64; 3],
    /// Natural carbon (split 98.93 / 1.07 at% into C-12 / C-13).
    pub c: f64,
}

impl Mix {
    /// Carbon atoms per uranium atom.
    pub fn c_per_u(&self) -> f64 {
        self.c / self.u.iter().sum::<f64>()
    }

    /// Atom densities in [`TAPES`] order: U-234, U-235, U-238, C-12, C-13.
    pub fn densities(&self) -> [f64; 5] {
        [
            self.u[0],
            self.u[1],
            self.u[2],
            C12_ATOM_FRACTION_OF_NATURAL_C * self.c,
            C13_ATOM_FRACTION_OF_NATURAL_C * self.c,
        ]
    }

    /// The material, with nuclide indices in [`TAPES`] order and zero
    /// densities dropped, at [`TEMPERATURE_K`].
    pub fn material(&self, id: i32, name: &str) -> Material {
        Material {
            id,
            name: name.into(),
            temperature: TEMPERATURE_K,
            components: self
                .densities()
                .iter()
                .enumerate()
                .filter(|(_, d)| **d > 0.0)
                .map(|(i, d)| NuclideComponent {
                    nuclide_idx: i,
                    atom_density: *d,
                })
                .collect(),
        }
    }

    /// The same atoms scaled by `factor` (used to homogenise a cell).
    pub fn scaled(&self, factor: f64) -> Mix {
        Mix {
            u: [self.u[0] * factor, self.u[1] * factor, self.u[2] * factor],
            c: self.c * factor,
        }
    }
}

/// One HTR-10 fuel pebble's inventory, as [`htr10_pebble_mix`] computes it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PebbleInventory {
    /// TRISO particles (8335 by construction of the packing fraction).
    pub particles: f64,
    /// Uranium mass \[g\] (5.0 g in the specification).
    pub uranium_g: f64,
    /// Carbon mass \[g\]: matrix, shell, buffer, both PyC layers, SiC carbon.
    pub carbon_g: f64,
    /// Carbon atoms per uranium atom.
    pub c_per_u: f64,
}

/// One HTR-10 fuel pebble's uranium and carbon, smeared over the 3 cm ball,
/// and the inventory it came from. See the module docs.
pub fn htr10_pebble_mix() -> (Mix, PebbleInventory) {
    let spec = TrisoSpec::HTR10_LI2014;
    let (r_fz, r_peb) = (2.5_f64, 3.0_f64);
    let ball = |r: f64| 4.0 / 3.0 * core::f64::consts::PI * r * r * r;
    let v = spec.layer_volumes(); // kernel, buffer, IPyC, SiC, OPyC
    let v_particle: f64 = v.iter().sum();
    let n_particles = (spec.packing_fraction * ball(r_fz) / v_particle).round();
    let v_matrix = ball(r_fz) - n_particles * v_particle;
    let v_shell = ball(r_peb) - ball(r_fz);

    let x5 = u235_atom_fraction();
    let m_u = x5 * M_U235 + (1.0 - x5) * M_U238;
    let mol_u = n_particles * v[0] * RHO_UO2 / (m_u + 2.0 * M_O16);
    let mol_c = (v_matrix + v_shell) * RHO_GRAPHITE / M_C
        + n_particles
            * (v[1] * RHO_BUFFER / M_C
                + (v[2] + v[4]) * RHO_PYC / M_C
                + v[3] * RHO_SIC / (M_C + M_SI));
    let v_peb = ball(r_peb);
    let n_u = mol_u * NA_B / v_peb;
    (
        Mix {
            u: [0.0, x5 * n_u, (1.0 - x5) * n_u],
            c: mol_c * NA_B / v_peb,
        },
        PebbleInventory {
            particles: n_particles,
            uranium_g: mol_u * m_u,
            carbon_g: mol_c * M_C,
            c_per_u: mol_c / mol_u,
        },
    )
}

/// Graphite at 1.73 g/cm3 ([`RHO_GRAPHITE`]) \[atoms/b-cm\].
pub fn graphite_density() -> f64 {
    RHO_GRAPHITE * NA_B / M_C
}

/// Natural uranium in graphite at 1.73 g/cm3 carbon, `c_per_u` carbon atoms
/// per uranium atom. (In an infinite homogeneous medium `k_inf` depends only
/// on the ratios, not on the absolute density.)
pub fn natural_mix(c_per_u: f64) -> Mix {
    let n_c = graphite_density();
    let n_u = n_c / c_per_u;
    Mix {
        u: [NAT_U[0] * n_u, NAT_U[1] * n_u, NAT_U[2] * n_u],
        c: n_c,
    }
}

/// Natural uranium metal at [`RHO_U_METAL`] \[atoms/b-cm\].
pub fn uranium_metal() -> Mix {
    let m_u = NAT_U[0] * M_U234 + NAT_U[1] * M_U235 + NAT_U[2] * M_U238;
    let n_u = RHO_U_METAL * NA_B / m_u;
    Mix {
        u: [NAT_U[0] * n_u, NAT_U[1] * n_u, NAT_U[2] * n_u],
        c: 0.0,
    }
}

/// Pure graphite at 1.73 g/cm3, as a [`Mix`].
pub fn graphite() -> Mix {
    Mix {
        u: [0.0; 3],
        c: graphite_density(),
    }
}

/// Uranium volume fraction of a cell whose uranium-metal lump and pure
/// graphite give a cell-average `c_per_u`: `(1 - v) N_C / (v N_U) = c_per_u`.
pub fn uranium_volume_fraction(c_per_u: f64) -> f64 {
    let n_u: f64 = uranium_metal().u.iter().sum();
    1.0 / (1.0 + c_per_u * n_u / graphite_density())
}

/// Wigner-Seitz cell radius for a lump of radius `r_lump` at cell-average
/// `c_per_u` \[cm\].
pub fn cell_radius(r_lump: f64, c_per_u: f64) -> f64 {
    r_lump / uranium_volume_fraction(c_per_u).cbrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pebble closes on the specification's own 5 g of heavy metal and
    /// 8335 particles, and the ratio the records quote is reproduced.
    #[test]
    fn pebble_inventory_matches_the_specification() {
        let (mix, inv) = htr10_pebble_mix();
        assert_eq!(inv.particles, 8335.0);
        assert!((inv.uranium_g - 5.0).abs() < 1e-3, "{}", inv.uranium_g);
        assert!((inv.c_per_u - 767.2).abs() < 0.05, "{}", inv.c_per_u);
        assert!((mix.c_per_u() - inv.c_per_u).abs() < 1e-9);
    }

    /// The homogenised lumped cell has the same atom ratio as the
    /// homogeneous mixture it is compared with.
    #[test]
    fn homogenised_cell_has_the_requested_ratio() {
        for cu in [100.0, 600.0, 2500.0] {
            let v = uranium_volume_fraction(cu);
            let hom = Mix {
                u: uranium_metal().scaled(v).u,
                c: graphite().c * (1.0 - v),
            };
            assert!((hom.c_per_u() - cu).abs() < 1e-9 * cu);
            assert!((natural_mix(cu).c_per_u() - cu).abs() < 1e-9 * cu);
        }
    }
}
