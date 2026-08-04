// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Built on outram-foam-basic-lib (OUTRAM PARK's OpenFOAM-derived
// finite-volume layer); no Moltres-derived content in this file — a closed
// 1-D loop mesh is this crate's own finite-volume device for the
// circulating-fuel primary loop (Moltres models the loop as separate MOOSE
// apps / boundary transfers instead).
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

//! A closed 1-D "ring" finite-volume mesh for the MSR primary loop.
//!
//! The circulating-fuel effect needs a **periodic** 1-D domain: fuel salt
//! leaves the top of the core, travels around the external loop (pump + heat
//! exchanger), and re-enters the core bottom. `outram-foam-basic-lib`'s
//! matrix assembly has no cyclic boundary-patch coupling, so instead the
//! loop is built as a mesh whose face topology **is** a ring: `n` cells,
//! `n` internal faces, face `i` joining cell `i` to cell `i+1` and the last
//! face joining cell `n-1` back to cell `0`. There are **no boundary
//! patches at all** — every finite-volume operator sees a purely internal,
//! periodic domain, which is exactly the physics of a closed loop.
//!
//! To keep the cell-centre distances that `fvm::laplacian` uses consistent
//! at the wrap-around face, the cells are laid out on a **circle** in the
//! x-y plane (radius `R = L / 2 pi`). Every adjacent-cell distance is then
//! the same chord length `2 R sin(pi/n)`, which underestimates the arc
//! spacing `dx = L/n` by a uniform relative `O((pi/n)^2 / 6)` (`< 1e-4` for
//! `n >= 130`) — a documented, mesh-refinement-vanishing geometric bias.
//! Cell volumes use the exact arc measure `dx * A`.

use std::f64::consts::PI;
use std::sync::Arc;

use outram_foam_basic_lib::prelude::{Field, FvMesh, FvMeshBuilder, SurfaceScalarField, Vector3};

use crate::error::MoltresError;
use crate::materials::FaceFluxField;

/// A closed 1-D loop mesh plus its loop-level metadata.
///
/// Construct with [`RingMesh::new`]; the underlying [`FvMesh`] (shared
/// `Arc`) is in `mesh`. Cell `i` spans arc length
/// `[i dx, (i+1) dx)` measured from an arbitrary loop origin, so zone maps
/// (core vs external loop) are most naturally written against
/// [`RingMesh::arc_centre`].
#[derive(Debug, Clone)]
pub struct RingMesh {
    /// The underlying finite-volume mesh (no boundary patches; `n` cells,
    /// `n` internal faces, face `n-1` wraps from cell `n-1` to cell `0`).
    pub mesh: Arc<FvMesh>,
    /// Loop circumference `L` in `m` (total salt path length).
    pub circumference: f64,
    /// Flow cross-sectional area `A` in `m^2` (uniform).
    pub flow_area: f64,
    /// Number of cells `n` (>= 3).
    pub n_cells: usize,
    /// Arc-length cell spacing `dx = L/n` in `m`.
    pub dx: f64,
}

impl RingMesh {
    /// Build a closed loop of `n_cells` cells with total path length
    /// `circumference` (m) and uniform flow area `flow_area` (m^2).
    ///
    /// Positive flow direction is by convention the direction of increasing
    /// cell index (owner → neighbour on every face, including the wrap
    /// face).
    ///
    /// # Errors
    /// [`MoltresError::InvalidMaterial`] message for non-physical inputs
    /// (`n_cells < 3`, non-positive length/area), or a wrapped
    /// [`MoltresError::InvalidMesh`] if the assembled mesh fails
    /// `FvMesh::validate` (should not happen for valid inputs).
    pub fn new(circumference: f64, flow_area: f64, n_cells: usize) -> Result<Self, MoltresError> {
        if n_cells < 3 {
            return Err(MoltresError::InvalidMaterial(format!(
                "ring mesh needs at least 3 cells, got {n_cells}"
            )));
        }
        if !(circumference > 0.0) || !(flow_area > 0.0) {
            return Err(MoltresError::InvalidMaterial(format!(
                "ring mesh needs positive circumference and flow area, got \
                 L = {circumference}, A = {flow_area}"
            )));
        }
        let n = n_cells;
        let dx = circumference / n as f64;
        let radius = circumference / (2.0 * PI);
        let dtheta = 2.0 * PI / n as f64;

        // Topology: face i joins owner i -> neighbour (i+1) mod n.
        let owner: Vec<usize> = (0..n).collect();
        let neighbour: Vec<usize> = (0..n).map(|i| (i + 1) % n).collect();

        // Geometry on the circle.
        let cell_centres: Vec<Vector3> = (0..n)
            .map(|i| {
                let th = (i as f64 + 0.5) * dtheta;
                Vector3::new(radius * th.cos(), radius * th.sin(), 0.0)
            })
            .collect();
        let face_centres: Vec<Vector3> = (0..n)
            .map(|i| {
                let th = (i as f64 + 1.0) * dtheta;
                Vector3::new(radius * th.cos(), radius * th.sin(), 0.0)
            })
            .collect();
        // Face normal = counter-clockwise tangent (owner -> neighbour
        // direction), magnitude = flow area.
        let face_area_vectors: Vec<Vector3> = (0..n)
            .map(|i| {
                let th = (i as f64 + 1.0) * dtheta;
                Vector3::new(-flow_area * th.sin(), flow_area * th.cos(), 0.0)
            })
            .collect();
        let cell_volumes = vec![dx * flow_area; n];

        let mesh = FvMeshBuilder::new()
            .n_cells(n)
            .n_internal_faces(n)
            .owner(owner)
            .neighbour(neighbour)
            .patches(Vec::new())
            .cell_volumes(cell_volumes)
            .cell_centres(cell_centres)
            .face_area_vectors(face_area_vectors)
            .face_centres(face_centres)
            .build()?;
        mesh.validate()?;

        Ok(Self {
            mesh: Arc::new(mesh),
            circumference,
            flow_area,
            n_cells: n,
            dx,
        })
    }

