// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Provenance: original OUTRAM PARK code. It introduces no meshing algorithm:
// it hands the input surface to this crate's port of TetGen 1.6.0
// (`crate::tetgen`, AGPL-3.0-or-later, Hang Si / WIAS) and returns the
// tetrahedra as a `VolumeMesh` for the tet -> dual -> layers pipeline.

//! **Where the tet -> dual -> layers pipeline gets its tetrahedra**
//! (gh:#1796, stage 7 of the TetGen epic gh:#1789).
//!
//! [`TetSource::CarvedCentroid`] is the pipeline's historical route and the
//! default: carve a Cartesian grid, snap its boundary to the surface and
//! split every hexahedron into tetrahedra about its centroid
//! ([`crate::tet::tetrahedralize`]). Its interior is a regular lattice, so
//! its dual is a regular truncated-octahedron lattice.
//!
//! [`TetSource::TetGen`] hands the surface to TetGen instead (`-pq<r>/<d>`,
//! plus `-a<V>` when a volume bound is given): a constrained Delaunay
//! tetrahedralisation of the surface itself, refined to a radius-edge
//! bound. The tetrahedra are unstructured and grade from the surface's own
//! triangle size inward, so the dual is a mesh of irregular polyhedra (the
//! picture the maintainer asked for on gh:#1796).
//!
//! # What the TetGen route replaces, and what it keeps
//!
//! - **Carve and snap are not run.** TetGen's boundary is the input surface
//!   (its vertices, plus Steiner points TetGen puts on its facets and
//!   segments), so there is no staircase to snap. The staged log lists both
//!   with `applied: false` and an empty mesh (they were given nothing).
//! - **Tetrahedralise** is TetGen's mesh ([`tetgen_tets`]).
//! - **The Delaunay flips are not run** (recorded with `applied: false` and
//!   the TetGen mesh, which they were given). TetGen's `-q` output is
//!   already a constrained Delaunay tetrahedralisation refined to the
//!   ratio bound and then improved by its own flips and smoothing
//!   (`improve_mesh`); [`crate::delaunay::flip_to_delaunay`] judges Delaunay
//!   with plain `f64` predicates and does not know TetGen's constraining
//!   subfaces, so the most it could do is undo a constrained face. Skipping
//!   it is the principled choice, not a speed one.
//! - **Everything after** (dual, smoothing, feature capture, optimise,
//!   layers, name patches, upper-triangular order) runs unchanged.
//!
//! # Patches
//!
//! The surface becomes a PLC through
//! [`PlcInput::from_triangle_soup`](crate::tetgen::PlcInput::from_triangle_soup)
//! with one marker per triangle: the triangle's region index when the
//! surface has named regions, else 0 for every triangle (one patch named
//! after [`super::TetDualOptions::wall_patch`]). Coplanar triangles of one
//! marker merge into one facet (TetGen's `meshsurface`), and TetGen keeps
//! the seams between markers as segments, so the tet mesh's patches are the
//! surface's regions ([`crate::tetgen::to_volume_mesh`] names them).
//!
//! # Wall grading (gh:#1825)
//!
//! `-pq<r>a<V>` alone refines only where the surface triangles are small
//! (near the pebble), not toward the pipe or channel walls. With
//! `wall_grade: Some(..)` ([`WallGrade`], [`wall_grade`]) every PLC point,
//! plus seed points inside the volume, gets a TetGen `-m` size graded from
//! the walls, and TetGen runs with `-m` (`-pq<r>/<d>[a<V>]m`). The walls are
//! the regions named in `walls` ([`tetgen_tets_graded`], the pipeline's
//! [`super::TetDualOptions::layer_patches`]), or the whole surface when it
//! is empty. Off (`None`) by default.
//!
//! **Untrusted AI-assisted draft pending human V&V.** Education, research
//! and V&V only, per the workspace `RESPONSIBLE_USE.md`.

pub mod wall_grade;
pub use wall_grade::WallGrade;

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use crate::math::Vec3;
use crate::patches::SurfaceRegions;
use crate::tetgen::{tetrahedralize_plc, to_volume_mesh, PlcInput, Switches};
use crate::volume_mesh::VolumeMesh;

