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

//! # Direct Monte Carlo <-> GeN-Foam coupling, driven to a steady state
//!
//! [`nee_soon::direct_coupling::McGenFoamDirect`] iterates `outram-mc` against
//! GeN-Foam's own lumped thermal region until the fuel temperature and the
//! eigenvalue both stop moving. This checks that the loop closes, that the
//! feedback goes the right way, and that a non-converging run fails loudly.
//!
//! ## What is asserted, and why each can fail
//!
//! - **The loop closes.** A fixed point exists and is reached inside the cap.
//! - **The temperature actually moved.** A coupled solve that converges on
//!   iteration one because nothing happened has tested nothing; the history is
//!   checked for real movement.
//! - **Doppler feedback is negative.** Heating a uranium system broadens the
//!   U-238 capture resonances, so `k` must fall as `T` rises. A positive
//!   coefficient here would be a physics defect, not a tolerance question. It
//!   is asserted only when the temperature swing is large enough for the signal
//!   to clear the Monte Carlo noise, and skipped with a printed note otherwise
//!   rather than asserted on noise.
//!
//! **Harness and coupling check, not V&V.** No experiment, no benchmark. The
//! lumped thermal parameters below are illustrative, not an HTR-10 or any other
//! real plant's.

use nee_soon::direct_coupling::{Convergence, McGenFoamDirect};
use outram_foam_appbuilder_lib::genfoam::multi_region::outer_iteration::LumpedThermal;
use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, SurfaceKind, XPlane, YPlane, ZPlane};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::transport_csg::SourceBox;
use uom::si::f64::{HeatCapacity, Power, ThermalConductance, ThermodynamicTemperature, Time};
use uom::si::heat_capacity::joule_per_kelvin;
use uom::si::power::watt;
use uom::si::thermal_conductance::watt_per_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

const HALF: f64 = 10.0;
/// Region volume [m^3] — a 20 cm cube.
const VOLUME_M3: f64 = 0.008;

fn model() -> (Geometry, Vec<Material>, Vec<Nuclide>, SourceBox) {
    let refl = BoundaryType::Reflective;
    let surfaces = vec![
        SurfaceKind::XPlane(XPlane {
            x0: -HALF,
            bc: refl,
        }),
        SurfaceKind::XPlane(XPlane { x0: HALF, bc: refl }),
        SurfaceKind::YPlane(YPlane {
            y0: -HALF,
            bc: refl,
        }),
        SurfaceKind::YPlane(YPlane { y0: HALF, bc: refl }),
        SurfaceKind::ZPlane(ZPlane {
            z0: -HALF,
            bc: refl,
        }),
        SurfaceKind::ZPlane(ZPlane { z0: HALF, bc: refl }),
    ];
    let cell = Cell::material(
        1,
        vec![
            RegionToken::HalfSpace {
                surface_idx: 0,
                sense: HalfSpaceSense::Outside,
            },
            RegionToken::HalfSpace {
                surface_idx: 1,
                sense: HalfSpaceSense::Inside,
            },
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 2,
                sense: HalfSpaceSense::Outside,
            },
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 3,
                sense: HalfSpaceSense::Inside,
            },
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 4,
                sense: HalfSpaceSense::Outside,
            },
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 5,
                sense: HalfSpaceSense::Inside,
            },
            RegionToken::Intersection,
        ],
        0,
        293.6,
    );
    let geometry = Geometry {
        surfaces,
        cells: vec![cell],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0],
        }],
        lattices: vec![],
        root_universe: 0,
    };
    // Enough U-238 that Doppler has something to act on: a resonance absorber
    // is what makes the feedback coefficient negative and measurable.
    let materials = vec![Material {
        id: 1,
        name: "uranium".into(),
        temperature: 293.6,
        components: vec![
            NuclideComponent {
                nuclide_idx: 0,
                atom_density: 2.0e-2,
            },
            NuclideComponent {
                nuclide_idx: 1,
                atom_density: 2.5e-2,
            },
        ],
    }];
    let nuclides = vec![
        Nuclide::from_core("U235").expect("U235 in CORE WMP library"),
        Nuclide::from_core("U238").expect("U238 in CORE WMP library"),
    ];
    let source = SourceBox {
        lower: Position::new(-HALF, -HALF, -HALF),
        upper: Position::new(HALF, HALF, HALF),
    };
    (geometry, materials, nuclides, source)
}

/// A lumped node that heats appreciably at the chosen power, so the coupling
/// has a real temperature swing to converge over.
fn thermal_node() -> LumpedThermal {
    LumpedThermal::new(
        "core",
        HeatCapacity::new::<joule_per_kelvin>(5.0e4),
        ThermalConductance::new::<watt_per_kelvin>(2.0e3),
        ThermodynamicTemperature::new::<kelvin>(293.6),
        ThermodynamicTemperature::new::<kelvin>(293.6),
        VOLUME_M3,
    )
}

