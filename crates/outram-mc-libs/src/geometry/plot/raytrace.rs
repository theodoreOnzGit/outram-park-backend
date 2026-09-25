// SPDX-License-Identifier: GPL-3.0
//
// Ported from OpenMC (MIT), commit d7d3284a1:
//   include/openmc/plot.h:337-572   RayTracePlot, WireframeRayTracePlot,
//                                   SolidRayTracePlot, ProjectionRay, PhongRay
//   src/plot.cpp:1200-1219          RayTracePlot::update_view
//   src/plot.cpp:1265-1324          WireframeRayTracePlot::trackstack_equivalent
//   src/plot.cpp:1326-1367          RayTracePlot::get_pixel_ray
//   src/plot.cpp:1369-1529          WireframeRayTracePlot::create_image
//   src/plot.cpp:1553-1581          WireframeRayTracePlot::set_opacities
//   src/plot.cpp:1683-1701          SolidRayTracePlot::create_image
//   src/plot.cpp:1750-1763          ProjectionRay::on_intersection
//   src/plot.cpp:1765-1891          PhongRay::on_intersection
//   src/ray.cpp:9-143               Ray::compute_distance, Ray::trace
//   src/particle_data.cpp:59-84     GeometryState::advance_to_boundary_from_void
// Copyright (c) 2011-2026 Massachusetts Institute of Technology, UChicago
// Argonne LLC, and OpenMC contributors. MIT notice in
// verification_and_validation/geometry_plotting/openmc_inputs/LICENSE.openmc.

//! **Ray-traced plots** — OpenMC's `wireframe_raytrace` and `solid_raytrace`.
//!
//! Rays are traced through the crate's own CSG machinery —
//! [`Geometry::locate`], [`Geometry::distance_to_boundary`] and
//! [`Cell::distance_to_boundary`](crate::geometry::cell::Cell::distance_to_boundary)
//! — so a ray-traced plot shows the geometry transport sees. There is no
//! second geometry engine here.
//!
//! # Where this can differ from OpenMC, and why
//!
//! 1. **Complex (union / complement) regions.** Upstream's
//!    `Region::distance_complex` walks along the ray until the region is
//!    actually left. This crate's cell distance takes the nearest bounding
//!    surface of *any* half-space, so a ray in a union region stops at an
//!    internal surface, re-locates into the same cell and records an extra
//!    segment. Colour is unaffected (consecutive segments of one domain compose
//!    to the same attenuation) but the wireframe can show an edge upstream
//!    does not. Intersection-only regions — every case in the V&V — are exact.
//! 2. **Relocation after a crossing.** Upstream re-searches from the crossed
//!    level down (`neighbor_list_find_cell`, `cross_lattice`); this re-locates
//!    from the root at the advanced position. Without overlaps the two find
//!    the same cell.
//! 3. **Void material under material colouring.** Upstream indexes its colour
//!    and opacity tables with `MATERIAL_VOID = -1` — out of bounds. Here a void
//!    segment is fully transparent and never opaque.

#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;

#[cfg(target_arch = "wasm32")]
use crate::wasm_par::prelude::*;

use super::colour::{ColourScheme, PlotColourBy, Rgb, BLACK};
use super::image::ImageData;
use crate::geometry::cell::{Cell, HalfSpaceSense, RegionToken, SurfaceToken};
use crate::geometry::geometry::{Crossing, Geometry, GeometryPath};
use crate::geometry::position::{Direction, Position};

/// `TINY_BIT` (`include/openmc/constants.h:50`) — how far past a boundary a
/// ray is pushed.
pub const TINY_BIT: f64 = 1e-8;

/// `Ray::MAX_INTERSECTIONS` (`include/openmc/ray.h`).
const MAX_INTERSECTIONS: u32 = 1_000_000;

/// `focal_plane_dist` in `get_pixel_ray` (`src/plot.cpp:1339`), in cm.
const FOCAL_PLANE_DIST: f64 = 10.0;

/// Camera projection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Projection {
    /// Perspective with this horizontal field of view in degrees, `(0, 180)`.
    /// Upstream default: 70 (`include/openmc/plot.h:397`).
    Perspective {
        /// Horizontal field of view \[degrees\].
        horizontal_fov_deg: f64,
    },
    /// Orthographic with this horizontal extent \[cm\] (`orthographic_width_`).
    Orthographic {
        /// Width of the view \[cm\].
        width: f64,
    },
}

