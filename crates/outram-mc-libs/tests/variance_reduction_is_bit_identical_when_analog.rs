// SPDX-License-Identifier: GPL-3.0

//! **The regression pin for GitHub #258** — adding variance reduction must not
//! move a single bit of an analog run.
//!
//! # Why this shape, and why a recorded number rather than a self-comparison
//!
//! #258 threads a statistical weight and a variance-reduction configuration
//! through `transport_history`, which is the routine every recorded V&V number
//! in this crate was measured with — the Godiva `+16 ± 11 pcm`, the `op-tm9f`
//! and `op-og56` ablations, the ICSBEP residuals. If that threading perturbed
//! the RNG stream by so much as one extra draw, every one of those numbers
//! would silently become unreproducible, and nothing else in the suite would
//! notice: a shifted stream gives a *different but equally plausible* `k`.
//!
//! Comparing the new code against itself cannot detect that. So the reference
//! below was measured on the **parent commit** `9b861a861`, before any of #258
//! existed, and is hard-coded here to all 17 significant figures. The test is
//! an exact `f64` equality, not a tolerance — a tolerance would hide exactly
//! the failure this exists to catch.
//!
//! # Methodology
//!
//! Bare HEU sphere at Godiva's critical radius (8.7407 cm), U-235 + U-238 from
//! `reference-data/endf/` at 293.6 K, 2000 histories × [15 inactive + 40
//! active], seed 20260922, single-thread CPU backend, analog settings
//! (`KeffSettings::default()`'s `variance_reduction`).
//!
//! # Results
//!
//! Recorded 2026-09-22 on `9b861a861`:
//!
//! | quantity | value |
//! |---|---|
//! | `k_mean` | `9.91850110380556260e-1` |
//! | `k_std` | `4.20512201888097875e-3` |
//! | first generation | `9.34800339898639976e-1` |
//! | last generation | `9.65455778325547076e-1` |
//!
//! Reproduced bit-for-bit on the #258 tree. The `k` itself is **not** a
//! physics claim — 55 active generations of 2000 histories on a bare sphere is
//! far too noisy to say anything about Godiva, and this file makes no such
//! claim. It is a fingerprint of the RNG stream.
//!
//! # What this does NOT pin
//!
//! That survival biasing is *correct*. It pins only that leaving it off
//! changes nothing. The estimator's unbiasedness is measured separately by
//! `examples/variance_reduction_ablation.rs`.

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::{ComputeType, KeffSettings};
use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};
use outram_mc_libs::physics::variance_reduction::VarianceReduction;

const R_FUEL: f64 = 8.7407;
const TEMP: f64 = 293.6;

/// Measured on `9b861a861`, before #258. Exact, not approximate.
const K_MEAN_PRE_258: f64 = 9.918_501_103_805_562_60e-1;
const K_STD_PRE_258: f64 = 4.205_122_018_880_978_75e-3;
const K_GEN_FIRST_PRE_258: f64 = 9.348_003_398_986_399_76e-1;
const K_GEN_LAST_PRE_258: f64 = 9.654_557_783_255_470_76e-1;

fn heu() -> Option<Vec<Nuclide>> {
    let base =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/endf");
    let load = |name: &str, f: &str| -> Option<Nuclide> {
        let p = base.join(f);
        p.exists().then_some(())?;
        Nuclide::from_endf_file(&p, name, TEMP, 1.0e-3).ok()
    };
    Some(vec![
        load("U235", "n-092_U_235-ENDF8.0.endf")?,
        load("U238", "n-092_U_238.endf")?,
    ])
}

fn model() -> (Geometry, Vec<Material>) {
    let geom = Geometry {
        surfaces: vec![SurfaceKind::Sphere(Sphere {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r: R_FUEL,
            bc: BoundaryType::Vacuum,
        })],
        cells: vec![Cell::material(
            1,
            vec![RegionToken::HalfSpace {
                surface_idx: 0,
                sense: HalfSpaceSense::Inside,
            }],
            0,
            TEMP,
        )],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0],
        }],
        lattices: vec![],
        root_universe: 0,
    };
    let mats = vec![Material {
        id: 1,
        name: "HEU".into(),
        components: vec![
            NuclideComponent {
                nuclide_idx: 0,
                atom_density: 4.4994e-2,
            },
            NuclideComponent {
                nuclide_idx: 1,
                atom_density: 2.4984e-3,
            },
        ],
        temperature: TEMP,
    }];
    (geom, mats)
}

