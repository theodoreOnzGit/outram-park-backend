// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// This file is part of OUTRAM PARK. See the module header of `htr10_rmc` for
// licence terms.

//! # The HTR-10 core built from an explicit list of pebble centres (a DEM fill)
//!
//! NEW WORK, not a port. Added 2026-10-05 so the HTR-10 Monte Carlo model can
//! be built on the bed a DEM pour (`outram-park-fork-liggghts`) actually
//! produced, instead of only on Şeker & Çolak (2003)'s ordered lattice
//! ([`super::bed::SekerBed`]).
//!
//! [`assemble_explicit_triso_from_centres`] takes pebble centres in the **DEM
//! frame** and a fuel/dummy flag per pebble, and returns the same
//! [`AssembledCore`] that [`super::core_model::assemble_explicit_triso`]
//! returns, with [`PebbleBed::Explicit`] as its bed. Everything around the bed
//! (TRISO particle and lattice, bed envelope, conus, discharge tube, cavity,
//! TECDOC zone map, borings, withdrawn rods) is the same code,
//! [`super::core_shell`], so the two builders cannot drift apart.
//!
//! # Frame
//!
//! - **DEM frame** (input): metres, `z = 0` is the bed floor (the top of the
//!   conus, where the core cylinder meets the cone), the conus below it to
//!   `z = -0.36946 m`, the discharge tube below that. This is the frame of
//!   `reference-data/liggghts/in.htr10_conus` and its outputs.
//! - **MC frame** (output): cm, the bed floor at `-bed_half_height`.
//!
//! `bed_half_height` is half the height of the **top of the highest pebble**
//! above the floor, so that `z_MC = 100 z_DEM - bed_half_height`. The top of
//! the highest ball is Şeker's own definition of the bed top (`SekerBed`), and
//! it is the only choice under which the bed envelope's top plane cuts no
//! pebble. The cavity above the bed is fixed hardware measured from the floor
//! ([`super::core_model::cavity_above_bed`]), so the choice moves no material:
//! between the highest ball and the cavity top there is helium either way.
//!
//! A poured surface is rough, so the highest ball is not a robust *loading
//! height*. [`ExplicitBed::surface_height`] records the robust one, the 99th
//! percentile of the centre heights above the floor plus one radius (the
//! measure the liggghts examples use). It is reported, not used to place
//! anything. The reference is matched by ball count
//! ([`PebbleBed::core_balls`], balls centred above the floor), exactly as for
//! Şeker's bed.
//!
//! # Overlap: lens split by the bisector plane (nothing shrunk)
//!
//! A soft-sphere DEM bed has every contact slightly interpenetrating (at
//! `E = 5e8 Pa` up to ~1.7 % of the radius). Two overlapping CSG spheres make
//! the lens between them belong to two cells. **No pebble is shrunk or moved.**
//! Instead each overlapping pair's lens is split by the plane bisecting the
//! centre line (the plane through the midpoint, normal to the centre line):
//! each ball's graphite cell is its sphere intersected with the half-space on
//! its own side. For two spheres of **equal radius** the bisector is the
//! radical plane, and the cap of ball *i* beyond it lies wholly inside ball
//! *j*, so the split moves exactly half of each lens to each ball: no carbon or
//! fuel is invented or removed beyond the lens itself, whose volume is counted
//! once instead of twice. That once-counted lens volume is the only change in
//! inventory, reported as [`ExplicitBed::lens_volume_fraction`] (lens volume
//! over the summed nominal sphere volume). The 2.5 cm fuel zone is never
//! touched as long as the centre distance is at least `2 x 2.5 = 5.0` cm (an
//! overlap of 1.0 cm); a bed with a closer pair is refused with a panic rather
//! than silently clipped.
//!
//! Pebbles touching (overlap below [`TOUCH_TOLERANCE_CM`]) get no plane.
//!
//! **What the DEM wall does.** The DEM walls are soft too, so a ball can sit a
//! few hundredths of a millimetre into the side wall or the faceted conus. The
//! bed envelope region clips that cap (it is graphite in the reflector's
//! place, not inside the bed). It is measured as
//! [`ExplicitBed::max_wall_penetration`] and not corrected. A ball whose
//! CENTRE is outside the container is a frame error and panics.
//!
//! # The discharge tube below the DEM column
//!
//! The DEM tube is short (`in.htr10_conus`: 25 cm, valve at `z = -0.61946 m`);
//! the MC tube runs [`HTR10_BOTTOM_REFLECTOR_CM`] = 221.236 cm below the
//! conus floor. Below the lowest DEM ball the tube is filled with the **same
//! balls the default Şeker model puts there**: the discharge-tube balls of a
//! `SekerBed` (all dummy, every ball whole, rejected at the tube wall),
//! translated so its conus floor is this bed's, keeping only balls whose top
//! is at or below `cut = min(conus floor, bottom of the lowest DEM ball)`. The
//! plane `z = cut` therefore separates the two sets and no DEM ball can
//! overlap a filler ball. The helium gap between the highest filler ball and
//! `cut` is reported as [`ExplicitBed::tube_gap`]. The filler is ordered, not
//! poured: it is the default model's treatment, stated, not a DEM result.
//! Under the `OUTRAM_HTR10_HOMOG_TUBE` ablation the tube is the old smear,
//! no filler is built, and DEM balls centred below the conus floor are dropped
//! (counted in [`ExplicitBed::dropped_dem_balls`]); a ball straddling the
//! floor is then cut by it, as that ablation already does to Şeker's balls.
//!
//! # Tiles
//!
//! The bed lattice is a `HexLattice` of Şeker's tile shape (pitch 16.392 cm,
//! height 9.798 cm), used purely as a spatial index. Every ball is placed in
//! **every tile it intersects** (exact hexagonal-prism / sphere distance), at
//! its tile-local centre, as the same cells as Şeker's: a fuel ball is a
//! fuel-zone cell filled with the TRISO lattice translated to the centre plus
//! a graphite shell, a dummy ball a graphite sphere, the rest of the tile
//! helium. Cell ids follow [`super::core_model::tile_cell_role`]. Unlike
//! Şeker's bed, almost every tile is unique, so each non-empty tile is its own
//! universe; empty tiles and the lattice `outer` share one all-helium universe.
//!
//! # Status
//!
//! AI-drafted, awaiting human review. The geometry is drawn in
//! `crates/nee_soon/verification_and_validation/htr10_dem_bed_images/`. No
//! k_eff on a DEM bed has been validated; see that folder's README for the
//! one smoke run and what it does and does not show. **Since 2026-10-08
//! (gh:#787)** one full-statistics k is recorded, on the gh:#216 bed cut to
//! the lattice's 16 681 balls by [`trim_to_core_balls`]: 0.989293 ±
//! 0.000962, −583 ± 143 pcm from the lattice at N = 12, on one pour
//! (`verification_and_validation/htr10_dem_bed_keff_2026_10_08/`).

