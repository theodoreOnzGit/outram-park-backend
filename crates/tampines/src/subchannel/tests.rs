// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! V&V of the subchannel solver (gh:#1078). Research, education and V&V
//! only. AI draft, unreviewed. Predictions were written in each doc comment
//! **before** the first run; results were added after, dated 2026-11-01.
//! Record: `crates/tampines/verification_and_validation/subchannel_crossflow_mixing.md`.
//!
//! Common inputs: a PWR-type square-lattice interior subchannel, pitch
//! 12.6 mm, rod diameter 9.5 mm: `A = p² − πd²/4 = 8.788e-5 m²`,
//! `D_h = 4A/(πd) = 11.78 mm`, gap `s = p − d = 3.1 mm`, `l = p`. Water near
//! 300 K at low pressure, constant: `ρ = 997 kg/m³`, `c_p = 4180 J/(kg K)`,
//! `μ = 8.9e-4 Pa s` (round textbook values, not fitted). Reference mass
//! flux 3500 kg/(m² s), `ṁ_0 = 0.3076 kg/s`, `u_0 = 3.51 m/s`,
//! `Re ≈ 4.6e4`.

use petir::mathf::RealMath;
use uom::si::acceleration::meter_per_second_squared;
use uom::si::area::square_meter;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{
    Acceleration, Area, DynamicViscosity, Length, LinearPowerDensity, MassDensity, MassRate,
    Pressure, SpecificHeatCapacity, ThermodynamicTemperature,
};
use uom::si::length::meter;
use uom::si::linear_power_density::watt_per_meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::pressure::pascal;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

use super::*;
use crate::closures::wall_friction::{darcy_friction_factor, wall_friction_acceleration};

const PITCH: f64 = 0.0126;
const ROD_D: f64 = 0.0095;
const RHO: f64 = 997.0;
const CP: f64 = 4180.0;
const MU: f64 = 8.9e-4;
const G_REF: f64 = 3500.0;
const P_OUT: f64 = 1.0e5;

fn area() -> f64 {
    PITCH * PITCH - core::f64::consts::PI * ROD_D * ROD_D / 4.0
}
fn dh() -> f64 {
    4.0 * area() / (core::f64::consts::PI * ROD_D)
}
fn m0() -> f64 {
    G_REF * area()
}
fn water() -> FluidProperties {
    FluidProperties {
        density: MassDensity::new::<kilogram_per_cubic_meter>(RHO),
        specific_heat: SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(CP),
        viscosity: DynamicViscosity::new::<pascal_second>(MU),
    }
}
fn channel(mdot: f64, t_in: f64) -> Subchannel {
    Subchannel::new(
        Area::new::<square_meter>(area()),
        Length::new::<meter>(dh()),
        MassRate::new::<kilogram_per_second>(mdot),
        ThermodynamicTemperature::new::<kelvin>(t_in),
    )
}
fn wpm(q: f64) -> LinearPowerDensity {
    LinearPowerDensity::new::<watt_per_meter>(q)
}
fn gap(i: usize, j: usize, k: f64) -> Gap {
    Gap::new(
        i,
        j,
        Length::new::<meter>(PITCH - ROD_D),
        Length::new::<meter>(PITCH),
        k,
    )
}
fn problem(channels: Vec<Subchannel>, gaps: Vec<Gap>, length: f64, nz: usize) -> SubchannelProblem {
    SubchannelProblem::new(
        channels,
        gaps,
        water(),
        Length::new::<meter>(length),
        nz,
        Pressure::new::<pascal>(P_OUT),
        TYPICAL_MIXING_COEFFICIENT_BETA,
    )
}
fn mdot(s: &SubchannelSolution, i: usize, k: usize) -> f64 {
    s.mass_flow[i][k].get::<kilogram_per_second>()
}
fn temp(s: &SubchannelSolution, i: usize, k: usize) -> f64 {
    s.temperature[i][k].get::<kelvin>()
}
fn pres(s: &SubchannelSolution, i: usize, k: usize) -> f64 {
    s.pressure[i][k].get::<pascal>()
}
fn max_mass_defect(s: &SubchannelSolution) -> f64 {
    let m_in = s.total_mass_flow(0).get::<kilogram_per_second>();
    (0..s.z.len())
        .map(|k| ((s.total_mass_flow(k).get::<kilogram_per_second>() - m_in) / m_in).abs())
        .fold(0.0, f64::max)
}

