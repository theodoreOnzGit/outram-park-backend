//! **A pour into any axisymmetric vessel** (gh:#1656): the HTR-10 fill's
//! seeding, walls and system build, taken out of `htr10_fill` so that any
//! vessel whose wall radius is a piecewise-linear function of height along
//! the gravity axis can be filled the same way.
//! `htr10_fill::Htr10Fill` is this module with HTR-10's profile
//! ([`AxisymVessel::htr10`]), and `outram-mc-libs`' `pack_in_region` reads a
//! CSG region into an [`AxisymVessel`] and pours through it.
//!
//! - [`AxisymVessel`]: a frame (origin, `up` = against gravity) and the wall
//!   radius `R(s)` at knots `(s, R)` along `up`, linear between knots and
//!   constant beyond the ends, with a floor plane at `s = floor`.
//! - [`seed_in_vessel`]: `n` loose centres, slab by slab from the floor up
//!   (`htr10_fill::seed_loose`'s algorithm, operation for operation).
//! - [`vessel_system`]: the `GranularSystem`: a cylinder at the top
//!   segment's radius, the floor as a plane wall, every lower segment as an
//!   inward-facing triangle band (`htr10_fill::discharge_mesh`'s bands).
//!
//! **Bit for bit HTR-10's.** For HTR-10's vessel the frame is the identity
//! (`up` = +z, origin 0) and every coordinate is formed as
//! `origin + u e1 + v e2 + s up`, which is exact for unit axis vectors and a
//! zero origin (each extra term is ±0). Knot radii and the interpolation
//! `r0 + (r1 − r0)(s − s0)/(s1 − s0)` reproduce `wall_radius_at` exactly
//! (the constant segments give `r0 + 0`; the conus gives HTR-10's own
//! expression; a knot belongs to the segment above it, as `wall_radius_at`'s
//! `z < 0` puts `z = 0` in the barrel). `tests/htr10_fill_fingerprint.rs`
//! pins it against the pre-refactor code.
//!
//! Units: whatever the DEM runs in (metres everywhere in this crate). The
//! seed's 1 mm wall clearance is [`SEED_WALL_CLEARANCE_M`].

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
#[allow(unused_imports)]
use petir::mathf::RealMath;

use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use crate::boundary::Boundary;
use crate::compute::ComputeType;
use crate::granular::{GranularContactModel, GranularMaterial, RollingModel};
use crate::granular_system::GranularSystem;
use crate::htr10_fill::{Htr10FillSettings, CONE_HEIGHT_M, CORE_RADIUS_M, TUBE_RADIUS_M, VALVE_Z_M};
use crate::mesh_wall::{MeshWall, MovingBoundary, Triangle, WallGeometry};
use crate::particle::{Particle, Vec3};
use crate::DemError;

/// Clearance between a seeded sphere and the wall \[m\] (`seed_loose`'s).
pub const SEED_WALL_CLEARANCE_M: f64 = 1.0e-3;
/// Solid fraction of each seeded slab (`seed_loose`'s).
pub const SEED_SOLID_FRACTION: f64 = 0.30;

/// A vessel whose inner wall radius depends only on the height `s` along
/// `up` (module docs).
#[derive(Debug, Clone, PartialEq)]
pub struct AxisymVessel {
    /// A point on the axis, where `s = 0`.
    pub origin: Vec3,
    /// Unit vector against gravity.
    pub up: Vec3,
    /// Knots `(s, R)`, `s` strictly increasing, at least one.
    pub knots: Vec<(f64, f64)>,
    /// The floor plane's height `s`.
    pub floor: f64,
}

/// `x + 0.0`: turns −0 into +0 and leaves everything else as it is.
fn canon(v: Vec3) -> Vec3 {
    Vec3::new(v.x + 0.0, v.y + 0.0, v.z + 0.0)
}