use std::collections::BTreeMap;

use outram_mc_libs::geometry::cell::{Cell, CellFill, RegionToken};
use outram_mc_libs::geometry::lattice::{HexLattice, HexOrientation};
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Plane, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use uom::si::f64::Length;
use uom::si::length::centimeter;

use super::bed::{hex_ring, tile_xy, FuelAssignment, PebbleBed, SekerBed, SekerCell};
use super::core_model::{
    mat, AssembledCore, HTR10_BOTTOM_REFLECTOR_CM, HTR10_CONUS_HEIGHT_CM, HTR10_CORE_CAVITY_CM,
    HTR10_CORE_RADIUS_CM, HTR10_DISCHARGE_TUBE_RADIUS_CM, TILE_DUMMY_BALL_CELL_ID,
    TILE_FUEL_SHELL_CELL_ID, TILE_FUEL_ZONE_CELL_ID, TILE_HELIUM_CELL_ID, TILE_SITE_STRIDE,
    TILE_VARIANTS_MAX,
};
use super::core_design::Htr10CoreDesign;
use super::core_shell::{ins, out, CoreFrame, CoreShell};
use super::reflector_geometry::Rgn;

/// HTR-10 pebble radius \[cm\] (6 cm balls). Every centre handed to
/// [`assemble_explicit_triso_from_centres`] is taken to be a ball of this
/// radius.
pub const PEBBLE_RADIUS_CM: f64 = 3.0;

/// Fuel-zone radius \[cm\] of a fuel pebble.
pub const FUEL_ZONE_RADIUS_CM: f64 = 2.5;

/// Overlap \[cm\] below which two balls count as touching and get no
/// bisector plane.
pub const TOUCH_TOLERANCE_CM: f64 = 1.0e-9;

/// Where a ball of an [`ExplicitBed`] came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplicitBallSource {
    /// The caller's centre list, at this index.
    Dem(usize),
    /// A Şeker discharge-tube ball below the DEM column (see the module docs).
    TubeFiller,
}

/// A pebble bed given ball by ball (normally a DEM fill), in the MC frame.
/// Built by [`assemble_explicit_triso_from_centres`]; see the module docs for
/// the frame, the overlap split and the tube.
#[derive(Debug, Clone)]
pub struct ExplicitBed {
    /// Ball radius \[cm\].
    pub radius: f64,
    /// Ball centres \[cm\], MC frame.
    pub centres: Vec<[f64; 3]>,
    /// Fuel flag per ball.
    pub fuel: Vec<bool>,
    /// Origin of each ball.
    pub source: Vec<ExplicitBallSource>,
    /// Overlapping pairs `(i, j)`, `i < j`, each split by its bisector plane.
    pub overlaps: Vec<(usize, usize)>,
    /// Balls per non-empty tile, keyed by `(a, b, level)`.
    pub tile_balls: BTreeMap<(i32, i32, i32), Vec<usize>>,
    /// Hex rings of the lattice.
    pub n_rings: usize,
    /// Axial lattice levels.
    pub n_levels: usize,
    /// z \[cm\] of the bottom face of lattice level 0.
    pub z_bottom: f64,
    /// Tile pitch (flat to flat) \[cm\].
    pub tile_pitch: f64,
    /// Tile height \[cm\].
    pub tile_height: f64,
    /// Bed cylinder radius \[cm\].
    pub bed_radius: f64,
    /// Bed floor \[cm\] (= top of the conus = DEM `z = 0`).
    pub bed_bottom: f64,
    /// Bed top \[cm\]: the top of the highest ball.
    pub bed_top: f64,
    /// Conus floor \[cm\].
    pub conus_floor: f64,
    /// Discharge-tube radius \[cm\].
    pub tube_radius: f64,
    /// Bottom of the container \[cm\] (the tube bottom, or the conus floor
    /// under the `OUTRAM_HTR10_HOMOG_TUBE` ablation).
    pub container_bottom: f64,
    /// Robust loading height \[cm\] above the floor: 99th percentile of the
    /// centre heights above the floor, plus one radius. Reported only.
    pub surface_height: f64,
    /// `z_MC - 100 z_DEM` \[cm\].
    pub dem_to_mc_dz: f64,
    /// The plane \[cm\] separating DEM balls (above) from tube filler balls
    /// (below); `None` when no filler is built.
    pub tube_cut: Option<f64>,
    /// Helium gap \[cm\] between the highest filler ball and `tube_cut`.
    pub tube_gap: f64,
    /// Largest pair overlap `2r - d` \[cm\] (0 if none).
    pub max_overlap: f64,
    /// Summed lens volume over the summed nominal ball volume \[-\].
    pub lens_volume_fraction: f64,
    /// Largest depth \[cm\] a ball reaches beyond the container wall (clipped
    /// by the bed envelope; 0 if none).
    pub max_wall_penetration: f64,
    /// DEM balls dropped (centre below the conus floor with no explicit tube).
    pub dropped_dem_balls: usize,
}

