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

//! # HTTR nuclides and materials
//!
//! The 48 nuclide slots ([`nuc`]) and 27 materials ([`mat`]) of the HTTR
//! start-up core, with every atom density from R1 §3.1.3 (Tables 3.1-3.9)
//! and the isotopic splits of R1's sample deck (App. A.1.8, material cards
//! m1-m27). See [`super::spec`] for what R1 is.
//!
//! **Three departures from R1's specification, each physical and stated:**
//!
//! 1. **Boron impurities carry B-11 as well as B-10.** R1 specifies each
//!    impurity as an equivalent *natural-boron* content but models only its
//!    B-10 (R1 §2.1, §3.1.1). Natural boron is 19.9 at.% B-10 and 80.1 at.%
//!    B-11 ([`B11_PER_B10`]), so B-11 is added beside every impurity's B-10,
//!    as the HTR-10 model does (gh:#311). Predicted worth: nil at this
//!    resolution (B-11 absorbs ~5 mb; the impurity is ~1e-8 atoms/(b cm)).
//!    [`HttrMaterialConfig::without_impurity_b11`] is the ablation back to
//!    R1's specification.
//! 2. **Carbon is natural carbon**, C-12 and C-13 at 98.93 / 1.07 at.%
//!    (IUPAC), where R1's deck uses elemental C-nat (ENDF/B-VIII.0 has no
//!    elemental carbon). The same as HTR-10 (gh:#425).
//! 3. **Thermal binding follows the material, not the deck's one law.** The
//!    deck puts graphite S(α,β) on every carbon, the SiC's included. Here
//!    the SiC carries ENDF/B-VIII.0's C-in-SiC and Si-in-SiC laws, and the
//!    control rods' B4C/C absorber splits its carbon: the stoichiometric
//!    B4C carbon (a quarter of the boron atoms) is free, the rest of the
//!    compact's carbon is graphite (R1 Table 1.15: sintered B4C/C). Alloy
//!    800H's carbon and silicon are free; its Fe-56 and Al-27 carry the
//!    iron and aluminium metal laws, as the deck's `mt24 Fe.00t Al.00t`.
//!
//! O-18 is not modelled separately: R1's oxygen total (Table 3.1) is split
//! by the deck into O-16 and O-17 only, so O-18's 0.2 % rides on O-16.

use outram_mc_libs::material::material::{Material, NuclideComponent};

use crate::htr10_rmc::data::{DataDir, NuclideSlot, Tape, ThermalLaw};
use crate::htr10_rmc::materials::GraphiteLaw;
use outram_mc_libs::pebble_beds::htr10::{
    C12_ATOM_FRACTION_OF_NATURAL_C, C13_ATOM_FRACTION_OF_NATURAL_C,
};

/// B-11 atoms per B-10 atom in natural boron, 0.801 / 0.199 (the 19.9 at.%
/// B-10 R1 itself uses, R1 §3.1.1).
pub const B11_PER_B10: f64 = 0.801 / 0.199;

