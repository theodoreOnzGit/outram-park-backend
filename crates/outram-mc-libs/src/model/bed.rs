//! **Which balls of a stacked hexagonal lattice are loaded** (tutorial 8 on
//! #943, epic #925): whole-ball rejection against a [`Hopper`], the fuel and
//! moderator identity of every kept ball, and the census of both.
//!
//! This is the arithmetic of `nee_soon::htr10_rmc::bed::SekerBed`
//! (gh:#472), ported for any tile of balls on a hexagonal lattice:
//!
//! - **axial layout** (`SekerBed::new`): a basal plane of ball centres sits
//!   one radius above the hopper's floor, the next `m` tile heights above it;
//!   the lattice reaches from one tile below the hopper's bottom to two above
//!   its top;
//! - **tile centres** (`bed::tile_xy`): `[sqrt(3)/2 a p, (b + a/2) p]` for
//!   skewed coordinates `(a, b)` of a lattice of flat-to-flat pitch `p` whose
//!   faces are normal to y;
//! - **rejection** (`SekerBed::reject`, `containment`): a ball is kept only
//!   if it lies wholly inside the hopper filled to the bed top
//!   ([`Hopper::contains_ball`]). Şeker & Çolak (2003) p.267: *"If any ball
//!   intersects with the reflector surface, it is rejected"*, *"Balls
//!   intersect with cone or discharge tube surface are rejected."* Nothing is
//!   ever cut (a cut pebble is wrong physics, maintainer 2026-10-01);
//! - **identity** (`SekerBed::assign`): of the kept balls whose centre is
//!   above the floor, ordered by half-layer, then distance from the axis
//!   squared, then angle, ball `n` (from 0) is fuel when `floor((n+1) f) >
//!   floor(n f)`, `f` the fuel fraction; every ball below the floor (the cone
//!   and the tube) is moderator (Terry et al. 2005 section 2; TECDOC-1382
//!   p.235; Şeker & Çolak p.267).
//!
//! A ball is named by the tile that owns it: skewed coordinates `(a, b)`, the
//! tile's level, and which of the tile's balls it is.

use std::collections::HashMap;

use super::hopper::{Hopper, Wall};

/// A ball of the stack: the tile that owns it `(a, b, level)` and which of
/// that tile's balls it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct BallId {
    pub(crate) a: i32,
    pub(crate) b: i32,
    pub(crate) level: i32,
    pub(crate) ball: usize,
}

/// One of the pieces of balls a tile draws: where, relative to the tile, the
/// tile that owns the ball sits, and which of its balls.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PieceGeom {
    /// Tile-local centre of the piece \[cm\].
    pub(crate) local: [f64; 3],
    /// Owner tile relative to this one: `(da, db, dlevel)`.
    pub(crate) owner: [i32; 3],
    /// Which of the owner's balls.
    pub(crate) ball: usize,
}

/// Hexagonal ring (0 = centre) of skewed coordinates `(a, b)`
/// (`nee_soon::htr10_rmc::bed::hex_ring`).
pub(crate) fn hex_ring(a: i32, b: i32) -> usize {
    ((a.abs() + b.abs() + (a + b).abs()) / 2) as usize
}

/// Planar centre of tile `(a, b)` in a lattice of flat-to-flat pitch `p`
/// with faces normal to y, centred on the axis
/// (`nee_soon::htr10_rmc::bed::tile_xy`, the arithmetic of
/// `HexLattice::center_offset`).
pub(crate) fn tile_xy(a: i32, b: i32, p: f64) -> [f64; 2] {
    let (a, b) = (f64::from(a), f64::from(b));
    [0.5 * 3.0_f64.sqrt() * a * p, (b + 0.5 * a) * p]
}

/// **A ball the stack rejected** (its centre is inside the hopper, but the
/// ball would cross a wall), for the picture and the census.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RejectedBall {
    /// Centre \[cm\].
    pub centre_cm: [f64; 3],
    /// The wall it would have crossed (the nearest).
    pub wall: Wall,
}

