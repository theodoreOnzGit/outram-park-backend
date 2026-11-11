//! Structured hex meshes of simple primitives, first a **pipe** (cylinder) and a
//! **box**, generated as `blockMeshDict` text and built by this crate's
//! [`block_mesh`](crate::block_mesh) port (gh:#1155, Foam Studio stage 1).
//!
//! # The pipe O-grid
//!
//! A central `n × n` block (half-width `square_frac · R`) and `4 n` outer
//! blocks of `nr × 1` cells, one per circumferential cell. Every block edge is
//! straight and every outer vertex lies on the circle, so the wall is a
//! `4n`-gon inscribed in it: exactly what any finite-volume mesh of a circle
//! is, and blockMesh's missing arc edges are never needed. Moved here from the
//! stage 0 harness (`outram-foam-appbuilder-lib/verification_and_validation/
//! wasm_pimple_3d_pipe`, gh:#1154), where it was measured: max
//! non-orthogonality 19.7 / 26.0 / 29.7° at 5k / 20k / 50k cells, no negative
//! cells, volume equal to the inscribed-polygon prism to round-off.
//!
//! **Why this crate.** The blockMesh port lives here, and `outram-blender`'s
//! `convert::block_mesh` only calls it; `outram_blender::foam_mesh` triangulates
//! surfaces for cfMesh, which gives wrong pipe flow (gh:#1160). A generator
//! that writes a blockMeshDict belongs beside the mesher that reads it, and it
//! keeps the solver path free of `outram-blender`.
//!
//! # The annulus and the double pipe (gh:#1156)
//!
//! [`Annulus`]: `n_theta` blocks of `nr × 1 × nz` cells between two
//! polygons, inner radius `r_inner` and outer `r_outer`, every vertex on its
//! circle at the O-grid's angles `θ_k = −3π/4 + 2πk/n_theta`. With
//! `n_theta = 4n` and `r_inner` equal to a [`PipeOGrid`]'s radius, the
//! annulus's inner wall has exactly the pipe's wall vertices, so the two
//! meshes meet face to face (conformal), which is what chtMultiRegionFoam's
//! coupled interface needs. [`DoublePipe`] builds the three regions of a
//! double-pipe heat exchanger that way: the inner fluid (an O-grid), the
//! tube wall (an annulus) and the outer fluid (an annulus), all with `4n`
//! cells around and the same axial layers.
//!
//! # The helical pipe (gh:#1157)
//!
//! [`HelicalPipe`]: a [`PipeOGrid`] as long as the coil's tube, its points
//! mapped onto the helix on a rotation-minimising frame (no twist between
//! layers); same topology and cell order as the straight grid. Not a
//! blockMeshDict: blockMesh's straight block edges cannot follow a helix
//! without one block per layer, and the point map is exact.
//!
//! # The box
//!
//! One hex block, `nx × ny × nz`, axis-aligned.
//!
//! # Patches
//!
//! Each primitive face carries a patch name chosen by the caller. Faces with
//! the same name go into one patch; a name containing `wall` gets
//! `type wall`, any other `type patch`. Patches appear in first-use order.
use crate::block_mesh::{block_mesh, PolyMesh};
use crate::MeshError;
use petir::mathf::RealMath;
use uom::si::f64::Length;
use uom::si::length::meter;

/// Fraction of R used by default for the central square's half-width.
pub const DEFAULT_SQUARE_FRAC: f64 = 0.55;

/// A circular pipe along +z, as an O-grid.
#[derive(Debug, Clone, PartialEq)]
pub struct PipeOGrid {
    /// Pipe radius.
    pub radius: Length,
    /// Pipe length; the pipe runs from `origin.z` to `origin.z + length`.
    pub length: Length,
    /// Centre of the `z`-minimum end face.
    pub origin: [Length; 3],
    /// Cells along each side of the central square (4 n around the wall).
    pub n: usize,
    /// Radial cells between the square and the wall.
    pub nr: usize,
    /// Axial cells.
    pub nz: usize,
    /// Half-width of the central square as a fraction of the radius, in (0, 1/√2).
    pub square_frac: f64,
    /// Patch names of the `z`-min end, the `z`-max end and the side wall.
    pub patch_names: [String; 3],
}

impl PipeOGrid {
    /// A pipe at the origin with patches `inlet` (z-min), `outlet` (z-max) and
    /// `walls`, and the default square fraction.
    pub fn new(radius: Length, length: Length, n: usize, nr: usize, nz: usize) -> Self {
        let zero = Length::new::<meter>(0.0);
        Self {
            radius,
            length,
            origin: [zero, zero, zero],
            n,
            nr,
            nz,
            square_frac: DEFAULT_SQUARE_FRAC,
            patch_names: ["inlet".into(), "outlet".into(), "walls".into()],
        }
    }

    /// The geometrically similar family used for grid studies and the demo:
    /// `(n, nr, nz) = (2m, 2m, 16m · L/(10 D))`, i.e. 320 m³ cells at
    /// L/D = 10 (m = 4 is 20 480). The axial count is rounded to at least 1.
    pub fn level(radius: Length, length: Length, m: usize) -> Self {
        let ld = length.get::<meter>() / (2.0 * radius.get::<meter>());
        let nz = ((16.0 * m as f64 * ld / 10.0).round() as usize).max(1);
        Self::new(radius, length, 2 * m, 2 * m, nz)
    }

    /// Number of cells: `(n² + 4 n nr) nz`.
    pub fn n_cells(&self) -> usize {
        (self.n * self.n + 4 * self.n * self.nr) * self.nz
    }

    /// The cross-section area of the inscribed `4n`-gon, `½ (4n) R² sin(2π/4n)`.
    pub fn polygon_area(&self) -> f64 {
        let r = self.radius.get::<meter>();
        let k = (4 * self.n) as f64;
        0.5 * k * r * r * (2.0 * std::f64::consts::PI / k).r_sin()
    }