/// **V&V 1: two identical channels with equal inlet.**
///
/// Methodology: two PWR-type subchannels (common inputs) joined by one gap
/// (`K = 0.5`), equal inlet flow `ṁ_0`, inlet 300 K, both heated at
/// 20 kW/m, `L = 2 m`, 50 cells, mixing on (`β = 0.02`), vertical upward
/// flow (gravity set explicitly to 9.80665 m/s²). Reference: the
/// single-channel hand result, `ΔP = f ρ u² L/(2D_h) + ρ g L` with `f` the
/// closure's Darcy factor at `Re = ρ u D_h/μ`, and the energy balance
/// `T_out = T_in + q' L/(ṁ c_p)`.
///
/// Prediction (written before the run): by symmetry no crossflow,
/// `|w| < 1e-12 w_ref`; `ΔP` and `T_out` match the hand values to `1e-12`
/// relative (ṁ is constant along the channel, so the cell-mean friction is
/// exact). Hand values: `f ≈ 0.0207`, `ΔP ≈ 21.6 kPa + 19.6 kPa ≈ 41 kPa`,
/// `ΔT ≈ 31.1 K`.
///
/// Result (2026-11-01, held): `f = 0.021271` at `Re = 46317`; `ΔP` =
/// 41745.078675 Pa in both channels against the hand 41745.078675 Pa
/// (relative −3.5e-16); `ΔT = 31.112607588 K` against the hand value
/// (relative 2.3e-15); `max|w|/w_ref = 5.2e-14`; Newton 2 iterations,
/// residual 3.7e-16.
#[test]
fn identical_channels_have_no_crossflow_and_match_the_hand_pressure_drop() {
    let (l, nz, q, t_in) = (2.0, 50, 20.0e3, 300.0);
    let chans = vec![
        channel(m0(), t_in).with_heat(AxialHeat::Uniform(wpm(q))),
        channel(m0(), t_in).with_heat(AxialHeat::Uniform(wpm(q))),
    ];
    let p = problem(chans, vec![gap(0, 1, 0.5)], l, nz)
        .with_gravity_along_flow(Acceleration::new::<meter_per_second_squared>(9.80665));
    let s = p.solve().expect("solve");

    let u = m0() / (RHO * area());
    let re = RHO * u * dh() / MU;
    let f = darcy_friction_factor(re, 0.0, 1.0);
    let dp_hand = f * RHO * u * u * l / (2.0 * dh()) + RHO * 9.80665 * l;
    let dt_hand = q * l / (m0() * CP);
    let w_ref = m0() / l;
    let w_max = s.crossflow[0]
        .iter()
        .map(|w| w.get::<pascal_second>().abs())
        .fold(0.0, f64::max);
    let dp0 = pres(&s, 0, 0) - pres(&s, 0, nz);
    let dp1 = pres(&s, 1, 0) - pres(&s, 1, nz);
    let dt0 = temp(&s, 0, nz) - t_in;
    eprintln!(
        "V&V1: f={f:.6} Re={re:.1} dp_hand={dp_hand:.6} dp0={dp0:.6} dp1={dp1:.6} rel={:.3e} \
         dt_hand={dt_hand:.9} dt={dt0:.9} rel={:.3e} |w|max/w_ref={:.3e} iters={} res={:.2e}",
        (dp0 - dp_hand) / dp_hand,
        (dt0 - dt_hand) / dt_hand,
        w_max / w_ref,
        s.newton_iterations,
        s.residual_inf_norm
    );
    assert!(w_max / w_ref < 1e-12);
    assert!(((dp0 - dp_hand) / dp_hand).abs() < 1e-12);
    assert!(((dp1 - dp_hand) / dp_hand).abs() < 1e-12);
    assert!(((dt0 - dt_hand) / dt_hand).abs() < 1e-12);
    assert!((mdot(&s, 0, nz) - m0()).abs() / m0() < 1e-12);
}

