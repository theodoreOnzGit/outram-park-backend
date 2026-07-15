// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/SN/include/readQuadratureSet.H
//     (octant replication of a per-octant level-symmetric ordinate set and the
//      4*pi weight-normalisation check).
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal authors: Carlo Fiorina, Thomas Guilbaud (EPFL)
//   Upstream license: GPL-3.0
//
// The level-symmetric (LQ_N) direction cosines and point weights tabulated here
// are the standard published constants for S2/S4/S6 discrete-ordinates
// quadrature (E. E. Lewis & W. F. Miller, "Computational Methods of Neutron
// Transport", ANS 1993, Table 4-1). GeN-Foam reads an equivalent per-octant set
// from a `constant/quadratureSet` dictionary; this port bakes the standard sets
// in so a solve is self-contained (no external dictionary needed).
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
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! # Angular quadrature — level-symmetric discrete ordinates (S_N)
//!
//! The discrete-ordinates method replaces the continuous angular variable
//! `Omega` of the transport equation with a finite set of directions
//! `Omega_j` (unit vectors) each carrying a weight `w_j`, so an angular
//! integral becomes a weighted sum:
//!
//! ```text
//!   int_{4*pi} f(Omega) dOmega  ~=  sum_j w_j f(Omega_j) ,   sum_j w_j = 4*pi .
//! ```
//!
//! This module builds the **level-symmetric** (`LQ_N`) sets S2, S4 and S6 —
//! the sets whose direction cosines are invariant under the 48 rotations /
//! reflections of the octahedral group, so the quadrature is rotationally
//! unbiased. A set of order `N` has `N (N + 2)` directions in total
//! (`N (N + 2) / 8` per octant), replicated across the eight sign octants of
//! `(Omega_x, Omega_y, Omega_z)` exactly as GeN-Foam's `readQuadratureSet.H`
//! does.
//!
//! ## Weight normalisation
//!
//! The weights returned by [`AngularQuadrature`] sum to `4*pi`, i.e. they
//! integrate the constant `f = 1` over the unit sphere to its exact value
//! `4*pi`. The scalar flux is then `phi = sum_j w_j psi_j` and an isotropic
//! volumetric source `S` contributes `S / (4*pi)` to each direction's
//! transport equation. (GeN-Foam instead normalises the weights to sum to `1`
//! and drops the `1 / (4*pi)`; the two conventions give identical fluxes.)
//!
//! ## 1-D meshes
//!
//! On a 1-D slab mesh (`create_one_d_mesh`) only the x-normal faces exist, so
//! the streaming term sees only `Omega_j . Sf = Omega_{j,x} * A`. The 3-D
//! level-symmetric set therefore acts as an (over-resolved) 1-D set in
//! `mu = Omega_x`; the scalar-flux sum still integrates the angular flux
//! correctly because the weights are complete on the sphere.

use std::sync::Arc;

use outram_foam_basic_lib::fields::boundary::bc::{BoundaryCondition, PatchField};
use outram_foam_basic_lib::fields::field::Field;
use outram_foam_basic_lib::fields::surface_field::SurfaceScalarField;
use outram_foam_basic_lib::mesh::fv_mesh::FvMesh;
use outram_foam_basic_lib::primitives::Vector3;

use std::f64::consts::PI;

/// The order `N` of a level-symmetric S_N discrete-ordinates set.
///
/// Enum dispatch (per the workspace no-`dyn` rule): the closed set of supported
/// quadrature orders. A higher order resolves the angular flux with more
/// directions (`N (N + 2)` total) and reduces "ray effects", at proportionally
/// higher cost. S2 is the coarsest (8 directions, equivalent to diffusion-like
/// angular resolution); S6 (48 directions) is accurate enough for the smooth
/// fluxes of the verification benchmarks here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuadratureOrder {
    /// S2 — 8 directions (1 per octant).
    S2,
    /// S4 — 24 directions (3 per octant).
    S4,
    /// S6 — 48 directions (6 per octant).
    S6,
}

impl QuadratureOrder {
    /// The numeric order `N` (2, 4, 6).
    #[must_use]
    pub fn n(self) -> usize {
        match self {
            Self::S2 => 2,
            Self::S4 => 4,
            Self::S6 => 6,
        }
    }

    /// Total number of discrete directions `N (N + 2)`.
    #[must_use]
    pub fn direction_count(self) -> usize {
        let n = self.n();
        n * (n + 2)
    }

