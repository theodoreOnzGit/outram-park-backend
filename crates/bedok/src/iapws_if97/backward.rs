//! Backward equations — temperature from pressure and enthalpy.
//!
//! # Provenance — third-party, BSD-2-Clause
//!
//! As [`crate::iapws_if97`]: translated from `IAPWS_IF97.m` by Mark Mikofski,
//! Copyright (c) 2013, BSD-2-Clause, terms reproduced in the crate `NOTICE`.
//! Source functions `T_ph`, `T1_ph`, `T2a_ph`, `T2b_ph`, `T2c_ph`, `h2bc_p`,
//! `TB23_p`.
//!
//! # What a backward equation is, and why it exists
//!
//! The IF97 region equations are explicit in `(p, T)`. Thermal-hydraulics
//! marches **enthalpy**, so it needs the inverse, `T(p, h)` — and inverting the
//! forward equation numerically at every node of every iteration is far too
//! slow. The formulation therefore publishes separate fitted polynomials for
//! the inverse, accurate to within the forward equation's own tolerance but
//! evaluable in one pass.
//!
//! The cost is that the `(p, h)` plane has to be **subdivided** more finely
//! than the `(p, T)` plane: region 2 alone needs three sub-regions, 2a, 2b and
//! 2c, with boundaries of their own. Most of `T_ph` is deciding which
//! polynomial applies.
//!
//! # Units
//!
//! Pressure MPa, enthalpy kJ/kg, temperature K.

use super::basic::{h1_pt, h2_pt};
use super::region4::{p_b13_sat, psat_t, tsat_p, T_MIN};

/// `h = h2bc_p(p)` — the enthalpy on the boundary between sub-regions 2b and
/// 2c, kJ/kg, from pressure in MPa.
///
/// The 2b/2c divide follows the 5.85 kJ/(kg·K) isentrope, which the formulation
/// fits as a square root in pressure.
pub fn h2bc_p(p: f64) -> f64 {
    const N: [f64; 5] = [
        0.905_842_785_147_23e3,
        -0.679_557_863_992_41,
        0.128_090_027_301_36e-3,
        0.265_265_719_084_28e4,
        0.452_575_789_059_48e1,
    ];
    N[3] + ((p - N[4]) / N[2]).sqrt()
}

/// `T = TB23_p(p)` — the temperature on the region 2 / region 3 boundary, K,
/// from pressure in MPa.
pub fn tb23_p(p: f64) -> f64 {
    const N: [f64; 5] = [
        0.348_051_856_289_69e3,
        -0.116_718_598_799_75e1,
        0.101_929_700_393_26e-2,
        0.572_544_598_627_46e3,
        0.139_188_397_788_70e2,
    ];
    N[3] + ((p - N[4]) / N[2]).sqrt()
}

/// `T = T1_ph(p, h)` — region 1 (compressed liquid), K.
///
/// Twenty terms in `pi^I * (eta + 1)^J` with `eta = h / 2500`. **No range
/// check**, matching the reference: outside region 1 this is an extrapolation.
/// [`t_ph`] does the region selection.
pub fn t1_ph(p: f64, h: f64) -> f64 {
    const I: [i32; 20] = [0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 2, 2, 3, 3, 4, 5, 6];
    const J: [i32; 20] = [
        0, 1, 2, 6, 22, 32, 0, 1, 2, 3, 4, 10, 32, 10, 32, 10, 32, 32, 32, 32,
    ];
    const N: [f64; 20] = [
        -238.724_899_245_210,
        404.211_886_379_450,
        113.497_468_817_180,
        -5.845_761_604_803_90,
        -0.000_152_854_824_131_400,
        -1.086_670_769_537_70e-06,
        -13.391_744_872_602_0,
        43.211_039_183_559_0,
        -54.010_067_170_506_0,
        30.535_892_203_916_0,
        -6.596_474_942_363_80,
        0.009_396_540_087_836_30,
        1.157_364_750_534_00e-07,
        -2.585_864_128_207_30e-05,
        -4.064_436_308_479_90e-09,
        6.645_618_619_163_50e-08,
        8.067_073_410_302_70e-11,
        -9.347_777_121_394_70e-13,
        5.826_544_202_060_10e-15,
        -1.502_018_595_350_30e-17,
    ];
    let eta = h / 2500.0;
    (0..20)
        .map(|k| N[k] * p.powi(I[k]) * (eta + 1.0).powi(J[k]))
        .sum()
}

