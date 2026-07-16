//! Second and third **virial coefficients** for the dry-air / water-vapour
//! mixture, following ASHRAE RP-1485 (Herrmann et al., 2009) — the cross terms
//! that make humid air a *real*-gas mixture rather than two ideal gases.
//!
//! Ported from CoolProp `HumidAirProp.cpp`'s `FlagUseVirialCorrelations == 1`
//! branch: literal degree-7-in-`T` polynomial fits for the pure-component
//! virials (`B_aa`, `B_ww`, `C_aaa`, `C_www`) and the fitted mixed-virial
//! correlations (`B_aw`, `C_aaw`, `C_aww`). This is CoolProp's *non-default*
//! path (`FlagUseVirialCorrelations` itself defaults to `0`, i.e. deriving the
//! pure-component `B`/`C` from the Air/Water Helmholtz EOS in the zero-density
//! limit) — chosen here because the EOS-derived path was tried first and found
//! numerically fragile: water's EOS has residual terms of the exponential
//! `n·δ^d·τ^t·exp(-δ^l)` form whose zero-density second `δ`-derivative is a
//! finite but delicate cancelling limit that converges very slowly at small
//! `δ` in `f64`. The literal polynomial fits have no such fragility. See
//! `tests/humid_air_reference.rs` for the resulting `c_p`/`V` cross-checks
//! (agree with the ASHRAE ideal-gas ballpark to <0.1%, once a `/rhobarstar²`
//! factor missing from an earlier version of [`c_aaw`] was found and fixed —
//! it had made the third-virial correction four orders of magnitude too
//! large, visible as a ~5.5% specific-volume error against the ideal-gas
//! limit, a magnitude no genuine real-gas correction at 1 atm could produce).
//!
//! Each coefficient is a function of temperature `T` \[K\]. Naming: `a` = dry
//! air, `w` = water. The mixture second virial is
//! `B_mix = (1-ψ_w)² B_aa + 2(1-ψ_w)ψ_w B_aw + ψ_w² B_ww`, and likewise for the
//! third virial `C_mix` in the pure/cross coefficients below (see [`b_mix`],
//! [`c_mix`]).

/// Second virial coefficient of dry air `B_aa(T)` \[m³/mol\].
pub fn b_aa(t: f64) -> f64 {
    -0.000721183853646 + 1.142682674467e-05 * t - 8.838228412173e-08 * t.powi(2)
        + 4.104150642775e-10 * t.powi(3)
        - 1.192780880645e-12 * t.powi(4)
        + 2.134201312070e-15 * t.powi(5)
        - 2.157430412913e-18 * t.powi(6)
        + 9.453830907795e-22 * t.powi(7)
}

/// `dB_aa/dT` \[m³/(mol·K)\].
fn d_b_aa_dt(t: f64) -> f64 {
    1.65159324353e-05 - 3.026130954749e-07 * t + 2.558323847166e-09 * t.powi(2)
        - 1.250695660784e-11 * t.powi(3)
        + 3.759401946106e-14 * t.powi(4)
        - 6.889086380822e-17 * t.powi(5)
        + 7.089457032972e-20 * t.powi(6)
        - 3.149942145971e-23 * t.powi(7)
}

/// Second virial coefficient of water vapour `B_ww(T)` \[m³/mol\].
pub fn b_ww(t: f64) -> f64 {
    -10.8963128394 + 2.439761625859e-01 * t - 2.353884845100e-03 * t.powi(2)
        + 1.265864734412e-05 * t.powi(3)
        - 4.092175700300e-08 * t.powi(4)
        + 7.943925411344e-11 * t.powi(5)
        - 8.567808759123e-14 * t.powi(6)
        + 3.958203548563e-17 * t.powi(7)
}

/// `dB_ww/dT` \[m³/(mol·K)\]. **Not** the analytic derivative of [`b_ww`] —
/// CoolProp fits `B_ww` and `dB_ww/dT` as two *independent* RP-1485
/// correlations (confirmed: their leading coefficients do not match `d(b_ww)`
/// by hand-differentiation). This is a documented CoolProp source quirk, not a
/// transcription error here — both were verified against the exact source
/// text.
fn d_b_ww_dt(t: f64) -> f64 {
    0.65615868848 - 1.487953162679e-02 * t + 1.450134660689e-04 * t.powi(2)
        - 7.863187630094e-07 * t.powi(3)
        + 2.559556607010e-09 * t.powi(4)
        - 4.997942221914e-12 * t.powi(5)
        + 5.417678681513e-15 * t.powi(6)
        - 2.513856275241e-18 * t.powi(7)
}

/// **Cross** second virial coefficient air–water `B_aw(T)` \[m³/mol\]
/// (Hyland & Wexler 1983, as fit in ASHRAE RP-1485 Eq. 3.10).
pub fn b_aw(t: f64) -> f64 {
    let a = [66.5687e0, -238.834e0, -176.755e0];
    let b = [-0.237, -1.048, -3.183];
    let (rhobarstar, tstar) = (1000.0, 100.0);
    1.0 / rhobarstar
        * (a[0] * (t / tstar).powf(b[0])
            + a[1] * (t / tstar).powf(b[1])
            + a[2] * (t / tstar).powf(b[2]))
        / 1000.0
}

