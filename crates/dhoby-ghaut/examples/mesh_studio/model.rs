//! The studio's model, with no GUI in it.
//!
//! Everything here is callable from a test or from `--headless`. That
//! separation is the whole point: the GUI module below owns only
//! widgets and drawing, so nothing about what the studio *computes*
//! depends on a window existing.

use outram_blender::foam_mesh::{
    check_closed_manifold, mesh_to_surface, revolved_profile_surface, TetDualOptions,
    TetDualReport, Vec3, VolumeMesh,
};
use outram_park_fork_cfmesh::patches::SurfaceRegions;
use outram_park_fork_cfmesh::pipeline::staged::{
    surface_to_tet_dual_mesh_multipatch_staged, surface_to_tet_dual_mesh_staged, LayerIndex,
    StageSnapshot,
};
use outram_blender::mesh::Mesh;
use outram_blender::primitives::{cube, cylinder, uv_sphere};
use outram_blender::subdivision::catmull_clark;

/// Which blender-authored surface to volume-mesh.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GeomKind {
    Box,
    Sphere,
    Cylinder,
    /// A procedurally-authored blender mesh: a cube run through one
    /// level of Catmull-Clark subdivision (a rounded, closed genus-0
    /// blob) — proof the bridge accepts an arbitrary authored surface,
    /// not just a primitive.
    SubdividedCube,
    /// Reactor shape (gh:#1768): a coolant pipe, radius `size/2`, length
    /// `3 size`; patches `inlet`, `outlet`, `walls`, layers on `walls`.
    Pipe,
    /// Reactor shape (gh:#1768): the coolant annulus around a fuel pin, pin
    /// radius `size/4`, channel radius `size/2`, height `2 size`; patches
    /// `inlet`, `outlet`, `pin`, `channel`, layers on `pin` and `channel`.
    FuelChannel,
    /// Reactor shape (gh:#1768): a pebble in a coolant channel, pebble
    /// radius `0.3 size` centred on the axis at mid-height, channel radius
    /// `size/2`, height `1.5 size`; patches `inlet`, `outlet`, `pebble`,
    /// `channel`, layers on `pebble` and `channel`.
    Pebble,
}

impl GeomKind {
    pub fn label(self) -> &'static str {
        match self {
            GeomKind::Box => "Box (primitive)",
            GeomKind::Sphere => "UV-sphere (primitive)",
            GeomKind::Cylinder => "Cylinder (primitive)",
            GeomKind::SubdividedCube => "Subdivided cube (procedural)",
            GeomKind::Pipe => "Coolant pipe (reactor)",
            GeomKind::FuelChannel => "Fuel pin in a coolant channel (reactor)",
            GeomKind::Pebble => "Pebble in a coolant channel (reactor)",
        }
    }

    /// Short machine-readable name, used by `--headless` and by the CSV.
    pub fn slug(self) -> &'static str {
        match self {
            GeomKind::Box => "box",
            GeomKind::Sphere => "sphere",
            GeomKind::Cylinder => "cylinder",
            GeomKind::SubdividedCube => "subdivided-cube",
            GeomKind::Pipe => "pipe",
            GeomKind::FuelChannel => "fuel-channel",
            GeomKind::Pebble => "pebble",
        }
    }

    pub fn from_slug(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|k| k.slug() == s)
    }

    pub const ALL: [GeomKind; 7] = [
        GeomKind::Box,
        GeomKind::Sphere,
        GeomKind::Cylinder,
        GeomKind::SubdividedCube,
        GeomKind::Pipe,
        GeomKind::FuelChannel,
        GeomKind::Pebble,
    ];

    /// The four blender-authored primitives: the headless sweep and its
    /// committed fixture cover exactly these (gh:#1768 added the reactor
    /// shapes without moving the fixture).
    pub const PRIMITIVES: [GeomKind; 4] = [
        GeomKind::Box,
        GeomKind::Sphere,
        GeomKind::Cylinder,
        GeomKind::SubdividedCube,
    ];

    /// Is this a reactor shape, meshed from a revolved R-Z profile with
    /// named patches (gh:#1768)?
    pub fn is_reactor(self) -> bool {
        matches!(
            self,
            GeomKind::Pipe | GeomKind::FuelChannel | GeomKind::Pebble
        )
    }

    /// The dual a new design of this shape starts with. Reactor shapes take
    /// the cell-centre dual (`polyDualMesh`), Foam Studio's maintainer-approved
    /// choice for multi-patch pipes (gh:#1298): measured 2026-11-26 on these
    /// three shapes with back-off layers, the per-corner median dual grew no
    /// layer on any of them (and was refused outright on the pebble), the
    /// cell-centre dual grew all three on the pipe (5.0 s) and the fuel
    /// channel (1.6 s).
    pub fn default_dual(self) -> outram_park_fork_cfmesh::dual::DualKind {
        use outram_park_fork_cfmesh::dual::DualKind;
        if self.is_reactor() {
            DualKind::CellCentre
        } else {
            TetDualOptions::default().dual_kind
        }
    }

    /// The background cell size a new design of this shape starts at, for a
    /// shape of `size` \[m\]: four cells across the narrowest gap of a
    /// reactor shape (the pin-to-channel annulus is `size/4` wide, the
    /// pebble-to-channel gap `size/5`), a quarter of the size for a
    /// primitive (the desktop app's historical 0.5 m at 2 m).
    pub fn default_cell_size(self, size: f64) -> f64 {
        match self {
            GeomKind::Pipe => size / 8.0,
            GeomKind::FuelChannel => size / 16.0,
            GeomKind::Pebble => size / 20.0,
            _ => size / 4.0,
        }
    }
}

