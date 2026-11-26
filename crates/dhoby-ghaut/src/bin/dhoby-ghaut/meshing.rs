//! Step 7's meshing (gh:#572): the three GeN-Foam meshes of the assembled
//! reactor, their regions, the mesh-to-mesh maps, the polyMesh files and the
//! review images.
//!
//! No meshing algorithm is implemented here. The bodies of revolution are
//! `outram_blender::foam_mesh::revolved_profile_surface`; the volume meshes
//! are the cfMesh port's tet -> dual -> layers pipeline
//! (`outram_park_fork_cfmesh::pipeline`), the structural one stopped at the
//! tetrahedra; the neutral mesh, its plotter and the GeN-Foam
//! `cellVolumeWeight` overlap are `outram_blender::unstructured`. This file
//! decides the domains, names the patches, assigns regions and writes files.
//!
//! **Region assignment is by cell centroid** for Step 7's three meshes: a
//! region on a mesh is the set of cells whose centroid lies in it, so its
//! meshed volume differs from the exact R-Z volume by a stair-step at the
//! cell size. Each mesh's summary reports both (gh:#594 tracks body-fitted
//! region boundaries). **UPDATED 2026-11-26:** the one conjugate mesh
//! ([`build_cht`], `--headless-cht-mesh`) is body-fitted by default
//! ([`ChtBoundaries::Fitted`]: TetGen with the interfaces as constraints,
//! `outram_park_fork_cfmesh::pipeline::multi_region`); Step 7's three
//! meshes still assign by centroid.

use petir::mathf::RealMath;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use dhoby_ghaut::workbench::meshes::{
    genfoam_default_fields, CellType, MeshMapping, MeshPlan, MeshRole, MeshSet, MeshSummary,
    RegionClass, RegionMap, RegionVolume, RzBox, RzDomain, RzTiling,
};
use outram_blender::csg::plot::{
    annotate_slice, default_colours, ImageData, LegendEntry, PlotBasis, SlicePlot,
};
use outram_blender::csg::position::Position;
use outram_blender::foam_mesh::{revolved_profile_surface, TetDualOptions, VolumeMesh};
use outram_blender::unstructured::convert::cfmesh::from_volume_mesh;
use outram_blender::unstructured::overlap::MeshOverlap;
use outram_blender::unstructured::{render_mesh_slice, MeshColourBy, MeshSlice, UnstructuredMesh, Zone};
use outram_park_fork_cfmesh::patches::{assign_patches_by_region, SurfaceRegions};

use crate::engine::AssemblyInfo;

/// The R-Z domain of the assembled HTR-10 model: the bed, conus and cavity
/// from the assembly, the reflector zone boxes from
/// `nee_soon::htr10_rmc::reflector_geometry::FIG_4_10_BOXES` (IAEA-TECDOC-1382
/// Fig. 4.10), converted from TECDOC's downward `z_T` to the model's z.
pub fn domain_from(a: &AssemblyInfo) -> RzDomain {
    use nee_soon::htr10_rmc::core_model::{
        HTR10_CORE_RADIUS_CM, HTR10_DISCHARGE_TUBE_RADIUS_CM, HTR10_REFLECTOR_OUTER_CM,
    };
    use nee_soon::htr10_rmc::reflector_geometry::FIG_4_10_BOXES;
    let z_top = a.z_range[1];
    let boxes = FIG_4_10_BOXES
        .iter()
        .map(|b| RzBox {
            zone: b.zone,
            r: [b.r_in, b.r_out],
            z: [z_top - b.zt_bot, z_top - b.zt_top],
        })
        .collect();
    RzDomain {
        r_outer: HTR10_REFLECTOR_OUTER_CM,
        core_radius: HTR10_CORE_RADIUS_CM,
        tube_radius: HTR10_DISCHARGE_TUBE_RADIUS_CM,
        z_bottom: a.z_range[0],
        z_top,
        conus_floor: a.conus_floor,
        conus_top: -a.bed_half_height,
        bed_top: a.bed_half_height,
        cavity_top: a.cavity_top,
        boxes,
    }
}

/// What the meshing thread reports while it works.
#[derive(Clone, Debug)]
pub enum MeshProgress {
    /// A stage started (`what`, overall fraction done).
    Stage(String, f64),
}

/// The built meshes, in memory, for the UI and Step 8.
pub struct BuiltMeshes {
    /// The hand-off.
    pub set: MeshSet,
    /// The three neutral meshes (metres), by [`MeshRole::index`], each with
    /// one zone per region (empty where the region is absent).
    pub meshes: Vec<Arc<UnstructuredMesh>>,
    /// Review images `(file stem, image)`.
    pub images: Vec<(String, ImageData)>,
}

fn opts(cell_cm: f64, refine: u8, dual: bool) -> TetDualOptions {
    TetDualOptions {
        cell_size: cell_cm / 100.0,
        refinement_levels: refine,
        dual,
        dual_min_faces: dual,
        n_layers: 0,
        // The carved, octree-graded source `refine` drives (gh:#1829 made
        //   TetGen the default; these meshes take no layers).
        tet_source: outram_park_fork_cfmesh::pipeline::TetSource::CarvedCentroid,
        ..TetDualOptions::default()
    }
}

/// [`opts`] on TetGen's tetrahedra (the cfMesh default, sized by the cell
/// size), unless `refine > 0` asks for the octree only the carved route
/// has. The neutronics and thermal-hydraulics meshes (maintainer,
/// 2026-11-26: "TetGen dual"); the structural Tet4 mesh stays carved.
fn opts_tetgen(cell_cm: f64, refine: u8, dual: bool) -> TetDualOptions {
    let o = opts(cell_cm, refine, dual);
    if refine > 0 {
        return o;
    }
    TetDualOptions { tet_source: TetDualOptions::default().tet_source, ..o }
}

/// Step 7's snapping settings: on unless `DHOBY_NO_SNAP=1`; the morph band is
/// one cell (`DHOBY_SNAP_MORPH=0` turns the morph off).
fn step7_snap(cell_cm: f64) -> (bool, f64) {
    let on = std::env::var("DHOBY_NO_SNAP").as_deref() != Ok("1");
    let delta = if std::env::var("DHOBY_SNAP_MORPH").as_deref() == Ok("0") { 0.0 } else { cell_cm / 100.0 };
    (on, delta)
}

fn to_m(profile: &[[f64; 2]]) -> Vec<[f64; 2]> {
    profile
        .iter()
        .map(|p| [p[0] / 100.0, p[1] / 100.0])
        .collect()
}

