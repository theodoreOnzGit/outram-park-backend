// SPDX-License-Identifier: GPL-3.0-only
//! **The HTR-10 material set, in one place.**
//!
//! # Why this module exists
//!
//! These materials (eleven until 2026-09-25, `mat::COUNT` since the explicit
//! reflector) were assembled inline in
//! `examples/htr10_rmc_keff.rs`. That was fine while the example was the only
//! consumer, and stopped being fine the moment a second one appeared
//! (`examples/htr10_geometry_export.rs`, which writes the model specification
//! for the double-heterogeneity manuscript). Two copies of a material set is
//! exactly the drift this workspace forbids: the exported table would go on
//! describing the model the eigenvalue *used to* be computed with.
//!
//! So the assembly lives here and both callers use it. The **environment
//! knobs stay in the example** — this function takes an explicit
//! [`Htr10MaterialConfig`] instead, so a caller that wants the default model
//! asks for the default and gets exactly what the benchmark runs.
//!
//! # The indices are load-bearing
//!
//! The returned `Vec` is indexed by [`super::core_model::mat`], and the
//! geometry refers to materials by that index. Reordering it silently
//! repoints every cell in the core at the wrong material, which is not a
//! failure that announces itself — `k_eff` simply comes out wrong. The
//! length is asserted against `mat::COUNT` for that reason.
//!
//! # Provenance
//!
//! Compositions: Li et al. (2014) Table 2 for the pebble (via
//! [`outram_mc_libs::pebble_beds::htr10::fuel_pebble_materials`]);
//! IAEA-TECDOC-1382 Table 4-3 for every reflector zone, with its p. 242
//! corrections; TECDOC § 4.1.2 for the control-rod B4C, steel and iron;
//! IUPAC/CIAAW for atomic weights and isotopic compositions.

use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::pebble_beds::htr10::{fuel_pebble_materials, BoronReading, Htr10Nuclides};

use super::core_model::{mat, HTR10_BORED_BORON, HTR10_BORED_CARBON, PAPER_FILLING_FRACTION};
use super::reflector::zone_composition;

/// Natural boron is 19.9 at.% B-10; the other 80.1 at.% is B-11, a
/// non-absorber that is placed anyway (gh:#311) because the reference states
/// natural boron.
pub const B10_OF_NATURAL: f64 = 0.199;

/// Everything the material set depends on, stated rather than read from the
/// environment — see the module docs.
#[derive(Debug, Clone, Copy)]
pub struct Htr10MaterialConfig {
    /// Temperature \[K\] every material is built at.
    pub temperature_k: f64,
    /// How the two "ppm natural boron" rows of Table 2 are read.
    pub boron: BoronReading,
    /// Which TECDOC Table 4-3 zone stands in for the whole reflector.
    /// 22 is the default and the OPTIMISTIC bound (cleanest graphite).
    pub reflector_zone: usize,
    /// Multiplier on the reflector's carbon density. `1.0` is unmodified.
    /// A sensitivity bound, not a model.
    pub reflector_carbon_scale: f64,
}

impl Htr10MaterialConfig {
    /// The configuration the reported benchmark results are computed with.
    #[must_use]
    pub fn benchmark_default(temperature_k: f64) -> Self {
        Self {
            temperature_k,
            boron: BoronReading::Natural,
            reflector_zone: 22,
            reflector_carbon_scale: 1.0,
        }
    }
}

/// Indices, into the caller's nuclide array, of the nuclides the withdrawn
/// control rods need beyond [`Htr10Nuclides`]: the sleeve steel and the iron
/// joints.
///
/// Silicon appears here AGAIN, as free gas: [`Htr10Nuclides`]'s silicon is
/// bound in SiC with its own S(alpha, beta), which is wrong for silicon
/// dissolved in steel. Carbon in steel and in B4C uses
/// [`Htr10Nuclides::c_free`] for the same reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub struct RodMetalNuclides {
    pub fe54: usize,
    pub fe56: usize,
    pub fe57: usize,
    pub fe58: usize,
    pub cr50: usize,
    pub cr52: usize,
    pub cr53: usize,
    pub cr54: usize,
    pub ni58: usize,
    pub ni60: usize,
    pub ni61: usize,
    pub ni62: usize,
    pub ni64: usize,
    pub mn55: usize,
    pub ti46: usize,
    pub ti47: usize,
    pub ti48: usize,
    pub ti49: usize,
    pub ti50: usize,
    /// Free-gas Si-28 (NOT the SiC-bound slot).
    pub si28: usize,
    /// Free-gas Si-29.
    pub si29: usize,
    /// Free-gas Si-30.
    pub si30: usize,
}

