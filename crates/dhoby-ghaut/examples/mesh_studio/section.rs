//! Cut-plane sections of a cfMesh [`VolumeMesh`], coloured cell by cell
//! (gh:#1766).
//!
//! What a reader needs to see of a volume mesh is its **inside**: the cells,
//! the boundary layers against the wall, and where the quality is poor. This
//! module cuts a mesh with an axis-aligned plane and gives one polygon per
//! cut cell, plus a value per cell to colour it by.
//!
//! # Reuse, not new section code
//!
//! The cutting is `outram_foam_mesh::section::plane_section`, the same
//! function Foam Studio draws its meshes with. It takes the solver's
//! `PolyMesh`, so a [`VolumeMesh`] goes there through the existing
//! converters: `outram_blender::unstructured::convert::cfmesh::from_volume_mesh`
//! to the neutral mesh, then `convert::foam::to_poly_mesh`, then
//! `PolyMesh::from_foam_poly_mesh` (Foam Studio's chain, gh:#1202). The
//! chain keeps the cell order, so a per-cell value indexed by the
//! `VolumeMesh` lands on the right polygon:
//! `tests::the_conversion_keeps_the_cell_order` checks it cell by cell.
//!
//! # What a cut cell's polygon is
//!
//! `plane_section` returns, per cell, the convex hull of the points where the
//! plane crosses the cell's edges. For a convex cell (the carved hexahedra,
//! the tetrahedra) that is the exact section. For a non-convex polyhedral
//! dual cell it is the hull of the true section, so neighbouring polygons can
//! overlap slightly. That is a drawing approximation only; no number reported
//! here comes from the polygons except in the convex-cell area test.
//!
//! # The colourings
//!
//! - **Boundary layer**: 0 for a core cell, k for the k-th layer from the
//!   wall (`outram_park_fork_cfmesh::pipeline::staged::boundary_layer_index`).
//!   Defined only on meshes that carry the finished cell labels: the layer
//!   stage and the two after it. Earlier stages are all 0.
//! - **Non-orthogonality** \[deg\] and **skewness**: per cell, the worst over
//!   its internal faces (`outram_park_fork_cfmesh::checks::cell_quality`),
//!   whose maxima equal `checkMesh`'s report exactly.

use outram_blender::foam_mesh::VolumeMesh;
use outram_foam_mesh::block_mesh::PolyMesh;
use outram_foam_mesh::section::{plane_section, Axis, CellSection};
use uom::si::f64::Length;
use uom::si::length::meter;

/// What the cut cells are coloured by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Colouring {
    /// Boundary-layer index: 0 core, k = k-th layer from the wall.
    Layer,
    /// Worst internal-face non-orthogonality of the cell \[deg\].
    NonOrthogonality,
    /// Worst internal-face skewness of the cell (dimensionless).
    Skewness,
}

impl Colouring {
    pub const ALL: [Colouring; 3] = [
        Colouring::Layer,
        Colouring::NonOrthogonality,
        Colouring::Skewness,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Colouring::Layer => "boundary layer",
            Colouring::NonOrthogonality => "non-orthogonality [deg]",
            Colouring::Skewness => "skewness",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Colouring::Layer => "layer",
            Colouring::NonOrthogonality => "nonorth",
            Colouring::Skewness => "skew",
        }
    }

    pub fn from_slug(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|c| c.slug() == s)
    }

    pub fn code(self) -> f64 {
        match self {
            Colouring::Layer => 0.0,
            Colouring::NonOrthogonality => 1.0,
            Colouring::Skewness => 2.0,
        }
    }

    pub fn from_code(c: f64) -> Option<Self> {
        match c as i64 {
            0 => Some(Colouring::Layer),
            1 => Some(Colouring::NonOrthogonality),
            2 => Some(Colouring::Skewness),
            _ => None,
        }
    }
}