/// **A ball the stack kept**: where it is and which kind it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeptBall {
    /// Centre \[cm\].
    pub centre_cm: [f64; 3],
    /// Fuel (`true`) or moderator.
    pub fuel: bool,
}

/// **What a stacked bed holds, counted from the lattice the model was
/// assembled with** ([`super::AssembledModel::bed_census`]): the balls kept,
/// the balls rejected and why, the fuel and moderator split, the bed's
/// height as Şeker & Çolak define it and the filling fraction.
///
/// A ball is **rejected** when its centre is inside the container but the
/// ball would cross a wall, the top of the loaded bed, the cone or the tube.
/// Balls whose centre is already outside the container are simply not
/// there, and are not counted as rejected (they are the rest of the
/// lattice).
#[derive(Debug, Clone, PartialEq)]
pub struct BedCensus {
    /// The floor of the bed (the top of the cone) \[cm\].
    pub floor_z_cm: f64,
    /// The top the bed was loaded to \[cm\].
    pub requested_top_cm: f64,
    /// Ball radius \[cm\].
    pub ball_radius_cm: f64,
    /// Balls kept anywhere in the container: the bed, the cone and the tube.
    pub loaded_balls: usize,
    /// Of those, the balls centred above the floor: the bed proper, which
    /// is the count Şeker & Çolak's Table 3 tabulates.
    pub bed_balls: usize,
    /// Balls kept in the cone and the tube (all moderator).
    pub cone_and_tube_balls: usize,
    /// Fuel balls (all in the bed).
    pub fuel_balls: usize,
    /// Moderator balls in the bed.
    pub moderator_balls_in_bed: usize,
    /// **Rejected balls**, total: centre inside the container, ball crossing
    /// a wall.
    pub rejected_balls: usize,
    /// Of those, by the wall that was nearest.
    pub rejected_at_top: usize,
    /// ... the cylinder's side wall.
    pub rejected_at_side_wall: usize,
    /// ... the cone.
    pub rejected_at_cone: usize,
    /// ... the outlet tube's wall.
    pub rejected_at_tube_wall: usize,
    /// ... the bottom of the tube.
    pub rejected_at_tube_bottom: usize,
    /// Rejected balls whose centre is above the floor (the bed proper).
    pub rejected_in_bed: usize,
    /// Every rejected ball, for drawing.
    pub rejected: Vec<RejectedBall>,
    /// Every ball kept, in the bed, the cone and the tube, for drawing and
    /// for checking against the geometry.
    pub kept: Vec<KeptBall>,
    /// **The bed's height as Şeker & Çolak define it**: from the bottom of
    /// the lowest ball above the floor to the top of the highest kept ball
    /// \[cm\] (`9.798 N + 6` cm for `N` layers of 6 cm balls).
    pub bed_height_cm: f64,
    /// Balls kept above the floor, per plane of ball centres, lowest first:
    /// `(z of the plane, balls)`. Şeker's Table 3 has 733 per basal plane
    /// and 613 per central plane.
    pub planes: Vec<(f64, usize)>,
    /// Volume of the kept bed balls over the volume of the bed cylinder
    /// (radius of the hopper, from the floor to the top of the highest
    /// ball): the filling fraction (Şeker's "61 %").
    pub filling_fraction: f64,
    /// Lattice rings and axial levels the lattice was sized to.
    pub rings: usize,
    /// See [`Self::rings`].
    pub levels: usize,
}

impl BedCensus {
    /// `loaded bed balls - reference`, signed: how many fewer (negative) or
    /// more balls the whole-ball rule keeps than a reference count at the
    /// same height.
    pub fn bed_balls_against(&self, reference: usize) -> i64 {
        self.bed_balls as i64 - reference as i64
    }
}

