//! End-to-end Windowed-Multipole test on **real U-238 data** (MIT CRPG
//! `WMP_Library`, ENDF/B-VII.1). Exercises [`WindowedMultipole::load_h5`] and the
//! analytic Doppler broadening in [`WindowedMultipole::evaluate`].
//!
//! Requires the WMP file on disk (a git-lfs object, outside this repo). The test
//! **skips** cleanly if the file is absent, so it never breaks a checkout that
//! lacks the nuclear data.
//!
//! Run: `cargo test -p njoy-outram-park-fork --release`.

use njoy_outram_park_fork::wmp::WindowedMultipole;
use std::path::PathBuf;

/// Path to the U-238 WMP file (dev machine layout: sibling `WMP_Library` clone).
fn u238_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../WMP_Library/WMP_Library/092238.h5")
}

#[test]
fn u238_loads_and_has_expected_metadata() {
    let path = u238_path();
    if !path.exists() {
        eprintln!("SKIP: {} not present (git-lfs)", path.display());
        return;
    }
    let wmp = WindowedMultipole::load_h5(&path).expect("load U-238 WMP");

    assert_eq!(wmp.name, "U238");
    assert!(wmp.fissionable, "U-238 carries fission residues");
    assert!((wmp.awr - 236.0).abs() < 0.2, "AWR = {}", wmp.awr);
    assert!((wmp.e_min - 1e-5).abs() < 1e-9, "e_min = {}", wmp.e_min);
    assert!((wmp.e_max - 20000.0).abs() < 0.02, "e_max = {}", wmp.e_max);
    assert!(wmp.poles.len() > 3000, "poles = {}", wmp.poles.len());
    assert_eq!(wmp.residues.len(), wmp.poles.len());
    assert_eq!(wmp.windows.len(), wmp.curvefit.len());
    assert_eq!(wmp.fit_order, 2);
}

#[test]
fn u238_capture_resonance_and_doppler_broadening() {
    let path = u238_path();
    if !path.exists() {
        eprintln!("SKIP: {} not present (git-lfs)", path.display());
        return;
    }
    let wmp = WindowedMultipole::load_h5(&path).expect("load U-238 WMP");

    // Scan the first big capture resonance (E0 ≈ 6.67 eV) at three temperatures.
    let scan = |t: f64| -> (f64, f64) {
        let mut e_peak = 0.0;
        let mut cap_peak = 0.0;
        let mut e = 6.0;
        while e < 7.4 {
            let cap = wmp.evaluate(e, t).capture();
            if cap > cap_peak {
                cap_peak = cap;
                e_peak = e;
            }
            e += 0.001;
        }
        (e_peak, cap_peak)
    };

    let (e0, peak_0k) = scan(0.0);
    let (_, peak_300) = scan(293.6);
    let (_, peak_1000) = scan(1000.0);

    println!("U-238 6.67 eV resonance: E_peak = {e0:.3} eV");
    println!("  capture peak  0 K  = {peak_0k:.1} b");
    println!("  capture peak 294 K = {peak_300:.1} b");
    println!("  capture peak 1000 K = {peak_1000:.1} b");

    // The resonance sits near 6.67 eV.
    assert!((e0 - 6.673).abs() < 0.05, "peak at {e0} eV");
    // A tall, narrow capture resonance — thousands of barns at 0 K.
    assert!(peak_0k > 1000.0, "0 K peak = {peak_0k} b");
    // Doppler broadening lowers the peak monotonically with temperature.
    assert!(peak_300 < peak_0k, "294 K peak {peak_300} !< 0 K {peak_0k}");
    assert!(
        peak_1000 < peak_300,
        "1000 K peak {peak_1000} !< 294 K {peak_300}"
    );

    // Wing absorption rises with temperature (area is broadly conserved).
    let wing_300 = wmp.evaluate(6.5, 293.6).capture();
    let wing_1000 = wmp.evaluate(6.5, 1000.0).capture();
    assert!(wing_1000 > wing_300, "wing: {wing_1000} !> {wing_300}");
}

#[test]
fn u238_cross_sections_are_physical_across_the_range() {
    let path = u238_path();
    if !path.exists() {
        eprintln!("SKIP: {} not present (git-lfs)", path.display());
        return;
    }
    let wmp = WindowedMultipole::load_h5(&path).expect("load U-238 WMP");

    // Sweep the whole multipole range; every channel must be finite and ≥ 0.
    let mut e = 1e-3;
    while e < wmp.e_max {
        let xs = wmp.evaluate(e, 293.6);
        assert!(
            xs.scatter.is_finite() && xs.scatter >= -1e-6,
            "scatter@{e} = {}",
            xs.scatter
        );
        assert!(
            xs.absorption.is_finite() && xs.absorption >= -1e-6,
            "abs@{e} = {}",
            xs.absorption
        );
        assert!(
            xs.fission.is_finite() && xs.fission >= -1e-6,
            "fis@{e} = {}",
            xs.fission
        );
        assert!(xs.total() >= xs.absorption - 1e-6);
        e *= 1.2;
    }

    // Thermal capture (0.0253 eV) for U-238 is ~2.7 b — check the right order.
    let thermal_cap = wmp.evaluate(0.0253, 293.6).capture();
    assert!(
        (1.0..10.0).contains(&thermal_cap),
        "thermal capture = {thermal_cap} b"
    );
}
