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

use petir::mathf::RealMath;
use crate::material::material::{Material, NuclideComponent};
use crate::material::material_sab::{assign_material_sab, MaterialSab};
use crate::material::nuclide::Nuclide;
use crate::material::thermal::ThermalScattering;
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
    r_lump / uranium_volume_fraction(c_per_u).r_cbrt()
}

/// What a nuclide of the five-nuclide table [`TAPES`] is, so that code asks
/// for "the carbon-12" and not for "slot 3" (GitHub #1635 item 3).
///
/// New work (2026-11-17), beside [`Mix`] and not in place of it:
/// [`Mix::densities`] keeps its positional order, and [`Mix::density_of`] and
/// [`RoleIndex`] are the role-keyed views of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    U234,
    U235,
    U238,
    C12,
    C13,
}

impl Role {
    /// Every role, in [`TAPES`] order.
    pub const ALL: [Role; 5] = [Role::U234, Role::U235, Role::U238, Role::C12, Role::C13];

    /// Position of this role's tape in [`TAPES`] (and of its density in
    /// [`Mix::densities`]): the one place the order is spelled out.
    pub const fn tape_index(self) -> usize {
        match self {
            Role::U234 => 0,
            Role::U235 => 1,
            Role::U238 => 2,
            Role::C12 => 3,
            Role::C13 => 4,
        }
    }

    /// True for the two carbons, the roles the graphite law binds to.
    pub fn is_carbon(self) -> bool {
        matches!(self, Role::C12 | Role::C13)
    }

    /// True for the three uranium isotopes.
    pub fn is_uranium(self) -> bool {
        !self.is_carbon()
    }
}

impl Mix {
    /// This mixture's atom density of one role \[atoms/barn-cm\]; the same
    /// numbers as [`Mix::densities`], asked for by role.
    pub fn density_of(&self, role: Role) -> f64 {
        self.densities()[role.tape_index()]
    }
}

/// Where each [`Role`]'s nuclide sits in the array that the materials
/// reference. With nothing bound ([`RoleIndex::free`]) that is [`TAPES`]
/// order; after [`bind_graphite`] the two carbons point at their bound copies,
/// which are appended after the table, so a density vector must be built
/// through this index, never by array position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoleIndex {
    slots: [usize; 5],
}

impl RoleIndex {
    /// Nothing bound: role `r` is nuclide `r.tape_index()`.
    pub fn free() -> Self {
        Self { slots: [0, 1, 2, 3, 4] }
    }

    /// Index of the nuclide that carries `role` in the materials' array.
    pub fn slot(&self, role: Role) -> usize {
        self.slots[role.tape_index()]
    }

    /// One atom density per nuclide of an array of `n_nuclides`: the mixture's
    /// density of each role on that role's slot, zero on every other nuclide
    /// (the free carbon the bound copy replaced, say).
    pub fn densities(&self, mix: &Mix, n_nuclides: usize) -> Vec<f64> {
        let mut d = vec![0.0; n_nuclides];
        for role in Role::ALL {
            d[self.slot(role)] = mix.density_of(role);
        }
        d
    }

    /// The per-nuclide values `per_nuclide` (an array of the same length as the
    /// nuclide array) read back by role.
    pub fn by_role<T: Copy>(&self, per_nuclide: &[T]) -> [T; 5] {
        [
            per_nuclide[self.slots[0]],
            per_nuclide[self.slots[1]],
            per_nuclide[self.slots[2]],
            per_nuclide[self.slots[3]],
            per_nuclide[self.slots[4]],
        ]
    }
}

/// The result of [`bind_graphite`].
#[derive(Debug, Clone)]
pub struct GraphiteBinding {
    /// The materials, carbon components repointed at the bound copies.
    pub materials: Vec<Material>,
    /// The table with the bound carbon copies appended after it.
    pub nuclides: Vec<Nuclide>,
    /// Where each role sits in [`Self::nuclides`].
    pub roles: RoleIndex,
}