    /// The blockMeshDict text.
    pub fn block_mesh_dict(&self) -> String {
        let r = self.radius.get::<meter>();
        let l = self.length.get::<meter>();
        let [ox, oy, oz] = self.origin.map(|v| v.get::<meter>());
        let n = self.n.max(1);
        let n4 = 4 * n;
        let s = self.square_frac * r;
        let corners = [(-s, -s), (s, -s), (s, s), (-s, s)];
        let sq_xy = |k: usize| {
            let side = (k / n) % 4;
            let t = (k % n) as f64 / n as f64;
            let (a, b) = (corners[side], corners[(side + 1) % 4]);
            (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1))
        };
        let ci_xy = |k: usize| {
            let th = -0.75 * std::f64::consts::PI + 2.0 * std::f64::consts::PI * k as f64 / n4 as f64;
            (r * th.r_cos(), r * th.r_sin())
        };
        let sq = |k: usize, zl: usize| zl * 2 * n4 + (k % n4);
        let ci = |k: usize, zl: usize| zl * 2 * n4 + n4 + (k % n4);

        // The central square is n x n single-cell-wide blocks (not one n x n
        // block), so that every block face the outer blocks touch is a face
        // of one central block: upstream `blockMesh` merges points only
        // across block faces the topology shares (gh:#1464), so a coarse
        // central block against n outer blocks would not be merged. The
        // interior vertices are the bilinear positions the single block's
        // map produced, so the mesh is geometrically the same.
        let n_in = (n - 1) * (n - 1);
        let base = 2 * 2 * n4;
        let gi = |i: usize, j: usize, zl: usize| -> usize {
            if j == 0 {
                sq(i, zl)
            } else if i == n {
                sq(n + j, zl)
            } else if j == n {
                sq(2 * n + (n - i), zl)
            } else if i == 0 {
                sq(3 * n + (n - j), zl)
            } else {
                base + zl * n_in + (i - 1) + (j - 1) * (n - 1)
            }
        };
        let mut d = String::from("convertToMeters 1;\nvertices\n(\n");
        for zl in 0..2 {
            let z = oz + if zl == 0 { 0.0 } else { l };
            for k in 0..n4 {
                let (x, y) = sq_xy(k);
                d.push_str(&format!("  ({:.17e} {:.17e} {z:.17e})\n", x + ox, y + oy));
            }
            for k in 0..n4 {
                let (x, y) = ci_xy(k);
                d.push_str(&format!("  ({:.17e} {:.17e} {z:.17e})\n", x + ox, y + oy));
            }
        }
        for zl in 0..2 {
            let z = oz + if zl == 0 { 0.0 } else { l };
            for j in 1..n {
                for i in 1..n {
                    let x = -s + 2.0 * s * (i as f64 / n as f64);
                    let y = -s + 2.0 * s * (j as f64 / n as f64);
                    d.push_str(&format!("  ({:.17e} {:.17e} {z:.17e})\n", x + ox, y + oy));
                }
            }
        }
        d.push_str(");\nblocks\n(\n");
        let mut lo = Vec::new();
        let mut hi = Vec::new();
        for j in 0..n {
            for i in 0..n {
                let b = [
                    gi(i, j, 0),
                    gi(i + 1, j, 0),
                    gi(i + 1, j + 1, 0),
                    gi(i, j + 1, 0),
                    gi(i, j, 1),
                    gi(i + 1, j, 1),
                    gi(i + 1, j + 1, 1),
                    gi(i, j + 1, 1),
                ];
                d.push_str(&format!(
                    "  hex ({} {} {} {} {} {} {} {}) (1 1 {}) simpleGrading (1 1 1)\n",
                    b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], self.nz
                ));
                lo.push([b[0], b[3], b[2], b[1]]);
                hi.push([b[4], b[5], b[6], b[7]]);
            }
        }
        let outer = |k: usize| [sq(k, 0), ci(k, 0), ci(k + 1, 0), sq(k + 1, 0), sq(k, 1), ci(k, 1), ci(k + 1, 1), sq(k + 1, 1)];
        for k in 0..n4 {
            let v = outer(k);
            d.push_str(&format!(
                "  hex ({} {} {} {} {} {} {} {}) ({} 1 {}) simpleGrading (1 1 1)\n",
                v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], self.nr, self.nz
            ));
        }
        let mut side = Vec::new();
        for k in 0..n4 {
            let v = outer(k);
            lo.push([v[0], v[3], v[2], v[1]]);
            hi.push([v[4], v[5], v[6], v[7]]);
            side.push([v[1], v[2], v[6], v[5]]);
        }
        d.push_str(");\nedges\n(\n);\n");
        d.push_str(&boundary_text(&[(&self.patch_names[0], lo), (&self.patch_names[1], hi), (&self.patch_names[2], side)]));
        d
    }

    /// Build the mesh.
    ///
    /// # Errors
    /// Whatever [`block_mesh`] returns; a valid `PipeOGrid` (n, nr, nz ≥ 1,
    /// positive radius and length) does not fail.
    pub fn mesh(&self) -> Result<PolyMesh, MeshError> {
        block_mesh(&self.block_mesh_dict())
    }
}

/// A circular annulus along +z between radii `r_inner` and `r_outer`, as
/// `n_theta` blocks of `nr × 1 × nz` cells; see the module docs for the
/// vertex angles (shared with [`PipeOGrid`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Annulus {
    /// Inner radius (the inner polygon's vertices lie on this circle).
    pub r_inner: Length,
    /// Outer radius.
    pub r_outer: Length,
    /// Length; the annulus runs from `origin.z` to `origin.z + length`.
    pub length: Length,
    /// Centre of the `z`-minimum end.
    pub origin: [Length; 3],
    /// Cells around (4 n to meet a [`PipeOGrid`] of side n).
    pub n_theta: usize,
    /// Radial cells.
    pub nr: usize,
    /// Axial cells.
    pub nz: usize,
    /// Patch names of the `z`-min end, the `z`-max end, the inner wall and
    /// the outer wall.
    pub patch_names: [String; 4],
}

impl Annulus {
    /// An annulus at the origin with patches `inlet`, `outlet`, `inner_wall`
    /// and `outer_wall`.
    pub fn new(r_inner: Length, r_outer: Length, length: Length, n_theta: usize, nr: usize, nz: usize) -> Self {
        let zero = Length::new::<meter>(0.0);
        Self {
            r_inner,
            r_outer,
            length,
            origin: [zero, zero, zero],
            n_theta,
            nr,
            nz,
            patch_names: ["inlet".into(), "outlet".into(), "inner_wall".into(), "outer_wall".into()],
        }
    }