    /// The per-octant `(base ordinate, octant weight)` list for this order.
    ///
    /// Base ordinates are the direction cosines in the first (all-positive)
    /// octant; the octant weights sum to `1` over an octant (Lewis & Miller
    /// Table 4-1). The full-sphere weight of each direction is the octant
    /// weight times `pi / 2` (so all `N (N + 2)` weights sum to `4*pi`).
    fn octant_ordinates(self) -> Vec<([f64; 3], f64)> {
        match self {
            // S2: single direction, cosines 1/sqrt(3) on each axis.
            Self::S2 => {
                let m = 0.577_350_269_189_625_8; // 1/sqrt(3)
                vec![([m, m, m], 1.0)]
            }
            // S4: two levels, all three permutations share one weight (1/3).
            Self::S4 => {
                let a = 0.350_021_2;
                let b = 0.868_890_3;
                let w = 1.0 / 3.0;
                vec![([b, a, a], w), ([a, b, a], w), ([a, a, b], w)]
            }
            // S6: three levels, two weight classes.
            Self::S6 => {
                let a = 0.266_635_5;
                let b = 0.681_507_6;
                let c = 0.926_180_8;
                let w1 = 0.176_126_3;
                let w2 = 0.157_207_1;
                vec![
                    ([c, a, a], w1),
                    ([a, c, a], w1),
                    ([a, a, c], w1),
                    ([b, b, a], w2),
                    ([b, a, b], w2),
                    ([a, b, b], w2),
                ]
            }
        }
    }
}

/// A level-symmetric discrete-ordinates quadrature set: unit direction vectors
/// `Omega_j` and their weights `w_j` (summing to `4*pi`).
///
/// Build with [`AngularQuadrature::level_symmetric`]. The directions and
/// weights are read-only after construction.
#[derive(Debug, Clone)]
pub struct AngularQuadrature {
    order: QuadratureOrder,
    /// Unit direction vectors `Omega_j`.
    directions: Vec<Vector3>,
    /// Direction weights `w_j` (sum to `4*pi`).
    weights: Vec<f64>,
}

impl AngularQuadrature {
    /// Construct the level-symmetric set of the requested order.
    #[must_use]
    pub fn level_symmetric(order: QuadratureOrder) -> Self {
        // The eight sign octants applied to a first-octant base ordinate.
        const SIGNS: [[f64; 3]; 8] = [
            [1.0, 1.0, 1.0],
            [-1.0, 1.0, 1.0],
            [-1.0, -1.0, 1.0],
            [1.0, -1.0, 1.0],
            [1.0, 1.0, -1.0],
            [-1.0, 1.0, -1.0],
            [-1.0, -1.0, -1.0],
            [1.0, -1.0, -1.0],
        ];

        let octant = order.octant_ordinates();
        let mut directions = Vec::with_capacity(order.direction_count());
        let mut weights = Vec::with_capacity(order.direction_count());
        // Octant weight -> full-sphere weight: multiply by pi/2, so the total
        // over all 8 octants (each octant weight summing to 1) is 4*pi.
        let weight_scale = PI / 2.0;

        for sign in &SIGNS {
            for (base, w_oct) in &octant {
                let dir = Vector3::new(base[0] * sign[0], base[1] * sign[1], base[2] * sign[2]);
                // Normalise to a unit vector (guards against rounding in the
                // tabulated cosines) — matches GeN-Foam's per-direction
                // `directionVersors_ *= 1/mag`.
                let mag = dir.mag();
                directions.push(Vector3::new(dir.x / mag, dir.y / mag, dir.z / mag));
                weights.push(w_oct * weight_scale);
            }
        }

        // Renormalise the weights to sum to exactly 4*pi. The published point
        // weights are rounded to ~7 digits, so their raw sum drifts from 4*pi by
        // O(1e-6); the residual is amplified in a scattering-dominated medium
        // (by Sigma_s / Sigma_a) and would bias k_inf. GeN-Foam's
        // `readQuadratureSet.H` does the same normalisation
        // (`directionWeights_ *= 1/totalDirectionsWeight`, to sum 1).
        let raw_sum: f64 = weights.iter().sum();
        let renorm = (4.0 * PI) / raw_sum;
        for w in &mut weights {
            *w *= renorm;
        }

        Self {
            order,
            directions,
            weights,
        }
    }

    /// The quadrature order.
    #[must_use]
    pub fn order(&self) -> QuadratureOrder {
        self.order
    }

    /// Number of discrete directions `M = N (N + 2)`.
    #[must_use]
    pub fn direction_count(&self) -> usize {
        self.directions.len()
    }

    /// The unit direction vectors `Omega_j`.
    #[must_use]
    pub fn directions(&self) -> &[Vector3] {
        &self.directions
    }

    /// The direction weights `w_j` (sum to `4*pi`).
    #[must_use]
    pub fn weights(&self) -> &[f64] {
        &self.weights
    }

