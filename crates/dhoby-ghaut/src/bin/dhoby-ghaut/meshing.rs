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
//! **Region assignment is by cell centroid**: a region on a mesh is the set of
//! cells whose centroid lies in it, so its meshed volume differs from the
//! exact R-Z volume by a stair-step at the cell size. Each mesh's summary
//! reports both (gh:#594 tracks body-fitted region boundaries).

use petir::mathf::RealMath;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use dhoby_ghaut::workbench::meshes::{
    genfoam_default_fields, CellType, MeshMapping, MeshPlan, MeshRole, MeshSet, MeshSummary,
    RegionClass, RegionMap, RegionVolume, RzBox, RzDomain,
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
        ..TetDualOptions::default()
    }
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
) -> Result<(VolumeMesh, Vec<String>, CellType), String> {
    let s = &plan.roles[role.index()];
    match role {
        MeshRole::Neutronics => {
            let prof = to_m(&d.neutronics_profile());
            let (p, t) = revolved_profile_surface(&prof, s.n_seg);
            let (vm, rep) = outram_park_fork_cfmesh::pipeline::surface_to_tet_dual_mesh(
                &p,
                &t,
                &opts(s.cell_size_cm, s.refine, true),
            )?;
            Ok((vm, rep.stage_notes, CellType::TetDual))
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
            Ok((vm, rep.stage_notes, CellType::Tet4))
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
            let (vm, rep) = outram_park_fork_cfmesh::pipeline::surface_to_tet_dual_mesh_multipatch(
                &p,
                &t,
                &regions,
                &opts(s.cell_size_cm, s.refine, true),
            )?;
            let mut notes = rep.stage_notes;
            // Boundary layers on the fluid's wall only (maintainer default:
            // fluid = tet-dual + layers, porous = tet-dual). The pipeline grows
            // layers before naming patches, so they are grown here, on the
            // named patch, and the patches re-named after (the layerer
            // rebuilds the mesh).
            if s.layers > 0 {
                let before = vm.n_cells;
                let layered = outram_park_fork_cfmesh::layers::add_boundary_layers_adaptive(
                    &vm,
                    "cavity_wall",
                    s.layers,
                    s.first_layer_cm / 100.0,
                    s.expansion.max(1.0),
                );
                let named = assign_patches_by_region(&layered, &p, &t, &regions)?;
                let q = outram_park_fork_cfmesh::checks::check_quality(&named);
                if named.validate().is_ok() && q.n_negative_volume_cells == 0 {
                    if named.n_cells == before {
                        notes.push("boundary layers added none on cavity_wall (the adaptive layerer backed off)".into());
                    }
                    return Ok((named, notes, CellType::TetDualWithLayers));
                }
                notes.push("boundary layers skipped: they would invalidate the mesh".into());
            }
            Ok((vm, notes, CellType::TetDual))
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
        let (vm, mut notes, cell_type) = mesh_role(d, plan, role)?;
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

/// Build [`ChtMesh`]: the whole R-Z domain (`r <= r_outer`, bottom to top)
/// revolved with `n_seg` segments, cfMesh's tet -> dual pipeline at
/// `cell_cm` with tetrahedra from `tets` ([`cht_tetgen`] for the TetGen
/// port, as Mesh Studio's default; `TetSource::CarvedCentroid` for cfMesh's
/// carved route), the outer surface named `top`, `bottom` and `side`, every cell
/// given the region of its centroid, written to `out_dir/constant/polyMesh`
/// with `cellZones`, and drawn: R-Z through the axis, and X-Y through the
/// middle of each of the three non-solid regions. Region boundaries are
/// stair-stepped at the cell size (assignment by centroid, as Step 7).
pub fn build_cht(
    d: &RzDomain,
    cell_cm: f64,
    n_seg: usize,
    tets: outram_park_fork_cfmesh::pipeline::TetSource,
    out_dir: &Path,
) -> Result<ChtMesh, String> {
    let t = Instant::now();
    let map = RegionMap::three_class_cht(d);
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
    let mut notes = rep.stage_notes;
    notes.push(format!("tetrahedra: {tets:?}"));
    let um = from_volume_mesh(&vm).map_err(|e| e.to_string())?;
    let mut cell_region = Vec::with_capacity(um.n_cells());
    let mut unassigned = 0;
    for c in 0..um.n_cells() {
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
    let vols = outram_blender::unstructured::overlap::cell_volumes_cm3(&um);
    let exact = exact_in(d, &map, &d.neutronics_profile(), 0.5);
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
    s.push_str("| cellZone | class | cells | meshed volume [m³] | exact R-Z volume [m³] | difference |\n|---|---|---|---|---|---|\n");
    for (id, class, n, mv, ev) in &m.regions {
        s.push_str(&format!(
            "| `{id}` | {class} | {n} | {:.4} | {:.4} | {:+.2} % |\n",
            mv * 1e-6,
            ev * 1e-6,
            100.0 * (mv - ev) / ev.max(1e-30)
        ));
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
    s.push_str(&format!("\n{} cellZones; {zoned} of {} cells are in one, by centroid.\n", m.map.regions.len(), m.mesh.n_cells()));
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
