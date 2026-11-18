//! **A pebble bed inside a user-given CSG region** (gh:#1656, maintainer
//! 2026-11-18: "OUTRAM MC should contain an api for easy pebble bed
//! construction within a user set CSG. Gravity direction will be set for
//! pour using Option. Default is -z").
//!
//! [`pack_in_region`] fills one cell of a [`Geometry`] (its region, any
//! intersection of plane, cylinder and sphere half-spaces) with equal
//! pebbles, every one wholly inside, by one of three [`RegionPacking`]s:
//!
//! - [`RegionPacking::Lattice`]: a simple cubic lattice of pitch `p`,
//!   whole-ball rejection against the region (as `model::Lattice::covering`
//!   and `Lattice::hex_bed` do for their own containers);
//! - [`RegionPacking::Random`]: random sequential addition to a target
//!   packing fraction (or until `max_attempts` misses), drawn from PETIR's
//!   LCG;
//! - [`RegionPacking::Pour`]: a granular DEM pour on
//!   `outram-park-fork-liggghts`' `GranularSystem` (the engine verified
//!   against LIGGGHTS, as `DemBed` asks), the pebbles first placed loose by
//!   random addition and then settled under gravity `g` along `gravity`
//!   (**`None` is −z**), the region's planes and cylinders as the DEM's
//!   walls. HTR-10's pour (`htr10_fill`) keeps its own vessel and −z.
//!
//! **Containment is exact, not sampled**: a ball of radius `r` at `c` is in
//! the region when `c` is and no surface of the region passes within `r`
//! of `c` (each surface's closest distance is computed in closed form, see
//! [`surface_distance`]). Then every point of the ball is on the same side
//! of every surface as `c`, so in the region whatever its Boolean form.
//! Cones, tori and general quadrics have no closed form here and are
//! refused by name.
//!
//! New work, not a port: OpenMC builds its pebble beds outside the code
//! (its `openmc.model.pack_spheres` is a random packing in a box, cylinder
//! or sphere container only). The bed is returned as centres; lowering it to
//! CSG (`DemBed`, `model::Lattice`) and **drawing it** is the caller's
//! (this crate's drawing rule). Research, education and V&V only.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
#[allow(unused_imports)]
use petir::mathf::RealMath;

use crate::geometry::cell::{HalfSpaceSense, RegionToken};
use crate::geometry::geometry::Geometry;
use crate::geometry::position::Position;
use crate::geometry::surface::SurfaceKind;

/// How to fill the region.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RegionPacking {
    /// A simple cubic lattice of pitch `pitch` \[cm\] through the origin.
    Lattice { pitch: f64 },
    /// Random sequential addition to packing fraction `fraction` of the
    /// region's bounding box, stopping early after `max_attempts`
    /// consecutive misses.
    Random { fraction: f64, seed: u64, max_attempts: usize },
    /// A DEM pour of `n` pebbles, settled for `steps` DEM steps under
    /// gravity along `gravity` (a direction, any length; `None` is −z).
    Pour { n: usize, seed: u64, gravity: Option<[f64; 3]>, steps: usize },
}

/// A bed: pebble centres \[cm\] and their common radius \[cm\].
#[derive(Debug, Clone, PartialEq)]
pub struct RegionBed {
    pub centres: Vec<Position>,
    pub radius: f64,
}

/// Why a bed could not be built.
#[derive(Debug, Clone, PartialEq)]
pub enum RegionBedError {
    /// The cell index is not in the geometry.
    NoSuchCell(usize),
    /// The region uses a surface with no closed-form distance here.
    UnsupportedSurface(String),
    /// The pour needs walls: only plane and inside-of-z/x/y-cylinder
    /// half-spaces in a plain intersection map to DEM walls.
    UnsupportedWall(String),
    /// The region is unbounded along an axis (no box to sample in).
    Unbounded,
    /// A setting is out of range.
    Invalid(String),
    /// The DEM refused the set-up.
    Dem(String),
}