/// `T = T2a_ph(p, h)` — region 2a (superheated vapour, p <= 4 MPa), K.
pub fn t2a_ph(p: f64, h: f64) -> f64 {
    const I: [i32; 34] = [
        0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 3, 3, 4, 4, 4, 5, 5,
        5, 6, 6, 7,
    ];
    const J: [i32; 34] = [
        0, 1, 2, 3, 7, 20, 0, 1, 2, 3, 7, 9, 11, 18, 44, 0, 2, 7, 36, 38, 40, 42, 44, 24, 44, 12,
        32, 44, 32, 36, 42, 34, 44, 28,
    ];
    const N: [f64; 34] = [
        1_089.895_231_828_80,
        849.516_544_955_350,
        -107.817_480_918_260,
        33.153_654_801_263_0,
        -7.423_201_679_024_80,
        11.765_048_724_356_0,
        1.844_574_935_579_00,
        -4.179_270_054_962_40,
        6.247_819_693_581_20,
        -17.344_563_108_114_0,
        -200.581_768_620_960,
        271.960_654_737_960,
        -455.113_182_858_180,
        3_091.968_860_475_50,
        252_266.403_578_720,
        -0.006_170_742_286_833_90,
        -0.310_780_466_295_830,
        11.670_873_077_107_0,
        128_127_984.040_460,
        -985_549_096.232_760,
        2_822_454_697.300_20,
        -3_594_897_141.070_30,
        1_722_734_991.319_70,
        -13_551.334_240_775_0,
        12_848_734.664_650_0,
        1.386_572_428_322_60,
        235_988.325_565_140,
        -13_105_236.545_054_0,
        7_399.983_547_476_60,
        -551_966.970_300_600,
        3_715_408.599_623_30,
        19_127.729_239_660_0,
        -415_351.648_356_340,
        -62.459_855_192_507_0,
    ];
    let eta = h / 2000.0;
    (0..34)
        .map(|k| N[k] * p.powi(I[k]) * (eta - 2.1).powi(J[k]))
        .sum()
}

/// `T = T2b_ph(p, h)` — region 2b (superheated vapour, the middle band), K.
pub fn t2b_ph(p: f64, h: f64) -> f64 {
    const I: [i32; 38] = [
        0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 4, 4,
        5, 5, 5, 6, 7, 7, 9, 9,
    ];
    const J: [i32; 38] = [
        0, 1, 2, 12, 18, 24, 28, 40, 0, 2, 6, 12, 18, 24, 28, 40, 2, 8, 18, 40, 1, 2, 12, 24, 2,
        12, 18, 24, 28, 40, 18, 24, 40, 28, 2, 28, 1, 40,
    ];
    const N: [f64; 38] = [
        1_489.504_107_951_60,
        743.077_983_140_340,
        -97.708_318_797_837_0,
        2.474_246_470_566_74,
        -0.632_813_200_160_260,
        1.138_595_212_965_80,
        -0.478_118_636_486_250,
        0.008_520_812_343_154_40,
        0.937_471_473_779_320,
        3.359_311_860_491_60,
        3.380_935_560_145_40,
        0.168_445_396_719_040,
        0.738_757_452_366_950,
        -0.471_287_374_361_860,
        0.150_202_731_397_070,
        -0.002_176_411_421_975_00,
        -0.021_810_755_324_761_0,
        -0.108_297_844_036_770,
        -0.046_333_324_635_812_0,
        7.128_035_195_955_10e-05,
        0.000_110_328_317_899_990,
        0.000_189_552_483_879_020,
        0.003_089_154_116_053_70,
        0.001_355_550_455_494_90,
        2.864_023_747_745_60e-07,
        -1.077_985_735_751_20e-05,
        -7.646_271_245_481_40e-05,
        1.405_239_281_831_60e-05,
        -3.108_381_433_134_40e-05,
        -1.030_273_821_210_30e-06,
        2.821_728_163_504_00e-07,
        1.270_490_227_194_50e-06,
        7.380_335_346_829_20e-08,
        -1.103_013_923_890_90e-08,
        -8.145_636_520_783_30e-14,
        -2.518_054_568_296_20e-11,
        -1.756_523_396_940_70e-18,
        8.693_415_634_416_30e-15,
    ];
    let eta = h / 2000.0;
    (0..38)
        .map(|k| N[k] * (p - 2.0).powi(I[k]) * (eta - 2.6).powi(J[k]))
        .sum()
}