impl RodMetalNuclides {
    /// Number of nuclide slots this table names.
    pub const COUNT: usize = 22;

    /// The slots laid out consecutively from `first`, in field order
    /// (Fe-54..58, Cr-50..54, Ni-58..64, Mn-55, Ti-46..50, free Si-28..30) --
    /// the order a caller appends them to its nuclide array in.
    #[must_use]
    pub const fn contiguous(first: usize) -> Self {
        let f = first;
        Self {
            fe54: f,
            fe56: f + 1,
            fe57: f + 2,
            fe58: f + 3,
            cr50: f + 4,
            cr52: f + 5,
            cr53: f + 6,
            cr54: f + 7,
            ni58: f + 8,
            ni60: f + 9,
            ni61: f + 10,
            ni62: f + 11,
            ni64: f + 12,
            mn55: f + 13,
            ti46: f + 14,
            ti47: f + 15,
            ti48: f + 16,
            ti49: f + 17,
            ti50: f + 18,
            si28: f + 19,
            si29: f + 20,
            si30: f + 21,
        }
    }
}

/// ENDF/B-VIII.0 tapes (in `reference-data/endf/`) for the rod-metal
/// nuclides, in [`RodMetalNuclides::contiguous`] order, as `(name, file)`.
///
/// The silicon tapes are the same files as the SiC slots'; they are loaded a
/// second time WITHOUT a thermal law. There is no ENDF/B-VII.0 counterpart in
/// the checkout for the metals, so a VII.0 run takes these VIII.0 tapes for
/// the rod metal only, and must say so.
pub const ROD_METAL_TAPES_ENDF8: [(&str, &str); RodMetalNuclides::COUNT] = [
    ("Fe54", "n-026_Fe_054-ENDF8.0.endf"),
    ("Fe56", "n-026_Fe_056-ENDF8.0.endf"),
    ("Fe57", "n-026_Fe_057-ENDF8.0.endf"),
    ("Fe58", "n-026_Fe_058-ENDF8.0.endf"),
    ("Cr50", "n-024_Cr_050-ENDF8.0.endf"),
    ("Cr52", "n-024_Cr_052-ENDF8.0.endf"),
    ("Cr53", "n-024_Cr_053-ENDF8.0.endf"),
    ("Cr54", "n-024_Cr_054-ENDF8.0.endf"),
    ("Ni58", "n-028_Ni_058-ENDF8.0.endf"),
    ("Ni60", "n-028_Ni_060-ENDF8.0.endf"),
    ("Ni61", "n-028_Ni_061-ENDF8.0.endf"),
    ("Ni62", "n-028_Ni_062-ENDF8.0.endf"),
    ("Ni64", "n-028_Ni_064-ENDF8.0.endf"),
    ("Mn55", "n-025_Mn_055-ENDF8.0.endf"),
    ("Ti46", "n-022_Ti_046-ENDF8.0.endf"),
    ("Ti47", "n-022_Ti_047-ENDF8.0.endf"),
    ("Ti48", "n-022_Ti_048-ENDF8.0.endf"),
    ("Ti49", "n-022_Ti_049-ENDF8.0.endf"),
    ("Ti50", "n-022_Ti_050-ENDF8.0.endf"),
    ("Si28", "n-014_Si_028-ENDF8.0.endf"),
    ("Si29", "n-014_Si_029-ENDF8.0.endf"),
    ("Si30", "n-014_Si_030-ENDF8.0.endf"),
];

/// Avogadro constant \[1/mol\] (CODATA 2018, exact).
const AVOGADRO: f64 = 6.022_140_76e23;

/// Standard atomic weights \[g/mol\], IUPAC/CIAAW (Meija et al., *Pure Appl.
/// Chem.* 88 (2016) 265-291, Table 1; conventional values where an interval is
/// given): the seven constituents of the rod steel.
pub mod atomic_weight {
    /// Chromium.
    pub const CR: f64 = 51.9961;
    /// Iron.
    pub const FE: f64 = 55.845;
    /// Nickel.
    pub const NI: f64 = 58.6934;
    /// Silicon (conventional value).
    pub const SI: f64 = 28.085;
    /// Manganese.
    pub const MN: f64 = 54.938_044;
    /// Carbon (conventional value).
    pub const C: f64 = 12.011;
    /// Titanium.
    pub const TI: f64 = 47.867;
    /// Boron (conventional value).
    pub const B: f64 = 10.811;
}