/// Point-in-polygon in the R-Z half plane (even-odd).
fn inside_profile(profile: &[[f64; 2]], r: f64, z: f64) -> bool {
    let mut inside = false;
    let n = profile.len();
    for i in 0..n {
        let (a, b) = (profile[i], profile[(i + 1) % n]);
        if (a[1] > z) != (b[1] > z) {
            let x = a[0] + (z - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
            if r < x {
                inside = !inside;
            }
        }
    }
    inside
}

/// Exact volume \[cm³\] of each region inside `profile` (quadrature on an
/// `h` cm grid).
fn exact_in(d: &RzDomain, map: &RegionMap, profile: &[[f64; 2]], h: f64) -> Vec<f64> {
    let mut v = vec![0.0; map.regions.len()];
    let nr = (d.r_outer / h).ceil() as usize;
    let nz = ((d.z_top - d.z_bottom) / h).ceil() as usize;
    let dr = d.r_outer / nr as f64;
    let dz = (d.z_top - d.z_bottom) / nz as f64;
    for i in 0..nr {
        let r = (i as f64 + 0.5) * dr;
        for k in 0..nz {
            let z = d.z_bottom + (k as f64 + 0.5) * dz;
            if !inside_profile(profile, r, z) {
                continue;
            }
            if let Some(g) = map.region_at(d, r, z) {
                v[g] += 2.0 * std::f64::consts::PI * r * dr * dz;
            }
        }
    }
    v
}

/// Mesh one role. Returns the cfMesh volume mesh (metres) and its notes.
fn mesh_role(
    d: &RzDomain,
    plan: &MeshPlan,
    role: MeshRole,
) -> Result<(VolumeMesh, Vec<String>, CellType, bool), String> {
    let s = &plan.roles[role.index()];
    match role {
        MeshRole::Neutronics => {
            let prof = to_m(&d.neutronics_profile());
            let (p, t) = revolved_profile_surface(&prof, s.n_seg);
            let (vm, rep) = outram_park_fork_cfmesh::pipeline::surface_to_tet_dual_mesh(
                &p,
                &t,
                &opts_tetgen(s.cell_size_cm, s.refine, true),
            )?;
            Ok((vm, rep.stage_notes, CellType::TetDual, false))
        }
        MeshRole::Structural => {
            let prof = to_m(&d.structural_profile());
            let (p, t) = revolved_profile_surface(&prof, s.n_seg);
            outram_blender::foam_mesh::check_closed_manifold(&p, &t).map_err(|e| e.to_string())?;
            let o = TetDualOptions {
                delaunay: true,
                ..opts(s.cell_size_cm, s.refine, false)
            };
            let (vm, rep) =
                outram_park_fork_cfmesh::pipeline::surface_to_tet_dual_mesh(&p, &t, &o)?;
            Ok((vm, rep.stage_notes, CellType::Tet4, false))
        }
        MeshRole::ThermalHydraulics => {
            let prof = to_m(&d.th_profile());
            let (p, t) = revolved_profile_surface(&prof, s.n_seg);
            // Name the surface: top disc = inlet (the helium enters the
            // cavity from the top plenum and flows down), bottom disc =
            // outlet, side above the bed = cavity_wall (fluid: layers), side
            // along the bed = bed_wall (porous: no layers).
            let (zb, zt, zbed) = (d.conus_top / 100.0, d.cavity_top / 100.0, d.bed_top / 100.0);
            let labels: Vec<&str> = t
                .iter()
                .map(|tri| {
                    let c = (p[tri[0]].z + p[tri[1]].z + p[tri[2]].z) / 3.0;
                    let flat = (p[tri[0]].z - p[tri[1]].z).abs() < 1e-12
                        && (p[tri[1]].z - p[tri[2]].z).abs() < 1e-12;
                    if flat && (c - zt).abs() < 1e-9 {
                        "inlet"
                    } else if flat && (c - zb).abs() < 1e-9 {
                        "outlet"
                    } else if c > zbed {
                        "cavity_wall"
                    } else {
                        "bed_wall"
                    }
                })
                .collect();
            let regions = SurfaceRegions::from_labels(&labels);
            let (mut vm, rep) = outram_park_fork_cfmesh::pipeline::surface_to_tet_dual_mesh_multipatch(
                &p,
                &t,
                &regions,
                &opts_tetgen(s.cell_size_cm, s.refine, true),
            )?;
            let mut notes = rep.stage_notes;
            // Snap the bare dual FIRST (2026-11-26, maintainer: "snap the
            // layered TH mesh properly"), then grow the layers on the true
            // wall, so every layer column is built from the snapped surface
            // rather than moved after it (snapping a layered mesh took its
            // worst face to 92.7 deg). After the layers, only the surface
            // faces are snapped again (no morph): the layerer's untangling
            // may move a wall point off the cylinder.
            let segs = profile_segments_m(&d.th_profile());
            let (snap_on, delta) = step7_snap(s.cell_size_cm);
            if snap_on {
                notes.push(snap_checked(&mut vm, None, &segs, s.n_seg, delta)?);
            }
            // Boundary layers on the fluid's wall only (maintainer default:
            // fluid = tet-dual + layers, porous = tet-dual). The pipeline grows
            // layers before naming patches, so they are grown here, on the
            // named patch, and the patches re-named after (the layerer
            // rebuilds the mesh).
            // ~~`layers::add_boundary_layers_adaptive` (this crate's global
            // back-off)~~ CHANGED 2026-11-26: since gh:#1215's quality check
            // the back-off shrank every column of `cavity_wall` to nothing
            // ("added none"; the 2026-10-05 record had 11 568 layer hexes).
            // cfMesh's own `boundaryLayerOptimisation` thins locally instead
            // (the layerer left callable by gh:#1829 for this workbench).
            // TetGen's layers-first route is not used: it refuses this mesh
            // at the cavity_wall / bed_wall seam (a column thinner than 1 %
            // of the asked thickness).
            if s.layers > 0 {
                let before = vm.n_cells;
                let (layered, lo) = outram_park_fork_cfmesh::boundary_layer_optimisation::add_boundary_layers_cfmesh(
                    &vm,
                    &["cavity_wall"],
                    s.layers,
                    s.first_layer_cm / 100.0,
                    s.expansion.max(1.0),
                    // With cfMesh's untangling passes (the boundary moves,
                    // the LP untangler, the Laplace pass): without them 2 of
                    // the 20 952 cells came out inverted and the layers were
                    // refused (measured 2026-11-26).
                    &outram_park_fork_cfmesh::boundary_layer_optimisation::BoundaryLayerOptimisationControls {
                        untangle_boundary_moves: true,
                        untangle_mesh_untangler: true,
                        optimize_mesh_fv_laplace: true,
                        ..Default::default()
                    },
                );
                notes.push(format!(
                    "boundary layers (cfMesh boundaryLayerOptimisation) on cavity_wall: {} layer cells, {:.1} % of wall faces with all {}",
                    lo.faces.iter().map(|f| f.1).sum::<usize>(),
                    100.0 * lo.full_coverage(),
                    s.layers
                ));
                let mut named = assign_patches_by_region(&layered, &p, &t, &regions)?;
                let q = outram_park_fork_cfmesh::checks::check_quality(&named);
                if named.validate().is_ok() && q.n_negative_volume_cells == 0 {
                    if named.n_cells == before {
                        notes.push("boundary layers added none on cavity_wall (the adaptive layerer backed off)".into());
                    }
                    if snap_on {
                        match snap_checked(&mut named.clone(), None, &segs, s.n_seg, 0.0) {
                            Ok(_) => {
                                let n = snap_checked(&mut named, None, &segs, s.n_seg, 0.0)?;
                                notes.push(format!("after the layers: {n}"));
                            }
                            Err(e) => notes.push(format!("after the layers, the surface re-snap was refused ({e}); the layered mesh is kept as grown on the snapped wall")),
                        }
                    }
                    return Ok((named, notes, CellType::TetDualWithLayers, snap_on));
                }
                notes.push("boundary layers skipped: they would invalidate the mesh".into());
            }
            Ok((vm, notes, CellType::TetDual, snap_on))
        }
    }
}

fn kinds(m: &UnstructuredMesh) -> Vec<(String, usize)> {
    let mut v: Vec<(String, usize)> = Vec::new();
    for c in 0..m.n_cells() {
        let k = format!("{:?}", m.cell_kind(c));
        match v.iter_mut().find(|x| x.0 == k) {
            Some(x) => x.1 += 1,
            None => v.push((k, 1)),
        }
    }
    v
}

/// The OpenFOAM `cellZones` file for `cell_region` (non-empty regions only).
fn cell_zones_text(map: &RegionMap, cell_region: &[usize]) -> String {
    let mut zones: Vec<(String, Vec<usize>)> = Vec::new();
    for (g, r) in map.regions.iter().enumerate() {
        let cells: Vec<usize> = cell_region
            .iter()
            .enumerate()
            .filter(|x| *x.1 == g)
            .map(|x| x.0)
            .collect();
        if !cells.is_empty() {
            zones.push((r.id.clone(), cells));
        }
    }
    let mut s = String::from(
        "FoamFile\n{\n    version 2.0;\n    format ascii;\n    class regIOobject;\n    location \"constant/polyMesh\";\n    object cellZones;\n}\n\n",
    );
    s.push_str(&format!("{}\n(\n", zones.len()));
    for (name, cells) in zones {
        s.push_str(&format!(
            "{name}\n{{\n    type cellZone;\ncellLabels      List<label> {}\n(\n",
            cells.len()
        ));
        for c in cells {
            s.push_str(&format!("{c}\n"));
        }
        s.push_str(")\n;\n}\n\n");
    }
    s.push_str(")\n");
    s
}

/// Region colours: one per region index, the same on every mesh.
pub fn region_colours(n: usize) -> Vec<outram_blender::csg::plot::Rgb> {
    use outram_blender::csg::plot::Rgb;
    // Distinct hand-picked colours (Kelly's set, minus white/black), then
    // OpenMC's random stream for any further user regions.
    const K: [(u8, u8, u8); 18] = [
        (255, 179, 0),
        (128, 62, 117),
        (255, 104, 0),
        (166, 189, 215),
        (193, 0, 32),
        (206, 162, 98),
        (129, 112, 102),
        (0, 125, 52),
        (246, 118, 142),
        (0, 83, 138),
        (255, 122, 92),
        (83, 55, 122),
        (255, 142, 0),
        (179, 40, 81),
        (244, 200, 0),
        (127, 24, 13),
        (147, 170, 0),
        (89, 51, 21),
    ];
    let mut v: Vec<Rgb> = K.iter().take(n).map(|c| Rgb::new(c.0, c.1, c.2)).collect();
    if n > K.len() {
        let mut seed = 7u64;
        v.extend(default_colours(n - K.len(), &mut seed));
    }
    v
}

/// Draw `mesh` (the mesh itself: cells cut by the plane, faces in black,
/// cells coloured by region) on `basis` through `origin` \[cm\].
pub fn draw(
    mesh: &UnstructuredMesh,
    map: &RegionMap,
    basis: PlotBasis,
    origin_cm: [f64; 3],
    title: &str,
    px: usize,
) -> ImageData {
    let mut sl = MeshSlice::framing(mesh, basis, px, MeshColourBy::Zone);
    let n = 3 - basis.axes().0 - basis.axes().1;
    sl.origin[n] = origin_cm[n];
    let (img, _) = render_mesh_slice(mesh, &sl);
    // The plotter's own zone colours are seeded per mesh; recolour so a
    // region has the same colour on every mesh, and list only regions this
    // mesh holds.
    let cols = region_colours(map.regions.len());
    let mut seed = 1u64;
    let plot_cols = default_colours(mesh.zones().len(), &mut seed);
    let mut img = img;
    for p in &mut img.pixels {
        if let Some(z) = plot_cols.iter().position(|c| c == p) {
            *p = cols[z];
        }
    }
    let legend: Vec<LegendEntry> = mesh
        .zones()
        .iter()
        .enumerate()
        .filter(|(_, z)| !z.cells.is_empty())
        .map(|(i, _)| LegendEntry::new(cols[i], map.regions[i].label.clone()))
        .collect();
    let frame = SlicePlot::new(
        basis,
        Position::new(sl.origin[0], sl.origin[1], sl.origin[2]),
        sl.width,
        sl.pixels,
    );
    annotate_slice(&img, &frame, title, &legend)
}

/// Build all three meshes, the six maps, the files and the images.
pub fn build(
    d: &RzDomain,
    plan: &MeshPlan,
    out_dir: &Path,
    progress: &mut impl FnMut(MeshProgress),
) -> Result<BuiltMeshes, String> {
    let map = &plan.regions;
    let mut summaries = Vec::new();
    let mut meshes: Vec<Arc<UnstructuredMesh>> = Vec::new();
    let mut images = Vec::new();
    let profiles = [
        d.neutronics_profile(),
        d.th_profile(),
        d.structural_profile(),
    ];
    for (k, role) in MeshRole::ALL.into_iter().enumerate() {
        progress(MeshProgress::Stage(
            format!("meshing: {}", role.name()),
            k as f64 / 6.0,
        ));
        let t = Instant::now();
        let (mut vm, mut notes, cell_type, already_snapped) = mesh_role(d, plan, role)?;
        // Snap onto the true surface of revolution (2026-11-26, "snap
        // properly"): the profile's cylinders and cone, not its facets.
        let prof = match role {
            MeshRole::Neutronics => d.neutronics_profile(),
            MeshRole::ThermalHydraulics => d.th_profile(),
            MeshRole::Structural => d.structural_profile(),
        };
        let n_seg = plan.roles[role.index()].n_seg;
        // `DHOBY_NO_SNAP=1` is the ablation (Step 7 has no flag of its own).
        let (snap_on, delta) = step7_snap(plan.roles[role.index()].cell_size_cm);
        // The TH mesh snaps itself, before its layers (`mesh_role`).
        let _ = cell_type;
        if already_snapped {
        } else if snap_on {
            notes.push(snap_checked(&mut vm, None, &profile_segments_m(&prof), n_seg, delta).map_err(|e| format!("{}: {e}", role.name()))?);
        } else {
            notes.push("not snapped: the boundary stays on the faceted surface (DHOBY_NO_SNAP=1)".into());
        }
        let um = from_volume_mesh(&vm).map_err(|e| format!("{}: {e}", role.name()))?;
        // Region of every cell, by centroid. A cell of the faceted, snapped
        // mesh whose centroid falls in a region this mesh's domain does not
        // hold (a structural cell poking into the cavity, say) takes the
        // nearest region it may hold, searched on rings of growing radius in
        // the R-Z plane; their number is reported.
        let allowed = |g: usize| match role {
            MeshRole::Neutronics => true,
            MeshRole::ThermalHydraulics => map.regions[g].in_core,
            MeshRole::Structural => !map.regions[g].in_core,
        };
        let mut cell_region = Vec::with_capacity(um.n_cells());
        let (mut moved, mut unassigned) = (0, 0);
        for c in 0..um.n_cells() {
            let x = um.cell_centre(c);
            let (r, z) = ((x[0] * x[0] + x[1] * x[1]).sqrt() * 100.0, x[2] * 100.0);
            let here = map.region_at(d, r, z).filter(|&g| allowed(g));
            let g = here.or_else(|| {
                (1..=200).find_map(|k| {
                    let rho = 0.5 * k as f64;
                    (0..16).find_map(|j| {
                        let a = std::f64::consts::TAU * j as f64 / 16.0;
                        let (rr, zz) = (r + rho * a.r_cos(), z + rho * a.r_sin());
                        if rr < 0.0 {
                            return None;
                        }
                        map.region_at(d, rr, zz).filter(|&g| allowed(g))
                    })
                })
            });
            match g {
                Some(g) => {
                    if here.is_none() {
                        moved += 1;
                    }
                    cell_region.push(g);
                }
                None => {
                    unassigned += 1;
                    cell_region.push(map.box_region.first().copied().unwrap_or(0));
                }
            }
        }
        if moved > 0 {
            notes.push(format!(
                "{moved} cells had a centroid in a region outside this mesh's domain (faceting and snapping) and took the nearest region inside it"
            ));
        }
        if unassigned > 0 {
            notes.push(format!("{unassigned} cells could not be given a region and were put in the first box's region"));
        }
        let zones: Vec<Zone> = map
            .regions
            .iter()
            .enumerate()
            .map(|(g, r)| Zone {
                name: r.id.clone(),
                cells: cell_region
                    .iter()
                    .enumerate()
                    .filter(|x| *x.1 == g)
                    .map(|x| x.0)
                    .collect(),
            })
            .collect();
        let um = um.with_zones(zones).map_err(|e| e.to_string())?;
        // Volumes.
        let vols = outram_blender::unstructured::overlap::cell_volumes_cm3(&um);
        let exact = exact_in(d, map, &profiles[k], 0.5);
        let regions: Vec<RegionVolume> = (0..map.regions.len())
            .filter_map(|g| {
                let cells: Vec<usize> =
                    (0..um.n_cells()).filter(|&c| cell_region[c] == g).collect();
                (exact[g] > 0.0 || !cells.is_empty()).then(|| RegionVolume {
                    region: g,
                    cells: cells.len(),
                    mesh_cm3: cells.iter().map(|&c| vols[c]).sum(),
                    exact_cm3: exact[g],
                })
            })
            .collect();
        let q = outram_park_fork_cfmesh::checks::check_quality(&vm);
        // Files.
        // GeN-Foam case layout: constant/<region>/polyMesh.
        let dir = out_dir
            .join("constant")
            .join(role.genfoam_region())
            .join("polyMesh");
        outram_park_fork_cfmesh::foam::write_polymesh(&vm, &dir)
            .map_err(|e| format!("{}: {e}", dir.display()))?;
        std::fs::write(dir.join("cellZones"), cell_zones_text(map, &cell_region))
            .map_err(|e| e.to_string())?;
        let um = Arc::new(um);
        // Images: R-Z through the axis, and radial at the bed mid-height.
        progress(MeshProgress::Stage(
            format!("drawing: {}", role.name()),
            (k as f64 + 0.5) / 6.0,
        ));
        let zmid = 0.5 * (d.conus_top + d.bed_top);
        let title = |w: &str| {
            format!(
                "HTR-10 {} MESH, {} CELLS: {w}",
                role.name().to_uppercase(),
                um.n_cells()
            )
        };
        images.push((
            format!("mesh_{}_xz", role.slug()),
            draw(
                &um,
                map,
                PlotBasis::Xz,
                [0.0, 0.0, 0.0],
                &title("X-Z THROUGH THE AXIS"),
                900,
            ),
        ));
        images.push((
            format!("mesh_{}_xy", role.slug()),
            draw(
                &um,
                map,
                PlotBasis::Xy,
                [0.0, 0.0, zmid],
                &title(&format!("X-Y AT Z = {zmid:.1} CM (BED MID-HEIGHT)")),
                900,
            ),
        ));
        summaries.push(MeshSummary {
            role,
            cell_type,
            cells: um.n_cells(),
            points: um.points().len(),
            faces: um.n_faces(),
            kinds: kinds(&um),
            volume_cm3: vols.iter().sum(),
            exact_cm3: exact.iter().sum(),
            non_star_cells: um.n_non_star_cells(),
            max_non_orthogonality_deg: q.max_non_orthogonality_deg,
            max_skewness: q.max_skewness,
            patches: vm
                .patches
                .iter()
                .map(|p| (p.name.clone(), p.n_faces))
                .collect(),
            notes,
            regions,
            cell_region,
            polymesh_dir: Some(dir.display().to_string()),
            seconds: t.elapsed().as_secs_f64(),
        });
        meshes.push(um);
    }
    // Mesh-to-mesh maps, every pair, both directions from one overlap pass.
    let mut mappings = Vec::new();
    let pairs = [
        (MeshRole::Neutronics, MeshRole::ThermalHydraulics),
        (MeshRole::Neutronics, MeshRole::Structural),
        (MeshRole::ThermalHydraulics, MeshRole::Structural),
    ];
    for (k, (a, b)) in pairs.into_iter().enumerate() {
        progress(MeshProgress::Stage(
            format!("mapping: {} <-> {}", a.name(), b.name()),
            (3.0 + k as f64) / 6.0,
        ));
        let t = Instant::now();
        let o = MeshOverlap::new(&meshes[a.index()], &meshes[b.index()]);
        let secs = t.elapsed().as_secs_f64();
        let uncovered = |w: &Vec<Vec<(usize, f64)>>| w.iter().filter(|r| r.is_empty()).count();
        let ab = o.weights_onto_target();
        let ba = o.weights_onto_source();
        mappings.push(MeshMapping {
            from: a,
            to: b,
            uncovered_cells: uncovered(&ab),
            weights: ab,
            overlap_cm3: o.overlap_volume(),
            to_coverage: o.target_coverage(),
            fields: genfoam_default_fields(a, b),
            seconds: secs,
        });
        mappings.push(MeshMapping {
            from: b,
            to: a,
            uncovered_cells: uncovered(&ba),
            weights: ba,
            overlap_cm3: o.overlap_volume(),
            to_coverage: o.source_coverage(),
            fields: genfoam_default_fields(b, a),
            seconds: secs,
        });
    }
    progress(MeshProgress::Stage("done".into(), 1.0));
    Ok(BuiltMeshes {
        set: MeshSet {
            domain: d.clone(),
            plan: plan.clone(),
            meshes: summaries,
            mappings,
        },
        meshes,
        images,
    })
}

/// One conjugate mesh of the whole HTR-10 (maintainer request, 2026-11-26):
/// solid graphite, fluid upper and lower plenums and the porous pebble bed
/// in ONE polyMesh, the regions as `cellZones`
/// ([`RegionMap::three_class_cht`]).
pub struct ChtMesh {
    /// The mesh (metres), one zone per region.
    pub mesh: Arc<UnstructuredMesh>,
    /// The region map it was assigned with.
    pub map: RegionMap,
    /// Region index of every cell.
    pub cell_region: Vec<usize>,
    /// Per region: `(id, class label, cells, meshed cm³, exact cm³)`.
    pub regions: Vec<(String, String, usize, f64, f64)>,
    /// Per region, the exact volume of the faceted body the mesh's surface
    /// is (`n_seg` facets around) \[cm³\]: what a body-fitted mesh must
    /// reproduce to round-off.
    pub faceted_cm3: Vec<f64>,
    /// How the region boundaries were made.
    pub boundaries: ChtBoundaries,
    /// Per internal interface `(name, faces)` (fitted only).
    pub interfaces: Vec<(String, usize)>,
    /// Boundary patches `(name, faces)`.
    pub patches: Vec<(String, usize)>,
    /// Cell kinds `(kind, count)`.
    pub kinds: Vec<(String, usize)>,
    /// cfMesh quality: max non-orthogonality \[deg\], max skewness.
    pub quality: (f64, f64),
    /// The pipeline's notes, plus region-assignment notes.
    pub notes: Vec<String>,
    /// The `constant/polyMesh` directory written.
    pub polymesh_dir: PathBuf,
    /// Review images `(file stem, image)`.
    pub images: Vec<(String, ImageData)>,
    /// Wall time \[s\].
    pub seconds: f64,
}

/// How [`build_cht`] makes the boundaries between regions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChtBoundaries {
    /// **Body-fitted** (default): every region interface is a constraint
    /// surface for TetGen, each region is dualled on its own and the duals
    /// are stitched (`outram_park_fork_cfmesh::pipeline::multi_region`).
    /// No cell crosses an interface. Needs the TetGen tet source.
    Fitted,
    /// One closed cylinder meshed as one region, each cell given the region
    /// of its centroid: interfaces stair-stepped at the cell size (Step 7's
    /// rule, gh:#594).
    Centroid,
}

