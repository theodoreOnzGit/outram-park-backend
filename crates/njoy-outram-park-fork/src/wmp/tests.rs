//! Unit tests for `wmp` (moved verbatim out of `wmp.rs` on split).

use super::evaluate::*;
use super::*;

const TOL: f64 = 1e-6;

#[test]
fn faddeeva_matches_reference_values() {
    // w(0) = 1.
    let w0 = w_standard(Cf64::new(0.0, 0.0));
    assert!(
        (w0.re - 1.0).abs() < TOL && w0.im.abs() < TOL,
        "w(0) = {w0:?}"
    );

    // For real x, Re w(x) = e^{−x²} exactly.
    for &x in &[0.5, 1.0, 2.0, 3.0] {
        let w = w_standard(Cf64::new(x, 0.0));
        assert!((w.re - (-x * x).exp()).abs() < TOL, "Re w({x}) = {}", w.re);
    }

    // w(i) = 0.4275835761558070… (real).
    let wi = w_standard(Cf64::new(0.0, 1.0));
    assert!((wi.re - 0.427_583_576_155_807).abs() < TOL, "w(i) = {wi:?}");
    assert!(wi.im.abs() < TOL);

    // w(1+i) = 0.3047442052569126 + 0.2082189382028316 i  (scipy.wofz).
    let w = w_standard(Cf64::new(1.0, 1.0));
    assert!(
        (w.re - 0.304_744_205_256_912_6).abs() < TOL,
        "Re = {}",
        w.re
    );
    assert!(
        (w.im - 0.208_218_938_202_831_6).abs() < TOL,
        "Im = {}",
        w.im
    );
}

#[test]
fn faddeeva_integral_form_reflection() {
    // For Im z > 0 the integral form equals the standard w.
    let z = Cf64::new(0.7, 0.3);
    assert_eq!(faddeeva(z), w_standard(z));
    // For Im z ≤ 0 it is −conj(w(conj z)).
    let z = Cf64::new(0.7, -0.3);
    let expected = w_standard(z.conj()).conj().neg();
    assert_eq!(faddeeva(z), expected);
}

#[test]
fn erf_matches_reference_values() {
    assert!((erf(0.0)).abs() < TOL);
    assert!((erf(0.5) - 0.520_499_877_813_046_5).abs() < TOL);
    assert!((erf(1.0) - 0.842_700_792_949_714_9).abs() < TOL);
    assert!((erf(2.0) - 0.995_322_265_018_952_7).abs() < TOL);
    assert!((erf(-1.0) + 0.842_700_792_949_714_9).abs() < TOL);
}

/// A one-pole, one-window synthetic table with an independent 0 K oracle.
fn single_pole_table() -> WindowedMultipole {
    WindowedMultipole {
        name: "TEST1".into(),
        awr: 235.0,
        e_min: 1.0,
        e_max: 100.0,
        fissionable: false,
        poles: vec![Cf64::new(5.0, 0.05)],
        // scatter, absorption, fission residues.
        residues: vec![[
            Cf64::new(0.3, 0.1),
            Cf64::new(0.2, 0.02),
            Cf64::new(0.0, 0.0),
        ]],
        curvefit: vec![vec![[0.0, 0.0, 0.0]]], // one window, order 0, all zero
        windows: vec![WmpWindow {
            start: 0,
            end: 0,
            broaden_poly: false,
        }],
        inv_spacing: 1.0,
        fit_order: 0,
    }
}

/// Exact (bit-for-bit) equality — the WMPB round trip is lossless, so no
/// tolerance is warranted. Assumes no NaN fields (none in the fixtures).
fn assert_wmp_eq(a: &WindowedMultipole, b: &WindowedMultipole) {
    assert_eq!(a.name, b.name);
    for (x, y) in [
        (a.awr, b.awr),
        (a.e_min, b.e_min),
        (a.e_max, b.e_max),
        (a.inv_spacing, b.inv_spacing),
    ] {
        assert_eq!(x.to_bits(), y.to_bits());
    }
    assert_eq!(a.fit_order, b.fit_order);
    assert_eq!(a.fissionable, b.fissionable);
    assert_eq!(a.poles, b.poles);
    assert_eq!(a.residues, b.residues);
    assert_eq!(a.curvefit, b.curvefit);
    assert_eq!(a.windows.len(), b.windows.len());
    for (x, y) in a.windows.iter().zip(&b.windows) {
        assert_eq!(
            (x.start, x.end, x.broaden_poly),
            (y.start, y.end, y.broaden_poly)
        );
    }
}

