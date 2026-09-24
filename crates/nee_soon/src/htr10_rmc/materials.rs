// SPDX-License-Identifier: GPL-3.0-only
//! **The HTR-10 material set, in one place.**
//!
//! # Why this module exists
//!
//! These eleven materials were assembled inline in
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
//! length is asserted against `mat::HOMOG_DUMMY + 1` for that reason.
//!
//! # Provenance
//!
//! Compositions: Li et al. (2014) Table 2 for the pebble (via
//! [`outram_mc_libs::pebble_beds::htr10::fuel_pebble_materials`]);
//! IAEA-TECDOC-1382 Table 4-3 for every reflector zone.

use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::pebble_beds::htr10::{fuel_pebble_materials, BoronReading, Htr10Nuclides};

use super::core_model::{mat, HTR10_BORED_BORON, HTR10_BORED_CARBON, PAPER_FILLING_FRACTION};
use super::reflector::zone_composition;

/// Natural boron is 19.9 at.% B-10; the rest is effectively a non-absorber.
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

/// Build the eleven-material set, indexed by [`mat`].
///
/// # Panics
///
/// If `reflector_zone` is not listed in TECDOC Table 4-3, or if the assembled
/// length does not match the index table.
#[must_use]
pub fn htr10_material_set(n: Htr10Nuclides, cfg: Htr10MaterialConfig) -> Vec<Material> {
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

    assert_eq!(
        mats.len(),
        mat::HOMOG_DUMMY + 1,
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
        i if i == n.u235 => "U-235",
        i if i == n.u238 => "U-238",
        i if i == n.o16 => "O-16",
        i if i == n.c_free => "C (free gas)",
        i if i == n.c_graphite => "C (graphite S(a,b))",
        i if i == n.si28 => "Si-28",
        i if i == n.b10 => "B-10",
        _ => "unknown",
    }
}
