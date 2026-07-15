// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/physicsModels/phasePairs/
//             {FSPair,FFPair}.{C,H}  (the Reynolds / relative-velocity cores)
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `phase_pair` — pairwise dimensionless numbers (Reynolds, relative velocity)
//!
//! Port of the **kinematic core** of GeN-Foam's `FSPair` (fluid–structure) and
//! `FFPair` (fluid–fluid) classes: the pairwise **Reynolds number** and the
//! fluid–fluid **relative-velocity magnitude**. As the upstream `FSPair`
//! documentation puts it, a Reynolds number *"cannot be defined for a fluid by
//! itself, it needs to be geometrically constrained in a domain (so that a
//! hydraulic diameter can be defined)"* — hence it lives on the phase **pair**,
//! not the [`Fluid`] itself.
//!
//! [`Fluid`]: super::fluid::Fluid
//!
//! ## Scope: kinematics only, no closure tables
//!
//! The full upstream `FSPair`/`FFPair` also assemble the drag tensor `Kd`, the
//! heat-transfer coefficient `htc`, the contact-partition fraction, interfacial
//! area, virtual mass, etc. — every one of which is a *closure* (owned by the
//! `closures` sub-modules) or *solver* assembly step. Those are **out of scope
//! here**; this module ports only the phase-state-derived dimensionless numbers
//! the closures consume as inputs. Each is a pure per-cell function of already-
//! assembled fields, so it is independently verifiable against a hand
//! calculation.
//!
//! ## Definitions
//!
//! Fluid–structure isotropic Reynolds number (`FSPair::correct`):
//!
//! ```text
//! Re_FS = max( alpha_norm * |U| * D_h / nu ,  Re_min )
//! ```
//!
//! Fluid–fluid relative velocity and Reynolds number (`FFPair::correct`):
//!
//! ```text
//! |U_r| = |U_1 - U_2|
//! Re_FF = max( |U_r| * D_h,disp / nu_cont ,  Re_min )
//! ```
//!
//! where `alpha_norm` is the [`Fluid::normalized`] volume fraction, `nu = mu/rho`
//! the kinematic viscosity ([`Fluid::nu`]), `D_h` the hydraulic diameter, and
//! `D_h,disp` / `nu_cont` the dispersed-phase diameter / continuous-phase
//! kinematic viscosity that `FFPair` blends from the two fluids.
//!
//! [`Fluid::normalized`]: super::fluid::Fluid::normalized
//! [`Fluid::nu`]: super::fluid::Fluid::nu

use outram_foam_basic_lib::fields::vol_field::{VolScalarField, VolVectorField};

use crate::genfoam::thermal_hydraulics::units::ReynoldsNumber;

use uom::si::ratio::ratio;

/// Fluid–structure isotropic Reynolds number field
/// `Re = max(alpha_norm * |U| * D_h / nu, Re_min)`.
///
/// Port of `FSPair::correct`'s `Re_ = fluid.normalized()*mag(U)*Dh/nu` followed
/// by `Re_ = max(Re_, minRe_)`. All four input fields are per-cell bare `f64`
/// (see the [`super::fluid`] unit table); `min_re` is a [`uom`]-typed
/// [`ReynoldsNumber`] floor. The result is a fresh [`VolScalarField`] named
/// `Re` on the same mesh as `normalized_alpha`.
///
/// # Panics
/// Debug-asserts every input shares the mesh cell count of `normalized_alpha`.
#[must_use]
pub fn fs_reynolds(
    normalized_alpha: &VolScalarField,
    mag_u: &VolScalarField,
    dh: &VolScalarField,
    nu: &VolScalarField,
    min_re: ReynoldsNumber,
) -> VolScalarField {
    let a = normalized_alpha.internal.as_slice();
    let m = mag_u.internal.as_slice();
    let d = dh.internal.as_slice();
    let n = nu.internal.as_slice();
    debug_assert_eq!(a.len(), m.len(), "mag_u must match normalized_alpha");
    debug_assert_eq!(a.len(), d.len(), "dh must match normalized_alpha");
    debug_assert_eq!(a.len(), n.len(), "nu must match normalized_alpha");
    let floor = min_re.get::<ratio>();
    let mut re = VolScalarField::zeros("Re", normalized_alpha.mesh.clone());
    let out = re.internal.as_mut_slice();
    for i in 0..out.len() {
        out[i] = (a[i] * m[i] * d[i] / n[i]).max(floor);
    }
    re
}