/// The whole cylinder as a multi-region surface for
/// `outram_park_fork_cfmesh::pipeline::multi_region` (metres): every edge
/// of `t` between two regions, and the outer top, bottom and side, revolved
/// with `n_seg` segments as [`revolved_profile_surface`] revolves a profile
/// (a vertex on the axis is one point). Collinear grid edges with the same
/// two regions on either side, meeting at a vertex no other kept edge
/// uses, are merged first, so a grid line that is a boundary elsewhere
/// adds no facet edge here. Interface patches are named
/// `<region a>_to_<region b>`. One seed per connected component of every
/// region, at the centre of its first rectangle, at angle 0 (where the
/// faceted rings pass through the true radii).
pub fn fitted_cht_surface(map: &RegionMap, t: &RzTiling, n_seg: usize) -> outram_park_fork_cfmesh::pipeline::multi_region::MultiRegionSurface {
    use outram_park_fork_cfmesh::math::Vec3 as V;
    let (nr, nz) = (t.nr(), t.nz());
    let n_reg = map.regions.len();
    // Region on either side of a grid edge; `n_reg` = outside.
    let cell = |i: isize, j: isize| -> usize {
        if i < 0 || j < 0 || i >= nr as isize || j >= nz as isize {
            n_reg
        } else {
            t.at(i as usize, j as usize)
        }
    };
    let mut patch_names = vec!["top".to_string(), "bottom".to_string(), "side".to_string()];
    let mut interface = vec![false, false, false];
    let mut patch_of = |a: usize, b: usize, outer: usize| -> usize {
        if a == n_reg || b == n_reg {
            return outer;
        }
        let (a, b) = (a.min(b), a.max(b));
        let name = format!("{}_to_{}", map.regions[a].id, map.regions[b].id);
        match patch_names.iter().position(|n| *n == name) {
            Some(k) => k,
            None => {
                patch_names.push(name);
                interface.push(true);
                patch_names.len() - 1
            }
        }
    };
    // Kept edges, per grid line: vertical at r[i] (i >= 1), cells (i-1, j)
    // and (i, j); horizontal at z[j], cells (i, j-1) and (i, j).
    // vert[i][j] / horiz[j][i] = Some(patch).
    let mut vert = vec![vec![None; nz]; nr + 1];
    let mut horiz = vec![vec![None; nr]; nz + 1];
    for i in 1..=nr {
        for j in 0..nz {
            let (a, b) = (cell(i as isize - 1, j as isize), cell(i as isize, j as isize));
            if a != b {
                vert[i][j] = Some(patch_of(a, b, 2));
            }
        }
    }
    for j in 0..=nz {
        for i in 0..nr {
            let (a, b) = (cell(i as isize, j as isize - 1), cell(i as isize, j as isize));
            if a != b {
                horiz[j][i] = Some(patch_of(a, b, if j == 0 { 1 } else { 0 }));
            }
        }
    }
    // Kept-edge degree of every grid node (i, j).
    let mut deg = vec![vec![0usize; nz + 1]; nr + 1];
    for i in 0..=nr {
        for j in 0..nz {
            if vert[i][j].is_some() {
                deg[i][j] += 1;
                deg[i][j + 1] += 1;
            }
        }
    }
    for j in 0..=nz {
        for i in 0..nr {
            if horiz[j][i].is_some() {
                deg[i][j] += 1;
                deg[i + 1][j] += 1;
            }
        }
    }
    // Merged runs as ((i0, j0), (i1, j1), patch).
    let mut runs: Vec<([usize; 2], [usize; 2], usize)> = Vec::new();
    for i in 1..=nr {
        let mut j = 0;
        while j < nz {
            let Some(p) = vert[i][j] else {
                j += 1;
                continue;
            };
            let j0 = j;
            j += 1;
            while j < nz && vert[i][j] == Some(p) && deg[i][j] == 2 {
                j += 1;
            }
            runs.push(([i, j0], [i, j], p));
        }
    }
    for j in 0..=nz {
        let mut i = 0;
        while i < nr {
            let Some(p) = horiz[j][i] else {
                i += 1;
                continue;
            };
            let i0 = i;
            i += 1;
            while i < nr && horiz[j][i] == Some(p) && deg[i][j] == 2 {
                i += 1;
            }
            runs.push(([i0, j], [i, j], p));
        }
    }
    // Points: one ring per used grid node (one point on the axis).
    let mut points: Vec<V> = Vec::new();
    let mut ring: std::collections::BTreeMap<[usize; 2], Vec<usize>> = std::collections::BTreeMap::new();
    let mut ring_of = |n: [usize; 2], points: &mut Vec<V>| -> Vec<usize> {
        ring.entry(n)
            .or_insert_with(|| {
                let (r, z) = (t.r[n[0]] / 100.0, t.z[n[1]] / 100.0);
                if n[0] == 0 {
                    points.push(V::new(0.0, 0.0, z));
                    vec![points.len() - 1; n_seg]
                } else {
                    (0..n_seg)
                        .map(|k| {
                            let phi = 2.0 * core::f64::consts::PI * k as f64 / n_seg as f64;
                            points.push(V::new(r * phi.r_cos(), r * phi.r_sin(), z));
                            points.len() - 1
                        })
                        .collect()
                }
            })
            .clone()
    };
    let (mut tris, mut marker) = (Vec::new(), Vec::new());
    for (a, b, p) in runs {
        let (ra, rb) = (ring_of(a, &mut points), ring_of(b, &mut points));
        for k in 0..n_seg {
            let l = (k + 1) % n_seg;
            for tr in [[ra[k], ra[l], rb[l]], [ra[k], rb[l], rb[k]]] {
                if tr[0] != tr[1] && tr[1] != tr[2] && tr[0] != tr[2] {
                    tris.push(tr);
                    marker.push(p);
                }
            }
        }
    }
    // Seeds: connected components of equal-region rectangles.
    let mut comp = vec![usize::MAX; nr * nz];
    let mut seeds = Vec::new();
    for start in 0..nr * nz {
        if comp[start] != usize::MAX {
            continue;
        }
        let g = t.region[start];
        let id = seeds.len();
        let (i0, j0) = (start % nr, start / nr);
        seeds.push((V::new(0.5 * (t.r[i0] + t.r[i0 + 1]) / 100.0, 0.0, 0.5 * (t.z[j0] + t.z[j0 + 1]) / 100.0), g));
        let mut stack = vec![start];
        comp[start] = id;
        while let Some(c) = stack.pop() {
            let (i, j) = ((c % nr) as isize, (c / nr) as isize);
            for (di, dj) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let (a, b) = (i + di, j + dj);
                if a >= 0 && b >= 0 && (a as usize) < nr && (b as usize) < nz {
                    let n = b as usize * nr + a as usize;
                    if comp[n] == usize::MAX && t.region[n] == g {
                        comp[n] = id;
                        stack.push(n);
                    }
                }
            }
        }
    }
    outram_park_fork_cfmesh::pipeline::multi_region::MultiRegionSurface {
        points,
        tris,
        marker,
        patch_names,
        interface,
        region_names: map.regions.iter().map(|r| r.id.clone()).collect(),
        seeds,
    }
}