/// An axis by number (0 x, 1 y, 2 z), as the wire and the URL carry it.
pub fn axis_of(a: u8) -> Axis {
    match a {
        0 => Axis::X,
        1 => Axis::Y,
        _ => Axis::Z,
    }
}

/// `vm` as the solver's `PolyMesh`, through Foam Studio's converters.
///
/// # Errors
/// When a converter refuses the mesh (it reports why).
pub fn to_poly(vm: &VolumeMesh) -> Result<PolyMesh, String> {
    let neutral = outram_blender::unstructured::convert::cfmesh::from_volume_mesh(vm)
        .map_err(|e| format!("{e:?}"))?;
    let foam = outram_blender::unstructured::convert::foam::to_poly_mesh(&neutral)
        .map_err(|e| format!("{e:?}"))?;
    Ok(PolyMesh::from_foam_poly_mesh(&foam))
}

/// The cells `poly` cut by the plane `axis = offset` \[m\], one polygon each
/// (in-plane coordinates \[m\], see `outram_foam_mesh::section::Axis`).
pub fn cut(poly: &PolyMesh, axis: u8, offset: f64) -> Vec<CellSection> {
    plane_section(poly, axis_of(axis), Length::new::<meter>(offset))
}

/// The value of `colouring` for every cell of `vm`. `layer` is the layer
/// index of `vm`'s cells when it has one (the layer stage onward), else
/// `None`, and every cell is then core.
pub fn values(vm: &VolumeMesh, colouring: Colouring, layer: Option<&[u32]>) -> Vec<f64> {
    match colouring {
        Colouring::Layer => match layer {
            Some(l) if l.len() == vm.n_cells => l.iter().map(|&k| f64::from(k)).collect(),
            _ => vec![0.0; vm.n_cells],
        },
        Colouring::NonOrthogonality => {
            outram_park_fork_cfmesh::checks::cell_quality(vm).non_orthogonality_deg
        }
        Colouring::Skewness => outram_park_fork_cfmesh::checks::cell_quality(vm).skewness,
    }
}