/// Fluid–fluid relative-velocity magnitude field `|U_r| = |U_1 - U_2|` (`m/s`).
///
/// Port of `FFPair::correct`'s `magUr_ = mag(U1 - U2)`. Result is a fresh
/// [`VolScalarField`] named `magUr` on the same mesh as `u1`.
///
/// # Panics
/// Debug-asserts `u1` and `u2` share a cell count.
#[must_use]
pub fn ff_relative_velocity_magnitude(u1: &VolVectorField, u2: &VolVectorField) -> VolScalarField {
    let a = u1.internal.as_slice();
    let b = u2.internal.as_slice();
    debug_assert_eq!(a.len(), b.len(), "u2 must match u1");
    let mut mag_ur = VolScalarField::zeros("magUr", u1.mesh.clone());
    let out = mag_ur.internal.as_mut_slice();
    for i in 0..out.len() {
        out[i] = (a[i] - b[i]).mag();
    }
    mag_ur
}

/// Fluid–fluid Reynolds number field
/// `Re = max(|U_r| * D_h,disp / nu_cont, Re_min)`.
///
/// Port of `FFPair::correct`'s
/// `Re_ = max(magUr_*DhDispersed_/nuContinuous_, minRe_)`. `mag_ur` is typically
/// the output of [`ff_relative_velocity_magnitude`]; `dh_dispersed` is the
/// dispersed-phase hydraulic diameter and `nu_continuous` the continuous-phase
/// kinematic viscosity (`m^2/s`). Result is a fresh [`VolScalarField`] named
/// `Re` on the same mesh as `mag_ur`.
///
/// # Panics
/// Debug-asserts every input shares the cell count of `mag_ur`.
#[must_use]
pub fn ff_reynolds(
    mag_ur: &VolScalarField,
    dh_dispersed: &VolScalarField,
    nu_continuous: &VolScalarField,
    min_re: ReynoldsNumber,
) -> VolScalarField {
    let m = mag_ur.internal.as_slice();
    let d = dh_dispersed.internal.as_slice();
    let n = nu_continuous.internal.as_slice();
    debug_assert_eq!(m.len(), d.len(), "dh_dispersed must match mag_ur");
    debug_assert_eq!(m.len(), n.len(), "nu_continuous must match mag_ur");
    let floor = min_re.get::<ratio>();
    let mut re = VolScalarField::zeros("Re", mag_ur.mesh.clone());
    let out = re.internal.as_mut_slice();
    for i in 0..out.len() {
        out[i] = (m[i] * d[i] / n[i]).max(floor);
    }
    re
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genfoam::thermal_hydraulics::units::reynolds_number;
    use outram_foam_basic_lib::mesh::fv_mesh::{FvMesh, FvMeshBuilder};
    use outram_foam_basic_lib::primitives::Vector3;
    use std::sync::Arc;

    /// Build a single-cell, no-face, no-patch mesh for pure per-cell arithmetic
    /// tests (all these helpers touch only the internal cell field).
    fn one_cell_mesh() -> Arc<FvMesh> {
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(1)
                .n_internal_faces(0)
                .cell_volumes(vec![1.0])
                .cell_centres(vec![Vector3::new(0.0, 0.0, 0.0)])
                .build()
                .expect("one_cell_mesh should be valid"),
        )
    }

    fn scalar_cell(mesh: &Arc<FvMesh>, value: f64) -> VolScalarField {
        VolScalarField::uniform("f", mesh.clone(), value)
    }

    /// V&V — FS Reynolds vs a closed-form hand calculation.
    ///
    /// Methodology: `Re = alpha_norm * |U| * D_h / nu`, evaluated in one cell
    /// with `alpha_norm = 0.5`, `|U| = 2 m/s`, `D_h = 0.01 m`, `nu = 1e-6 m^2/s`,
    /// floor `Re_min = 1`. Reference (hand calc):
    /// `Re = 0.5 * 2 * 0.01 / 1e-6 = 1.0e4`. Pass criterion: relative error
    /// `< 1e-12`.
    ///
    /// Result (2026-07-15): `Re = 10000.0000000000` exactly (rel. err 0.0),
    /// PASS. Confirms the isotropic FSPair Reynolds core reproduces the
    /// analytic value with no floor interference (`1e4 >> 1`).
    #[test]
    fn fs_reynolds_matches_hand_calc() {
        let mesh = one_cell_mesh();
        let re = fs_reynolds(
            &scalar_cell(&mesh, 0.5),
            &scalar_cell(&mesh, 2.0),
            &scalar_cell(&mesh, 0.01),
            &scalar_cell(&mesh, 1.0e-6),
            reynolds_number(1.0),
        );
        let got = re.internal.as_slice()[0];
        assert!((got - 1.0e4).abs() / 1.0e4 < 1e-12, "Re = {got}");
    }

    /// V&V — the `Re_min` floor engages when the raw Reynolds falls below it.
    ///
    /// Methodology: same formula with a near-stagnant cell (`|U| = 1e-9 m/s`)
    /// so the raw `Re ~ 5e-6`; floor `Re_min = 1`. Reference: the result must
    /// clamp to exactly `1.0`. Result (2026-07-15): `Re = 1.0`, PASS.
    #[test]
    fn fs_reynolds_applies_floor() {
        let mesh = one_cell_mesh();
        let re = fs_reynolds(
            &scalar_cell(&mesh, 0.5),
            &scalar_cell(&mesh, 1.0e-9),
            &scalar_cell(&mesh, 0.01),
            &scalar_cell(&mesh, 1.0e-6),
            reynolds_number(1.0),
        );
        assert_eq!(re.internal.as_slice()[0], 1.0);
    }

    /// V&V — FF relative velocity and Reynolds vs a hand calculation.
    ///
    /// Methodology: `U_1 = (3,4,0) m/s`, `U_2 = (0,0,0)` gives
    /// `|U_r| = 5 m/s` (3-4-5 triangle); then
    /// `Re = |U_r| * D_h,disp / nu_cont = 5 * 0.002 / 1e-5 = 1000`.
    /// Floor `Re_min = 1`. Pass criterion: rel. err `< 1e-12` for both.
    /// Result (2026-07-15): `|U_r| = 5.0`, `Re = 1000.0` exactly, PASS.
    #[test]
    fn ff_relative_velocity_and_reynolds() {
        let mesh = one_cell_mesh();
        let u1 = VolVectorField::uniform("U1", mesh.clone(), Vector3::new(3.0, 4.0, 0.0));
        let u2 = VolVectorField::zero("U2", mesh.clone());
        let mag_ur = ff_relative_velocity_magnitude(&u1, &u2);
        let got_ur = mag_ur.internal.as_slice()[0];
        assert!((got_ur - 5.0).abs() < 1e-12, "|Ur| = {got_ur}");

        let re = ff_reynolds(
            &mag_ur,
            &scalar_cell(&mesh, 0.002),
            &scalar_cell(&mesh, 1.0e-5),
            reynolds_number(1.0),
        );
        let got_re = re.internal.as_slice()[0];
        assert!((got_re - 1000.0).abs() / 1000.0 < 1e-12, "Re = {got_re}");
    }
}