/// **Snap a mesh's surface faces onto the true surface of revolution**, by
/// face rather than by point (2026-11-26). The surface faces are the
/// boundary faces and, given `cell_region`, the internal faces between two
/// regions. A surface face whose unit normal is mostly radial (`|n_z| <
/// 0.5`) lies on the cylinder or cone `segs[i]` whose true radius at the
/// face centre's height `R(z_c)` satisfies `R cos(pi/n) - 0.005 R <= r_c <= R
/// + 0.005 R` with `z_c` in its range: every point of that face moves
/// radially, at its own height, to `R(z)`. Faces of the discs (`|n_z| >=
/// 0.5`) do not move their points; a rim point shared with a cylinder face
/// moves radially and so stays in the disc's plane. It classifies faces,
/// not points, so a point need not lie on a facet plane (after
/// [`morph_to_revolved`] most do not). Returns the number of points moved.
///
/// Original OUTRAM PARK code (dhoby-ghaut), not a port.
pub fn snap_faces_to_revolved(mesh: &mut VolumeMesh, cell_region: Option<&[usize]>, segs: &[[[f64; 2]; 2]], n_seg: usize) -> usize {
    use std::f64::consts::PI;
    let cosn = (PI / n_seg as f64).r_cos();
    let segs: Vec<[[f64; 2]; 2]> = segs.iter().copied().filter(|s| (s[1][1] - s[0][1]).abs() > 1e-9).collect();
    let r_of = |s: &[[f64; 2]; 2], z: f64| s[0][0] + (z - s[0][1]) / (s[1][1] - s[0][1]) * (s[1][0] - s[0][0]);
    // Segment index of every point, from the faces it is on.
    let mut seg_of: Vec<Option<usize>> = vec![None; mesh.points.len()];
    for f in 0..mesh.face_count() {
        let surface = match (mesh.neighbour[f], cell_region) {
            (None, _) => true,
            (Some(nb), Some(reg)) => reg[nb] != reg[mesh.owner[f]],
            (Some(_), None) => false,
        };
        if !surface {
            continue;
        }
        let a = mesh.face_area_vector(f);
        let al = a.length();
        if al <= 0.0 || (a.z / al).abs() >= 0.5 {
            continue;
        }
        let c = mesh.face_centroid(f);
        let rc = (c.x * c.x + c.y * c.y).sqrt();
        let hit = segs.iter().position(|sg| {
            let (zl, zh) = (sg[0][1].min(sg[1][1]), sg[0][1].max(sg[1][1]));
            if c.z < zl - 1e-9 || c.z > zh + 1e-9 {
                return false;
            }
            // Margin 0.5 % of R either side: a face the layerer's untangler
            // pushed off the cylinder (0.8 mm out on the 90 cm TH wall,
            // 2026-11-26) is still recognised; neighbouring cylinders of these
            // profiles are at least 5 % apart.
            let rt = r_of(sg, c.z);
            let m = 5e-3 * rt;
            rc >= rt * cosn - m && rc <= rt + m
        });
        if let Some(i) = hit {
            for &v in &mesh.faces[f] {
                seg_of[v] = Some(i);
            }
        }
    }
    let mut moved = 0;
    for (p, sg) in mesh.points.iter_mut().zip(&seg_of) {
        let Some(i) = sg else { continue };
        let r = (p.x * p.x + p.y * p.y).sqrt();
        if r < 1e-12 {
            continue;
        }
        let rt = r_of(&segs[*i], p.z);
        if rt != r {
            p.x *= rt / r;
            p.y *= rt / r;
            moved += 1;
        }
    }
    moved
}