#[test]
fn wmpb_blob_round_trips_single_pole() {
    let wmp = single_pole_table();
    let back = WindowedMultipole::from_blob(&wmp.to_blob()).unwrap();
    assert_wmp_eq(&wmp, &back);
}

#[test]
fn wmpb_blob_round_trips_fissionable_multiwindow() {
    // Three windows (incl. an empty one), fissionable, order-1 curve fit.
    let wmp = WindowedMultipole {
        name: "U235".into(),
        awr: 233.024_8,
        e_min: 1e-5,
        e_max: 2250.0,
        fissionable: true,
        poles: vec![
            Cf64::new(1.5, -0.02),
            Cf64::new(12.7, -0.3),
            Cf64::new(30.1, 0.4),
        ],
        residues: vec![
            [
                Cf64::new(0.1, 0.2),
                Cf64::new(0.3, -0.4),
                Cf64::new(0.5, 0.6),
            ],
            [
                Cf64::new(-1.1, 0.9),
                Cf64::new(2.2, -0.8),
                Cf64::new(3.3, 0.7),
            ],
            [
                Cf64::new(0.01, -0.02),
                Cf64::new(0.03, 0.04),
                Cf64::new(0.05, -0.06),
            ],
        ],
        curvefit: vec![
            vec![[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]],
            vec![[7.0, 8.0, 9.0], [10.0, 11.0, 12.0]],
            vec![[13.0, 14.0, 15.0], [16.0, 17.0, 18.0]],
        ],
        windows: vec![
            WmpWindow {
                start: 0,
                end: 1,
                broaden_poly: true,
            },
            WmpWindow {
                start: 1,
                end: 0,
                broaden_poly: false,
            }, // empty window
            WmpWindow {
                start: 2,
                end: 2,
                broaden_poly: true,
            },
        ],
        inv_spacing: 0.5,
        fit_order: 1,
    };
    let back = WindowedMultipole::from_blob(&wmp.to_blob()).unwrap();
    assert_wmp_eq(&wmp, &back);

    // The decoded table must evaluate bit-identically to the original.
    for &e in &[2.0, 150.0, 900.0] {
        let a = wmp.evaluate(e, 300.0);
        let b = back.evaluate(e, 300.0);
        assert_eq!(
            (a.scatter, a.absorption, a.fission),
            (b.scatter, b.absorption, b.fission)
        );
    }
}

#[test]
fn from_blob_rejects_corruption() {
    let good = single_pole_table().to_blob();
    // Bad magic.
    let mut m = good.clone();
    m[0] = b'X';
    assert!(WindowedMultipole::from_blob(&m).is_err());
    // Bad version.
    let mut v = good.clone();
    v[4] = 99;
    assert!(WindowedMultipole::from_blob(&v).is_err());
    // Truncated below the header.
    assert!(WindowedMultipole::from_blob(&good[..10]).is_err());
}

#[test]
fn wmpl_container_round_trips() {
    let a = single_pole_table(); // name "TEST1"
    let mut b = single_pole_table();
    b.name = "TEST2".into();
    b.poles = vec![Cf64::new(7.0, -0.2)];
    b.residues = vec![[
        Cf64::new(0.5, 0.0),
        Cf64::new(0.9, 0.1),
        Cf64::new(0.0, 0.0),
    ]];

    let image = WmpLibrary::pack(&[a.clone(), b.clone()]);
    let lib = WmpLibrary::from_blob(&image).unwrap();

    assert_eq!(lib.len(), 2);
    assert!(!lib.is_empty());
    assert_eq!(lib.names(), vec!["TEST1", "TEST2"]);
    assert!(lib.contains("TEST2") && !lib.contains("U999"));

    assert_wmp_eq(&a, &lib.get("TEST1").unwrap());
    assert_wmp_eq(&b, &lib.get("TEST2").unwrap());
    assert!(lib.get("MISSING").is_err());
}