/// Nuclide slot indices. Each slot is one nuclide with one thermal
/// treatment, so carbon and silicon appear more than once.
pub mod nuc {
    /// U-234 (U-in-UO2 law).
    pub const U234: usize = 0;
    /// U-235 (U-in-UO2 law).
    pub const U235: usize = 1;
    /// U-238 (U-in-UO2 law).
    pub const U238: usize = 2;
    /// O-16 (O-in-UO2 law).
    pub const O16: usize = 3;
    /// O-17 (free; the O-in-UO2 law is O-16's, as OpenMC binds it).
    pub const O17: usize = 4;
    /// C-12 bound in graphite.
    pub const C12_GRAPHITE: usize = 5;
    /// C-13 bound in graphite.
    pub const C13_GRAPHITE: usize = 6;
    /// C-12 bound in SiC.
    pub const C12_SIC: usize = 7;
    /// C-13 bound in SiC.
    pub const C13_SIC: usize = 8;
    /// C-12 free (B4C carbon, steel carbon).
    pub const C12_FREE: usize = 9;
    /// C-13 free.
    pub const C13_FREE: usize = 10;
    /// B-10.
    pub const B10: usize = 11;
    /// B-11.
    pub const B11: usize = 12;
    /// Si-28 bound in SiC.
    pub const SI28_SIC: usize = 13;
    /// Si-29 bound in SiC.
    pub const SI29_SIC: usize = 14;
    /// Si-30 bound in SiC.
    pub const SI30_SIC: usize = 15;
    /// Si-28 free (Alloy 800H).
    pub const SI28: usize = 16;
    /// Si-29 free.
    pub const SI29: usize = 17;
    /// Si-30 free.
    pub const SI30: usize = 18;
    /// He-3.
    pub const HE3: usize = 19;
    /// He-4.
    pub const HE4: usize = 20;
    /// Al-27 (aluminium metal law).
    pub const AL27: usize = 21;
    /// P-31.
    pub const P31: usize = 22;
    /// S-32, S-33, S-34, S-36.
    pub const S: [usize; 4] = [23, 24, 25, 26];
    /// Ti-46 to Ti-50.
    pub const TI: [usize; 5] = [27, 28, 29, 30, 31];
    /// Cr-50, 52, 53, 54.
    pub const CR: [usize; 4] = [32, 33, 34, 35];
    /// Mn-55.
    pub const MN55: usize = 36;
    /// Fe-54, Fe-56 (iron metal law), Fe-57, Fe-58.
    pub const FE: [usize; 4] = [37, 38, 39, 40];
    /// Ni-58, 60, 61, 62, 64.
    pub const NI: [usize; 5] = [41, 42, 43, 44, 45];
    /// Cu-63, Cu-65.
    pub const CU: [usize; 2] = [46, 47];
    /// Number of slots.
    pub const COUNT: usize = 48;
}

/// Material indices, in the order of R1's deck (m1-m27, less one).
pub mod mat {
    /// The twelve UO2 kernels, `KERNEL0 + i` for enrichment
    /// `spec::ENRICHMENTS[i]` (deck m1-m12).
    pub const KERNEL0: usize = 0;
    /// Buffer (m13).
    pub const BUFFER: usize = 12;
    /// Inner pyrolytic carbon (m14).
    pub const IPYC: usize = 13;
    /// Silicon carbide (m15).
    pub const SIC: usize = 14;
    /// Outer pyrolytic carbon (m16).
    pub const OPYC: usize = 15;
    /// Resinated-graphite overcoat (m17).
    pub const OVERCOAT: usize = 16;
    /// Fuel-compact matrix (m18).
    pub const MATRIX: usize = 17;
    /// IG-110 fuel sleeve and end caps (m19).
    pub const SLEEVE: usize = 18;
    /// Burnable poison, 2.0 wt.% natural boron (m20).
    pub const BP_2_0: usize = 19;
    /// Burnable poison, 2.5 wt.% (m21).
    pub const BP_2_5: usize = 20;
    /// Graphite disks between the BP stacks (m22).
    pub const DISKS: usize = 21;
    /// B4C/C control-rod absorber (m23).
    pub const ABSORBER: usize = 22;
    /// Alloy 800H (m24).
    pub const ALLOY_800H: usize = 23;
    /// IG-110 block graphite, every column (m25).
    pub const BLOCK: usize = 24;
    /// PGX permanent reflector (m26).
    pub const PGX: usize = 25;
    /// Helium (m27).
    pub const HELIUM: usize = 26;
    /// Number of materials.
    pub const COUNT: usize = 27;
    /// The materials inside the fuel compact's delta-tracked region: the
    /// kernels, the coatings, the overcoat and the matrix.
    pub const COMPACT: [usize; 18] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17];
}

/// How thermal scattering is treated (the free-gas arm is the harness
/// check, as for HTR-10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HttrThermal {
    /// Every bound law (default).
    #[default]
    Bound,
    /// ABLATION: every nuclide a free gas.
    FreeGas,
}

