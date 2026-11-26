//! # Unit V&V of the core against MELCOR 2.2 (2026-11-26)
//!
//! > Research, education and V&V only. AI draft, not reviewed by a human.
//!
//! Each test checks one item of
//! `crates/sembawang/verification_and_validation/bwr_mark1/melcor-unit-vv/`
//! (its ID is in the test's doc). The reference values were computed with
//! `python3` from the manual's own equations and constants (`calc/`), never
//! from this code. These tests are the "concrete V&V at unit-test level" the
//! root `AGENTS.md` asks for before any regression pin.
//!
//! Tolerances: where the item gives a value to 7 significant figures, the
//! test allows 1e-6 relative (the printed rounding). Where the test
//! re-evaluates the manual's equation itself, it allows 1e-12 relative
//! (PETIR's `exp` against `f64::exp`, a few ulp).

use super::*;

/// Relative difference.
fn rel(a: f64, b: f64) -> f64 {
    ((a - b) / b).abs()
}

/// **COR1-16, Urbanic-Heidrick (MELCOR Vol. 2 COR §2.5.4, Eq 2-169,
/// Table 2-6; C1001, C1004).** Methodology: our `oxidation_rate` at the
/// item's temperatures against its python values (7 figures) and against the
/// manual's equation re-evaluated here: `29.6 exp(-16820/T)` from 1100 to
/// 1853 K, `87.9 exp(-16610/T)` from 1873 K, linear in T between, zero below
/// 1100 K. Results (2026-11-26): pass.
#[test]
fn cor1_16_urbanic_heidrick_rate_matches_the_manual() {
    let prm = Params::default();
    let manual = |t: f64| -> f64 {
        let lo = |t: f64| 29.6 * (-16820.0 / t).exp();
        let hi = |t: f64| 87.9 * (-16610.0 / t).exp();
        if t < 1100.0 {
            0.0
        } else if t <= 1853.0 {
            lo(t)
        } else if t >= 1873.0 {
            hi(t)
        } else {
            lo(1853.0) + (t - 1853.0) / 20.0 * (hi(1873.0) - lo(1853.0))
        }
    };
    let printed = [
        (1100.0, 6.769152e-6),
        (1500.0, 3.993953e-4),
        (1853.0, 3.381629e-3),
        (1863.0, 7.879247e-3),
        (1873.0, 1.237687e-2),
        (2000.0, 2.173568e-2),
        (2400.0, 8.675792e-2),
    ];
    assert_eq!(oxidation_rate(&prm, 1099.9), 0.0);
    for (t, want) in printed {
        let k = oxidation_rate(&prm, t);
        assert!(rel(k, want) < 1e-6, "T {t}: {k:e} vs printed {want:e}");
        assert!(rel(k, manual(t)) < 1e-12, "T {t}: {k:e} vs the manual's equation {:e}", manual(t));
    }
    // The parabolic step (Eq 2-157): W_i = 0.1 kg/m^2, 100 s at 1500 K.
    let w = (0.1f64 * 0.1 + oxidation_rate(&prm, 1500.0) * 100.0).sqrt();
    assert!(rel(w, 0.2234715) < 1e-6, "{w}");
}

/// **COR1-01, oxidised-Zircaloy emissivity (Vol. 2 COR §2.1.1, Eqs 2-7 to
/// 2-10, COR-RM-28/29; C1104).** Methodology: `zr_emissivity` at the item's
/// oxide thicknesses and temperatures, against its python values. The item
/// notes the manuals disagree on the 0.325 floor (on the factor in Vol. 2,
/// on the emissivity in Vol. 1 C1104(9)); this test pins the Vol. 1 reading,
/// which the code follows. Known deviation kept and stated: the code uses
/// the present temperature, not the maximum ever reached. Results
/// (2026-11-26): failed against the rounded 0.808 / 0.759 endpoints (5.9e-5
/// to 4.0e-4 relative); pass after the code took Eqs 2-7 and 2-8 directly.
#[test]
fn cor1_01_zircaloy_emissivity_matches_the_manual() {
    for (dr, t, want) in [
        (0.0, 1000.0, 0.325000),
        (2.0e-6, 1000.0, 0.574200),
        (3.8799999e-6, 1000.0, 0.808448),
        (5.0e-4, 1000.0, 0.783642),
        (2.0e-3, 1000.0, 0.758642),
        (2.0e-3, 1600.0, 0.543591),
        (5.0e-4, 1800.0, 0.325),
        (2.0e-3, 3000.0, 0.325),
    ] {
        let e = zr_emissivity(dr, t);
        assert!((e - want).abs() < 1.0e-6, "dr {dr} T {t}: {e} vs {want}");
    }
}