    /// Number of cells: `n_theta · nr · nz`.
    pub fn n_cells(&self) -> usize {
        self.n_theta * self.nr * self.nz
    }

    /// Cross-section area between the two inscribed `n_theta`-gons.
    pub fn polygon_area(&self) -> f64 {
        let k = self.n_theta.max(3) as f64;
        let s = 0.5 * k * (2.0 * std::f64::consts::PI / k).r_sin();
        let (a, b) = (self.r_inner.get::<meter>(), self.r_outer.get::<meter>());
        s * (b * b - a * a)
    }

    /// The blockMeshDict text.
    pub fn block_mesh_dict(&self) -> String {
        let (ri, ro) = (self.r_inner.get::<meter>(), self.r_outer.get::<meter>());
        let l = self.length.get::<meter>();
        let [ox, oy, oz] = self.origin.map(|v| v.get::<meter>());
        let nt = self.n_theta.max(3);
        let xy = |r: f64, k: usize| {
            let th = -0.75 * std::f64::consts::PI + 2.0 * std::f64::consts::PI * (k % nt) as f64 / nt as f64;
            (r * th.r_cos(), r * th.r_sin())
        };
        let vi = |k: usize, zl: usize| zl * 2 * nt + (k % nt);
        let vo = |k: usize, zl: usize| zl * 2 * nt + nt + (k % nt);
        let mut d = String::from("convertToMeters 1;\nvertices\n(\n");
        for zl in 0..2 {
            let z = oz + if zl == 0 { 0.0 } else { l };
            for r in [ri, ro] {
                for k in 0..nt {
                    let (x, y) = xy(r, k);
                    d.push_str(&format!("  ({:.17e} {:.17e} {z:.17e})\n", x + ox, y + oy));
                }
            }
        }
        d.push_str(");\nblocks\n(\n");
        let hexv = |k: usize| [vi(k, 0), vo(k, 0), vo(k + 1, 0), vi(k + 1, 0), vi(k, 1), vo(k, 1), vo(k + 1, 1), vi(k + 1, 1)];
        let (mut lo, mut hi, mut inner, mut outer) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for k in 0..nt {
            let v = hexv(k);
            d.push_str(&format!(
                "  hex ({} {} {} {} {} {} {} {}) ({} 1 {}) simpleGrading (1 1 1)\n",
                v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], self.nr.max(1), self.nz.max(1)
            ));
            lo.push([v[0], v[3], v[2], v[1]]);
            hi.push([v[4], v[5], v[6], v[7]]);
            inner.push([v[0], v[4], v[7], v[3]]);
            outer.push([v[1], v[2], v[6], v[5]]);
        }
        d.push_str(");\nedges\n(\n);\n");
        let p = &self.patch_names;
        d.push_str(&boundary_text(&[(&p[0], lo), (&p[1], hi), (&p[2], inner), (&p[3], outer)]));
        d
    }

    /// Build the mesh.
    ///
    /// # Errors
    /// Whatever [`block_mesh`] returns.
    pub fn mesh(&self) -> Result<PolyMesh, MeshError> {
        block_mesh(&self.block_mesh_dict())
    }
}

/// The three conformal region meshes of a double-pipe heat exchanger along
/// +z: the inner fluid (`r < r_tube_inner`), the tube wall
/// (`r_tube_inner < r < r_tube_outer`) and the outer (annulus) fluid
/// (`r_tube_outer < r < r_shell`), each with `4 n` cells around and `nz`
/// axial layers, so every interface is face to face.
///
/// Patch names (the defaults; the interface patch has the same name on both
/// sides):
///
/// | region | z-min | z-max | inner | outer |
/// |---|---|---|---|---|
/// | inner fluid | `inlet` | `outlet` | — | `tube_inner_wall` |
/// | tube wall | `tube_ends` | `tube_ends` | `tube_inner_wall` | `tube_outer_wall` |
/// | outer fluid | `outlet` | `inlet` | `tube_outer_wall` | `shell_wall` |
///
/// The outer fluid's inlet is at z-max: counterflow.
#[derive(Debug, Clone, PartialEq)]
pub struct DoublePipe {
    /// Inner radius of the tube (the inner fluid's radius).
    pub r_tube_inner: Length,
    /// Outer radius of the tube.
    pub r_tube_outer: Length,
    /// Inner radius of the shell (the outer fluid's outer radius).
    pub r_shell: Length,
    /// Length.
    pub length: Length,
    /// Centre of the z-min end.
    pub origin: [Length; 3],
    /// O-grid side: `4 n` cells around every region.
    pub n: usize,
    /// Radial cells: inner fluid ring (beyond its central square), tube
    /// wall, outer fluid.
    pub nr: [usize; 3],
    /// Axial cells.
    pub nz: usize,
}

impl DoublePipe {
    /// Number of cells in each region (inner fluid, tube wall, outer fluid).
    pub fn n_cells(&self) -> [usize; 3] {
        let (a, b, c) = self.region_grids();
        [a.n_cells(), b.n_cells(), c.n_cells()]
    }

    /// The three region grids, positioned and named (see the type docs).
    pub fn region_grids(&self) -> (PipeOGrid, Annulus, Annulus) {
        let mut inner = PipeOGrid::new(self.r_tube_inner, self.length, self.n, self.nr[0], self.nz);
        inner.origin = self.origin;
        inner.patch_names = ["inlet".into(), "outlet".into(), "tube_inner_wall".into()];
        let mut tube = Annulus::new(self.r_tube_inner, self.r_tube_outer, self.length, 4 * self.n, self.nr[1], self.nz);
        tube.origin = self.origin;
        tube.patch_names = ["tube_ends".into(), "tube_ends".into(), "tube_inner_wall".into(), "tube_outer_wall".into()];
        let mut outer = Annulus::new(self.r_tube_outer, self.r_shell, self.length, 4 * self.n, self.nr[2], self.nz);
        outer.origin = self.origin;
        outer.patch_names = ["outlet".into(), "inlet".into(), "tube_outer_wall".into(), "shell_wall".into()];
        (inner, tube, outer)
    }

