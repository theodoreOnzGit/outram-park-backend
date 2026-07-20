// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/physicsModels/dragModels/
//             FSDragFactor.{C,H} and phasePairs/FSPair.{C,H}
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `solver::porous_drag` — fluid-structure drag-tensor assembly
//!
//! Assembles the porous-medium **fluid-structure drag coefficient** `Kd` that
//! the momentum equation ([`super::one_phase`]) sinks velocity through. This is
//! the field-wide side of GeN-Foam's `FSPair::Kd()`: the [`super::super::closures::fs_drag`]
//! family gives the *scalar* Darcy friction factor `f(Re)` per cell; this module
//! turns that into the per-cell drag coefficient the solver assembles into
//! `UEqn`.
//!
//! ## The isotropic drag coefficient
//!
//! Port of `FSDragFactor::correctField` (isotropic branch). For each cell the
//! diagonal drag coefficient is
//!
//! ```text
//! Kd = 0.5 / Dh * (1 - alpha_s) * rho * max(|U|, minMagU) * f(Re)
//! ```
//!
//! where `alpha_s` is the **structure** volume fraction (so `1 - alpha_s` is the
//! open/fluid fraction `alpha_f`), `Dh` the hydraulic diameter, `rho` the fluid
//! density, `|U|` the velocity magnitude, and `f` the Darcy friction factor from
//! the wall-friction closure. The momentum sink assembled from it is `Kd * U`
//! (an implicit `fvm::sp` term), i.e. the Darcy-Weisbach body force
//! `f/Dh * rho |U| U / 2` scaled by the open fraction.
//!
//! Only the **isotropic** (`Kd = Kd * I`) case is ported here; the anisotropic
//! (localX/Y/Z) branch of `FSDragFactor` is a documented follow-up.
//!
//! ## Units
//!
//! `Kd` has dimensions `kg / (m^3 s)` (a volumetric drag coefficient): `Kd * U`
//! is a force per unit volume `N/m^3 = Pa/m`, balancing a pressure gradient. It
//! is stored in a bare-`f64` [`VolScalarField`] following the basic-lib field
//! convention; the scalar tuning parameters that cross the public API carry
//! their [`uom`] types.

use outram_foam_basic_lib::fields::vol_field::VolScalarField;

use crate::genfoam::thermal_hydraulics::closures::fs_drag::FsWallFriction;
use crate::genfoam::thermal_hydraulics::phase::fluid::Fluid;
use crate::genfoam::thermal_hydraulics::phase::phase_pair::fs_reynolds;
use crate::genfoam::thermal_hydraulics::units::{reynolds_number, ReynoldsNumber};

/// Configuration of the isotropic fluid-structure porous drag.
///
/// Holds the wall-friction closure ([`FsWallFriction`], the correlation that
/// maps a Reynolds number to a Darcy friction factor) plus the two numerical
/// floors upstream applies while assembling `Kd`: a minimum velocity magnitude
/// `minMagU` (keeps the drag finite as `U -> 0`) and a minimum Reynolds number
/// (keeps `f(Re)` off its `Re -> 0` singularity).
#[derive(Debug, Clone)]
pub struct PorousDrag {
    /// The fluid-structure wall-friction correlation `f(Re)`.
    pub friction: FsWallFriction,
    /// Minimum velocity magnitude `minMagU` [m/s] used in the drag coefficient
    /// (upstream `PIMPLE/minMagU`, default `0`). A small positive value avoids
    /// a vanishing drag at stagnation.
    pub min_mag_u: f64,
    /// Reynolds-number floor for the friction-factor evaluation (dimensionless).
    /// Upstream typically clamps at `Re = 1` to avoid the `1/Re` laminar
    /// singularity; default here `1.0`.
    pub min_reynolds: ReynoldsNumber,
}

impl PorousDrag {
    /// Build a porous drag from a wall-friction closure, with `minMagU = 0` and
    /// a Reynolds floor of `1.0`.
    #[must_use]
    pub fn new(friction: FsWallFriction) -> Self {
        Self {
            friction,
            min_mag_u: 0.0,
            min_reynolds: reynolds_number(1.0),
        }
    }

    /// Set the minimum velocity magnitude `minMagU` [m/s].
    #[must_use]
    pub fn with_min_mag_u(mut self, min_mag_u: f64) -> Self {
        self.min_mag_u = min_mag_u;
        self
    }