/// Which tetrahedra the tet -> dual -> layers pipeline starts from
/// ([`super::TetDualOptions::tet_source`]). See the module docs.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum TetSource {
    /// Carve a Cartesian grid, snap it, split each hexahedron about its
    /// centroid. The default, unchanged (changing it needs the maintainer).
    #[default]
    CarvedCentroid,
    /// TetGen's quality tetrahedralisation of the surface:
    /// `-pq<radius_edge>/<min_dihedral>`, and `-a<max_volume>` when given.
    TetGen {
        /// Radius-edge ratio bound (`-q`'s first number; TetGen's default
        /// 2.0, `1.414` is the common quality choice). Must be > 1.
        radius_edge: f64,
        /// Smallest dihedral angle TetGen refines toward, in degrees
        /// (`-q`'s second number; TetGen's default 3.5, see
        /// [`TetSource::TETGEN_DEFAULT_MIN_DIHEDRAL`]).
        min_dihedral: f64,
        /// Largest tetrahedron volume, in cubic metres (`-a`), or `None` for
        /// no volume bound (the tetrahedra then grade from the surface
        /// triangles' size only).
        max_volume: Option<f64>,
        /// Sizes graded from the walls (`-m`, gh:#1825; see the module
        /// docs), or `None` (the default) for none.
        wall_grade: Option<WallGrade>,
    },
}

impl TetSource {
    /// TetGen's own default smallest dihedral angle under `-q`, degrees
    /// (`tetgenbehavior::mindihedral`, `tetgen.h`).
    pub const TETGEN_DEFAULT_MIN_DIHEDRAL: f64 = 3.5;

    /// TetGen with radius-edge bound `radius_edge`, TetGen's default
    /// dihedral bound, an optional volume bound \[m³\], and no wall
    /// grading.
    #[must_use]
    pub fn tetgen(radius_edge: f64, max_volume: Option<f64>) -> Self {
        TetSource::TetGen {
            radius_edge,
            min_dihedral: Self::TETGEN_DEFAULT_MIN_DIHEDRAL,
            max_volume,
            wall_grade: None,
        }
    }

    /// This source with the wall grading `grade` (gh:#1825); the carved
    /// route is returned unchanged (it has its own near-wall refinement,
    /// [`super::TetDualOptions::refinement_levels`]).
    #[must_use]
    pub fn with_wall_grade(self, grade: Option<WallGrade>) -> Self {
        match self {
            TetSource::CarvedCentroid => self,
            TetSource::TetGen {
                radius_edge,
                min_dihedral,
                max_volume,
                ..
            } => TetSource::TetGen {
                radius_edge,
                min_dihedral,
                max_volume,
                wall_grade: grade,
            },
        }
    }

    /// The TetGen command-line switches this source runs (`"pq1.414/3.5a0.0026"`,
    /// with a trailing `m` when wall-graded), or `None` for the carved
    /// route. For the report and the stage note.
    #[must_use]
    pub fn switches(&self) -> Option<String> {
        match *self {
            TetSource::CarvedCentroid => None,
            TetSource::TetGen {
                radius_edge,
                min_dihedral,
                max_volume,
                wall_grade,
            } => {
                let m = if wall_grade.is_some() { "m" } else { "" };
                Some(match max_volume {
                    Some(v) => alloc::format!("pq{radius_edge}/{min_dihedral}a{v}{m}"),
                    None => alloc::format!("pq{radius_edge}/{min_dihedral}{m}"),
                })
            }
        }
    }
}

/// A mesh with no cells: what the staged log records for a stage the
/// TetGen route does not run and gives nothing (carve, snap).
pub(crate) fn empty_mesh() -> VolumeMesh {
    VolumeMesh {
        points: Vec::new(),
        faces: Vec::new(),
        owner: Vec::new(),
        neighbour: Vec::new(),
        n_cells: 0,
        patches: Vec::new(),
        cells: None,
    }
}

