// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! Helical-coil and flow-boiling correlations for the once-through steam
//! generator march ([`super`]), gh:#1157.
//!
//! > Research, education and V&V only. AI draft, unreviewed. Not for
//! > facility operation, licensing or safety decisions.
//!
//! Every function here is a published correlation, evaluated in
//! dimensionless form (Reynolds, Prandtl, Dean numbers, curvature ratio
//! `d/D_c`) or in strict SI where a dimensional result is unavoidable. The
//! dimensioned API of the model is [`super::HelicalCoilOtsg`]; these are its
//! closures.
//!
//! # Provenance and how each was checked (2026-11-03)
//!
//! Each correlation was checked against a source read during this session,
//! not written from memory. "Secondary" means the equation was read in an
//! open implementation or reproduction that cites the primary paper; the
//! primary paper itself was not accessible from this session (paywalled, or
//! the host was not reachable through the session proxy).
//!
//! | Function | Correlation | Primary source | Checked against |
//! |---|---|---|---|
//! | [`critical_reynolds_schmidt`] | `Re_crit = 2300 [1 + 8.6 (d/D_c)^0.45]` | Schmidt, E.F. (1967). Wärmeübergang und Druckverlust in Rohrschlangen. *Chemie Ingenieur Technik* 39(13), 781–789. doi:10.1002/cite.330391302 | Secondary: `fluids` (C. Bell, MIT) `helical_transition_Re_Schmidt`, with its doctest value; Modelica Standard Library 4.0.0 `Fluid.Dissipation.HeatTransfer.HelicalPipe` (from VDI Heat Atlas 2002, Gc 2) |
//! | [`curvature_diameter`] | `D_c = D_w [1 + (h/(π D_w))²]` | VDI Heat Atlas (2002), section Gc | Secondary: MSL 4.0.0, same package |
//! | [`darcy_friction_laminar_mishra_gupta`] | `f/f_s = 1 + 0.033 (log₁₀ De)⁴`, `1 < De < 3000` | Mishra, P. & Gupta, S.N. (1979). Momentum transfer in curved pipes. 1. Newtonian fluids. *Ind. Eng. Chem. Process Des. Dev.* 18(1), 130–137. doi:10.1021/i260069a017 | Secondary: the IIT (BHU) repository record of the paper (form and range); `De = Re √(d/D_c)` |
//! | [`darcy_friction_turbulent_coil`] | `ξ = 0.3164 Re^-0.25 + 0.03 (d/D_c)^0.5` | Gnielinski, V. (1986). Heat transfer and pressure drop in helically coiled tubes. *Proc. 8th Int. Heat Transfer Conf.*, vol. 6, 2847–2854. Hemisphere | Secondary: MSL 4.0.0 `kc_turbulent_KC` (`zeta_TOT`). Also equals Mishra & Gupta's turbulent `f_c − f_s = 0.0075 (d/D_c)^0.5` (Fanning) ×4, read in the same repository record |
//! | [`nusselt_laminar_gnielinski`] | `Nu = 3.66 + 0.08 [1 + 0.8 (d/D_c)^0.9] Re^m Pr^(1/3)`, `m = 0.5 + 0.2903 (d/D_c)^0.194` | Gnielinski (1986), as above; VDI Heat Atlas (2002) Gc 2 eq. 5 | Secondary: MSL 4.0.0 `kc_laminar_KC` |
//! | [`nusselt_turbulent_gnielinski`] | `Nu = (ξ/8) Re Pr / [1 + 12.7 √(ξ/8) (Pr^(2/3) − 1)]` | Gnielinski (1986); VDI Heat Atlas (2002) Ga 2 eq. 6 | Secondary: MSL 4.0.0 `kc_turbulent_KC` |
//! | [`nusselt_coil`] | laminar below `Re_crit`, turbulent above `2.2e4`, **linear interpolation between** | Gnielinski (1986) / VDI Heat Atlas | **[U]**: the interval (`Re_crit` to `2.2e4`) is verified (MSL `kc_overall_KC`); the *linear* interpolation form `Nu = γ Nu_lam(Re_crit) + (1−γ) Nu_turb(2.2e4)` is from memory of the VDI Heat Atlas and was NOT verified. MSL blends with its own smooth step instead |
//! | [`martinelli_xtt`] | `X_tt = ((1−x)/x)^0.9 (ρ_g/ρ_l)^0.5 (μ_l/μ_g)^0.1` | Lockhart, R.W. & Martinelli, R.C. (1949). Proposed correlation of data for isothermal two-phase, two-component flow in pipes. *Chem. Eng. Prog.* 45(1), 39–48 | Secondary: `ht` (C. Bell, MIT) `Chen_Edelstein` docstring |
//! | [`chen_f`], [`chen_s`] | `F = (1 + X_tt^-0.5)^1.78`, `S = 0.9622 − 0.5822 atan(Re_l F^1.25 / 6.18e4)` | Chen, J.C. (1966). Correlation for boiling heat transfer to saturated fluids in convective flow. *Ind. Eng. Chem. Process Des. Dev.* 5(3), 322–329. doi:10.1021/i260019a023; the closed forms are Edelstein, S., Pérez, A.J. & Chen, J.C. (1984). Analytic representation of convective boiling functions. *AIChE J.* 30(5), 840–841. doi:10.1002/aic.690300528 | Secondary: `ht` `Chen_Edelstein` (code and doctest value, reproduced in a test here) |
//! | [`forster_zuber_htc`] | `h = 0.00122 [k^0.79 c_p^0.45 ρ_l^0.49 / (σ^0.5 μ^0.29 h_fg^0.24 ρ_g^0.24)] ΔT_sat^0.24 Δp_sat^0.75` | Forster, H.K. & Zuber, N. (1955). Dynamics of vapor bubbles and boiling heat transfer. *AIChE J.* 1(4), 531–535. doi:10.1002/aic.690010425 | Secondary: `ht` `Forster_Zuber` (code and the doctest value from Cao (2009), reproduced in a test here). **Note: the pressure exponent is 0.75**; the value 0.99 recalled before checking was wrong |
//! | [`chisholm_c`], [`two_phase_friction_gradient`] | `φ_l² = 1 + C/X + 1/X²`, `C` = 20 / 12 / 10 / 5 | Lockhart & Martinelli (1949); Chisholm, D. (1967). A theoretical basis for the Lockhart–Martinelli correlation for two-phase flow. *Int. J. Heat Mass Transfer* 10(12), 1767–1778. doi:10.1016/0017-9310(67)90047-6 | Secondary: `fluids` `Lockhart_Martinelli` (code, C table and doctest value, reproduced in a test here) |
//!
//! Reused rather than re-implemented (not in this file): the Biasi et al.
//! (1967) critical heat flux (`outram_foam_multiphase::chf::Biasi`) and the
//! Dougall–Rohsenow (1963) post-dryout coefficient
//! (`outram_foam_multiphase::dryout::PostDryoutModel`). Their citations
//! and checks are in that crate; this session could not re-check either
//! against a source (the open CTF theory manual and AECL reports were not
//! reachable), so they carry that crate's verification status only.
//!
//! # Adaptations to a coil (stated, not hidden)
//!
//! - **Chen's convective term uses the coil correlation.** Chen (1966)
//!   built `F` on Dittus–Boelter for the liquid-alone flow. Here the
//!   liquid-alone coefficient is [`nusselt_coil`] at the liquid-alone
//!   Reynolds number, so the secondary-flow enhancement carries into the
//!   two-phase convective term. `F` and `S` are used unchanged.
//! - **Lockhart–Martinelli uses coil friction factors**, and its
//!   laminar/turbulent split for `C` is the coil's `Re_crit`, not the
//!   1000/2000 of the original straight-pipe work.
//! - Neither adaptation has been validated against helical-coil boiling
//!   data in this workspace (see the V&V record).