/// The 48 nuclide slots in [`nuc`] order, every one ENDF/B-VIII.0, with
/// the thermal law each carries, and the modelling notes a run records.
///
/// The graphite law defaults to `GraphiteLaw::Reactor30P` for the reason
/// HTR-10 chose it (maintainer, 2026-09-27): IG-110 is 1.75-1.77 g/cm³ and
/// PGX 1.73 (R1 Tables 1.13, 1.19, 1.27) against crystal graphite's ~2.25,
/// i.e. ~22 % porous, and 30 % is the nearer of ENDF/B-VIII.0's two
/// porous laws. R1's own results used ENDF/B-VI.8 to VII.0 graphite, which
/// is crystalline: `GraphiteLaw::Crystalline` is the like-for-like ablation.
#[must_use]
pub fn httr_nuclide_slots(graphite: GraphiteLaw, thermal: HttrThermal) -> (Vec<NuclideSlot>, Vec<String>) {
    let bound = thermal == HttrThermal::Bound;
    let law = |l: ThermalLaw| bound.then_some(l);
    let gr = law(ThermalLaw::Graphite {
        file: graphite.tape(),
        mat: graphite.mat(),
    });
    let e = Tape::endf;
    let ni = |file: &'static str| Tape {
        dir: DataDir::AceSubmoduleEndfB8,
        file,
    };
    let s = |name: &'static str, tape: Tape, thermal: Option<ThermalLaw>| NuclideSlot {
        name,
        tape,
        thermal,
    };
    let c12 = e("n-006_C_012-ENDF8.0.endf");
    let c13 = e("n-006_C_013-ENDF8.0.endf");
    let slots = vec![
        s("U234", e("n-092_U_234-ENDF8.0.endf"), law(ThermalLaw::UInUO2)),
        s("U235", e("n-092_U_235-ENDF8.0.endf"), law(ThermalLaw::UInUO2)),
        s("U238", e("n-092_U_238.endf"), law(ThermalLaw::UInUO2)),
        s("O16", e("n-008_O_016-ENDF8.0.endf"), law(ThermalLaw::OInUO2)),
        s("O17", e("n-008_O_017-ENDF8.0.endf"), None),
        s("C12", c12, gr),
        s("C13", c13, gr),
        s("C12", c12, law(ThermalLaw::CInSiC)),
        s("C13", c13, law(ThermalLaw::CInSiC)),
        s("C12", c12, None),
        s("C13", c13, None),
        s("B10", e("n-005_B_010-ENDF8.0.endf"), None),
        s("B11", e("n-005_B_011-ENDF8.0.endf"), None),
        s("Si28", e("n-014_Si_028-ENDF8.0.endf"), law(ThermalLaw::SiInSiC)),
        s("Si29", e("n-014_Si_029-ENDF8.0.endf"), law(ThermalLaw::SiInSiC)),
        s("Si30", e("n-014_Si_030-ENDF8.0.endf"), law(ThermalLaw::SiInSiC)),
        s("Si28", e("n-014_Si_028-ENDF8.0.endf"), None),
        s("Si29", e("n-014_Si_029-ENDF8.0.endf"), None),
        s("Si30", e("n-014_Si_030-ENDF8.0.endf"), None),
        s("He3", e("n-002_He_003-ENDF8.0.endf"), None),
        s("He4", e("n-002_He_004-ENDF8.0.endf"), None),
        s("Al27", e("n-013_Al_027-ENDF8.0.endf"), law(ThermalLaw::AluminumMetal)),
        s("P31", e("n-015_P_031-ENDF8.0.endf"), None),
        s("S32", e("n-016_S_032-ENDF8.0.endf"), None),
        s("S33", e("n-016_S_033-ENDF8.0.endf"), None),
        s("S34", e("n-016_S_034-ENDF8.0.endf"), None),
        s("S36", e("n-016_S_036-ENDF8.0.endf"), None),
        s("Ti46", e("n-022_Ti_046-ENDF8.0.endf"), None),
        s("Ti47", e("n-022_Ti_047-ENDF8.0.endf"), None),
        s("Ti48", e("n-022_Ti_048-ENDF8.0.endf"), None),
        s("Ti49", e("n-022_Ti_049-ENDF8.0.endf"), None),
        s("Ti50", e("n-022_Ti_050-ENDF8.0.endf"), None),
        s("Cr50", e("n-024_Cr_050-ENDF8.0.endf"), None),
        s("Cr52", e("n-024_Cr_052-ENDF8.0.endf"), None),
        s("Cr53", e("n-024_Cr_053-ENDF8.0.endf"), None),
        s("Cr54", e("n-024_Cr_054-ENDF8.0.endf"), None),
        s("Mn55", e("n-025_Mn_055-ENDF8.0.endf"), None),
        s("Fe54", e("n-026_Fe_054-ENDF8.0.endf"), None),
        s("Fe56", e("n-026_Fe_056-ENDF8.0.endf"), law(ThermalLaw::IronMetal)),
        s("Fe57", e("n-026_Fe_057-ENDF8.0.endf"), None),
        s("Fe58", e("n-026_Fe_058-ENDF8.0.endf"), None),
        s("Ni58", ni("n-028_Ni_058-ENDF8.0.endf"), None),
        s("Ni60", ni("n-028_Ni_060-ENDF8.0.endf"), None),
        s("Ni61", ni("n-028_Ni_061-ENDF8.0.endf"), None),
        s("Ni62", ni("n-028_Ni_062-ENDF8.0.endf"), None),
        s("Ni64", ni("n-028_Ni_064-ENDF8.0.endf"), None),
        s("Cu63", e("n-029_Cu_063-ENDF8.0.endf"), None),
        s("Cu65", e("n-029_Cu_065-ENDF8.0.endf"), None),
    ];
    assert_eq!(slots.len(), nuc::COUNT);
    let mut notes = vec![
        "library: ENDF/B-VIII.0 for every nuclide".to_string(),
        "carbon: natural, C-12 / C-13 at 98.93 / 1.07 at.% (IUPAC)".to_string(),
    ];
    if bound {
        notes.push(format!(
            "S(a,b): graphite {} (MAT {}) on graphite carbon; C-in-SiC and Si-in-SiC on the \
             SiC; U-in-UO2 on U-234/235/238 and O-in-UO2 on O-16 (LEAPR); Fe metal on \
             Fe-56 and Al metal on Al-27 (LEAPR); everything else free gas",
            graphite.tape(),
            graphite.mat()
        ));
    } else {
        notes.push("ABLATION: every S(a,b) disabled -- all nuclides free gas".to_string());
    }
    (slots, notes)
}

