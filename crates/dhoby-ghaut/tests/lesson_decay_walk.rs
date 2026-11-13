//! Reaches every function of the generator of "An atom's clock" and
//! "Walking on spheres" (`examples/lesson_decay_walk/{decay,walk}.rs`) and
//! checks each against something it can fail on: the library's half-lives
//! and branch sums, the sampled future against the simulator's own stepping
//! and against CRAM48, the branching frequencies against the library's
//! ratios, one Gaussian step's landing regions, the Walk-on-Spheres hop
//! geometry and clock, and the release against Crank's series. Smaller
//! populations than the generator, with tolerances set from the binomial
//! sigma before running (4 sigma). The generated page data are checked by
//! `docs/site/science/fission-products/check-decaywalk.js`.
//!
//! Results 2026-11-04: all 4 pass after two assertions were corrected on
//! the first run (the µm radii carry round-off; at t = 0 the capture shell
//! already holds 0.25 % of the walkers) (`cargo test --release -p dhoby-ghaut
//! --test lesson_decay_walk`).
#![cfg(not(any(target_arch = "wasm32", target_os = "android")))]

#[path = "../examples/lesson_decay_walk/decay.rs"]
mod decay;
#[path = "../examples/lesson_decay_walk/walk.rs"]
mod walk;

use boon_lay::prelude::decay_library::DecayLibrary;
use boon_lay::Nuclide;

#[test]
fn decay_library_and_branches() {
    let lib = DecayLibrary::new();
    assert_eq!(decay::pretty(Nuclide::Kr88), "Kr-88");
    assert_eq!(decay::pretty(Nuclide::Ba137m), "Ba-137m");
    // I-131: 8.02 d (handbook), 1 %.
    let t = decay::half_life_s(&lib, Nuclide::I131).unwrap();
    assert!((t / (8.02 * 86400.0) - 1.0).abs() < 0.01, "{t}");
    assert_eq!(decay::half_life_s(&lib, Nuclide::Pb208), None, "stable");
    for p in decay::PARENTS {
        let s: f64 = decay::branches(&lib, p).iter().map(|b| b.ratio).sum();
        assert!((s - 1.0).abs() < 1e-6, "{p:?} sums to {s}");
    }
    let bi = decay::branches(&lib, Nuclide::Bi212);
    assert!(bi.iter().any(|b| b.to == Some(Nuclide::Tl208)) && bi.iter().any(|b| b.to == Some(Nuclide::Po212)));
}

#[test]
fn sampled_futures_follow_the_simulator_and_cram() {
    let mut lib = DecayLibrary::new();
    let p = Nuclide::Kr88;
    let t12 = decay::half_life_s(&lib, p).unwrap();
    let n = 2000;
    let (mut sims, atoms): (Vec<_>, Vec<_>) = (0..n).map(|_| decay::sample_atom(&mut lib, p)).unzip();
    let a = &atoms[0];
    assert_eq!(a.stages[0], p);
    assert_eq!(a.stages.len(), a.ends.len());
    assert!(a.ends.last().unwrap().is_infinite() && a.ends.windows(2).all(|w| w[0] <= w[1]));
    assert_eq!(decay::stage_at(a, 0.0), p);
    assert_eq!(decay::stage_at(a, f64::MAX), *a.stages.last().unwrap());
    let ts: Vec<f64> = (0..=10).map(|i| 0.5 * i as f64 * t12).collect();
    let (species, expected) = decay::cram(&lib, p, &ts);
    assert_eq!(species[0], p);
    assert!((expected[0][2] - 0.5).abs() < 1e-10, "CRAM48 at one half-life");
    let stepped = decay::count_by_stepping(&mut sims, &species, &ts);
    // The stepped simulators are what each atom's sampled future says.
    for (k, &t) in ts.iter().enumerate() {
        for (i, s) in species.iter().enumerate() {
            let by_clock = atoms.iter().filter(|a| decay::stage_at(a, t) == *s).count() as f64 / n as f64;
            assert!((by_clock - stepped[i][k]).abs() <= 1.0 / n as f64 + 1e-12, "{s:?} at {t}");
        }
    }
    for i in 0..species.len() {
        let (w, _) = decay::worst_sigma(&stepped[i], &expected[i], n);
        assert!(w < 4.0, "{:?}: {w} sigma", species[i]);
    }
    let bi: Vec<decay::Atom> = (0..4000).map(|_| decay::sample_atom(&mut lib, Nuclide::Bi212).1).collect();
    let r = decay::branches(&lib, Nuclide::Bi212).into_iter().find(|b| b.to == Some(Nuclide::Tl208)).unwrap().ratio;
    let f = decay::first_branch_fractions(&bi, &[Nuclide::Tl208])[0];
    assert!((f - r).abs() < 4.0 * (r * (1.0 - r) / 4000.0).sqrt(), "{f} vs {r}");
}