/// The camera of a ray-traced plot. `RayTracePlot`'s members
/// (`include/openmc/plot.h:397-414`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// Eye position \[cm\].
    pub position: Position,
    /// Point at the centre of the view \[cm\].
    pub look_at: Position,
    /// Which way is up. Upstream default `(0, 0, 1)`; it is not settable from
    /// `plots.xml`, only through the C API.
    pub up: Direction,
    /// Pixels across and down.
    pub pixels: [usize; 2],
    /// Projection.
    pub projection: Projection,
}

/// `Position::cross` (`include/openmc/position.h:86-90`) on direction triples.
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn norm(a: [f64; 3]) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

fn scale(a: [f64; 3], s: f64) -> [f64; 3] {
    [a[0] / s, a[1] / s, a[2] / s]
}

impl Camera {
    /// A perspective camera with upstream's defaults (70 degrees, up = +z).
    #[must_use]
    pub fn perspective(position: Position, look_at: Position, pixels: [usize; 2]) -> Self {
        Self {
            position,
            look_at,
            up: Direction::new(0.0, 0.0, 1.0),
            pixels,
            projection: Projection::Perspective {
                horizontal_fov_deg: 70.0,
            },
        }
    }

    /// Camera-to-model matrix, row-major with the camera axes as columns.
    /// Port of `RayTracePlot::update_view` (`src/plot.cpp:1200-1219`).
    ///
    /// # Panics
    /// If `up` is parallel to the viewing direction — upstream's fatal error.
    #[must_use]
    pub fn camera_to_model(&self) -> [f64; 9] {
        let up = [self.up.u, self.up.v, self.up.w];
        let up = scale(up, norm(up));
        let ld = [
            self.look_at.x - self.position.x,
            self.look_at.y - self.position.y,
            self.look_at.z - self.position.z,
        ];
        let ld = scale(ld, norm(ld));
        let dot = ld[0] * up[0] + ld[1] * up[1] + ld[2] * up[2];
        assert!(
            (dot.abs() - 1.0).abs() >= 1e-9,
            "Up vector cannot align with vector between camera position and look_at!"
        );
        let cy = cross(ld, up);
        let cy = scale(cy, norm(cy));
        let cz = cross(cy, ld);
        let cz = scale(cz, norm(cz));
        [
            ld[0], cy[0], cz[0], ld[1], cy[1], cz[1], ld[2], cy[2], cz[2],
        ]
    }

    /// Start point and direction of the ray through pixel `(horiz, vert)`.
    /// Port of `RayTracePlot::get_pixel_ray` (`src/plot.cpp:1326-1367`) — which
    /// aims at the pixel's corner, not its centre; kept.
    #[must_use]
    pub fn pixel_ray(&self, m: &[f64; 9], horiz: usize, vert: usize) -> (Position, Direction) {
        let p0 = self.pixels[0] as f64;
        let p1 = self.pixels[1] as f64;
        match self.projection {
            Projection::Perspective { horizontal_fov_deg } => {
                const DEGREE_TO_RADIAN: f64 = std::f64::consts::PI / 180.0;
                let horiz_fov_radians = horizontal_fov_deg * DEGREE_TO_RADIAN;
                let dx = 2.0 * FOCAL_PLANE_DIST * (0.5 * horiz_fov_radians).tan();
                let dy = p1 / p0 * dx;
                let mut v = [
                    FOCAL_PLANE_DIST,
                    -0.5 * dx + horiz as f64 * dx / p0,
                    0.5 * dy - vert as f64 * dy / p1,
                ];
                let n = norm(v);
                v = [v[0] / n, v[1] / n, v[2] / n];
                // Position::rotate (include/openmc/position.h:99-104).
                let d = [
                    v[0] * m[0] + v[1] * m[1] + v[2] * m[2],
                    v[0] * m[3] + v[1] * m[4] + v[2] * m[5],
                    v[0] * m[6] + v[1] * m[7] + v[2] * m[8],
                ];
                (self.position, Direction::new(d[0], d[1], d[2]))
            }
            Projection::Orthographic { width } => {
                let x_pix = (horiz as f64 - p0 / 2.0) / p0;
                let y_pix = (vert as f64 - p1 / 2.0) / p1;
                let cy = Position::new(m[1], m[4], m[7]);
                let cz = Position::new(m[2], m[5], m[8]);
                let r = self.position + cy * x_pix * width + cz * y_pix * width;
                (r, Direction::new(m[0], m[3], m[6]))
            }
        }
    }
}