#[test]
fn wmpl_from_blob_rejects_corruption() {
    let image = WmpLibrary::pack(&[single_pole_table()]);
    let mut bad = image.clone();
    bad[0] = b'Z'; // clobber magic
    assert!(WmpLibrary::from_blob(&bad).is_err());
    // Truncated below the header.
    assert!(WmpLibrary::from_blob(&image[..8]).is_err());
}

#[test]
fn embedded_core_library_loads_and_evaluates() {
    let lib = WmpLibrary::core();
    assert_eq!(lib.len(), 125, "CORE nuclide count");
    assert!(lib.contains("U238") && lib.contains("H1"));

    // Fissionable actinide: the 6.673 eV capture resonance is large at 300 K
    // (the U-238 Doppler target — see tests/wmp_u238.rs for the full gate).
    let u238 = lib.get("U238").unwrap();
    assert!(u238.fissionable);
    let on_res = u238.evaluate(6.673, 300.0).absorption;
    assert!(on_res > 100.0, "U-238 6.673 eV absorption = {on_res} b");

    // Non-fissionable nuclide decodes with an all-zero fission channel.
    let h1 = lib.get("H1").unwrap();
    assert!(!h1.fissionable);
    assert_eq!(h1.evaluate(1.0, 300.0).fission, 0.0);
}

#[test]
fn zero_kelvin_pole_matches_hand_computation() {
    let wmp = single_pole_table();
    let e = 25.0_f64;
    let sqrt_e = e.sqrt();
    let inv_e = 1.0 / e;
    let pole = wmp.poles[0];

    // ψχ = −i / (pole − √E) computed independently with plain f64 arithmetic.
    let dr = pole.re - sqrt_e;
    let di = pole.im;
    let mag2 = dr * dr + di * di;
    // −i / (dr + i·di) = (−di − i·dr) / mag2 · … actually −i·conj / mag2:
    // −i·(dr − i·di) = −di − i·dr ; divide by mag2, then × inv_e.
    let psi_re = (-di / mag2) * inv_e;
    let psi_im = (-dr / mag2) * inv_e;
    // σ_a = Re(residue_a · ψχ).
    let ra = wmp.residues[0][CH_ABSORPTION];
    let expected_a = ra.re * psi_re - ra.im * psi_im;

    let xs = wmp.evaluate(e, 0.0);
    assert!(
        (xs.absorption - expected_a).abs() < 1e-12,
        "{} vs {}",
        xs.absorption,
        expected_a
    );
    assert_eq!(xs.fission, 0.0);
}

#[test]
fn doppler_broadening_lowers_and_widens_the_peak() {
    // Put a narrow resonance at √E = 5 (E = 25 eV) and check that heating
    // lowers the on-resonance absorption and raises it in the wing.
    let mut wmp = single_pole_table();
    // Physical poles sit in the lower half-plane (Im < 0) so absorption is
    // positive on resonance; narrow width → stronger Doppler effect.
    wmp.poles = vec![Cf64::new(5.0, -0.01)];
    wmp.residues = vec![[
        Cf64::new(0.0, 0.0),
        Cf64::new(1.0, 0.0),
        Cf64::new(0.0, 0.0),
    ]];

    let e_peak = 25.0; // √E = 5, on resonance
    let e_wing = 30.0;

    let cold_peak = wmp.evaluate(e_peak, 300.0).absorption;
    let hot_peak = wmp.evaluate(e_peak, 2500.0).absorption;
    let cold_wing = wmp.evaluate(e_wing, 300.0).absorption;
    let hot_wing = wmp.evaluate(e_wing, 2500.0).absorption;

    assert!(
        hot_peak < cold_peak,
        "peak should drop: {hot_peak} !< {cold_peak}"
    );
    assert!(
        hot_wing > cold_wing,
        "wing should rise: {hot_wing} !> {cold_wing}"
    );
    // All evaluations finite.
    for v in [cold_peak, hot_peak, cold_wing, hot_wing] {
        assert!(v.is_finite());
    }
}