/// **Morph the band near each true cylinder** before
/// [`snap_faces_to_revolved`] (2026-11-26). A point at radius `r`, angle
/// `theta`, height `z` within `delta` (m) of the faceted cylinder of a
/// vertical segment `(R, z0, z1)` moves radially by `w R (1 - c)`, where `R c`
/// is the facet's radius at `theta` (so a point on the facet gets the full
/// chord-to-circle move) and `w = (1 - |r - R c| / delta)` times `(1 - d_z /
/// delta)` for a height `d_z` beyond the segment's ends, clamped to `[0, 1]`;
/// the cylinder with the largest `w` wins. Cells next to the wall then
/// follow the snapped wall instead of being squashed by it (moving only the
/// surface points took one corner cell of the HTR-10 CHT mesh from 83.6 to
/// 88.7 deg and skewness 6.4 to 32.9). Slanted segments are not morphed. Returns
/// the number of points moved.
pub fn morph_to_revolved(mesh: &mut VolumeMesh, segs: &[[[f64; 2]; 2]], n_seg: usize, delta: f64) -> usize {
    use std::f64::consts::PI;
    let w_ang = 2.0 * PI / n_seg as f64;
    let cosn = (PI / n_seg as f64).r_cos();
    let cyl: Vec<(f64, f64, f64)> = segs
        .iter()
        .filter(|s| (s[1][1] - s[0][1]).abs() > 1e-9 && (s[1][0] - s[0][0]).abs() < 1e-9)
        .map(|s| (s[0][0], s[0][1].min(s[1][1]), s[0][1].max(s[1][1])))
        .collect();
    let mut moved = 0;
    for p in mesh.points.iter_mut() {
        let r = (p.x * p.x + p.y * p.y).sqrt();
        if r < 1e-12 {
            continue;
        }
        let th = p.y.r_atan2(p.x).rem_euclid(2.0 * PI);
        let mid = ((th / w_ang).floor() + 0.5) * w_ang;
        let c = cosn / (th - mid).r_cos();
        let mut best = (0.0f64, 0.0f64);
        for &(rr, z0, z1) in &cyl {
            let wr = 1.0 - (r - rr * c).abs() / delta;
            if wr <= 0.0 {
                continue;
            }
            let dz = if p.z < z0 { z0 - p.z } else if p.z > z1 { p.z - z1 } else { 0.0 };
            let w = wr * (1.0 - dz / delta).max(0.0);
            if w > best.0 {
                best = (w, rr * (1.0 - c));
            }
        }
        if best.0 > 0.0 && best.1 != 0.0 {
            let f = (r + best.0 * best.1) / r;
            p.x *= f;
            p.y *= f;
            moved += 1;
        }
    }
    moved
}

