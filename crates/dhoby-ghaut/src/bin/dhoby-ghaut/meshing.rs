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
    RegionClass, RegionMap, RegionVolume, RzBox, RzDomain, RzPartition,
};
use outram_blender::csg::plot::{
    annotate_slice, default_colours, ImageData, LegendEntry, PlotBasis, SlicePlot,
};
use outram_blender::csg::position::Position;
use outram_blender::foam_mesh::{revolved_profile_surface, TetDualOptions, VolumeMesh};
use outram_blender::unstructured::convert::cfmesh::from_volume_mesh;
use outram_blender::unstructured::overlap::MeshOverlap;
use outram_blender::unstructured::{render_mesh_slice, MeshColourBy, MeshSlice, UnstructuredMesh, Zone};
use outram_park_fork_cfmesh::patches::SurfaceRegions;

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

/// Whether Step 7's `role` mesh holds region `g`: the neutronics mesh holds
/// every region, the thermal-hydraulics mesh the bed and the cavity above it
/// ([`RzDomain::th_profile`]), the structural mesh every region outside the
/// core ([`RzDomain::structural_profile`]; the conus and the discharge tube
/// are in neither).
fn role_holds(map: &RegionMap, role: MeshRole, g: usize) -> bool {
    match role {
        MeshRole::Neutronics => true,
        MeshRole::ThermalHydraulics => map.is_bed(g) || g == map.cavity,
        MeshRole::Structural => !map.regions[g].in_core,
    }
}

/// One Step 7 mesh, as [`mesh_role`] builds it.
struct RoleMesh {
    /// The cfMesh volume mesh (metres).
    vm: VolumeMesh,
    /// What was run.
    notes: Vec<String>,
    /// The cell type.
    cell_type: CellType,
    /// Region of every cell (a [`RegionMap`] index), from the fitted
    /// surface: no cell crosses a region boundary.
    cell_region: Vec<usize>,
    /// Exact (round, analytic) volume of every region inside this mesh's
    /// domain \[cm³\]; zero for a region the mesh does not hold.
    exact_cm3: Vec<f64>,
    /// The `cyclicAMI` interface pairs (empty on the conforming route).
    ami: Vec<outram_park_fork_cfmesh::pipeline::region_join::AmiPair>,
}