impl ExplicitBed {
    /// z \[cm\] of the lattice centre.
    #[must_use]
    pub fn lattice_centre_z(&self) -> f64 {
        self.z_bottom + 0.5 * self.n_levels as f64 * self.tile_height
    }

    /// Balls centred above the bed floor (the count Şeker's Table 3
    /// tabulates and the reference is matched by).
    #[must_use]
    pub fn core_balls(&self) -> usize {
        self.centres
            .iter()
            .filter(|c| c[2] > self.bed_bottom)
            .count()
    }

    /// Balls centred above the floor, and of which fuelled.
    #[must_use]
    pub fn eligible_and_fuel_balls(&self) -> (usize, usize) {
        let mut e = 0;
        let mut f = 0;
        for (c, &fu) in self.centres.iter().zip(&self.fuel) {
            if c[2] > self.bed_bottom {
                e += 1;
                f += usize::from(fu);
            }
        }
        (e, f)
    }

    /// Tile-local centre \[cm\] of tile `(a, b, level)`.
    #[must_use]
    pub fn tile_centre(&self, a: i32, b: i32, level: i32) -> [f64; 3] {
        let [x, y] = tile_xy(a, b, self.tile_pitch);
        [
            x,
            y,
            self.z_bottom + (f64::from(level) + 0.5) * self.tile_height,
        ]
    }
}

/// Volume \[cm^3\] of the lens shared by two spheres of radius `r` whose
/// centres are `d` apart (`d < 2r`).
#[must_use]
pub fn lens_volume(r: f64, d: f64) -> f64 {
    std::f64::consts::PI * (4.0 * r + d) * (2.0 * r - d).powi(2) / 12.0
}

/// **Fuel/dummy flags by the literature's rule**, for centres in the DEM
/// frame (`z = 0` the bed floor): balls centred above the floor take the
/// 57:43 split ([`super::table1::FUEL_BALL_FRACTION`]) by the same
/// low-discrepancy rule `SekerBed` uses, in order of height from the floor up
/// (then radius, then angle); balls centred in the conus or the tube are all
/// dummy (Terry 2005 sec. 2, TECDOC-1382 p. 235, Şeker & Çolak 2003 p. 267).
#[must_use]
pub fn paper_fuel_assignment(centres: &[[Length; 3]]) -> Vec<bool> {
    fuel_assignment_at(centres, super::table1::FUEL_BALL_FRACTION)
}

/// [`paper_fuel_assignment`] with fraction `f` (clamped to `[0, 1]`) of the
/// balls above the floor fuelled instead of 0.57 (gh:#566): the same
/// eligibility, order and low-discrepancy rule.
#[must_use]
pub fn fuel_assignment_at(centres: &[[Length; 3]], f: f64) -> Vec<bool> {
    let f = f.clamp(0.0, 1.0);
    let mut eligible: Vec<(f64, f64, f64, usize)> = centres
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            let [x, y, z] = c.map(|v| v.get::<centimeter>());
            (z > 0.0).then_some((z, x * x + y * y, y.atan2(x), i))
        })
        .collect();
    eligible.sort_by(|p, q| {
        p.0.total_cmp(&q.0)
            .then(p.1.total_cmp(&q.1))
            .then(p.2.total_cmp(&q.2))
            .then(p.3.cmp(&q.3))
    });
    let mut fuel = vec![false; centres.len()];
    for (n, &(_, _, _, i)) in eligible.iter().enumerate() {
        fuel[i] = ((n + 1) as f64 * f).floor() > (n as f64 * f).floor();
    }
    fuel
}