/// The non-horizontal edges of a closed R-Z profile \[cm\], in metres, for
/// [`snap_faces_to_revolved`].
fn profile_segments_m(profile: &[[f64; 2]]) -> Vec<[[f64; 2]; 2]> {
    let m = to_m(profile);
    (0..m.len()).map(|i| [m[i], m[(i + 1) % m.len()]]).collect()
}

/// Every vertical edge of `t` with different regions (or the outside) on
/// its two sides \[m\]: the cylinders of the fitted CHT mesh, for
/// [`snap_faces_to_revolved`].
fn tiling_cylinders_m(t: &RzTiling) -> Vec<[[f64; 2]; 2]> {
    let (nr, nz) = (t.nr(), t.nz());
    let mut v = Vec::new();
    for i in 1..=nr {
        for j in 0..nz {
            let inside = t.at(i - 1, j);
            if i == nr || t.at(i, j) != inside {
                v.push([[t.r[i] / 100.0, t.z[j] / 100.0], [t.r[i] / 100.0, t.z[j + 1] / 100.0]]);
            }
        }
    }
    v
}

/// Morph and snap `vm` ([`morph_to_revolved`], [`snap_faces_to_revolved`]) and refuse a result with a
/// non-positive cell; a one-line note for the report.
fn snap_checked(vm: &mut VolumeMesh, cell_region: Option<&[usize]>, segs: &[[[f64; 2]; 2]], n_seg: usize, delta: f64) -> Result<String, String> {
    // The morph's band is measured from the facets, so it runs first; the
    // face snap then puts every surface point exactly on the true surface.
    let nm = if delta > 0.0 { morph_to_revolved(vm, segs, n_seg, delta) } else { 0 };
    let n = snap_faces_to_revolved(vm, cell_region, segs, n_seg);
    let v = outram_park_fork_cfmesh::pipeline::multi_region::cell_volumes(vm);
    let bad = v.iter().filter(|&&x| !(x > 0.0)).count();
    if bad > 0 {
        return Err(format!("snapping to the true surface left {bad} cells with non-positive volume"));
    }
    vm.validate().map_err(|e| format!("snapped mesh: {e}"))?;
    Ok(format!("snapped {n} surface points radially onto the true surface of revolution ({n_seg}-facet input), after morphing {nm} points within {:.1} cm of a cylinder", delta * 100.0))
}

/// [`ChtBoundaries::Centroid`]'s mesh: the whole cylinder as one closed
/// surface, `top` / `bottom` / `side` patches, cfMesh's tet -> dual
/// pipeline; the regions are assigned afterwards by centroid.
fn cht_centroid_mesh(
    d: &RzDomain,
    cell_cm: f64,
    n_seg: usize,
    tets: outram_park_fork_cfmesh::pipeline::TetSource,
) -> Result<(VolumeMesh, Vec<String>), String> {
    let prof = to_m(&d.neutronics_profile());
    let (p, tri) = revolved_profile_surface(&prof, n_seg);
    let (zb, zt) = (d.z_bottom / 100.0, d.z_top / 100.0);
    let labels: Vec<&str> = tri
        .iter()
        .map(|f| {
            let c = (p[f[0]].z + p[f[1]].z + p[f[2]].z) / 3.0;
            let flat = (p[f[0]].z - p[f[1]].z).abs() < 1e-12 && (p[f[1]].z - p[f[2]].z).abs() < 1e-12;
            if flat && (c - zt).abs() < 1e-9 {
                "top"
            } else if flat && (c - zb).abs() < 1e-9 {
                "bottom"
            } else {
                "side"
            }
        })
        .collect();
    let surf = SurfaceRegions::from_labels(&labels);
    let (vm, rep) = outram_park_fork_cfmesh::pipeline::surface_to_tet_dual_mesh_multipatch(
        &p,
        &tri,
        &surf,
        &TetDualOptions {
            tet_source: tets,
            ..opts(cell_cm, 0, true)
        },
    )?;
    Ok((vm, rep.stage_notes))
}

/// Where the worst cells of a fitted mesh are: the per-cell maxima of
/// cfMesh's quality checks (`checks::cell_quality`, the same numbers as
/// `check_quality`) over the cells with a face on a region interface,
/// against the rest.
fn interface_quality_note(vm: &VolumeMesh, cell_region: &[usize]) -> String {
    let q = outram_park_fork_cfmesh::checks::cell_quality(vm);
    let mut at = vec![false; vm.n_cells];
    for f in 0..vm.face_count() {
        if let Some(nb) = vm.neighbour[f] {
            if cell_region[nb] != cell_region[vm.owner[f]] {
                at[nb] = true;
                at[vm.owner[f]] = true;
            }
        }
    }
    let max = |sel: bool, v: &[f64]| v.iter().zip(&at).filter(|(_, &a)| a == sel).map(|(x, _)| *x).fold(0.0, f64::max);
    let n_at = at.iter().filter(|&&a| a).count();
    // Where the worst cells are (r, z of their vertex average, cm).
    let at_cm = |c: usize| {
        let mut sum = outram_park_fork_cfmesh::math::Vec3::ZERO;
        let mut n = 0.0f64;
        for f in 0..vm.face_count() {
            if vm.owner[f] == c || vm.neighbour[f] == Some(c) {
                for &v in &vm.faces[f] {
                    sum = sum.add(vm.points[v]);
                    n += 1.0;
                }
            }
        }
        let p = sum.scale(1.0 / n.max(1.0));
        ((p.x * p.x + p.y * p.y).sqrt() * 100.0, p.z * 100.0)
    };
    let argmax = |v: &[f64]| (0..v.len()).max_by(|&a, &b| v[a].total_cmp(&v[b])).unwrap_or(0);
    let (cn, cs) = (argmax(&q.non_orthogonality_deg), argmax(&q.skewness));
    let (pn, ps) = (at_cm(cn), at_cm(cs));
    format!(
        "quality by place: {n_at} cells on a region interface, max non-orthogonality {:.1} deg, max skewness {:.2}; the other {} cells {:.1} deg, {:.2}. Worst non-orthogonality at r {:.1}, z {:.1} cm; worst skewness at r {:.1}, z {:.1} cm",
        max(true, &q.non_orthogonality_deg),
        max(true, &q.skewness),
        vm.n_cells - n_at,
        max(false, &q.non_orthogonality_deg),
        max(false, &q.skewness),
        pn.0, pn.1, ps.0, ps.1
    )
}