// ─── The ray engine: Ray::trace ─────────────────────────────────────────────

/// What a ray visitor sees at each boundary it crosses. The fields are the
/// pieces of `GeometryState` that `ProjectionRay` and `PhongRay` read.
#[derive(Debug, Clone, Copy)]
struct Intersection {
    /// Leaf cell index (`lowest_coord().cell()`); for the exit crossing, the
    /// stale value upstream would read — the cell at the crossing level.
    cell: usize,
    /// Leaf material (`material()`), stale on the exit crossing as upstream.
    material: Option<usize>,
    /// Index of the crossed surface; `None` for a lattice-tile crossing
    /// (`boundary().surface_index()` of `SURFACE_NONE` is `-1`).
    surface_index: Option<usize>,
    /// Upstream's `surface()` token at this point.
    token: SurfaceToken,
    /// `traversal_distance_`.
    traversal_distance: f64,
    /// Global position.
    r: Position,
    /// Global direction.
    u: Direction,
    /// Coordinate level (0-based) of the crossing.
    level: usize,
}

/// What a visitor asks the tracer to do next.
#[derive(Debug, Clone, Copy)]
enum RayAction {
    /// Keep tracing.
    Continue,
    /// `stop()`.
    Stop,
    /// Point the ray along `u` and re-locate it with surface token `token`
    /// (`PhongRay`'s reflection toward the light, `src/plot.cpp:1848-1877`).
    Redirect { u: Direction, token: SurfaceToken },
}

fn flip(s: HalfSpaceSense) -> HalfSpaceSense {
    match s {
        HalfSpaceSense::Inside => HalfSpaceSense::Outside,
        HalfSpaceSense::Outside => HalfSpaceSense::Inside,
    }
}

/// The side of surface `i_surf` that cell `cell`'s region selects, if the
/// surface appears in it.
fn region_sense(cell: &Cell, i_surf: usize) -> Option<HalfSpaceSense> {
    cell.region.iter().find_map(|t| match *t {
        RegionToken::HalfSpace { surface_idx, sense } if surface_idx == i_surf => Some(sense),
        _ => None,
    })
}

/// Upstream's signed boundary token for a crossing of `i_surf` computed from
/// `cell`'s region: `i_surf = -token` (`Region::distance_to_nearest_surface`,
/// `src/cell.cpp:989`), i.e. the side **opposite** the one the cell's region
/// selects.
fn boundary_token(cell: &Cell, i_surf: usize) -> SurfaceToken {
    match region_sense(cell, i_surf) {
        Some(s) => SurfaceToken::on(i_surf, flip(s)),
        None => SurfaceToken::NONE,
    }
}

fn advance(r: Position, u: Direction, d: f64) -> Position {
    Position::new(r.x + d * u.u, r.y + d * u.v, r.z + d * u.w)
}

