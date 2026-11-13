//! Stage 0 feasibility gate (gh:#1154): a 3-D laminar pipe, meshed two ways
//! and solved by `PimpleFoam`, packaged so the SAME code runs natively and as
//! `wasm32-unknown-unknown` inside a Web Worker. Methodology, predictions and
//! results: `README.md` beside this crate. Pattern: `../wasm_worker_timing`.
//!
//! Two mesh routes, both ending in the neutral
//! `outram_blender::unstructured::UnstructuredMesh` and converted to the
//! solver's `FvMesh` by the same converter (`convert::foam::to_fv_mesh`):
//!
//! - **cfMesh:** the `outram-blender` cylinder primitive, triangulated and
//!   checked by `outram_blender::foam_mesh::mesh_to_surface`, through the cfMesh
//!   port's tet → dual → layers pipeline with named `inlet` / `outlet` /
//!   `walls` regions (`surface_to_tet_dual_mesh_multipatch`).
//! - **O-grid:** a generated blockMeshDict ([`ogrid_block_mesh_dict`]) through
//!   the blockMesh port. The outer ring has one block per circumferential cell,
//!   so every block edge is straight and every outer vertex lies on the circle;
//!   blockMesh's missing arc edges are never needed.
//!
//! No filesystem, no clock, no threads on any path the wasm build exports.
use std::cell::RefCell;
use std::f64::consts::PI;
use std::sync::Arc;

use outram_blender::foam_mesh::mesh_to_surface;
use outram_blender::primitives;
use outram_blender::unstructured::convert::{block_mesh, cfmesh, foam};
use outram_blender::unstructured::UnstructuredMesh;
use outram_foam_appbuilder_lib::io::control_dict::{ControlDict, StartControl, StopControl};
use outram_foam_appbuilder_lib::io::fv_schemes::FvSchemes;
use outram_foam_appbuilder_lib::io::fv_solution::FvSolution;
use outram_foam_appbuilder_lib::solvers::pimple_foam::PimpleFoam;
use outram_foam_basic_lib::fields::boundary::bc::PatchField;
use outram_foam_basic_lib::fields::field::Field;
use outram_foam_basic_lib::fields::vol_field::{VolScalarField, VolVectorField};
use outram_foam_basic_lib::mesh::FvMesh;
use outram_foam_basic_lib::primitives::Vector3;
use outram_foam_mesh::mesh_quality::{assess_quality, MeshQualityReport};
use outram_park_fork_cfmesh::patches::SurfaceRegions;
use outram_park_fork_cfmesh::pipeline::{surface_to_tet_dual_mesh_multipatch, TetDualOptions, TetDualReport};

/// Pipe diameter \[m\].
pub const D: f64 = 0.02;
/// Pipe radius \[m\].
pub const R: f64 = 0.5 * D;
/// Pipe length \[m\]; the pipe runs z ∈ \[0, L\], inlet at z = 0.
pub const L: f64 = 0.2;
/// Kinematic viscosity \[m²/s\], water-like.
pub const NU: f64 = 1.0e-6;
/// Uniform inlet velocity = bulk velocity \[m/s\]; Re = U D / ν = 100.
pub const U_IN: f64 = 5.0e-3;
/// Slab used for the developed-flow check, z ∈ \[Z_CHECK.0, Z_CHECK.1\] \[m\].
pub const Z_CHECK: (f64, f64) = (0.13, 0.17);
/// Hagen–Poiseuille developed kinematic pressure gradient 32 ν U / D² \[m/s²\].
pub const DPDZ_HP: f64 = 32.0 * NU * U_IN / (D * D);

/// O-grid resolution: `n` × `n` cells in the central square, `nr` radial
/// cells in the outer ring (4 n circumferential cells), `nz` axial cells.
/// Cell count is `(n² + 4 n nr) nz`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OGrid {
    /// Cells along each side of the central square.
    pub n: usize,
    /// Radial cells between the square and the wall.
    pub nr: usize,
    /// Axial cells.
    pub nz: usize,
}

impl OGrid {
    /// Number of cells the O-grid will have.
    pub fn n_cells(&self) -> usize {
        (self.n * self.n + 4 * self.n * self.nr) * self.nz
    }
}