/// One TetGen hole point (`holelist`) inside every cavity of the domain a
/// closed, outward-wound surface bounds: a pebble in a channel, a void in
/// a block.
///
/// TetGen's `carveholes` removes what it can reach from the convex hull
/// without crossing a facet, and the region inside each hole point; a
/// cavity enclosed by its own closed surface is neither, so without a hole
/// point TetGen fills it (on Mesh Studio's pebble it meshed the pebble's
/// inside and the pebble patch vanished into internal faces). The surface's
/// winding says which parts are cavities: each connected component (by
/// shared vertices) whose signed volume is **negative** is wound with its
/// normals into what it encloses, so what it encloses is outside the
/// domain. Its hole point is the centroid of its largest triangle moved
/// along the triangle's normal by 1e-3 of the triangle's shortest edge
/// (inside the cavity, nearer that triangle than any other facet for any
/// cavity whose walls are not within that distance of each other).
///
/// A component with positive volume (the outer boundary, or an annulus
/// such as the fuel channel) gets none: what lies outside it is reached
/// from the hull.
pub fn hole_points(points: &[Vec3], tris: &[[usize; 3]]) -> Vec<[f64; 3]> {
    // Union-find over the vertices the triangles use.
    let mut parent: Vec<usize> = (0..points.len()).collect();
    fn root(parent: &mut [usize], mut a: usize) -> usize {
        while parent[a] != a {
            parent[a] = parent[parent[a]];
            a = parent[a];
        }
        a
    }
    for t in tris {
        for k in 1..3 {
            let (a, b) = (root(&mut parent, t[0]), root(&mut parent, t[k]));
            if a != b {
                parent[a.max(b)] = a.min(b);
            }
        }
    }
    // Per component (keyed by its root, in order of first triangle):
    // six times its signed volume, and its largest triangle.
    let mut comps: Vec<(usize, f64, usize, f64)> = Vec::new();
    for (i, t) in tris.iter().enumerate() {
        let r = root(&mut parent, t[0]);
        let (a, b, c) = (points[t[0]], points[t[1]], points[t[2]]);
        let v6 = a.dot(b.cross(c));
        let area2 = b.sub(a).cross(c.sub(a)).length();
        match comps.iter_mut().find(|x| x.0 == r) {
            Some(x) => {
                x.1 += v6;
                if area2 > x.3 {
                    x.2 = i;
                    x.3 = area2;
                }
            }
            None => comps.push((r, v6, i, area2)),
        }
    }
    comps
        .iter()
        .filter(|x| x.1 < 0.0 && x.3 > 0.0)
        .map(|x| {
            let t = tris[x.2];
            let (a, b, c) = (points[t[0]], points[t[1]], points[t[2]]);
            let n = b.sub(a).cross(c.sub(a)).normalize();
            let shortest = b
                .sub(a)
                .length()
                .min(c.sub(b).length())
                .min(a.sub(c).length());
            let p = a
                .add(b)
                .add(c)
                .scale(1.0 / 3.0)
                .add(n.scale(1e-3 * shortest));
            [p.x, p.y, p.z]
        })
        .collect()
}

/// TetGen's tetrahedra of the closed surface `points` / `tris`, as a
/// [`VolumeMesh`] with one cell per tetrahedron and one boundary patch per
/// region (`regions`' names, or `[wall_patch]` for a surface without
/// regions), and a one-line note for the report.
///
/// # Errors
/// When the source is not [`TetSource::TetGen`], when a bound is out of
/// range, or when TetGen refuses the surface (a self-intersection, a
/// degenerate input); the message says which. Never a silent fallback to
/// the carved route.
pub fn tetgen_tets(
    points: &[Vec3],
    tris: &[[usize; 3]],
    regions: Option<&SurfaceRegions>,
    wall_patch: &str,
    source: TetSource,
) -> Result<(VolumeMesh, String), String> {
    tetgen_tets_graded(points, tris, regions, wall_patch, &[], source)
}

/// [`tetgen_tets`] with the walls of a wall-graded source named: the
/// triangles of the regions in `walls`, or every triangle when `walls` is
/// empty or the surface has no regions (gh:#1825). Without a
/// [`WallGrade`] in `source`, `walls` is not read.
///
/// # Errors
/// As [`tetgen_tets`], and a [`WallGrade`] out of range
/// ([`WallGrade::check`]).
pub fn tetgen_tets_graded(
    points: &[Vec3],
    tris: &[[usize; 3]],
    regions: Option<&SurfaceRegions>,
    wall_patch: &str,
    walls: &[String],
    source: TetSource,
) -> Result<(VolumeMesh, String), String> {
    let run = tetgen_plc(points, tris, regions, wall_patch, walls, source)?;
    let r = tetrahedralize_plc(&run.plc, &run.behavior)
        .map_err(|e| alloc::format!("TetGen refused the surface: {e:?}"))?;
    let out = r
        .output
        .ok_or("TetGen wrote no output (diagnose mode is not used here)")?;
    let mesh = to_volume_mesh(&out, Some(&run.names));
    let mut note = alloc::format!(
        "tet source TetGen -{}: {} tetrahedra on {} nodes; carve, snap and the Delaunay flips not run \
         (TetGen's constrained Delaunay refinement replaces them)",
        source.switches().unwrap_or_default(),
        out.tets.len(),
        out.points.len()
    );
    if let TetSource::TetGen {
        wall_grade: Some(g),
        ..
    } = source
    {
        note += &alloc::format!(
            "; wall-graded sizes {:.4} m at the walls, x{} growth, {:.4} m inside ({} seed points)",
            g.wall_size,
            g.growth,
            g.interior_size,
            run.n_seeds
        );
    }
    Ok((mesh, note))
}

