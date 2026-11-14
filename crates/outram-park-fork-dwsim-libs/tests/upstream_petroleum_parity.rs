// SPDX-License-Identifier: GPL-3.0-only
//! # V&V — petroleum property correlations against upstream DWSIM (gh:#1356)
//!
//! **Methodology.** Upstream `PetroleumCharacterization/PropertyMethods.vb`
//! (DWSIM `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`, lines 32-470) is
//! transcribed verbatim into
//! `verification_and_validation/upstream_differential_gh1356/petroleum_driver.py`
//! (route and limits: that folder's README; `check_verbatim.sh`). Every
//! correlation the port ports (Tc, Pc, omega, MW, kinematic viscosities, SG
//! conversions: 36 call forms) is evaluated on a Tb x SG grid
//! (350-800 K x 0.70-0.95, plus Farah A/B, viscosity and MW grids): 1328
//! points, inputs identical bit for bit. Upstream results are frozen as raw
//! IEEE bit patterns in `tests/fixtures/upstream_gh1356/`; the port is
//! recomputed. Units: K, dimensionless SG, g/mol, m2/s, Pa on both sides.
//!
//! **Bound, derived before measuring** (README): <= 4 ulp, 1e-14 relative;
//! gate 1e-13. 1-ulp differences from PETIR `exp/ln/pow` vs libm, from
//! `powi(2)` vs `pow(x, 2)`, and from the uom `kg/mol` <-> `g/mol` round trip are
//! accepted, counted and pinned.
//!
//! **Results (2026-11-13, release; 1328 points).** 1280 bit-identical; 48
//! differ, worst relative gap 4.2e-15. By function (count, worst ulp):
//! `MW_Farah4` 21 (1), `MW_Winn` 6 (1), `MW_LeeKesler` 4 (1), `MW_Riazi` 3 (1),
//! `MW_Riazi_visc` 2 (1), `MW_Farah2` 1 (1), `Pc_LeeKesler` 1 (1),
//! `AcentricFactor_Korsten` 2 (2), `AcentricFactor_LeeKesler` 2 (2),
//! `ViscWaltherASTM_A/B` 6 rows (2-26). **Prediction check:** the relative bound
//! (<= 1e-14) held; the "<= 4 ulp" prediction FAILED on one row, the Walther
//! pair with `Vc` 0.29 vs 0.31, where `logZ1 - logZ2` cancels to ~1e-3 of
//! its terms (condition number ~ 1e3). Localisation:
//! [`walther_gap_is_libm_vs_petir_only`] re-evaluates upstream's operation
//! order with the platform libm and reproduces all 18 Walther rows bit for bit,
//! so the first divergent operation is `log10`/`exp` (PETIR vs libm), not the
//! arithmetic. The port's arithmetic order was already upstream's: no port
//! change was needed. The 1-ulp rows are attributed (NOT separately controlled) to `exp`/`pow` plus the `kg/mol`
//! unit round trip.

use outram_park_fork_dwsim_libs::petroleum::property_methods as pm;
use uom::si::f64::{KinematicViscosity, MolarMass, Pressure, Ratio, ThermodynamicTemperature};
use uom::si::kinematic_viscosity::square_meter_per_second as m2s;
use uom::si::molar_mass::gram_per_mole as gpm;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

fn f(hex: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(hex, 16).expect("hex bits"))
}
fn t(v: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(v)
}
fn sg(v: f64) -> Ratio {
    Ratio::new::<ratio>(v)
}
fn nu(v: f64) -> KinematicViscosity {
    KinematicViscosity::new::<m2s>(v)
}
fn pa(v: f64) -> Pressure {
    Pressure::new::<pascal>(v)
}
fn mw(v: f64) -> MolarMass {
    MolarMass::new::<gpm>(v)
}