    /// Assemble the isotropic drag-coefficient field `Kd` [kg/(m^3 s)] from the
    /// current fluid state.
    ///
    /// Port of `FSDragFactor::correctField` (isotropic branch). Reads the
    /// fluid's volume fraction `alpha_f` (the open fraction `1 - alpha_s`),
    /// density, velocity magnitude, hydraulic diameter, and kinematic viscosity;
    /// evaluates the cell Reynolds number and friction factor; and returns
    ///
    /// ```text
    /// Kd = 0.5 / Dh * alpha_f * rho * max(|U|, minMagU) * f(Re)
    /// ```
    ///
    /// The caller assembles `fvm::sp_vec(&Kd, U)` into the momentum matrix.
    ///
    /// # Parameters
    /// - `fluid` — the fluid phase supplying `alpha`, `rho`, `magU`, `Dh`, `mu`.
    #[must_use]
    pub fn assemble_kd(&self, fluid: &Fluid) -> VolScalarField {
        let mesh = fluid.mesh().clone();
        let nu = fluid.nu();
        let re = fs_reynolds(
            fluid.alpha(),
            fluid.mag_velocity(),
            fluid.hydraulic_diameter(),
            &nu,
            self.min_reynolds,
        );

        let alpha = fluid.alpha().internal.as_slice();
        let rho = fluid.density().internal.as_slice();
        let mag_u = fluid.mag_velocity().internal.as_slice();
        let dh = fluid.hydraulic_diameter().internal.as_slice();
        let re_s = re.internal.as_slice();

        let mut kd = VolScalarField::zeros("Kd", mesh);
        let out = kd.internal.as_mut_slice();
        for i in 0..out.len() {
            let f = self
                .friction
                .friction_factor(reynolds_number(re_s[i]))
                .get::<uom::si::ratio::ratio>();
            let mag = mag_u[i].max(self.min_mag_u);
            out[i] = 0.5 / dh[i] * alpha[i] * rho[i] * mag * f;
        }
        kd
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::prelude::*;
    use std::sync::Arc;

    use crate::genfoam::thermal_hydraulics::phase::fluid::StateOfMatter;

    fn one_cell_mesh() -> Arc<FvMesh> {
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(1)
                .n_internal_faces(0)
                .owner(vec![0, 0])
                .neighbour(vec![])
                .patches(vec![
                    BoundaryPatch::new("left", 0, 1, PatchKind::Wall),
                    BoundaryPatch::new("right", 1, 1, PatchKind::Wall),
                ])
                .cell_volumes(vec![1.0])
                .cell_centres(vec![Vector3::new(0.5, 0.0, 0.0)])
                .face_area_vectors(vec![
                    Vector3::new(-1.0, 0.0, 0.0),
                    Vector3::new(1.0, 0.0, 0.0),
                ])
                .face_centres(vec![
                    Vector3::new(0.0, 0.0, 0.0),
                    Vector3::new(1.0, 0.0, 0.0),
                ])
                .build()
                .unwrap(),
        )
    }

    /// V&V — laminar drag coefficient reduces to the exact Stokes/Darcy value.
    ///
    /// Methodology: in the laminar regime the Darcy friction factor obeys the
    /// exact analytical limit `f = 64/Re` (`f*Re = 64`), so the assembled drag
    /// coefficient collapses to a velocity-independent constant:
    ///
    /// ```text
    /// Kd = 0.5/Dh * rho * |U| * (64/Re)
    ///    = 0.5/Dh * rho * |U| * 64 * mu / (rho |U| Dh)
    ///    = 32 * mu / Dh^2
    /// ```
    ///
    /// This is the porous-medium Darcy permeability limit and is independent of
    /// velocity and density — a clean closed-form check on the `Kd` assembly
    /// wiring (Reynolds number, friction closure, and the `0.5 alpha rho |U| f
    /// / Dh` combination).
    ///
    /// Inputs: alpha_f = 1, rho = 1000 kg/m^3, mu = 1.0e-3 Pa s, Dh = 0.01 m,
    /// |U| = 0.05 m/s (so Re = rho |U| Dh / mu = 500, laminar). The
    /// `ReynoldsPower{coeff: 64, exp: -1, constant: 0}` closure is the exact
    /// `f = 64/Re`.
    /// Hand calc: Kd = 32 * 1.0e-3 / 0.01^2 = 320.0 kg/(m^3 s).
    ///
    /// Result (2026-07-15): Kd = 320.000 kg/(m^3 s), relative error < 1e-12.
    /// Pass.
    #[test]
    fn laminar_drag_matches_stokes_darcy_limit() {
        let mesh = one_cell_mesh();
        let mut fluid = Fluid::new(mesh.clone(), "coolant", StateOfMatter::Liquid);
        fluid.alpha_mut().internal.as_mut_slice()[0] = 1.0;
        fluid.density_mut().internal.as_mut_slice()[0] = 1000.0;
        fluid.mu_mut().internal.as_mut_slice()[0] = 1.0e-3;
        fluid.hydraulic_diameter_mut().internal.as_mut_slice()[0] = 0.01;
        fluid.velocity_mut().internal.as_mut_slice()[0] = Vector3::new(0.05, 0.0, 0.0);
        fluid.correct_mag_velocity();

        let drag = PorousDrag::new(FsWallFriction::ReynoldsPower {
            coeff: 64.0,
            exp: -1.0,
            constant: 0.0,
        });
        let kd = drag.assemble_kd(&fluid);

        let expected = 32.0 * 1.0e-3 / (0.01 * 0.01); // 320.0
        let got = kd.internal.as_slice()[0];
        assert!(
            (got - expected).abs() / expected < 1e-12,
            "Kd = {got} kg/(m^3 s), expected {expected}"
        );
    }
}