/// `dB_aw/dT` \[m³/(mol·K)\] — the exact analytic derivative of [`b_aw`].
fn d_b_aw_dt(t: f64) -> f64 {
    let a = [66.5687e0, -238.834e0, -176.755e0];
    let b = [-0.237, -1.048, -3.183];
    let (rhobarstar, tstar) = (1000.0, 100.0);
    1.0 / rhobarstar / tstar
        * (a[0] * b[0] * (t / tstar).powf(b[0] - 1.0)
            + a[1] * b[1] * (t / tstar).powf(b[1] - 1.0)
            + a[2] * b[2] * (t / tstar).powf(b[2] - 1.0))
        / 1000.0
}

/// Third virial coefficient of dry air `C_aaa(T)` \[m⁶/mol²\].
pub fn c_aaa(t: f64) -> f64 {
    1.29192158975e-08 - 1.776054020409e-10 * t + 1.359641176409e-12 * t.powi(2)
        - 6.234878717893e-15 * t.powi(3)
        + 1.791668730770e-17 * t.powi(4)
        - 3.175283581294e-20 * t.powi(5)
        + 3.184306136120e-23 * t.powi(6)
        - 1.386043640106e-26 * t.powi(7)
}

/// `dC_aaa/dT` \[m⁶/(mol²·K)\]. See [`d_b_ww_dt`] on independently-fit `d/dT`
/// correlations.
fn d_c_aaa_dt(t: f64) -> f64 {
    -2.46582342273e-10 + 4.425401935447e-12 * t - 3.669987371644e-14 * t.powi(2)
        + 1.765891183964e-16 * t.powi(3)
        - 5.240097805744e-19 * t.powi(4)
        + 9.502177003614e-22 * t.powi(5)
        - 9.694252610339e-25 * t.powi(6)
        + 4.276261986741e-28 * t.powi(7)
}

/// Third virial coefficient of water vapour `C_www(T)` \[m⁶/mol²\].
pub fn c_www(t: f64) -> f64 {
    -0.580595811134 + 1.365952762696e-02 * t - 1.375986293288e-04 * t.powi(2)
        + 7.687692259692e-07 * t.powi(3)
        - 2.571440816920e-09 * t.powi(4)
        + 5.147432221082e-12 * t.powi(5)
        - 5.708156494894e-15 * t.powi(6)
        + 2.704605721778e-18 * t.powi(7)
}

/// `dC_www/dT` \[m⁶/(mol²·K)\]. See [`d_b_ww_dt`] on independently-fit `d/dT`
/// correlations.
fn d_c_www_dt(t: f64) -> f64 {
    0.0984601196142 - 2.356713397262e-03 * t + 2.409113323685e-05 * t.powi(2)
        - 1.363083778715e-07 * t.powi(3)
        + 4.609623799524e-10 * t.powi(4)
        - 9.316416405390e-13 * t.powi(5)
        + 1.041909136255e-15 * t.powi(6)
        - 4.973918480607e-19 * t.powi(7)
}

/// **Cross** third virial coefficient `C_aaw(T)` \[m⁶/mol²\] (two air, one
/// water; Hyland & Wexler 1983 fit). CoolProp's `_C_aaw` divides by
/// `rhobarstar² = 1000² = 1e6` *and* the `1e6` dm⁶→m⁶ unit conversion (two
/// separate `/1e6` factors) — unlike [`c_aww`], whose `rhobarstar = 1` makes
/// that factor a no-op.
pub fn c_aaw(t: f64) -> f64 {
    let c = [
        0.482737e3,
        0.105678e6,
        -0.656394e8,
        0.294442e11,
        -0.319317e13,
    ];
    let s: f64 = c
        .iter()
        .enumerate()
        .map(|(idx, ci)| ci * t.powi(-(idx as i32)))
        .sum();
    s / 1e6 / 1e6
}

/// `dC_aaw/dT` \[m⁶/(mol²·K)\] — the exact analytic derivative of [`c_aaw`].
fn d_c_aaw_dt(t: f64) -> f64 {
    let c = [
        0.482737e3,
        0.105678e6,
        -0.656394e8,
        0.294442e11,
        -0.319317e13,
    ];
    let s: f64 = c
        .iter()
        .enumerate()
        .skip(1)
        .map(|(idx, ci)| ci * (-(idx as f64)) * t.powi(-(idx as i32) - 1))
        .sum();
    s / 1e6 / 1e6
}

