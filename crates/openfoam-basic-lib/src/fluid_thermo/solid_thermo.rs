use std::sync::Arc;

use crate::fields::field::Field;
use crate::fields::boundary::bc::{BoundaryCondition, PatchField};
use crate::fields::vol_field::VolScalarField;
use crate::mesh::fv_mesh::FvMesh;

/// Field-level solid thermodynamic model.
///
/// Solid regions have no flow — the only governing equation is the heat
/// conduction equation:
///
/// ```text
/// ρ·Cp·∂T/∂t = ∇·(κ∇T) + q̇
/// ```
///
/// This trait provides the two coefficients the energy equation needs:
/// `kappa()` for the Laplacian and `rho_cp()` for the ddt term.
///
/// Mirrors the role of `Foam::solidThermo` from
/// `src/thermophysicalModels/solidThermo/`.
pub trait SolidThermo {
    fn mesh(&self) -> &Arc<FvMesh>;

    /// Temperature field [K].
    fn t(&self) -> &VolScalarField;
    fn t_mut(&mut self) -> &mut VolScalarField;

    /// Thermal conductivity κ [W/(m·K)] — used in `fvm::laplacian(kappa, T)`.
    fn kappa(&self) -> VolScalarField;

    /// Volumetric heat capacity ρ·Cp [J/(m³·K)] — used in `fvm::ddt(rho_cp, T)`.
    fn rho_cp(&self) -> VolScalarField;

    /// Recompute temperature-dependent properties after T has been updated.
    ///
    /// For `ConstSolidThermo` this is a no-op; temperature-dependent models
    /// (e.g. polynomial κ(T)) update their cached values here.
    fn correct(&mut self);
}

// ── ConstSolidThermo ──────────────────────────────────────────────────────────

/// Solid thermo with constant κ and ρ·Cp.
///
/// Corresponds to `Foam::constSolidThermo` — the standard first choice for
/// metals, ceramics, and PCB substrates where property variation with T is
/// small.
///
/// ```rust
/// use openfoam_basic_lib::prelude::*;
/// use openfoam_basic_lib::fluid_thermo::{ConstSolidThermo, SolidThermo};
/// use std::sync::Arc;
///
/// let mesh = Arc::new(
///     FvMeshBuilder::new()
///         .n_cells(1).n_internal_faces(0)
///         .owner(vec![0]).neighbour(vec![])
///         .patches(vec![BoundaryPatch::new("wall", 0, 1, PatchKind::Wall)])
///         .cell_volumes(vec![1.0])
///         .cell_centres(vec![Vector3::ZERO])
///         .face_area_vectors(vec![Vector3::new(1.0, 0.0, 0.0)])
///         .face_centres(vec![Vector3::ZERO])
///         .build().unwrap()
/// );
/// let solid = ConstSolidThermo::new(mesh, 300.0, 16.0, 3.96e6);
/// assert!((solid.kappa().internal[0] - 16.0).abs() < 1e-12);
/// ```
#[derive(Debug, Clone)]
pub struct ConstSolidThermo {
    pub t:      VolScalarField,
    kappa_val:  f64,   // [W/(m·K)]
    rho_cp_val: f64,   // [J/(m³·K)]
}

impl ConstSolidThermo {
    /// Create a uniform solid thermo.
    ///
    /// - `t_init`: initial temperature [K]
    /// - `kappa`: thermal conductivity [W/(m·K)]
    /// - `rho_cp`: volumetric heat capacity ρ·Cp [J/(m³·K)]
    pub fn new(mesh: Arc<FvMesh>, t_init: f64, kappa: f64, rho_cp: f64) -> Self {
        Self {
            t: VolScalarField::uniform("T", mesh, t_init),
            kappa_val: kappa,
            rho_cp_val: rho_cp,
        }
    }

    fn uniform_field(&self, name: &str, val: f64) -> VolScalarField {
        let mesh = self.t.mesh.clone();
        let n = mesh.n_cells;
        let boundary = mesh.patches.iter()
            .map(|p| PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: Field::uniform(p.size, val),
            })
            .collect();
        VolScalarField::new(name, mesh, Field::uniform(n, val), boundary)
    }
}