/// Real roots of `r³ − a r − b = 0` (three real roots assumed), by Newton
/// from three starting points, ascending.
fn cubic_roots(a: f64, b: f64) -> [f64; 3] {
    let newton = |mut r: f64| {
        for _ in 0..200 {
            let f = r * r * r - a * r - b;
            let d = 3.0 * r * r - a;
            r -= f / d;
        }
        r
    };
    let sa = a.sqrt();
    let mut r = [newton(-2.0 * sa), newton(-b / a), newton(2.0 * sa)];
    r.sort_by(|x, y| x.partial_cmp(y).unwrap());
    r
}

/// Outcome of one V&V 2 run against the linear analysis.
struct RedistributionCheck {
    r_slow: f64,
    measured_rate: f64,
    worst_profile_miss: f64,
    mass_defect: f64,
    outlet_split: f64,
}

/// One V&V 2 run at turbulent-momentum factor `f_t`, compared with the
/// three-mode linear analysis (doc of the test below) whose friction slope is
/// `c_eff = c + 2 f_T w'/(ρA²)`.
fn redistribution_case(f_t: f64) -> RedistributionCheck {
    let (l, nz, eps) = (3.0, 300, 0.02);
    let chans = vec![channel(m0() * (1.0 + eps), 300.0), channel(m0() * (1.0 - eps), 300.0)];
    let s = problem(chans, vec![gap(0, 1, 0.5)], l, nz)
        .with_momentum_mixing_factor(f_t)
        .solve()
        .expect("solve");
    let mass = max_mass_defect(&s);

    // Linear analysis.
    let (a, sg, lg) = (area(), PITCH - ROD_D, PITCH);
    let u0 = m0() / (RHO * a);
    let fgrad = |m: f64| RHO * wall_friction_acceleration(m / (RHO * a), RHO, MU, dh(), 0.0, 1.0);
    let dm = 1e-4 * m0();
    let c = (fgrad(m0() + dm) - fgrad(m0() - dm)) / (2.0 * dm);
    let wp = TYPICAL_MIXING_COEFFICIENT_BETA * sg * G_REF;
    let c_eff = c + 2.0 * f_t * wp / (RHO * a * a);
    let ca = 2.0 * sg / (lg * a);
    let cb = 2.0 * c_eff * sg / (lg * u0);
    let r = cubic_roots(ca, cb);
    // Modes: e^{r0 z}, e^{r1 z}, e^{r2 (z − L)}; r1 is the slow root.
    let r_slow = r[1];
    // Rows: δ(0), δ'(0), δ''(L).
    let e2 = (-r[2] * l).r_exp();
    let mtx = [
        [1.0, 1.0, e2],
        [r[0], r[1], r[2] * e2],
        [r[0] * r[0] * (r[0] * l).r_exp(), r[1] * r[1] * (r[1] * l).r_exp(), r[2] * r[2]],
    ];
    let rhs = [eps * m0(), 0.0, 0.0];
    let det3 = |m: &[[f64; 3]; 3]| {
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    };
    let d = det3(&mtx);
    let mut coef = [0.0; 3];
    for (col, cf) in coef.iter_mut().enumerate() {
        let mut mm = mtx;
        for row in 0..3 {
            mm[row][col] = rhs[row];
        }
        *cf = det3(&mm) / d;
    }
    let delta_an = |z: f64| {
        coef[0] * (r[0] * z).r_exp() + coef[1] * (r[1] * z).r_exp() + coef[2] * (r[2] * (z - l)).r_exp()
    };
    let delta = |k: usize| 0.5 * (mdot(&s, 0, k) - mdot(&s, 1, k));
    let dz = l / nz as f64;
    let node = |z: f64| (z / dz).round() as usize;
    let (ka, kb) = (node(0.5), node(2.0));
    let rate = (delta(kb) / delta(ka)).r_ln() / (2.0 - 0.5);
    eprintln!(
        "V&V2 f_T={f_t}: c={c:.4e} c_eff={c_eff:.4e} roots={r:?} r_slow={r_slow:.6} measured={rate:.6} \
         rel={:.3e} mass_defect={mass:.3e} outlet_split={:.4e} analytic_outlet={:.4e} iters={} res={:.2e}",
        rate / r_slow - 1.0,
        delta(nz) / delta(0),
        delta_an(l) / delta_an(0.0),
        s.newton_iterations,
        s.residual_inf_norm
    );
    let mut worst: f64 = 0.0;
    for z in [0.25, 0.5, 1.0, 1.5, 2.0] {
        let k = node(z);
        let rel = delta(k) / delta_an(z) - 1.0;
        worst = worst.max(rel.abs());
        eprintln!(
            "V&V2 f_T={f_t}: z={z} delta/delta0={:.6} analytic={:.6} rel={rel:.3e}",
            delta(k) / delta(0),
            delta_an(z) / delta_an(0.0)
        );
    }
    RedistributionCheck {
        r_slow,
        measured_rate: rate,
        worst_profile_miss: worst,
        mass_defect: mass,
        outlet_split: delta(nz) / delta(0),
    }
}