impl std::fmt::Display for BedCensus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "bed: {} balls kept above the floor ({} fuel, {} moderator), {} more in the cone and \
             tube (all moderator); height {:.3} cm (asked to load to {:.3} cm above z = {:.3} cm), \
             filling fraction {:.4}",
            self.bed_balls,
            self.fuel_balls,
            self.moderator_balls_in_bed,
            self.cone_and_tube_balls,
            self.bed_height_cm,
            self.requested_top_cm - self.floor_z_cm,
            self.floor_z_cm,
            self.filling_fraction
        )?;
        write!(
            f,
            "rejected whole (centre inside, ball would cross a wall): {} = {} at the side wall, {} \
             at the cone, {} at the tube wall, {} at the tube bottom, {} at the top; {} of them in \
             the bed proper",
            self.rejected_balls,
            self.rejected_at_side_wall,
            self.rejected_at_cone,
            self.rejected_at_tube_wall,
            self.rejected_at_tube_bottom,
            self.rejected_at_top,
            self.rejected_in_bed
        )
    }
}

/// **The layout of a stacked bed**: which balls are kept, which are fuel.
#[derive(Debug, Clone)]
pub(crate) struct BedLayout {
    pub(crate) pitch: f64,
    pub(crate) height: f64,
    pub(crate) ball_r: f64,
    pub(crate) n_rings: usize,
    pub(crate) n_levels: usize,
    /// z of the bottom face of level 0.
    pub(crate) z_bottom: f64,
    hopper: Hopper,
    bed_top: f64,
    /// Tile-local centres of the balls a tile owns.
    sites: Vec<[f64; 3]>,
    present: HashMap<BallId, bool>,
    fuel: HashMap<BallId, bool>,
    pub(crate) census: BedCensus,
}

impl BedLayout {
    /// Lay the bed out. `sites` are the tile-local centres of the balls a
    /// tile owns; `pitch`, `height` the tile's; `fuel_fraction` in `[0, 1]`.
    pub(crate) fn compute(
        hopper: Hopper,
        sites: Vec<[f64; 3]>,
        ball_r: f64,
        pitch: f64,
        height: f64,
        fuel_fraction: f64,
        bed_top: f64,
    ) -> Self {
        let floor = hopper.floor_z_cm();
        // SekerBed::new: a basal plane of ball centres one radius above the
        // floor; one margin level below the container, two above.
        let face0 = floor + ball_r;
        let m_lo = ((hopper.bottom_z_cm() - face0) / height).floor() as i32 - 1;
        let m_hi = ((hopper.top_z_cm() - face0) / height).floor() as i32 + 2;
        let n_levels = (m_hi - m_lo) as usize;
        // core_model::assemble_explicit_triso_with: rings to tile the bed
        // cylinder, `(R / (sqrt(3)/2 pitch)).ceil() + 1`.
        let ring_reach = (3.0_f64.sqrt() / 2.0) * pitch;
        let n_rings = (hopper.radius_cm() / ring_reach).ceil() as usize + 1;
        let mut layout = Self {
            pitch,
            height,
            ball_r,
            n_rings,
            n_levels,
            z_bottom: face0 + f64::from(m_lo) * height,
            hopper,
            bed_top,
            sites,
            present: HashMap::new(),
            fuel: HashMap::new(),
            census: BedCensus {
                floor_z_cm: floor,
                requested_top_cm: bed_top,
                ball_radius_cm: ball_r,
                loaded_balls: 0,
                bed_balls: 0,
                cone_and_tube_balls: 0,
                fuel_balls: 0,
                moderator_balls_in_bed: 0,
                rejected_balls: 0,
                rejected_at_top: 0,
                rejected_at_side_wall: 0,
                rejected_at_cone: 0,
                rejected_at_tube_wall: 0,
                rejected_at_tube_bottom: 0,
                rejected_in_bed: 0,
                rejected: Vec::new(),
                kept: Vec::new(),
                bed_height_cm: 0.0,
                planes: Vec::new(),
                filling_fraction: 0.0,
                rings: n_rings,
                levels: n_levels,
            },
        };
        layout.reject();
        layout.assign(fuel_fraction);
        layout.count();
        layout
    }

    /// z of the lattice centre, for the CSG lattice
    /// (`SekerBed::lattice_centre_z`).
    pub(crate) fn lattice_centre_z(&self) -> f64 {
        self.z_bottom + 0.5 * self.n_levels as f64 * self.height
    }

