use crate::mesh::{BoundaryPatch, FvMesh, FvMeshBuilder, MeshError, PatchKind};
use crate::primitives::Vector3;
use uom::si::area::square_meter;
use uom::si::f64::*;
use uom::si::length::meter;

/// Creates a uniform 1-D finite-volume mesh along the x-axis.
///
/// Produces `number_of_cells` equal-width cells spanning x ∈ \[0, `l`\] with a
/// constant cross-sectional area of `xs_area`.  All geometry is aligned with
/// the x-axis; y and z components are zero everywhere.
///
/// ## Layout
/// ```text
/// |  cell 0  |  cell 1  |  …  |  cell n-1  |
/// ^          ^          ^     ^             ^
/// left       i-face 0   …   i-face n-2    right
/// (patch)                                 (patch)
/// ```
///
/// Face ordering follows the OpenFOAM convention:
/// - `[0, n-1)` — internal faces (face `i` separates cell `i` from cell `i+1`)
/// - face `n-1` — `"right"` boundary at x = `l`  (outward normal = +x)
/// - face `n`   — `"left"`  boundary at x = 0   (outward normal = −x)
///
/// Both patches are typed [`PatchKind::Patch`] (generic).  Replace them via
/// [`FvMesh::patches`] if you need `Wall`, `Cyclic`, etc.
///
/// ## Parameters
/// - `l`               — total pipe length \[m\]
/// - `xs_area`         — constant cross-sectional area \[m²\]
/// - `number_of_cells` — number of cells; must be ≥ 1
///
/// ## Errors
/// Returns `Err` if `number_of_cells < 1`.
///
/// ## Example
/// ```rust
/// use uom::si::f64::*;
/// use uom::si::length::meter;
/// use uom::si::area::square_meter;
/// use outram_foam_basic_lib::interface::one_dimensional_meshing::create_one_d_mesh;
///
/// let mesh = create_one_d_mesh(
///     Length::new::<meter>(1.0),
///     Area::new::<square_meter>(0.01),
///     10,
/// ).unwrap();
///
/// assert_eq!(mesh.n_cells, 10);
/// assert_eq!(mesh.n_internal_faces, 9);
/// assert_eq!(mesh.n_faces, 11);
/// ```
pub fn create_one_d_mesh(
    l: Length,
    xs_area: Area,
    number_of_cells: i64,
) -> Result<FvMesh, MeshError> {
    if number_of_cells < 1 {
        return Err(MeshError::NonPositiveCellCount { got: number_of_cells });
    }
    let n = number_of_cells as usize;
    let l_m = l.get::<meter>();
    let a_m2 = xs_area.get::<square_meter>();
    let dx = l_m / n as f64;

    // ── Topology ──────────────────────────────────────────────────────────────
    // Face indices:
    //   [0, n-1)   internal — face i separates cell i from cell i+1
    //   n-1        right boundary, owned by cell n-1
    //   n          left  boundary, owned by cell 0
    let n_internal = n - 1;
    let n_faces = n + 1;

    let mut owner = Vec::with_capacity(n_faces);
    let mut neighbour = Vec::with_capacity(n_internal);
    for i in 0..n_internal {
        owner.push(i);
        neighbour.push(i + 1);
    }
    owner.push(n - 1); // right boundary
    owner.push(0);      // left boundary

    // ── Geometry ──────────────────────────────────────────────────────────────
    let cell_volumes: Vec<f64> = vec![dx * a_m2; n];
    let cell_centres: Vec<Vector3> = (0..n)
        .map(|i| Vector3::new((i as f64 + 0.5) * dx, 0.0, 0.0))
        .collect();

    let mut face_centres = Vec::with_capacity(n_faces);
    let mut face_area_vectors = Vec::with_capacity(n_faces);

    // Internal faces: pointing +x (owner → neighbour)
    for i in 0..n_internal {
        face_centres.push(Vector3::new((i as f64 + 1.0) * dx, 0.0, 0.0));
        face_area_vectors.push(Vector3::new(a_m2, 0.0, 0.0));
    }
    // Right boundary: outward from cell n-1, pointing +x
    face_centres.push(Vector3::new(l_m, 0.0, 0.0));
    face_area_vectors.push(Vector3::new(a_m2, 0.0, 0.0));
    // Left boundary: outward from cell 0, pointing -x
    face_centres.push(Vector3::new(0.0, 0.0, 0.0));
    face_area_vectors.push(Vector3::new(-a_m2, 0.0, 0.0));

    // ── Patches ───────────────────────────────────────────────────────────────
    // Must cover [n_internal, n_faces) without gaps.
    let patches = vec![
        BoundaryPatch::new("right", n_internal,     1, PatchKind::Patch),
        BoundaryPatch::new("left",  n_internal + 1, 1, PatchKind::Patch),
    ];

    FvMeshBuilder::new()
        .n_cells(n)
        .n_internal_faces(n_internal)
        .owner(owner)
        .neighbour(neighbour)
        .patches(patches)
        .cell_volumes(cell_volumes)
        .cell_centres(cell_centres)
        .face_area_vectors(face_area_vectors)
        .face_centres(face_centres)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::area::square_meter;
    use uom::si::length::meter;

    #[test]
    fn single_cell_mesh_is_valid() {
        let mesh = create_one_d_mesh(
            Length::new::<meter>(1.0),
            Area::new::<square_meter>(0.01),
            1,
        )
        .unwrap();
        assert_eq!(mesh.n_cells, 1);
        assert_eq!(mesh.n_internal_faces, 0);
        assert_eq!(mesh.n_faces, 2);
        assert_eq!(mesh.neighbour.len(), 0);
        mesh.validate().unwrap();
    }

    #[test]
    fn ten_cell_mesh_topology() {
        let mesh = create_one_d_mesh(
            Length::new::<meter>(1.0),
            Area::new::<square_meter>(0.05),
            10,
        )
        .unwrap();
        assert_eq!(mesh.n_cells, 10);
        assert_eq!(mesh.n_internal_faces, 9);
        assert_eq!(mesh.n_faces, 11);
        assert_eq!(mesh.owner.len(), 11);
        assert_eq!(mesh.neighbour.len(), 9);
        mesh.validate().unwrap();
    }

    #[test]
    fn cell_volumes_are_uniform() {
        let l = 2.0_f64;
        let a = 0.03_f64;
        let n = 5_i64;
        let mesh = create_one_d_mesh(
            Length::new::<meter>(l),
            Area::new::<square_meter>(a),
            n,
        )
        .unwrap();
        let expected = l * a / n as f64;
        for &v in &mesh.cell_volumes {
            assert!((v - expected).abs() < 1e-14, "volume {v} != {expected}");
        }
    }

    #[test]
    fn cell_centres_are_midpoints() {
        let n = 4_i64;
        let dx = 1.0 / n as f64;
        let mesh = create_one_d_mesh(
            Length::new::<meter>(1.0),
            Area::new::<square_meter>(1.0),
            n,
        )
        .unwrap();
        for (i, c) in mesh.cell_centres.iter().enumerate() {
            let expected_x = (i as f64 + 0.5) * dx;
            assert!((c.x - expected_x).abs() < 1e-14);
            assert_eq!(c.y, 0.0);
            assert_eq!(c.z, 0.0);
        }
    }

    #[test]
    fn boundary_patches_named_correctly() {
        let mesh = create_one_d_mesh(
            Length::new::<meter>(1.0),
            Area::new::<square_meter>(1.0),
            3,
        )
        .unwrap();
        assert_eq!(mesh.patches[0].name, "right");
        assert_eq!(mesh.patches[1].name, "left");
    }

    #[test]
    fn left_face_area_vector_points_negative_x() {
        let a = 0.1_f64;
        let mesh = create_one_d_mesh(
            Length::new::<meter>(1.0),
            Area::new::<square_meter>(a),
            3,
        )
        .unwrap();
        // left boundary is the last face
        let left_sf = mesh.face_area_vectors.last().unwrap();
        assert!((left_sf.x + a).abs() < 1e-14);
        assert_eq!(left_sf.y, 0.0);
    }

    #[test]
    fn zero_cells_returns_error() {
        assert!(create_one_d_mesh(
            Length::new::<meter>(1.0),
            Area::new::<square_meter>(1.0),
            0,
        )
        .is_err());
    }
}