/// Build [`ChtMesh`]: the whole R-Z domain (`r <= r_outer`, bottom to top)
/// revolved with `n_seg` segments, meshed tet -> dual at `cell_cm` with
/// tetrahedra from `tets` ([`cht_tetgen`] for the TetGen port, as Mesh
/// Studio's default; `TetSource::CarvedCentroid` for cfMesh's carved
/// route, centroid boundaries only), the outer surface named `top`,
/// `bottom` and `side`, the region boundaries made as `boundaries` says,
/// snapped onto the true cylinders when `snap` (the default; see
/// [`snap_faces_to_revolved`]), written to `out_dir/constant/polyMesh`
/// with `cellZones`, and drawn: R-Z
/// through the axis, and X-Y through the middle of each of the three
/// non-solid regions.
pub fn build_cht(
    d: &RzDomain,
    cell_cm: f64,
    n_seg: usize,
    tets: outram_park_fork_cfmesh::pipeline::TetSource,
    boundaries: ChtBoundaries,
    snap: bool,
    out_dir: &Path,
) -> Result<ChtMesh, String> {
    let t = Instant::now();
    let map = RegionMap::three_class_cht(d);
    let tiling = map.rz_tiling(d)?;
    let faceted_cm3 = tiling.volumes(map.regions.len(), Some(n_seg));
    let (vm, fitted_region, mut notes, interfaces) = match boundaries {
        ChtBoundaries::Fitted => {
            // Snapping TetGen's own surface vertices before the dual was
            // measured and rejected (2026-11-26): it took the cells away
            // from the interfaces from 63.8 to 87.4 deg and skewness 1.3 to
            // 14.9; the dual is snapped after, with a morph (below).
            let surf = fitted_cht_surface(&map, &tiling, n_seg);
            let m = outram_park_fork_cfmesh::pipeline::multi_region::multi_region_mesh(&surf, tets, true)?;
            (m.mesh, Some(m.cell_region), m.notes, m.interface_faces)
        }
        ChtBoundaries::Centroid => {
            let (vm, notes) = cht_centroid_mesh(d, cell_cm, n_seg, tets)?;
            (vm, None, notes, Vec::new())
        }
    };
    notes.push(format!("tetrahedra: {tets:?}; region boundaries: {boundaries:?}"));
    let mut vm = vm;
    if snap {
        let segs = match boundaries {
            ChtBoundaries::Fitted => tiling_cylinders_m(&tiling),
            ChtBoundaries::Centroid => profile_segments_m(&d.neutronics_profile()),
        };
        let delta = if std::env::var("DHOBY_SNAP_MORPH").as_deref() == Ok("0") { 0.0 } else { cell_cm / 100.0 };
        notes.push(snap_checked(&mut vm, fitted_region.as_deref(), &segs, n_seg, delta)?);
    } else {
        notes.push("not snapped: the boundary stays on the faceted surface (--no-snap)".into());
    }
    if let Some(reg) = &fitted_region {
        notes.push(interface_quality_note(&vm, reg));
    }
    let um = from_volume_mesh(&vm).map_err(|e| e.to_string())?;
    let mut cell_region = Vec::with_capacity(um.n_cells());
    let mut unassigned = 0;
    if let Some(r) = fitted_region {
        cell_region = r;
    }
    for c in 0..um.n_cells() {
        if boundaries == ChtBoundaries::Fitted {
            break;
        }
        let x = um.cell_centre(c);
        let (r, z) = ((x[0] * x[0] + x[1] * x[1]).sqrt() * 100.0, x[2] * 100.0);
        // The domain is the whole cylinder, so every centroid inside it has a
        // part; one that the faceted surface puts outside takes solid.
        match map.region_at(d, r.min(d.r_outer - 1e-9), z.clamp(d.z_bottom + 1e-9, d.z_top - 1e-9)) {
            Some(g) => cell_region.push(g),
            None => {
                unassigned += 1;
                cell_region.push(3);
            }
        }
    }
    if unassigned > 0 {
        notes.push(format!("{unassigned} cells had no R-Z part at their centroid and were put in solid_graphite"));
    }
    let zones: Vec<Zone> = map
        .regions
        .iter()
        .enumerate()
        .map(|(g, r)| Zone {
            name: r.id.clone(),
            cells: (0..um.n_cells()).filter(|&c| cell_region[c] == g).collect(),
        })
        .collect();
    let um = Arc::new(um.with_zones(zones).map_err(|e| e.to_string())?);
    notes.push(format!(
        "{} of {} cells are not star-shaped about their centroid (a consumer that splits cells about the centroid must check `decomposition_is_valid`)",
        um.n_non_star_cells(),
        um.n_cells()
    ));
    let vols = outram_blender::unstructured::overlap::cell_volumes_cm3(&um);
    // The tiling's volumes are analytic (each rectangle revolved exactly);
    // `exact_in`'s 0.5 cm quadrature was off by up to 0.4 % where a region
    // edge is not on its grid (the bed's top and bottom).
    let exact = tiling.volumes(map.regions.len(), None);
    let regions = map
        .regions
        .iter()
        .enumerate()
        .map(|(g, r)| {
            let cells: Vec<usize> = (0..um.n_cells()).filter(|&c| cell_region[c] == g).collect();
            (
                r.id.clone(),
                r.class.label().to_string(),
                cells.len(),
                cells.iter().map(|&c| vols[c]).sum::<f64>(),
                exact[g],
            )
        })
        .collect();
    let q = outram_park_fork_cfmesh::checks::check_quality(&vm);
    let dir = out_dir.join("constant").join("polyMesh");
    outram_park_fork_cfmesh::foam::write_polymesh(&vm, &dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    std::fs::write(dir.join("cellZones"), cell_zones_text(&map, &cell_region)).map_err(|e| e.to_string())?;
    // Images. Heights: bed middle, middle of the cavity above the bed, and
    // middle of the hot plenum's first box.
    let z_bed = 0.5 * (d.conus_top + d.bed_top);
    let z_up = 0.5 * (d.bed_top + d.cavity_top);
    let z_low = d
        .boxes
        .iter()
        .find(|b| RegionMap::HOT_PLENUM_ZONES.contains(&b.zone))
        .map(|b| 0.5 * (b.z[0] + b.z[1]));
    let title = |w: &str| format!("HTR-10 SOLID / FLUID / POROUS MESH, {} CELLS: {w}", um.n_cells());
    let mut images = vec![
        ("cht_regions_rz".to_string(), draw_region_map(d, &map, 900)),
        (
            "cht_mesh_xz".to_string(),
            draw(&um, &map, PlotBasis::Xz, [0.0, 0.0, 0.0], &title("X-Z THROUGH THE AXIS"), 900),
        ),
        (
            "cht_mesh_xy_bed".to_string(),
            draw(&um, &map, PlotBasis::Xy, [0.0, 0.0, z_bed], &title(&format!("X-Y AT Z = {z_bed:.1} CM (BED MIDDLE)")), 900),
        ),
        (
            "cht_mesh_xy_upper_plenum".to_string(),
            draw(&um, &map, PlotBasis::Xy, [0.0, 0.0, z_up], &title(&format!("X-Y AT Z = {z_up:.1} CM (CAVITY MIDDLE)")), 900),
        ),
    ];
    if let Some(z) = z_low {
        images.push((
            "cht_mesh_xy_lower_plenum".to_string(),
            draw(&um, &map, PlotBasis::Xy, [0.0, 0.0, z], &title(&format!("X-Y AT Z = {z:.1} CM (HOT PLENUM MIDDLE)")), 900),
        ));
    }
    Ok(ChtMesh {
        kinds: kinds(&um),
        mesh: um,
        map,
        cell_region,
        regions,
        faceted_cm3,
        boundaries,
        interfaces,
        patches: vm.patches.iter().map(|p| (p.name.clone(), p.n_faces)).collect(),
        quality: (q.max_non_orthogonality_deg, q.max_skewness),
        notes,
        polymesh_dir: dir,
        images,
        seconds: t.elapsed().as_secs_f64(),
    })
}

/// TetGen as the tet source at cell size `cell_cm`, with Mesh Studio's
/// defaults (gh:#1796): radius-edge ratio 1.414 and the volume of a regular
/// tetrahedron of edge `cell_cm`, `h^3 / (6 sqrt 2)`.
pub fn cht_tetgen(cell_cm: f64) -> outram_park_fork_cfmesh::pipeline::TetSource {
    let h = cell_cm / 100.0;
    outram_park_fork_cfmesh::pipeline::TetSource::tetgen(1.414, Some(h * h * h / (6.0 * 2f64.sqrt())))
}

/// [`ChtMesh`] as a short markdown report (regions, patches, quality).
pub fn cht_report(m: &ChtMesh, cell_cm: f64, n_seg: usize) -> String {
    let mut s = format!(
        "HTR-10 solid / fluid / porous mesh: {} cells, {} points, {} faces; tet -> dual at {cell_cm} cm, {n_seg} segments; {:.1} s\n\n",
        m.mesh.n_cells(),
        m.mesh.points().len(),
        m.mesh.n_faces(),
        m.seconds
    );
    s.push_str(&format!(
        "Region boundaries: {}.\n\n",
        match m.boundaries {
            ChtBoundaries::Fitted => "body-fitted (every interface is a face of the mesh)",
            ChtBoundaries::Centroid => "by cell centroid (stair-stepped at the cell size)",
        }
    ));
    s.push_str(&format!("| cellZone | class | cells | meshed volume [m³] | exact R-Z volume (analytic) [m³] | difference | faceted ({n_seg} segments) volume [m³] | difference |\n|---|---|---|---|---|---|---|---|\n"));
    for ((id, class, n, mv, ev), fv) in m.regions.iter().zip(&m.faceted_cm3) {
        s.push_str(&format!(
            "| `{id}` | {class} | {n} | {:.4} | {:.4} | {:+.2} % | {:.4} | {:+.2e} |\n",
            mv * 1e-6,
            ev * 1e-6,
            100.0 * (mv - ev) / ev.max(1e-30),
            fv * 1e-6,
            (mv - fv) / fv.max(1e-30)
        ));
    }
    if !m.interfaces.is_empty() {
        s.push_str("\n| interface | faces |\n|---|---|\n");
        for (p, n) in &m.interfaces {
            s.push_str(&format!("| `{p}` | {n} |\n"));
        }
    }
    s.push_str("\n| patch | faces |\n|---|---|\n");
    for (p, n) in &m.patches {
        s.push_str(&format!("| `{p}` | {n} |\n"));
    }
    s.push_str(&format!(
        "\nCell kinds: {}. Max non-orthogonality {:.1} deg, max skewness {:.2}.\n",
        m.kinds.iter().map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join(", "),
        m.quality.0,
        m.quality.1
    ));
    let zoned = (0..m.map.regions.len()).map(|g| m.cell_region.iter().filter(|&&x| x == g).count()).sum::<usize>();
    s.push_str(&format!("\n{} cellZones; {zoned} of {} cells are in one.\n", m.map.regions.len(), m.mesh.n_cells()));
    for n in &m.notes {
        s.push_str(&format!("\n- {n}"));
    }
    s.push('\n');
    s
}

/// The R-Z region map drawn directly from [`RegionMap::region_at`] (what the
/// mesh cells are assigned from), for the Step 7 main view and review.
pub fn draw_region_map(d: &RzDomain, map: &RegionMap, px: usize) -> ImageData {
    let cols = region_colours(map.regions.len());
    let w = 2.0 * d.r_outer * 1.05;
    let h = (d.z_top - d.z_bottom) * 1.05;
    let py = ((px as f64) * h / w).round() as usize;
    let zc = 0.5 * (d.z_top + d.z_bottom);
    let mut img = ImageData::filled(px, py, outram_blender::csg::plot::Rgb::new(255, 255, 255));
    for j in 0..py {
        let z = zc + h * (0.5 - (j as f64 + 0.5) / py as f64);
        for i in 0..px {
            let x = w * ((i as f64 + 0.5) / px as f64 - 0.5);
            if let Some(g) = map.region_at(d, x.abs(), z) {
                img.set(i, j, cols[g]);
            }
        }
    }
    let legend: Vec<LegendEntry> = map
        .regions
        .iter()
        .enumerate()
        .map(|(g, r)| LegendEntry::new(cols[g], format!("{} [{}]", r.label, class_short(r.class))))
        .collect();
    let frame = SlicePlot::new(PlotBasis::Xz, Position::new(0.0, 0.0, zc), [w, h], [px, py]);
    annotate_slice(
        &img,
        &frame,
        "HTR-10 R-Z REGIONS (FROM THE REGION MAP)",
        &legend,
    )
}

fn class_short(c: RegionClass) -> &'static str {
    match c {
        RegionClass::PebbleBed => "bed",
        RegionClass::Fluid => "fluid",
        RegionClass::Porous => "porous",
        RegionClass::Solid => "solid",
    }
}