use petir::mathf::RealMath;

/// Lower Reynolds number of the fully turbulent coil heat-transfer
/// correlation (Gnielinski 1986; MSL 4.0.0 `kc_turbulent`, "Minimum
/// Reynolds number for turbulent regime").
pub const RE_TURBULENT_HEAT_TRANSFER: f64 = 2.2e4;

/// **Mean curvature diameter** of a helix \[m\]: `D_c = D_w [1 + (h/(π D_w))²]`,
/// with `D_w` the helix (coil) diameter at the tube centreline and `h` the
/// pitch (rise per turn). For `h → 0` it is `D_w` (a torus).
pub fn curvature_diameter(coil_diameter_m: f64, pitch_m: f64) -> f64 {
    let r = pitch_m / (std::f64::consts::PI * coil_diameter_m);
    coil_diameter_m * (1.0 + r * r)
}

/// **Schmidt (1967) critical Reynolds number** of a coiled tube,
/// `2300 [1 + 8.6 (d/D_c)^0.45]`. Tends to 2300 as the curvature ratio
/// `d/D_c → 0`. Recommended (by `fluids`, citing the Heat Exchanger Design
/// Handbook) for `d/D_c < 0.14`.
pub fn critical_reynolds_schmidt(curvature_ratio: f64) -> f64 {
    2300.0 * (1.0 + 8.6 * curvature_ratio.max(0.0).r_powf(0.45))
}