/// Port of `Ray::trace` (`src/ray.cpp:14-143`) with
/// `advance_to_boundary_from_void` (`src/particle_data.cpp:59-84`).
///
/// `on_intersection` is the ray type's `on_intersection` (upstream's virtual
/// method), passed as a closure: static dispatch, no trait object.
fn trace<F>(geom: &Geometry, r0: Position, u0: Direction, mut on_intersection: F)
where
    F: FnMut(&Geometry, Option<&GeometryPath>, &Intersection) -> RayAction,
{
    let mut r = r0;
    let mut u = u0;
    let mut counter = 0u32;

    // Phase 1: reach the model from outside it.
    let mut path = geom.locate(r, u, SurfaceToken::NONE);
    // boundary() after advance_to_boundary_from_void: (surface index, token).
    let mut entry: Option<(usize, SurfaceToken)> = None;
    while path.is_none() {
        let mut best = f64::INFINITY;
        let mut best_hit: Option<(usize, usize)> = None; // (cell, surface)
        for &c in &geom.universes[geom.root_universe].cell_indices {
            let (d, s) =
                geom.cells[c].distance_to_boundary(r, u, &geom.surfaces, SurfaceToken::NONE);
            if d < best {
                best = d;
                best_hit = if s == usize::MAX { None } else { Some((c, s)) };
            }
        }
        if best > 1e300 || best_hit.is_none() {
            return; // no intersection with the model: background
        }
        let (c, s) = best_hit.unwrap_or((0, 0));
        r = advance(r, u, best + TINY_BIT);
        entry = Some((s, boundary_token(&geom.cells[c], s)));
        path = geom.locate(r, u, SurfaceToken::NONE);
        if path.is_some() {
            break;
        }
        counter += 1;
        if counter > MAX_INTERSECTIONS {
            return;
        }
    }
    let Some(mut path) = path else { return };

    let mut traversal = 0.0;
    let leaf_cell = |p: &GeometryPath| p.levels.last().map_or(usize::MAX, |c| c.cell);

    // The first intersection, if the ray entered from outside.
    if let Some((s, token)) = entry {
        let hit = Intersection {
            cell: leaf_cell(&path),
            material: path.material,
            surface_index: Some(s),
            token,
            traversal_distance: traversal,
            r,
            u,
            level: 0,
        };
        match on_intersection(geom, Some(&path), &hit) {
            RayAction::Stop => return,
            RayAction::Continue => {}
            RayAction::Redirect { u: nu, token } => {
                u = nu;
                match geom.locate(r, u, token) {
                    Some(p) => path = p,
                    None => return, // upstream: fatal "Lost particle after reflection"
                }
            }
        }
    }
    // `surface() = 0` after the first intersection (src/ray.cpp:78-80).
    path.on_surface = SurfaceToken::NONE;

    loop {
        let bh = geom.distance_to_boundary(&path);
        let d0 = bh.distance;
        if !d0.is_finite() || d0 >= f64::MAX || d0 < 0.0 {
            return;
        }
        let call = d0 >= 10.0 * TINY_BIT;
        let d = d0 + TINY_BIT;
        r = advance(r, u, d);
        traversal += d;

        let level = bh.coord_level;
        let (surface_index, token) = match bh.crossing {
            Crossing::Surface(s) => {
                let left = &geom.cells[path.levels[level].cell];
                (Some(s), boundary_token(left, s))
            }
            Crossing::Lattice | Crossing::None => (None, SurfaceToken::NONE),
        };
        let next = geom.locate(r, u, token);
        let inside = next.is_some();
        let hit = match &next {
            Some(p) => Intersection {
                cell: leaf_cell(p),
                material: p.material,
                surface_index,
                token,
                traversal_distance: traversal,
                r,
                u,
                level,
            },
            // Left the model: upstream's find failed, so `material()` and the
            // cell at `n_coord = coord_level` are the stale ones.
            None => Intersection {
                cell: path.levels[level].cell,
                material: path.material,
                surface_index,
                token,
                traversal_distance: traversal,
                r,
                u,
                level,
            },
        };
        if let Some(p) = next {
            path = p;
        }
        if call {
            match on_intersection(geom, if inside { Some(&path) } else { None }, &hit) {
                RayAction::Stop => return,
                RayAction::Continue => {}
                RayAction::Redirect { u: nu, token } => {
                    u = nu;
                    match geom.locate(r, u, token) {
                        Some(p) => path = p,
                        None => return,
                    }
                }
            }
        }
        if !inside {
            return;
        }
        counter += 1;
        if counter > MAX_INTERSECTIONS {
            return;
        }
    }
}

// ─── Wireframe ──────────────────────────────────────────────────────────────

/// One recorded crossing. `WireframeRayTracePlot::TrackSegment`
/// (`include/openmc/plot.h:457-469`).
#[derive(Debug, Clone, Copy, PartialEq)]
struct TrackSegment {
    /// Cell or material index entered; `None` = void material.
    id: Option<usize>,
    /// Cumulative distance along the ray at this crossing.
    length: f64,
    /// Crossed surface, `-1` for a lattice crossing.
    surface_index: i64,
}

struct ProjectionRay {
    by: PlotColourBy,
    segments: Vec<TrackSegment>,
}

