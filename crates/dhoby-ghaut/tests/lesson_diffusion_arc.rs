//! Reaches every function of the diffusion arc's generator
//! (`examples/lesson_diffusion_arc/arc.rs`) and checks each against
//! something it can fail on, on small cases: finite volumes against Crank's
//! series, the exact profile against its own limits, Booth on the bare
//! kernel against the finite volumes, and the ordering the silver comparison
//! rests on. The generated page data are checked by
//! `docs/site/science/fission-products/check-diffarc.js`.
//!
//! Results 2026-11-04: all pass (`cargo test --release -p dhoby-ghaut --test
//! lesson_diffusion_arc`).
#![cfg(not(any(target_arch = "wasm32", target_os = "android")))]

#[path = "../examples/lesson_diffusion_arc/arc.rs"]
mod arc;

use arc::*;

#[test]
fn one_sphere_three_ways_agree() {
    let d = crp6_d(1200.0);
    assert!((d / 2.2519e-15 - 1.0).abs() < 1e-3, "{d}");
    let hours = [0.0, 100.0, 200.0];
    let crank = crank_curve(d, CRP6_KERNEL_M, &hours);
    assert_eq!(crank[0], 0.0);
    assert!((crank[2] - 0.5337).abs() < 1e-3, "CRP-6 Case 1a: {}", crank[2]);
    let fv = fv_curve(&[(CRP6_KERNEL_M, 40)], &[d], 1, 400, &hours);
    for (a, b) in fv.iter().zip(&crank) {
        assert!((a - b).abs() < 3e-3, "{a} vs {b}");
    }
}

#[test]
fn exact_profile_limits_and_fv_profile() {
    // Long after the start the profile is the first mode; at x -> 0 the
    // limit form matches a tiny x.
    let tau = 0.05;
    let near = exact_profile(1e-9, tau, 200);
    let at0 = exact_profile(0.0, tau, 200);
    assert!((near - at0).abs() < 1e-6, "{near} vs {at0}");
    assert!(exact_profile(1.0, tau, 200).abs() < 1e-9, "zero at the surface");
    assert!((exact_profile(0.5, 1e-4, 2000) - 1.0).abs() < 1e-6, "uniform deep inside early");
    let (x, c) = fv_profile(40, tau, 500);
    assert_eq!(x.len(), 40);
    for (xi, ci) in x.iter().zip(&c) {
        assert!((ci - exact_profile(*xi, tau, 400)).abs() < 0.01, "x {xi}: {ci}");
    }
}

#[test]
fn particle_setups_and_atops_formulas() {
    let r = htr10_radii();
    assert!((r[0] - 250e-6).abs() < 1e-12 && (r[4] - 455e-6).abs() < 1e-12, "{r:?}");
    assert_eq!(elements().len(), 3);
    let (name, z, nuc) = elements()[0];
    assert_eq!((name, z), ("Cs", 55));
    let d = d_layers(z, nuc, 1600.0);
    assert!(d.iter().all(|x| x.is_finite() && *x > 0.0));
    assert_eq!(d[2], d[4], "IPyC and OPyC share the PyC value");
    // Shell: ten diffusion lengths, capped at half the kernel.
    assert!((kernel_shell_m(1e-20, 500.0, 250e-6) - 10.0 * (1e-20f64 * 1.8e6).sqrt()).abs() < 1e-15);
    assert_eq!(kernel_shell_m(1e-10, 500.0, 250e-6), 125e-6);
    // B1 on a small case: the bare kernel's finite volumes reproduce Booth.
    let hours = [0.0, 50.0, 100.0];
    let (layers, dd, start) = setup(Particle::BareKernel, z, nuc, 1400.0, 100.0, 1);
    assert_eq!((layers.len(), start), (2, 2));
    let fv = fv_curve(&layers, &dd, start, 1000, &hours);
    for (h, f) in hours.iter().zip(&fv).skip(1) {
        let b = atops_failed_kernel(z, 1400.0, *h);
        assert!((f - b).abs() < 2e-3, "{h} h: FV {f} vs Booth {b}");
    }
    // Silver: breakthrough >= well-mixed >= intact at 100 h, 1600 °C.
    let ag = elements()[2];
    let run = |p| {
        let (l, dd, s) = setup(p, ag.1, ag.2, 1600.0, 100.0, 1);
        fv_curve(&l, &dd, s, 1000, &[0.0, 100.0])[1]
    };
    let (mixed, intact) = (run(Particle::WellMixedInsideSic), run(Particle::Intact));
    let atops = atops_silver(1600.0, 100.0);
    assert!(atops >= mixed && mixed >= intact, "{atops} {mixed} {intact}");
    assert_eq!(atops_silver(1600.0, 0.0), 0.0);
    assert!(dco(1.0).value == 1.0 && metres(2.0).value == 2.0 && celsius(0.0).value > 273.0);
}