/// Closest distance \[cm\] from `p` to `s`, in closed form for planes,
/// axis-aligned cylinders and spheres.
///
/// # Errors
/// [`RegionBedError::UnsupportedSurface`] for cones, tori and general
/// quadrics.
pub fn surface_distance(s: &SurfaceKind, p: Position) -> Result<f64, RegionBedError> {
    let r2 = |a: f64, b: f64| (a * a + b * b).sqrt();
    Ok(match s {
        SurfaceKind::XPlane(x) => (p.x - x.x0).abs(),
        SurfaceKind::YPlane(y) => (p.y - y.y0).abs(),
        SurfaceKind::ZPlane(z) => (p.z - z.z0).abs(),
        SurfaceKind::Plane(q) => (q.a * p.x + q.b * p.y + q.c * p.z - q.d).abs() / (q.a * q.a + q.b * q.b + q.c * q.c).sqrt(),
        SurfaceKind::ZCylinder(c) => (r2(p.x - c.x0, p.y - c.y0) - c.r).abs(),
        SurfaceKind::XCylinder(c) => (r2(p.y - c.y0, p.z - c.z0) - c.r).abs(),
        SurfaceKind::YCylinder(c) => (r2(p.x - c.x0, p.z - c.z0) - c.r).abs(),
        SurfaceKind::Sphere(c) => (((p.x - c.x0).r_powi(2) + (p.y - c.y0).r_powi(2) + (p.z - c.z0).r_powi(2)).sqrt() - c.r).abs(),
        other => return Err(RegionBedError::UnsupportedSurface(format!("{other:?}"))),
    })
}

/// Whether `p` is in `region` (RPN over `surfaces`), off every surface.
fn in_region(region: &[RegionToken], surfaces: &[SurfaceKind], p: Position) -> bool {
    let mut stack: Vec<bool> = Vec::with_capacity(region.len());
    for t in region {
        match t {
            RegionToken::HalfSpace { surface_idx, sense } => {
                let positive = surfaces[*surface_idx].evaluate(p) > 0.0;
                stack.push(match sense {
                    HalfSpaceSense::Outside => positive,
                    HalfSpaceSense::Inside => !positive,
                });
            }
            RegionToken::Intersection => {
                let (b, a) = (stack.pop().unwrap_or(true), stack.pop().unwrap_or(true));
                stack.push(a && b);
            }
            RegionToken::Union => {
                let (b, a) = (stack.pop().unwrap_or(false), stack.pop().unwrap_or(false));
                stack.push(a || b);
            }
            RegionToken::Complement => {
                let a = stack.pop().unwrap_or(true);
                stack.push(!a);
            }
        }
    }
    stack.pop().unwrap_or(true)
}

/// Whether a ball of radius `r` at `c` lies wholly in `region`: `c` in the
/// region and no surface of the region within `r` of `c` (module docs).
///
/// # Errors
/// [`RegionBedError::UnsupportedSurface`] if the region uses a surface
/// without a closed-form distance.
pub fn ball_in_region(region: &[RegionToken], surfaces: &[SurfaceKind], c: Position, r: f64) -> Result<bool, RegionBedError> {
    for t in region {
        if let RegionToken::HalfSpace { surface_idx, .. } = t {
            if surface_distance(&surfaces[*surface_idx], c)? < r {
                return Ok(false);
            }
        }
    }
    Ok(in_region(region, surfaces, c))
}

