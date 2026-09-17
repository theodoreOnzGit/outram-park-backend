// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/phaseModels/fluid/fluid.{C,H}
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! Unit + V&V tests for the [`Fluid`](super::Fluid) field-state container.

use super::{Fluid, StateOfMatter};
use crate::genfoam::thermal_hydraulics::phase::phase_base::volume_fraction;
use outram_foam_basic_lib::mesh::fv_mesh::{FvMesh, FvMeshBuilder};
use outram_foam_basic_lib::primitives::Vector3;
use std::sync::Arc;
use uom::si::mass::kilogram;

/// `n`-cell, no-face, no-patch mesh for pure per-cell field arithmetic.
fn cells(n: usize) -> Arc<FvMesh> {
    Arc::new(
        FvMeshBuilder::new()
            .n_cells(n)
            .n_internal_faces(0)
            .cell_volumes(vec![1.0; n])
            .cell_centres((0..n).map(|i| Vector3::new(i as f64, 0.0, 0.0)).collect())
            .build()
            .expect("mesh should be valid"),
    )
}

#[test]
fn state_of_matter_dispatch() {
    assert!(StateOfMatter::Liquid.is_liquid());
    assert!(!StateOfMatter::Liquid.is_gas());
    assert!(StateOfMatter::Gas.is_gas());
    assert!(!StateOfMatter::Undetermined.is_liquid());
    assert!(!StateOfMatter::Undetermined.is_gas());
    assert_eq!(StateOfMatter::Liquid.name(), "liquid");
    assert_eq!(StateOfMatter::default(), StateOfMatter::Undetermined);
}

#[test]
fn new_sets_gen_foam_defaults() {
    let f = Fluid::new(cells(1), "liquid", StateOfMatter::Liquid);
    assert_eq!(f.name(), "liquid");
    assert!(f.is_liquid());
    // Dh initialised to SMALL, XLM to 1e4, markers as constructed.
    assert_eq!(f.hydraulic_diameter().internal.as_slice()[0], Fluid::SMALL);
    assert_eq!(f.lockhart_martinelli().internal.as_slice()[0], 1.0e4);
    assert_eq!(f.min_xlm(), 1.0e-4);
    assert_eq!(f.max_xlm(), 1.0e4);
    assert!(!f.is_boussinesq());
    assert_eq!(f.cumulative_continuity_error().get::<kilogram>(), 0.0);
    assert_eq!(f.alpha().internal.as_slice()[0], 0.0);
}

/// V&V — velocity-magnitude update `magU = |U|`.
///
/// Methodology: set `U = (3, 4, 0) m/s` in one cell, call
/// [`Fluid::correct_mag_velocity`]; reference `|U| = 5 m/s` (3-4-5 triangle).
/// Pass: abs. err `< 1e-12`. Result (2026-07-15): `magU = 5.0`, PASS.
#[test]
fn correct_mag_velocity_matches_hand_calc() {
    let mut f = Fluid::new(cells(1), "l", StateOfMatter::Liquid);
    f.velocity_mut().internal.as_mut_slice()[0] = Vector3::new(3.0, 4.0, 0.0);
    f.correct_mag_velocity();
    assert!((f.mag_velocity().internal.as_slice()[0] - 5.0).abs() < 1e-12);
}

/// V&V — cell mass-flux update `alphaRhoMagU = alpha * rho * magU`.
///
/// Methodology: `alpha = 0.5`, `rho = 1000 kg/m^3`, `magU = 2 m/s` in one cell;
/// reference `alphaRhoMagU = 0.5 * 1000 * 2 = 1000 kg/(m^2 s)`. Pass: abs. err
/// `< 1e-9`. Result (2026-07-15): `1000.0`, PASS.
#[test]
fn correct_alpha_rho_mag_u_matches_hand_calc() {
    let mut f = Fluid::new(cells(1), "l", StateOfMatter::Liquid);
    f.alpha_mut().internal.as_mut_slice()[0] = 0.5;
    f.density_mut().internal.as_mut_slice()[0] = 1000.0;
    f.mag_velocity_mut().internal.as_mut_slice()[0] = 2.0;
    f.correct_alpha_rho_mag_u();
    assert!((f.alpha_rho_mag_u().internal.as_slice()[0] - 1000.0).abs() < 1e-9);
}

/// V&V — thermo-residual markers partition cells by `alpha_norm` vs threshold.
///
/// Methodology: `thermoResidualAlpha = 0.1`; two cells with
/// `alpha_norm = [0.05, 0.20]`. Reference: `above = pos(alpha_norm - 0.1) =
/// [0, 1]`, `below = neg0(alpha_norm - 0.1) = [1, 0]`, and the two markers sum
/// to 1 everywhere. Result (2026-07-15): above `[0,1]`, below `[1,0]`, PASS.
#[test]
fn correct_thermo_residual_markers_partition_cells() {
    let mut f = Fluid::new(cells(2), "l", StateOfMatter::Liquid);
    f.set_thermo_residual_alpha(volume_fraction(0.1));
    f.normalized_mut()
        .internal
        .as_mut_slice()
        .copy_from_slice(&[0.05, 0.20]);
    f.correct_thermo_residual_markers();
    assert_eq!(
        f.above_thermo_residual_alpha().internal.as_slice(),
        &[0.0, 1.0]
    );
    assert_eq!(
        f.below_thermo_residual_alpha().internal.as_slice(),
        &[1.0, 0.0]
    );
}

/// V&V — kinematic viscosity `nu = mu / rho`.
///
/// Methodology: `mu = 1e-3 Pa s`, `rho = 1000 kg/m^3`; reference
/// `nu = 1e-6 m^2/s`. Pass: rel. err `< 1e-12`. Result (2026-07-15): `1e-6`,
/// PASS.
#[test]
fn nu_matches_hand_calc() {
    let mut f = Fluid::new(cells(1), "l", StateOfMatter::Liquid);
    f.mu_mut().internal.as_mut_slice()[0] = 1.0e-3;
    f.density_mut().internal.as_mut_slice()[0] = 1000.0;
    let nu = f.nu();
    assert!((nu.internal.as_slice()[0] - 1.0e-6).abs() / 1.0e-6 < 1e-12);
}

/// The Boussinesq-sensitive density switches between `rho0` and thermodynamic
/// `rho`, mirroring `fluid::rho(isVariableIfBoussinesq)`.
#[test]
fn rho_boussinesq_selection() {
    let mut f = Fluid::new(cells(1), "l", StateOfMatter::Liquid);
    f.density_mut().internal.as_mut_slice()[0] = 1000.0;
    f.reference_density_mut().internal.as_mut_slice()[0] = 999.0;

    // Non-Boussinesq: always the thermodynamic density.
    assert_eq!(f.rho(false).internal.as_slice()[0], 1000.0);

    f.set_boussinesq(true);
    // Boussinesq, constant requested (default): the reference density.
    assert_eq!(f.rho(false).internal.as_slice()[0], 999.0);
    // Boussinesq, variable explicitly requested: the thermodynamic density.
    assert_eq!(f.rho(true).internal.as_slice()[0], 1000.0);
}
