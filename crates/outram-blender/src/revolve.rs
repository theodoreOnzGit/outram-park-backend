//! Revolve / spin — sweep a profile polyline around an axis into a surface of
//! revolution.
//!
//! This is the pure-Rust analogue of Blender's **Spin**: a profile (an ordered
//! list of points) is rotated around an arbitrary axis in `segments` steps
//! through a total `angle`, and consecutive rings are stitched with quads. It
//! generates the surfaces of revolution a reactor geometry is full of — pipes,
//! tubes, pressure-vessel walls, cone frusta.
//!
//! - A **full** sweep (`angle >= 2π`) closes the seam: the last ring wraps back
//!   onto the first, giving a closed loop of quads (an open-ended tube — cap it
//!   with [`crate::fill_holes`] for a solid).
//! - A **partial** sweep leaves the two end rings unstitched (an open strip).
//!
//! # Profile orientation
//!
//! The profile is a polyline in space; each point is rotated about the axis.
//! Neighbouring profile points become the two rails of a quad band, so the
//! profile's own ordering sets the surface's `v` direction and the sweep sets
//! its `u` direction. The winding is internally consistent across the whole
//! surface (fix its outward sense with
//! [`crate::recalc_normals::recalculate_normals`] if a particular profile winds
//! it inward).
//!
//! # Limitation (v1): on-axis profile points
//!
//! A profile point lying **on** the axis (a pole, radius ≈ 0) is rotated to the
//! same location on every ring, producing coincident duplicate vertices rather
//! than a shared pole. For a profile that touches the axis (e.g. a semicircle
//! swept into a sphere), run [`crate::weld::weld`] afterward to merge the pole
//! copies. Off-axis profiles need no post-processing. No `faer`, no external
//! dependency; Android-safe.

use crate::math::Vec3;
use crate::mesh::Mesh;
use std::f64::consts::TAU;

/// Sweep `profile` around the axis through `axis_point` along `axis_dir` by
/// `angle` radians in `segments` steps, returning the surface of revolution.
///
/// `segments` is the number of quad bands around the sweep (`>= 3` for a full
/// revolution, `>= 1` for a partial one); `profile` needs at least two points.
/// An `angle` of `2π` (or more) makes a closed loop; a smaller angle leaves the
/// ends open. `axis_dir` need not be unit length. This is infallible for valid
/// inputs; a profile with fewer than two points or `segments == 0` yields an
/// empty mesh.
///
/// # Examples
///
/// ```
/// use outram_blender::{revolve::revolve, fill_holes::fill_holes, math::Vec3};
/// use std::f64::consts::TAU;
///
/// // Revolve a vertical segment at radius 1 around Z into a 16-gon tube, then
/// // cap it into a closed prism.
/// let profile = [Vec3::new(1.0, 0.0, -1.0), Vec3::new(1.0, 0.0, 1.0)];
/// let tube = revolve(&profile, Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0), 16, TAU);
/// assert_eq!(tube.face_count(), 16);
/// assert_eq!(fill_holes(&tube).euler_characteristic(), 2);
/// ```
pub fn revolve(
    profile: &[Vec3],
    axis_point: Vec3,
    axis_dir: Vec3,
    segments: usize,
    angle: f64,
) -> Mesh {
    let np = profile.len();
    if np < 2 || segments == 0 {
        return Mesh::new();
    }
    let k = axis_dir.normalize();
    if k.length() == 0.0 {
        return Mesh::new();
    }

    // A full revolution wraps the last ring back onto the first.
    let closed = angle >= TAU - 1e-9;
    let num_rings = if closed { segments } else { segments + 1 };

    // Ring-major vertex layout: vertex(j, i) = j * np + i.
    let mut positions: Vec<Vec3> = Vec::with_capacity(num_rings * np);
    for j in 0..num_rings {
        let theta = angle * (j as f64) / (segments as f64);
        for &p in profile {
            positions.push(rotate_about_axis(p, axis_point, k, theta));
        }
    }

    let mut faces: Vec<Vec<usize>> = Vec::new();
    for j in 0..segments {
        let jn = if closed { (j + 1) % segments } else { j + 1 };
        for i in 0..np - 1 {
            let a = j * np + i;
            let b = jn * np + i;
            let c = jn * np + i + 1;
            let d = j * np + i + 1;
            // Quad band between profile rows i and i+1 across rings j -> jn.
            faces.push(vec![a, b, c, d]);
        }
    }

    Mesh::from_polygons(&positions, &faces)
}