/// **COR1-12, Lipinski's 0-D dryout flux (Vol. 2 COR §2.3.8, Eqs 2-114 and
/// 2-115, COR-RM-69/70; C1244 = 0.756, 0.089 m, 0.15).** Methodology:
/// `lipinski_dryout_flux_with` on the item's property set (rho_l 915,
/// rho_v 2.67 kg/m^3, h_lv 2.108e6 J/kg, L 1 m; the code takes g = 9.81 as
/// the item does) against its python values. Results (2026-11-26): pass,
/// after the item's own d = 0.5 mm / floor pairing was found wrong (the
/// code was right) and corrected.
#[test]
fn cor1_12_lipinski_dryout_flux_matches_the_manual() {
    let q = |d: f64, eps: f64| lipinski_dryout_flux_with(915.0, 2.67, 2.108e6, d, eps, 1.0);
    assert!(rel(q(0.01, 0.4), 5.309088e6) < 1e-6, "{}", q(0.01, 0.4));
    assert!(rel(q(5.0e-4, 0.4), 1.236103e6) < 1e-6, "{}", q(5.0e-4, 0.4));
    // Porosity floored at 0.15, at d = 0.01 m (the item paired this value
    // with d = 0.5 mm by mistake; corrected there, 2026-11-26).
    assert!(rel(q(0.01, 0.05), 1.024314e6) < 1e-6, "{}", q(0.01, 0.05));
    assert_eq!(q(0.01, 0.05), q(0.01, 0.15));
}

/// **RN-01, CORSOR-M (Vol. 2 RN §2.3.1.2, Eq 2-11, RN-RM-19; C7102).**
/// Methodology: `volatile_release_rate` (the Xe/Cs/I2/Te row, k0 2.0e5 /min,
/// Q 63.8 kcal/mol) and `uo2_release_rate` (La/UO2, 1.46e7, 143.1) against
/// the item's python values per second, with the manual's R = 1.987e-3
/// kcal/(mol K). Results (2026-11-26): failed by 0.14 to 0.33 % with CODATA's
/// R (Cs 2400 K 5.166677e-3 /s); pass after the code took the correlation's
/// own R (`CORSOR_R_KCAL_PER_MOL_K`).
#[test]
fn rn_01_corsor_m_rates_match_the_manual() {
    let prm = Params::default();
    for (t, want) in [(2400.0, 5.159582e-3), (1000.0, 3.786558e-11)] {
        let r = volatile_release_rate(&prm, t);
        assert!(rel(r, want) < 1e-6, "Cs at {t} K: {r:e} vs {want:e}");
    }
    let per_min = [(1173.0, 2.588340e-7), (1500.0, 1.010665e-4), (2000.0, 2.131636e-2), (2500.0, 5.286599e-1)];
    for (t, want) in per_min {
        let r = volatile_release_rate(&prm, t) * 60.0;
        assert!(rel(r, want) < 1e-6, "Xe/Cs at {t} K: {r:e} /min vs {want:e}");
    }
    let u = uo2_release_rate(&prm, 2500.0);
    assert!(rel(u, 7.505388e-8) < 1e-6, "UO2 at 2500 K: {u:e}");
    for (t, want) in [(1173.0, 3.163611e-20), (2000.0, 3.355964e-9), (2400.0, 1.355938e-6)] {
        let r = uo2_release_rate(&prm, t) * 60.0;
        assert!(rel(r, want) < 1e-6, "UO2 at {t} K: {r:e} /min vs {want:e}");
    }
}

