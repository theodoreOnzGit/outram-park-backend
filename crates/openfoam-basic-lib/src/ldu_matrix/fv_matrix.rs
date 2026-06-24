use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};
use std::sync::Arc;

use crate::mesh::fv_mesh::FvMesh;
use crate::fields::field::Field;
use crate::fields::vol_field::VolScalarField;
use crate::fields::boundary::bc::PatchField;
use super::ldu_matrix::LduMatrix;
use super::solvers::gauss_seidel::gauss_seidel;

/// Sparse implicit matrix equation `A·φ = b` for a scalar field φ.
///
/// Mirrors `Foam::fvMatrix<scalar>` from
/// `src/finiteVolume/fvMatrices/fvMatrix/fvMatrix.H`.
///
/// Assembled incrementally by `fvm::` operators in Layer 3; solved via
/// `self.solve()`.
pub struct FvMatrix {
    pub mesh: Arc<FvMesh>,
    pub ldu: LduMatrix,
    /// Right-hand-side source term, length `n_cells`.
    pub source: Field<f64>,
}

/// Solver settings passed to `FvMatrix::solve`.
#[derive(Debug, Clone, Copy)]
pub struct SolverSettings {
    pub tolerance: f64,
    pub max_iter: usize,
}

impl Default for SolverSettings {
    fn default() -> Self {
        Self { tolerance: 1e-7, max_iter: 1000 }
    }
}

/// Summary of a linear solve.
#[derive(Debug, Clone, Copy)]
pub struct SolverPerformance {
    pub n_iterations: usize,
    pub final_residual: f64,
    pub converged: bool,
}

impl FvMatrix {
    /// Create a new zero-initialised FvMatrix for the given mesh.
    pub fn new(mesh: Arc<FvMesh>) -> Self {
        let owner    = mesh.owner[..mesh.n_internal_faces].to_vec();
        let neighbour = mesh.neighbour.to_vec();
        let n_cells  = mesh.n_cells;
        Self {
            mesh,
            ldu: LduMatrix::new(n_cells, owner, neighbour),
            source: Field::zeros(n_cells),
        }
    }

    /// Solve `A·φ = source` and return the solution as a `VolScalarField`.
    ///
    /// Uses Gauss-Seidel as the default solver.  Layer 3 will add conjugate
    /// gradient / GAMG when those are ported.
    pub fn solve(
        &self,
        name: impl Into<String>,
        settings: SolverSettings,
    ) -> (VolScalarField, SolverPerformance) {
        let mut x = vec![0.0_f64; self.mesh.n_cells];
        let b: Vec<f64> = self.source.iter().copied().collect();
        let (iters, res) = gauss_seidel(&self.ldu, &b, &mut x, settings.tolerance, settings.max_iter);

        let boundary = self.mesh.patches.iter()
            .map(|p| PatchField::zero_gradient(p.size))
            .collect();
        let field = VolScalarField::new(
            name,
            self.mesh.clone(),
            Field::new(x),
            boundary,
        );
        let perf = SolverPerformance {
            n_iterations: iters,
            final_residual: res,
            converged: res < settings.tolerance,
        };
        (field, perf)
    }

    // ── Operator helpers (used by fvm:: in Layer 3) ────────────────────────

    /// Add `coeff * I` to the diagonal (e.g. from a time derivative term).
    pub fn add_to_diag(&mut self, coeff: &Field<f64>) {
        for (d, &c) in self.ldu.diag.iter_mut().zip(coeff.iter()) {
            *d += c;
        }
    }

    /// Add `coeff[c]` to the source at cell `c`.
    pub fn add_to_source(&mut self, term: &Field<f64>) {
        self.source += term.clone();
    }

    /// Add upper/lower contributions from a face (used by fvm::laplacian etc.).
    pub fn add_face_coeff(&mut self, face: usize, coeff: f64) {
        let o = self.ldu.owner[face];
        let n = self.ldu.neighbour[face];
        // Laplacian: upper[f] = lower[f] = -coeff (off-diagonal negative)
        self.ldu.upper[face] -= coeff;
        self.ldu.lower[face]  -= coeff;
        self.ldu.diag[o]     += coeff;
        self.ldu.diag[n]     += coeff;
    }
}