#[test]
fn monte_carlo_and_genfoam_reach_a_coupled_steady_state() {
    let (geometry, materials, nuclides, source) = model();
    let mut coupling = McGenFoamDirect::new(
        geometry,
        materials,
        nuclides,
        source,
        thermal_node(),
        Power::new::<watt>(5.0e5),
        VOLUME_M3,
    )
    .with_particles(400)
    .with_batches(10, 25)
    .with_time_step(Time::new::<second>(20.0));

    let criteria = Convergence {
        temperature_k: 2.0,
        // Above the Monte Carlo one-sigma at this particle count: asking the
        // loop to converge tighter than its own noise would stop it on chance.
        k_eff: 6.0e-3,
        max_iterations: 25,
    };

    let result = coupling
        .solve_steady_state(criteria)
        .expect("the coupled loop should reach a fixed point");

    println!(
        "coupled steady state: T = {:.2} K, k = {:.5} +/- {:.5}",
        result.temperature_k, result.k_eff, result.k_std
    );
    println!("outer iterations: {}", result.history.len());
    for it in &result.history {
        println!(
            "  {:>2}  T_in {:>8.2} K  k {:.5} +/- {:.5}  -> T_out {:>8.2} K",
            it.iteration, it.temperature_k, it.k_eff, it.k_std, it.temperature_after_k
        );
    }

    assert!(result.temperature_k.is_finite() && result.temperature_k > 0.0);
    assert!(result.k_eff.is_finite() && result.k_eff > 0.0);
    assert!(
        result.history.len() >= 2,
        "converging on the first iteration means nothing was coupled"
    );

    // The temperature must genuinely have moved, or the loop proved nothing.
    let t_first = result.history[0].temperature_k;
    let t_last = result.temperature_k;
    let swing = t_last - t_first;
    println!(
        "temperature swing: {swing:+.2} K over {} iterations",
        result.history.len()
    );
    assert!(
        swing.abs() > 5.0,
        "the fuel temperature barely moved ({swing:+.2} K); the coupling did not exercise feedback"
    );
    assert!(
        swing > 0.0,
        "a heated core must end hotter than it started, got {swing:+.2} K"
    );

    // Doppler: k must fall as T rises. Only assert it where the signal clears
    // the noise -- differencing two Monte Carlo eigenvalues gives roughly
    // sqrt(2)*sigma, and asserting below that would be asserting on noise.
    match result.feedback_pcm_per_k() {
        Some(alpha) => {
            let sigma_k = result.k_std;
            let noise_pcm_per_k = 2.0f64.sqrt() * sigma_k / result.k_eff * 1.0e5 / swing.abs();
            println!("temperature coefficient: {alpha:+.2} pcm/K (noise floor ~{noise_pcm_per_k:.2} pcm/K)");
            if alpha.abs() > 2.0 * noise_pcm_per_k {
                assert!(
                    alpha < 0.0,
                    "Doppler feedback must be NEGATIVE in a uranium system -- heating broadens \
                     the U-238 capture resonances -- but measured {alpha:+.2} pcm/K, which is \
                     {:.1}x the {noise_pcm_per_k:.2} pcm/K noise floor and so not a statistical \
                     accident",
                    alpha.abs() / noise_pcm_per_k
                );
            } else {
                println!(
                    "  (not asserted: |{alpha:+.2}| is within 2x the noise floor, so the sign \
                     is not resolved at this particle count)"
                );
            }
        }
        None => panic!("temperature moved, so a coefficient must be computable"),
    }
}

/// An impossible tolerance fails loudly, carrying the residuals.
#[test]
fn a_non_converging_solve_reports_its_residuals() {
    let (geometry, materials, nuclides, source) = model();
    let mut coupling = McGenFoamDirect::new(
        geometry,
        materials,
        nuclides,
        source,
        thermal_node(),
        Power::new::<watt>(5.0e5),
        VOLUME_M3,
    )
    .with_particles(200)
    .with_batches(5, 10);

    let err = coupling
        .solve_steady_state(Convergence {
            temperature_k: 1.0e-12,
            k_eff: 1.0e-12,
            max_iterations: 3,
        })
        .expect_err("an impossible tolerance must not silently succeed");

    let msg = err.to_string();
    println!("non-convergence reported as: {msg}");
    assert!(msg.contains("did not converge"));
    // The residuals must be in the message: "close but capped" and "diverging"
    // need opposite responses, and a bare failure cannot tell them apart.
    assert!(
        msg.contains("dT"),
        "the message must carry the temperature residual: {msg}"
    );
    assert!(
        msg.contains("dk"),
        "the message must carry the eigenvalue residual: {msg}"
    );
}
