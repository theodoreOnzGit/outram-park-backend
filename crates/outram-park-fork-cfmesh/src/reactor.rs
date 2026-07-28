//! Reactor geometry generators — structured packings of the shapes a reactor
//! model is built from, ready to hand to [`crate::carve::carve_around`].
//!
//! - [`sphere_packing`] — a structured lattice of spheres (a **pebble bed** /
//!   TRISO packing).
//! - [`pin_lattice`] — a square lattice of vertical cylinders (**LWR** fuel
//!   pins / a channel bundle).
//! - [`bounding_domain`] — an axis-aligned box that wraps a set of surfaces
//!   with a margin (the coolant domain around a packing).
//!
//! Each returns owned [`TriSoup`](crate::shapes::TriSoup)s; carve the coolant
//! region around them with [`crate::carve::carve_around`]. Pure Rust,
//! education/research use only (see the crate `RESPONSIBLE_USE.md`).

use crate::math::Vec3;
use crate::shapes::{box_surface, cylinder_surface, sphere_surface, TriSoup};

/// A structured `[nx, ny, nz]` cubic lattice of spheres of `radius`, centres
/// `spacing` apart, the first at the origin — a pebble-bed / particle packing.
///
/// `n_lat`/`n_lon` set each sphere's UV tessellation. Returns one
/// [`TriSoup`](crate::shapes::TriSoup) per pebble.
pub fn sphere_packing(counts: [usize; 3], spacing: f64, radius: f64, n_lat: usize, n_lon: usize) -> Vec<TriSoup> {
    let [nx, ny, nz] = counts;
    let mut out = Vec::with_capacity(nx * ny * nz);
    for i in 0..nx {
        for j in 0..ny {
            for k in 0..nz {
                let c = Vec3::new(i as f64 * spacing, j as f64 * spacing, k as f64 * spacing);
                out.push(sphere_surface(c, radius, n_lat, n_lon));
            }
        }
    }
    out
}

/// A square `[nx, ny]` lattice of vertical cylinders of `radius` and `height`,
/// centre-to-centre `pitch`, bases on `z = 0`, the first at the origin — an LWR
/// fuel-pin bundle / channel array.
///
/// `n_seg` sets each cylinder's circumferential tessellation. Returns one
/// [`TriSoup`](crate::shapes::TriSoup) per pin.
pub fn pin_lattice(counts: [usize; 2], pitch: f64, radius: f64, height: f64, n_seg: usize) -> Vec<TriSoup> {
    let [nx, ny] = counts;
    let mut out = Vec::with_capacity(nx * ny);
    for i in 0..nx {
        for j in 0..ny {
            let base = Vec3::new(i as f64 * pitch, j as f64 * pitch, 0.0);
            out.push(cylinder_surface(base, radius, height, n_seg));
        }
    }
    out
}

/// An axis-aligned box surface wrapping all `soups` with a uniform `margin` —
/// the coolant domain enclosing a packing.
///
/// # Panics
///
/// If `soups` is empty or contains an empty surface.
pub fn bounding_domain(soups: &[TriSoup], margin: f64) -> TriSoup {
    let first = soups.iter().flat_map(|(p, _)| p.iter()).next().copied().expect("non-empty soups");
    let mut lo = first;
    let mut hi = first;
    for (pts, _) in soups {
        for p in pts {
            lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
            hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
        }
    }
    let m = Vec3::new(margin, margin, margin);
    box_surface(lo.sub(m), hi.add(m))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::carve::carve_around;
    use crate::shapes::surface_volume;
    use std::f64::consts::PI;

    /// V&V — sphere_packing produces the right count, and every pebble is the
    /// same sphere translated: a 2×2×2 packing is 8 pebbles whose enclosed
    /// volumes are all equal (surface volume is translation-invariant for a
    /// closed surface), so the total is exactly 8× a single pebble, and that
    /// single volume is within a few % of the analytic (4/3)π0.8³ (12×24 UV).
    #[test]
    fn sphere_packing_count_and_volume() {
        let pebbles = sphere_packing([2, 2, 2], 2.0, 0.8, 12, 24);
        assert_eq!(pebbles.len(), 8);
        let one = surface_volume(&pebbles[0].0, &pebbles[0].1);
        let total: f64 = pebbles.iter().map(|(p, t)| surface_volume(p, t)).sum();
        assert!((total - 8.0 * one).abs() < 1e-9, "8 identical pebbles sum exactly");
        let analytic = 4.0 / 3.0 * PI * 0.8_f64.powi(3);
        assert!((one - analytic).abs() / analytic < 0.05, "single pebble within 5% analytic (coarse UV)");
    }

    /// V&V — pin_lattice count + total volume. A 3×3 bundle of r = 0.5, h = 4
    /// pins is 9 cylinders summing to 9·π·0.5²·4.
    #[test]
    fn pin_lattice_count_and_volume() {
        let pins = pin_lattice([3, 3], 1.5, 0.5, 4.0, 32);
        assert_eq!(pins.len(), 9);
        let total: f64 = pins.iter().map(|(p, t)| surface_volume(p, t)).sum();
        let one = PI * 0.5_f64.powi(2) * 4.0;
        assert!((total - 9.0 * one).abs() / (9.0 * one) < 0.01, "9 pins' volume within 1%");
    }

    /// V&V — bounding_domain wraps every packing point with the margin, and
    /// carve_around meshes the coolant region as a valid closed mesh with a
    /// walls patch plus one patch per pin.
    #[test]
    fn domain_wraps_and_carve_around_meshes_pins() {
        let pins = pin_lattice([2, 2], 1.5, 0.4, 3.0, 20);
        let domain = bounding_domain(&pins, 0.5);
        let dv = surface_volume(&domain.0, &domain.1);
        assert!(dv > 0.0, "domain has positive volume");

        let m = carve_around(&domain, &pins, 0.15);
        m.validate().expect("coolant cells closed");
        // walls + one patch per pin (pins that expose faces).
        assert!(m.patches.iter().any(|p| p.name == "walls"));
        assert!(m.patches.len() >= 2, "at least walls + a pin patch");
    }
}
