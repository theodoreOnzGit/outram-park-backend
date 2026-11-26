// SPDX-License-Identifier: AGPL-3.0-only
//
// Unit tests for `boon_lay::strato::sturm_liouville`.
//
// The first block is PORTED from STRATO's `src/analytical/sturm_liouville.rs`
// `#[cfg(test)] mod tests` (https://github.com/weewzr/STRATO, commit
// 36f1a98848bec9d66fe515994e075cff09d6cefb, author Wee Zhern Ray, weewzr,
// AGPL-3.0-only): same inputs, same tolerances, same names. The second block
// is boon-lay's own (marked), reaching every local addition.

use super::sturm_liouville::*;
use core::f64::consts::PI;
use uom::si::diffusion_coefficient::square_meter_per_second;
use uom::si::f64::{DiffusionCoefficient, Length, Time};
use uom::si::length::meter;
use uom::si::ratio::ratio;
use uom::si::time::second;

fn five_equal(d: f64) -> Vec<Layer> {
    [2.0e-4, 3.0e-4, 3.5e-4, 4.0e-4, 4.5e-4]
        .iter()
        .map(|&r| Layer {
            outer_radius: r,
            diffusivity: d,
        })
        .collect()
}

/// Closed form for kernel-only initial data in a homogeneous absorbing sphere.
fn homogeneous_kernel_release(t: f64, r1: f64, r: f64, d: f64) -> f64 {
    let mut remaining = 0.0;
    for n in 1..2000 {
        let a = n as f64 * PI / r;
        let i1 = ((a * r1).sin() - a * r1 * (a * r1).cos()) / (a * a);
        let i_n = r * r * (-1.0f64).powi(n + 1) / (n as f64 * PI);
        let w = 3.0 * i1 * i_n / (r1.powi(3) * (0.5 * r));
        remaining += w * (-a * a * d * t).exp();
    }
    1.0 - remaining
}

fn strato_five_layers() -> [Layer; 5] {
    [
        Layer {
            outer_radius: 2.0e-4,
            diffusivity: 1.0e-12,
        },
        Layer {
            outer_radius: 3.0e-4,
            diffusivity: 4.0e-10,
        },
        Layer {
            outer_radius: 3.5e-4,
            diffusivity: 8.0e-13,
        },
        Layer {
            outer_radius: 4.0e-4,
            diffusivity: 2.0e-14,
        },
        Layer {
            outer_radius: 4.5e-4,
            diffusivity: 7.0e-13,
        },
    ]
}

#[test]
fn robin_roots_satisfy_residual() {
    let roots = robin_roots(2.0, 4);
    assert_eq!(roots.len(), 4);
    for z in roots {
        assert!(robin_residual(z, 2.0).abs() < 1e-10);
    }
}

#[test]
fn transfer_zero_length_is_identity() {
    let (u, q) = transfer_layer(2.0, -3.0, 0.5, 1.7, 0.0);
    assert!((u - 2.0).abs() < 1e-14);
    assert!((q + 3.0).abs() < 1e-14);
}

#[test]
fn transfer_preserves_state_across_equal_material_subdivision() {
    let (u1, q1) = transfer_layer(1.2, 0.7, 2.0, 0.9, 0.3);
    let (u2, q2) = transfer_layer(u1, q1, 2.0, 0.9, 0.4);
    let (u, q) = transfer_layer(1.2, 0.7, 2.0, 0.9, 0.7);
    assert!((u2 - u).abs() < 1e-12);
    assert!((q2 - q).abs() < 1e-12);
}

#[test]
fn interface_state_conserves_flux_and_reduces_to_identity() {
    // equal diffusivity: no jump
    assert_eq!(interface_state(0.7, 1.3, 2.0, 2.0, 0.5), 0.7);
    // D (u' - u/r) must match on both sides
    let (u, r, d1, d2, q1) = (1.3, 0.5, 2.0, 5.0, 0.7);
    let q2 = interface_state(q1, u, d1, d2, r);
    let lhs = q1 - d1 * u / r;
    let rhs = q2 - d2 * u / r;
    assert!((lhs - rhs).abs() < 1e-14);
}