impl ProjectionRay {
    /// `ProjectionRay::on_intersection` (`src/plot.cpp:1750-1763`).
    fn on_intersection(&mut self, hit: &Intersection) -> RayAction {
        let id = match self.by {
            PlotColourBy::Material => hit.material,
            PlotColourBy::Cell => Some(hit.cell),
        };
        self.segments.push(TrackSegment {
            id,
            length: hit.traversal_distance,
            surface_index: hit.surface_index.map_or(-1, |s| s as i64),
        });
        RayAction::Continue
    }
}

/// A wireframe ("x-ray") plot. `WireframeRayTracePlot`.
#[derive(Debug, Clone)]
pub struct WireframeRayTracePlot {
    /// The camera.
    pub camera: Camera,
    /// Attenuation per colour index \[1/cm\]; upstream default `1e6` = opaque
    /// (`set_opacities`, `src/plot.cpp:1555`). Same length as the scheme's colours.
    pub xs: Vec<f64>,
    /// Line thickness in pixels; 0 = no wireframe. Default 1.
    pub wireframe_thickness: i32,
    /// Line colour. Default [`BLACK`].
    pub wireframe_colour: Rgb,
    /// Colour indices to outline; empty = every boundary (`wireframe_ids_`).
    pub wireframe_ids: Vec<usize>,
}

impl WireframeRayTracePlot {
    /// Defaults: everything opaque, thickness 1, black lines on every boundary.
    #[must_use]
    pub fn new(camera: Camera, n_domains: usize) -> Self {
        Self {
            camera,
            xs: vec![1e6; n_domains],
            wireframe_thickness: 1,
            wireframe_colour: BLACK,
            wireframe_ids: Vec::new(),
        }
    }

    /// Set one domain's attenuation (`<color id=.. xs=..>`).
    #[must_use]
    pub fn with_xs(mut self, index: usize, xs: f64) -> Self {
        if let Some(x) = self.xs.get_mut(index) {
            *x = xs;
        }
        self
    }

    /// Port of `trackstack_equivalent` (`src/plot.cpp:1265-1324`).
    fn trackstack_equivalent(&self, t1: &[TrackSegment], t2: &[TrackSegment]) -> bool {
        if self.wireframe_ids.is_empty() {
            if t1.len() != t2.len() {
                return false;
            }
            return t1
                .iter()
                .zip(t2)
                .all(|(a, b)| a.id == b.id && a.surface_index == b.surface_index);
        }
        for &id in &self.wireframe_ids {
            let id = Some(id);
            let (mut i1, mut i2) = (0usize, 0usize);
            while i1 < t1.len() && i2 < t2.len() {
                while i1 < t1.len() && t1[i1].id != id {
                    i1 += 1;
                }
                while i2 < t2.len() && t2[i2].id != id {
                    i2 += 1;
                }
                if (i1 == t1.len()) != (i2 == t2.len()) {
                    return false;
                }
                if i1 == t1.len() && i2 == t2.len() {
                    break;
                }
                if t1[i1].surface_index != t2[i2].surface_index {
                    return false;
                }
                if i2 != 0 && i1 != 0 && t1[i1 - 1].surface_index != t2[i2 - 1].surface_index {
                    return false;
                }
                i1 += 1;
                i2 += 1;
            }
        }
        true
    }

    fn trace_pixel(
        &self,
        geom: &Geometry,
        m: &[f64; 9],
        by: PlotColourBy,
        h: usize,
        v: usize,
    ) -> Vec<TrackSegment> {
        let (r, u) = self.camera.pixel_ray(m, h, v);
        let mut ray = ProjectionRay {
            by,
            segments: Vec::new(),
        };
        trace(geom, r, u, |_, _, hit| ray.on_intersection(hit));
        ray.segments
    }