/// Half-width of the O-grid's central square, as a fraction of R.
pub const SQUARE_FRAC: f64 = 0.55;

/// A blockMeshDict for a pipe of radius [`R`] and length [`L`] as an O-grid:
/// one central `n × n × nz` block and `4 n` outer blocks of `nr × 1 × nz`,
/// each spanning one circumferential cell. Patches `inlet` (z = 0), `outlet`
/// (z = L) and `walls`.
pub fn ogrid_block_mesh_dict(g: &OGrid) -> String {
    let n4 = 4 * g.n;
    let s = SQUARE_FRAC * R;
    let corners = [(-s, -s), (s, -s), (s, s), (-s, s)];
    let sq_xy = |k: usize| {
        let side = (k / g.n) % 4;
        let t = (k % g.n) as f64 / g.n as f64;
        let (a, b) = (corners[side], corners[(side + 1) % 4]);
        (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1))
    };
    let ci_xy = |k: usize| {
        let th = -0.75 * PI + 2.0 * PI * k as f64 / n4 as f64;
        (R * th.cos(), R * th.sin())
    };
    // Vertex indices: per z level, 4n square points then 4n circle points.
    let sq = |k: usize, zl: usize| zl * 2 * n4 + (k % n4);
    let ci = |k: usize, zl: usize| zl * 2 * n4 + n4 + (k % n4);

    let mut d = String::from("convertToMeters 1;\nvertices\n(\n");
    for zl in 0..2 {
        let z = if zl == 0 { 0.0 } else { L };
        for k in 0..n4 {
            let (x, y) = sq_xy(k);
            d.push_str(&format!("  ({x:.17e} {y:.17e} {z:.17e})\n"));
        }
        for k in 0..n4 {
            let (x, y) = ci_xy(k);
            d.push_str(&format!("  ({x:.17e} {y:.17e} {z:.17e})\n"));
        }
    }
    d.push_str(");\nblocks\n(\n");
    let c = [sq(0, 0), sq(g.n, 0), sq(2 * g.n, 0), sq(3 * g.n, 0)];
    let ct = [sq(0, 1), sq(g.n, 1), sq(2 * g.n, 1), sq(3 * g.n, 1)];
    d.push_str(&format!(
        "  hex ({} {} {} {} {} {} {} {}) ({} {} {}) simpleGrading (1 1 1)\n",
        c[0], c[1], c[2], c[3], ct[0], ct[1], ct[2], ct[3], g.n, g.n, g.nz
    ));
    let outer = |k: usize| {
        [sq(k, 0), ci(k, 0), ci(k + 1, 0), sq(k + 1, 0), sq(k, 1), ci(k, 1), ci(k + 1, 1), sq(k + 1, 1)]
    };
    for k in 0..n4 {
        let v = outer(k);
        d.push_str(&format!(
            "  hex ({} {} {} {} {} {} {} {}) ({} 1 {}) simpleGrading (1 1 1)\n",
            v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], g.nr, g.nz
        ));
    }
    d.push_str(");\nedges\n(\n);\nboundary\n(\n");
    let face = |q: [usize; 4]| format!("    ({} {} {} {})\n", q[0], q[1], q[2], q[3]);
    let mut inlet = face([c[0], c[3], c[2], c[1]]);
    let mut outlet = face([ct[0], ct[1], ct[2], ct[3]]);
    let mut walls = String::new();
    for k in 0..n4 {
        let v = outer(k);
        inlet.push_str(&face([v[0], v[3], v[2], v[1]]));
        outlet.push_str(&face([v[4], v[5], v[6], v[7]]));
        walls.push_str(&face([v[1], v[2], v[6], v[5]]));
    }
    for (name, kind, faces) in [("inlet", "patch", inlet), ("outlet", "patch", outlet), ("walls", "wall", walls)] {
        d.push_str(&format!("  {name}\n  {{\n    type {kind};\n    faces\n    (\n{faces}    );\n  }}\n"));
    }
    d.push_str(");\n");
    d
}

/// The O-grid pipe as a neutral mesh (blockMesh port).
pub fn mesh_ogrid(g: &OGrid) -> UnstructuredMesh {
    block_mesh::block_mesh(&ogrid_block_mesh_dict(g)).expect("blockMesh O-grid")
}