/// **V&V 2: unequal inlet flow, no heating — crossflow redistribution.**
///
/// Methodology: two identical channels, one gap (`K = 0.5`, an
/// order-of-magnitude rod-gap value; the linear analysis below shows it does
/// not enter at first order), inlet flows `ṁ_0(1 ± ε)` with `ε = 0.02`,
/// unheated, `L = 3 m`, 300 cells, mixing on (`β = 0.02`). With
/// `δ = (ṁ_1 − ṁ_2)/2`, linearising the model's own continuum equations about
/// `ṁ_0` (`u* ≈ u_0`; the `K w|w|` term is second order in `δ`; `c = dF/dṁ`
/// is the slope of the friction pressure gradient `F = ρ a_f`, by central
/// difference of the closure) gives
///
/// ```text
/// u_0 δ''' − (2 u_0 s/(l A)) δ' − (2 c_eff s/l) δ = 0,
/// δ(0) = ε ṁ_0,  δ'(0) = −w(0) = 0,  δ''(L) ∝ Δp(L) = 0,
/// ```
///
/// three real exponential modes: a fast inlet mode and a fast outlet mode
/// (`|r| ≈ √(2s/(lA)) ≈ 75 m⁻¹`) and one slow redistribution mode
/// `r_slow ≈ −c_eff A/u_0` (acceleration balancing the friction difference
/// at equal pressure). The analytic `δ(z)` is the three-mode sum fitted to the
/// three boundary conditions.
///
/// Prediction (written before the first run): total mass conserved at every
/// node to `1e-12` relative; slow decay rate measured between `z = 0.5` and
/// `2.0 m` within 3 % of `r_slow`; `δ(z)` within 3 % of the analytic profile
/// at `z = 0.25, 0.5, 1, 1.5, 2 m`; outlet split `|δ(L)/δ(0)| < 0.02`.
///
/// **MISS of the first prediction, and its cause (2026-11-01).** The first
/// analysis used `c_eff = c` (6.40e4 Pa m⁻¹ per kg/s), calling the mixing
/// terms second order. They are not: `w' = β s Ḡ` is O(1), so the momentum
/// exchange `f_T w'(u_1 − u_2)` is **first order** in `δ` and adds
/// `2 f_T w'/(ρA²) = 5.64e4` to `c`. The first run measured `r = −3.023 m⁻¹`
/// against the predicted `−1.604 m⁻¹` (+88 %); the profile missed by 30–94 %.
/// No model input or tolerance was changed. The analysis was corrected
/// (`c_eff = c + 2 f_T w'/(ρA²)`), and the correction was made falsifiable:
/// with `f_T = 0` the rate must fall back to the original `−1.604 m⁻¹`.
///
/// Result (2026-11-01, after the correction; held for both `f_T`):
///
/// | `f_T` | `r_slow` (m⁻¹) | measured | rel | profile miss, z = 0.25 … 2 m | outlet split (analytic) |
/// |---|---|---|---|---|---|
/// | 1 (default) | −3.0192 | −3.0225 | +1.1e-3 | −1.6 % … −2.6 % | 1.18e-4 (1.21e-4) |
/// | 0 (ablated) | −1.6040 | −1.6095 | +3.5e-3 | −1.0 % … −2.2 % | 8.12e-3 (8.31e-3) |
///
/// Mass defect 1.8e-16 at every node in both runs; Newton 3 iterations,
/// residual ≤ 7.3e-16. The solver's `δ` always lags the linear profile by
/// 1–2.6 %, a near-constant offset set in the inlet region (where `w` peaks
/// and the second-order terms are largest). It sits inside the 3 % bound but
/// uses most of it.
#[test]
fn unequal_inlet_flow_redistributes_by_crossflow_at_the_linear_rate() {
    for f_t in [DEFAULT_MOMENTUM_MIXING_FACTOR, 0.0] {
        let c = redistribution_case(f_t);
        assert!(c.mass_defect < 1e-12, "mass defect {:e}", c.mass_defect);
        assert!((c.measured_rate / c.r_slow - 1.0).abs() < 0.03, "f_T={f_t}");
        assert!(c.worst_profile_miss < 0.03, "f_T={f_t}: worst {:e}", c.worst_profile_miss);
        assert!(c.outlet_split.abs() < 0.02);
    }
}