/// A reactor shape's closed surface and its triangle labels (gh:#1768):
/// a revolved R-Z profile (`outram_blender::foam_mesh::revolved_profile_surface`,
/// 32 segments) labelled by where each triangle's centroid lies. Metres.
/// Proportions are illustrative, not a particular reactor's.
pub fn reactor_surface(
    kind: GeomKind,
    size: f64,
) -> (Vec<Vec3>, Vec<[usize; 3]>, Vec<&'static str>, Vec<String>) {
    const N_SEG: usize = 32;
    let rc = 0.5 * size;
    let (profile, height, walls): (Vec<[f64; 2]>, f64, Vec<String>) = match kind {
        GeomKind::Pipe => {
            let l = 3.0 * size;
            (
                vec![[0.0, 0.0], [rc, 0.0], [rc, l], [0.0, l]],
                l,
                vec!["walls".into()],
            )
        }
        GeomKind::FuelChannel => {
            let (rp, h) = (0.25 * size, 2.0 * size);
            (
                vec![[rp, 0.0], [rc, 0.0], [rc, h], [rp, h]],
                h,
                vec!["pin".into(), "channel".into()],
            )
        }
        _ => {
            // Pebble: the channel rectangle with a half-disc of radius `rb`
            // cut out on the axis at mid-height (32 arc points).
            let (rb, h) = (0.3 * size, 1.5 * size);
            let zc = 0.5 * h;
            let mut prof = vec![
                [0.0, zc + rb],
                [0.0, h],
                [rc, h],
                [rc, 0.0],
                [0.0, 0.0],
                [0.0, zc - rb],
            ];
            for k in 1..32 {
                let a = -core::f64::consts::FRAC_PI_2 + core::f64::consts::PI * k as f64 / 32.0;
                prof.push([rb * a.cos(), zc + rb * a.sin()]);
            }
            (prof, h, vec!["pebble".into(), "channel".into()])
        }
    };
    let (p, t) = revolved_profile_surface(&profile, N_SEG);
    let tol = 1e-9 * size;
    let labels = t
        .iter()
        .map(|tri| {
            let (a, b, c) = (p[tri[0]], p[tri[1]], p[tri[2]]);
            let flat = (a.z - b.z).abs() < tol && (b.z - c.z).abs() < tol;
            let (cx, cy, cz) = (
                (a.x + b.x + c.x) / 3.0,
                (a.y + b.y + c.y) / 3.0,
                (a.z + b.z + c.z) / 3.0,
            );
            let r = (cx * cx + cy * cy).sqrt();
            match kind {
                _ if flat && cz.abs() < tol => "inlet",
                _ if flat && (cz - height).abs() < tol => "outlet",
                GeomKind::Pipe => "walls",
                GeomKind::FuelChannel => {
                    if r < 0.375 * size {
                        "pin"
                    } else {
                        "channel"
                    }
                }
                _ => {
                    if r < 0.45 * size {
                        "pebble"
                    } else {
                        "channel"
                    }
                }
            }
        })
        .collect();
    (p, t, labels, walls)
}

