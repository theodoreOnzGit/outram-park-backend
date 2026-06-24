use std::sync::Arc;

use crate::fields::boundary::bc::{BoundaryCondition, PatchField};
use crate::fields::field::Field;
use crate::fields::vol_field::VolScalarField;
use crate::mesh::fv_mesh::FvMesh;
use crate::primitives::Vector3;

/// Face-to-face mapping between two mesh patches at a shared interface.
///
/// Used by `chtMultiRegionFoam`-style solvers where a fluid region and a
/// solid region share an interface.  Each side has a patch (identified by
/// mesh + patch index); the `face_map` gives the paired face index on side B
/// for each face on side A.
///
/// For matching meshes (same layout, same face count) `face_map[i] = i`.
/// For non-matching meshes (different refinements) the map is built by
/// nearest-face-centre search (see `from_face_centres`).
#[derive(Debug, Clone)]
pub struct RegionInterface {
    pub mesh_a:    Arc<FvMesh>,
    pub patch_a:   usize,
    pub mesh_b:    Arc<FvMesh>,
    pub patch_b:   usize,
    /// `face_map[fi_a]` = `fi_b` on the B-side patch.
    pub face_map:  Vec<usize>,
}

impl RegionInterface {
    /// Construct a matching interface: face `i` on A is coupled to face `i` on B.
    ///
    /// Panics if the two patches have different sizes.
    pub fn matching(
        mesh_a: Arc<FvMesh>, patch_a: usize,
        mesh_b: Arc<FvMesh>, patch_b: usize,
    ) -> Self {
        let n_a = mesh_a.patches[patch_a].size;
        let n_b = mesh_b.patches[patch_b].size;
        assert_eq!(n_a, n_b, "matching interface requires equal patch sizes");
        Self {
            mesh_a, patch_a,
            mesh_b, patch_b,
            face_map: (0..n_a).collect(),
        }
    }

    /// Construct a non-matching interface via nearest-face-centre search.
    ///
    /// For each face on patch A, the closest face centre on patch B is found
    /// and stored in `face_map`.  O(n_A × n_B) — acceptable for boundary
    /// patches which are typically small.
    pub fn from_face_centres(
        mesh_a: Arc<FvMesh>, patch_a: usize,
        mesh_b: Arc<FvMesh>, patch_b: usize,
    ) -> Self {
        let patch_info_a = &mesh_a.patches[patch_a];
        let patch_info_b = &mesh_b.patches[patch_b];

        let centres_b: Vec<Vector3> = (0..patch_info_b.size)
            .map(|fi| mesh_b.face_centres[patch_info_b.start + fi])
            .collect();

        let face_map: Vec<usize> = (0..patch_info_a.size)
            .map(|fi_a| {
                let ca = mesh_a.face_centres[patch_info_a.start + fi_a];
                centres_b.iter().enumerate()
                    .min_by(|(_, cb1), (_, cb2)| {
                        (ca - **cb1).mag_sqr().partial_cmp(&(ca - **cb2).mag_sqr()).unwrap()
                    })
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            })
            .collect();

        Self { mesh_a, patch_a, mesh_b, patch_b, face_map }
    }

    /// Exchange temperature boundary values at the interface.
    ///
    /// Returns `(bc_a_new, bc_b_new)` where each is a `PatchField` of
    /// `FixedValue` temperatures drawn from the cell-centre values of the
    /// opposite region's cells adjacent to the interface.
    ///
    /// This implements the simplest coupling: Dirichlet T on each side equals
    /// the adjacent cell T of the other side.  A more accurate coupling would
    /// use the heat-flux equality condition; that requires knowing `kappa` on
    /// both sides (available from `SolidThermo::kappa()`).
    pub fn exchange_temperature(
        &self,
        t_a: &VolScalarField,
        t_b: &VolScalarField,
    ) -> (PatchField<f64>, PatchField<f64>) {
        let pa = &self.mesh_a.patches[self.patch_a];
        let pb = &self.mesh_b.patches[self.patch_b];

        // Values on A-side face come from adjacent B-side cells
        let vals_a: Vec<f64> = (0..pa.size)
            .map(|fi_a| {
                let fi_b  = self.face_map[fi_a];
                let gf_b  = pb.start + fi_b;
                let cell_b = self.mesh_b.owner[gf_b];
                t_b.internal[cell_b]
            })
            .collect();

        // Values on B-side face come from adjacent A-side cells
        let n_b = pb.size;
        let mut vals_b = vec![0.0; n_b];
        for fi_a in 0..pa.size {
            let fi_b   = self.face_map[fi_a];
            let gf_a   = pa.start + fi_a;
            let cell_a = self.mesh_a.owner[gf_a];
            vals_b[fi_b] = t_a.internal[cell_a];
        }

        let bc_a = PatchField {
            bc: BoundaryCondition::FixedValue(0.0),   // sentinel; values override
            values: Field::new(vals_a),
        };
        let bc_b = PatchField {
            bc: BoundaryCondition::FixedValue(0.0),
            values: Field::new(vals_b),
        };
        (bc_a, bc_b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::vol_field::VolScalarField;
    use crate::mesh::fv_mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};

    fn simple_mesh(_hot: bool) -> Arc<FvMesh> {
        Arc::new(FvMeshBuilder::new()
            .n_cells(2).n_internal_faces(1)
            .owner(vec![0, 1, 0]).neighbour(vec![1])
            .patches(vec![
                BoundaryPatch::new("interface", 1, 1, PatchKind::Wall),
                BoundaryPatch::new("outer",     2, 1, PatchKind::Wall),
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
    fn exchange_temperature_matching() {
        let m_a = simple_mesh(true);
        let m_b = simple_mesh(false);
        let iface = RegionInterface::matching(m_a.clone(), 0, m_b.clone(), 0);

        let t_a = VolScalarField::uniform("T_a", m_a.clone(), 500.0);
        let t_b = VolScalarField::uniform("T_b", m_b.clone(), 300.0);
        let (bc_a, bc_b) = iface.exchange_temperature(&t_a, &t_b);

        // A-side BC gets B's adjacent cell temperature (300)
        assert!((bc_a.values[0] - 300.0).abs() < 1e-10);
        // B-side BC gets A's adjacent cell temperature (500)
        assert!((bc_b.values[0] - 500.0).abs() < 1e-10);
    }

    #[test]
    fn non_matching_interface_nearest_centre() {
        let m_a = simple_mesh(true);
        let m_b = simple_mesh(false);
        let iface = RegionInterface::from_face_centres(m_a.clone(), 0, m_b.clone(), 0);
        // Should map face 0 → face 0 (same centre)
        assert_eq!(iface.face_map[0], 0);
    }
}