#[test]
fn sturm_count_matches_root_count_and_is_monotone() {
    // Upstream writes the five layers inline; boon-lay shares them through
    // `strato_five_layers` (same values).
    let layers = strato_five_layers();
    let (roots, covered) = eigenvalues(&layers, 2.0e-3, 10_000).unwrap();
    assert_eq!(roots.len(), eigenvalue_count(covered, &layers));
    assert!(roots.windows(2).all(|w| w[1] > w[0]));
    let mut last = 0;
    for i in 0..400 {
        let n = eigenvalue_count(2.0e-3 * (i as f64 + 1.0) / 400.0, &layers);
        assert!(n >= last);
        last = n;
    }
}

#[test]
fn modal_release_matches_closed_form_for_equal_diffusivity() {
    let d = 1.0e-10;
    let layers = five_equal(d);
    let series = ModalRelease::new(&layers, 45.0 / 20.0, 20_000).unwrap();
    for t in [20.0, 100.0, 500.0, 2000.0] {
        let exact = homogeneous_kernel_release(t, 2.0e-4, 4.5e-4, d);
        assert!(
            (series.release_fraction(t) - exact).abs() < 1e-9,
            "t={t}: {} vs {exact}",
            series.release_fraction(t)
        );
    }
}

#[test]
fn release_is_monotone_and_bounded() {
    let layers = strato_five_layers();
    let series = ModalRelease::new(&layers, 45.0 / 5.0e3, 20_000).unwrap();
    let mut last = 0.0;
    for i in 1..=60 {
        let f = series.release_fraction(5.0e3 * i as f64 * 5.0);
        assert!((0.0..=1.0).contains(&f) && f >= last - 1e-12, "F={f}");
        last = f;
    }
}

// ---------------------------------------------------------------------------
// boon-lay's own tests (NOT upstream): every local change is reached here.
// ---------------------------------------------------------------------------

/// Crank's uniform-sphere (Booth) series, written here from u_n = sin(n pi r/R):
/// F = 1 - (6/pi^2) sum_{n>=1} exp(-n^2 pi^2 D t / R^2) / n^2.
fn booth_uniform_sphere(t: f64, r: f64, d: f64) -> f64 {
    let mut sum = 0.0;
    for n in 1..20_000 {
        let nf = n as f64;
        sum += (-nf * nf * PI * PI * d * t / (r * r)).exp() / (nf * nf);
    }
    1.0 - 6.0 / (PI * PI) * sum
}

/// L5 smoke check: `single_sphere` (one layer, uniform start) against the
/// Booth closed form at D t / R^2 in {1e-3, 1e-2, 0.1, 1}, R = 450 um,
/// D = 1e-12 m^2/s, absolute 1e-9. The formal gate (B2 in the plan) lives
/// with the V&V record, not here.
#[test]
fn single_sphere_matches_booth_uniform_sphere() {
    let (r, d) = (4.5e-4, 1.0e-12);
    let taus = [1e-3, 1e-2, 0.1, 1.0];
    let t_min = taus[0] * r * r / d;
    let series = ModalRelease::single_sphere(r, d, 45.0 / t_min, 20_000).unwrap();
    // No interface: min_flux_ratio is never updated and the floor is eps-level.
    assert!(series.min_flux_ratio.is_infinite());
    assert_eq!(series.accuracy_floor(), 2.2e-16);
    assert!(series.min_valid_time() <= t_min);
    for tau in taus {
        let t = tau * r * r / d;
        let exact = booth_uniform_sphere(t, r, d);
        let f = series.release_fraction(t);
        assert!((f - exact).abs() < 1e-9, "Dt/R^2={tau}: {f} vs {exact}");
    }
}

/// L5: the eigenvalues of `single_sphere` are n^2 pi^2 D / R^2.
#[test]
fn single_sphere_eigenvalues_are_booth_eigenvalues() {
    let (r, d) = (4.5e-4, 1.0e-12);
    let series = ModalRelease::single_sphere(r, d, 1.0, 50).unwrap();
    assert_eq!(series.eigenvalues.len(), 50);
    for (i, lam) in series.eigenvalues.iter().enumerate() {
        let n = (i + 1) as f64;
        let exact = n * n * PI * PI * d / (r * r);
        assert!((lam - exact).abs() <= 1e-12 * exact, "n={n}: {lam} vs {exact}");
    }
}