    /// Build the three meshes (inner fluid, tube wall, outer fluid).
    ///
    /// # Errors
    /// Whatever [`block_mesh`] returns.
    pub fn meshes(&self) -> Result<[PolyMesh; 3], MeshError> {
        let (a, b, c) = self.region_grids();
        Ok([a.mesh()?, b.mesh()?, c.mesh()?])
    }
}

/// A circular pipe coiled into a helix (gh:#1157, the CFD coil section): a
/// [`PipeOGrid`] of the tube's arc length, its points swept along the helix
/// centreline on a **rotation-minimising frame**, so successive cross-sections
/// are not twisted about the tube axis.
///
/// # Geometry
///
/// The helix axis is +z through `axis_origin`; the centreline starts at
/// `axis_origin + (R, 0, 0)` and winds counter-clockwise seen from +z,
/// `C(φ) = (R cos φ, R sin φ, c φ)`, with `R = D_w/2` (half the coil
/// diameter at the tube centreline), `c = h/(2π)` (`h` the pitch, rise per
/// turn) and arc length `s = ℓ φ`, `ℓ = √(R² + c²)`. Its Frenet frame is
///
/// - `T = (−R sin φ, R cos φ, c)/ℓ`,
/// - `N = (−cos φ, −sin φ, 0)`: towards the helix axis (the inner side of the
///   bend),
/// - `B = T × N = (c sin φ, −c cos φ, R)/ℓ`,
///
/// with constant curvature `κ = R/ℓ²` and torsion `τ = c/ℓ²`
/// (Frenet–Serret: `T' = κN`, `N' = −κT + τB`, `B' = −τN`). So the
/// **curvature diameter** is `1/(κ/2) = D_w [1 + (h/(π D_w))²]`, the same
/// formula as `tampines::components::helical_coil_otsg::correlations::curvature_diameter`
/// (VDI Heat Atlas Gc, there), derived here from κ rather than called, because
/// this crate does not depend on `tampines`.
///
/// **Why not the Frenet frame.** It turns about `T` at the rate τ, so a
/// cross-section swept on it is rotated by `τ Δs` against its neighbour a
/// distance `Δs` along: over one turn (`τ · 2πℓ = h/ℓ` rad) of a `h = 2d`,
/// `D_w = 10d` coil that is 23°, and every cell between layers is sheared by
/// it. The rotation-minimising
/// (Bishop) frame `U = cos θ N + sin θ B`, `V = −sin θ N + cos θ B` with
/// `θ = −τ s` has `U' = −κ cos θ T` and `V' = κ sin θ T`, i.e. no component
/// about `T` (derived from Frenet–Serret above; the frame is Bishop's, R. L.
/// Bishop (1975), "There is more than one way to frame a curve", *Amer. Math.
/// Monthly* 82(3), 246–251, doi:10.2307/2319846, cited for the idea, not
/// read in this session). The test `helix_frames_are_orthonormal_and_untwisted`
/// checks `U'·V = 0` numerically.
///
/// # Mesh
///
/// [`PipeOGrid::new`]`(radius, arc_length, n, nr, n_layers)`, patches
/// `patch_names` (inlet at `s = 0`, outlet at `s = L`, the tube wall), every
/// point `(x, y, z)` mapped to `C(z) + x U(z) + y V(z)`. The topology, cell
/// order and patch faces are the straight grid's, so cell `i` of
/// [`HelicalPipe::mesh`] is cell `i` of [`HelicalPipe::straight_grid`]'s
/// mesh (the blockMesh port's cell order is not layer by layer, so use
/// [`HelicalPipe::locate`] for a cell's arc position). Faces between layers
/// are bilinear, not planar; the face-area vectors of the FV mesh are the
/// usual triangle-fan sums.
///
/// With `pitch = 0` the coil is a torus section; turns must then not exceed 1
/// or the tube overlaps itself (`h ≥ 2 r_tube` is the general no-overlap
/// condition, which [`HelicalPipe::check`] enforces for more than one turn).
#[derive(Debug, Clone, PartialEq)]
pub struct HelicalPipe {
    /// Tube (inner) radius.
    pub radius: Length,
    /// Coil diameter `D_w` at the tube centreline.
    pub coil_diameter: Length,
    /// Pitch `h`: rise of the centreline per turn.
    pub pitch: Length,
    /// Number of turns (any positive real).
    pub turns: f64,
    /// The point of the helix axis at the inlet's height.
    pub axis_origin: [Length; 3],
    /// O-grid side (`4 n` cells around the wall).
    pub n: usize,
    /// Radial cells between the O-grid square and the wall.
    pub nr: usize,
    /// Cells along the tube.
    pub n_layers: usize,
    /// Half-width of the O-grid's central square over the radius.
    pub square_frac: f64,
    /// Patch names of the inlet (`s = 0`), outlet (`s = L`) and wall.
    pub patch_names: [String; 3],
}

/// Where a point lies relative to a [`HelicalPipe`]'s centreline:
/// arc position and Frenet coordinates of its offset from `C(s)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoilPoint {
    /// Arc length of the nearest centreline point \[m\].
    pub s: f64,
    /// Offset along `N` (positive towards the helix axis, the inner side) \[m\].
    pub n: f64,
    /// Offset along `B` \[m\].
    pub b: f64,
    /// The Frenet frame at `s`: `T`, `N`, `B`.
    pub frenet: [[f64; 3]; 3],
}

impl HelicalPipe {
    /// A coil with its axis on the z axis, patches `inlet`, `outlet`, `walls`
    /// and the default square fraction.
    pub fn new(radius: Length, coil_diameter: Length, pitch: Length, turns: f64, n: usize, nr: usize, n_layers: usize) -> Self {
        let zero = Length::new::<meter>(0.0);
        Self {
            radius,
            coil_diameter,
            pitch,
            turns,
            axis_origin: [zero, zero, zero],
            n,
            nr,
            n_layers,
            square_frac: DEFAULT_SQUARE_FRAC,
            patch_names: ["inlet".into(), "outlet".into(), "walls".into()],
        }
    }

    fn rc(&self) -> (f64, f64, f64) {
        let r = 0.5 * self.coil_diameter.get::<meter>();
        let c = self.pitch.get::<meter>() / (2.0 * std::f64::consts::PI);
        (r, c, (r * r + c * c).sqrt())
    }