/// The extent of `vm` along each axis \[m\]: `(min, max)`.
pub fn bounds(vm: &VolumeMesh) -> ([f64; 3], [f64; 3]) {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for p in &vm.points {
        for (k, v) in [p.x, p.y, p.z].into_iter().enumerate() {
            lo[k] = lo[k].min(v);
            hi[k] = hi[k].max(v);
        }
    }
    (lo, hi)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{build, GeomKind, Params};
    use outram_park_fork_cfmesh::pipeline::staged::PipelineStage;

    /// Sum of each cell's vertex positions and its vertex count, from the
    /// faces: an order-sensitive fingerprint of which points a cell has.
    fn cell_vertex_means(
        points: &[[f64; 3]],
        faces: &[(Vec<usize>, usize, Option<usize>)],
        n_cells: usize,
    ) -> Vec<[f64; 3]> {
        let mut sets: Vec<std::collections::BTreeSet<usize>> = vec![Default::default(); n_cells];
        for (ring, own, nb) in faces {
            sets[*own].extend(ring.iter().copied());
            if let Some(n) = nb {
                sets[*n].extend(ring.iter().copied());
            }
        }
        sets.iter()
            .map(|s| {
                let mut m = [0.0; 3];
                for &v in s {
                    for k in 0..3 {
                        m[k] += points[v][k];
                    }
                }
                let n = s.len() as f64;
                [m[0] / n, m[1] / n, m[2] / n]
            })
            .collect()
    }

    /// Cell `i` of the `VolumeMesh` is cell `i` of the `PolyMesh`.
    ///
    /// **Methodology.** For every cell, the mean of its distinct vertex
    /// positions, computed from each mesh's own faces, must agree. Checked
    /// on the finished box mesh (polyhedral dual cells plus 3 boundary
    /// layers), where a reordering would move every layer colour.
    #[test]
    fn the_conversion_keeps_the_cell_order() {
        let b = build(&Params::default()).expect("build");
        let poly = to_poly(&b.mesh).expect("convert");
        assert_eq!(poly.n_cells, b.mesh.n_cells);

        let vm_points: Vec<[f64; 3]> = b.mesh.points.iter().map(|p| [p.x, p.y, p.z]).collect();
        let vm_faces: Vec<(Vec<usize>, usize, Option<usize>)> = (0..b.mesh.faces.len())
            .map(|f| {
                (
                    b.mesh.faces[f].clone(),
                    b.mesh.owner[f],
                    b.mesh.neighbour[f],
                )
            })
            .collect();
        let pm_points: Vec<[f64; 3]> = poly.points.iter().map(|p| [p.x, p.y, p.z]).collect();
        let pm_faces: Vec<(Vec<usize>, usize, Option<usize>)> = poly
            .faces
            .iter()
            .map(|f| (f.verts.clone(), f.owner, f.neighbour))
            .collect();

        let a = cell_vertex_means(&vm_points, &vm_faces, b.mesh.n_cells);
        let c = cell_vertex_means(&pm_points, &pm_faces, poly.n_cells);
        let worst = a
            .iter()
            .zip(&c)
            .map(|(x, y)| (0..3).map(|k| (x[k] - y[k]).abs()).fold(0.0, f64::max))
            .fold(0.0, f64::max);
        assert!(
            worst < 1e-12,
            "cell order moved: worst vertex-mean difference {worst} m"
        );
    }

    /// A mid-plane cut through the carved box covers the box's cross-section.
    ///
    /// **Methodology.** The carve of the 2 m cube is axis-aligned hexahedra
    /// (convex, so each polygon is exact) with no snap needed: a cut at z = 0
    /// must tile the 2 m x 2 m square, total area 4 m^2, with one polygon per
    /// cut cell. Cut at a non-node height so no face lies on the plane.
    #[test]
    fn a_mid_plane_cut_of_the_carved_box_has_the_box_area() {
        // The carved source, bare (gh:#1829: TetGen is the default, and the
        //   carved source takes no layers).
        let mut p = Params::default();
        p.opts.tet_source = outram_park_fork_cfmesh::pipeline::TetSource::CarvedCentroid;
        p.opts.n_layers = 0;
        let b = build(&p).expect("build");
        let carve = &b
            .stages
            .iter()
            .find(|s| s.stage == PipelineStage::Carve)
            .expect("carve stage")
            .mesh;
        let poly = to_poly(carve).expect("convert");
        let cut = cut(&poly, 2, 0.1);
        let area: f64 = cut
            .iter()
            .map(|c| outram_foam_mesh::section::polygon_area(&c.polygon).abs())
            .sum();
        assert!((area - 4.0).abs() < 1e-9, "cut area {area} m^2, expected 4");
        let ids: std::collections::BTreeSet<usize> = cut.iter().map(|c| c.cell).collect();
        assert_eq!(ids.len(), cut.len(), "one polygon per cut cell");
    }

    /// Layer colouring is 0 everywhere before the layer stage and carries the
    /// index after it; the box's finished mesh has layers 1..=3 in its cut.
    #[test]
    fn layer_colouring_follows_the_stage() {
        let b = build(&Params {
            kind: GeomKind::Box,
            ..Default::default()
        })
        .expect("build");
        let carve = &b.stages[0].mesh;
        assert!(values(carve, Colouring::Layer, None)
            .iter()
            .all(|&v| v == 0.0));
        let fin = &b.mesh;
        let v = values(fin, Colouring::Layer, Some(&b.layer_index.per_cell));
        let poly = to_poly(fin).expect("convert");
        let seen: std::collections::BTreeSet<u32> = cut(&poly, 2, 0.1)
            .iter()
            .map(|c| v[c.cell] as u32)
            .collect();
        assert!(
            seen.contains(&0) && seen.contains(&1) && seen.contains(&3),
            "layers in the cut: {seen:?}"
        );
    }
}