/// Evaluate the port's counterpart of the upstream function `name`.
fn port(name: &str, a: &[f64]) -> f64 {
    match name {
        "Tc_RiaziDaubert" => pm::tc_riazi_daubert(t(a[0]), sg(a[1])).get::<kelvin>(),
        "Pc_RiaziDaubert" => pm::pc_riazi_daubert(t(a[0]), sg(a[1])).get::<pascal>(),
        "Tc_Riazi" => pm::tc_riazi_2005(t(a[0]), sg(a[1])).get::<kelvin>(),
        "Tc_LeeKesler" => pm::tc_lee_kesler(t(a[0]), sg(a[1])).get::<kelvin>(),
        "Pc_LeeKesler" => pm::pc_lee_kesler(t(a[0]), sg(a[1])).get::<pascal>(),
        "Tc_Farah2" => pm::tc_farah_ab(a[0], a[1]).get::<kelvin>(),
        "Tc_Farah3" => pm::tc_farah_ab_tb(a[0], a[1], t(a[2])).get::<kelvin>(),
        "Tc_Farah4" => pm::tc_farah_ab_sg_tb(a[0], a[1], sg(a[2]), t(a[3])).get::<kelvin>(),
        "Pc_Farah2" => pm::pc_farah_ab(a[0], a[1]).get::<pascal>(),
        "Pc_Farah3" => pm::pc_farah_ab_tb(a[0], a[1], t(a[2])).get::<pascal>(),
        "Pc_Farah4" => pm::pc_farah_ab_tb_sg(a[0], a[1], t(a[2]), sg(a[3])).get::<pascal>(),
        "AcentricFactor_LeeKesler" => {
            pm::acentric_factor_lee_kesler(t(a[0]), pa(a[1]), t(a[2])).get::<ratio>()
        }
        "AcentricFactor_Korsten" => {
            pm::acentric_factor_korsten(t(a[0]), pa(a[1]), t(a[2])).get::<ratio>()
        }
        "MW_Winn" => pm::mw_winn(t(a[0]), sg(a[1])).get::<gpm>(),
        "MW_Riazi" => pm::mw_riazi(t(a[0]), sg(a[1])).get::<gpm>(),
        "MW_Riazi_visc" => pm::mw_riazi_viscosity(nu(a[0]), nu(a[1]), sg(a[2])).get::<gpm>(),
        "MW_LeeKesler" => pm::mw_lee_kesler(t(a[0]), sg(a[1])).get::<gpm>(),
        "MW_Farah2" => pm::mw_farah_ab(a[0], a[1]).get::<gpm>(),
        "MW_Farah4" => pm::mw_farah_ab_sg_tb(a[0], a[1], sg(a[2]), t(a[3])).get::<gpm>(),
        "Visc37_Abbott" => pm::visc37_abbott(t(a[0]), sg(a[1])).get::<m2s>(),
        "Visc98_Abbott" => pm::visc98_abbott(t(a[0]), sg(a[1])).get::<m2s>(),
        "ViscT_Beg_Amin" => pm::visc_t_beg_amin(t(a[0]), t(a[1]), sg(a[2])).get::<m2s>(),
        "ViscWaltherASTM_A" => pm::visc_walther_astm_a(t(a[0]), nu(a[1]), t(a[2]), nu(a[3])),
        "ViscWaltherASTM_B" => pm::visc_walther_astm_b(t(a[0]), nu(a[1]), t(a[2]), nu(a[3])),
        "ViscTwu" => pm::visc_twu(t(a[0]), t(a[1]), t(a[2]), nu(a[3]), nu(a[4])).get::<m2s>(),
        "d15_v37v98" => pm::d15_from_viscosities(nu(a[0]), nu(a[1])).get::<ratio>(),
        "d20d15" => pm::d20_from_d15(sg(a[0])).get::<ratio>(),
        "d15d20" => pm::d15_from_d20(sg(a[0])).get::<ratio>(),
        "d15_Riazi" => pm::d15_riazi(mw(a[0])).get::<ratio>(),
        other => panic!("no port counterpart mapped for {other}"),
    }
}

fn ulps(a: f64, b: f64) -> u64 {
    (a.to_bits() as i64 - b.to_bits() as i64).unsigned_abs()
}