impl AxisymVessel {
    /// A vessel along `up` (any length; normalised) through `origin`.
    ///
    /// # Errors
    /// [`DemError::InvalidInput`] for a zero `up`, no knots, knots not
    /// strictly increasing in `s`, or a non-positive radius.
    pub fn new(origin: Vec3, up: Vec3, knots: Vec<(f64, f64)>, floor: f64) -> Result<Self, DemError> {
        let n = up.norm();
        if !(n > 0.0) {
            return Err(DemError::InvalidInput("vessel axis of zero length".into()));
        }
        let up = canon(if n == 1.0 { up } else { up.scale(1.0 / n) });
        if knots.is_empty() || knots.windows(2).any(|w| !(w[1].0 > w[0].0)) || knots.iter().any(|k| !(k.1 > 0.0)) {
            return Err(DemError::InvalidInput(format!("vessel knots must rise in s with positive radii: {knots:?}")));
        }
        Ok(Self { origin, up, knots, floor })
    }

    /// HTR-10's DEM vessel (`htr10_fill`): discharge tube from the valve,
    /// conus, then the barrel, +z up, origin at the conus inlet.
    #[must_use]
    pub fn htr10() -> Self {
        Self {
            origin: Vec3::zero(),
            up: Vec3::new(0.0, 0.0, 1.0),
            knots: vec![(VALVE_Z_M, TUBE_RADIUS_M), (-CONE_HEIGHT_M, TUBE_RADIUS_M), (0.0, CORE_RADIUS_M)],
            floor: VALVE_Z_M,
        }
    }

    /// The wall radius at height `s` (module docs).
    #[must_use]
    pub fn radius_at(&self, s: f64) -> f64 {
        let k = &self.knots;
        if s < k[0].0 || k.len() == 1 {
            return k[0].1;
        }
        // The segment containing s, a knot belonging to the segment above.
        for w in k.windows(2) {
            let ((s0, r0), (s1, r1)) = (w[0], w[1]);
            if s >= s0 && s < s1 {
                return r0 + (r1 - r0) * (s - s0) / (s1 - s0);
            }
        }
        k[k.len() - 1].1
    }

    /// `(e1, e2)`: the right-handed frame `e1 × e2 = up`; `(x, y)` for `up`
    /// along +z.
    #[must_use]
    pub fn frame(&self) -> (Vec3, Vec3) {
        let u = self.up;
        let a = if u.x.abs() > 0.9 { Vec3::new(0.0, 1.0, 0.0) } else { Vec3::new(1.0, 0.0, 0.0) };
        let e2 = u.cross(a);
        let e2 = canon(e2.scale(1.0 / e2.norm()));
        let e1 = canon(e2.cross(u));
        (e1, e2)
    }

    /// The world point at `(u, v)` across the axis and `s` along it.
    #[must_use]
    pub fn to_world(&self, (e1, e2): (Vec3, Vec3), u: f64, v: f64, s: f64) -> Vec3 {
        let o = self.origin;
        let up = self.up;
        Vec3::new(o.x + u * e1.x + v * e2.x + s * up.x, o.y + u * e1.y + v * e2.y + s * up.y, o.z + u * e1.z + v * e2.z + s * up.z)
    }

    /// Every segment below the top one, as an inward-facing band of
    /// `segments` quads, from the top down (`htr10_fill::discharge_mesh`).
    ///
    /// # Errors
    /// A [`DemError`] from a degenerate triangle.
    pub fn lower_mesh(&self, segments: usize) -> Result<Option<MeshWall>, DemError> {
        let n = segments.max(3);
        let f = self.frame();
        let mut tris = Vec::with_capacity(2 * n * self.knots.len());
        for i in (1..self.knots.len()).rev() {
            let ((z1, r1), (z0, r0)) = (self.knots[i - 1], self.knots[i]);
            for j in 0..n {
                let (a0, a1) = (core::f64::consts::TAU * j as f64 / n as f64, core::f64::consts::TAU * (j + 1) as f64 / n as f64);
                let p0 = self.to_world(f, r0 * a0.r_cos(), r0 * a0.r_sin(), z0);
                let p1 = self.to_world(f, r0 * a1.r_cos(), r0 * a1.r_sin(), z0);
                let q0 = self.to_world(f, r1 * a0.r_cos(), r1 * a0.r_sin(), z1);
                let q1 = self.to_world(f, r1 * a1.r_cos(), r1 * a1.r_sin(), z1);
                tris.push(Triangle::new(p0, q1, q0)?);
                tris.push(Triangle::new(p0, p1, q1)?);
            }
        }
        if tris.is_empty() {
            return Ok(None);
        }
        Ok(Some(MeshWall::new(tris)?))
    }
}

