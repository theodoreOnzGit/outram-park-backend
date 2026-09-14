//! **V&V — the ported THERMR `calcem`/`sigl` against NJOY2016 itself.**
//!
//! # This is a cross-code golden, not an internal consistency check
//!
//! GitHub #188 has been recorded for days as *blocked on NJOY's own emission
//! matrix*. It was not: NJOY2016 is built in this environment
//! (`njoy 2016.79 18Mar25`), so the reference can simply be produced. This test
//! is that reference, frozen.
//!
//! # Provenance (DATA_POLICY.md)
//!
//! - **Code:** NJOY2016, version `2016.79 18Mar25`, run 2026-09-13.
//! - **Data:** ENDF/B-VIII.0 `tsl-crystalline-graphite.endf` (MAT 30) as the
//!   thermal scattering law, and `n-006_C_012-ENDF8.0.endf` (MAT 625) as the
//!   base evaluation. Both are open, publicly distributed evaluations already
//!   catalogued in `reference-data/endf/README.md`.
//! - **Deck** (verbatim; `iprint = 2` is what makes THERMR dump the table):
//!
//! ```text
//!   reconr
//!   21 22
//!   'c12 pendf'/
//!   625 0/
//!   .001/
//!   0/
//!   broadr
//!   21 22 23
//!   625 1/
//!   .001/
//!   600./
//!   0/
//!   thermr
//!   20 23 24
//!   30 625 8 1 2 1 0 1 229 2
//!   600.
//!   .05 4.0
//!   stop
//! ```
//!
//!   Card 2 is `matde matdp nbin ntemp iinc icoh iform natom mtref iprint`, so:
//!   graphite TSL against C-12, **8 equally-probable cosine bins**, one
//!   temperature, `iinc = 2` (tabulated `S(α,β)`), `icoh = 1`, **`iform = 0`**,
//!   one principal atom, MT=229, full print. Card 4 is `tol = 0.05`,
//!   `emax = 4.0 eV`.
//!
//! # What it pins, and why that is the whole chain
//!
//! THERMR's `iprint = 2` prints, per incident energy, the inelastic cross
//! section and the first three Legendre moments of the angular distribution.
//! Reproducing those requires every ported piece to be right at once:
//! [`double_differential`] (`sig`), [`sigl`]'s adaptive angular linearisation
//! **and** its `fract`/`gral`/`disc`/`xbar` equally-probable-cosine solve, and
//! `calcem`'s signed-β walk with its adaptive `E'` panels and moment
//! accumulation. A defect anywhere shows up here.
//!
//! # Result (2026-09-13)
//!
//! Across **all 107 incident-energy points** of the reconstructed grid:
//!
//! ```text
//!   cross section   worst -0.0000 %      at 2.5300e-2 eV
//!   mubar           worst -0.000005      at 1.1500e-2 eV
//! ```
//!
//! i.e. agreement to NJOY's printed precision — seven significant figures,
//! which is this crate's declared maturity bar. The grid itself also matches
//! point for point (the energies are asserted to 1e-6 relative before any
//! quantity is compared).

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;
use njoy_outram_park_fork::thermr::calcem::iform0::compute_iform0;
use njoy_outram_park_fork::thermr::mf7::parse_mf7_at_temperature;