fn settings() -> KeffSettings {
    KeffSettings {
        n_particles: 2000,
        n_inactive: 15,
        n_active: 40,
        temperature_k: TEMP,
        seed: 20260922,
        compute: ComputeType::CpuSingleThread,
        ..KeffSettings::default()
    }
}

fn source() -> SourceBox {
    SourceBox {
        lower: Position::new(-R_FUEL, -R_FUEL, -R_FUEL),
        upper: Position::new(R_FUEL, R_FUEL, R_FUEL),
    }
}

/// **THE PIN.** An analog run on the #258 tree must reproduce the pre-#258
/// eigenvalue to the last bit.
#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "reconstructs U-235 and U-238 from ENDF (~2 min); runs by default"
)]
fn the_analog_path_is_bit_identical_to_the_pre_258_build() {
    let Some(nucs) = heu() else {
        eprintln!("SKIP: ENDF tapes not in this checkout");
        return;
    };
    let (geom, mats) = model();
    let st = settings();
    assert!(
        st.variance_reduction.is_analog(),
        "KeffSettings::default() must be analog; if this changed it is a \
         maintainer decision and every recorded V&V number needs re-measuring"
    );

    let r = run_keff_csg(&geom, &mats, &nucs, source(), &st, None);
    println!("k_mean = {:.17e} (recorded {K_MEAN_PRE_258:.17e})", r.k_mean);

    assert_eq!(
        r.k_mean, K_MEAN_PRE_258,
        "the analog eigenvalue moved. #258 threads a weight and a \
         variance-reduction config through the history loop; if the analog \
         path now consumes the RNG stream differently, every recorded V&V \
         number in this crate is unreproducible and none of them would say so."
    );
    assert_eq!(r.k_std, K_STD_PRE_258, "the per-generation spread moved");
    assert_eq!(r.k_by_generation[0], K_GEN_FIRST_PRE_258);
    assert_eq!(
        *r.k_by_generation.last().unwrap(),
        K_GEN_LAST_PRE_258,
        "the last generation moved: the streams diverge somewhere after the start"
    );
    assert_eq!(r.k_by_generation.len(), 55);
}

/// Turning survival biasing on must actually change the run — otherwise the
/// wiring is dead and the ablation below would be comparing a thing to itself.
///
/// This asserts only that the two differ, **not** which is right; unbiasedness
/// is a separate, statistical question measured by the ablation example.
#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "reconstructs U-235 and U-238 from ENDF (~4 min); runs by default"
)]
fn survival_biasing_is_actually_reachable_from_settings() {
    let Some(nucs) = heu() else {
        eprintln!("SKIP: ENDF tapes not in this checkout");
        return;
    };
    let (geom, mats) = model();

    let analog = run_keff_csg(&geom, &mats, &nucs, source(), &settings(), None);
    let biased_settings = KeffSettings {
        variance_reduction: VarianceReduction {
            survival_biasing: true,
            ..VarianceReduction::default()
        },
        ..settings()
    };
    assert!(!biased_settings.variance_reduction.is_analog());
    let biased = run_keff_csg(&geom, &mats, &nucs, source(), &biased_settings, None);

    println!(
        "analog k = {:.6} +/- {:.6}; survival-biased k = {:.6} +/- {:.6}",
        analog.k_mean, analog.k_std, biased.k_mean, biased.k_std
    );
    assert_ne!(
        analog.k_mean, biased.k_mean,
        "survival biasing changed nothing at all — the setting is not wired to \
         the transport kernel, and the ablation would be measuring noise"
    );
    // A sanity bound, deliberately loose: the two are different ESTIMATORS of
    // the same eigenvalue, so they must land in the same neighbourhood. This
    // is not the unbiasedness measurement.
    let sigma = (analog.k_std.powi(2) + biased.k_std.powi(2)).sqrt();
    let dev = (analog.k_mean - biased.k_mean).abs() / sigma;
    println!("difference = {:+.0} pcm ({dev:.2} sigma)", 1.0e5 * (biased.k_mean - analog.k_mean));
    assert!(
        dev < 5.0,
        "survival biasing moved k by {dev:.2} sigma. The two estimators must \
         agree within statistics; a resolved difference is a BIAS, not a \
         variance saving."
    );
}