/// The region's bounding box \[cm\] from its bounding half-spaces (planes
/// and the insides of cylinders and spheres), `None` if open along an axis.
fn bounding_box(region: &[RegionToken], surfaces: &[SurfaceKind]) -> Option<([f64; 3], [f64; 3])> {
    let (mut lo, mut hi) = ([f64::NEG_INFINITY; 3], [f64::INFINITY; 3]);
    let mut cap = |axis: usize, a: f64, b: f64| {
        lo[axis] = lo[axis].max(a);
        hi[axis] = hi[axis].min(b);
    };
    for t in region {
        let RegionToken::HalfSpace { surface_idx, sense } = t else { continue };
        let inside = *sense == HalfSpaceSense::Inside;
        match &surfaces[*surface_idx] {
            SurfaceKind::XPlane(p) => if inside { cap(0, f64::NEG_INFINITY, p.x0) } else { cap(0, p.x0, f64::INFINITY) },
            SurfaceKind::YPlane(p) => if inside { cap(1, f64::NEG_INFINITY, p.y0) } else { cap(1, p.y0, f64::INFINITY) },
            SurfaceKind::ZPlane(p) => if inside { cap(2, f64::NEG_INFINITY, p.z0) } else { cap(2, p.z0, f64::INFINITY) },
            SurfaceKind::ZCylinder(c) if inside => {
                cap(0, c.x0 - c.r, c.x0 + c.r);
                cap(1, c.y0 - c.r, c.y0 + c.r);
            }
            SurfaceKind::XCylinder(c) if inside => {
                cap(1, c.y0 - c.r, c.y0 + c.r);
                cap(2, c.z0 - c.r, c.z0 + c.r);
            }
            SurfaceKind::YCylinder(c) if inside => {
                cap(0, c.x0 - c.r, c.x0 + c.r);
                cap(2, c.z0 - c.r, c.z0 + c.r);
            }
            SurfaceKind::Sphere(c) if inside => {
                cap(0, c.x0 - c.r, c.x0 + c.r);
                cap(1, c.y0 - c.r, c.y0 + c.r);
                cap(2, c.z0 - c.r, c.z0 + c.r);
            }
            _ => {}
        }
    }
    (lo.iter().chain(hi.iter()).all(|v| v.is_finite())).then_some((lo, hi))
}

/// Pebbles of radius `radius` \[cm\] in cell `cell` of `geom`, by `packing`.
///
/// # Errors
/// See [`RegionBedError`].
pub fn pack_in_region(geom: &Geometry, cell: usize, radius: f64, packing: RegionPacking) -> Result<RegionBed, RegionBedError> {
    let region = &geom.cells.get(cell).ok_or(RegionBedError::NoSuchCell(cell))?.region;
    let surfaces = &geom.surfaces;
    if !(radius > 0.0) {
        return Err(RegionBedError::Invalid(format!("radius {radius}")));
    }
    let (lo, hi) = bounding_box(region, surfaces).ok_or(RegionBedError::Unbounded)?;
    let centres = match packing {
        RegionPacking::Lattice { pitch } => {
            if pitch < 2.0 * radius {
                return Err(RegionBedError::Invalid(format!("pitch {pitch} < 2 r")));
            }
            let mut out = Vec::new();
            let idx = |a: f64| (a / pitch).ceil() as i64;
            for i in idx(lo[0])..=idx(hi[0]) {
                for j in idx(lo[1])..=idx(hi[1]) {
                    for k in idx(lo[2])..=idx(hi[2]) {
                        let c = Position::new(i as f64 * pitch, j as f64 * pitch, k as f64 * pitch);
                        if ball_in_region(region, surfaces, c, radius)? {
                            out.push(c);
                        }
                    }
                }
            }
            out
        }
        RegionPacking::Random { fraction, seed, max_attempts } => random_addition(region, surfaces, lo, hi, radius, fraction, usize::MAX, seed, max_attempts)?,
        RegionPacking::Pour { n, seed, gravity, steps } => pour(region, surfaces, lo, hi, radius, n, seed, gravity, steps)?,
    };
    Ok(RegionBed { centres, radius })
}