/// Representative natural isotopic compositions \[atom fraction\], IUPAC/CIAAW
/// (Meija et al., *Pure Appl. Chem.* 88 (2016) 293-306, Table 1).
pub mod abundance {
    /// Fe-54, Fe-56, Fe-57, Fe-58.
    pub const FE: [f64; 4] = [0.05845, 0.91754, 0.02119, 0.00282];
    /// Cr-50, Cr-52, Cr-53, Cr-54.
    pub const CR: [f64; 4] = [0.04345, 0.83789, 0.09501, 0.02365];
    /// Ni-58, Ni-60, Ni-61, Ni-62, Ni-64.
    pub const NI: [f64; 5] = [0.680_769, 0.262_231, 0.011_399, 0.036_345, 0.009_256];
    /// Ti-46, Ti-47, Ti-48, Ti-49, Ti-50.
    pub const TI: [f64; 5] = [0.0825, 0.0744, 0.7372, 0.0541, 0.0518];
}

/// Rod sleeve steel density \[g/cm³\], TECDOC § 4.1.2.
pub const ROD_STEEL_DENSITY: f64 = 7.9;
/// Rod sleeve steel composition \[weight fraction\], TECDOC § 4.1.2:
/// Cr 18, Fe 68.1, Ni 10, Si 1, Mn 2, C 0.1, Ti 0.8 (sums to 100 %).
pub const ROD_STEEL_WT: [(&str, f64); 7] = [
    ("Cr", 0.18),
    ("Fe", 0.681),
    ("Ni", 0.10),
    ("Si", 0.01),
    ("Mn", 0.02),
    ("C", 0.001),
    ("Ti", 0.008),
];
/// Iron atom density of the rod joints and ends \[atoms/(b cm)\], TECDOC
/// § 4.1.2: iron alone, filling 27.5 mm < R < 55 mm.
pub const ROD_JOINT_IRON_DENSITY: f64 = 0.04;

