//! **Packing fraction of a pebble bed** (maintainer, 2026-11-18: "the
//! packing fraction calculator should also go to OUTRAM MC libs").
//!
//! Three instruments, each for any container and any pebble radius, in any
//! one length unit (metres or centimetres, consistently):
//!
//! - [`surface_height`]: the bed's surface, the 99th-percentile centre
//!   height above the floor (`z > 0`) plus one radius. Robust to a single
//!   pebble on top, where `max z` is not.
//! - [`whole_core_fraction`]: `N V / (π R² h)` over the pebbles above the
//!   floor, `h` the [`surface_height`]: the like-for-like comparison with a
//!   published whole-core figure (HTR-10's 0.61, gh:#216).
//! - [`bulk_fraction`]: the solid fraction of the vessel between `4 r`
//!   above its bottom and `4 r` below the bed surface, the pebbles' caps
//!   integrated exactly and the vessel's volume `∫ π R(z)² dz` by the
//!   midpoint rule on slices of `slice` (the bulk-slab instrument of
//!   `outram-park-fork-liggghts`' `htr10_recirculation_sweep`, widened to
//!   any wall `R(z)`).
//!
//! Ported unchanged, operation for operation, from the HTR-10 copies they
//! replace (`outram_park_fork_liggghts::htr10_fill::{surface_height_m,
//! whole_core_fraction}`, `dhoby-ghaut`'s `dem_web` `vessel_bulk_fraction`),
//! so the numbers are bit for bit theirs (`tests` here pin the first two
//! against the port; `dem_web` calls [`bulk_fraction`]). The LIGGGHTS port
//! keeps its own two (this crate depends on it, never the reverse), pinned
//! equal by the same test. Pure arithmetic: no `std`, no data.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
#[allow(unused_imports)]
use petir::mathf::RealMath;

/// The bed's surface: the 99th-percentile height of the centres above the
/// floor (`z > 0`) plus `r`; `None` if no centre is above the floor.
pub fn surface_height(centres: &[[f64; 3]], r: f64) -> Option<f64> {
    let mut zs: Vec<f64> = centres.iter().map(|c| c[2]).filter(|z| *z > 0.0).collect();
    if zs.is_empty() {
        return None;
    }
    zs.sort_by(f64::total_cmp);
    let k = ((zs.len() as f64 * 0.99) as usize).min(zs.len() - 1);
    Some(zs[k] + r)
}

/// Whole-core fraction `N V / (π R² h)` of the pebbles above the floor in
/// a cylinder of radius `core_radius`, `h` the [`surface_height`]; NaN if
/// no pebble is above the floor.
pub fn whole_core_fraction(centres: &[[f64; 3]], r: f64, core_radius: f64) -> f64 {
    let Some(top) = surface_height(centres, r) else {
        return f64::NAN;
    };
    let n = centres.iter().filter(|c| c[2] > 0.0).count();
    let v = 4.0 / 3.0 * core::f64::consts::PI * r.r_powi(3);
    n as f64 * v / (core::f64::consts::PI * core_radius * core_radius * top)
}

