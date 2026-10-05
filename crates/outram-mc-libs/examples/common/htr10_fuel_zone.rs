// SPDX-License-Identifier: GPL-3.0

//! **The HTR-10 fuel-zone infinite medium**: TRISO kernels resolved in their
//! graphite matrix against the same atoms homogenised (tutorial rung 5,
//! gh:#528). Shared by `htr10_fuel_zone_kinf.rs` (the record,
//! `verification_and_validation/tutorial_rung5/README.md`) and the
//! `dhoby-ghaut` Monte Carlo web demo's `htr10` rung, which pulls this file in
//! with `#[path]`, so the browser and the record build one model. Moved here
//! unchanged from the example on 2026-10-05.
//!
//! Atom densities: IAEA-TECDOC-1382 Table 4-38; kernel radius, fuelled-zone
//! radius and particles per pebble: its Table 4-2 and Monte Carlo notes (see
//! the example's "Data provenance").

#![allow(dead_code)]

use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::pebble_beds::delta_tracking::Majorant;

/// UO2 kernel atom densities, atoms/barn-cm (IAEA-TECDOC-1382 Table 4-38).
pub const KERNEL_U235: f64 = 3.992067e-3;
pub const KERNEL_U238: f64 = 1.924449e-2;
pub const KERNEL_O16: f64 = 4.647329e-2;
pub const KERNEL_B10: f64 = 1.849637e-8;
pub const KERNEL_B11: f64 = 7.445022e-8;

/// Matrix graphite atom densities, atoms/barn-cm (IAEA-TECDOC-1382 Table 4-38).
/// The carbon figure corresponds to the specified 1.73 g/cm^3 graphite.
pub const MATRIX_C: f64 = 8.674169e-2;
pub const MATRIX_B10: f64 = 2.244010e-8;
pub const MATRIX_B11: f64 = 9.032424e-8;

/// Fuel kernel radius, cm (IAEA-TECDOC-1382 Table 4-2: 0.25 mm).
pub const KERNEL_RADIUS_CM: f64 = 0.025;
/// Fuelled-zone radius of a fuel pebble, cm (5.0 cm diameter).
pub const FUEL_ZONE_RADIUS_CM: f64 = 2.5;
/// Coated particles per fuel pebble (IAEA-TECDOC-1382 Monte Carlo notes).
pub const PARTICLES_PER_PEBBLE: f64 = 8335.0;

/// Where each nuclide sits in the nuclide table, for one data route.
///
/// Carbon is a list because the ENDF/B-VIII.0 route carries C-12 and C-13 as
/// separate evaluations (split at the natural abundance), while the LOW tier
/// has one elemental carbon.
pub struct Layout {
    pub u235: usize,
    pub u238: usize,
    pub o16: usize,
    /// `(nuclide index, atom fraction of natural carbon)`.
    pub carbon: Vec<(usize, f64)>,
    pub b10: usize,
    pub b11: usize,
}

/// Kernel, matrix and the inventory-matched homogenised fuel zone, for either
/// route. `f` is the kernel volume fraction.
pub fn build_materials(l: &Layout, temperature: f64, f: f64) -> (Material, Material, Material) {
    let mk = |id: i32, name: &str, comps: Vec<(usize, f64)>| Material {
        id,
        name: name.into(),
        temperature,
        components: comps
            .into_iter()
            .map(|(nuclide_idx, atom_density)| NuclideComponent {
                nuclide_idx,
                atom_density,
            })
            .collect(),
    };
    let kernel = mk(
        1,
        "HTR-10 UO2 kernel (17 wt% enriched)",
        vec![
            (l.u235, KERNEL_U235),
            (l.u238, KERNEL_U238),
            (l.o16, KERNEL_O16),
            (l.b10, KERNEL_B10),
            (l.b11, KERNEL_B11),
        ],
    );
    let mut m = Vec::new();
    for &(i, frac) in &l.carbon {
        m.push((i, MATRIX_C * frac));
    }
    m.push((l.b10, MATRIX_B10));
    m.push((l.b11, MATRIX_B11));
    let matrix = mk(2, "HTR-10 graphite matrix (1.73 g/cm^3, 1.3 ppm EBC)", m);

    // The homogenised counterpart: exactly the same nuclide inventory, volume
    // weighted into a single medium. Same atoms, no geometry.
    let mut h = vec![
        (l.u235, KERNEL_U235 * f),
        (l.u238, KERNEL_U238 * f),
        (l.o16, KERNEL_O16 * f),
    ];
    for &(i, frac) in &l.carbon {
        h.push((i, MATRIX_C * frac * (1.0 - f)));
    }
    h.push((l.b10, KERNEL_B10 * f + MATRIX_B10 * (1.0 - f)));
    h.push((l.b11, KERNEL_B11 * f + MATRIX_B11 * (1.0 - f)));
    let homogenised = mk(3, "HTR-10 fuel zone, homogenised", h);
    (kernel, matrix, homogenised)
}