/// Distance \[cm\] from `p` to the hexagonal prism of a `HexOrientation::Y`
/// tile centred at `tc` (flat normals at 30° + 60° k, as [`tile_xy`]'s
/// neighbour directions), 0 inside.
fn hex_prism_distance(p: [f64; 3], tc: [f64; 3], pitch: f64, height: f64) -> f64 {
    let (qx, qy) = (p[0] - tc[0], p[1] - tc[1]);
    let apothem = 0.5 * pitch;
    let inside = (0..6).all(|k| {
        let t = (30.0 + 60.0 * f64::from(k)).to_radians();
        qx * t.cos() + qy * t.sin() <= apothem
    });
    let d2 = if inside {
        0.0
    } else {
        let rc = pitch / 3.0_f64.sqrt();
        let v = |k: i32| {
            let t = (60.0 * f64::from(k)).to_radians();
            [rc * t.cos(), rc * t.sin()]
        };
        (0..6)
            .map(|k| {
                let (a, b) = (v(k), v(k + 1));
                let (ex, ey) = (b[0] - a[0], b[1] - a[1]);
                let s =
                    (((qx - a[0]) * ex + (qy - a[1]) * ey) / (ex * ex + ey * ey)).clamp(0.0, 1.0);
                (qx - a[0] - s * ex).hypot(qy - a[1] - s * ey)
            })
            .fold(f64::INFINITY, f64::min)
    };
    let dz = ((p[2] - tc[2]).abs() - 0.5 * height).max(0.0);
    d2.hypot(dz)
}

/// The tile `(a, b)` whose hexagon holds the planar point `(x, y)`.
fn containing_tile(x: f64, y: f64, pitch: f64) -> (i32, i32) {
    let af = x / (0.5 * 3.0_f64.sqrt() * pitch);
    let bf = y / pitch - 0.5 * af;
    let (a0, b0) = (af.round() as i32, bf.round() as i32);
    let mut best = (a0, b0, f64::INFINITY);
    for a in a0 - 1..=a0 + 1 {
        for b in b0 - 1..=b0 + 1 {
            let [tx, ty] = tile_xy(a, b, pitch);
            let d = (x - tx).powi(2) + (y - ty).powi(2);
            if d < best.2 {
                best = (a, b, d);
            }
        }
    }
    (best.0, best.1)
}