/// The two-channel mixing case of V&V 3: heated first half (channel 0 only),
/// unheated second half.
fn mixing_case(nz: usize, l: f64, q: f64) -> SubchannelProblem {
    let half = nz / 2;
    let heat0: Vec<LinearPowerDensity> = (0..nz).map(|k| wpm(if k < half { q } else { 0.0 })).collect();
    let chans = vec![
        channel(m0(), 300.0).with_heat(AxialHeat::PerCell(heat0)),
        channel(m0(), 300.0),
    ];
    problem(chans, vec![gap(0, 1, 0.5)], l, nz)
}

/// **V&V 3: mixing only — equal flow and pressure, unequal heating.**
///
/// Methodology: two identical channels at equal inlet flow and temperature;
/// channel 0 heated at 20 kW/m over the first half (`0 ≤ z < 2 m`), nothing
/// else heated, `L = 4 m`, 200 cells, `β = 0.02`. Constant properties make
/// the hydraulics independent of temperature, so the only exchange is
/// turbulent mixing. Analytic: `ṁ c_p ΔT' = q'_0 − 2 w' c_p ΔT`, `w' = β s
/// G_0`, so `ΔT(z) = ΔT_∞ (1 − e^{−λz})` in the heated half (`ΔT_∞ =
/// q'/(2 w' c_p)`) and `ΔT ∝ e^{−λz}` in the unheated half, with
/// `λ = 2 β s G_0/ṁ_0 = 2 β s/A = 1.411 m⁻¹`.
///
/// Prediction (written before the run): no crossflow (`|w| < 1e-12 w_ref`);
/// the decay rate measured over the unheated half within `1e-4` of `λ`
/// (the cell-average scheme's per-cell factor `(1 − λΔz/2)/(1 + λΔz/2)`
/// over-predicts the rate by `(λΔz)²/12 ≈ 6.6e-5`); `ΔT(2 m)` within `1e-4`
/// of the continuous analytic value.
///
/// Result (2026-11-01, held): `λ = 1.41105009 m⁻¹`, measured 1.41114375
/// (relative +6.64e-5, the predicted `(λΔz)²/12` = 6.6e-5; against the
/// discrete closed form, 1.6e-14); `ΔT(2 m) = 10.36899709 K` against the
/// analytic 10.36887427 K (+1.2e-5); `ΔT(L) = 0.6166 K`;
/// `max|w|/w_ref = 8.4e-22`; Newton 2 iterations.
#[test]
fn turbulent_mixing_decays_the_temperature_difference_at_the_analytic_rate() {
    let (nz, l, q) = (200, 4.0, 20.0e3);
    let s = mixing_case(nz, l, q).solve().expect("solve");
    let lambda = 2.0 * TYPICAL_MIXING_COEFFICIENT_BETA * (PITCH - ROD_D) / area();
    let wp = TYPICAL_MIXING_COEFFICIENT_BETA * (PITCH - ROD_D) * G_REF;
    let dt_inf = q / (2.0 * wp * CP);
    let half = nz / 2;
    let dtk = |k: usize| temp(&s, 0, k) - temp(&s, 1, k);
    let dt_half_an = dt_inf * (1.0 - (-lambda * 2.0).r_exp());
    let measured = (dtk(half) / dtk(nz)).r_ln() / (l - 2.0);
    let dz = l / nz as f64;
    let discrete = (2.0 / dz) * ((1.0 + lambda * dz / 2.0) / (1.0 - lambda * dz / 2.0)).r_ln() / 2.0;
    let w_max = s.crossflow[0]
        .iter()
        .map(|w| w.get::<pascal_second>().abs())
        .fold(0.0, f64::max);
    eprintln!(
        "V&V3: lambda={lambda:.8} measured={measured:.8} rel={:.3e} discrete_rel={:.3e} \
         dT(2m)={:.8} analytic={dt_half_an:.8} rel={:.3e} dT(L)={:.6} |w|max/w_ref={:.3e} iters={}",
        measured / lambda - 1.0,
        measured / discrete - 1.0,
        dtk(half),
        dtk(half) / dt_half_an - 1.0,
        dtk(nz),
        w_max / (m0() / l),
        s.newton_iterations
    );
    assert!(w_max / (m0() / l) < 1e-12);
    assert!((measured / lambda - 1.0).abs() < 1e-4);
    assert!((dtk(half) / dt_half_an - 1.0).abs() < 1e-4);
}