/// Random sequential addition in the box `lo..hi`: at most `n_max` pebbles,
/// stopping at `fraction` of the box volume or after `max_attempts`
/// consecutive misses. A cell list keeps the overlap test local.
#[allow(clippy::too_many_arguments)]
fn random_addition(
    region: &[RegionToken],
    surfaces: &[SurfaceKind],
    lo: [f64; 3],
    hi: [f64; 3],
    r: f64,
    fraction: f64,
    n_max: usize,
    seed: u64,
    max_attempts: usize,
) -> Result<Vec<Position>, RegionBedError> {
    if !(fraction > 0.0 && fraction < 1.0) {
        return Err(RegionBedError::Invalid(format!("fraction {fraction}")));
    }
    let v_box = (hi[0] - lo[0]) * (hi[1] - lo[1]) * (hi[2] - lo[2]);
    let n_target = ((fraction * v_box) / (4.0 / 3.0 * core::f64::consts::PI * r * r * r)).floor() as usize;
    let n_target = n_target.min(n_max);
    let cell = 2.0 * r;
    let key = |p: Position| (((p.x - lo[0]) / cell).floor() as i64, ((p.y - lo[1]) / cell).floor() as i64, ((p.z - lo[2]) / cell).floor() as i64);
    let mut grid: crate::hash::HashMap<(i64, i64, i64), Vec<usize>> = crate::hash::HashMap::new();
    let mut out: Vec<Position> = Vec::new();
    let mut s = seed.max(1);
    let mut misses = 0usize;
    while out.len() < n_target && misses < max_attempts {
        let mut u = || crate::rng::lcg::prn(&mut s);
        let c = Position::new(lo[0] + (hi[0] - lo[0]) * u(), lo[1] + (hi[1] - lo[1]) * u(), lo[2] + (hi[2] - lo[2]) * u());
        let (i, j, k) = key(c);
        let mut clear = ball_in_region(region, surfaces, c, r)?;
        'near: for di in -1..=1 {
            for dj in -1..=1 {
                for dk in -1..=1 {
                    if !clear {
                        break 'near;
                    }
                    if let Some(list) = grid.get(&(i + di, j + dj, k + dk)) {
                        for &q in list {
                            let d = out[q];
                            if (d.x - c.x).r_powi(2) + (d.y - c.y).r_powi(2) + (d.z - c.z).r_powi(2) < 4.0 * r * r {
                                clear = false;
                                break;
                            }
                        }
                    }
                }
            }
        }
        if clear {
            grid.entry((i, j, k)).or_default().push(out.len());
            out.push(c);
            misses = 0;
        } else {
            misses += 1;
        }
    }
    Ok(out)
}