/// The cfMesh pipe as a neutral mesh: blender cylinder (`n_seg` sides, axis
/// z, moved to z ∈ \[0, L\]) → closed-manifold check → tet → dual → layers,
/// with each surface triangle labelled `inlet` / `outlet` / `walls` by where
/// its centroid lies.
pub fn mesh_cfmesh(cell_size: f64, n_layers: usize, n_seg: usize) -> (UnstructuredMesh, TetDualReport) {
    let cyl = primitives::cylinder(n_seg, R, L);
    let (mut pts, tris) = mesh_to_surface(&cyl).expect("closed cylinder surface");
    for p in pts.iter_mut() {
        p.z += 0.5 * L;
    }
    let labels: Vec<&str> = tris
        .iter()
        .map(|t| {
            let zc = (pts[t[0]].z + pts[t[1]].z + pts[t[2]].z) / 3.0;
            let flat = (pts[t[0]].z - pts[t[1]].z).abs() < 1e-12 && (pts[t[1]].z - pts[t[2]].z).abs() < 1e-12;
            if flat && zc < 0.5 * L {
                "inlet"
            } else if flat {
                "outlet"
            } else {
                "walls"
            }
        })
        .collect();
    let regions = SurfaceRegions::from_labels(&labels);
    let opts = TetDualOptions {
        cell_size,
        n_layers,
        first_layer_thickness: 0.1 * cell_size,
        ..TetDualOptions::default()
    };
    let (vm, report) = surface_to_tet_dual_mesh_multipatch(&pts, &tris, &regions, &opts).expect("cfMesh pipe");
    (cfmesh::from_volume_mesh(&vm).expect("cfMesh -> neutral"), report)
}

/// checkMesh-style quality of the mesh the solver will get (the same
/// neutral → polyMesh conversion as [`to_fv`]).
pub fn quality(m: &UnstructuredMesh) -> MeshQualityReport {
    assess_quality(&foam::to_poly_mesh(m).expect("polyMesh"))
}

/// Neutral mesh → solver `FvMesh`.
pub fn to_fv(m: &UnstructuredMesh) -> Arc<FvMesh> {
    Arc::new(foam::to_fv_mesh(m).expect("FvMesh"))
}

/// The pipe case on `mesh`: `inlet` fixed U = (0, 0, U_IN) and zeroGradient
/// p, kinematic viscosity `nu` \[m²/s\] ([`NU`] for Re = 100); `outlet`
/// zeroGradient U and fixed p = 0; every other patch a no-slip
/// wall with zeroGradient p. Starts from rest. PISO: 1 outer, 2 correctors;
/// Euler, upwind (the `PimpleFoam` defaults).
pub fn build_case(mesh: Arc<FvMesh>, dt: f64, nu: f64) -> PimpleFoam {
    let control = ControlDict {
        start: StartControl::StartTime(0.0),
        stop: StopControl::EndTime(1.0e9),
        delta_t: dt,
        ..ControlDict::default()
    };
    let mut solution = FvSolution::default();
    solution.pimple.n_outer_correctors = 1;
    solution.pimple.n_correctors = 2;
    let mut s = PimpleFoam::new(mesh.clone(), control, FvSchemes::default(), solution);
    let n = mesh.n_cells;
    let mut ub = Vec::new();
    let mut pb = Vec::new();
    for p in &mesh.patches {
        match p.name.as_str() {
            "inlet" => {
                ub.push(PatchField::fixed_value_vec(p.size, Vector3::new(0.0, 0.0, U_IN)));
                pb.push(PatchField::zero_gradient(p.size));
            }
            "outlet" => {
                ub.push(PatchField::zero_gradient_vec(p.size));
                pb.push(PatchField::fixed_value(p.size, 0.0));
            }
            _ => {
                ub.push(PatchField::fixed_value_vec(p.size, Vector3::ZERO));
                pb.push(PatchField::zero_gradient(p.size));
            }
        }
    }
    s.u = VolVectorField::new("U", mesh.clone(), Field::new(vec![Vector3::ZERO; n]), ub);
    s.p = VolScalarField::new("p", mesh.clone(), Field::new(vec![0.0; n]), pb);
    s.nu = VolScalarField::uniform("nu", mesh, nu);
    s
}