/// **Ablation of V&V 3, and the mixing-on default pinned.** `new` carries
/// mixing with the caller's `β` (correct physics is the default);
/// `without_turbulent_mixing()` removes it, and then the heated channel's
/// rise is the plain energy balance `q' (L/2)/(ṁ c_p)` and the unheated one
/// stays at its inlet temperature, both to `1e-12`.
#[test]
fn mixing_is_on_by_default_and_its_ablation_leaves_the_channels_unmixed() {
    let (nz, l, q) = (40, 4.0, 20.0e3);
    let p = mixing_case(nz, l, q);
    let m = p.mixing().expect("mixing on by default");
    assert_eq!(m.beta, TYPICAL_MIXING_COEFFICIENT_BETA);
    assert_eq!(m.momentum_factor, DEFAULT_MOMENTUM_MIXING_FACTOR);
    let p = p.without_turbulent_mixing();
    assert!(p.mixing().is_none());
    // f_T has nothing to act on once mixing is off.
    let p = p.with_momentum_mixing_factor(0.5);
    assert!(p.mixing().is_none());
    let s = p.solve().expect("solve");
    let rise = q * (l / 2.0) / (m0() * CP);
    assert!(((temp(&s, 0, nz) - 300.0) / rise - 1.0).abs() < 1e-12);
    assert!((temp(&s, 1, nz) - 300.0).abs() < 1e-12 * rise);
    assert!(s.turbulent_mixing[0].iter().all(|w| w.get::<pascal_second>() == 0.0));
}