/// Distance from `(px, pz)` to the segment `a`-`b` in the meridional plane.
fn segment_distance(px: f64, pz: f64, a: [f64; 2], b: [f64; 2]) -> f64 {
    let (ex, ez) = (b[0] - a[0], b[1] - a[1]);
    let l2 = ex * ex + ez * ez;
    let s = if l2 > 0.0 {
        (((px - a[0]) * ex + (pz - a[1]) * ez) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (px - a[0] - s * ex).hypot(pz - a[1] - s * ez)
}

impl ExplicitBed {
    /// The container's meridional profile `(rho, z)`, as `SekerBed`'s.
    fn profile(&self) -> Vec<[f64; 2]> {
        let mut p = vec![
            [0.0, self.bed_top],
            [self.bed_radius, self.bed_top],
            [self.bed_radius, self.bed_bottom],
            [self.tube_radius, self.conus_floor],
        ];
        if self.container_bottom < self.conus_floor {
            p.push([self.tube_radius, self.container_bottom]);
        }
        p.push([0.0, self.container_bottom]);
        p
    }

    /// Whether the centre `c` is inside the container, and its distance to
    /// the container wall (the axis excluded).
    fn wall_distance(&self, c: [f64; 3]) -> (bool, f64) {
        let (rho, z) = (c[0].hypot(c[1]), c[2]);
        let p = self.profile();
        let mut inside = false;
        let mut dmin = f64::INFINITY;
        for i in 0..p.len() {
            let (u, v) = (p[i], p[(i + 1) % p.len()]);
            if (u[1] > z) != (v[1] > z) && rho < u[0] + (z - u[1]) / (v[1] - u[1]) * (v[0] - u[0]) {
                inside = !inside;
            }
            if i + 1 < p.len() {
                dmin = dmin.min(segment_distance(rho, z, u, v));
            }
        }
        (inside, dmin)
    }

    /// Build the bed description from DEM-frame centres in cm.
    #[allow(clippy::too_many_lines)]
    fn build(
        dem_cm: &[[f64; 3]],
        is_fuel: &[bool],
        n_rings: usize,
        bed_half_height: f64,
        explicit_tube: bool,
        r_fuel_zone: f64,
    ) -> Self {
        let r = PEBBLE_RADIUS_CM;
        let cell = SekerCell::from_paper();
        let (tile_pitch, tile_height) = (cell.pitch(), cell.height);
        let bed_radius = HTR10_CORE_RADIUS_CM;
        let bed_bottom = -bed_half_height;
        let bed_top = bed_half_height;
        let conus_floor = bed_bottom - HTR10_CONUS_HEIGHT_CM;
        let container_bottom = if explicit_tube {
            conus_floor - HTR10_BOTTOM_REFLECTOR_CM
        } else {
            conus_floor
        };
        let z_bottom = container_bottom - 0.5 * tile_height;
        let n_levels = ((bed_top + 0.5 * tile_height - z_bottom) / tile_height).ceil() as usize;

        // Robust surface: 99th percentile of the centre heights above the floor.
        let mut above: Vec<f64> = dem_cm.iter().map(|c| c[2]).filter(|&z| z > 0.0).collect();
        above.sort_by(f64::total_cmp);
        let surface_height = if above.is_empty() {
            0.0
        } else {
            let k = ((0.99 * (above.len() - 1) as f64).round() as usize).min(above.len() - 1);
            above[k] + r
        };

        let mut bed = Self {
            radius: r,
            centres: Vec::with_capacity(dem_cm.len()),
            fuel: Vec::with_capacity(dem_cm.len()),
            source: Vec::with_capacity(dem_cm.len()),
            overlaps: Vec::new(),
            tile_balls: BTreeMap::new(),
            n_rings,
            n_levels,
            z_bottom,
            tile_pitch,
            tile_height,
            bed_radius,
            bed_bottom,
            bed_top,
            conus_floor,
            tube_radius: HTR10_DISCHARGE_TUBE_RADIUS_CM,
            container_bottom,
            surface_height,
            dem_to_mc_dz: -bed_half_height,
            tube_cut: None,
            tube_gap: 0.0,
            max_overlap: 0.0,
            lens_volume_fraction: 0.0,
            max_wall_penetration: 0.0,
            dropped_dem_balls: 0,
        };
        for (i, (c, &f)) in dem_cm.iter().zip(is_fuel).enumerate() {
            let z = c[2] - bed_half_height;
            if !explicit_tube && z < conus_floor {
                bed.dropped_dem_balls += 1;
                continue;
            }
            bed.centres.push([c[0], c[1], z]);
            bed.fuel.push(f);
            bed.source.push(ExplicitBallSource::Dem(i));
        }
        // Containment: a centre outside the container is a frame error.
        for (k, &c) in bed.centres.iter().enumerate() {
            let (inside, d) = bed.wall_distance(c);
            assert!(
                inside,
                "ball {k} centred at ({:.4}, {:.4}, {:.4}) cm (MC frame) lies outside the \
                 HTR-10 container: are the centres in the DEM frame (metres, z = 0 the bed floor)?",
                c[0], c[1], c[2]
            );
            bed.max_wall_penetration = bed.max_wall_penetration.max(r - d);
        }

        // Tube filler: Şeker's discharge-tube balls below the DEM column.
        if explicit_tube {
            let lowest = bed
                .centres
                .iter()
                .map(|c| c[2] - r)
                .fold(f64::INFINITY, f64::min);
            let cut = lowest.min(conus_floor);
            let sb = SekerBed::new(
                cell,
                n_rings,
                0,
                HTR10_CONUS_HEIGHT_CM,
                bed_radius,
                HTR10_DISCHARGE_TUBE_RADIUS_CM,
                Some(HTR10_BOTTOM_REFLECTOR_CM),
                FuelAssignment::Paper,
            );
            let shift = conus_floor - sb.conus_floor;
            let mut top = container_bottom;
            for id in sb.all_balls() {
                if !sb.is_present(id) {
                    continue;
                }
                let [x, y, z0] = sb.centre(id);
                let z = z0 + shift;
                if z < conus_floor && z + r <= cut {
                    top = top.max(z + r);
                    bed.centres.push([x, y, z]);
                    bed.fuel.push(false);
                    bed.source.push(ExplicitBallSource::TubeFiller);
                }
            }
            bed.tube_cut = Some(cut);
            bed.tube_gap = cut - top;
        }

        // Overlapping pairs, through a 2r spatial hash.
        let bin = 2.0 * r;
        let key = |p: &[f64; 3]| p.map(|v| (v / bin).floor() as i64);
        let mut grid: BTreeMap<[i64; 3], Vec<usize>> = BTreeMap::new();
        for (i, c) in bed.centres.iter().enumerate() {
            grid.entry(key(c)).or_default().push(i);
        }
        let mut lens = 0.0;
        let mut dmin = f64::INFINITY;
        for (i, c) in bed.centres.iter().enumerate() {
            let k = key(c);
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        let Some(v) = grid.get(&[k[0] + dx, k[1] + dy, k[2] + dz]) else {
                            continue;
                        };
                        for &j in v {
                            if j <= i {
                                continue;
                            }
                            let q = bed.centres[j];
                            let d = ((c[0] - q[0]).powi(2)
                                + (c[1] - q[1]).powi(2)
                                + (c[2] - q[2]).powi(2))
                            .sqrt();
                            dmin = dmin.min(d);
                            if d < 2.0 * r - TOUCH_TOLERANCE_CM {
                                assert!(
                                    d >= 2.0 * r_fuel_zone,
                                    "balls {i} and {j} are {d:.4} cm apart: the bisector would cut a \
                                     {r_fuel_zone} cm fuel zone. Refusing the bed rather than \
                                     clipping fuel."
                                );
                                lens += lens_volume(r, d);
                                bed.overlaps.push((i, j));
                            }
                        }
                    }
                }
            }
        }
        bed.max_overlap = (2.0 * r - dmin).max(0.0);
        let v_ball = 4.0 / 3.0 * std::f64::consts::PI * r.powi(3);
        bed.lens_volume_fraction = lens / (v_ball * bed.centres.len() as f64);

        // Every tile each ball intersects.
        let nr = n_rings as i32;
        let neighbours = [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1), (1, -1), (-1, 1)];
        for (i, c) in bed.centres.iter().enumerate() {
            let (a0, b0) = containing_tile(c[0], c[1], tile_pitch);
            let l0 = ((c[2] - z_bottom) / tile_height).floor() as i32;
            for (da, db) in neighbours {
                for dl in -1..=1 {
                    let (a, b, l) = (a0 + da, b0 + db, l0 + dl);
                    let [tx, ty] = tile_xy(a, b, tile_pitch);
                    let tz = z_bottom + (f64::from(l) + 0.5) * tile_height;
                    if hex_prism_distance(*c, [tx, ty, tz], tile_pitch, tile_height) >= r {
                        continue;
                    }
                    assert!(
                        hex_ring(a, b) < n_rings && l >= 0 && (l as usize) < n_levels,
                        "ball {i} reaches tile ({a}, {b}, {l}) outside the {nr}-ring, \
                         {n_levels}-level lattice"
                    );
                    bed.tile_balls.entry((a, b, l)).or_default().push(i);
                }
            }
        }
        bed
    }
}