impl SolidThermo for ConstSolidThermo {
    fn mesh(&self) -> &Arc<FvMesh> { &self.t.mesh }
    fn t(&self) -> &VolScalarField { &self.t }
    fn t_mut(&mut self) -> &mut VolScalarField { &mut self.t }
    fn kappa(&self) -> VolScalarField { self.uniform_field("kappa", self.kappa_val) }
    fn rho_cp(&self) -> VolScalarField { self.uniform_field("rhoCp", self.rho_cp_val) }
    fn correct(&mut self) {}  // properties are constant
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::Vector3;
    use crate::mesh::fv_mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};
    use approx::assert_relative_eq;

    fn one_cell_mesh() -> Arc<FvMesh> {
        Arc::new(FvMeshBuilder::new()
            .n_cells(1).n_internal_faces(0)
            .owner(vec![0, 0]).neighbour(vec![])
            .patches(vec![
                BoundaryPatch::new("hot",  0, 1, PatchKind::Wall),
                BoundaryPatch::new("cold", 1, 1, PatchKind::Wall),
            ])
            .cell_volumes(vec![1.0])
            .cell_centres(vec![Vector3::ZERO])
            .face_area_vectors(vec![
                Vector3::new(-1.0, 0.0, 0.0),
                Vector3::new( 1.0, 0.0, 0.0),
            ])
            .face_centres(vec![Vector3::ZERO, Vector3::ZERO])
            .build().unwrap())
    }

    #[test]
    fn kappa_field_uniform() {
        let m = one_cell_mesh();
        let s = ConstSolidThermo::new(m, 300.0, 16.0, 3.96e6);
        let k = s.kappa();
        assert_relative_eq!(k.internal[0], 16.0, epsilon = 1e-12);
    }

    #[test]
    fn rho_cp_field_uniform() {
        let m = one_cell_mesh();
        let s = ConstSolidThermo::new(m, 300.0, 16.0, 3.96e6);
        let rc = s.rho_cp();
        assert_relative_eq!(rc.internal[0], 3.96e6, epsilon = 1.0);
    }

    #[test]
    fn t_mutable() {
        let m = one_cell_mesh();
        let mut s = ConstSolidThermo::new(m, 300.0, 16.0, 3.96e6);
        s.t_mut().internal[0] = 500.0;
        assert_relative_eq!(s.t().internal[0], 500.0, epsilon = 1e-12);
    }

    #[test]
    fn conduction_solve() {
        // 2-cell 1D solid, T_left=500K, T_right=300K, kappa=1, dx=0.5
        // Steady state: T[0]=450K, T[1]=350K (linear profile)
        use crate::fv_operators::fvm;
        use std::sync::Arc;
        use crate::mesh::fv_mesh::FvMeshBuilder;

        let mesh = Arc::new(FvMeshBuilder::new()
            .n_cells(2).n_internal_faces(1)
            .owner(vec![0, 1, 0]).neighbour(vec![1])
            .patches(vec![
                BoundaryPatch::new("hot",  1, 1, PatchKind::Wall),
                BoundaryPatch::new("cold", 2, 1, PatchKind::Wall),
            ])
            .cell_volumes(vec![0.5, 0.5])
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
            .build().unwrap());

        // Set FixedValue BCs: hot face (right, owner=1) = 500K, cold (left, owner=0) = 300K
        use crate::fields::boundary::bc::BoundaryCondition;
        use crate::fields::field::Field;
        let t_bcs = vec![
            PatchField { bc: BoundaryCondition::FixedValue(500.0), values: Field::new(vec![0.0]) },
            PatchField { bc: BoundaryCondition::FixedValue(300.0), values: Field::new(vec![0.0]) },
        ];
        let solid = ConstSolidThermo {
            t: VolScalarField::new("T", mesh.clone(),
                Field::uniform(2, 400.0), t_bcs),
            kappa_val: 1.0,
            rho_cp_val: 1.0,
        };

        // Assemble: -∇·(κ∇T) = 0  →  laplacian(κ, T) == 0
        // Need surface kappa — use internal kappa value directly for this test
        use crate::fields::surface_field::SurfaceScalarField;
        let kappa_surf = {
            let n_int = mesh.n_internal_faces;
            let bnd: Vec<_> = mesh.patches.iter()
                .map(|p| PatchField { bc: BoundaryCondition::ZeroGradient, values: Field::uniform(p.size, 1.0) })
                .collect();
            SurfaceScalarField::new("kappa", mesh.clone(), Field::uniform(n_int, 1.0), bnd)
        };
        let eqn = fvm::laplacian(&kappa_surf, &solid.t);
        let settings = crate::ldu_matrix::fv_matrix::SolverSettings::default();
        let (t_new, perf) = eqn.solve("T", settings);
        assert!(perf.converged, "residual = {}", perf.final_residual);
        // Linear profile in 1D: cell centres at 0.25 and 0.75
        // T(x) = 300 + (500-300)*(x) where x goes 0→1 for cold→hot
        // T[0] at x=0.25: 300 + 200*0.25 = 350?
        // Wait: hot is on the right (face at x=1, owner=cell 1), cold on left (face at x=0, owner=cell 0)
        // T(x=0)=300 (cold, left), T(x=1)=500 (hot, right) → T = 300 + 200x
        // T[0] at x=0.25: 350K; T[1] at x=0.75: 450K
        assert_relative_eq!(t_new.internal[0], 350.0, epsilon = 1.0);
        assert_relative_eq!(t_new.internal[1], 450.0, epsilon = 1.0);
    }
}