/// Volume-weighted least-squares fit `y ≈ a + b x` over the samples; returns
/// `(a, b)`. Used for both the centreline (u_z against r²) and the pressure
/// gradient (p against z).
pub fn fit_line(samples: &[(f64, f64, f64)]) -> (f64, f64) {
    let (mut sw, mut sx, mut sy, mut sxx, mut sxy) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for &(x, y, w) in samples {
        sw += w;
        sx += w * x;
        sy += w * y;
        sxx += w * x * x;
        sxy += w * x * y;
    }
    let det = sw * sxx - sx * sx;
    let b = (sw * sxy - sx * sy) / det;
    let a = (sy - b * sx) / sw;
    (a, b)
}

/// Centreline axial velocity at the slab `|z − z0| < hz`: the intercept of a
/// u_z = a + b r² fit over the cells with r < R/2 (exact for a parabola, so no
/// averaging bias toward the wall).
pub fn centreline_u(s: &PimpleFoam, z0: f64, hz: f64) -> f64 {
    let m = &s.mesh;
    let u = s.u.internal.as_slice();
    let pts: Vec<(f64, f64, f64)> = (0..m.n_cells)
        .filter_map(|c| {
            let x = m.cell_centres[c];
            let r2 = x.x * x.x + x.y * x.y;
            ((x.z - z0).abs() < hz && r2 < 0.25 * R * R).then(|| (r2, u[c].z, m.cell_volumes[c]))
        })
        .collect();
    fit_line(&pts).0
}

/// Axial kinematic pressure gradient in the check slab ([`Z_CHECK`]): slope
/// of a volume-weighted p-against-z fit over every cell in it \[m/s²\].
pub fn dpdz(s: &PimpleFoam) -> f64 {
    let m = &s.mesh;
    let p = s.p.internal.as_slice();
    let pts: Vec<(f64, f64, f64)> = (0..m.n_cells)
        .filter_map(|c| {
            let z = m.cell_centres[c].z;
            (z >= Z_CHECK.0 && z <= Z_CHECK.1).then(|| (z, p[c], m.cell_volumes[c]))
        })
        .collect();
    fit_line(&pts).1
}

/// Net face flux through the named patch as the solver stores it in `phi`
/// \[m³/s\] (outward positive).
pub fn patch_flux(s: &PimpleFoam, name: &str) -> f64 {
    s.mesh
        .patches
        .iter()
        .position(|p| p.name == name)
        .map(|i| s.phi.boundary[i].values.as_slice().iter().sum())
        .unwrap_or(0.0)
}

/// Worst cell continuity error, max over cells of |Σ_f φ_f| (internal faces
/// from `phi`, boundary faces from `phi`'s boundary values), divided by the
/// inlet flow rate.
pub fn max_continuity_error(s: &PimpleFoam) -> f64 {
    let m = &s.mesh;
    let mut div = vec![0.0; m.n_cells];
    let phi = s.phi.internal.as_slice();
    for f in 0..m.n_internal_faces {
        div[m.owner[f]] += phi[f];
        div[m.neighbour[f]] -= phi[f];
    }
    for (pi, p) in m.patches.iter().enumerate() {
        for fi in 0..p.size {
            div[m.owner[p.start + fi]] += s.phi.boundary[pi].values[fi];
        }
    }
    let q = -patch_flux(s, "inlet");
    div.iter().fold(0.0_f64, |a, d| a.max(d.abs())) / q
}

/// Physical time after which `xs` (pairs of time and value) stays within
/// `tol` (relative) of its last value; 0 if it always did.
pub fn settling_time(xs: &[(f64, f64)], tol: f64) -> f64 {
    let last = xs.last().map_or(0.0, |x| x.1);
    xs.iter().rev().find(|x| (x.1 - last).abs() > tol * last.abs()).map_or(0.0, |x| x.0)
}

/// One step; returns the steady-state metric: the largest cell velocity
/// change, relative to U_IN, per flow-through time L / U_IN.
pub fn step(s: &mut PimpleFoam) -> f64 {
    let before = s.u.internal.as_slice().to_vec();
    s.step().expect("PimpleFoam step");
    let after = s.u.internal.as_slice();
    let du = before.iter().zip(after).fold(0.0_f64, |a, (b, c)| a.max((*c - *b).mag()));
    du / U_IN * (L / U_IN) / s.control.delta_t
}