/// The delta-tracking majorant: [`Majorant::bounding`] over `[1e-5, 2e7]` eV,
/// 4096 x 32, margin 0.1.
///
/// # History
///
/// Until 2026-10-05 this example used the old `Majorant::bounding`, which
/// samples each of 4096 log bins at 32 points and adds a margin. On this
/// problem's ENDF/B-VIII.0 data that **under-bounds**: the dense audit
/// measured `Sigma_t / Sigma_maj = 1.1827` at 1.689 MeV (margin 0.1, first
/// pilot), because a resonance narrower than the ~340 eV sampling step there
/// slipped between the samples. An under-bound majorant is a silent bias, so
/// the protocol was changed rather than the margin raised until the check
/// passed.
///
/// f0d701bfc fixed it here first, with an example-local `build_majorant`:
/// `Majorant::from_materials` on the union of every nuclide's own grid and a
/// 4096 x 32 log backbone. The 2026-10-05 ENDF record below was measured
/// with that construction. GitHub #585 then moved the union-grid
/// construction into the library, so `Majorant::bounding` is now a bound by
/// construction. It also adds the S(alpha,beta) and URR breakpoints and the
/// one-sided limits at steps, which the local version did not have. Both
/// are valid bounds, and delta tracking is unbiased on any valid majorant.
/// The record therefore stands, but a re-run now draws a different random
/// stream and does not reproduce it digit for digit.
///
/// `OUTRAM_HTR10_MAJORANT=bounding` keeps the pre-#585 construction
/// (`Majorant::bounding_without_breakpoints`) as an ablation.
pub fn build_majorant(materials: &[Material], nuclides: &[Nuclide]) -> Majorant {
    Majorant::bounding(materials, nuclides, 1.0e-5, 2.0e7, 4096, 32, 0.1)
}

/// The ENDF/B-VIII.0 route's tapes, in nuclide-table order: `(name, file)`.
/// C-12 and C-13 carry the graphite law. Matches [`endf_layout`].
pub const ENDF_TAPES: [(&str, &str); 7] = [
    ("U235", "n-092_U_235-ENDF8.0.endf"),
    ("U238", "n-092_U_238.endf"),
    ("O16", "n-008_O_016-ENDF8.0.endf"),
    ("C12", "n-006_C_012-ENDF8.0.endf"),
    ("C13", "n-006_C_013-ENDF8.0.endf"),
    ("B10", "n-005_B_010-ENDF8.0.endf"),
    ("B11", "n-005_B_011-ENDF8.0.endf"),
];

/// The default graphite thermal-scattering law: `(file, MAT)`.
pub const GRAPHITE_TSL_30P: (&str, i32) = ("tsl-reactor-graphite-30P.endf", 32);

/// Where each nuclide of [`ENDF_TAPES`] sits.
pub fn endf_layout() -> Layout {
    use outram_mc_libs::pebble_beds::htr10::{C12_ATOM_FRACTION_OF_NATURAL_C, C13_ATOM_FRACTION_OF_NATURAL_C};
    Layout {
        u235: 0,
        u238: 1,
        o16: 2,
        carbon: vec![(3, C12_ATOM_FRACTION_OF_NATURAL_C), (4, C13_ATOM_FRACTION_OF_NATURAL_C)],
        b10: 5,
        b11: 6,
    }
}

/// Kernel volume fraction of the fuelled zone: `n (r_k / R_fz)^3`.
pub fn kernel_packing_fraction() -> f64 {
    PARTICLES_PER_PEBBLE * (KERNEL_RADIUS_CM / FUEL_ZONE_RADIUS_CM).powi(3)
}
