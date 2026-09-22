// SPDX-License-Identifier: GPL-3.0

//! **V&V gate** — stochastic CSG volume calculation against closed-form
//! volumes, and its *quoted uncertainty* against the observed scatter.
//! GitHub #267.
//!
//! # Why analytic rather than OpenMC
//!
//! OpenMC has the same estimator, but running it would compare one Monte Carlo
//! estimate against another and inherit both noises. For a sphere, a spherical
//! shell and a box the answer is **exact**, so the analytic value is the
//! stronger reference and is the one the issue asks for. The port itself is
//! line-referenced to `src/volume_calc.cpp` at `afa7a14`; what needs measuring
//! here is whether the estimator and its error bar are right, and an exact
//! reference measures that better than a noisy one.
//!
//! (The upstream commit the issue cites, `608a1c33`, is not available in this
//! container and is not fetchable — see
//! `verification_and_validation/white_boundary/white_boundary_vs_openmc.md`.)
//!
//! # Methodology
//!
//! Three geometries with closed-form volumes, sampled in a box that bounds
//! them:
//!
//! | case | region | exact volume |
//! |---|---|---|
//! | sphere | `r < 4` | `4/3 pi r^3` = 268.082573 |
//! | shell | `2 < r < 4` | `4/3 pi (R^3 - r^3)` = 234.572400 |
//! | box | `\|x\|,\|y\|,\|z\| < 1.5` | `3^3` = 27 (exact, and the sample box is
//!   the same box, so this also checks the trivial `hit fraction = 1` limit) |
//!
//! Two separate things are then checked, and the second is the one that is
//! usually skipped:
//!
//! 1. **Accuracy** — the estimate sits within `4 sigma` of the exact value,
//!    using the estimator's *own* quoted sigma. A gate built on a fixed
//!    tolerance would not be testing the error bar at all.
//! 2. **The error bar itself** — 32 independent repeats (different master
//!    seeds, so genuinely independent streams) give an *observed* standard
//!    deviation of the 32 estimates. That is compared against the sigma the
//!    estimator quotes. If the quoted sigma were optimistic, item 1 would still
//!    pass on a lucky seed and the number would be quietly wrong for every
//!    future user.
//!
//! # Results (2026-09-22), 400 000 samples
//!
//! | case | estimate | exact | \|d\| | sigma out |
//! |---|---|---|---|---|
//! | inner ball `r < 2` | 33.4515 +/- 0.2001 | 33.5103 | 0.0588 | **0.29** |
//! | shell `2 < r < 4` | 234.9222 +/- 0.4034 | 234.5723 | 0.3500 | **0.87** |
//! | cube, self-bounded | 27.000000 +/- 0.000000 | 27 | 0 | exact |
//!
//! The hit fraction in the sphere case came back **0.5242** against the exact
//! `pi/6 = 0.5236` — an independent check that the box is being sampled
//! uniformly, since that ratio is the sphere-in-cube packing fraction and
//! depends on nothing else.
//!
//! The self-bounded cube is the degenerate limit and it is exact in both
//! directions: hit fraction exactly `1`, volume exactly `27`, and sigma exactly
//! `0` because `f(1-f)` vanishes. That is asserted with `assert_eq!`, not a
//! tolerance — an off-by-one in the binomial arrangement would show up here and
//! nowhere else, because on a curved shape it would hide inside the noise.
//!
//! ## The error bar, over 32 independent repeats
//!
//! ```text
//! exact 268.0826   mean of 32 268.0875   observed sigma 0.3386
//! quoted sigma 0.4043   ratio 0.837
//! ```
//!
//! The mean of 32 runs lands 0.0049 from exact against a standard error of
//! 0.0715, so the estimator is **unbiased** at the precision this resolves.
//!
//! **The quoted sigma runs about 19 % larger than the observed scatter.** With
//! 32 repeats the sample standard deviation is itself uncertain by
//! `1/sqrt(2(n-1))` = 12.7 %, so a ratio of 0.837 is 1.28 sigma low — entirely
//! consistent with noise, and it passes the band derived from that same
//! figure. It is recorded rather than rounded to "agrees" because the
//! *direction* is the one that matters: a quoted sigma that is too **large** is
//! conservative, and the failure mode worth fearing is the opposite. If this
//! ratio drifts above 1 on a future run, that is the signal to take seriously.
//!
//! A firmer statement would need a few hundred repeats. Not done: at 400 000
//! samples each that is minutes of wall time for a number already consistent
//! with its own noise, and the asymmetry above means the cheap version is
//! informative in the direction that counts.

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind, XPlane, YPlane, ZPlane};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::geometry::volume_calc::{
    BoundingBox, VolumeCalculation, VolumeDomain,
};