/// **Assemble the HTR-10 core on an explicit pebble list** (a DEM fill).
///
/// # Parameters
/// - `centres` — pebble centres in the **DEM frame**: `z = 0` the bed floor
///   (top of the conus), the conus and the discharge tube below it, the core
///   axis at `x = y = 0`. Every pebble is an HTR-10 6 cm ball.
/// - `is_fuel` — one flag per centre ([`paper_fuel_assignment`] gives the
///   literature's 57:43 split with an all-dummy conus and tube).
/// - `n_rings` — a FLOOR on the bed-lattice ring count, as for
///   [`super::core_model::assemble_explicit_triso`] (the lattice always tiles
///   the 90 cm core).
/// - `majorant_index` — the bed's delta-tracking majorant (`usize::MAX`
///   surface-tracks the bed).
///
/// Everything outside the bed is [`super::core_shell`], the code
/// `assemble_explicit_triso` uses. The ablation knobs that act on the
/// envelope (`OUTRAM_HTR10_NOREFL`, `_REFLECTIVE`, `_HOMOG_TUBE`,
/// `_NO_ZONE_MAP`, `_NO_WITHDRAWN_RODS`) apply; the fuel-identity knobs
/// (`_ALLFUEL`, `_FUEL_CONUS`) do not, because the caller gives the identity.
///
/// # Panics
/// If the lengths differ, the list is empty, a centre lies outside the
/// container, two centres are closer than 5.0 cm (the fuel zone would be
/// cut), the bed is taller than the core cavity, or a tile would hold more
/// than [`TILE_SITE_STRIDE`] balls.
#[must_use]
pub fn assemble_explicit_triso_from_centres(
    centres: &[[Length; 3]],
    is_fuel: &[bool],
    n_rings: usize,
    majorant_index: usize,
) -> AssembledCore {
    assemble_explicit_triso_from_centres_with(
        centres,
        is_fuel,
        n_rings,
        majorant_index,
        &Htr10CoreDesign::default(),
    )
}