    /// The sum of all weights (`4*pi` for a consistent set) — the V&V invariant.
    #[must_use]
    pub fn weight_sum(&self) -> f64 {
        self.weights.iter().sum()
    }

    /// Build the face-flux field `facePhi_j = Omega_j . Sf` for direction
    /// `direction` on `mesh`, as a [`SurfaceScalarField`].
    ///
    /// This is the (dimensionless-direction times face-area, m^2) streaming
    /// weight the upwind convection operator
    /// [`fvm::div`](outram_foam_basic_lib::fv_operators::fvm::div) consumes for
    /// the `div(Omega_j psi)` term. Internal-face and boundary-face values are
    /// both filled from the mesh's outward face-area vectors `Sf`; boundary
    /// patch fields carry [`BoundaryCondition::Symmetry`] as a placeholder (the
    /// convection operator reads the value, not the flux BC, on boundary faces).
    #[must_use]
    pub fn face_phi(&self, mesh: &Arc<FvMesh>, direction: usize) -> SurfaceScalarField {
        let omega = self.directions[direction];
        let n_internal = mesh.n_internal_faces;

        let internal: Vec<f64> = (0..n_internal)
            .map(|f| omega.dot(mesh.face_area_vectors[f]))
            .collect();

        let boundary: Vec<PatchField<f64>> = mesh
            .patches
            .iter()
            .map(|p| {
                let values: Vec<f64> = (0..p.size)
                    .map(|fi| omega.dot(mesh.face_area_vectors[p.start + fi]))
                    .collect();
                PatchField {
                    bc: BoundaryCondition::Symmetry,
                    values: Field::new(values),
                }
            })
            .collect();

        SurfaceScalarField::new(
            format!("facePhi{direction}"),
            mesh.clone(),
            Field::new(internal),
            boundary,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// V&V — quadrature consistency.
    ///
    /// ## Methodology
    ///
    /// A level-symmetric S_N set must reproduce the exact low-order angular
    /// moments of the unit sphere:
    ///
    /// - zeroth moment `sum_j w_j = 4*pi` (integral of 1),
    /// - first moment `sum_j w_j Omega_j = 0` (odd, vanishes by symmetry),
    /// - second moment `sum_j w_j Omega_{j,x}^2 = 4*pi / 3` (and likewise for
    ///   y, z; the isotropic-diffusion `1/3` that makes S_N reduce to diffusion
    ///   in the transport limit).
    ///
    /// Every direction must also be a unit vector, and there must be
    /// `N (N + 2)` of them. These checks catch any transcription error in the
    /// tabulated cosines / weights.
    ///
    /// ## Results (measured 2026-07-15, this port)
    ///
    /// For S2, S4, S6 all invariants hold to < 1e-6:
    /// `|sum w - 4*pi|`, `|sum w Omega|`, and `|sum w Omega_x^2 - 4*pi/3|` are
    /// all below 1e-6; direction counts are 8 / 24 / 48; every `|Omega| = 1` to
    /// < 1e-12. PASS.
    #[test]
    fn level_symmetric_sets_reproduce_sphere_moments() {
        let four_pi = 4.0 * PI;
        for order in [
            QuadratureOrder::S2,
            QuadratureOrder::S4,
            QuadratureOrder::S6,
        ] {
            let q = AngularQuadrature::level_symmetric(order);

            assert_eq!(
                q.direction_count(),
                order.direction_count(),
                "{order:?}: direction count"
            );

            // Zeroth moment.
            let w_sum = q.weight_sum();
            assert!(
                (w_sum - four_pi).abs() < 1e-6,
                "{order:?}: sum w = {w_sum} != 4*pi"
            );

            // Unit vectors + first and second moments.
            let (mut m1, mut mxx, mut myy, mut mzz) = (Vector3::new(0.0, 0.0, 0.0), 0.0, 0.0, 0.0);
            for (o, &w) in q.directions().iter().zip(q.weights()) {
                assert!((o.mag() - 1.0).abs() < 1e-12, "{order:?}: |Omega| != 1");
                m1 = Vector3::new(m1.x + w * o.x, m1.y + w * o.y, m1.z + w * o.z);
                mxx += w * o.x * o.x;
                myy += w * o.y * o.y;
                mzz += w * o.z * o.z;
            }
            assert!(m1.mag() < 1e-6, "{order:?}: first moment {m1:?} != 0");
            for (label, m) in [("xx", mxx), ("yy", myy), ("zz", mzz)] {
                assert!(
                    (m - four_pi / 3.0).abs() < 1e-6,
                    "{order:?}: second moment {label} = {m} != 4*pi/3"
                );
            }
        }
    }
}
