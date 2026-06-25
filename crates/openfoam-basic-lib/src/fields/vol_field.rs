// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
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

use std::sync::Arc;
use std::ops::{Add, Sub, Mul, Div, Neg};

use crate::primitives::{Vector3, Tensor, SymmTensor};
use crate::mesh::fv_mesh::FvMesh;
use super::field::Field;
use super::boundary::bc::PatchField;

/// A volume field: one value per cell in the internal field, plus one
/// `PatchField` per boundary patch.
///
/// Mirrors `Foam::volScalarField` / `Foam::VolumeField<Type>`.
/// The internal field has length `mesh.n_cells`.
#[derive(Debug, Clone)]
pub struct VolField<T: Clone> {
    pub name: String,
    pub mesh: Arc<FvMesh>,
    /// Cell-centred values; length == `mesh.n_cells`.
    pub internal: Field<T>,
    /// One entry per boundary patch; `boundary[i].values` has length
    /// `mesh.patches[i].size`.
    pub boundary: Vec<PatchField<T>>,
}

// ── Type aliases (matching OpenFOAM names) ────────────────────────────────────

pub type VolScalarField = VolField<f64>;
pub type VolVectorField = VolField<Vector3>;
pub type VolTensorField = VolField<Tensor>;
pub type VolSymmTensorField = VolField<SymmTensor>;

// ── Construction ─────────────────────────────────────────────────────────────

impl<T: Clone + Default> VolField<T> {
    pub fn new(
        name: impl Into<String>,
        mesh: Arc<FvMesh>,
        internal: Field<T>,
        boundary: Vec<PatchField<T>>,
    ) -> Self {
        debug_assert_eq!(internal.len(), mesh.n_cells,
            "VolField internal field length must equal n_cells");
        debug_assert_eq!(boundary.len(), mesh.patches.len(),
            "VolField boundary length must equal number of patches");
        Self { name: name.into(), mesh, internal, boundary }
    }
}

impl VolScalarField {
    /// Uniform scalar field over the entire domain.
    pub fn uniform(name: impl Into<String>, mesh: Arc<FvMesh>, value: f64) -> Self {
        let n_cells = mesh.n_cells;
        let boundary = mesh.patches.iter()
            .map(|p| PatchField::zero_gradient(p.size))
            .collect();
        Self::new(name, mesh, Field::uniform(n_cells, value), boundary)
    }

    pub fn zeros(name: impl Into<String>, mesh: Arc<FvMesh>) -> Self {
        Self::uniform(name, mesh, 0.0)
    }
}

impl VolVectorField {
    /// Uniform vector field over the entire domain.
    pub fn uniform(name: impl Into<String>, mesh: Arc<FvMesh>, value: Vector3) -> Self {
        let n_cells = mesh.n_cells;
        let boundary = mesh.patches.iter()
            .map(|p| PatchField::zero_gradient_vec(p.size))
            .collect();
        Self::new(name, mesh, Field::uniform(n_cells, value), boundary)
    }

    pub fn zero(name: impl Into<String>, mesh: Arc<FvMesh>) -> Self {
        Self::uniform(name, mesh, Vector3::ZERO)
    }
}

// ── Arithmetic — borrows rhs so we don't move the mesh Arc ───────────────────

impl<T> Add for VolField<T>
where
    T: Add<Output=T> + Clone + Default,
{
    type Output = VolField<T>;
    fn add(mut self, rhs: Self) -> Self::Output {
        self.name = format!("({} + {})", self.name, rhs.name);
        self.internal = self.internal + rhs.internal;
        for (l, r) in self.boundary.iter_mut().zip(rhs.boundary) {
            l.values = l.values.clone() + r.values;
        }
        self
    }
}

impl<T> Sub for VolField<T>
where
    T: Sub<Output=T> + Clone + Default,
{
    type Output = VolField<T>;
    fn sub(mut self, rhs: Self) -> Self::Output {
        self.name = format!("({} - {})", self.name, rhs.name);
        self.internal = self.internal - rhs.internal;
        for (l, r) in self.boundary.iter_mut().zip(rhs.boundary) {
            l.values = l.values.clone() - r.values;
        }
        self
    }
}