// ── wasm / native C-ABI surface: one case at a time ────────────────────────

#[derive(Default)]
struct State {
    mesh: Option<UnstructuredMesh>,
    fv: Option<Arc<FvMesh>>,
    q: Option<MeshQualityReport>,
    case: Option<PimpleFoam>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// Build the O-grid mesh; returns its cell count.
#[no_mangle]
pub extern "C" fn pipe_mesh_ogrid(n: u32, nr: u32, nz: u32) -> u32 {
    let m = mesh_ogrid(&OGrid { n: n as usize, nr: nr as usize, nz: nz as usize });
    let c = m.n_cells() as u32;
    STATE.with(|s| *s.borrow_mut() = State { mesh: Some(m), ..State::default() });
    c
}

/// Build the cfMesh mesh; returns its cell count.
#[no_mangle]
pub extern "C" fn pipe_mesh_cfmesh(cell_size: f64, n_layers: u32, n_seg: u32) -> u32 {
    let (m, _report) = mesh_cfmesh(cell_size, n_layers as usize, n_seg as usize);
    let c = m.n_cells() as u32;
    STATE.with(|s| *s.borrow_mut() = State { mesh: Some(m), ..State::default() });
    c
}

/// Neutral → `FvMesh`; returns the cell count.
#[no_mangle]
pub extern "C" fn pipe_to_fv() -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let fv = to_fv(s.mesh.as_ref().expect("mesh first"));
        let c = fv.n_cells as u32;
        s.fv = Some(fv);
        c
    })
}

/// Mesh quality: 0 max non-orthogonality \[deg\], 1 max skewness, 2 negative
/// cells, 3 total volume \[m³\], 4 mean non-orthogonality \[deg\],
/// 5 max aspect ratio.
#[no_mangle]
pub extern "C" fn pipe_quality(i: u32) -> f64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.q.is_none() {
            let q = quality(s.mesh.as_ref().expect("mesh first"));
            s.q = Some(q);
        }
        let q = s.q.as_ref().expect("quality");
        match i {
            0 => q.max_non_ortho_deg,
            1 => q.max_skewness,
            2 => q.n_negative_volume_cells as f64,
            3 => q.total_volume,
            4 => q.mean_non_ortho_deg,
            _ => q.max_aspect_ratio,
        }
    })
}

/// Build the PimpleFoam case on the `FvMesh` at time step `dt` \[s\].
#[no_mangle]
pub extern "C" fn pipe_case_new(dt: f64) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let fv = s.fv.clone().expect("fv first");
        let c = fv.n_cells as u32;
        s.case = Some(build_case(fv, dt, NU));
        c
    })
}

/// As [`pipe_case_new`] with viscosity `nu` \[m²/s\] (Re = U D / nu): the
/// entry-length control of the README.
#[no_mangle]
pub extern "C" fn pipe_case_new_nu(dt: f64, nu: f64) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let fv = s.fv.clone().expect("fv first");
        let c = fv.n_cells as u32;
        s.case = Some(build_case(fv, dt, nu));
        c
    })
}

/// Take `n` steps; returns the steady metric of the last one.
#[no_mangle]
pub extern "C" fn pipe_steps(n: u32) -> f64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let case = s.case.as_mut().expect("case first");
        let mut r = 0.0;
        for _ in 0..n {
            r = step(case);
        }
        r
    })
}