    /// Why the coil cannot be meshed, if it cannot: positive sizes, the tube
    /// inside the coil radius, and `pitch ≥ 2 r_tube` beyond one turn.
    ///
    /// # Errors
    /// [`MeshError::Construction`] with the reason.
    pub fn check(&self) -> Result<(), MeshError> {
        let (rt, dw, h) = (self.radius.get::<meter>(), self.coil_diameter.get::<meter>(), self.pitch.get::<meter>());
        let bad = |m: &str| Err(MeshError::Construction(format!("HelicalPipe: {m}")));
        if !(rt > 0.0 && dw > 2.0 * rt && h >= 0.0 && self.turns > 0.0) {
            return bad("needs radius > 0, coil diameter > tube diameter, pitch ≥ 0, turns > 0");
        }
        if self.turns > 1.0 && h < 2.0 * rt {
            return bad("turns overlap: pitch must be at least the tube diameter beyond one turn");
        }
        if self.n == 0 || self.nr == 0 || self.n_layers == 0 {
            return bad("n, nr and n_layers must be at least 1");
        }
        Ok(())
    }

    /// Developed length of the tube, `turns · √((π D_w)² + h²)`.
    pub fn arc_length(&self) -> Length {
        let (_, _, l) = self.rc();
        Length::new::<meter>(2.0 * std::f64::consts::PI * l * self.turns)
    }

    /// Curvature `κ = R/(R² + c²)` \[1/m\].
    pub fn curvature(&self) -> f64 {
        let (r, _, l) = self.rc();
        r / (l * l)
    }

    /// Torsion `τ = c/(R² + c²)` \[1/m\].
    pub fn torsion(&self) -> f64 {
        let (_, c, l) = self.rc();
        c / (l * l)
    }

    /// Curvature diameter `2/κ = D_w [1 + (h/(π D_w))²]`.
    pub fn curvature_diameter(&self) -> Length {
        Length::new::<meter>(2.0 / self.curvature())
    }

    /// Cells in one cross-section, `n² + 4 n nr`.
    pub fn cells_per_section(&self) -> usize {
        self.n * self.n + 4 * self.n * self.nr
    }

    /// Number of cells.
    pub fn n_cells(&self) -> usize {
        self.cells_per_section() * self.n_layers
    }

    /// Centreline point at arc length `s` \[m\].
    pub fn centreline(&self, s: Length) -> [f64; 3] {
        let (r, c, l) = self.rc();
        let phi = s.get::<meter>() / l;
        let o = self.axis_origin.map(|v| v.get::<meter>());
        [o[0] + r * phi.r_cos(), o[1] + r * phi.r_sin(), o[2] + c * phi]
    }

    /// The Frenet frame `[T, N, B]` at arc length `s`.
    pub fn frenet(&self, s: Length) -> [[f64; 3]; 3] {
        let (r, c, l) = self.rc();
        let phi = s.get::<meter>() / l;
        let (sp, cp) = (phi.r_sin(), phi.r_cos());
        [[-r * sp / l, r * cp / l, c / l], [-cp, -sp, 0.0], [c * sp / l, -c * cp / l, r / l]]
    }

    /// The rotation-minimising frame `[T, U, V]` at `s` (see the type docs),
    /// equal to the Frenet frame at `s = 0`.
    pub fn frame(&self, s: Length) -> [[f64; 3]; 3] {
        let [t, n, b] = self.frenet(s);
        let th = -self.torsion() * s.get::<meter>();
        let (st, ct) = (th.r_sin(), th.r_cos());
        let u = [0, 1, 2].map(|i| ct * n[i] + st * b[i]);
        let v = [0, 1, 2].map(|i| -st * n[i] + ct * b[i]);
        [t, u, v]
    }

    /// The straight O-grid that is swept: radius, arc length, `n`, `nr`,
    /// `n_layers`, patches and square fraction of this coil, at the origin.
    pub fn straight_grid(&self) -> PipeOGrid {
        let mut g = PipeOGrid::new(self.radius, self.arc_length(), self.n, self.nr, self.n_layers);
        g.square_frac = self.square_frac;
        g.patch_names = self.patch_names.clone();
        g
    }

    /// Build the mesh: [`HelicalPipe::straight_grid`]'s mesh with every
    /// point swept onto the coil.
    ///
    /// # Errors
    /// [`HelicalPipe::check`]'s reason, or whatever [`block_mesh`] returns.
    pub fn mesh(&self) -> Result<PolyMesh, MeshError> {
        self.check()?;
        let mut pm = self.straight_grid().mesh()?;
        for p in pm.points.iter_mut() {
            let s = Length::new::<meter>(p.z);
            let c = self.centreline(s);
            let [_, u, v] = self.frame(s);
            *p = outram_foam_basic_lib::primitives::Vector3::new(
                c[0] + p.x * u[0] + p.y * v[0],
                c[1] + p.x * u[1] + p.y * v[1],
                c[2] + p.x * u[2] + p.y * v[2],
            );
        }
        Ok(pm)
    }

    /// The nearest centreline point to `p` \[m\] and `p`'s Frenet offsets
    /// from it: the turn is picked from `p`'s height, then Newton's method on
    /// `(p − C(s))·T(s) = 0` (exact to round-off within the tube; not meant
    /// for points far outside it). `s` is not clamped to `[0, L]`.
    pub fn locate(&self, p: [f64; 3]) -> CoilPoint {
        let (r, c, l) = self.rc();
        let o = self.axis_origin.map(|v| v.get::<meter>());
        let (x, y, z) = (p[0] - o[0], p[1] - o[1], p[2] - o[2]);
        let two_pi = 2.0 * std::f64::consts::PI;
        let mut phi = y.r_atan2(x);
        if c > 0.0 {
            let k = ((z / c - phi) / two_pi).round();
            phi += two_pi * k;
        } else if phi < -0.5 * std::f64::consts::PI {
            phi += two_pi;
        }
        for _ in 0..30 {
            let (sp, cp) = (phi.r_sin(), phi.r_cos());
            let d = [x - r * cp, y - r * sp, z - c * phi];
            // g = d · C'(φ), C' = (−R sin, R cos, c); g' = −|C'|² + d · C''.
            let g = -d[0] * r * sp + d[1] * r * cp + d[2] * c;
            let gp = -(l * l) + (-d[0] * r * cp - d[1] * r * sp);
            let step = g / gp;
            phi -= step;
            if step.abs() < 1e-15 * (1.0 + phi.abs()) {
                break;
            }
        }
        let s = phi * l;
        let sl = Length::new::<meter>(s);
        let cc = self.centreline(sl);
        let fr = self.frenet(sl);
        let d = [p[0] - cc[0], p[1] - cc[1], p[2] - cc[2]];
        let dot = |a: [f64; 3]| a[0] * d[0] + a[1] * d[1] + a[2] * d[2];
        CoilPoint { s, n: dot(fr[1]), b: dot(fr[2]), frenet: fr }
    }
}