/// Rotate `p` by `theta` radians about the axis through `axis_point` with unit
/// direction `k`, via Rodrigues' rotation formula.
fn rotate_about_axis(p: Vec3, axis_point: Vec3, k: Vec3, theta: f64) -> Vec3 {
    let v = p.sub(axis_point);
    let cos = theta.cos();
    let sin = theta.sin();
    // v_rot = v cosθ + (k × v) sinθ + k (k · v)(1 − cosθ).
    let term1 = v.scale(cos);
    let term2 = k.cross(v).scale(sin);
    let term3 = k.scale(k.dot(v) * (1.0 - cos));
    axis_point.add(term1).add(term2).add(term3)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fill_holes::fill_holes;
    use std::collections::HashSet;
    use std::f64::consts::PI;

    /// Six times the signed volume of the mesh as-wound.
    fn signed_volume6(mesh: &Mesh) -> f64 {
        let ps = mesh.positions();
        let mut v = 0.0;
        for poly in mesh.polygons() {
            let p0 = ps[poly[0].0];
            for i in 1..poly.len() - 1 {
                v += p0.dot(ps[poly[i].0].cross(ps[poly[i + 1].0]));
            }
        }
        v
    }

    /// Count directed half-edges that lack a reverse partner (boundary edges).
    fn boundary_edge_count(mesh: &Mesh) -> usize {
        let mut directed: HashSet<(usize, usize)> = HashSet::new();
        for poly in mesh.polygons() {
            let kk = poly.len();
            for i in 0..kk {
                directed.insert((poly[i].0, poly[(i + 1) % kk].0));
            }
        }
        directed.iter().filter(|&&(a, b)| !directed.contains(&(b, a))).count()
    }

    /// V&V — headline. Methodology: revolve a vertical segment at radius 1
    /// (`z ∈ [−1, 1]`) around the Z axis, full 2π in 16 segments. Pass
    /// criterion: a closed-seam 16-gon tube, open at top and bottom. Result:
    /// V = 32 (16 rings × 2 rows), F = 16 quads, χ = 32 − 48 + 16 = 0 (an open
    /// cylinder), with 32 boundary edges (16 top + 16 bottom).
    #[test]
    fn segment_revolves_to_open_tube() {
        let profile = [Vec3::new(1.0, 0.0, -1.0), Vec3::new(1.0, 0.0, 1.0)];
        let tube = revolve(&profile, Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0), 16, TAU);
        assert_eq!(tube.vertex_count(), 32, "16 rings × 2 profile rows");
        assert_eq!(tube.face_count(), 16, "16 quad bands");
        assert_eq!(tube.euler_characteristic(), 0, "open cylinder: χ = 0");
        assert_eq!(boundary_edge_count(&tube), 32, "16 top + 16 bottom rim edges");
    }

    /// V&V — capping the tube yields a closed prism of the expected volume.
    /// Methodology: `fill_holes` the 16-gon tube. Pass criterion: closed
    /// genus-0 solid whose volume matches a regular 16-gon prism of circumradius
    /// 1 and height 2. Result: χ = 2; volume = area(16-gon)·2 =
    /// (½·16·sin(2π/16))·2 ≈ 6.1229, so 6·V ≈ 36.74.
    #[test]
    fn capped_tube_is_a_closed_prism() {
        let profile = [Vec3::new(1.0, 0.0, -1.0), Vec3::new(1.0, 0.0, 1.0)];
        let tube = revolve(&profile, Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0), 16, TAU);
        let solid = fill_holes(&tube);
        assert_eq!(solid.euler_characteristic(), 2, "capped tube is a closed solid");
        let expected_vol = 0.5 * 16.0 * (TAU / 16.0).sin() * 2.0;
        let v6 = signed_volume6(&solid).abs();
        assert!(
            (v6 - 6.0 * expected_vol).abs() < 1e-6,
            "6·V = {v6} vs expected {}",
            6.0 * expected_vol,
        );
    }

    /// V&V — a partial sweep leaves the surface open at both ends *and* along
    /// the two unstitched profile edges. Methodology: revolve the segment by
    /// only π (half turn) in 8 segments. Result: 9 rings (not wrapped), so
    /// V = 18, F = 8, and the seam is open (more boundary edges than the closed
    /// tube would have per band).
    #[test]
    fn partial_sweep_is_open() {
        let profile = [Vec3::new(1.0, 0.0, -1.0), Vec3::new(1.0, 0.0, 1.0)];
        let strip = revolve(&profile, Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0), 8, PI);
        assert_eq!(strip.vertex_count(), 18, "9 rings × 2 rows (no wrap)");
        assert_eq!(strip.face_count(), 8);
        // An open strip is a topological disc: χ = 1.
        assert_eq!(strip.euler_characteristic(), 1);
    }

    /// V&V — a two-row profile at different radii revolves into a cone frustum;
    /// its rings have the profile radii. Methodology: profile from (2,0,0) to
    /// (1,0,2) revolved full 2π/12. Result: the bottom ring sits at radius 2,
    /// the top at radius 1 (checked on the generated positions).
    #[test]
    fn frustum_rings_have_profile_radii() {
        let profile = [Vec3::new(2.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 2.0)];
        let frustum = revolve(&profile, Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0), 12, TAU);
        let ps = frustum.positions();
        // Vertex layout is ring-major, 2 rows: even indices are row 0 (r=2),
        // odd indices row 1 (r=1).
        let r_row0 = (ps[0].x * ps[0].x + ps[0].y * ps[0].y).sqrt();
        let r_row1 = (ps[1].x * ps[1].x + ps[1].y * ps[1].y).sqrt();
        assert!((r_row0 - 2.0).abs() < 1e-9, "bottom ring radius 2");
        assert!((r_row1 - 1.0).abs() < 1e-9, "top ring radius 1");
        assert_eq!(frustum.face_count(), 12);
    }
}