/// NJOY2016's own `(E [eV], sigma_inel [barn], mubar, p2, p3)` for graphite at
/// 600 K, from the deck in this file's header. Transcribed from THERMR's
/// `iprint = 2` output exactly as printed.
#[rustfmt::skip]
const NJOY_GRAPHITE_600K: &[(f64, f64, f64, f64, f64)] = &[
    (1.000000e-05, 1.048821e+01, -1.0281e-02, -7.8224e-03, -3.9192e-04),
    (1.780000e-05, 7.866135e+00, -1.3714e-02, -7.8302e-03, -5.2339e-04),
    (2.500000e-05, 6.641263e+00, -1.6251e-02, -7.8374e-03, -6.2082e-04),
    (3.500000e-05, 5.617371e+00, -1.9224e-02, -7.8474e-03, -7.3522e-04),
    (5.000000e-05, 4.705459e+00, -2.2969e-02, -7.8624e-03, -8.7984e-04),
    (7.000000e-05, 3.983199e+00, -2.7167e-02, -7.8825e-03, -1.0428e-03),
    (1.000000e-04, 3.340588e+00, -3.2450e-02, -7.9125e-03, -1.2491e-03),
    (1.260000e-04, 2.982234e+00, -3.6406e-02, -7.9385e-03, -1.4048e-03),
    (1.600000e-04, 2.653699e+00, -4.0995e-02, -7.9725e-03, -1.5868e-03),
    (2.000000e-04, 2.381177e+00, -4.5796e-02, -8.0124e-03, -1.7785e-03),
    (2.530000e-04, 2.126253e+00, -5.1438e-02, -8.0650e-03, -2.0060e-03),
    (2.970000e-04, 1.969403e+00, -5.5681e-02, -8.1089e-03, -2.1785e-03),
    (3.500000e-04, 1.821946e+00, -6.0378e-02, -8.1614e-03, -2.3708e-03),
    (4.200000e-04, 1.672646e+00, -6.6044e-02, -8.2308e-03, -2.6045e-03),
    (5.060000e-04, 1.534565e+00, -7.2360e-02, -8.3157e-03, -2.8674e-03),
    (6.150000e-04, 1.404373e+00, -7.9592e-02, -8.4228e-03, -3.1711e-03),
    (7.500000e-04, 1.285864e+00, -8.7646e-02, -8.5547e-03, -3.5120e-03),
    (8.700000e-04, 1.205760e+00, -9.4159e-02, -8.6708e-03, -3.7896e-03),
    (1.012000e-03, 1.131200e+00, -1.0125e-01, -8.8077e-03, -4.0934e-03),
    (1.230000e-03, 1.044901e+00, -1.1111e-01, -9.0159e-03, -4.5170e-03),
    (1.500000e-03, 9.678645e-01, -1.2196e-01, -9.2710e-03, -4.9851e-03),
    (1.800000e-03, 9.062482e-01, -1.3272e-01, -9.5504e-03, -5.4471e-03),
    (2.030000e-03, 8.702277e-01, -1.4022e-01, -9.7617e-03, -5.7658e-03),
    (2.277000e-03, 8.393783e-01, -1.4772e-01, -9.9875e-03, -6.0801e-03),
    (2.600000e-03, 8.075426e-01, -1.5665e-01, -1.0279e-02, -6.4525e-03),
    (3.000000e-03, 7.782067e-01, -1.6668e-01, -1.0633e-02, -6.8616e-03),
    (3.500000e-03, 7.521140e-01, -1.7781e-01, -1.1067e-02, -7.3019e-03),
    (4.048000e-03, 7.329002e-01, -1.8854e-01, -1.1538e-02, -7.7146e-03),
    (4.500000e-03, 7.223519e-01, -1.9643e-01, -1.1919e-02, -8.0073e-03),
    (5.000000e-03, 7.147774e-01, -2.0428e-01, -1.2336e-02, -8.2877e-03),
    (5.600000e-03, 7.100414e-01, -2.1267e-01, -1.2827e-02, -8.5708e-03),
    (6.325000e-03, 7.090464e-01, -2.2151e-01, -1.3412e-02, -8.8565e-03),
    (7.200000e-03, 7.129011e-01, -2.3060e-01, -1.4108e-02, -9.1291e-03),
    (8.100000e-03, 7.210865e-01, -2.3846e-01, -1.4807e-02, -9.3412e-03),
    (9.108000e-03, 7.338993e-01, -2.4579e-01, -1.5581e-02, -9.5232e-03),
    (1.000000e-02, 7.476522e-01, -2.5121e-01, -1.6263e-02, -9.6492e-03),
    (1.063000e-02, 7.584213e-01, -2.5454e-01, -1.6741e-02, -9.7188e-03),
    (1.150000e-02, 7.744867e-01, -2.5854e-01, -1.7390e-02, -9.7895e-03),
    (1.239700e-02, 7.922077e-01, -2.6204e-01, -1.8061e-02, -9.8475e-03),
    (1.330000e-02, 8.109800e-01, -2.6501e-01, -1.8729e-02, -9.8944e-03),
    (1.417000e-02, 8.299274e-01, -2.6741e-01, -1.9363e-02, -9.9262e-03),
    (1.500000e-02, 8.485696e-01, -2.6936e-01, -1.9963e-02, -9.9418e-03),
    (1.619200e-02, 8.761917e-01, -2.7164e-01, -2.0820e-02, -9.9614e-03),
    (1.820000e-02, 9.246847e-01, -2.7431e-01, -2.2238e-02, -9.9765e-03),
    (1.990000e-02, 9.675677e-01, -2.7567e-01, -2.3414e-02, -9.9778e-03),
    (2.049300e-02, 9.829406e-01, -2.7597e-01, -2.3819e-02, -9.9779e-03),
    (2.150000e-02, 1.009244e+00, -2.7637e-01, -2.4500e-02, -9.9798e-03),
    (2.280000e-02, 1.043318e+00, -2.7670e-01, -2.5370e-02, -9.9791e-03),
    (2.530000e-02, 1.108784e+00, -2.7677e-01, -2.7034e-02, -9.9858e-03),
    (2.800000e-02, 1.179967e+00, -2.7620e-01, -2.8826e-02, -9.9954e-03),
    (3.061300e-02, 1.243885e+00, -2.7517e-01, -3.0469e-02, -1.0024e-02),
    (3.380000e-02, 1.323947e+00, -2.7342e-01, -3.2475e-02, -1.0054e-02),
    (3.650000e-02, 1.390335e+00, -2.7151e-01, -3.4142e-02, -1.0117e-02),
    (3.950000e-02, 1.462371e+00, -2.6922e-01, -3.5936e-02, -1.0172e-02),
    (4.275700e-02, 1.540753e+00, -2.6629e-01, -3.7945e-02, -1.0338e-02),
    (4.650000e-02, 1.623947e+00, -2.6286e-01, -4.0134e-02, -1.0535e-02),
    (5.000000e-02, 1.701033e+00, -2.5944e-01, -4.2135e-02, -1.0750e-02),
    (5.692500e-02, 1.846457e+00, -2.5254e-01, -4.5825e-02, -1.1158e-02),
    (6.250000e-02, 1.958216e+00, -2.4683e-01, -4.8580e-02, -1.1503e-02),
    (6.900000e-02, 2.078822e+00, -2.4035e-01, -5.1689e-02, -1.1981e-02),
    (7.500000e-02, 2.182864e+00, -2.3436e-01, -5.4380e-02, -1.2484e-02),
    (8.197200e-02, 2.300029e+00, -2.2726e-01, -5.7211e-02, -1.3070e-02),
    (9.000000e-02, 2.426892e+00, -2.1929e-01, -6.0179e-02, -1.3799e-02),
    (9.600000e-02, 2.515413e+00, -2.1357e-01, -6.2262e-02, -1.4372e-02),
    (1.035000e-01, 2.619589e+00, -2.0664e-01, -6.4673e-02, -1.5100e-02),
    (1.115730e-01, 2.723964e+00, -1.9943e-01, -6.7038e-02, -1.5899e-02),
    (1.200000e-01, 2.822004e+00, -1.9229e-01, -6.9335e-02, -1.6756e-02),
    (1.280000e-01, 2.906992e+00, -1.8573e-01, -7.1355e-02, -1.7610e-02),
    (1.355000e-01, 2.980153e+00, -1.7978e-01, -7.3046e-02, -1.8379e-02),
    (1.457280e-01, 3.071747e+00, -1.7193e-01, -7.5138e-02, -1.9456e-02),
    (1.600000e-01, 3.190155e+00, -1.6131e-01, -7.7618e-02, -2.0936e-02),
    (1.720000e-01, 3.276619e+00, -1.5317e-01, -7.9233e-02, -2.2111e-02),
    (1.844370e-01, 3.365041e+00, -1.4502e-01, -8.0489e-02, -2.3269e-02),
    (2.000000e-01, 3.465158e+00, -1.3574e-01, -8.1667e-02, -2.4601e-02),
    (2.277000e-01, 3.613961e+00, -1.2083e-01, -8.3201e-02, -2.6865e-02),
    (2.510392e-01, 3.714292e+00, -1.0987e-01, -8.3810e-02, -2.8561e-02),
    (2.705304e-01, 3.784989e+00, -1.0150e-01, -8.3902e-02, -2.9876e-02),
    (2.907501e-01, 3.848776e+00, -9.3729e-02, -8.3658e-02, -3.0958e-02),
    (3.011332e-01, 3.878302e+00, -8.9912e-02, -8.3485e-02, -3.1523e-02),
    (3.206421e-01, 3.928814e+00, -8.3289e-02, -8.2935e-02, -3.2396e-02),
    (3.576813e-01, 4.010357e+00, -7.2005e-02, -8.1553e-02, -3.3708e-02),
    (3.900000e-01, 4.068562e+00, -6.3534e-02, -7.9955e-02, -3.4464e-02),
    (4.170351e-01, 4.111406e+00, -5.7145e-02, -7.8573e-02, -3.4936e-02),
    (4.500000e-01, 4.156657e+00, -5.0161e-02, -7.6806e-02, -3.5318e-02),
    (5.032575e-01, 4.217987e+00, -4.0358e-02, -7.3917e-02, -3.5577e-02),
    (5.600000e-01, 4.269724e+00, -3.1807e-02, -7.0850e-02, -3.5251e-02),
    (6.250000e-01, 4.317398e+00, -2.3649e-02, -6.7450e-02, -3.4509e-02),
    (7.000000e-01, 4.360319e+00, -1.5948e-02, -6.3726e-02, -3.3291e-02),
    (7.800000e-01, 4.397931e+00, -9.2334e-03, -6.0274e-02, -3.1758e-02),
    (8.600000e-01, 4.429187e+00, -3.5018e-03, -5.7144e-02, -3.0210e-02),
    (9.500000e-01, 4.457983e+00, 1.8576e-03, -5.4003e-02, -2.8379e-02),
    (1.050000e+00, 4.483062e+00, 6.7402e-03, -5.0934e-02, -2.6405e-02),
    (1.160000e+00, 4.506184e+00, 1.1304e-02, -4.7996e-02, -2.4366e-02),
    (1.280000e+00, 4.527805e+00, 1.5066e-02, -4.5092e-02, -2.2223e-02),
    (1.420000e+00, 4.548216e+00, 1.8808e-02, -4.2232e-02, -2.0026e-02),
    (1.550000e+00, 4.563613e+00, 2.1675e-02, -4.0006e-02, -1.8225e-02),
    (1.700000e+00, 4.578819e+00, 2.4602e-02, -3.7782e-02, -1.6402e-02),
    (1.855000e+00, 4.591754e+00, 2.7147e-02, -3.5764e-02, -1.4802e-02),
    (2.020000e+00, 4.602898e+00, 2.9432e-02, -3.3939e-02, -1.3394e-02),
    (2.180000e+00, 4.612579e+00, 3.1418e-02, -3.2378e-02, -1.2305e-02),
    (2.360000e+00, 4.621659e+00, 3.3230e-02, -3.0890e-02, -1.1262e-02),
    (2.590000e+00, 4.629483e+00, 3.5021e-02, -2.9195e-02, -1.0175e-02),
    (2.855000e+00, 4.640027e+00, 3.7184e-02, -2.7165e-02, -9.2090e-03),
    (3.120000e+00, 4.646837e+00, 3.8848e-02, -2.5575e-02, -8.5524e-03),
    (3.420000e+00, 4.654471e+00, 4.0476e-02, -2.4072e-02, -7.9786e-03),
    (3.750000e+00, 4.661005e+00, 4.2047e-02, -2.2508e-02, -7.5534e-03),
    (4.070000e+00, 4.664845e+00, 4.3463e-02, -2.1412e-02, -6.9970e-03),];