/// [`assemble_explicit_triso_from_centres`] built to `design` (gh:#566,
/// gh:#580): its fuel-zone radius, TRISO radii and count, and rod insertion.
/// The fuel identity is the caller's `is_fuel` (use [`fuel_assignment_at`]
/// for the design's fraction); the design's `fuel_ball_fraction` is not read
/// here. Default design: the same geometry as before it existed.
///
/// # Panics
/// As [`assemble_explicit_triso_from_centres`], and if `design` fails
/// [`Htr10CoreDesign::check`]; two centres must be at least twice the
/// design's fuel-zone radius apart.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn assemble_explicit_triso_from_centres_with(
    centres: &[[Length; 3]],
    is_fuel: &[bool],
    n_rings: usize,
    majorant_index: usize,
    design: &Htr10CoreDesign,
) -> AssembledCore {
    let r_fz = design.fuel_zone_radius_cm;
    assert_eq!(centres.len(), is_fuel.len(), "one fuel flag per centre");
    assert!(!centres.is_empty(), "no pebbles");
    let r = PEBBLE_RADIUS_CM;
    let cell = SekerCell::from_paper();
    let (lat_pitch, lat_height) = (cell.pitch(), cell.height);
    let bed_radius = HTR10_CORE_RADIUS_CM;
    // The same ring rule as `assemble_explicit_triso`: tile the whole cylinder.
    let ring_reach = (3.0_f64.sqrt() / 2.0) * lat_pitch;
    let n_rings = n_rings.max((bed_radius / ring_reach).ceil() as usize + 1);

    let dem_cm: Vec<[f64; 3]> = centres
        .iter()
        .map(|c| c.map(|v| v.get::<centimeter>()))
        .collect();
    let h_top = dem_cm
        .iter()
        .map(|c| c[2] + r)
        .fold(f64::NEG_INFINITY, f64::max)
        .max(2.0 * r);
    assert!(
        h_top < HTR10_CORE_CAVITY_CM,
        "the bed top ({h_top:.2} cm above the floor) is above the core cavity"
    );
    let bed_half_height = 0.5 * h_top;
    let frame = CoreFrame::new(bed_radius, bed_half_height);
    let bed = ExplicitBed::build(
        &dem_cm,
        is_fuel,
        n_rings,
        bed_half_height,
        frame.explicit_tube,
        r_fz,
    );

    let mut sh = CoreShell::new(&frame, [0.0; 3], design, r, majorant_index);
    debug_assert!((sh.conus_floor - bed.conus_floor).abs() < 1e-9);
    // The reflector first: its z-plane de-duplication scans every surface, and
    // the tiles below add hundreds of thousands. Only the order of entries in
    // the surface and cell lists differs from the Şeker path.
    sh.add_reflector();
    let mut surfaces = std::mem::take(&mut sh.surfaces);
    let mut cells = std::mem::take(&mut sh.cells);
    let mut universes = std::mem::take(&mut sh.universes);

    // Empty tiles and the lattice `outer`: all helium. Two cells, because an
    // empty region would be all space and one sphere's inside/outside covers
    // the tile (surface 6, the site-0 pebble sphere at the tile centre).
    let helium_u = universes.len();
    let mut idx = Vec::new();
    for rg in [Rgn::ins(6), Rgn::out(6)] {
        idx.push(cells.len());
        cells.push(Cell::material(
            TILE_HELIUM_CELL_ID,
            rg.0,
            mat::HELIUM,
            293.6,
        ));
    }
    universes.push(Universe {
        id: helium_u as i32,
        cell_indices: idx,
    });

    let mut partners: Vec<Vec<usize>> = vec![Vec::new(); bed.centres.len()];
    for &(i, j) in &bed.overlaps {
        partners[i].push(j);
    }
    let sphere = |c: [f64; 3], rad: f64| {
        SurfaceKind::Sphere(Sphere {
            x0: c[0],
            y0: c[1],
            z0: c[2],
            r: rad,
            bc: BoundaryType::Transmissive,
        })
    };
    let mut tile_universe: Vec<((i32, i32, i32), usize)> = Vec::with_capacity(bed.tile_balls.len());
    for (ord, (&(a, b, l), balls)) in bed.tile_balls.iter().enumerate() {
        let v = ord + 1; // variant 0 is the shared helium universe
        assert!(
            v < TILE_VARIANTS_MAX,
            "tile cell ids hold at most {TILE_VARIANTS_MAX} variants"
        );
        assert!(
            balls.len() < TILE_SITE_STRIDE as usize,
            "tile ({a}, {b}, {l}) holds {} balls, the id stride {TILE_SITE_STRIDE}",
            balls.len()
        );
        let base = TILE_SITE_STRIDE * v as i32;
        let tc = bed.tile_centre(a, b, l);
        let local = |i: usize| {
            let c = bed.centres[i];
            [c[0] - tc[0], c[1] - tc[1], c[2] - tc[2]]
        };
        let site: BTreeMap<usize, usize> = balls.iter().enumerate().map(|(k, &i)| (i, k)).collect();
        // Bisector planes of the overlapping pairs that are both in this tile.
        // If only one ball of a pair reaches the tile, the lens (inside both)
        // is outside the tile and needs no plane here.
        let mut cuts: Vec<Vec<RegionToken>> = vec![Vec::new(); balls.len()];
        for (k, &i) in balls.iter().enumerate() {
            for &j in &partners[i] {
                let Some(&kj) = site.get(&j) else { continue };
                let (ci, cj) = (local(i), local(j));
                let e = [cj[0] - ci[0], cj[1] - ci[1], cj[2] - ci[2]];
                let d = (e[0] * e[0] + e[1] * e[1] + e[2] * e[2]).sqrt();
                let n = e.map(|x| x / d);
                let m = [
                    0.5 * (ci[0] + cj[0]),
                    0.5 * (ci[1] + cj[1]),
                    0.5 * (ci[2] + cj[2]),
                ];
                let s = surfaces.len();
                surfaces.push(SurfaceKind::Plane(Plane {
                    a: n[0],
                    b: n[1],
                    c: n[2],
                    d: n[0] * m[0] + n[1] * m[1] + n[2] * m[2],
                    bc: BoundaryType::Transmissive,
                }));
                // Inside (n.x < n.m) is ball i's side, Outside ball j's.
                cuts[k].push(ins(s));
                cuts[kj].push(out(s));
            }
        }
        let mut idx = Vec::new();
        let mut helium: Option<Rgn> = None;
        for (k, &i) in balls.iter().enumerate() {
            let lc = local(i);
            let id = base + k as i32;
            let pb = surfaces.len();
            surfaces.push(sphere(lc, r));
            let clip = |rg: Rgn| cuts[k].iter().fold(rg, |acc, &t| acc.and(Rgn(vec![t])));
            if bed.fuel[i] {
                let fz = surfaces.len();
                surfaces.push(sphere(lc, r_fz));
                idx.push(cells.len());
                cells.push(Cell::fill(
                    TILE_FUEL_ZONE_CELL_ID + id,
                    vec![ins(fz)],
                    CellFill::Lattice(1),
                    Position::new(lc[0], lc[1], lc[2]),
                ));
                idx.push(cells.len());
                cells.push(Cell::material(
                    TILE_FUEL_SHELL_CELL_ID + id,
                    clip(Rgn::out(fz).and(Rgn::ins(pb))).0,
                    mat::GRAPHITE,
                    293.6,
                ));
            } else {
                idx.push(cells.len());
                cells.push(Cell::material(
                    TILE_DUMMY_BALL_CELL_ID + id,
                    clip(Rgn::ins(pb)).0,
                    mat::GRAPHITE,
                    293.6,
                ));
            }
            helium = Some(match helium {
                None => Rgn::out(pb),
                Some(h) => h.and(Rgn::out(pb)),
            });
        }
        let helium = helium.expect("a tile in the map holds a ball");
        idx.push(cells.len());
        cells.push(Cell::material(
            TILE_HELIUM_CELL_ID + base,
            helium.0,
            mat::HELIUM,
            293.6,
        ));
        let u = universes.len();
        universes.push(Universe {
            id: u as i32,
            cell_indices: idx,
        });
        tile_universe.push(((a, b, l), u));
    }

    let nr = bed.n_rings as i32;
    let placeholder: Vec<Vec<Vec<usize>>> = (0..bed.n_levels)
        .map(|_| {
            (0..bed.n_rings)
                .rev()
                .map(|q| vec![helium_u; if q == 0 { 1 } else { 6 * q }])
                .collect()
        })
        .collect();
    let mut bed_lattice = HexLattice::from_rings_3d(
        0,
        HexOrientation::Y,
        Position::new(0.0, 0.0, bed.lattice_centre_z()),
        lat_pitch,
        lat_height,
        &placeholder,
        Some(helium_u),
    );
    for &((a, b, l), u) in &tile_universe {
        let i = [a + nr - 1, b + nr - 1, l];
        debug_assert!(bed_lattice.are_valid_indices(i));
        let flat = bed_lattice.flat_index(i);
        bed_lattice.universes[flat] = u as i32;
    }
    let tiles = bed.n_levels * (3 * bed.n_rings * (bed.n_rings - 1) + 1);

    sh.surfaces = surfaces;
    sh.cells = cells;
    sh.universes = universes;
    sh.finish(
        bed_lattice,
        tiles,
        lat_pitch,
        lat_height,
        PebbleBed::Explicit(bed),
    )
}