/// The DEM pour (module docs). Units: the DEM runs in metres, the bed is
/// returned in cm. Pebble density and contact parameters are HTR-10's
/// pour defaults (`htr10_fill::Htr10FillSettings`, graphite 1730 kg/m³):
/// they set how the pebbles settle, not where the walls are.
#[allow(clippy::too_many_arguments)]
fn pour(
    region: &[RegionToken],
    surfaces: &[SurfaceKind],
    lo: [f64; 3],
    hi: [f64; 3],
    r: f64,
    n: usize,
    seed: u64,
    gravity: Option<[f64; 3]>,
    steps: usize,
) -> Result<Vec<Position>, RegionBedError> {
    use outram_park_fork_liggghts::boundary::Boundary;
    use outram_park_fork_liggghts::granular::{GranularContactModel, GranularMaterial};
    use outram_park_fork_liggghts::granular_system::GranularSystem;
    use outram_park_fork_liggghts::htr10_fill::{Htr10FillSettings, PEBBLE_DENSITY};
    use outram_park_fork_liggghts::particle::{Particle, Vec3};
    use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
    use uom::si::length::meter;
    use uom::si::mass::kilogram;
    use uom::si::pressure::pascal;
    use uom::si::thermodynamic_temperature::kelvin;
    use uom::si::time::second;

    let g = gravity.unwrap_or([0.0, 0.0, -1.0]);
    let norm = (g[0] * g[0] + g[1] * g[1] + g[2] * g[2]).sqrt();
    if !(norm > 0.0) {
        return Err(RegionBedError::Invalid("gravity direction of zero length".into()));
    }
    let g = Vec3::new(9.81 * g[0] / norm, 9.81 * g[1] / norm, 9.81 * g[2] / norm);
    let walls = dem_walls(region, surfaces)?;
    // Loose start: random addition at a low fraction, at most n pebbles.
    let start = random_addition(region, surfaces, lo, hi, r, 0.30, n, seed, 20_000)?;
    if start.len() < n {
        return Err(RegionBedError::Invalid(format!("only {} of {n} pebbles fit loosely; enlarge the region or ask for fewer", start.len())));
    }
    let s = Htr10FillSettings::default();
    let material = GranularMaterial::new(s.youngs_modulus.get::<pascal>(), s.poisson_ratio, s.restitution, s.friction).map_err(|e| RegionBedError::Dem(format!("{e:?}")))?;
    let model = GranularContactModel::hertz_history(material);
    let rm = r * 1e-2;
    let mass = PEBBLE_DENSITY * 4.0 / 3.0 * core::f64::consts::PI * rm * rm * rm;
    let particles: Vec<Particle> = start
        .iter()
        .map(|c| {
            Particle::new(
                Vec3::new(c.x * 1e-2, c.y * 1e-2, c.z * 1e-2),
                Vec3::zero(),
                Vec3::zero(),
                Mass::new::<kilogram>(mass),
                Length::new::<meter>(rm),
                ThermodynamicTemperature::new::<kelvin>(300.0),
            )
        })
        .collect::<Result<_, _>>()
        .map_err(|e| RegionBedError::Dem(format!("{e:?}")))?;
    let boundaries: Vec<Boundary> = walls
        .into_iter()
        .map(|w| match w {
            Wall::Plane { point, normal } => Boundary::wall(Vec3::new(point[0] * 1e-2, point[1] * 1e-2, point[2] * 1e-2), Vec3::new(normal[0], normal[1], normal[2])),
            Wall::Cylinder { point, axis, radius } => Boundary::cylinder(Vec3::new(point[0] * 1e-2, point[1] * 1e-2, point[2] * 1e-2), Vec3::new(axis[0], axis[1], axis[2]), radius * 1e-2),
        })
        .collect::<Result<_, _>>()
        .map_err(|e| RegionBedError::Dem(format!("{e:?}")))?;
    let mut sys = GranularSystem::new(particles, boundaries, model, g, s.dt.get::<second>()).map_err(|e| RegionBedError::Dem(format!("{e:?}")))?;
    sys.run(steps);
    Ok(sys.particles().iter().map(|p| Position::new(p.position.x * 1e2, p.position.y * 1e2, p.position.z * 1e2)).collect())
}

/// A DEM wall in the region's terms \[cm\]: the domain is the side the
/// normal points into, or the inside of the cylinder.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Wall {
    Plane { point: [f64; 3], normal: [f64; 3] },
    Cylinder { point: [f64; 3], axis: [f64; 3], radius: f64 },
}