/// Dean number `De = Re √(d/D_c)`.
pub fn dean_number(reynolds: f64, curvature_ratio: f64) -> f64 {
    reynolds * curvature_ratio.max(0.0).sqrt()
}

/// **Laminar Darcy friction factor in a coil, Mishra & Gupta (1979)**:
/// `f = (64/Re) [1 + 0.033 (log₁₀ De)⁴]` for `De > 1`; `64/Re` (the straight
/// tube) for `De ≤ 1`, where the correction term is zero at `De = 1` so the
/// two meet continuously. The published range is `1 < De < 3000`; above it
/// the formula is extrapolated (not clamped).
pub fn darcy_friction_laminar_mishra_gupta(reynolds: f64, curvature_ratio: f64) -> f64 {
    let f_s = 64.0 / reynolds;
    let de = dean_number(reynolds, curvature_ratio);
    if de <= 1.0 {
        f_s
    } else {
        f_s * (1.0 + 0.033 * de.r_log10().r_powi(4))
    }
}

/// **Turbulent Darcy friction factor in a coil**,
/// `ξ = 0.3164 Re^-0.25 + 0.03 (d/D_c)^0.5` (Gnielinski 1986; the same as
/// Mishra & Gupta's Fanning form ×4). Blasius at `d/D_c = 0`.
pub fn darcy_friction_turbulent_coil(reynolds: f64, curvature_ratio: f64) -> f64 {
    0.3164 * reynolds.r_powf(-0.25) + 0.03 * curvature_ratio.max(0.0).sqrt()
}

/// **Darcy friction factor of a smooth coil** at any Reynolds number:
/// [`darcy_friction_laminar_mishra_gupta`] up to [`critical_reynolds_schmidt`],
/// [`darcy_friction_turbulent_coil`] above. There is no blend, so the factor
/// jumps at `Re_crit` (+5.0 % at `d/D_c = 0.1`, measured in the V&V record). `reynolds` must be positive.
pub fn darcy_friction_coil(reynolds: f64, curvature_ratio: f64) -> f64 {
    if reynolds <= critical_reynolds_schmidt(curvature_ratio) {
        darcy_friction_laminar_mishra_gupta(reynolds, curvature_ratio)
    } else {
        darcy_friction_turbulent_coil(reynolds, curvature_ratio)
    }
}

/// **Laminar coil Nusselt number, Gnielinski (1986)**:
/// `3.66 + 0.08 [1 + 0.8 (d/D_c)^0.9] Re^m Pr^(1/3)`,
/// `m = 0.5 + 0.2903 (d/D_c)^0.194`.
///
/// **It does not tend to the straight-tube 3.66 as `d/D_c → 0`**: the limit
/// is `3.66 + 0.08 Re^0.5 Pr^(1/3)`. The V&V record measures the gap.
pub fn nusselt_laminar_gnielinski(reynolds: f64, prandtl: f64, curvature_ratio: f64) -> f64 {
    let r = curvature_ratio.max(0.0);
    let m = 0.5 + 0.2903 * r.r_powf(0.194);
    3.66 + 0.08 * (1.0 + 0.8 * r.r_powf(0.9)) * reynolds.max(0.0).r_powf(m) * prandtl.r_cbrt()
}

