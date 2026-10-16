// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! Quick checks of the gh:#849 cells, by sampling the ASSEMBLED geometry.
//!
//! # Methodology
//!
//! Points uniform in the cell's bounding volume (a fixed-seed xorshift) are
//! located in the built `Geometry`, and the fraction landing in each
//! material is compared with an estimate that does not go through the CSG:
//! the analytic packing for the ball fraction, and, for the kernel, the
//! fraction of the same points within the fuel-zone radius of a fuel ball
//! centre (from [`Htr10Cell::balls`]) times the kernel share of a fuel zone,
//! `n_particles * r_kernel^3 / r_zone^3`. Pass: each pair within 4 combined
//! binomial sigma. Points outside the cell (the hexagon's bounding box) are
//! skipped.
//!
//! # Results (2026-10-13)
//!
//! Printed with `--nocapture`; recorded in
//! `verification_and_validation/htr10_mgxs_cells/README.md`.

use super::*;
use crate::htr10_rmc::core_model::mat;
use outram_mc_libs::geometry::cell::SurfaceToken;
use outram_mc_libs::geometry::position::Direction;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// `(points inside, ball fraction, kernel fraction, kernel fraction expected
/// from the ball list)`.
fn sample(c: &Htr10Cell, n: usize) -> (usize, f64, f64, f64) {
    let design = Htr10CoreDesign::default();
    let r_zone = design.fuel_zone_radius_cm;
    let r_kernel = design.triso_radii_cm[0];
    let kernel_share = c.triso_particles as f64 * (r_kernel / r_zone).powi(3);
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let [hx, hy, hz] = c.half_extent;
    let u = Direction::new(0.3, 0.4, (1.0f64 - 0.25).sqrt());
    let (mut inside, mut ball, mut kernel, mut zone) = (0usize, 0usize, 0usize, 0usize);
    for _ in 0..n {
        let p = Position::new(
            hx * (2.0 * rng.next() - 1.0),
            hy * (2.0 * rng.next() - 1.0),
            hz * (2.0 * rng.next() - 1.0),
        );
        let Some(path) = c.geometry.locate(p, u, SurfaceToken::NONE) else {
            continue; // outside the prism
        };
        inside += 1;
        match path.material {
            Some(m) if m == mat::HELIUM => {}
            Some(m) => {
                ball += 1;
                if m == mat::KERNEL {
                    kernel += 1;
                }
            }
            None => panic!("a point inside the cell has no material: {p:?}"),
        }
        let in_zone = c.balls.iter().any(|&([x, y, z], fuel)| {
            fuel && (p.x - x).powi(2) + (p.y - y).powi(2) + (p.z - z).powi(2) < r_zone * r_zone
        });
        zone += usize::from(in_zone);
    }
    let f = |k: usize| k as f64 / inside as f64;
    (inside, f(ball), f(kernel), f(zone) * kernel_share)
}

fn within(label: &str, got: f64, want: f64, n: usize) {
    let sigma = (want * (1.0 - want) / n as f64).sqrt().max(1e-12);
    let z = (got - want) / sigma;
    println!("{label}: sampled {got:.5}, expected {want:.5} ({z:+.2} sigma)");
    assert!(z.abs() < 4.0, "{label}: {got} vs {want} is {z:.2} sigma");
}

#[test]
fn single_pebble_holds_the_core_pebble_at_simple_cubic_packing() {
    let c = Htr10Cell::single_pebble();
    assert_eq!(c.triso_particles, 8335, "Li (2014) Table 2, Şeker p.266");
    assert_eq!(c.volume_cm3, 216.0);
    let (n, ball, kernel, kernel_want) = sample(&c, 200_000);
    assert_eq!(n, 200_000, "every point of the cube is inside the cell");
    within("single pebble ball fraction", ball, std::f64::consts::PI / 6.0, n);
    within("single pebble kernel fraction", kernel, kernel_want, n);
}

#[test]
fn seker_clusters_hold_seker_packing_and_the_core_fuel_rule() {
    let seker = SekerCell::from_paper().packing_fraction();
    for (rings, tiles) in [(1, 1.0), (2, 7.0), (3, 19.0)] {
        let c = Htr10Cell::seker_cluster(rings);
        let tile_vol = SekerCell::from_paper().area() * SekerCell::from_paper().height;
        assert!(
            (c.volume_cm3 / tile_vol - tiles).abs() < 1e-9,
            "rings {rings}: the prism holds {tiles} tiles' worth"
        );
        let n_balls = c.fuel_balls + c.dummy_balls;
        // The core's low-discrepancy rule: floor(n f) of n balls are fuel.
        assert_eq!(c.fuel_balls, (n_balls as f64 * 0.57).floor() as usize, "rings {rings}");
        println!(
            "rings {rings}: {} balls with a piece inside ({} fuel, {} dummy)",
            n_balls, c.fuel_balls, c.dummy_balls
        );
        let (n, ball, kernel, kernel_want) = sample(&c, 200_000);
        within(&format!("rings {rings} ball fraction"), ball, seker, n);
        within(&format!("rings {rings} kernel fraction"), kernel, kernel_want, n);
    }
}

#[test]
fn rings_out_of_range_are_refused() {
    let r = std::panic::catch_unwind(|| Htr10Cell::seker_cluster(4));
    assert!(r.is_err());
}
