//! HEM reference curves digitised from Figure 2 of Zaloudek, as reproduced in:
//! Saha, P. (1978). A review of two-phase steam-water critical flow models
//! with emphasis on thermal nonequilibrium. NUREG/CR-0417, BNL-NUREG-50907.
//! Brookhaven National Laboratory, Upton, New York.
//! https://www.nrc.gov/docs/ML1925/ML19256F779.pdf
//!
//! ⚠ These are NOT experimental measurements. They are Homogeneous Equilibrium
//! Model (HEM) computed curves that Zaloudek published and Saha reproduced in
//! graphical form. The values here were graph-read (digitised) from those curves.
//! Expect loose agreement at the bubble-point edge (x_t ≈ 0) where HEM is known
//! to diverge from reality; that is a physics limitation, not a code bug.
//!
//! Data format: (critical_pressure_psia, critical_mass_flux_lb_per_s_per_ft2,
//!               stagnation_enthalpy_btu_per_lb)
//! Critical pressures: 5, 10, 15, 20, 30, 50, 75, 100, 150, 200, 300, 500,
//!                     750, 1000, 1500, 2000, 3000 psia

/// For this test suite, it uses Zaloudek HEM reference curves (digitised from
/// Saha 1978 NUREG/CR-0417 — NOT raw experimental data).
/// Given the throat pressure and enthalpy, it should guess the stagnation
/// enthalpy correctly.
pub mod backward_throat_to_stagnation;
/// this contains a test suite for stagnation (p0,h0) points within
/// the vapour liquid equilibrium (VLE)
///
/// this VLE looks like a dome on the (p,h) diagram for the steam
/// tables.
///
/// Hence this test suite is called in dome stagnation
pub mod in_dome_stagnation;
pub mod generic_multiphase_stagnation;

/// exploratory test suite for subcooled-liquid / liquid-like stagnation
/// points (OUTSIDE the dome, left side), validating the subcooled solver
pub mod outside_dome_stagnation_subcooled;

/// exploratory test suite for superheated-vapour / supercritical (vapour-like)
/// stagnation points (OUTSIDE the dome, right side / above the dome),
/// validating the superheated-vapour solver
pub mod outside_dome_stagnation_superheated;

