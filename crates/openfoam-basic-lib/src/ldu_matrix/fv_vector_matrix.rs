use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};
use std::sync::Arc;

use crate::fields::boundary::bc::PatchField;
use crate::fields::field::Field;
use crate::fields::vol_field::{VolScalarField, VolVectorField};
use crate::mesh::fv_mesh::FvMesh;
use crate::primitives::Vector3;
use super::fv_matrix::{SolverPerformance, SolverSettings};
use super::ldu_matrix::LduMatrix;
use super::solvers::gauss_seidel::gauss_seidel as gs;

/// Implicit vector equation `A·U = b` for a `VolVectorField`.
///
/// Mirrors `Foam::fvVectorMatrix` (`fvMatrix<vector>`).
///
/// The LDU coefficients are **scalar** — they multiply the entire velocity
/// vector equally in all three directions.  The source vector is a
/// `Field<Vector3>`.  Solving decomposes into three independent scalar
/// Gauss-Seidel solves (one per component).
#[derive(Debug, Clone)]
pub struct FvVectorMatrix {
    pub mesh:   Arc<FvMesh>,
    pub ldu:    LduMatrix,
    pub source: Field<Vector3>,
}

impl FvVectorMatrix {
    pub fn new(mesh: Arc<FvMesh>) -> Self {
        let n_cells = mesh.n_cells;
        let owner  = mesh.owner[..mesh.n_internal_faces].to_vec();
        let neighbour = mesh.neighbour.clone();
        Self {
            ldu: LduMatrix::new(n_cells, owner, neighbour),
            source: Field::from_fn(n_cells, |_| Vector3::ZERO),
            mesh,
        }
    }

    pub fn add_to_diag(&mut self, coeff: &Field<f64>) {
        for c in 0..self.mesh.n_cells { self.ldu.diag[c] += coeff[c]; }
    }

    pub fn add_to_source(&mut self, term: &Field<Vector3>) {
        for c in 0..self.mesh.n_cells { self.source[c] = self.source[c] + term[c]; }
    }

    /// Pin one cell's velocity to a fixed value (reference cell for closed domains).
    pub fn set_reference(&mut self, cell: usize, value: Vector3) {
        self.ldu.diag[cell] += 1e30;
        self.source[cell] = self.source[cell] + value * 1e30;
    }

    /// Diagonal coefficient per cell: `A[c] = diag[c]`.
    ///
    /// Used in PISO: `rAU = 1 / UEqn.a_field()`.
    pub fn a_field(&self) -> VolScalarField {
        let mesh = self.mesh.clone();
        let boundary = mesh.patches.iter()
            .map(|p| PatchField::zero_gradient(p.size))
            .collect();
        VolScalarField::new("A", mesh.clone(), Field::new(self.ldu.diag.clone()), boundary)
    }

    /// Off-diagonal + source residual: `H[c] = source[c] − Σ off-diag · U`.
    ///
    /// For a zero field x this returns `source[c]` directly.
    /// Used in PISO: `HbyA = rAU * UEqn.h_field(U)`.
    pub fn h_field(&self, u: &VolVectorField) -> VolVectorField {
        let mesh = self.mesh.clone();
        let n = mesh.n_cells;
        let mut h = vec![Vector3::ZERO; n];
        for c in 0..n { h[c] = self.source[c]; }
        for f in 0..mesh.n_internal_faces {
            let o = self.ldu.owner[f];
            let nb = self.ldu.neighbour[f];
            h[o] = h[o] - u.internal[nb] * self.ldu.upper[f];
            h[nb] = h[nb] - u.internal[o] * self.ldu.lower[f];
        }
        let boundary = mesh.patches.iter()
            .map(|p| PatchField::zero_gradient_vec(p.size))
            .collect();
        VolVectorField::new("H", mesh, Field::new(h), boundary)
    }

    /// Solve each component (x, y, z) as an independent scalar Gauss-Seidel problem.
    pub fn solve(&self, name: &str, settings: SolverSettings) -> (VolVectorField, SolverPerformance) {
        let mesh = self.mesh.clone();
        let n = mesh.n_cells;

        let bx: Vec<f64> = (0..n).map(|c| self.source[c].x).collect();
        let by: Vec<f64> = (0..n).map(|c| self.source[c].y).collect();
        let bz: Vec<f64> = (0..n).map(|c| self.source[c].z).collect();

        let mut xx = vec![0.0_f64; n];
        let mut xy = vec![0.0_f64; n];
        let mut xz = vec![0.0_f64; n];

        let (ix, rx) = gs(&self.ldu, &bx, &mut xx, settings.tolerance, settings.max_iter);
        let (iy, ry) = gs(&self.ldu, &by, &mut xy, settings.tolerance, settings.max_iter);
        let (iz, rz) = gs(&self.ldu, &bz, &mut xz, settings.tolerance, settings.max_iter);

        let internal = Field::from_fn(n, |c| Vector3::new(xx[c], xy[c], xz[c]));
        let boundary = mesh.patches.iter()
            .map(|p| PatchField::zero_gradient_vec(p.size))
            .collect();
        let u = VolVectorField::new(name, mesh, internal, boundary);

        let n_iters  = ix.max(iy).max(iz);
        let final_res = rx.max(ry).max(rz);
        let perf = SolverPerformance {
            n_iterations: n_iters,
            final_residual: final_res,
            converged: final_res < settings.tolerance,
        };
        (u, perf)
    }
}