/// An axis-aligned box as one hex block.
#[derive(Debug, Clone, PartialEq)]
pub struct BoxBlock {
    /// Minimum corner.
    pub origin: [Length; 3],
    /// Edge lengths along x, y, z.
    pub size: [Length; 3],
    /// Cells along x, y, z.
    pub cells: [usize; 3],
    /// Patch names of the faces x-min, x-max, y-min, y-max, z-min, z-max.
    pub patch_names: [String; 6],
}

impl BoxBlock {
    /// A box at the origin with `inlet` on z-min, `outlet` on z-max and
    /// `walls` on the four sides.
    pub fn new(size: [Length; 3], cells: [usize; 3]) -> Self {
        let zero = Length::new::<meter>(0.0);
        let w = || String::from("walls");
        Self {
            origin: [zero, zero, zero],
            size,
            cells,
            patch_names: [w(), w(), w(), w(), "inlet".into(), "outlet".into()],
        }
    }

    /// Number of cells.
    pub fn n_cells(&self) -> usize {
        self.cells.iter().product()
    }

    /// The blockMeshDict text.
    pub fn block_mesh_dict(&self) -> String {
        let o = self.origin.map(|v| v.get::<meter>());
        let s = self.size.map(|v| v.get::<meter>());
        let mut d = String::from("convertToMeters 1;\nvertices\n(\n");
        for (i, j, k) in [(0, 0, 0), (1, 0, 0), (1, 1, 0), (0, 1, 0), (0, 0, 1), (1, 0, 1), (1, 1, 1), (0, 1, 1)] {
            d.push_str(&format!(
                "  ({:.17e} {:.17e} {:.17e})\n",
                o[0] + s[0] * i as f64,
                o[1] + s[1] * j as f64,
                o[2] + s[2] * k as f64
            ));
        }
        let [nx, ny, nz] = self.cells.map(|c| c.max(1));
        d.push_str(&format!(
            ");\nblocks\n(\n  hex (0 1 2 3 4 5 6 7) ({nx} {ny} {nz}) simpleGrading (1 1 1)\n);\nedges\n(\n);\n"
        ));
        let faces = [[0, 4, 7, 3], [1, 2, 6, 5], [0, 1, 5, 4], [3, 7, 6, 2], [0, 3, 2, 1], [4, 5, 6, 7]];
        let groups: Vec<(&String, Vec<[usize; 4]>)> = self.patch_names.iter().zip(faces).map(|(nm, f)| (nm, vec![f])).collect();
        d.push_str(&boundary_text(&groups));
        d
    }

    /// Build the mesh.
    ///
    /// # Errors
    /// Whatever [`block_mesh`] returns.
    pub fn mesh(&self) -> Result<PolyMesh, MeshError> {
        block_mesh(&self.block_mesh_dict())
    }
}