    /// Global centre \[cm\] of ball `id` (`SekerBed::centre`).
    pub(crate) fn centre(&self, id: BallId) -> [f64; 3] {
        let [tx, ty] = tile_xy(id.a, id.b, self.pitch);
        let [lx, ly, lz] = self.sites[id.ball];
        let tz = self.z_bottom + (f64::from(id.level) + 0.5) * self.height;
        [tx + lx, ty + ly, tz + lz]
    }

    /// Whether ball `id` is kept (any ball in space; one outside the lattice
    /// is simply not inside the hopper).
    pub(crate) fn is_present(&self, id: BallId) -> bool {
        match self.present.get(&id) {
            Some(&p) => p,
            None => {
                self.hopper
                    .contains_ball(self.centre(id), self.ball_r, self.bed_top)
                    .whole
            }
        }
    }

    /// Whether ball `id` is fuel.
    pub(crate) fn is_fuel(&self, id: BallId) -> bool {
        self.fuel.get(&id).copied().unwrap_or(false)
    }

    /// The owned balls of every tile within the lattice's rings, in a fixed
    /// order: levels, then `a`, then `b`, then the tile's ball index.
    pub(crate) fn all_balls(&self) -> Vec<BallId> {
        let nr = self.n_rings as i32;
        let mut out = Vec::new();
        for level in 0..self.n_levels as i32 {
            for a in -(nr - 1)..=(nr - 1) {
                for b in -(nr - 1)..=(nr - 1) {
                    if hex_ring(a, b) > self.n_rings - 1 {
                        continue;
                    }
                    for ball in 0..self.sites.len() {
                        out.push(BallId { a, b, level, ball });
                    }
                }
            }
        }
        out
    }

    /// Half-layer index of a ball from the lattice's bottom face: the plane
    /// of ball centres it sits on (`SekerBed::layer`: basal even, central
    /// odd).
    fn layer(&self, id: BallId) -> i32 {
        let z = self.centre(id)[2];
        ((z - self.z_bottom) / (0.5 * self.height)).round() as i32
    }

    /// `SekerBed::reject`: every ball not wholly inside the hopper (filled to
    /// the bed top) is absent; those whose centre is inside are the rejected.
    fn reject(&mut self) {
        let ids = self.all_balls();
        let floor = self.census.floor_z_cm;
        let mut rejected = Vec::new();
        for id in ids {
            let c = self.centre(id);
            let fit = self.hopper.contains_ball(c, self.ball_r, self.bed_top);
            self.present.insert(id, fit.whole);
            if !fit.whole && fit.centre_inside {
                rejected.push(RejectedBall {
                    centre_cm: c,
                    wall: fit.nearest,
                });
                let r = &mut self.census;
                match fit.nearest {
                    Wall::Top => r.rejected_at_top += 1,
                    Wall::SideWall => r.rejected_at_side_wall += 1,
                    Wall::Cone => r.rejected_at_cone += 1,
                    Wall::TubeWall => r.rejected_at_tube_wall += 1,
                    Wall::TubeBottom => r.rejected_at_tube_bottom += 1,
                }
                r.rejected_in_bed += usize::from(c[2] > floor);
            }
        }
        self.census.rejected_balls = rejected.len();
        self.census.rejected = rejected;
    }