/// **V&V 4: global energy and mass balance with crossflow and mixing.**
///
/// Methodology: three channels in a row (gaps 0–1, 1–2, `K = 0.5`), inlet
/// flows `ṁ_0 × (1.1, 1.0, 0.85)`, inlet temperatures 300, 305, 295 K,
/// uniform heating 25, 15, 5 kW/m, `L = 2 m`, 100 cells, rough walls
/// (`ε = 1.5 μm`), mixing on with `f_T = 0.8` (a non-default value, to reach
/// the setter). Reference: `Σ ṁ_out c_p T_out − Σ ṁ_in c_p T_in = Σ q' L`
/// and `Σ ṁ` constant along the bundle.
///
/// Prediction (written before the run): both balances close to `1e-11`
/// relative (the crossflow and mixing terms cancel pairwise; what is left is
/// the Newton residual), and the crossflow is not zero.
///
/// Result (2026-11-01, held): energy balance relative error 0.0 (exactly
/// zero in `f64` on this run), mass defect 1.2e-16, `max|w| = 0.0856
/// kg/(m s)`; outlet flows `ṁ/ṁ_0 = 0.98472, 0.98335, 0.98193` (from 1.1,
/// 1.0, 0.85: nearly equalised), outlet temperatures 332.59, 324.07,
/// 315.27 K; Newton 4 iterations, residual 6.5e-16.
#[test]
fn three_channel_bundle_conserves_energy_and_mass() {
    let (l, nz) = (2.0, 100);
    let flows = [1.1, 1.0, 0.85];
    let tins = [300.0, 305.0, 295.0];
    let qs = [25.0e3, 15.0e3, 5.0e3];
    let chans: Vec<Subchannel> = (0..3)
        .map(|i| {
            channel(m0() * flows[i], tins[i])
                .with_heat(AxialHeat::Uniform(wpm(qs[i])))
                .with_roughness(Length::new::<meter>(1.5e-6))
                .with_shape_factor(1.0)
        })
        .collect();
    let s = problem(chans, vec![gap(0, 1, 0.5), gap(1, 2, 0.5)], l, nz)
        .with_momentum_mixing_factor(0.8)
        .solve()
        .expect("solve");
    let e_in: f64 = (0..3).map(|i| CP * mdot(&s, i, 0) * temp(&s, i, 0)).sum();
    let e_out: f64 = (0..3).map(|i| CP * mdot(&s, i, nz) * temp(&s, i, nz)).sum();
    let q_tot: f64 = qs.iter().map(|q| q * l).sum();
    let energy = (e_out - e_in - q_tot) / q_tot;
    let mass = max_mass_defect(&s);
    let w_max = s
        .crossflow
        .iter()
        .flat_map(|g| g.iter().map(|w| w.get::<pascal_second>().abs()))
        .fold(0.0, f64::max);
    eprintln!(
        "V&V4: energy_rel={energy:.3e} mass_defect={mass:.3e} |w|max={w_max:.4e} kg/(m s) \
         outlet flows=({:.6},{:.6},{:.6}) outlet T=({:.4},{:.4},{:.4}) iters={} res={:.2e}",
        mdot(&s, 0, nz) / m0(),
        mdot(&s, 1, nz) / m0(),
        mdot(&s, 2, nz) / m0(),
        temp(&s, 0, nz),
        temp(&s, 1, nz),
        temp(&s, 2, nz),
        s.newton_iterations,
        s.residual_inf_norm
    );
    assert!(energy.abs() < 1e-11);
    assert!(mass < 1e-11);
    assert!(w_max > 1e-3);
}

/// Every refusal in the input check is reached, and none panics.
#[test]
fn unusable_problems_are_refused() {
    let ok = || vec![channel(m0(), 300.0), channel(m0(), 300.0)];
    let bad = |p: SubchannelProblem| matches!(p.solve(), Err(crate::TampinesError::InvalidInput(_)));
    assert!(bad(problem(vec![], vec![], 1.0, 4)));
    assert!(bad(problem(ok(), vec![], 1.0, 0)));
    assert!(bad(problem(ok(), vec![], -1.0, 4)));
    assert!(bad(problem(vec![channel(-1.0, 300.0)], vec![], 1.0, 4)));
    assert!(bad(problem(
        vec![channel(m0(), 300.0).with_roughness(Length::new::<meter>(-1.0))],
        vec![],
        1.0,
        4
    )));
    assert!(bad(problem(
        vec![channel(m0(), 300.0).with_heat(AxialHeat::PerCell(vec![wpm(1.0); 3]))],
        vec![],
        1.0,
        4
    )));
    assert!(bad(problem(
        vec![channel(m0(), 300.0).with_heat(AxialHeat::Uniform(wpm(f64::NAN)))],
        vec![],
        1.0,
        4
    )));
    assert!(bad(problem(ok(), vec![gap(0, 2, 0.5)], 1.0, 4)));
    assert!(bad(problem(ok(), vec![gap(1, 1, 0.5)], 1.0, 4)));
    assert!(bad(problem(ok(), vec![gap(0, 1, -0.5)], 1.0, 4)));
    let mut g = gap(0, 1, 0.5);
    g.width = Length::new::<meter>(0.0);
    assert!(bad(problem(ok(), vec![g], 1.0, 4)));
    assert!(bad(SubchannelProblem::new(
        ok(),
        vec![gap(0, 1, 0.5)],
        water(),
        Length::new::<meter>(1.0),
        4,
        Pressure::new::<pascal>(P_OUT),
        -0.1
    )));
    assert!(bad(problem(ok(), vec![gap(0, 1, 0.5)], 1.0, 4).with_momentum_mixing_factor(f64::NAN)));
    assert!(bad(problem(ok(), vec![], 1.0, 4)
        .with_gravity_along_flow(Acceleration::new::<meter_per_second_squared>(f64::INFINITY))));
}