/// Stagnation state `(p0, h0, G_t)` of an HEM throat **on the saturated-liquid
/// line** (`x_t = 0`) at pressure `p_t`, built so that the throat is the
/// maximum of `G` along the stagnation state's own isentrope (gh:#1111,
/// 2026-11-02). This defines the fixture for Zaloudek's saturated-liquid
/// (x ≈ 0) curve.
///
/// - `s0 = s_1(T_sat(p_t), p_t)`, so the isentrope reaches saturation exactly at
///   `p_t`.
/// - The throat velocity is the HEM two-phase sound speed at `x = 0⁺`:
///   `G_t² = −1/(∂v/∂p)_s`, with the derivative taken one-sided on the dome
///   side. It uses `v(p) = v_f + x (v_g − v_f)` and
///   `x = (s0 − s_f)/(s_g − s_f)`, from forward equations only, with a
///   second-order one-sided difference, `δ = 1e-6 p_t`.
/// - `h0 = h_f + u_t²/2` (energy), so `u_b = c_2φ(0⁺)` and `dG/dp → 0` from
///   below at `p_t`: the kink is the maximum.
/// - `(p0, T0)` by 2D Newton on `s_1(T, p) = s0`, `h_1(T, p) = h0` to
///   `|ΔT| ≤ 1e-12 T`, `|Δp| ≤ 1e-10 p` (forward equations; `∂s/∂T = c_p/T`, `∂s/∂p = −v α_v`, `∂h/∂T = c_p`,
///   `∂h/∂p = v (1 − T α_v)`).
///
/// **Why the old fixture was replaced.** It put the throat at `x_t = 1e-4`
/// and used `mass_flux_ps_eqm_throat` there. At low pressure `x = 1e-4` is far
/// from the saturated-liquid line (`v_g/v_f` = 4472 at 5 psia, so
/// `ρ_t ≈ 0.69 ρ_f`). That throat was therefore not the maximum of `G` along
/// its own isentrope, and an energy-consistent solver correctly chokes at the
/// bubble point instead. Its numbers are recorded at the test.
///
/// Returns `None` when `T_sat(p_t) > 623.15 K` (saturated liquid in Region 3)
/// or when Newton does not converge; the caller then keeps the old fixture.
pub(super) fn hem_stagnation_for_saturated_liquid_throat(
    p_t: uom::si::f64::Pressure,
) -> Option<(uom::si::f64::Pressure, uom::si::f64::AvailableEnergy, uom::si::f64::MassFlux)> {
    use crate::region_1_subcooled_liquid::{alpha_v_tp_1, cp_tp_1, h_tp_1, s_tp_1, v_tp_1};
    use crate::region_2_vapour::{s_tp_2, v_tp_2};
    use crate::region_4_vap_liq_equilibrium::sat_temp_4;
    use uom::si::f64::{AvailableEnergy, MassFlux, Pressure, ThermodynamicTemperature};
    use uom::si::thermodynamic_temperature::kelvin;

    let t_t = sat_temp_4(p_t);
    if t_t.get::<kelvin>() > 623.15 {
        return None;
    }
    let s0 = s_tp_1(t_t, p_t).value;
    let v_f = v_tp_1(t_t, p_t).value;
    let h_f = h_tp_1(t_t, p_t).value;
    let v_dome = |p_pa: f64| -> f64 {
        let p = Pressure::new::<uom::si::pressure::pascal>(p_pa);
        let ts = sat_temp_4(p);
        let (s_f, s_g) = (s_tp_1(ts, p).value, s_tp_2(ts, p).value);
        let x = (s0 - s_f) / (s_g - s_f);
        let (vf, vg) = (v_tp_1(ts, p).value, v_tp_2(ts, p).value);
        vf + x * (vg - vf)
    };
    let pt = p_t.value;
    let d = 1e-6 * pt;
    let dvdp = (3.0 * v_f - 4.0 * v_dome(pt - d) + v_dome(pt - 2.0 * d)) / (2.0 * d);
    let g_t = (-1.0 / dvdp).sqrt(); // IEEE-754 correctly rounded, no libm
    let u_t = g_t * v_f;
    let h0 = h_f + 0.5 * u_t * u_t;

    let (mut t, mut p) = (t_t.value, pt + (h0 - h_f) / v_f);
    for _ in 0..50 {
        let tk = ThermodynamicTemperature::new::<kelvin>(t);
        let pk = Pressure::new::<uom::si::pressure::pascal>(p);
        let (cp, v, a) = (cp_tp_1(tk, pk).value, v_tp_1(tk, pk).value, alpha_v_tp_1(tk, pk).value);
        let f1 = s_tp_1(tk, pk).value - s0;
        let f2 = h_tp_1(tk, pk).value - h0;
        let (j11, j12, j21, j22) = (cp / t, -v * a, cp, v * (1.0 - t * a));
        let det = j11 * j22 - j12 * j21;
        let dt = (f1 * j22 - j12 * f2) / det;
        let dp = (j11 * f2 - j21 * f1) / det;
        t -= dt;
        p -= dp;
        // p is set by h − h_f ≈ v Δp; at round-off of h (≈ 1e-16 · 3e5
        // J/kg) it cannot settle below ≈ 1e-11 relative at 5 psia, so 1e-10.
        if dt.abs() <= 1e-12 * t && dp.abs() <= 1e-10 * p {
            return Some((
                Pressure::new::<uom::si::pressure::pascal>(p),
                AvailableEnergy::new::<uom::si::available_energy::joule_per_kilogram>(h0),
                MassFlux::new::<uom::si::mass_flux::kilogram_per_square_meter_second>(g_t),
            ));
        }
    }
    None
}