/// L5: one-layer validation rejects what upstream's per-layer predicate
/// rejects; upstream's `new` still rejects a single layer.
#[test]
fn single_sphere_validation_and_new_still_needs_two_layers() {
    assert_eq!(
        ModalRelease::single_sphere(-1.0, 1e-12, 1.0, 10).unwrap_err(),
        StratoError::InvalidLayers
    );
    assert_eq!(
        ModalRelease::single_sphere(1e-4, 0.0, 1.0, 10).unwrap_err(),
        StratoError::InvalidLayers
    );
    assert_eq!(
        ModalRelease::single_sphere(1e-4, f64::NAN, 1.0, 10).unwrap_err(),
        StratoError::InvalidLayers
    );
    assert_eq!(
        ModalRelease::single_sphere(1e-4, 1e-12, 0.0, 10).unwrap_err(),
        StratoError::InvalidTruncation
    );
    let one = [Layer {
        outer_radius: 1e-4,
        diffusivity: 1e-12,
    }];
    assert_eq!(
        ModalRelease::new(&one, 1.0, 10).unwrap_err(),
        StratoError::TooFewLayers
    );
}

/// L1: every error variant prints upstream's `bail!` text.
#[test]
fn strato_error_display_matches_upstream_messages() {
    use alloc::string::ToString;
    let cases = [
        (StratoError::TooFewLayers, "at least two layers are required"),
        (
            StratoError::InvalidLayers,
            "layers need increasing finite radii and positive finite diffusivities",
        ),
        (
            StratoError::InvalidTruncation,
            "lambda_max and max_modes must be positive",
        ),
        (
            StratoError::CouldNotBracketLowestEigenvalue,
            "could not bracket the lowest eigenvalue",
        ),
        (
            StratoError::UnresolvedEigenvalueCluster { lambda: 1.5e-3 },
            "unresolved eigenvalue cluster near Lambda = 1.500000e-3",
        ),
        (
            StratoError::NonFiniteModalWeight { lambda: 2.0 },
            "non-finite modal weight at Lambda = 2.000000e0",
        ),
        (StratoError::NoPositiveTime, "need at least one positive time"),
    ];
    for (e, msg) in cases {
        assert_eq!(e.to_string(), msg);
    }
}

/// `exact_release` (upstream, untested upstream): None before
/// `min_valid_time`, the series value after, and the no-positive-time error.
#[test]
fn exact_release_masks_early_times() {
    let layers = strato_five_layers();
    let out = exact_release(&layers, &[0.0, 1.0e4, 2.0e5], 20_000).unwrap();
    assert_eq!(out[0], None);
    let series = ModalRelease::new(&layers, 45.0 / 1.0e4, 20_000).unwrap();
    assert_eq!(out[1], Some(series.release_fraction(1.0e4)));
    assert_eq!(out[2], Some(series.release_fraction(2.0e5)));
    assert_eq!(
        exact_release(&layers, &[0.0, -1.0], 10).unwrap_err(),
        StratoError::NoPositiveTime
    );
}

/// L4: the uom facade stores SI values and `release_fraction_at` is exactly
/// `release_fraction` where valid, `None` before `min_valid_time`.
#[test]
fn uom_facade_round_trip() {
    use uom::si::length::micrometer;
    let l = Layer::new(
        Length::new::<meter>(4.5e-4),
        DiffusionCoefficient::new::<square_meter_per_second>(1.0e-12),
    );
    assert_eq!(l.outer_radius, 4.5e-4);
    assert_eq!(l.diffusivity, 1.0e-12);
    let lu = Layer::new(
        Length::new::<micrometer>(450.0),
        DiffusionCoefficient::new::<square_meter_per_second>(1.0e-12),
    );
    assert!((lu.outer_radius - 4.5e-4).abs() <= 1e-18);

    let layers = strato_five_layers();
    let series = ModalRelease::new(&layers, 45.0 / 1.0e4, 20_000).unwrap();
    let valid = series.min_valid_time();
    assert!(series
        .release_fraction_at(Time::new::<second>(0.5 * valid))
        .is_none());
    let t = 2.0e5;
    let f = series.release_fraction_at(Time::new::<second>(t)).unwrap();
    assert_eq!(f.get::<ratio>(), series.release_fraction(t));
}
