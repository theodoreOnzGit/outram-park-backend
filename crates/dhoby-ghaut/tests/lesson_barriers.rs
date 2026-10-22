//! Reaches every function of chapter 4's generator
//! (`examples/lesson_barriers/{layers,walk,failure}.rs`) and checks each
//! against something it can fail on: the walker's filled coefficients (every
//! pair the gap table calls filled is called, so a wrong entry would panic
//! here, not on a page), the exact mean exit time against a hand-solved
//! uniform sphere, the thin-shell stress against r p / (2 d), Weibull's
//! median, step independence, and the fuel_failure::htr10 docs' table.
//! The generated page data are checked by
//! `docs/site/science/fission-products/check-fission.js` (checks 36-44).
//!
//! Results 2026-10-22: all 6 pass (`cargo test --release -p dhoby-ghaut
//! --test lesson_barriers`).
#![cfg(not(any(target_arch = "wasm32", target_os = "android")))]

#[path = "../examples/lesson_barriers/layers.rs"]
mod layers;
#[path = "../examples/lesson_barriers/walk.rs"]
mod walk;
#[path = "../examples/lesson_barriers/failure.rs"]
mod failure;

use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::TrisoPebbleLayerMaterial as M;
use boon_lay::Nuclide;

#[test]
fn every_filled_coefficient_can_be_called_and_the_gaps_are_refused() {
    for (_, n) in layers::walker_elements() {
        for (_, l) in layers::layers() {
            let d = layers::walker_d(l, n, 1200.0);
            match layers::has_walker_coefficient(l, n) {
                Ok(()) => assert!(d.unwrap() > 0.0),
                Err(_) => assert!(d.is_none()),
            }
        }
        assert!(layers::has_walker_coefficient(M::MatrixGraphite, n).is_err());
        assert!(layers::walker_d(M::StructuralGraphite, n, 1200.0).is_none());
    }
    assert!(layers::has_walker_coefficient(M::Buffer, Nuclide::Kr85).is_err());
    let th = layers::htr10_thicknesses();
    let r = layers::htr10_radii();
    assert!((th[3].1 - 35e-6).abs() < 1e-12 && (r[4] - 455e-6).abs() < 1e-12);
    assert_eq!(layers::crossing_time(2.0, 4.0), 1.0);
}

#[test]
fn exact_mean_exit_time() {
    // Sum of the terms, and the kernel term equal to a^2/(15 D) (the uniform
    // sphere's mean residence, born evenly, absorbed at the surface).
    let (m, t) = walk::exact_mean_exit(Nuclide::Cs137, 1600.0).unwrap();
    assert!((t.iter().sum::<f64>() - m).abs() <= 1e-12 * m);
    let a = layers::htr10_radii()[0];
    let d = layers::walker_d(M::KernelUO2, Nuclide::Cs137, 1600.0).unwrap();
    assert!((t[0] - a * a / (15.0 * d)).abs() <= 1e-12 * t[0]);
    assert!(t[3] > 0.5 * m, "SiC dominates Cs at 1600 °C");
    assert!(walk::exact_mean_exit(Nuclide::Kr85, 1600.0).is_none(), "Kr: buffer gap");
    let cell = walk::htr10_cell(1400.0);
    assert!((cell.get_opyc_radius().get::<uom::si::length::meter>() - 455e-6).abs() < 1e-12);
    assert!(walk::params(10).max_steps == 10 && walk::capture_eps_nm() > 0.0);
}

#[test]
fn the_walker_probe_stays_inside() {
    let (rmax, per, _, released) = walk::probe(Nuclide::Cs137, 1600.0, 7, 20_000);
    assert!(!released && rmax > 0.0 && rmax < layers::htr10_radii()[4]);
    assert_eq!(per.iter().sum::<u64>(), 20_000);
}

