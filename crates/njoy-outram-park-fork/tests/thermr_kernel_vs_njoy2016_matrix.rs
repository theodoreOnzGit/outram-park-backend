//! **V&V — the thermal emission *kernel* against NJOY2016's own MF=6 matrix,
//! and the measurement that localises GitHub #188.**
//!
//! # What this settles
//!
//! #188 records the bound thermal kernel as **one-signed narrow** against
//! NJOY — graphite's stationary-distribution shape sits ≈1.5 % below a
//! Maxwellian — and a long series of candidates has been eliminated by
//! measurement: the S(α,β) interpolation and its β-axis scheme, the declared
//! INT law, temperature interpolation, the small-α corner, `terpq`, the liquid
//! branch, the `test2` floor, E′ truncation at β_max, uniform and adaptive E′
//! refinement, and μ quadrature refinement.
//!
//! It is none of those. It is **the emission tabulation itself** — the
//! home-grown `IncoherentInelastic::equiprobable_emission`, which is not a port
//! of anything. Measured here against NJOY2016's own MF=6/MT=229 emission
//! matrix, over all 107 incident energies of THERMR's grid:
//!
//! ```text
//!   path                                    worst shape deviation vs NJOY
//!   ported calcem iform=0                        +0.0000 %   (exact)
//!   equiprobable_emission (what the MC uses)     -1.584 %  at 2.2800e-2 eV
//! ```
//!
//! The old path's error is one-signed narrow, largest in the thermal range
//! (−1.06 % at 1e-5 eV, −1.55 % at 0.0253 eV, −0.04 % by 1.7 eV), which is
//! #188's signature; and −1.58 % is the size of the fixed-point shape deficit
//! (−1.46 % for graphite) that the detailed-balance oracle measures
//! independently. The **cure is to feed the Monte Carlo from the ported
//! `calcem` instead**, which reproduces NJOY exactly.
//!
//! # Provenance (DATA_POLICY.md)
//!
//! NJOY2016 `2016.79 18Mar25`, run 2026-09-13 on ENDF/B-VIII.0
//! `tsl-crystalline-graphite.endf` (MAT 30) against `n-006_C_012-ENDF8.0.endf`
//! (MAT 625) at 600 K. The deck is reproduced in
//! `thermr_calcem_vs_njoy2016_golden.rs`; this test uses the **same run**, whose
//! MF=6/MT=229 section was reduced to the moments below. Both evaluations are
//! open and catalogued in `reference-data/endf/README.md`.
//!
//! The golden is stored as moments rather than the raw matrix because the
//! matrix is 76 277 card images; the moments are what the comparison uses and
//! what #188 is about.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;
use njoy_outram_park_fork::thermr::calcem::iform0::compute_iform0;
use njoy_outram_park_fork::thermr::mf7::parse_mf7_at_temperature;