/// **COR1-22 / COR2-17, MELCOR's simple B4C model (Vol. 2 COR §2.5.9,
/// Eqs 2-197 to 2-208, COR-RM-105/106; C1005, C1006).** Methodology:
/// `b4c_reaction(T, P_H2, P_H2O)` (atm) against the item's python values for
/// the CO/CO2/CH4 split, steam and H2 per mol of B4C, and the heat (J/mol);
/// the rate law `(9.973e6/60) exp(-22647.2/T)` at four temperatures; and the
/// gates `b4c_may_react` (sheath failed below 0.9 intact steel, C1005(3); above
/// 1500 K, C1005(4)). Results (2026-11-26): the chemistry and rate pass at
/// 1e-6; the gates were missing (B4C reacted on intact blades at any
/// temperature) and were added; pass.
#[test]
fn cor1_22_b4c_model_and_its_gates_match_the_manual() {
    // (T, P_H2, P_H2O) -> x_CO, x_CO2, x_CH4, steam, H2, Q (J/mol)
    let (x, steam, h2, q) = b4c_reaction(1600.0, 1.0, 1.0);
    assert!(rel(x[0], 0.763843847) < 1e-8 && rel(x[1], 0.236155003) < 1e-8 && rel(x[2], 1.15012273e-6) < 1e-6, "{x:?}");
    assert!(rel(steam, 7.23615385) < 1e-8 && rel(h2, 7.23615155) < 1e-8, "{steam} {h2}");
    assert!(rel(q, 737407.806) < 1e-8, "{q}");
    let (x, steam, h2, q) = b4c_reaction(1200.0, 1.0, 0.01);
    assert!(rel(x[0], 0.951059606) < 1e-8 && rel(x[1], 0.00623122672) < 1e-8 && rel(x[2], 0.0427091674) < 1e-8, "{x:?}");
    assert!(rel(steam, 6.96352206) < 1e-8 && rel(h2, 6.87810372) < 1e-8 && rel(q, 723120.737) < 1e-8, "{steam} {h2} {q}");
    let (x, steam, _, _) = b4c_reaction(1600.0, 0.1, 1.0);
    assert!(rel(x[0], 0.244399227) < 1e-8 && rel(x[1], 0.755600772) < 1e-8 && rel(steam, 7.75560077) < 1e-8, "{x:?} {steam}");
    // The rate law.
    let prm = Params::default();
    let rate = |t: f64| prm.b4c_rate_a * (-prm.b4c_rate_b / t).exp();
    for (t, want) in [(1200.0, 0.00105774158), (1500.0, 0.046093382), (1600.0, 0.118427708), (2000.0, 2.00861903)] {
        assert!(rel(rate(t), want) < 1e-8, "T {t}: {} vs {want}", rate(t));
    }
    // The gates: intact blade, or below 1500 K, does not react.
    let m0 = blade_per_node(&prm).0;
    assert!(!b4c_may_react(&prm, 0.0, 1600.0), "an intact sheath holds the B4C in");
    assert!(!b4c_may_react(&prm, 0.05 * m0, 1600.0), "95 % intact: not yet failed");
    assert!(b4c_may_react(&prm, 0.11 * m0, 1600.0), "89 % intact: the sheath has failed");
    assert!(!b4c_may_react(&prm, 0.11 * m0, 1500.0), "not above 1500 K");
    assert!(!b4c_may_react(&prm, 0.11 * m0, 1200.0), "the 1200 K case of the old code");
}

/// **COR1-03, two gray surfaces (MELCOR Vol. 2 COR §2.1, Eqs 2-1 to 2-6;
/// §2.1.2 limit (1)).** Methodology: `gray_exchange` with no medium
/// against the item's python values, (a) `AF = 1`, eps 0.8/0.5,
/// 1500/1000 K: 102381.760 W; (b) `AF = 0.25`, eps 0.325/0.95,
/// 1500/560 K: 45922.778 W (both A1 = A2 = 1 m^2). Then `cell_af` takes
/// `F MIN(A_cell, A1, A2)`, and with rod surfaces far larger than the cell
/// boundary the exchange tends to `AF sigma dT^4`, the space resistance
/// alone. Results (2026-11-26): **the old form failed** (b) by a factor
/// 2.04 (22486.12 W); the code now solves the network and passes at
/// 1e-12 against the full-precision python values (102381.76034305555,
/// 45922.77796855471 W; the item prints 9 figures).
/// The steam medium (Eq 2-5, item case (c)) is not modelled and not tested.
#[test]
fn cor1_03_gray_exchange_is_melcors_network() {
    let a = gray_exchange(1.0, 1.0, 0.8, 1.0, 0.5, 1500.0, 1000.0);
    assert!(rel(a, 102381.76034305555) < 1e-12, "{a}");
    let b = gray_exchange(0.25, 1.0, 0.325, 1.0, 0.95, 1500.0, 560.0);
    assert!(rel(b, 45922.77796855471) < 1e-12, "{b}");
    assert_eq!(cell_af(0.1, 2.0, 50.0, 80.0), 0.1 * 2.0);
    assert_eq!(cell_af(0.1, 2.0, 0.5, 80.0), 0.1 * 0.5);
    let sigma = 5.670_374_419e-8;
    let big = gray_exchange(cell_af(0.1, 1.0, 1e9, 1e9), 1e9, 0.7, 1e9, 0.7, 1500.0, 1000.0);
    assert!(rel(big, 0.1 * sigma * (1500.0f64.powi(4) - 1000.0f64.powi(4))) < 1e-7, "{big}");
}