#[test]
fn a_gaussian_step_jumps_layers_and_wos_does_not() {
    let cell = walk::crp6_cell(1200.0);
    let r = walk::radii_um(&cell);
    // The cell stores metres; µm comes back with round-off (312.49999999999994).
    for (a, b) in r.iter().zip([212.5, 312.5, 352.5, 387.5, 427.5]) {
        assert!((a - b).abs() < 1e-9, "{r:?}");
    }
    let d = walk::buffer_d(1200.0);
    let small = walk::gaussian_steps(&cell, d, (1e-6f64).powi(2) / (2.0 * d), 2000, 1);
    let f = walk::region_fractions(&small);
    assert!(f[1] > 0.98, "a 1 µm step stays in the buffer: {f:?}");
    let s = small[0];
    assert!(s.from[2] == 0.0 && ((s.from[0].powi(2) + s.from[1].powi(2)).sqrt() - 262.5).abs() <= 50.0);
    let big = walk::region_fractions(&walk::gaussian_steps(&cell, d, (447e-6f64).powi(2) / (2.0 * d), 2000, 2));
    assert!(big[3] + big[4] + big[5] > 0.3, "a 447 µm step often lands past the IPyC: {big:?}");
    assert!((f.iter().sum::<f64>() - 1.0).abs() < 1e-9);
    assert_eq!(walk::region_index(boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::TrisoRegion::Outside), 5);

    let paths = walk::wos_paths(1200.0, 50, 9);
    for p in &paths {
        assert!(p.len() > 1);
        for w in p.windows(2) {
            let c = (w[0][0].powi(2) + w[0][1].powi(2) + w[0][2].powi(2)).sqrt();
            assert!(c + w[1][3] <= 212.5 * (1.0 + 1e-9), "hop sphere inside the kernel");
            assert!(w[1][4] > w[0][4], "the clock advances");
        }
    }
    let (m, se) = walk::theta_mean(20_000, 3);
    assert!((m - 1.0 / 6.0).abs() < 4.0 * se, "{m} +- {se}");
    assert_eq!(walk::kernel_d(1200.0), walk::cell_kernel_d(1200.0));
}

#[test]
fn release_against_crank() {
    let hours = [0.0, 50.0, 100.0, 200.0];
    let t = walk::release_times(1200.0, 2000, walk::CASE1_SEED);
    let mc = walk::fraction_by(&t, &hours);
    // At t = 0 the walkers born inside the capture shell (eps = 1e-3 R)
    // already count as released: expected 1 - 0.999^3 = 0.0030 of them
    // (first run 2026-11-04: 0.0025, 5 of 2000). That is Walk-on-Spheres'
    // own epsilon bias, the same in `mc_kernel_release_fraction`.
    let shell = 1.0 - 0.999f64.powi(3);
    assert!((mc[0] - shell).abs() < 4.0 * (shell / 2000.0).sqrt(), "{}", mc[0]);
    for (h, m) in hours.iter().zip(&mc).skip(1) {
        let c = walk::crank(1200.0, *h);
        assert!((m - c).abs() < 4.0 * (c * (1.0 - c) / 2000.0).sqrt(), "{h} h: {m} vs {c}");
    }
    assert!((walk::crank(1200.0, 200.0) - 0.5337).abs() < 1e-3);
    // The fixed-step Gaussian walk, coarse, under-releases.
    let g = walk::fraction_by(&walk::gaussian_release_times(1200.0, 0.1, 2000, 5, 200.0), &[200.0])[0];
    assert!(g < walk::crank(1200.0, 200.0) - 0.05, "{g}");
    assert!(walk::celsius(0.0).get::<uom::si::thermodynamic_temperature::kelvin>() > 273.0);
    // The record's own check, 1b (fast; 1a at 40 000 histories is the generator's).
    assert!(walk::case1(1600.0) > 0.999);
}