/// SplitMix64 in `[0, 1)` (`htr10_fill`'s jitter generator).
fn splitmix(state: &mut u64) -> f64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

/// `n` loose centres of radius `r` in `vessel`, slab by slab (one diameter
/// thick) from the floor up, each slab to [`SEED_SOLID_FRACTION`], random
/// in the disc the slab's narrowest wall leaves and rejected on overlap
/// (`htr10_fill::seed_loose`, which this is, for HTR-10's vessel).
/// Deterministic in `seed`. A vessel that cannot take `n` keeps adding
/// slabs above its top knot, where the radius stays the top one.
#[must_use]
pub fn seed_in_vessel(vessel: &AxisymVessel, n: usize, seed: u64, r: f64) -> Vec<Vec3> {
    let d = 2.0 * r;
    let mut state = seed;
    let mut out: Vec<Vec3> = Vec::with_capacity(n);
    let mut grid: alloc::collections::BTreeMap<(i64, i64, i64), Vec<usize>> = alloc::collections::BTreeMap::new();
    let cell = |p: Vec3| ((p.x / d).floor() as i64, (p.y / d).floor() as i64, (p.z / d).floor() as i64);
    let v_pebble = 4.0 / 3.0 * core::f64::consts::PI * r.r_powi(3);
    let mut z0 = vessel.floor;
    while out.len() < n {
        let z1 = z0 + d;
        let reach = vessel.radius_at(z0 - r).min(vessel.radius_at(z1 + r)) - r - SEED_WALL_CLEARANCE_M;
        if reach <= 0.0 {
            z0 = z1;
            continue;
        }
        let slab_volume = core::f64::consts::PI * (reach + r).r_powi(2) * d;
        let target = ((SEED_SOLID_FRACTION * slab_volume / v_pebble).round() as usize).max(1);
        let (mut placed, mut tries) = (0usize, 0usize);
        while placed < target && out.len() < n && tries < 200 * target {
            tries += 1;
            let rr = reach * splitmix(&mut state).sqrt();
            let th = core::f64::consts::TAU * splitmix(&mut state);
            // Local coordinates (u, v, s); the overlap test is done in them.
            let p = Vec3::new(rr * th.r_cos(), rr * th.r_sin(), z0 + (z1 - z0) * splitmix(&mut state));
            if p.z - r <= vessel.floor {
                continue;
            }
            let (cx, cy, cz) = cell(p);
            let mut clear = true;
            'scan: for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        if let Some(ids) = grid.get(&(cx + dx, cy + dy, cz + dz)) {
                            for &k in ids {
                                let q = out[k];
                                if (p.x - q.x).r_powi(2) + (p.y - q.y).r_powi(2) + (p.z - q.z).r_powi(2) < d * d {
                                    clear = false;
                                    break 'scan;
                                }
                            }
                        }
                    }
                }
            }
            if !clear {
                continue;
            }
            grid.entry((cx, cy, cz)).or_default().push(out.len());
            out.push(p);
            placed += 1;
        }
        z0 = z1;
    }
    let f = vessel.frame();
    out.into_iter().map(|p| vessel.to_world(f, p.x, p.y, p.z)).collect()
}

/// A pebble at rest at `x` (radius `r`, density `rho`, 300 K).
///
/// # Errors
/// A [`DemError`] from [`Particle::new`].
pub fn pebble_at(x: Vec3, r: f64, rho: f64) -> Result<Particle, DemError> {
    let mass = rho * 4.0 / 3.0 * core::f64::consts::PI * r.r_powi(3);
    Particle::new(x, Vec3::zero(), Vec3::zero(), Mass::new::<kilogram>(mass), Length::new::<meter>(r), ThermodynamicTemperature::new::<kelvin>(300.0))
}