/// Optional surface-cleanup stage, run between authoring and meshing.
///
/// These are the mesh-quality operators ported from Blender in
/// `outram-blender`. They matter here specifically: the cfmesh pipeline
/// takes the surface as the truth about the geometry, and a warped or
/// concave face has no single well-defined normal, centroid or area for
/// it to work from.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Cleanup {
    /// Hand the authored surface over untouched.
    #[default]
    None,
    /// Split faces that are non-planar by more than 5 degrees
    /// ([`outram_blender::connect_nonplanar`]). Adds edges, moves
    /// nothing.
    SplitNonPlanar,
    /// Split concave faces into convex pieces
    /// ([`outram_blender::connect_concave`]). Adds edges, moves
    /// nothing.
    SplitConcave,
    /// Both splits, non-planar first.
    SplitBoth,
}

impl Cleanup {
    pub fn label(self) -> &'static str {
        match self {
            Cleanup::None => "None (author surface as-is)",
            Cleanup::SplitNonPlanar => "Split non-planar faces (>5 deg)",
            Cleanup::SplitConcave => "Split concave faces",
            Cleanup::SplitBoth => "Split non-planar, then concave",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Cleanup::None => "none",
            Cleanup::SplitNonPlanar => "nonplanar",
            Cleanup::SplitConcave => "concave",
            Cleanup::SplitBoth => "both",
        }
    }

    pub const ALL: [Cleanup; 4] = [
        Cleanup::None,
        Cleanup::SplitNonPlanar,
        Cleanup::SplitConcave,
        Cleanup::SplitBoth,
    ];
}

/// The immutable authoring + meshing parameters handed to the worker
/// thread, or to the headless driver.
#[derive(Clone, Debug)]
pub struct Params {
    pub kind: GeomKind,
    pub size: f64,
    pub cyl_radius: f64,
    pub cyl_height: f64,
    pub sphere_lat: usize,
    pub sphere_lon: usize,
    pub cleanup: Cleanup,
    pub opts: TetDualOptions,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            kind: GeomKind::Box,
            size: 2.0,
            cyl_radius: 2.0,
            cyl_height: 5.0,
            sphere_lat: 12,
            sphere_lon: 24,
            cleanup: Cleanup::None,
            opts: TetDualOptions {
                cell_size: 0.5,
                first_layer_thickness: 0.02,
                ..Default::default()
            },
        }
    }
}

/// A generated volume mesh plus its report, the mesh after each pipeline
/// stage, and the boundary-layer index of every cell (gh:#1765).
pub struct Built {
    pub mesh: VolumeMesh,
    pub report: TetDualReport,
    /// The mesh after each stage that ran, in pipeline order, ending with
    /// the finished mesh.
    pub stages: Vec<StageSnapshot>,
    /// Per cell of [`Self::mesh`]: 0 for core, k for the k-th layer from
    /// the wall.
    pub layer_index: LayerIndex,
    /// Face count of the surface actually handed to the mesher, after
    /// any [`Cleanup`]. Recorded so the CSV shows what cleanup did.
    pub surface_faces: usize,
}