/// **Turbulent coil Nusselt number, Gnielinski (1986)**:
/// `(ξ/8) Re Pr / [1 + 12.7 √(ξ/8) (Pr^(2/3) − 1)]` with
/// `ξ` = [`darcy_friction_turbulent_coil`].
///
/// At `d/D_c = 0` it is the Petukhov form with the Blasius factor, **not**
/// the straight-tube Gnielinski (1976) form, which carries `Re − 1000` in
/// place of `Re` (measured in the V&V record).
pub fn nusselt_turbulent_gnielinski(reynolds: f64, prandtl: f64, curvature_ratio: f64) -> f64 {
    let xi8 = darcy_friction_turbulent_coil(reynolds, curvature_ratio) / 8.0;
    xi8 * reynolds * prandtl / (1.0 + 12.7 * xi8.sqrt() * (prandtl.r_powf(2.0 / 3.0) - 1.0))
}

/// **Coil Nusselt number at any Reynolds number**: laminar up to
/// `Re_crit`, turbulent from [`RE_TURBULENT_HEAT_TRANSFER`], and between them
/// `Nu = γ Nu_lam(Re_crit) + (1 − γ) Nu_turb(2.2e4)` with
/// `γ = (2.2e4 − Re)/(2.2e4 − Re_crit)`. **[U]**: that linear form is
/// unverified (see the module table). If `Re_crit ≥ 2.2e4` (only for
/// `d/D_c` above about 0.85, far outside any coil), the turbulent form is
/// used above `Re_crit`.
pub fn nusselt_coil(reynolds: f64, prandtl: f64, curvature_ratio: f64) -> f64 {
    let re_crit = critical_reynolds_schmidt(curvature_ratio);
    if reynolds <= re_crit {
        nusselt_laminar_gnielinski(reynolds, prandtl, curvature_ratio)
    } else if reynolds >= RE_TURBULENT_HEAT_TRANSFER || re_crit >= RE_TURBULENT_HEAT_TRANSFER {
        nusselt_turbulent_gnielinski(reynolds, prandtl, curvature_ratio)
    } else {
        let gamma = (RE_TURBULENT_HEAT_TRANSFER - reynolds) / (RE_TURBULENT_HEAT_TRANSFER - re_crit);
        gamma * nusselt_laminar_gnielinski(re_crit, prandtl, curvature_ratio)
            + (1.0 - gamma)
                * nusselt_turbulent_gnielinski(RE_TURBULENT_HEAT_TRANSFER, prandtl, curvature_ratio)
    }
}

/// **Martinelli parameter, turbulent–turbulent**:
/// `X_tt = ((1−x)/x)^0.9 (ρ_g/ρ_l)^0.5 (μ_l/μ_g)^0.1`. `x` must lie in
/// `(0, 1)`; at `x → 0` it diverges and at `x → 1` it vanishes.
pub fn martinelli_xtt(quality: f64, rho_l: f64, rho_g: f64, mu_l: f64, mu_g: f64) -> f64 {
    ((1.0 - quality) / quality).r_powf(0.9) * (rho_g / rho_l).sqrt() * (mu_l / mu_g).r_powf(0.1)
}

/// **Chen's convective enhancement factor**, Edelstein et al. (1984) form:
/// `F = (1 + X_tt^-0.5)^1.78`. `F → 1` as `X_tt → ∞` (all liquid).
pub fn chen_f(xtt: f64) -> f64 {
    if xtt.is_infinite() {
        return 1.0;
    }
    (1.0 + 1.0 / xtt.sqrt()).r_powf(1.78)
}

/// **Chen's nucleate-boiling suppression factor**, Edelstein et al. (1984)
/// form: `S = 0.9622 − 0.5822 atan(Re_tp / 6.18e4)` with
/// `Re_tp = Re_l F^1.25`. Lies between 0.9622 (`Re_tp = 0`) and 0.0477.
pub fn chen_s(two_phase_reynolds: f64) -> f64 {
    0.9622 - 0.5822 * (two_phase_reynolds / 6.18e4).r_atan()
}