/// **Cross** third virial coefficient `C_aww(T)` \[m⁶/mol²\] (one air, two
/// water; Hyland & Wexler 1983 fit).
pub fn c_aww(t: f64) -> f64 {
    let d = [-0.1072887e2, 0.347804e4, -0.383383e6, 0.334060e8];
    let s: f64 = d
        .iter()
        .enumerate()
        .map(|(idx, di)| di * t.powi(-(idx as i32)))
        .sum();
    -s.exp() / 1e6
}

/// `dC_aww/dT` \[m⁶/(mol²·K)\] — the exact analytic derivative of [`c_aww`].
fn d_c_aww_dt(t: f64) -> f64 {
    let d = [-0.1072887e2, 0.347804e4, -0.383383e6, 0.334060e8];
    let s1: f64 = d
        .iter()
        .enumerate()
        .map(|(idx, di)| di * t.powi(-(idx as i32)))
        .sum();
    let s2: f64 = d
        .iter()
        .enumerate()
        .skip(1)
        .map(|(idx, di)| di * (-(idx as f64)) * t.powi(-(idx as i32) - 1))
        .sum();
    -s1.exp() * s2 / 1e6
}

/// Mixture second virial coefficient `B_m(T, ψ_w)` \[m³/mol\].
pub fn b_mix(t: f64, psi_w: f64) -> f64 {
    (1.0 - psi_w).powi(2) * b_aa(t)
        + 2.0 * (1.0 - psi_w) * psi_w * b_aw(t)
        + psi_w * psi_w * b_ww(t)
}

/// `dB_m/dT` \[m³/(mol·K)\].
pub fn d_b_mix_dt(t: f64, psi_w: f64) -> f64 {
    (1.0 - psi_w).powi(2) * d_b_aa_dt(t)
        + 2.0 * (1.0 - psi_w) * psi_w * d_b_aw_dt(t)
        + psi_w * psi_w * d_b_ww_dt(t)
}

/// Mixture third virial coefficient `C_m(T, ψ_w)` \[m⁶/mol²\].
pub fn c_mix(t: f64, psi_w: f64) -> f64 {
    (1.0 - psi_w).powi(3) * c_aaa(t)
        + 3.0 * (1.0 - psi_w).powi(2) * psi_w * c_aaw(t)
        + 3.0 * (1.0 - psi_w) * psi_w * psi_w * c_aww(t)
        + psi_w.powi(3) * c_www(t)
}

/// `dC_m/dT` \[m⁶/(mol²·K)\].
pub fn d_c_mix_dt(t: f64, psi_w: f64) -> f64 {
    (1.0 - psi_w).powi(3) * d_c_aaa_dt(t)
        + 3.0 * (1.0 - psi_w).powi(2) * psi_w * d_c_aaw_dt(t)
        + 3.0 * (1.0 - psi_w) * psi_w * psi_w * d_c_aww_dt(t)
        + psi_w.powi(3) * d_c_www_dt(t)
}

#[cfg(test)]
mod self_consistency_tests {
    use super::*;

    fn fd(f: impl Fn(f64) -> f64, t: f64) -> f64 {
        let h = 1e-4;
        (f(t + h) - f(t - h)) / (2.0 * h)
    }

    /// The `B_aw`/`C_aaw`/`C_aww` cross-term derivatives *are* the exact
    /// analytic derivative of their value functions (unlike the pure-component
    /// `B_ww`/`C_aaa`/`C_www`/`B_aa` — see [`d_b_ww_dt`]), so this must hold
    /// tightly.
    #[test]
    fn cross_term_derivatives_match_finite_difference_of_values() {
        let t = 300.0;
        let cases: [(&str, f64, f64); 3] = [
            ("dB_aw", d_b_aw_dt(t), fd(b_aw, t)),
            ("dC_aaw", d_c_aaw_dt(t), fd(c_aaw, t)),
            ("dC_aww", d_c_aww_dt(t), fd(c_aww, t)),
        ];
        for (name, analytic, numeric) in cases {
            let rel = ((analytic - numeric) / numeric).abs();
            eprintln!("{name}: analytic={analytic:e} fd={numeric:e} rel={rel:e}");
            assert!(
                rel < 1e-4,
                "{name}: analytic {analytic} vs FD {numeric}, rel err {rel}"
            );
        }
    }

    #[test]
    fn b_aa_and_b_ww_match_literature_ballpark() {
        // Dry air / water second virial coefficients at 300 K, literature
        // ballpark (Dymond & Smith; Harvey & Lemmon 2004): B_air ~ -6.6e-6 to
        // -7.0e-6 m^3/mol, B_water ~ -1.15e-3 m^3/mol.
        let b_air = b_aa(300.0);
        let b_water = b_ww(300.0);
        eprintln!("B_air(300K)={b_air:e}, B_water(300K)={b_water:e}");
        assert!(
            (-9e-6..-4e-6).contains(&b_air),
            "B_air(300K) = {b_air}, outside the expected ballpark"
        );
        assert!(
            (-1.6e-3..-0.8e-3).contains(&b_water),
            "B_water(300K) = {b_water}, outside the expected ballpark"
        );
    }
}