/// Give every material that holds carbon the crystalline-graphite law on its
/// carbon, per material as OpenMC does (GitHub #462, #1635 item 3): the table
/// `nuclides` (the five of [`TAPES`] first, free) keeps its entries and its
/// indices, and [`assign_material_sab`] appends a bound copy of C-12 and of
/// C-13 for the materials that reference them. A material without carbon (a
/// uranium lump) is untouched. `law = None` binds nothing (the explicit
/// ablation): the materials and the table come back unchanged.
///
/// # Errors
///
/// Fewer than five nuclides, or whatever [`assign_material_sab`] refuses
/// (notably a carbon slot already bound by the deprecated nuclide-level call).
pub fn bind_graphite(
    materials: &[Material],
    nuclides: &[Nuclide],
    law: Option<&ThermalScattering>,
) -> Result<GraphiteBinding, String> {
    if nuclides.len() < TAPES.len() {
        return Err(format!("{} nuclides, the table has {}", nuclides.len(), TAPES.len()));
    }
    let Some(law) = law else {
        return Ok(GraphiteBinding {
            materials: materials.to_vec(),
            nuclides: nuclides.to_vec(),
            roles: RoleIndex::free(),
        });
    };
    let mut entries: Vec<MaterialSab> = Vec::new();
    for role in [Role::C12, Role::C13] {
        entries.extend(MaterialSab::for_materials_containing(materials, role.tape_index(), 0, &[]));
    }
    let (out_mats, out_nucs) =
        assign_material_sab(materials, nuclides, std::slice::from_ref(law), &entries)?;
    // The bound copy of a role: what the first material holding it now
    // references at the same component position (fraction 1 keeps positions).
    let mut roles = RoleIndex::free();
    for role in [Role::C12, Role::C13] {
        let free = role.tape_index();
        let found = materials.iter().enumerate().find_map(|(m, mat)| {
            mat.components
                .iter()
                .position(|c| c.nuclide_idx == free)
                .map(|k| out_mats[m].components[k].nuclide_idx)
        });
        if let Some(bound) = found {
            roles.slots[free] = bound;
        }
    }
    Ok(GraphiteBinding { materials: out_mats, nuclides: out_nucs, roles })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Five stand-in nuclides (the roles do not depend on the data) and the
    /// graphite law replaced by any law: the structure of the binding is what
    /// is under test. Pins that the carbon of every carbon-bearing material
    /// moves to a bound copy, the uranium and a carbon-free material do not,
    /// and a density vector built by role puts each density on the nuclide
    /// its material references (GitHub #1635 item 3).
    #[test]
    fn binding_graphite_moves_carbon_to_bound_copies_and_roles_follow() {
        use crate::material::nuclide::Nuclide;
        use njoy_outram_park_fork::leapr::decks::SabMaterial;
        let table: Vec<Nuclide> = (0..5).map(|_| Nuclide::from_core("H1").expect("core H1")).collect();
        let law = ThermalScattering::from_embedded_leapr_deck(SabMaterial::HInH2O, 296.0, "law")
            .expect("embedded law");
        let mix = natural_mix(600.0);
        let mats = vec![uranium_metal().material(1, "U"), graphite().material(2, "C"), mix.material(3, "mix")];

        // Free (ablation): nothing changes, roles are the positional order.
        let free = bind_graphite(&mats, &table, None).expect("free");
        assert_eq!(free.nuclides.len(), 5);
        assert_eq!(free.roles, RoleIndex::free());
        assert_eq!(free.roles.densities(&mix, 5), mix.densities().to_vec());

        let b = bind_graphite(&mats, &table, Some(&law)).expect("bound");
        assert_eq!(b.nuclides.len(), 7, "two bound carbon copies appended");
        assert_eq!((b.roles.slot(Role::C12), b.roles.slot(Role::C13)), (5, 6));
        for r in [Role::U234, Role::U235, Role::U238] {
            assert_eq!(b.roles.slot(r), r.tape_index());
        }
        assert!(b.nuclides[5].has_thermal_scattering() && b.nuclides[6].has_thermal_scattering());
        assert!(!b.nuclides[3].has_thermal_scattering() && !b.nuclides[4].has_thermal_scattering());
        // The uranium lump is untouched; graphite and mix reference the copies.
        let key = |m: &Material| m.components.iter().map(|c| (c.nuclide_idx, c.atom_density)).collect::<Vec<_>>();
        assert_eq!(key(&b.materials[0]), key(&mats[0]));
        for m in [1, 2] {
            for c in &b.materials[m].components {
                assert!(c.nuclide_idx != 3 && c.nuclide_idx != 4, "material {m} still holds a free carbon");
            }
        }
        // Role-keyed densities equal what the material holds, nuclide by nuclide.
        let d = b.roles.densities(&mix, b.nuclides.len());
        for c in &b.materials[2].components {
            assert_eq!(d[c.nuclide_idx], c.atom_density, "nuclide {}", c.nuclide_idx);
        }
        assert_eq!(d[3], 0.0);
        assert_eq!(d[4], 0.0);
        assert_eq!(b.roles.by_role(&d), mix.densities());
        // Too short a table is refused.
        assert!(bind_graphite(&mats, &table[..4], Some(&law)).is_err());
    }

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