/// Why a DEM bed could not be read or cut ([`read_dem_centres_csv`],
/// [`trim_to_core_balls`]).
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DemBedError {
    /// The first line is not the `id,x,y,z,...` header of
    /// `reference-data/liggghts/`.
    #[error("unexpected DEM CSV header {0:?} (want `id,x,y,z,...`)")]
    Header(String),
    /// A row has too few fields or a field is not a number.
    #[error("DEM CSV line {line}: {why}")]
    Row { line: usize, why: String },
    /// The bed holds fewer balls above the floor than were asked for.
    #[error("the bed holds {available} balls above the floor, {wanted} were asked for")]
    TooFewBalls { available: usize, wanted: usize },
}

/// Pebble centres, DEM frame, from the text of an `id,x,y,z,...` CSV (the
/// `reference-data/liggghts/` format, SI metres, e.g.
/// `htr10_conus_presettled_mu10_mur00.csv`), in file order.
///
/// # Errors
/// [`DemBedError::Header`] or [`DemBedError::Row`] on a malformed file.
pub fn read_dem_centres_csv(text: &str) -> Result<Vec<[Length; 3]>, DemBedError> {
    let mut lines = text.lines();
    let header = lines.next().unwrap_or_default();
    if !header.starts_with("id,x,y,z") {
        return Err(DemBedError::Header(header.to_string()));
    }
    let mut out = Vec::new();
    for (i, l) in lines.enumerate() {
        if l.trim().is_empty() {
            continue;
        }
        let f: Vec<f64> = l
            .split(',')
            .skip(1)
            .take(3)
            .map(|v| v.trim().parse::<f64>())
            .collect::<Result<_, _>>()
            .map_err(|e| DemBedError::Row {
                line: i + 2,
                why: e.to_string(),
            })?;
        if f.len() < 3 {
            return Err(DemBedError::Row {
                line: i + 2,
                why: format!("{} coordinates, need 3", f.len()),
            });
        }
        out.push([f[0], f[1], f[2]].map(Length::new::<uom::si::length::meter>));
    }
    Ok(out)
}

/// **A poured bed cut to a ball count** (gh:#787): every centre at or below
/// the bed floor (`z <= 0`, DEM frame: the conus and the tube) is kept, and of
/// the centres above it the lowest `core_balls`, ordered by height, then `x`,
/// then `y`. This is the rule of the web demo's beds view
/// (`dhoby-ghaut/examples/common/htr10_beds.rs`, `Bed::trimmed_to_core`), so a
/// `k` computed on the result is the `k` of the bed that view draws. The top
/// of the cut bed is a flat cut through the packing, not a poured surface.
///
/// Returned in that order: the kept floor-and-below centres in input order,
/// then the core centres from the floor up.
///
/// # Errors
/// [`DemBedError::TooFewBalls`] if fewer than `core_balls` centres lie above
/// the floor.
pub fn trim_to_core_balls(
    centres: &[[Length; 3]],
    core_balls: usize,
) -> Result<Vec<[Length; 3]>, DemBedError> {
    let mut below: Vec<[Length; 3]> = centres
        .iter()
        .copied()
        .filter(|c| c[2].value <= 0.0)
        .collect();
    let mut above: Vec<[Length; 3]> = centres
        .iter()
        .copied()
        .filter(|c| c[2].value > 0.0)
        .collect();
    if above.len() < core_balls {
        return Err(DemBedError::TooFewBalls {
            available: above.len(),
            wanted: core_balls,
        });
    }
    above.sort_by(|a, b| {
        a[2].value
            .total_cmp(&b[2].value)
            .then(a[0].value.total_cmp(&b[0].value))
            .then(a[1].value.total_cmp(&b[1].value))
    });
    above.truncate(core_balls);
    below.extend(above);
    Ok(below)
}

#[cfg(test)]
#[path = "explicit_bed_tests.rs"]
mod tests;