// ── Arithmetic ────────────────────────────────────────────────────────────────

impl Add for FvVectorMatrix {
    type Output = Self;
    fn add(mut self, rhs: Self) -> Self {
        for (a, b) in self.ldu.diag.iter_mut().zip(&rhs.ldu.diag) { *a += b; }
        for (a, b) in self.ldu.lower.iter_mut().zip(&rhs.ldu.lower) { *a += b; }
        for (a, b) in self.ldu.upper.iter_mut().zip(&rhs.ldu.upper) { *a += b; }
        for c in 0..self.source.len() { self.source[c] = self.source[c] + rhs.source[c]; }
        self
    }
}

impl Sub for FvVectorMatrix {
    type Output = Self;
    fn sub(mut self, rhs: Self) -> Self {
        for (a, b) in self.ldu.diag.iter_mut().zip(&rhs.ldu.diag) { *a -= b; }
        for (a, b) in self.ldu.lower.iter_mut().zip(&rhs.ldu.lower) { *a -= b; }
        for (a, b) in self.ldu.upper.iter_mut().zip(&rhs.ldu.upper) { *a -= b; }
        for c in 0..self.source.len() { self.source[c] = self.source[c] - rhs.source[c]; }
        self
    }
}

impl Neg for FvVectorMatrix {
    type Output = Self;
    fn neg(mut self) -> Self {
        for x in self.ldu.diag.iter_mut() { *x = -*x; }
        for x in self.ldu.lower.iter_mut() { *x = -*x; }
        for x in self.ldu.upper.iter_mut() { *x = -*x; }
        for c in 0..self.source.len() { self.source[c] = -self.source[c]; }
        self
    }
}

impl AddAssign for FvVectorMatrix {
    fn add_assign(&mut self, rhs: Self) {
        for (a, b) in self.ldu.diag.iter_mut().zip(&rhs.ldu.diag) { *a += b; }
        for (a, b) in self.ldu.lower.iter_mut().zip(&rhs.ldu.lower) { *a += b; }
        for (a, b) in self.ldu.upper.iter_mut().zip(&rhs.ldu.upper) { *a += b; }
        for c in 0..self.source.len() { self.source[c] = self.source[c] + rhs.source[c]; }
    }
}

impl SubAssign for FvVectorMatrix {
    fn sub_assign(&mut self, rhs: Self) {
        for (a, b) in self.ldu.diag.iter_mut().zip(&rhs.ldu.diag) { *a -= b; }
        for (a, b) in self.ldu.lower.iter_mut().zip(&rhs.ldu.lower) { *a -= b; }
        for (a, b) in self.ldu.upper.iter_mut().zip(&rhs.ldu.upper) { *a -= b; }
        for c in 0..self.source.len() { self.source[c] = self.source[c] - rhs.source[c]; }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::fv_mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};

    fn unit_mesh() -> Arc<FvMesh> {
        Arc::new(FvMeshBuilder::new()
            .n_cells(2).n_internal_faces(1)
            .owner(vec![0, 1, 0]).neighbour(vec![1])
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
            .build().unwrap())
    }

    #[test]
    fn diagonal_system_solves_vector() {
        let m = unit_mesh();
        let mut mat = FvVectorMatrix::new(m.clone());
        mat.ldu.diag[0] = 2.0;
        mat.ldu.diag[1] = 3.0;
        mat.source[0] = Vector3::new(4.0, 6.0, 8.0);
        mat.source[1] = Vector3::new(6.0, 9.0, 12.0);
        let (u, perf) = mat.solve("U", SolverSettings::default());
        assert!(perf.converged, "residual = {}", perf.final_residual);
        assert!((u.internal[0].x - 2.0).abs() < 1e-8);
        assert!((u.internal[0].y - 3.0).abs() < 1e-8);
        assert!((u.internal[1].x - 2.0).abs() < 1e-8);
    }

    #[test]
    fn a_field_returns_diagonal() {
        let m = unit_mesh();
        let mut mat = FvVectorMatrix::new(m.clone());
        mat.ldu.diag[0] = 5.0;
        mat.ldu.diag[1] = 7.0;
        let a = mat.a_field();
        assert!((a.internal[0] - 5.0).abs() < 1e-12);
        assert!((a.internal[1] - 7.0).abs() < 1e-12);
    }
}