/// **Forster & Zuber (1955) nucleate-boiling coefficient** \[W/(m²·K)\],
/// strict SI in and out:
/// `0.00122 [k_l^0.79 c_p,l^0.45 ρ_l^0.49 / (σ^0.5 μ_l^0.29 h_fg^0.24 ρ_g^0.24)] ΔT_sat^0.24 Δp_sat^0.75`,
/// with `ΔT_sat = T_w − T_sat` \[K\] and `Δp_sat = p_sat(T_w) − p` \[Pa\].
/// Zero when either difference is not positive (no superheated wall, no
/// nucleation).
#[allow(clippy::too_many_arguments)]
pub fn forster_zuber_htc(
    k_l: f64,
    cp_l: f64,
    rho_l: f64,
    sigma: f64,
    mu_l: f64,
    h_fg: f64,
    rho_g: f64,
    wall_superheat_k: f64,
    dp_sat_pa: f64,
) -> f64 {
    if wall_superheat_k <= 0.0 || dp_sat_pa <= 0.0 {
        return 0.0;
    }
    0.00122
        * (k_l.r_powf(0.79) * cp_l.r_powf(0.45) * rho_l.r_powf(0.49)
            / (sigma.sqrt() * mu_l.r_powf(0.29) * h_fg.r_powf(0.24) * rho_g.r_powf(0.24)))
        * wall_superheat_k.r_powf(0.24)
        * dp_sat_pa.r_powf(0.75)
}

/// **Chisholm's constant** for the Lockhart–Martinelli multiplier:
/// 20 (liquid turbulent, gas turbulent), 12 (laminar, turbulent),
/// 10 (turbulent, laminar), 5 (laminar, laminar).
pub fn chisholm_c(liquid_turbulent: bool, gas_turbulent: bool) -> f64 {
    match (liquid_turbulent, gas_turbulent) {
        (true, true) => 20.0,
        (false, true) => 12.0,
        (true, false) => 10.0,
        (false, false) => 5.0,
    }
}