/// Bulk solid fraction of a vessel of wall radius `wall_radius_at(z)`
/// whose bottom is at `z_bottom`: the slab from `z_bottom + 4 r` to the
/// 99th-percentile centre `+ r − 4 r`, the pebbles' spherical caps in the
/// slab exactly, the vessel's volume by the midpoint rule on `slice`-thick
/// slices. NaN without pebbles or when the slab is empty.
pub fn bulk_fraction<F: Fn(f64) -> f64>(centres: &[[f64; 3]], r: f64, z_bottom: f64, wall_radius_at: F, slice: f64) -> f64 {
    if centres.is_empty() {
        return f64::NAN;
    }
    let mut zs: Vec<f64> = centres.iter().map(|c| c[2]).collect();
    zs.sort_by(f64::total_cmp);
    let k = ((zs.len() as f64 * 0.99) as usize).min(zs.len() - 1);
    let (z_lo, z_hi) = (z_bottom + 4.0 * r, zs[k] + r - 4.0 * r);
    if z_hi <= z_lo {
        return f64::NAN;
    }
    let cap = |zc: f64| {
        let (lo, hi) = (z_lo.max(zc - r), z_hi.min(zc + r));
        if hi <= lo {
            return 0.0;
        }
        let f = |z: f64| core::f64::consts::PI * (r * r * (z - zc) - (z - zc).r_powi(3) / 3.0);
        f(hi) - f(lo)
    };
    let solid: f64 = centres.iter().map(|c| cap(c[2])).sum();
    let n = ((z_hi - z_lo) / slice).ceil().max(1.0) as usize;
    let dz = (z_hi - z_lo) / n as f64;
    let volume: f64 = (0..n).map(|i| core::f64::consts::PI * wall_radius_at(z_lo + (i as f64 + 0.5) * dz).r_powi(2) * dz).sum();
    solid / volume
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A seeded loose bed in an HTR-10-sized cylinder (m).
    fn bed() -> Vec<[f64; 3]> {
        let mut s = 12345u64;
        (0..3000)
            .map(|_| {
                let mut u = || crate::rng::lcg::prn(&mut s);
                let (a, rho, z) = (u() * 6.283_185_307_179_586, 0.85 * u().sqrt(), -0.3 + 1.6 * u());
                [rho * a.cos(), rho * a.sin(), z]
            })
            .collect()
    }

    /// Bit for bit the LIGGGHTS port's HTR-10 copies (it keeps them; this
    /// crate depends on it, so they are pinned equal here).
    #[cfg(feature = "std")]
    #[test]
    fn whole_core_fraction_is_the_ports_bit_for_bit() {
        use outram_park_fork_liggghts::htr10_fill::{surface_height_m, whole_core_fraction as port, CORE_RADIUS_M, PEBBLE_RADIUS_M};
        use outram_park_fork_liggghts::particle::Vec3;
        let b = bed();
        let v: Vec<Vec3> = b.iter().map(|c| Vec3::new(c[0], c[1], c[2])).collect();
        assert_eq!(surface_height(&b, PEBBLE_RADIUS_M).map(f64::to_bits), surface_height_m(&v).map(f64::to_bits));
        assert_eq!(whole_core_fraction(&b, PEBBLE_RADIUS_M, CORE_RADIUS_M).to_bits(), port(&v).to_bits());
    }

    /// In a straight cylinder, a cubic stack of touching spheres: the bulk
    /// fraction is the analytic π/6, to the midpoint rule's slicing.
    #[test]
    fn bulk_fraction_of_a_cubic_stack_is_pi_over_six() {
        let (r, rc) = (1.0, 30.0);
        let mut c = Vec::new();
        let n = 40;
        for i in -n..=n {
            for j in -n..=n {
                for k in 0..40 {
                    let (x, y, z) = (2.0 * i as f64, 2.0 * j as f64, 1.0 + 2.0 * k as f64);
                    if (x * x + y * y).sqrt() <= rc {
                        c.push([x, y, z]);
                    }
                }
            }
        }
        // The cylinder's own wall cuts lattice cells, so compare with the
        // lattice count over the same volume instead: N_cells V / V_cyl.
        let phi = bulk_fraction(&c, r, 0.0, |_| rc, 0.01);
        let ideal = core::f64::consts::PI / 6.0;
        assert!((phi / ideal - 1.0).abs() < 0.02, "{phi} vs {ideal}");
        assert!(bulk_fraction(&c[..1], r, 0.0, |_| rc, 0.01).is_nan(), "one pebble: no slab");
        assert!(whole_core_fraction(&[], r, rc).is_nan());
        assert_eq!(surface_height(&[[0.0, 0.0, -1.0]], r), None);
    }
}