/// Diagnostics: 0 centreline u / 2U at z = 0.15 m, 1 dp/dz / (32 ν U / D², the
/// case's ν) in
/// the check slab, 2 outlet flux / inlet flux, 3 max cell continuity error /
/// inlet flux, 4 centreline u / 2U at z = 0.17 m, 5 pressure drop: the mean
/// kinematic p of the cells with z < 1 cm (outlet p = 0) \[m²/s²\].
#[no_mangle]
pub extern "C" fn pipe_diag(i: u32) -> f64 {
    STATE.with(|s| {
        let s = s.borrow();
        let c = s.case.as_ref().expect("case first");
        match i {
            0 => centreline_u(c, 0.15, 0.01) / (2.0 * U_IN),
            1 => dpdz(c) / -(DPDZ_HP * c.nu.internal[0] / NU),
            2 => patch_flux(c, "outlet") / -patch_flux(c, "inlet"),
            3 => max_continuity_error(c),
            4 => centreline_u(c, 0.17, 0.01) / (2.0 * U_IN),
            _ => {
                let m = &c.mesh;
                let p = c.p.internal.as_slice();
                let v: Vec<f64> = (0..m.n_cells).filter(|&k| m.cell_centres[k].z < 1e-2).map(|k| p[k]).collect();
                v.iter().sum::<f64>() / v.len().max(1) as f64
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The O-grid dict builds a closed mesh with the predicted cell count, no
    /// negative cells, and the volume of the inscribed 4n-gon prism
    /// (n_sides/2 · R² sin(2π/n_sides) · L) to round-off, since every outer
    /// vertex is on the circle and every face is planar.
    #[test]
    fn ogrid_has_the_polygon_volume() {
        let g = OGrid { n: 4, nr: 2, nz: 3 };
        let m = mesh_ogrid(&g);
        assert_eq!(m.n_cells(), g.n_cells());
        let q = quality(&m);
        assert_eq!(q.n_negative_volume_cells, 0);
        let sides = (4 * g.n) as f64;
        let v = 0.5 * sides * R * R * (2.0 * PI / sides).sin() * L;
        assert!(((q.total_volume - v) / v).abs() < 1e-12, "{} vs {v}", q.total_volume);
        let fv = to_fv(&m);
        let names: Vec<&str> = fv.patches.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["inlet", "outlet", "walls"]);
    }

    /// Settling: the last time the series is outside the band.
    #[test]
    fn settling_time_finds_the_last_excursion() {
        let xs = [(1.0, 0.5), (2.0, 0.97), (3.0, 1.005), (4.0, 1.0)];
        assert_eq!(settling_time(&xs, 0.01), 2.0);
        assert_eq!(settling_time(&xs[2..], 0.01), 0.0);
    }

    /// The fit recovers an exact line.
    #[test]
    fn fit_line_is_exact_on_a_line() {
        let s: Vec<(f64, f64, f64)> = (0..5).map(|i| (i as f64, 3.0 - 2.0 * i as f64, 1.0 + i as f64)).collect();
        let (a, b) = fit_line(&s);
        assert!((a - 3.0).abs() < 1e-12 && (b + 2.0).abs() < 1e-12);
    }

    /// A few steps on a coarse O-grid conserve mass through the pipe: the
    /// outlet flux matches the inlet flux to 1 %.
    #[test]
    fn coarse_pipe_conserves_mass() {
        let fv = to_fv(&mesh_ogrid(&OGrid { n: 4, nr: 2, nz: 10 }));
        let mut c = build_case(fv, 0.5, NU);
        for _ in 0..5 {
            step(&mut c);
        }
        let ratio = patch_flux(&c, "outlet") / -patch_flux(&c, "inlet");
        assert!((ratio - 1.0).abs() < 1e-2, "outlet/inlet flux {ratio}");
    }

    /// The cfMesh route yields a closed, positive mesh with the three named
    /// patches, and the C-ABI sequence the Worker calls runs end to end on a
    /// coarse O-grid with finite diagnostics and conserved mass.
    #[test]
    fn cfmesh_route_and_c_abi_sequence_run() {
        let (m, _) = mesh_cfmesh(0.008, 0, 24);
        let q = quality(&m);
        assert!(m.n_cells() > 0 && q.n_negative_volume_cells == 0);
        let mut names: Vec<String> = m.patches().iter().map(|p| p.name.clone()).collect();
        names.sort();
        assert_eq!(names, ["inlet", "outlet", "walls"]);

        assert_eq!(pipe_mesh_ogrid(4, 2, 10), 480);
        assert_eq!(pipe_to_fv(), 480);
        assert!(pipe_quality(0) < 45.0 && pipe_quality(2) == 0.0);
        pipe_case_new_nu(0.5, 4.0 * NU);
        assert!(pipe_steps(3).is_finite());
        assert!((pipe_diag(2) - 1.0).abs() < 1e-9);
        assert!(pipe_diag(3) < 1e-9, "continuity {}", pipe_diag(3));
        for i in 0..6 {
            assert!(pipe_diag(i).is_finite(), "diag {i}");
        }
    }
}