/// Write `images` as PNGs into `dir`; returns the paths.
pub fn write_images(images: &[(String, ImageData)], dir: &Path) -> Result<Vec<PathBuf>, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for (stem, img) in images {
        let p = dir.join(format!("{stem}.png"));
        img.write_png(&p)
            .map_err(|e| format!("{}: {e}", p.display()))?;
        out.push(p);
    }
    Ok(out)
}

/// One CSV of the meshes and maps (for the headless mode and its test).
pub fn summary_csv(set: &MeshSet) -> String {
    let mut s = String::from("mesh,cell_type,cells,points,faces,volume_cm3,exact_cm3,volume_rel_diff,non_star_cells,max_non_orth_deg,patches\n");
    for m in &set.meshes {
        s.push_str(&format!(
            "{},{},{},{},{},{:.6e},{:.6e},{:.4e},{},{:.1},{}\n",
            m.role.slug(),
            m.cell_type.label(),
            m.cells,
            m.points,
            m.faces,
            m.volume_cm3,
            m.exact_cm3,
            m.volume_cm3 / m.exact_cm3 - 1.0,
            m.non_star_cells,
            m.max_non_orthogonality_deg,
            m.patches
                .iter()
                .map(|p| format!("{}:{}", p.0, p.1))
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    s.push_str("\nmesh,region,cells,mesh_cm3,exact_cm3,rel_diff\n");
    for m in &set.meshes {
        for r in &m.regions {
            s.push_str(&format!(
                "{},{},{},{:.6e},{:.6e},{:.4e}\n",
                m.role.slug(),
                set.plan.regions.regions[r.region].id,
                r.cells,
                r.mesh_cm3,
                r.exact_cm3,
                if r.exact_cm3 > 0.0 {
                    r.mesh_cm3 / r.exact_cm3 - 1.0
                } else {
                    f64::NAN
                }
            ));
        }
    }
    s.push_str("\nfrom,to,overlap_cm3,to_coverage,uncovered_target_cells,seconds\n");
    for m in &set.mappings {
        s.push_str(&format!(
            "{},{},{:.6e},{:.6},{},{:.1}\n",
            m.from.slug(),
            m.to.slug(),
            m.overlap_cm3,
            m.to_coverage,
            m.uncovered_cells,
            m.seconds
        ));
    }
    s
}
