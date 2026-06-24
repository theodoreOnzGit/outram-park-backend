use openmc_libs::rng::lcg::prn;
use std::f64::consts::PI;
use std::time::{SystemTime, UNIX_EPOCH};

/// Returns (x, y, z) uniformly distributed in the volume of the spherical shell
/// between radii r_in and r_out (0 <= r_in < r_out).
fn random_point_in_spherical_shell(
    r_in: f64,
    r_out: f64,
    seed: &mut u64,
) -> (f64, f64, f64) {
    assert!(r_in >= 0.0 && r_in < r_out, "Require 0 <= r_in < r_out");

    // Sample radius with correct volume weighting: r ~ proportional to r^2
    let u: f64 = prn(seed);
    let r_in3 = r_in * r_in * r_in;
    let r_out3 = r_out * r_out * r_out;
    let rho = (u * (r_out3 - r_in3) + r_in3).cbrt();

    // Sample direction uniformly on the sphere.
    let z: f64 = prn(seed) * 2.0 - 1.0;
    let phi: f64 = prn(seed) * 2.0 * PI;
    let t = (1.0 - z * z).max(0.0).sqrt();
    let x = t * phi.cos();
    let y = t * phi.sin();

    (rho * x, rho * y, rho * z)
}

#[test]
fn rng_test() {
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let mut seed: u64 = t.subsec_nanos() as u64 ^ t.as_secs().wrapping_mul(0x9e3779b97f4a7c15);

    let r = 1.0;
    let r_in = 0.0;
    let r_out = r;

    for _ in 0..5 {
        let (x, y, z) = random_point_in_spherical_shell(r_in, r_out, &mut seed);
        println!("{x:.6}, {y:.6}, {z:.6}");
    }

    let r_in2 = 0.6 * r;
    let r_out2 = r;
    for _ in 0..5 {
        let (x, y, z) = random_point_in_spherical_shell(r_in2, r_out2, &mut seed);
        println!("shell: {x:.6}, {y:.6}, {z:.6}");
    }
}


type Point3 = (f64, f64, f64);

/// Returns all points whose z is between z1 and z2 (inclusive).
fn filter_points_by_z(points: &[Point3], z1: f64, z2: f64) -> Vec<Point3> {
    let (lo, hi) = if z1 <= z2 { (z1, z2) } else { (z2, z1) };
    points
        .iter()
        .copied()
        .filter(|&(_, _, z)| z >= lo && z <= hi)
        .collect()
}

#[test]
fn test_for_filtering() {
    let particles: Vec<Point3> = vec![
        (0.1, 0.2, -0.3),
        (0.5, -0.1, 0.0),
        (0.7, 0.8, 0.9),
        (-0.4, 0.3, 0.4),
    ];

    let z1 = 0.0;
    let z2 = 0.5;

    let filtered = filter_points_by_z(&particles, z1, z2);
    for (x, y, z) in filtered {
        println!("({x}, {y}, {z})");
    }
}