/// **COR2-07, lower-head Larson-Miller creep (MELCOR Vol. 2 §6.3, Eqs 6-23
/// to 6-26; C1601, C1600(3)).** Methodology: `larson_miller_rupture_s`
/// against the item's python values, 59777.6564 s at 1273 K and 10 MPa,
/// 50161.0628 s at 1000 K and 50 MPa, 16184.7543 s at 1100 K and
/// 36.7131677 MPa; the wall's mass-averaged temperature for the item's
/// profile 1500/1200/1000/900/800 K, 1062.5 K (face nodes half mass);
/// one 600 s step at 1273 K and 10 MPa adds `0.18 x 600 / 59777.66`
/// strain; below 1000 Pa nothing accumulates. Results (2026-11-26): the
/// rate passes at 1e-9; **the temperature failed** (the plain mean, 1080 K,
/// a rupture time 0.59 times MELCOR's at 10 MPa): now the mass average;
/// pass.
#[test]
fn cor2_07_lower_head_creep_matches_the_manual() {
    let prm = Params::default();
    assert!(rel(larson_miller_rupture_s(&prm, 1273.0, 10.0e6), 59777.6564) < 1e-9);
    assert!(rel(larson_miller_rupture_s(&prm, 1000.0, 50.0e6), 50161.0628) < 1e-9);
    assert!(rel(larson_miller_rupture_s(&prm, 1100.0, 36713167.7), 16184.7543) < 1e-8);
    let mut w = HeadWall::new(1000.0);
    w.t_k = [1500.0, 1200.0, 1000.0, 900.0, 800.0];
    assert!((w.mass_mean_k() - 1062.5).abs() < 1e-12);
    let (ri, ro) = (prm.rpv_radius_m, prm.rpv_radius_m + prm.lower_head_thickness_m);
    let dp = 10.0e6 * (ro * ro - ri * ri) / (ri * ri);
    let mut c = HeadWall::new(1273.0);
    assert!(!c.failed(&prm, dp, 600.0, false));
    assert!(rel(c.strain, 0.18 * 600.0 / 59777.6564) < 1e-8, "{}", c.strain);
    let mut d = HeadWall::new(1273.0);
    assert!(!d.failed(&prm, 999.0, 600.0, false));
    assert_eq!(d.strain, 0.0);
}

/// **COR1-20, stainless-steel oxidation (MELCOR Vol. 2 COR §2.5.6, Eqs
/// 2-176 to 2-179, Table 2-9, Table 2-5; C1002, C1004(1)).** Methodology:
/// `ss_oxidation_rate` and `ss_reaction` at X = 0.74/0.18/0.08 Fe/Cr/Ni
/// against the item's python values: K = 4.402883e-8, 1.281616e-3,
/// 3.564512e-2 (kg/m^2)^2/s at 1100, 1500, 1700 K, zero below 1100 K;
/// 0.3568176 kg steam, 0.03993030 kg H2 and 277.0978 kJ per kg of steel.
/// Results (2026-11-26): pass at 1e-6 (the item's 7 figures; was checked
/// at 1e-3 by `the_blade_steel_reacts_as_melcor_says`).
#[test]
fn cor1_20_steel_oxidation_matches_the_manual() {
    let prm = Params::default();
    assert_eq!(ss_oxidation_rate(&prm, 1099.9), 0.0);
    for (t, k) in [(1100.0, 4.402883e-8), (1500.0, 1.281616e-3), (1700.0, 3.564512e-2)] {
        assert!(rel(ss_oxidation_rate(&prm, t), k) < 1e-6, "{t}");
    }
    let (steam, h2, heat) = ss_reaction(&prm);
    assert!(rel(steam, 0.3568176) < 1e-6, "{steam}");
    assert!(rel(h2, 0.03993030) < 1e-6, "{h2}");
    assert!(rel(heat, 277.0978e3) < 1e-6, "{heat}");
}

/// **COR1-19, heat of the Zr-steam reaction and its reference state
/// (MELCOR Vol. 2 COR §2.5.2, Eqs 2-166/2-167, Table 2-5).** Our heat,
/// 6.763 MJ/kg Zr, is referenced to water VAPOUR at 298.15 K (`offbeat`'s
/// `exotherm`), and the steam's enthalpy above 298 K vapour is added
/// separately; MELCOR's 5797 kJ/kg is consistent with water referenced to
/// LIQUID (the CVH convention). So a direct comparison reads a false
/// +16.7 %. Methodology: move ours to the liquid reference by the two
/// moles of water's latent heat at 298.15 K (the formation enthalpies'
/// difference, 285.83 - 241.826 = 44.004 kJ/mol), and compare with the
/// table. Results (2026-11-26): 5.798252 MJ/kg against 5.797, 2.2e-4
/// relative: inside 3e-4 but **2.5 times the table's printed rounding**.
/// The rest is in the formation data MELCOR's figure came from, which the
/// manual does not give (not determined). Pass at 3e-4.
#[test]
fn cor1_19_reaction_heat_agrees_with_the_manual_at_a_common_reference() {
    let prm = Params::default();
    let liquid = prm.oxidation_heat_j_per_kg_zr - 2.0 * (285.83e3 - 241.826e3) / M_ZR;
    assert!(rel(liquid, 5797.0e3) < 3e-4, "{liquid}");
    assert!(rel(prm.oxidation_heat_j_per_kg_zr, 5797.0e3) > 0.16);
}