/// The pour's `GranularSystem`: `particles` in `vessel` under `gravity`
/// \[m/s²\], contact law and timestep from `settings` (`Htr10FillSettings`:
/// its `n_pebbles`, `seed`, `settle_target` and `max_steps` are not read
/// here), walls as in the module docs with `mesh_segments` quads per band.
///
/// # Errors
/// A [`DemError`] for an invalid setting or wall.
pub fn vessel_system(vessel: &AxisymVessel, particles: Vec<Particle>, gravity: Vec3, settings: &Htr10FillSettings, mesh_segments: usize) -> Result<GranularSystem, DemError> {
    let material = GranularMaterial::new(settings.youngs_modulus.get::<pascal>(), settings.poisson_ratio, settings.restitution, settings.friction)?;
    let mut model = GranularContactModel::hertz_history(material);
    if settings.rolling_friction > 0.0 {
        model = model.with_rolling(RollingModel::cdt(settings.rolling_friction)?);
    }
    let top = vessel.knots[vessel.knots.len() - 1].1;
    let f = vessel.frame();
    let boundaries = vec![Boundary::cylinder(vessel.origin, vessel.up, top)?, Boundary::wall(vessel.to_world(f, 0.0, 0.0, vessel.floor), vessel.up)?];
    let sys = GranularSystem::new(particles, boundaries, model, gravity, settings.dt.get::<second>())?.with_compute(ComputeType::CpuMultiThread(settings.threads));
    Ok(match vessel.lower_mesh(mesh_segments)? {
        Some(mesh) => sys.with_moving_walls(vec![MovingBoundary::new(WallGeometry::Mesh(mesh), Vec3::zero(), Vec3::zero(), Vec3::zero())]),
        None => sys,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::htr10_fill::{discharge_mesh, seed_loose, wall_radius_at, PEBBLE_RADIUS_M};

    /// `radius_at` of HTR-10's vessel is `wall_radius_at`, bit for bit,
    /// on a dense sweep including every knot and points either side.
    #[test]
    fn htr10_profile_is_wall_radius_at() {
        let v = AxisymVessel::htr10();
        let mut zs: Vec<f64> = (0..=4000).map(|i| -0.8 + 3.0 * f64::from(i) / 4000.0).collect();
        for k in [VALVE_Z_M, -CONE_HEIGHT_M, 0.0] {
            zs.extend([k, k.next_up(), k.next_down()]);
        }
        for z in zs {
            assert_eq!(v.radius_at(z).to_bits(), wall_radius_at(z).to_bits(), "z = {z}");
        }
        assert_eq!(v.frame(), (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)));
    }

    /// The mesh and the seed of HTR-10's vessel are `discharge_mesh` and
    /// `seed_loose`, bit for bit.
    #[test]
    fn htr10_mesh_and_seed_are_the_ports() {
        let v = AxisymVessel::htr10();
        let a = v.lower_mesh(120).expect("mesh").expect("bands");
        let b = discharge_mesh(120).expect("mesh");
        assert_eq!(a.triangles.len(), b.triangles.len());
        let bits = |v: Vec3| [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()];
        for (s, t) in a.triangles.iter().zip(&b.triangles) {
            assert_eq!([bits(s.a), bits(s.b), bits(s.c)], [bits(t.a), bits(t.b), bits(t.c)]);
        }
        let s = seed_in_vessel(&v, 2000, 11, PEBBLE_RADIUS_M);
        let t = seed_loose(2000, 11);
        assert!(s.iter().zip(&t).all(|(p, q)| p.x.to_bits() == q.x.to_bits() && p.y.to_bits() == q.y.to_bits() && p.z.to_bits() == q.z.to_bits()));
        assert_eq!(s.len(), t.len());
    }

    /// Gravity along −x: the seed lies in the vessel turned on its side,
    /// floor at x = floor, every centre within the wall.
    #[test]
    fn a_vessel_along_x_seeds_inside_it() {
        let v = AxisymVessel::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), vec![(0.0, 0.1), (0.2, 0.3)], 0.0).expect("vessel");
        let (e1, e2) = v.frame();
        assert_eq!(e1.cross(e2), v.up);
        let c = seed_in_vessel(&v, 200, 3, 0.02);
        assert_eq!(c.len(), 200);
        for p in &c {
            assert!(p.x - 0.02 > 0.0, "below the floor: {p:?}");
            let rho = (p.y * p.y + p.z * p.z).sqrt();
            assert!(rho + 0.02 <= v.radius_at(p.x).max(v.radius_at(p.x - 0.02)) + 1e-12, "outside the wall: {p:?}");
        }
    }
}