/// NJOY2016's own `(E [eV], <E'^2>/<E'>^2, <E'> [eV])` for graphite at 600 K,
/// reduced from the MF=6/MT=229 emission matrix of the run described above.
#[rustfmt::skip]
const NJOY_KERNEL_600K: &[(f64, f64, f64)] = &[
    (1.000000e-05, 1.547702e+00, 6.370110e-02),
    (1.780000e-05, 1.547936e+00, 6.368763e-02),
    (2.500000e-05, 1.548151e+00, 6.367518e-02),
    (3.500000e-05, 1.548452e+00, 6.365788e-02),
    (5.000000e-05, 1.548903e+00, 6.363189e-02),
    (7.000000e-05, 1.549507e+00, 6.359716e-02),
    (1.000000e-04, 1.550416e+00, 6.354498e-02),
    (1.260000e-04, 1.551207e+00, 6.349966e-02),
    (1.600000e-04, 1.552249e+00, 6.344010e-02),
    (2.000000e-04, 1.553480e+00, 6.336995e-02),
    (2.530000e-04, 1.555166e+00, 6.327454e-02),
    (2.970000e-04, 1.556528e+00, 6.319723e-02),
    (3.500000e-04, 1.558182e+00, 6.310367e-02),
    (4.200000e-04, 1.560390e+00, 6.297939e-02),
    (5.060000e-04, 1.563135e+00, 6.282571e-02),
    (6.150000e-04, 1.566667e+00, 6.262930e-02),
    (7.500000e-04, 1.571121e+00, 6.238372e-02),
    (8.700000e-04, 1.575152e+00, 6.216335e-02),
    (1.012000e-03, 1.580004e+00, 6.190037e-02),
    (1.230000e-03, 1.587618e+00, 6.149226e-02),
    (1.500000e-03, 1.597163e+00, 6.098583e-02),
    (1.800000e-03, 1.608064e+00, 6.041692e-02),
    (2.030000e-03, 1.616531e+00, 5.998010e-02),
    (2.277000e-03, 1.626045e+00, 5.949865e-02),
    (2.600000e-03, 1.638052e+00, 5.889072e-02),
    (3.000000e-03, 1.653224e+00, 5.813496e-02),
    (3.500000e-03, 1.671558e+00, 5.722454e-02),
    (4.048000e-03, 1.690915e+00, 5.626661e-02),
    (4.500000e-03, 1.706221e+00, 5.550850e-02),
    (5.000000e-03, 1.722242e+00, 5.470952e-02),
    (5.600000e-03, 1.740100e+00, 5.380583e-02),
    (6.325000e-03, 1.759436e+00, 5.279802e-02),
    (7.200000e-03, 1.779284e+00, 5.170429e-02),
    (8.100000e-03, 1.795600e+00, 5.071510e-02),
    (9.108000e-03, 1.808788e+00, 4.976729e-02),
    (1.000000e-02, 1.816206e+00, 4.905884e-02),
    (1.063000e-02, 1.819141e+00, 4.862801e-02),
    (1.150000e-02, 1.820343e+00, 4.811894e-02),
    (1.239700e-02, 1.818385e+00, 4.769189e-02),
    (1.330000e-02, 1.813582e+00, 4.734823e-02),
    (1.417000e-02, 1.806525e+00, 4.710042e-02),
    (1.500000e-02, 1.797954e+00, 4.692630e-02),
    (1.619200e-02, 1.783032e+00, 4.677461e-02),
    (1.820000e-02, 1.753005e+00, 4.673704e-02),
    (1.990000e-02, 1.725100e+00, 4.686903e-02),
    (2.049300e-02, 1.715241e+00, 4.694312e-02),
    (2.150000e-02, 1.697983e+00, 4.710621e-02),
    (2.280000e-02, 1.674845e+00, 4.739289e-02),
    (2.530000e-02, 1.629057e+00, 4.815642e-02),
    (2.800000e-02, 1.579886e+00, 4.922602e-02),
    (3.061300e-02, 1.534372e+00, 5.052761e-02),
    (3.380000e-02, 1.483338e+00, 5.226438e-02),
    (3.650000e-02, 1.444135e+00, 5.388272e-02),
    (3.950000e-02, 1.405028e+00, 5.580578e-02),
    (4.275700e-02, 1.367478e+00, 5.798968e-02),
    (4.650000e-02, 1.330422e+00, 6.066124e-02),
    (5.000000e-02, 1.300647e+00, 6.323099e-02),
    (5.692500e-02, 1.253690e+00, 6.848933e-02),
    (6.250000e-02, 1.225362e+00, 7.280463e-02),
    (6.900000e-02, 1.197833e+00, 7.803739e-02),
    (7.500000e-02, 1.177524e+00, 8.296014e-02),
    (8.197200e-02, 1.160582e+00, 8.859895e-02),
    (9.000000e-02, 1.145647e+00, 9.511808e-02),
    (9.600000e-02, 1.136546e+00, 1.000295e-01),
    (1.035000e-01, 1.127183e+00, 1.061912e-01),
    (1.115730e-01, 1.118945e+00, 1.128521e-01),
    (1.200000e-01, 1.110858e+00, 1.199465e-01),
    (1.280000e-01, 1.104059e+00, 1.267523e-01),
    (1.355000e-01, 1.098430e+00, 1.331737e-01),
    (1.457280e-01, 1.091955e+00, 1.419482e-01),
    (1.600000e-01, 1.084926e+00, 1.541725e-01),
    (1.720000e-01, 1.080959e+00, 1.643449e-01),
    (1.844370e-01, 1.079429e+00, 1.745527e-01),
    (2.000000e-01, 1.077963e+00, 1.872720e-01),
    (2.277000e-01, 1.073343e+00, 2.103480e-01),
    (2.510392e-01, 1.069038e+00, 2.300466e-01),
    (2.705304e-01, 1.065791e+00, 2.465536e-01),
    (2.907501e-01, 1.062758e+00, 2.637153e-01),
    (3.011332e-01, 1.061306e+00, 2.725409e-01),
    (3.206421e-01, 1.058740e+00, 2.891548e-01),
    (3.576813e-01, 1.054512e+00, 3.207348e-01),
    (3.900000e-01, 1.051478e+00, 3.482836e-01),
    (4.170351e-01, 1.049209e+00, 3.713649e-01),
    (4.500000e-01, 1.046731e+00, 3.995308e-01),
    (5.032575e-01, 1.043303e+00, 4.450625e-01),
    (5.600000e-01, 1.040274e+00, 4.935939e-01),
    (6.250000e-01, 1.037397e+00, 5.492351e-01),
    (7.000000e-01, 1.034678e+00, 6.134435e-01),
    (7.800000e-01, 1.032304e+00, 6.819327e-01),
    (8.600000e-01, 1.030325e+00, 7.504703e-01),
    (9.500000e-01, 1.028473e+00, 8.275730e-01),
    (1.050000e+00, 1.026762e+00, 9.132507e-01),
    (1.160000e+00, 1.025196e+00, 1.007537e+00),
    (1.280000e+00, 1.023791e+00, 1.110296e+00),
    (1.420000e+00, 1.022432e+00, 1.230223e+00),
    (1.550000e+00, 1.021378e+00, 1.341592e+00),
    (1.700000e+00, 1.020354e+00, 1.470129e+00),
    (1.855000e+00, 1.019462e+00, 1.602974e+00),
    (2.020000e+00, 1.018660e+00, 1.744374e+00),
    (2.180000e+00, 1.017993e+00, 1.881546e+00),
    (2.360000e+00, 1.017344e+00, 2.035786e+00),
    (2.590000e+00, 1.016645e+00, 2.232789e+00),
    (2.855000e+00, 1.015979e+00, 2.460008e+00),
    (3.120000e+00, 1.015421e+00, 2.687171e+00),
    (3.420000e+00, 1.014891e+00, 2.944334e+00),
    (3.750000e+00, 1.014406e+00, 3.227243e+00),
    (4.070000e+00, 1.014012e+00, 3.501630e+00),
];