/// Tolerance on the cross section, relative. NJOY prints 7 significant figures,
/// so 1e-6 is the printed precision itself — the measured worst is below it.
const XS_TOL: f64 = 1.0e-6;
/// Tolerance on `mubar`, absolute. NJOY prints it to 5 significant figures;
/// measured worst deviation is 5e-6.
const MUBAR_TOL: f64 = 2.0e-5;

#[test]
fn calcem_iform0_reproduces_njoy2016_for_graphite_at_600k() {
    let Some(path) = reference_endf_or_skip("tsl-crystalline-graphite.endf", "calcem-golden")
    else {
        return;
    };
    let tape = Tape::read_file(&path).expect("graphite TSL tape");
    let mat = *tape.materials().first().expect("a material");
    let ii = parse_mf7_at_temperature(&tape, mat, Some(600.0))
        .expect("MF=7 parse")
        .incoherent_inelastic
        .expect("graphite has incoherent inelastic");

    // The same card values the deck used: natom = 1, nbin = 8, emax = 4.0 eV,
    // tol = 0.05.
    let table = compute_iform0(&ii, 1.0, 8, 4.0, 0.05).expect("calcem iform=0");

    assert_eq!(
        table.records.len(),
        NJOY_GRAPHITE_600K.len(),
        "grid length differs from NJOY's: {} against {}",
        table.records.len(),
        NJOY_GRAPHITE_600K.len()
    );

    let (mut worst_xs, mut worst_ub) = ((0.0f64, 0.0f64), (0.0f64, 0.0f64));
    for (r, &(e_n, xs_n, ub_n, _p2, _p3)) in table.records.iter().zip(NJOY_GRAPHITE_600K) {
        assert!(
            (r.e_in_ev - e_n).abs() / e_n < 1.0e-6,
            "incident grid diverged: ours {:e}, NJOY {:e}",
            r.e_in_ev,
            e_n
        );
        let rx = (r.cross_section_b - xs_n) / xs_n;
        if rx.abs() > worst_xs.0.abs() {
            worst_xs = (rx, e_n);
        }
        let du = r.mubar - ub_n;
        if du.abs() > worst_ub.0.abs() {
            worst_ub = (du, e_n);
        }
    }
    println!(
        "  vs NJOY2016: worst xs {:+.4e} relative at {:.4e} eV; worst mubar {:+.2e} at {:.4e} eV",
        worst_xs.0, worst_xs.1, worst_ub.0, worst_ub.1
    );
    assert!(
        worst_xs.0.abs() < XS_TOL,
        "sigma_inel is {:+.3e} relative from NJOY2016 at {:.4e} eV (recorded \
         -0.0000 % worst on 2026-09-13). This pins sig + sigl + calcem's panel \
         walk together, so a failure localises to whichever of them changed.",
        worst_xs.0,
        worst_xs.1
    );
    assert!(
        worst_ub.0.abs() < MUBAR_TOL,
        "mubar is {:+.3e} from NJOY2016 at {:.4e} eV (recorded -5e-6 worst). \
         mubar comes from the equally-probable cosines, so this is the check on \
         sigl's fract/gral/disc/xbar bin solve specifically.",
        worst_ub.0,
        worst_ub.1
    );
}