    /// Render. Port of `WireframeRayTracePlot::create_image`
    /// (`src/plot.cpp:1369-1529`). Upstream's OpenMP row interleaving reduces
    /// to "each row is compared with the row above, row 0 with an empty row";
    /// that is what is done here, rows traced in parallel in bands.
    #[must_use]
    pub fn create_image(&self, geom: &Geometry, scheme: &ColourScheme) -> ImageData {
        let [w, h] = self.camera.pixels;
        let mut data = ImageData::filled(w, h, scheme.background);
        let mut wire = vec![false; w * h];
        let m = self.camera.camera_to_model();
        let by = scheme.colour_by;
        let bg = [
            f64::from(scheme.background.r),
            f64::from(scheme.background.g),
            f64::from(scheme.background.b),
        ];

        let mut above: Vec<Vec<TrackSegment>> = vec![Vec::new(); w];
        const BAND: usize = 32;
        let mut v0 = 0;
        while v0 < h {
            let v1 = (v0 + BAND).min(h);
            let band: Vec<Vec<Vec<TrackSegment>>> = (v0..v1)
                .into_par_iter()
                .map(|v| {
                    (0..w)
                        .map(|x| self.trace_pixel(geom, &m, by, x, v))
                        .collect()
                })
                .collect();
            for (k, line) in band.into_iter().enumerate() {
                let vert = v0 + k;
                for horiz in 0..w {
                    let segs = &line[horiz];
                    if segs.len() <= 1 {
                        continue;
                    }
                    let mut c = bg;
                    for i in (0..segs.len() - 1).rev() {
                        let Some(id) = segs[i].id else { continue };
                        let sc = scheme.colours[id];
                        let s = [f64::from(sc.r), f64::from(sc.g), f64::from(sc.b)];
                        let mixing = (-self.xs[id] * (segs[i + 1].length - segs[i].length)).exp();
                        for ch in 0..3 {
                            c[ch] = c[ch] * mixing + (1.0 - mixing) * s[ch];
                        }
                    }
                    data.set(horiz, vert, Rgb::new(c[0] as u8, c[1] as u8, c[2] as u8));
                    if horiz > 0 && !self.trackstack_equivalent(segs, &line[horiz - 1]) {
                        wire[vert * w + horiz] = true;
                    }
                }
                for horiz in 0..w {
                    if !self.trackstack_equivalent(&line[horiz], &above[horiz]) {
                        wire[vert * w + horiz] = true;
                    }
                }
                above = line;
            }
            v0 = v1;
        }

        // Thicken and apply (src/plot.cpp:1508-1526).
        let t = self.wireframe_thickness;
        for vert in 0..h {
            for horiz in 0..w {
                if !wire[vert * w + horiz] {
                    continue;
                }
                if t == 1 {
                    data.set(horiz, vert, self.wireframe_colour);
                }
                for i in -t / 2..t / 2 {
                    for j in -t / 2..t / 2 {
                        if i * i + j * j < t * t {
                            let wi = (horiz as i64 + i64::from(i)).clamp(0, w as i64 - 1) as usize;
                            let wj = (vert as i64 + i64::from(j)).clamp(0, h as i64 - 1) as usize;
                            data.set(wi, wj, self.wireframe_colour);
                        }
                    }
                }
            }
        }
        data
    }
}

// ─── Solid (Phong) ──────────────────────────────────────────────────────────

/// A solid, lit plot. `SolidRayTracePlot`.
#[derive(Debug, Clone)]
pub struct SolidRayTracePlot {
    /// The camera.
    pub camera: Camera,
    /// Which colour indices are opaque (`opaque_ids_`); everything else is
    /// invisible.
    pub opaque: Vec<bool>,
    /// Light position; `None` = at the camera (upstream default,
    /// `src/plot.cpp:1735-1737`).
    pub light_position: Option<Position>,
    /// Share of ambient light, `[0, 1]`. Default 0.1.
    pub diffuse_fraction: f64,
}

impl SolidRayTracePlot {
    /// Defaults: nothing opaque, light at the camera, diffuse fraction 0.1.
    #[must_use]
    pub fn new(camera: Camera, n_domains: usize) -> Self {
        Self {
            camera,
            opaque: vec![false; n_domains],
            light_position: None,
            diffuse_fraction: 0.1,
        }
    }

    /// Make one domain opaque (`<opaque_ids>`).
    #[must_use]
    pub fn with_opaque(mut self, index: usize) -> Self {
        if let Some(o) = self.opaque.get_mut(index) {
            *o = true;
        }
        self
    }