/// What [`tetgen_plc`] hands TetGen: the PLC (with hole points and, when
/// wall-graded, the seed points and every point's `-m` size), the
/// switches, the patch name of every facet marker, and the number of seed
/// points appended after the surface's points.
#[derive(Debug, Clone)]
pub struct TetgenRun {
    /// The PLC (`.smesh` + `.mtr`, [`PlcInput::to_smesh`]).
    pub plc: PlcInput,
    /// The behaviour: `-pq<r>/<d>`, with `a<V>` and `m` as the source asks.
    pub behavior: crate::tetgen::Behavior,
    /// The patch name of each facet marker.
    pub names: Vec<String>,
    /// Seed points appended to [`PlcInput::points`] (0 without grading).
    pub n_seeds: usize,
}

/// The PLC and switches [`tetgen_tets_graded`] runs TetGen on (public so
/// that the V&V record can write the same input for the upstream binary,
/// gh:#1825).
///
/// # Errors
/// As [`tetgen_tets_graded`].
pub fn tetgen_plc(
    points: &[Vec3],
    tris: &[[usize; 3]],
    regions: Option<&SurfaceRegions>,
    wall_patch: &str,
    walls: &[String],
    source: TetSource,
) -> Result<TetgenRun, String> {
    let TetSource::TetGen {
        radius_edge,
        min_dihedral,
        max_volume,
        wall_grade,
    } = source
    else {
        return Err("tetgen_tets called with the carved tet source".into());
    };
    if !(radius_edge > 1.0 && radius_edge.is_finite()) {
        return Err(alloc::format!(
            "TetGen radius-edge bound must be > 1 (got {radius_edge})"
        ));
    }
    if !(0.0..90.0).contains(&min_dihedral) {
        return Err(alloc::format!(
            "TetGen dihedral bound must be in [0, 90) degrees (got {min_dihedral})"
        ));
    }
    let mut sw = Switches::new().quality(radius_edge, min_dihedral);
    if let Some(v) = max_volume {
        if !(v > 0.0 && v.is_finite()) {
            return Err(alloc::format!("TetGen volume bound must be > 0 (got {v})"));
        }
        sw = sw.max_volume(v);
    }
    let pts: Vec<[f64; 3]> = points.iter().map(|p| [p.x, p.y, p.z]).collect();
    let (markers, names): (Vec<i32>, Vec<String>) = match regions {
        Some(r) => (
            r.region_of_tri.iter().map(|&k| k as i32).collect(),
            r.names.clone(),
        ),
        None => (alloc::vec![0; tris.len()], alloc::vec![wall_patch.into()]),
    };
    let mut plc = PlcInput::from_triangle_soup(&pts, tris, &markers);
    plc.holes = hole_points(points, tris);
    let mut n_seeds = 0;
    if let Some(g) = wall_grade {
        g.check()?;
        let is_wall: Vec<bool> = (0..tris.len())
            .map(|i| match regions {
                Some(r) if !walls.is_empty() => walls.contains(&r.names[r.region_of_tri[i]]),
                _ => true,
            })
            .collect();
        let (seeds, sizes) = wall_grade::wall_graded_sizes(points, tris, &is_wall, &g);
        n_seeds = seeds.len();
        plc.points.extend(seeds.iter().map(|p| [p.x, p.y, p.z]));
        plc.point_mtrs = sizes;
        sw = sw.metric(1.0);
    }
    Ok(TetgenRun {
        plc,
        behavior: sw.behavior(),
        names,
        n_seeds,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::staged::{surface_to_tet_dual_mesh_multipatch_staged, PipelineStage};
    use crate::pipeline::tests::{mesh_fingerprint, pipe_surface};
    use crate::pipeline::{surface_to_tet_dual_mesh, TetDualOptions};
    use crate::shapes::box_surface;

    /// ~~The default is the carved route~~ **CHANGED 2026-11-26 (gh:#1829):**
    /// the default is TetGen `-pq1.414/3.5` (the layers, grown first, need
    /// it), and the switch string is upstream's command line.
    #[test]
    fn the_default_is_tetgen_and_switches_read_as_upstream() {
        assert_eq!(
            TetDualOptions::default().tet_source,
            TetSource::tetgen(1.414, None)
        );
        assert_eq!(TetSource::default(), TetSource::CarvedCentroid);
        assert_eq!(TetSource::CarvedCentroid.switches(), None);
        assert_eq!(
            TetSource::tetgen(1.414, Some(0.0026)).switches().as_deref(),
            Some("pq1.414/3.5a0.0026")
        );
        assert_eq!(
            TetSource::tetgen(2.0, None).switches().as_deref(),
            Some("pq2/3.5")
        );
        // The switch string parses to the same behaviour the builder sets.
        let s = TetSource::tetgen(1.414, Some(0.0026)).switches().unwrap();
        let parsed = Switches::parse(&s).unwrap().behavior();
        let built = Switches::new()
            .quality(1.414, 3.5)
            .max_volume(0.0026)
            .behavior();
        assert_eq!(parsed, built);
    }

    /// Bad bounds and the carved source are errors, not fallbacks.
    #[test]
    fn bad_bounds_are_refused() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        for s in [
            TetSource::CarvedCentroid,
            TetSource::tetgen(1.0, None),
            TetSource::tetgen(1.414, Some(0.0)),
            TetSource::TetGen {
                radius_edge: 1.414,
                min_dihedral: 95.0,
                max_volume: None,
                wall_grade: None,
            },
        ] {
            assert!(tetgen_tets(&p, &t, None, "walls", s).is_err(), "{s:?}");
        }
    }

    /// TetGen's tetrahedra of a unit box: every tetrahedron a cell, one
    /// `walls` patch, volume exactly 1 (the box's faces are planar, so no
    /// facet is approximated), every tet within the volume bound.
    #[test]
    fn tetgen_tets_of_a_box_fill_it_exactly() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let (m, note) =
            tetgen_tets(&p, &t, None, "walls", TetSource::tetgen(1.414, Some(0.01))).unwrap();
        assert!(crate::pipeline::acceptable(&m));
        assert_eq!(m.patches.len(), 1);
        assert_eq!(m.patches[0].name, "walls");
        assert!(
            (m.total_volume() - 1.0).abs() < 1e-12,
            "{}",
            m.total_volume()
        );
        assert!(m.n_cells >= 100, "{} cells", m.n_cells);
        assert!(note.contains("pq1.414/3.5a0.01"), "{note}");
    }

    /// A cavity is a hole, not a region TetGen fills: a 2 m box with an
    /// inward-wound 0.5 m box inside it meshes to 8 - 0.125 m³ exactly,
    /// with both boundaries kept, and the hole point lies in the cavity.
    #[test]
    fn a_cavity_gets_a_hole_point_and_stays_empty() {
        let (mut p, mut t) = box_surface(Vec3::ZERO, Vec3::new(2.0, 2.0, 2.0));
        let (ip, it) = box_surface(Vec3::new(0.75, 0.75, 0.75), Vec3::new(1.25, 1.25, 1.25));
        let off = p.len();
        p.extend(ip);
        t.extend(it.iter().map(|q| [q[0] + off, q[2] + off, q[1] + off]));
        let holes = hole_points(&p, &t);
        assert_eq!(holes.len(), 1, "{holes:?}");
        assert!(
            holes[0].iter().all(|&x| (0.75..1.25).contains(&x)),
            "{holes:?}"
        );
        assert!(
            hole_points(&p[..off], &t[..12]).is_empty(),
            "no cavity, no hole"
        );
        let (m, _) =
            tetgen_tets(&p, &t, None, "walls", TetSource::tetgen(1.414, Some(0.05))).unwrap();
        assert!(crate::pipeline::acceptable(&m));
        assert!(
            (m.total_volume() - (8.0 - 0.125)).abs() < 1e-12,
            "{}",
            m.total_volume()
        );
        let bnd: usize = m.patches.iter().map(|q| q.n_faces).sum();
        let inner = (0..bnd).filter(|&k| {
            let f = m.patches[0].start_face + k;
            m.faces[f].iter().all(|&v| {
                (0.74..1.26).contains(&m.points[v].x)
                    && (0.74..1.26).contains(&m.points[v].y)
                    && (0.74..1.26).contains(&m.points[v].z)
            })
        });
        assert!(inner.count() >= 12, "the cavity's faces are boundary faces");
    }

    fn pipe_opts(source: TetSource, dual: bool, n_layers: usize) -> TetDualOptions {
        TetDualOptions {
            cell_size: 0.006,
            n_layers,
            first_layer_thickness: 3e-4,
            expansion: 1.2,
            layer_patches: alloc::vec!["walls".into()],
            dual,
            dual_kind: crate::dual::DualKind::CellCentre,
            dual_feature_points: crate::dual::FeaturePointRule::Corners,
            tet_source: source,
            ..Default::default()
        }
    }

    /// The pipeline on TetGen's tetrahedra: carve, snap and Delaunay are
    /// listed as skipped, the tet stage is TetGen's mesh, the patches
    /// survive, and every one of the four mesh types is valid.
    ///
    /// **Methodology.** The 1 cm pipe of the crate's other pipeline tests
    /// (`pipe_surface(0.01, 0.06, 48)`, patches `inlet`, `outlet`, `walls`),
    /// TetGen `-pq1.414/3.5a2e-8` (2e-8 m³ is about h³/6 at h = 5 mm),
    /// cell-centre dual with corner feature points, ~~back-off~~ layers (3,
    /// first 0.3 mm, x 1.2) on `walls` only. ~~**Results (2026-11-26,
    /// release):** on the dual, back-off grows all 3 layers on every wall
    /// face; on the tetrahedra it grows **none**; pinned in
    /// `tetgen_route_is_pinned_bit_for_bit`.~~ **CHANGED 2026-11-26
    /// (gh:#1829):** the layers are grown first; with layers the
    /// tetrahedralise stage records TetGen's mesh of the input surface (the
    /// wall triangulation the layers grow from). Every mesh type is valid
    /// with no negative cell, and every wall face carries all 3 layers, on
    /// the prisms and tetrahedra and inside their dual (asserted, not
    /// pinned).
    #[test]
    fn tetgen_route_runs_every_mesh_type_on_the_pipe() {
        let (p, t, regions) = pipe_surface(0.01, 0.06, 48);
        let src = TetSource::tetgen(1.414, Some(2e-8));
        for (dual, n) in [(false, 0), (false, 3), (true, 0), (true, 3)] {
            let s = surface_to_tet_dual_mesh_multipatch_staged(
                &p,
                &t,
                &regions,
                &pipe_opts(src, dual, n),
            )
            .unwrap();
            let what = alloc::format!("dual={dual} n={n}");
            assert_eq!(s.report.n_negative_volume_cells, 0, "{what}");
            let stage = |k: PipelineStage| s.stages.iter().find(|x| x.stage == k);
            assert!(!stage(PipelineStage::Carve).unwrap().applied, "{what}");
            assert_eq!(
                stage(PipelineStage::Carve).unwrap().mesh.n_cells,
                0,
                "{what}"
            );
            assert!(!stage(PipelineStage::Snap).unwrap().applied, "{what}");
            let tets = stage(PipelineStage::Tetrahedralise).unwrap();
            assert!(tets.applied, "{what}");
            let del = stage(PipelineStage::Delaunay).unwrap();
            assert!(!del.applied, "{what}");
            assert_eq!(
                mesh_fingerprint(&del.mesh),
                mesh_fingerprint(&tets.mesh),
                "{what}"
            );
            assert_eq!(
                stage(PipelineStage::Dual).map(|x| x.applied),
                dual.then_some(true),
                "{what}"
            );
            let names: Vec<&str> = s.mesh.patches.iter().map(|q| q.name.as_str()).collect();
            for want in ["inlet", "outlet", "walls"] {
                assert!(names.contains(&want), "{what}: {names:?}");
            }
            assert!(
                s.report.stage_notes[0].contains("tet source TetGen -pq1.414/3.5a0.00000002"),
                "{what}: {:?}",
                s.report.stage_notes
            );
            if n > 0 {
                // gh:#1829: the layers are grown first, on every wall face,
                // as prisms (no dual) or inside the dual.
                let lo = s.report.layers.as_ref().expect("layers grown");
                assert_eq!(lo.full_coverage(), 1.0, "{what}");
                if !dual {
                    let walls: usize = s.mesh.patches.iter().filter(|q| q.name == "walls").map(|q| q.n_faces).sum();
                    let first = s.first_layer_cell.expect("layers grew");
                    assert_eq!(s.mesh.n_cells - first, n * walls, "{what}: every wall face, every layer");
                }
            }
        }
    }

    // ~~`tetgen_route_is_pinned_bit_for_bit` (tets + 3 back-off layers 9874
    // cells, dual + 3 layers 30413)~~ REMOVED 2026-11-26 (gh:#1829): it
    // pinned the layers-last route, which is gone, and the new route gets
    // no regression pin before unit-level V&V (root AGENTS.md).

    /// The single-patch entry point names the one patch after
    /// `wall_patch`, so the default layer stage finds it.
    #[test]
    fn single_patch_route_grows_layers_on_tetgen_tets() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let o = TetDualOptions {
            tet_source: TetSource::tetgen(1.414, Some(0.01)),
            first_layer_thickness: 0.01,
            ..Default::default()
        };
        let (m, rep) = surface_to_tet_dual_mesh(&p, &t, &o).unwrap();
        assert_eq!(rep.n_negative_volume_cells, 0);
        assert!(
            rep.layers.as_ref().is_some_and(|l| l.full_coverage() > 0.0),
            "{:?}",
            rep.stage_notes
        );
        assert!(
            (m.total_volume() - 1.0).abs() < 1e-9,
            "{}",
            m.total_volume()
        );
    }

    /// Why no layer grows on TetGen's tetrahedra (gh:#1796): the boundary
    /// tetrahedra of the two sources, counted. Run with `--ignored
    /// --nocapture`.
    ///
    /// **Methodology.** The 1 cm pipe, tetrahedra only (no dual, no
    /// layers), carved (h 6 mm, `pipe_opts`) and TetGen `-pq1.414a2e-8`.
    /// For each tetrahedron with a face on `walls`, count its faces on
    /// `walls` and whether all four of its vertices are boundary points
    /// (no interior vertex to give its layer column room).
    ///
    /// **Results (2026-11-26, release).** Carved: 2232 tets, 472 with a
    /// wall face, 0 with two, **0** with every vertex on the boundary (each
    /// carved tet has its hexahedron's centroid inside). TetGen: 9874
    /// tets, 3102 with a wall face, 0 with two, **146** with every vertex
    /// on the boundary (a wall face whose fourth vertex lies on the inlet,
    /// the outlet or the wall itself). Such a tetrahedron is flattened by
    /// any wall-normal shift of its wall face, which is consistent with
    /// back-off backing off to zero and `CfMeshOptimise` being refused on
    /// every TetGen tet mesh measured; it is not proven to be the only
    /// cause.
    #[test]
    #[ignore]
    fn diagnose_boundary_tets_of_both_sources() {
        let (p, t, regions) = pipe_surface(0.01, 0.06, 48);
        for src in [
            TetSource::CarvedCentroid,
            TetSource::tetgen(1.414, Some(2e-8)),
        ] {
            let (m, _) = crate::pipeline::surface_to_tet_dual_mesh_multipatch(
                &p,
                &t,
                &regions,
                &pipe_opts(src, false, 0),
            )
            .unwrap();
            let mut on_boundary = alloc::vec![false; m.points.len()];
            let mut wall_faces_of = alloc::vec![0usize; m.n_cells];
            for q in &m.patches {
                for f in q.start_face..q.start_face + q.n_faces {
                    for &v in &m.faces[f] {
                        on_boundary[v] = true;
                    }
                    if q.name == "walls" {
                        wall_faces_of[m.owner[f]] += 1;
                    }
                }
            }
            let cells = m.cell_face_lists();
            let (mut wall_tets, mut two_wall, mut all_bnd) = (0, 0, 0);
            for c in 0..m.n_cells {
                if wall_faces_of[c] == 0 {
                    continue;
                }
                wall_tets += 1;
                if wall_faces_of[c] >= 2 {
                    two_wall += 1;
                }
                let mut vs: Vec<usize> = cells[c]
                    .iter()
                    .flat_map(|&f| m.faces[f].iter().copied())
                    .collect();
                vs.sort_unstable();
                vs.dedup();
                if vs.iter().all(|&v| on_boundary[v]) {
                    all_bnd += 1;
                }
            }
            std::println!("{src:?}: {} tets, {wall_tets} with a wall face, {two_wall} with two or more, {all_bnd} with every vertex on the boundary", m.n_cells);
        }
    }
}