/// Tolerance on the kernel shape `<E'^2>/<E'>^2`, relative. The ported
/// `calcem` reproduces NJOY to the printed precision of the matrix itself.
const SHAPE_TOL: f64 = 5.0e-6;

#[test]
fn calcem_emission_kernel_reproduces_njoy2016_shape() {
    let Some(path) = reference_endf_or_skip("tsl-crystalline-graphite.endf", "kernel-golden")
    else {
        return;
    };
    let tape = Tape::read_file(&path).expect("graphite TSL tape");
    let mat = *tape.materials().first().expect("a material");
    let ii = parse_mf7_at_temperature(&tape, mat, Some(600.0))
        .expect("MF=7 parse")
        .incoherent_inelastic
        .expect("graphite has incoherent inelastic");
    let table = compute_iform0(&ii, 1.0, 8, 4.0, 0.05).expect("calcem iform=0");
    assert_eq!(table.records.len(), NJOY_KERNEL_600K.len());

    let mut worst = (0.0f64, 0.0f64);
    for (r, &(e_n, shape_n, _m1)) in table.records.iter().zip(NJOY_KERNEL_600K) {
        if shape_n <= 0.0 {
            continue;
        }
        assert!(
            (r.e_in_ev - e_n).abs() / e_n < 1.0e-6,
            "grid diverged: ours {:e}, NJOY {:e}",
            r.e_in_ev,
            e_n
        );
        let (mut n, mut m1, mut m2) = (0.0, 0.0, 0.0);
        for w in r.rows.windows(2) {
            let (x0, y0) = (w[0].ep_ev, w[0].pdf);
            let (x1, y1) = (w[1].ep_ev, w[1].pdf);
            let dx = x1 - x0;
            n += 0.5 * (y0 + y1) * dx;
            m1 += 0.5 * (x1 * y1 + x0 * y0) * dx;
            m2 += 0.5 * (x1 * x1 * y1 + x0 * x0 * y0) * dx;
        }
        if n <= 0.0 || m1 <= 0.0 {
            continue;
        }
        let shape = (m2 / n) / (m1 / n).powi(2);
        let rel = (shape - shape_n) / shape_n;
        if rel.abs() > worst.0.abs() {
            worst = (rel, e_n);
        }
    }
    println!(
        "  calcem kernel shape vs NJOY2016: worst {:+.3e} relative at {:.4e} eV",
        worst.0, worst.1
    );
    assert!(
        worst.0.abs() < SHAPE_TOL,
        "the ported calcem's emission kernel is {:+.3e} from NJOY2016's own MF=6 \
         matrix at {:.4e} eV. Recorded 2026-09-13: exact to the matrix's printed \
         precision. This is the oracle GitHub #188 was waiting on.",
        worst.0,
        worst.1
    );
}