    /// Render. Port of `SolidRayTracePlot::create_image` (`src/plot.cpp:1683-1701`).
    #[must_use]
    pub fn create_image(&self, geom: &Geometry, scheme: &ColourScheme) -> ImageData {
        let [w, h] = self.camera.pixels;
        let m = self.camera.camera_to_model();
        let light = self.light_position.unwrap_or(self.camera.position);
        let rows: Vec<Vec<Rgb>> = (0..h)
            .into_par_iter()
            .map(|v| {
                (0..w)
                    .map(|x| {
                        let (r, u) = self.camera.pixel_ray(&m, x, v);
                        let mut ray = PhongRay {
                            reflected: false,
                            orig_hit: None,
                            result: scheme.background,
                        };
                        trace(geom, r, u, |g, p, hit| {
                            ray.on_intersection(self, scheme, light, g, p, hit)
                        });
                        ray.result
                    })
                    .collect()
            })
            .collect();
        ImageData {
            width: w,
            height: h,
            pixels: rows.into_iter().flatten().collect(),
        }
    }
}

/// The per-ray state of `PhongRay` (`include/openmc/plot.h:545-572`); the
/// plot, scheme and light are passed in, not stored.
struct PhongRay {
    reflected: bool,
    orig_hit: Option<usize>,
    result: Rgb,
}

impl PhongRay {
    /// Port of `PhongRay::on_intersection` (`src/plot.cpp:1765-1891`).
    fn on_intersection(
        &mut self,
        plot: &SolidRayTracePlot,
        scheme: &ColourScheme,
        light: Position,
        geom: &Geometry,
        path: Option<&GeometryPath>,
        hit: &Intersection,
    ) -> RayAction {
        let hit_id = match scheme.colour_by {
            PlotColourBy::Material => hit.material,
            PlotColourBy::Cell => Some(hit.cell),
        };
        let cam = plot.camera.position;
        let toward =
            (hit.r.x - cam.x) * hit.u.u + (hit.r.y - cam.y) * hit.u.v + (hit.r.z - cam.z) * hit.u.w;
        if self.reflected && toward >= 0.0 {
            return RayAction::Stop;
        }
        let opaque = hit_id.is_some_and(|i| plot.opaque.get(i).copied().unwrap_or(false));
        if !opaque {
            return RayAction::Continue;
        }
        let id = hit_id.unwrap_or(0);
        if self.reflected {
            let orig = self.orig_hit.unwrap_or(id);
            self.result = scheme.colours[orig].scaled(plot.diffuse_fraction);
            return RayAction::Stop;
        }

        self.reflected = true;
        self.result = scheme.colours[id];
        let r_hit = Position::new(
            hit.r.x - TINY_BIT * hit.u.u,
            hit.r.y - TINY_BIT * hit.u.v,
            hit.r.z - TINY_BIT * hit.u.w,
        );
        let tl = [light.x - r_hit.x, light.y - r_hit.y, light.z - r_hit.z];
        let tl = scale(tl, norm(tl));
        let to_light = Direction::new(tl[0], tl[1], tl[2]);

        // A crossing with no surface token (a lattice-tile edge): upstream
        // paints the overlap colour and stops (src/plot.cpp:1795-1808).
        let SurfaceToken::On { surface_idx, sense } = hit.token else {
            self.result = scheme.overlap_colour;
            return RayAction::Stop;
        };
        let Some(path) = path else {
            return RayAction::Stop;
        };

        let lvl = hit.level.min(path.levels.len().saturating_sub(1));
        let c = &path.levels[lvl];
        let r_lvl = Position::new(
            c.r.x - TINY_BIT * c.u.u,
            c.r.y - TINY_BIT * c.u.v,
            c.r.z - TINY_BIT * c.u.w,
        );
        let n = geom.surfaces[surface_idx].normal(r_lvl);
        let nn = norm([n.u, n.v, n.w]);
        let mut n = [n.u / nn, n.v / nn, n.w / nn];
        if n[0] * hit.u.u + n[1] * hit.u.v + n[2] * hit.u.w > 0.0 {
            n = [-n[0], -n[1], -n[2]];
        }
        let dot = (n[0] * to_light.u + n[1] * to_light.v + n[2] * to_light.w).max(0.0);
        let df = plot.diffuse_fraction;
        let modulation = df + (1.0 - df) * dot;
        self.result = self.result.scaled(modulation);
        self.orig_hit = Some(id);
        // `surface() = -surface()`: go to the other side, then re-locate.
        RayAction::Redirect {
            u: to_light,
            token: SurfaceToken::on(surface_idx, flip(sense)),
        }
    }
}