/// `(points, bit-identical, worst relative gap, worst ulp, worst function)`
fn compare() -> (usize, usize, f64, u64, String) {
    let text = include_str!("fixtures/upstream_gh1356/petroleum_upstream_output.txt");
    let (mut pts, mut same, mut worst, mut worst_ulp, mut who) = (0, 0, 0.0_f64, 0_u64, String::new());
    for line in text.lines().filter(|l| !l.starts_with('#') && !l.is_empty()) {
        let (lhs, rhs) = line.split_once('|').expect("row format");
        let mut it = lhs.split_whitespace();
        let name = it.next().expect("name");
        let args: Vec<f64> = it.map(f).collect();
        let theirs = f(rhs.trim());
        let mine = port(name, &args);
        pts += 1;
        if mine.to_bits() == theirs.to_bits() || (mine.is_nan() && theirs.is_nan()) {
            same += 1;
            continue;
        }
        let rel = ((mine - theirs) / theirs).abs();
        let u = ulps(mine, theirs);
        if u > worst_ulp {
            worst_ulp = u;
            who = name.to_string();
        }
        worst = worst.max(rel);
    }
    (pts, same, worst, worst_ulp, who)
}

#[test]
fn petroleum_correlations_match_upstream_property_methods() {
    let (pts, same, worst, worst_ulp, who) = compare();
    println!("petroleum: {pts} points, {same} bit-identical, worst rel {worst:e}, worst {worst_ulp} ulp ({who})");
    assert_eq!(pts, 1328, "fixture row count");
    assert!(worst < 1e-13, "gate 1e-13 breached: {worst:e} in {who}");
    // Pinned 2026-11-13: 48 of 1328 differ, all <= 3 ulp except the ill-conditioned
    // ViscWalther row with logZ1 ~ logZ2 (Vc 0.29 vs 0.31, below the correlation's range).
    assert_eq!(same, 1280, "bit-identical count moved");
    assert_eq!(worst_ulp, 26);
}

/// Localisation control for the worst rows (Walther): the same upstream
/// operation order evaluated with the platform libm (`std`) instead of PETIR.
fn walther_libm(a: &[f64], b_form: bool) -> f64 {
    fn log_z(vc: f64) -> f64 {
        let (mut c, mut d, mut e, mut f, mut g, mut h) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        if vc < 2.0 {
            c = (-1.14883 - 2.65868 * vc).exp();
            if vc < 1.65 {
                d = (-0.0038138 - 12.5645 * vc).exp();
                if vc < 0.9 {
                    e = (5.46491 - 37.6289 * vc).exp();
                    if vc < 0.3 {
                        f = (13.0458 - 74.6851 * vc).exp();
                        g = (37.4619 - 192.643 * vc).exp();
                        if vc < 0.24 {
                            h = (80.4945 - 400.468 * vc).exp();
                        }
                    }
                }
            }
        }
        (vc + 0.7 + c - d + e - f + g - h).log10().log10()
    }
    let (tt1, tt2) = (a[0].log10(), a[2].log10());
    let (z1, z2) = (log_z(a[1] * 1_000_000.0), log_z(a[3] * 1_000_000.0));
    if b_form {
        (z2 - z1) / (tt1 - tt2)
    } else {
        z2 + (z2 - z1) / (tt1 - tt2) * tt2
    }
}

#[test]
fn walther_gap_is_libm_vs_petir_only() {
    let text = include_str!("fixtures/upstream_gh1356/petroleum_upstream_output.txt");
    let mut n = 0;
    for line in text.lines().filter(|l| l.starts_with("ViscWalther")) {
        let (lhs, rhs) = line.split_once('|').expect("row format");
        let mut it = lhs.split_whitespace();
        let name = it.next().expect("name");
        let args: Vec<f64> = it.map(f).collect();
        let theirs = f(rhs.trim());
        let mine = walther_libm(&args, name.ends_with('B'));
        assert!(mine.to_bits() == theirs.to_bits() || (mine.is_nan() && theirs.is_nan()), "{name} {args:?}");
        n += 1;
    }
    assert_eq!(n, 8 + 10);
}