    /// Arc-length coordinate of cell `i`'s centre, `s_i = (i + 1/2) dx` in
    /// `m`, measured from the loop origin in the positive-flow direction.
    #[must_use]
    pub fn arc_centre(&self, cell: usize) -> f64 {
        (cell as f64 + 0.5) * self.dx
    }

    /// Face volumetric flux for a rigid-loop circulation at salt speed
    /// `speed` (m/s, positive = increasing cell index): `phi_f = u A` on
    /// every face (`m^3/s`). This is the divergence-free prescribed loop
    /// velocity of the first-pass model — no CFD, no pump model, just an
    /// incompressible slug flow around the ring.
    #[must_use]
    pub fn uniform_flux(&self, speed: f64) -> FaceFluxField {
        SurfaceScalarField::new(
            "phi",
            self.mesh.clone(),
            Field::uniform(self.mesh.n_internal_faces, speed * self.flow_area),
            Vec::new(),
        )
    }

    /// Two-zone map: zone `0` ("core") for cells whose arc centre lies in
    /// `[0, core_length)`, zone `1` ("external loop") otherwise.
    /// `core_length` in `m`; values outside `(0, circumference)` simply give
    /// an all-core or all-external map.
    #[must_use]
    pub fn two_zone_map(&self, core_length: f64) -> Vec<usize> {
        (0..self.n_cells)
            .map(|i| usize::from(self.arc_centre(i) >= core_length))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_topology_and_volume() {
        let ring = RingMesh::new(15.0, 0.1, 300).unwrap();
        let m = &ring.mesh;
        assert_eq!(m.n_cells, 300);
        assert_eq!(m.n_internal_faces, 300);
        assert_eq!(m.n_faces, 300);
        assert!(m.patches.is_empty());
        // Wrap face joins the last cell back to cell 0.
        assert_eq!(m.owner[299], 299);
        assert_eq!(m.neighbour[299], 0);
        // Total volume is exactly L * A (arc measure).
        let vol: f64 = m.cell_volumes.iter().sum();
        assert!((vol - 1.5).abs() < 1e-12, "vol = {vol}");
    }

    #[test]
    fn adjacent_cell_distances_are_uniform_including_wrap() {
        let ring = RingMesh::new(15.0, 0.1, 200).unwrap();
        let m = &ring.mesh;
        let d0 = (m.cell_centres[1] - m.cell_centres[0]).mag();
        for f in 0..m.n_internal_faces {
            let d = (m.cell_centres[m.neighbour[f]] - m.cell_centres[m.owner[f]]).mag();
            assert!((d - d0).abs() < 1e-12, "face {f}: {d} vs {d0}");
        }
        // Chord underestimates arc dx by O((pi/n)^2/6).
        let dx = ring.dx;
        let rel = (dx - d0) / dx;
        assert!(rel > 0.0 && rel < 5e-5, "chord/arc rel diff = {rel}");
    }

    #[test]
    fn uniform_flux_and_zone_map() {
        let ring = RingMesh::new(10.0, 0.2, 100).unwrap();
        let phi = ring.uniform_flux(0.5);
        assert_eq!(phi.internal.len(), 100);
        assert!((phi.internal[42] - 0.1).abs() < 1e-14);
        let zones = ring.two_zone_map(3.0);
        assert_eq!(zones[0], 0);
        assert_eq!(zones[29], 0); // s = 2.95 m < 3
        assert_eq!(zones[30], 1); // s = 3.05 m >= 3
        assert_eq!(zones[99], 1);
    }

    #[test]
    fn rejects_degenerate_inputs() {
        assert!(RingMesh::new(15.0, 0.1, 2).is_err());
        assert!(RingMesh::new(-1.0, 0.1, 10).is_err());
        assert!(RingMesh::new(15.0, 0.0, 10).is_err());
    }
}