/// The region's half-spaces as DEM walls: it must be a plain intersection
/// of plane half-spaces and cylinder insides.
fn dem_walls(region: &[RegionToken], surfaces: &[SurfaceKind]) -> Result<Vec<Wall>, RegionBedError> {
    let mut out = Vec::new();
    for t in region {
        match t {
            RegionToken::Intersection => {}
            RegionToken::Union | RegionToken::Complement => return Err(RegionBedError::UnsupportedWall("a union or complement in the region".into())),
            RegionToken::HalfSpace { surface_idx, sense } => {
                let inside = *sense == HalfSpaceSense::Inside;
                let sgn = if inside { -1.0 } else { 1.0 };
                out.push(match &surfaces[*surface_idx] {
                    SurfaceKind::XPlane(p) => Wall::Plane { point: [p.x0, 0.0, 0.0], normal: [sgn, 0.0, 0.0] },
                    SurfaceKind::YPlane(p) => Wall::Plane { point: [0.0, p.y0, 0.0], normal: [0.0, sgn, 0.0] },
                    SurfaceKind::ZPlane(p) => Wall::Plane { point: [0.0, 0.0, p.z0], normal: [0.0, 0.0, sgn] },
                    SurfaceKind::ZCylinder(c) if inside => Wall::Cylinder { point: [c.x0, c.y0, 0.0], axis: [0.0, 0.0, 1.0], radius: c.r },
                    SurfaceKind::XCylinder(c) if inside => Wall::Cylinder { point: [0.0, c.y0, c.z0], axis: [1.0, 0.0, 0.0], radius: c.r },
                    SurfaceKind::YCylinder(c) if inside => Wall::Cylinder { point: [c.x0, 0.0, c.z0], axis: [0.0, 1.0, 0.0], radius: c.r },
                    other => return Err(RegionBedError::UnsupportedWall(format!("{other:?} ({sense:?})"))),
                });
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::cell::Cell;
    use crate::geometry::surface::{BoundaryType, XPlane, ZCylinder, ZPlane};
    use crate::geometry::universe::Universe;

    /// A region that is not a cylinder: the box 0 < x < 30, |y| < 10 cm
    /// (from a z-cylinder of radius 10 cut by two x-planes), 0 < z < 40.
    fn region_geometry() -> Geometry {
        let t = BoundaryType::Transmissive;
        let surfaces = vec![
            SurfaceKind::ZCylinder(ZCylinder { x0: 15.0, y0: 0.0, r: 10.0, bc: t }),
            SurfaceKind::ZPlane(ZPlane { z0: 0.0, bc: t }),
            SurfaceKind::ZPlane(ZPlane { z0: 40.0, bc: t }),
            SurfaceKind::XPlane(XPlane { x0: 8.0, bc: t }),
        ];
        let hs = |surface_idx: usize, sense: HalfSpaceSense| RegionToken::HalfSpace { surface_idx, sense };
        let region = vec![
            hs(0, HalfSpaceSense::Inside),
            hs(1, HalfSpaceSense::Outside),
            RegionToken::Intersection,
            hs(2, HalfSpaceSense::Inside),
            RegionToken::Intersection,
            hs(3, HalfSpaceSense::Outside),
            RegionToken::Intersection,
        ];
        Geometry { surfaces, cells: vec![Cell::material(1, region, 0, 293.6)], universes: vec![Universe { id: 0, cell_indices: vec![0] }], lattices: vec![], root_universe: 0 }
    }

    /// Every pebble wholly in the region and no two overlapping.
    fn check(geom: &Geometry, bed: &RegionBed) {
        let region = &geom.cells[0].region;
        for (i, c) in bed.centres.iter().enumerate() {
            assert!(ball_in_region(region, &geom.surfaces, *c, bed.radius * (1.0 - 1e-9)).unwrap(), "pebble {i} at {c:?} not inside");
        }
    }

    fn no_overlap(bed: &RegionBed, tol: f64) {
        for (i, a) in bed.centres.iter().enumerate() {
            for b in &bed.centres[i + 1..] {
                let d = ((a.x - b.x).r_powi(2) + (a.y - b.y).r_powi(2) + (a.z - b.z).r_powi(2)).sqrt();
                assert!(d >= 2.0 * bed.radius * (1.0 - tol), "overlap {d}");
            }
        }
    }

    /// Closed-form distances, and the ball test at the boundary.
    #[test]
    fn ball_containment_is_exact_at_the_walls() {
        let g = region_geometry();
        let region = &g.cells[0].region;
        let s = &g.surfaces;
        assert_eq!(surface_distance(&s[0], Position::new(15.0, 0.0, 5.0)).unwrap(), 10.0);
        assert!(ball_in_region(region, s, Position::new(15.0, 0.0, 20.0), 3.0).unwrap());
        assert!(ball_in_region(region, s, Position::new(11.0, 0.0, 3.0), 3.0).unwrap(), "touching x = 8 and z = 0");
        assert!(!ball_in_region(region, s, Position::new(10.9, 0.0, 20.0), 3.0).unwrap(), "crosses x = 8");
        assert!(!ball_in_region(region, s, Position::new(15.0, 7.1, 20.0), 3.0).unwrap(), "crosses the cylinder");
        assert!(!ball_in_region(region, s, Position::new(5.0, 0.0, 20.0), 1.0).unwrap(), "centre outside");
    }

    #[test]
    fn lattice_and_random_beds_stay_inside() {
        let g = region_geometry();
        let lat = pack_in_region(&g, 0, 2.0, RegionPacking::Lattice { pitch: 4.0 }).unwrap();
        assert!(lat.centres.len() > 50);
        check(&g, &lat);
        no_overlap(&lat, 1e-12);
        let rnd = pack_in_region(&g, 0, 2.0, RegionPacking::Random { fraction: 0.3, seed: 7, max_attempts: 5000 }).unwrap();
        assert!(rnd.centres.len() > 50);
        check(&g, &rnd);
        no_overlap(&rnd, 1e-12);
        assert_eq!(rnd, pack_in_region(&g, 0, 2.0, RegionPacking::Random { fraction: 0.3, seed: 7, max_attempts: 5000 }).unwrap(), "seeded: reproducible");
    }

    /// The pour settles against the floor gravity points to: with gravity
    /// −z (the default) the bed's mean height drops, with gravity +x the
    /// mean x rises; every pebble ends inside (to the DEM's soft overlap,
    /// a few per cent of a radius).
    #[test]
    fn the_pour_settles_along_gravity_and_stays_inside() {
        let g = region_geometry();
        let r = 2.0;
        let start = pack_in_region(&g, 0, r, RegionPacking::Random { fraction: 0.3, seed: 3, max_attempts: 20_000 }).unwrap();
        let mean = |b: &RegionBed, f: fn(&Position) -> f64| b.centres.iter().map(f).sum::<f64>() / b.centres.len() as f64;
        let n = 40;
        let down = pack_in_region(&g, 0, r, RegionPacking::Pour { n, seed: 3, gravity: None, steps: 6000 }).unwrap();
        let side = pack_in_region(&g, 0, r, RegionPacking::Pour { n, seed: 3, gravity: Some([1.0, 0.0, 0.0]), steps: 6000 }).unwrap();
        assert_eq!(down.centres.len(), n);
        let z0 = mean(&RegionBed { centres: start.centres[..n].to_vec(), radius: r }, |p| p.z);
        let x0 = mean(&RegionBed { centres: start.centres[..n].to_vec(), radius: r }, |p| p.x);
        assert!(mean(&down, |p| p.z) < z0 - 3.0, "fell along -z: {} -> {}", z0, mean(&down, |p| p.z));
        assert!(mean(&side, |p| p.x) > x0 + 3.0, "fell along +x: {} -> {}", x0, mean(&side, |p| p.x));
        for b in [&down, &side] {
            let soft = RegionBed { centres: b.centres.clone(), radius: r * 0.9 };
            check(&g, &soft);
            no_overlap(b, 0.1);
        }
    }

    #[test]
    fn unsupported_regions_are_refused_by_name() {
        let mut g = region_geometry();
        g.cells[0].region.push(RegionToken::Complement);
        assert!(matches!(pack_in_region(&g, 0, 1.0, RegionPacking::Pour { n: 1, seed: 1, gravity: None, steps: 1 }), Err(RegionBedError::Unbounded | RegionBedError::UnsupportedWall(_))));
        assert_eq!(pack_in_region(&g, 9, 1.0, RegionPacking::Lattice { pitch: 3.0 }), Err(RegionBedError::NoSuchCell(9)));
    }
}