/// Options for the material set.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HttrMaterialConfig {
    /// Material temperature \[K\] (R1 §3.1.4: 300 K).
    pub temperature_k: f64,
    /// Add B-11 beside each impurity's B-10 (default `true`, see the module
    /// docs).
    pub impurity_b11: bool,
}

impl HttrMaterialConfig {
    /// The default: 300 K, natural boron in every impurity.
    #[must_use]
    pub fn benchmark(temperature_k: f64) -> Self {
        Self {
            temperature_k,
            impurity_b11: true,
        }
    }

    /// ABLATION: impurity boron as B-10 only, R1's own specification.
    #[must_use]
    pub fn without_impurity_b11(mut self) -> Self {
        self.impurity_b11 = false;
        self
    }
}

/// UO2 kernel atom densities \[atoms/(b cm)\] by enrichment, R1 Table 3.1:
/// U-234, U-235, U-238. Oxygen (4.6404e-2) and B-10 (1.7299e-7) are the
/// same in all twelve.
pub const KERNEL_U: [[f64; 3]; 12] = [
    [6.1026e-06, 7.9888e-04, 2.2405e-02],
    [7.0000e-06, 9.1637e-04, 2.2288e-02],
    [7.7180e-06, 1.0104e-03, 2.2195e-02],
    [8.6154e-06, 1.1278e-03, 2.2078e-02],
    [9.3334e-06, 1.2218e-03, 2.1984e-02],
    [1.0590e-05, 1.3863e-03, 2.1821e-02],
    [1.1308e-05, 1.4803e-03, 2.1727e-02],
    [1.2026e-05, 1.5743e-03, 2.1634e-02],
    [1.2923e-05, 1.6918e-03, 2.1517e-02],
    [1.4180e-05, 1.8562e-03, 2.1353e-02],
    [1.6872e-05, 2.2087e-03, 2.1002e-02],
    [1.7769e-05, 2.3262e-03, 2.0886e-02],
];
/// Kernel O-16 and O-17 \[atoms/(b cm)\]: R1 Table 3.1's 4.6404e-2 split
/// as the deck's m1-m12 split it.
pub const KERNEL_O: [f64; 2] = [4.6386e-02, 1.7633e-05];
/// Kernel B-10 \[atoms/(b cm)\] (R1 Table 3.1).
pub const KERNEL_B10: f64 = 1.7299e-07;

