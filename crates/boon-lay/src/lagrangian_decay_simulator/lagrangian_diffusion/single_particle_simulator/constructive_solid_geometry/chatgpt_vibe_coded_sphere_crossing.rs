use uom::si::f64::{Length, Time, Velocity};
use uom::si::length::meter;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SphereCrossing {
    Entry { t: Time },
    Exit { t: Time },
}

impl SphereCrossing {
    #[inline]
    pub fn time(self) -> Time {
        match self {
            SphereCrossing::Entry { t } => t,
            SphereCrossing::Exit { t } => t,
        }
    }
}

/// Earliest forward-time crossing of the sphere surface by x(t) = p + t*v,
/// classified as Entry or Exit, using `uom` units.
///
/// All computations are done in SI base units (m, s) internally.
#[inline]
pub fn sphere_first_crossing_uom(
    center: [Length; 3],
    radius: Length,
    position: [Length; 3],
    velocity: [Velocity; 3],
) -> Option<SphereCrossing> {
    // Convert to raw SI values
    let cx = center[0].get::<meter>();
    let cy = center[1].get::<meter>();
    let cz = center[2].get::<meter>();

    let px = position[0].get::<meter>();
    let py = position[1].get::<meter>();
    let pz = position[2].get::<meter>();

    let vx = velocity[0].get::<meter_per_second>();
    let vy = velocity[1].get::<meter_per_second>();
    let vz = velocity[2].get::<meter_per_second>();

    let r = radius.get::<meter>();

    // Offset o = p - center (meters)
    let ox = px - cx;
    let oy = py - cy;
    let oz = pz - cz;

    // a = |v|^2  (m^2 / s^2)
    let a = vx * vx + vy * vy + vz * vz;
    if a == 0.0 {
        return None;
    }

    // b = o·v (m^2 / s)
    let b = ox * vx + oy * vy + oz * vz;

    // c = |o|^2 - r^2 (m^2)
    let c = (ox * ox + oy * oy + oz * oz) - r * r;

    // Solve: a t^2 + 2 b t + c = 0
    // disc = b^2 - a c  (m^4 / s^2)
    let disc = b * b - a * c;
    if disc < 0.0 {
        return None;
    }
    let sqrt_disc = disc.sqrt(); // (m^2 / s)

    // Numerically stable quadratic solve
    let q = if b > 0.0 { -b - sqrt_disc } else { -b + sqrt_disc }; // (m^2 / s)
    let mut t0 = q / a; // (s)
    let mut t1 = if q != 0.0 { c / q } else { t0 }; // (s)
    if t0 > t1 {
        std::mem::swap(&mut t0, &mut t1);
    }

    // Starting region classification (inside/outside/on), tolerance in m^2
    let r2 = r * r;
    let o2 = ox * ox + oy * oy + oz * oz;
    let eps_r2 = 1e-12_f64.max(1e-12 * r2);

    let on_surface = (o2 - r2).abs() <= eps_r2;
    let inside = o2 < r2 - eps_r2;

    // Earliest nonnegative root (tolerance in seconds)
    let eps_t = 1e-12;
    let t_hit_s = if t0 >= -eps_t {
        if t0 < 0.0 { 0.0 } else { t0 }
    } else if t1 >= -eps_t {
        if t1 < 0.0 { 0.0 } else { t1 }
    } else {
        return None;
    };

    let t_hit = Time::new::<second>(t_hit_s);

    // Classify:
    // - Strictly outside => first crossing is Entry
    // - Strictly inside  => first crossing is Exit
    // - On surface       => decide by radial motion sign (o·v): inward => Entry, outward => Exit
    if on_surface {
        if b < 0.0 {
            Some(SphereCrossing::Entry { t: t_hit })
        } else {
            Some(SphereCrossing::Exit { t: t_hit })
        }
    } else if inside {
        Some(SphereCrossing::Exit { t: t_hit })
    } else {
        Some(SphereCrossing::Entry { t: t_hit })
    }
}