const N: u64 = 400_000;

fn sphere_geom(radii: &[f64]) -> Geometry {
    // Surfaces: concentric spheres, outermost vacuum.
    let surfaces: Vec<SurfaceKind> = radii
        .iter()
        .enumerate()
        .map(|(i, &r)| {
            SurfaceKind::Sphere(Sphere {
                x0: 0.0,
                y0: 0.0,
                z0: 0.0,
                r,
                bc: if i + 1 == radii.len() {
                    BoundaryType::Vacuum
                } else {
                    BoundaryType::Transmissive
                },
            })
        })
        .collect();

    let mut cells = Vec::new();
    // Innermost ball.
    cells.push(Cell::material(
        1,
        vec![RegionToken::HalfSpace {
            surface_idx: 0,
            sense: HalfSpaceSense::Inside,
        }],
        0,
        293.6,
    ));
    // Shells between consecutive radii.
    for i in 1..radii.len() {
        cells.push(Cell::material(
            (i + 1) as i32,
            vec![
                RegionToken::HalfSpace {
                    surface_idx: i - 1,
                    sense: HalfSpaceSense::Outside,
                },
                RegionToken::HalfSpace {
                    surface_idx: i,
                    sense: HalfSpaceSense::Inside,
                },
                RegionToken::Intersection,
            ],
            0,
            293.6,
        ));
    }
    let cell_indices = (0..cells.len()).collect();
    Geometry {
        surfaces,
        cells,
        universes: vec![Universe { id: 0, cell_indices }],
        lattices: vec![],
        root_universe: 0,
    }
}

fn cube_geom(a: f64) -> Geometry {
    let bc = BoundaryType::Vacuum;
    let surfaces = vec![
        SurfaceKind::XPlane(XPlane { x0: -a, bc }),
        SurfaceKind::XPlane(XPlane { x0: a, bc }),
        SurfaceKind::YPlane(YPlane { y0: -a, bc }),
        SurfaceKind::YPlane(YPlane { y0: a, bc }),
        SurfaceKind::ZPlane(ZPlane { z0: -a, bc }),
        SurfaceKind::ZPlane(ZPlane { z0: a, bc }),
    ];
    let mut region = vec![
        RegionToken::HalfSpace {
            surface_idx: 0,
            sense: HalfSpaceSense::Outside,
        },
        RegionToken::HalfSpace {
            surface_idx: 1,
            sense: HalfSpaceSense::Inside,
        },
        RegionToken::Intersection,
    ];
    for (idx, sense) in [
        (2, HalfSpaceSense::Outside),
        (3, HalfSpaceSense::Inside),
        (4, HalfSpaceSense::Outside),
        (5, HalfSpaceSense::Inside),
    ] {
        region.push(RegionToken::HalfSpace {
            surface_idx: idx,
            sense,
        });
        region.push(RegionToken::Intersection);
    }
    Geometry {
        surfaces,
        cells: vec![Cell::material(1, region, 0, 293.6)],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0],
        }],
        lattices: vec![],
        root_universe: 0,
    }
}