/// **Lockhart–Martinelli two-phase frictional gradient**, Chisholm (1967)
/// form, from the liquid-alone and gas-alone gradients `dp_l`, `dp_g`
/// (any consistent unit, e.g. Pa/m): `φ_l² dp_l = dp_l + C √(dp_l dp_g) + dp_g`.
/// Written without dividing by `X`, so it is finite and continuous at
/// `x = 0` (`= dp_l`) and `x = 1` (`= dp_g`).
pub fn two_phase_friction_gradient(dp_l: f64, dp_g: f64, c: f64) -> f64 {
    dp_l + c * (dp_l * dp_g).max(0.0).sqrt() + dp_g
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `fluids`' doctest: `helical_transition_Re_Schmidt(1, 7.) = 10540.094061770815`.
    #[test]
    fn schmidt_critical_reynolds_matches_the_fluids_library() {
        let re = critical_reynolds_schmidt(1.0 / 7.0);
        assert!((re - 10540.094061770815).abs() < 1e-9 * re, "{re}");
        assert_eq!(critical_reynolds_schmidt(0.0), 2300.0);
    }

    /// `ht`'s doctest (example from Cao, *Heat Transfer in Process
    /// Engineering*, 2009): water, `ΔT = 4.3 K`, `Δp = 3906·4.3 Pa`,
    /// `h = 3519.9239897462644 W/(m²K)`.
    #[test]
    fn forster_zuber_matches_the_ht_library() {
        let h = forster_zuber_htc(0.688, 4180.0, 958.0, 0.0588, 0.275e-3, 2.25e6, 0.597, 4.3, 3906.0 * 4.3);
        assert!((h - 3519.9239897462644).abs() < 1e-9 * h, "{h}");
        assert_eq!(forster_zuber_htc(0.688, 4180.0, 958.0, 0.0588, 0.275e-3, 2.25e6, 0.597, -1.0, 1.0), 0.0);
    }

    /// `ht`'s `Chen_Edelstein` doctest, rebuilt from this file's parts with
    /// the Dittus–Boelter liquid term `ht` uses (0.023, heating):
    /// `3289.058731974052 W/(m²K)`.
    #[test]
    fn chen_edelstein_matches_the_ht_library() {
        let (m, x, d) = (0.106, 0.2, 0.0212);
        let (rhol, rhog, mul, mug, kl, cpl, hvap, sigma) =
            (567.0, 18.09, 156e-6, 7.11e-6, 0.086, 2730.0, 2e5, 0.02);
        let g = m / (std::f64::consts::PI / 4.0 * d * d);
        let re_l = d * g * (1.0 - x) / mul;
        let pr_l = cpl * mul / kl;
        let h_l = 0.023 * re_l.r_powf(0.8) * pr_l.r_powf(0.4) * kl / d;
        let f = chen_f(martinelli_xtt(x, rhol, rhog, mul, mug));
        let s = chen_s(re_l * f.r_powf(1.25));
        let h_nb = forster_zuber_htc(kl, cpl, rhol, sigma, mul, hvap, rhog, 3.0, 1e5);
        let h = h_nb * s + h_l * f;
        assert!((h - 3289.058731974052).abs() < 1e-9 * h, "{h}");
        assert_eq!(chen_f(f64::INFINITY), 1.0);
    }

    /// `fluids`' `Lockhart_Martinelli` doctest (`716.469565488 Pa` over 1 m),
    /// rebuilt with that function's own straight-pipe friction factors
    /// (64/Re below 2000, 0.184 Re^-0.2 above) and this file's multiplier.
    #[test]
    fn lockhart_martinelli_matches_the_fluids_library() {
        let (m, x, rhol, rhog, mul, mug, d) = (0.6, 0.1, 915.0, 2.67, 180e-6, 14e-6, 0.05);
        let a = 0.25 * std::f64::consts::PI * d * d;
        let (v_l, v_g) = (m * (1.0 - x) / (rhol * a), m * x / (rhog * a));
        let (re_l, re_g) = (rhol * v_l * d / mul, rhog * v_g * d / mug);
        let fd = |re: f64| if re < 2000.0 { 64.0 / re } else { 0.184 * re.r_powf(-0.2) };
        let dp_l = fd(re_l) / d * 0.5 * rhol * v_l * v_l;
        let dp_g = fd(re_g) / d * 0.5 * rhog * v_g * v_g;
        let c = chisholm_c(re_l >= 2000.0, re_g >= 2000.0);
        let dp = two_phase_friction_gradient(dp_l, dp_g, c);
        // fluids prints 9 significant figures in its doctest.
        assert!((dp - 716.469565488).abs() < 1e-8 * dp, "{dp}");
        assert_eq!(two_phase_friction_gradient(3.0, 0.0, 20.0), 3.0);
        assert_eq!(two_phase_friction_gradient(0.0, 2.0, 20.0), 2.0);
    }

    /// The straight-tube limits the coil forms reach as `d/D_c → 0`
    /// (exact ones only; the two that do not reduce are measured in
    /// `super::tests::single_phase_straight_tube_limit`).
    #[test]
    fn coil_friction_tends_to_the_straight_tube() {
        for re in [500.0, 1500.0, 2200.0] {
            assert_eq!(darcy_friction_coil(re, 0.0), 64.0 / re);
        }
        for re in [1e4, 5e4, 1e5] {
            let f = darcy_friction_coil(re, 0.0);
            assert!((f - 0.3164 * re.r_powf(-0.25)).abs() < 1e-15);
        }
        // Mishra–Gupta is continuous at De = 1.
        let re = 1.0 / 0.01f64.sqrt();
        assert!((darcy_friction_laminar_mishra_gupta(re * (1.0 + 1e-9), 0.01) - 64.0 / re).abs() < 1e-6);
        // The coil raises friction and heat transfer.
        assert!(darcy_friction_coil(1500.0, 0.05) > 64.0 / 1500.0);
        assert!(nusselt_coil(4e4, 3.0, 0.05) > nusselt_coil(4e4, 3.0, 0.0));
    }

    /// The blended Nusselt number meets its two legs at the interval ends.
    #[test]
    fn coil_nusselt_is_continuous_at_both_ends_of_the_blend() {
        let r = 0.07;
        let re_c = critical_reynolds_schmidt(r);
        let below = nusselt_coil(re_c, 2.0, r);
        let above = nusselt_coil(re_c * (1.0 + 1e-12), 2.0, r);
        assert!((below - above).abs() < 1e-6 * below, "{below} {above}");
        let t1 = nusselt_coil(RE_TURBULENT_HEAT_TRANSFER * (1.0 - 1e-12), 2.0, r);
        let t2 = nusselt_coil(RE_TURBULENT_HEAT_TRANSFER, 2.0, r);
        assert!((t1 - t2).abs() < 1e-6 * t2, "{t1} {t2}");
        assert!((curvature_diameter(1.0, 0.0) - 1.0).abs() < 1e-15);
        assert!(chen_s(0.0) - 0.9622 < 1e-15 && chen_s(1e12) > 0.04);
        assert_eq!(chisholm_c(false, false), 5.0);
        assert_eq!(chisholm_c(true, false), 10.0);
    }
}