/// Author the blender surface [`Mesh`] for `p`, before cleanup; `None`
/// for a reactor shape, whose surface is a revolved profile instead
/// ([`reactor_surface`]).
///
/// This is deterministic: the primitives and Catmull-Clark are pure
/// functions of the parameters, with no clock and no RNG.
pub fn author_surface(p: &Params) -> Option<Mesh> {
    match p.kind {
        GeomKind::Box => Some(cube(p.size)),
        GeomKind::Sphere => Some(uv_sphere(p.sphere_lon.max(3), p.sphere_lat.max(2), p.size)),
        GeomKind::Cylinder => Some(cylinder(24, p.cyl_radius, p.cyl_height)),
        // One Catmull-Clark level turns the 6-quad cube into a rounded,
        // still-closed genus-0 surface — a genuine authored
        // (non-primitive) mesh to feed the bridge.
        GeomKind::SubdividedCube => Some(catmull_clark(&cube(p.size), 1)),
        GeomKind::Pipe | GeomKind::FuelChannel | GeomKind::Pebble => None,
    }
}

/// Apply the chosen [`Cleanup`] to an authored surface.
pub fn clean_surface(surface: &Mesh, cleanup: Cleanup) -> Mesh {
    use outram_blender::connect_nonplanar::{connect_nonplanar, DEFAULT_ANGLE_LIMIT};
    match cleanup {
        Cleanup::None => surface.clone(),
        Cleanup::SplitNonPlanar => connect_nonplanar(surface, DEFAULT_ANGLE_LIMIT),
        Cleanup::SplitConcave => outram_blender::connect_concave::connect_concave(surface),
        Cleanup::SplitBoth => outram_blender::connect_concave::connect_concave(&connect_nonplanar(
            surface,
            DEFAULT_ANGLE_LIMIT,
        )),
    }
}

/// Author, clean, and volume-mesh — the whole model in one call, with
/// no GUI and no thread.
///
/// Meshes through cfMesh's **staged** route (gh:#1765), which keeps every
/// stage's mesh and numbers the layer cells. The finished mesh is
/// bit-identical to `outram_blender::foam_mesh::mesh_to_tet_dual`'s (the
/// staged route is pinned to the plain one in
/// `outram_park_fork_cfmesh::pipeline::staged`), so the committed headless
/// fixture did not move when this switched over.
pub fn build(p: &Params) -> Result<Built, String> {
    if p.kind.is_reactor() {
        // Reactor shapes (gh:#1768): a revolved profile with named patches,
        // layers grown on its walls only. Surface cleanup does not apply (it
        // works on blender meshes; these surfaces are already triangles).
        let (points, tris, labels, walls) = reactor_surface(p.kind, p.size);
        check_closed_manifold(&points, &tris).map_err(|e| e.to_string())?;
        let regions = SurfaceRegions::from_labels(&labels);
        // Corner feature points only (the `polyDualMesh` class's rule): with
        // every feature-edge point kept, the back-off grows no layer on a pipe
        // (cfMesh's border cap is 0 at a rim vertex collinear with its
        // neighbours; gh:#1240, recorded in the cfMesh port's AGENTS.md).
        let opts = TetDualOptions {
            layer_patches: walls,
            dual_feature_points: outram_park_fork_cfmesh::dual::FeaturePointRule::Corners,
            ..p.opts.clone()
        };
        let s = surface_to_tet_dual_mesh_multipatch_staged(&points, &tris, &regions, &opts)?;
        return Ok(Built {
            mesh: s.mesh,
            report: s.report,
            stages: s.stages,
            layer_index: s.layer_index,
            surface_faces: tris.len(),
        });
    }
    let authored = author_surface(p).ok_or("no blender surface for this shape")?;
    let surface = clean_surface(&authored, p.cleanup);
    let surface_faces = surface.face_count();
    let (points, tris) = mesh_to_surface(&surface).map_err(|e| e.to_string())?;
    let s = surface_to_tet_dual_mesh_staged(&points, &tris, &p.opts)?;
    Ok(Built {
        mesh: s.mesh,
        report: s.report,
        stages: s.stages,
        layer_index: s.layer_index,
        surface_faces,
    })
}