/// `T = T2c_ph(p, h)` — region 2c (superheated vapour, high pressure), K.
///
/// Note the **negative** `I` exponents on `(pi + 25)`, which is why the
/// exponent arrays here are signed.
pub fn t2c_ph(p: f64, h: f64) -> f64 {
    const I: [i32; 23] = [
        -7, -7, -6, -6, -5, -5, -2, -2, -1, -1, 0, 0, 1, 1, 2, 6, 6, 6, 6, 6, 6, 6, 6,
    ];
    const J: [i32; 23] = [
        0, 4, 0, 2, 0, 2, 0, 1, 0, 2, 0, 1, 4, 8, 4, 0, 1, 4, 10, 12, 16, 20, 22,
    ];
    const N: [f64; 23] = [
        -3_236_839_855_524.20,
        7_326_335_090_218.10,
        358_250_899_454.470,
        -583_401_318_515.900,
        -10_783_068_217.470_0,
        20_825_544_563.171_0,
        610_747.835_645_160,
        859_777.225_355_800,
        -25_745.723_604_170_0,
        31_081.088_422_714_0,
        1_208.231_586_593_60,
        482.197_551_092_550,
        3.796_600_127_248_60,
        -10.842_984_880_077_0,
        -0.045_364_172_676_660_0,
        1.455_911_565_869_80e-13,
        1.126_159_740_723_00e-12,
        -1.780_498_224_068_60e-11,
        1.232_457_969_083_20e-07,
        -1.160_692_113_098_40e-06,
        2.784_636_708_855_40e-05,
        -0.000_592_700_384_741_760,
        0.001_291_858_299_187_80,
    ];
    let eta = h / 2000.0;
    (0..23)
        .map(|k| N[k] * (p + 25.0).powi(I[k]) * (eta - 1.8).powi(J[k]))
        .sum()
}