    /// `SekerBed::assign` with `FuelAssignment::Paper`: the low-discrepancy
    /// rule over the kept balls centred above the floor.
    ///
    /// **One deliberate difference from `nee_soon`:** distances from the axis
    /// squared are compared on a `1e-6 cm^2` grid, so balls at the same
    /// distance (the six of a flower, the three `Inner`, the three `Outer`,
    /// and every mirror image) are ordered by their angle, as the rule says,
    /// and not by the last bits of `x*x + y*y`, which `nee_soon` compares
    /// raw (tutorial 7's `fuel_flags` does the same). The number of fuel
    /// balls is the same (it depends on how many balls there are, not their
    /// order); which of two symmetric balls is the fuel one can differ.
    fn assign(&mut self, f: f64) {
        let floor = self.census.floor_z_cm;
        let mut eligible: Vec<(i32, i64, f64, BallId)> = Vec::new();
        for id in self.all_balls() {
            let [x, y, z] = self.centre(id);
            if z > floor && self.present[&id] {
                eligible.push((
                    self.layer(id),
                    ((x * x + y * y) * 1.0e6).round() as i64,
                    y.atan2(x),
                    id,
                ));
            }
        }
        eligible.sort_by(|p, q| {
            p.0.cmp(&q.0)
                .then(p.1.cmp(&q.1))
                .then(p.2.total_cmp(&q.2))
                .then(p.3.cmp(&q.3))
        });
        for (n, &(_, _, _, id)) in eligible.iter().enumerate() {
            if ((n + 1) as f64 * f).floor() > (n as f64 * f).floor() {
                self.fuel.insert(id, true);
            }
        }
    }

    /// The census: counts from the kept balls.
    fn count(&mut self) {
        let floor = self.census.floor_z_cm;
        let r = self.ball_r;
        let mut planes: Vec<(f64, usize)> = Vec::new();
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        let mut c = self.census.clone();
        for id in self.all_balls() {
            if !self.present[&id] {
                continue;
            }
            c.loaded_balls += 1;
            let centre = self.centre(id);
            c.kept.push(KeptBall {
                centre_cm: centre,
                fuel: self.is_fuel(id),
            });
            let z = centre[2];
            if z > floor {
                c.bed_balls += 1;
                if self.is_fuel(id) {
                    c.fuel_balls += 1;
                } else {
                    c.moderator_balls_in_bed += 1;
                }
                lo = lo.min(z - r);
                hi = hi.max(z + r);
                match planes.iter_mut().find(|p| (p.0 - z).abs() < 1.0e-6) {
                    Some(p) => p.1 += 1,
                    None => planes.push((z, 1)),
                }
            } else {
                c.cone_and_tube_balls += 1;
            }
        }
        planes.sort_by(|p, q| p.0.total_cmp(&q.0));
        c.planes = planes;
        if c.bed_balls > 0 {
            c.bed_height_cm = hi - lo;
            let ball_v = 4.0 / 3.0 * std::f64::consts::PI * r.powi(3);
            let cylinder_v = std::f64::consts::PI * self.hopper.radius_cm().powi(2) * (hi - lo);
            c.filling_fraction = c.bed_balls as f64 * ball_v / cylinder_v;
        }
        self.census = c;
    }

    /// Whether any kept ball is owned by a tile one ring beyond the lattice
    /// (then the lattice does not cover the container: a defect).
    pub(crate) fn lattice_covers_container(&self) -> bool {
        let nr = self.n_rings as i32 + 1;
        for level in 0..self.n_levels as i32 {
            for a in -nr..=nr {
                for b in -nr..=nr {
                    if hex_ring(a, b) != self.n_rings {
                        continue;
                    }
                    for ball in 0..self.sites.len() {
                        if self.is_present(BallId { a, b, level, ball }) {
                            return false;
                        }
                    }
                }
            }
        }
        // Levels beyond the stack.
        for level in [-1, self.n_levels as i32] {
            for a in -(nr - 1)..=(nr - 1) {
                for b in -(nr - 1)..=(nr - 1) {
                    if hex_ring(a, b) >= self.n_rings {
                        continue;
                    }
                    for ball in 0..self.sites.len() {
                        if self.is_present(BallId { a, b, level, ball }) {
                            return false;
                        }
                    }
                }
            }
        }
        true
    }

    /// The state of the piece `p` of tile `(a, b, level)`: 0 absent, 1
    /// moderator, 2 fuel.
    pub(crate) fn piece_state(&self, a: i32, b: i32, level: i32, p: &PieceGeom) -> u8 {
        let id = BallId {
            a: a + p.owner[0],
            b: b + p.owner[1],
            level: level + p.owner[2],
            ball: p.ball,
        };
        if !self.is_present(id) {
            0
        } else if self.is_fuel(id) {
            2
        } else {
            1
        }
    }
}