/// The `boundary ( ... );` section, faces grouped by patch name in first-use
/// order.
fn boundary_text(groups: &[(&String, Vec<[usize; 4]>)]) -> String {
    let mut order: Vec<(&str, Vec<[usize; 4]>)> = Vec::new();
    for (name, faces) in groups {
        match order.iter_mut().find(|(n, _)| n == name) {
            Some((_, v)) => v.extend_from_slice(faces),
            None => order.push((name.as_str(), faces.clone())),
        }
    }
    let mut d = String::from("boundary\n(\n");
    for (name, faces) in order {
        let kind = if name.contains("wall") { "wall" } else { "patch" };
        d.push_str(&format!("  {name}\n  {{\n    type {kind};\n    faces\n    (\n"));
        for q in faces {
            d.push_str(&format!("      ({} {} {} {})\n", q[0], q[1], q[2], q[3]));
        }
        d.push_str("    );\n  }\n");
    }
    d.push_str(");\n");
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(v: f64) -> Length {
        Length::new::<meter>(v)
    }

    #[test]
    fn pipe_has_the_polygon_volume_and_counts() {
        let g = PipeOGrid::new(m(0.01), m(0.2), 6, 4, 40);
        assert_eq!(g.n_cells(), 5280);
        let pm = g.mesh().unwrap();
        assert_eq!(pm.n_cells, 5280);
        let vol = pm.total_volume();
        let want = g.polygon_area() * 0.2;
        assert!((vol / want - 1.0).abs() < 1e-12, "volume {vol} vs {want}");
        let names: Vec<&str> = pm.patches.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["inlet", "outlet", "walls"]);
        // 24-gon: inscribed area / πR² = 0.98862...
        assert!((g.polygon_area() / (std::f64::consts::PI * 1e-4) - 0.988_616).abs() < 1e-5);
        let fv = pm.to_fv_mesh().unwrap();
        assert!(fv.cell_volumes.iter().all(|v| *v > 0.0));
    }

    #[test]
    fn pipe_level_family_is_similar_and_patches_merge() {
        let g = PipeOGrid::level(m(0.01), m(0.2), 4);
        assert_eq!((g.n, g.nr, g.nz), (8, 8, 64));
        assert_eq!(g.n_cells(), 20480);
        let mut g2 = PipeOGrid::new(m(0.01), m(0.05), 2, 2, 4);
        g2.origin = [m(0.1), m(0.0), m(-0.02)];
        g2.patch_names = ["walls".into(), "outlet".into(), "walls".into()];
        let pm = g2.mesh().unwrap();
        let names: Vec<&str> = pm.patches.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["walls", "outlet"]);
        let fv = pm.to_fv_mesh().unwrap();
        let zmin = fv.cell_centres.iter().map(|c| c.z).fold(f64::INFINITY, f64::min);
        assert!(zmin > -0.02 && zmin < 0.0);
    }

    #[test]
    fn annulus_has_the_polygon_ring_volume() {
        let mut a = Annulus::new(m(0.006), m(0.01), m(0.05), 16, 3, 5);
        a.origin = [m(0.0), m(0.0), m(0.1)];
        assert_eq!(a.n_cells(), 240);
        let pm = a.mesh().unwrap();
        assert_eq!(pm.n_cells, 240);
        let want = a.polygon_area() * 0.05;
        assert!((pm.total_volume() / want - 1.0).abs() < 1e-12, "{} {want}", pm.total_volume());
        let names: Vec<&str> = pm.patches.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["inlet", "outlet", "inner_wall", "outer_wall"]);
        let fv = pm.to_fv_mesh().unwrap();
        assert!(fv.cell_volumes.iter().all(|v| *v > 0.0));
        // Every cell centre lies in the ring and in the slab.
        assert!(fv.cell_centres.iter().all(|c| {
            let r = (c.x * c.x + c.y * c.y).sqrt();
            r > 0.0055 && r < 0.01 && c.z > 0.1 && c.z < 0.15
        }));
    }

    /// The double pipe's interfaces are conformal: every face of the inner
    /// fluid's wall has a tube face at the same centre, and likewise between
    /// the tube and the outer fluid.
    #[test]
    fn double_pipe_regions_meet_face_to_face() {
        let dp = DoublePipe {
            r_tube_inner: m(0.005),
            r_tube_outer: m(0.006),
            r_shell: m(0.01),
            length: m(0.04),
            origin: [m(0.0); 3],
            n: 3,
            nr: [2, 2, 3],
            nz: 4,
        };
        assert_eq!(dp.n_cells(), [(9 + 24) * 4, 12 * 2 * 4, 12 * 3 * 4]);
        let [a, b, c] = dp.meshes().unwrap();
        let faces = |pm: &PolyMesh, name: &str| -> Vec<[f64; 3]> {
            let fv = pm.to_fv_mesh().unwrap();
            let p = fv.patches.iter().find(|p| p.name == name).unwrap();
            (0..p.size).map(|i| fv.face_centres[p.start + i]).map(|v| [v.x, v.y, v.z]).collect()
        };
        for (x, y, name) in [(&a, &b, "tube_inner_wall"), (&b, &c, "tube_outer_wall")] {
            let (fx, fy) = (faces(x, name), faces(y, name));
            assert_eq!(fx.len(), fy.len());
            assert_eq!(fx.len(), 12 * 4);
            for p in &fx {
                let d = fy.iter().map(|q| ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()).fold(f64::INFINITY, f64::min);
                assert!(d < 1e-15, "{name}: no partner within {d}");
            }
        }
        let names: Vec<&str> = c.patches.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["outlet", "inlet", "tube_outer_wall", "shell_wall"]);
        let names: Vec<&str> = b.patches.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["tube_ends", "tube_inner_wall", "tube_outer_wall"]);
        // The three side by side, for drawing: volumes and counts add up.
        let all = PolyMesh::concatenate(&[("inner", &a), ("tube", &b), ("outer", &c)]);
        assert_eq!(all.n_cells, a.n_cells + b.n_cells + c.n_cells);
        assert_eq!(all.n_internal_faces, a.n_internal_faces + b.n_internal_faces + c.n_internal_faces);
        let v = a.total_volume() + b.total_volume() + c.total_volume();
        assert!((all.total_volume() / v - 1.0).abs() < 1e-12);
        assert!(all.patches.iter().any(|p| p.name == "outer/shell_wall"));
        let fv = all.to_fv_mesh().unwrap();
        assert_eq!(fv.n_cells, all.n_cells);
    }

    fn coil() -> HelicalPipe {
        // d = 10 mm, D_w = 100 mm, h = 20 mm, 1.5 turns.
        HelicalPipe::new(m(0.005), m(0.1), m(0.02), 1.5, 3, 2, 120)
    }

    #[test]
    fn helix_frames_are_orthonormal_and_untwisted() {
        let g = coil();
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let (r, c) = (0.05, 0.02 / (2.0 * std::f64::consts::PI));
        assert!((g.curvature() - r / (r * r + c * c)).abs() < 1e-12);
        assert!((g.torsion() - c / (r * r + c * c)).abs() < 1e-12);
        // 2/κ is the VDI curvature diameter D_w [1 + (h/(π D_w))²].
        let dc = 0.1 * (1.0 + (0.02 / (std::f64::consts::PI * 0.1)).powi(2));
        assert!((g.curvature_diameter().get::<meter>() / dc - 1.0).abs() < 1e-14);
        let l = g.arc_length().get::<meter>();
        assert!((l - 1.5 * ((std::f64::consts::PI * 0.1).powi(2) + 0.02 * 0.02).sqrt()).abs() < 1e-14);
        assert_eq!(g.centreline(m(0.0)), [0.05, 0.0, 0.0]);
        let h = 1e-5;
        for k in 0..7 {
            let s = l * k as f64 / 6.0;
            for fr in [g.frenet(m(s)), g.frame(m(s))] {
                for i in 0..3 {
                    for j in 0..3 {
                        let want = if i == j { 1.0 } else { 0.0 };
                        assert!((dot(fr[i], fr[j]) - want).abs() < 1e-14, "frame not orthonormal at s {s}");
                    }
                }
            }
            // Unit speed, and T is the centreline's derivative.
            let (a, b) = (g.centreline(m(s - h)), g.centreline(m(s + h)));
            let d = [0, 1, 2].map(|i| (b[i] - a[i]) / (2.0 * h));
            let t = g.frenet(m(s))[0];
            assert!((0..3).all(|i| (d[i] - t[i]).abs() < 1e-8));
            // N points to the helix axis.
            let cc = g.centreline(m(s));
            let nn = g.frenet(m(s))[1];
            assert!((dot(nn, [-cc[0], -cc[1], 0.0]) - 0.05).abs() < 1e-12);
            // The Frenet frame turns about T at the torsion; the swept frame does not.
            let (f0, f1) = (g.frenet(m(s - h)), g.frenet(m(s + h)));
            let dn = [0, 1, 2].map(|i| (f1[1][i] - f0[1][i]) / (2.0 * h));
            assert!((dot(dn, g.frenet(m(s))[2]) - g.torsion()).abs() < 1e-6);
            let (u0, u1) = (g.frame(m(s - h)), g.frame(m(s + h)));
            let du = [0, 1, 2].map(|i| (u1[1][i] - u0[1][i]) / (2.0 * h));
            assert!(dot(du, g.frame(m(s))[2]).abs() < 1e-6, "twist {} at s {s}", dot(du, g.frame(m(s))[2]));
            assert!((dot(du, t) + g.curvature() * dot(g.frame(m(s))[1], nn)).abs() < 1e-6);
        }
    }

    #[test]
    fn helical_mesh_has_the_swept_volume_and_locates() {
        let g = coil();
        assert_eq!(g.cells_per_section(), 9 + 24);
        assert_eq!(g.n_cells(), 33 * 120);
        let pm = g.mesh().unwrap();
        assert_eq!(pm.n_cells, g.n_cells());
        let names: Vec<&str> = pm.patches.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["inlet", "outlet", "walls"]);
        let l = g.arc_length().get::<meter>();
        // Pappus: a section swept with its centroid on the curve encloses A·L.
        // The mesh's straight edges between layers cut the bend's chords, a
        // second-order shortfall; measured (2026-11-03) at 60/120/240/480
        // layers: −4.09e-3, −1.02e-3, −2.56e-4, −6.40e-5, i.e.
        // −0.166 (κ Δs)² (order 2.00).
        let want = g.straight_grid().polygon_area() * l;
        let shortfall = |g: &HelicalPipe| {
            let v = g.mesh().unwrap().total_volume();
            let ks = g.curvature() * g.arc_length().get::<meter>() / g.n_layers as f64;
            (v / (g.straight_grid().polygon_area() * g.arc_length().get::<meter>()) - 1.0) / (ks * ks)
        };
        let vol = pm.total_volume();
        assert!((vol / want - 1.0).abs() < 1.1e-3, "volume {vol} vs {want}: {:e}", vol / want - 1.0);
        let mut fine = coil();
        fine.n_layers = 240;
        for k in [shortfall(&g), shortfall(&fine)] {
            assert!((k + 0.166).abs() < 0.002, "volume shortfall / (κ Δs)² = {k}");
        }
        let fv = pm.to_fv_mesh().unwrap();
        assert!(fv.cell_volumes.iter().all(|v| *v > 0.0));
        let r = 0.005;
        for c in &fv.cell_centres {
            let q = g.locate([c.x, c.y, c.z]);
            assert!(q.s > 0.0 && q.s < l, "s {}", q.s);
            assert!((q.n * q.n + q.b * q.b).sqrt() < r, "outside the tube: {q:?}");
            // The offset is perpendicular to T: locate found the foot point.
            let cc = g.centreline(m(q.s));
            let d = [c.x - cc[0], c.y - cc[1], c.z - cc[2]];
            let t = q.frenet[0];
            assert!((d[0] * t[0] + d[1] * t[1] + d[2] * t[2]).abs() < 1e-12);
        }
        // The inlet sits on the centreline start, facing −T; the outlet at s = L.
        let ends = |name: &str| {
            let p = fv.patches.iter().find(|p| p.name == name).unwrap();
            let (mut a, mut sf) = (0.0, outram_foam_basic_lib::primitives::Vector3::ZERO);
            let mut cen = outram_foam_basic_lib::primitives::Vector3::ZERO;
            for i in 0..p.size {
                let f = p.start + i;
                a += fv.face_areas[f];
                sf = sf + fv.face_area_vectors[f];
                cen = cen + fv.face_centres[f] * fv.face_areas[f];
            }
            (cen * (1.0 / a), sf * (1.0 / a))
        };
        let (ci, ni) = ends("inlet");
        assert!((ci.x - 0.05).abs() < 1e-12 && ci.y.abs() < 1e-12 && ci.z.abs() < 1e-12, "{ci:?}");
        let t0 = g.frenet(m(0.0))[0];
        assert!((ni.x + t0[0]).abs() < 1e-9 && (ni.y + t0[1]).abs() < 1e-9 && (ni.z + t0[2]).abs() < 1e-9);
        let (co, _) = ends("outlet");
        let cl = g.centreline(m(l));
        assert!((co.x - cl[0]).abs() < 1e-9 && (co.y - cl[1]).abs() < 1e-9 && (co.z - cl[2]).abs() < 1e-9);
        // Locating a centreline point gives its arc length back, also on a torus.
        let q = g.locate(g.centreline(m(0.3)));
        assert!((q.s - 0.3).abs() < 1e-12 && q.n.abs() < 1e-12 && q.b.abs() < 1e-12);
        let torus = HelicalPipe::new(m(0.005), m(0.1), m(0.0), 0.75, 2, 1, 30);
        assert_eq!(torus.torsion(), 0.0);
        let q = torus.locate(torus.centreline(m(0.2)));
        assert!((q.s - 0.2).abs() < 1e-12);
        assert!(torus.mesh().is_ok());
    }

    #[test]
    fn helical_pipe_rejects_impossible_coils() {
        let mut g = coil();
        g.pitch = m(0.009);
        assert!(g.check().is_err(), "turns overlap");
        g.turns = 0.9;
        assert!(g.check().is_ok(), "under one turn any pitch is fine");
        g.coil_diameter = m(0.008);
        assert!(g.mesh().is_err());
        let mut g = coil();
        g.n_layers = 0;
        assert!(g.check().is_err());
    }

    #[test]
    fn box_block_volume_and_patches() {
        let mut b = BoxBlock::new([m(0.1), m(0.02), m(0.03)], [10, 4, 6]);
        b.origin = [m(1.0), m(0.0), m(0.0)];
        assert_eq!(b.n_cells(), 240);
        let pm = b.mesh().unwrap();
        assert!((pm.total_volume() - 0.1 * 0.02 * 0.03).abs() < 1e-15);
        let names: Vec<&str> = pm.patches.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["walls", "inlet", "outlet"]);
        let fv = pm.to_fv_mesh().unwrap();
        assert!(fv.cell_centres.iter().all(|c| c.x > 1.0 && c.x < 1.1));
    }
}