// ── Arithmetic ────────────────────────────────────────────────────────────────

impl Add for FvMatrix {
    type Output = Self;
    fn add(mut self, rhs: Self) -> Self {
        for (a, b) in self.ldu.diag.iter_mut().zip(&rhs.ldu.diag) { *a += b; }
        for (a, b) in self.ldu.lower.iter_mut().zip(&rhs.ldu.lower) { *a += b; }
        for (a, b) in self.ldu.upper.iter_mut().zip(&rhs.ldu.upper) { *a += b; }
        self.source += rhs.source;
        self
    }
}

impl Sub for FvMatrix {
    type Output = Self;
    fn sub(mut self, rhs: Self) -> Self {
        for (a, b) in self.ldu.diag.iter_mut().zip(&rhs.ldu.diag) { *a -= b; }
        for (a, b) in self.ldu.lower.iter_mut().zip(&rhs.ldu.lower) { *a -= b; }
        for (a, b) in self.ldu.upper.iter_mut().zip(&rhs.ldu.upper) { *a -= b; }
        self.source -= rhs.source;
        self
    }
}

impl Neg for FvMatrix {
    type Output = Self;
    fn neg(mut self) -> Self {
        for x in self.ldu.diag.iter_mut() { *x = -*x; }
        for x in self.ldu.lower.iter_mut() { *x = -*x; }
        for x in self.ldu.upper.iter_mut() { *x = -*x; }
        self.source = -self.source;
        self
    }
}

impl AddAssign for FvMatrix {
    fn add_assign(&mut self, rhs: Self) {
        for (a, b) in self.ldu.diag.iter_mut().zip(&rhs.ldu.diag) { *a += b; }
        for (a, b) in self.ldu.lower.iter_mut().zip(&rhs.ldu.lower) { *a += b; }
        for (a, b) in self.ldu.upper.iter_mut().zip(&rhs.ldu.upper) { *a += b; }
        self.source += rhs.source;
    }
}

impl SubAssign for FvMatrix {
    fn sub_assign(&mut self, rhs: Self) {
        for (a, b) in self.ldu.diag.iter_mut().zip(&rhs.ldu.diag) { *a -= b; }
        for (a, b) in self.ldu.lower.iter_mut().zip(&rhs.ldu.lower) { *a -= b; }
        for (a, b) in self.ldu.upper.iter_mut().zip(&rhs.ldu.upper) { *a -= b; }
        self.source -= rhs.source;
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::Vector3;
    use crate::mesh::fv_mesh::{FvMeshBuilder, BoundaryPatch, PatchKind};
    use approx::assert_relative_eq;

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
                .cell_centres(vec![
                    Vector3::new(0.25, 0.0, 0.0),
                    Vector3::new(0.75, 0.0, 0.0),
                ])
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
    fn solve_diagonal_system() {
        let mesh = unit_mesh();
        let mut mat = FvMatrix::new(mesh);

        // Set A = [[3,0],[0,5]], b = [6, 10]  → x = [2, 2]
        mat.ldu.diag[0] = 3.0;
        mat.ldu.diag[1] = 5.0;
        mat.source = Field::new(vec![6.0, 10.0]);

        let (phi, perf) = mat.solve("phi", SolverSettings::default());
        assert!(perf.converged, "did not converge: residual = {}", perf.final_residual);
        assert_relative_eq!(phi.internal[0], 2.0, epsilon = 1e-6);
        assert_relative_eq!(phi.internal[1], 2.0, epsilon = 1e-6);
    }

    #[test]
    fn add_face_coeff_symmetrises_diag() {
        let mesh = unit_mesh();
        let mut mat = FvMatrix::new(mesh);
        mat.add_face_coeff(0, 1.0);
        // owner = 0, neighbour = 1 → diag[0] += 1, diag[1] += 1
        assert_relative_eq!(mat.ldu.diag[0], 1.0);
        assert_relative_eq!(mat.ldu.diag[1], 1.0);
        assert_relative_eq!(mat.ldu.upper[0], -1.0);
        assert_relative_eq!(mat.ldu.lower[0], -1.0);
    }
}