/// Alloy 800H \[atoms/(b cm)\], R1 Table 3.8 element totals split into
/// isotopes as the deck's m24 card splits them. `(slot, density)`.
pub const ALLOY_800H: [(usize, f64); 31] = [
    (nuc::C12_FREE, 3.2210e-04), // carbon: split into C-12/13 when built
    (nuc::AL27, 6.7209e-04),
    (nuc::SI28, 5.5580e-04),
    (nuc::SI29, 2.8222e-05),
    (nuc::SI30, 1.8604e-05),
    (nuc::P31, 3.1225e-05),
    (nuc::S[0], 1.4316e-05),
    (nuc::S[1], 1.1462e-07),
    (nuc::S[2], 6.4698e-07),
    (nuc::S[3], 3.0162e-09),
    (nuc::TI[0], 3.1254e-05),
    (nuc::TI[1], 2.8186e-05),
    (nuc::TI[2], 2.7928e-04),
    (nuc::TI[3], 2.0495e-05),
    (nuc::TI[4], 1.9624e-05),
    (nuc::CR[0], 8.4860e-04),
    (nuc::CR[1], 1.6364e-02),
    (nuc::CR[2], 1.8556e-03),
    (nuc::CR[3], 4.6189e-04),
    (nuc::MN55, 8.8022e-04),
    (nuc::FE[0], 2.2265e-03),
    (nuc::FE[1], 3.4951e-02),
    (nuc::FE[2], 8.0717e-04),
    (nuc::FE[3], 1.0742e-04),
    (nuc::NI[0], 1.8229e-02),
    (nuc::NI[1], 7.0217e-03),
    (nuc::NI[2], 3.0523e-04),
    (nuc::NI[3], 9.7320e-04),
    (nuc::NI[4], 2.4785e-04),
    (nuc::CU[0], 1.5791e-04),
    (nuc::CU[1], 7.0383e-05),
];

/// A material under construction: components summed by slot.
struct Mix {
    name: &'static str,
    parts: Vec<(usize, f64)>,
}

impl Mix {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            parts: Vec::new(),
        }
    }
    fn add(mut self, slot: usize, n: f64) -> Self {
        self.parts.push((slot, n));
        self
    }
    /// Natural carbon on the C-12/C-13 slot pair `(c12, c13)`.
    fn carbon(self, n: f64, c12: usize, c13: usize) -> Self {
        self.add(c12, n * C12_ATOM_FRACTION_OF_NATURAL_C)
            .add(c13, n * C13_ATOM_FRACTION_OF_NATURAL_C)
    }
    /// Graphite carbon `c` with an impurity of `b10` B-10, plus its B-11 if
    /// `cfg` asks for natural boron.
    fn graphite(self, c: f64, b10: f64, cfg: &HttrMaterialConfig) -> Self {
        self.carbon(c, nuc::C12_GRAPHITE, nuc::C13_GRAPHITE)
            .impurity(b10, cfg)
    }
    fn impurity(self, b10: f64, cfg: &HttrMaterialConfig) -> Self {
        let m = self.add(nuc::B10, b10);
        if cfg.impurity_b11 {
            m.add(nuc::B11, b10 * B11_PER_B10)
        } else {
            m
        }
    }
    fn build(self, id: usize, t: f64) -> Material {
        Material {
            id: id as i32 + 1,
            name: self.name.to_string(),
            components: self
                .parts
                .into_iter()
                .map(|(nuclide_idx, atom_density)| NuclideComponent {
                    nuclide_idx,
                    atom_density,
                })
                .collect(),
            temperature: t,
        }
    }
}