impl<T> Neg for VolField<T>
where
    T: Neg<Output=T> + Clone,
{
    type Output = VolField<T>;
    fn neg(mut self) -> Self::Output {
        self.name = format!("(-{})", self.name);
        self.internal = -self.internal;
        for p in self.boundary.iter_mut() {
            p.values = -p.values.clone();
        }
        self
    }
}

impl<T> Mul<f64> for VolField<T>
where
    T: Mul<f64, Output=T> + Clone,
{
    type Output = VolField<T>;
    fn mul(mut self, s: f64) -> Self::Output {
        self.internal = self.internal * s;
        for p in self.boundary.iter_mut() {
            p.values = p.values.clone() * s;
        }
        self
    }
}

impl<T> Div<f64> for VolField<T>
where
    T: Mul<f64, Output=T> + Clone,
{
    type Output = VolField<T>;
    fn div(self, s: f64) -> Self::Output {
        self * (1.0 / s)
    }
}

// Pointwise scalar * scalar
impl Mul for VolScalarField {
    type Output = Self;
    fn mul(mut self, rhs: Self) -> Self {
        self.internal = self.internal * rhs.internal;
        for (l, r) in self.boundary.iter_mut().zip(rhs.boundary) {
            l.values = l.values.clone() * r.values;
        }
        self
    }
}

// Scalar field * vector field
impl Mul<VolVectorField> for VolScalarField {
    type Output = VolVectorField;
    fn mul(self, mut rhs: VolVectorField) -> VolVectorField {
        rhs.internal = rhs.internal * self.internal;
        for (r, l) in rhs.boundary.iter_mut().zip(self.boundary) {
            r.values = r.values.clone() * l.values;
        }
        rhs
    }
}

// f64 * VolField<T>
impl<T: Mul<f64, Output=T> + Clone> Mul<VolField<T>> for f64 {
    type Output = VolField<T>;
    fn mul(self, mut rhs: VolField<T>) -> VolField<T> {
        rhs.internal = rhs.internal * self;
        for p in rhs.boundary.iter_mut() {
            p.values = p.values.clone() * self;
        }
        rhs
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::fv_mesh::{FvMeshBuilder, BoundaryPatch, PatchKind};

    fn unit_mesh() -> Arc<FvMesh> {
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(2)
                .n_internal_faces(1)
                .owner(vec![0, 1, 0])
                .neighbour(vec![1])
                .patches(vec![
                    BoundaryPatch::new("right", 1, 1, PatchKind::Wall),
                    BoundaryPatch::new("left",  2, 1, PatchKind::Wall),
                ])
                .cell_volumes(vec![1.0, 1.0])
                .cell_centres(vec![Vector3::new(0.25, 0.0, 0.0), Vector3::new(0.75, 0.0, 0.0)])
                .face_area_vectors(vec![
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(-1.0, 0.0, 0.0),
                ])
                .face_centres(vec![
                    Vector3::new(0.5, 0.0, 0.0),
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(0.0, 0.0, 0.0),
                ])
                .build()
                .unwrap()
        )
    }

    #[test]
    fn uniform_vol_scalar_field() {
        let m = unit_mesh();
        let p = VolScalarField::uniform("p", m, 101325.0);
        assert_eq!(p.internal.len(), 2);
        assert!((p.internal[0] - 101325.0).abs() < 1e-8);
        assert_eq!(p.boundary.len(), 2);
    }

    #[test]
    fn add_vol_scalar_fields() {
        let m = unit_mesh();
        let a = VolScalarField::uniform("a", m.clone(), 1.0);
        let b = VolScalarField::uniform("b", m, 2.0);
        let c = a + b;
        assert!((c.internal[0] - 3.0).abs() < 1e-15);
        assert!((c.internal[1] - 3.0).abs() < 1e-15);
    }

    #[test]
    fn mul_scalar_field_by_f64() {
        let m = unit_mesh();
        let a = VolScalarField::uniform("a", m, 3.0);
        let b = a * 2.0;
        assert!((b.internal[0] - 6.0).abs() < 1e-15);
    }

    #[test]
    fn neg_vol_vector_field() {
        let m = unit_mesh();
        let u = VolVectorField::uniform("U", m, Vector3::new(1.0, 2.0, 3.0));
        let neg_u = -u;
        assert_eq!(neg_u.internal[0], Vector3::new(-1.0, -2.0, -3.0));
    }
}