/// Accuracy: every closed-form volume recovered inside the estimator's own
/// quoted 4 sigma.
#[test]
fn closed_form_volumes_are_recovered() {
    let pi = std::f64::consts::PI;

    // Sphere r = 4, and the shell 2 < r < 4, from one geometry.
    let geom = sphere_geom(&[2.0, 4.0]);
    let calc = VolumeCalculation {
        domain: VolumeDomain::Cell,
        ids: vec![0, 1],
        box_: BoundingBox {
            lower: Position::new(-4.0, -4.0, -4.0),
            upper: Position::new(4.0, 4.0, 4.0),
        },
        n_samples: N,
        master_seed: 1,
    };
    let (res, located) = calc.execute(&geom);

    let inner_exact = 4.0 / 3.0 * pi * 2.0_f64.powi(3);
    let shell_exact = 4.0 / 3.0 * pi * (4.0_f64.powi(3) - 2.0_f64.powi(3));
    println!("hit fraction in geometry: {located:.4} (exact: pi/6 = {:.4})", pi / 6.0);
    for (r, exact, name) in [
        (res[0], inner_exact, "inner ball r<2"),
        (res[1], shell_exact, "shell 2<r<4"),
    ] {
        let d = (r.volume - exact).abs();
        println!(
            "{name:<16} got {:.4} +/- {:.4}  exact {exact:.4}  |d| {d:.4}  ({:.2} sigma)",
            r.volume,
            r.std_dev,
            d / r.std_dev
        );
        assert!(
            d <= 4.0 * r.std_dev,
            "{name}: {:.4} +/- {:.4} vs exact {exact:.4} is {:.2} sigma out",
            r.volume,
            r.std_dev,
            d / r.std_dev
        );
    }

    // A box sampled by its own bounding box: hit fraction must be exactly 1 and
    // the volume exactly the box volume, with zero variance. This is the
    // degenerate limit and it catches an off-by-one in the binomial formula
    // that a curved shape would hide in the noise.
    let cube = cube_geom(1.5);
    let calc = VolumeCalculation {
        domain: VolumeDomain::Cell,
        ids: vec![0],
        box_: BoundingBox {
            lower: Position::new(-1.5, -1.5, -1.5),
            upper: Position::new(1.5, 1.5, 1.5),
        },
        n_samples: 10_000,
        master_seed: 7,
    };
    let (res, located) = calc.execute(&cube);
    println!(
        "cube self-bounded: V = {:.6} +/- {:.6}, hit fraction {located}",
        res[0].volume, res[0].std_dev
    );
    assert_eq!(located, 1.0, "every sample must land inside a self-bounded box");
    assert!(
        (res[0].volume - 27.0).abs() < 1e-9,
        "self-bounded box volume must be exact, got {}",
        res[0].volume
    );
    assert_eq!(
        res[0].std_dev, 0.0,
        "f = 1 gives f(1-f) = 0, so sigma must be EXACTLY zero, not merely small"
    );
}

/// The quoted sigma must match the observed scatter over independent repeats.
///
/// This is the check that makes the error bar meaningful. Without it the
/// accuracy test above would pass on any estimator whose sigma is merely *big
/// enough*, including one that is wrong by a constant factor.
#[test]
fn the_quoted_uncertainty_matches_the_observed_scatter() {
    const REPEATS: usize = 32;
    let geom = sphere_geom(&[4.0]);
    let exact = 4.0 / 3.0 * std::f64::consts::PI * 4.0_f64.powi(3);

    let mut estimates = Vec::with_capacity(REPEATS);
    let mut quoted = 0.0_f64;
    for k in 0..REPEATS {
        // A different MASTER seed per repeat: `init_seed` jump-ahead makes these
        // genuinely independent streams, not offsets into one.
        let calc = VolumeCalculation {
            domain: VolumeDomain::Cell,
            ids: vec![0],
            box_: BoundingBox {
                lower: Position::new(-4.0, -4.0, -4.0),
                upper: Position::new(4.0, 4.0, 4.0),
            },
            n_samples: N,
            master_seed: 1000 + k as i64,
        };
        let (res, _) = calc.execute(&geom);
        estimates.push(res[0].volume);
        quoted = res[0].std_dev;
    }

    let mean = estimates.iter().sum::<f64>() / REPEATS as f64;
    let observed = (estimates
        .iter()
        .map(|v| (v - mean).powi(2))
        .sum::<f64>()
        / (REPEATS as f64 - 1.0))
        .sqrt();

    println!(
        "exact {exact:.4}  mean of {REPEATS} {mean:.4}  observed sigma {observed:.4}  \
         quoted sigma {quoted:.4}  ratio {:.3}",
        observed / quoted
    );

    // With 32 repeats the sample standard deviation is itself uncertain by
    // ~1/sqrt(2(n-1)) = 12.7 %, so the band is 3 of those either side, i.e.
    // roughly a factor 1.6. Derived, not picked: a tighter band would fail on
    // its own sampling noise.
    let tol = 3.0 / (2.0 * (REPEATS as f64 - 1.0)).sqrt();
    let ratio = observed / quoted;
    assert!(
        (ratio - 1.0).abs() < tol,
        "observed scatter {observed:.4} vs quoted {quoted:.4} (ratio {ratio:.3}); \
         outside 1 +/- {tol:.3}. The error bar is wrong, not the volume."
    );

    // And the mean of 32 repeats must be far closer to exact than one run is.
    let sem = quoted / (REPEATS as f64).sqrt();
    assert!(
        (mean - exact).abs() <= 4.0 * sem,
        "mean of {REPEATS} runs {mean:.4} vs exact {exact:.4} exceeds 4 x SEM ({:.4})",
        4.0 * sem
    );
}