/// `T = T_ph(p, h)` — temperature of liquid, vapour or mixture, K.
///
/// # Arguments
///
/// - `p` — pressure, **MPa**.
/// - `h` — specific enthalpy, **kJ/kg**.
///
/// # Returns
///
/// Temperature in **K**, or `NaN` where this translation does not cover the
/// state — see the range note below.
///
/// # How the region is chosen
///
/// The `(p, h)` plane is divided by comparing `h` against boundary enthalpies
/// computed at the given pressure:
///
/// | Condition | Region | Equation |
/// |---|---|---|
/// | `h <= h1(p, Tsat)` | 1, compressed liquid | [`t1_ph`] |
/// | `h1(p, Tsat) < h <= h2(p, Tsat)` | 4, two-phase | `Tsat(p)` |
/// | `h > h2(p, Tsat)`, `p <= 4 MPa` | 2a | [`t2a_ph`] |
/// | `h > h2(p, Tsat)`, `4 < p <= p2bc,sat`, or `h > h2bc(p)` above it | 2b | [`t2b_ph`] |
/// | `h > h2(p, Tsat)`, `p > p2bc,sat`, `h <= h2bc(p)` | 2c | [`t2c_ph`] |
///
/// **In the two-phase region the answer is `Tsat(p)`**, which is exact but
/// carries no information about quality — that is what `h` is for, and
/// `singleflow1devap.m` recovers it separately.
///
/// # Range — capped at the region 1/3 boundary
///
/// This returns `NaN` for `p > 16.5292 MPa`, the same region-3 gap
/// [`super::basic::hl_p`] documents. Above that pressure the liquid branch, the
/// saturation line and part of the vapour branch all need region 3, which is
/// not translated. Both BEDOK operating points — a PWR at 15.5 MPa and a BWR at
/// 7 MPa — sit below it.
///
/// Below the triple-point pressure it also returns `NaN`, which is the
/// reference's own behaviour.
pub fn t_ph(p: f64, h: f64) -> f64 {
    let p_min = psat_t(T_MIN);
    let p_b13 = p_b13_sat();
    if !(p_min..=p_b13).contains(&p) {
        return f64::NAN;
    }

    let tsat = tsat_p(p);
    let h1l = h1_pt(p, tsat);
    let h2v = h2_pt(p, tsat);

    if h <= h1l {
        return t1_ph(p, h);
    }
    if h <= h2v {
        // Region 4 — the two-phase plateau.
        return tsat;
    }

    // Region 2, sub-divided.
    // `p2ab = 4 MPa`; `p2bcsat = psat_T(554.485 K)`.
    const P2AB: f64 = 4.0;
    let p2bcsat = psat_t(554.485);

    if p <= P2AB {
        t2a_ph(p, h)
    } else if p <= p2bcsat || h > h2bc_p(p) {
        // 2b covers everything below the 2b/2c isentrope, and everything above
        // it at pressures where 2c does not apply. The reference writes these
        // as two separate conditions producing the same call.
        t2b_ph(p, h)
    } else {
        t2c_ph(p, h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::region4::T_B13;

    /// IAPWS-IF97 Table 7 — the region 1 backward equation `T1_ph`.
    ///
    /// # Methodology
    ///
    /// Three published `(p, h)` states with reference temperatures to 9
    /// significant figures. Pass criterion 1e-8 relative, the bar every other
    /// IF97 test in this crate is held to.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// Relative deviation 6.064e-10, 3.190e-10 and 6.590e-10 at the three
    /// states — agreement at the published table's 9-figure precision.
    #[test]
    fn region1_backward_matches_published_values() {
        // (p [MPa], h [kJ/kg], T [K])
        let cases = [
            (3.0, 500.0, 0.391_798_509e3),
            (80.0, 500.0, 0.378_108_626e3),
            (80.0, 1500.0, 0.611_041_229e3),
        ];
        for (p, h, t_ref) in cases {
            let t = t1_ph(p, h);
            eprintln!(
                "T1_ph({p}, {h}) = {t}, expected {t_ref}, rel_err {:.3e}",
                (t - t_ref).abs() / t_ref
            );
            assert!((t - t_ref).abs() / t_ref < 1e-8, "got {t}, want {t_ref}");
        }
    }

    /// IAPWS-IF97 Table 24 — the region 2 backward equations, all three
    /// sub-regions.
    ///
    /// # Methodology
    ///
    /// Three published states per sub-region, nine in all. Pass criterion 1e-8
    /// relative. Testing the sub-equations directly rather than through
    /// [`t_ph`] is deliberate: several of the published states sit above
    /// 16.53 MPa, where `t_ph` deliberately returns `NaN` because region 3 is
    /// not translated.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// All nine states agree to better than **4.603e-9** relative, the worst
    /// being 2b at (5 MPa, 4000 kJ/kg). Per sub-region: 2a to 1.3e-9, 2b to
    /// 4.6e-9, 2c to 5.9e-10.
    ///
    /// **Interpretation.** Three separate coefficient tables — 34, 38 and 23
    /// terms — each reproduce their published states at the tables' own
    /// precision, so all three are verified against the standard.
    #[test]
    fn region2_backward_matches_published_values() {
        let cases_a = [
            (0.001, 3000.0, 0.534_433_241e3),
            (3.0, 3000.0, 0.575_373_370e3),
            (3.0, 4000.0, 0.101_077_577e4),
        ];
        let cases_b = [
            (5.0, 3500.0, 0.801_299_102e3),
            (5.0, 4000.0, 0.101_531_583e4),
            (25.0, 3500.0, 0.875_279_054e3),
        ];
        let cases_c = [
            (40.0, 2700.0, 0.743_056_411e3),
            (60.0, 2700.0, 0.791_137_067e3),
            (60.0, 3200.0, 0.882_756_860e3),
        ];

        for (name, cases, f) in [
            ("2a", &cases_a, t2a_ph as fn(f64, f64) -> f64),
            ("2b", &cases_b, t2b_ph as fn(f64, f64) -> f64),
            ("2c", &cases_c, t2c_ph as fn(f64, f64) -> f64),
        ] {
            for (p, h, t_ref) in cases {
                let t = f(*p, *h);
                eprintln!(
                    "T{name}_ph({p}, {h}) = {t}, expected {t_ref}, rel_err {:.3e}",
                    (t - t_ref).abs() / t_ref
                );
                assert!(
                    (t - t_ref).abs() / t_ref < 1e-8,
                    "T{name}_ph({p}, {h}): got {t}, want {t_ref}"
                );
            }
        }
    }

    /// The backward equation inverts the forward one — an independent check
    /// that does not use any published value.
    ///
    /// # Methodology
    ///
    /// For a grid of compressed-liquid states, compute `h = h1_pT(p, T)` and
    /// feed it back through `T1_ph(p, h)`. The two are **separate fits**, not
    /// algebraic inverses, so they agree only to the formulation's stated
    /// backward-equation tolerance rather than to machine precision. IAPWS
    /// states that tolerance as 25 mK for `T1_ph`.
    ///
    /// Pass criterion: within 25 mK, the published tolerance.
    ///
    /// This catches a transcription error the published-value test could miss —
    /// three tabulated states leave most of the coefficient table unexercised,
    /// whereas a sweep visits the whole surface.
    ///
    /// **The sweep must stay inside region 1**, and getting that wrong is easy:
    /// the first version of this test swept to 600 K at every pressure and
    /// failed by 1.23 K at 1 MPa — where 600 K is superheated *vapour*, since
    /// `Tsat(1 MPa) = 453 K`. `T1_ph` is not wrong there; it is simply being
    /// asked about a state that is not region 1. The sweep is therefore capped
    /// at `min(Tsat(p), 623.15 K)`, the region's own upper boundary, less a 1 K
    /// margin to stay off the saturation line itself.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// Over **122 region-1 states**, the worst round-trip error was
    /// **0.023178 K**, at 50 MPa and 500 K — just inside the 25 mK tolerance.
    ///
    /// **Interpretation.** Landing at 93% of the published tolerance rather
    /// than orders below it is the expected signature of a correctly
    /// transcribed fit: the backward equation is *designed* to be this
    /// accurate and no more, so agreement much tighter than 25 mK would suggest
    /// the sweep was not reaching the hard parts of the surface. A
    /// mistranscribed coefficient would blow well past it. This exercises far
    /// more of the 20-term table than the three tabulated states do.
    #[test]
    fn the_liquid_backward_equation_inverts_the_forward_one() {
        let mut worst: f64 = 0.0;
        let mut worst_at = (0.0, 0.0);
        let mut samples = 0;
        for &p in &[1.0, 3.0, 7.0, 10.0, 15.5, 20.0, 50.0, 80.0] {
            // Region 1's upper edge: the saturation line below p_b13sat, and
            // the fixed 623.15 K boundary above it.
            let ceiling = if p <= p_b13_sat() {
                tsat_p(p).min(T_B13)
            } else {
                T_B13
            } - 1.0;
            let mut t = 280.0;
            while t <= ceiling {
                let h = h1_pt(p, t);
                let back = t1_ph(p, h);
                let err = (back - t).abs();
                if err > worst {
                    worst = err;
                    worst_at = (p, t);
                }
                samples += 1;
                t += 20.0;
            }
        }
        eprintln!(
            "T1_ph round trip over {samples} states: worst {worst:.6} K at p = {} MPa, T = {} K",
            worst_at.0, worst_at.1
        );
        assert!(samples > 100, "the sweep only visited {samples} states");
        assert!(
            worst < 0.025,
            "worst round-trip error {worst} K exceeds 25 mK"
        );
    }

    /// The same round trip for the vapour side, through the `t_ph` dispatcher.
    ///
    /// # Methodology
    ///
    /// Superheated states at pressures below the region 1/3 boundary, so
    /// [`t_ph`] covers them. Going through the dispatcher rather than the
    /// sub-equations also verifies the **sub-region selection**: picking 2b
    /// where 2c applies would show up as a large error, since the two fits
    /// agree only on their shared boundary.
    ///
    /// Pass criterion: within 25 mK, as above.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// Worst round-trip error **0.014972 K**, at 10 MPa and 604 K — inside the
    /// 25 mK tolerance across all seven pressures.
    ///
    /// **Interpretation.** Because this goes through [`t_ph`], it also
    /// confirms the 2a/2b/2c selection: the three fits agree only on their
    /// shared boundaries, so choosing the wrong one anywhere in the sweep
    /// would produce an error far larger than the fit tolerance.
    #[test]
    fn the_vapour_backward_equation_inverts_through_the_dispatcher() {
        let mut worst: f64 = 0.0;
        let mut worst_at = (0.0, 0.0);
        for &p in &[0.1, 1.0, 3.0, 5.0, 7.0, 10.0, 15.0] {
            let tsat = tsat_p(p);
            let mut t = tsat + 20.0;
            while t <= 1000.0 {
                let h = h2_pt(p, t);
                let back = t_ph(p, h);
                let err = (back - t).abs();
                if err > worst {
                    worst = err;
                    worst_at = (p, t);
                }
                t += 25.0;
            }
        }
        eprintln!(
            "T_ph vapour round trip: worst {worst:.6} K at p = {} MPa, T = {} K",
            worst_at.0, worst_at.1
        );
        assert!(
            worst < 0.025,
            "worst round-trip error {worst} K exceeds 25 mK"
        );
    }

    /// Inside the saturation dome the dispatcher returns `Tsat`, exactly, at
    /// every quality.
    ///
    /// # Methodology
    ///
    /// At 7 MPa (the BWR pressure) the two-phase enthalpy runs from `hL` to
    /// `hV`. Sweeping quality strictly inside that range must give
    /// `Tsat(7 MPa)` exactly — the two-phase plateau.
    ///
    /// **The endpoints are not on the plateau, and that is correct.** The
    /// region test is `h <= h1L` for region 1 and `h > h1L` for region 4, so
    /// `x = 0` lands on the *liquid* branch and is evaluated by `T1_ph` rather
    /// than returned as `Tsat`. The two differ by the backward equation's fit
    /// tolerance — measured at 2.9 mK here — not by a modelling error. The same
    /// applies at `x = 1` on the vapour side. This mirrors the reference's own
    /// inclusive/exclusive boundary convention.
    ///
    /// Pass criterion: exact equality with `tsat_p` for `0 < x < 1`, and
    /// agreement within the 25 mK backward tolerance at the two endpoints.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// At 7 MPa, `Tsat = 558.9800 K`, `hL = 1267.44` and `hV = 2772.57` kJ/kg.
    /// Every quality from 0.1 to 0.9 returned `Tsat` **exactly** — bit-for-bit,
    /// since the branch returns the same value it computed.
    ///
    /// The endpoints behave as the boundary convention predicts: `x = 0` falls
    /// to the region-1 branch and reads **2.879 mK** below `Tsat`, well inside
    /// the fit tolerance; `x = 1` satisfies `h <= h2V` and so stays on the
    /// two-phase branch, giving `Tsat` exactly, difference 0.
    #[test]
    fn the_two_phase_region_returns_the_saturation_temperature() {
        use super::super::basic::{hl_p, hv_p};
        let p = 7.0;
        let tsat = tsat_p(p);
        let (hl, hv) = (hl_p(p), hv_p(p));
        eprintln!("at {p} MPa: Tsat = {tsat} K, hL = {hl}, hV = {hv} kJ/kg");

        for i in 1..10 {
            let x = i as f64 / 10.0;
            let h = hl + x * (hv - hl);
            let t = t_ph(p, h);
            assert_eq!(t, tsat, "quality {x} gave {t}, not Tsat = {tsat}");
        }

        // The endpoints fall to the single-phase branches by the boundary
        // convention, and agree with Tsat to the fit tolerance.
        for (label, h) in [("x = 0", hl), ("x = 1", hv)] {
            let t = t_ph(p, h);
            eprintln!(
                "{label}: t_ph = {t}, Tsat = {tsat}, diff = {}",
                (t - tsat).abs()
            );
            assert!(
                (t - tsat).abs() < 0.025,
                "{label} gave {t}, more than 25 mK from Tsat = {tsat}"
            );
        }
    }

    /// Above the region 1/3 boundary the dispatcher is `NaN`, loudly.
    #[test]
    fn above_the_region_boundary_is_nan_pending_region_three() {
        assert!(t_ph(18.0, 1500.0).is_nan());
        assert!(t_ph(50.0, 2000.0).is_nan());
        // The benchmark pressures work.
        assert!(t_ph(15.5, 1200.0).is_finite());
        assert!(t_ph(7.0, 1200.0).is_finite());
    }

    /// The 2b/2c boundary enthalpy is continuous with the sub-region fits that
    /// meet on it.
    ///
    /// # Methodology
    ///
    /// At a pressure where 2b and 2c are neighbours, `h2bc_p(p)` is the dividing
    /// enthalpy. Evaluating both fits there must give nearly the same
    /// temperature — they are separate polynomials that the formulation
    /// constructs to agree on this line. A disagreement would mean either
    /// `h2bc_p` or one of the fits is mistranscribed.
    ///
    /// Pass criterion: the two agree within 0.1 K, which is loose relative to
    /// the 25 mK fit tolerance because the boundary is exactly where both fits
    /// are at their worst.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// The two fits differ by **7.06 mK** at 30 MPa, **3.19 mK** at 50 MPa and
    /// **7.44 mK** at 80 MPa — all well inside the 0.1 K criterion and inside
    /// the 25 mK fit tolerance too.
    ///
    /// **Interpretation.** `h2bc_p`, `T2b_ph` and `T2c_ph` are three
    /// independently transcribed coefficient sets, and they meet on a shared
    /// line to within the formulation's own accuracy. An error in any one of
    /// the three would break the agreement.
    #[test]
    fn the_2b_2c_boundary_is_continuous() {
        for p in [30.0, 50.0, 80.0] {
            let h = h2bc_p(p);
            let tb = t2b_ph(p, h);
            let tc = t2c_ph(p, h);
            eprintln!(
                "at {p} MPa, h2bc = {h} kJ/kg: T2b = {tb}, T2c = {tc}, diff = {}",
                (tb - tc).abs()
            );
            assert!(
                (tb - tc).abs() < 0.1,
                "2b/2c disagree by {} K at {p} MPa",
                (tb - tc).abs()
            );
        }
    }

    /// `TB23_p` reproduces the region 2/3 boundary's published anchor point.
    ///
    /// # Methodology
    ///
    /// IAPWS-IF97 states the boundary passes through
    /// `(p, T) = (16.5292 MPa, 623.15 K)`, which is also where region 1, 2, 3
    /// and 4 meet. Pass criterion 0.01 K.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// `TB23_p(16.5292 MPa) = 623.149999999997 K` against the defined
    /// 623.15 K — agreement to 3e-12 K. Since `p_b13_sat()` itself comes from
    /// [`super::region4::psat_t`], this closes a loop across three separately
    /// transcribed equations.
    #[test]
    fn the_region_23_boundary_passes_through_its_anchor() {
        let t = tb23_p(p_b13_sat());
        eprintln!("TB23_p(p_b13_sat) = {t} K, expected {T_B13}");
        assert!((t - T_B13).abs() < 0.01, "got {t}");
    }
}