/// The 27 materials of R1 §3.1.3, in [`mat`] order.
#[must_use]
pub fn httr_material_set(cfg: HttrMaterialConfig) -> Vec<Material> {
    const KERNEL_NAMES: [&str; 12] = [
        "UO2 kernel 3.4 wt%",
        "UO2 kernel 3.9 wt%",
        "UO2 kernel 4.3 wt%",
        "UO2 kernel 4.8 wt%",
        "UO2 kernel 5.2 wt%",
        "UO2 kernel 5.9 wt%",
        "UO2 kernel 6.3 wt%",
        "UO2 kernel 6.7 wt%",
        "UO2 kernel 7.2 wt%",
        "UO2 kernel 7.9 wt%",
        "UO2 kernel 9.4 wt%",
        "UO2 kernel 9.9 wt%",
    ];
    let t = cfg.temperature_k;
    let mut mixes: Vec<Mix> = KERNEL_U
        .iter()
        .zip(KERNEL_NAMES)
        .map(|(u, name)| {
            Mix::new(name)
                .add(nuc::U234, u[0])
                .add(nuc::U235, u[1])
                .add(nuc::U238, u[2])
                .add(nuc::O16, KERNEL_O[0])
                .add(nuc::O17, KERNEL_O[1])
                .impurity(KERNEL_B10, &cfg)
        })
        .collect();
    // R1 Table 3.2 (coatings), 3.3 (matrix), 3.4 (sleeve).
    mixes.push(Mix::new("buffer").graphite(5.5153e-02, 1.8290e-08, &cfg));
    mixes.push(Mix::new("IPyC").graphite(9.2758e-02, 3.0761e-08, &cfg));
    mixes.push(
        Mix::new("SiC")
            .carbon(4.8061e-02, nuc::C12_SIC, nuc::C13_SIC)
            .add(nuc::SI28_SIC, 4.4327e-02)
            .add(nuc::SI29_SIC, 2.2508e-03)
            .add(nuc::SI30_SIC, 1.4837e-03)
            .impurity(5.3208e-08, &cfg),
    );
    mixes.push(Mix::new("OPyC").graphite(9.2758e-02, 3.0761e-08, &cfg));
    mixes.push(Mix::new("overcoat").graphite(8.5237e-02, 2.8267e-08, &cfg));
    mixes.push(Mix::new("compact matrix").graphite(8.5237e-02, 1.5452e-08, &cfg));
    mixes.push(Mix::new("IG-110 sleeve").graphite(8.8747e-02, 7.2596e-09, &cfg));
    // R1 Table 3.5: burnable poison and disks. The poison's B-11 is part of
    // its specification, not an impurity.
    mixes.push(
        Mix::new("burnable poison 2.0 wt% B")
            .carbon(8.8446e-02, nuc::C12_GRAPHITE, nuc::C13_GRAPHITE)
            .add(nuc::B10, 3.9906e-04)
            .add(nuc::B11, 1.6063e-03),
    );
    mixes.push(
        Mix::new("burnable poison 2.5 wt% B")
            .carbon(8.7995e-02, nuc::C12_GRAPHITE, nuc::C13_GRAPHITE)
            .add(nuc::B10, 4.9882e-04)
            .add(nuc::B11, 2.0078e-03),
    );
    mixes.push(Mix::new("graphite disks").graphite(8.8747e-02, 7.2596e-09, &cfg));
    // R1 Table 3.7: B4C/C absorber. Stoichiometric B4C carbon is free; the
    // rest is the graphite of the sintered compact (module docs).
    let (b10, b11, c) = (6.3184e-03, 2.5432e-02, 6.6685e-02);
    let c_b4c = (b10 + b11) / 4.0;
    mixes.push(
        Mix::new("B4C/C absorber")
            .add(nuc::B10, b10)
            .add(nuc::B11, b11)
            .carbon(c_b4c, nuc::C12_FREE, nuc::C13_FREE)
            .carbon(c - c_b4c, nuc::C12_GRAPHITE, nuc::C13_GRAPHITE),
    );
    // R1 Table 3.8 / deck m24: Alloy 800H.
    let mut alloy = Mix::new("Alloy 800H");
    for (slot, n) in ALLOY_800H {
        alloy = if slot == nuc::C12_FREE {
            alloy.carbon(n, nuc::C12_FREE, nuc::C13_FREE)
        } else {
            alloy.add(slot, n)
        };
    }
    mixes.push(alloy);
    // R1 Tables 3.6 and 3.9; helium as the deck's m27.
    mixes.push(Mix::new("IG-110 block").graphite(8.7804e-02, 1.1453e-08, &cfg));
    mixes.push(Mix::new("PGX reflector").graphite(8.6134e-02, 3.6372e-08, &cfg));
    mixes.push(
        Mix::new("helium")
            .add(nuc::HE3, 3.3724e-11)
            .add(nuc::HE4, 2.4616e-05),
    );
    assert_eq!(mixes.len(), mat::COUNT);
    mixes
        .into_iter()
        .enumerate()
        .map(|(i, m)| m.build(i, t))
        .collect()
}