/// Mesh one role, **with every internal region boundary fitted and snapped**
/// (maintainer, 2026-11-26: "always snap internal region boundaries";
/// `outram-foam-appbuilder-lib/AGENTS.md`). The regions the role holds
/// ([`role_holds`]) are cut into one-region polygons
/// ([`RegionMap::rz_partition`]), revolved into one multi-region surface
/// ([`fitted_surface`]) and meshed by TetGen with region attributes
/// (`outram_park_fork_cfmesh::pipeline::multi_region`): the neutronics and
/// thermal-hydraulics meshes as per-region duals stitched at the interfaces,
/// the structural mesh as the tetrahedra themselves (Tet4). Each is then
/// snapped onto the true surfaces of revolution, interfaces included
/// ([`snap_checked`] with the cell regions), unless `DHOBY_NO_SNAP=1`. The
/// TH mesh grows its `cavity_wall` layers after the snap, on the true wall.
fn mesh_role(d: &RzDomain, plan: &MeshPlan, role: MeshRole) -> Result<RoleMesh, String> {
    let s = &plan.roles[role.index()];
    let map = &plan.regions;
    if s.refine > 0 {
        return Err(format!(
            "{}: wall refinement levels ({}) need the carved octree, which cannot fit internal region boundaries; Step 7's meshes are fitted on TetGen (set the levels to 0 and the cell size smaller instead)",
            role.name(),
            s.refine
        ));
    }
    let part = map.rz_partition(d)?;
    let mut exact_cm3 = part.volumes(map.regions.len(), None);
    for (g, v) in exact_cm3.iter_mut().enumerate() {
        if !role_holds(map, role, g) {
            *v = 0.0;
        }
    }
    let (zb, zt, zbed) = (d.conus_top, d.cavity_top, d.bed_top);
    let flat = |a: [f64; 2], b: [f64; 2], z: f64| (a[1] - z).abs() < 1e-9 && (b[1] - z).abs() < 1e-9;
    let th_outer = |a: [f64; 2], b: [f64; 2]| {
        // Top disc = inlet (the helium enters the cavity from the top plenum
        // and flows down), bottom disc = outlet, side above the bed =
        // cavity_wall (fluid: layers), side along the bed = bed_wall
        // (porous: no layers).
        if flat(a, b, zt) {
            "inlet".to_string()
        } else if flat(a, b, zb) {
            "outlet".to_string()
        } else if 0.5 * (a[1] + b[1]) > zbed {
            "cavity_wall".to_string()
        } else {
            "bed_wall".to_string()
        }
    };
    // The FV meshes: every region meshed on its own and the regions joined by
    // `cyclicAMI` interfaces (maintainer, 2026-11-27, gh:#1862). The
    // structural Tet4 mesh, and the FV meshes with
    // `DHOBY_STEP7_BOUNDARIES=conformal` (the ablation), mesh all regions in
    // one TetGen run instead (below).
    let conformal = std::env::var("DHOBY_STEP7_BOUNDARIES").as_deref() == Ok("conformal");
    if role != MeshRole::Structural && !conformal {
        let (snap_on, delta) = step7_snap(s.cell_size_cm);
        let layers = |g: usize| {
            (role == MeshRole::ThermalHydraulics && g == map.cavity && s.layers > 0).then(|| RegionLayers {
                patches: vec!["cavity_wall".to_string()],
                n: s.layers,
                first_m: s.first_layer_cm / 100.0,
                expansion: s.expansion.max(1.0),
            })
        };
        let outer = |a: [f64; 2], b: [f64; 2]| if role == MeshRole::ThermalHydraulics { th_outer(a, b) } else { "walls".to_string() };
        let j = mesh_regions_joined(map, &part, s.n_seg, s.cell_size_cm, |g| role_holds(map, role, g), outer, layers, snap_on.then_some(delta))
            .map_err(|e| format!("{}: {e}", role.name()))?;
        let mut notes = j.notes;
        if !snap_on {
            notes.push("not snapped: the boundary stays on the faceted surface (DHOBY_NO_SNAP=1)".into());
        }
        let layered = role == MeshRole::ThermalHydraulics && s.layers > 0;
        notes.push(interface_quality_note(&j.mesh, &j.cell_region));
        return Ok(RoleMesh {
            vm: j.mesh,
            notes,
            cell_type: if layered { CellType::TetDualWithLayers } else { CellType::TetDual },
            cell_region: j.cell_region,
            exact_cm3,
            ami: j.ami,
        });
    }
    let fitted = match role {
        MeshRole::ThermalHydraulics => fitted_surface(map, &part, s.n_seg, |g| role_holds(map, role, g), |a, b, _| th_outer(a, b)),
        _ => fitted_surface(map, &part, s.n_seg, |g| role_holds(map, role, g), |_, _, _| "walls".to_string()),
    };
    let h = s.cell_size_cm / 100.0;
    let tets = outram_park_fork_cfmesh::pipeline::TetSource::tetgen(1.414, Some(h * h * h / (6.0 * 2f64.sqrt())));
    let dual = role != MeshRole::Structural;
    let m = outram_park_fork_cfmesh::pipeline::multi_region::multi_region_mesh(&fitted.surf, tets, dual)
        .map_err(|e| format!("{}: {e}", role.name()))?;
    let mut vm = m.mesh;
    let mut cell_region: Vec<usize> = m.cell_region.iter().map(|&l| fitted.global[l]).collect();
    let mut notes = m.notes;
    notes.push(format!(
        "region boundaries fitted: {} interface faces ({})",
        m.interface_faces.iter().map(|i| i.1).sum::<usize>(),
        m.interface_faces.iter().map(|i| format!("{} {}", i.0, i.1)).collect::<Vec<_>>().join(", ")
    ));
    let (snap_on, delta) = step7_snap(s.cell_size_cm);
    if snap_on {
        notes.push(snap_checked(&mut vm, Some(&cell_region), &fitted.segs_m, s.n_seg, delta).map_err(|e| format!("{}: {e}", role.name()))?);
    } else {
        notes.push("not snapped: the boundary stays on the faceted surface (DHOBY_NO_SNAP=1)".into());
    }
    let cell_type = match role {
        MeshRole::Structural => CellType::Tet4,
        _ => CellType::TetDual,
    };
    if role != MeshRole::ThermalHydraulics || s.layers == 0 {
        notes.push(interface_quality_note(&vm, &cell_region));
        return Ok(RoleMesh { vm, notes, cell_type, cell_region, exact_cm3, ami: Vec::new() });
    }
    // Boundary layers on the fluid's wall only (maintainer default: fluid =
    // tet-dual + layers, porous = tet-dual), grown on the SNAPPED dual
    // (snapping a layered mesh took its worst face to 92.7 deg, 2026-11-26).
    // ~~`layers::add_boundary_layers_adaptive` (this crate's global
    // back-off)~~ CHANGED 2026-11-26: since gh:#1215's quality check the
    // back-off shrank every column of `cavity_wall` to nothing ("added
    // none"; the 2026-10-05 record had 11 568 layer hexes). cfMesh's own
    // `boundaryLayerOptimisation` thins locally instead (the layerer left
    // callable by gh:#1829 for this workbench). TetGen's layers-first route
    // is not used: it refuses this mesh at the cavity_wall / bed_wall seam
    // (a column thinner than 1 % of the asked thickness).
    let before = vm.n_cells;
    let (layered, lo) = outram_park_fork_cfmesh::boundary_layer_optimisation::add_boundary_layers_cfmesh(
        &vm,
        &["cavity_wall"],
        s.layers,
        s.first_layer_cm / 100.0,
        s.expansion.max(1.0),
        // With cfMesh's untangling passes (the boundary moves, the LP
        // untangler, the Laplace pass): without them 2 of the 20 952 cells
        // came out inverted and the layers were refused (measured
        // 2026-11-26).
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
    // The layerer keeps the cells in order and appends the layer cells
    // (`n_prism0 = mesh.n_cells`); every one sits on `cavity_wall`, which
    // only the cavity touches, so each is the cavity's.
    if layered.n_cells < before {
        return Err(format!("thermal-hydraulics: the layered mesh has {} cells, fewer than the {before} it was grown on", layered.n_cells));
    }
    // The layerer rebuilds the mesh without its patch names: name its
    // boundary faces again from the TH domain's own geometry
    // ([`name_th_patches`]). ~~By the nearest triangle of the fitted
    // surface~~ (2026-11-26): that surface is faceted, the mesh is snapped
    // out to the true radius, so a disc face at the rim lay as near a wall
    // facet as its own disc, and a layer prism's end face on the inlet was
    // named `cavity_wall` (a point 1 cm inside the wall on that patch).
    let mut named = name_th_patches(&layered, d);
    let q = outram_park_fork_cfmesh::checks::check_quality(&named);
    if named.validate().is_err() || q.n_negative_volume_cells != 0 {
        notes.push("boundary layers skipped: they would invalidate the mesh".into());
        notes.push(interface_quality_note(&vm, &cell_region));
        return Ok(RoleMesh { vm, notes, cell_type, cell_region, exact_cm3, ami: Vec::new() });
    }
    cell_region.resize(named.n_cells, map.cavity);
    if named.n_cells == before {
        notes.push("boundary layers added none on cavity_wall (the layerer backed off)".into());
    }
    if snap_on {
        // The layerer's untangling may move a wall point off the cylinder:
        // snap the surface faces again, with no morph (a dry run first, so a
        // refusal keeps the mesh as grown).
        match snap_checked(&mut named.clone(), Some(&cell_region), &fitted.segs_m, s.n_seg, 0.0) {
            Ok(_) => {
                let n = snap_checked(&mut named, Some(&cell_region), &fitted.segs_m, s.n_seg, 0.0)?;
                notes.push(format!("after the layers: {n}"));
            }
            Err(e) => notes.push(format!("after the layers, the surface re-snap was refused ({e}); the layered mesh is kept as grown on the snapped wall")),
        }
    }
    notes.push(interface_quality_note(&named, &cell_region));
    Ok(RoleMesh { vm: named, notes, cell_type: CellType::TetDualWithLayers, cell_region, exact_cm3, ami: Vec::new() })
}

/// Boundary layers one region grows in [`mesh_regions_joined`], grown first
/// by the single-region pipeline (`outram_park_fork_cfmesh` HARD RULE,
/// gh:#1829).
#[derive(Debug, Clone)]
pub struct RegionLayers {
    /// The region's patches the layers grow on.
    pub patches: Vec<String>,
    /// Number of layers.
    pub n: usize,
    /// First layer thickness \[m\].
    pub first_m: f64,
    /// Growth ratio.
    pub expansion: f64,
}

/// [`mesh_regions_joined`]' result.
pub struct JoinedRegions {
    /// The joined mesh (metres): each region's cells, its interface sides as
    /// boundary patches `<region>_to_<other>`.
    pub mesh: VolumeMesh,
    /// Region of every cell (a [`RegionMap`] index).
    pub cell_region: Vec<usize>,
    /// The interfaces, each a `cyclicAMI` pair.
    pub ami: Vec<outram_park_fork_cfmesh::pipeline::region_join::AmiPair>,
    /// What was run, region by region.
    pub notes: Vec<String>,
}

/// **Every region meshed on its own, joined by AMI interfaces** (gh:#1862;
/// maintainer, 2026-11-27: "mesh each region separately, then combine them
/// into a multiregion mesh using a AMI interface").
///
/// - **Surface.** Every region `holds` keeps is cut from `part` as its own
///   closed surface ([`fitted_surface`] keeping that region alone). An edge
///   to another kept region `h` is the patch `<region>_to_<h>`; any other
///   boundary edge is named by `outer(a, b)` (cm).
/// - **Mesh.** The single-region pipeline meshes it
///   (`surface_to_tet_dual_mesh_multipatch`: TetGen `-pq1.414a<V>`, `V` the
///   regular tetrahedron of `cell_cm`, then the polyhedral dual), with the
///   layers `layers(region)` asks for grown first.
/// - **Snap.** With `snap = Some(delta)` (metres) each region's mesh is
///   snapped onto the true surfaces of revolution, its interface sides
///   included ([`snap_checked`]: a morph within `delta`, then the surface
///   faces; a non-positive cell is refused).
/// - **Join.** The meshes are joined
///   (`outram_park_fork_cfmesh::pipeline::region_join`), each interface a
///   pair of patches for `cyclicAMI`.
///
/// The two sides of a curved interface are different tilings of the same
/// true surface, so they need not coincide. The AMI weights carry the
/// coupling, and the pairs' areas are reported.
///
/// # Errors
/// If a region fails to mesh or to snap, or the join refuses.
#[allow(clippy::too_many_arguments)]
pub fn mesh_regions_joined(
    map: &RegionMap,
    part: &RzPartition,
    n_seg: usize,
    cell_cm: f64,
    holds: impl Fn(usize) -> bool,
    outer: impl Fn([f64; 2], [f64; 2]) -> String,
    layers: impl Fn(usize) -> Option<RegionLayers>,
    snap: Option<f64>,
) -> Result<JoinedRegions, String> {
    let mut present: Vec<usize> = part.polygons.iter().map(|p| p.1).filter(|&g| holds(g)).collect();
    present.sort_unstable();
    present.dedup();
    let h = cell_cm / 100.0;
    let tets = outram_park_fork_cfmesh::pipeline::TetSource::tetgen(1.414, Some(h * h * h / (6.0 * 2f64.sqrt())));
    let mut meshes = Vec::with_capacity(present.len());
    let mut notes = Vec::new();
    let side = |g: usize, o: usize| format!("{}_to_{}", map.regions[g].id, map.regions[o].id);
    for &g in &present {
        let id = &map.regions[g].id;
        let fs = fitted_surface(map, part, n_seg, |x| x == g, |a, b, across| match across {
            Some(o) if holds(o) => side(g, o),
            _ => outer(a, b),
        });
        let labels: Vec<&str> = fs.surf.marker.iter().map(|&k| fs.surf.patch_names[k].as_str()).collect();
        let regions = SurfaceRegions::from_labels(&labels);
        let lay = layers(g);
        let opts = TetDualOptions {
            cell_size: h,
            dual: true,
            dual_min_faces: true,
            n_layers: lay.as_ref().map_or(0, |l| l.n),
            first_layer_thickness: lay.as_ref().map_or(0.0, |l| l.first_m),
            expansion: lay.as_ref().map_or(1.0, |l| l.expansion),
            layer_patches: lay.as_ref().map_or_else(Vec::new, |l| l.patches.clone()),
            tet_source: tets,
            ..TetDualOptions::default()
        };
        let (mut vm, rep) = outram_park_fork_cfmesh::pipeline::surface_to_tet_dual_mesh_multipatch(&fs.surf.points, &fs.surf.tris, &regions, &opts)
            .map_err(|e| format!("{id}: {e}"))?;
        notes.extend(rep.stage_notes.into_iter().map(|n| format!("{id}: {n}")));
        if let Some(delta) = snap {
            let n = snap_checked(&mut vm, None, &fs.segs_m, n_seg, delta).map_err(|e| format!("{id}: {e}"))?;
            notes.push(format!("{id}: {n}"));
        }
        notes.push(format!("{id}: {} cells", vm.n_cells));
        meshes.push(vm);
    }
    let has = |k: usize, name: &str| meshes[k].patches.iter().any(|p| p.name == name);
    let mut pairs = Vec::new();
    for (i, &g) in present.iter().enumerate() {
        for (j, &o) in present.iter().enumerate().skip(i + 1) {
            let (a, b) = (side(g, o), side(o, g));
            match (has(i, &a), has(j, &b)) {
                (true, true) => pairs.push((a, b)),
                (false, false) => {}
                _ => return Err(format!("{a} / {b}: only one side of the interface was meshed")),
            }
        }
    }
    let j = outram_park_fork_cfmesh::pipeline::region_join::join_regions(&meshes, &pairs)?;
    for p in &j.ami {
        notes.push(format!("AMI {} / {}: areas {:.6e} / {:.6e} m², mismatch {:+.2e}", p.a, p.b, p.area_a, p.area_b, p.area_mismatch()));
    }
    let cell_region = j.cell_region.iter().map(|&k| present[k]).collect();
    Ok(JoinedRegions { mesh: j.mesh, cell_region, ami: j.ami, notes })
}

/// Name the boundary faces of a Step 7 thermal-hydraulics mesh (the bed
/// and the cavity, `r <= core_radius`, `conus_top <= z <= cavity_top`) by
/// their geometry: a face whose unit normal is mostly axial (`|n_z| >= 0.5`,
/// the rule of [`snap_faces_to_revolved`]) is `inlet` at the top and
/// `outlet` at the bottom; any other is the side wall, `cavity_wall` above
/// `bed_top` and `bed_wall` along the bed (the fitted surface has a ring at
/// `bed_top`, so no face straddles it). Internal faces keep their order.
fn name_th_patches(mesh: &VolumeMesh, d: &RzDomain) -> VolumeMesh {
    use outram_park_fork_cfmesh::volume_mesh::BoundaryPatch;
    const NAMES: [&str; 4] = ["inlet", "outlet", "cavity_wall", "bed_wall"];
    let zmid = 0.5 * (d.conus_top + d.cavity_top) / 100.0;
    let zbed = d.bed_top / 100.0;
    let mut order: Vec<usize> = (0..mesh.face_count()).filter(|&f| mesh.neighbour[f].is_some()).collect();
    let mut buckets: [Vec<usize>; 4] = Default::default();
    for f in 0..mesh.face_count() {
        if mesh.neighbour[f].is_some() {
            continue;
        }
        let a = mesh.face_area_vector(f);
        let c = mesh.face_centroid(f);
        let axial = a.z.abs() >= 0.5 * (a.x * a.x + a.y * a.y + a.z * a.z).sqrt();
        let k = if axial {
            if c.z > zmid { 0 } else { 1 }
        } else if c.z > zbed {
            2
        } else {
            3
        };
        buckets[k].push(f);
    }
    let mut patches = Vec::new();
    for (k, b) in buckets.iter().enumerate() {
        if b.is_empty() {
            continue;
        }
        patches.push(BoundaryPatch { name: NAMES[k].to_string(), start_face: order.len(), n_faces: b.len() });
        order.extend(b.iter().copied());
    }
    VolumeMesh {
        points: mesh.points.clone(),
        faces: order.iter().map(|&f| mesh.faces[f].clone()).collect(),
        owner: order.iter().map(|&f| mesh.owner[f]).collect(),
        neighbour: order.iter().map(|&f| mesh.neighbour[f]).collect(),
        n_cells: mesh.n_cells,
        patches,
        cells: None,
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
    for (k, role) in MeshRole::ALL.into_iter().enumerate() {
        progress(MeshProgress::Stage(
            format!("meshing: {}", role.name()),
            k as f64 / 6.0,
        ));
        let t = Instant::now();
        let RoleMesh { vm, notes, cell_type, cell_region, exact_cm3: exact, ami } = mesh_role(d, plan, role)?;
        let um = from_volume_mesh(&vm).map_err(|e| format!("{}: {e}", role.name()))?;
        if cell_region.len() != um.n_cells() {
            return Err(format!("{}: {} cell regions for {} cells", role.name(), cell_region.len(), um.n_cells()));
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
        // ~~`exact_in`'s 0.5 cm quadrature~~ the partition's analytic volumes (2026-11-26:
        // the quadrature was off by up to 0.4 % where a region edge is off its grid).
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
        // A joined mesh writes its interfaces as `cyclicAMI` pairs.
        outram_park_fork_cfmesh::foam::write_polymesh_with_ami(&vm, &ami, &dir)
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

/// A multi-region surface built from an [`RzPartition`], with what the
/// mesh needs back: the global region of each local region, and the
/// non-horizontal walls and interfaces to snap onto \[m\].
pub struct FittedSurface {
    /// The surface for `outram_park_fork_cfmesh::pipeline::multi_region`
    /// (metres; local region indices).
    pub surf: outram_park_fork_cfmesh::pipeline::multi_region::MultiRegionSurface,
    /// `global[local]`: the [`RegionMap`] index of each local region.
    pub global: Vec<usize>,
    /// Every non-horizontal edge of the surface (walls and interfaces), in
    /// metres, for [`snap_faces_to_revolved`] and [`morph_to_revolved`].
    pub segs_m: Vec<[[f64; 2]; 2]>,
}

/// The **body-fitted multi-region surface** of the regions `allowed` keeps
/// (root rule: internal region boundaries are always fitted, gh:#1800
/// follow-up): every edge of `part` between two kept regions of different
/// index (an interface patch `<region a>_to_<region b>`) and between a kept
/// region and the outside or a region not kept (an outer patch, named by
/// `outer(a, b, across)` for the edge from `a` to `b` in cm, `across` the
/// region not kept on its other side, `None` outside the map), revolved with `n_seg`
/// segments as [`revolved_profile_surface`] revolves a profile (a vertex on
/// the axis is one point; an edge on the axis sweeps nothing). Collinear
/// edges of one patch meeting at a vertex no other kept edge uses are merged
/// first. One seed per connected component of every kept region, at the
/// area centroid of its first polygon, at angle 0 (where the faceted rings
/// pass through the true radii).
pub fn fitted_surface(
    map: &RegionMap,
    part: &RzPartition,
    n_seg: usize,
    allowed: impl Fn(usize) -> bool,
    outer: impl Fn([f64; 2], [f64; 2], Option<usize>) -> String,
) -> FittedSurface {
    use outram_park_fork_cfmesh::math::Vec3 as V;
    use std::collections::BTreeMap;
    // Shared vertices (1e-6 cm keys) and each polygon's vertex ids.
    let mut vid: BTreeMap<(i64, i64), usize> = BTreeMap::new();
    let mut verts: Vec<[f64; 2]> = Vec::new();
    let key = |p: [f64; 2]| ((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64);
    let mut polys: Vec<(Vec<usize>, usize)> = Vec::new();
    for (poly, g) in &part.polygons {
        let mut ids: Vec<usize> = Vec::new();
        for p in poly {
            let k = key(*p);
            let id = *vid.entry(k).or_insert_with(|| {
                verts.push(*p);
                verts.len() - 1
            });
            if ids.last() != Some(&id) {
                ids.push(id);
            }
        }
        if ids.len() > 1 && ids[0] == *ids.last().unwrap() {
            ids.pop();
        }
        polys.push((ids, *g));
    }
    // Edge -> polygons on it.
    let mut edge_polys: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
    for (pi, (ids, _)) in polys.iter().enumerate() {
        for k in 0..ids.len() {
            let (a, b) = (ids[k], ids[(k + 1) % ids.len()]);
            edge_polys.entry((a.min(b), a.max(b))).or_default().push(pi);
        }
    }
    // Local regions: the kept ones present, in map order.
    let mut present: Vec<usize> = polys.iter().map(|p| p.1).filter(|&g| allowed(g)).collect();
    present.sort_unstable();
    present.dedup();
    let local = |g: usize| present.iter().position(|&x| x == g);
    let mut patch_names: Vec<String> = Vec::new();
    let mut interface: Vec<bool> = Vec::new();
    let mut patch_of = |name: String, iface: bool| -> usize {
        match patch_names.iter().position(|n| *n == name) {
            Some(k) => k,
            None => {
                patch_names.push(name);
                interface.push(iface);
                patch_names.len() - 1
            }
        }
    };
    // Kept edges with their patch.
    let mut kept: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    for (&(a, b), ps) in &edge_polys {
        let (pa, pb) = (verts[a], verts[b]);
        if pa[0].abs() < 1e-9 && pb[0].abs() < 1e-9 {
            continue; // on the axis
        }
        let regs: Vec<usize> = ps.iter().map(|&pi| polys[pi].1).filter(|&g| allowed(g)).collect();
        // The region across an outer edge: a region not kept, or none (the
        // outside of the R-Z map).
        let across = ps.iter().map(|&pi| polys[pi].1).find(|&g| !allowed(g));
        let patch = match regs.as_slice() {
            [_] => Some(patch_of(outer(pa, pb, across), false)),
            [g, h] if g != h => {
                let (x, y) = ((*g).min(*h), (*g).max(*h));
                Some(patch_of(format!("{}_to_{}", map.regions[x].id, map.regions[y].id), true))
            }
            _ => None,
        };
        if let Some(p) = patch {
            kept.insert((a, b), p);
        }
    }
    // Merge collinear runs of one patch through vertices of degree 2.
    let mut at: BTreeMap<usize, Vec<(usize, usize)>> = BTreeMap::new();
    for &e in kept.keys() {
        at.entry(e.0).or_default().push(e);
        at.entry(e.1).or_default().push(e);
    }
    let collinear = |a: usize, m: usize, b: usize| {
        let (u, w) = ([verts[m][0] - verts[a][0], verts[m][1] - verts[a][1]], [verts[b][0] - verts[m][0], verts[b][1] - verts[m][1]]);
        let cross = u[0] * w[1] - u[1] * w[0];
        let lu = (u[0] * u[0] + u[1] * u[1]).sqrt();
        let lw = (w[0] * w[0] + w[1] * w[1]).sqrt();
        cross.abs() <= 1e-9 * lu * lw && u[0] * w[0] + u[1] * w[1] > 0.0
    };
    let mut done: std::collections::BTreeSet<(usize, usize)> = std::collections::BTreeSet::new();
    let mut runs: Vec<(usize, usize, usize)> = Vec::new();
    let other = |e: (usize, usize), v: usize| if e.0 == v { e.1 } else { e.0 };
    for (&e, &p) in &kept {
        if done.contains(&e) {
            continue;
        }
        done.insert(e);
        // Extend both ways.
        let mut ends = [e.0, e.1];
        for side in 0..2 {
            loop {
                let v = ends[side];
                let prev = ends[1 - side];
                let es = &at[&v];
                if es.len() != 2 {
                    break;
                }
                let next = if done.contains(&es[0]) { es[1] } else { es[0] };
                if done.contains(&next) || kept[&next] != p {
                    break;
                }
                let w = other(next, v);
                let ok = if side == 1 { collinear(prev, v, w) } else { collinear(w, v, prev) };
                if !ok {
                    break;
                }
                done.insert(next);
                ends[side] = w;
            }
        }
        runs.push((ends[0], ends[1], p));
    }
    // Points: one ring per used vertex (one point on the axis).
    let mut points: Vec<V> = Vec::new();
    let mut ring: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    let mut ring_of = |v: usize, points: &mut Vec<V>| -> Vec<usize> {
        ring.entry(v)
            .or_insert_with(|| {
                let (r, z) = (verts[v][0] / 100.0, verts[v][1] / 100.0);
                if r.abs() < 1e-12 {
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
    let mut segs_m = Vec::new();
    for &(a, b, p) in &runs {
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
        if (verts[a][1] - verts[b][1]).abs() > 1e-9 {
            segs_m.push([[verts[a][0] / 100.0, verts[a][1] / 100.0], [verts[b][0] / 100.0, verts[b][1] / 100.0]]);
        }
    }
    // Seeds: connected components of kept polygons of one region.
    let mut comp = vec![usize::MAX; polys.len()];
    let mut seeds = Vec::new();
    for start in 0..polys.len() {
        let g = polys[start].1;
        if comp[start] != usize::MAX || !allowed(g) {
            continue;
        }
        let id = seeds.len();
        let poly = &part.polygons[start].0;
        let n = poly.len();
        let area: f64 = 0.5 * (0..n).map(|k| poly[k][0] * poly[(k + 1) % n][1] - poly[(k + 1) % n][0] * poly[k][1]).sum::<f64>();
        let rc = RzPartition::first_moment(poly) / area;
        let zc = poly.iter().map(|q| q[1]).sum::<f64>() / n as f64;
        seeds.push((V::new(rc / 100.0, 0.0, zc / 100.0), local(g).expect("kept")));
        let mut stack = vec![start];
        comp[start] = id;
        while let Some(pi) = stack.pop() {
            let ids = &polys[pi].0;
            for k in 0..ids.len() {
                let (a, b) = (ids[k], ids[(k + 1) % ids.len()]);
                for &q in &edge_polys[&(a.min(b), a.max(b))] {
                    if comp[q] == usize::MAX && polys[q].1 == g {
                        comp[q] = id;
                        stack.push(q);
                    }
                }
            }
        }
    }
    FittedSurface {
        surf: outram_park_fork_cfmesh::pipeline::multi_region::MultiRegionSurface {
            points,
            tris,
            marker,
            patch_names,
            interface,
            region_names: present.iter().map(|&g| map.regions[g].id.clone()).collect(),
            seeds,
        },
        global: present,
        segs_m,
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
    let part = map.rz_partition(d)?;
    let faceted_cm3 = part.volumes(map.regions.len(), Some(n_seg));
    let (zb, zt) = (d.z_bottom, d.z_top);
    let fitted = fitted_surface(&map, &part, n_seg, |_| true, |a: [f64; 2], b: [f64; 2], _: Option<usize>| {
        if (a[1] - zt).abs() < 1e-9 && (b[1] - zt).abs() < 1e-9 {
            "top".to_string()
        } else if (a[1] - zb).abs() < 1e-9 && (b[1] - zb).abs() < 1e-9 {
            "bottom".to_string()
        } else {
            "side".to_string()
        }
    });
    let (vm, fitted_region, mut notes, interfaces) = match boundaries {
        ChtBoundaries::Fitted => {
            // Snapping TetGen's own surface vertices before the dual was
            // measured and rejected (2026-11-26): it took the cells away
            // from the interfaces from 63.8 to 87.4 deg and skewness 1.3 to
            // 14.9; the dual is snapped after, with a morph (below).
            let m = outram_park_fork_cfmesh::pipeline::multi_region::multi_region_mesh(&fitted.surf, tets, true)?;
            let global: Vec<usize> = m.cell_region.iter().map(|&l| fitted.global[l]).collect();
            (m.mesh, Some(global), m.notes, m.interface_faces)
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
            ChtBoundaries::Fitted => fitted.segs_m.clone(),
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
    // The partition's volumes are analytic (each polygon revolved exactly);
    // `exact_in`'s 0.5 cm quadrature was off by up to 0.4 % where a region
    // edge is not on its grid (the bed's top and bottom).
    let exact = part.volumes(map.regions.len(), None);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Diagnostic (2026-11-26): the TH mesh at 25 cm with 0 and 3 layers,
    /// the cavity's volume against exact and how far the disc points left
    /// their planes. `#[ignore]`: run by hand.
    #[test]
    #[ignore]
    fn diagnose_th_cavity_volume_at_25_cm() {
        let r = crate::preset::htr10();
        let mut engine = crate::engine::Engine::default();
        let a = crate::headless::assemble_for_test(&mut engine, &r).expect("assembly");
        let d = domain_from(&a);
        for layers in [0, 3] {
            let mut plan = MeshPlan::default_for(&d);
            plan.roles[1].cell_size_cm = 25.0;
            plan.roles[1].layers = layers;
            let m = mesh_role(&d, &plan, MeshRole::ThermalHydraulics).expect("th");
            let mut v = vec![0.0; plan.regions.regions.len()];
            let um = from_volume_mesh(&m.vm).unwrap();
            let vols = outram_blender::unstructured::overlap::cell_volumes_cm3(&um);
            for (c, &g) in m.cell_region.iter().enumerate() {
                v[g] += vols[c];
            }
            let (zt, zb) = (d.cavity_top / 100.0, d.conus_top / 100.0);
            let (mut dzt, mut dzb, mut rmax) = (0.0f64, 0.0f64, 0.0f64);
            for p in &m.vm.points {
                let rr = (p.x * p.x + p.y * p.y).sqrt();
                rmax = rmax.max(rr);
                if (p.z - zt).abs() < 0.05 { dzt = dzt.max((p.z - zt).abs()); }
                if (p.z - zb).abs() < 0.05 { dzb = dzb.max((p.z - zb).abs()); }
            }
            let zmax = m.vm.points.iter().map(|p| p.z).fold(f64::MIN, f64::max);
            let zmin = m.vm.points.iter().map(|p| p.z).fold(f64::MAX, f64::min);
            for g in 0..v.len() {
                if m.exact_cm3[g] > 0.0 {
                    eprintln!("layers {layers}: {} {:.6e} vs {:.6e}: {:+.4e}", plan.regions.regions[g].id, v[g], m.exact_cm3[g], v[g] / m.exact_cm3[g] - 1.0);
                }
            }
            eprintln!("layers {layers}: z {zmin}..{zmax} (want {zb}..{zt}); near-disc dz top {dzt:e} bottom {dzb:e}; r max {rmax}; patches {:?}", m.vm.patches.iter().map(|p| (p.name.clone(), p.n_faces)).collect::<Vec<_>>());
        }
    }
}