/// Build the material set, indexed by [`mat`] (`mat::COUNT` materials).
///
/// **Signature changed 2026-09-25** to take [`RodMetalNuclides`]: the ten
/// control rods are now explicit geometry at their withdrawn position, and
/// their steel sleeves and iron joints need nuclides the pebble set has no
/// slots for.
///
/// # Panics
///
/// If `reflector_zone` is not listed in TECDOC Table 4-3, or if the assembled
/// length does not match the index table.
#[must_use]
pub fn htr10_material_set(
    n: Htr10Nuclides,
    metal: RodMetalNuclides,
    cfg: Htr10MaterialConfig,
) -> Vec<Material> {
    let t = cfg.temperature_k;
    let boron = cfg.boron;
    // B-10 density for a zone, honouring the `BoronReading::None` ablation.
    let b10 = |natural: f64| {
        if matches!(boron, BoronReading::None) {
            0.0
        } else {
            natural * B10_OF_NATURAL
        }
    };
    // B-11, the rest of the same natural boron (gh:#311). Until 2026-09-25 only
    // B-10 was placed, leaving the boronated brick ~3.5 % short on atoms.
    let b11 = |natural: f64| {
        if matches!(boron, BoronReading::None) {
            0.0
        } else {
            natural * (1.0 - B10_OF_NATURAL)
        }
    };

    let mut mats = fuel_pebble_materials(n, boron, t);
    // `fuel_pebble_materials` returns a 7th entry ("shell graphite") that is
    // identical to the matrix graphite at index 5; the core model uses one
    // index for both, so slot 6 is reused for helium.
    mats.truncate(6);

    // 6: helium -- deliberately near-void, as the reference's own model omits it.
    mats.push(Material {
        id: 70,
        name: "helium".into(),
        components: vec![],
        temperature: t,
    });

    // 7: reflector graphite, TECDOC Table 4-3.
    let z = zone_composition(cfg.reflector_zone).expect("reflector zone is listed in Table 4-3");
    mats.push(Material {
        id: 71,
        name: format!("reflector graphite (TECDOC zone {})", cfg.reflector_zone),
        components: vec![
            NuclideComponent {
                nuclide_idx: n.c_graphite,
                atom_density: z.carbon * cfg.reflector_carbon_scale,
            },
            NuclideComponent {
                nuclide_idx: n.b10,
                atom_density: b10(z.natural_boron),
            },
            NuclideComponent {
                nuclide_idx: n.b11,
                atom_density: b11(z.natural_boron),
            },
        ],
        temperature: t,
    });

    // 8: boronated carbon brick, the outermost reflector annulus. Zone 17 --
    // natural boron 3.4635e-3, ~7300x zone 22's.
    let zb = zone_composition(17).expect("zone 17 is listed");
    mats.push(Material {
        id: 72,
        name: "boronated carbon brick (TECDOC zone 17)".into(),
        components: vec![
            NuclideComponent {
                nuclide_idx: n.c_graphite,
                atom_density: zb.carbon,
            },
            NuclideComponent {
                nuclide_idx: n.b10,
                atom_density: b10(zb.natural_boron),
            },
            NuclideComponent {
                nuclide_idx: n.b11,
                atom_density: b11(zb.natural_boron),
            },
        ],
        temperature: t,
    });

    // 9: side reflector homogenised with its control-rod borings, TECDOC
    // zones 31-40 -- 28.1 % less carbon than zone 22.
    mats.push(Material {
        id: 73,
        name: "bored side reflector (TECDOC zones 31-40)".into(),
        components: vec![
            NuclideComponent {
                nuclide_idx: n.c_graphite,
                atom_density: HTR10_BORED_CARBON,
            },
            NuclideComponent {
                nuclide_idx: n.b10,
                atom_density: b10(HTR10_BORED_BORON),
            },
            NuclideComponent {
                nuclide_idx: n.b11,
                atom_density: b11(HTR10_BORED_BORON),
            },
        ],
        temperature: t,
    });

    // 10: homogenised dummy pebbles = pebble graphite scaled to the bed's
    // filling fraction. What the discharge tube actually contains (Terry 2005
    // section 2), between the bounds of solid graphite and pure helium.
    let dummy = mats[mat::GRAPHITE].clone();
    mats.push(Material {
        id: 74,
        name: "homogenised dummy pebbles (0.61 packing)".into(),
        components: dummy
            .components
            .iter()
            .map(|c| NuclideComponent {
                nuclide_idx: c.nuclide_idx,
                atom_density: c.atom_density * PAPER_FILLING_FRACTION,
            })
            .collect(),
        temperature: t,
    });

    // 11..: the Table 4-3 zones that keep a composition of their own in the
    // Monte Carlo model, with TECDOC p. 242's correction factors (see
    // `mat::TABLE_4_3_ZONES`). The factor scales carbon and boron alike: it
    // restores the graphite a homogenised boring void had diluted.
    assert_eq!(mats.len(), mat::ZONE_TABLE_FIRST);
    for (zone, factor) in mat::TABLE_4_3_ZONES {
        let z = zone_composition(zone).expect("every TABLE_4_3_ZONES entry is in Table 4-3");
        let mut components = vec![NuclideComponent {
            nuclide_idx: n.c_graphite,
            atom_density: z.carbon * factor,
        }];
        if z.natural_boron > 0.0 {
            components.push(NuclideComponent {
                nuclide_idx: n.b10,
                atom_density: b10(z.natural_boron * factor),
            });
            components.push(NuclideComponent {
                nuclide_idx: n.b11,
                atom_density: b11(z.natural_boron * factor),
            });
        }
        mats.push(Material {
            id: 100 + zone as i32,
            name: if factor == 1.0 {
                format!("TECDOC Table 4-3 zone {zone}")
            } else {
                format!("TECDOC Table 4-3 zone {zone} x {factor} (p. 242)")
            },
            components,
            temperature: t,
        });
    }

    // Control-rod B4C: 1.7 g/cm3 of B4C with natural boron (TECDOC § 4.1.2).
    // Its carbon is NOT graphite, so it takes the free-gas carbon slot.
    let n_b4c = super::control_rod::b4c_molecular_density();
    mats.push(Material {
        id: 90,
        name: "control-rod B4C (1.7 g/cm3)".into(),
        components: vec![
            NuclideComponent {
                nuclide_idx: n.b10,
                atom_density: b10(4.0 * n_b4c),
            },
            NuclideComponent {
                nuclide_idx: n.b11,
                atom_density: b11(4.0 * n_b4c),
            },
            NuclideComponent {
                nuclide_idx: n.c_free,
                atom_density: n_b4c,
            },
        ],
        temperature: t,
    });

    // Control-rod sleeve steel, 7.9 g/cm3 (TECDOC § 4.1.2), split into
    // natural isotopes. N_e = rho w_e N_A / M_e.
    let n_elem = |w: f64, m: f64| ROD_STEEL_DENSITY * w * AVOGADRO / m * 1.0e-24;
    let wt = |e: &str| {
        ROD_STEEL_WT
            .iter()
            .find(|(s, _)| *s == e)
            .map(|(_, w)| *w)
            .expect("listed element")
    };
    let mut steel: Vec<NuclideComponent> = Vec::new();
    let mut split = |total: f64, idx: &[usize], frac: &[f64]| {
        for (&i, &f) in idx.iter().zip(frac) {
            steel.push(NuclideComponent {
                nuclide_idx: i,
                atom_density: total * f,
            });
        }
    };
    split(
        n_elem(wt("Fe"), atomic_weight::FE),
        &[metal.fe54, metal.fe56, metal.fe57, metal.fe58],
        &abundance::FE,
    );
    split(
        n_elem(wt("Cr"), atomic_weight::CR),
        &[metal.cr50, metal.cr52, metal.cr53, metal.cr54],
        &abundance::CR,
    );
    split(
        n_elem(wt("Ni"), atomic_weight::NI),
        &[metal.ni58, metal.ni60, metal.ni61, metal.ni62, metal.ni64],
        &abundance::NI,
    );
    split(
        n_elem(wt("Ti"), atomic_weight::TI),
        &[metal.ti46, metal.ti47, metal.ti48, metal.ti49, metal.ti50],
        &abundance::TI,
    );
    split(
        n_elem(wt("Si"), atomic_weight::SI),
        &[metal.si28, metal.si29, metal.si30],
        &[
            outram_mc_libs::pebble_beds::htr10::SI28_ATOM_FRACTION,
            outram_mc_libs::pebble_beds::htr10::SI29_ATOM_FRACTION,
            outram_mc_libs::pebble_beds::htr10::SI30_ATOM_FRACTION,
        ],
    );
    split(n_elem(wt("Mn"), atomic_weight::MN), &[metal.mn55], &[1.0]);
    split(n_elem(wt("C"), atomic_weight::C), &[n.c_free], &[1.0]);
    mats.push(Material {
        id: 91,
        name: "control-rod sleeve steel (7.9 g/cm3)".into(),
        components: steel,
        temperature: t,
    });

    // Control-rod joints and ends: iron only, 0.04 atoms/(b cm).
    mats.push(Material {
        id: 92,
        name: "control-rod joint iron (0.04 /b-cm)".into(),
        components: [metal.fe54, metal.fe56, metal.fe57, metal.fe58]
            .iter()
            .zip(abundance::FE)
            .map(|(&i, f)| NuclideComponent {
                nuclide_idx: i,
                atom_density: ROD_JOINT_IRON_DENSITY * f,
            })
            .collect(),
        temperature: t,
    });

    assert_eq!(
        mats.len(),
        mat::COUNT,
        "material set length must match the `mat` index table"
    );
    mats
}

