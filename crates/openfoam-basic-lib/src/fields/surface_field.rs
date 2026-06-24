use std::sync::Arc;
use std::ops::{Add, Sub, Mul, Div, Neg};

use crate::primitives::Vector3;
use crate::mesh::fv_mesh::FvMesh;
use super::field::Field;
use super::boundary::bc::PatchField;

/// A surface field: one value per *internal* face in the internal field, plus
/// one `PatchField` per boundary patch.
///
/// Mirrors `Foam::surfaceScalarField` / `Foam::SurfaceField<Type>`.
///
/// ## Why `internal` has length `n_internal_faces`, not `n_faces`
///
/// In OpenFOAM, `surfaceScalarField.internalField()` only covers the internal
/// faces; boundary-face values live in `boundaryField()[patch]`.  This matches
/// the LDU matrix structure: `lower` and `upper` arrays have length
/// `n_internal_faces`.
#[derive(Debug, Clone)]
pub struct SurfaceField<T: Clone> {
    pub name: String,
    pub mesh: Arc<FvMesh>,
    /// Face values for all internal faces; length == `mesh.n_internal_faces`.
    pub internal: Field<T>,
    /// One entry per boundary patch; `boundary[i].values` has length
    /// `mesh.patches[i].size`.
    pub boundary: Vec<PatchField<T>>,
}

// ── Type aliases ──────────────────────────────────────────────────────────────

pub type SurfaceScalarField = SurfaceField<f64>;
pub type SurfaceVectorField = SurfaceField<Vector3>;

// ── Construction ─────────────────────────────────────────────────────────────

impl<T: Clone> SurfaceField<T> {
    pub fn new(
        name: impl Into<String>,
        mesh: Arc<FvMesh>,
        internal: Field<T>,
        boundary: Vec<PatchField<T>>,
    ) -> Self {
        debug_assert_eq!(internal.len(), mesh.n_internal_faces,
            "SurfaceField internal field length must equal n_internal_faces");
        debug_assert_eq!(boundary.len(), mesh.patches.len(),
            "SurfaceField boundary length must equal number of patches");
        Self { name: name.into(), mesh, internal, boundary }
    }
}

impl SurfaceScalarField {
    pub fn zeros(name: impl Into<String>, mesh: Arc<FvMesh>) -> Self {
        let n_int = mesh.n_internal_faces;
        let boundary = mesh.patches.iter()
            .map(|p| PatchField::zero_gradient(p.size))
            .collect();
        Self::new(name, mesh, Field::zeros(n_int), boundary)
    }

    pub fn uniform(name: impl Into<String>, mesh: Arc<FvMesh>, value: f64) -> Self {
        let n_int = mesh.n_internal_faces;
        let boundary = mesh.patches.iter()
            .map(|p| PatchField::zero_gradient(p.size))
            .collect();
        Self::new(name, mesh, Field::uniform(n_int, value), boundary)
    }
}

impl SurfaceVectorField {
    pub fn zero(name: impl Into<String>, mesh: Arc<FvMesh>) -> Self {
        let n_int = mesh.n_internal_faces;
        let boundary = mesh.patches.iter()
            .map(|p| PatchField::zero_gradient_vec(p.size))
            .collect();
        Self::new(name, mesh, Field::zero_vec(n_int), boundary)
    }
}

// ── Value access across all faces ─────────────────────────────────────────────

impl<T: Clone + Default> SurfaceField<T> {
    /// Value at any face: internal face → from `internal`; boundary face →
    /// from the appropriate patch's `values`.
    pub fn face_value(&self, f: usize) -> T {
        if self.mesh.is_internal_face(f) {
            self.internal[f].clone()
        } else {
            let (pi, fi) = self.mesh.patch_for_face(f)
                .expect("face index out of range for boundary");
            self.boundary[pi].values[fi].clone()
        }
    }
}

// ── Arithmetic ────────────────────────────────────────────────────────────────

impl<T> Add for SurfaceField<T>
where
    T: Add<Output=T> + Clone + Default,
{
    type Output = Self;
    fn add(mut self, rhs: Self) -> Self {
        self.name = format!("({} + {})", self.name, rhs.name);
        self.internal = self.internal + rhs.internal;
        for (l, r) in self.boundary.iter_mut().zip(rhs.boundary) {
            l.values = l.values.clone() + r.values;
        }
        self
    }
}

impl<T> Sub for SurfaceField<T>
where
    T: Sub<Output=T> + Clone + Default,
{
    type Output = Self;
    fn sub(mut self, rhs: Self) -> Self {
        self.name = format!("({} - {})", self.name, rhs.name);
        self.internal = self.internal - rhs.internal;
        for (l, r) in self.boundary.iter_mut().zip(rhs.boundary) {
            l.values = l.values.clone() - r.values;
        }
        self
    }
}

impl<T> Neg for SurfaceField<T>
where
    T: Neg<Output=T> + Clone,
{
    type Output = Self;
    fn neg(mut self) -> Self {
        self.name = format!("(-{})", self.name);
        self.internal = -self.internal;
        for p in self.boundary.iter_mut() {
            p.values = -p.values.clone();
        }
        self
    }
}

impl<T> Mul<f64> for SurfaceField<T>
where
    T: Mul<f64, Output=T> + Clone,
{
    type Output = Self;
    fn mul(mut self, s: f64) -> Self {
        self.internal = self.internal * s;
        for p in self.boundary.iter_mut() {
            p.values = p.values.clone() * s;
        }
        self
    }
}

impl<T> Div<f64> for SurfaceField<T>
where
    T: Mul<f64, Output=T> + Clone,
{
    type Output = Self;
    fn div(self, s: f64) -> Self {
        self * (1.0 / s)
    }
}

// Pointwise scalar * scalar
impl Mul for SurfaceScalarField {
    type Output = Self;
    fn mul(mut self, rhs: Self) -> Self {
        self.internal = self.internal * rhs.internal;
        for (l, r) in self.boundary.iter_mut().zip(rhs.boundary) {
            l.values = l.values.clone() * r.values;
        }
        self
    }
}

// f64 * SurfaceField<T>
impl<T: Mul<f64, Output=T> + Clone> Mul<SurfaceField<T>> for f64 {
    type Output = SurfaceField<T>;
    fn mul(self, mut rhs: SurfaceField<T>) -> SurfaceField<T> {
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
    fn zeros_surface_scalar_field() {
        let m = unit_mesh();
        let phi = SurfaceScalarField::zeros("phi", m.clone());
        // Internal: 1 face; boundary: 2 patches × 1 face each
        assert_eq!(phi.internal.len(), 1);
        assert_eq!(phi.boundary.len(), 2);
        assert_eq!(phi.boundary[0].values.len(), 1);
    }

    #[test]
    fn face_value_internal() {
        let m = unit_mesh();
        let phi = SurfaceScalarField::uniform("phi", m.clone(), 5.0);
        assert!((phi.face_value(0) - 5.0).abs() < 1e-15); // internal face 0
    }

    #[test]
    fn add_surface_fields() {
        let m = unit_mesh();
        let a = SurfaceScalarField::uniform("a", m.clone(), 2.0);
        let b = SurfaceScalarField::uniform("b", m, 3.0);
        let c = a + b;
        assert!((c.internal[0] - 5.0).abs() < 1e-15);
    }

    #[test]
    fn mul_surface_field_by_scalar() {
        let m = unit_mesh();
        let a = SurfaceScalarField::uniform("a", m, 4.0);
        let b = a * 0.5;
        assert!((b.internal[0] - 2.0).abs() < 1e-15);
    }
}