/// The *other* half of the measurement: how far the home-grown emission path —
/// the one `outram-mc-libs` actually consumes — sits from NJOY.
///
/// This is deliberately a **characterisation**, not a pass/fail bar on being
/// small: it records a known defect so that fixing it shows up as a failure
/// here, and so that nobody has to re-derive the number. Recorded −1.584 % worst
/// at 0.0228 eV on 2026-09-13.
#[test]
fn equiprobable_emission_is_the_narrow_one() {
    let Some(path) = reference_endf_or_skip("tsl-crystalline-graphite.endf", "kernel-old-path")
    else {
        return;
    };
    let tape = Tape::read_file(&path).expect("graphite TSL tape");
    let mat = *tape.materials().first().expect("a material");
    let ii = parse_mf7_at_temperature(&tape, mat, Some(600.0))
        .expect("MF=7 parse")
        .incoherent_inelastic
        .expect("graphite has incoherent inelastic");

    let mut worst = (0.0f64, 0.0f64);
    for &(e_n, shape_n, _m1) in NJOY_KERNEL_600K {
        if shape_n <= 0.0 {
            continue;
        }
        let bins = ii.equiprobable_emission(e_n, 600.0, 1.0, 64, 8);
        if bins.is_empty() {
            continue;
        }
        let nb = bins.len() as f64;
        let s1: f64 = bins.iter().map(|b| b.e_out_ev).sum();
        let s2: f64 = bins.iter().map(|b| b.e_out_ev * b.e_out_ev).sum();
        if s1 <= 0.0 {
            continue;
        }
        let shape = (s2 / nb) / (s1 / nb).powi(2);
        let rel = (shape - shape_n) / shape_n;
        if rel.abs() > worst.0.abs() {
            worst = (rel, e_n);
        }
    }
    println!(
        "  equiprobable_emission vs NJOY2016: worst {:+.3} % at {:.4e} eV",
        100.0 * worst.0,
        worst.1
    );
    assert!(
        worst.0 < 0.0,
        "the home-grown emission path is recorded as one-signed NARROW against \
         NJOY (worst -1.584 % on 2026-09-13). A positive worst deviation means \
         the defect changed character and this characterisation is stale."
    );
    assert!(
        worst.0.abs() < 0.025,
        "the home-grown emission path is now {:+.2} % from NJOY at {:.4e} eV, \
         worse than the -1.584 % recorded on 2026-09-13 (GitHub #188)",
        100.0 * worst.0,
        worst.1
    );
}