#[test]
fn pressure_stress_and_weibull() {
    let p = failure::particle(false);
    let (fd, opf, tot, gas, co) = failure::pressure_parts(&p, 1600.0, 200.0);
    assert!(fd > 0.0 && fd <= 1.0 && opf > 0.0);
    assert!((gas + co - tot).abs() <= 1e-12 * tot);
    assert!((failure::pressure_at(&p, 1600.0, 200.0) - tot).abs() <= 1e-12 * tot);
    let (ri, ra, rm, d0, vk, vf) = failure::geometry();
    assert!(ri < rm && rm < ra && (d0 - (ra - ri)).abs() < 1e-15 && vk > 0.0 && vf > 0.0);
    assert!((failure::stress(1e7, 1.0) - rm * 1e7 / (2.0 * d0)).abs() < 1e-6);
    assert!((failure::stress(1e7, 1.1) / failure::stress(1e7, 1.0) - 1.1).abs() < 1e-12);
    let (s0, m) = failure::strength_at(1.4);
    assert!((failure::weibull(s0, s0, m) - 0.5).abs() < 1e-12);
    assert!(failure::strength_at(0.0).0 > s0);
    // Linear in burnup (P12's finding).
    let p1 = failure::pressure_parts(&failure::particle_at_burnup(0.04), 1600.0, 200.0).2;
    let p3 = failure::pressure_parts(&failure::particle_at_burnup(0.12), 1600.0, 200.0).2;
    assert!((p3 / p1 - 3.0).abs() < 1e-9);
}

#[test]
fn thinning_and_decomposition() {
    let f1 = failure::thinning(1600.0, 200.0, 1);
    let f40 = failure::thinning(1600.0, 200.0, 40);
    assert!((f1 - f40).abs() < 1e-12 && f1 > 1.0);
    let (fig, printed) = failure::corrosion(1600.0);
    assert!((printed / fig - 10.0).abs() < 1e-9);
    assert!(failure::prefactors().0 < failure::prefactors().1);
    let (k, z, sphere, loose) = failure::decomposition(2200.0, 200.0);
    assert!(k > 0.0 && z > 0.0 && sphere > 0.9 && loose > 0.0);
    let (rate, m) = failure::grain_boundary(1600.0, 200.0, 8.0);
    assert!(rate > 0.0 && m < 8.0 && m > 0.44 * 8.0);
    assert!((failure::total(0.0, 0.1, 0.2) - 0.28).abs() < 1e-12);
    assert_eq!(failure::as_manufactured_target(), 6.0e-5);
}

#[test]
fn accident_history_and_release() {
    // The htr10 docs' 200 h table at 1600 °C (2026-09-24): phi1 3.10e-5.
    let rows = failure::run_history(&failure::particle(false), &failure::hold(1600.0, 200.0, 40));
    let last = rows.last().unwrap();
    assert!((last.hours - 200.0).abs() < 1e-9 && (last.phi1 / 3.10e-5 - 1.0).abs() < 0.002, "{}", last.phi1);
    assert!((failure::phi1_start() / 1.2e-12 - 1.0).abs() < 0.03);
    assert!((failure::eoi(failure::T_B_C) - failure::phi1_start()).abs() == 0.0);
    let ramp = failure::run_history(&failure::particle(false), &failure::ramp(1800.0));
    assert!(ramp.windows(2).all(|w| w[1].phi1 >= w[0].phi1 && w[1].phi2 >= w[0].phi2));
    let rel = failure::kernel_release(55, &rows);
    assert!(rel[0] == 0.0 && *rel.last().unwrap() > 0.99);
    assert_eq!(failure::kernel_radius_um(), 250.0);
    assert_eq!(failure::sic_um(), (380.0, 415.0));
    assert!((failure::micrometres(1e-6) - 1.0).abs() < 1e-12 && (failure::seconds(1.0) - 3600.0).abs() < 1e-9);
    assert!((failure::r(0.5).get::<uom::si::ratio::ratio>() - 0.5).abs() == 0.0);
    assert!(failure::temp(0.0).get::<uom::si::thermodynamic_temperature::kelvin>() > 273.0);
}
