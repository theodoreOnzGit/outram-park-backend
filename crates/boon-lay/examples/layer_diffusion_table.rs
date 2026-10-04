// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

//! **How strong a barrier is the SiC, in the random-walk model's own numbers?**
//! (lesson rung 4, gh:#531).
//!
//! Prints the diffusion coefficient the Walk-on-Spheres engine uses in each
//! TRISO layer, through the same call the walker makes
//! ([`try_get_diffusion_coeff_jiang`], the Jiang et al. 2023 two-term Arrhenius
//! correlations), for Cs-137, Sr-90, Ag-110m and Kr-85 at 1000, 1200 and
//! 1600 °C with zero fast fluence. For each case it then gives the PyC/SiC
//! contrast and the interface transmission probability
//! ([`transmission_probability`], `K = 1`) for a walker arriving at the SiC
//! from the inner PyC.
//!
//! This is a table of what the code computes, not a comparison with a
//! reference: it is the number a reader needs to judge statements such as
//! "SiC's `D` is about 10⁶ times smaller than the PyC's".
//!
//! ```bash
//! cargo run --release -p boon-lay --example layer_diffusion_table
//! ```
//!
//! # Results (2026-10-04)
//!
//! Recorded in the lesson page `docs/tutorial/src/layers.md` (step 1, "The
//! check") with the printed table.

use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::interface::transmission_probability;
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::{
    try_get_diffusion_coeff_jiang, TrisoPebbleLayerMaterial,
};
use boon_lay::Nuclide;
use uom::si::diffusion_coefficient::square_meter_per_second;
use uom::si::f64::{ArealNumberDensity, ThermodynamicTemperature};
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::ConstZero;

fn main() {
    // A caught `todo!()` still prints its panic message on stderr; the
    // table itself goes to stdout.
    let layers = [
        ("kernel", TrisoPebbleLayerMaterial::KernelUO2),
        ("buffer", TrisoPebbleLayerMaterial::Buffer),
        ("PyC", TrisoPebbleLayerMaterial::PyC),
        ("SiC", TrisoPebbleLayerMaterial::SiC),
    ];
    let nuclides = [
        ("Cs-137", Nuclide::Cs137),
        ("Sr-90", Nuclide::Sr90),
        ("Ag-110m", Nuclide::Ag110m),
        ("Kr-85", Nuclide::Kr85),
    ];
    println!(
        "{:<8} {:>6} | {:>11} {:>11} {:>11} {:>11} | {:>10} {:>11}",
        "nuclide", "T (C)", "D kernel", "D buffer", "D PyC", "D SiC", "PyC/SiC", "p(PyC->SiC)"
    );
    for (name, nuclide) in nuclides {
        for t_c in [1000.0, 1200.0, 1600.0] {
            let t = ThermodynamicTemperature::new::<degree_celsius>(t_c);
            // A coefficient the table does not have is a `todo!()` in
            // `diffusion_coeffs` (gh:#541). Catch it and print it as such
            // rather than hide the row; NaN marks it in the table.
            let d: Vec<f64> = layers
                .iter()
                .map(|(_, layer)| {
                    let layer = *layer;
                    std::panic::catch_unwind(move || {
                        try_get_diffusion_coeff_jiang(layer, nuclide, t, Some(ArealNumberDensity::ZERO))
                            .map(|d| d.get::<square_meter_per_second>())
                            .unwrap_or(f64::NAN)
                    })
                    .unwrap_or(f64::NAN)
                })
                .collect();
            let d_pyc = try_get_diffusion_coeff_jiang(
                TrisoPebbleLayerMaterial::PyC,
                nuclide,
                t,
                Some(ArealNumberDensity::ZERO),
            )
            .unwrap();
            let d_sic = try_get_diffusion_coeff_jiang(
                TrisoPebbleLayerMaterial::SiC,
                nuclide,
                t,
                Some(ArealNumberDensity::ZERO),
            )
            .unwrap();
            let p = transmission_probability(d_pyc, d_sic, 1.0);
            println!(
                "{:<8} {:>6.0} | {:>11.3e} {:>11.3e} {:>11.3e} {:>11.3e} | {:>10.3e} {:>11.3e}",
                name,
                t_c,
                d[0],
                d[1],
                d[2],
                d[3],
                d[2] / d[3],
                p
            );
        }
    }
}