/// Human-readable name for each nuclide slot, for reporting.
///
/// [`Htr10Nuclides`] is a table of *indices into the caller's nuclide array*,
/// so a material component carries an index and nothing else. Anything that
/// reports a composition needs this to turn that index back into a name.
#[must_use]
pub fn nuclide_name(n: Htr10Nuclides, idx: usize) -> &'static str {
    match idx {
        // The thermal law named is the one `htr10_rmc_keff` binds to the slot
        // in its default (ENDF/B-VIII.0) arm. The VII.0 arm leaves the SiC
        // slots free gas: VII.0 ships no SiC evaluation.
        i if i == n.u235 => "U-235 (U-in-UO2 S(a,b))",
        i if i == n.u238 => "U-238 (U-in-UO2 S(a,b))",
        i if i == n.o16 => "O-16 (O-in-UO2 S(a,b))",
        i if i == n.c_free => "C (free gas)",
        i if i == n.c_graphite => "C (graphite S(a,b))",
        i if i == n.c_sic => "C (C-in-SiC S(a,b))",
        i if i == n.si28 => "Si-28 (Si-in-SiC S(a,b))",
        i if i == n.si29 => "Si-29 (Si-in-SiC S(a,b))",
        i if i == n.si30 => "Si-30 (Si-in-SiC S(a,b))",
        i if i == n.b10 => "B-10",
        i if i == n.b11 => "B-11",
        _ => "unknown",
    }
}
